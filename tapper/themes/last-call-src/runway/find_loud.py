"""Which shots of a folder hold the probe's loud colours, and where.

    python3 runway/find_loud.py SHOTS_DIR

For each shot, the count and the box of its magenta (glass), cyan and yellow (drink bands) pixels,
as probe_fill.py paints them. Shots with none are left out.
"""
import glob
import sys

from PIL import Image

LOUD = {'glass': (255, 0, 255), 'band': (0, 255, 255), 'band-left': (255, 255, 0)}


def main():
    for f in sorted(glob.glob(sys.argv[1] + '/*.png')):
        im = Image.open(f).convert('RGB')
        px = im.load()
        found = {}
        for y in range(0, im.height):
            for x in range(0, im.width):
                p = px[x, y]
                for name, c in LOUD.items():
                    if p == c:
                        b = found.setdefault(name, [0, x, y, x, y])
                        b[0] += 1
                        b[1], b[2], b[3], b[4] = min(b[1], x), min(b[2], y), max(b[3], x), max(b[4], y)
        if found:
            print(f.split('-')[-1][:-4], {k: (v[0], v[1:]) for k, v in found.items()})


if __name__ == '__main__':
    main()
