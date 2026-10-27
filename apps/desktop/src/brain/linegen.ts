/** Turns a compiled persona into `lines.json` in one model call, so the
 *  pet talks in character all day without touching the network again. */
import type { LinesFile } from "../lines";
import type { Request } from "./llm";

/** The keys the renderer asks for (`lines.ts`, `bubble.ts`) with the
 *  situation each one describes to the model. */
export const LINE_KEYS: ReadonlyArray<{ key: string; when: string }> = [
  { key: "idle", when: "nothing is running; the user is typing or away" },
  { key: "thinking", when: "an agent is thinking before acting" },
  { key: "tool", when: "an agent is running some tool" },
  { key: "tool.shell", when: "an agent is running a shell command" },
  { key: "tool.edit", when: "an agent is editing files" },
  { key: "tool.read", when: "an agent is reading files" },
  { key: "tool.search", when: "an agent is searching the code" },
  { key: "waiting", when: "an agent is waiting for the user's approval; the user should look" },
  { key: "done", when: "an agent just finished its turn" },
  { key: "error", when: "an agent just hit an error" },
  { key: "click", when: "the user clicked you" },
  { key: "drag", when: "the user is dragging you around the screen" },
  { key: "drop", when: "the user dropped a file on you" },
  { key: "morning", when: "the first time the user shows up in the morning" },
  { key: "night", when: "it is late at night and an agent is still working" },
  { key: "waiting_long", when: "an agent has been waiting for the user's approval for several minutes" },
  { key: "errors", when: "the agent has failed twice in a row" },
  { key: "long_done", when: "a task that took more than twenty minutes just finished" },
  { key: "report", when: "the end of the working day; mention the numbers with the placeholders {tools} (tool calls), {turns}, {errors} and {waited} (minutes the user kept an agent waiting), each line using at least one" },
  { key: "break", when: "the user has worked for two hours without a pause; suggest a short break" },
  { key: "back", when: "the user just came back after being away for a while" },
  { key: "chatter", when: "nothing in particular is happening; idle small talk in character" },
];

export const LINES_PER_KEY = 30;
/** The prompt asks for under 60 characters; the filter allows twice
 *  that so a slightly long line survives and a paragraph does not. */
const MAX_LINE_CHARS = 120;

export function linesRequest(systemPrompt: string): Request {
  const keys = LINE_KEYS.map((k) => `- "${k.key}": ${k.when}`).join("\n");
  const user = [
    `Write ${LINES_PER_KEY} short lines you might say in each of these situations, in character, in the language your instructions ask for.`,
    "Each line stands alone: no numbering, no quotes, no emoji, under 60 characters, varied in mood and wording. Never mention a specific file, command or error you have not been told about.",
    "",
    "Situations:",
    keys,
    "",
    `Reply with one JSON object only, no prose and no code fence: the keys above, each an array of ${LINES_PER_KEY} strings.`,
  ].join("\n");
  // 22 keys by 30 lines at roughly 25 tokens each is about 17k tokens.
  return { purpose: "lines", system: systemPrompt, user, maxTokens: 24000 };
}

/** Takes what a model returns, including fences or prose around the
 *  JSON, and keeps only well-formed lines for known keys. */
export function parseLines(text: string, generatedFrom: string): LinesFile {
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start < 0 || end <= start) throw new Error("the reply contained no JSON object");
  let parsed: unknown;
  try {
    parsed = JSON.parse(text.slice(start, end + 1));
  } catch {
    throw new Error("the reply was not valid JSON");
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("the reply was not a JSON object");
  }
  const file: LinesFile = { version: 1, generatedFrom };
  let total = 0;
  for (const { key } of LINE_KEYS) {
    const raw = (parsed as Record<string, unknown>)[key];
    if (!Array.isArray(raw)) continue;
    const seen = new Set<string>();
    const lines: string[] = [];
    for (const item of raw) {
      if (typeof item !== "string") continue;
      const line = item.trim().replace(/^["'“”‘’]+|["'“”‘’]+$/g, "").trim();
      if (!line || line.length > MAX_LINE_CHARS || seen.has(line)) continue;
      seen.add(line);
      lines.push(line);
    }
    if (lines.length) {
      file[key] = lines;
      total += lines.length;
    }
  }
  if (total === 0) throw new Error("the reply had no usable lines");
  return file;
}

export function countLines(file: LinesFile): number {
  let n = 0;
  for (const v of Object.values(file)) if (Array.isArray(v)) n += v.length;
  return n;
}
