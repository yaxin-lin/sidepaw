import type { PendingApproval } from "../generated/PendingApproval";

/** Mirrors `Labels` in src-tauri/src/keys.rs. */
export interface ApprovalKeys {
  allow?: string | null;
  deny?: string | null;
}
import type { State } from "../generated/State";
import { t } from "../i18n";
import { keysFor, Lines } from "../lines";

/** How long a line stays up, scaled a little for longer text. Short
 *  enough that the pet never looks stuck mid-sentence. */
function holdMs(line: string): number {
  return Math.min(5000, 2200 + line.length * 40);
}

const FADE_MS = 160;
/** Long enough to read and decide whether to react. */
const NUDGE_MS = 9000;

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
  private fade = 0;
  private lastSaid: Partial<Record<State["state"], number>> = {};
  private lastState: State["state"] | null = null;

  /** The prompt currently shown with buttons, if any. */
  private asking: number | null = null;
  /** An unprompted line is up: state chatter and click lines wait,
   *  since they come every few seconds and would wipe it unread. */
  private nudging = false;

  constructor(private el: HTMLElement, private lines: Lines, private now: () => number = Date.now) {}

  /** The setup window regenerates lines; the pet picks them up live. */
  setLines(lines: Lines) {
    this.lines = lines;
  }

  /** Show the oldest held permission prompt with Allow and Deny, how
   *  many more wait behind it, how long until the terminal's own prompt
   *  takes over, and the keyboard shortcuts when they are registered.
   *  Replaces any line and stays until `clearApproval`; called on every
   *  poll, it only rebuilds for a different prompt. */
  askApproval(
    p: PendingApproval,
    queued: number,
    keys: ApprovalKeys,
    project: string | null,
    answer: (allow: boolean) => void,
  ) {
    if (this.asking === p.id) {
      this.setQueued(queued);
      return;
    }
    this.asking = p.id;
    clearTimeout(this.timer);
    clearTimeout(this.fade);
    const host = hostLabel(p.key);
    this.el.replaceChildren();
    const q = document.createElement("div");
    q.className = "q";
    const what = document.createElement("span");
    const who = project ? t("ask.wants.in", { host, project }) : t("ask.wants", { host });
    what.textContent = `${who}${p.tool ? ` ${p.tool}` : ""}`;
    const more = document.createElement("span");
    more.className = "more";
    q.append(what, more);
    const code = document.createElement("code");
    code.textContent = p.summary ?? "";
    // Shrinks to nothing as the hold runs out; a CSS transition does
    // the animation, so it costs nothing per frame.
    const clock = document.createElement("div");
    clock.className = "clock";
    const fill = document.createElement("i");
    clock.append(fill);
    const row = document.createElement("div");
    row.className = "btns";
    for (const [label, allow, key] of [
      [t("ask.allow"), true, keys.allow],
      [t("ask.deny"), false, keys.deny],
    ] as const) {
      const b = document.createElement("button");
      b.textContent = label;
      if (key) {
        const k = document.createElement("kbd");
        k.textContent = key;
        b.append(k);
      }
      b.className = allow ? "allow" : "deny";
      b.addEventListener("click", () => answer(allow));
      row.append(b);
    }
    this.el.append(q, code, clock, row);
    this.setQueued(queued);
    fill.style.transitionDuration = `${p.expires_in_ms}ms`;
    requestAnimationFrame(() => requestAnimationFrame(() => (fill.style.width = "0%")));
    this.el.classList.add("ask");
    this.el.hidden = false;
    requestAnimationFrame(() => requestAnimationFrame(() => this.el.classList.add("show")));
  }

  private setQueued(queued: number) {
    const more = this.el.querySelector<HTMLElement>(".more");
    if (more) more.textContent = queued > 0 ? t("ask.more", { n: queued }) : "";
  }

  clearApproval() {
    if (this.asking === null) return;
    this.asking = null;
    this.el.classList.remove("ask");
    this.hide();
  }

  /** Do not disturb: state changes say nothing. Clicks still answer
   *  and permission prompts still show, since the user asked for both. */
  quiet = false;

  /** Call on every poll; only transitions can trigger a line. */
  onState(state: State) {
    if (state.state === this.lastState) return;
    this.lastState = state.state;
    if (this.quiet) return;
    const t = this.now();
    const gap = GAP_MS[state.state];
    const last = this.lastSaid[state.state];
    if (last !== undefined && t - last < gap) return;
    const line = this.lines.pick(keysFor(state));
    if (line === null) return;
    this.lastSaid[state.state] = t;
    this.say(line);
  }

  /** A click, drag or file drop always gets an answer when the pack
   *  has one; a drop without its own lines borrows the click ones and
   *  shows the file name so the user knows it landed. */
  onInteraction(kind: "click" | "drag" | "drop", detail?: string) {
    const line = this.lines.pick(kind === "drop" ? ["drop", "click"] : kind);
    if (line !== null) this.say(detail ? `${detail}\n${line}` : line);
    else if (detail) this.say(detail);
  }

  /** An unprompted line the user can react to: a click on it is an
   *  answer, the cross a dismissal, and letting it fade is neither.
   *  Returns false when a permission prompt holds the bubble. */
  nudge(line: string, react: (answered: boolean) => void): boolean {
    if (this.asking !== null) return false;
    this.nudging = false;
    this.say(line, NUDGE_MS);
    this.nudging = true;
    this.el.classList.add("nudge");
    const close = document.createElement("button");
    close.className = "x";
    close.textContent = "×";
    close.title = t("nudge.dismiss");
    let done = false;
    const once = (answered: boolean) => {
      if (done) return;
      done = true;
      react(answered);
      this.hide();
    };
    close.addEventListener("click", (e) => {
      e.stopPropagation();
      once(false);
    });
    this.el.onclick = () => once(true);
    this.el.append(close);
    return true;
  }

  say(line: string, hold = holdMs(line)) {
    if (this.asking !== null || this.nudging) return; // a prompt or a nudge outranks chatter
    clearTimeout(this.timer);
    clearTimeout(this.fade);
    this.el.classList.remove("ask", "nudge");
    this.el.onclick = null;
    this.el.textContent = line;
    this.el.hidden = false;
    // Two frames so the transition runs from the hidden state.
    requestAnimationFrame(() => requestAnimationFrame(() => this.el.classList.add("show")));
    this.timer = window.setTimeout(() => this.hide(), hold);
  }

  private hide() {
    this.nudging = false;
    this.el.onclick = null;
    this.el.classList.remove("show");
    this.fade = window.setTimeout(() => {
      this.el.hidden = true;
    }, FADE_MS);
  }
}

function hostLabel(key: string): string {
  switch (key.split(":")[0]) {
    case "claude-code":
      return "Claude Code";
    case "codex":
      return "Codex";
    case "opencode":
      return "OpenCode";
    case "dsh":
      return "DeepSeek Harness";
    case "kimi":
      return "Kimi Code";
    default:
      return key.split(":")[0] || "agent";
  }
}
