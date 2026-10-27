import { describe, expect, it } from "vitest";
import { HOP_MAX } from "./physics";
import { plan, type PlanInput } from "./roam";

const base = (extra: Partial<PlanInput> = {}): PlanInput => ({ waiting: null, noticed: null, due: true, idle: true, random: () => 0.9, ...extra });

describe("plan", () => {
  it("hops for attention once per waiting session, on the spot", () => {
    expect(plan(base({ waiting: "c:1", due: false }))).toEqual({ kind: "attention", dx: 0 });
    expect(plan(base({ waiting: "c:1", noticed: "c:1", due: false }))).toBeNull();
  });

  it("hops a little, only when idle and when it is time", () => {
    const g = plan(base());
    expect(g?.kind).toBe("hop");
    expect(Math.abs(g!.dx)).toBeLessThanOrEqual(HOP_MAX);
    expect(plan(base({ random: () => 0.1 }))?.dx).toBe(0);
    expect(plan(base({ due: false }))).toBeNull();
    expect(plan(base({ idle: false }))).toBeNull();
  });
});
