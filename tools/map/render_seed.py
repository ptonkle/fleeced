#!/usr/bin/env python3
"""Draw a top-down picture of a map seed, from the core's own JSON.

    python3 tools/map/render_seed.py 1 2 3 --out notes/map-seeds
    python3 tools/map/render_seed.py 7 --sheep 400 --out notes/map-seeds

The core decides everything drawn here: this script asks the `map_json`
example for a seed and only paints what comes back. It writes a PNG with the
standard library alone, so it needs nothing installed, and the picture can
be opened by anything. x runs left to right, up the hill; z runs up the page.
"""

import argparse
import json
import pathlib
import struct
import subprocess
import zlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCALE = 4
MARGIN = 10
BAND = 92
FX = 65536.0

GROUND_LOW = (96, 142, 74)
GROUND_HIGH = (182, 196, 112)
OUTSIDE = (120, 130, 118)
FOLD_TINT = (214, 186, 132)
OUTER_WALL = (58, 56, 54)
INNER_WALL = (112, 110, 104)
FOLD_WALL = (92, 62, 40)
GAP = (246, 214, 60)
FOLD_GAP = (240, 130, 40)
CROWN = (36, 84, 44)
TRUNK = (84, 56, 34)
CAIRN = (150, 148, 142)
CAIRN_EDGE = (72, 70, 68)
MOUND = (120, 78, 40)
SHEEP = (252, 250, 240)
SHEEP_EDGE = (50, 50, 50)
SHEEPDOG = (40, 90, 200)
TUNNELS = [(214, 40, 120), (120, 40, 200), (20, 150, 170), (220, 90, 20)]
INK = (24, 24, 24)
PAPER = (240, 238, 228)

# The classic 5x7 font, column by column, from space to Z.
FONT = bytes.fromhex(
    "0000000000" "00005f0000" "0007000700" "147f147f14" "242a7f2a12" "2313086462"
    "3649552250" "0005030000" "001c224100" "0041221c00" "14083e0814" "08083e0808"
    "0050300000" "0808080808" "0060600000" "2010080402" "3e5149453e" "00427f4000"
    "4261514946" "2141454b31" "1814127f10" "2745454539" "3c4a494930" "0171090503"
    "3649494936" "064949291e" "0036360000" "0056360000" "0814224100" "1414141414"
    "0041221408" "0201510906" "324979413e" "7e1111117e" "7f49494936" "3e41414122"
    "7f4141221c" "7f49494941" "7f09090901" "3e4149497a" "7f0808087f" "00417f4100"
    "2040413f01" "7f08142241" "7f40404040" "7f020c027f" "7f0408107f" "3e4141413e"
    "7f09090906" "3e4151215e" "7f09192946" "4649494931" "01017f0101" "3f4040403f"
    "1f2040201f" "3f4038403f" "6314081463" "0708700807" "6151494543"
)


class Canvas:
    def __init__(self, width, height, fill):
        self.w, self.h = width, height
        self.px = bytearray(bytes(fill) * (width * height))

    def set(self, x, y, color, alpha=1.0):
        if 0 <= x < self.w and 0 <= y < self.h:
            i = (y * self.w + x) * 3
            if alpha >= 1.0:
                self.px[i:i + 3] = bytes(color)
            else:
                for c in range(3):
                    self.px[i + c] = int(self.px[i + c] * (1 - alpha) + color[c] * alpha)

    def disc(self, cx, cy, r, color, alpha=1.0):
        r2 = r * r
        for y in range(int(cy - r), int(cy + r) + 2):
            for x in range(int(cx - r), int(cx + r) + 2):
                if (x - cx) ** 2 + (y - cy) ** 2 <= r2:
                    self.set(x, y, color, alpha)

    def ring(self, cx, cy, r, width, color):
        outer, inner = r * r, max(r - width, 0) ** 2
        for y in range(int(cy - r), int(cy + r) + 2):
            for x in range(int(cx - r), int(cx + r) + 2):
                d = (x - cx) ** 2 + (y - cy) ** 2
                if inner <= d <= outer:
                    self.set(x, y, color)

    def line(self, x0, y0, x1, y1, width, color, dash=0):
        length = max(((x1 - x0) ** 2 + (y1 - y0) ** 2) ** 0.5, 1)
        steps = int(length * 2) + 1
        for i in range(steps + 1):
            if dash and int(i / 2 / dash) % 2:
                continue
            t = i / steps
            self.disc(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, width / 2, color)

    def text(self, x, y, words, color, scale=2):
        for ch in words.upper():
            code = ord(ch) - 32
            if 0 <= code <= 58:
                for col in range(5):
                    bits = FONT[code * 5 + col]
                    for row in range(7):
                        if bits >> row & 1:
                            for sy in range(scale):
                                for sx in range(scale):
                                    self.set(x + col * scale + sx, y + row * scale + sy, color)
            x += 6 * scale

    def write(self, path):
        rows = (b"\0" + bytes(self.px[y * self.w * 3:(y + 1) * self.w * 3]) for y in range(self.h))
        raw = b"".join(rows)

        def chunk(kind, data):
            crc = zlib.crc32(kind + data)
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", crc)

        path.write_bytes(
            b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", self.w, self.h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9))
            + chunk(b"IEND", b"")
        )


