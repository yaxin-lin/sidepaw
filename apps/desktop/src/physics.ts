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

/** Where the window's origin may go: the work area minus the window. */
export interface Bounds {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
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
  } else if (y >= bounds.maxY) {
    y = bounds.maxY;
    vy = vy > SETTLE_VY ? -vy * FLOOR_BOUNCE : 0;
    const slow = Math.min(Math.abs(vx), FLOOR_FRICTION * dt);
    vx -= Math.sign(vx) * slow;
  }
  return { x, y, vx, vy };
}

export function airborne(b: Body, bounds: Bounds): boolean {
  return b.y < bounds.maxY - 0.5;
}

export function atRest(b: Body, bounds: Bounds): boolean {
  return !airborne(b, bounds) && Math.abs(b.vx) < REST_SPEED && Math.abs(b.vy) < REST_SPEED;
}
