/** Mini mode: the pet parks at the right edge of the screen with only
 *  a sliver showing, slides out when the cursor reaches it or an agent
 *  needs the user, and tucks back in a moment after. */
import { invoke } from "@tauri-apps/api/core";
import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";

/** Mirrors `CursorRelative` in src-tauri/src/hit.rs. */
interface CursorRelative {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Share of the pet's width left on screen while tucked in. */
const PEEK = 0.4;
const POLL_MS = 150;
const LINGER_MS = 700;
const SLIDE_MS = 180;

export class Dock {
  private win = getCurrentWindow();
  private parked = false;
  /** Where the pet stood before parking, to go back to; physical px. */
  private home: { x: number; y: number } | null = null;
  private edge: { inX: number; outX: number; y: number } | null = null;
  private out = false;
  private leftAt = 0;
  private attention = false;
  private poller = 0;
  private sliding = false;

  isParked(): boolean {
    return this.parked;
  }

  /** Also called again after a size change, to recompute the edge. */
  async park() {
    const monitor = await currentMonitor();
    if (!monitor) return;
    const [pos, size] = await Promise.all([this.win.outerPosition(), this.win.outerSize()]);
    if (!this.parked) this.home = { x: pos.x, y: pos.y };
    const area = monitor.workArea;
    const right = area.position.x + area.size.width;
    const y = Math.max(area.position.y, Math.min(pos.y, area.position.y + area.size.height - size.height));
    this.edge = { inX: Math.round(right - size.width * PEEK), outX: right - size.width, y };
    this.parked = true;
    this.out = false;
    await this.slideTo(this.edge.inX, y);
    clearInterval(this.poller);
    this.poller = window.setInterval(() => void this.tick(), POLL_MS);
  }

  async leave() {
    if (!this.parked) return;
    this.parked = false;
    clearInterval(this.poller);
    const back = this.home ?? (this.edge ? { x: this.edge.outX, y: this.edge.y } : null);
    this.home = null;
    if (back) await this.slideTo(back.x, back.y);
  }

  /** An agent waiting for the user keeps the pet out. */
  setAttention(on: boolean) {
    this.attention = on;
  }

  private async tick() {
    if (!this.parked || !this.edge || this.sliding) return;
    const c = await invoke<CursorRelative | null>("cursor_relative");
    const over = !!c && c.x >= 0 && c.x < c.width && c.y >= 0 && c.y < c.height;
    const now = performance.now();
    if (over || this.attention) {
      this.leftAt = 0;
      if (!this.out) {
        this.out = true;
        await this.slideTo(this.edge.outX, this.edge.y);
      }
      return;
    }
    if (!this.out) return;
    if (this.leftAt === 0) this.leftAt = now;
    if (now - this.leftAt < LINGER_MS) return;
    this.out = false;
    this.leftAt = 0;
    await this.slideTo(this.edge.inX, this.edge.y);
  }

  private async slideTo(x: number, y: number) {
    this.sliding = true;
    const start = await this.win.outerPosition();
    const t0 = performance.now();
    await new Promise<void>((resolve) => {
      const frame = async () => {
        const k = Math.min(1, (performance.now() - t0) / SLIDE_MS);
        const ease = 1 - (1 - k) ** 3;
        await this.win.setPosition(
          new PhysicalPosition(Math.round(start.x + (x - start.x) * ease), Math.round(start.y + (y - start.y) * ease)),
        );
        if (k >= 1) resolve();
        else requestAnimationFrame(() => void frame());
      };
      void frame();
    });
    this.sliding = false;
  }
}