def meters(value):
    return value / FX


def seed_json(seed, sheep):
    args = ["cargo", "run", "--quiet", "--release", "--offline", "-p", "tool_core", "--example", "map_json"]
    out = subprocess.run(args + ["--", str(seed), str(sheep)], cwd=ROOT, capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(out.stderr)
    return json.loads(out.stdout)


class View:
    """Map meters to picture pixels."""

    def __init__(self, length, width):
        self.width = int((meters(length) + 2 * MARGIN) * SCALE)
        self.height = int((meters(width) + 2 * MARGIN) * SCALE)

    def at(self, point):
        return self.x(point[0]), self.y(point[1])

    def x(self, value):
        return (meters(value) + MARGIN) * SCALE

    def y(self, value):
        return self.height - (meters(value) + MARGIN) * SCALE


def polygon_rows(points, height):
    """For each picture row, the x spans inside a polygon of pixel points."""
    rows = {}
    for y in range(height):
        crossings = []
        for (x0, y0), (x1, y1) in zip(points, points[1:] + points[:1]):
            if (y0 > y) != (y1 > y):
                crossings.append(x0 + (y - y0) * (x1 - x0) / (y1 - y0))
        crossings.sort()
        rows[y] = list(zip(crossings[::2], crossings[1::2]))
    return rows


def draw_ground(canvas, view, data):
    terrain = data["map"]["terrain"]
    cols, rows, samples = terrain["cols"], terrain["rows"], terrain["samples"]
    cell = (1 << terrain["cell_shift"]) / FX
    x0, z0 = meters(terrain["x0"]), meters(terrain["z0"])
    low, high = min(samples) / FX, max(samples) / FX
    span = max(high - low, 1)

    def height(x, z):
        u, v = (x - x0) / cell, (z - z0) / cell
        c, r = min(max(int(u), 0), cols - 2), min(max(int(v), 0), rows - 2)
        fu, fv = u - c, v - r
        a = samples[r * cols + c] * (1 - fu) + samples[r * cols + c + 1] * fu
        b = samples[(r + 1) * cols + c] * (1 - fu) + samples[(r + 1) * cols + c + 1] * fu
        return (a * (1 - fv) + b * fv) / FX

    outline = [view.at(p) for p in data["map"]["outer"]]
    inside = polygon_rows(outline, view.height)
    fold = polygon_rows([view.at(p) for p in data["map"]["fold"]["outline"]], view.height)
    # slopes over several meters, so the picture's coarse lookup shows no grid
    step = 3.0
    for py in range(view.height):
        z = (view.height - py) / SCALE - MARGIN
        spans, fold_spans = inside[py], fold[py]
        for px in range(view.width):
            x = px / SCALE - MARGIN
            h = height(x, z)
            t = (h - low) / span
            shade = 1 - (height(x + step, z) - height(x - step, z)) * 0.11
            shade += (height(x, z + step) - height(x, z - step)) * 0.11
            color = [GROUND_LOW[c] + (GROUND_HIGH[c] - GROUND_LOW[c]) * t for c in range(3)]
            color = [min(max(c * shade, 0), 255) for c in color]
            if int(h) != int(h + 0.05) and int(h) % 2 == 0:
                color = [c * 0.8 for c in color]
            if not any(a <= px <= b for a, b in spans):
                color = [(color[c] + OUTSIDE[c] * 3) / 4 for c in range(3)]
            elif any(a <= px <= b for a, b in fold_spans):
                color = [(color[c] + FOLD_TINT[c] * 2) / 3 for c in range(3)]
            canvas.set(px, py, [int(c) for c in color])


def draw_walls(canvas, view, data):
    colors = {"Outer": (OUTER_WALL, 8), "Inner": (INNER_WALL, 5), "Fold": (FOLD_WALL, 5)}
    for wall in data["map"]["walls"]:
        color, width = colors[wall["kind"]]
        canvas.line(*view.at(wall["a"]), *view.at(wall["b"]), width, color)
    for gap in data["map"]["gaps"]:
        canvas.line(*view.at(gap["a"]), *view.at(gap["b"]), 4, GAP)
        x, y = view.at(gap["center"])
        canvas.text(int(x) + 10, int(y) - 7, f"{meters(gap['width']):.1f}", INK)
    gap = data["map"]["fold"]["gap"]
    canvas.line(*view.at(gap["a"]), *view.at(gap["b"]), 6, FOLD_GAP)
    x, y = view.at(data["map"]["fold"]["center"])
    canvas.text(int(x) - 26, int(y) - 40, "FOLD", INK, 3)


def draw_props(canvas, view, data):
    game = data["map"]
    for tree in game["trees"]:
        x, y = view.x(tree["x"]), view.y(tree["z"])
        canvas.disc(x, y, meters(tree["crown"]) * SCALE, CROWN, 0.45)
    for tree in game["trees"]:
        x, y = view.x(tree["x"]), view.y(tree["z"])
        canvas.disc(x, y, meters(tree["trunk"]) * SCALE + 1, TRUNK)
    for cairn in game["cairns"]:
        x, y = view.x(cairn["x"]), view.y(cairn["z"])
        for tier in cairn["tiers"]:
            r = meters(tier["radius"]) * SCALE
            canvas.disc(x, y, r, CAIRN)
            canvas.ring(x, y, r, 1.5, CAIRN_EDGE)
    for number, tunnel in enumerate(game["tunnels"]):
        color = TUNNELS[number % len(TUNNELS)]
        canvas.line(*view.at(tunnel["a"]), *view.at(tunnel["b"]), 1.5, color, dash=4)
        for mouth in (tunnel["a"], tunnel["b"]):
            x, y = view.at(mouth)
            canvas.disc(x, y, 9, color)
            canvas.disc(x, y, 6, INK)
            canvas.text(int(x) + 11, int(y) - 7, "ABCDEFGH"[number], color)
    for mound in game["mounds"]:
        x, y = view.x(mound["x"]), view.y(mound["z"])
        canvas.disc(x, y, 6, MOUND)
    for number, mound in enumerate(game["spare_mounds"]):
        x, y = view.x(mound["x"]), view.y(mound["z"])
        canvas.ring(x, y, 7, 2.5, MOUND)
        canvas.text(int(x) + 9, int(y) - 3, str(number + 1), MOUND, 1)


def draw_starts(canvas, view, data):
    for spot in data["sheep"]:
        x, y = view.at(spot)
        canvas.disc(x, y, 3.2, SHEEP_EDGE)
        canvas.disc(x, y, 2.2, SHEEP)
    for spot in data["map"]["sheepdog_starts"]:
        x, y = view.at(spot)
        canvas.disc(x, y, 4, SHEEPDOG)


def draw_key(canvas, view, data, seed):
    y = view.height + 6
    game = data["map"]
    canvas.text(12, y, f"SEED {seed}", INK, 3)
    widths = [meters(g["width"]) for g in game["gaps"]]
    fold_gap = meters(game["fold"]["gap"]["width"])
    canvas.text(190, y + 2, f"ROUTE {meters(data['route_width']):.1f} M WIDE", INK)
    canvas.text(460, y + 2, f"GAPS {len(widths)}: {min(widths):.1f} TO {max(widths):.1f} M", INK)
    canvas.text(790, y + 2, f"FOLD GAP {fold_gap:.1f} M", INK)
    key_y = y + 30
    entries = [
        (OUTER_WALL, "OUTER WALL"),
        (INNER_WALL, "INNER WALL"),
        (GAP, "GAP"),
        (FOLD_GAP, "FOLD GAP"),
        (CAIRN, "CAIRN"),
        (CROWN, "TREE"),
    ]
    x = 14
    for color, label in entries:
        canvas.disc(x, key_y + 6, 7, color)
        canvas.text(x + 12, key_y, label, INK)
        x += 28 + 12 * len(label)
    key_y += 26
    x = 14
    canvas.disc(x, key_y + 6, 6, MOUND)
    canvas.text(x + 12, key_y, f"MOUND ({len(game['mounds'])})", INK)
    x += 150
    canvas.ring(x, key_y + 6, 7, 2.5, MOUND)
    canvas.text(x + 12, key_y, f"SPARE MOUND ({len(game['spare_mounds'])})", INK)
    x += 230
    canvas.disc(x, key_y + 6, 9, TUNNELS[0])
    canvas.disc(x, key_y + 6, 6, INK)
    canvas.text(x + 14, key_y, "MOUTH, SAME LETTER = SAME TUNNEL", INK)
    x += 410
    canvas.disc(x, key_y + 6, 3.2, SHEEP_EDGE)
    canvas.disc(x, key_y + 6, 2.2, SHEEP)
    canvas.text(x + 10, key_y, f"SHEEP ({len(data['sheep'])})", INK)
    x += 150
    canvas.disc(x, key_y + 6, 4, SHEEPDOG)
    canvas.text(x + 10, key_y, "SHEEPDOG", INK)


def render(seed, sheep, out_dir):
    data = seed_json(seed, sheep)
    view = View(data["length"], data["width"])
    canvas = Canvas(view.width, view.height + BAND, PAPER)
    draw_ground(canvas, view, data)
    draw_walls(canvas, view, data)
    draw_props(canvas, view, data)
    draw_starts(canvas, view, data)
    draw_key(canvas, view, data, seed)
    name = f"seed-{seed}.png" if sheep == 100 else f"seed-{seed}-{sheep}-sheep.png"
    path = out_dir / name
    canvas.write(path)
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("seeds", nargs="+", type=int)
    parser.add_argument("--sheep", type=int, default=100)
    parser.add_argument("--out", default="notes/map-seeds")
    args = parser.parse_args()
    out_dir = pathlib.Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    for seed in args.seeds:
        print(render(seed, args.sheep, out_dir))


if __name__ == "__main__":
    main()
