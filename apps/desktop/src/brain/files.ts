/** Per-pet files come from three places: a bundled pack (served next
 *  to the page), an installed one under `~/.sidepaw/packs/<id>/`, and
 *  the user's overrides under `~/.sidepaw/pets/<id>/` (both through
 *  Tauri commands). The override wins when it exists. */
import { invoke } from "@tauri-apps/api/core";
import type { Persona } from "./persona";

/** Packs that ship inside the app. */
export const BUNDLED = ["placeholder"] as const;
export const FALLBACK_PACK = "placeholder";

export function isBundled(id: string): boolean {
  return (BUNDLED as readonly string[]).includes(id);
}

export type PetFile = "persona.json" | "persona.md" | "lines.json";

export function readUser(id: string, name: PetFile): Promise<string | null> {
  return invoke<string | null>("pet_file_read", { id, name });
}

/** Empty text removes the override. */
export function writeUser(id: string, name: PetFile, text: string): Promise<void> {
  return invoke("pet_file_write", { id, name, text });
}

/** A file of a pack as bytes; null when the pack does not have it.
 *  The Vite dev server answers unknown paths with index.html and
 *  status 200, so an HTML content type also reads as absent. */
export async function readPackBytes(id: string, name: string): Promise<ArrayBuffer | null> {
  if (isBundled(id)) {
    const res = await fetch(`/${id}/${name}`);
    if (!res.ok || (res.headers.get("content-type") ?? "").includes("html")) return null;
    return res.arrayBuffer();
  }
  const bytes = await invoke<ArrayBuffer>("pack_file", { id, name }).catch(() => null);
  return bytes && bytes.byteLength > 0 ? bytes : null;
}

export async function readPackText(id: string, name: string): Promise<string | null> {
  const bytes = await readPackBytes(id, name);
  return bytes ? new TextDecoder().decode(bytes) : null;
}

/** `[pet] active`, or the bundled pack when that one is gone. */
export async function activePack(): Promise<string> {
  const id = await invoke<string>("get_active_pack").catch(() => FALLBACK_PACK);
  if (isBundled(id)) return id;
  return (await readPackBytes(id, "pet.json")) ? id : FALLBACK_PACK;
}

/** The user's override, else the pack's file, else a persona named
 *  after the pack (most downloaded packs bring art only). */
export async function loadPersona(pack: string): Promise<{ persona: Persona; md: string }> {
  const manifest = parseJson<{ displayName?: string }>(await readPackText(pack, "pet.json"));
  const persona =
    parseJson<Persona>(await readUser(pack, "persona.json")) ??
    parseJson<Persona>(await readPackText(pack, "persona.json")) ??
    ({ version: 1, name: manifest?.displayName || "Pet" } as Persona);
  const md = (await readUser(pack, "persona.md")) ?? (await readPackText(pack, "persona.md")) ?? "";
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
