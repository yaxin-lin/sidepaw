import { describe, expect, it } from "vitest";
import { cleanLine, dailyCap, DAILY_CAP, FREE_CAP, nudgeRequest, RECENT_LINES } from "./nudge";
import { ORDER } from "./proactive";

describe("nudgeRequest", () => {
  it("describes the moment and the context, and asks for one line", () => {
    const r = nudgeRequest("errors", "PERSONA", "Now:\n- claude-code: error for 5 s");
    expect(r.purpose).toBe("nudge");
    expect(r.system).toBe("PERSONA");
    expect(r.user).toContain("failed twice in a row");
    expect(r.user).toContain("claude-code: error for 5 s");
    expect(r.user).not.toContain("already said");
    expect(r.maxTokens).toBeLessThanOrEqual(200);
  });

  it("has a situation for every trigger, with the moment's numbers", () => {
    for (const id of ORDER) expect(nudgeRequest(id, "P", "C").user.split("\n")[0].length).toBeGreaterThan(20);
    expect(nudgeRequest("back", "P", "C", { news: { minutes: 25, done: 1, errors: 0, waiting: 2 } }).user).toContain("25 minutes away. Meanwhile 1 sessions finished, 0 failed, and 2 are waiting");
    expect(nudgeRequest("report", "P", "C", { counters: { tools: 40, turns: 9, errors: 1, sessions: 2, waited: 7 } }).user).toContain("2 sessions, 9 turns, 40 tool calls, 1 errors, 7 minutes");
    expect(nudgeRequest("app", "P", "C", { app: "browser" }).user).toContain("switched to a browser app");
  });

  it("quotes the day's lines back and the pack line for tone", () => {
    const recent = Array.from({ length: RECENT_LINES + 3 }, (_, i) => `line ${i}`);
    const r = nudgeRequest("chatter", "P", "C", { recent, packLine: "Quiet day." });
    expect(r.user).toContain("do not repeat");
    expect(r.user).not.toContain("- line 2\n");
    expect(r.user).toContain(`- line ${RECENT_LINES + 2}`);
    expect(r.user).toContain("for tone only: Quiet day.");
  });
});

describe("dailyCap", () => {
  it("is loose where a call is free", () => {
    expect(dailyCap({ provider: "anthropic", local: false })).toBe(DAILY_CAP);
    expect(dailyCap({ provider: "claude-code", local: false })).toBe(FREE_CAP);
    expect(dailyCap({ provider: "ollama", local: true })).toBe(FREE_CAP);
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
