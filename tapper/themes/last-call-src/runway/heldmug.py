"""The guest's hand on the handle of the mug he holds.

    python3 runway/heldmug.py THEME_DIR

The board's held-mug pictures (customer 23, dd, f1 and fb, the attract scene's 23 and the attract demo's, a bonus_object) are a
full mug with the guest's hand gripping its handle, the hand in colour number 12, which the
board's palette gives each guest's own skin. Our mug-23.png replaced the whole picture, so no
guest had a hand on the handle.

Now the board's picture is drawn with everything but the hand left out (`#+ recolor`: the
mug's colour numbers 2, 4, 7, 10, 14 and 15 as nothing everywhere, and the outline's 9 as
nothing outside the hand's box, x 0 to 5 and y 15 to 22), and our mug is drawn over it
(`also`) with a hole where the hand's pixels are, so the hand shows in front of the glass
handle in the guest's own colour. HAND is the hand's pixels in that box (# kept), read from
the board's picture as the page draws it; it is a shape only.
Writes art/obj/mug-held.png and the rules.
"""
import re
import sys

from PIL import Image

HAND = [
    '###.##',
    '######',
    '######',
    '######',
    '######',
    '######',
    '######',
    '######',
]
X0, Y0 = 0, 15
NAMES = ['sprite-customer-23', 'sprite-customer-dd', 'sprite-customer-f1', 'sprite-customer-fb', 'sprite-scene_customer-23', 'sprite-bonus_object-23']


def main():
    theme = sys.argv[1]
    mug = Image.open(f'{theme}/art/obj/mug-23.png').convert('RGBA')
    px = mug.load()
    for j, row in enumerate(HAND):
        for i, c in enumerate(row):
            if c == '#':
                for dy in range(2):
                    for dx in range(2):
                        px[(X0 + i) * 2 + dx, (Y0 + j) * 2 + dy] = (0, 0, 0, 0)
    mug.save(f'{theme}/art/obj/mug-held.png')
    p = f'{theme}/art.txt'
    a = open(p).read()
    a = re.sub(r'^(?:pic|also) (?:' + '|'.join(NAMES) + r') art/obj/mug-(?:23|held)\.png .*\n', '', a, flags=re.M)
    a = re.sub(r'^#\+ recolor (?:' + '|'.join(NAMES) + r') .*\n', '', a, flags=re.M)
    a = a.replace("# A held mug keeps the board's hand on its handle (runway/heldmug.py).\n", '')
    x1, y1 = X0 + len(HAND[0]) - 1, Y0 + len(HAND) - 1
    lines = "# A held mug keeps the board's hand on its handle (runway/heldmug.py).\n"
    for n in NAMES:
        lines += f'#+ recolor {n} 2=- 4=- 7=- 10=- 14=- 15=-\n'
        lines += f'#+ recolor {n} box={x1 + 1},0,31,31 9=- 12=-\n'
        lines += f'#+ recolor {n} box=0,0,{x1},{Y0 - 1} 9=- 12=-\n'
        lines += f'#+ recolor {n} box=0,{y1 + 1},{x1},31 9=- 12=-\n'
        lines += f'also {n} art/obj/mug-held.png 32 32 0 0\n'
    k = a.index('pic letter-A ')
    a = a[:k] + lines + a[k:]
    open(p, 'w').write(a)
    print('held mug written for', len(NAMES), 'pictures')


if __name__ == '__main__':
    main()
