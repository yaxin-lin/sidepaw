/** The pet's life on the desktop, without any OS permission: a throw
 *  with real physics, bouncing off the work area, flinching from a
 *  cursor that comes too close, sizing up a cursor that lingers, and
 *  catching files dropped on it. */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import type { Bubble } from "./bubble";
import { Dock } from "./dock";
import { t } from "../i18n";
import type { IdleBehaviour } from "./idle";
import { LOOP_MS, ROW } from "../pack";
import { airborne, atRest, type Body, type Bounds, FLING_SPEED, HOP_MS, hopLift, hopTarget, type Ledge, type Rect, type Sample, step, support, velocityFrom } from "./physics";
import type { SpriteRenderer } from "./renderer";

/** Mirrors `CursorRelative` in src-tauri/src/hit.rs: logical pixels
 *  relative to the window's origin. */
interface CursorRelative {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Mirrors `Shelf` and `Scene` in src-tauri/src/shelf.rs, physical pixels. */
export interface Shelf extends Rect {
  id: number;
  pid: number;
  front: boolean;
}

export interface Scene {
  shelves: Shelf[];
  fullscreen: boolean;
  front_app: { name: string; id: string } | null;
}

/** The pet and its surroundings in logical pixels, from one look. */
export interface World {
  bounds: Bounds;
  scale: number;
  pet: { x: number; y: number };
  scene: Scene;
}

const SAMPLE_MS = 16;
const CLICK_MS = 250;
const DOUBLE_CLICK_MS = 350;
/** Often enough that the pet keeps up with a window being dragged. */
const SUPPORT_MS = 150;
const HOVER_MS = 1500;
const NEAR_MS = 700;
const FLEE_COOLDOWN_MS = 6000;
const CURSOR_POLL_MS = 120;
const FLING_MAX_MS = 6000;
const DRAG_MIN_PX = 20;

export class DesktopLife {
  private win = getCurrentWindow();
  private pressed = false;
  private dock = new Dock();
  private pressedAt = 0;
  private lastClickAt = 0;
  private scale = 1;
  private samples: Sample[] = [];
  private sampler = 0;
  private hoverTimer = 0;
  private nearSince = 0;
  private fleeAfter = 0;
  /** The window the pet stands on and where it was, to ride along. */
  private riding: { id: number; x: number; y: number } | null = null;
  private lastWorld: World | null = null;
  private checking = false;

  constructor(
    private renderer: SpriteRenderer,
    private idle: IdleBehaviour,
    private bubble: Bubble,
    private canvas: HTMLCanvasElement,
    /** First say in a click; true means it was used up (a jump to a
     *  terminal) and the pet should not also hop. */
    private onClick: () => Promise<boolean> = async () => false,
  ) {}

