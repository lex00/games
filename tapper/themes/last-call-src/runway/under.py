"""Switch the bartender's hover craft to gp's `under` rule, and put the tunnels' lamps out of step.

    python3 runway/under.py THEME_DIR

`#+ on SCREEN... under ENTITY DX DY PIECE:FRAMES...` draws a stage piece under every
body of an entity, cycling by the game's frame. The bartender's own entity is `player`:
his top half when he is drawn in two sprites, his whole body when in one, always at the
same height, so the craft hangs a fixed distance under that picture's box: its top 26
screen pixels below the box's foot, which is 6 above the soles of his feet (his legs'
sprite is the 32 below). The three jet frames cycle short, medium, long, medium.
The `also` lines that stood in for it are removed.
Each `room_react` of the tunnels' lamps gets a `room_react_stagger`, so each tunnel on
the screen shows its cycle a different number of frames on.
"""
import re
import sys

from PIL import Image

SCREENS = 'screen_8021 screen_87a1 screen_8f21 screen_96a1'


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    w, h = Image.open(f'{theme}/pieces/space/craft0.png').size
    a = re.sub(r'#\+ on screen_8021[^\n]* also sprite-[a-z_]+-[0-9a-f]+ pieces/space/[^\n]*\n', '', a)
    a = a.replace('# A small round hover craft carries the bartender on the space screens: drawn with his legs, its deck at his feet.\n', '')
    a = a.replace('# The disc rides with the bartender on the space screens: drawn with each of his pictures, a little below his feet.\n', '')
    a = re.sub(r'stage_piece craft\d [^\n]*\n', '', a)
    a = re.sub(r'# A small round hover craft carries the bartender[^\n]*\n(#[^\n]*\n)*#\+ on [^\n]* under player [^\n]*\n', '', a)
    k = a.index('stage_piece stars_far pieces/space/stars_far.png')
    a = a[:k] + ''.join(f'stage_piece craft{i} pieces/space/craft{i}.png {w} {h}\n' for i in range(3)) + a[k:]
    k = a.index('#+ room screen_8021')
    a = a[:k] + "# A small round hover craft carries the bartender on the space screens: drawn under him wherever he stands, pours or\n# walks, its deck at his feet, its jets flickering all the time.\n" + f'#+ on {SCREENS} under player 0 26 craft0:4 craft1:4 craft2:4 craft1:4\n' + a[k:]
    a = re.sub(r'#\+ room_react_stagger \d+\n', '', a)
    n = 0
    def stagger(m):
        nonlocal n
        n += 1
        return m.group(0) + f'#+ room_react_stagger {(41, 67, 53, 79, 37)[(n - 1) % 5]}\n'
    a = re.sub(r'#\+ room_react sbay_at -?\d+ -?\d+ [^\n]*lamps\d[^\n]*\n', stagger, a)
    open(p, 'w').write(a)
    print('craft under the bartender; lamp groups staggered:', n)


if __name__ == '__main__':
    main()
