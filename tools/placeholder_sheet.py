#!/usr/bin/env python3
"""Generate pets/placeholder/spritesheet.png: a Codex-layout sheet
(8 columns by 9 rows of 192 by 208 cells, six frames used per row) of a
round blob with a face, drawn with the standard library only so the
app needs no third-party art to run. Shapes are tested at 2x2 samples
per pixel and averaged, which gives smooth edges.

Run with `python3 -I tools/placeholder_sheet.py [out.png]`."""

import math
import struct
import sys
import zlib
from pathlib import Path

CELL_W, CELL_H, COLS, ROWS = 192, 208, 8, 9
W, H = CELL_W * COLS, CELL_H * ROWS
FRAMES = 6
SS = 2  # samples per pixel per axis

BODY = (120, 168, 250)
BODY_SAD = (150, 170, 205)
INK = (36, 38, 56)
WHITE = (255, 255, 255)
CHEEK = (255, 140, 160)
GROUND_Y = 196
R = 56


# --- shapes: each has a bounding box and an inside test --------------------

def ellipse(cx, cy, rx, ry):
    return ((cx - rx, cy - ry, cx + rx, cy + ry),
            lambda x, y: ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1)


def capsule(x1, y1, x2, y2, r):
    dx, dy = x2 - x1, y2 - y1
    ll = dx * dx + dy * dy or 1e-9

    def inside(x, y):
        t = max(0.0, min(1.0, ((x - x1) * dx + (y - y1) * dy) / ll))
        px, py = x1 + t * dx - x, y1 + t * dy - y
        return px * px + py * py <= r * r
    return ((min(x1, x2) - r, min(y1, y2) - r, max(x1, x2) + r, max(y1, y2) + r), inside)


def arc(cx, cy, radius, width, a0, a1):
    """A stroke along a circle between angles a0..a1 (radians, y down)."""
    def inside(x, y):
        d = math.hypot(x - cx, y - cy)
        if abs(d - radius) > width / 2:
            return False
        a = math.atan2(y - cy, x - cx)
        return a0 <= a <= a1 or a0 <= a + 2 * math.pi <= a1
    r = radius + width
    return ((cx - r, cy - r, cx + r, cy + r), inside)


def rrect(x0, y0, x1, y1, rad):
    def inside(x, y):
        qx = max(x0 + rad - x, 0, x - (x1 - rad))
        qy = max(y0 + rad - y, 0, y - (y1 - rad))
        return qx * qx + qy * qy <= rad * rad and x0 <= x <= x1 and y0 <= y <= y1
    return ((x0, y0, x1, y1), inside)


class Cell:
    """A cell's samples, painted shape by shape (source over)."""

    def __init__(self):
        self.w, self.h = CELL_W * SS, CELL_H * SS
        self.buf = [[0.0, 0.0, 0.0, 0.0] for _ in range(self.w * self.h)]

    def paint(self, shape, colour, alpha=1.0, clip=None):
        (x0, y0, x1, y1), inside = shape
        clip_inside = clip[1] if clip else None
        sx0, sy0 = max(0, int(x0 * SS)), max(0, int(y0 * SS))
        sx1, sy1 = min(self.w - 1, int(x1 * SS) + 1), min(self.h - 1, int(y1 * SS) + 1)
        r, g, b = colour
        for sy in range(sy0, sy1 + 1):
            y = (sy + 0.5) / SS
            row = sy * self.w
            for sx in range(sx0, sx1 + 1):
                x = (sx + 0.5) / SS
                if not inside(x, y) or (clip_inside and not clip_inside(x, y)):
                    continue
                p = self.buf[row + sx]
                k = 1 - alpha
                p[0] = r * alpha + p[0] * k
                p[1] = g * alpha + p[1] * k
                p[2] = b * alpha + p[2] * k
                p[3] = alpha + p[3] * k

    def write_into(self, px, ox, oy):
        n = SS * SS
        for y in range(CELL_H):
            for x in range(CELL_W):
                r = g = b = a = 0.0
                for dy in range(SS):
                    row = (y * SS + dy) * self.w
                    for dx in range(SS):
                        p = self.buf[row + x * SS + dx]
                        r += p[0] * p[3]
                        g += p[1] * p[3]
                        b += p[2] * p[3]
                        a += p[3]
                if a > 0:
                    px[oy + y][ox + x] = (round(r / a), round(g / a), round(b / a), round(255 * a / n))


# --- the blob ------------------------------------------------------------

