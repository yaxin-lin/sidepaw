import { invoke } from "@tauri-apps/api/core";
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
    const r = await invoke<Report>(cmd, { host: HOST });
    render(r);
    msg.textContent = done;
  } catch (e) {
    msg.className = "msg err";
    msg.textContent = String(e);
  }
}

async function main() {
  const port = await invoke<number>("server_port");
  el("server-addr").textContent = `127.0.0.1:${port}`;
  await run("host_status", "");
  el("cc-install").addEventListener("click", () => void run("host_install", "Written. A backup is next to the file."));
  el("cc-remove").addEventListener("click", () => void run("host_remove", "Removed sidepaw's hooks; everything else untouched."));
}

void main();
