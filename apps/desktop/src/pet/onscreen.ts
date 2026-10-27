/** The pet must never end up where nobody can see it. Whatever moved it
 *  (a window it rode, the dock, a display that went away), a check
 *  every few seconds brings it back to the bottom right corner of a
 *  screen when too little of it is visible, and logs where it was so
 *  the cause can be found. */
import { availableMonitors, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Share of the pet that has to be on a screen. Mini mode leaves 40 %
 *  of its width showing at the right edge on purpose. */
const MIN_VISIBLE = 0.3;
const CHECK_MS = 3000;

function overlap(a: Box, b: Box): number {
  const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
  return w > 0 && h > 0 ? (w * h) / (a.width * a.height) : 0;
}

/** Where to put the pet when it is lost, or null when enough of it is
 *  on one of `screens` (work areas, same pixel space as `pet`). */
export function rescue(pet: Box, screens: Box[]): { x: number; y: number } | null {
  if (screens.length === 0) return null;
  if (screens.some((s) => overlap(pet, s) >= MIN_VISIBLE)) return null;
  // The screen nearest to where it was, so it reappears near its home.
  const cx = pet.x + pet.width / 2;
  const cy = pet.y + pet.height / 2;
  const dist = (s: Box) => Math.hypot(Math.max(s.x - cx, 0, cx - (s.x + s.width)), Math.max(s.y - cy, 0, cy - (s.y + s.height)));
  const s = [...screens].sort((a, b) => dist(a) - dist(b))[0];
  return { x: Math.round(s.x + s.width - pet.width - 24), y: Math.round(s.y + s.height - pet.height) };
}

export async function bringBackIfLost(): Promise<boolean> {
  const win = getCurrentWindow();
  const [pos, size, monitors] = await Promise.all([win.outerPosition(), win.outerSize(), availableMonitors()]);
  const pet = { x: pos.x, y: pos.y, width: size.width, height: size.height };
  const screens = monitors.map((m) => ({ x: m.workArea.position.x, y: m.workArea.position.y, width: m.workArea.size.width, height: m.workArea.size.height }));
  const to = rescue(pet, screens);
  if (!to) return false;
  console.error(`pet was off screen at ${pet.x},${pet.y} (${pet.width}x${pet.height}); moved to ${to.x},${to.y}`);
  await win.setPosition(new PhysicalPosition(to.x, to.y));
  return true;
}

export function keepOnScreen(onRescue: () => void = () => {}) {
  const check = () =>
    void bringBackIfLost().then(
      (moved) => moved && onRescue(),
      () => {},
    );
  check();
  setInterval(check, CHECK_MS);
}