  /** Wires the DOM and Tauri events. The window has no decorations, so
   *  the pet is its own drag handle: every press starts a native drag
   *  and the release decides whether it was a click, a drag or a throw. */
  async attach() {
    this.idle.limits = () => this.walkRange();
    this.canvas.addEventListener("mousedown", (e) => {
      const parked = this.dock.isParked();
      if (e.button !== 0 || (this.idle.hold && !parked)) return;
      this.pressed = true;
      this.pressedAt = Date.now();
      this.samples = [];
      // Parked at the edge the pet is not draggable; a press is a click.
      if (parked) return;
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
    setInterval(() => void this.checkSupport(), SUPPORT_MS);
    await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop" || event.payload.paths.length === 0) return;
      const path = event.payload.paths[0];
      const name = path.split(/[\\/]/).pop() ?? "";
      this.renderer.play(ROW.jumping, LOOP_MS);
      // A dropped zip is most likely a pet; try it as one, quietly
      // falling back to an ordinary drop when it is not.
      if (name.toLowerCase().endsWith(".zip")) {
        invoke<{ display_name: string }>("pack_install_path", { path }).then(
          (info) => this.bubble.say(t("packs.dropped", { name: info.display_name })),
          () => this.bubble.onInteraction("drop", name),
        );
        return;
      }
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
      const now = Date.now();
      if (now - this.lastClickAt < DOUBLE_CLICK_MS) {
        this.lastClickAt = 0;
        void invoke("open_chat");
        return;
      }
      this.lastClickAt = now;
      if (await this.onClick()) return;
      this.bubble.onInteraction("click");
      this.idle.onClick();
    }
  }

  /** Integrates until the pet rests on the bottom of the work area. A
   *  gentle release never gets here, so placing the pet on a shelf of
   *  your choosing still works. */
  private async fling(vx: number, vy: number) {
    const world = await this.world();
    if (world) await this.fly(world, vx, vy);
  }

  /** A small hop on the spot or a little to the side, landing on the
   *  same edge it stood on. Returns false when the pet is busy. */
  async hop(dx: number): Promise<boolean> {
    if (this.pressed || this.idle.hold || this.idle.isWalking() || this.dock.isParked()) return false;
    const world = await this.world();
    if (!world) return false;
    const { scale, pet, bounds } = world;
    const edge = this.walkRange();
    const range = edge ? { minX: edge.minX / scale, maxX: edge.maxX / scale } : { minX: bounds.minX, maxX: bounds.maxX };
    const x1 = hopTarget(pet.x, dx, range);
    this.idle.hold = true;
    this.renderer.play(ROW.jumping, HOP_MS);
    const t0 = performance.now();
    await new Promise<void>((resolve) => {
      const frame = async () => {
        const k = Math.min(1, (performance.now() - t0) / HOP_MS);
        const x = pet.x + (x1 - pet.x) * k;
        const y = pet.y - hopLift(k);
        await this.win.setPosition(new PhysicalPosition(Math.round(x * scale), Math.round(y * scale)));
        if (k >= 1) resolve();
        else requestAnimationFrame(() => void frame());
      };
      void frame();
    });
    this.renderer.stopOverride();
    this.idle.hold = false;
    return true;
  }

  /** The physical x range of the visible edge the pet stands on, so a
   *  stroll stays on it; null on the floor. */
  walkRange(): { minX: number; maxX: number } | null {
    const w = this.lastWorld;
    if (!w) return null;
    const l = support(w.bounds, w.pet.x, w.pet.y).ledge;
    if (!l) return null;
    const pw = w.bounds.pet.width;
    return { minX: (l.x1 - pw / 3) * w.scale, maxX: (l.x2 - (pw * 2) / 3) * w.scale };
  }

  /** The last look around, refreshed several times a second. */
  world0(): World | null {
    return this.lastWorld;
  }

  private async fly(world: World, vx: number, vy: number) {
    const { bounds, scale } = world;
    let body: Body = { ...world.pet, vx, vy };
    this.riding = null;
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

  /** The pet and what it can land on, in logical pixels. Other apps'
   *  windows come from the OS each time; they move and close. */
  private async world(): Promise<World | null> {
    const monitor = await currentMonitor();
    if (!monitor) return null;
    const scale = monitor.scaleFactor;
    const [size, pos, scene] = await Promise.all([this.win.outerSize(), this.win.outerPosition(), invoke<Scene>("scene")]);
    const area = monitor.workArea;
    const bounds: Bounds = {
      minX: area.position.x / scale,
      maxX: (area.position.x + area.size.width - size.width) / scale,
      minY: area.position.y / scale,
      maxY: (area.position.y + area.size.height - size.height) / scale,
      pet: { width: size.width / scale, height: size.height / scale },
      shelves: scene.shelves.map((r) => ({ x: r.x / scale, y: r.y / scale, width: r.width / scale, height: r.height / scale, id: r.id })),
    };
    this.lastWorld = { bounds, scale, pet: { x: pos.x / scale, y: pos.y / scale }, scene };
    return this.lastWorld;
  }

  /** Mini mode on or off. Walks, throws and the support check stop
   *  while parked, since the dock owns the window's position. */
  async setMini(on: boolean) {
    if (on) {
      this.idle.hold = true;
      await this.dock.park();
    } else if (this.dock.isParked()) {
      await this.dock.leave();
      this.idle.hold = false;
    }
  }

  setAttention(on: boolean) {
    this.dock.setAttention(on);
  }

  /** Whatever held the pet up may have moved or closed; when nothing
   *  is under it any more, it falls. Not while a walk or a throw is
   *  moving the window. */
  private async checkSupport() {
    if (this.checking) return;
    this.checking = true;
    try {
      await this.followSupport();
    } finally {
      this.checking = false;
    }
  }

  /** Rides the window it stands on when that window moves, and falls
   *  when it closes, shrinks away or goes behind another. */
  private async followSupport() {
    if (this.pressed || this.idle.hold || this.dock.isParked()) return;
    const world = await this.world();
    if (!world || this.idle.isWalking()) {
      this.riding = null;
      return;
    }
    const { bounds, pet, scale } = world;
    if (this.riding) {
      const now = bounds.shelves.find((s) => s.id === this.riding?.id);
      if (now && (now.x !== this.riding.x || now.y !== this.riding.y)) {
        const x = pet.x + (now.x - this.riding.x);
        const y = pet.y + (now.y - this.riding.y);
        this.riding = { id: this.riding.id, x: now.x, y: now.y };
        await this.win.setPosition(new PhysicalPosition(Math.round(x * scale), Math.round(y * scale)));
        return;
      }
    }
    const under = support(bounds, pet.x, pet.y);
    if (pet.y < under.y - 2) {
      await this.fling(0, 0);
      return;
    }
    this.riding = ledgeRide(under.ledge, bounds.shelves);
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

function ledgeRide(ledge: Ledge | null, shelves: Rect[]): { id: number; x: number; y: number } | null {
  if (!ledge || ledge.id === undefined) return null;
  const s = shelves.find((r) => r.id === ledge.id);
  return s ? { id: ledge.id, x: s.x, y: s.y } : null;
}
