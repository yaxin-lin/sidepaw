/** What the pet remembers, as it reaches the model and comes back.
 *  The list itself lives in SQLite (`memories`), visible and editable
 *  in the setup window; nothing here is remembered without showing up
 *  there. */
import type { Memory } from "../generated/Memory";

/** At most this many go into a chat request, newest first. */
export const MEMORY_LIMIT = 30;

/** The system prompt part for chat. Asks for at most one new memory
 *  per reply, in a marker the client strips and stores. */
export function memoryBlock(memories: Memory[]): string {
  const lines = memories.slice(0, MEMORY_LIMIT).map((m) => `- ${m.body}`);
  return [
    lines.length
      ? `Things you remember about the user (they can see and edit this list):\n${lines.join("\n")}`
      : "You do not remember anything about the user yet.",
    "When the user tells you something about themselves or their work that is worth remembering for later, end your reply with one line exactly like [remember: <a short fact in the third person>]. At most one per reply, only for lasting facts, never secrets, keys, passwords or private data about other people. Do not mention the marker.",
  ].join("\n");
}

const MARKER = /^\s*\[remember:\s*([^\]\n]+?)\s*\]\s*$/gim;

/** Splits a finished reply into what the user sees and the facts the
 *  model asked to remember. */
export function extractRemember(text: string): { reply: string; facts: string[] } {
  const facts: string[] = [];
  const reply = text
    .replace(MARKER, (_, fact: string) => {
      facts.push(fact.trim());
      return "";
    })
    .trim();
  return { reply, facts: facts.filter((f) => f.length > 0 && f.length <= 200).slice(0, 1) };
}

/** While streaming, hide a marker that has started but not finished. */
export function hidePartialMarker(text: string): string {
  return text.replace(/\n?\s*\[rem[^\]\n]*\]?\s*$/i, "").replace(MARKER, "").trimEnd();
}

/** "记住…" / "remember …" typed by the user is stored as they wrote it,
 *  without a model, so it works with no provider configured. */
export function directRemember(message: string): string | null {
  const m = message.trim().match(/^(?:记住|请记住|remember(?: that)?)[\s:：，,]*([\s\S]+)$/i);
  const fact = m?.[1]?.trim();
  return fact && fact.length <= 200 ? fact : null;
}
