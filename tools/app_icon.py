#!/usr/bin/env python3
"""Generate a 1024 by 1024 PNG of the placeholder blob for
`pnpm tauri icon`. Standard library only."""

import struct
import sys
import zlib
from pathlib import Path

N = 1024


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("apps/desktop/app-icon.png")
    cx = cy = N // 2
    r = 400
    rows = []
    for y in range(N):
        row = bytearray(b"\x00")
        for x in range(N):
            d = ((x - cx) / r) ** 2 + ((y - cy - 30) / r) ** 2
            eye = min(((x - cx - 120) / 42) ** 2 + ((y - cy - 50) / 56) ** 2,
                      ((x - cx + 120) / 42) ** 2 + ((y - cy - 50) / 56) ** 2)
            if eye <= 1:
                row += bytes((30, 30, 40, 255))
            elif d <= 1:
                row += bytes((120, 170, 255, 255))
            elif d <= 1.06:
                row += bytes((40, 40, 60, 255))
            else:
                row += bytes((0, 0, 0, 0))
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    ihdr = struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0)
    out.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
                    + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
