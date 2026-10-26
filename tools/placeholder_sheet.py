#!/usr/bin/env python3
"""Generate pets/placeholder/spritesheet.png: a Codex-layout sheet
(8 columns by 9 rows of 192 by 208 cells) drawn with the standard
library only, so the renderer can be tested without a third-party pack.
Run with `python3 -I tools/placeholder_sheet.py`."""

import math
import struct
import sys
import zlib
from pathlib import Path

CELL_W, CELL_H, COLS, ROWS = 192, 208, 8, 9
W, H = CELL_W * COLS, CELL_H * ROWS
FRAMES = 6

# One colour per row so the state change is visible even without motion.
ROW_COLOURS = [
    (120, 170, 255),  # idle
    (255, 170, 90),   # running-right
    (255, 170, 90),   # running-left
    (120, 220, 140),  # waving
    (120, 220, 140),  # jumping
    (240, 90, 90),    # failed
    (250, 210, 80),   # waiting
    (190, 140, 255),  # running (thinking)
    (90, 200, 220),   # review
]


def blob(px, row, frame):
    """Draw a round body with two eyes into cell (row, frame)."""
    colour = ROW_COLOURS[row]
    t = frame / FRAMES
    ox = frame * CELL_W
    oy = row * CELL_H
    bounce = int(8 * math.sin(t * 2 * math.pi))
    squash = 1.0 + 0.06 * math.sin(t * 2 * math.pi)
    cx, cy = CELL_W // 2, CELL_H // 2 + 20 - bounce
    rx, ry = int(62 * squash), int(62 / squash)
    if row in (1, 2):  # running: lean in the direction of travel
        cx += (10 if row == 1 else -10)
    if row == 4:  # jumping: higher arc
        cy -= 30 + bounce
    if row == 5:  # failed: slump
        cy += 15
        ry = int(ry * 0.85)
    for y in range(cy - ry - 2, cy + ry + 3):
        for x in range(cx - rx - 2, cx + rx + 3):
            if not (0 <= x < CELL_W and 0 <= y < CELL_H):
                continue
            d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2
            if d <= 1.0:
                px[oy + y][ox + x] = (*colour, 255)
            elif d <= 1.08:
                px[oy + y][ox + x] = (40, 40, 60, 255)
    eye_dy = -12
    blink = row == 0 and frame == 4
    for ex in (cx - 22, cx + 22):
        for y in range(cy + eye_dy - 9, cy + eye_dy + 10):
            for x in range(ex - 7, ex + 8):
                if blink and abs(y - (cy + eye_dy)) > 2:
                    continue
                if ((x - ex) / 7) ** 2 + ((y - cy - eye_dy) / 9) ** 2 <= 1:
                    px[oy + y][ox + x] = (30, 30, 40, 255)
    if row == 5:  # failed: a flat mouth
        for x in range(cx - 18, cx + 19):
            for y in range(cy + 22, cy + 26):
                px[oy + y][ox + x] = (30, 30, 40, 255)
    if row == 6:  # waiting: a question-mark dot above
        for y in range(cy - ry - 30, cy - ry - 18):
            for x in range(cx - 6, cx + 7):
                px[oy + y][ox + x] = (30, 30, 40, 255)


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
            blob(px, row, frame)
    png(out, px)
    print(f"wrote {out} {W}x{H}")


if __name__ == "__main__":
    main()
