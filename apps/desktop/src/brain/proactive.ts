/** When the pet speaks unprompted. The interesting part is when it
 *  stays quiet: every trigger has a chattiness level it needs, a gap
 *  it learned from how the user reacted (`trigger_stats`), and quiet
 *  hours and do-not-disturb override all but the ones that need the
 *  user. Pure logic; `pet/scheduler.ts` feeds it and speaks. */
import type { SessionView } from "../generated/SessionView";
import type { State } from "../generated/State";
import type { TriggerStat } from "../generated/TriggerStat";

export type TriggerId = "waiting_long" | "errors" | "long_done" | "report" | "morning" | "night" | "break" | "back" | "app" | "hop" | "chatter";

export interface TriggerDef {
  /** The lowest chattiness (0 to 5) at which it may fire. */
  chatty: number;
  /** Shortest gap between two firings, in seconds; dismissals lengthen it. */
  baseS: number;
  /** About something the user has to act on: speaks in quiet hours and
   *  ignores the spacing between lines. */
  needsYou?: boolean;
  /** At most once per local day. */
  daily?: boolean;
}

/** In priority order: when two fire at once, the earlier one speaks. */
export const TRIGGERS: Record<TriggerId, TriggerDef> = {
  waiting_long: { chatty: 1, baseS: 600, needsYou: true },
  errors: { chatty: 1, baseS: 900, needsYou: true },
  long_done: { chatty: 2, baseS: 600 },
  report: { chatty: 2, baseS: 3600, daily: true },
  morning: { chatty: 3, baseS: 3600, daily: true },
  night: { chatty: 3, baseS: 3600, daily: true },
  break: { chatty: 3, baseS: 7200 },
  back: { chatty: 3, baseS: 3600 },
  /** Switching to a kind of app (`app` in the observation). */
  app: { chatty: 3, baseS: 900 },
  /** A line with one of the pet's own small hops (`pet/roam.ts`). */
  hop: { chatty: 2, baseS: 240 },
  chatter: { chatty: 4, baseS: 1200 },
};

export const ORDER = Object.keys(TRIGGERS) as TriggerId[];

export const WAITING_LONG_MS = 3 * 60_000;
export const LONG_TASK_MS = 20 * 60_000;
export const BREAK_AFTER_MS = 2 * 3600_000;
/** No input and no agent activity for this long ends a stretch of work. */
export const PAUSE_MS = 10 * 60_000;
export const AWAY_MS = 15 * 60_000;

export interface Observation {
  /** ms since epoch */
  now: number;
  /** Local hour 0 to 23. */
  hour: number;
  overall: State;
  sessions: SessionView[];
  /** When the cursor last moved, ms since epoch. */
  userActiveAt: number;
  /** The kind of app in front (`pet/apps.ts`), when known. */
  app?: string | null;
}

const busy = (s: State) => s.state !== "idle" && s.state !== "sleeping" && s.state !== "done";

/** Follows the observations over time and says which triggers' moments
 *  have come. A moment is offered until it is consumed by speaking or
 *  goes stale, so a trigger held back by its cooldown does not fire
 *  late about something long past. */
export class Watcher {
  private prev: State["state"] | null = null;
  private errorStreak = 0;
  private busySince = 0;
  private longDoneAt = 0;
  private workSince = 0;
  private lastWork = 0;
  private lastUserActive = 0;
  private backAt = 0;
  private app: string | null = null;
  private appAt = 0;
  /** The kind of app just switched to, for the `app` line's key. */
  switchedTo: string | null = null;

  observe(o: Observation): TriggerId[] {
    const out: TriggerId[] = [];
    const state = o.overall.state;

    if (o.sessions.some((s) => s.state === "waiting" && s.for_ms >= WAITING_LONG_MS)) out.push("waiting_long");

    if (state !== this.prev) {
      if (state === "error") this.errorStreak++;
      if (state === "done") this.errorStreak = 0;
      if (busy(o.overall) && this.busySince === 0) this.busySince = o.now;
      if (state === "done") {
        if (this.busySince && o.now - this.busySince >= LONG_TASK_MS) this.longDoneAt = o.now;
        this.busySince = 0;
      }
      if (state === "idle" || state === "sleeping") this.busySince = 0;
      this.prev = state;
    }
    if (this.errorStreak >= 2) out.push("errors");
    if (this.longDoneAt && o.now - this.longDoneAt < 120_000) out.push("long_done");

    const workedAt = Math.max(o.userActiveAt, busy(o.overall) ? o.now : 0);
    if (workedAt > this.lastWork) {
      if (o.now - this.lastWork > PAUSE_MS) this.workSince = workedAt;
      this.lastWork = workedAt;
    }
    if (this.workSince && o.now - this.lastWork < PAUSE_MS && o.now - this.workSince >= BREAK_AFTER_MS) out.push("break");

    if (o.userActiveAt > this.lastUserActive) {
      if (this.lastUserActive && o.userActiveAt - this.lastUserActive >= AWAY_MS) this.backAt = o.now;
      this.lastUserActive = o.userActiveAt;
    }
    if (this.backAt && o.now - this.backAt < 60_000) out.push("back");

    if (o.app !== undefined && o.app !== this.app) {
      // The first look is where the user already was, not a switch.
      if (this.appAt && o.app) {
        this.switchedTo = o.app;
        this.appAt = o.now;
      }
      if (!this.appAt) this.appAt = o.now;
      this.app = o.app;
    }
    if (this.switchedTo && o.now - this.appAt < 20_000 && this.switchedTo === this.app) out.push("app");

    const recentlyActive = o.now - o.userActiveAt < 60_000;
    if (o.hour >= 18 && (o.sessions.length > 0 || this.lastWork > 0) && recentlyActive) out.push("report");
    if (o.hour >= 5 && o.hour < 11 && recentlyActive) out.push("morning");
    if ((o.hour >= 22 || o.hour < 5) && busy(o.overall)) out.push("night");
    if (state === "idle" && recentlyActive) out.push("chatter");

    return ORDER.filter((id) => out.includes(id));
  }

