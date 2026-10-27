import { CELL_H, CELL_W, FRAMES, LOOP_MS, ROW } from "../pack";
import type { State } from "../generated/State";

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
  /** A row that takes precedence over the state's row until `until`
   *  (performance.now ms); used for walks and the click hop. */
  private override: { row: number; until: number } | null = null;
  /** Animation speed factor; 0.5 while dozing. */
  speed = 1;
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

  /** Another pack was chosen; the next frame draws from it. */
  setSheet(sheet: HTMLImageElement) {
    this.sheet = sheet;
    this.lastDrawn = -1;
  }

  setState(state: State) {
    this.row = rowFor(state);
  }

  /** Play `row` instead of the state's row for `ms`. */
  play(row: number, ms: number) {
    this.override = { row, until: performance.now() + ms };
  }

  stopOverride() {
    this.override = null;
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
      if (this.override && t >= this.override.until) this.override = null;
      const row = this.override?.row ?? this.row;
      const loop = LOOP_MS / this.speed;
      const frame = Math.floor((t % loop) / (loop / FRAMES));
      const key = row * FRAMES + frame;
      if (key !== this.lastDrawn) {
        this.lastDrawn = key;
        this.ctx.clearRect(0, 0, CELL_W, CELL_H);
        this.ctx.drawImage(
          this.sheet,
          frame * CELL_W, row * CELL_H, CELL_W, CELL_H,
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
