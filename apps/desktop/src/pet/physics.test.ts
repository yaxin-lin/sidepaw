import { describe, expect, it } from "vitest";
import { airborne, atRest, type Bounds, floorFor, step, velocityFrom } from "./physics";

const bounds: Bounds = { minX: 0, maxX: 1000, minY: 0, maxY: 600, pet: { width: 100, height: 100 }, shelves: [] };

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

  it("lands on a window under it and ignores one around it", () => {
    const shelved: Bounds = { ...bounds, shelves: [{ x: 200, y: 300, width: 400, height: 300 }] };
    // Standing position on the shelf is y = 300 - 100.
    expect(floorFor(shelved, 300, 0)).toBe(200);
    expect(floorFor(shelved, 300, 200)).toBe(200);
    // Only a sliver of overlap: not standing on it.
    expect(floorFor(shelved, 580, 0)).toBe(600);
    // Already below the window's top: it is around the pet, fall through.
    expect(floorFor(shelved, 300, 250)).toBe(600);
    let b = { x: 300, y: 0, vx: 0, vy: 0 };
    for (let i = 0; i < 600 && !atRest(b, shelved); i++) b = step(b, 1 / 60, shelved);
    expect(b.y).toBe(200);
    expect(airborne(b, shelved)).toBe(false);
  });

  it("does not bounce forever on a gentle landing", () => {
    const b = step({ x: 10, y: bounds.maxY, vx: 0, vy: 100 }, 1 / 60, bounds);
    expect(b.vy).toBe(0);
    expect(b.y).toBe(bounds.maxY);
  });
});
