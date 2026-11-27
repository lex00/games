"""The space room's bars: Runway's metal counter (samples/spacebar16.png) as one painting the height of the screen.

    python3 runway/spacebar.py THEME_DIR

The crop sent to Runway was the bar's slab, game rows 110 to 146, at 15 picture pixels a
game pixel from row 370 of the picture; the counter came back on rows 395 to 890. Those
rows are the bar's own two cells (game rows 112 to 144, then every 96 down), so they are
brought to 64 stage pixels and laid at each bar's height, the picture's width repeated
across the screen with every other copy turned round so the joins meet.
"""
import sys

import numpy as np
from PIL import Image

import os

W, H = 1024, 960
TOP = 40


def main():
    theme = sys.argv[1]
    src = Image.open(os.path.join(os.path.dirname(__file__), 'samples/spacebar16.png')).convert('RGB')
    a = np.asarray(src).astype(int)
    bg = np.median(a[:20, :20].reshape(-1, 3), axis=0)
    rows = np.nonzero((np.abs(a - bg).sum(axis=2) > 120).mean(axis=1) > 0.9)[0]
    y0, y1 = int(rows.min()), int(rows.max()) + 1
    band = src.crop((0, y0, src.width, y1))
    # The bartop is given more of the bar's height than Runway drew it (40 of its 64 stage pixels, not 24), so what
    # the bartender wipes is bartop: the picture's upper part (the slab, its edging and the lit strip, down to the
    # shadow line) and its lower part (the shadow and the front) are brought to size apart.
    k = 64.0 / band.height
    cut = round(band.height * 0.40)
    wide = round(band.width * k)
    upper = band.crop((0, 0, band.width, cut)).resize((wide, TOP), Image.BOX)
    lower = band.crop((0, cut, band.width, band.height)).resize((wide, 64 - TOP), Image.BOX)
    band = Image.new('RGB', (wide, 64))
    band.paste(upper, (0, 0))
    band.paste(lower, (0, TOP))
    out = Image.new('RGB', (W, H), (10, 10, 16))
    x, i = 0, 0
    row = Image.new('RGB', (W, 64))
    while x < W:
        row.paste(band if i % 2 == 0 else band.transpose(Image.FLIP_LEFT_RIGHT), (x, 0))
        x += band.width
        i += 1
    for kbar in range(-1, 5):
        y = 224 + 192 * kbar
        if -64 < y < H:
            out.paste(row, (0, y))
    out.save(f'{theme}/pieces/space/bar.png')
    print('bar rows', y0, y1, 'copy width', band.width)


if __name__ == '__main__':
    main()
