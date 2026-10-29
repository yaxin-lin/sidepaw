/** The petdex store in Setup > Look: a grid of the gallery's pets with
 *  their thumbnails, sorted and filtered by petdex's search API, and
 *  one click to adopt one (download, install, switch to it). Nothing is
 *  fetched until the store is opened. When the search API is down the
 *  store falls back to petdex's static manifest, filtered here. */
import { invoke } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { activePack } from "../brain/files";
import type { PackInfo } from "../generated/PackInfo";
import type { Source } from "../generated/Source";
import { t } from "../i18n";
import { busy, el, note } from "./dom";

const SEARCH = "https://petdex.dev/api/pets/search";
const MANIFEST = "https://petdex.dev/api/manifest";
const ASSETS = "https://assets.petdex.dev/pets";
const PAGE = 24;
const TYPING_MS = 350;

export interface StorePet {
  slug: string;
  displayName: string;
  description?: string;
  kind?: string;
  zipUrl: string;
  /** An object from the search API, a plain name in the manifest. */
  submittedBy?: { name?: string } | string | null;
  metrics?: { installCount?: number };
}

export interface Query {
  q: string;
  sort: string;
  kind: string;
}

export function searchUrl(query: Query, cursor: number): string {
  const p = new URLSearchParams({ sort: query.sort, limit: String(PAGE), cursor: String(cursor), includeMeta: "0" });
  if (query.q) p.set("q", query.q);
  if (query.kind) p.set("kinds", query.kind);
  return `${SEARCH}?${p}`;
}

export function author(p: StorePet): string {
  const by = p.submittedBy;
  return (typeof by === "string" ? by : by?.name) ?? "";
}

/** The manifest's pets that match, for when the search API is down;
 *  it has no metrics, so it keeps petdex's own order. */
export function filterManifest(pets: StorePet[], query: Query): StorePet[] {
  const q = query.q.toLowerCase();
  return pets.filter(
    (p) =>
      (!query.kind || p.kind === query.kind) &&
      (!q || p.displayName.toLowerCase().includes(q) || p.slug.includes(q)),
  );
}

let manifest: StorePet[] | null = null;
let query: Query = { q: "", sort: "popular", kind: "" };
let cursor: number | null = 0;
/** Bumped on every new query so a slow page cannot land after it. */
let generation = 0;
let owned = new Map<string, string>();
let active = "";

async function page(query: Query, at: number): Promise<{ pets: StorePet[]; next: number | null }> {
  try {
    // Through the Rust HTTP client, which honours the system proxy.
    const res = await tauriFetch(searchUrl(query, at));
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const body = (await res.json()) as { pets: StorePet[]; nextCursor: number | null };
    return { pets: body.pets, next: body.nextCursor };
  } catch (e) {
    if (!manifest) {
      const res = await tauriFetch(MANIFEST);
      if (!res.ok) throw new Error(`petdex: ${String(e)}`);
      manifest = ((await res.json()) as { pets: StorePet[] }).pets;
    }
    const hits = filterManifest(manifest, query);
    const next = at + PAGE < hits.length ? at + PAGE : null;
    return { pets: hits.slice(at, at + PAGE), next };
  }
}

/** Header values must be ASCII; JSON with \u escapes is, and parses
 *  back to the same text. */
