"""Centre the Last Call sign on the back wall of each saloon.

    python3 runway/centre_sign.py THEME_DIR

The sign was laid over the board's own sign cells (`room_box`), which stand left of the
wall's middle: the first saloon's back wall runs game x 128 to 352 (middle 240), the
second's 144 to 368 (middle 256), and the sign's cells have their middle at 232 in both.
The sign (144 game pixels wide) is now placed by its first cell (tile 0x064, at x 160,
y 16) so its middle is the wall's: 8 right in the first saloon, 24 right in the second.
"""
import sys


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    for head, dx in (('#+ room screen_saloon screen_a5a1\n', 8), ('#+ room screen_9e21\n', 24)):
        start = a.index(head)
        end = a.index('#+ room ', start + 10)
        block = a[start:end]
        if '#+ room_cells sign_at ' not in block:
            i = block.index('#+ room_cells sign ')
            j = block.index('\n', i) + 1
            block = block[:j] + '#+ room_cells sign_at tile=0x064\n' + block[j:]
        block = block.replace('#+ room_box sign sign\n', f"# the sign's middle on the back wall's middle (its own cells stand left of it)\n#+ room_at sign_at sign {dx} -16\n")
        a = a[:start] + block + a[end:]
    open(p, 'w').write(a)
    print('sign centred in both saloons')


if __name__ == '__main__':
    main()
