/** The two things every section of the setup window does: find an
 *  element by id and put a status line under a button. */
import { t } from "../i18n";

export const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

export function note(id: string, text: string, err = false) {
  const m = el(id);
  m.className = err ? "msg err" : "msg";
  m.textContent = text;
}

/** Runs `work` with "Working..." shown and any error shown in its place. */
export async function busy<T>(id: string, work: () => Promise<T>): Promise<T | undefined> {
  note(id, t("working"));
  try {
    return await work();
  } catch (e) {
    note(id, String(e), true);
    return undefined;
  }
}
