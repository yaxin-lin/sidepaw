/** Runs the proactive engine (`brain/proactive.ts`) in the pet window:
 *  feeds it the snapshot and whether the user is at the computer,
 *  picks a line from the pack for the first trigger allowed to speak,
 *  and reports how the user reacted so the cooldowns learn. */
import { invoke } from "@tauri-apps/api/core";
import { type Gate, fill, inQuietHours, mayFire, type TriggerId, TRIGGERS, Watcher } from "../brain/proactive";
import type { DayReport } from "../generated/DayReport";
import type { Quiet } from "../generated/Quiet";
import type { Snapshot } from "../generated/Snapshot";
import type { TriggerStat } from "../generated/TriggerStat";
import type { Lines } from "../lines";
import type { Bubble } from "./bubble";
import { t } from "../i18n";

const EVAL_MS = 5000;
const CURSOR_MS = 2000;

function localDay(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** With more than one session going, a line about one of them says
 *  which project it is about. */
export function withProject(id: TriggerId, snap: Snapshot, line: string): string {
  if (snap.sessions.length < 2) return line;
  const about = id === "waiting_long" ? snap.sessions.find((s) => s.state === "waiting") : id === "errors" || id === "long_done" ? snap.sessions[0] : undefined;
  return about?.project ? t("nudge.about", { project: about.project, line }) : line;
}

export class Scheduler {
  private watcher = new Watcher();
  private stats = new Map<string, TriggerStat>();
  private quiet: Quiet = { from: "23:00", to: "08:00" };
  private userActiveAt = Date.now();
  private lastCursor: [number, number] | null = null;
  private lastEval = 0;
  private spoken: number[] = [];
  chatty = 2;
  dnd = false;

  constructor(
    private bubble: Bubble,
    private lines: () => Lines,
    private onAnswer: () => void,
  ) {}

  async start() {
    try {
      for (const s of await invoke<TriggerStat[]>("trigger_stats")) this.stats.set(s.trigger, s);
      this.quiet = await invoke<Quiet>("quiet_hours");
    } catch (e) {
      console.warn(`scheduler: ${String(e)}`);
    }
    setInterval(() => void this.watchCursor(), CURSOR_MS);
  }

  /** The cursor moving anywhere on screen means someone is there. No
   *  keyboard watching: that would need an OS permission. */
  private async watchCursor() {
    const p = await invoke<[number, number] | null>("cursor_global").catch(() => null);
    if (!p) return;
    if (this.lastCursor && (p[0] !== this.lastCursor[0] || p[1] !== this.lastCursor[1])) this.userActiveAt = Date.now();
    this.lastCursor = p;
  }

  /** Call on every poll; it evaluates every few seconds. */
  async onSnapshot(snap: Snapshot) {
    const now = Date.now();
    const d = new Date(now);
    const due = this.watcher.observe({ now, hour: d.getHours(), overall: snap.overall, sessions: snap.sessions, userActiveAt: this.userActiveAt });
    if (now - this.lastEval < EVAL_MS && !due.some((id) => TRIGGERS[id].needsYou)) return;
    this.lastEval = now;
    this.spoken = this.spoken.filter((t) => now - t < 3600_000);
    const gate: Omit<Gate, "stat"> = {
      chatty: this.chatty,
      dnd: this.dnd,
      quiet: inQuietHours(this.quiet.from, this.quiet.to, d),
      now,
      today: localDay(d),
      lastSpokeAt: this.spoken.at(-1) ?? 0,
      lastHour: this.spoken.length,
      dayOf: (iso) => localDay(new Date(iso)),
    };
    for (const id of due) {
      if (!mayFire(id, { ...gate, stat: this.stats.get(id) })) continue;
      // Small talk is a chance, not a schedule.
      if (id === "chatter" && Math.random() > 0.3) continue;
      const line = this.lines().pick(id);
      if (line === null) continue; // this pack has nothing to say here
      const text = withProject(id, snap, id === "report" ? fill(line, await this.counters()) : line);
      if (!this.bubble.nudge(text, (answered) => void this.react(id, answered))) return;
      this.watcher.consume(id, now);
      this.spoken.push(now);
      await this.record(id, "fired");
      return;
    }
  }

  private async counters(): Promise<Record<string, number>> {
    const r = await invoke<DayReport>("report_today").catch(() => null);
    if (!r) return {};
    return { tools: r.tool_calls, turns: r.turns, errors: r.errors, sessions: r.sessions, waited: Math.round(r.waiting_ms / 60_000) };
  }

  private async react(id: TriggerId, answered: boolean) {
    await this.record(id, answered ? "answered" : "dismissed");
    if (answered) this.onAnswer();
  }

  private async record(id: TriggerId, outcome: "fired" | "dismissed" | "answered") {
    try {
      const row = await invoke<TriggerStat>("record_trigger", { trigger: id, outcome, baseS: TRIGGERS[id].baseS });
      this.stats.set(id, row);
    } catch (e) {
      console.warn(`record ${id}: ${String(e)}`);
    }
  }
}
