import { invoke } from "@tauri-apps/api/core";

/** Logical pixels per mask cell. 192 / 4 = 48 columns fits in the 64 */
/** bits the Rust side reads per row; 240 / 4 = 60 rows. */
export const CELL = 4;
const ALPHA_MIN = 16;

/** Build the window's opacity mask from the canvas plus any solid */
/** elements below it, and hand it to the Rust poller. */
export function pushHitMask(canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D, solid: HTMLElement[]) {
  const dpr = canvas.width / canvas.clientWidth;
  const img = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
  const cols = Math.ceil(window.innerWidth / CELL);
  const rowsN = Math.ceil(window.innerHeight / CELL);
  const rows: number[] = new Array(rowsN).fill(0);

  const canvasRows = Math.ceil(canvas.clientHeight / CELL);
  for (let cy = 0; cy < canvasRows && cy < rowsN; cy++) {
    let bits = 0;
    for (let cx = 0; cx < cols; cx++) {
      const px = Math.min(canvas.width - 1, Math.floor((cx * CELL + CELL / 2) * dpr));
      const py = Math.min(canvas.height - 1, Math.floor((cy * CELL + CELL / 2) * dpr));
      if (img[(py * canvas.width + px) * 4 + 3] > ALPHA_MIN) bits += 2 ** cx;
    }
    rows[cy] = bits;
  }

  for (const el of solid) {
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;
    const x0 = Math.floor(r.left / CELL), x1 = Math.ceil(r.right / CELL);
    const y0 = Math.floor(r.top / CELL), y1 = Math.ceil(r.bottom / CELL);
    for (let cy = y0; cy < y1 && cy < rowsN; cy++) {
      for (let cx = x0; cx < x1 && cx < cols; cx++) rows[cy] += rows[cy] & 2 ** cx ? 0 : 2 ** cx;
    }
  }

  void invoke("set_hit_mask", { cell: CELL, rows });
}
