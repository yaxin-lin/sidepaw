/** The model and persona half of the setup window: provider config,
 *  the key, the persona editor, line generation and the audit list. */
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { parseJson, readBundled, readUser, writeUser } from "./brain/files";
import { countLines, linesRequest, parseLines } from "./brain/linegen";
import { complete, preview, PROVIDERS, type Purpose, type Request } from "./brain/llm";
import { compileSystemPrompt, FLAW_TAGS, personaDigest, SLIDERS, STRENGTH_TAGS, type Persona, type Slider } from "./brain/persona";
import type { AuditRow } from "./generated/AuditRow";
import type { KeySource } from "./generated/KeySource";
import type { Model } from "./generated/Model";
import { t, type Key } from "./i18n";
import type { LinesFile } from "./lines";
import { DEFAULT_PACK } from "./pack";

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function note(id: string, text: string, err = false) {
  const m = el(id);
  m.className = err ? "msg err" : "msg";
  m.textContent = text;
}

async function busy<T>(id: string, work: () => Promise<T>): Promise<T | undefined> {
  note(id, t("working"));
  try {
    return await work();
  } catch (e) {
    note(id, String(e), true);
    return undefined;
  }
}

// ---- model -------------------------------------------------------------

function modelFromForm(): Model {
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
  el<HTMLInputElement>("m-key").placeholder = p && !p.needsKey ? "optional" : "";
}

async function renderKeySource() {
  const src = await invoke<KeySource>("key_source");
  el("m-key-source").textContent = t(`model.key.${src}` as Key);
  el<HTMLButtonElement>("m-key-remove").disabled = src !== "keyring";
}

async function loadModel() {
  const select = el<HTMLSelectElement>("m-provider");
  select.replaceChildren();
  for (const [id, p] of Object.entries(PROVIDERS)) {
    const o = document.createElement("option");
    o.value = id;
    o.textContent = p.label;
    select.append(o);
  }
  const cfg = await invoke<Model>("get_model");
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
  el("m-test").addEventListener("click", () =>
    busy("m-msg", async () => {
      const system = compileSystemPrompt(personaFromForm(), el<HTMLTextAreaElement>("p-md").value);
      const text = await send({ purpose: "persona", system, user: t("model.test.user"), maxTokens: 200 });
      if (text !== null) note("m-msg", t("model.test.ok", { text: text.trim() }));
    }),
  );
}

// ---- preview before send ------------------------------------------------

/** Remote providers get the confirmation the permissions contract
 *  promises; local ones go straight out. Resolves to null when the
 *  user cancels. */
async function send(req: Request): Promise<string | null> {
  const cfg = modelFromForm();
  const pv = preview(cfg, req);
  if (!pv.local) {
    const ok = await confirmSend(pv.provider, pv.model, pv.purpose, pv.approxTokens, pv.bytes, pv.text);
    if (!ok) return null;
  }
  const key = await invoke<string | null>("api_key");
  try {
    return await complete(cfg, req, key);
  } finally {
    void renderAudit();
  }
}

function confirmSend(provider: string, model: string, purpose: Purpose, tokens: number, bytes: number, text: string): Promise<boolean> {
  const dialog = el<HTMLDialogElement>("send");
  el("send-title").textContent = t("send.title", { provider: PROVIDERS[provider]?.label ?? provider });
  el("send-summary").textContent = t("send.summary", { purpose: t(`purpose.${purpose}`), tokens, bytes, model });
  el<HTMLPreElement>("send-text").textContent = text;
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

// ---- persona --------------------------------------------------------------

const PACK = DEFAULT_PACK;

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
  const n = el<HTMLTextAreaElement>("p-md").value.length;
  note("p-md-hint", t("pet.md.hint", { n }), n > 1500);
}

