import { describe, expect, it } from "vitest";
import { directRemember, extractRemember, hidePartialMarker, memoryBlock } from "./memory";

const mem = (id: number, body: string) => ({ id, created_at: "", source: "user", body, retention: "forever" });

describe("memoryBlock", () => {
  it("lists memories, or says there are none, and explains the marker", () => {
    expect(memoryBlock([mem(1, "calls me boss")])).toContain("- calls me boss");
    expect(memoryBlock([])).toContain("do not remember anything");
    expect(memoryBlock([])).toContain("[remember:");
  });
});

describe("extractRemember", () => {
  it("strips the marker and keeps one short fact", () => {
    const r = extractRemember("Got it, tabs it is.\n[remember: prefers tabs over spaces]");
    expect(r.reply).toBe("Got it, tabs it is.");
    expect(r.facts).toEqual(["prefers tabs over spaces"]);
    expect(extractRemember("No marker here.").facts).toEqual([]);
    expect(extractRemember("a\n[remember: x]\n[remember: y]").facts).toEqual(["x"]);
  });

  it("hides a half-streamed marker", () => {
    expect(hidePartialMarker("Sure.\n[remem")).toBe("Sure.");
    expect(hidePartialMarker("Sure.\n[remember: likes te")).toBe("Sure.");
    expect(hidePartialMarker("Arrays [0] are fine")).toBe("Arrays [0] are fine");
  });
});

describe("directRemember", () => {
  it("takes what follows 记住 or remember", () => {
    expect(directRemember("记住：我叫小林")).toBe("我叫小林");
    expect(directRemember("请记住 我用 pnpm")).toBe("我用 pnpm");
    expect(directRemember("Remember that I work on sidepaw")).toBe("I work on sidepaw");
    expect(directRemember("what do you remember?")).toBeNull();
    expect(directRemember("记住")).toBeNull();
  });
});
