"""The paintings seen front on, for a wall that faces the player (the bonus room).

    python3 runway/paintings_front.py OUT_DIR [PICTURE.png ...]

One rule for every painting, so a new one comes out like the others and
like its leant view on the bars' wall (build.py's `hang`):
- the frame is the source's own ornate frame, kept whole: its top and
  bottom bars with all four corner ornaments as drawn, and its side bars
  cut shorter. Nothing of it is stretched or sliced through an ornament;
- the picture is a window of the source's picture, a little below the
  middle, as tall as the shorter frame leaves;
- it is drawn at twice the source's pixels (never smoothed).
Each comes out 500 by 348 (250 by 174 game pixels).

With no pictures named it makes the theme's own 13: the room's picture from
samples/pieces.png (wide00) and the twelve of samples/paintings.png (wide01
to wide12, in the order build.py numbers them). A picture named on the
command line is one framed painting on a plain backdrop, as the samples
are; it comes out as wideNN after the last one in OUT_DIR.
"""
import os
import sys
from collections import deque

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from build_lib import crop, load, objects  # noqa: E402

OUT_W, OUT_H = 500, 348


def gold(p):
    return p[0] > 140 and p[1] > 85 and p[2] < 125 and int(p[0]) > int(p[2]) + 60


def frame_mask(a, m):
    """The source's frame: every gold or dark pixel reached from the
    painting's rim without leaving the rim's band (a fifth of its width)."""
    h, w = m.shape
    band = int(w * 0.2)
    ok = np.zeros((h, w), bool)
    for y in range(h):
        for x in range(w):
            if not m[y, x]:
                continue
            if not (x < band or x >= w - band or y < band or y >= h - band):
                continue
            p = a[y, x]
            ok[y, x] = gold(p) or max(p) < 90
    seen = np.zeros((h, w), bool)
    q = deque()
    for y in range(h):
        for x in range(w):
            rim = m[y, x] and (x == 0 or y == 0 or x == w - 1 or y == h - 1
                               or not m[y, x - 1] or not m[y, x + 1] or not m[y - 1, x] or not m[y + 1, x])
            if rim and ok[y, x]:
                seen[y, x] = True
                q.append((y, x))
    while q:
        y, x = q.popleft()
        for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            yy, xx = y + dy, x + dx
            if 0 <= yy < h and 0 <= xx < w and ok[yy, xx] and not seen[yy, xx]:
                seen[yy, xx] = True
                q.append((yy, xx))
    return seen


def front(a, m):
    a = np.clip(a[:, :, :3], 0, 255).astype(np.uint8)
    m = np.asarray(m) > 0
    h, w = m.shape
    f = frame_mask(a, m)
    hout = int(round(OUT_H * w / OUT_W))
    corner = int(w * 0.2)                 # rows that hold a corner ornament
    mid = hout - 2 * corner               # rows between them: side bars and the picture's window
    assert mid > 20, (w, h, hout)
    # the picture's window: source rows for every output row, as one run
    span = h - 2 * corner
    ys = corner + int(round((span - mid) * 0.55))     # the middle rows' first source row
    win_row = lambda r: ys + (r - corner)             # the window, carried on under the corners too
    out = np.zeros((hout, w, 4), np.uint8)
    src = [r if r < corner else (h - (hout - r) if r >= hout - corner else win_row(r)) for r in range(hout)]
    fo = np.array([f[fr] for fr in src])              # the frame, as the output has it
    # the picture shows only inside the frame: frame to its left and right, above and below
    left = np.maximum.accumulate(fo, axis=1); right = np.maximum.accumulate(fo[:, ::-1], axis=1)[:, ::-1]
    up = np.maximum.accumulate(fo, axis=0); down = np.maximum.accumulate(fo[::-1], axis=0)[::-1]
    hole = left & right & up & down & ~fo
    for r in range(hout):
        fr = src[r]
        pr = min(h - 1, max(0, win_row(r)))
        for x in range(w):
            if fo[r, x]:
                out[r, x] = (*a[fr, x], 255)
            elif hole[r, x]:
                out[r, x] = (*a[pr, x], 255)
    im = Image.fromarray(out, 'RGBA')
    px = im.load()
    for x in range(corner, w - corner):   # a stray white line down a source picture
        c = [px[x, y] for y in range(corner, hout - corner)]
        if sum(1 for p in c if min(p[:3]) > 215) > len(c) * 0.6:
            for y in range(hout):
                if min(px[x, y][:3]) > 200:
                    px[x, y] = px[x - 1, y]
    return im.resize((OUT_W, OUT_H), Image.NEAREST)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    if len(sys.argv) > 2:
        have = [int(f[4:6]) for f in os.listdir(out) if f.startswith('wide') and f[4:6].isdigit()]
        n = max(have, default=-1) + 1
        for path in sys.argv[2:]:
            im = Image.open(path).convert('RGBA')
            a = np.asarray(im.convert('RGB')).astype(float)
            m = np.asarray(im)[:, :, 3] > 0
            if m.all():                   # no alpha: the backdrop is the corner's colour
                bg = a[0, 0]
                m = np.abs(a - bg).sum(axis=2) > 60
            front(a, m).save(f'{out}/wide{n:02d}.png')
            print(f'wide{n:02d}', path)
            n += 1
        return
    a, m = load('pieces')
    pic = [crop(a, m, s) for s in objects(a, m)][3]
    front(*pic).save(f'{out}/wide00.png')
    pa, pm = load('paintings')
    found = sorted(objects(pa, pm), key=lambda sl: (sl[0].start // 200, sl[1].start))
    assert len(found) == 12, len(found)
    for k, sl in enumerate(found):
        front(*crop(pa, pm, sl)).save(f'{out}/wide{k + 1:02d}.png')
    print('13 paintings in', out)


if __name__ == '__main__':
    main()
