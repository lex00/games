"""The space bar's bays: starship hull plating, one painting the size of the screen.

    python3 runway/hull.py THEME_DIR [PREVIEW_PNG]

Plates of many sizes cut by recursive halving, each a slightly different gunmetal with a
lit top-left bevel and a shadowed bottom-right, and on them the clutter a hull carries:
rivets, louvred vents, round hatches, small pipes with collars running between plates,
and a few tiny lamps.
"""
import random
import sys

from PIL import Image, ImageDraw

W, H = 1024, 960
O = (16, 18, 26)


def main():
    t = sys.argv[1]
    rnd = random.Random(7)
    im = Image.new('RGB', (W, H), O)
    d = ImageDraw.Draw(im)
    plates = []

    def cut(x0, y0, x1, y1, depth):
        w, h = x1 - x0, y1 - y0
        if depth > 0 and (w > 84 or h > 84 or (rnd.random() < 0.7 and min(w, h) > 30)):
            if w >= h:
                m = x0 + int(w * rnd.uniform(0.35, 0.65)) // 4 * 4
                cut(x0, y0, m, y1, depth - 1); cut(m, y0, x1, y1, depth - 1)
            else:
                m = y0 + int(h * rnd.uniform(0.35, 0.65)) // 4 * 4
                cut(x0, y0, x1, m, depth - 1); cut(x0, m, x1, y1, depth - 1)
        else:
            plates.append((x0, y0, x1, y1))

    cut(0, 0, W, H, 10)
    for (x0, y0, x1, y1) in plates:
        g = rnd.choice([78, 86, 94, 102, 112])
        base = (g - 6, g, g + 12)
        hi = tuple(min(255, v + 46) for v in base)
        lo = tuple(max(0, v - 34) for v in base)
        d.rectangle((x0 + 1, y0 + 1, x1 - 2, y1 - 2), fill=base)
        d.line((x0 + 1, y0 + 1, x1 - 2, y0 + 1), fill=hi); d.line((x0 + 1, y0 + 1, x0 + 1, y1 - 2), fill=hi)
        d.line((x0 + 1, y1 - 2, x1 - 2, y1 - 2), fill=lo); d.line((x1 - 2, y0 + 1, x1 - 2, y1 - 2), fill=lo)
        w, h = x1 - x0, y1 - y0
        kind = rnd.random()
        if w > 20 and h > 20:
            for (rx, ry) in ((x0 + 5, y0 + 5), (x1 - 7, y0 + 5), (x0 + 5, y1 - 7), (x1 - 7, y1 - 7)):
                d.rectangle((rx, ry, rx + 1, ry + 1), fill=hi); d.point((rx + 1, ry + 1), fill=lo)
        if kind < 0.22 and w > 36 and h > 28:                       # a louvred vent
            vx0, vy0, vx1, vy1 = x0 + 10, y0 + 10, x1 - 11, y1 - 11
            d.rectangle((vx0, vy0, vx1, vy1), fill=lo)
            for y in range(vy0 + 2, vy1 - 1, 4):
                d.line((vx0 + 2, y, vx1 - 2, y), fill=O); d.line((vx0 + 2, y + 1, vx1 - 2, y + 1), fill=base)
        elif kind < 0.36 and w > 34 and h > 34:                     # a round hatch
            cx, cy, r = (x0 + x1) // 2, (y0 + y1) // 2, min(w, h) // 2 - 9
            d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=O)
            d.ellipse((cx - r + 2, cy - r + 2, cx + r - 2, cy + r - 2), fill=tuple(max(0, v - 14) for v in base))
            d.arc((cx - r + 2, cy - r + 2, cx + r - 2, cy + r - 2), 180, 300, fill=hi)
            d.line((cx - r + 5, cy, cx + r - 5, cy), fill=lo)
        elif kind < 0.5 and w > 28 and h > 16:                      # a dent: a pressed-in panel
            d.rectangle((x0 + 8, y0 + 7, x1 - 9, y1 - 8), fill=tuple(max(0, v - 12) for v in base))
            d.line((x0 + 8, y0 + 7, x1 - 9, y0 + 7), fill=lo); d.line((x0 + 8, y0 + 7, x0 + 8, y1 - 8), fill=lo)
            d.line((x0 + 8, y1 - 8, x1 - 9, y1 - 8), fill=hi); d.line((x1 - 9, y0 + 7, x1 - 9, y1 - 8), fill=hi)
        elif kind < 0.58 and w > 24 and h > 14:                     # a lamp
            lx, ly = x0 + rnd.randint(8, w - 12), y0 + rnd.randint(6, h - 10)
            c = rnd.choice([(255, 150, 40), (80, 230, 255), (255, 70, 60)])
            d.rectangle((lx - 1, ly - 1, lx + 3, ly + 3), fill=O); d.rectangle((lx, ly, lx + 2, ly + 2), fill=c); d.point((lx, ly), fill=(255, 255, 255))
    # small pipes over the plates: runs with collars and elbows
    for _ in range(110):
        x, y = rnd.randrange(0, W, 4), rnd.randrange(0, H, 4)
        col = rnd.choice([(150, 156, 170), (176, 120, 70), (120, 128, 144)])
        hi = tuple(min(255, v + 60) for v in col); lo = tuple(max(0, v - 60) for v in col)
        for seg in range(rnd.randint(2, 4)):
            horiz = (seg % 2 == 0) ^ (rnd.random() < 0.2)
            n = rnd.randrange(24, 96, 4)
            if horiz:
                d.rectangle((x, y - 1, x + n, y + 4), fill=O); d.rectangle((x, y, x + n, y + 3), fill=col)
                d.line((x, y, x + n, y), fill=hi); d.line((x, y + 3, x + n, y + 3), fill=lo)
                for cx in range(x + 8, x + n - 4, 20):
                    d.rectangle((cx, y - 2, cx + 4, y + 5), fill=O); d.rectangle((cx + 1, y - 1, cx + 3, y + 4), fill=hi)
                x += n
            else:
                d.rectangle((x - 1, y, x + 4, y + n), fill=O); d.rectangle((x, y, x + 3, y + n), fill=col)
                d.line((x, y, x, y + n), fill=hi); d.line((x + 3, y, x + 3, y + n), fill=lo)
                for cy in range(y + 8, y + n - 4, 20):
                    d.rectangle((x - 2, cy, x + 5, cy + 4), fill=O); d.rectangle((x - 1, cy + 1, x + 4, cy + 3), fill=hi)
                y += n
            d.rectangle((x - 2, y - 2, x + 5, y + 5), fill=O); d.rectangle((x - 1, y - 1, x + 4, y + 4), fill=col); d.point((x, y), fill=hi)
    im.save(f'{t}/pieces/space/hull.png')
    if len(sys.argv) > 2:
        im.crop((0, 100, 420, 420)).resize((840, 640), Image.NEAREST).save(sys.argv[2])
    print('hull written')


if __name__ == '__main__':
    main()
