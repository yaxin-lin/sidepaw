import { describe, expect, it } from "vitest";
import type { State } from "../generated/State";
import type { SessionView } from "../generated/SessionView";
import { fill, type Gate, inQuietHours, mayFire, type Observation, shouldOfferLevel, Watcher } from "./proactive";

const MIN = 60_000;
const t0 = Date.UTC(2026, 9, 27, 2, 0);

function obs(now: number, overall: State, extra: Partial<Observation> = {}): Observation {
  return { now, hour: 10, overall, sessions: [], userActiveAt: now, ...extra };
}

describe("Watcher", () => {
  it("offers waiting_long only after three minutes of waiting", () => {
    const w = new Watcher();
    const s = (for_ms: number): SessionView[] => [{ key: "c:1", state: "waiting", for_ms, quiet_ms: 0 }];
    expect(w.observe(obs(t0, { state: "waiting" }, { sessions: s(2 * MIN) }))).not.toContain("waiting_long");
    expect(w.observe(obs(t0, { state: "waiting" }, { sessions: s(3 * MIN) }))).toContain("waiting_long");
  });

  it("counts errors in a row and forgets them after a finished turn", () => {
    const w = new Watcher();
    w.observe(obs(t0, { state: "error" }));
    w.observe(obs(t0 + 1, { state: "thinking" }));
    expect(w.observe(obs(t0 + 2, { state: "error" }))).toContain("errors");
    w.consume("errors", t0 + 2);
    expect(w.observe(obs(t0 + 3, { state: "error" }))).not.toContain("errors");
    const v = new Watcher();
    v.observe(obs(t0, { state: "error" }));
    v.observe(obs(t0 + 1, { state: "done" }));
    expect(v.observe(obs(t0 + 2, { state: "error" }))).not.toContain("errors");
  });

  it("notices a long task finishing, but not a short one", () => {
    const w = new Watcher();
    w.observe(obs(t0, { state: "thinking" }));
    expect(w.observe(obs(t0 + 25 * MIN, { state: "done" }))).toContain("long_done");
    const v = new Watcher();
    v.observe(obs(t0, { state: "thinking" }));
    expect(v.observe(obs(t0 + 5 * MIN, { state: "done" }))).not.toContain("long_done");
  });

  it("suggests a break after two hours without a pause, and a welcome back after time away", () => {
    const w = new Watcher();
    for (let m = 0; m <= 125; m += 5) w.observe(obs(t0 + m * MIN, { state: "thinking" }));
    expect(w.observe(obs(t0 + 126 * MIN, { state: "thinking" }))).toContain("break");
    const v = new Watcher();
    v.observe(obs(t0, { state: "idle" }));
    // 20 minutes without the cursor moving, then it moves.
    v.observe(obs(t0 + 20 * MIN, { state: "idle" }, { userActiveAt: t0 }));
    expect(v.observe(obs(t0 + 20 * MIN, { state: "idle" }))).toContain("back");
  });

  it("puts what needs the user first", () => {
    const w = new Watcher();
    const s: SessionView[] = [{ key: "c:1", state: "waiting", for_ms: 5 * MIN, quiet_ms: 0 }];
    const out = w.observe(obs(t0, { state: "waiting" }, { sessions: s, hour: 9 }));
    expect(out[0]).toBe("waiting_long");
    expect(out).toContain("morning");
  });
});

