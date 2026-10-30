/** The chat's short-term memory (ADR 11): turns stored in `chat_turns`
 *  and reloaded when the window opens, together with the lines the
 *  pet said in its bubble, so a bubble line can be answered in chat
 *  and the pet does not say the same thing twice. */
import type { ChatTurn } from "../generated/ChatTurn";
import type { Turn } from "./llm";

/** How far back the window reloads; the rows themselves live a week. */
export const RELOAD_HOURS = 24;
/** Bubble lines are few per day, so this many fit the prompt easily. */
export const BUBBLE_LINES = 12;

/** Stored turns as model history. Bubble lines stay out (they go in
 *  the system prompt instead), consecutive turns of one role are
 *  joined, and a leading pet turn is dropped: every provider wants the
 *  user to speak first, and a failed send leaves a user turn alone. */
export function toHistory(turns: ChatTurn[]): Turn[] {
  const out: Turn[] = [];
  for (const t of turns) {
    if (t.channel !== "chat") continue;
    const role = t.role === "user" ? "user" : "assistant";
    const last = out.at(-1);
    if (last && last.role === role) last.content += `\n\n${t.body}`;
    else if (out.length > 0 || role === "user") out.push({ role, content: t.body });
  }
  return out;
}

/** The system prompt part listing what the pet said in its bubble
 *  lately, oldest first with its age, so "yes, that one" has a
 *  referent and the next line is not the same again. */
export function bubbleBlock(turns: ChatTurn[], now = Date.now()): string {
  const lines = turns.filter((t) => t.channel === "bubble").slice(-BUBBLE_LINES);
  if (lines.length === 0) return "";
  const rows = lines.map((t) => `- ${ago(Date.parse(t.ts), now)}: ${t.body}`);
  return `Lines you said on your own in your speech bubble recently (the user may be replying to the last one):\n${rows.join("\n")}`;
}

function ago(then: number, now: number): string {
  const m = Math.max(0, Math.round((now - then) / 60_000));
  if (m < 1) return "just now";
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  return h < 24 ? `${h} h ago` : `${Math.round(h / 24)} d ago`;
}
