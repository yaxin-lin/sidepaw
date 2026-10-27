import { describe, expect, it } from "vitest";
import { type Motion, pose } from "./juice";

const still: Motion = { t: 0, breathing: "none", squash: null, lean: 0, sway: 0 };

describe("pose", () => {
  it("is the identity when nothing moves", () => {
    expect(pose(still)).toEqual({ sx: 1, sy: 1, rot: 0 });
  });

  it("breathes a little, more slowly and deeply when asleep", () => {
    const awake = pose({ ...still, breathing: "awake", t: 800 });
    expect(awake.sy).toBeGreaterThan(1);
    expect(awake.sy).toBeLessThan(1.03);
    const asleep = pose({ ...still, breathing: "asleep", t: 1300 });
    expect(asleep.sy - 1).toBeGreaterThan(awake.sy - 1);
  });

  it("squashes wide and short on landing, deepest halfway", () => {
    const mid = pose({ ...still, squash: { kind: "land", k: 0.5 } });
    expect(mid.sy).toBeCloseTo(0.86);
    expect(mid.sx).toBeGreaterThan(1.1);
    expect(pose({ ...still, squash: { kind: "crouch", k: 0.5 } }).sy).toBeGreaterThan(mid.sy);
  });

  it("leans into a walk and adds the drag sway", () => {
    expect(pose({ ...still, lean: 1 }).rot).toBe(5);
    expect(pose({ ...still, lean: -1, sway: 3 }).rot).toBe(-2);
  });
});
