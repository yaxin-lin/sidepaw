import { invoke } from "@tauri-apps/api/core";
import type { DayReport } from "./generated/DayReport";
import type { Report } from "./generated/Report";
import type { Status } from "./generated/Status";

const HOST = "claude-code";

const EXPLAIN: Record<Status, string> = {
  installed: "Hooks are installed and point at this server. Start a Claude Code session and the pet reacts.",
  stale: "Hooks are installed but point at an old port or token. Install again to update them.",
  absent: "Claude Code has a settings file but no sidepaw hooks yet.",
  no_file: "No Claude Code settings file yet; installing will create it.",
};

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function render(r: Report) {
  const badge = el<HTMLSpanElement>("cc-badge");
  badge.textContent = r.status.replace("_", " ");
  badge.className = `badge ${r.status}`;
  el("cc-path").textContent = r.path;
  el("cc-explain").textContent = EXPLAIN[r.status];
  el<HTMLPreElement>("cc-preview").textContent = r.preview;
  el<HTMLButtonElement>("cc-install").textContent = r.status === "stale" ? "Update hooks" : "Install hooks";
  el<HTMLButtonElement>("cc-remove").disabled = r.status === "absent" || r.status === "no_file";
}

async function run(cmd: "host_status" | "host_install" | "host_remove", done: string) {
  const msg = el("cc-msg");
  msg.className = "msg";
  try {
    const r = await invoke<Report>(cmd, { hostName: HOST });
    render(r);
    msg.textContent = done;
  } catch (e) {
    msg.className = "msg err";
    msg.textContent = String(e);
  }
}

function minutes(ms: number): string {
  if (ms < 60_000) return `${Math.round(ms / 1000)} s`;
  const m = Math.round(ms / 60_000);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
}

async function renderToday() {
  try {
    const r = await invoke<DayReport>("report_today");
    el("t-sessions").textContent = String(r.sessions);
    el("t-turns").textContent = String(r.turns);
    el("t-tools").textContent = String(r.tool_calls);
    el("t-errors").textContent = String(r.errors);
    el("t-waiting").textContent = minutes(r.waiting_ms);
    el("t-top").textContent = r.tools.length
      ? r.tools.slice(0, 3).map(([name, n]) => `${name} ×${n}`).join(", ")
      : "–";
  } catch (e) {
    el("t-top").textContent = String(e);
  }
}

const LEVELS: Array<[number, string, string]> = [
  [0, "Observe", "Only the state: thinking, tool, waiting, done. Animation and local lines."],
  [1, "Record", "Plus tool names, error types, token counts. Daily report and counters."],
  [2, "Read", "Plus file paths, command text, error messages (secrets redacted). Needed to answer \"why did that fail\"."],
  [3, "Notify", "Same as Read, plus system notifications when an agent waits for you, errors or finishes."],
  [4, "Act", "Same as Read, plus answering agents (approve, deny, send a message), each confirmed."],
];

async function renderLevels() {
  const box = el("levels");
  const current = await invoke<number>("get_level");
  box.innerHTML = "";
  for (const [n, name, text] of LEVELS) {
    const row = document.createElement("div");
    row.className = "level";
    const input = document.createElement("input");
    input.type = "radio";
    input.name = "level";
    input.id = `level-${n}`;
    input.checked = n === current;
    input.addEventListener("change", () => void setLevel(n));
    const label = document.createElement("label");
    label.htmlFor = input.id;
    label.textContent = `${n} ${name}`;
    const desc = document.createElement("span");
    desc.className = "msg";
    desc.textContent = text;
    row.append(input, label, desc);
    box.append(row);
  }
}

async function setLevel(n: number) {
  const msg = el("level-msg");
  try {
    await invoke("set_level", { level: n });
    msg.className = "msg";
    msg.textContent = `Level ${n} saved to config.toml.`;
  } catch (e) {
    msg.className = "msg err";
    msg.textContent = String(e);
  }
}

async function main() {
  const port = await invoke<number>("server_port");
  el("server-addr").textContent = `127.0.0.1:${port}`;
  await renderToday();
  setInterval(() => void renderToday(), 10_000);
  await renderLevels();
  el("notify-test").addEventListener("click", () => {
    const msg = el("level-msg");
    invoke("test_notification").then(
      () => {
        msg.className = "msg";
        msg.textContent = "Sent. Nothing shown? Notifications need level 3 for real events, and on macOS a bundled build.";
      },
      (e) => {
        msg.className = "msg err";
        msg.textContent = String(e);
      },
    );
  });
  await run("host_status", "");
  el("cc-install").addEventListener("click", () => void run("host_install", "Written. A backup is next to the file."));
  el("cc-remove").addEventListener("click", () => void run("host_remove", "Removed sidepaw's hooks; everything else untouched."));
}

void main();
