"""The punk room's doorways and swing doors at each bar's end.

    python3 runway/door.py THEME_DIR [PREVIEW_PNG]

Measured on the board's second bar, right wall, game pixels:
  the doorway  a dark wedge in the wall seen edge-on, its top at (403, 160), its foot
               from x 400 to 415 at y 207, a painted frame down both edges, and the
               frame's post on down from (417, 237) to the floor at y 256
  upper leaf   behind the bar: its top edge from (369, 185) to (387, 176.5), its long
               edges running (15, 30) down from there, cut by the bar's top at y 207
  lower leaf   in front of the bar's end: corners (411, 206.5), (429.5, 222),
               (397, 257), (380, 240)
The pieces are drawn in that view at twice the game's pixels and saved turned round:
the board keeps the left wall's doors and mirrors them for the right, and the room
rules mirror a piece with its cell.

  pieces/punk/door_main.png   the doorway, its frame and post, and the lower leaf: box x 378..432, y 156..260
  pieces/punk/door_upper.png  the upper leaf, cut at the bar's top:                 box x 364..404, y 172..208
"""
import sys

from PIL import Image, ImageDraw

S = 2
O = (38, 24, 18, 255)
OR_HI, OR_MID, OR_LO = (255, 170, 84, 255), (236, 112, 28, 255), (170, 64, 14, 255)
NV, NV_HI = (22, 26, 96, 255), (70, 92, 190, 255)
YL, YL_LO = (244, 196, 70, 255), (176, 116, 22, 255)
RED, RED_HI, RED_LO = (206, 44, 36, 255), (244, 104, 76, 255), (124, 20, 20, 255)


