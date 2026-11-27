"""Every empty mug a clear glass: the frosted inside made faint, the walls, rim, base and handle kept.

    python3 runway/clear_mugs.py THEME_DIR FROM_DIR

Reads each mug picture from FROM_DIR (the pictures as they were, frosted) and writes it clear into
THEME_DIR/art/obj. The inside of the glass is found by colour: pale, grey-white pixels at least two pixels in from
anything that is not glass (the outline, the drink, the air). Those are drawn faint, so the room shows through,
with a sheen and two dark facet lines across the glass so an empty mug still reads as a mug. A broken mug's shards stay whole pieces, a little see-through.
"""
import os
import sys

import numpy as np
from PIL import Image

TINT = 0.16                      # how much of the glass's own pale colour is left in its inside
# bands across the glass: from, to (0 to 1 from wall to wall), colour, how solid (0 to 255)
SHEEN = [(0.14, 0.30, (255, 255, 255), 150), (0.30, 0.34, (255, 255, 255), 70),
         (0.48, 0.52, (58, 66, 84), 120), (0.70, 0.74, (58, 66, 84), 120), (0.84, 0.90, (255, 255, 255), 110)]
WHOLE = ['mug-17', 'mug-10', 'mug-24', 'mug-59', 'mug-14']
SHARDS = ['mug-11', 'mug-12', 'mug-13']


def grow(m):
    e = m.copy()
    e[1:, :] |= m[:-1, :]; e[:-1, :] |= m[1:, :]; e[:, 1:] |= m[:, :-1]; e[:, :-1] |= m[:, 1:]
    return e


def shrink(m):
    e = m.copy()
    e[1:, :] &= m[:-1, :]; e[:-1, :] &= m[1:, :]; e[:, 1:] &= m[:, :-1]; e[:, :-1] &= m[:, 1:]
    e[0, :] = e[-1, :] = False; e[:, 0] = e[:, -1] = False
    return e


def clear(path, depth):
    a = np.asarray(Image.open(path).convert('RGBA')).copy()
    rgb = a[..., :3].astype(int)
    solid = a[..., 3] > 0
    pale = solid & (rgb.min(axis=2) > 135) & (rgb.max(axis=2) - rgb.min(axis=2) < 60)
    if depth == 0:                      # shards: the whole piece of glass a little see-through, its edge kept
        inner = shrink(pale)
        a[inner, 3] = (a[inner, 3] * 0.45).astype(np.uint8)
        return Image.fromarray(a, 'RGBA'), int(inner.sum())
    # specks inside the glass (a last drop, a streak of shade) count as glass, so the inside has a smooth edge
    body = shrink(shrink(grow(grow(pale)))) & solid
    inner = body
    for _ in range(depth):
        inner = shrink(inner)
    inner = shrink(grow(inner))         # no one-pixel spurs
    glass = inner & pale                # a drop left in the glass stays, as a drop
    a[glass, 3] = (a[glass, 3] * TINT).astype(np.uint8)
    # across the glass, by its own lie (a tilted mug's lines tilt with it): a broad sheen left of the middle, a thin
    # one near the far wall, and two dark lines for the glass's facets, as the board's own mug has its lines
    ys, xs = np.nonzero(glass)
    if len(xs) > 30:
        pts = np.stack([xs - xs.mean(), ys - ys.mean()])
        w, v = np.linalg.eigh(np.cov(pts))
        along = v[:, np.argmax(w)]
        if abs(along[1]) < abs(along[0]) and (ys.max() - ys.min()) > 0.7 * (xs.max() - xs.min()):
            along = np.array([0.0, 1.0])            # an upright mug that is about as wide as tall
        across = np.array([-along[1], along[0]])
        if across[0] < 0:
            across = -across
        u = pts[0] * across[0] + pts[1] * across[1]
        t = (u - u.min()) / max(1e-6, u.max() - u.min())       # 0 at one wall, 1 at the other
        for (lo, hi, rgb, alpha) in SHEEN:
            m = (t >= lo) & (t < hi)
            a[ys[m], xs[m], :3] = rgb
            a[ys[m], xs[m], 3] = alpha
    return Image.fromarray(a, 'RGBA'), int(glass.sum())


def main():
    theme, src = sys.argv[1], sys.argv[2]
    for names, depth in ((WHOLE, 2), (SHARDS, 0)):
        for n in names:
            im, k = clear(f'{src}/{n}.png', depth)
            im.save(f'{theme}/art/obj/{n}.png')
            print(n, k, 'pixels made clear')


if __name__ == '__main__':
    main()
