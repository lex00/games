#!/usr/bin/env python3
"""Writes Rivet Rodeo's authored data and delta from one layout.

Rivet Rodeo (templates/rivet-rodeo) is an original stage on Donkey Kong's
model and rules: the public page plays it when no ROM is loaded
(docs/web.md, "The public page"). Every number here is laid out for this
stage; none is read from a ROM.

    python3 -I scripts/rivet-rodeo/stage.py

writes templates/rivet-rodeo/authored.toml and delta.toml (and art.py's
tiles.txt, sprites.txt and scene_sprites.txt). Then run
`gp template derive templates/rivet-rodeo --write` and
`gp template build templates/rivet-rodeo --write`, and paint the paper
theme again with the title (the script prints the commands).

The name players read is kept in scripts/rivet-rodeo/name.txt: NAME below
reads it, and the model, the asset map and the page (scripts/web/build-page.sh)
take it from there. The logo text drawn on the title screen and in the paper
themes' title screens (`gp theme paint --logo-file`) is title.txt, TITLE below.
"""

import os
import sys

TITLE = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "title.txt")).readline().strip()
# The name a player or visitor reads (the page, the template's titles),
# kept in name.txt. title.txt is only the logo text drawn on the title screen.
NAME = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "name.txt")).readline().strip()

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(REPO, "open")

sys.path.insert(0, HERE)
import art  # noqa: E402  (the stage's pictures, art.py beside this file)

# --- Tiles of the background sheet ------------------------------------------
BLANK = 0x24
# Girder cells, numbered so the base model's tile kinds and surfaces read
# them as their own: the girder whose top is k pixels down its cell (U),
# its lower part in the cell below (L), and each with a ladder showing
# above or below it (UA, LB). 76 and 77 are new to the base's lists.
U = {0: 48, **{k: 63 - k for k in range(1, 8)}}
L = {k: 48 + k for k in range(1, 8)}
LADDER = 63
UA = {7: 64, 5: 65, 4: 66, 3: 67, 2: 68, 1: 69, 6: 76}
LB = {1: 70, 2: 71, 3: 72, 4: 73, 5: 74, 6: 75, 7: 77}
DRUM = 0x4E  # 4 tiles, 2 by 2
PILE = 0x52  # the barrel pile beside the thrower: 2 by 4 tiles
BLOCK = 0x5A  # the title's letter block
HUDL = 0x5B  # HUD frame pieces 0x5B to 0x5F
KONG = 0x60  # the thrower's poses: 6 poses of 24 tiles, 0x60 to 0xEF

# --- The stage ---------------------------------------------------------------
# Girders are made of 24-pixel pieces from x = 24k + 8, each a pixel above
# or below the last where the girder slopes, as Donkey Kong's model's walk
# and barrels read them. Girder 1 is the bottom one; even girders roll
# barrels right, odd ones left; 6 is the thrower's. 7 is the top platform.


def piece(x):
    return (x - 8) // 24


def surface(g, x):
    """The girder's top row at x, or None where it has no piece.

    Tuned on 2026-11-03 to play like Donkey Kong's 25m, measured with
    `gp level measure` (docs/originals.md, "Rivet Rodeo"): girders about
    30 pixels apart at their middles, sloping a pixel each 24-pixel piece
    across, the bottom one flat for its left half. The ladders and hammers
    are this stage's own places, and `gp template check --original` holds
    it apart from 25m's layout.
    """
    k = piece(x)
    if g == 1:
        return 216 if x < 128 else 216 - (k - 4)
    if g == 2:
        return None if not 8 <= x < 224 else 182 + max(0, k)
    if g == 3:
        return None if not 32 <= x < 248 else 153 + (8 - min(max(k, 0), 8))
    if g == 4:
        return None if not 8 <= x < 224 else 122 + max(0, k)
    if g == 5:
        return None if not 32 <= x < 248 else 93 + (8 - min(max(k, 0), 8))
    if g == 6:
        if not 16 <= x < 224:
            return None
        return 64 + max(0, k - 2)
    if g == 7:
        return 40 if 96 <= x < 152 else None
    raise ValueError(g)


# Ladders: (x, the girder at their top, broken). A ladder runs from that
# girder down to the one below it. Each full one stands clear of a piece's
# step, so its foot is level from both sides.
LADDERS = [
    (184, 2, False), (64, 2, True),
    (88, 3, False), (64, 3, False), (208, 3, True),
    (160, 4, False), (216, 4, False), (112, 4, True),
    (64, 5, False), (200, 5, True),
    (136, 6, False), (184, 6, False),
    (112, 7, False),
]

HAMMERS = [(176, 3), (152, 5)]
PLAYER_X = 88
# The rescue figure stands on the top girder (row 40): 24 tall, at row 16.
# Until 2026-11-04 it was Donkey Kong's row 25, 9 px through the girder.
PAULINE = (100, 16)

# The attract demo: (way, frames). Way 1 right, 2 left, 4 down, 8 up, 0 none,
# 5 a jump.
DEMO = [
    (1, 120), (0, 24), (8, 90), (2, 70), (5, 1), (2, 60), (0, 30), (1, 150),
    (8, 40), (2, 100), (5, 1), (1, 40), (0, 60), (8, 120), (1, 80), (5, 1),
    (2, 90), (0, 40), (1, 200), (2, 255),
]

