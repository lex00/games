"""The paintings, 16-bit: one frame for all, each picture set in it, front on.

    python3 runway/paintings16.py OUT_DIR [--leant DIR] NAME[:LEANT]=PICTURE.png ...

- samples/frame16.png is the one frame: carved gold on a pale blue backdrop,
  its inside flat magenta. The backdrop is cleared by a flood from the
  picture's edge, the inside by a flood from its middle.
- each PICTURE is a painting that fills its whole picture, no frame. It is
  cut to the frame's inside (its middle kept) and laid under the frame.
- the whole is brought down to 500 wide by area averaging, the edge kept
  hard: a pixel is solid when half its area is frame or picture, and takes
  the mean colour of that part only. Never smoothed, never a glow.
Each comes out as OUT_DIR/NAME.png, 500 wide (250 game pixels).
"""
import os
import sys
from collections import deque

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
OUT_W = 500


def flood(ok, seeds):
    h, w = ok.shape
    seen = np.zeros((h, w), bool)
    q = deque()
    for y, x in seeds:
        if ok[y, x] and not seen[y, x]:
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


def the_frame():
    a = np.asarray(Image.open(f'{HERE}/samples/frame16.png').convert('RGB')).astype(int)
    h, w = a.shape[:2]
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    blue = (b > 200) & (g > 150) & (r < 170) & (b > r + 60)
    magenta = (r > 180) & (b > 140) & (g < 110)
    rim = [(y, x) for y in range(h) for x in (0, w - 1)] + [(y, x) for x in range(w) for y in (0, h - 1)]
    out = flood(blue, rim)
    inside = flood(magenta, [(h // 2, w // 2)])
    ys, xs = np.nonzero(~out)
    box = (xs.min(), ys.min(), xs.max() + 1, ys.max() + 1)
    iy, ix = np.nonzero(inside)
    hole = (ix.min(), iy.min(), ix.max() + 1, iy.max() + 1)
    return a.astype(np.uint8), ~out & ~inside, box, hole


def make(picture, frame):
    a, solid, box, hole = frame
    x0, y0, x1, y1 = box
    hx0, hy0, hx1, hy1 = hole
    # the picture, cut to the hole's shape about its middle, a few pixels larger so no gap shows at the frame
    pad = 6
    hw, hh = hx1 - hx0 + 2 * pad, hy1 - hy0 + 2 * pad
    p = Image.open(picture).convert('RGB')
    s = max(hw / p.width, hh / p.height)
    p = p.resize((int(round(p.width * s)), int(round(p.height * s))), Image.LANCZOS if s < 1 else Image.NEAREST)
    cx, cy = (p.width - hw) // 2, (p.height - hh) // 2
    p = np.asarray(p.crop((cx, cy, cx + hw, cy + hh)))
    full = np.zeros((a.shape[0], a.shape[1], 4), np.float64)
    full[hy0 - pad:hy1 + pad, hx0 - pad:hx1 + pad, :3] = p
    full[hy0 - pad:hy1 + pad, hx0 - pad:hx1 + pad, 3] = 1
    full[solid, :3] = a[solid]
    full[solid, 3] = 1
    full = full[y0:y1, x0:x1]
    # down by area averaging, colour weighted by cover
    h, w = full.shape[:2]
    ow = OUT_W
    oh = int(round(h * ow / w))
    pre = full.copy()
    pre[..., :3] *= pre[..., 3:4]
    def box_down(ch):
        return np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((ow, oh), Image.BOX))
    al = box_down(pre[..., 3])
    out = np.zeros((oh, ow, 4), np.uint8)
    for c in range(3):
        out[..., c] = np.clip(np.where(al > 0, box_down(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255).astype(np.uint8)
    out[..., 3] = np.where(al >= 0.5, 255, 0)
    out[out[..., 3] == 0] = 0
    return Image.fromarray(out, 'RGBA')


def hang(front):
    """The same framed painting on the bars' left wall, leant as build.py's
    `hang` leans it: 116 wide in a 128 by 563 picture, its top and bottom
    edges rising 1.6 pixels for each one across, the right side's top 14 down.
    Area averaged across, never smoothed."""
    lean_k, pw, ph_all = 1.6, 116, 563
    lean = int(round(lean_k * (pw - 1)))
    ph = ph_all - 14 - lean - 6
    a = np.asarray(front).astype(np.float64)
    pre = a.copy()
    pre[..., :3] *= pre[..., 3:4] / 255.0
    def down(ch):
        return np.asarray(Image.fromarray(ch.astype(np.float32), 'F').resize((pw, ph), Image.BOX))
    al = down(pre[..., 3]) / 255.0
    src = np.zeros((ph, pw, 4), np.uint8)
    for c in range(3):
        src[..., c] = np.clip(np.where(al > 0, down(pre[..., c]) / np.maximum(al, 1e-6), 0), 0, 255).astype(np.uint8)
    src[..., 3] = np.where(al >= 0.5, 255, 0)
    src[src[..., 3] == 0] = 0
    out = np.zeros((ph_all, 128, 4), np.uint8)
    x0 = (128 - pw) // 2
    for x in range(pw):
        dy = 14 + int(round(lean * (1 - x / (pw - 1))))
        out[dy:dy + ph, x0 + x] = src[:, x]
    return Image.fromarray(out, 'RGBA')


def main():
    """OUT_DIR for the front views; with --leant DIR before the pictures, each
    is also written leant to DIR under the name after its colon
    (wide03:painting03=PICTURE.png)."""
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    args = sys.argv[2:]
    leant = None
    if args and args[0] == '--leant':
        leant, args = args[1], args[2:]
    frame = the_frame()
    for arg in args:
        name, path = arg.split('=', 1)
        name, _, lname = name.partition(':')
        im = make(path, frame)
        im.save(f'{out}/{name}.png')
        if leant and lname:
            hang(im).save(f'{leant}/{lname}.png')
        print(name, lname, im.size)


if __name__ == '__main__':
    main()
