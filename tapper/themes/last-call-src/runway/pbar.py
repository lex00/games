"""The punk room's bars: one painting the height of the screen, a bar's rows repeating every 96 game pixels.

    python3 runway/pbar.py THEME_DIR [PREVIEW_PNG]

A bar's rows on the board, game pixels down from its top at y 112 (then every 96): 16 of
top surface, 8 of the front lip, 8 of dark under it, 16 of the front's pads, 2 of a red
line at the foot. The room rules lay this painting wherever the board has a bar's own
colours, so the bar keeps every edge the board gives it: its slanted ends, the doors in
front of it, the guests.

The top is glossy red plastic: deep red with the room mirrored in it as soft upright
streaks of light, and a hard glint along its front edge.
"""
import sys

import numpy as np
from PIL import Image

W, H = 1024, 960


def main():
    t = sys.argv[1]
    rng = np.random.default_rng(11)
    Y, X = np.mgrid[0:H, 0:W]
    o = np.mod(Y - 224, 192)
    out = np.zeros((H, W, 3), float)
    out[:] = (40, 6, 10)
    # the top: deep at its back edge, brighter to the front
    top = o < 32
    f = np.clip(o / 31.0, 0, 1)
    base = np.array([112, 6, 16.])[None, None, :] + f[..., None] * np.array([92, 16, 18.])
    # the room in it: upright streaks of light, each a few pixels wide, strongest at the back and fading to the front,
    # and a different set on each bar
    bar = (Y - 224) // 192
    streak = np.zeros((H, W))
    for b in range(-2, 5):
        for _ in range(11):
            cx, wd, k = rng.integers(0, W), rng.integers(5, 15), rng.uniform(0.35, 1.0)
            m = (bar == b) & (np.abs(X - cx - (o * 0.35)) < wd)
            streak[m] = np.maximum(streak[m], k)
    refl = streak * (1.0 - 0.75 * f)
    base = base + refl[..., None] * np.array([120, 70, 76.])
    # a long soft band of light across the middle, as a gloss
    band = np.exp(-((o - 13) / 3.0) ** 2) * 0.35
    base = base + band[..., None] * np.array([110, 60, 66.])
    out[top] = base[top]
    glint = (o >= 27) & (o < 29)
    out[glint] = (255, 214, 214)
    out[(o >= 29) & (o < 32)] = (236, 60, 66)
    # the front lip: a rounded edge of the same plastic
    for a, b_, col in ((32, 35, (255, 150, 150)), (35, 44, (232, 28, 40)), (44, 48, (150, 10, 20)), (48, 56, (70, 6, 12)), (56, 64, (22, 8, 12))):
        out[(o >= a) & (o < b_)] = col
    spec = (o >= 36) & (o < 38) & (np.mod(X + bar * 37, 96) < 40)
    out[spec] = (255, 120, 124)
    # the front: padded pads, one to a cell, a chrome stud between each
    pad = (o >= 64) & (o < 96)
    px_, py_ = np.mod(X, 32), o - 64
    out[pad] = (40, 6, 10)
    inner = pad & (px_ >= 3) & (px_ < 29) & (py_ >= 3) & (py_ < 29)
    corner = ((px_ < 6) | (px_ >= 26)) & ((py_ < 6) | (py_ >= 26)) & (np.minimum(np.abs(px_ - 5.5), np.abs(px_ - 25.5)) + np.minimum(np.abs(py_ - 5.5), np.abs(py_ - 25.5)) > 3.2) & ~((px_ >= 6) & (px_ < 26)) & ~((py_ >= 6) & (py_ < 26))
    inner &= ~corner
    out[inner] = (146, 16, 26)
    out[inner & (py_ < 8)] = (204, 44, 54)
    out[inner & (px_ < 7) & (py_ >= 8)] = (176, 28, 38)
    out[inner & (py_ >= 24)] = (88, 6, 14)
    out[inner & (px_ >= 25) & (py_ < 24)] = (110, 10, 18)
    out[inner & (py_ >= 9) & (py_ < 11) & (px_ >= 9) & (px_ < 17)] = (244, 120, 126)
    stud = pad & ((px_ < 2) | (px_ >= 30)) & (py_ >= 14) & (py_ < 18)
    out[stud] = (214, 218, 228)
    # the line at the foot
    out[(o >= 96) & (o < 98)] = (255, 120, 124)
    out[(o >= 98) & (o < 100)] = (214, 24, 36)
    out += rng.normal(0, 1.2, (H, W))[..., None] * top[..., None]
    im = Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), 'RGB')
    im.save(f'{t}/pieces/punk/bar.png')
    if len(sys.argv) > 2:
        im.crop((300, 200, 780, 440)).resize((960, 480), Image.NEAREST).save(sys.argv[2])
    print('bar painting written')


if __name__ == '__main__':
    main()
