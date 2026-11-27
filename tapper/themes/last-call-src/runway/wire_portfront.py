"""Add the port front (the bar's fade into the port and the docking collar) to the theme's art.txt, and square the bars' ends.

    python3 runway/wire_portfront.py THEME_DIR
"""
import re
import sys

from PIL import Image


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    pf = Image.open(f'{theme}/pieces/space/portfront.png')
    m = re.search(r'#\+ room_at sbay_at sbaypic (-?\d+) (-?\d+)\n', a)
    dx, dy = m.group(1), m.group(2)
    a = re.sub(r'stage_piece sportfront [^\n]*\n', '', a)
    a = re.sub(r'#[^\n]*\n#[^\n]*\n#\+ room_at sbay_at sportfront [^\n]*\n', '', a)
    k = a.index('stage_piece stars_far pieces/space/stars_far.png')
    a = a[:k] + f'stage_piece sportfront pieces/space/portfront.png {pf.width} {pf.height}\n' + a[k:]
    # the bars' free ends: the whole end cell takes the bar, so the end is square and none of the board's cap shows
    a = re.sub(r'#\+ room_mask sbarcap sbarpaint [^\n]*\n', '', a)
    if '#+ room_fill sbarcap sbarpaint\n' not in a:
        a = a.replace('#+ room_fill sbarin sbarpaint\n', '#+ room_fill sbarin sbarpaint\n#+ room_fill sbarcap sbarpaint\n')
    k = a.index('# The floating kegs:')
    a = a[:k] + "# In front of each port, after the bars: the bar carried back into the port's dark, darker the deeper it lies, and a\n# docking collar where it leaves, so the bar comes out of the tunnel and does not start at a line.\n" + f'#+ room_at sbay_at sportfront {dx} {dy}\n' + a[k:]
    open(p, 'w').write(a)
    print('port front placed', dx, dy)


if __name__ == '__main__':
    main()
