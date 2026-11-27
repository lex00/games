# Cuts Last Call's saloon pieces from our Runway sheets: white backdrop
# cleared by a flood from the edges, each object cropped to its outline,
# shrunk by area averaging (a pixel is solid when half its area is the
# object, and takes the mean of the object's pixels only), no rims.
import sys, numpy as np
from PIL import Image
from PIL import ImageDraw
R = 'runway/samples'; OUT = sys.argv[1]
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
def save(im, n): im.save(f'{OUT}/{n}.png')
# Materials: nine swatches, row by row.
a, m = load('materials'); sw = objects(a, m)
assert len(sw) == 9, len(sw)
S = 32
def sub(k, fy0=0, fy1=1, fx0=0, fx1=1, w=S, h=S, flip=False):
    aa, mm = crop(a, m, sw[k], 16)
    H, W = mm.shape
    aa = aa[int(H*fy0):int(H*fy1), int(W*fx0):int(W*fx1)]; mm = np.ones(aa.shape[:2], bool)
    im = shrink(aa, mm, w, h)
    return im.transpose(Image.FLIP_LEFT_RIGHT) if flip else im
wall = sub(0, fx1=0.5)                         # two boards a cell, as the original's seams
back = sub(1, 0, 0.6, 0, 0.5)
post = sub(2, fx0=0.1, fx1=0.6)
# Three whole boards, seam to seam (the swatch's seams at rows 63, 149,
# 229 and 312 of 518): a cut of two and a part left a sliver of a board
# with no nails every third row, out of step with the rest.
floor = sub(3, 68 / 518, 316 / 518, 0, 0.45)
# A cooler, weathered grey-brown, so a full glass stands out on the floor.
_f = np.asarray(floor).astype(float); _g = _f[..., :3].mean(axis=2, keepdims=True)
_f[..., :3] = (_g + (_f[..., :3] - _g) * 0.35) * 0.92 + np.array([4, 2, 0])
floor = Image.fromarray(np.clip(_f, 0, 255).astype(np.uint8), 'RGBA')
top = sub(4, 0.35, 0.95, 0, 0.5)
front = sub(5, 0.15, 0.95, 0, 0.36)
rail = sub(6, 0.1, 0.9, 0, 0.5)
for n, im in [('t007', wall), ('t008', back), ('t015', post), ('t004', floor), ('t001', top), ('t01b', front), ('t01a', rail)]:
    save(im, n)
# Counter's right end: the counter's own top, front and foot, ended by a
# strip of end grain with a bright edge, so it reads as the counter stopping.
def ended(im, rail=False):
    o = np.asarray(im).copy()
    for x in range(S - 7, S):
        o[:, x, :3] = (o[:, x, :3] * 0.55).astype(np.uint8)
    o[:, S - 7, :3] = np.minimum(o[:, S - 7, :3].astype(int) + 70, 255)
    o[:, S - 1, :3] = (40, 22, 12)
    return Image.fromarray(o, 'RGBA')
save(ended(top), 't017'); save(ended(front), 't018'); save(ended(rail), 't019')
# The floor's slanted right edge: floor left of the room's line, wall right,
# the line (x0 at the cell's top to x1 at its bottom, in cell pixels)
# measured on each edge tile's own cells of the contact sheet.
fl, wl = np.asarray(floor), np.asarray(wall)
for n, (x0, x1) in {'t012': (2.4, 8.0), 't011': (7.5, 13.9), 't014': (14.0, 20.0), 't013': (-4.0, 2.0)}.items():
    o = wl.copy()
    for y in range(S):
        lx = (x0 + (x1 - x0) * (y + 0.5) / S) * 2
        for x in range(S):
            if x + 0.5 < lx: o[y, x] = fl[y, x]
            elif x + 0.5 < lx + 1.5: o[y, x] = (60, 34, 20, 255)
    save(Image.fromarray(o, 'RGBA'), n)
# Pieces.
a, m = load('pieces'); ob = objects(a, m)
assert len(ob) == 5, len(ob)
door, cask, _, pic, horns = [crop(a, m, s) for s in ob]
def shear(im, w, h, dx_top, dy_left=0):
    # An upright picture leant into the room's perspective: shifted right by
    # dx_top at its top (doors) or down by dy_left at its left (the picture).
    o = Image.new('RGBA', (w, h))
    src = np.asarray(im); out = np.zeros((h, w, 4), np.uint8)
    ih, iw = src.shape[:2]
    for y in range(ih):
        for x in range(iw):
            if src[y, x, 3]:
                X = x + int(round(dx_top * (1 - y / max(ih - 1, 1))))
                Y = y + int(round(dy_left * (1 - x / max(iw - 1, 1))))
                if 0 <= X < w and 0 <= Y < h: out[Y, X] = src[y, x]
    return Image.fromarray(out, 'RGBA')