# The climb's pictures: the top zone's 16 steps and the climb's 6, by the
# player's pictures the model draws (36, 84, 96, 100 and 104 are climbs).
TOP_POSES = [104, 104, 100, 100, 96, 96, 100, 100, 104, 104, 84, 84, 36, 36, 36, 36]
STEP_POSES = [36, 36, 84, 84, 104, 104]
# The base model reads the climb table at fixed places from its first byte
# (the dk_mario hook's TOPS, TOP_SHAPES, TOP_POSES and STEP_POSES, and
# play_mario's $C147 and $C159): the tops from 0, their box from 28, the top
# zone's pictures from 32 and the climb's from 50. The 2 bytes between the
# top zone's 16 and the climb's 6 are read by neither; they hold the top
# zone's last picture.
CLIMB_AT = {"tops": 0, "box": 28, "top_poses": 32, "step_poses": 50, "end": 56}
CLIMB_GAP = [36, 36]

# Palettes: four for the background (girders, ladders, the thrower, the HUD),
# four for sprites (player, friend, fires, barrels and hammers).
PALETTES = [
    [0x0F, 0x07, 0x27, 0x37],
    [0x0F, 0x0C, 0x2C, 0x3C],
    [0x0F, 0x02, 0x22, 0x32],
    [0x0F, 0x30, 0x21, 0x16],
    [0x0F, 0x16, 0x36, 0x02],
    [0x0F, 0x14, 0x34, 0x30],
    [0x0F, 0x16, 0x27, 0x38],
    [0x0F, 0x08, 0x18, 0x28],
]


def lower(g):
    return g - 1


