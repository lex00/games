"""The last small pictures that are not characters, and the bonus round's card.

    python3 runway/leftovers.py THEME_DIR

1. The bonus round's card (screen_bonus_card): the board shows its brand's logo on a plain
   blue screen. Ours is a navy card with the Last Call sign in the middle, where the
   board's logo is: a room of its own, every cell the navy, the sign placed by the logo's
   first cell.
2. The three bar-end sprites the plays show (4e, 74, 7b) are empty pictures on the board:
   they take a clear picture, so nothing of the board's is drawn for them.
3. The high score table's two counters still on the board's digits take our 0; the
   self-test's colour bars at power-on are left off, so the game boots from black to the
   Last Call logo.
4. The letter Z takes our Z, and the board's blank text cell (x000) a clear picture.
Prints where the pours are in the punk and space clips, for cards.
"""
import os
import re
import sys

from PIL import Image

W = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/')


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    os.makedirs(f'{theme}/pieces/card', exist_ok=True)
    Image.new('RGB', (32, 32), (22, 30, 58)).save(f'{theme}/pieces/card/navy.png')
    Image.new('RGBA', (2, 2), (0, 0, 0, 0)).save(f'{theme}/pieces/card/none.png')
    g = []
    for line in open(W + 'cells-screen_bonus_card.txt'):
        q = line.split()
        if len(q) == 33 and q[0].isdigit():
            g.append([c[:3] for c in q[1:]])
    flat = [t for row in g for t in row]
    first = next((r, c) for r in range(30) for c in range(32) if g[r][c] != '008' and 9 <= r <= 17 and flat.count(g[r][c]) == 1)
    r0, c0 = first
    tile = g[r0][c0]
    # the sign (144 by 112 game pixels) with its middle on the logo's: cells 9..21 across, 9..17 down
    x, y = (9 * 16 + 22 * 16) // 2 - 72, (9 * 16 + 18 * 16) // 2 - 56
    if '#+ room screen_bonus_card' not in a:
        k = a.index('stage_piece stars_far pieces/space/stars_far.png')
        a = a[:k] + 'stage_piece cardnavy pieces/card/navy.png 32 32\n' + a[k:]
        a = a.rstrip('\n') + f"""
# The bonus round's card: a navy card with the Last Call sign in its middle, where the board shows its logo.
#+ room screen_bonus_card
#+ room_cells call tile=0x000-0x3ff
#+ room_cells card_at tile=0x{tile}
#+ room_fill call cardnavy
#+ room_at card_at sign {x - c0 * 16} {y - r0 * 16}
"""
    lines = ''
    for name, art in (('sprite-bar_end-4e', 'pieces/card/none.png'), ('sprite-bar_end-74', 'pieces/card/none.png'), ('sprite-bar_end-7b', 'pieces/card/none.png'),
                      ('counter-table_1_b-0', 'art/font/letter-0.png'), ('counter-table_2_b-0', 'art/font/letter-0.png'),
                      ('letter-Z', 'art/font/letter-Z.png'), ('letter-x000', 'pieces/card/none.png')):
        if not re.search(rf'^pic {re.escape(name)} ', a, re.M):
            size = '32 32' if name.startswith('sprite') else '16 16'
            lines += f'pic {name} {art} {size} 0 0\n'
    k = a.index('pic letter-A ')
    a = a[:k] + lines + a[k:]
    if '#+ counter_off boot_bars' not in a:
        k = a.index('#+ counter_off ')
        a = a[:k] + "# The self-test's colour bars at power-on are left off: the game boots from black to the logo.\n#+ counter_off boot_bars\n" + a[k:]
    open(p, 'w').write(a)
    print('card anchored on tile', tile, 'at', first, 'sign at game', x, y, '; pic lines added:', lines.count('\n'))


if __name__ == '__main__':
    main()
