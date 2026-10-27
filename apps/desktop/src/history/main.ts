/** Usage history: totals, tool calls per day and a per-project table
 *  over the last 7, 30 or 90 days. Read-only; everything comes from
 *  the local store through the `history` command. */
import { invoke } from "@tauri-apps/api/core";
import type { DayRow } from "../generated/DayRow";
import type { History } from "../generated/History";
import type { ProjectRow } from "../generated/ProjectRow";
import { applyToDom, detectLang, initLang, onLangChange, t } from "../i18n";
import { compact, duration, fillDays, niceScale, sum } from "./days";

const RANGES = [7, 30, 90] as const;
const RANGE_KEY = "history.range";
const SVG = "http://www.w3.org/2000/svg";

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function svg<K extends keyof SVGElementTagNameMap>(name: K, attrs: Record<string, string | number>): SVGElementTagNameMap[K] {
  const node = document.createElementNS(SVG, name);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
  return node;
}

function dayLabel(day: string, long = false): string {
  const [y, m, d] = day.split("-").map(Number);
  const date = new Date(y, m - 1, d);
  return date.toLocaleDateString(detectLang(), long ? { weekday: "short", month: "short", day: "numeric" } : { month: "numeric", day: "numeric" });
}

function renderTiles(days: DayRow[]) {
  const s = sum(days);
  const tiles: [string, string][] = [
    [compact(s.sessions), t("today.sessions")],
    [compact(s.turns), t("today.turns")],
    [compact(s.tool_calls), t("today.tools")],
    [compact(s.errors), t("today.errors")],
    [duration(s.waiting_ms), t("today.waiting")],
  ];
  if (s.tokens > 0) tiles.push([compact(s.tokens), t("history.tokens")]);
  el("tiles").replaceChildren(
    ...tiles.map(([v, k]) => {
      const div = document.createElement("div");
      div.className = "tile";
      div.innerHTML = `<div class="v"></div><div class="k"></div>`;
      (div.firstChild as HTMLElement).textContent = v;
      (div.lastChild as HTMLElement).textContent = k;
      return div;
    }),
  );
}

/** One bar per day. Bars round only at the top (the data end), sit on
 *  a baseline, and keep a 2 px gap; each column has a full-height hit
 *  area and a tooltip with the rest of that day's counters. */
function renderChart(days: DayRow[]) {
  const box = el("chart");
  box.replaceChildren();
  const W = Math.max(320, box.clientWidth || 640);
  const H = 220;
  const pad = { l: 34, r: 6, t: 8, b: 22 };
  const plotW = W - pad.l - pad.r;
  const plotH = H - pad.t - pad.b;
  const { top, ticks } = niceScale(Math.max(...days.map((d) => d.tool_calls)));
  const y = (v: number) => pad.t + plotH - (v / top) * plotH;
  const colW = plotW / days.length;
  const barW = Math.max(2, Math.min(28, colW - 2));

  const root = svg("svg", { viewBox: `0 0 ${W} ${H}`, role: "img", "aria-label": t("history.per_day") });
  const grid = svg("g", { class: "grid" });
  for (const v of ticks) {
    grid.append(svg("line", { x1: pad.l, x2: W - pad.r, y1: y(v), y2: y(v) }));
    const label = svg("text", { x: pad.l - 6, y: y(v) + 4, "text-anchor": "end" });
    label.textContent = compact(v);
    grid.append(label);
  }
  root.append(grid);

  const tip = document.createElement("div");
  tip.className = "tip";
  tip.hidden = true;
  // Label every day for a week, then about seven labels across the range.
  const every = days.length <= 7 ? 1 : Math.ceil(days.length / 7);
  days.forEach((d, i) => {
    const cx = pad.l + colW * i + colW / 2;
    const g = svg("g", { class: "col" });
    g.append(svg("rect", { class: "hit", x: pad.l + colW * i, y: pad.t, width: colW, height: plotH }));
    if (d.tool_calls > 0) {
      const h = Math.max(1, (d.tool_calls / top) * plotH);
      const r = Math.min(4, barW / 2, h);
      const x0 = cx - barW / 2;
      const y0 = pad.t + plotH - h;
      const yb = pad.t + plotH;
      g.append(
        svg("path", {
          class: "bar",
          d: `M${x0},${yb}V${y0 + r}Q${x0},${y0} ${x0 + r},${y0}H${x0 + barW - r}Q${x0 + barW},${y0} ${x0 + barW},${y0 + r}V${yb}Z`,
        }),
      );
    }
    if ((days.length - 1 - i) % every === 0) {
      const label = svg("text", { x: cx, y: H - 6, "text-anchor": "middle" });
      label.textContent = dayLabel(d.day);
      root.append(label);
    }
    g.addEventListener("mouseenter", () => {
      g.classList.add("on");
      tip.innerHTML = "";
      const b = document.createElement("b");
      b.textContent = dayLabel(d.day, true);
      tip.append(b);
      for (const [k, v] of [
        [t("today.tools"), compact(d.tool_calls)],
        [t("today.turns"), compact(d.turns)],
        [t("today.sessions"), compact(d.sessions)],
        [t("today.errors"), compact(d.errors)],
        [t("today.waiting"), duration(d.waiting_ms)],
      ]) {
        const line = document.createElement("div");
        const key = document.createElement("span");
        key.textContent = `${k} `;
        line.append(key, v);
        tip.append(line);
      }
      tip.hidden = false;
      const left = Math.min(Math.max(0, (cx / W) * box.clientWidth - 70), box.clientWidth - 150);
      tip.style.left = `${left}px`;
      tip.style.top = "0px";
    });
    g.addEventListener("mouseleave", () => {
      g.classList.remove("on");
      tip.hidden = true;
    });
    root.append(g);
  });
  root.append(svg("line", { class: "base", x1: pad.l, x2: W - pad.r, y1: pad.t + plotH, y2: pad.t + plotH }));
  box.append(root, tip);
}

