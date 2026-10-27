import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Snapshot } from "./generated/Snapshot";
import { Bubble } from "./bubble";
import { DesktopLife } from "./desktop";
import { HitMaskSource } from "./hitmask";
import { t } from "./i18n";
import { IdleBehaviour } from "./idle";
import { DEFAULT_PACK, loadLines, loadPack } from "./pack";
import { SpriteRenderer } from "./renderer";

const POLL_MS = 500;

function describe(snap: Snapshot): string {
  const s = snap.overall;
  const detail = s.state === "tool" ? ` ${s.detail}` : "";
  const n = snap.sessions.length;
  return `${t(`state.${s.state}`)}${detail}${n > 1 ? ` (${n})` : ""}`;
}

async function main() {
  const canvas = document.getElementById("pet") as HTMLCanvasElement;
  const label = document.getElementById("label") as HTMLDivElement;
  const bubbleEl = document.getElementById("bubble") as HTMLDivElement;

  const pack = await loadPack(DEFAULT_PACK);
  const renderer = new SpriteRenderer(canvas, pack.sheet);
  const bubble = new Bubble(bubbleEl, pack.lines);
  const idle = new IdleBehaviour(renderer, canvas);
  // Transparent parts of the window let clicks through; see src-tauri/src/hit.rs.
  // The bubble is solid while shown, so its box joins the mask.
  const mask = new HitMaskSource(canvas, (ctx, w, h) => renderer.drawCurrentInto(ctx, w, h));
  // The bubble sits just above the pet's top edge. Idle animations
  // bob a few pixels, so only follow moves larger than that.
  let anchor = -1;
  renderer.onFrame = () => {
    const top = mask.push([label, bubbleEl]);
    if (top !== null && Math.abs(top - anchor) > 12) {
      anchor = top;
      bubbleEl.style.bottom = `${window.innerHeight - top + 6}px`;
    }
  };
  renderer.start();
  await listen("pack-changed", () => {
    loadLines(DEFAULT_PACK, pack.manifest).then(
      (lines) => bubble.setLines(lines),
      (err: unknown) => console.error(String(err)),
    );
  });

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
    if (e.button === 0 && Date.now() - pressedAt < 250) {
      bubble.onInteraction("click");
      idle.onClick();
    }
  });
  await new DesktopLife(renderer, idle, bubble, canvas).attach();

  canvas.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    void invoke("open_setup");
  });

  const poll = async () => {
    try {
      const snap = await invoke<Snapshot>("snapshot");
      renderer.setState(snap.overall);
      const pending = snap.pending[0];
      if (pending) {
        bubble.askApproval(pending, (allow) => {
          bubble.clearApproval();
          void invoke("decide", { id: pending.id, allow });
        });
      } else {
        bubble.clearApproval();
      }
      bubble.onState(snap.overall);
      idle.onState(snap.overall);
      label.textContent = describe(snap);
      void idle.tick();
    } catch (err) {
      label.textContent = `error: ${String(err)}`;
    }
  };
  await poll();
  setInterval(() => void poll(), POLL_MS);
}

main().catch((err: unknown) => {
  const label = document.getElementById("label");
  if (label) label.textContent = `failed: ${String(err)}`;
  // The dev log only shows the message; the stack says where.
  console.error(err instanceof Error ? `${err.message}\n${err.stack ?? ""}` : String(err));
});
window.addEventListener("unhandledrejection", (e) => {
  const r: unknown = e.reason;
  console.error(`unhandled: ${r instanceof Error ? `${r.message}\n${r.stack ?? ""}` : String(r)}`);
});