describe("mayFire", () => {
  const base: Gate = { chatty: 2, dnd: false, quiet: false, now: t0, today: "2026-10-27", lastSpokeAt: 0, lastHour: 0, dayOf: (iso) => iso.slice(0, 10) };

  it("respects chattiness, do-not-disturb and quiet hours", () => {
    expect(mayFire("long_done", base)).toBe(true);
    expect(mayFire("break", base)).toBe(false);
    expect(mayFire("waiting_long", { ...base, chatty: 0 })).toBe(false);
    expect(mayFire("waiting_long", { ...base, dnd: true })).toBe(false);
    expect(mayFire("waiting_long", { ...base, quiet: true })).toBe(true);
    expect(mayFire("long_done", { ...base, quiet: true })).toBe(false);
  });

  it("spaces unprompted lines, but not ones that need the user", () => {
    const g = { ...base, lastSpokeAt: t0 - 30_000 };
    expect(mayFire("long_done", g)).toBe(false);
    expect(mayFire("waiting_long", g)).toBe(true);
  });

  it("waits out the learned cooldown and says daily things once a day", () => {
    const stat = { trigger: "long_done", fired: 1, dismissed: 1, answered: 0, last_fired: new Date(t0 - 15 * MIN).toISOString(), cooldown_s: 1200 };
    expect(mayFire("long_done", { ...base, stat })).toBe(false);
    expect(mayFire("long_done", { ...base, stat: { ...stat, cooldown_s: 600 } })).toBe(true);
    const report = { ...stat, trigger: "report", last_fired: "2026-10-27T00:10:00.000Z", cooldown_s: 0 };
    expect(mayFire("report", { ...base, stat: report })).toBe(false);
    expect(mayFire("report", { ...base, stat: { ...report, last_fired: "2026-10-26T00:10:00.000Z" } })).toBe(true);
  });

  it("caps small talk at three an hour at level 4", () => {
    expect(mayFire("chatter", { ...base, chatty: 4, lastHour: 3 })).toBe(false);
    expect(mayFire("chatter", { ...base, chatty: 5, lastHour: 3 })).toBe(true);
  });
});

describe("shouldOfferLevel", () => {
  const day = 86_400_000;
  const base = { level: 1, daysActive: 3, firstEventAt: t0 - 4 * day, now: t0 };
  it("asks at level 1 after three days of use", () => {
    expect(shouldOfferLevel(base)).toBe(true);
    expect(shouldOfferLevel({ ...base, level: 2 })).toBe(false);
    expect(shouldOfferLevel({ ...base, daysActive: 2 })).toBe(false);
    expect(shouldOfferLevel({ ...base, firstEventAt: t0 - day })).toBe(false);
    expect(shouldOfferLevel({ ...base, firstEventAt: null })).toBe(false);
  });

  it("asks twice at most, a month apart, and never again once answered", () => {
    const stat = { trigger: "offer_level", fired: 1, dismissed: 1, answered: 0, last_fired: new Date(t0 - 10 * day).toISOString(), cooldown_s: 0 };
    expect(shouldOfferLevel({ ...base, stat })).toBe(false);
    expect(shouldOfferLevel({ ...base, stat: { ...stat, last_fired: new Date(t0 - 31 * day).toISOString() } })).toBe(true);
    expect(shouldOfferLevel({ ...base, stat: { ...stat, fired: 2, last_fired: new Date(t0 - 90 * day).toISOString() } })).toBe(false);
    expect(shouldOfferLevel({ ...base, stat: { ...stat, answered: 1, last_fired: new Date(t0 - 90 * day).toISOString() } })).toBe(false);
  });
});

describe("inQuietHours and fill", () => {
  it("handles ranges across midnight", () => {
    const at = (h: number, m = 0) => new Date(2026, 9, 27, h, m);
    expect(inQuietHours("23:00", "08:00", at(23, 30))).toBe(true);
    expect(inQuietHours("23:00", "08:00", at(7, 59))).toBe(true);
    expect(inQuietHours("23:00", "08:00", at(8))).toBe(false);
    expect(inQuietHours("13:00", "14:00", at(13, 30))).toBe(true);
    expect(inQuietHours("bad", "08:00", at(1))).toBe(false);
  });

  it("fills known placeholders and leaves the rest", () => {
    expect(fill("{tools} tools, {nope}", { tools: 12 })).toBe("12 tools, {nope}");
  });
});
