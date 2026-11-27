"""The tunnel piece cut from Runway's picture (samples/bay16b.png) at the size the room rules place it, 272 by 264.

    python3 runway/refit_bay.py OUT_PNG
"""
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
from paintings16 import flood  # noqa: E402


def main():
    a = np.asarray(Image.open(os.path.join(os.path.dirname(__file__), 'samples/bay16b.png')).convert('RGB')).astype(int)
    bg = np.median(a[:30, :30].reshape(-1, 3), axis=0)
    d = np.abs(a - bg).sum(axis=2)
    h, w = d.shape
    rim = [(y, x) for y in range(h) for x in (0, w - 1)] + [(y, x) for x in range(w) for y in (0, h - 1)]
    solid = ~flood(d < 110, rim)
    ys, xs = np.nonzero(solid)
    a, solid = a[ys.min():ys.max() + 1, xs.min():xs.max() + 1], solid[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    ow, oh = 272, 264
    pre = np.dstack([a * solid[..., None], solid * 255.0])
    f = lambda ch: np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((ow, oh), Image.BOX))
    al = f(pre[..., 3]) / 255
    o = np.zeros((oh, ow, 4), np.uint8)
    for c in range(3):
        o[..., c] = np.clip(np.where(al > 0, f(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255)
    o[..., 3] = np.where(al >= 0.5, 255, 0)
    Image.fromarray(o, 'RGBA').save(sys.argv[1])
    print('tunnel piece', ow, oh)


if __name__ == '__main__':
    main()
