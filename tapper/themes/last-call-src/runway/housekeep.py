"""Housekeeping on the theme: three rules that no longer find a cell, and the record of where each picture came from.

    python3 runway/housekeep.py THEME_ROOT

1. In the attract demo screen's room, three lines name classes that find no cell on that
   screen (its tiles 0x008, 0x015 and 0x01c are not on it): the two fills and the leaf
   placed by the last are taken out of that room only. The same lines in the saloons'
   rooms, where the cells exist, stay.
2. theme/made-from.txt: Last Call's pictures were not fitted from sheets' cells. Each was
   made for the one game picture it stands for and placed there by hand: painted by an
   image model on Runway from our own words (and, for five pieces of the punk and space
   rooms, from a crop of the board's own drawing of that piece, on the owner's word), or
   drawn by a script in runway/. So every `pic` line's game picture is listed as `placed`.
"""
import re
import sys


def main():
    root = sys.argv[1]
    p = f'{root}/theme/art.txt'
    a = open(p).read()
    start = a.index('#+ room screen_attract_demo\n')
    end = a.index('#+ room ', start + 10)
    block = a[start:end]
    n = 0
    for line in ('#+ room_fill t008 t008\n', '#+ room_fill t015 t015\n', '#+ room_at top_leaf t007 0 16\n'):
        if line in block:
            block = block.replace(line, '', 1)
            n += 1
    a = a[:start] + block + a[end:]
    open(p, 'w').write(a)
    names = []
    for m in re.finditer(r'^(?:#\+ on [^\n]*? )?(?:pic|timed|also) (\S+) ', a, re.M):
        if m.group(1) not in names:
            names.append(m.group(1))
    head = ("gp-made-from 1\n"
            "# Where each picture of Last Call came from (`gp theme look check` reads it). None was fitted from a sheet's\n"
            "# cell: each was made for the one game picture it stands for and placed there by hand, at that picture's own\n"
            "# box. They were painted by image models on Runway (gpt_image_2, gen4_image) from our own words and our own\n"
            "# earlier pictures, 2026-11-16 to 2026-11-24, or drawn by the scripts kept with the work (runway/*.py).\n"
            "# Five pieces of the punk and space rooms (the window, the tunnel, the keg, the bar, the pad) were painted\n"
            "# with a crop of the board's own drawing of that piece shown to the model as a guide to its shape, on the\n"
            "# owner's word in session, 2026-11-24; no pixel of the board's is in any picture here.\n")
    body = ''.join('placed ' + ' '.join(names[i:i + 8]) + '\n' for i in range(0, len(names), 8))
    open(f'{root}/theme/made-from.txt', 'w').write(head + body)
    print('rules removed from the attract screen:', n, '; pictures listed as placed:', len(names))


if __name__ == '__main__':
    main()
