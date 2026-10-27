import { describe, expect, it } from "vitest";
import { countLines, LINE_KEYS, LINES_PER_KEY, linesRequest, parseLines } from "./linegen";

describe("linesRequest", () => {
  it("asks for every renderer key by name", () => {
    const r = linesRequest("SYS");
    expect(r.system).toBe("SYS");
    for (const { key } of LINE_KEYS) expect(r.user).toContain(`"${key}"`);
    expect(r.user).toContain(String(LINES_PER_KEY));
    expect(r.purpose).toBe("lines");
  });
});

describe("parseLines", () => {
  it("tolerates fences and prose, drops junk, dedupes and records the digest", () => {
    const text = 'Sure! ```json\n{"idle": [" Nap time ", "Nap time", "", 5, "\\"quoted\\""], "bogus": ["x"], "error": "not a list"}\n```';
    const file = parseLines(text, "abc");
    expect(file).toEqual({ version: 1, generatedFrom: "abc", idle: ["Nap time", "quoted"] });
    expect(countLines(file)).toBe(2);
  });

  it("rejects replies with nothing usable", () => {
    expect(() => parseLines("no json here", "d")).toThrow("no JSON");
    expect(() => parseLines("{not json}", "d")).toThrow("not valid JSON");
    expect(() => parseLines('{"bogus": ["x"]}', "d")).toThrow("no usable lines");
    expect(() => parseLines("[1,2]", "d")).toThrow();
  });
});
