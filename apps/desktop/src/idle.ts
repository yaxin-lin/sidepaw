import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import type { State } from "./generated/State";
import { LOOP_MS, ROW } from "./pack";
import type { SpriteRenderer } from "./renderer";

/** Idle flourishes within what the Codex sheet can show: short walks
 *  on the running rows, a slower dimmer loop while asleep, a hop on
 *  click. Anything the agents do cancels a walk at once. */
export class IdleBehaviour {
  private walking = false;
  private nextWalkAt = 0;
  private state: State["state"] = "idle";

  constructor(
    private renderer: SpriteRenderer,
    private canvas: HTMLElement,
    private random: () => number = Math.random,
  ) {
    this.scheduleWalk();
  }

  onState(state: State) {
    this.state = state.state;
    const dozing = state.state === "sleeping";
    this.renderer.speed = dozing ? 0.5 : 1;
    this.canvas.style.opacity = dozing ? "0.75" : "1";
    if (state.state !== "idle" && state.state !== "sleeping") this.walking = false;
  }

  onClick() {
    if (this.walking) return;
    this.renderer.play(ROW.jumping, LOOP_MS);
  }

  /** Call often; it decides on its own when to wander. */
  async tick(now = Date.now()) {
    if (this.walking || now < this.nextWalkAt || this.state !== "idle") return;
    this.scheduleWalk(now);
    await this.walk();
  }

  private scheduleWalk(now = Date.now()) {
    this.nextWalkAt = now + 20_000 + this.random() * 40_000;
  }

  private async walk() {
    const win = getCurrentWindow();
    const monitor = await currentMonitor();
    if (!monitor) return;
    const scale = monitor.scaleFactor;
    const start = await win.outerPosition();
    const size = await win.outerSize();
    const minX = monitor.workArea.position.x;
    const maxX = monitor.workArea.position.x + monitor.workArea.size.width - size.width;

    let dir = this.random() < 0.5 ? -1 : 1;
    const distance = (40 + this.random() * 80) * scale;
    // Turn around rather than walk off the screen.
    if (start.x + dir * distance < minX || start.x + dir * distance > maxX) dir = -dir;
    const target = Math.max(minX, Math.min(maxX, start.x + dir * distance));
    const speed = 60 * scale; // physical px per second
    const duration = (Math.abs(target - start.x) / speed) * 1000;
    if (duration < 200) return;

    this.walking = true;
    this.renderer.play(dir < 0 ? ROW.runningLeft : ROW.runningRight, duration);
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
    this.walking = false;
  }
}
