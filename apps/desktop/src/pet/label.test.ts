import { describe, expect, it } from "vitest";
import { label } from "./label";

describe("label", () => {
  it("puts the project of the shown session in front", () => {
    const sessions = [
      { key: "c:1", project: "sidepaw", state: "waiting" as const, for_ms: 0, quiet_ms: 0 },
      { key: "c:2", state: "thinking" as const, for_ms: 0, quiet_ms: 0 },
    ];
    expect(label({ overall: { state: "waiting" }, sessions, pending: [] })).toBe("sidepaw · waiting (2)");
    expect(label({ overall: { state: "idle" }, sessions: [], pending: [] })).toBe("idle");
  });
});
