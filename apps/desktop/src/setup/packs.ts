/** The look section: the packs on this machine (bundled and installed)
 *  with a preview of each, importing a zip or a folder, bringing over
 *  pets from Codex or petdex folders (the petdex store is `store.ts`).
 *  Choosing one is saved in `[pet] active`; the pet swaps at once and
 *  this page reloads for the new pet's persona. */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { activePack, BUNDLED, parseJson, readPackText } from "../brain/files";
import type { Found } from "../generated/Found";
import type { PackInfo } from "../generated/PackInfo";
import { t, type Key } from "../i18n";
import { CELL_H, CELL_W, loadSheet, type Manifest } from "../pack";
import { busy, el, note } from "./dom";

/** The first idle frame, small. */
async function preview(id: string, manifest: Manifest): Promise<HTMLCanvasElement> {
  const c = document.createElement("canvas");
  const dpr = window.devicePixelRatio || 1;
  c.width = 96 * dpr;
  c.height = 104 * dpr;
  try {
    const sheet = await loadSheet(id, manifest);
    c.getContext("2d")?.drawImage(sheet, 0, 0, CELL_W, CELL_H, 0, 0, c.width, c.height);
  } catch {
    // A broken sheet shows as an empty frame; installing checked it once.
  }
  return c;
}

function byline(info: PackInfo): string {
  const s = info.source;
  if (!s) return info.license ?? "";
  const from = t(`packs.from.${s.kind}` as Key);
  return s.author ? `${from} · ${s.author}` : from;
}

async function card(id: string, info: PackInfo | null, active: string): Promise<HTMLElement> {
  const manifest = parseJson<Manifest>(await readPackText(id, "pet.json"));
  const div = document.createElement("div");
  div.className = id === active ? "card on" : "card";
  if (manifest) div.append(await preview(id, manifest));
  const name = document.createElement("div");
  name.className = "name";
  name.textContent = info?.display_name ?? manifest?.displayName ?? id;
  name.title = info?.description ?? manifest?.description ?? "";
  const by = document.createElement("div");
  by.className = "by";
  by.textContent = info ? byline(info) : t("packs.bundled");
  const acts = document.createElement("div");
  acts.className = "acts";
  if (id !== active) {
    const use = document.createElement("button");
    use.className = "primary";
    use.textContent = t("packs.use");
    use.addEventListener("click", () => void busy("pack-msg", () => invoke("set_active_pack", { id })));
    acts.append(use);
  }
  if (info) {
    const del = document.createElement("button");
    del.textContent = t("packs.remove");
    del.addEventListener("click", () =>
      busy("pack-msg", async () => {
        await invoke("pack_remove", { id });
        note("pack-msg", t("packs.removed", { name: info.display_name }));
        await renderCards();
      }),
    );
    acts.append(del);
  }
  div.append(name, by, acts);
  return div;
}

async function renderCards() {
  const [installed, active] = await Promise.all([invoke<PackInfo[]>("packs_installed"), activePack()]);
  const cards = await Promise.all([
    ...BUNDLED.map((id) => card(id, null, active)),
    ...installed.map((p) => card(p.id, p, active)),
  ]);
  el("pack-cards").replaceChildren(...cards);
  await renderFound();
}

async function installed(info: PackInfo) {
  note("pack-msg", t("packs.installed", { name: info.display_name }));
  await renderCards();
}

async function renderFound() {
  const found = await invoke<Found[]>("packs_found").catch(() => [] as Found[]);
  const box = el("pack-found");
  box.replaceChildren();
  if (found.length === 0) return;
  const head = document.createElement("h3");
  head.className = "sub3";
  head.textContent = t("packs.found");
  const list = document.createElement("ul");
  list.className = "results";
  for (const f of found) {
    const li = document.createElement("li");
    const name = document.createElement("span");
    name.textContent = f.display_name;
    const app = document.createElement("em");
    app.textContent = t(`packs.from.${f.app}` as Key);
    const b = document.createElement("button");
    b.textContent = t("packs.bring");
    b.addEventListener("click", () =>
      busy("pack-msg", async () => installed(await invoke<PackInfo>("pack_install_path", { path: f.path, kind: f.app }))),
    );
    li.append(name, app, b);
    list.append(li);
  }
  box.append(head, list);
}

async function importPath(directory: boolean) {
  const picked = await open(
    directory ? { directory: true } : { multiple: false, filters: [{ name: "Pet pack", extensions: ["zip"] }] },
  );
  if (typeof picked !== "string") return;
  await busy("pack-msg", async () => installed(await invoke<PackInfo>("pack_install_path", { path: picked })));
}

export async function mountPacks() {
  el("pack-import-zip").addEventListener("click", () => void importPath(false));
  el("pack-import-folder").addEventListener("click", () => void importPath(true));
  // The persona below belongs to the chosen pet, so the page follows.
  await listen("pack-active-changed", () => location.reload());
  await renderCards();
}
