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
}

export async function loadPack(id: string): Promise<{ manifest: Manifest; sheet: HTMLImageElement }> {
  const res = await fetch(`/${id}/pet.json`);
  if (!res.ok) throw new Error(`pack ${id}: ${res.status}`);
  const manifest = (await res.json()) as Manifest;
  const sheet = new Image();
  sheet.src = `/${id}/${manifest.spritesheetPath}`;
  await sheet.decode();
  if (sheet.naturalWidth !== CELL_W * COLUMNS || sheet.naturalHeight !== CELL_H * ROWS) {
    throw new Error(`pack ${id}: sheet is ${sheet.naturalWidth}x${sheet.naturalHeight}`);
  }
  return { manifest, sheet };
}
