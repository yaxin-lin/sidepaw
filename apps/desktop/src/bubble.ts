import type { State } from "./generated/State";
import { keysFor, Lines } from "./lines";

/** How long a line stays up, scaled a little for longer text. */
function holdMs(line: string): number {
  return Math.min(8000, 3000 + line.length * 60);
}

/** Minimum gap between unprompted lines per state. Tool calls and
 *  thinking flip every few seconds; narrating each one is noise.
 *  States that need the user, or close a turn, always speak. */
const GAP_MS: Record<State["state"], number> = {
  waiting: 0,
  error: 0,
  done: 0,
  thinking: 30_000,
  tool: 60_000,
  idle: 120_000,
  sleeping: Infinity,
};

/** A speech bubble over the pet that says pack lines on state changes
 *  and interactions, without becoming a ticker. */
export class Bubble {
  private timer = 0;
  private lastSaid: Partial<Record<State["state"], number>> = {};
  private lastState: State["state"] | null = null;

  constructor(private el: HTMLElement, private lines: Lines, private now: () => number = Date.now) {}

  /** Call on every poll; only transitions can trigger a line. */
  onState(state: State) {
    if (state.state === this.lastState) return;
    this.lastState = state.state;
    const t = this.now();
    const gap = GAP_MS[state.state];
    const last = this.lastSaid[state.state];
    if (last !== undefined && t - last < gap) return;
    const line = this.lines.pick(keysFor(state));
    if (line === null) return;
    this.lastSaid[state.state] = t;
    this.say(line);
  }

  /** A click or drag always gets an answer when the pack has one. */
  onInteraction(kind: "click" | "drag") {
    const line = this.lines.pick(kind);
    if (line !== null) this.say(line);
  }

  say(line: string) {
    this.el.textContent = line;
    this.el.hidden = false;
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => {
      this.el.hidden = true;
    }, holdMs(line));
  }
}
