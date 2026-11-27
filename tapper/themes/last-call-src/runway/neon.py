"""The space bars' neon flashes, on the board's own light cycle.

    python3 runway/neon.py THEME_DIR

The board lights the space room's bars by stepping three colour slots in turn, 5 frames
each (the model's counter cycle_step: 1, 2, 3, round again while the cycle runs, 0 when
it is off). Here each step lights the bartops in a neon colour: on step 1 the first and
third bars cyan, on step 2 the second and fourth magenta, on step 3 all four lime.

A flash is one cell-sized picture laid on every top-row cell of a bar (the bartop is the
same along its length, so one cell's picture does for all): the bartop's own rows from
the bar painting, pushed toward the neon colour, brightest along the slab's middle.
Writes pieces/space/neon_cyan.png, neon_magenta.png, neon_lime.png and the rules.
"""
import re
import sys

from PIL import Image

COLOURS = {'cyan': (70, 235, 255), 'magenta': (255, 80, 225), 'lime': (160, 255, 90), 'orange': (255, 150, 40),
           'violet': (170, 110, 255), 'gold': (255, 226, 80), 'rose': (255, 90, 120), 'teal': (60, 255, 190)}
# The owner, 2026-11-24: slower than the board's own beat, a colour held a few seconds, and more colours. So each
# pair of bars has a cycle of its own by the game's frame: a colour for 3 seconds, the plain bartop for 1.5, the next.
HOLD, REST = 180, 90
COLOURS.update({'blue': (70, 130, 255), 'red': (255, 60, 50), 'ice': (210, 240, 255), 'pink': (255, 150, 220),
                'aqua': (90, 220, 230), 'amber': (255, 190, 60), 'green': (70, 235, 110), 'purple': (210, 80, 255)})
import random  # noqa: E402
ORDER_A = sorted(COLOURS)
ORDER_B = sorted(COLOURS)
random.Random(3).shuffle(ORDER_A)
random.Random(11).shuffle(ORDER_B)


def main():
    theme = sys.argv[1]
    bar = Image.open(f'{theme}/pieces/space/bar.png').convert('RGB')
    cell = bar.crop((64, 224, 96, 256))
    for name, (R, G, B) in COLOURS.items():
        out = Image.new('RGBA', (32, 32), (0, 0, 0, 0))
        o = out.load()
        for y in range(32):
            t = 1.0 - abs(y - 15) / 22.0                 # strongest across the slab's middle
            for x in range(32):
                r, g, b = cell.getpixel((x, y))
                lum = (r + g + b) / 3 / 255.0
                k = 0.55 + 0.45 * t
                o[x, y] = (int(r * (1 - k) + R * k * (0.6 + 0.4 * lum)), int(g * (1 - k) + G * k * (0.6 + 0.4 * lum)), int(b * (1 - k) + B * k * (0.6 + 0.4 * lum)), 255)
        for x in range(32):
            o[x, 15] = (255, 255, 255, 255) if x % 8 < 5 else o[x, 15]
        out.save(f'{theme}/pieces/space/neon_{name}.png')
    p = f'{theme}/art.txt'
    a = open(p).read()
    a = re.sub(r'stage_piece neon_\w+ [^\n]*\n', '', a)
    a = re.sub(r'#\+ room_cells sbartop[ab] [^\n]*\n', '', a)
    a = re.sub(r"# The bars flash neon[^\n]*\n(# [^\n]*\n)*(#\+ room_react sbartop[ab] [^\n]*\n(#\+ room_react_when [^\n]*\n)*)+", '', a)
    k = a.index('stage_piece stars_far pieces/space/stars_far.png')
    a = a[:k] + ''.join(f'stage_piece neon_{n} pieces/space/neon_{n}.png 32 32\n' for n in COLOURS) + a[k:]
    a = a.replace('#+ room_cells sbarin tile=0x0f7 tile=0x0f6\n', '#+ room_cells sbarin tile=0x0f7 tile=0x0f6\n#+ room_cells sbartopa tile=0x0f4 tile=0x0f7 tile=0x0fa tile=0x0fd\n')
    a = re.sub(r'#\+ room_react_stagger 97 rows\n', '', a)
    k = a.index('# The floating kegs:')
    # One sequence for every bar, each colour once, held an uneven while (2.5 to 4.5 seconds) with an uneven rest
    # after it; every cell of a bar is on the bar's row, and each row runs 97 frames a row ahead of the row above
    # (`room_react_stagger 97 rows`). The bars are 6 rows apart, so each is 582 frames ahead of the one above:
    # further than the longest colour and its rest together, so no two bars ever show the same colour, and not a whole
    # number of steps, so no two change at the same moment.
    rnd = random.Random(29)
    steps = ' '.join(f'neon_{c}:{rnd.randrange(150, 271, 10)} snone:{rnd.randrange(50, 101, 10)}' for c in ORDER_A)
    a = a[:k] + ("# The bars flash neon: each bartop glows a colour for a few seconds, rests as plain bartop, then takes the next of\n"
                 "# sixteen. Every bar runs the same sequence from a different point, so each changes at its own moment and no two\n"
                 "# bars show the same colour at once.\n"
                 f"#+ room_react sbartopa 0 0 {steps}\n#+ room_react_stagger 97 rows\n") + a[k:]
    open(p, 'w').write(a)
    print('neon flashes wired')


if __name__ == '__main__':
    main()
