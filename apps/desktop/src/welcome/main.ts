/** The first-run window: connect Claude Code, pick a level, see the
 *  first event arrive. Everything here is also in the setup window;
 *  this is the short path through it. */
import { invoke } from "@tauri-apps/api/core";
import type { Report } from "../generated/Report";
import type { Snapshot } from "../generated/Snapshot";
import type { SessionView } from "../generated/SessionView";
import type { UiLanguage } from "../generated/UiLanguage";
import { applyToDom, initLang, onLangChange, setLang, t, type Key } from "../i18n";
import { el, note } from "../setup/dom";

const HOST = "claude-code";
/** The levels a newcomer chooses between; 0 and 4 are in Setup. */
const CHOICES = [1, 2, 3] as const;
const POLL_MS = 1000;

function mark(id: string, state: "ok" | "later" | "") {
  el(id).className = state;
}

function renderConnect(r: Report, found: boolean) {
  const installed = r.status === "installed";
  el("w-cc-path").textContent = r.path;
  el<HTMLPreElement>("w-cc-preview").textContent = r.preview;
  el("w-cc-text").textContent = installed
    ? t("welcome.connected")
    : found
      ? t("welcome.found")
      : t("welcome.not_found");
  const button = el<HTMLButtonElement>("w-install");
  button.hidden = installed;
  button.textContent = r.status === "stale" ? t("cc.update") : t("welcome.install");
  mark("step-connect", installed ? "ok" : "");
  mark("step-try", installed ? "" : "later");
  if (installed) watchFirstEvent();
}

async function connect(found: boolean) {
  try {
    renderConnect(await invoke<Report>("host_install", { hostName: HOST }), found);
    note("w-cc-msg", t("cc.written"));
  } catch (e) {
    note("w-cc-msg", String(e), true);
  }
}

async function renderLevels() {
  const box = el("w-levels");
  const current = await invoke<number>("get_level");
  box.replaceChildren();
  for (const n of CHOICES) {
    const label = document.createElement("label");
    label.className = "choice";
    const input = document.createElement("input");
    input.type = "radio";
    input.name = "level";
    input.checked = n === current;
    input.addEventListener("change", () => {
      invoke("set_level", { level: n }).then(
        () => note("w-level-msg", t("level.saved", { n })),
        (e) => note("w-level-msg", String(e), true),
      );
    });
    const name = document.createElement("b");
    name.textContent = `${t(`level.${n}`)}${n === 1 ? ` · ${t("welcome.default")}` : ""}`;
    const text = document.createElement("span");
    text.className = "msg";
    text.textContent = t(`welcome.level.${n}` as Key);
    label.append(input, name, text);
    box.append(label);
  }
}

let watching = false;

/** The proof that it works end to end: a Claude Code session shows up. */
function watchFirstEvent() {
  if (watching) return;
  watching = true;
  note("w-try-msg", t("welcome.waiting"));
  const tick = async () => {
    const snap = await invoke<Snapshot>("snapshot").catch(() => null);
    const s = snap?.sessions.find((v: SessionView) => v.key.startsWith(`${HOST}:`));
    if (s) {
      note("w-try-msg", t("welcome.heard", { state: t(`state.${s.state}`) }));
      el("w-try-msg").classList.add("ok");
      mark("step-try", "ok");
      return;
    }
    setTimeout(() => void tick(), POLL_MS);
  };
  void tick();
}

async function renderLanguage() {
  const select = el<HTMLSelectElement>("lang");
  const current = await invoke<UiLanguage>("get_ui_language");
  for (const l of ["system", "zh-CN", "en"] as UiLanguage[]) {
    const o = document.createElement("option");
    o.value = l;
    o.textContent = t(`lang.${l}` as Key);
    o.selected = l === current;
    select.append(o);
  }
  select.addEventListener("change", () => void setLang(select.value as UiLanguage));
  await onLangChange(() => location.reload());
}

async function main() {
  await initLang();
  applyToDom();
  await renderLanguage();
  const found = await invoke<boolean>("host_found", { hostName: HOST }).catch(() => false);
  renderConnect(await invoke<Report>("host_status", { hostName: HOST }), found);
  el("w-install").addEventListener("click", () => void connect(found));
  await renderLevels();
  const finish = () => invoke("finish_welcome");
  el("w-done").addEventListener("click", () => void finish());
  el("w-later").addEventListener("click", () => void finish());
  el("w-setup").addEventListener("click", () => {
    void invoke("open_setup", { section: "claude-code" }).then(finish);
  });
}

void main();
