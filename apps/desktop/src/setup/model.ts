/** The model section: provider, model name, base URL and the key. */
import { invoke } from "@tauri-apps/api/core";
import { PROVIDERS, subscriptionUse } from "../brain/llm";
import type { CliStatus } from "../generated/CliStatus";
import type { KeySource } from "../generated/KeySource";
import type { ModelConfig } from "../generated/ModelConfig";
import { t, type Key } from "../i18n";
import { renderAudit } from "./audit";
import { busy, el, note } from "./dom";
import { currentPersonaPrompt } from "./pet";
import { send } from "./send";

export function modelFromForm(): ModelConfig {
  const provider = el<HTMLSelectElement>("m-provider").value;
  return {
    provider,
    model: el<HTMLInputElement>("m-model").value.trim(),
    base_url: provider === "ollama" || provider === "custom" ? el<HTMLInputElement>("m-url").value.trim() || null : null,
  };
}

/** Whether the user's own Claude Code can be run and is signed in,
 *  and how much of the plan's window the pet has seen used. */
async function renderCli() {
  const box = el("m-cli");
  box.textContent = t("model.cli.checking");
  const st = await invoke<CliStatus>("claude_cli_status").catch(() => null);
  const parts: string[] = [];
  if (!st?.path) parts.push(t("model.cli.missing"));
  else {
    parts.push(t("model.cli.found", { path: st.path }));
    if (!st.logged_in) parts.push(t("model.cli.nologin"));
    else parts.push(st.plan ? t("model.cli.plan", { plan: st.plan }) : t("model.cli.login", { auth: st.auth }));
  }
  const use = subscriptionUse();
  if (use !== null) parts.push(t("model.cli.use", { pct: Math.round(use * 100) }));
  box.textContent = parts.join(" · ");
}

function reflectProvider() {
  const provider = el<HTMLSelectElement>("m-provider").value;
  const p = PROVIDERS[provider];
  const urlRow = provider === "ollama" || provider === "custom";
  const cli = provider === "claude-code";
  el("m-url").hidden = !urlRow;
  el("m-url-label").hidden = !urlRow;
  for (const id of ["m-key-label", "m-key-row", "m-key-hint", "m-key-remove"]) el(id).hidden = cli;
  el("m-cli").hidden = !cli;
  el("m-cli-hint").hidden = !cli;
  if (cli) void renderCli();
  el<HTMLInputElement>("m-model").placeholder = p?.defaultModel ? t("model.default", { model: p.defaultModel }) : "";
  el<HTMLInputElement>("m-key").placeholder = p && !p.needsKey ? t("model.key.optional") : "";
}

async function renderKeySource() {
  const src = await invoke<KeySource>("key_source");
  el("m-key-source").textContent = t(`model.key.${src}` as Key);
  el<HTMLButtonElement>("m-key-remove").disabled = src !== "keyring";
}

/** Asks the configured model for one line in the current persona and
 *  shows it under `msgId`. Shared by "Test" and "Try a line". */
export function tryLine(msgId: string) {
  return busy(msgId, async () => {
    const req = { purpose: "persona" as const, system: currentPersonaPrompt(), user: t("model.test.user"), maxTokens: 200 };
    const text = await send(modelFromForm, req, () => void renderAudit());
    if (text !== null) note(msgId, t("model.test.ok", { text: text.trim() }));
    if (modelFromForm().provider === "claude-code") void renderCli();
  });
}

export async function mountModel() {
  const select = el<HTMLSelectElement>("m-provider");
  select.replaceChildren();
  for (const [id, p] of Object.entries(PROVIDERS)) {
    const o = document.createElement("option");
    o.value = id;
    o.textContent = p.label;
    select.append(o);
  }
  const cfg = await invoke<ModelConfig>("get_model");
  select.value = PROVIDERS[cfg.provider] ? cfg.provider : "deepseek";
  el<HTMLInputElement>("m-model").value = cfg.model;
  el<HTMLInputElement>("m-url").value = cfg.base_url ?? "";
  reflectProvider();
  await renderKeySource();
  select.addEventListener("change", reflectProvider);

  el("m-save").addEventListener("click", () =>
    busy("m-msg", async () => {
      await invoke("set_model", { model: modelFromForm() });
      const key = el<HTMLInputElement>("m-key");
      if (key.value.trim()) {
        await invoke("set_api_key", { key: key.value });
        key.value = "";
      }
      await renderKeySource();
      note("m-msg", t("model.saved"));
    }),
  );
  el("m-key-remove").addEventListener("click", () =>
    busy("m-msg", async () => {
      await invoke("clear_api_key");
      await renderKeySource();
      note("m-msg", t("model.key.removed"));
    }),
  );
  el("m-test").addEventListener("click", () => void tryLine("m-msg"));
}
