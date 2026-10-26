import { describe, expect, it } from "vitest";
import { keysFor, Lines } from "./lines";

describe("Lines.pick", () => {
  it("falls back from the specific key to the general one", () => {
    const lines = new Lines({ version: 1, tool: ["busy"] }, () => 0);
    expect(lines.pick(["tool.shell", "tool"])).toBe("busy");
    expect(lines.pick("waiting")).toBeNull();
  });

  it("avoids the last three lines while there are others", () => {
    const lines = new Lines({ version: 1, idle: ["a", "b", "c", "d"] }, () => 0);
    expect([lines.pick("idle"), lines.pick("idle"), lines.pick("idle"), lines.pick("idle")]).toEqual([
      "a",
      "b",
      "c",
      "d",
    ]);
    expect(lines.pick("idle")).toBe("a");
  });

  it("still speaks when every line is recent", () => {
    const lines = new Lines({ version: 1, idle: ["only"] }, () => 0.99);
    expect(lines.pick("idle")).toBe("only");
    expect(lines.pick("idle")).toBe("only");
  });

  it("ignores malformed entries", () => {
    const lines = new Lines({ version: 1, idle: [1, "", "ok"] as unknown[] }, () => 0);
    expect(lines.pick("idle")).toBe("ok");
  });
});

describe("keysFor", () => {
  it("asks for the tool category first", () => {
    expect(keysFor({ state: "tool", detail: "shell" })).toEqual(["tool.shell", "tool"]);
    expect(keysFor({ state: "sleeping" })).toEqual(["night", "idle"]);
    expect(keysFor({ state: "waiting" })).toEqual(["waiting"]);
  });
});
