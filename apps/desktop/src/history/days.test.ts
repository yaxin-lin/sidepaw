import { describe, expect, it } from "vitest";
import { compact, fillDays, niceScale, sum } from "./days";

const row = (day: string, tool_calls: number) => ({ day, sessions: 1, turns: 1, tool_calls, errors: 0, tokens: 10, waiting_ms: 60_000 });

describe("fillDays", () => {
  it("gives every day of the range, oldest first, zeros where idle", () => {
    const out = fillDays([row("2026-10-25", 7)], 4, new Date(2026, 9, 27, 15));
    expect(out.map((d) => d.day)).toEqual(["2026-10-24", "2026-10-25", "2026-10-26", "2026-10-27"]);
    expect(out.map((d) => d.tool_calls)).toEqual([0, 7, 0, 0]);
  });

  it("crosses a month boundary", () => {
    expect(fillDays([], 3, new Date(2026, 10, 1)).map((d) => d.day)).toEqual(["2026-10-30", "2026-10-31", "2026-11-01"]);
  });
});

describe("niceScale", () => {
  it("rounds the top up to a 1-2-5 step", () => {
    expect(niceScale(0)).toEqual({ top: 4, ticks: [0, 1, 2, 3, 4] });
    expect(niceScale(7)).toEqual({ top: 8, ticks: [0, 2, 4, 6, 8] });
    expect(niceScale(130).top).toBe(150);
    expect(niceScale(130).ticks).toEqual([0, 50, 100, 150]);
  });
});

describe("totals and formatting", () => {
  it("sums and abbreviates", () => {
    expect(sum([row("a", 2), row("b", 3)]).tool_calls).toBe(5);
    expect(compact(950)).toBe("950");
    expect(compact(1250)).toBe("1.3k");
    expect(compact(48_000)).toBe("48k");
    expect(compact(2_400_000)).toBe("2.4M");
  });
});
