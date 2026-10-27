import { describe, expect, it } from "vitest";
import { rescue } from "./onscreen";

// A 1728x1080 point screen at 2x, as on the machine this was found on.
const screen = { x: 0, y: 0, width: 3456, height: 2160 };
const pet = (x: number, y: number) => ({ x, y, width: 288, height: 450 });

describe("rescue", () => {
  it("leaves a visible pet alone, including one parked at the edge", () => {
    expect(rescue(pet(1000, 1700), [screen])).toBeNull();
    // Mini mode: 40 % of the width past the right edge is still shown.
    expect(rescue(pet(3456 - 115, 1700), [screen])).toBeNull();
  });

  it("brings back a pet that is off every screen, to the nearest one", () => {
    expect(rescue(pet(3960, 3142), [screen])).toEqual({ x: 3456 - 288 - 24, y: 2160 - 450 });
    const left = { x: -2560, y: 0, width: 2560, height: 1440 };
    expect(rescue(pet(-4000, 200), [screen, left])).toEqual({ x: -288 - 24, y: 1440 - 450 });
  });

  it("does nothing without screen information", () => {
    expect(rescue(pet(9999, 9999), [])).toBeNull();
  });
});