def eyes(c, body, cx, cy, kind, gaze=(0, 0)):
    for side in (-1, 1):
        ex, ey = cx + side * 21, cy - 10
        if kind == "open":
            c.paint(ellipse(ex, ey, 10, 12), WHITE)
            gx, gy = gaze
            c.paint(ellipse(ex + gx * 4, ey + gy * 4 + 1, 6, 7.5), INK)
            c.paint(ellipse(ex + gx * 4 - 2, ey + gy * 4 - 2, 2.2, 2.2), WHITE)
        elif kind == "blink":
            c.paint(capsule(ex - 8, ey + 2, ex + 8, ey + 2, 2.2), INK)
        elif kind == "happy":
            c.paint(arc(ex, ey + 5, 8, 4.5, -math.pi + 0.35, -0.35), INK)
        elif kind == "squint":
            c.paint(ellipse(ex, ey + 2, 10, 7), WHITE)
            c.paint(ellipse(ex + gaze[0] * 4, ey + 3, 5.5, 5.5), INK)
            c.paint(capsule(ex - 11, ey - 3, ex + 11, ey - 3, 2.4), INK)
        elif kind == "x":
            c.paint(capsule(ex - 6, ey - 6, ex + 6, ey + 6, 2.4), INK)
            c.paint(capsule(ex - 6, ey + 6, ex + 6, ey - 6, 2.4), INK)


def mouth(c, cx, cy, kind):
    my = cy + 16
    if kind == "smile":
        c.paint(arc(cx, my - 6, 9, 3.6, 0.45, math.pi - 0.45), INK)
    elif kind == "open":
        c.paint(ellipse(cx, my + 1, 9, 7), INK)
        c.paint(ellipse(cx, my + 5, 5.5, 3), (240, 110, 120))
    elif kind == "o":
        c.paint(ellipse(cx, my + 1, 4.5, 5.5), INK)
    elif kind == "flat":
        c.paint(arc(cx, my + 12, 10, 3.6, -math.pi + 0.6, -0.6), INK)


def blob(c, cx, cy, rx, ry, colour, eye_kind, mouth_kind, gaze=(0, 0), cheeks=True):
    # Ground shadow shrinks as the body rises.
    lift = max(0.0, GROUND_Y - (cy + ry))
    s = max(0.45, 1 - lift / 90)
    c.paint(ellipse(cx, GROUND_Y + 2, 46 * s, 7 * s), INK, 0.16)
    body = ellipse(cx, cy, rx, ry)
    c.paint(ellipse(cx, cy, rx + 2.2, ry + 2.2), INK)
    c.paint(body, colour)
    shade = tuple(int(v * 0.82) for v in colour)
    c.paint(ellipse(cx + 6, cy + ry * 0.55, rx * 0.95, ry * 0.6), shade, 0.55, clip=body)
    c.paint(ellipse(cx - rx * 0.38, cy - ry * 0.5, rx * 0.32, ry * 0.2), WHITE, 0.55, clip=body)
    if cheeks:
        for side in (-1, 1):
            c.paint(ellipse(cx + side * 34, cy + 8, 8, 5), CHEEK, 0.45, clip=body)
    eyes(c, body, cx, cy, eye_kind, gaze)
    mouth(c, cx, cy, mouth_kind)
    return body


def question(c, x, y):
    c.paint(rrect(x - 17, y - 20, x + 17, y + 18, 12), INK)
    c.paint(rrect(x - 15, y - 18, x + 15, y + 16, 10), WHITE)
    c.paint(arc(x, y - 4, 7, 4, -math.pi, 0.9), INK)
    c.paint(capsule(x + 4.5, y + 1, x, y + 5, 2), INK)
    c.paint(ellipse(x, y + 11, 2.6, 2.6), INK)


