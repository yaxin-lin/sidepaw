/** The pet's life on the desktop, without any OS permission: a throw
 *  with real physics, bouncing off the work area, flinching from a
 *  cursor that comes too close, sizing up a cursor that lingers, and
 *  catching files dropped on it. */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import type { Bubble } from "./bubble";
import type { IdleBehaviour } from "./idle";
import { LOOP_MS, ROW } from "../pack";
import { airborne, atRest, type Body, type Bounds, FLING_SPEED, type Sample, step, velocityFrom } from "./physics";
import type { SpriteRenderer } from "./renderer";

/** Mirrors `CursorRelative` in src-tauri/src/hit.rs: logical pixels
 *  relative to the window's origin. */
interface CursorRelative {
  x: number;
  y: number;
  width: number;
  height: number;
}

const SAMPLE_MS = 16;
const CLICK_MS = 250;
const HOVER_MS = 1500;
const NEAR_MS = 700;
const FLEE_COOLDOWN_MS = 6000;
const CURSOR_POLL_MS = 120;
const FLING_MAX_MS = 6000;
const DRAG_MIN_PX = 20;

export class DesktopLife {
  private win = getCurrentWindow();
  private pressed = false;
  private pressedAt = 0;
  private scale = 1;
  private samples: Sample[] = [];
  private sampler = 0;
  private hoverTimer = 0;
  private nearSince = 0;
  private fleeAfter = 0;

  constructor(
    private renderer: SpriteRenderer,
    private idle: IdleBehaviour,
    private bubble: Bubble,
    private canvas: HTMLCanvasElement,
  ) {}

  /** Wires the DOM and Tauri events. The window has no decorations, so
   *  the pet is its own drag handle: every press starts a native drag
   *  and the release decides whether it was a click, a drag or a throw. */
  async attach() {
    this.canvas.addEventListener("mousedown", (e) => {
      if (e.button !== 0 || this.idle.hold) return;
      this.pressed = true;
      this.pressedAt = Date.now();
      this.samples = [];
      clearInterval(this.sampler);
      void this.win.scaleFactor().then((s) => (this.scale = s));
      this.sampler = window.setInterval(() => void this.sample(), SAMPLE_MS);
      void this.win.startDragging();
    });
    this.canvas.addEventListener("mouseup", (e) => {
      if (e.button === 0) void this.release();
    });
    window.addEventListener("blur", () => void this.release());
    this.canvas.addEventListener("mouseenter", () => {
      clearTimeout(this.hoverTimer);
      this.hoverTimer = window.setTimeout(() => {
        if (this.idle.isIdle() && !this.pressed && !this.idle.hold) this.renderer.play(ROW.review, LOOP_MS);
      }, HOVER_MS);
    });
    this.canvas.addEventListener("mouseleave", () => clearTimeout(this.hoverTimer));
    setInterval(() => void this.watchCursor(), CURSOR_POLL_MS);
    await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop" || event.payload.paths.length === 0) return;
      const name = event.payload.paths[0].split(/[\\/]/).pop() ?? "";
      this.renderer.play(ROW.jumping, LOOP_MS);
      this.bubble.onInteraction("drop", name);
    });
  }

  private async sample() {
    if (!this.pressed) return;
    const p = await this.win.outerPosition();
    this.samples.push({ t: performance.now(), x: p.x / this.scale, y: p.y / this.scale });
    if (this.samples.length > 16) this.samples.shift();
  }

  /** A short press that did not move is a click; a long or moving one
   *  is a drag; a fast release is a throw. */
  private async release() {
    if (!this.pressed) return;
    this.pressed = false;
    clearInterval(this.sampler);
    const first = this.samples[0];
    const last = this.samples[this.samples.length - 1];
    const moved = first && last ? Math.hypot(last.x - first.x, last.y - first.y) : 0;
    const v = velocityFrom(this.samples);
    if (Math.hypot(v.vx, v.vy) >= FLING_SPEED) await this.fling(v.vx, v.vy);
    else if (moved >= DRAG_MIN_PX) this.bubble.onInteraction("drag");
    else if (Date.now() - this.pressedAt < CLICK_MS) {
      this.bubble.onInteraction("click");
      this.idle.onClick();
    }
  }

  /** Integrates until the pet rests on the bottom of the work area. A
   *  gentle release never gets here, so placing the pet on a shelf of
   *  your choosing still works. */
  private async fling(vx: number, vy: number) {
    const monitor = await currentMonitor();
    if (!monitor) return;
    const scale = monitor.scaleFactor;
    const size = await this.win.outerSize();
    const pos = await this.win.outerPosition();
    const area = monitor.workArea;
    const bounds: Bounds = {
      minX: area.position.x / scale,
      maxX: (area.position.x + area.size.width - size.width) / scale,
      minY: area.position.y / scale,
      maxY: (area.position.y + area.size.height - size.height) / scale,
    };
    let body: Body = { x: pos.x / scale, y: pos.y / scale, vx, vy };
    this.idle.hold = true;
    this.renderer.play(ROW.jumping, FLING_MAX_MS);
    const t0 = performance.now();
    let last = t0;
    await new Promise<void>((resolve) => {
      const frame = async (t: number) => {
        const dt = Math.min(0.05, (t - last) / 1000);
        last = t;
        body = step(body, dt, bounds);
        await this.win.setPosition(new PhysicalPosition(Math.round(body.x * scale), Math.round(body.y * scale)));
        if (!airborne(body, bounds)) this.renderer.stopOverride();
        if (atRest(body, bounds) || t - t0 > FLING_MAX_MS) resolve();
        else requestAnimationFrame((n) => void frame(n));
      };
      requestAnimationFrame((n) => void frame(n));
    });
    this.renderer.stopOverride();
    this.idle.hold = false;
  }

  /** A cursor hovering just beside the pet for a moment makes it step
   *  away; over the pet is hover, handled by the DOM events. */
  private async watchCursor() {
    if (!this.idle.isIdle() || this.pressed || this.idle.hold || performance.now() < this.fleeAfter) return;
    const c = await invoke<CursorRelative | null>("cursor_relative");
    if (!c) return;
    const inside = c.x >= 0 && c.x < c.width && c.y >= 0 && c.y < c.height;
    const sameBand = c.y >= 0 && c.y < c.height;
    const dx = c.x - c.width / 2;
    const near = !inside && sameBand && Math.abs(dx) < c.width * 1.2;
    const now = performance.now();
    if (!near) {
      this.nearSince = 0;
      return;
    }
    if (this.nearSince === 0) this.nearSince = now;
    if (now - this.nearSince < NEAR_MS) return;
    this.nearSince = 0;
    this.fleeAfter = now + FLEE_COOLDOWN_MS;
    await this.idle.flee(dx > 0 ? -1 : 1);
  }
}
