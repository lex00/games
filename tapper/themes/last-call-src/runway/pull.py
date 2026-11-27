# The pull frames (bartender parts 03 and 06): the theme's brass tap with a
# hand on its lever, from samples/pull-hand.png (our tap as its reference),
# the hand reaching in from the left as his arm does.
import numpy as np
from PIL import Image
R = '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/samples'
T = '/Users/alex/.gameprinter/work/themes/tapper-pixel/repo/theme'
a = np.asarray(Image.open(f'{R}/pull-hand2.png').convert('RGBA')).astype(int)
bg = (a[..., :3].min(axis=2) > 235)
a[bg, 3] = 0
def cell(x0, x1):
    c = a[:, x0:x1].copy()
    ys, xs = np.nonzero(c[..., 3] > 0)
    return c[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
def parts(c):
    r, g, b = c[..., 0], c[..., 1], c[..., 2]
    on = c[..., 3] > 0
    brass = on & (r > 150) & (g > 110) & (b < 90) & (r - b > 90) & (g > 0.6 * r)
    lever = on & (r > 80) & (r < 200) & (g < 0.62 * r) & (b < 0.5 * r)
    skin = on & (r > 180) & (g > 120) & (b > 70) & (b < 190) & ~brass
    white = on & (c[..., :3].min(axis=2) > 200)
    return on, brass, lever, skin, white
def mirror_hand(c):
    on, brass, lever, skin, white = parts(c)
    hand = skin | white
    ys, xs = np.nonzero(hand)
    y0, y1 = ys.min(), ys.max()
    # The hand's outline: dark pixels beside it, in its rows.
    dark = on & (c[..., :3].max(axis=2) < 90)
    near = np.zeros_like(hand)
    for dy in (-2, -1, 0, 1, 2):
        for dx in (-2, -1, 0, 1, 2):
            near |= np.roll(np.roll(hand, dy, 0), dx, 1)
    take = (hand | (dark & near)) & ~lever
    take[:y0] = False; take[y1 + 1:] = False
    # About the grip: the lever's middle in the hand's lowest rows.
    lx = np.nonzero(lever[y1 - 6:y1 + 1].any(axis=0))[0]
    gx = int(lx.mean()) if len(lx) else int(xs.min())
    w = c.shape[1]
    pad = w
    out = np.zeros((c.shape[0], w + 2 * pad, 4), int)
    keep = c.copy(); keep[take, 3] = 0
    out[:, pad:pad + w] = keep
    yy, xx = np.nonzero(take)
    for y, x in zip(yy, xx):
        X = 2 * gx - x + pad
        if 0 <= X < out.shape[1] and out[y, X, 3] == 0 or lever[y, min(max(X - pad, 0), w - 1)] == False:
            out[y, X] = c[y, x]
    ys, xs = np.nonzero(out[..., 3] > 0)
    return out[ys.min():ys.max() + 1, xs.min():xs.max() + 1], brass
for name, (x0, x1), ref in (('pull-03', (0, 960), 'tap-pulled'), ('pull-06', (960, 1920), 'tap-09')):
    m = cell(x0, x1)
    img = Image.fromarray(m.astype(np.uint8), 'RGBA')
    # The tap at the theme's tap's size and place: the brass's width and its
    # bottom-right as the reference picture's.
    rp = np.asarray(Image.open(f'{T}/art/obj/{ref}.png').convert('RGBA')).astype(int)
    _, rb, _, _, _ = parts(rp)
    ry, rx = np.nonzero(rb)
    _, mb, _, _, _ = parts(m)
    my, mx = np.nonzero(mb)
    k = (rx.max() - rx.min() + 1) / (mx.max() - mx.min() + 1)
    img = img.resize((max(1, round(img.width * k)), max(1, round(img.height * k))), Image.LANCZOS)
    ox, oy = rx.max() - round(mx.max() * k), ry.max() - round(my.max() * k)
    out = Image.new('RGBA', (64, 64))
    out.alpha_composite(img.crop((max(0, -ox), max(0, -oy), img.width, img.height)), (max(0, ox), max(0, oy)))
    out.save(f'{T}/art/obj/{name}.png')
    print(name, img.size, (ox, oy))
# His sleeve from the cuff down to his shoulder (the part of his arm the
# board's glass picture carried): a white sleeve with a dark outline, from
# (54, 10) to (22, 60) in its picture, drawn 16 left of and 17 below the
# pull picture so its top meets the cuff.
from PIL import ImageDraw
sl = Image.new('RGBA', (64, 64))
d = ImageDraw.Draw(sl)
d.line([(54, 10), (22, 60)], fill=(40, 28, 28, 255), width=14)
d.line([(54, 10), (22, 60)], fill=(236, 236, 240, 255), width=10)
d.line([(56, 12), (25, 60)], fill=(200, 204, 214, 255), width=3)
sl.save(f'{T}/art/obj/sleeve.png')
print('sleeve')
