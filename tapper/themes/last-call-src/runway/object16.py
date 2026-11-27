"""One generated object on a flat magenta backdrop, cut out and brought to size for a wall.

    python3 runway/object16.py SAMPLE OUT.png WIDTH HEIGHT [flip] [lean=K]

SAMPLE is a name in samples/ (buck16, lamp16). The backdrop is cleared by a
flood from the picture's edge, and any magenta caught inside the object or
left on its rim goes too. The object is cut to its own box and brought down
to WIDTH by HEIGHT (stage pixels, twice the game's) by area averaging with a
hard edge: a pixel is solid when half its area is object and takes the mean
colour of that part only. `flip` mirrors it left to right. `lean=K` leans it
for a wall seen from the side: each column K pixels lower than the one to
its left (K below 0 leans it the other way).
"""
import os
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from paintings16 import flood  # noqa: E402


def cut(sample):
    a = np.asarray(Image.open(f'{HERE}/samples/{sample}.png').convert('RGB')).astype(int)
    h, w = a.shape[:2]
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    mag = (r > 170) & (b > 150) & (g < 120) & (r - g > 90)
    rim = [(y, x) for y in range(h) for x in (0, w - 1)] + [(y, x) for x in range(w) for y in (0, h - 1)]
    solid = ~flood(mag, rim) & ~mag
    ys, xs = np.nonzero(solid)
    x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
    return np.dstack([a[y0:y1, x0:x1].astype(np.uint8), (solid[y0:y1, x0:x1] * 255).astype(np.uint8)])


def down(rgba, ow, oh):
    pre = rgba.astype(np.float64)
    pre[..., :3] *= pre[..., 3:4] / 255.0
    f = lambda ch: np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((ow, oh), Image.BOX))
    al = f(pre[..., 3]) / 255.0
    o = np.zeros((oh, ow, 4), np.uint8)
    for c in range(3):
        o[..., c] = np.clip(np.where(al > 0, f(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255).astype(np.uint8)
    o[..., 3] = np.where(al >= 0.5, 255, 0)
    # the backdrop's pink left on an edge
    pink = (o[..., 3] > 0) & (o[..., 2].astype(int) > o[..., 1].astype(int) + 25) & (o[..., 0].astype(int) > o[..., 1].astype(int) + 40)
    o[pink] = 0
    o[o[..., 3] == 0] = 0
    return o


def main():
    sample, out, w, h = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
    opts = sys.argv[5:]
    o = down(cut(sample), w, h)
    if 'flip' in opts:
        o = o[:, ::-1]
    k = next((float(x[5:]) for x in opts if x.startswith('lean=')), 0.0)
    if k:
        rise = int(round(abs(k) * (w - 1)))
        leant = np.zeros((h + rise, w, 4), np.uint8)
        for x in range(w):
            dy = int(round(abs(k) * (x if k > 0 else (w - 1 - x))))
            leant[dy:dy + h, x] = o[:, x]
        o = leant
    Image.fromarray(o, 'RGBA').save(out)
    print(out, o.shape[1], o.shape[0])


if __name__ == '__main__':
    main()
