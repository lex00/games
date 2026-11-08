"""Rivet Rodeo's pictures, drawn by code: paper cutouts in four colors.

stage.py calls write() to make templates/rivet-rodeo/tiles.txt (the
background's 256 tiles), sprites.txt (the sprites' 256) and
scene_sprites.txt (the sprites of the stage-clear scene's carry), each a text
picture of 128 by 128 pixels, one character a pixel, 0 to 3. Every shape
here is drawn for this game; the sprite tiles sit at the numbers Donkey
Kong's model draws each body with (templates/donkey-kong-oe/model.toml),
so the model needs no change to show them.
"""

import os
import tomllib

W = 128


class Sheet:
    def __init__(self):
        self.px = [[0] * W for _ in range(W)]

    def tile(self, n, rows):
        """Puts 8 rows of 8 color numbers at tile n."""
        tx, ty = (n % 16) * 8, (n // 16) * 8
        for y, row in enumerate(rows):
            for x, c in enumerate(row):
                self.px[ty + y][tx + x] = c

    def from_canvas(self, canvas, tiles, width):
        """Cuts a canvas of width*8 columns into tiles, row by row."""
        for i, n in enumerate(tiles):
            cx, cy = (i % width) * 8, (i // width) * 8
            self.tile(n, [canvas[cy + y][cx:cx + 8] for y in range(8)])

    def text(self):
        return "".join("".join(str(c) for c in row) + "\n" for row in self.px)


def canvas(w, h):
    return [[0] * w for _ in range(h)]


def rect(c, x0, y0, x1, y1, color):
    for y in range(max(0, y0), min(len(c), y1)):
        for x in range(max(0, x0), min(len(c[0]), x1)):
            c[y][x] = color


def disc(c, cx, cy, r, color):
    for y in range(len(c)):
        for x in range(len(c[0])):
            if (x - cx + 0.5) ** 2 + (y - cy + 0.5) ** 2 <= r * r:
                c[y][x] = color


def outline(c, color=1):
    """A dark edge around every filled pixel: the cut paper's border."""
    h, w = len(c), len(c[0])
    out = [row[:] for row in c]
    for y in range(h):
        for x in range(w):
            if c[y][x]:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                if 0 <= x + dx < w and 0 <= y + dy < h and c[y + dy][x + dx] not in (0, color):
                    out[y][x] = color
                    break
    return out


def mirror(c):
    return [row[::-1] for row in c]


# A 5 by 7 font drawn for this game, in rows of 5 bits.
FONT = {
    "0": "01110 10011 10101 10101 11001 10001 01110", "1": "00100 01100 00100 00100 00100 00100 01110",
    "2": "01110 10001 00001 00110 01000 10000 11111", "3": "11110 00001 00001 01110 00001 00001 11110",
    "4": "00010 00110 01010 10010 11111 00010 00010", "5": "11111 10000 11110 00001 00001 10001 01110",
    "6": "00110 01000 10000 11110 10001 10001 01110", "7": "11111 00001 00010 00100 01000 01000 01000",
    "8": "01110 10001 10001 01110 10001 10001 01110", "9": "01110 10001 10001 01111 00001 00010 01100",
    "A": "01110 10001 10001 11111 10001 10001 10001", "B": "11110 10001 10001 11110 10001 10001 11110",
    "C": "01111 10000 10000 10000 10000 10000 01111", "D": "11110 10001 10001 10001 10001 10001 11110",
    "E": "11111 10000 10000 11110 10000 10000 11111", "F": "11111 10000 10000 11110 10000 10000 10000",
    "G": "01111 10000 10000 10011 10001 10001 01111", "H": "10001 10001 10001 11111 10001 10001 10001",
    "I": "01110 00100 00100 00100 00100 00100 01110", "J": "00111 00010 00010 00010 00010 10010 01100",
    "K": "10001 10010 10100 11000 10100 10010 10001", "L": "10000 10000 10000 10000 10000 10000 11111",
    "M": "10001 11011 10101 10101 10001 10001 10001", "N": "10001 11001 10101 10011 10001 10001 10001",
    "O": "01110 10001 10001 10001 10001 10001 01110", "P": "11110 10001 10001 11110 10000 10000 10000",
    "Q": "01110 10001 10001 10001 10101 10010 01101", "R": "11110 10001 10001 11110 10100 10010 10001",
    "S": "01111 10000 10000 01110 00001 00001 11110", "T": "11111 00100 00100 00100 00100 00100 00100",
    "U": "10001 10001 10001 10001 10001 10001 01110", "V": "10001 10001 10001 10001 10001 01010 00100",
    "W": "10001 10001 10001 10101 10101 10101 01010", "X": "10001 10001 01010 00100 01010 10001 10001",
    "Y": "10001 10001 01010 00100 00100 00100 00100", "Z": "11111 00001 00010 00100 01000 10000 11111",
}


def glyph(ch, color=1):
    rows = [[0] * 8 for _ in range(8)]
    for y, bits in enumerate(FONT[ch].split()):
        for x, b in enumerate(bits):
            if b == "1":
                rows[y][x + 1] = color
    return rows


def girder_rows():
    """The girder's 8 rows: a light top edge, a dark line, the beam with its
    cross braces, a dark line, a cut gap."""
    rows = []
    rows.append([3] * 8)
    rows.append([1] * 8)
    for y in range(4):
        rows.append([1 if x in (y * 2 % 8, (7 - y * 2) % 8) else 2 for x in range(8)])
    rows.append([1] * 8)
    rows.append([0] * 8)
    return rows


LADDER_ROWS = [[0, 3, 0, 0, 0, 0, 3, 0] if y % 4 else [0, 3, 2, 2, 2, 2, 3, 0] for y in range(8)]


def girder_tile(k, part, ladder):
    """The cell of a girder k pixels down: its upper part (`U`) or lower
    (`L`), with a ladder above or below it."""
    g = girder_rows()
    rows = [[0] * 8 for _ in range(8)]
    for y in range(8):
        gy = y - k if part == "U" else y + 8 - k
        if 0 <= gy < 8:
            rows[y] = g[gy][:]
        elif ladder:
            rows[y] = LADDER_ROWS[y][:]
    return rows


def background(kong_first):
    s = Sheet()
    for ch in "0123456789":
        s.tile(int(ch), glyph(ch))
    for i, ch in enumerate("ABCDEFGHIJKLMNOPQRSTUVWXYZ"):
        s.tile(0x0A + i, glyph(ch))
    # The turn screen's player numbers, which the model writes as `|` and
    # `#` (PLAYER | and PLAYER #): drawn as 1 and 2, after the thrower's
    # poses (stage.py's charset names them).
    s.tile(0xF0, glyph("1"))
    s.tile(0xF1, glyph("2"))
    # Girders, numbered so Donkey Kong's model's tile kinds and surfaces
    # read them (stage.py): U_0 48, U_k 63 - k, L_k 48 + k, the ladder 63,
    # U_k with a ladder above 64 to 69 and 76, L_k with one below 70 to 75
    # and 77.
    s.tile(48, girder_tile(0, "U", False))
    for k in range(1, 8):
        s.tile(63 - k, girder_tile(k, "U", False))
        s.tile(48 + k, girder_tile(k, "L", False))
    s.tile(63, [r[:] for r in LADDER_ROWS])
    for k, n in zip((7, 5, 4, 3, 2, 1, 6), (64, 65, 66, 67, 68, 69, 76)):
        s.tile(n, girder_tile(k, "U", True))
    for k, n in zip((1, 2, 3, 4, 5, 6, 7), (70, 71, 72, 73, 74, 75, 77)):
        s.tile(n, girder_tile(k, "L", True))
    # The oil drum, 16 by 16 at 0x4E.
    d = canvas(16, 16)
    rect(d, 2, 1, 14, 16, 2)
    for y in (3, 8, 13):
        rect(d, 2, y, 14, y + 1, 1)
    rect(d, 4, 5, 7, 7, 3)
    s.from_canvas(outline(d), [0x4E, 0x4F, 0x50, 0x51], 2)
    # The barrel pile, 16 by 32 at 0x52.
    p = canvas(16, 32)
    for i in range(4):
        rect(p, 1, i * 8 + 1, 15, i * 8 + 7, 2)
        rect(p, 1, i * 8 + 3, 15, i * 8 + 4, 3)
    s.from_canvas(outline(p), list(range(0x52, 0x5A)), 2)
    # The title's block and the HUD's line.
    s.tile(0x5A, [[1] * 8] + [[1, 2, 2, 2, 2, 2, 3, 1] for _ in range(6)] + [[1] * 8])
    line = [[0] * 8 for _ in range(8)]
    line[3] = [2] * 8
    s.tile(0x5B, line)
    s.tile(0x5C, line)
    s.tile(0x5D, line)
    for name, tiles in kong_poses(kong_first).items():
        c = thrower(name)
        cols = []
        for col in range(6):
            for row in range(4):
                cols.append((col, row))
        for (col, row), n in zip(cols, tiles):
            s.tile(n, [c[row * 8 + y][col * 8:col * 8 + 8] for y in range(8)])
    for n, rows in fallen_poses()[1].items():
        s.tile(n, rows)
    return s


# The background tiles no glyph, girder, drum, pile, HUD piece or standing
# pose of the thrower draws: the fallen thrower's pictures go here and
# nowhere else (gp template font lists a bank's free tiles). 0xF0 and 0xF1
# are the turn screen's `|` and `#`.
FALLEN_TILES = list(range(0xF2, 0x100)) + list(range(0x25, 0x30))
FALLEN = {"fallen_a": "stand", "fallen_b": "beat_b"}


def fallen_poses():
    """The thrower toppled onto its mast (the base's kong_fallen_a and
    kong_fallen_b, 100m's ending, which this stage never plays): thrower()
    upside down, arms by pose. Returns each pose's 24 tile numbers, a column
    of four at a time, and the pictures by tile number; cells the two poses
    share, and the empty ones, share a tile, so both fit FALLEN_TILES."""
    free = iter(FALLEN_TILES)
    by_rows, poses, pics = {}, {}, {}
    for name, pose in FALLEN.items():
        c = thrower(pose)[::-1]
        tiles = []
        for col in range(6):
            for row in range(4):
                rows = tuple(tuple(c[row * 8 + y][col * 8:col * 8 + 8]) for y in range(8))
                if rows not in by_rows:
                    n = next(free)
                    by_rows[rows] = n
                    pics[n] = [list(r) for r in rows]
                tiles.append(by_rows[rows])
        poses[name] = tiles
    return poses, pics


POSES = ["stand", "take", "hold", "throw", "beat_a", "beat_b"]


def kong_poses(first):
    """Each pose's 24 tile numbers, a column of four at a time."""
    return {p: [first + 24 * i + j for j in range(24)] for i, p in enumerate(POSES)}


def thrower(pose):
    """The barrel thrower, a boxy crane robot 48 by 32: a cab with two round
    lamps, treads, and two arms that move by pose."""
    c = canvas(48, 32)
    rect(c, 14, 6, 34, 26, 2)  # the cab
    rect(c, 16, 8, 32, 14, 3)  # its window
    disc(c, 20, 11, 2, 1)
    disc(c, 28, 11, 2, 1)
    rect(c, 18, 18, 30, 20, 1)  # the grille
    rect(c, 12, 26, 36, 31, 1)  # the treads
    for x in range(13, 36, 4):
        rect(c, x, 27, x + 2, 30, 3)
    rect(c, 22, 2, 26, 6, 2)  # the mast
    disc(c, 24, 2, 2, 3)
    barrel = None
    if pose == "stand":
        arms = [(6, 14, 14, 18), (34, 14, 42, 18)]
        hands = [(4, 18, 9, 24), (39, 18, 44, 24)]
    elif pose == "take":
        arms = [(4, 16, 14, 20), (34, 14, 42, 18)]
        hands = [(0, 18, 6, 24), (39, 18, 44, 24)]
        barrel = (0, 22, 10, 30)
    elif pose == "hold":
        arms = [(10, 2, 14, 12), (34, 2, 38, 12)]
        hands = [(8, 0, 14, 4), (34, 0, 40, 4)]
        barrel = (14, 0, 34, 6)
    elif pose == "throw":
        arms = [(6, 14, 14, 18), (34, 16, 46, 20)]
        hands = [(4, 18, 9, 24), (42, 18, 48, 24)]
        barrel = (38, 22, 48, 30)
    elif pose == "beat_a":
        arms = [(8, 4, 14, 10), (34, 14, 42, 18)]
        hands = [(6, 0, 12, 6), (39, 18, 44, 24)]
    elif pose == "fall":
        arms = [(8, 4, 14, 10), (34, 4, 40, 10)]
        hands = [(6, 0, 12, 6), (36, 0, 42, 6)]
    else:
        arms = [(6, 14, 14, 18), (34, 4, 40, 10)]
        hands = [(4, 18, 9, 24), (36, 0, 42, 6)]
    for a in arms:
        rect(c, *a, 2)
    for h in hands:
        rect(c, *h, 3)
    if barrel:
        x0, y0, x1, y1 = barrel
        rect(c, x0, y0, x1, y1, 2)
        rect(c, x0, (y0 + y1) // 2, x1, (y0 + y1) // 2 + 1, 3)
    return outline(c)


# --- Sprites ----------------------------------------------------------------


def worker(kind, frame):
    """The player, 16 by 16: a climber in a hard hat. `kind` is walk, climb,
    jump, swing or spin."""
    c = canvas(16, 16)
    rect(c, 5, 1, 11, 3, 3)  # the hat
    rect(c, 4, 3, 12, 4, 3)
    rect(c, 5, 4, 11, 8, 2)  # the face
    rect(c, 9, 5, 10, 6, 1)  # an eye
    rect(c, 4, 8, 12, 12, 1)  # the overalls
    rect(c, 6, 8, 10, 10, 3)
    step = frame % 3
    if kind == "climb":
        rect(c, 3, 4 + step, 5, 9, 2)
        rect(c, 11, 6 - step, 13, 9, 2)
        rect(c, 5, 12, 7, 16 - step, 1)
        rect(c, 9, 12, 11, 14 + step, 1)
        return outline(c)
    if kind == "swing":
        rect(c, 11, 2 + frame % 2 * 6, 15, 4 + frame % 2 * 6, 2)
    else:
        rect(c, 11, 8, 13, 11, 2)
    if kind == "jump":
        rect(c, 4, 12, 7, 14, 1)
        rect(c, 9, 12, 12, 15, 1)
    elif step == 0:
        rect(c, 5, 12, 7, 16, 1)
        rect(c, 9, 12, 11, 16, 1)
    elif step == 1:
        rect(c, 3, 12, 6, 15, 1)
        rect(c, 10, 12, 13, 16, 1)
    else:
        rect(c, 6, 12, 10, 16, 1)
    if kind == "spin":
        c = [list(r) for r in zip(*c[::-1])] if frame % 2 else c
    return outline(c)


def barrel_pic(frame, down):
    c = canvas(16, 16)
    if down:
        rect(c, 1, 3, 15, 13, 2)
        rect(c, 1, 7 + frame % 2, 15, 8 + frame % 2, 3)
    else:
        disc(c, 8, 8, 7, 2)
        a = frame % 4
        rect(c, 2 + a * 3, 3, 4 + a * 3, 13, 3)
    return outline(c)


def fire_pic(frame):
    c = canvas(16, 16)
    disc(c, 8, 10, 6, 2)
    rect(c, 5 + frame % 2, 2, 8 + frame % 2, 8, 3)
    rect(c, 9 - frame % 2, 4, 12 - frame % 2, 8, 3)
    rect(c, 5, 9, 7, 11, 1)
    rect(c, 9, 9, 11, 11, 1)
    return outline(c)


def heart_pic():
    """The heart over the climber and the friend in 100m's ending, 16 by
    16: two round lobes over a point, a light fold on the left lobe."""
    c = canvas(16, 16)
    disc(c, 5, 6, 4, 2)
    disc(c, 11, 6, 4, 2)
    for y in range(6, 15):
        half = (15 - y) * 7 // 9
        rect(c, 8 - half, y, 8 + half, y + 1, 2)
    rect(c, 3, 4, 5, 6, 3)
    return outline(c)


def friend_pic(calling):
    c = canvas(16, 24)
    disc(c, 8, 4, 4, 2)
    rect(c, 4, 0, 12, 2, 3)
    rect(c, 4, 8, 12, 20, 3)
    rect(c, 5, 20, 7, 24, 1)
    rect(c, 9, 20, 11, 24, 1)
    if calling:
        rect(c, 12, 6, 16, 8, 2)
    return outline(c)


# The frame number each sprite picture is drawn as (worker's, barrel_pic's
# and fire_pic's `frame`, which picks its step), by the picture's first
# tile. Until 2026-11-07 it was the frame's place in the base's lists, so
# a frame the base gained moved every picture after it; these are the
# numbers the pictures had then, kept. The fires' 168 and 172 (100m's
# fires while a hammer is out, which this stage never plays) are new.
PLAYER_STEPS = {0: 26, 4: 27, 8: 28, 12: 31, 16: 34, 20: 32, 24: 35, 28: 33, 32: 36, 36: 22, 40: 29,
                44: 30, 96: 23, 100: 24, 104: 25, 108: 11, 112: 12, 116: 13, 120: 14}
BARREL_STEPS = {48: 1, 52: 2, 56: 3, 60: 4, 128: 12, 132: 13, 136: 14, 140: 15, 144: 16, 148: 17}
FIRE_STEPS = {152: 4, 156: 6, 168: 0, 172: 1}


def sprites(model):
    s = Sheet()
    m = tomllib.load(open(model, "rb"))
    ents = m["entities"]

    def frames(e):
        sp = ents[e].get("sprite", {})
        out = [(sp.get("tiles"), sp.get("width", 2))]
        for clip in sp.get("clips", []):
            for f in clip.get("frames", []):
                if f.get("tiles"):
                    out.append((f["tiles"], f.get("width", sp.get("width", 2))))
        return [(t, w) for t, w in out if t]

    def pictures(e, steps):
        """e's 2-by-2 pictures whose first tile `steps` names, each once,
        with the frame number it is drawn as. Any other frame of e (one the
        base draws from tiles across two pictures, as Original Edition's
        player's [2, 4, 3, 5], or another body's tiles, as its 100m fires'
        48 to 63, which are the barrels') draws nothing."""
        done = set()
        for tiles, w in frames(e):
            t = tuple(tiles)
            if len(t) == 4 and w == 2 and t[0] in steps and t == (t[0], t[0] + 2, t[0] + 1, t[0] + 3) and t not in done:
                done.add(t)
                yield tiles, steps[t[0]]

    climb = {36, 84, 96, 100, 104}
    for tiles, i in pictures("player", PLAYER_STEPS):
        first = tiles[0]
        if first in climb:
            kind = "climb"
        elif 0x0C <= first < 0x24:
            kind = "swing"
        elif first in (0x28, 0x2C):
            kind = "jump"
        elif 0x6C <= first < 0x7C:
            kind = "spin"
        else:
            kind = "walk"
        s.from_canvas(worker(kind, i), tiles, 2)
    for tiles, i in pictures("barrel", BARREL_STEPS):
        s.from_canvas(barrel_pic(i, 0x90 <= tiles[0] < 0x98), tiles, 2)
    for tiles, i in pictures("fire", FIRE_STEPS):
        s.from_canvas(fire_pic(i), tiles, 2)
    for i, (tiles, w) in enumerate(frames("pauline")):
        if len(tiles) == 6:
            s.from_canvas(friend_pic(i > 0), tiles, 2)
    # 100m's ending, which this stage never plays but a ROM version short
    # of these pictures fills from it: the heart, and the thrower falling
    # with both arms up, a sprite at each of 24 tiles.
    for tiles, w in frames("heart"):
        s.from_canvas(heart_pic(), tiles, w)
    for tiles, w in frames("kong_fall"):
        s.from_canvas(thrower("fall"), tiles, w)
    # The hammer: a mallet, 8 by 16, upright and sideways.
    h = canvas(8, 16)
    rect(h, 0, 0, 8, 6, 2)
    rect(h, 3, 6, 5, 16, 3)
    for tiles, w in frames("hammer"):
        if w == 1 and len(tiles) == 2:
            s.from_canvas(outline(h), tiles, 1)
        elif len(tiles) == 2:
            side = canvas(16, 8)
            rect(side, 10, 0, 16, 8, 2)
            rect(side, 0, 3, 10, 5, 3)
            s.from_canvas(outline(side), tiles, 2)
    # The drum's flames and its top.
    for tiles, w in frames("oil_drum"):
        f = canvas(16, 8)
        if tiles[0] == 0x3A:
            rect(f, 1, 2, 15, 8, 1)
        else:
            for x in range(1, 15, 4):
                rect(f, x, 2 + (x + tiles[0]) % 3, x + 3, 8, 2)
                rect(f, x + 1, 5, x + 2, 8, 3)
        s.from_canvas(f, tiles, 2)
    # The menu's cursor and the points.
    cur = canvas(8, 8)
    for y in range(7):
        rect(cur, 1, y, 1 + min(y, 6 - y) + 1, y + 1, 2)
    s.tile(0xA2, cur)
    for n, text in ((0xD0, "1"), (0xD1, "3"), (0xD2, "5"), (0xD3, "8"), (0xD4, "0")):
        s.tile(n, glyph(text, 3))
    return s


def scene_sprites():
    """The sprite sheet the stage-clear scene switches to as the thrower
    goes up with the friend (the base's scene_sprite_tiles): the thrower,
    48 by 40, a column of five tiles at a time from 0x40 and, for the other
    step, from 0x60, and the friend's head and shoulders, 16 by 16, at 0x80.
    The rest is blank."""
    s = Sheet()
    for first, pose in ((0x40, "beat_a"), (0x60, "beat_b")):
        c = canvas(48, 40)
        for y, row in enumerate(thrower(pose)):
            c[y] = row[:]
        tiles = []
        for row in range(5):
            tiles += [first + col * 5 + row for col in range(6)]
        s.from_canvas(c, tiles, 6)
    s.from_canvas(friend_pic(True)[:16], [0x80, 0x82, 0x81, 0x83], 2)
    return s


def write(out, kong_first):
    bg = background(kong_first)
    model = os.path.join(os.path.dirname(out), "donkey-kong-oe", "model.toml")
    sp = sprites(model)
    head = "# Drawn by scripts/rivet-rodeo/art.py; don't edit by hand.\n"
    with open(os.path.join(out, "scene_sprites.txt"), "w") as fh:
        fh.write(head + scene_sprites().text())
    with open(os.path.join(out, "tiles.txt"), "w") as fh:
        fh.write(head + bg.text())
    with open(os.path.join(out, "sprites.txt"), "w") as fh:
        fh.write(head + sp.text())
