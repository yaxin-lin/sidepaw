import { describe, expect, it } from "vitest";
import { airborne, atRest, type Bounds, step, velocityFrom } from "./physics";

const bounds: Bounds = { minX: 0, maxX: 1000, minY: 0, maxY: 600 };

describe("velocityFrom", () => {
  it("uses only the tail of the drag", () => {
    const v = velocityFrom([
      { t: 0, x: 0, y: 0 },
      { t: 500, x: 0, y: 0 },
      { t: 550, x: 50, y: -20 },
      { t: 600, x: 100, y: -40 },
    ]);
    expect(v.vx).toBeCloseTo(1000);
    expect(v.vy).toBeCloseTo(-400);
  });

  it("reads a still or too-short tail as no throw", () => {
    expect(velocityFrom([{ t: 0, x: 0, y: 0 }])).toEqual({ vx: 0, vy: 0 });
    expect(velocityFrom([{ t: 0, x: 0, y: 0 }, { t: 5, x: 90, y: 0 }])).toEqual({ vx: 0, vy: 0 });
  });
});

describe("step", () => {
  it("falls, bounces off walls and settles on the floor inside the bounds", () => {
    let b = { x: 900, y: 100, vx: 1500, vy: -300 };
    let bouncedOffWall = false;
    let t = 0;
    while (!atRest(b, bounds) && t < 20) {
      const next = step(b, 1 / 60, bounds);
      if (next.x === bounds.maxX) bouncedOffWall = true;
      expect(next.x).toBeGreaterThanOrEqual(bounds.minX);
      expect(next.x).toBeLessThanOrEqual(bounds.maxX);
      expect(next.y).toBeLessThanOrEqual(bounds.maxY);
      b = next;
      t += 1 / 60;
    }
    expect(bouncedOffWall).toBe(true);
    expect(atRest(b, bounds)).toBe(true);
    expect(airborne(b, bounds)).toBe(false);
    expect(t).toBeLessThan(10);
  });

  it("does not bounce forever on a gentle landing", () => {
    const b = step({ x: 10, y: bounds.maxY, vx: 0, vy: 100 }, 1 / 60, bounds);
    expect(b.vy).toBe(0);
    expect(b.y).toBe(bounds.maxY);
  });
});
