"""Find the space room's bar-end cells and give each bar a clean square end.

    python3 runway/barends.py THEME_DIR

Lists, from the four rooms' cell listings with mirror marks, every tile that stands
directly left or right of a bar's own cells in a bar's two rows, with the side the bar
is on where the board draws the tile unmirrored. Then writes:

  pieces/space/barend.png   a steel end plate, 8 by 64 stage pixels (the bar's two rows),
                            cut from the bar painting's own edging so it matches
  the rules, printed, to add to art.txt: the end cells filled with the bar painting (so
  the bar runs clean to the cell's edge and none of the board's rounded cap shows) and
  the end plate at the outer edge of each top-row end cell, mirrored with its cell.
"""
import os
import sys
from collections import defaultdict

from PIL import Image

W = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/')
BAR = {'0f4', '0f5', '0fa', '0fb'}
TOP = {'0f4', '0fa'}


def main():
    theme = sys.argv[1]
    found = defaultdict(set)          # tile -> {(bar side when unmirrored, is top row)}
    for s in ('screen_8021', 'screen_87a1', 'screen_8f21', 'screen_96a1'):
        g = []
        for line in open(W + f'flips-{s}.txt'):
            p = line.split()
            if len(p) == 33 and p[0].isdigit():
                g.append([(c[:3], c.endswith('<')) for c in p[1:]])
        for r in range(30):
            for c in range(32):
                t, fl = g[r][c]
                if t in BAR or t == '000':
                    continue
                for side, cc in (('right', c + 1), ('left', c - 1)):
                    if 0 <= cc < 32 and g[r][cc][0] in BAR:
                        native = side if not fl else ('left' if side == 'right' else 'right')
                        found[t].add((native, g[r][cc][0] in TOP))
    for t, v in sorted(found.items()):
        print(t, sorted(v))
    bar = Image.open(f'{theme}/pieces/space/bar.png').convert('RGB')
    row = bar.crop((0, 224, 8, 288))
    plate = Image.new('RGBA', (8, 64), (16, 16, 22, 255))
    px = plate.load()
    for y in range(64):
        base = row.getpixel((3, y))
        lum = sum(base) / 3
        px[0, y] = (16, 16, 22, 255)
        for x in range(1, 7):
            k = 1.25 if x in (1, 2) else 0.8 if x >= 5 else 1.0
            g_ = min(255, int((90 + lum * 0.35) * k))
            px[x, y] = (g_ - 6, g_, g_ + 8, 255)
        px[7, y] = (16, 16, 22, 255)
    for y in (4, 30, 58):
        px[3, y] = (230, 234, 240, 255)
        px[4, y + 1] = (40, 42, 50, 255)
    plate.save(f'{theme}/pieces/space/barend.png')
    print('end plate written')


if __name__ == '__main__':
    main()