function renderDayTable(days: DayRow[]) {
  const head = [t("history.day"), t("today.tools"), t("today.turns"), t("today.sessions"), t("today.errors"), t("today.waiting")];
  const rows = [...days].reverse().filter((d) => d.tool_calls || d.turns || d.sessions);
  table(el("day-table"), head, rows.map((d) => [dayLabel(d.day, true), compact(d.tool_calls), compact(d.turns), compact(d.sessions), compact(d.errors), duration(d.waiting_ms)]));
}

function table(node: HTMLTableElement, head: string[], rows: string[][], meter?: number[]) {
  node.replaceChildren();
  const tr = document.createElement("tr");
  head.forEach((h, i) => {
    const th = document.createElement("th");
    th.textContent = h;
    if (i > 0) th.className = "n";
    tr.append(th);
  });
  if (meter) tr.append(document.createElement("th"));
  node.append(tr);
  rows.forEach((cells, r) => {
    const row = document.createElement("tr");
    cells.forEach((c, i) => {
      const td = document.createElement("td");
      td.textContent = c;
      if (i > 0) td.className = "n";
      row.append(td);
    });
    if (meter) {
      const td = document.createElement("td");
      td.className = "meter";
      const track = document.createElement("div");
      const fill = document.createElement("i");
      fill.style.width = `${Math.round(meter[r] * 100)}%`;
      track.append(fill);
      td.append(track);
      row.append(td);
    }
    node.append(row);
  });
}

function renderProjects(projects: ProjectRow[]) {
  const max = Math.max(1, ...projects.map((p) => p.tool_calls));
  const rel = (iso: string) => new Date(iso).toLocaleDateString(detectLang(), { month: "short", day: "numeric" });
  table(
    el("projects"),
    [t("history.project"), t("today.tools"), t("today.turns"), t("today.sessions"), t("today.errors"), t("history.last")],
    projects.map((p) => [
      `${p.project ?? t("history.unrecorded")} · ${p.hosts.join(", ")}`,
      compact(p.tool_calls),
      compact(p.turns),
      compact(p.sessions),
      compact(p.errors),
      rel(p.last_active),
    ]),
    projects.map((p) => p.tool_calls / max),
  );
  el("projects-note").textContent = projects.some((p) => p.project === null) ? t("history.unrecorded.note") : "";
}

async function load(days: number) {
  const h = await invoke<History>("history", { days });
  const filled = fillDays(h.days, days, new Date());
  renderTiles(filled);
  if (h.days.length === 0) {
    el("chart").innerHTML = "";
    const p = document.createElement("p");
    p.className = "empty";
    p.textContent = t("history.empty");
    el("chart").append(p);
  } else {
    renderChart(filled);
  }
  renderDayTable(filled);
  renderProjects(h.projects);
  return filled;
}

function storedRange(): number {
  try {
    const n = Number(localStorage.getItem(RANGE_KEY));
    return (RANGES as readonly number[]).includes(n) ? n : 30;
  } catch {
    return 30;
  }
}

async function main() {
  await initLang();
  applyToDom();
  document.title = t("history.title");
  await onLangChange(() => location.reload());
  let range = storedRange();
  const seg = el("range");
  const buttons = RANGES.map((n) => {
    const b = document.createElement("button");
    b.textContent = t("history.days", { n });
    b.addEventListener("click", () => {
      range = n;
      try {
        localStorage.setItem(RANGE_KEY, String(n));
      } catch {
        // per-window convenience only
      }
      for (const other of buttons) other.setAttribute("aria-pressed", String(other === b));
      void load(range).then((d) => (last = d));
    });
    b.setAttribute("aria-pressed", String(n === range));
    return b;
  });
  seg.replaceChildren(...buttons);
  let last = await load(range);
  window.addEventListener("resize", () => {
    if (last.some((d) => d.tool_calls)) renderChart(last);
  });
}

void main();
