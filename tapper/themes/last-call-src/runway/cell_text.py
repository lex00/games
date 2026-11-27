"""A guest picture from the kit's sheet as text, to read its shapes.

    python3 runway/cell_text.py CODE [X0 X1 Y0 Y1]

# black, . nothing, b the mug's blues, r red, w white, d dark blue, s anything else.
"""
import sys

from PIL import Image

sys.path.insert(0, __import__('os').path.dirname(__file__))
from drinkers import SHEET, cell, codes  # noqa: E402

SYM = {(0, 0, 0): '#', (0x60, 0x60, 0x68): '.', (0, 0x92, 0xff): 'b', (0x24, 0x92, 0xff): 'b', (255, 0, 0): 'r', (255, 255, 255): 'w', (0, 0x49, 0x92): 'd'}


def main():
    code = sys.argv[1].lower()
    x0, x1, y0, y1 = (int(v) for v in sys.argv[2:6]) if len(sys.argv) > 5 else (0, 31, 0, 31)
    im = cell(Image.open(SHEET).convert('RGB'), codes(), code).load()
    print('   ' + ''.join(str(x % 10) for x in range(x0, x1 + 1)))
    for y in range(y0, y1 + 1):
        print(f'{y:2} ' + ''.join(SYM.get(im[x, y], 's') for x in range(x0, x1 + 1)))


if __name__ == '__main__':
    main()
