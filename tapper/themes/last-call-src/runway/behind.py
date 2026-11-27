"""Lay every patch of the galaxy before the things that fly through it.

    python3 runway/behind.py THEME_DIR

A line that comes after a moving piece and lies over it is drawn over it. The galaxy
laid on the cells of the tunnels, kegs, pad, pylons, rails and joints, and on the void's
black in every other cell, came after the planets, comet, convoy and ship, so each of
those cells was a square of galaxy the comet went behind. They are moved up to just
after the void's own stars; what stands in those cells stays after, in front.
"""
import re
import sys


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    moved = []
    for name in ('sbay', 'skeg', 'spad', 'spylon', 'srail', 'sjoint'):
        line = f'#+ room_fill {name} stars_far\n'
        if line in a:
            a = a.replace(line, '', 1)
            moved.append(line)
    line = '#+ room_fill srest dk\n'
    if line in a:
        a = a.replace(line, '', 1)
        moved.append(line)
    m = re.search(r'#\+ room_mask srest stars_far [^\n]*\n', a)
    if m:
        a = a.replace(m.group(0), '', 1)
        moved.append(m.group(0))
    anchor = '#+ room_scroll stars_near -1 6\n'
    assert a.count(anchor) == 1
    a = a.replace(anchor, anchor + "# The galaxy on every other cell too, before anything flies through it: the cells of the tunnels, kegs, pad, pylons\n# and rails whole, and the void's black wherever else it shows.\n" + ''.join(moved))
    open(p, 'w').write(a)
    print('moved before the moving pieces:', len(moved), 'lines')


if __name__ == '__main__':
    main()