function asciiJson(value: unknown): string {
  return JSON.stringify(value).replace(/[\u007f-￿]/g, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, "0")}`);
}

async function adopt(p: StorePet) {
  await busy("store-msg", async () => {
    let id = owned.get(p.slug);
    if (!id) {
      const res = await tauriFetch(p.zipUrl);
      if (!res.ok) throw new Error(`petdex: HTTP ${res.status}`);
      const bytes = new Uint8Array(await res.arrayBuffer());
      const source: Source = { kind: "petdex", slug: p.slug, author: author(p) || null, url: p.zipUrl };
      const info = await invoke<PackInfo>("pack_install_zip", bytes, { headers: { "x-source": asciiJson(source) } });
      id = info.id;
    }
    // The page reloads on `pack-active-changed` with the new pet on.
    await invoke("set_active_pack", { id });
  });
}

/** The thumbnail, and on hover the first row of the sheet playing,
 *  when petdex has made that strip for the pet. */
function art(p: StorePet): HTMLElement {
  const box = document.createElement("div");
  box.className = "art";
  const img = document.createElement("img");
  img.loading = "lazy";
  img.alt = "";
  img.src = `${ASSETS}/${encodeURIComponent(p.slug)}/thumb.webp`;
  box.append(img);
  box.addEventListener(
    "mouseenter",
    () => {
      const strip = new Image();
      strip.onload = () => {
        box.style.backgroundImage = `url("${strip.src}")`;
        box.classList.add("plays");
      };
      strip.src = `${ASSETS}/${encodeURIComponent(p.slug)}/preview.webp`;
    },
    { once: true },
  );
  return box;
}

function card(p: StorePet): HTMLElement {
  const div = document.createElement("div");
  const id = owned.get(p.slug);
  div.className = id && id === active ? "card on" : "card";
  div.title = p.description ?? "";
  const name = document.createElement("div");
  name.className = "name";
  name.textContent = p.displayName;
  const by = document.createElement("div");
  by.className = "by";
  const installs = p.metrics?.installCount;
  by.textContent = [author(p), installs ? t("store.installs", { n: installs }) : ""].filter(Boolean).join(" · ");
  const b = document.createElement("button");
  if (id && id === active) {
    b.textContent = t("store.using");
    b.disabled = true;
  } else {
    b.className = "primary";
    b.textContent = id ? t("packs.use") : t("store.adopt");
    b.addEventListener("click", () => void adopt(p));
  }
  div.append(art(p), name, by, b);
  return div;
}

async function load(fresh: boolean) {
  if (cursor === null) return;
  const mine = fresh ? ++generation : generation;
  const at = fresh ? 0 : cursor;
  const more = el<HTMLButtonElement>("store-more");
  more.disabled = true;
  note("store-msg", t("working"));
  try {
    const { pets, next } = await page(query, at);
    if (mine !== generation) return;
    const grid = el("store-grid");
    if (fresh) grid.replaceChildren();
    grid.append(...pets.map(card));
    cursor = next;
    more.hidden = next === null;
    note("store-msg", grid.childElementCount ? "" : t("store.none"));
  } catch (e) {
    if (mine === generation) note("store-msg", t("store.offline", { err: String(e) }), true);
  } finally {
    more.disabled = false;
  }
}

function requery() {
  query = {
    q: el<HTMLInputElement>("store-q").value.trim(),
    sort: el<HTMLSelectElement>("store-sort").value,
    kind: el<HTMLSelectElement>("store-kind").value,
  };
  cursor = 0;
  void load(true);
}

async function open() {
  const [installed, current] = await Promise.all([invoke<PackInfo[]>("packs_installed"), activePack()]);
  owned = new Map(installed.filter((p) => p.source?.kind === "petdex" && p.source.slug).map((p) => [p.source!.slug!, p.id]));
  active = current;
  requery();
}

export function mountStore() {
  el<HTMLInputElement>("store-q").placeholder = t("store.search");
  const options = (id: string, pairs: [string, string][]) =>
    el<HTMLSelectElement>(id).replaceChildren(
      ...pairs.map(([value, label]) => Object.assign(document.createElement("option"), { value, textContent: label })),
    );
  options("store-sort", [
    ["popular", t("store.sort.popular")],
    ["installed", t("store.sort.installed")],
    ["recent", t("store.sort.recent")],
    ["curated", t("store.sort.curated")],
  ]);
  options("store-kind", [
    ["", t("store.kind.all")],
    ["character", t("store.kind.character")],
    ["creature", t("store.kind.creature")],
    ["object", t("store.kind.object")],
  ]);
  let typing: number | undefined;
  el("store-q").addEventListener("input", () => {
    clearTimeout(typing);
    typing = window.setTimeout(requery, TYPING_MS);
  });
  el("store-sort").addEventListener("change", requery);
  el("store-kind").addEventListener("change", requery);
  el("store-more").addEventListener("click", () => void load(false));
  let opened = false;
  el<HTMLDetailsElement>("store").addEventListener("toggle", (e) => {
    if (opened || !(e.currentTarget as HTMLDetailsElement).open) return;
    opened = true;
    void open();
  });
}
