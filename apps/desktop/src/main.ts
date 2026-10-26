import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Snapshot } from "./generated/Snapshot";
import { Bubble } from "./bubble";
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
  const bubbleEl = document.getElementById("bubble") as HTMLDivElement;

  const pack = await loadPack("placeholder");
  const renderer = new SpriteRenderer(canvas, pack.sheet);
  const bubble = new Bubble(bubbleEl, pack.lines);
  // Transparent parts of the window let clicks through; see src-tauri/src/hit.rs.
  // The bubble is solid while shown, so its box joins the mask.
  renderer.onFrame = () => pushHitMask(canvas, renderer.ctx, [label, bubbleEl]);
  renderer.start();

  // The window has no decorations, so the pet itself is the drag handle
  // and right-click is the only way in to settings. A click that does
  // not turn into a drag gets a line.
  let pressedAt = 0;
  canvas.addEventListener("mousedown", (e) => {
    if (e.button !== 0) return;
    pressedAt = Date.now();
    void getCurrentWindow().startDragging();
  });
  canvas.addEventListener("mouseup", (e) => {
    if (e.button === 0 && Date.now() - pressedAt < 250) bubble.onInteraction("click");
  });
  canvas.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    void invoke("open_setup");
  });

  const poll = async () => {
    try {
      const snap = await invoke<Snapshot>("snapshot");
      renderer.setState(snap.overall);
      bubble.onState(snap.overall);
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
