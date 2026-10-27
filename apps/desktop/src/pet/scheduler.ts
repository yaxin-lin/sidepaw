/** Runs the proactive engine (`brain/proactive.ts`) in the pet window:
 *  feeds it the snapshot and whether the user is at the computer,
 *  picks a line from the pack for the first trigger allowed to speak,
 *  and reports how the user reacted so the cooldowns learn. */
import { invoke } from "@tauri-apps/api/core";
import { buildContext } from "../brain/context";
import { activePack, loadPersona } from "../brain/files";
import { complete, resolve } from "../brain/llm";
import { cleanLine, DAILY_CAP, LIVE, nudgeRequest, TIMEOUT_MS } from "../brain/nudge";
import { compileSystemPrompt } from "../brain/persona";
import { type Gate, fill, inQuietHours, mayFire, OFFER_AGAIN_MS, shouldOfferLevel, type TriggerId, TRIGGERS, Watcher } from "../brain/proactive";
import type { EventRow } from "../generated/EventRow";
import type { ModelConfig } from "../generated/ModelConfig";
import type { Usage } from "../generated/Usage";
import type { DayReport } from "../generated/DayReport";
import type { Quiet } from "../generated/Quiet";
import type { Snapshot } from "../generated/Snapshot";
import type { TriggerStat } from "../generated/TriggerStat";
import type { Lines } from "../lines";
import type { Bubble } from "./bubble";
import { detectLang, t } from "../i18n";

