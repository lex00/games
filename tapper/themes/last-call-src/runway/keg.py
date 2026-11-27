"""The punk room's kegs: a steel keg with its copper pipe up to the tap, and its leak, a loop of four pictures.

    python3 runway/keg.py THEME_DIR [PREVIEW_PNG]

Measured on the board's left-wall keg, game pixels: the body stands at x 100..128,
y 138..186; its pipe rises from the middle of its top at x 112..116 to y 99, where the
tap's sprite has its fitting (x 118..124, y 88..102); beer lies at its foot out to x 140.
Everything here is drawn in that left-wall view in a box x 92..144, y 90..192, at twice
the game's pixels, then turned round: the board keeps the right-wall keg and mirrors it
for the left wall, and the room rules mirror a piece with its cell.

  pieces/punk/keg.png        the pipe, its brackets and the keg (from samples/steelkeg16.png)
  pieces/punk/leak0..3.png   the beer: a trickle down the keg's side and the pool at its
                             foot, four pictures of the same loop
"""
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(__file__))
from paintings16 import flood  # noqa: E402

X0, Y0, X1, Y1 = 92, 90, 144, 192
S = 2
O = (38, 24, 18, 255)
C_HI, C_MID, C_LO = (246, 176, 116, 255), (200, 112, 60, 255), (126, 60, 30, 255)
B_HI, B_MID, B_LO, B_DK = (255, 228, 120, 255), (238, 172, 44, 255), (184, 116, 22, 255), (104, 60, 12, 255)
FOAM = (255, 250, 232, 255)


def at(x, y):
    return (round((x - X0) * S), round((y - Y0) * S))