def draw(row, f):
    c = Cell()
    t = f / FRAMES
    wave = math.sin(t * 2 * math.pi)
    cx, base = CELL_W / 2, GROUND_Y - R

    if row == 0:  # idle: a slow bob, a blink, eyes wandering
        cy = base - 3 - 3 * wave
        gaze = [(0, 0), (-1, 0), (-1, 0.2), (0, 0), (0, 0), (1, 0)][f]
        blob(c, cx, cy, R * (1 + 0.02 * wave), R * (1 - 0.02 * wave), BODY,
             "blink" if f == 4 else "open", "smile", gaze)
    elif row in (1, 2):  # running: hop along, lean, feet and speed lines
        d = 1 if row == 1 else -1
        hop = abs(math.sin(t * 2 * math.pi)) * 14
        cy = base - hop
        for i, phase in enumerate((0, math.pi)):
            fx = cx + d * 14 * math.sin(t * 2 * math.pi + phase) + (i * 2 - 1) * 16
            c.paint(ellipse(fx, GROUND_Y - 4 - max(0, math.sin(t * 2 * math.pi + phase)) * 6, 11, 6), INK)
        for k, ly in enumerate((cy - 18, cy, cy + 18)):
            lx = cx - d * (R + 14 + k * 4)
            c.paint(capsule(lx, ly, lx - d * (14 - k * 3), ly, 2), INK, 0.35)
        blob(c, cx + d * 6, cy, R * 1.04, R * 0.97, BODY, "open", "open", (d, 0))
    elif row == 3:  # waving: happy eyes and a little arm going back and forth
        cy = base - 2
        ang = -1.1 + 0.55 * wave
        hx, hy = cx + R * 0.82, cy - 4
        c.paint(capsule(hx, hy, hx + 30 * math.cos(ang), hy + 30 * math.sin(ang), 9.5), INK)
        c.paint(capsule(hx, hy, hx + 30 * math.cos(ang), hy + 30 * math.sin(ang), 7.3), BODY)
        blob(c, cx - 4, cy, R, R, BODY, "happy", "open")
    elif row == 4:  # jumping: crouch, stretch, up, hang, fall, squash
        pose = [(0, 1.18, 0.80), (34, 0.86, 1.18), (60, 0.95, 1.05),
                (66, 1.0, 1.0), (40, 0.9, 1.12), (0, 1.2, 0.8)][f]
        lift, sx, sy = pose
        ry = R * sy
        cy = GROUND_Y - ry - lift
        happy = f in (2, 3)
        blob(c, cx, cy, R * sx, ry, BODY, "happy" if happy else "open",
             "open" if happy else "o", (0, -1 if f in (1, 2) else 0.5))
    elif row == 5:  # failed: slumped, crossed eyes, a sweat drop
        sag = 0.84 + 0.02 * wave
        ry = R * sag
        cy = GROUND_Y - ry
        blob(c, cx, cy, R * 1.1, ry, BODY_SAD, "x", "flat", cheeks=False)
        dy = 4 * t
        c.paint(ellipse(cx + R * 0.85, cy - ry * 0.55 + dy * 10, 6, 8), (150, 205, 255))
        c.paint(capsule(cx + R * 0.85, cy - ry * 0.55 - 9 + dy * 10, cx + R * 0.85, cy - ry * 0.55 - 3 + dy * 10, 3.2), (150, 205, 255))
    elif row == 6:  # waiting: looking up at a question mark that bobs
        cy = base - 2 - 2 * wave
        blob(c, cx, cy, R, R, BODY, "open", "o", (0.3, -1))
        question(c, cx + 34, cy - R - 26 + 4 * math.sin(t * 2 * math.pi + 1))
    elif row == 7:  # thinking: eyes up, three dots circling above
        cy = base - 2 - 2 * wave
        blob(c, cx, cy, R, R, BODY, "open", "o", (-0.6, -1))
        for k in range(3):
            a = t * 2 * math.pi + k * 2 * math.pi / 3
            c.paint(ellipse(cx + 26 * math.cos(a), cy - R - 22 + 8 * math.sin(a), 5.5, 5.5), INK, 0.4 + 0.2 * k)
    elif row == 8:  # review: squinting, eyes scanning, holding a lens
        cy = base - 2
        g = math.sin(t * 2 * math.pi) * 0.9
        blob(c, cx - 6, cy, R, R, BODY, "squint", "smile", (g, 0))
        lx, ly = cx + R * 0.7 + g * 6, cy + 14
        c.paint(capsule(lx + 10, ly + 12, lx + 26, ly + 30, 5), INK)
        c.paint(ellipse(lx, ly, 17, 17), INK)
        c.paint(ellipse(lx, ly, 13, 13), (210, 235, 255))
        c.paint(ellipse(lx - 4, ly - 4, 4, 4), WHITE, 0.8)
    return c


def png(path, px):
    raw = b"".join(b"\x00" + bytes(c for p in row for c in p) for row in px)

    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    ihdr = struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
                     + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("pets/placeholder/spritesheet.png")
    px = [[(0, 0, 0, 0)] * W for _ in range(H)]
    for row in range(ROWS):
        for frame in range(FRAMES):
            draw(row, frame).write_into(px, frame * CELL_W, row * CELL_H)
    png(out, px)
    print(f"wrote {out} {W}x{H}")


if __name__ == "__main__":
    main()
