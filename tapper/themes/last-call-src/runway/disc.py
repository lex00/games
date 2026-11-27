"""The bartender's pad as a hovering disc of neon rings, and the pylons gone.

    python3 runway/disc.py THEME_DIR [PREVIEW_PNG]

Cuts Runway's disc (samples/disc16.png): everything the backdrop's colour goes, the gaps
between the rings too, so only the rings are left and the galaxy shows between them.
It is brought to the pad's own width (the board's pad is 48 game pixels wide in a block
of 3 by 2 cells) and saved as pieces/space/disc.png, 96 by 64 stage pixels, the disc in
the block's upper part where the board's oval is.
Also writes pieces/space/t0ee.png: the rail's own rows, for the cell where a pylon met
the rail, so the rail runs on and the pylon's joint is gone.
Then writes the rules: the pad's six tiles and the two pylon tiles take the galaxy, the
disc is placed by the pad's top-left tile (mirrored with it), the joint cell takes the
rail.
"""
import os
import re
import sys

import numpy as np
from PIL import Image

KIT = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/')


def main():
    theme = sys.argv[1]
    a = np.asarray(Image.open(os.path.join(os.path.dirname(__file__), 'samples/disc16.png')).convert('RGB')).astype(int)
    bg = np.median(a[:30, :30].reshape(-1, 3), axis=0)
    d = np.abs(a - bg).sum(axis=2)
    solid = d > 150                                   # anything near the backdrop's colour is empty, wherever it is
    ys, xs = np.nonzero(solid)
    a, solid = a[ys.min():ys.max() + 1, xs.min():xs.max() + 1], solid[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    ow = 100
    oh = 46          # the board's oval is about half as high as it is wide; Runway's came flatter, and thin rings are lost at this size
    pre = np.dstack([a * solid[..., None], solid * 255.0])
    f = lambda ch: np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((ow, oh), Image.BOX))
    al = f(pre[..., 3]) / 255
    o = np.zeros((oh, ow, 4), np.uint8)
    for c in range(3):
        o[..., c] = np.clip(np.where(al > 0, f(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255)
    o[..., 3] = np.where(al >= 0.42, 255, 0)
    disc = Image.fromarray(o, 'RGBA')
    piece = Image.new('RGBA', (104, 72), (0, 0, 0, 0))
    piece.alpha_composite(disc, (2, max(0, 34 - oh // 2)))            # the board's oval has its middle 17 game pixels down its block
    piece.save(f'{theme}/pieces/space/disc.png')
    # the rail's rows, from the board's rail tile, only for where they are (which rows of the cell)
    im = Image.open(KIT + 'screen_8021.png').convert('RGB')
    rail = im.crop((8 * 16, 9 * 16, 9 * 16, 10 * 16)).resize((32, 32), Image.NEAREST).convert('RGBA')
    rp = rail.load()
    for y in range(32):
        for x in range(32):
            if rp[x, y][:3] == (0, 0, 0):
                rp[x, y] = (0, 0, 0, 0)
    rail.save(f'{theme}/pieces/space/t0ee.png')
    p = f'{theme}/art.txt'
    s = open(p).read()
    if 'sdisc' not in s:
        k = s.index('stage_piece stars_far pieces/space/stars_far.png')
        s = s[:k] + 'stage_piece sdisc pieces/space/disc.png 104 72\nstage_piece st0ee pieces/space/t0ee.png 32 32\n' + s[k:]
        m = re.search(r'#\+ room_cells srest ([^\n]*)\n', s)
        s = s.replace(m.group(0), '#+ room_cells spad tile=0x0e8 tile=0x0e5 tile=0x0e4 tile=0x0e7 tile=0x0e6 tile=0x0e3\n#+ room_cells spad_at tile=0x0e8\n#+ room_cells spylon tile=0x0e9\n#+ room_cells sjoint tile=0x0ee\n' + f'#+ room_cells srest {m.group(1)} !spad !spylon !sjoint\n')
        s = s.replace('#+ room_fill skeg stars_far\n', "#+ room_fill skeg stars_far\n# The bartender's pad and the pylons: the galaxy where the board has them. Nothing upright holds anything up here;\n# where a pylon met the rail, the rail runs on.\n#+ room_fill spad stars_far\n#+ room_fill spylon stars_far\n#+ room_fill sjoint stars_far\n#+ room_fill sjoint st0ee\n")
        k = s.index('# The floating kegs:')
        s = s[:k] + "# The bartender's pad: a hovering disc of neon rings with nothing between them, so the galaxy shows through, placed\n# by the pad's top-left cell and mirrored with it.\n#+ room_at spad_at sdisc -2 -2\n" + s[k:]
        open(p, 'w').write(s)
    print('disc', disc.size)
    if len(sys.argv) > 2:
        gal = Image.open(f'{theme}/pieces/space/stars_far.png').convert('RGBA').crop((400, 250, 608, 394))
        gal.alpha_composite(piece, (52, 36))
        gal.convert('RGB').resize((832, 576), Image.NEAREST).save(sys.argv[2])


if __name__ == '__main__':
    main()