def girder_grid():
    """The name table: 32 by 30 tile numbers."""
    grid = [[BLANK] * 32 for _ in range(30)]
    # Girders, a cell column at a time.
    for g in range(1, 8):
        for col in range(32):
            x = col * 8
            y = surface(g, x + 4)
            if y is None:
                continue
            r, k = divmod(y, 8)
            grid[r][col] = U[k]
            if k:
                grid[r + 1][col] = L[k]
    # Ladders.
    for x, top, broken in LADDERS:
        col = x // 8
        y0, y1 = surface(top, x + 4), surface(lower(top), x + 4)
        # Pixel rows the ladder shows in: below the top girder's 8 pixels to
        # the lower girder's top.
        show = set(range(y0 + 8, y1))
        if broken:
            gap = set(range(y0 + 12, y1 - 10))
            show -= gap
        for r in range(30):
            rows = set(range(r * 8, r * 8 + 8)) & show
            if not rows:
                continue
            t = grid[r][col]
            ku = {v: k for k, v in U.items() if k}
            kl = {v: k for k, v in L.items()}
            if t == BLANK:
                grid[r][col] = LADDER
            elif t in ku:
                grid[r][col] = UA[ku[t]]
            elif t in kl:
                grid[r][col] = LB[kl[t]]
    # The oil drum and the barrel pile.
    grid[25][4], grid[25][5], grid[26][4], grid[26][5] = DRUM, DRUM + 1, DRUM + 2, DRUM + 3
    for i in range(8):
        grid[4 + i // 2][1 + i % 2] = PILE + i
    return grid


def rows_updates(grid, first_row=0, last_row=30):
    """Updates that draw the grid's non-blank runs, row by row."""
    out = []
    for r in range(first_row, last_row):
        c = 0
        while c < 32:
            if grid[r][c] == BLANK:
                c += 1
                continue
            e = c
            while e < 32 and grid[r][e] != BLANK:
                e += 1
            run = grid[r][c:e]
            if len(set(run)) == 1 and len(run) > 3:
                out.append(f"{{ ppu = 0x{0x2000 + r * 32 + c:04X}, repeat = {len(run)}, tile = 0x{run[0]:02X} }}")
            else:
                tiles = ", ".join(f"0x{t:02X}" for t in run)
                out.append(f"{{ ppu = 0x{0x2000 + r * 32 + c:04X}, tiles = [{tiles}] }}")
            c = e
    return out


def clear_updates():
    return [f"{{ ppu = 0x{0x2000 + r * 32:04X}, repeat = 32, tile = 0x{BLANK:02X} }}" for r in range(30)]


def attributes(groups):
    """The attribute table's 64 bytes from a palette per 32-pixel block."""
    out = []
    for by in range(8):
        for bx in range(8):
            p = groups(bx, by)
            out.append(p | p << 2 | p << 4 | p << 6)
    return out


def palette_updates(pals):
    flat = [c for p in pals for c in p]
    return [f"{{ ppu = 0x3F00, tiles = [{', '.join(f'0x{c:02X}' for c in flat)}] }}"]


def attr_updates(attrs):
    return [f"{{ ppu = 0x23C0, tiles = [{', '.join(f'0x{a:02X}' for a in attrs[:32])}] }}",
            f"{{ ppu = 0x23E0, tiles = [{', '.join(f'0x{a:02X}' for a in attrs[32:])}] }}"]


def stage_attr(bx, by):
    if by == 0:
        return 3
    if by == 1 and bx in (1, 2):
        return 2
    if by == 1 and bx >= 5:
        return 3
    return 0


def title_attr(bx, by):
    if 1 <= by <= 2:
        return 1
    if by >= 4:
        return 3
    return 0


# Big letters for the title, 3 cells wide and 5 tall, as rows of 0/1.
BIG = {
    "A": ["111", "101", "111", "101", "101"], "B": ["110", "101", "110", "101", "110"],
    "C": ["111", "100", "100", "100", "111"], "D": ["110", "101", "101", "101", "110"],
    "E": ["111", "100", "110", "100", "111"], "F": ["111", "100", "110", "100", "100"],
    "G": ["111", "100", "101", "101", "111"], "H": ["101", "101", "111", "101", "101"],
    "I": ["111", "010", "010", "010", "111"], "J": ["001", "001", "001", "101", "111"],
    "K": ["101", "101", "110", "101", "101"], "L": ["100", "100", "100", "100", "111"],
    "M": ["101", "111", "111", "101", "101"], "N": ["110", "101", "101", "101", "101"],
    "O": ["111", "101", "101", "101", "111"], "P": ["111", "101", "111", "100", "100"],
    "R": ["110", "101", "110", "101", "101"], "S": ["111", "100", "111", "001", "111"],
    "T": ["111", "010", "010", "010", "010"], "U": ["101", "101", "101", "101", "111"],
    "V": ["101", "101", "101", "101", "010"], "W": ["101", "101", "111", "111", "101"],
    "Y": ["101", "101", "010", "010", "010"], "Z": ["111", "001", "010", "100", "111"],
    " ": ["000"] * 5,
}


def title_grid():
    grid = [[BLANK] * 32 for _ in range(30)]
    words = TITLE.split()
    for i, word in enumerate(words[:2]):
        width = len(word) * 4 - 1
        c0 = (32 - width) // 2
        r0 = 3 + i * 6
        for j, ch in enumerate(word):
            glyph = BIG.get(ch, BIG[" "])
            for dy, line in enumerate(glyph):
                for dx, bit in enumerate(line):
                    if bit == "1":
                        grid[r0 + dy][c0 + j * 4 + dx] = BLOCK
    return grid


def text_update(row, col, text):
    return f'{{ ppu = 0x{0x2000 + row * 32 + col:04X}, text = "{text}" }}'


def ppu_strings():
    stage = clear_updates() + palette_updates(PALETTES) + attr_updates(attributes(stage_attr)) + rows_updates(girder_grid())
    title_pals = [[0x0F, 0x27, 0x17, 0x37], [0x0F, 0x16, 0x26, 0x36], [0x0F, 0x30, 0x30, 0x30], [0x0F, 0x30, 0x21, 0x16]]
    title = clear_updates() + palette_updates(title_pals) + [
        "{ ppu = 0x3F11, tiles = [0x16, 0x36, 0x30] }",
    ] + attr_updates(attributes(title_attr)) + rows_updates(title_grid()) + [
        text_update(16, 9, "1 PLAYER GAME A"),
        text_update(18, 9, "1 PLAYER GAME B"),
        text_update(20, 9, "2 PLAYER GAME A"),
        text_update(22, 9, "2 PLAYER GAME B"),
        text_update(25, 7, "AN ORIGINAL STAGE"),
        text_update(27, 9, "GAME PRINTER"),
    ]
    status = [
        text_update(2, 4, "1UP"),
        text_update(2, 16, "HIGH"),
        f"{{ ppu = 0x{0x2000 + 4 * 32 + 20:04X}, tiles = [0x{HUDL:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 1:02X}, 0x{HUDL + 2:02X}] }}",
        # The labels over their numbers, as the original's box has them:
        # M (lives) over column 21, L (the loop) over 28; the bonus's is
        # the frame's line, which the paper theme labels BONUS.
        text_update(4, 21, "M"),
        text_update(4, 28, "L"),
    ]
    blank = ["{ ppu = 0x2000, repeat = 1, tile = 0x24 }"]

    def block(name, ups, note):
        body = ",\n    ".join(ups)
        return f'  {{ name = "{name}", note = "{note}", updates = [\n    {body},\n  ] }},'

    return "\n".join([
        "[[asset]]",
        'name = "stage_screens"',
        'format = "ppu-strings"',
        'charset = [[0x00, "0123456789"], [0x0A, "ABCDEFGHIJKLMNOPQRSTUVWXYZ"], [0x24, " "], [0xF0, "|#"]]',
        "string = [",
        block("screen_25m", stage, "the stage: palettes, attributes, girders, ladders"),
        block("screen_100m", blank, "unused: Rivet Rodeo has no 100m"),
        block("screen_50m", blank, "unused: Rivet Rodeo has no 50m"),
        block("screen_75m", blank, "unused: Rivet Rodeo has no 75m"),
        block("screen_title", title, "the title screen"),
        block("screen_25m_more", status, "the status labels"),
        "]",
    ])


def ground_runs():
    """Runs of the walk's ground: x, y, shape, step across, count, end."""
    runs = []
    # Each girder is written piece by piece as runs of equal steps.
    for g in range(1, 8):
        xs = [x for x in range(0, 256, 8) if surface(g, x + 4) is not None]
        if not xs:
            continue
        x0, x1 = xs[0], xs[-1] + 8
        # Flat stretches as one box each; slopes as runs of 24-pixel pieces.
        segs = []
        x = x0
        while x < x1:
            y = surface(g, x + 4)
            e = x
            while e < x1 and surface(g, e + 4) == y:
                e += 8
            segs.append((x, e, y))
            x = e
        for i, (a, b, y) in enumerate(segs):
            end = 0xFE if i == len(segs) - 1 else 0x00
            runs.append((a, y, b - a, end))
    return runs


# The stage's own way of writing a box: each shape's rows are LIFT pixels
# below its run's or place's y (a box is the place plus the shape's
# corners, so the boxes are the same wherever the lift goes). Its shapes
# and places are then this stage's own bytes, as `gp template served` and
# `gp template check --original` ask: a girder piece 24 pixels wide and a
# ladder about 30 tall, written with no lift, are rows Donkey Kong's
# lifted 25m has too.
LIFT = 3


def ground_assets():
    runs = ground_runs()
    widths = sorted({w for _, _, w, _ in runs})
    shapes = [[0, LIFT, w, LIFT] for w in widths]
    rows = [[x, y - LIFT, widths.index(w) * 4, 0, 1, end] for x, y, w, end in runs]
    return rows, shapes


def ladder_assets():
    places, shapes = [], []
    for x, top, broken in LADDERS:
        y0, y1 = surface(top, x + 4), surface(lower(top), x + 4)
        if broken:
            box = [0, LIFT, 8, 10 + LIFT]
            y = y1 - 10
        else:
            box = [0, LIFT, 8, y1 - y0 + LIFT]
            y = y0
        if box not in shapes:
            shapes.append(box)
        places.append([x, y - LIFT, shapes.index(box) * 4])
    return places, shapes


def climb_table():
    tops = []
    for x, top, broken in LADDERS:
        if not broken:
            tops += [x, surface(top, x + 4), 0]
    assert len(tops) <= 27, "the climb table holds 9 ladder tops"
    tops += [0xFE] * (CLIMB_AT["box"] - len(tops))
    t = tops + [0, 0, 8, 12]
    assert len(t) == CLIMB_AT["top_poses"]
    t += TOP_POSES + CLIMB_GAP
    assert len(t) == CLIMB_AT["step_poses"], "the climb's pictures are read from 50"
    t += STEP_POSES
    assert len(t) == CLIMB_AT["end"]
    return t


def sound_table():
    """The driver's bytes from 0xFAE5 to its last jingle's end. Only the
    jingles need any: the jingle table's first note is a silent one (its
    period's low byte, at 0xFB01 + 2, is 0) held 16 frames, then 0 loops
    it."""
    base = 0xFAE5
    t = [0] * (0xFFD6 - base)
    t[0xFB62 - base] = 0x10  # jingle lengths: 16 frames
    t[0xFFD4 - base] = 0x02  # the jingle: one silent note, then 0
    # Trailing zeros are left to the asset's count.
    while t and t[-1] == 0:
        t.pop()
    return t + [0] * (0xFFD6 - base - len(t))


def barrel_params():
    """The barrels' numbers the delta sets: all but the tables'."""
    return {k: v for k, v in barrel_numbers().items() if k not in TABLE_KEYS}


def barrel_numbers():
    p = {}
    for g in range(2, 7):
        mine = [(x, b) for x, t, b in LADDERS if t == g]
        for c, (x, _) in zip("abc", mine):
            p[f"ladder_{c}{g}"] = x - 4
            p[f"ladder_{c}foot{g}"] = surface(lower(g), x + 4) - 14
        for c in "abc"[len(mine):]:
            p[f"ladder_{c}{g}"] = 2
            p[f"ladder_{c}foot{g}"] = 240
    # Where each girder steps a pixel: barrel x at multiples of 24.
    p.update({"step_lo1": 120, "step_hi1": 216, "step_lo6": 72, "step_hi6": 192})
    for g in (2, 4):
        p[f"step_lo{g}"], p[f"step_hi{g}"] = 24, 192
    for g in (3, 5):
        p[f"step_lo{g}"], p[f"step_hi{g}"] = 48, 192
    # The straight drop from the thrower's hands, at x 77 to 93.
    stops = [surface(g, 85) - 14 for g in (5, 4, 3, 2, 1)]
    for way in (1, 2, 3):
        for i, s in enumerate(stops, 1):
            p[f"straight{way}_stop{i}"] = s
    # The fast rows of the straight drop (way 1): two pixels a frame for 8
    # pixels, starting 6 below where it leaves girders 6, 4 and 2, this
    # stage's own pace.
    above = [surface(g, 85) - 14 for g in (6, 4, 2)]
    for i, top in enumerate(above, 1):
        p[f"straight_fast_lo{i}"] = top + 6
        p[f"straight_fast_hi{i}"] = top + 14
    # The hops a barrel takes where it lands from a girder's end: x where its
    # y moves by bounce_dy<i>, from where it lands on each side
    # (BOUNCE_START). Two hops up 2 and down 2, then one up 1 and down 2, 11
    # moves as the base's lists have (a delta can't shorten them), a pixel
    # down in all as the base's.
    dys = [-1, -1, 1, 1, -1, -1, 1, 1, -1, 1, 1]
    at = [0, 1, 3, 4, 8, 9, 12, 13, 16, 18, 20]
    for side, (x0, d) in BOUNCE_START.items():
        for i, a in enumerate(at, 1):
            p[f"bounce_{side}{i}"] = x0 + d * a
    for i, dy in enumerate(dys, 1):
        p[f"bounce_dy{i}"] = dy
    p["crowd_drop"] = 99
    return p


# Where a barrel lands from a girder's end, the first x of its hops, and the
# way it hops: from the left end (rolling left off girders 3 and 5) it lands
# near x 11 and hops right from 12; from the right end near 228 and hops
# left from 227.
BOUNCE_START = {"right": (12, 1), "left": (227, -1)}


# The fires' girders' heights by row, the play routine's heights table
# (play_heights, from $C0BC), the bottom girder's first.
GIRDER_HEIGHTS = [195, 165, 135, 105, 75, 48]

# The hammer's pictures by its swing, 1 to 6: the player's frames that
# art.py draws as swings, mallet up and down in turn.
SWING_POSES = [0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20]

# The play routine's tables (play_tables) start at this CPU address; the
# hooks read the barrels', fires' and swing's tables from it by address.
PLAY_TABLES = 0xC15F
# Where in it each list is, by CPU address: (the numbers' names, start).
TABLE_LISTS = [
    ([f"ladder_afoot{g}" for g in range(2, 7)], 0xC172),
    ([f"ladder_a{g}" for g in range(2, 7)], 0xC177),
    ([f"ladder_bfoot{g}" for g in range(2, 7)], 0xC17C),
    ([f"ladder_b{g}" for g in range(2, 7)], 0xC181),
    ([f"slope_{k}" for k in range(1, 9)], 0xC1C5),
    *[([f"straight{w}_stop{i}" for i in range(1, 6)], 0xC1D5 + 6 * (w - 1)) for w in (1, 2, 3)],
    ([f"bounce_right{i}" for i in range(1, 12)], 0xC3FC),
    ([f"bounce_dy{i}" for i in range(1, 12)], 0xC407),
    ([f"bounce_left{i}" for i in range(1, 12)], 0xC412),
    ([f"straight_fast_lo{i}" for i in range(1, 4)], 0xC41D),
    ([f"straight_fast_hi{i}" for i in range(1, 4)], 0xC420),
]
TABLE_KEYS = {k for names, _ in TABLE_LISTS for k in names}
# The base's map splits the play routine's tables at 75m's spans: play_tables
# to $C20B, then 75m's ($C20C to $C2EF, EMPTY_75M), 100m's ($C2F0 to $C3E1,
# EMPTY_100M), then play_tables_c2f0 (which keeps its name) to $C4FF.
PLAY_TABLES_75M = 0xC20C
PLAY_TABLES_C3E2 = 0xC3E2
PLAY_TABLES_END = 0xC500
# 75m's spans as (name, format, rows, width): byte-rows and one-column tables.
EMPTY_75M = [
    ("ground_runs_75m", "byte-rows", 21, 3),
    ("ground_runs_75m_end", "table", 1, 1),
    ("heights_75m", "table", 6, 1),
    ("ground_shapes_75m", "byte-rows", 7, 4),
    ("ladders_75m", "byte-rows", 10, 3),
    ("ladders_75m_end", "table", 1, 1),
    ("ladder_shapes_75m", "byte-rows", 6, 4),
    ("ladder_tops_75m", "byte-rows", 10, 3),
    ("play_tables_75m", "table", 21, 1),
    ("fire_ways_75m", "byte-rows", 8, 3),
]
# 100m's spans, the same way.
EMPTY_100M = [
    ("ground_runs_100m", "byte-rows", 5, 3),
    ("ground_runs_100m_end", "table", 1, 1),
    ("heights_100m", "table", 6, 1),
    ("ground_shapes_100m", "byte-rows", 4, 4),
    ("ladders_100m", "byte-rows", 14, 3),
    ("ladders_100m_end", "table", 1, 1),
    ("ladder_shapes_100m", "byte-rows", 2, 4),
    ("ladder_tops_100m", "byte-rows", 14, 3),
    ("ladder_tops_100m_end", "table", 1, 1),
    ("fire_index_100m", "table", 6, 1),
    ("fire_ways_100m", "byte-rows", 28, 3),
    ("fire_starts_100m", "byte-rows", 8, 2),
    ("fire_gaps_100m", "table", 4, 1),
]

# 50m's spans, which Original Edition's model names for its 50m (its lower
# half and CHR bank 1), the same way. The screen is a blank entry of
# stage_screens and the tiles and palettes are this stage's own (the
# delta's 50M_FROM below).
EMPTY_50M = [
    ("ground_runs_50m", "byte-rows", 7, 3),
    ("ground_shapes_50m", "byte-rows", 4, 4),
    ("ladders_50m", "byte-rows", 12, 3),
    ("ladder_shapes_50m", "byte-rows", 2, 4),
    # A list the model reads up to its end mark, 0xFE: one, so no rows.
    ("climb_tops_50m", "table", 37, 1, 0xFE),
    ("heights_50m", "table", 5, 1),
    ("ladder_waits_50m", "table", 10, 1),
    ("fire_lists_50m", "table", 84, 1),
    ("conveyor_pattern", "table", 10, 1),
    ("conveyor_turns", "table", 5, 1),
    ("pie_row_masks", "table", 5, 1),
]
# The slots of Original Edition's model that name an asset of its lower
# half or CHR bank 1, and this stage's asset each takes instead: the title
# is an entry of stage_screens, drawn with its one set of tiles.
FROM_50M = {
    "title": "stage_screens",
    "title_tiles": "tiles",
    "tiles_50m": "tiles",
    "sprite_tiles_50m": "sprite_tiles",
    "stage_screens_50m": "stage_screens",
    "palettes_50m": "palettes",
}


def empty_spans(spans):
    """Spans (EMPTY_75M, EMPTY_100M, EMPTY_50M) as authored assets, every
    byte 0 but a table's first when the span gives one."""
    out = []
    for name, fmt, rows, width, *first in spans:
        out += ["[[asset]]", f'name = "{name}"', f'format = "{fmt}"']
        if fmt == "table":
            sets = f"[[0, 0, {first[0]}]]" if first else "[]"
            out += ["bits = 8", 'columns = ["byte"]', f"count = {rows}", "rows = [[0]]", f"set = {sets}"]
        else:
            out += [f"width = {width}", table_rows([[0] * width] * rows, width)]
        out.append("")
    return out


def play_table_sets():
    """play_tables' values as [row, column, value]: the barrels' and fires'
    lists and the swing's pictures, at their addresses."""
    nums = {**barrel_numbers(), **fire_numbers()}
    out = []
    for names, at in TABLE_LISTS:
        for i, k in enumerate(names):
            out.append([at - PLAY_TABLES + i, 0, nums[k] & 0xFF])
    for i, pose in enumerate(SWING_POSES):
        out.append([0xC1A2 - PLAY_TABLES + i, 0, pose])
    out += stop_sets()
    return sorted(out)


def fire_params():
    """The fires' numbers the delta sets: all but the tables'."""
    return fire_numbers(tables=False)


def fire_numbers(tables=True):
    p = {f"girder_{i}": v for i, v in enumerate(GIRDER_HEIGHTS)}
    # Where the girders step, as the barrels' step_lo and step_hi say; the
    # bottom girder steps only from 120 (index 4) on.
    for i, x in enumerate([24, 48, 72, 96, 120, 144, 168, 192, 216]):
        p[f"slope_{i}"] = x
    p["slope_ends"] = 4
    if not tables:
        p = {k: v for k, v in p.items() if not k.startswith("girder_") and k not in TABLE_KEYS}
    full = [(x, t, b) for x, t, b in LADDERS if t == 2]
    mid = [(x, t, b) for x, t, b in LADDERS if t == 3 and not b]
    for set_, ls in (("bottom", full), ("mid", mid)):
        for c, (x, t, _) in zip("ab", ls):
            p[f"ladder_{set_}_{c}_x"] = x - 4
            p[f"ladder_{set_}_{c}_top"] = surface(t, x + 4) - 16
            p[f"ladder_{set_}_{c}_foot"] = surface(lower(t), x + 4) - 16
    return p


def stop_sets():
    """play_tables' values for the broken ladders' zones where the climb is
    refused, as [row, column, value], where the base reads them
    (levels.main.ladder_stops: rows of x, y and a shape from 0xC161, ended
    by 0xFE, and the 8 by 24 box from 0xC16E)."""
    rows = []
    for x, top, broken in LADDERS:
        if broken:
            rows += [x, surface(lower(top), x + 4) - 32, 0]
    assert len(rows) <= 12, "the stop list holds 4 zones"
    rows += [0xFE] * (13 - len(rows))
    at = 0xC161 - PLAY_TABLES
    return [[at + i, 0, v] for i, v in enumerate(rows + [0, 0, 8, 24]) if v]


def inline(d):
    def val(v):
        return f'"{v}"' if isinstance(v, str) else str(v)
    return "{ " + ", ".join(f"{k} = {val(v)}" for k, v in d.items()) + " }"


def table_rows(values, columns):
    return "rows = [\n" + "".join(f"  [{', '.join(str(v) for v in r)}],\n" for r in values) + "]"


def authored():
    runs, shapes = ground_assets()
    places, lshapes = ladder_assets()
    # The thrower's poses, and its two fallen ones (100m's ending, which
    # this stage never plays) in the background's free tiles (art.FALLEN_TILES).
    poses = {**art.kong_poses(KONG), **art.fallen_poses()[0]}
    parts = [
        "# Rivet Rodeo's assets (docs/originals.md, \"Authored assets\"), written by",
        "# scripts/rivet-rodeo/stage.py from its layout: don't edit by hand. Every",
        "# number is this stage's own; the pictures are art.py's.",
        'format = "0.1.1"',
        'source = "rivet-rodeo"',
        f'title = "{NAME}"',
        "",
        "[[asset]]",
        'name = "sprite_tiles"',
        'format = "chr-2bpp"',
        "tiles = 256",
        'file = "sprites.txt"',
        "",
        "# The sprites the stage-clear scene draws the thrower and the friend",
        "# with as he goes up (the base's CHR bank 2, scene_sprite_tiles).",
        "[[asset]]",
        'name = "scene_sprite_tiles"',
        'format = "chr-2bpp"',
        "tiles = 256",
        'file = "scene_sprites.txt"',
        "",
        "[[asset]]",
        'name = "tiles"',
        'format = "chr-2bpp"',
        "tiles = 256",
        'file = "tiles.txt"',
        'charset = [[0x00, "0123456789"], [0x0A, "ABCDEFGHIJKLMNOPQRSTUVWXYZ"], [0x24, " "], [0xF0, "|#"]]',
        "",
        ppu_strings(),
        "",
        "[[asset]]",
        'name = "palettes"',
        'format = "palette"',
        "palettes = [" + ", ".join("[" + ", ".join(f"0x{c:02X}" for c in p) + "]" for p in PALETTES) + "]",
        "",
        "# The one-palette updates the base's model names: the stage-clear",
        "# scene's sprite palette 0 for the thrower going up (the thrower's own",
        "# colors), and 100m's hammer and ending palettes, which this stage",
        "# never plays (the fires' and the thrower's colors).",
        *[line for name, pal in (("palette_scene", PALETTES[2]), ("palette_hammer", PALETTES[6]), ("palette_ending", PALETTES[2]))
          for line in ("[[asset]]", f'name = "{name}"', 'format = "palette"', "palettes = [[" + ", ".join(f"0x{c:02X}" for c in pal) + "]]", "")],
        "[[asset]]",
        'name = "ladders"',
        'format = "byte-rows"',
        "width = 3",
        table_rows(places, 3),
        "",
        "[[asset]]",
        'name = "ladder_shapes"',
        'format = "byte-rows"',
        "width = 4",
        table_rows(lshapes, 4),
        "",
        "[[asset]]",
        'name = "ground_runs"',
        'format = "byte-rows"',
        "width = 6",
        table_rows(runs, 6),
        "",
        "[[asset]]",
        'name = "ground_shapes"',
        'format = "byte-rows"',
        "width = 4",
        table_rows(shapes, 4),
        "",
        "[[asset]]",
        'name = "climb_rom"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        table_rows([[v] for v in climb_table()], 1),
        "",
        "[[asset]]",
        'name = "demo_script"',
        'format = "table"',
        "bits = 8",
        'columns = ["way", "frames"]',
        table_rows(DEMO, 2),
        "",
        "# The play routine's tables that the handler's cost reads, empty for",
        "# now, but for the girders' heights and, in play_tables, the barrels',",
        "# fires' and swing's lists the hooks read by address.",
        "[[asset]]",
        'name = "play_pointers"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        "count = 80",
        "rows = [[0]]",
        "set = []",
        "",
        "[[asset]]",
        'name = "play_heights"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        "count = 7",
        table_rows([[v] for v in GIRDER_HEIGHTS], 1),
        "set = []",
        "",
        "[[asset]]",
        'name = "play_gaps"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        "count = 20",
        "rows = [[0]]",
        "set = []",
        "",
        "[[asset]]",
        'name = "play_ladders_end"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        "count = 1",
        "rows = [[0]]",
        "set = []",
        "",
        "[[asset]]",
        'name = "play_tables"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        f"count = {PLAY_TABLES_75M - PLAY_TABLES}",
        "rows = [[0]]",
        "set = [" + ", ".join(f"[{r}, {c}, {v}]" for r, c, v in play_table_sets() if r < PLAY_TABLES_75M - PLAY_TABLES) + "]",
        "",
        "# Donkey Kong's 75m spans, $C20C to $C2EF, which the base's model names",
        "# for its 75m: empty here, since Rivet Rodeo has no 75m.",
        *empty_spans(EMPTY_75M),
        "# Donkey Kong's 100m spans, $C2F0 to $C3E1: empty here too.",
        *empty_spans(EMPTY_100M),
        "# Original Edition's 50m spans, in its lower half: empty here too.",
        *empty_spans(EMPTY_50M),
        "[[asset]]",
        'name = "play_tables_c2f0"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        f"count = {PLAY_TABLES_END - PLAY_TABLES_C3E2}",
        "rows = [[0]]",
        "set = [" + ", ".join(f"[{r - (PLAY_TABLES_C3E2 - PLAY_TABLES)}, {c}, {v}]" for r, c, v in play_table_sets() if r >= PLAY_TABLES_C3E2 - PLAY_TABLES) + "]",
        "",
        "# The code slots the handler's cost walks (docs/cycles.md): an original",
        "# has no such code, so they are empty and its routines' costs are the",
        "# cost terms' medians.",
        "[[asset]]",
        'name = "code_c500"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        "count = 269",
        "rows = [[0]]",
        "set = []",
        "",
        "[[asset]]",
        'name = "code_c6a3"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        "count = 93",
        "rows = [[0]]",
        "set = []",
        "",
        "# The rest of that code, in the spans the base's model names round",
        "# 100m's ending's palette and poses, empty too.",
        *empty_spans([("code_c707", "table", 22, 1), ("code_c735", "table", 1, 1), ("code_c74e", "table", 11789, 1)]),
        "[[asset]]",
        'name = "code_fa45"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        "count = 160",
        "rows = [[0]]",
        "set = []",
        "",
        "# The sound driver's tables, from 0xFAE5: silent for now. Every song ends",
        "# at once; each jingle is one silent note (period 0) that loops.",
        "[[asset]]",
        'name = "sound_rom"',
        'format = "table"',
        "bits = 8",
        'columns = ["byte"]',
        f"count = {len(sound_table())}",
        "rows = [[0]]",
        "set = [" + ", ".join(f"[{i}, 0, {v}]" for i, v in enumerate(sound_table()) if v) + "]",
        "",
        "# Original Edition's jingles in its lower half ($8340), which its",
        "# driver names; this stage's driver never does, so they are empty.",
        *empty_spans([("sound_jingles", "table", 42, 1)]),
    ]
    for name, tiles in poses.items():
        parts += ["", "[[asset]]", f'name = "kong_{name}"', 'format = "byte-rows"', "width = 24", table_rows([tiles], 24)]
    return "\n".join(parts) + "\n"


def delta():
    b = barrel_params()
    f = fire_params()
    places, _ = ladder_assets()
    spawn_hammers = [(x, surface(g, x + 4) - 29) for x, g in HAMMERS]
    import tomllib
    base = tomllib.load(open(os.path.join(REPO, "model.toml"), "rb"))
    spawns = []
    hammers = iter(spawn_hammers)
    for sp in base["levels"]["main"]["spawn"]:
        sp = dict(sp)
        if sp["entity"] == "player":
            sp["x"], sp["y"] = PLAYER_X, surface(1, PLAYER_X + 8) - 12
        elif sp["entity"] == "hammer":
            sp["x"], sp["y"] = next(hammers)
        elif sp["entity"] == "pauline":
            sp["x"], sp["y"] = PAULINE
        spawns.append(sp)
    spawn_text = "spawn = [\n" + "".join("  " + inline(sp) + ",\n" for sp in spawns) + "]"
    lines = [
        "# Rivet Rodeo as a delta on Donkey Kong's model (docs/templates.md,",
        "# \"Derived templates\"), written by scripts/rivet-rodeo/stage.py: its",
        "# own stage's numbers over the base's. Edit the script, run it, then",
        "# `gp template derive templates/rivet-rodeo --write`.",
        "",
        '["model.toml".model]',
        'name = "rivet_rodeo"',
        f'title = "{NAME}"',
        f'description = "{NAME}, an original stage on Donkey Kong\'s model and rules (templates/donkey-kong-oe), for the public page with no ROM: its girders, ladders, tables and pictures are authored in templates/rivet-rodeo, laid out by scripts/rivet-rodeo/stage.py"',
        "",
        '["model.toml".levels.main]',
        f'description = "{NAME}: six sloped girders, full and broken ladders, an oil drum and a top platform"',
        spawn_text,
        "",
        "# Two cells the base's lists lack: a girder 6 pixels down its cell with a",
        "# ladder above it, and a girder's lower part with a ladder below it.",
        '["model.toml".levels.main.tile_kinds]',
        '76 = "ladder"',
        '77 = "ladder"',
        "",
        '["model.toml".levels.main.tile_surfaces]',
        "76 = 6",
        "",
        "# Donkey Kong's model plays 50m, 75m and 100m in its levels m50, m75 and",
        "# m100; Rivet Rodeo has none of them, so those slots of the stages list",
        "# name no level and 25m stands in.",
        '["model.toml".stages]',
        'order = ["main", "m50_off", "m75_off", "m100_off"]',
        "",
        '["fidelity.toml".contract]',
        'model = "rivet_rodeo"',
        f'description = "{NAME}: no original to hold it to; its own model baselines (baselines/)"',
        "",
        '["observe.toml"]',
        'model = "rivet_rodeo"',
        "",
        '["mapping.toml"]',
        'model = "rivet_rodeo"',
        "",
        "# The slots that name Donkey Kong's 25m assets take this stage's.",
        *[f'["model.toml".assets.{n}.from]\nasset = "{n}"\n' for n in ("ground_runs", "ground_shapes", "ladders", "ladder_shapes", "palettes")],
        "# The slots that name Original Edition's lower half or CHR bank 1 take",
        "# this stage's assets (FROM_50M).",
        *[f'["model.toml".assets.{n}.from]\nasset = "{a}"\n' for n, a in FROM_50M.items()],
        "# The base's note on 100m's fallen thrower quotes two of its pointers",
        "# ($C03C's entries $14 and $16), bytes of its ROM; this stage has no",
        "# 100m; its kong_fallen_a and kong_fallen_b are art.py's toppled thrower.",
        '["model.toml".evidence."entities.kong_fallen"]',
        'note = "$CDF3-$CE21: $C815 with $00 $8D, $01 $22 draws entry $14 or $16 of $C03C (kong_fallen_a or kong_fallen_b after a size byte: 24 tiles a column at a time) at $228D, after the attributes $23EB-$23EC $AA (entry $12); 100m\'s Kong draws at $208D with background palette 2 the same way. Rivet Rodeo has no 100m; its two poses are its thrower toppled onto its mast (scripts/rivet-rodeo/art.py, fallen_poses)"',
        "",
        '["model.toml".entities.barrel]',
        "ext_params = " + inline(b),
        "",
        '["model.toml".entities.fire]',
        "ext_params = " + inline(f),
        "",
    ]
    return "\n".join(lines) + "\n", spawn_hammers


def main():
    os.makedirs(OUT, exist_ok=True)
    art.write(OUT, KONG)
    with open(os.path.join(OUT, "authored.toml"), "w") as fh:
        fh.write(authored())
    text, hammers = delta()
    with open(os.path.join(OUT, "delta.toml"), "w") as fh:
        fh.write(text)
    print("wrote", OUT, "hammers", hammers)
    print("then:")
    print("  gp template derive templates/rivet-rodeo --write")
    print("  gp template build templates/rivet-rodeo --write")
    print("  gp theme kit templates/rivet-rodeo")
    print(f'  gp theme paint templates/rivet-rodeo -o templates/rivet-rodeo/themes/paper --logo-file scripts/rivet-rodeo/title.txt --force')


if __name__ == "__main__":
    main()
