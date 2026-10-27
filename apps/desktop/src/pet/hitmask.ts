import { invoke } from "@tauri-apps/api/core";

/** Logical pixels per mask cell. 192 / 4 = 48 columns fits in the 64
 *  bits the Rust side reads per row. */
export const CELL = 4;
const ALPHA_MIN = 16;

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface MaskInput {
  /** Window size in logical pixels. */
  width: number;
  height: number;
  /** Where the canvas sits in the window and whether a canvas-local
   *  logical pixel is opaque. The canvas is not at the window origin:
   *  the bubble takes the space above it. */
  canvas: Rect;
  opaqueAt: (x: number, y: number) => boolean;
  /** Boxes that are solid regardless of pixels (bubble, label). */
  solid: Rect[];
}

/** One number per row, bit `cx` set when the cell is opaque. Numbers,
 *  not bigints: 48 bits fit in a double and cross the IPC as JSON. */
export function buildMask(input: MaskInput): number[] {
  const cols = Math.ceil(input.width / CELL);
  const rowsN = Math.ceil(input.height / CELL);
  const rows: number[] = new Array(rowsN).fill(0);
  const inside = (r: Rect, x: number, y: number) => x >= r.left && x < r.right && y >= r.top && y < r.bottom;
  for (let cy = 0; cy < rowsN; cy++) {
    let bits = 0;
    for (let cx = 0; cx < cols; cx++) {
      const x = cx * CELL + CELL / 2;
      const y = cy * CELL + CELL / 2;
      let opaque = input.solid.some((r) => inside(r, x, y));
      if (!opaque && inside(input.canvas, x, y)) {
        opaque = input.opaqueAt(x - input.canvas.left, y - input.canvas.top);
      }
      if (opaque) bits += 2 ** cx;
    }
    rows[cy] = bits;
  }
  return rows;
}

function rectOf(el: Element): Rect {
  const r = el.getBoundingClientRect();
  return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
}

/** Samples the pet at one pixel per mask cell. Reading the full
 *  device-pixel canvas six times a second cost more than drawing the
 *  frame again at 48 by 52. */
export class HitMaskSource {
  private small: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;

  constructor(
    private canvas: HTMLCanvasElement,
    private draw: (ctx: CanvasRenderingContext2D, w: number, h: number) => void,
  ) {
    this.small = document.createElement("canvas");
    this.small.width = Math.ceil(canvas.clientWidth / CELL);
    this.small.height = Math.ceil(canvas.clientHeight / CELL);
    const ctx = this.small.getContext("2d", { willReadFrequently: true });
    if (!ctx) throw new Error("no 2d context");
    this.ctx = ctx;
  }

  /** Build the window's opacity mask from the pet plus any solid
   *  elements, hand it to the Rust poller, and return where the pet's
   *  top edge is in window coordinates (null for an empty frame) so
   *  the bubble can sit on its head whatever the pack draws. */
  push(solid: HTMLElement[]): number | null {
    const { width, height } = this.small;
    this.draw(this.ctx, width, height);
    const img = this.ctx.getImageData(0, 0, width, height).data;
    const canvas = rectOf(this.canvas);
    const rows = buildMask({
      width: window.innerWidth,
      height: window.innerHeight,
      canvas,
      opaqueAt: (x, y) => {
        const px = Math.min(width - 1, Math.floor(x / CELL));
        const py = Math.min(height - 1, Math.floor(y / CELL));
        return img[(py * width + px) * 4 + 3] > ALPHA_MIN;
      },
      solid: solid.filter((el) => !el.hidden).map(rectOf),
    });
    void invoke("set_hit_mask", { cell: CELL, rows });
    const row = topOpaqueRow(img, width, height);
    return row === null ? null : canvas.top + row * CELL;
  }
}

function topOpaqueRow(img: Uint8ClampedArray, width: number, height: number): number | null {
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (img[(y * width + x) * 4 + 3] > ALPHA_MIN) return y;
    }
  }
  return null;
}
