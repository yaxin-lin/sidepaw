import { describe, expect, it } from "vitest";
import { buildContext } from "./context";
import type { EventRow } from "../generated/EventRow";

const report = { since: "", sessions: 2, turns: 7, tool_calls: 30, errors: 1, tokens_in: 0, tokens_out: 0, waiting_ms: 125_000, tools: [] };

describe("buildContext", () => {
  it("describes sessions, counters and events oldest first, clipped", () => {
    const events: EventRow[] = [
      { ts: "2026-10-27T03:05:00.000Z", host: "claude-code", project: "sidepaw", kind: "tool_fail", tool_name: "Bash", text: null, error_type: "exit", error_msg: "x".repeat(300) },
      { ts: "2026-10-27T03:04:00.000Z", host: "claude-code", project: "sidepaw", kind: "tool_start", tool_name: "Bash", text: "cargo   test\n--workspace", error_type: null, error_msg: null },
    ];
    const text = buildContext({
      snapshot: { overall: { state: "tool", detail: "shell" }, sessions: [{ key: "claude-code:1", project: "sidepaw", state: "tool", detail: "shell", for_ms: 12_000, quiet_ms: 0 }], pending: [] },
      report,
      events,
    });
    const lines = text.split("\n");
    expect(lines[0]).toBe("Now:");
    expect(lines[1]).toBe("- claude-code in sidepaw: tool (shell) for 12 s");
    expect(lines[2]).toBe("Today: 2 sessions, 7 turns, 30 tool calls, 1 errors, waited on the user 2 min.");
    expect(lines[4]).toMatch(/^\d\d:\d\d claude-code\/sidepaw tool_start Bash: cargo test --workspace$/);
    expect(lines[5]).toMatch(/tool_fail Bash \[exit: x+…\]$/);
    expect(lines[5].length).toBeLessThan(220);
  });

  it("says so when nothing runs and caps the event count", () => {
    const many: EventRow[] = Array.from({ length: 20 }, (_, i) => ({ ts: `2026-10-27T03:00:${String(i).padStart(2, "0")}.000Z`, host: "codex", project: null, kind: "turn_end", tool_name: null, text: null, error_type: null, error_msg: null }));
    const text = buildContext({ snapshot: { overall: { state: "idle" }, sessions: [], pending: [] }, report, events: many }, 3);
    expect(text).toContain("Now: no agent session is running.");
    expect(text.split("\n").filter((l) => l.includes("turn_end"))).toHaveLength(3);
  });
});
