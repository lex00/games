"""Read, for timing only, the space room's colour cycles and the cells beyond each bar's end.

    python3 runway/cycles.py

From my local ROM (never copied anywhere): the three lists the space room's lights step
through, each step (frames to wait, colour slot, colour), so a theme's flashes can keep
the board's own time. And from the rooms' cell listings: the tiles one cell beyond a
bar's end cell that still hold some of the bar's colours, where the bar's rounded end
reaches.
"""
import os
import zipfile
from collections import Counter

from PIL import Image

W = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/')
KIT = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/')
BAR = {'0f4', '0f5', '0fa', '0fb'}
END = {'0f6', '0f7', '0fc', '0fd'}


def main():
    z = zipfile.ZipFile('/Users/alex/Documents/roms/arcade/tapper.zip')
    d = z.read('tapper_c.p.u._pg_1_2c_1-27-84.2c')
    for at in (0x985, 0x98f, 0x999):
        rows, i = [], at
        while d[i] != 0 and len(rows) < 12:
            rows.append((d[i], d[i + 1], hex(d[i + 2])))
            i += 3
        print(hex(0x4000 + at), rows)
    beyond = Counter()
    for s in ('screen_8021', 'screen_87a1', 'screen_8f21', 'screen_96a1'):
        im = Image.open(KIT + s + '.png').convert('RGB')
        g = []
        for line in open(W + f'flips-{s}.txt'):
            p = line.split()
            if len(p) == 33 and p[0].isdigit():
                g.append([c[:3] for c in p[1:]])
        for r in range(30):
            for c in range(32):
                if g[r][c] in END:
                    for cc in (c - 1, c + 1):
                        if 0 <= cc < 32 and g[r][cc] not in BAR and g[r][cc] not in END and g[r][cc] != '000':
                            cols = Counter(im.crop((cc * 16, r * 16, cc * 16 + 16, r * 16 + 16)).getdata())
                            beyond[(g[r][cc], tuple(sorted((k, n) for k, n in cols.items() if k != (0, 0, 0))))] += 1
    for (t, cols), n in beyond.most_common():
        print('beyond an end:', t, n, cols)


if __name__ == '__main__':
    main()
