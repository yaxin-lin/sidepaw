import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { PetPrefs } from "../generated/PetPrefs";
import type { Snapshot } from "../generated/Snapshot";
import { Bubble, type ApprovalKeys } from "./bubble";
import { DesktopLife } from "./desktop";
import { HitMaskSource } from "./hitmask";
import { initLang, onLangChange, t } from "../i18n";
import { IdleBehaviour } from "./idle";
import { activePack, FALLBACK_PACK } from "../brain/files";
import { loadLines, loadPack, LOOP_MS, type Pack, ROW } from "../pack";
import { label as describe } from "./label";
import { SpriteRenderer } from "./renderer";
import { Scheduler } from "./scheduler";

const POLL_MS = 500;

async function main() {
  const canvas = document.getElementById("pet") as HTMLCanvasElement;
  const label = document.getElementById("label") as HTMLDivElement;
  const bubbleEl = document.getElementById("bubble") as HTMLDivElement;

  await initLang();
  let packId = await activePack();
  // A pack that fails to load (a sheet deleted by hand) must not leave
  // an empty window; the bundled one always works.
  let pack: Pack = await loadPack(packId).catch(async (err: unknown) => {
    console.error(String(err));
    packId = FALLBACK_PACK;
    return loadPack(FALLBACK_PACK);
  });
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
  // Lines come per language, so both a pack edit and a language change
  // reload them; the label re-reads its strings on the next poll.
  const reloadLines = () => {
    loadLines(packId, pack.manifest).then(
      (next) => {
        lines = next;
        bubble.setLines(next);
      },
      (err: unknown) => console.error(String(err)),
    );
  };
  await listen("pack-changed", reloadLines);
  await listen<string>("pack-active-changed", (e) => {
    loadPack(e.payload).then(
      (next) => {
        packId = e.payload;
        pack = next;
        renderer.setSheet(next.sheet);
        lines = next.lines;
        bubble.setLines(next.lines);
        renderer.play(ROW.jumping, LOOP_MS);
      },
      (err: unknown) => bubble.say(String(err)),
    );
  });
  await onLangChange(reloadLines);

  // Presses, drags, throws and drops: see desktop.ts. Right-click is
  // the only way in to settings.
  // A click while an agent waits for you is a request to go there.
  let latest: Snapshot | null = null;
  const jumpToWaiting = async () => {
    const waiting = latest?.sessions.find((s) => s.state === "waiting" && s.origin);
    if (!waiting) return false;
    try {
      await invoke("focus_session", { key: waiting.key });
      return true;
    } catch (err) {
      console.warn(String(err));
      return false;
    }
  };
  // Lines change with the language and the persona, so the scheduler
  // asks for the current set each time. Answering a line opens chat.
  let lines = pack.lines;
  const scheduler = new Scheduler(bubble, () => lines, () => void invoke("open_chat"));
  await scheduler.start();
  const life = new DesktopLife(renderer, idle, bubble, canvas, jumpToWaiting);
  await life.attach();

  // Size is applied on the Rust side (window and zoom together); this
  // window carries out do-not-disturb and mini mode. See prefs.rs.
  let prefs = await invoke<PetPrefs>("pet_prefs");
  const applyPrefs = async (next: PetPrefs, previous: PetPrefs | null) => {
    bubble.quiet = next.dnd;
    scheduler.dnd = next.dnd;
    scheduler.chatty = next.chatty;
    if (next.mini !== previous?.mini || (next.mini && next.size !== previous?.size)) await life.setMini(next.mini);
  };
  await applyPrefs(prefs, null);
  await listen<PetPrefs>("prefs-changed", (e) => {
    const previous = prefs;
    prefs = e.payload;
    void applyPrefs(prefs, previous);
  });

  canvas.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    void invoke("open_setup");
  });

  // The shortcuts register a moment after the first prompt arrives, so
  // they are asked for once per prompt rather than once at startup.
  let keysFor = -1;
  let keys: ApprovalKeys = {};
  const poll = async () => {
    try {
      const snap = await invoke<Snapshot>("snapshot");
      latest = snap;
      renderer.setState(snap.overall);
      // Prompts queue oldest first; the bubble shows the head and a count,
      // and the keyboard shortcuts answer the same one (keys.rs).
      const pending = snap.pending[0];
      if (pending) {
        if (keysFor !== pending.id) {
          keysFor = pending.id;
          keys = await invoke<ApprovalKeys>("approval_keys").catch(() => ({}));
        }
        const project = snap.sessions.find((s) => s.key === pending.key)?.project ?? null;
        bubble.askApproval(pending, snap.pending.length - 1, keys, project, (allow) => {
          bubble.clearApproval();
          void invoke("decide", { id: pending.id, allow });
        });
      } else {
        bubble.clearApproval();
      }
      bubble.onState(snap.overall);
      idle.onState(snap.overall);
      life.setAttention(snap.sessions.some((s) => s.state === "waiting"));
      void scheduler.onSnapshot(snap);
      label.textContent = prefs.dnd ? `${describe(snap)} · ${t("dnd")}` : describe(snap);
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
