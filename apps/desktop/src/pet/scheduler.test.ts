import { describe, expect, it } from "vitest";
import type { Snapshot } from "../generated/Snapshot";
import { withProject } from "./scheduler";

const snap = (sessions: Snapshot["sessions"]): Snapshot => ({ overall: sessions[0] ?? { state: "idle" }, sessions, pending: [] }) as Snapshot;

describe("withProject", () => {
  it("names the project only when several sessions run", () => {
    const one = snap([{ key: "c:1", project: "sidepaw", state: "waiting", for_ms: 0, quiet_ms: 0 }]);
    expect(withProject("waiting_long", one, "Still waiting.")).toBe("Still waiting.");
    const two = snap([
      { key: "c:2", project: "blog", state: "thinking", for_ms: 0, quiet_ms: 0 },
      { key: "c:1", project: "sidepaw", state: "waiting", for_ms: 0, quiet_ms: 0 },
    ]);
    expect(withProject("waiting_long", two, "Still waiting.")).toBe("sidepaw: Still waiting.");
    expect(withProject("errors", two, "Again?")).toBe("blog: Again?");
    expect(withProject("chatter", two, "Quiet day.")).toBe("Quiet day.");
  });
});
