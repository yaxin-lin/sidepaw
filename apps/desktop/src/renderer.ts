import { CELL_H, CELL_W, FRAMES, LOOP_MS, ROW } from "./pack";
import type { State } from "./generated/State";

/** Which spritesheet row a pet state plays; see docs/contracts/pet-pack.md. */
export function rowFor(state: State): number {
  switch (state.state) {
    case "sleeping":
    case "idle":
      return ROW.idle;
    case "done":
      return ROW.waving;
    case "thinking":
      return ROW.running;
    case "tool":
      return state.detail === "read" || state.detail === "search" ? ROW.review : ROW.runningRight;
    case "error":
      return ROW.failed;
    case "waiting":
      return ROW.waiting;
  }
}

export class SpriteRenderer {
  readonly ctx: CanvasRenderingContext2D;
  private row: number = ROW.idle;
  private raf = 0;
  private lastDrawn = -1;
  /** Called after a frame that differs from the previous one was drawn. */
  onFrame: (() => void) | null = null;

  constructor(canvas: HTMLCanvasElement, private sheet: HTMLImageElement) {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = CELL_W * dpr;
    canvas.height = CELL_H * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("no 2d context");
    ctx.scale(dpr, dpr);
    ctx.imageSmoothingEnabled = true;
    this.ctx = ctx;
  }

  setState(state: State) {
    this.row = rowFor(state);
  }

  /** Draw the frame currently on screen into another context at
   *  `w` by `h`, for callers that need its shape at low resolution. */
  drawCurrentInto(ctx: CanvasRenderingContext2D, w: number, h: number) {
    if (this.lastDrawn < 0) return;
    const frame = this.lastDrawn % FRAMES;
    const row = Math.floor(this.lastDrawn / FRAMES);
    ctx.clearRect(0, 0, w, h);
    ctx.drawImage(this.sheet, frame * CELL_W, row * CELL_H, CELL_W, CELL_H, 0, 0, w, h);
  }

  start() {
    const draw = (t: number) => {
      const frame = Math.floor((t % LOOP_MS) / (LOOP_MS / FRAMES));
      const key = this.row * FRAMES + frame;
      if (key !== this.lastDrawn) {
        this.lastDrawn = key;
        this.ctx.clearRect(0, 0, CELL_W, CELL_H);
        this.ctx.drawImage(
          this.sheet,
          frame * CELL_W, this.row * CELL_H, CELL_W, CELL_H,
          0, 0, CELL_W, CELL_H,
        );
        this.onFrame?.();
      }
      this.raf = requestAnimationFrame(draw);
    };
    this.raf = requestAnimationFrame(draw);
  }

  stop() {
    cancelAnimationFrame(this.raf);
  }
}
