/** The model section: provider, model name, base URL and the key. */
import { invoke } from "@tauri-apps/api/core";
import { PROVIDERS } from "../brain/llm";
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

function reflectProvider() {
  const provider = el<HTMLSelectElement>("m-provider").value;
  const p = PROVIDERS[provider];
  const urlRow = provider === "ollama" || provider === "custom";
  el("m-url").hidden = !urlRow;
  el("m-url-label").hidden = !urlRow;
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
