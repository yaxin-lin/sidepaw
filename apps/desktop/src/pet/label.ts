import type { Snapshot } from "../generated/Snapshot";
import { t } from "../i18n";

/** The line under the pet, "sidepaw · waiting (2)": the project of the
 *  session the pet shows, its state, and how many sessions run. */
export function label(snap: Snapshot): string {
  const s = snap.overall;
  const detail = s.state === "tool" ? ` ${s.detail}` : "";
  const n = snap.sessions.length;
  const project = snap.sessions[0]?.project;
  return `${project ? `${project} · ` : ""}${t(`state.${s.state}`)}${detail}${n > 1 ? ` (${n})` : ""}`;
}
