/** The only code that talks to a model (ADR 7). One provider at a time
 *  (ADR 5): an OpenAI-compatible endpoint (DeepSeek, OpenAI, Ollama,
 *  anything with a base URL) or Anthropic through its SDK. Every call
 *  has a preview the UI can show first and leaves an audit row. */
import Anthropic from "@anthropic-ai/sdk";
import { invoke } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import type { ModelConfig } from "../generated/ModelConfig";
import { sha256Hex } from "./digest";

export type Purpose = "chat" | "lines" | "persona" | "nudge";

export interface Turn {
  role: "user" | "assistant";
  content: string;
}

export interface Request {
  purpose: Purpose;
  system: string;
  /** Earlier turns of a chat, oldest first; empty for one-shot calls. */
  history?: Turn[];
  user: string;
  maxTokens: number;
}

export interface Provider {
  label: string;
  baseUrl: string;
  defaultModel: string;
  needsKey: boolean;
  /** Ollama and anything on this machine: nothing leaves the box, so
   *  the preview-before-send step is skipped per the permissions contract. */
  local: boolean;
}

/** Default models are each provider's cheap general model, except
 *  Anthropic where the API guidance is the current Opus; a user who
 *  wants Haiku names it in the model field. */
export const PROVIDERS: Record<string, Provider> = {
  deepseek: { label: "DeepSeek", baseUrl: "https://api.deepseek.com/v1", defaultModel: "deepseek-chat", needsKey: true, local: false },
  openai: { label: "OpenAI", baseUrl: "https://api.openai.com/v1", defaultModel: "gpt-5-mini", needsKey: true, local: false },
  anthropic: { label: "Anthropic", baseUrl: "https://api.anthropic.com", defaultModel: "claude-opus-5-5", needsKey: true, local: false },
  ollama: { label: "Ollama", baseUrl: "http://127.0.0.1:11434/v1", defaultModel: "", needsKey: false, local: true },
  custom: { label: "OpenAI-compatible URL", baseUrl: "", defaultModel: "", needsKey: false, local: false },
};

/** A config with defaults filled in: where a request goes. */
export interface Endpoint {
  provider: string;
  model: string;
  baseUrl: string;
  local: boolean;
}

/** Fills in defaults and rejects a config that cannot be used, with a
 *  message the setup window shows as is. */
export function resolve(cfg: ModelConfig): Endpoint {
  const p = PROVIDERS[cfg.provider];
  if (!p) throw new Error(cfg.provider ? `unknown provider "${cfg.provider}"` : "no model provider configured");
  const model = cfg.model.trim() || p.defaultModel;
  if (!model) throw new Error(`${p.label} needs a model name`);
  let baseUrl = p.baseUrl;
  if (cfg.provider === "ollama" || cfg.provider === "custom") {
    baseUrl = (cfg.base_url ?? "").trim().replace(/\/+$/, "") || p.baseUrl;
  }
  if (!baseUrl) throw new Error("a custom provider needs a base URL");
  const local = p.local || /^https?:\/\/(127\.0\.0\.1|localhost|\[::1\])(:|\/|$)/.test(baseUrl);
  return { provider: cfg.provider, model, baseUrl, local };
}

export interface Preview {
  provider: string;
  model: string;
  purpose: Purpose;
  local: boolean;
  /** Exactly the text the model receives, system part first. */
  text: string;
  bytes: number;
  approxTokens: number;
}

/** Rough and deliberately pessimistic: CJK is about one token per
 *  character, everything else about four characters per token. */
export function approxTokens(text: string): number {
  let cjk = 0;
  for (const ch of text) if (CJK.test(ch)) cjk++;
  return Math.ceil(cjk + (text.length - cjk) / 4);
}

// U+3000 (CJK punctuation) through the unified ideographs, plus the
// compatibility ideographs block.
const CJK = /[\u3000-\u9fff\uf900-\ufaff]/;

export function preview(cfg: ModelConfig, req: Request): Preview {
  const r = resolve(cfg);
  const turns = (req.history ?? []).map((t) => `[${t.role}]\n${t.content}\n\n`).join("");
  const text = `[system]\n${req.system}\n\n${turns}[user]\n${req.user}`;
  return {
    provider: r.provider,
    model: r.model,
    purpose: req.purpose,
    local: r.local,
    text,
    bytes: new TextEncoder().encode(text).length,
    approxTokens: approxTokens(text),
  };
}

/** Request body for an OpenAI-compatible `chat/completions` call. OpenAI
 *  itself rejects `max_tokens` on current models; everyone else still
 *  wants it. */
export function openaiBody(r: Endpoint, req: Request, stream = false): Record<string, unknown> {
  const body: Record<string, unknown> = {
    model: r.model,
    messages: [{ role: "system", content: req.system }, ...(req.history ?? []), { role: "user", content: req.user }],
    stream,
  };
  body[r.provider === "openai" ? "max_completion_tokens" : "max_tokens"] = req.maxTokens;
  return body;
}

export function openaiText(json: unknown): string {
  const choices = (json as { choices?: Array<{ message?: { content?: unknown } }> }).choices;
  const content = choices?.[0]?.message?.content;
  if (typeof content !== "string") throw new Error("the reply had no text");
  return content;
}

