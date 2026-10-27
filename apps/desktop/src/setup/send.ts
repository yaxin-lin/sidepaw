/** One remote model call from the setup window, with the preview the
 *  permissions contract promises before anything leaves the machine. */
import { invoke } from "@tauri-apps/api/core";
import { complete, preview, PROVIDERS, type Purpose, type Request } from "../brain/llm";
import type { ModelConfig } from "../generated/ModelConfig";
import { t } from "../i18n";
import { el } from "./dom";

/** The config to send with. The model section reads its own form so a
 *  "Test" tries what is typed, not what was last saved. */
export type ConfigSource = () => ModelConfig;

/** Resolves to null when the user cancels the preview. Local providers
 *  (Ollama, loopback URLs) skip it; every call is audited by `complete`. */
export async function send(config: ConfigSource, req: Request, afterwards: () => void): Promise<string | null> {
  const cfg = config();
  const pv = preview(cfg, req);
  if (!pv.local) {
    const ok = await confirm(pv.provider, pv.model, pv.purpose, pv.approxTokens, pv.bytes, pv.text);
    if (!ok) return null;
  }
  const key = await invoke<string | null>("api_key");
  try {
    return await complete(cfg, req, key);
  } finally {
    afterwards();
  }
}

function confirm(provider: string, model: string, purpose: Purpose, tokens: number, bytes: number, text: string): Promise<boolean> {
  return ask(
    t("send.title", { provider: PROVIDERS[provider]?.label ?? provider }),
    t("send.summary", { purpose: t(`purpose.${purpose}`), tokens, bytes, model }),
    text,
  );
}

/** The send dialog with any title and summary; resolves to whether the
 *  user pressed the primary button. */
export function ask(title: string, summary: string, text: string, go = t("send.go"), showText = false): Promise<boolean> {
  const dialog = el<HTMLDialogElement>("send");
  const details = dialog.querySelector("details");
  if (details) details.open = showText;
  el("send-title").textContent = title;
  el("send-summary").textContent = summary;
  el<HTMLPreElement>("send-text").textContent = text;
  el("send-go").textContent = go;
  return new Promise((resolve) => {
    const done = (ok: boolean) => {
      dialog.close();
      resolve(ok);
    };
    el("send-go").onclick = () => done(true);
    el("send-cancel").onclick = () => done(false);
    dialog.oncancel = () => done(false);
    dialog.showModal();
  });
}
