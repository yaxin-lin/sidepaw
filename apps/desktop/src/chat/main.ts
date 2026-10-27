/** The chat window: talk to the pet about what the agents did. Each
 *  message carries the current context (sessions, today's counters,
 *  recent events); the line under the log says how much, and opens to
 *  show the exact text, as the permissions contract promises. */
import { invoke } from "@tauri-apps/api/core";
import { buildContext, CONTEXT_EVENTS } from "../brain/context";
import { loadPersona } from "../brain/files";
import { approxTokens, resolve, stream, type Turn } from "../brain/llm";
import { compileSystemPrompt } from "../brain/persona";
import type { DayReport } from "../generated/DayReport";
import type { EventRow } from "../generated/EventRow";
import type { ModelConfig } from "../generated/ModelConfig";
import type { Snapshot } from "../generated/Snapshot";
import { applyToDom, detectLang, initLang, onLangChange, t } from "../i18n";
import { DEFAULT_PACK } from "../pack";

const HISTORY_TURNS = 10;
const REPLY_TOKENS = 1024;
const CONTEXT_REFRESH_MS = 5000;

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function add(kind: "user" | "pet" | "err", text: string): HTMLDivElement {
  const div = document.createElement("div");
  div.className = `turn ${kind}`;
  div.textContent = text;
  const log = el("log");
  log.append(div);
  log.scrollTop = log.scrollHeight;
  return div;
}

async function context(): Promise<string> {
  const [snapshot, report, events] = await Promise.all([
    invoke<Snapshot>("snapshot"),
    invoke<DayReport>("report_today"),
    invoke<EventRow[]>("recent_events", { limit: CONTEXT_EVENTS }),
  ]);
  return buildContext({ snapshot, report, events });
}

async function main() {
  await initLang();
  applyToDom();
  await onLangChange(() => location.reload());
  const cfg = await invoke<ModelConfig>("get_model");
  try {
    resolve(cfg);
  } catch (e) {
    el("notice-text").textContent = `${t("chat.no_model")} ${String(e)}`;
    el("notice").hidden = false;
    el("notice-setup").addEventListener("click", () => void invoke("open_setup"));
    el<HTMLTextAreaElement>("input").disabled = true;
    el<HTMLButtonElement>("send").disabled = true;
    return;
  }

  const { persona, md } = await loadPersona(DEFAULT_PACK);
  const personaPrompt = compileSystemPrompt(persona, md, detectLang());
  const history: Turn[] = [];
  let ctx = "";
  let busy = false;

  const refresh = async () => {
    if (busy) return;
    ctx = await context();
    const n = Math.min(CONTEXT_EVENTS, ctx.split("\n").length);
    el("status-line").textContent = t("chat.sending", { n, tokens: approxTokens(personaPrompt + ctx) });
    el("status-text").textContent = `[system]\n${personaPrompt}\n\n${ctx}`;
  };
  await refresh();
  setInterval(() => void refresh(), CONTEXT_REFRESH_MS);

  const input = el<HTMLTextAreaElement>("input");
  input.placeholder = t("chat.placeholder");
  input.focus();
  const send = async () => {
    const text = input.value.trim();
    if (!text || busy) return;
    busy = true;
    input.value = "";
    el<HTMLButtonElement>("send").disabled = true;
    add("user", text);
    const reply = add("pet", "…");
    let got = "";
    try {
      const key = await invoke<string | null>("api_key");
      const full = await stream(
        cfg,
        {
          purpose: "chat",
          system: `${personaPrompt}\n\nWhat you can see right now:\n${ctx}`,
          history: history.slice(-HISTORY_TURNS),
          user: text,
          maxTokens: REPLY_TOKENS,
        },
        key,
        (d) => {
          got += d;
          reply.textContent = got;
          el("log").scrollTop = el("log").scrollHeight;
        },
      );
      reply.textContent = full;
      history.push({ role: "user", content: text }, { role: "assistant", content: full });
    } catch (e) {
      reply.remove();
      add("err", t("chat.failed", { error: String(e) }));
    } finally {
      busy = false;
      el<HTMLButtonElement>("send").disabled = false;
      input.focus();
      void refresh();
    }
  };
  el<HTMLFormElement>("form").addEventListener("submit", (e) => {
    e.preventDefault();
    void send();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      void send();
    }
  });
}

main().catch((e: unknown) => add("err", String(e)));