function buildControls() {
  const sliders = el("p-sliders");
  for (const s of SLIDERS) {
    const wrap = document.createElement("div");
    wrap.className = "slider";
    const span = document.createElement("span");
    const [left, right] = t(`slider.${s}`).split(" / ");
    span.append(Object.assign(document.createElement("em"), { textContent: left }), Object.assign(document.createElement("em"), { textContent: right }));
    const input = document.createElement("input");
    input.type = "range";
    input.min = "0";
    input.max = "1";
    input.step = "0.05";
    input.id = `p-s-${s}`;
    wrap.append(span, input);
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

async function savedDigest(): Promise<string> {
  const { persona, md } = await loadPersona();
  return personaDigest(persona, md);
}

async function renderLinesStatus() {
  const user = parseJson<LinesFile>(await readUser(PACK, "lines.json"));
  if (user) {
    const current = user.generatedFrom === (await savedDigest());
    note("p-lines", t(current ? "pet.lines.current" : "pet.lines.stale"), !current);
    return;
  }
  const bundled = parseJson<LinesFile>(await readBundled(PACK, "lines.json"));
  note("p-lines", t(bundled ? "pet.lines.pack" : "pet.lines.none"));
}

async function loadPet() {
  buildControls();
  const { persona, md } = await loadPersona();
  fillForm(persona, md);
  await renderLinesStatus();

  el("p-save").addEventListener("click", () =>
    busy("p-msg", async () => {
      await writeUser(PACK, "persona.json", JSON.stringify(personaFromForm(), null, 2) + "\n");
      await writeUser(PACK, "persona.md", el<HTMLTextAreaElement>("p-md").value);
      await emit("pack-changed");
      await renderLinesStatus();
      note("p-msg", t("pet.saved"));
    }),
  );
  el("p-reset").addEventListener("click", () =>
    busy("p-msg", async () => {
      for (const name of ["persona.json", "persona.md", "lines.json"] as const) await writeUser(PACK, name, "");
      const fresh = await loadPersona();
      fillForm(fresh.persona, fresh.md);
      await emit("pack-changed");
      await renderLinesStatus();
      note("p-msg", t("pet.reset.done"));
    }),
  );
  el("p-preview").addEventListener("click", () =>
    busy("p-msg", async () => {
      const system = compileSystemPrompt(personaFromForm(), el<HTMLTextAreaElement>("p-md").value);
      const text = await send({ purpose: "persona", system, user: t("model.test.user"), maxTokens: 200 });
      if (text !== null) note("p-msg", text.trim());
    }),
  );
  el("p-generate").addEventListener("click", () =>
    busy("p-msg", async () => {
      // Generate from what is saved, so generatedFrom means what it says.
      const { persona, md } = await loadPersona();
      const text = await send(linesRequest(compileSystemPrompt(persona, md)));
      if (text === null) return;
      const file = parseLines(text, await personaDigest(persona, md));
      await writeUser(PACK, "lines.json", JSON.stringify(file, null, 2) + "\n");
      await emit("pack-changed");
      await renderLinesStatus();
      note("p-msg", t("pet.generated", { n: countLines(file) }));
    }),
  );
}

// ---- audit ------------------------------------------------------------------

async function renderAudit() {
  const rows = await invoke<AuditRow[]>("audit_recent", { limit: 20 });
  const body = el<HTMLTableSectionElement>("audit-rows");
  body.replaceChildren();
  el("audit-empty").hidden = rows.length > 0;
  for (const r of rows) {
    const tr = document.createElement("tr");
    for (const [text, mono] of [
      [r.ts.replace("T", " ").slice(0, 19), true],
      [PROVIDERS[r.provider]?.label ?? r.provider, false],
      [t(`purpose.${r.purpose}` as Key), false],
      [`${r.bytes} B`, true],
      [r.sha256.slice(0, 12), true],
    ] as const) {
      const td = document.createElement("td");
      if (mono) td.className = "mono";
      td.textContent = text;
      tr.append(td);
    }
    body.append(tr);
  }
}

export async function mountBrain() {
  await loadModel();
  await loadPet();
  await renderAudit();
}
