"""The guests' drinking pictures from the kit's contact sheet, side by side and enlarged, to look at.

    python3 runway/drinkers.py OUT_PNG [CODE...]

The sheet (theme-kits/tapper/board/sprites-customer.png) has 8 cells a row, each 80 wide and 92
high, the picture 64 by 64 (the board's 32 at 2) with its code under it, in code order. Prints each
picture's colours and how many pixels each has. For looking and measuring only: nothing here goes
into the theme or to an image model.
"""
import os
import re
import sys
import tomllib

from PIL import Image

SHEET = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/sprites-customer.png')
NAMES = os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/names.toml')
DRINK = ['1f', '31', '3d', '7e', '85', '8d', '95', '9d', 'b1', 'b9', 'c1', 'c9', 'd9', 'e3', 'ed', 'f7']


def codes():
    text = open(NAMES).read()
    return sorted(set(re.findall(r'sprite-customer-([0-9a-f]{2})', text)))


def cell(sheet, order, code):
    k = order.index(code)
    x, y = (k % 8) * 80, (k // 8) * 92
    return sheet.crop((x + 8, y + 4, x + 72, y + 68)).resize((32, 32), Image.NEAREST)


def main():
    out = sys.argv[1]
    want = [c.lower() for c in sys.argv[2:]] or DRINK
    sheet = Image.open(SHEET).convert('RGB')
    order = codes()
    k = 8
    big = Image.new('RGB', (len(want) * (32 * k + 8), 32 * k), (255, 0, 255))
    for i, c in enumerate(want):
        im = cell(sheet, order, c)
        big.paste(im.resize((32 * k, 32 * k), Image.NEAREST), (i * (32 * k + 8), 0))
        cols = sorted(im.getcolors(1024), reverse=True)
        print(c, ' '.join(f'{r:02x}{g:02x}{b:02x}:{n}' for n, (r, g, b) in cols))
    big.save(out)


if __name__ == '__main__':
    main()
