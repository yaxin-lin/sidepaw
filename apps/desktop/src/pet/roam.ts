/** What the pet does on its own between agent events: now and then a
 *  small hop, on the spot or a little to the side, with a line that
 *  invites the user in; and a hop for attention when a session starts
 *  waiting. Hops stay on the edge the pet stands on. Decisions are pure
 *  (`plan`); `Roam` runs them. */
import type { Snapshot } from "../generated/Snapshot";
import type { DesktopLife } from "./desktop";
import type { IdleBehaviour } from "./idle";
import { HOP_MAX } from "./physics";

export interface Goal {
  /** `hop` comes with a line; `attention` is about a waiting session,
   *  whose own lines already say what it is. */
  kind: "hop" | "attention";
  dx: number;
}

export interface PlanInput {
  /** A session waiting for the user right now. */
  waiting: string | null;
  /** The waiting session the pet already hopped for. */
  noticed: string | null;
  /** Time for a hop, by the caller's clock. */
  due: boolean;
  idle: boolean;
  random: () => number;
}

export function plan(p: PlanInput): Goal | null {
  if (p.waiting && p.waiting !== p.noticed) return { kind: "attention", dx: 0 };
  if (!p.due || !p.idle) return null;
  // Half on the spot, half a little to one side.
  const r = p.random();
  const dx = r < 0.5 ? 0 : (r < 0.75 ? -1 : 1) * (HOP_MAX / 3 + p.random() * (HOP_MAX * 2) / 3);
  return { kind: "hop", dx };
}

const TICK_MS = 1500;
const HOP_MIN_MS = 45_000;
const HOP_SPREAD_MS = 75_000;

export class Roam {
  private next = Date.now() + HOP_MIN_MS;
  private noticed: string | null = null;
  private busy = false;

  constructor(
    private life: DesktopLife,
    private idle: IdleBehaviour,
    /** Asked for a line after a hop; it decides whether to speak. */
    private onHop: () => void,
    private random: () => number = Math.random,
  ) {}

  start(latest: () => Snapshot | null) {
    setInterval(() => void this.tick(latest()), TICK_MS);
  }

  private async tick(snap: Snapshot | null) {
    if (!snap || this.busy) return;
    const now = Date.now();
    const waiting = snap.sessions.find((s) => s.state === "waiting")?.key ?? null;
    if (!waiting) this.noticed = null;
    const due = now >= this.next;
    const goal = plan({ waiting, noticed: this.noticed, due, idle: this.idle.isIdle(), random: this.random });
    if (due) this.next = now + HOP_MIN_MS + this.random() * HOP_SPREAD_MS;
    if (!goal) return;
    this.busy = true;
    try {
      if (!(await this.life.hop(goal.dx))) return;
      if (goal.kind === "attention") this.noticed = waiting;
      else this.onHop();
    } finally {
      this.busy = false;
    }
  }
}
