import { describe, expect, it } from "vitest";
import { buildMask, CELL } from "./hitmask";

const bit = (rows: number[], x: number, y: number) =>
  (rows[Math.floor(y / CELL)] & (2 ** Math.floor(x / CELL))) !== 0;

describe("buildMask", () => {
  it("places canvas pixels at the canvas's position in the window, not at the origin", () => {
    // Window 192x300, canvas 192x208 starting 60px down, fully opaque.
    const rows = buildMask({
      width: 192,
      height: 300,
      canvas: { left: 0, top: 60, right: 192, bottom: 268 },
      opaqueAt: () => true,
      solid: [],
    });
    expect(rows).toHaveLength(75);
    expect(bit(rows, 96, 10)).toBe(false); // above the canvas: transparent
    expect(bit(rows, 96, 100)).toBe(true); // on the canvas
    expect(bit(rows, 96, 290)).toBe(false); // below it
  });

  it("maps canvas-local coordinates when sampling", () => {
    const seen: Array<[number, number]> = [];
    buildMask({
      width: 8,
      height: 8,
      canvas: { left: 4, top: 4, right: 8, bottom: 8 },
      opaqueAt: (x, y) => {
        seen.push([x, y]);
        return false;
      },
      solid: [],
    });
    expect(seen).toEqual([[2, 2]]); // the one cell over the canvas, at its local centre
  });

  it("marks solid boxes opaque even where the canvas is transparent", () => {
    const rows = buildMask({
      width: 16,
      height: 16,
      canvas: { left: 0, top: 0, right: 16, bottom: 16 },
      opaqueAt: () => false,
      solid: [{ left: 8, top: 8, right: 16, bottom: 16 }],
    });
    expect(bit(rows, 2, 2)).toBe(false);
    expect(bit(rows, 12, 12)).toBe(true);
  });
});
