"""Look at the space room's pads and pylons: which tiles they are, where, and which way round.

    python3 runway/pads.py

Prints, for the four rooms, the tiles in each room that are neither void, tunnel, bar
nor keg, with how often and where they stand, sorted so the upright pylons (one tile
repeated down a column) and the pads (a small block under each bar's end) are plain to
see; and saves an enlarged crop of one pad with its stem for a look.
"""
import os
import re
from collections import Counter, defaultdict

from PIL import Image

W = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/')
T = os.path.expanduser('~/.gameprinter/work/themes/tapper-pixel/repo/theme/')
KIT = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/')


def main():
    s = open(T + 'art.txt').read()
    known = set()
    for name in ('sbay', 'skeg', 'sbar', 'sbarin', 'sbarcap', 'sship'):
        m = re.search(rf'#\+ room_cells {name} ([^\n]*)\n', s)
        if m:
            known |= {p[7:] for p in m.group(1).split() if p.startswith('tile=')}
    pos = defaultdict(list)
    for scr in ('screen_8021', 'screen_87a1', 'screen_8f21', 'screen_96a1'):
        g = []
        for line in open(W + f'flips-{scr}.txt'):
            p = line.split()
            if len(p) == 33 and p[0].isdigit():
                g.append([(c[:3], c.endswith('<')) for c in p[1:]])
        for r in range(30):
            for c in range(32):
                t, fl = g[r][c]
                if t != '000' and t not in known:
                    pos[t].append((scr[-4:], r, c, fl))
        if scr == 'screen_8021':
            grid = g
    im = Image.open(KIT + 'screen_8021.png').convert('RGB')
    for t, v in sorted(pos.items(), key=lambda kv: -len(kv[1])):
        here = [(r, c, fl) for sc, r, c, fl in v if sc == '8021']
        r, c, _ = here[0] if here else (0, 0, 0)
        cols = Counter(im.crop((c * 16, r * 16, c * 16 + 16, r * 16 + 16)).getdata()) if here else {}
        print(t, len(v), 'in 8021:', here[:7], 'colours', [(k, n) for k, n in cols.most_common(3)])
    im.crop((288, 136, 368, 200)).resize((640, 512), Image.NEAREST).save(W + 'pad-orig.png')
    for r in range(8, 13):
        print(r, ' '.join(grid[r][c][0] + ('<' if grid[r][c][1] else ' ') for c in range(17, 24)))


if __name__ == '__main__':
    main()
