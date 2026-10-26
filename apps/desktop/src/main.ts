import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Snapshot } from "./generated/Snapshot";
import { pushHitMask } from "./hitmask";
import { loadPack } from "./pack";
import { SpriteRenderer } from "./renderer";

const POLL_MS = 500;

function describe(snap: Snapshot): string {
  const s = snap.overall;
  const detail = s.state === "tool" ? ` ${s.detail}` : "";
  const n = snap.sessions.length;
  return `${s.state}${detail}${n > 1 ? ` (${n})` : ""}`;
}

async function main() {
  const canvas = document.getElementById("pet") as HTMLCanvasElement;
  const label = document.getElementById("label") as HTMLDivElement;

  const { sheet } = await loadPack("placeholder");
  const renderer = new SpriteRenderer(canvas, sheet);
  // Transparent parts of the window let clicks through; see src-tauri/src/hit.rs.
  renderer.onFrame = () => pushHitMask(canvas, renderer.ctx, [label]);
  renderer.start();

  // The window has no decorations, so the pet itself is the drag handle.
  canvas.addEventListener("mousedown", (e) => {
    if (e.button === 0) void getCurrentWindow().startDragging();
  });

  const poll = async () => {
    try {
      const snap = await invoke<Snapshot>("snapshot");
      renderer.setState(snap.overall);
      label.textContent = describe(snap);
    } catch (err) {
      label.textContent = `error: ${String(err)}`;
    }
  };
  await poll();
  setInterval(() => void poll(), POLL_MS);
}

main().catch((err) => {
  const label = document.getElementById("label");
  if (label) label.textContent = `failed: ${String(err)}`;
  console.error(err);
});