def leaf(knob):
    """One door leaf, flat: orange enamelled steel, a riveted navy border, two emblem plates."""
    w, h = 24 * S * 2, 48 * S * 2                      # drawn at twice the stage's pixels, then laid into its place
    im = Image.new('RGBA', (w, h), NV)
    d = ImageDraw.Draw(im)
    d.rectangle((3, 3, w - 4, h - 4), fill=NV)
    d.rectangle((3, 3, w - 4, 5), fill=NV_HI)
    for i in range(8, h - 8, 14):
        for x in (6, w - 9):
            d.rectangle((x, i, x + 2, i + 2), fill=NV_HI)
    d.rectangle((12, 12, w - 13, h - 13), fill=O)
    d.rectangle((14, 14, w - 15, h - 15), fill=OR_MID)
    d.rectangle((14, 14, w - 15, 19), fill=OR_HI)
    d.rectangle((14, 14, 19, h - 15), fill=OR_HI)
    d.rectangle((w - 21, 20, w - 15, h - 15), fill=OR_LO)
    d.rectangle((14, h - 21, w - 15, h - 15), fill=OR_LO)
    for i in range(30, h - 30, 9):                     # pressed ribs across the panel
        d.line((22, i, w - 23, i), fill=OR_LO)
        d.line((22, i + 1, w - 23, i + 1), fill=OR_HI)
    for cy in (h // 4 + 4, 3 * h // 4 - 4):            # the emblem plates: a navy diamond, a yellow middle, a dark star
        cx = w // 2
        r = 26
        d.polygon([(cx, cy - r - 3), (cx + r + 3, cy), (cx, cy + r + 3), (cx - r - 3, cy)], fill=O)
        d.polygon([(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)], fill=NV)
        d.line([(cx - r + 3, cy), (cx, cy - r + 3)], fill=NV_HI, width=2)
        r2 = 16
        d.polygon([(cx, cy - r2), (cx + r2, cy), (cx, cy + r2), (cx - r2, cy)], fill=YL)
        d.polygon([(cx, cy), (cx + r2, cy), (cx, cy + r2)], fill=YL_LO)
        st = [(cx, cy - 9), (cx + 3, cy - 3), (cx + 9, cy - 3), (cx + 4, cy + 1), (cx + 6, cy + 8), (cx, cy + 4), (cx - 6, cy + 8), (cx - 4, cy + 1), (cx - 9, cy - 3), (cx - 3, cy - 3)]
        d.polygon(st, fill=O)
    # the knob, only on the leaf in front of the bar, where the board's has its own: near the long edge away from the
    # hinge, a little above the middle (measured: 0.23 across, 0.45 down)
    if not knob:
        return im
    cx, cy = int(w * 0.25), int(h * 0.45)
    d.ellipse((cx - 8, cy - 5, cx + 10, cy + 12), fill=(96, 40, 10, 255))
    d.ellipse((cx - 9, cy - 9, cx + 9, cy + 9), fill=O)
    d.ellipse((cx - 7, cy - 7, cx + 7, cy + 7), fill=(193, 136, 36, 255))
    d.ellipse((cx - 6, cy - 6, cx + 3, cy + 3), fill=(244, 196, 70, 255))
    d.ellipse((cx - 4, cy - 4, cx - 1, cy - 1), fill=(255, 244, 190, 255))
    return im


def lay(canvas, box, corners, src):
    """The leaf's picture laid into four screen corners (game pixels: top left, top right, bottom right, bottom left)."""
    x0, y0 = box[0], box[1]
    W, H = canvas.size
    w, h = src.size
    (ax, ay), (bx, by), (cx, cy), (dx, dy) = [((x - x0) * S, (y - y0) * S) for x, y in corners]
    sp = src.load()
    # a leaf is a parallelogram: u along its top edge, v along its long edge. Each stage pixel is the mean of nine
    # points inside it and is solid when half of them fall on the leaf, so its leaning edges step evenly; then one
    # dark line is drawn all round it.
    ux, uy, vx, vy = bx - ax, by - ay, dx - ax, dy - ay
    det = ux * vy - uy * vx
    solid = {}
    for Y in range(H):
        for X in range(W):
            acc, n = [0, 0, 0], 0
            for sy in (1 / 6, 0.5, 5 / 6):
                for sx in (1 / 6, 0.5, 5 / 6):
                    px, py = X + sx - ax, Y + sy - ay
                    u = (px * vy - py * vx) / det
                    v = (ux * py - uy * px) / det
                    if -0.035 <= u < 1.035 and -0.02 <= v < 1.02:      # a touch past the board's leaf, so none of it shows round ours
                        c = sp[min(w - 1, max(0, int(u * w))), min(h - 1, max(0, int(v * h)))]
                        acc[0] += c[0]; acc[1] += c[1]; acc[2] += c[2]; n += 1
            if n >= 5:
                solid[(X, Y)] = (acc[0] // n, acc[1] // n, acc[2] // n, 255)
    out = canvas.load()
    for (X, Y) in solid:
        for ex, ey in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            q = (X + ex, Y + ey)
            if q not in solid and 0 <= q[0] < W and 0 <= q[1] < H:
                out[q] = O
    for q, c in solid.items():
        out[q] = c


def main():
    t = sys.argv[1]
    lf = leaf(True)
    # the doorway, its frame and post, and the lower leaf
    box = (378, 156, 432, 260)
    im = Image.new('RGBA', ((box[2] - box[0]) * S, (box[3] - box[1]) * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    P = lambda x, y: ((x - box[0]) * S, (y - box[1]) * S)
    # its top is flat and 4 wide at y 160, as the board's frame is, with nothing above it; the dark starts at y 171
    fl, fr = P(398, 208), P(418, 208)
    # (the top bar's frame stands 4 higher than the others', y 156 here: the piece takes that height for all four, so
    # none of the board's frame is left showing above it)
    d.polygon([P(402, 156), P(406, 156), P(406, 160), fr, fl, P(402, 160)], fill=O)
    d.polygon([P(404, 171), P(415, 207), P(401, 207)], fill=(18, 14, 24, 255))
    d.polygon([P(404, 175), P(409, 192), P(402.5, 192)], fill=(34, 28, 44, 255))      # a little light falling in at its top
    # the frame: red painted steel down the wall-side edge with a yellow line outside it, and a thin one on the near edge
    d.rectangle((P(402.5, 156.5)[0], P(402.5, 156.5)[1], P(404, 162)[0], P(404, 162)[1]), fill=RED)
    d.rectangle((P(404.5, 156.5)[0], P(404.5, 156.5)[1], P(405.5, 162)[0], P(405.5, 162)[1]), fill=YL)
    d.line([P(404, 161), P(416, 207)], fill=RED, width=4)
    d.line([P(405.5, 161), P(417.5, 207)], fill=YL, width=2)
    d.line([P(403.2, 162), P(415.2, 207)], fill=RED_HI, width=1)
    d.line([P(402.5, 162), P(400, 207)], fill=RED, width=3)
    d.line([P(400.5, 165), P(399, 207)], fill=YL, width=1)
    # the post on down to the floor
    a, b = P(416.5, 207), P(416.5, 256)
    d.rectangle((a[0] - 1, a[1], a[0] + 8, b[1] + 1), fill=O)
    d.rectangle((a[0], a[1], a[0] + 3, b[1]), fill=RED)
    d.rectangle((a[0], a[1], a[0], b[1]), fill=RED_HI)
    d.rectangle((a[0] + 4, a[1], a[0] + 7, b[1]), fill=YL)
    d.rectangle((a[0] + 6, a[1], a[0] + 7, b[1]), fill=YL_LO)
    lay(im, box, [(411, 205.5), (430.5, 222), (397, 258), (379, 240)], lf)
    im.transpose(Image.FLIP_LEFT_RIGHT).save(f'{t}/pieces/punk/door_main.png')
    # the upper leaf, behind the bar: cut at the bar's top
    ub = (364, 172, 404, 208)
    up = Image.new('RGBA', ((ub[2] - ub[0]) * S, (ub[3] - ub[1]) * S), (0, 0, 0, 0))
    lay(up, ub, [(368.5, 185.5), (387.5, 176), (387.5 + 24, 176 + 48), (368.5 + 24, 185.5 + 48)], leaf(False))
    up.transpose(Image.FLIP_LEFT_RIGHT).save(f'{t}/pieces/punk/door_upper.png')
    if len(sys.argv) > 2:
        sheet = Image.new('RGBA', (160 * 3, 208 * 3), (110, 116, 124, 255))
        sheet.alpha_composite(up.resize((up.width * 3, up.height * 3), Image.NEAREST), ((ub[0] - 364) * S * 3, (ub[1] - 156) * S * 3))
        sheet.alpha_composite(im.resize((im.width * 3, im.height * 3), Image.NEAREST), ((box[0] - 364) * S * 3, 0))
        sheet.convert('RGB').save(sys.argv[2])
    print('door pieces', im.size, up.size)


if __name__ == '__main__':
    main()
