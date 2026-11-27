"""Write the space room's bar rules and the wider tunnel's place into the theme's art.txt.

    python3 runway/wire_spacebar.py THEME_DIR
"""
import re
import sys

from PIL import Image


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    test = lambda R, G, B: f'r>0+{R-1} 0>r-{R+1} g>0+{G-1} 0>g-{G+1} b>0+{B-1} 0>b-{B+1}'.replace('+-', '-')
    bay = Image.open(f'{theme}/pieces/space/bay.png')
    a = re.sub(r'stage_piece sbaypic pieces/space/bay\.png \d+ \d+\n', f'stage_piece sbaypic pieces/space/bay.png {bay.width} {bay.height}\n', a)
    extra = (bay.width - 272) // 2
    a = re.sub(r'#\+ room_at sbay_at sbaypic -?\d+ (-?\d+)\n', lambda m: f'#+ room_at sbay_at sbaypic {-112 - extra} {m.group(1)}\n', a)
    if 'sbarpaint' not in a:
        k = a.index('stage_piece stars_far pieces/space/stars_far.png')
        a = a[:k] + 'stage_piece sbarpaint pieces/space/bar.png 1024 960\n' + a[k:]
        a = a.replace('#+ room_cells sbar tile=0x0f4 tile=0x0f5\n', '#+ room_cells sbar tile=0x0f4 tile=0x0f5 tile=0x0fa tile=0x0fb\n#+ room_cells sbarin tile=0x0f7 tile=0x0f6\n#+ room_cells sbarcap tile=0x0fd tile=0x0fc\n')
        a = re.sub(r'#\+ room_mask sbar [^\n]*\n', '', a)
        a = a.replace('#+ room_fill sbar dk\n', "# The bars: a metal counter, mirror-bright on top with a lit orange strip along its edge, one painting with a bar's\n# rows at each bar's height.\n#+ room_fill sbar sbarpaint\n")
        k = a.index('# The floating kegs:')
        caps = ''.join(f'#+ room_mask sbarcap sbarpaint {test(*c)}\n' for c in ((255, 109, 0), (109, 109, 109), (73, 73, 73), (36, 36, 36)))
        a = a[:k] + "# The bar's first cell lies inside the tunnel's port, so it is laid after the tunnel: the bar comes out of the middle\n# of the opening. Its far end keeps the board's rounded shape, the painting on the bar's own colours there.\n#+ room_fill sbarin sbarpaint\n" + caps + a[k:]
    open(p, 'w').write(a)
    print('tunnel', bay.size, 'placed', -112 - extra)


if __name__ == '__main__':
    main()
