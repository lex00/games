"""The space bar's bays and their openings, tile by tile: each of the board's slanted bars a separate ribbed tube.

    python3 runway/bays.py THEME_DIR [PREVIEW_PNG]

The board draws a bay as separate slanted bars with the void between them, and an opening
as a few coloured lines down the bay's inner edge. Each stays what it is: a bar becomes a
round ribbed tube of bone-grey metal, lit down one side and dark down the other, notched
every few pixels like a spine; an opening's lines become a ridge of the same material.
Nothing is filled in between, so the bars stay apart and the void shows through.

One 32 by 32 picture is made for each tile the bays and openings use, read from the board
picture in the local kit only for where each bar's pixels are (no colour of the board's
goes in). A tile is taken where the board draws it unmirrored (the listing with mirror
marks, flips-SCREEN.txt), since the room rules mirror a cell-sized piece with its cell.
Prints the room rules to add.
"""
import os
import sys
from collections import Counter

from PIL import Image

KIT = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/')
WORK = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/')
SCREENS = ('screen_8021', 'screen_87a1', 'screen_8f21', 'screen_96a1')
M, K = (109, 109, 109), (0, 0, 0)
ORANGE, RED, YELLOW = (255, 109, 0), (255, 0, 0), (219, 146, 0)
BAY = ['0be', '0bf', '0c0', '0c1', '0c2', '0c3', '0c4', '0c5', '0c6', '0cd', '0ce']
TUBE = [(226, 226, 214), (168, 170, 172), (112, 114, 124), (68, 70, 82), (30, 30, 40)]
RIDGE = {ORANGE: (222, 218, 200), YELLOW: (150, 146, 136), RED: (86, 84, 94)}


def main():
    theme = sys.argv[1]
    rooms = {}
    for s in SCREENS:
        im = Image.open(KIT + s + '.png').convert('RGB')
        grid = []
        for line in open(WORK + f'flips-{s}.txt'):
            p = line.split()
            if len(p) == 33 and p[0].isdigit():
                grid.append([(c[:3], c.endswith('<')) for c in p[1:]])
        rooms[s] = (im, grid)
    # the opening tiles: those with the frame's red and yellow, away from the ship at the top
    frame = set()
    for s, (im, grid) in rooms.items():
        for r in range(4, 30):
            for c in range(32):
                t = grid[r][c][0]
                cols = set(im.crop((c * 16, r * 16, c * 16 + 16, r * 16 + 16)).getdata())
                if RED in cols and YELLOW in cols and t not in BAY:
                    frame.add(t)
    tiles = BAY + sorted(frame)
    made = []
    for t in tiles:
        best = None
        for s, (im, grid) in rooms.items():
            for r in range(30):
                for c in range(32):
                    if grid[r][c][0] == t and (best is None or (best[3] and not grid[r][c][1])):
                        best = (s, r, c, grid[r][c][1])
        if best is None:
            continue
        s, r, c, flipped = best
        px = rooms[s][0].load()
        out = Image.new('RGBA', (32, 32), (0, 0, 0, 0))
        o = out.load()
        for y in range(16):
            gy = r * 16 + y
            for x in range(16):
                gx = c * 16 + x
                p = px[gx, gy]
                if p == M:
                    # this bar's run along the row, across cell edges, so its roundness is whole
                    a = gx
                    while a > 0 and px[a - 1, gy] == M:
                        a -= 1
                    b = gx
                    while b < 511 and px[b + 1, gy] == M:
                        b += 1
                    n = (b - a + 1) * 2
                    for sx in range(2):
                        i = (gx - a) * 2 + sx
                        if flipped:
                            i = n - 1 - i
                        f = i / max(1, n - 1)
                        k = 0 if f < 0.16 else 1 if f < 0.42 else 2 if f < 0.7 else 3 if f < 0.9 else 4
                        for sy in range(2):
                            Y = y * 2 + sy
                            kk = min(4, k + 1) if Y % 8 == 7 else max(0, k - 1) if Y % 8 == 0 and k > 0 else k
                            o[x * 2 + sx, Y] = TUBE[kk] + (255,)
                elif p in RIDGE:
                    for sx in range(2):
                        for sy in range(2):
                            Y = y * 2 + sy
                            col = RIDGE[p]
                            if Y % 8 == 7:
                                col = tuple(max(0, v - 40) for v in col)
                            o[x * 2 + sx, Y] = col + (255,)
                elif p != K and t not in BAY:
                    for sx in range(2):
                        for sy in range(2):
                            o[x * 2 + sx, y * 2 + sy] = p + (255,)      # something else in the cell: left as it is for now
        if flipped:
            out = out.transpose(Image.FLIP_LEFT_RIGHT)
        out.save(f'{theme}/pieces/space/t{t}.png')
        made.append(t)
    with open(WORK + 'bays-rules.txt', 'w') as f:
        f.write('PIECES\n' + ''.join(f'stage_piece st{t} pieces/space/t{t}.png 32 32\n' for t in made))
        f.write('CLASSES\n' + ''.join(f'#+ room_cells sb{t} tile=0x{t}\n' for t in made))
        f.write('ALL ' + ' '.join(f'tile=0x{t}' for t in made) + '\n')
        f.write('FILLS\n' + ''.join(f'#+ room_fill sb{t} st{t}\n' for t in made))
    print('tiles', len(made), 'of which openings', len(frame))
    if len(sys.argv) > 2:
        s = 'screen_8021'
        im, grid = rooms[s]
        gal = Image.open(f'{theme}/pieces/space/stars_far.png').convert('RGBA')
        prev = im.resize((1024, 960), Image.NEAREST).convert('RGBA')
        for r in range(30):
            for c in range(32):
                t, fl = grid[r][c]
                if t in made:
                    prev.paste(gal.crop((c * 32, r * 32, c * 32 + 32, r * 32 + 32)), (c * 32, r * 32))
                    tp = Image.open(f'{theme}/pieces/space/t{t}.png').convert('RGBA')
                    if fl:
                        tp = tp.transpose(Image.FLIP_LEFT_RIGHT)
                    prev.alpha_composite(tp, (c * 32, r * 32))
        prev.convert('RGB').crop((0, 100, 560, 620)).save(sys.argv[2])


if __name__ == '__main__':
    main()
