/** Per-pet files come from two places: the bundled pack (served next
 *  to the page) and the user's overrides under `~/.sidepaw/pets/<id>/`
 *  (through Tauri commands). The override wins when it exists. */
import { invoke } from "@tauri-apps/api/core";
import type { Persona } from "./persona";

export type PetFile = "persona.json" | "persona.md" | "lines.json";

export function readUser(id: string, name: PetFile): Promise<string | null> {
  return invoke<string | null>("pet_file_read", { id, name });
}

/** Empty text removes the override. */
export function writeUser(id: string, name: PetFile, text: string): Promise<void> {
  return invoke("pet_file_write", { id, name, text });
}

/** A missing file is null. The Vite dev server answers unknown paths
 *  with index.html and status 200, so an HTML content type also reads
 *  as absent. */
export async function readBundled(id: string, name: string): Promise<string | null> {
  const res = await fetch(`/${id}/${name}`);
  if (!res.ok || (res.headers.get("content-type") ?? "").includes("html")) return null;
  return res.text();
}

/** The user's override, else the pack's file, else a blank persona. */
export async function loadPersona(pack: string): Promise<{ persona: Persona; md: string }> {
  const persona =
    parseJson<Persona>(await readUser(pack, "persona.json")) ??
    parseJson<Persona>(await readBundled(pack, "persona.json")) ??
    ({ version: 1, name: "Pet" } as Persona);
  const md = (await readUser(pack, "persona.md")) ?? (await readBundled(pack, "persona.md")) ?? "";
  return { persona, md };
}

export function parseJson<T>(text: string | null): T | null {
  if (text === null) return null;
  try {
    return JSON.parse(text) as T;
  } catch {
    return null;
  }
}
