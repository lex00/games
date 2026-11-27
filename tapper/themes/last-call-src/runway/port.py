"""Open the tunnel's port: black everywhere inside the hazard line that outlines the opening.

    python3 runway/port.py THEME_DIR [PREVIEW_PNG]

The opening is the tall shape at the tunnel's end whose outer edge is the hazard-striped
line: it runs down from the top tip, and a second hazard-striped edge closes the shape
below. Row by row, the port is everything from just inside the first hazard edge across
to the opening's other side: the thin upright at the right above the bracket, the piece's
own edge at the bracket's height (where the bar comes out), and the second hazard edge
below it. All of that becomes black; the hazard line itself stays as the port's rim.
Works from the piece kept aside before any port was made (bay-solid.png).
"""
import os
import sys

from PIL import Image

SRC = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/bay-solid.png')
BLACK = (8, 6, 12, 255)


def hazard(p):
    r, g, b, a = p
    return a and r > 170 and g > 115 and b < 110 and r - b > 90


def runs(px, y, w):
    out, x = [], 150
    while x < w:
        if hazard(px[x, y]):
            a = x
            while x < w and (hazard(px[x, y]) or (x + 1 < w and hazard(px[x + 1, y])) or (x + 2 < w and hazard(px[x + 2, y]))):
                x += 1
            out.append((a, x - 1))
        else:
            x += 1
    return out


def main():
    theme = sys.argv[1]
    im = Image.open(SRC).convert('RGBA')
    px = im.load()
    w, h = im.size
    rows = {y: runs(px, y, w) for y in range(h)}
    first = {y: r[0] for y, r in rows.items() if r}
    ys = sorted(first)
    top, foot = ys[0], ys[-1]
    # carry the first hazard edge through rows where its black stripe hides the yellow
    last = None
    edge = {}
    for y in range(top, foot + 1):
        # (a row whose first stripe is black shows the opening's other striped edge first: not a jump to follow)
        if y in first and (last is None or abs(first[y][1] - last[1]) <= 4):
            last = first[y]
        edge[y] = last
    # The port's other side is one clean line: from the top tip out to the piece's edge at the bar's height
    # (rows 96 to 168, where the bar comes out), then back in to the foot of the hazard line. Everything the picture
    # had right of the hazard line (a bracket, a rod, a second striped edge) is taken away first.
    # The far edge runs parallel to the near one all the way down: the same line, DEEP stage pixels further out, so
    # the opening is one band of even width with the hazard line's own bend in it. The piece is widened to hold it.
    # The opening ends just under the bar that comes out of it (the bar's foot is 160 down the piece); below that the
    # tunnel's end is closed, only the hazard line running on down.
    WIDE0, WIDE1 = 96, 162
    PORT_FOOT = foot          # the owner, 2026-11-24: ending it under the bar left its foot out of line with the tunnel's; full height again
    DEEP = 46
    grown = Image.new('RGBA', (edge[top][1] + DEEP + 10, h), (0, 0, 0, 0))
    grown.paste(im, (0, 0))
    im = grown
    px = im.load()
    w = im.width
    def far(y):
        e = edge[y][1] if y in edge else (edge[top][1] if y < top else edge[foot][1])
        return e + 3 + DEEP
    n = 0
    for y in range(h):
        a = (edge[y][1] + 3) if y in edge else (edge[top][1] + 3 if y < top else edge[foot][1] + 3)
        for x in range(a, w):
            px[x, y] = (0, 0, 0, 0)
        if top + 1 <= y <= PORT_FOOT - 1:
            for x in range(a, int(far(y)) + 1):
                px[x, y] = BLACK
                n += 1
    # The opening's other two sides carry the same hazard stripes as the two the picture has, so all four of its
    # sides are striped: a band 6 wide along the far line above the bar and below it, yellow and black by turns.
    YEL, YLO, BLK, RIM = (236, 170, 40, 255), (176, 116, 22, 255), (20, 18, 22, 255), (60, 62, 66, 255)
    # its top and its foot are closed by the same stripes, level
    for cap in (range(top + 1, top + 7), range(PORT_FOOT - 6, PORT_FOOT)):
        for y in cap:
            a = edge[y][1] + 3
            for x in range(a, int(far(y)) + 1):
                col = YEL if ((x - a) // 7) % 2 == 0 else BLK
                px[x, y] = RIM if y in (cap[0], cap[-1]) else col
    for y in range(top + 1, PORT_FOOT):
        if WIDE0 <= y <= WIDE1:
            continue
        fx = int(far(y))
        col = YEL if (y // 7) % 2 == 0 else BLK
        for k in range(7):
            x = fx - k
            if x > edge.get(y, edge[foot])[1] + 3 and 0 <= x < w:
                px[x, y] = RIM if k in (0, 6) else (YLO if col == YEL and k == 5 else col)
    # The tunnel goes on off the screen: more of its ribs are added on its outer side (the same rib and wall, a rib's
    # spacing apart), so a bay that reaches further across the screen than this one is covered to the screen's edge.
    PERIOD, FIRST, MORE = 46, 8, 5
    cell = im.crop((FIRST, 0, FIRST + PERIOD, h))
    wide = Image.new('RGBA', (w + PERIOD * MORE, h), (0, 0, 0, 0))
    wide.paste(im, (PERIOD * MORE, 0))
    for i in range(MORE):
        wide.paste(cell, (FIRST + PERIOD * i, 0))
    wide.paste(im.crop((0, 0, FIRST, h)), (0, 0))
    im = wide
    w = im.width
    print('piece now', im.size, 'added on the outer side:', PERIOD * MORE, 'stage pixels')
    im.save(f'{theme}/pieces/space/bay.png')
    print('port made black:', n, 'pixels, rows', top, 'to', foot, '; first edge at top', edge[top], 'at foot', edge[foot])
    if len(sys.argv) > 2:
        im.resize((w * 3, h * 3), Image.NEAREST).save(sys.argv[2])


if __name__ == '__main__':
    main()
