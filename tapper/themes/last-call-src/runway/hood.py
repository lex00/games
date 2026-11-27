"""The ballpark's hood at a bar's end, drawn on the original's own geometry.

    python3 runway/hood.py OUT.png

Measured on the board's screen at the top bar (game pixels): a brass post at x 368, a dark opening beside it that
widens downward (the way in, seen almost edge-on), which the bar's top runs into at rows 112 to 127; a canvas roof
slanting up to the right over it; and the canvas front hanging below the roof, its foot stepped as the stand's
tiers are. Drawn at twice the game's pixels in a 76 by 72 game pixel picture whose top left is game (366, 58).
The bar's own end is left clear so the bar shows in front of the opening.
"""
import sys
from PIL import Image, ImageDraw

OX, OY = 366, 44
S = 2
W, H = 86, 98
O = (38, 24, 18, 255)
# the hanging side is a clear step darker than the roof: one faces up into the light, the other faces the field
ORANGE, ORANGE_D = (236, 122, 32, 255), (160, 68, 16, 255)
CREAM, CREAM_D = (250, 232, 190, 255), (184, 152, 108, 255)
BRASS, BRASS_D, BRASS_H = (232, 190, 80, 255), (150, 96, 20, 255), (255, 236, 150, 255)


def P(pts):
    return [((x - OX) * S, (y - OY) * S) for x, y in pts]


# the roof is the original's whole parallelogram: its upper edge runs straight from the post up to its peak
# its back edge leans as the stand's rows do, 1 across for 3 down
# four corners, one knob each: the post's top, the peak, the back corner, the near corner
# its front and back edges both run exactly as the stand's rows do: 11 across for 33 down
roof = [(372, 74), (424, 49), (435, 82), (383, 107)]
# the hanging side is upright, so its back edge drops straight down from the roof's corner, as its near edge does
front = [(383, 107), (435, 82), (435, 101), (421, 101), (421, 113), (403, 113), (403, 126), (383, 126)]
wedge = [(370, 74), (372, 74), (383, 107), (384, 127), (370, 127)]

