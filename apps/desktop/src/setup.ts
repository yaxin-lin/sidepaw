import { invoke } from "@tauri-apps/api/core";
import type { DayReport } from "./generated/DayReport";
import type { Report } from "./generated/Report";
import { applyToDom, t } from "./i18n";
import { mountBrain } from "./setup-brain";

const HOST = "claude-code";

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function render(r: Report) {
  const badge = el<HTMLSpanElement>("cc-badge");
  badge.textContent = t(`status.${r.status}`);
  badge.className = `badge ${r.status}`;
  el("cc-path").textContent = r.path;
  el("cc-explain").textContent = t(`cc.${r.status}`);
  el<HTMLPreElement>("cc-preview").textContent = r.preview;
  el<HTMLButtonElement>("cc-install").textContent = r.status === "stale" ? t("cc.update") : t("cc.install");
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

const LEVELS = [0, 1, 2, 3, 4] as const;

async function renderLevels() {
  const box = el("levels");
  const current = await invoke<number>("get_level");
  box.innerHTML = "";
  for (const n of LEVELS) {
    const name = t(`level.${n}`);
    const text = t(`level.${n}.text`);
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
    msg.textContent = t("level.saved", { n });
  } catch (e) {
    msg.className = "msg err";
    msg.textContent = String(e);
  }
}

async function main() {
  applyToDom();
  document.title = t("setup.title");
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
        msg.textContent = t("notify.sent");
      },
      (e) => {
        msg.className = "msg err";
        msg.textContent = String(e);
      },
    );
  });
  await run("host_status", "");
  const act = el<HTMLInputElement>("cc-act");
  act.checked = await invoke<boolean>("get_host_act", { hostName: HOST });
  act.addEventListener("change", () => {
    invoke("set_host_act", { hostName: HOST, act: act.checked }).then(
      () => {
        el("cc-msg").className = "msg";
        el("cc-msg").textContent = t("act.saved");
      },
      (e) => {
        el("cc-msg").className = "msg err";
        el("cc-msg").textContent = String(e);
      },
    );
  });
  el("cc-install").addEventListener("click", () => void run("host_install", t("cc.written")));
  el("cc-remove").addEventListener("click", () => void run("host_remove", t("cc.removed")));
  await mountBrain();
}

void main();
