"""The bartender's hover craft: Runway's three frames (samples/hovercraft16.png) cut to pieces, and tied to his legs.

    python3 runway/craft.py THEME_DIR

Writes pieces/space/craft0..2.png, the same craft with its jets short, medium and long,
88 stage pixels wide (44 game pixels), each as tall as the longest so the craft stands at
the same place in all three. Then points the bartender's `also` lines (runway/ride.py's)
at the middle frame, its deck's top 4 game pixels above the foot of his legs' picture.
"""
import os
import re
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
from paintings16 import flood  # noqa: E402

GW = 44


def main():
    theme = sys.argv[1]
    a = np.asarray(Image.open(os.path.join(os.path.dirname(__file__), 'samples/hovercraft16.png')).convert('RGB')).astype(int)
    bg = np.median(a[:30, :30].reshape(-1, 3), axis=0)
    d = np.abs(a - bg).sum(axis=2)
    h, w = d.shape
    rim = [(y, x) for y in range(h) for x in (0, w - 1)] + [(y, x) for x in range(w) for y in (0, h - 1)]
    solid = ~flood(d < 110, rim) & ~(d < 70)
    cells = []
    for i in range(3):
        m = solid[:, i * w // 3:(i + 1) * w // 3]
        ys, xs = np.nonzero(m)
        cells.append((i * w // 3 + xs.min(), ys.min(), i * w // 3 + xs.max() + 1, ys.max() + 1))
    kw = max(c[2] - c[0] for c in cells)
    top = min(c[1] for c in cells)
    bot = max(c[3] for c in cells)
    k = GW * 2 / kw
    ow, oh = GW * 2, round((bot - top) * k)
    for i, c in enumerate(cells):
        cx = (c[0] + c[2]) // 2
        x0 = cx - kw // 2
        sub, sm = a[top:bot, x0:x0 + kw], solid[top:bot, x0:x0 + kw]
        pre = np.dstack([sub * sm[..., None], sm * 255.0])
        f = lambda ch: np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((ow, oh), Image.BOX))
        al = f(pre[..., 3]) / 255
        o = np.zeros((oh, ow, 4), np.uint8)
        for ch in range(3):
            o[..., ch] = np.clip(np.where(al > 0, f(pre[..., ch]) / np.maximum(al, 1e-6), 0), 0, 255)
        o[..., 3] = np.where(al >= 0.5, 255, 0)
        Image.fromarray(o, 'RGBA').save(f'{theme}/pieces/space/craft{i}.png')
    p = f'{theme}/art.txt'
    s = open(p).read()
    s, n = re.subn(r'(#\+ on screen_8021[^\n]* also sprite-(?:bartender_part|player)-[0-9a-f]+) pieces/space/\w+\.png \d+ \d+ -?\d+ -?\d+\n', rf'\1 pieces/space/craft1.png {GW} {oh // 2} {(32 - GW) // 2} 26\n', s)
    s = s.replace('# The disc rides with the bartender on the space screens: drawn with each of his pictures, a little below his feet.\n', '# A small round hover craft carries the bartender on the space screens: drawn with his legs, its deck at his feet.\n')
    open(p, 'w').write(s)
    print('craft frames', ow, oh, 'lines pointed at it:', n)


if __name__ == '__main__':
    main()
