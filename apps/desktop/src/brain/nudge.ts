/** The one kind of model call the pet starts by itself: an unprompted
 *  line written for what just happened (ADR 8). Everything it sends is
 *  built here, so the preview the user accepted when turning it on is
 *  the same shape as every call after; `LIVE_SHAPE` names that shape,
 *  and changing what goes out means bumping it, which asks again. */
import type { Request } from "./llm";
import type { AwayNews, TriggerId } from "./proactive";

/** Bumped whenever `nudgeRequest` sends something new (ADR 8). */
export const LIVE_SHAPE = 2;

/** At most this many live lines a day on a paid API; beyond it the
 *  pack's lines. A subscription or a local model costs nothing per
 *  call, so there the cap only guards against a runaway loop. */
export const DAILY_CAP = 20;
export const FREE_CAP = 120;

export function dailyCap(r: { provider: string; local: boolean }): number {
  return r.provider === "claude-code" || r.local ? FREE_CAP : DAILY_CAP;
}

/** A slow reply is worse than a canned line, which is the fallback.
 *  Claude Code starts a process first, so it gets longer. */
export const TIMEOUT_MS = 8000;
export const CLI_TIMEOUT_MS = 15_000;

export function timeoutMs(r: { provider: string }): number {
  return r.provider === "claude-code" ? CLI_TIMEOUT_MS : TIMEOUT_MS;
}

/** What is known about the moment beyond the context. */
export interface Moment {
  /** `app`: the kind of app switched to. */
  app?: string | null;
  /** `back`: what happened while the user was away. */
  news?: AwayNews | null;
  /** `report`: today's counters. */
  counters?: Record<string, number>;
  /** Lines the pet already said today, so it says something else. */
  recent?: string[];
  /** The pack's stock line for this moment, for tone. */
  packLine?: string | null;
}

/** How many of the day's lines are quoted back; a day has few. */
export const RECENT_LINES = 12;

function situation(trigger: TriggerId, m: Moment): string {
  switch (trigger) {
    case "waiting_long":
      return "An agent has been waiting several minutes for the user to approve something.";
    case "errors":
      return "The agent has just failed twice in a row.";
    case "long_done":
      return "A task that took more than twenty minutes just finished.";
    case "report": {
      const c = m.counters ?? {};
      return `The day is winding down. Today: ${c.sessions ?? 0} sessions, ${c.turns ?? 0} turns, ${c.tools ?? 0} tool calls, ${c.errors ?? 0} errors, ${c.waited ?? 0} minutes spent waiting for the user.`;
    }
    case "morning":
      return "The user has just started the day at the computer.";
    case "night":
      return "It is late at night and the user is still working with an agent.";
    case "break":
      return "The user has been at it for over two hours without a pause.";
    case "back": {
      const n = m.news;
      if (!n) return "The user just came back to the computer after a while away.";
      return `The user just came back after ${n.minutes} minutes away. Meanwhile ${n.done} sessions finished, ${n.errors} failed, and ${n.waiting} are waiting for them now.`;
    }
    case "app":
      return m.app ? `The user just switched to a ${m.app} app.` : "The user just switched to another app.";
    case "hop":
      return "You just hopped a little way across the screen on your own. Nothing in particular happened.";
    case "chatter":
      return "Nothing in particular happened. A bit of small talk, drawn from what you can see if there is anything worth it.";
  }
}

/** `context` comes from `brain/context.ts` over stored events only, so
 *  the permission level and redaction have already been applied. */
export function nudgeRequest(trigger: TriggerId, personaPrompt: string, context: string, m: Moment = {}): Request {
  const recent = (m.recent ?? []).slice(-RECENT_LINES);
  return {
    purpose: "nudge",
    system: personaPrompt,
    user: [
      situation(trigger, m),
      "",
      "What you can see:",
      context,
      ...(recent.length > 0 ? ["", "Lines you already said today, oldest first. Say something else; do not repeat or paraphrase any of them:", ...recent.map((l) => `- ${l}`)] : []),
      ...(m.packLine ? ["", `A stock line for this moment, for tone only: ${m.packLine}`] : []),
      "",
      "Say one short line to the user about it, in character, under 80 characters. Mention a concrete detail from what you can see when there is one. Reply with the line only.",
    ].join("\n"),
    maxTokens: 120,
  };
}

/** One line, no quotes, no markdown, short enough for the bubble; null
 *  when the reply is not usable and the pack line should be said. */
export function cleanLine(text: string): string | null {
  const line = text
    .split("\n")
    .map((l) => l.trim())
    .find((l) => l.length > 0);
  if (!line) return null;
  const bare = line.replace(/^["'“”‘’「」*_`]+|["'“”‘’「」*_`]+$/g, "").trim();
  return bare.length > 0 && bare.length <= 140 ? bare : null;
}
