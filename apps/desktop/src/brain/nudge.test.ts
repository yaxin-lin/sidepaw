import { describe, expect, it } from "vitest";
import { cleanLine, LIVE, nudgeRequest } from "./nudge";

describe("nudgeRequest", () => {
  it("describes the moment and the context, and asks for one line", () => {
    const r = nudgeRequest("errors", "PERSONA", "Now:\n- claude-code: error for 5 s");
    expect(r.purpose).toBe("nudge");
    expect(r.system).toBe("PERSONA");
    expect(r.user).toContain("failed twice in a row");
    expect(r.user).toContain("claude-code: error for 5 s");
    expect(r.maxTokens).toBeLessThanOrEqual(200);
    expect(LIVE.has("chatter")).toBe(false);
  });
});

describe("cleanLine", () => {
  it("takes the first line without quotes and rejects junk", () => {
    expect(cleanLine('\n"Third try failing on the same test, boss."\n')).toBe("Third try failing on the same test, boss.");
    expect(cleanLine("「又是 TypeError。」")).toBe("又是 TypeError。");
    expect(cleanLine("   ")).toBeNull();
    expect(cleanLine("x".repeat(200))).toBeNull();
  });
});
