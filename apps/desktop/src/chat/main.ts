/** The chat window: talk to the pet about what the agents did. Each
 *  message carries the current context (sessions, today's counters,
 *  recent events); the line under the log says how much, and opens to
 *  show the exact text, as the permissions contract promises. The
 *  last day of turns, bubble lines included, is back when the window
 *  opens (ADR 11), so a bubble line can be answered here. */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { bubbleBlock, RELOAD_HOURS, toHistory } from "../brain/chatlog";
import { buildContext, CONTEXT_EVENTS } from "../brain/context";
import { activePack, loadPersona } from "../brain/files";
import { approxTokens, resolve, stream, type Turn } from "../brain/llm";
import { directRemember, extractRemember, hidePartialMarker, memoryBlock } from "../brain/memory";
import { compileSystemPrompt } from "../brain/persona";
import type { ChatTurn } from "../generated/ChatTurn";
import type { Memory } from "../generated/Memory";
import type { DayReport } from "../generated/DayReport";
import type { EventRow } from "../generated/EventRow";
import type { ModelConfig } from "../generated/ModelConfig";
import type { Snapshot } from "../generated/Snapshot";
import { applyToDom, detectLang, initLang, onLangChange, t } from "../i18n";

const HISTORY_TURNS = 10;
const REPLY_TOKENS = 1024;
const CONTEXT_REFRESH_MS = 5000;

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function add(kind: "user" | "pet" | "err" | "note", text: string, viaBubble = false): HTMLDivElement {
  const div = document.createElement("div");
  div.className = `turn ${kind}`;
  if (viaBubble) {
    div.classList.add("bubble");
    const via = document.createElement("span");
    via.className = "via";
    via.textContent = t("chat.via_bubble");
    div.append(via, document.createTextNode(text));
  } else div.textContent = text;
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
  // Without a model the window still takes "记住…", which needs none.
  let hasModel = true;
  try {
    resolve(cfg);
  } catch (e) {
    hasModel = false;
    el("notice-text").textContent = `${t("chat.no_model")} ${String(e)}`;
    el("notice").hidden = false;
    el("notice-setup").addEventListener("click", () => void invoke("open_setup"));
  }

  const { persona, md } = await loadPersona(await activePack());
  const personaPrompt = compileSystemPrompt(persona, md, detectLang());
  // What was said in the last day, in the window and in the bubble.
  let turns = await invoke<ChatTurn[]>("chat_turns", { hours: RELOAD_HOURS }).catch((e: unknown) => {
    console.warn(`chat turns: ${String(e)}`);
    return [] as ChatTurn[];
  });
  for (const turn of turns) add(turn.role === "user" ? "user" : "pet", turn.body, turn.channel === "bubble");
  const history: Turn[] = toHistory(turns);
  // Stored redacted; the model saw the original once. A failure only
  // loses the continuation, not the reply.
  const keep = async (role: "user" | "pet", channel: "chat" | "bubble", body: string) => {
    try {
      turns.push(await invoke<ChatTurn>("add_chat_turn", { role, channel, body }));
    } catch (e) {
      console.warn(`keep turn: ${String(e)}`);
    }
  };
  // A line said in the bubble while this window is open lands here too.
  await listen<ChatTurn>("chat-turn", (e) => {
    if (e.payload.channel !== "bubble") return;
    turns.push(e.payload);
    add("pet", e.payload.body, true);
  });
  let ctx = "";
  let memories = "";
  let busy = false;

  const refresh = async () => {
    if (busy) return;
    [ctx, memories] = await Promise.all([context(), invoke<Memory[]>("memories").then(memoryBlock, () => memoryBlock([]))]);
    const n = Math.min(CONTEXT_EVENTS, ctx.split("\n").length);
    el("status-line").textContent = t("chat.sending", { n, tokens: approxTokens(personaPrompt + memories + ctx) });
    el("status-text").textContent = `[system]\n${system()}`;
  };
  const system = () => [personaPrompt, memories, bubbleBlock(turns), `What you can see right now:\n${ctx}`].filter((p) => p.length > 0).join("\n\n");
  const remembered = (fact: string) => {
    const note = add("note", t("chat.remembered", { fact }));
    note.title = t("chat.remembered.manage");
    note.addEventListener("click", () => void invoke("open_setup"));
  };
  await refresh();
  setInterval(() => void refresh(), CONTEXT_REFRESH_MS);
  el("forget").addEventListener("click", () =>
    void invoke("clear_chat_turns").then(
      () => {
        turns = [];
        history.length = 0;
        el("log").replaceChildren();
        add("note", t("chat.forgot"));
        void refresh();
      },
      (e: unknown) => add("err", t("chat.failed", { error: String(e) })),
    ),
  );

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
    const direct = directRemember(text);
    if (direct !== null) {
      try {
        await invoke("add_memory", { source: "user", body: direct, retention: "forever" });
        remembered(direct);
      } catch (e) {
        add("err", t("chat.failed", { error: String(e) }));
      }
      busy = false;
      el<HTMLButtonElement>("send").disabled = false;
      void refresh();
      return;
    }
    if (!hasModel) {
      add("err", t("chat.no_model"));
      busy = false;
      el<HTMLButtonElement>("send").disabled = false;
      return;
    }
    await keep("user", "chat", text);
    const reply = add("pet", "…");
    let got = "";
    try {
      const key = await invoke<string | null>("api_key");
      const full = await stream(
        cfg,
        {
          purpose: "chat",
          system: system(),
          history: history.slice(-HISTORY_TURNS),
          user: text,
          maxTokens: REPLY_TOKENS,
        },
        key,
        (d) => {
          got += d;
          reply.textContent = hidePartialMarker(got);
          el("log").scrollTop = el("log").scrollHeight;
        },
      );
      const { reply: shown, facts } = extractRemember(full);
      reply.textContent = shown;
      history.push({ role: "user", content: text }, { role: "assistant", content: shown });
      await keep("pet", "chat", shown);
      // Observed facts fade after a month unless the user keeps them.
      for (const fact of facts) {
        await invoke("add_memory", { source: "observed", body: fact, retention: "30d" }).then(
          () => remembered(fact),
          (e: unknown) => console.warn(String(e)),
        );
      }
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
