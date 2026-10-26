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

export interface Manifest {
  id: string;
  displayName: string;
  description?: string;
  spritesheetPath: string;
  "x-sidepaw"?: { persona?: string; personaText?: string; lines?: string; voice?: string };
}

export interface Pack {
  manifest: Manifest;
  sheet: HTMLImageElement;
  lines: Lines;
}

/** A missing optional file is null. The Vite dev server answers unknown
 *  paths with index.html and status 200, so the content type is checked
 *  and a parse failure also reads as absent. */
async function optionalJson<T>(url: string): Promise<T | null> {
  const res = await fetch(url);
  if (!res.ok || !(res.headers.get("content-type") ?? "").includes("json")) return null;
  try {
    return (await res.json()) as T;
  } catch {
    return null;
  }
}

export async function loadPack(id: string): Promise<Pack> {
  const res = await fetch(`/${id}/pet.json`);
  if (!res.ok) throw new Error(`pack ${id}: ${res.status}`);
  const manifest = (await res.json()) as Manifest;
  const sheet = new Image();
  sheet.src = `/${id}/${manifest.spritesheetPath}`;
  await sheet.decode();
  if (sheet.naturalWidth !== CELL_W * COLUMNS || sheet.naturalHeight !== CELL_H * ROWS) {
    throw new Error(`pack ${id}: sheet is ${sheet.naturalWidth}x${sheet.naturalHeight}`);
  }
  // A pack without lines is a valid Codex pack; the pet is just quiet.
  // `lines.<lang>.json` next to `lines.json` wins for that UI language.
  const linesPath = manifest["x-sidepaw"]?.lines;
  let file: LinesFile | null = null;
  if (linesPath) {
    const localized = linesPath.replace(/\.json$/, `.${detectLang()}.json`);
    file = (await optionalJson<LinesFile>(`/${id}/${localized}`)) ?? (await optionalJson<LinesFile>(`/${id}/${linesPath}`));
  }
  return { manifest, sheet, lines: file ? new Lines(file) : Lines.empty() };
}
