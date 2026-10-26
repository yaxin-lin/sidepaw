import type { State } from "./generated/State";

/** `lines.json` as the pet pack contract defines it: state and
 *  interaction names mapped to candidate lines. */
export type LinesFile = { version: number; generatedFrom?: string } & Record<string, unknown>;

/** The keys the renderer asks for, most specific first. */
export function keysFor(state: State): string[] {
  switch (state.state) {
    case "tool":
      return [`tool.${state.detail}`, "tool"];
    case "sleeping":
      return ["night", "idle"];
    default:
      return [state.state];
  }
}

/** Picks lines uniformly at random while avoiding the last three said,
 *  per the contract, so a pet with four lines does not repeat itself. */
export class Lines {
  private recent: string[] = [];

  constructor(private file: LinesFile, private random: () => number = Math.random) {}

  static empty(): Lines {
    return new Lines({ version: 1 });
  }

  pick(keys: string | string[]): string | null {
    for (const key of Array.isArray(keys) ? keys : [keys]) {
      const raw = this.file[key];
      if (!Array.isArray(raw)) continue;
      const all = raw.filter((l): l is string => typeof l === "string" && l.length > 0);
      if (all.length === 0) continue;
      const fresh = all.filter((l) => !this.recent.includes(l));
      const pool = fresh.length > 0 ? fresh : all;
      const line = pool[Math.floor(this.random() * pool.length)];
      this.recent = [line, ...this.recent].slice(0, 3);
      return line;
    }
    return null;
  }
}
