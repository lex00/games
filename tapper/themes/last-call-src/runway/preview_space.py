"""An offline look at a space room as the rules now build it, to check a piece before the owner looks.

    python3 runway/preview_space.py [SCREEN] [OUT_PNG]

Not the game's own drawing: the galaxy, the board's own picture where the rules keep it
(everything that is not void, a bay cell or a keg cell), then the tunnel at each bay and
a keg at each pod, in the rules' order. Sprites are not drawn.
"""
import os
import re
import sys

from PIL import Image

T = os.path.expanduser('~/.gameprinter/work/themes/tapper-pixel/repo/theme/')
W = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/')
KIT = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/')


def main():
    scr = sys.argv[1] if len(sys.argv) > 1 else 'screen_8021'
    out = sys.argv[2] if len(sys.argv) > 2 else W + 'space-preview.png'
    s = open(T + 'art.txt').read()
    tiles = lambda name: {p[7:] for p in re.search(rf'#\+ room_cells {name} ([^\n]*)\n', s).group(1).split() if p.startswith('tile=')}
    sbay, skeg = tiles('sbay'), tiles('skeg')
    im = Image.open(KIT + scr + '.png').convert('RGB')
    grid = []
    for line in open(W + f'flips-{scr}.txt'):
        p = line.split()
        if len(p) == 33 and p[0].isdigit():
            grid.append([(c[:3], c.endswith('<')) for c in p[1:]])
    stage = Image.open(T + 'pieces/space/stars_far.png').convert('RGBA')
    big = im.resize((1024, 960), Image.NEAREST).convert('RGBA')
    bp = big.load()
    sp = stage.load()
    sbar = tiles('sbar') if 'room_fill sbar sbarpaint' in s else set()
    barpaint = Image.open(T + 'pieces/space/bar.png').convert('RGBA') if sbar else None
    for r in range(30):
        for c in range(32):
            t = grid[r][c][0]
            if t in sbar:
                stage.paste(barpaint.crop((c * 32, r * 32, c * 32 + 32, r * 32 + 32)), (c * 32, r * 32))
                continue
            if t in sbay or t in skeg or t == '000':
                continue
            for y in range(r * 32, r * 32 + 32):
                for x in range(c * 32, c * 32 + 32):
                    if bp[x, y][:3] != (0, 0, 0):
                        sp[x, y] = bp[x, y]
    m = re.search(r'#\+ room_at sbay_at sbaypic (-?\d+) (-?\d+)', s)
    dx, dy = int(m.group(1)), int(m.group(2))
    bay = Image.open(T + 'pieces/space/bay.png').convert('RGBA')
    for r in range(30):
        for c in range(32):
            if grid[r][c][0] == '0bd':
                fl = grid[r][c][1]
                left = c * 16 + 16 - dx - bay.width // 2 if fl else c * 16 + dx
                piece = bay.transpose(Image.FLIP_LEFT_RIGHT) if fl else bay
                stage.paste(piece, (left * 2, (r * 16 + dy) * 2), piece)
    # the bar's first cell, laid after the tunnel
    if 'room_fill sbarin sbarpaint' in s:
        barp = Image.open(T + 'pieces/space/bar.png').convert('RGBA')
        for r in range(30):
            for c in range(32):
                if grid[r][c][0] in ('0f7', '0f6'):
                    stage.paste(barp.crop((c * 32, r * 32, c * 32 + 32, r * 32 + 32)), (c * 32, r * 32))
    m = re.search(r'#\+ room_react skeg_at (-?\d+) (-?\d+) (\w+):', s)
    if m:
        kx, ky = int(m.group(1)), int(m.group(2))
        keg = Image.open(T + f'pieces/space/{m.group(3)}.png').convert('RGBA')
        for r in range(30):
            for c in range(32):
                if grid[r][c][0] == '0d8':
                    x, y = (c * 16 + kx) * 2, (r * 16 + ky) * 2
                    if x >= 0 and y >= 0:
                        stage.alpha_composite(keg, (x, y))
    stage.convert('RGB').save(out)
    print('saved', out)


if __name__ == '__main__':
    main()
