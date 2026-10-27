/** The only code that talks to a model (ADR 7). One provider at a time
 *  (ADR 5): an OpenAI-compatible endpoint (DeepSeek, OpenAI, Ollama,
 *  anything with a base URL) or Anthropic through its SDK. Every call
 *  has a preview the UI can show first and leaves an audit row. */
import Anthropic from "@anthropic-ai/sdk";
import { invoke } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import type { Model } from "../generated/Model";

export type Purpose = "chat" | "lines" | "persona";

export interface Request {
  purpose: Purpose;
  system: string;
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

export const PROVIDERS: Record<string, Provider> = {
  deepseek: { label: "DeepSeek", baseUrl: "https://api.deepseek.com/v1", defaultModel: "deepseek-chat", needsKey: true, local: false },
  openai: { label: "OpenAI", baseUrl: "https://api.openai.com/v1", defaultModel: "gpt-5-mini", needsKey: true, local: false },
  anthropic: { label: "Anthropic", baseUrl: "https://api.anthropic.com", defaultModel: "claude-opus-5-5", needsKey: true, local: false },
  ollama: { label: "Ollama", baseUrl: "http://127.0.0.1:11434/v1", defaultModel: "", needsKey: false, local: true },
  custom: { label: "OpenAI-compatible URL", baseUrl: "", defaultModel: "", needsKey: false, local: false },
};

export interface Resolved {
  provider: string;
  model: string;
  baseUrl: string;
  local: boolean;
}

/** Fills in defaults and rejects a config that cannot be used, with a
 *  message the setup window shows as is. */
export function resolve(cfg: Model): Resolved {
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
  for (const ch of text) if (/[　-鿿豈-﫿]/.test(ch)) cjk++;
  return Math.ceil(cjk + (text.length - cjk) / 4);
}

export function preview(cfg: Model, req: Request): Preview {
  const r = resolve(cfg);
  const text = `[system]\n${req.system}\n\n[user]\n${req.user}`;
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

export async function sha256Hex(text: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Request body for an OpenAI-compatible `chat/completions` call. OpenAI
 *  itself rejects `max_tokens` on current models; everyone else still
 *  wants it. */
export function openaiBody(r: Resolved, req: Request): Record<string, unknown> {
  const body: Record<string, unknown> = {
    model: r.model,
    messages: [
      { role: "system", content: req.system },
      { role: "user", content: req.user },
    ],
    stream: false,
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

/** One request, one text reply. The audit row is written before the
 *  request goes out so a crash mid-flight still leaves a record. */
export async function complete(cfg: Model, req: Request, key: string | null, deps: Deps = live): Promise<string> {
  const r = resolve(cfg);
  const p = PROVIDERS[r.provider];
  if (p.needsKey && !key) throw new Error(`${p.label} needs an API key`);
  const pv = preview(cfg, req);
  await deps.audit(r.provider, req.purpose, pv.bytes, await sha256Hex(pv.text));

  if (r.provider === "anthropic") {
    const client = new Anthropic({
      apiKey: key ?? "",
      fetch: deps.fetch,
      dangerouslyAllowBrowser: true,
      maxRetries: 1,
    });
    const res = await client.beta.messages.create({
      model: r.model,
      max_tokens: req.maxTokens,
      system: req.system,
      messages: [{ role: "user", content: req.user }],
      output_config: { effort: "low" },
      betas: ["server-side-fallback-2026-07-01"],
      fallbacks: "default",
    });
    if (res.stop_reason === "refusal") throw new Error("the model declined this request");
    const text = res.content
      .filter((b): b is Anthropic.Beta.BetaTextBlock => b.type === "text")
      .map((b) => b.text)
      .join("");
    if (!text) throw new Error("the reply had no text");
    return text;
  }

  const headers: Record<string, string> = { "content-type": "application/json" };
  if (key) headers.authorization = `Bearer ${key}`;
  const res = await deps.fetch(`${r.baseUrl}/chat/completions`, {
    method: "POST",
    headers,
    body: JSON.stringify(openaiBody(r, req)),
  });
  if (!res.ok) throw new Error(`${p.label}: HTTP ${res.status} ${await errorText(res)}`);
  return openaiText(await res.json());
}
