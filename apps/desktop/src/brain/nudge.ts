/** The one kind of model call the pet starts by itself: a proactive
 *  line written for what just happened (ADR 8). Everything it sends is
 *  built here, so the preview the user accepted when turning it on is
 *  the same shape as every call after. */
import type { Request } from "./llm";
import type { TriggerId } from "./proactive";

/** The triggers worth a model call: their value is in the specifics. */
export const LIVE: ReadonlySet<TriggerId> = new Set(["waiting_long", "errors", "long_done"]);

/** At most this many live lines a day; beyond it the pack's lines. */
export const DAILY_CAP = 20;

/** A slow reply is worse than a canned line, which is the fallback. */
export const TIMEOUT_MS = 8000;

const SITUATION: Record<string, string> = {
  waiting_long: "An agent has been waiting several minutes for the user to approve something.",
  errors: "The agent has just failed twice in a row.",
  long_done: "A task that took more than twenty minutes just finished.",
};

/** `context` comes from `brain/context.ts` over stored events only, so
 *  the permission level and redaction have already been applied. */
export function nudgeRequest(trigger: TriggerId, personaPrompt: string, context: string): Request {
  return {
    purpose: "nudge",
    system: personaPrompt,
    user: [
      SITUATION[trigger] ?? "",
      "",
      "What you can see:",
      context,
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