# Door: 3 cells by 9 at scale 2 (96 by 288), drawn upright as the room
# shows it: the doorway a dark band whose wooden frame follows the wall's
# slant, and two upright swinging leaves, a small far one and a bigger near
# one, where the room's doors have them; the floor's wedge at its foot.
da, dm = door
H, W = dm.shape
def part(fy0, fy1, fx0, fx1, w, h):
    return shrink(da[int(H*fy0):int(H*fy1), int(W*fx0):int(W*fx1)], dm[int(H*fy0):int(H*fy1), int(W*fx0):int(W*fx1)], w, h)
leaf = part(0.32, 0.82, 0.15, 0.50, 1, 1)  # just to keep the call shape
leaf_img = part(0.32, 0.82, 0.15, 0.50, 34, 72)
far_img = part(0.32, 0.82, 0.15, 0.50, 26, 58)
post = np.asarray(part(0.1, 0.9, 0.0, 0.14, 8, 64))
# The hole in the wall, its own piece, drawn under the counter: dark inside
# a wooden frame whose outer edge leans with the wall from (58, 22) to
# (32, 96), then runs straight down to the floor, as the room shows it.
from PIL import ImageDraw as _D
def frame_x(y): return 58 - 26 * (y - 22) / 74 if y < 96 else 32
o = np.zeros((288, 96, 4), np.uint8)
for y in range(22, 216):
    xl = frame_x(y); xr = 62 if y < 98 else (80 if y < 170 else 46 - (y - 170) // 2)
    for x in range(int(xl), xr):
        o[y, x] = (16, 12, 20, 255)
    for k in range(6):
        xx = int(round(xl)) - 5 + k
        if 0 <= xx < 96: o[y, xx] = post[y % 64, k + 1]
hole = Image.fromarray(o, 'RGBA')
save(hole, 'hole')
# The leaves, alone: a small far one and a bigger near one, each without
# the dark backdrop its source had round it (a flood over dark pixels from
# the leaf's edge, so the slats' lines inside stay).
def unbacked(img):
    a = np.array(img.convert('RGBA'))
    h, w = a.shape[:2]
    dark = (a[..., :3].max(axis=2) < 70) | (a[..., 3] == 0)
    seen = np.zeros((h, w), bool)
    stack = [(y, x) for y in range(h) for x in (0, w - 1)] + [(y, x) for x in range(w) for y in (0, h - 1)]
    while stack:
        y, x = stack.pop()
        if 0 <= y < h and 0 <= x < w and not seen[y, x] and dark[y, x]:
            seen[y, x] = True
            stack += [(y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)]
    a[seen, 3] = 0
    # Then the loose specks the cut left: any patch of solid pixels apart
    # from the leaf, smaller than 40.
    solid = a[..., 3] > 0
    lab = np.zeros((h, w), int); n = 0; sizes = {}
    for y0 in range(h):
        for x0 in range(w):
            if solid[y0, x0] and not lab[y0, x0]:
                n += 1; st = [(y0, x0)]; lab[y0, x0] = n; c = 0
                while st:
                    y, x = st.pop(); c += 1
                    for yy, xx in ((y+1, x), (y-1, x), (y, x+1), (y, x-1)):
                        if 0 <= yy < h and 0 <= xx < w and solid[yy, xx] and not lab[yy, xx]:
                            lab[yy, xx] = n; st.append((yy, xx))
                sizes[n] = c
    for k, c in sizes.items():
        if c < 40:
            a[lab == k, 3] = 0
    return Image.fromarray(a, 'RGBA')
far_img, leaf_img = unbacked(far_img), unbacked(leaf_img)
# The leaves, alone: a small far one and a bigger near one.
im = Image.new('RGBA', (96, 288))
im.alpha_composite(far_img, (61, 40))  # hinged on the doorway's edge (its dark band ends at 61)
im.alpha_composite(leaf_img, (34, 102))
save(im, 'door')
a2, m2 = load('cask2'); c = shrink(*crop(a2, m2, objects(a2, m2)[0]), 92, 116)
o = Image.new('RGBA', (96, 128)); o.paste(c, (2, 6))
# The barrel's end turned a quarter inside its rim, so its boards run across
# and don't line up with the wall's upright planks; its red diamond kept.
from PIL import ImageDraw as _D2
cx, cy, rx, ry = 63, 63, 22, 46
box = (cx - rx, cy - ry, cx + rx, cy + ry)
patch = o.crop(box)
turned = patch.transpose(Image.Transpose.ROTATE_90).resize(patch.size, Image.NEAREST)
mask = Image.new('L', patch.size, 0); _D2.Draw(mask).ellipse((0, 0, patch.width - 1, patch.height - 1), fill=255)
was = np.asarray(o).copy()
o.paste(turned, box[:2], mask)
now = np.array(o)
r, g, b = was[..., 0].astype(int), was[..., 1].astype(int), was[..., 2].astype(int)
red = (r > 2 * g) & (g < 70) & (r > 100) & (was[..., 3] > 0)
o = Image.fromarray(now, 'RGBA')
ys, xs = np.nonzero(red)
if len(xs):
    x0, x1, y0, y1 = xs.min(), xs.max(), ys.min(), ys.max()
    col = tuple(int(v) for v in np.median(was[red][:, :3], axis=0)) + (255,)
    mx, my = (x0 + x1) / 2, (y0 + y1) / 2
    _D2.Draw(o).polygon([(mx, y0), (x1, my), (mx, y1), (x0, my)], fill=col)
save(o, 'cask')
# Paintings hang as the room's picture does: upright sides filling its box
# 128 wide, top and bottom edges leaning with the wall, 180 lower at the
# left than at the right (the original frame's lean, measured on its
# screen), the right side's top 14 down. Each a different painting; the
# room picks one each game (picture|painting01|...).
# A painting on the left wall, leant as the wall's lines lean: its top and
# bottom edges rise 1.6 pixels for each one across, between the tops of the
# doorways below it (2.75, measured on the screen) and front-facing, so the
# picture reads. Drawn at its own size (128
# by 563, placed from the picture's top tile by `room_at`), its bottom where
# the original's picture ends and its top higher up the wall: 104 wide.
LEAN, PW, PH = 1.6, 116, 563
def hang(img):
    lean = int(round(LEAN * (PW - 1)))
    ph = PH - 14 - lean - 6
    src = np.asarray(img.convert('RGBA').resize((PW, ph), Image.LANCZOS))
    out = np.zeros((PH, 128, 4), np.uint8)
    x0 = (128 - PW) // 2
    for x in range(PW):
        dy = 14 + int(round(lean * (1 - x / (PW - 1))))
        out[dy:dy + ph, x0 + x] = src[:, x]
    return Image.fromarray(out, 'RGBA')
save(hang(shrink(*pic, 128, 250)), 'picture')
# The idle screen draws no tap sprites, so its barrels carry a brass tap of
# their own (cut from the first pieces sheet's barrel by its gold, with
# its dark outline), in the middle of the barrel's end.
pz = np.asarray(Image.open(f'{R}/pieces.png').convert('RGB')).astype(float)
tb = pz[300:500, 720:830]
gold = (tb[..., 0] > 170) & (tb[..., 1] > 0.74 * tb[..., 0]) & (tb[..., 2] < 0.5 * tb[..., 0])
near = gold.copy()
for dy in (-3, -2, -1, 0, 1, 2, 3):
    for dx in (-3, -2, -1, 0, 1, 2, 3):
        near |= np.roll(np.roll(gold, dy, 0), dx, 1)
dark = tb.max(axis=2) < 90
tm = gold | (near & dark & (tb[..., 2] < tb[..., 0]))
ys, xs = np.nonzero(tm)
tap = shrink(tb[ys.min():ys.max() + 1, xs.min():xs.max() + 1], tm[ys.min():ys.max() + 1, xs.min():xs.max() + 1], 20, 40)
ct = Image.open(f'{OUT}/cask.png').convert('RGBA')
ct.alpha_composite(tap, (63 - 10, 63 - 14))
save(ct, 'cask_tap')
# The tap at rest (the board's sprite 09, 32 by 32: its pixels at x 11 to
# 28, y 5 to 31) drawn as the brass tap, 64 by 64 at the theme's scale 2,
# over the same place.
brass = shrink(tb[ys.min():ys.max() + 1, xs.min():xs.max() + 1], tm[ys.min():ys.max() + 1, xs.min():xs.max() + 1], 30, 54)
t09 = Image.new('RGBA', (64, 64)); t09.alpha_composite(brass, (24, 10))
t09.save(f'{OUT}/../../art/tap09.png')
pa, pm = load('paintings')
found = sorted(objects(pa, pm), key=lambda sl: (sl[0].start // 200, sl[1].start))
assert len(found) == 12, len(found)
for k, sl in enumerate(found):
    front = shrink(*crop(pa, pm, sl), 128, 250)
    # Front-facing, for a wall seen straight on (the bonus room); the
    # leant one for the saloon's left wall is made from it.
    import os; os.makedirs(f'{OUT}/../front', exist_ok=True)
    front.save(f'{OUT}/../front/painting{k + 1:02d}.png')
    save(hang(front), f'painting{k + 1:02d}')
h = shrink(*horns, 128, 104); o = Image.new('RGBA', (128, 160)); o.paste(h, (0, 28)); save(o, 'figure')
a, m = load('sign'); s = shrink(*crop(a, m, objects(a, m)[0]), 288, 84)
o = Image.new('RGBA', (288, 224)); o.paste(s, (0, 70)); save(o, 'sign')
print('built')
