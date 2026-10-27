/** The pet section: the persona editor and line generation. Edits are
 *  overrides under ~/.sidepaw/pets/<id>/; the pack itself is never
 *  written. */
import { emit } from "@tauri-apps/api/event";
import { parseJson, readBundled, readUser, writeUser } from "../brain/files";
import { countLines, linesRequest, parseLines } from "../brain/linegen";
import { compileSystemPrompt, FLAW_TAGS, personaDigest, SLIDERS, STRENGTH_TAGS, type Persona, type Slider } from "../brain/persona";
import { t, type Key } from "../i18n";
import type { LinesFile } from "../lines";
import { DEFAULT_PACK } from "../pack";
import { renderAudit } from "./audit";
import { busy, el, note } from "./dom";
import { modelFromForm, tryLine } from "./model";
import { send } from "./send";

const PACK = DEFAULT_PACK;
const MD_MAX = 1500;

function lines(text: string): string[] {
  return text
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function checked(box: string): string[] {
  return Array.from(el(box).querySelectorAll<HTMLInputElement>("input:checked"), (i) => i.value);
}

function personaFromForm(): Persona {
  const sliders: Partial<Record<Slider, number>> = {};
  for (const s of SLIDERS) sliders[s] = Number(el<HTMLInputElement>(`p-s-${s}`).value);
  const p: Persona = {
    version: 1,
    name: el<HTMLInputElement>("p-name").value.trim() || "Pet",
    sliders,
    strengths: checked("p-strengths"),
    flaws: checked("p-flaws"),
    catchphrases: lines(el<HTMLTextAreaElement>("p-phrases").value),
    avoid: lines(el<HTMLTextAreaElement>("p-avoid").value),
  };
  const species = el<HTMLInputElement>("p-species").value.trim();
  const calls = el<HTMLInputElement>("p-calls").value.trim();
  const lang = el<HTMLInputElement>("p-lang").value.trim();
  if (species) p.species = species;
  if (calls) p.callsUser = calls;
  if (lang) p.language = lang;
  return p;
}

function mdFromForm(): string {
  return el<HTMLTextAreaElement>("p-md").value;
}

/** What the form says right now, compiled; the model section's "Test"
 *  uses it so the two buttons agree. */
export function currentPersonaPrompt(): string {
  return compileSystemPrompt(personaFromForm(), mdFromForm());
}

function fillForm(p: Persona, md: string) {
  el<HTMLInputElement>("p-name").value = p.name;
  el<HTMLInputElement>("p-species").value = p.species ?? "";
  el<HTMLInputElement>("p-calls").value = p.callsUser ?? "";
  el<HTMLInputElement>("p-lang").value = p.language ?? "";
  for (const s of SLIDERS) el<HTMLInputElement>(`p-s-${s}`).value = String(p.sliders?.[s] ?? 0.5);
  for (const [box, chosen] of [["p-strengths", p.strengths], ["p-flaws", p.flaws]] as const) {
    const set = new Set((chosen ?? []).map((x) => x.toLowerCase()));
    for (const i of el(box).querySelectorAll<HTMLInputElement>("input")) i.checked = set.has(i.value);
  }
  el<HTMLTextAreaElement>("p-phrases").value = (p.catchphrases ?? []).join("\n");
  el<HTMLTextAreaElement>("p-avoid").value = (p.avoid ?? []).join("\n");
  el<HTMLTextAreaElement>("p-md").value = md;
  mdHint();
}

function mdHint() {
  const n = mdFromForm().length;
  note("p-md-hint", t("pet.md.hint", { n, max: MD_MAX }), n > MD_MAX);
}

function buildControls() {
  const sliders = el("p-sliders");
  for (const s of SLIDERS) {
    const wrap = document.createElement("div");
    wrap.className = "slider";
    const ends = document.createElement("span");
    for (const side of ["low", "high"] as const) {
      ends.append(Object.assign(document.createElement("em"), { textContent: t(`slider.${s}.${side}` as Key) }));
    }
    const input = document.createElement("input");
    input.type = "range";
    input.min = "0";
    input.max = "1";
    input.step = "0.05";
    input.id = `p-s-${s}`;
    wrap.append(ends, input);
    sliders.append(wrap);
  }
  for (const [box, tags] of [["p-strengths", STRENGTH_TAGS], ["p-flaws", FLAW_TAGS]] as const) {
    for (const tag of tags) {
      const label = document.createElement("label");
      const input = document.createElement("input");
      input.type = "checkbox";
      input.value = tag;
      label.append(input, t(`trait.${tag}` as Key));
      el(box).append(label);
    }
  }
  el("p-md").addEventListener("input", mdHint);
}

/** The user's override, else the pack's file, else a blank persona. */
async function loadPersona(): Promise<{ persona: Persona; md: string }> {
  const persona =
    parseJson<Persona>(await readUser(PACK, "persona.json")) ??
    parseJson<Persona>(await readBundled(PACK, "persona.json")) ??
    ({ version: 1, name: "Pet" } as Persona);
  const md = (await readUser(PACK, "persona.md")) ?? (await readBundled(PACK, "persona.md")) ?? "";
  return { persona, md };
}

/** Whether lines.json matches the saved persona; the caller passes
 *  what it just loaded or wrote so this does not read it again. */
async function renderLinesStatus(persona: Persona, md: string) {
  const user = parseJson<LinesFile>(await readUser(PACK, "lines.json"));
  if (user) {
    const current = user.generatedFrom === (await personaDigest(persona, md));
    note("p-lines", t(current ? "pet.lines.current" : "pet.lines.stale"), !current);
    return;
  }
  const bundled = parseJson<LinesFile>(await readBundled(PACK, "lines.json"));
  note("p-lines", t(bundled ? "pet.lines.pack" : "pet.lines.none"));
}

export async function mountPet() {
  buildControls();
  const saved = await loadPersona();
  fillForm(saved.persona, saved.md);
  await renderLinesStatus(saved.persona, saved.md);

  el("p-save").addEventListener("click", () =>
    busy("p-msg", async () => {
      const persona = personaFromForm();
      const md = mdFromForm();
      await writeUser(PACK, "persona.json", JSON.stringify(persona, null, 2) + "\n");
      await writeUser(PACK, "persona.md", md);
      await emit("pack-changed");
      await renderLinesStatus(persona, md);
      note("p-msg", t("pet.saved"));
    }),
  );
  el("p-reset").addEventListener("click", () =>
    busy("p-msg", async () => {
      for (const name of ["persona.json", "persona.md", "lines.json"] as const) await writeUser(PACK, name, "");
      const fresh = await loadPersona();
      fillForm(fresh.persona, fresh.md);
      await emit("pack-changed");
      await renderLinesStatus(fresh.persona, fresh.md);
      note("p-msg", t("pet.reset.done"));
    }),
  );
  el("p-preview").addEventListener("click", () => void tryLine("p-msg"));
  el("p-generate").addEventListener("click", () =>
    busy("p-msg", async () => {
      // Generate from what is saved, so generatedFrom means what it says.
      const { persona, md } = await loadPersona();
      const text = await send(modelFromForm, linesRequest(compileSystemPrompt(persona, md)), () => void renderAudit());
      if (text === null) return;
      const file = parseLines(text, await personaDigest(persona, md));
      await writeUser(PACK, "lines.json", JSON.stringify(file, null, 2) + "\n");
      await emit("pack-changed");
      await renderLinesStatus(persona, md);
      note("p-msg", t("pet.generated", { n: countLines(file) }));
    }),
  );
}
