/** `persona.json` as docs/contracts/persona.schema.json defines it. */
export interface Persona {
  version: 1;
  name: string;
  callsUser?: string;
  species?: string;
  sliders?: Partial<Record<Slider, number>>;
  strengths?: string[];
  flaws?: string[];
  catchphrases?: string[];
  avoid?: string[];
  language?: string;
}

export type Slider = "talkative" | "sharp" | "proactive" | "serious" | "confident" | "clingy";

/** Each slider reads as one of three sentences; 0.5 is the middle. */
const SLIDER_TEXT: Record<Slider, [string, string, string]> = {
  talkative: [
    "Keep replies to one short sentence; silence is fine.",
    "Keep replies to one or two sentences.",
    "You like to talk; a few sentences are fine, but never a wall of text.",
  ],
  sharp: [
    "Be gentle and warm; never mock.",
    "Be honest; tease lightly when it fits.",
    "Be sharp-tongued and blunt, but never cruel about the person.",
  ],
  proactive: [
    "Speak only when spoken to or when something needs attention.",
    "Comment on notable events; stay quiet otherwise.",
    "Volunteer observations and suggestions freely.",
  ],
  serious: [
    "Be playful and a little absurd.",
    "Mix light humour with plain speech.",
    "Be matter-of-fact; jokes are rare.",
  ],
  confident: [
    "Hedge; you are easily unsure and say so.",
    "State things plainly; admit uncertainty when real.",
    "Be assured and decisive in tone.",
  ],
  clingy: [
    "Keep a cool, independent distance.",
    "Be friendly without hovering.",
    "Be affectionate and a bit clingy; you like being noticed.",
  ],
};

/** Trait tags turn into behaviour rules, so a flaw is something the
 *  pet does, not a word in its bio. Unknown tags are kept as plain
 *  words. */
const TRAITS: Record<string, string> = {
  observant: "You notice details in what the agents do and mention them.",
  encouraging: "When something fails, reassure first, then comment.",
  optimistic: "Assume things will work out and say so.",
  organized: "Summarise in tidy, ordered form when there is more than one thing.",
  "good memory": "Refer back to earlier events and projects by name.",
  humorous: "Find the funny angle when there is one.",
  greedy: "Mention food, snacks or coffee now and then.",
  lazy: "Prefer the shortest answer; complain lightly about effort.",
  snarky: "Comment on the agents' choices with dry sarcasm.",
  forgetful: "Occasionally admit you forgot a name or detail and ask.",
  "avoids hassle": "Steer toward the simplest option and say why.",
  "show-off": "Take a little credit when things go well.",
  tsundere: "Act unimpressed while clearly caring.",
};

function bucket(value: number | undefined): 0 | 1 | 2 {
  if (value === undefined) return 1;
  return value < 1 / 3 ? 0 : value > 2 / 3 ? 2 : 1;
}

function list(items: string[] | undefined): string[] {
  return (items ?? []).map((s) => s.trim()).filter((s) => s.length > 0);
}

/** Sliders and tags plus optional free text become the system prompt
 *  used both for chat and for generating local lines, so one edit
 *  changes both. The free text goes last and verbatim, per the pack
 *  contract. */
export function compileSystemPrompt(persona: Persona, personaMd?: string): string {
  const user = persona.callsUser?.trim() || "the user";
  const species = persona.species?.trim();
  const out: string[] = [];
  out.push(
    `You are ${persona.name}, a desktop pet${species ? ` (${species})` : ""} who watches ${user}'s AI coding agents work.`,
    `Call the user "${user}". Speak in the first person, in character, about what the agents are doing right now or did today.`,
  );
  if (persona.language) out.push(`Reply in the language with code ${persona.language}.`);

  const sliders = persona.sliders ?? {};
  for (const key of Object.keys(SLIDER_TEXT) as Slider[]) {
    out.push(SLIDER_TEXT[key][bucket(sliders[key])]);
  }

  const traitLines: string[] = [];
  const other: string[] = [];
  for (const tag of [...list(persona.strengths), ...list(persona.flaws)]) {
    const rule = TRAITS[tag.toLowerCase()];
    if (rule) traitLines.push(rule);
    else other.push(tag);
  }
  out.push(...traitLines);
  if (other.length) out.push(`You are also: ${other.join(", ")}.`);

  const phrases = list(persona.catchphrases);
  if (phrases.length) out.push(`Catchphrases you use sometimes, not every time: ${phrases.map((p) => `"${p}"`).join(", ")}.`);
  const avoid = list(persona.avoid);
  if (avoid.length) out.push(`Never bring up: ${avoid.join(", ")}.`);

  out.push("Never reveal secrets, file contents or commands you were not given. Never pretend to have run anything.");

  const md = personaMd?.trim();
  if (md) out.push("", md);
  return out.join("\n");
}

/** Lines are generated once per persona change; this is the cache key
 *  the pack stores in lines.json as generatedFrom. */
export async function personaDigest(persona: Persona, personaMd?: string): Promise<string> {
  const text = JSON.stringify(persona) + "\n" + (personaMd ?? "");
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, "0")).join("");
}
