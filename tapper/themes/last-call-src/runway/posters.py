"""The punk room's two posters, laid on the left wall where the board has its long window.

    python3 runway/posters.py THEME_DIR [LEFT_NAME RIGHT_NAME]

The board draws the left wall's upright lines upright and its depth as a line that falls
3 down for each 1 to the left, so the window is a band between two such lines, 124 game
pixels high at any column: its top edge runs from (60, 32) down to the screen's left, its
bottom edge 124 below. A poster on that wall keeps its columns upright and slides each one
3 down per pixel to the left.

The piece is the whole band at twice the game's pixels: the wall's own steel over every
pixel the board's window had (so none of it shows), and the two posters on top. The pixels
are read from the board picture in the local kit and only say where to paint steel; no
colour of the board's goes into the piece.
"""
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
from paintings16 import flood  # noqa: E402

KIT = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/')
GREYS = {(146, 146, 146), (109, 109, 109), (73, 73, 73)}
X0, Y0, X1, Y1 = 0, 24, 68, 330            # the piece's box, game pixels
HIGH = 124                                  # the band's height at any column


def top(x):
    return 32 + 3 * (60 - x)


def window_mask():
    """Where the board's window has a pixel of its own, by game pixel, in the piece's box."""
    m = np.zeros((Y1 - Y0, X1 - X0), bool)
    for s in ('screen_7121', 'screen_78a1'):
        px = Image.open(KIT + s + '.png').convert('RGB').load()
        for y in range(Y0, Y1):
            for x in range(X0, min(X1, 64)):
                if px[x, y] not in GREYS and top(x) - 3 <= y <= top(x) + HIGH + 6:
                    m[y - Y0, x - X0] = True
    return m


def poster(sample, half, w, h):
    """One poster cut from its half of the sheet, at w by h stage pixels."""
    a = np.asarray(Image.open(sample).convert('RGB')).astype(int)
    if half is not None:
        a = a[:, :a.shape[1] // 2] if half == 0 else a[:, a.shape[1] // 2:]
    bg = np.median(a[:30, :30].reshape(-1, 3), axis=0)
    near = np.abs(a - bg).sum(axis=2) < 90
    hh, ww = near.shape
    rim = [(y, x) for y in range(hh) for x in (0, ww - 1)] + [(y, x) for x in range(ww) for y in (0, hh - 1)]
    solid = ~flood(near, rim)
    ys, xs = np.nonzero(solid)
    a, solid = a[ys.min():ys.max() + 1, xs.min():xs.max() + 1], solid[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    pre = np.dstack([a * solid[..., None], solid * 255.0])
    f = lambda ch: np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((w, h), Image.BOX))
    al = f(pre[..., 3]) / 255
    out = np.zeros((h, w, 4), np.uint8)
    for c in range(3):
        out[..., c] = np.clip(np.where(al > 0, f(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255)
    out[..., 3] = np.where(al >= 0.5, 255, 0)
    return out


def main():
    theme = sys.argv[1]
    sample = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(__file__), 'samples/posters16.png')
    W, H = (X1 - X0) * 2, (Y1 - Y0) * 2
    piece = np.zeros((H, W, 4), np.uint8)
    steel = np.asarray(Image.open(f'{theme}/pieces/punk/steel.png').convert('RGB'))
    m = np.repeat(np.repeat(window_mask(), 2, axis=0), 2, axis=1)
    piece[..., :3][m] = steel[Y0 * 2:Y1 * 2, X0 * 2:X1 * 2][m]
    piece[..., 3][m] = 255
    # One tall banner fills the window's own frame: the band between its two leaning edges, 42 game pixels across at
    # every row, from the frame's top to its foot, cut where the frame is cut (its upright right edge, the screen's
    # left). The banner's rows stay level and each is 1 further left for every 3 down, the wall's own line.
    sample = os.path.join(os.path.dirname(__file__), 'samples/banner16.png')
    YA, YB, BW = 34, 316, 42
    p = poster(sample, None, BW * 2, (YB - YA) * 2)
    for j in range((YB - YA) * 2):
        gy = YA + j / 2.0
        x = int(round((60 - (gy - 32) / 3.0 - X0) * 2))
        for i in range(BW * 2):
            X = x + i
            if 0 <= X < 64 * 2 and p[j, i, 3]:
                piece[int((YA - Y0) * 2) + j, X] = p[j, i]
    Image.fromarray(piece, 'RGBA').save(f'{theme}/pieces/punk/posters.png')
    print('posters', W, H, 'at game', X0, Y0)


if __name__ == '__main__':
    main()
