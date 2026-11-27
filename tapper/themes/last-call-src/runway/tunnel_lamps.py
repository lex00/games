"""The tunnel's red lamps: dim on the tunnel itself, and lit in pieces that come and go at their own times.

    python3 runway/tunnel_lamps.py THEME_DIR

Finds each red lamp on the tunnel piece (pieces/space/bay.png, as port.py leaves it),
dims it there, and writes GROUPS pieces the tunnel's size, clear except for some of the
lamps lit: the lamp's own pixels brightened to a hot red with a white heart, and a
dithered glow round it. Each lamp is in one group. The room rules show each group on a
cycle of its own, lengths that share no factor, so across the tunnel the lamps come on
in no pattern the eye can follow, and the same every time a clip is replayed.
Also writes pieces/space/none.png, one clear pixel, for the frames a group is dark.
Prints the rules.
"""
import os
import sys
from collections import deque

from PIL import Image

GROUPS = 5
CYCLES = [
    'lampsG:9 snone:61 lampsG:5 snone:113',
    'snone:37 lampsG:11 snone:89 lampsG:6 snone:47',
    'snone:71 lampsG:8 snone:131 lampsG:10',
    'snone:19 lampsG:7 snone:157 lampsG:9 snone:43',
    'snone:103 lampsG:12 snone:67 lampsG:5 snone:29',
]


def red(p):
    r, g, b, a = p
    return a and r > 150 and g < 80 and b < 80


def main():
    theme = sys.argv[1]
    bay = Image.open(f'{theme}/pieces/space/bay.png').convert('RGBA')
    px = bay.load()
    w, h = bay.size
    seen = set()
    lamps = []
    for y in range(h):
        for x in range(w):
            if red(px[x, y]) and (x, y) not in seen:
                q = deque([(x, y)])
                seen.add((x, y))
                comp = [(x, y)]
                while q:
                    cx, cy = q.popleft()
                    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)):
                        n = (cx + dx, cy + dy)
                        if 0 <= n[0] < w and 0 <= n[1] < h and n not in seen and red(px[n]):
                            seen.add(n)
                            q.append(n)
                            comp.append(n)
                if len(comp) >= 12:
                    lamps.append(comp)
    lamps.sort(key=lambda c: min(p[0] for p in c))
    groups = [Image.new('RGBA', (w, h), (0, 0, 0, 0)) for _ in range(GROUPS)]
    order = [0, 3, 1, 4, 2]                       # neighbours never share a group
    for i, comp in enumerate(lamps):
        g = groups[order[i % GROUPS]]
        gp = g.load()
        cx = sum(p[0] for p in comp) / len(comp)
        cy = sum(p[1] for p in comp) / len(comp)
        # the glow on the steel round the lamp, dithered, before the lamp itself
        for yy in range(int(cy) - 11, int(cy) + 12):
            for xx in range(int(cx) - 11, int(cx) + 12):
                d2 = (xx - cx) ** 2 + (yy - cy) ** 2
                if 0 <= xx < w and 0 <= yy < h and d2 <= 121 and px[xx, yy][3]:
                    if (d2 <= 49 and (xx + yy) % 2 == 0) or (d2 > 49 and xx % 2 == 0 and yy % 2 == 0):
                        gp[xx, yy] = (255, 70, 40, 255)
        for (x, y) in comp:
            r, g_, b, a = px[x, y]
            d2 = (x - cx) ** 2 + (y - cy) ** 2
            gp[x, y] = (255, 250, 230, 255) if d2 <= 3 else (255, 150, 110, 255) if d2 <= 9 else (255, 50, 36, 255)
            px[x, y] = (int(r * 0.42), int(g_ * 0.5), int(b * 0.5), 255)      # dark on the tunnel itself
    bay.save(f'{theme}/pieces/space/bay.png')
    for i, g in enumerate(groups):
        g.save(f'{theme}/pieces/space/lamps{i}.png')
    Image.new('RGBA', (2, 2), (0, 0, 0, 0)).save(f'{theme}/pieces/space/none.png')
    with open(os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/lamps-rules.txt'), 'w') as f:
        f.write(f'{w} {h}\n')
        for i in range(GROUPS):
            f.write(CYCLES[i].replace('lampsG', f'lamps{i}') + '\n')
    print('lamps found', len(lamps), 'in', GROUPS, 'groups')


if __name__ == '__main__':
    main()
