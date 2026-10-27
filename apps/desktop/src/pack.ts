import { FALLBACK_PACK, isBundled, parseJson, readPackBytes, readPackText, readUser } from "./brain/files";
import type { Manifest } from "./generated/Manifest";
import { detectLang } from "./i18n";
import { Lines, type LinesFile } from "./lines";


// Codex spritesheet layout, mirrored from crates/sidepaw-core/src/pack.rs
// and docs/contracts/pet-pack.md.
export const CELL_W = 192;
export const CELL_H = 208;
export const COLUMNS = 8;
export const ROWS = 9;
export const FRAMES = 6;
export const LOOP_MS = 1100;

export const ROW = {
  idle: 0,
  runningRight: 1,
  runningLeft: 2,
  waving: 3,
  jumping: 4,
  failed: 5,
  waiting: 6,
  running: 7,
  review: 8,
} as const;

export type { Manifest };

export interface Pack {
  manifest: Manifest;
  sheet: HTMLImageElement;
  lines: Lines;
}

/** The user's generated or edited lines win over the pack's; among the
 *  pack's, `lines.<lang>.json` wins for the UI language. A pack without
 *  lines is a valid Codex pack; the pet is just quiet. */
async function packLines(id: string, manifest: Manifest): Promise<LinesFile | null> {
  // An installed pack without x-sidepaw may still carry lines.json.
  const linesPath = manifest["x-sidepaw"]?.lines ?? (isBundled(id) ? undefined : "lines.json");
  if (!linesPath) return null;
  const localized = linesPath.replace(/\.json$/, `.${detectLang()}.json`);
  return parseJson<LinesFile>(await readPackText(id, localized)) ?? parseJson<LinesFile>(await readPackText(id, linesPath));
}

/** The user's lines for this pet, else the pack's, else the bundled
 *  pack's: most downloaded packs are art only, and a pet that never
 *  speaks reads as broken. */
export async function loadLines(id: string, manifest: Manifest): Promise<Lines> {
  let file = parseJson<LinesFile>(await readUser(id, "lines.json")) ?? (await packLines(id, manifest));
  if (!file && id !== FALLBACK_PACK) {
    const fallback = parseJson<Manifest>(await readPackText(FALLBACK_PACK, "pet.json"));
    if (fallback) file = await packLines(FALLBACK_PACK, fallback);
  }
  return file ? new Lines(file) : Lines.empty();
}

/** A Codex sheet is 8 by 9 cells; petdex's v2 adds two rows below,
 *  which the renderer ignores. */
export function sheetSizeOk(width: number, height: number): boolean {
  return width === CELL_W * COLUMNS && (height === CELL_H * ROWS || height === CELL_H * (ROWS + 2));
}

export async function loadSheet(id: string, manifest: Manifest): Promise<HTMLImageElement> {
  const sheet = new Image();
  if (isBundled(id)) {
    sheet.src = `/${id}/${manifest.spritesheetPath}`;
  } else {
    const bytes = await readPackBytes(id, manifest.spritesheetPath);
    if (!bytes) throw new Error(`pack ${id}: no ${manifest.spritesheetPath}`);
    const type = manifest.spritesheetPath.endsWith(".png") ? "image/png" : "image/webp";
    sheet.src = URL.createObjectURL(new Blob([bytes], { type }));
  }
  await sheet.decode();
  if (!sheetSizeOk(sheet.naturalWidth, sheet.naturalHeight)) {
    throw new Error(`pack ${id}: sheet is ${sheet.naturalWidth}x${sheet.naturalHeight}`);
  }
  return sheet;
}

export async function loadPack(id: string): Promise<Pack> {
  const manifest = parseJson<Manifest>(await readPackText(id, "pet.json"));
  if (!manifest) throw new Error(`pack ${id}: no pet.json`);
  return { manifest, sheet: await loadSheet(id, manifest), lines: await loadLines(id, manifest) };
}