/** Text deltas in a run of server-sent events from a streaming
 *  `chat/completions` call. Takes whole lines only; the caller keeps
 *  the unterminated tail for the next chunk. */
export function sseDeltas(lines: string): string[] {
  const out: string[] = [];
  for (const line of lines.split("\n")) {
    if (!line.startsWith("data:")) continue;
    const data = line.slice(5).trim();
    if (!data || data === "[DONE]") continue;
    try {
      const json = JSON.parse(data) as { choices?: Array<{ delta?: { content?: unknown } }> };
      const delta = json.choices?.[0]?.delta?.content;
      if (typeof delta === "string" && delta) out.push(delta);
    } catch {
      // A malformed event is dropped rather than ending the reply.
    }
  }
  return out;
}

export interface Deps {
  fetch: typeof globalThis.fetch;
  audit: (provider: string, purpose: Purpose, bytes: number, sha256: string) => Promise<void>;
}

const live: Deps = {
  fetch: tauriFetch as unknown as typeof globalThis.fetch,
  audit: (provider, purpose, bytes, sha256) => invoke("audit_call", { provider, purpose, bytes, sha256 }),
};

async function errorText(res: Response): Promise<string> {
  const text = await res.text().catch(() => "");
  try {
    const j = JSON.parse(text) as { error?: { message?: string } | string };
    const m = typeof j.error === "string" ? j.error : j.error?.message;
    if (m) return m;
  } catch {
    // not JSON; fall through to the raw body
  }
  return text.slice(0, 300);
}

/** Everything before a request goes out: defaults filled, key checked,
 *  the audit row written. Written first so a crash mid-flight still
 *  leaves a record. */
async function prepare(cfg: ModelConfig, req: Request, key: string | null, deps: Deps): Promise<Endpoint> {
  const r = resolve(cfg);
  const p = PROVIDERS[r.provider];
  if (p.needsKey && !key) throw new Error(`${p.label} needs an API key`);
  const pv = preview(cfg, req);
  await deps.audit(r.provider, req.purpose, pv.bytes, await sha256Hex(pv.text));
  return r;
}

function anthropic(key: string | null, deps: Deps) {
  return new Anthropic({ apiKey: key ?? "", fetch: deps.fetch, dangerouslyAllowBrowser: true, maxRetries: 1 });
}

// Lines and chat replies are easy work, so the lowest effort; the
// fallback beta re-runs a refused request on another model inside the
// same call, which a pet should not surface as an error.
function anthropicParams(r: Endpoint, req: Request) {
  return {
    model: r.model,
    max_tokens: req.maxTokens,
    system: req.system,
    messages: [...(req.history ?? []), { role: "user" as const, content: req.user }],
    output_config: { effort: "low" as const },
    betas: ["server-side-fallback-2026-07-01"],
    fallbacks: "default" as const,
  };
}

function anthropicText(msg: Anthropic.Beta.BetaMessage): string {
  if (msg.stop_reason === "refusal") throw new Error("the model declined this request");
  const text = msg.content
    .filter((b): b is Anthropic.Beta.BetaTextBlock => b.type === "text")
    .map((b) => b.text)
    .join("");
  if (!text) throw new Error("the reply had no text");
  return text;
}

async function openaiFetch(r: Endpoint, req: Request, key: string | null, deps: Deps, stream: boolean): Promise<Response> {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (key) headers.authorization = `Bearer ${key}`;
  const res = await deps.fetch(`${r.baseUrl}/chat/completions`, {
    method: "POST",
    headers,
    body: JSON.stringify(openaiBody(r, req, stream)),
  });
  if (!res.ok) throw new Error(`${PROVIDERS[r.provider].label}: HTTP ${res.status} ${await errorText(res)}`);
  return res;
}

/** One request, one text reply. */
export async function complete(cfg: ModelConfig, req: Request, key: string | null, deps: Deps = live): Promise<string> {
  const r = await prepare(cfg, req, key, deps);
  if (r.provider === "anthropic") {
    return anthropicText(await anthropic(key, deps).beta.messages.create(anthropicParams(r, req)));
  }
  return openaiText(await (await openaiFetch(r, req, key, deps, false)).json());
}

/** The same call with the reply arriving in pieces; resolves to the
 *  whole text once it is complete. */
export async function stream(cfg: ModelConfig, req: Request, key: string | null, onDelta: (text: string) => void, deps: Deps = live): Promise<string> {
  const r = await prepare(cfg, req, key, deps);
  if (r.provider === "anthropic") {
    const s = anthropic(key, deps).beta.messages.stream(anthropicParams(r, req));
    s.on("text", onDelta);
    return anthropicText(await s.finalMessage());
  }
  const res = await openaiFetch(r, req, key, deps, true);
  if (!res.body) throw new Error("the reply had no body");
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let tail = "";
  let all = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    tail += decoder.decode(value, { stream: true });
    const cut = tail.lastIndexOf("\n");
    if (cut < 0) continue;
    for (const d of sseDeltas(tail.slice(0, cut))) {
      all += d;
      onDelta(d);
    }
    tail = tail.slice(cut + 1);
  }
  for (const d of sseDeltas(tail)) {
    all += d;
    onDelta(d);
  }
  if (!all) throw new Error("the reply had no text");
  return all;
}
