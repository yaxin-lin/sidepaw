import { describe, expect, it } from "vitest";
import { compileSystemPrompt, personaDigest, type Persona } from "./persona";

const mochi: Persona = {
  version: 1,
  name: "Mochi",
  callsUser: "boss",
  species: "cat",
  sliders: { talkative: 0.1, sharp: 0.9 },
  strengths: ["observant", "Encouraging"],
  flaws: ["lazy", "left-handed"],
  catchphrases: ["again?"],
  avoid: ["politics"],
  language: "zh-CN",
};

describe("compileSystemPrompt", () => {
  it("names the pet and the user and sets the language", () => {
    const p = compileSystemPrompt(mochi);
    expect(p).toContain("You are Mochi, a desktop pet (cat) who watches boss's AI coding agents work.");
    expect(p).toContain('Call the user "boss"');
    expect(p).toContain("language with code zh-CN");
    expect(p).toContain("answer in that language");
  });

  it("falls back to the UI language only when the persona has none", () => {
    expect(compileSystemPrompt({ version: 1, name: "X" }, undefined, "en")).toContain("code en by default");
    expect(compileSystemPrompt(mochi, undefined, "en")).toContain("code zh-CN by default");
    expect(compileSystemPrompt({ version: 1, name: "X" })).not.toContain("by default");
  });

  it("turns sliders into one sentence each, by thirds", () => {
    const p = compileSystemPrompt(mochi);
    expect(p).toContain("Keep replies to one short sentence");
    expect(p).toContain("sharp-tongued");
    expect(p).toContain("Comment on notable events"); // unset sliders read as the middle;
  });

  it("turns known tags into rules and keeps unknown ones as words", () => {
    const p = compileSystemPrompt(mochi);
    expect(p).toContain("You notice details");
    expect(p).toContain("reassure first"); // tags match case-insensitively;
    expect(p).toContain("shortest answer");
    expect(p).toContain("You are also: left-handed.");
  });

  it("appends free text last and verbatim", () => {
    const p = compileSystemPrompt(mochi, "  Backstory: born in a server room.  ");
    expect(p.endsWith("Backstory: born in a server room.")).toBe(true);
    expect(compileSystemPrompt(mochi, "   ")).not.toContain("Backstory");
  });

  it("works with the minimum persona", () => {
    const p = compileSystemPrompt({ version: 1, name: "Dot" });
    expect(p).toContain("You are Dot, a desktop pet who watches the user's");
    expect(p).not.toContain("Catchphrases");
  });
});

describe("personaDigest", () => {
  it("changes with any edit and is stable otherwise", async () => {
    const a = await personaDigest(mochi);
    expect(a).toMatch(/^[0-9a-f]{64}$/);
    expect(await personaDigest(mochi)).toBe(a);
    expect(await personaDigest({ ...mochi, name: "Mochi2" })).not.toBe(a);
    expect(await personaDigest(mochi, "x")).not.toBe(a);
  });
});
