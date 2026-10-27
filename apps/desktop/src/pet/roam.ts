/** Where the pet goes on its own: onto the terminal of a session that
 *  waits for you, up onto the window you are working in now and then,
 *  down again, along to a neighbouring window, and into a corner for
 *  the night. Decisions are pure (`plan`); `Roam` runs them. */
import { invoke } from "@tauri-apps/api/core";
import type { Snapshot } from "../generated/Snapshot";
import type { DesktopLife, World } from "./desktop";
import type { IdleBehaviour } from "./idle";
import { type Ledge, ledges, support } from "./physics";

export type GoalKind = "to_session" | "to_front" | "down" | "hop" | "bed";

export interface Goal {
  kind: GoalKind;
  /** Origin position to stand at, logical pixels. */
  x: number;
  y: number;
}

export interface PlanInput {
  world: World;
  /** A session waiting for the user, and the windows that may hold it. */
  waiting: { key: string; pids: number[] } | null;
  /** A waiting session the pet already went to; it does not go twice. */
  visited: string | null;
  /** Time for a wander, decided by the caller's clock. */
  wander: boolean;
  night: boolean;
  /** Already went to bed tonight. */
  inBed: boolean;
  idle: boolean;
  random: () => number;
}

const NEAR_PX = 700;

/** Where to stand on a ledge: near its right end for a session (out of
 *  the way of the text), anywhere for a wander. */
export function spotOn(l: Ledge, pet: { width: number; height: number }, where: "right" | "any", random: () => number): { x: number; y: number } {
  const lo = l.x1 - pet.width / 3;
  const hi = l.x2 - (pet.width * 2) / 3;
  const x = where === "right" ? hi - 24 : lo + (hi - lo) * random();
  return { x: Math.max(lo, Math.min(hi, x)), y: l.y - pet.height };
}

export function plan(p: PlanInput): Goal | null {
  const { bounds, pet, scene } = p.world;
  const visible = ledges(bounds.shelves);
  const here = support(bounds, pet.x, pet.y).ledge;
  const byId = (id: number) => visible.filter((l) => l.id === id).sort((a, b) => b.x2 - b.x1 - (a.x2 - a.x1))[0];

  if (p.waiting && p.visited !== p.waiting.key) {
    const win = scene.shelves.find((s) => p.waiting?.pids.includes(s.pid));
    const ledge = win ? byId(win.id) : undefined;
    if (ledge && here?.id !== ledge.id) return { kind: "to_session", ...spotOn(ledge, bounds.pet, "right", p.random) };
  }
  if (!p.idle) return null;
  if (p.night && !p.inBed) return { kind: "bed", x: bounds.maxX - 12, y: bounds.maxY };
  if (!p.wander || p.night) return null;

  const roll = p.random();
  if (!here) {
    const front = scene.shelves.find((s) => s.front);
    const ledge = front ? byId(front.id) : undefined;
    if (ledge && roll < 0.6) return { kind: "to_front", ...spotOn(ledge, bounds.pet, "any", p.random) };
    return null;
  }
  if (roll < 0.4) {
    const x = Math.max(bounds.minX, Math.min(bounds.maxX, pet.x + (p.random() - 0.5) * 300));
    return { kind: "down", x, y: bounds.maxY };
  }
  if (roll < 0.7) {
    const near = visible.filter((l) => l.id !== here.id && Math.abs(l.y - here.y) < NEAR_PX && Math.abs((l.x1 + l.x2) / 2 - pet.x) < NEAR_PX);
    const pick = near[Math.floor(p.random() * near.length)];
    if (pick) return { kind: "hop", ...spotOn(pick, bounds.pet, "any", p.random) };
  }
  return null;
}

const TICK_MS = 1500;
const WANDER_MIN_MS = 45_000;
const WANDER_SPREAD_MS = 60_000;

export class Roam {
  private nextWander = Date.now() + WANDER_MIN_MS;
  private visited: string | null = null;
  private inBedOn = "";
  private pids = new Map<string, number[]>();
  private busy = false;

  constructor(
    private life: DesktopLife,
    private idle: IdleBehaviour,
    private random: () => number = Math.random,
  ) {}

  start(latest: () => Snapshot | null) {
    setInterval(() => void this.tick(latest()), TICK_MS);
  }

  private async lineage(key: string): Promise<number[]> {
    const known = this.pids.get(key);
    if (known?.length) return known;
    const pids = await invoke<number[]>("session_pids", { key }).catch(() => []);
    this.pids.set(key, pids);
    return pids;
  }

  private async tick(snap: Snapshot | null) {
    const world = this.life.world0();
    if (!snap || !world || this.busy) return;
    const now = Date.now();
    const hour = new Date(now).getHours();
    const tonight = new Date(now - 6 * 3600_000).toDateString();
    const w = snap.sessions.find((s) => s.state === "waiting" && s.origin);
    if (!w) this.visited = null;
    const waiting = w ? { key: w.key, pids: await this.lineage(w.key) } : null;
    const wander = now >= this.nextWander;
    const goal = plan({
      world,
      waiting,
      visited: this.visited,
      wander,
      night: hour >= 23 || hour < 6,
      inBed: this.inBedOn === tonight,
      idle: this.idle.isIdle() || snap.overall.state === "sleeping",
      random: this.random,
    });
    if (wander) this.nextWander = now + WANDER_MIN_MS + this.random() * WANDER_SPREAD_MS;
    if (!goal) return;
    this.busy = true;
    try {
      if (await this.life.jumpTo(goal.x, goal.y)) {
        if (goal.kind === "to_session" && waiting) this.visited = waiting.key;
        if (goal.kind === "bed") this.inBedOn = tonight;
      }
    } finally {
      this.busy = false;
    }
  }
}
