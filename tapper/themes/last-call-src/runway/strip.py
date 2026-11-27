"""Lay a folder of shots side by side, cropped to one box, to compare a spot across moments.

    python3 runway/strip.py SHOTS_DIR OUT_PNG [X0 Y0 X1 Y1]

The box is in the shots' own pixels (1024 by 960); without one, each whole shot at a third.
"""
import glob
import os
import sys

from PIL import Image


def main():
    files = sorted(f for f in glob.glob(os.path.join(sys.argv[1], '*.png')) if 'original' not in os.path.basename(f))
    box = tuple(int(v) for v in sys.argv[3:7]) if len(sys.argv) >= 7 else None
    tiles = []
    for f in files:
        im = Image.open(f).convert('RGB')
        tiles.append(im.crop(box).resize(((box[2] - box[0]) * 2, (box[3] - box[1]) * 2), Image.NEAREST) if box else im.resize((341, 320)))
    out = Image.new('RGB', (sum(t.width for t in tiles) + 4 * len(tiles), max(t.height for t in tiles)), 'white')
    x = 0
    for t in tiles:
        out.paste(t, (x, 0))
        x += t.width + 4
    out.save(sys.argv[2])
    print(len(tiles), 'shots:', [os.path.basename(f)[-14:-4] for f in files])


if __name__ == '__main__':
    main()
