import { describe, expect, it } from "vitest";
import type { World } from "./desktop";
import { plan, type PlanInput } from "./roam";

const pet = { width: 100, height: 150 };
function world(shelves: World["scene"]["shelves"], at = { x: 50, y: 850 }): World {
  return {
    scale: 1,
    pet: at,
    bounds: { minX: 0, maxX: 1500, minY: 0, maxY: 850, pet, shelves },
    scene: { shelves, fullscreen: false, front_app: null },
  };
}
const term = { x: 600, y: 300, width: 500, height: 400, id: 11, pid: 900, front: false };
const editor = { x: 100, y: 200, width: 300, height: 500, id: 22, pid: 500, front: true };
const base = (w: World, extra: Partial<PlanInput> = {}): PlanInput => ({
  world: w,
  waiting: null,
  visited: null,
  wander: false,
  night: false,
  inBed: false,
  idle: true,
  random: () => 0.1,
  ...extra,
});

describe("plan", () => {
  it("goes to the window of the session that waits, near its right end, once", () => {
    const w = world([term, editor]);
    const g = plan(base(w, { waiting: { key: "c:1", pids: [42, 900] } }));
    expect(g?.kind).toBe("to_session");
    expect(g?.y).toBe(300 - 150);
    expect(g!.x).toBeGreaterThan(900);
    expect(plan(base(w, { waiting: { key: "c:1", pids: [900] }, visited: "c:1" }))).toBeNull();
    // No window for that process: nothing to go to.
    expect(plan(base(w, { waiting: { key: "c:2", pids: [7] } }))).toBeNull();
  });

  it("wanders onto the front window from the floor, and off it again", () => {
    const g = plan(base(world([term, editor]), { wander: true }));
    expect(g?.kind).toBe("to_front");
    expect(g?.y).toBe(200 - 150);
    const onEditor = world([term, editor], { x: 200, y: 50 });
    expect(plan(base(onEditor, { wander: true }))?.kind).toBe("down");
    expect(plan(base(onEditor, { wander: true, random: () => 0.5 }))?.kind).toBe("hop");
  });

  it("does nothing between wanders, while busy, or when hidden behind another window", () => {
    expect(plan(base(world([term, editor])))).toBeNull();
    expect(plan(base(world([term, editor]), { wander: true, idle: false }))).toBeNull();
    // The front window's top edge is fully covered by a window in front.
    const cover = { x: 0, y: 100, width: 600, height: 700, id: 33, pid: 1, front: false };
    expect(plan(base(world([cover, editor]), { wander: true }))).toBeNull();
  });

  it("goes to bed once a night", () => {
    expect(plan(base(world([]), { night: true }))?.kind).toBe("bed");
    expect(plan(base(world([]), { night: true, inBed: true, wander: true }))).toBeNull();
  });
});
