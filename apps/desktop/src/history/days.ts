/** Pure helpers for the history chart, kept apart so they are tested
 *  without a DOM. */
import type { DayRow } from "../generated/DayRow";

/** The store returns only days with activity; a chart needs every day
 *  of the range, oldest first, ending today. */
export function fillDays(rows: DayRow[], days: number, today: Date): DayRow[] {
  const byDay = new Map(rows.map((r) => [r.day, r]));
  const out: DayRow[] = [];
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i);
    const key = localDay(d);
    out.push(byDay.get(key) ?? { day: key, sessions: 0, turns: 0, tool_calls: 0, errors: 0, tokens: 0, waiting_ms: 0 });
  }
  return out;
}

export function localDay(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** A round top for the y axis and the gridlines under it: 1, 2 or 5
 *  times a power of ten, split in at most four steps. */
export function niceScale(max: number): { top: number; ticks: number[] } {
  if (max <= 0) return { top: 4, ticks: [0, 1, 2, 3, 4] };
  const rough = max / 4;
  const pow = 10 ** Math.floor(Math.log10(rough));
  const step = [1, 2, 5, 10].map((m) => m * pow).find((s) => s >= rough) ?? 10 * pow;
  const top = Math.ceil(max / step) * step;
  const ticks: number[] = [];
  for (let v = 0; v <= top + step / 2; v += step) ticks.push(Math.round(v));
  return { top, ticks };
}

export function sum(rows: DayRow[]): Omit<DayRow, "day"> {
  const t = { sessions: 0, turns: 0, tool_calls: 0, errors: 0, tokens: 0, waiting_ms: 0 };
  for (const r of rows) {
    t.sessions += r.sessions;
    t.turns += r.turns;
    t.tool_calls += r.tool_calls;
    t.errors += r.errors;
    t.tokens += r.tokens;
    t.waiting_ms += r.waiting_ms;
  }
  return t;
}

export function compact(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

/** Short enough for a stat tile: `45m`, `8h 35m`. */
export function duration(ms: number): string {
  const m = Math.round(ms / 60_000);
  if (m < 60) return `${m}m`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}
