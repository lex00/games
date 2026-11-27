"""Add the tunnel lamps' rules to the theme's art.txt.

    python3 runway/wire_lamps.py THEME_DIR
"""
import os
import re
import sys


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    lines = open(os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/lamps-rules.txt')).read().split('\n')
    w, h = lines[0].split()
    cycles = [l for l in lines[1:] if l]
    m = re.search(r'#\+ room_at sbay_at sbaypic (-?\d+) (-?\d+)\n', a)
    dx, dy = m.group(1), m.group(2)
    a = re.sub(r'stage_piece lamps\d [^\n]*\n', '', a)
    a = re.sub(r'stage_piece snone [^\n]*\n', '', a)
    a = re.sub(r'#\+ room_react sbay_at -?\d+ -?\d+ [^\n]*lamps\d[^\n]*\n', '', a)
    a = a.replace("# The tunnels' red lamps come on and go off, each group on a cycle of its own length (the lengths share no factor),\n# so no pattern shows; by the game's frame number, so a clip replays the same.\n", '')
    k = a.index('stage_piece stars_far pieces/space/stars_far.png')
    a = a[:k] + ''.join(f'stage_piece lamps{i} pieces/space/lamps{i}.png {w} {h}\n' for i in range(len(cycles))) + 'stage_piece snone pieces/space/none.png 2 2\n' + a[k:]
    k = a.index('# The floating kegs:')
    a = a[:k] + "# The tunnels' red lamps come on and go off, each group on a cycle of its own length (the lengths share no factor),\n# so no pattern shows; by the game's frame number, so a clip replays the same.\n" + ''.join(f'#+ room_react sbay_at {dx} {dy} {c}\n' for c in cycles) + a[k:]
    open(p, 'w').write(a)
    print('lamp groups wired', len(cycles))


if __name__ == '__main__':
    main()
