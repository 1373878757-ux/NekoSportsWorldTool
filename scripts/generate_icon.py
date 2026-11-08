#!/usr/bin/env python3
"""Deterministically draw the neutral sports/route icon; no third-party libraries."""

from pathlib import Path
import math
import struct
import zlib

OUTPUT = Path(__file__).resolve().parent.parent / "assets" / "branding"
POINTS = [(52, 173), (85, 119), (118, 149), (164, 79), (202, 79)]
BG = (11, 13, 18)
MUTED = (38, 42, 53)
ACCENT = (139, 158, 255)
WHITE = (245, 245, 247)


def segment_distance(x, y, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0, min(1, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)))
    return math.hypot(x - a[0] - t * dx, y - a[1] - t * dy)


def sample(x, y):
    # Rounded-square background and a quiet track outline.
    qx, qy = max(abs(x - 128) - 84, 0), max(abs(y - 128) - 84, 0)
    if math.hypot(qx, qy) > 28:
        return (0, 0, 0, 0)
    color = BG
    ring = math.hypot((x - 128) / 1.05, y - 128)
    if abs(ring - 84) < 3:
        color = MUTED
    if min(segment_distance(x, y, a, b) for a, b in zip(POINTS, POINTS[1:])) <= 9:
        color = ACCENT
    if math.hypot(x - 52, y - 173) <= 12:
        color = WHITE
    if math.hypot(x - 202, y - 79) <= 12:
        color = WHITE
    return (*color, 255)


def png(size):
    scanlines = bytearray()
    for row in range(size):
        scanlines.append(0)
        for col in range(size):
            samples = [sample((col + (sx + 0.5) / 2) * 256 / size,
                              (row + (sy + 0.5) / 2) * 256 / size)
                       for sy in range(2) for sx in range(2)]
            alpha = sum(p[3] for p in samples)
            rgb = [round(sum(p[c] * p[3] for p in samples) / alpha) if alpha else 0
                   for c in range(3)]
            scanlines.extend((*rgb, round(alpha / 4)))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(scanlines), 9)) + chunk(b"IEND", b""))


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    svg = '''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
<rect x="16" y="16" width="224" height="224" rx="28" fill="#0B0D12"/>
<ellipse cx="128" cy="128" rx="88.2" ry="84" fill="none" stroke="#262A35" stroke-width="6"/>
<polyline points="52,173 85,119 118,149 164,79 202,79" fill="none" stroke="#8B9EFF" stroke-width="18" stroke-linecap="round" stroke-linejoin="round"/>
<g fill="#F5F5F7"><circle cx="52" cy="173" r="12"/><circle cx="202" cy="79" r="12"/></g>
</svg>
'''
    (OUTPUT / "sports-route.svg").write_text(svg, encoding="utf-8")
    sizes = [16, 24, 32, 48, 64, 128, 256]
    images = [png(size) for size in sizes]
    (OUTPUT / "sports-route.png").write_bytes(images[-1])
    offset = 6 + 16 * len(sizes)
    entries = bytearray(struct.pack("<HHH", 0, 1, len(sizes)))
    for size, image in zip(sizes, images):
        entries.extend(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(image), offset))
        offset += len(image)
    (OUTPUT / "sports-route.ico").write_bytes(entries + b"".join(images))


if __name__ == "__main__":
    main()