im = Image.new('RGBA', (W * S, H * S), (0, 0, 0, 0))
mask = lambda poly: (lambda m: (ImageDraw.Draw(m).polygon(P(poly), fill=255), m)[1])(Image.new('L', im.size, 0))
mr, mf, mw = mask(roof), mask(front), mask(wedge)
px = im.load(); pr, pf, pw = mr.load(), mf.load(), mw.load()
for y in range(H * S):
    for x in range(W * S):
        gx, gy = OX + x / S, OY + y / S
        if pw[x, y]:
            # the bar's end stays clear: its top's right edge runs from (373,112) to (381,127)
            # the bar's top ends in steps, 2 pixels further right every 4 rows from 373 at row 112 to 381 at row 126
            if gy >= 112 and gx < 374 + 2 * ((int(gy) - 110) // 4):
                continue
            d = (gx - 370) / 14.0
            px[x, y] = (int(20 + 26 * d), int(14 + 16 * d), int(12 + 10 * d), 255)     # deep in shadow, a little lighter at its mouth's far side
        elif pr[x, y]:
            # stripes run down the roof's slope, leaning as its right edge does
            u = gx - (gy - 49) / 3.0
            k = int((u - 372) // 7) % 2
            c = ORANGE if k == 0 else CREAM
            shade = 1.0 - 0.16 * (gy - 49) / 59.0
            px[x, y] = (int(c[0] * shade), int(c[1] * shade), int(c[2] * shade), 255)
        elif pf[x, y]:
            # the hanging side's stripes drop straight down, where the roof's lean: that tells the eye it is upright
            k = int((gx - 386) // 6) % 2
            px[x, y] = ORANGE_D if k == 0 else CREAM_D
d = ImageDraw.Draw(im)
# the post stands under the roof's first knob, so that knob joins the post and both of the roof's rods
p0, p1 = P([(371, 76)])[0], P([(373, 112)])[0]
d.rectangle((p0[0] - 1, p0[1] - 1, p1[0], p1[1]), fill=O); d.rectangle((p0[0], p0[1], p1[0] - 2, p1[1] - 1), fill=BRASS); d.line((p0[0], p0[1], p0[0], p1[1] - 1), fill=BRASS_H)


def rod(a, b):
    d.line([a, b], fill=O, width=6); d.line([a, b], fill=BRASS, width=3); d.line([(a[0], a[1] - 1), (b[0], b[1] - 1)], fill=BRASS_H, width=1)


for poly in (roof, front):
    pts = P(poly)
    for i in range(len(pts)):
        rod(pts[i], pts[(i + 1) % len(pts)])
    for q in pts:
        d.ellipse((q[0] - 4, q[1] - 4, q[0] + 4, q[1] + 4), fill=O); d.ellipse((q[0] - 3, q[1] - 3, q[0] + 3, q[1] + 3), fill=BRASS)
# the seam between roof and front: a brass rail with a highlight
a, b = P([(383, 107)])[0], P([(435, 82)])[0]
d.line([a, b], fill=O, width=6); d.line([a, b], fill=BRASS, width=3); d.line([(a[0], a[1] - 1), (b[0], b[1] - 1)], fill=BRASS_H, width=1)
for q in (a, b):
    d.ellipse((q[0] - 4, q[1] - 4, q[0] + 4, q[1] + 4), fill=O); d.ellipse((q[0] - 3, q[1] - 3, q[0] + 3, q[1] + 3), fill=BRASS)
# scallops along each foot of the front
for (x0, x1, y) in ((421, 435, 101), (403, 421, 113), (383, 403, 126)):
    for gx in range(x0, x1, 6):
        c = P([(gx + 3, y)])[0]
        d.pieslice((c[0] - 6, c[1] - 6, c[0] + 6, c[1] + 6), 0, 180, fill=O)
        d.pieslice((c[0] - 4, c[1] - 5, c[0] + 4, c[1] + 4), 0, 180, fill=ORANGE_D if (gx // 6) % 2 else CREAM_D)
# the post at the opening's near side, down to the bar's top, and the front's near edge
e0, e1 = P([(382, 108)])[0], P([(385, 126)])[0]
d.rectangle((e0[0], e0[1], e1[0], e1[1]), fill=O); d.rectangle((e0[0] + 1, e0[1] + 1, e1[0] - 2, e1[1] - 1), fill=BRASS_D)
# The bar's rows where the board draws them black over the stand's first step (the top's front edge and its shadow,
# and the shadow under the frame): taken from the bars' own painting, so the bar runs on unbroken to the door.
import os
bar = Image.open(os.path.join(os.path.dirname(sys.argv[1]) if False else os.path.expanduser('~/.gameprinter/work/themes/tapper-pixel/repo/theme/pieces/sports'), 'bar_all.png')).convert('RGBA')
for (y0, y1) in ((128, 129), (136, 139)):
    box = (367 * S, y0 * S, 382 * S, (y1 + 1) * S)
    im.paste(bar.crop(box), ((367 - OX) * S, (y0 - OY) * S))
# The steps the hood's foot stands on, drawn under the hood as part of the stand: each is the top of one of the
# stand's rows, so it runs from one of the stand's leaning lines to the next (they stand at 316 + 24k + y/3 in game
# pixels) and leans with them; a lit tread, its dark edge, and a shade below that fades into the concrete.
under = Image.new('RGBA', im.size, (0, 0, 0, 0))
up = under.load()
for (k, y) in ((4, 104), (3, 116), (2, 129)):
    for r in range(0, 22):
        Y = y * S + r
        xl = (314 + 24 * (k - 1)) * S + Y / 3.0 + 8        # past that line's own shadow
        xr = (314 + 24 * k) * S + Y / 3.0
        if r < 4:
            c = (228, 218, 198, 255)
        elif r < 6:
            c = O
        else:
            t = (r - 6) / 15.0
            c = (int(104 + 80 * t), int(98 + 76 * t), int(88 + 70 * t), int(255 * (1 - t) ** 1.5))
        for X in range(int(xl), int(xr)):
            px_, py_ = X - OX * S, Y - OY * S
            if 0 <= px_ < W * S and 0 <= py_ < H * S:
                up[px_, py_] = c
under.alpha_composite(im)
im = under
im.save(sys.argv[1])
print(im.size)
