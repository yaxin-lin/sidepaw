import { describe, expect, it } from "vitest";
import type { ChatTurn } from "../generated/ChatTurn";
import { bubbleBlock, toHistory } from "./chatlog";

const turn = (id: number, role: string, channel: string, body: string, ts = "2026-10-30T09:00:00.000Z"): ChatTurn => ({ id, ts, role, channel, body });

describe("toHistory", () => {
  it("keeps chat turns alternating and starts with the user", () => {
    const h = toHistory([
      turn(1, "pet", "bubble", "the PR is green"),
      turn(2, "pet", "chat", "orphaned by the window cut"),
      turn(3, "user", "chat", "hi"),
      turn(4, "user", "chat", "are you there"),
      turn(5, "pet", "chat", "yes"),
    ]);
    expect(h).toEqual([
      { role: "user", content: "hi\n\nare you there" },
      { role: "assistant", content: "yes" },
    ]);
  });

  it("is empty without chat turns", () => {
    expect(toHistory([turn(1, "pet", "bubble", "hop")])).toEqual([]);
  });
});

describe("bubbleBlock", () => {
  it("lists bubble lines with their age and nothing else", () => {
    const now = Date.parse("2026-10-30T10:00:00.000Z");
    const block = bubbleBlock(
      [turn(1, "user", "chat", "hi"), turn(2, "pet", "bubble", "two errors in a row", "2026-10-30T09:58:00.000Z"), turn(3, "pet", "bubble", "done after 25 min", "2026-10-30T07:00:00.000Z")],
      now,
    );
    expect(block).toContain("- 2 min ago: two errors in a row");
    expect(block).toContain("- 3 h ago: done after 25 min");
    expect(block).not.toContain("hi");
    expect(bubbleBlock([turn(1, "user", "chat", "hi")], now)).toBe("");
  });
});
