/** The live lines switch (ADR 8): turning it on shows exactly what one
 *  such line would send right now and needs a yes; turning it off does
 *  not. A consent given for an older request shape counts as off until
 *  the new preview is accepted. */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { buildContext } from "../brain/context";
import { activePack, loadPersona } from "../brain/files";
import { preview, resolve } from "../brain/llm";
import { dailyCap, LIVE_SHAPE, nudgeRequest } from "../brain/nudge";
import type { ChatTurn } from "../generated/ChatTurn";
import { loadPack } from "../pack";
import { compileSystemPrompt } from "../brain/persona";
import type { DayReport } from "../generated/DayReport";
import type { EventRow } from "../generated/EventRow";
import type { PetPrefs } from "../generated/PetPrefs";
import type { Snapshot } from "../generated/Snapshot";
import { detectLang, t } from "../i18n";
import { el, note } from "./dom";
import { modelFromForm } from "./model";
import { ask } from "./send";

async function sampleRequest() {
  const pack = await activePack();
  const [snapshot, report, events, persona, turns, lines] = await Promise.all([
    invoke<Snapshot>("snapshot"),
    invoke<DayReport>("report_today"),
    invoke<EventRow[]>("recent_events", { limit: 5 }),
    loadPersona(pack),
    invoke<ChatTurn[]>("chat_turns", { hours: 24 }).catch(() => [] as ChatTurn[]),
    loadPack(pack).then((p) => p.lines),
  ]);
  const prompt = compileSystemPrompt(persona.persona, persona.md, detectLang());
  const recent = turns.filter((t) => t.channel === "bubble").map((t) => t.body);
  return nudgeRequest("errors", prompt, buildContext({ snapshot, report, events }, 5), { recent, packLine: lines.pick("errors") });
}

export async function mountLive() {
  const box = el<HTMLInputElement>("live-lines");
  const reflect = (p: PetPrefs) => {
    box.checked = p.live_lines && p.live_shape === LIVE_SHAPE;
    if (p.live_lines && p.live_shape !== LIVE_SHAPE) note("live-msg", t("live.changed"));
  };
  reflect(await invoke<PetPrefs>("pet_prefs"));
  await listen<PetPrefs>("prefs-changed", (e) => reflect(e.payload));
  box.addEventListener("change", async () => {
    if (!box.checked) {
      await invoke("set_live_lines", { on: false, shape: LIVE_SHAPE });
      note("live-msg", t("live.off"));
      return;
    }
    box.checked = false;
    try {
      // The provider chosen above counts even before Save: the preview
      // names the model, and accepting it saves the choice as well, so
      // the switch never fails on a form that is filled in but unsaved.
      const cfg = modelFromForm();
      resolve(cfg);
      const req = await sampleRequest();
      const pv = preview(cfg, req);
      const ok = await ask(t("live.title"), t("live.summary", { tokens: pv.approxTokens, cap: dailyCap(resolve(cfg)), model: pv.model }), pv.text, t("live.accept"), true);
      if (!ok) return;
      await invoke("set_model", { model: cfg });
      await invoke("set_live_lines", { on: true, shape: LIVE_SHAPE });
      box.checked = true;
      note("live-msg", t("live.on"));
    } catch (e) {
      note("live-msg", String(e), true);
    }
  });
}
