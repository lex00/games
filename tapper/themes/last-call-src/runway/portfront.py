"""What stands in front of the tunnel's port: the bar reaching back into the dark, and a docking collar where it leaves.

    python3 runway/portfront.py THEME_DIR [PREVIEW_PNG]

A piece the size of the tunnel's, laid at the same place after the bars, clear except:
  - the bar's own rows (96 to 160 down the piece) carried back from the port's far edge
    toward its near edge, each column darker the deeper it lies, so the bar comes out of
    shadow instead of starting at a line;
  - Runway's docking collar (samples/collar16.png) upright on the port's far edge, a
    little taller than the bar, where the bar passes through.
Reads the tunnel piece for where the port's black is, and the bar painting for the bar.
"""
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
from paintings16 import flood  # noqa: E402

BAR0, BAR1 = 96, 160


def collar(h):
    a = np.asarray(Image.open(os.path.join(os.path.dirname(__file__), 'samples/collar16.png')).convert('RGB')).astype(int)
    bg = np.median(a[:30, :30].reshape(-1, 3), axis=0)
    d = np.abs(a - bg).sum(axis=2)
    hh, ww = d.shape
    rim = [(y, x) for y in range(hh) for x in (0, ww - 1)] + [(y, x) for x in range(ww) for y in (0, hh - 1)]
    solid = ~flood(d < 110, rim)
    ys, xs = np.nonzero(solid)
    a, solid = a[ys.min():ys.max() + 1, xs.min():xs.max() + 1], solid[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    w = max(10, round(a.shape[1] * h / a.shape[0]))
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
    bay = Image.open(f'{theme}/pieces/space/bay.png').convert('RGBA')
    bp = bay.load()
    w, h = bay.size
    bar = Image.open(f'{theme}/pieces/space/bar.png').convert('RGB')
    rows = [bar.getpixel((40, 224 + y)) for y in range(64)]       # the bar is the same along its length: one column of it
    out = Image.new('RGBA', (w, h), (0, 0, 0, 0))
    op = out.load()
    # the port's black in the bar's middle row: from its near edge to its far edge
    mid = (BAR0 + BAR1) // 2
    black = [x for x in range(w) if bp[x, mid] == (8, 6, 12, 255)]     # the port's own black, not a dark seam of the wall
    near, far = min(black), max(black)
    for x in range(near + 6, far + 3):                              # only inside the port: past the collar the bar is the room's own
        t = min(1.0, (x - near - 6) / max(1, far - near - 10))
        k = 0.10 + 0.90 * t * t                                   # deep in the port almost black, full at the collar
        for y in range(BAR0, BAR1):
            r, g, b = rows[y - BAR0]
            op[x, y] = (int(r * k), int(g * k), int(b * k), 255)
    c = collar(BAR1 - BAR0 + 16)
    out.alpha_composite(c, (far - c.width // 2 - 2, BAR0 - 8))
    out.save(f'{theme}/pieces/space/portfront.png')
    print('port front', out.size, 'port from', near, 'to', far, 'collar', c.size)
    if len(sys.argv) > 2:
        both = bay.copy()
        both.alpha_composite(out)
        both.crop((max(0, near - 120), 0, w, h)).resize(((w - max(0, near - 120)) * 3, h * 3), Image.NEAREST).save(sys.argv[2])


if __name__ == '__main__':
    main()
