/** Motion layered on top of the spritesheet, so any pack feels alive
 *  without extra art: breathing while idle, a crouch before a hop and
 *  a squash on landing, a lean into a walk, a sway while being dragged.
 *  `pose` is pure; `Juice` applies it to the canvas as a CSS transform
 *  anchored at the feet. The hit mask ignores these few percent. */

export interface Pose {
  sx: number;
  sy: number;
  /** Degrees, clockwise. */
  rot: number;
}

export interface Motion {
  /** ms since start, for the breathing cycle. */
  t: number;
  breathing: "awake" | "asleep" | "none";
  /** A squash in progress: what kind and how far through (0 to 1). */
  squash: { kind: "crouch" | "land"; k: number } | null;
  /** -1 walking left, 1 right, 0 standing. */
  lean: number;
  /** Degrees of sway from being dragged, already eased. */
  sway: number;
}

const BREATH_MS = { awake: 3200, asleep: 5200 };
const BREATH_AMP = { awake: 0.018, asleep: 0.03 };
const LEAN_DEG = 5;

export function pose(m: Motion): Pose {
  let sx = 1;
  let sy = 1;
  if (m.breathing !== "none") {
    const b = Math.sin((m.t / BREATH_MS[m.breathing]) * 2 * Math.PI) * BREATH_AMP[m.breathing];
    sy += b;
    sx -= b / 2;
  }
  if (m.squash) {
    // Down and out, then back: a crouch is shallower than a landing.
    const depth = m.squash.kind === "land" ? 0.14 : 0.08;
    const s = Math.sin(m.squash.k * Math.PI) * depth;
    sy -= s;
    sx += s * 0.8;
  }
  return { sx, sy, rot: m.lean * LEAN_DEG + m.sway };
}

/** `[0, 1]` progress of something that started at `start` and lasts `ms`. */
function progress(now: number, start: number, ms: number): number | null {
  const k = (now - start) / ms;
  return k >= 0 && k < 1 ? k : null;
}

export const CROUCH_MS = 120;
const LAND_MS = 220;
const SWAY_MAX = 14;

export class Juice {
  private born = performance.now();
  private squashAt = 0;
  private squashKind: "crouch" | "land" = "land";
  private leanDir = 0;
  private swayTarget = 0;
  private sway = 0;
  private raf = 0;
  breathing: Motion["breathing"] = "awake";

  constructor(private el: HTMLElement) {
    el.style.transformOrigin = "50% 100%";
  }

  start() {
    const tick = () => {
      this.apply();
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  stop() {
    cancelAnimationFrame(this.raf);
  }

  crouch() {
    this.squashKind = "crouch";
    this.squashAt = performance.now();
  }

  land() {
    this.squashKind = "land";
    this.squashAt = performance.now();
  }

  lean(dir: number) {
    this.leanDir = Math.sign(dir);
  }

  /** Horizontal speed of a drag in logical px per second; the pet hangs
   *  back from the direction it is pulled. 0 when let go. */
  drag(vx: number) {
    this.swayTarget = Math.max(-SWAY_MAX, Math.min(SWAY_MAX, -vx / 60));
  }

  private apply() {
    const now = performance.now();
    // Ease the sway toward its target so it swings rather than snaps.
    this.sway += (this.swayTarget - this.sway) * 0.15;
    const k = progress(now, this.squashAt, this.squashKind === "land" ? LAND_MS : CROUCH_MS);
    const p = pose({
      t: now - this.born,
      breathing: this.breathing,
      squash: k === null ? null : { kind: this.squashKind, k },
      lean: this.leanDir,
      sway: Math.abs(this.sway) < 0.05 ? 0 : this.sway,
    });
    this.el.style.transform = `rotate(${p.rot.toFixed(2)}deg) scale(${p.sx.toFixed(4)}, ${p.sy.toFixed(4)})`;
  }
}
