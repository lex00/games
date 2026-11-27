import sys, numpy as np
from PIL import Image, ImageDraw
R = '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/samples'
def load(n):
    a = np.asarray(Image.open(f'{R}/{n}.png').convert('RGB')).astype(float)
    white = (a.min(axis=2) > 225) & (np.ptp(a, axis=2) < 25)
    im = Image.fromarray(np.where(white, 255, 0).astype(np.uint8)).copy()
    H, W = white.shape
    for x, y in [(x, 0) for x in range(0, W, 8)] + [(x, H - 1) for x in range(0, W, 8)] + [(0, y) for y in range(0, H, 8)] + [(W - 1, y) for y in range(0, H, 8)]:
        if im.getpixel((x, y)) == 255: ImageDraw.floodfill(im, (x, y), 128)
    return a, np.asarray(im) != 128
def runs(v, gap=12, least=60):
    out, start, blank = [], None, 0
    for i, on in enumerate(list(v) + [False] * (gap + 1)):
        if on:
            if start is None: start = i
            blank = 0; end = i + 1
        elif start is not None:
            blank += 1
            if blank > gap:
                if end - start >= least: out.append((start, end))
                start = None
    return out
def objects(a, m, minarea=20000):
    out = []
    for y0, y1 in runs(m.sum(axis=1) > 3):
        for x0, x1 in runs(m[y0:y1].sum(axis=0) > 3):
            ys = np.nonzero(m[y0:y1, x0:x1].any(axis=1))[0]
            sl = (slice(y0 + ys[0], y0 + ys[-1] + 1), slice(x0, x1))
            if m[sl].sum() >= minarea: out.append(sl)
    return out
def shrink(a, m, w, h):
    # Area average to w x h; alpha by coverage.
    H, W = m.shape
    ys = np.linspace(0, H, h + 1).astype(int); xs = np.linspace(0, W, w + 1).astype(int)
    o = np.zeros((h, w, 4), np.uint8)
    for j in range(h):
        for i in range(w):
            mm = m[ys[j]:ys[j+1], xs[i]:xs[i+1]]; aa = a[ys[j]:ys[j+1], xs[i]:xs[i+1]]
            if mm.size and mm.mean() >= 0.5:
                o[j, i, :3] = aa[mm].mean(axis=0); o[j, i, 3] = 255
    return Image.fromarray(o, 'RGBA')
def crop(a, m, sl, inset=0):
    y, x = sl
    return a[y.start+inset:y.stop-inset, x.start+inset:x.stop-inset], m[y.start+inset:y.stop-inset, x.start+inset:x.stop-inset]
