"""Paint a scratch copy's drinking glasses and drink bands in flat loud colours, to find where the page draws them.

    python3 runway/probe_fill.py SCRATCH_THEME_DIR

Every art/drink/glass-*.png becomes magenta wherever it has any pixel, and beer-15.png and
beer-15-left.png become cyan and yellow. For a scratch copy only.
"""
import glob
import sys

from PIL import Image


def flat(path, colour):
    im = Image.open(path).convert('RGBA')
    px = im.load()
    for y in range(im.height):
        for x in range(im.width):
            if px[x, y][3] > 0:
                px[x, y] = colour + (255,)
    im.save(path)


def main():
    t = sys.argv[1]
    for f in glob.glob(f'{t}/art/drink/glass-*.png'):
        flat(f, (255, 0, 255))
    flat(f'{t}/art/obj/beer-15.png', (0, 255, 255))
    flat(f'{t}/art/obj/beer-15-left.png', (255, 255, 0))
    print('painted')


if __name__ == '__main__':
    main()
