"""The part a set of the board's pictures share: the pixels that are the same colour in every one.

    python3 runway/shared_part.py OUT_PNG [CODE...]

The guests' drinking pictures all hold the board's own mug. Where every picture has the same
colour at a pixel, and that colour is not the sheet's backdrop, the pixel is part of the thing
they share. Prints the shared part as rows of text (# shared, . not), its box, and for each
picture how many of its pixels inside the box are its own (the hand, the face) and which
colours those are. Writes the shared part enlarged, to look at. For measuring only.
"""
import sys

from PIL import Image

sys.path.insert(0, __import__('os').path.dirname(__file__))
from drinkers import DRINK, SHEET, cell, codes  # noqa: E402

BACK = (0x60, 0x60, 0x68)


def main():
    out = sys.argv[1]
    want = [c.lower() for c in sys.argv[2:]] or DRINK
    sheet = Image.open(SHEET).convert('RGB')
    order = codes()
    ims = [cell(sheet, order, c).load() for c in want]
    shared = [[all(ims[k][x, y] == ims[0][x, y] for k in range(len(ims))) and ims[0][x, y] != BACK for x in range(32)] for y in range(32)]
    xs = [x for y in range(32) for x in range(32) if shared[y][x]]
    ys = [y for y in range(32) for x in range(32) if shared[y][x]]
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
    print('shared pixels', len(xs), 'box x', x0, x1, 'y', y0, y1)
    for y in range(y0, y1 + 1):
        print(''.join('#' if shared[y][x] else '.' for x in range(x0, x1 + 1)))
    for c, im in zip(want, ims):
        own = {}
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                if not shared[y][x] and im[x, y] != BACK:
                    own[im[x, y]] = own.get(im[x, y], 0) + 1
        print(c, 'own pixels in the box:', sum(own.values()), ' '.join(f'{r:02x}{g:02x}{b:02x}:{n}' for (r, g, b), n in sorted(own.items(), key=lambda t: -t[1])))
    k = 16
    big = Image.new('RGB', (32 * k, 32 * k), (255, 0, 255))
    for y in range(32):
        for x in range(32):
            if shared[y][x]:
                big.paste(ims[0][x, y], (x * k, y * k, x * k + k, y * k + k))
    big.save(out)


if __name__ == '__main__':
    main()