  /** The trigger spoke; its moment is used up. */
  consume(id: TriggerId, now: number) {
    if (id === "errors") this.errorStreak = 0;
    if (id === "long_done") this.longDoneAt = 0;
    if (id === "break") this.workSince = now;
    if (id === "back") this.backAt = 0;
    if (id === "app") this.switchedTo = null;
  }
}

/** `from` and `to` as "HH:MM"; a range that crosses midnight wraps. */
export function inQuietHours(from: string, to: string, d: Date): boolean {
  const mins = (s: string) => {
    const [h, m] = s.split(":").map(Number);
    return Number.isFinite(h) ? h * 60 + (Number.isFinite(m) ? m : 0) : NaN;
  };
  const a = mins(from);
  const b = mins(to);
  if (Number.isNaN(a) || Number.isNaN(b) || a === b) return false;
  const now = d.getHours() * 60 + d.getMinutes();
  return a < b ? now >= a && now < b : now >= a || now < b;
}

export interface Gate {
  chatty: number;
  dnd: boolean;
  quiet: boolean;
  /** A meeting app is in front: like quiet hours, for its duration. */
  meeting?: boolean;
  now: number;
  /** Local day of `now`, `YYYY-MM-DD`. */
  today: string;
  /** When the pet last spoke unprompted, ms since epoch. */
  lastSpokeAt: number;
  /** Unprompted lines in the last hour, of any trigger. */
  lastHour: number;
  stat?: TriggerStat;
  /** Turns an RFC 3339 time into a local `YYYY-MM-DD`. */
  dayOf: (iso: string) => string;
}

/** Whether a trigger whose moment has come may speak now. */
export function mayFire(id: TriggerId, g: Gate): boolean {
  const def = TRIGGERS[id];
  if (g.dnd || g.chatty < def.chatty) return false;
  if ((g.quiet || g.meeting) && !def.needsYou) return false;
  if (!def.needsYou) {
    if (g.now - g.lastSpokeAt < (g.chatty >= 5 ? 60_000 : 120_000)) return false;
    if (id === "chatter" && g.chatty === 4 && g.lastHour >= 3) return false;
  }
  const last = g.stat?.last_fired;
  if (last) {
    if (def.daily && g.dayOf(last) === g.today) return false;
    const base = id === "chatter" && g.chatty >= 5 ? 300 : def.baseS;
    const gap = Math.max(base, g.stat?.cooldown_s ?? 0) * 1000;
    if (g.now - Date.parse(last) < gap) return false;
  }
  return true;
}

export const OFFER_AFTER_DAYS = 3;
export const OFFER_AGAIN_MS = 30 * 86_400_000;
export const OFFER_TIMES = 2;

export interface OfferInput {
  level: number;
  daysActive: number;
  /** First stored event, ms since epoch; null on a fresh install. */
  firstEventAt: number | null;
  now: number;
  stat?: TriggerStat;
}

/** Whether to ask once, in character, for level 2 (ADR 4): only at the
 *  default level, after a few days of real use, at most twice, a month
 *  apart. Do-not-disturb and quiet hours apply as for any line. */
export function shouldOfferLevel(o: OfferInput): boolean {
  if (o.level !== 1 || o.firstEventAt === null) return false;
  if (o.daysActive < OFFER_AFTER_DAYS || o.now - o.firstEventAt < OFFER_AFTER_DAYS * 86_400_000) return false;
  if ((o.stat?.fired ?? 0) >= OFFER_TIMES) return false;
  if (o.stat?.answered) return false;
  const last = o.stat?.last_fired ? Date.parse(o.stat.last_fired) : 0;
  return o.now - last >= OFFER_AGAIN_MS;
}

/** `{tools}`-style placeholders in a line, from today's counters. */
export function fill(line: string, vars: Record<string, string | number>): string {
  return line.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}
