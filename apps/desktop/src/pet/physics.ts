/** Fling physics for the pet window, in logical pixels and seconds.
 *  Pure functions so the tuning is testable; `desktop.ts` applies the
 *  result to the window. */

export interface Sample {
  /** performance.now() ms */
  t: number;
  x: number;
  y: number;
}

export interface Velocity {
  vx: number;
  vy: number;
}

/** Release velocity from the tail of a drag: the straight line through
 *  the samples inside the last `windowMs`. Two samples less than 10 ms
 *  apart say nothing reliable and count as a still release. */
export function velocityFrom(samples: Sample[], windowMs = 90): Velocity {
  if (samples.length < 2) return { vx: 0, vy: 0 };
  const last = samples[samples.length - 1];
  let first = last;
  for (let i = samples.length - 2; i >= 0 && last.t - samples[i].t <= windowMs; i--) first = samples[i];
  const dt = (last.t - first.t) / 1000;
  if (dt < 0.01) return { vx: 0, vy: 0 };
  return { vx: (last.x - first.x) / dt, vy: (last.y - first.y) / dt };
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
  /** The window's id, when it is one; see `Shelf` in shelf.rs. */
  id?: number;
}

/** The part of a window's top edge nothing in front of it covers: what
 *  the pet can actually be seen standing on. */
export interface Ledge {
  x1: number;
  x2: number;
  y: number;
  id?: number;
}

const LEDGE_MIN = 40;

/** Visible top edges of `shelves`, which come front to back. A window
 *  hidden behind another is not a place to stand, even where its frame
 *  would be. */
export function ledges(shelves: Rect[]): Ledge[] {
  const out: Ledge[] = [];
  shelves.forEach((s, i) => {
    let parts: [number, number][] = [[s.x, s.x + s.width]];
    for (const f of shelves.slice(0, i)) {
      if (!(f.y <= s.y && s.y < f.y + f.height)) continue;
      const a = f.x;
      const b = f.x + f.width;
      parts = parts.flatMap(([p, q]): [number, number][] => {
        if (b <= p || a >= q) return [[p, q]];
        return ([[p, Math.min(a, q)], [Math.max(b, p), q]] as [number, number][]).filter(([u, v]) => v - u > 0);
      });
    }
    for (const [x1, x2] of parts) if (x2 - x1 >= LEDGE_MIN) out.push({ x1, x2, y: s.y, id: s.id });
  });
  return out;
}

/** Where the window's origin may go: the work area minus the window,
 *  plus other apps' windows the pet can stand on. */
export interface Bounds {
  minX: number;
  maxX: number;
  minY: number;
  /** The work area's floor, as an origin y. */
  maxY: number;
  pet: { width: number; height: number };
  shelves: Rect[];
}

/** The origin y the pet rests at: the top of the highest window under
 *  it that it overlaps by at least a third of its width, else the
 *  work area's floor. A window whose top is above the pet's feet is
 *  around the pet, not under it, and does not count. */
export function floorFor(bounds: Bounds, x: number, y: number): number {
  return support(bounds, x, y).y;
}

/** Where the pet would rest from here and on what: the highest visible
 *  ledge under it that it overlaps by a third of its width, else the
 *  work area's floor (`ledge` null). */
export function support(bounds: Bounds, x: number, y: number): { y: number; ledge: Ledge | null } {
  const { width, height } = bounds.pet;
  let best: { y: number; ledge: Ledge | null } = { y: bounds.maxY, ledge: null };
  for (const l of ledges(bounds.shelves)) {
    const overlap = Math.min(x + width, l.x2) - Math.max(x, l.x1);
    if (overlap < width / 3) continue;
    const top = l.y - height;
    if (top < y - 1 || top >= best.y) continue;
    best = { y: top, ledge: l };
  }
  return best;
}

export interface Body extends Velocity {
  x: number;
  y: number;
}

export const GRAVITY = 2400;
export const WALL_BOUNCE = 0.5;
export const FLOOR_BOUNCE = 0.35;
export const FLOOR_FRICTION = 1500;
/** Slower releases are a placement, not a throw. */
export const FLING_SPEED = 500;
const REST_SPEED = 30;
const SETTLE_VY = 200;

export function step(b: Body, dt: number, bounds: Bounds): Body {
  let { x, y, vx, vy } = b;
  vy += GRAVITY * dt;
  x += vx * dt;
  // The floor is judged from where the feet were, so a pet falling
  // past a window's top this frame lands on it instead of clipping in.
  const floor = floorFor(bounds, x, y);
  y += vy * dt;
  if (x < bounds.minX) {
    x = bounds.minX;
    vx = Math.abs(vx) * WALL_BOUNCE;
  } else if (x > bounds.maxX) {
    x = bounds.maxX;
    vx = -Math.abs(vx) * WALL_BOUNCE;
  }
  if (y < bounds.minY) {
    y = bounds.minY;
    vy = Math.abs(vy) * WALL_BOUNCE;
  } else if (y >= floor) {
    y = floor;
    vy = vy > SETTLE_VY ? -vy * FLOOR_BOUNCE : 0;
    const slow = Math.min(Math.abs(vx), FLOOR_FRICTION * dt);
    vx -= Math.sign(vx) * slow;
  }
  return { x, y, vx, vy };
}

export function airborne(b: Body, bounds: Bounds): boolean {
  return b.y < floorFor(bounds, b.x, b.y) - 0.5;
}

export function atRest(b: Body, bounds: Bounds): boolean {
  return !airborne(b, bounds) && Math.abs(b.vx) < REST_SPEED && Math.abs(b.vy) < REST_SPEED;
}

export const HOP_HEIGHT = 34;
export const HOP_MS = 380;
export const HOP_MAX = 100;

/** Height above the start, in logical pixels, `k` (0 to 1) through a
 *  small hop: a parabola peaking at `HOP_HEIGHT` halfway. */
export function hopLift(k: number): number {
  return 4 * HOP_HEIGHT * k * (1 - k);
}

/** Where a sideways hop of `dx` may land: no further than `HOP_MAX`,
 *  and inside `range` (the edge the pet stands on, or the floor). */
export function hopTarget(x: number, dx: number, range: { minX: number; maxX: number }): number {
  const d = Math.max(-HOP_MAX, Math.min(HOP_MAX, dx));
  return Math.max(range.minX, Math.min(range.maxX, x + d));
}
