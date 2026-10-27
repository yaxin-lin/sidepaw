/** What the pet knows when you talk to it: the sessions right now,
 *  today's counters and the last few stored events. Everything here
 *  was already filtered by the permission level and redacted at
 *  ingest, so the text can go to the model as is. */
import type { DayReport } from "../generated/DayReport";
import type { EventRow } from "../generated/EventRow";
import type { Snapshot } from "../generated/Snapshot";

export const CONTEXT_EVENTS = 12;
const CLIP = 160;

export interface ContextInput {
  snapshot: Snapshot;
  report: DayReport;
  /** Newest first, as the store returns them. */
  events: EventRow[];
}

function clip(s: string): string {
  const one = s.replace(/\s+/g, " ").trim();
  return one.length > CLIP ? one.slice(0, CLIP - 1) + "…" : one;
}

function hhmm(ts: string): string {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts.slice(11, 16);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

function minutes(ms: number): string {
  const m = Math.round(ms / 60_000);
  return m < 1 ? `${Math.round(ms / 1000)} s` : m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
}

function eventLine(e: EventRow): string {
  const where = e.project ? `${e.host}/${e.project}` : e.host;
  let line = `${hhmm(e.ts)} ${where} ${e.kind}`;
  if (e.tool_name) line += ` ${e.tool_name}`;
  if (e.text) line += `: ${clip(e.text)}`;
  if (e.error_type) line += ` [${e.error_type}${e.error_msg ? `: ${clip(e.error_msg)}` : ""}]`;
  return line;
}

export function buildContext(input: ContextInput, maxEvents = CONTEXT_EVENTS): string {
  const out: string[] = [];
  const sessions = input.snapshot.sessions;
  if (sessions.length === 0) out.push("Now: no agent session is running.");
  else {
    out.push("Now:");
    for (const s of sessions) {
      const host = s.key.split(":")[0];
      const detail = s.state === "tool" ? ` (${s.detail})` : "";
      out.push(`- ${host}${s.project ? ` in ${s.project}` : ""}: ${s.state}${detail} for ${minutes(s.for_ms)}`);
    }
  }
  const r = input.report;
  out.push(`Today: ${r.sessions} sessions, ${r.turns} turns, ${r.tool_calls} tool calls, ${r.errors} errors, waited on the user ${minutes(r.waiting_ms)}.`);
  const events = input.events.slice(0, maxEvents).reverse();
  if (events.length) {
    out.push("Recent events, oldest first:");
    for (const e of events) out.push(eventLine(e));
  }
  return out.join("\n");
}
