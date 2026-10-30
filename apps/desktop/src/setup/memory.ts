/** The memory section: everything the pet remembers, each editable,
 *  deletable, and kept forever, for a month or for a week. */
import { invoke } from "@tauri-apps/api/core";
import type { ChatTurn } from "../generated/ChatTurn";
import type { Memory } from "../generated/Memory";
import { t, type Key } from "../i18n";
import { busy, el, note } from "./dom";

const RETENTIONS = ["forever", "30d", "7d"] as const;

function retentionSelect(value: string, onChange: (v: string) => void, s = document.createElement("select")): HTMLSelectElement {
  s.style.width = "auto";
  s.replaceChildren();
  for (const r of RETENTIONS) {
    const o = document.createElement("option");
    o.value = r;
    o.textContent = t(`mem.ret.${r}` as Key);
    o.selected = r === value;
    s.append(o);
  }
  s.addEventListener("change", () => onChange(s.value));
  return s;
}

const WEEK_HOURS = 24 * 7;

/** The chat log is memory too (ADR 11), so it is counted here and can
 *  be forgotten here, not only from the chat window. */
async function renderChat() {
  const turns = await invoke<ChatTurn[]>("chat_turns", { hours: WEEK_HOURS }).catch(() => [] as ChatTurn[]);
  el("mem-chat").textContent = t("mem.chat", { n: turns.length });
  el("mem-chat-clear").hidden = turns.length === 0;
}

async function render() {
  await renderChat();
  const list = await invoke<Memory[]>("memories");
  const body = el<HTMLTableSectionElement>("mem-rows");
  body.replaceChildren();
  el("mem-empty").hidden = list.length > 0;
  for (const m of list) {
    const tr = document.createElement("tr");
    const text = document.createElement("td");
    const input = document.createElement("input");
    input.type = "text";
    input.value = m.body;
    input.maxLength = 200;
    const save = (retention = m.retention) =>
      busy("mem-msg", async () => {
        await invoke("update_memory", { id: m.id, body: input.value, retention });
        note("mem-msg", t("mem.saved"));
        await render();
      });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.isComposing) void save();
    });
    input.addEventListener("change", () => void save());
    text.append(input);
    const source = document.createElement("td");
    source.className = "msg";
    source.textContent = t(m.source === "user" ? "mem.src.user" : "mem.src.observed");
    const ret = document.createElement("td");
    ret.append(retentionSelect(m.retention, (v) => void save(v)));
    const del = document.createElement("td");
    const b = document.createElement("button");
    b.textContent = t("mem.delete");
    b.addEventListener("click", () =>
      busy("mem-msg", async () => {
        await invoke("delete_memory", { id: m.id });
        note("mem-msg", "");
        await render();
      }),
    );
    del.append(b);
    tr.append(text, source, ret, del);
    body.append(tr);
  }
}

export async function mountMemory() {
  const select = retentionSelect("forever", () => {}, el<HTMLSelectElement>("mem-new-ret"));
  const input = el<HTMLInputElement>("mem-new");
  input.placeholder = t("mem.placeholder");
  const add = () =>
    busy("mem-msg", async () => {
      await invoke("add_memory", { source: "user", body: input.value, retention: select.value });
      input.value = "";
      note("mem-msg", "");
      await render();
    });
  el("mem-add").addEventListener("click", () => void add());
  el("mem-chat-clear").addEventListener("click", () =>
    busy("mem-msg", async () => {
      await invoke("clear_chat_turns");
      await renderChat();
    }),
  );
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.isComposing) void add();
  });
  await render();
}