def keg_art(w, h):
    a = np.asarray(Image.open(os.path.join(os.path.dirname(__file__), 'samples/steelkeg16.png')).convert('RGB')).astype(int)
    bg = np.median(a[:30, :30].reshape(-1, 3), axis=0)
    d = np.abs(a - bg).sum(axis=2)
    hh, ww = d.shape
    rim = [(y, x) for y in range(hh) for x in (0, ww - 1)] + [(y, x) for x in range(ww) for y in (0, hh - 1)]
    solid = ~flood(d < 110, rim) & ~(d < 60)          # the backdrop from the rim, and what shows through the rim's hand holes
    ys, xs = np.nonzero(solid)
    a, solid = a[ys.min():ys.max() + 1, xs.min():xs.max() + 1], solid[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    pre = np.dstack([a * solid[..., None], solid * 255.0])
    f = lambda ch: np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((w, h), Image.BOX))
    al = f(pre[..., 3]) / 255
    out = np.zeros((h, w, 4), np.uint8)
    for c in range(3):
        out[..., c] = np.clip(np.where(al > 0, f(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255)
    out[..., 3] = np.where(al >= 0.5, 255, 0)
    return Image.fromarray(out, 'RGBA')


def static():
    im = Image.new('RGBA', ((X1 - X0) * S, (Y1 - Y0) * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    # the copper pipe: up from the keg's top, an elbow, and a short run to the tap's fitting
    px0, px1 = at(112, 0)[0], at(116, 0)[0]
    top, foot = at(0, 97)[1], at(0, 140)[1]
    d.rectangle((px0 - 1, top - 1, px1 + 1, foot), fill=O)
    d.rectangle((px0, top, px1, foot), fill=C_MID)
    d.rectangle((px0, top, px0 + 1, foot), fill=C_HI)
    d.rectangle((px1 - 2, top, px1, foot), fill=C_LO)
    ex = at(123, 0)[0]
    d.rectangle((px0 - 1, top - 9, ex + 1, top + 1), fill=O)
    d.rectangle((px0, top - 8, ex, top), fill=C_MID)
    d.rectangle((px0, top - 8, ex, top - 7), fill=C_HI)
    d.rectangle((px0 + 3, top - 2, ex, top), fill=C_LO)
    # a brass ring at the elbow and two steel brackets holding the pipe to the wall
    d.rectangle((px0 - 2, top - 1, px1 + 2, top + 4), fill=O)
    d.rectangle((px0 - 1, top, px1 + 1, top + 3), fill=(232, 190, 80, 255))
    d.line((px0 - 1, top, px1 + 1, top), fill=(255, 236, 150, 255))
    for gy in (110, 126):
        y = at(0, gy)[1]
        d.rectangle((px0 - 6, y - 1, px1 + 6, y + 5), fill=O)
        d.rectangle((px0 - 5, y, px1 + 5, y + 4), fill=(150, 156, 166, 255))
        d.line((px0 - 5, y, px1 + 5, y), fill=(214, 220, 228, 255))
        for bx in (px0 - 4, px1 + 3):
            d.rectangle((bx, y + 1, bx + 1, y + 2), fill=(70, 74, 84, 255))
    # the keg, a touch larger than the board's
    k = keg_art(31 * S, 51 * S)
    im.alpha_composite(k, at(98.5, 136))
    return im


def leak(n):
    """Picture n of 4: the trickle's drops a quarter further down, the pool's ring a quarter wider."""
    im = Image.new('RGBA', ((X1 - X0) * S, (Y1 - Y0) * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    # the pool at the keg's foot, out toward the bartender
    cx, cy = at(123, 184)
    for grow, col in ((2, O), (0, B_MID)):
        d.ellipse((cx - 30 - grow, cy - 7 - grow, cx + 34 + grow, cy + 9 + grow), fill=col)
        d.ellipse((cx + 14 - grow, cy - 13 - grow, cx + 40 + grow, cy + 1 + grow), fill=col)
    d.ellipse((cx - 26, cy + 1, cx + 30, cy + 8), fill=B_LO)
    d.ellipse((cx - 24, cy - 6, cx + 20, cy - 1), fill=B_HI)
    # a ring spreading from where the trickle lands
    lx, ly = at(129.5, 181)
    r = 3 + n * 3
    d.arc((lx - r * 2, ly - r // 2 - 1, lx + r * 2, ly + r // 2 + 1), 0, 360, fill=B_HI if n < 3 else B_MID)
    # foam at the pool's edge, each fleck a step round with the loop
    for i, (fx, fy) in enumerate(((-24, 2), (-10, 7), (8, 8), (26, 4), (34, -8), (20, -11))):
        if (i + n) % 2 == 0:
            d.rectangle((cx + fx, cy + fy - 1, cx + fx + 3, cy + fy + 1), fill=FOAM)
        else:
            d.rectangle((cx + fx + 1, cy + fy, cx + fx + 2, cy + fy), fill=FOAM)
    # the trickle: out of the lower seam and down the keg's side, with drops running down it
    tx = at(128, 0)[0]
    y0, y1 = at(0, 165)[1], at(0, 181)[1]
    d.rectangle((tx - 1, y0, tx + 2, y1), fill=B_DK)
    d.rectangle((tx, y0, tx + 1, y1), fill=B_MID)
    span = y1 - y0
    for k in range(3):
        yy = y0 + ((k * span // 3) + n * span // 12) % span
        d.rectangle((tx, yy, tx + 1, yy + 3), fill=B_HI)
    d.rectangle((tx - 2, y0 - 2, tx + 3, y0), fill=O)          # the split seam it comes from
    d.rectangle((tx - 1, y0 - 1, tx + 2, y0 - 1), fill=B_LO)
    return im


def main():
    t = sys.argv[1]
    flip = lambda im: im.transpose(Image.FLIP_LEFT_RIGHT)
    st = static()
    flip(st).save(f'{t}/pieces/punk/keg.png')
    frames = [leak(n) for n in range(4)]
    for n, f in enumerate(frames):
        flip(f).save(f'{t}/pieces/punk/leak{n}.png')
    if len(sys.argv) > 2:
        w, h = st.size
        sheet = Image.new('RGBA', (w * 4 * 3, h * 3), (96, 100, 108, 255))
        for n, f in enumerate(frames):
            cell = Image.new('RGBA', (w, h), (0, 0, 0, 0))
            cell.alpha_composite(st)
            cell.alpha_composite(f)
            sheet.alpha_composite(cell.resize((w * 3, h * 3), Image.NEAREST), (n * w * 3, 0))
        sheet.convert('RGB').save(sys.argv[2])
    print('keg', st.size, 'box game', X0, Y0, X1, Y1)


if __name__ == '__main__':
    main()