const EVAL_MS = 5000;
const OFFER_CHECK_MS = 3600_000;
const OFFER = "offer_level";
const LIVE_COUNT_KEY = "live-lines";
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
  private lastOfferCheck = 0;
  /** An evaluation may wait seconds on the model; polls meanwhile skip. */
  private evaluating = false;
  chatty = 2;
  dnd = false;
  /** `[pet] live_lines`, on only after the user accepted the preview. */
  liveLines = false;

  /** The kind of app in front, from the pet's last look at the screen. */
  appKind: () => string | null = () => null;

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
    // Every snapshot is observed, so no state change is missed while a
    // line is being decided; only the deciding waits its turn.
    const now = Date.now();
    const due = this.watcher.observe({
      now,
      hour: new Date(now).getHours(),
      overall: snap.overall,
      sessions: snap.sessions,
      userActiveAt: this.userActiveAt,
      app: this.appKind(),
    });
    if (this.evaluating) return;
    this.evaluating = true;
    try {
      await this.evaluate(snap, due, now);
    } finally {
      this.evaluating = false;
    }
  }

  /** A line for something the pet did itself (a hop), through the same
   *  gate as every unprompted line. */
  async say(id: "hop") {
    if (this.evaluating) return;
    this.evaluating = true;
    try {
      const now = Date.now();
      const gate = this.gate(now);
      if (!mayFire(id, { ...gate, stat: this.stats.get(id) })) return;
      const line = this.lines().pick([id, "click"]);
      if (line === null) return;
      if (!this.bubble.nudge(line, (answered) => void this.react(id, answered, this.onAnswer))) return;
      this.spoken.push(now);
      await this.record(id, "fired", TRIGGERS[id].baseS);
    } finally {
      this.evaluating = false;
    }
  }

  private gate(now: number): Omit<Gate, "stat"> {
    const d = new Date(now);
    this.spoken = this.spoken.filter((t) => now - t < 3600_000);
    return {
      chatty: this.chatty,
      dnd: this.dnd,
      quiet: inQuietHours(this.quiet.from, this.quiet.to, d),
      meeting: this.appKind() === "meeting",
      now,
      today: localDay(d),
      lastSpokeAt: this.spoken.at(-1) ?? 0,
      lastHour: this.spoken.length,
      dayOf: (iso) => localDay(new Date(iso)),
    };
  }

  private async evaluate(snap: Snapshot, due: TriggerId[], now: number) {
    if (now - this.lastEval < EVAL_MS && !due.some((id) => TRIGGERS[id].needsYou)) return;
    this.lastEval = now;
    const gate = this.gate(now);
    for (const id of due) {
      if (!mayFire(id, { ...gate, stat: this.stats.get(id) })) continue;
      // Small talk is a chance, not a schedule.
      if (id === "chatter" && Math.random() > 0.3) continue;
      const key = id === "app" ? `app.${this.watcher.switchedTo}` : id;
      const packLine = this.lines().pick(key);
      const live = LIVE.has(id) ? await this.liveLine(id, snap) : null;
      const line = live ?? packLine;
      if (line === null) continue; // this pack has nothing to say here
      const text = live ? live : withProject(id, snap, id === "report" ? fill(line, await this.counters()) : line);
      if (!this.bubble.nudge(text, (answered) => void this.react(id, answered, this.onAnswer))) return;
      this.watcher.consume(id, now);
      this.spoken.push(now);
      await this.record(id, "fired", TRIGGERS[id].baseS);
      return;
    }
    if (now - this.lastOfferCheck >= OFFER_CHECK_MS && !gate.dnd && !gate.quiet && gate.chatty >= 1 && now - gate.lastSpokeAt >= 120_000) {
      this.lastOfferCheck = now;
      await this.maybeOfferLevel(now);
    }
  }

  /** Asks once, in character, whether it may read more (ADR 4); a click
   *  opens Setup at the permission levels. */
  private async maybeOfferLevel(now: number) {
    const [level, usage] = await Promise.all([invoke<number>("get_level"), invoke<Usage>("usage")]).catch(() => [null, null] as const);
    if (level === null || usage === null) return;
    const firstEventAt = usage.first_event ? Date.parse(usage.first_event) : null;
    if (!shouldOfferLevel({ level, daysActive: usage.days_active, firstEventAt, now, stat: this.stats.get(OFFER) })) return;
    const line = this.lines().pick(OFFER) ?? t("offer.level");
    const opened = this.bubble.nudge(line, (answered) =>
      void this.react(OFFER, answered, () => void invoke("open_setup", { section: "level" }), OFFER_AGAIN_MS / 1000),
    );
    if (!opened) return;
    this.spoken.push(now);
    await this.record(OFFER, "fired", OFFER_AGAIN_MS / 1000);
  }

  /** A line written by the model for this moment (ADR 8), or null for
   *  the pack's: when it is off, over the day's cap, unconfigured, slow
   *  or unusable. */
  private async liveLine(id: TriggerId, snap: Snapshot): Promise<string | null> {
    if (!this.liveLines || !this.underCap()) return null;
    try {
      const cfg = await invoke<ModelConfig>("get_model");
      resolve(cfg);
      const [report, events, key, persona] = await Promise.all([
        invoke<DayReport>("report_today"),
        invoke<EventRow[]>("recent_events", { limit: 5 }),
        invoke<string | null>("api_key"),
        activePack().then(loadPersona),
      ]);
      const req = nudgeRequest(id, compileSystemPrompt(persona.persona, persona.md, detectLang()), buildContext({ snapshot: snap, report, events }, 5));
      this.countLive();
      const reply = await Promise.race([
        complete(cfg, req, key),
        new Promise<null>((r) => setTimeout(() => r(null), TIMEOUT_MS)),
      ]);
      return reply === null ? null : cleanLine(reply);
    } catch (e) {
      console.warn(`live line: ${String(e)}`);
      return null;
    }
  }

  // Per day, in this window's storage; a restart within a day only
  // forgets the count, which errs toward the pack's lines.
  private underCap(): boolean {
    const today = localDay(new Date());
    try {
      const [day, n] = (localStorage.getItem(LIVE_COUNT_KEY) ?? "").split(" ");
      return day !== today || Number(n) < DAILY_CAP;
    } catch {
      return true;
    }
  }

  private countLive() {
    const today = localDay(new Date());
    try {
      const [day, n] = (localStorage.getItem(LIVE_COUNT_KEY) ?? "").split(" ");
      localStorage.setItem(LIVE_COUNT_KEY, `${today} ${day === today ? Number(n) + 1 : 1}`);
    } catch {
      // storage unavailable: the cap is not enforced across calls
    }
  }

  private async counters(): Promise<Record<string, number>> {
    const r = await invoke<DayReport>("report_today").catch(() => null);
    if (!r) return {};
    return { tools: r.tool_calls, turns: r.turns, errors: r.errors, sessions: r.sessions, waited: Math.round(r.waiting_ms / 60_000) };
  }

  private async react(id: string, answered: boolean, onAnswer: () => void, baseS = TRIGGERS[id as TriggerId]?.baseS ?? 3600) {
    await this.record(id, answered ? "answered" : "dismissed", baseS);
    if (answered) onAnswer();
  }

  private async record(id: string, outcome: "fired" | "dismissed" | "answered", baseS: number) {
    try {
      const row = await invoke<TriggerStat>("record_trigger", { trigger: id, outcome, baseS });
      this.stats.set(id, row);
    } catch (e) {
      console.warn(`record ${id}: ${String(e)}`);
    }
  }
}
