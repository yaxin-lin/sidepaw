/** The live lines switch (ADR 8): turning it on shows exactly what one
 *  such line would send right now and needs a yes; turning it off does
 *  not. */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { buildContext } from "../brain/context";
import { activePack, loadPersona } from "../brain/files";
import { preview, resolve } from "../brain/llm";
import { DAILY_CAP, nudgeRequest } from "../brain/nudge";
import { compileSystemPrompt } from "../brain/persona";
import type { DayReport } from "../generated/DayReport";
import type { EventRow } from "../generated/EventRow";
import type { ModelConfig } from "../generated/ModelConfig";
import type { PetPrefs } from "../generated/PetPrefs";
import type { Snapshot } from "../generated/Snapshot";
import { detectLang, t } from "../i18n";
import { el, note } from "./dom";
import { ask } from "./send";

async function sampleRequest() {
  const [snapshot, report, events, persona] = await Promise.all([
    invoke<Snapshot>("snapshot"),
    invoke<DayReport>("report_today"),
    invoke<EventRow[]>("recent_events", { limit: 5 }),
    activePack().then(loadPersona),
  ]);
  const prompt = compileSystemPrompt(persona.persona, persona.md, detectLang());
  return nudgeRequest("errors", prompt, buildContext({ snapshot, report, events }, 5));
}

export async function mountLive() {
  const box = el<HTMLInputElement>("live-lines");
  box.checked = (await invoke<PetPrefs>("pet_prefs")).live_lines;
  await listen<PetPrefs>("prefs-changed", (e) => (box.checked = e.payload.live_lines));
  box.addEventListener("change", async () => {
    if (!box.checked) {
      await invoke("set_live_lines", { on: false });
      note("live-msg", t("live.off"));
      return;
    }
    box.checked = false;
    try {
      const cfg = await invoke<ModelConfig>("get_model");
      resolve(cfg);
      const req = await sampleRequest();
      const pv = preview(cfg, req);
      const ok = await ask(t("live.title"), t("live.summary", { tokens: pv.approxTokens, cap: DAILY_CAP, model: pv.model }), pv.text, t("live.accept"), true);
      if (!ok) return;
      await invoke("set_live_lines", { on: true });
      box.checked = true;
      note("live-msg", t("live.on"));
    } catch (e) {
      note("live-msg", String(e), true);
    }
  });
}
