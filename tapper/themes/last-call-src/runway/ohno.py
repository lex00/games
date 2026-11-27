"""The trophies' fright, from Runway's shocked faces: bigger than a change of the eyes.

    python3 runway/ohno.py THEME_DIR [PREVIEW_PNG]

samples/jackalope16-ohno.png and samples/moose16-ohno.png are the same trophies with
their mouths open and their eyes wide. Each is cut out and brought to the size its plain
trophy has in the theme (pieces/saloon/buck_right.png, moose.png), at the scale the plain
one was cut from its own sample, and set in a canvas of the plain piece's size with its
plaque's lower right corner where the plain one's is, so the plaque does not move when
the face changes. If the shocked one stands taller than the canvas it is made a little
smaller to fit. Written over pieces/saloon/buck_right_oh.png and moose_oh.png.
"""
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
from paintings16 import flood  # noqa: E402

HERE = os.path.dirname(__file__)


def cut(name):
    a = np.asarray(Image.open(os.path.join(HERE, 'samples', name)).convert('RGB')).astype(int)
    bg = np.median(a[:30, :30].reshape(-1, 3), axis=0)
    d = np.abs(a - bg).sum(axis=2)
    h, w = d.shape
    rim = [(y, x) for y in range(h) for x in (0, w - 1)] + [(y, x) for x in range(w) for y in (0, h - 1)]
    solid = ~flood(d < 120, rim) & ~(d < 80)             # and the backdrop caught between antlers and ears
    ys, xs = np.nonzero(solid)
    return a[ys.min():ys.max() + 1, xs.min():xs.max() + 1], solid[ys.min():ys.max() + 1, xs.min():xs.max() + 1]


def down(a, solid, w, h):
    pre = np.dstack([a * solid[..., None], solid * 255.0])
    f = lambda ch: np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((w, h), Image.BOX))
    al = f(pre[..., 3]) / 255
    o = np.zeros((h, w, 4), np.uint8)
    for c in range(3):
        o[..., c] = np.clip(np.where(al > 0, f(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255)
    o[..., 3] = np.where(al >= 0.5, 255, 0)
    return Image.fromarray(o, 'RGBA')


def main():
    theme = sys.argv[1]
    sheet = []
    # The moose hangs on the left wall, so the theme's moose is the sample turned round, its plaque on its left.
    for plain_sample, oh_sample, piece, turned in (('jackalope16.png', 'jackalope16-ohno.png', 'buck_right', False), ('moose16.png', 'moose16-ohno.png', 'moose', True)):
        base = Image.open(f'{theme}/pieces/saloon/{piece}.png').convert('RGBA')
        pa, _ = cut(plain_sample)
        oa, om = cut(oh_sample)
        k = base.height / pa.shape[0]                       # the scale the plain one was cut at
        w, h = round(oa.shape[1] * k), round(oa.shape[0] * k)
        if h > base.height or w > base.width:
            f = min(base.height / h, base.width / w)
            w, h = int(w * f), int(h * f)
        oh = down(oa, om, w, h)
        out = Image.new('RGBA', base.size, (0, 0, 0, 0))
        if turned:
            oh = oh.transpose(Image.FLIP_LEFT_RIGHT)
            out.alpha_composite(oh, (0, base.height - h))              # the plaque's lower left corner stays put
        else:
            out.alpha_composite(oh, (base.width - w, base.height - h))     # the plaque's lower right corner stays put
        out.save(f'{theme}/pieces/saloon/{piece}_oh.png')
        sheet += [base, out]
        print(piece, 'plain', base.size, 'shocked', (w, h))
    if len(sys.argv) > 2:
        wd = sum(s.width for s in sheet) + 10 * len(sheet)
        o = Image.new('RGBA', (wd, max(s.height for s in sheet)), (40, 110, 200, 255))
        x = 0
        for s in sheet:
            o.alpha_composite(s, (x, 0))
            x += s.width + 10
        o.convert('RGB').save(sys.argv[2])


if __name__ == '__main__':
    main()
