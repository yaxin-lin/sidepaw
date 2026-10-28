import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import type { State } from "../generated/State";
import { LOOP_MS, ROW } from "../pack";
import type { SpriteRenderer } from "./renderer";

/** Idle flourishes within what the Codex sheet can show: short walks
 *  on the running rows, a slower dimmer loop while asleep or while the
 *  user is away, a wave when they come back, a hop on click. Anything
 *  the agents do cancels a walk at once. */
export class IdleBehaviour {
  private walking = false;
  private nextWalkAt = 0;
  private state: State["state"] = "idle";
  private dozing = false;
  /** Set while something else moves the window (a throw). */
  hold = false;
  /** Told when a walk starts (-1 left, 1 right) and ends (0). */
  onLean: (dir: number) => void = () => {};
  /** Physical x range the pet may walk in right now, when it stands
   *  on a window: walks stay on that window's visible edge. */
  limits: (() => { minX: number; maxX: number } | null) | null = null;

  constructor(
    private renderer: SpriteRenderer,
    private canvas: HTMLElement,
    private random: () => number = Math.random,
  ) {
    this.scheduleWalk();
  }

  /** `away`: no input for a while. A pet with nothing to watch nods
   *  off then, and waves when the user is back. */
  onState(state: State, away = false) {
    this.state = state.state;
    const resting = state.state === "idle" || state.state === "done";
    const dozing = state.state === "sleeping" || (away && resting);
    if (this.dozing && !dozing && !away && resting && !this.hold) this.renderer.play(ROW.waving, 2 * LOOP_MS);
    this.dozing = dozing;
    this.renderer.speed = dozing ? 0.5 : 1;
    this.canvas.style.opacity = dozing ? "0.75" : "1";
    if (state.state !== "idle" && state.state !== "sleeping") this.walking = false;
  }

  isDozing(): boolean {
    return this.dozing;
  }

  onClick() {
    if (this.walking || this.hold) return;
    this.renderer.play(ROW.jumping, LOOP_MS);
  }

  /** Awake with nothing to do: free to walk, hop and look around. */
  isIdle(): boolean {
    return this.state === "idle" && !this.dozing;
  }

  isWalking(): boolean {
    return this.walking;
  }

  /** Call often; it decides on its own when to wander. */
  async tick(now = Date.now()) {
    if (this.walking || this.hold || now < this.nextWalkAt || !this.isIdle()) return;
    this.scheduleWalk(now);
    await this.walk();
  }

  /** A quick step away from something, further and faster than a stroll. */
  async flee(dir: -1 | 1) {
    if (this.walking || this.hold || !this.isIdle()) return;
    await this.walk({ dir, distance: 120 + this.random() * 60, speed: 160 });
  }

  private scheduleWalk(now = Date.now()) {
    this.nextWalkAt = now + 20_000 + this.random() * 40_000;
  }

  /** Logical pixels and pixels per second; a stroll picks its own. */
  private async walk(opts: { dir?: -1 | 1; distance?: number; speed?: number } = {}) {
    const win = getCurrentWindow();
    const monitor = await currentMonitor();
    if (!monitor) return;
    const scale = monitor.scaleFactor;
    const start = await win.outerPosition();
    const size = await win.outerSize();
    const edge = this.limits?.() ?? null;
    const minX = Math.max(monitor.workArea.position.x, edge?.minX ?? -Infinity);
    const maxX = Math.min(monitor.workArea.position.x + monitor.workArea.size.width - size.width, edge?.maxX ?? Infinity);
    if (maxX - minX < 20 * scale) return;

    let dir: -1 | 1 = opts.dir ?? (this.random() < 0.5 ? -1 : 1);
    const distance = (opts.distance ?? 40 + this.random() * 80) * scale;
    // Turn around rather than walk off the screen.
    if (start.x + dir * distance < minX || start.x + dir * distance > maxX) dir = dir === 1 ? -1 : 1;
    const target = Math.max(minX, Math.min(maxX, start.x + dir * distance));
    const speed = (opts.speed ?? 60) * scale; // physical px per second
    const duration = (Math.abs(target - start.x) / speed) * 1000;
    if (duration < 200) return;

    this.walking = true;
    this.renderer.play(dir < 0 ? ROW.runningLeft : ROW.runningRight, duration);
    this.onLean(dir);
    const t0 = performance.now();
    await new Promise<void>((resolve) => {
      const step = async () => {
        if (!this.walking) return resolve();
        const k = Math.min(1, (performance.now() - t0) / duration);
        await win.setPosition(new PhysicalPosition(Math.round(start.x + (target - start.x) * k), start.y));
        if (k >= 1) resolve();
        else requestAnimationFrame(() => void step());
      };
      void step();
    });
    this.renderer.stopOverride();
    this.onLean(0);
    this.walking = false;
  }
}
