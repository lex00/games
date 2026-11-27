"""The disc rides with the bartender, and the rails under the bars go.

    python3 runway/ride.py THEME_DIR

Until gp has a rule that draws a cycling piece under a body, the disc is tied to the
bartender with `also` lines, one for each of his pictures the recorded plays show, held
to the four space screens: it is drawn with him wherever he stands or runs, a little
below his feet so that only its far rim crosses his shoes (an `also` picture is drawn
over its body). The disc standing at each bar's end and the thin rail under each bar are
taken out: the rail's tile and the joint's take the galaxy.
"""
import os
import re
import sys

SCREENS = 'screen_8021 screen_87a1 screen_8f21 screen_96a1'


def main():
    theme = sys.argv[1]
    p = f'{theme}/art.txt'
    a = open(p).read()
    names = open(os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/names.toml')).read()
    # The board draws him as two sprites, his top half and his legs 32 lower: the disc goes with the legs (the codes
    # seen directly under a top half in the recorded plays: runway/legs.py), so it is at his feet, not his middle.
    # Standing, pouring and walking each have their own legs; a few of his pictures are his whole body in one sprite.
    codes = [f'sprite-bartender_part-{c}' for c in ('0f', '19', '25', '27', '36', '37', '46', '48', '4a', '4c', '72', 'a2', 'a4')]
    codes += [f'sprite-player-{c}' for c in ('28', '29', '2a', '2b', '33', '68', '71')]
    # The board also lights the pad beside his bar by writing six cells of its own there (letters x33c to x341):
    # they take a clear picture, since the pad is gone.
    a = re.sub(r'pic letter-x33[c-f] [^\n]*\n|pic letter-x34[01] [^\n]*\n', '', a)
    k = a.index('pic letter-A ')
    a = a[:k] + ''.join(f'pic letter-x{c} pieces/space/none.png 16 16 0 0\n' for c in ('33c', '33d', '33e', '33f', '340', '341')) + a[k:]
    a = re.sub(r"# The bartender's pad: a hovering disc[^\n]*\n# by the pad's top-left cell[^\n]*\n#\+ room_at spad_at sdisc [^\n]*\n", '', a)
    a = a.replace('#+ room_fill sjoint st0ee\n', '')
    if '#+ room_cells srail' not in a:
        m = re.search(r'#\+ room_cells srest ([^\n]*)\n', a)
        a = a.replace(m.group(0), '#+ room_cells srail tile=0x0f1\n' + f'#+ room_cells srest {m.group(1)} !srail\n')
        a = a.replace('#+ room_fill spylon stars_far\n', '#+ room_fill spylon stars_far\n#+ room_fill srail stars_far\n')
    a = re.sub(r'#\+ on screen_8021[^\n]* also sprite-[a-z_]+-[0-9a-f]+ pieces/space/[^\n]*\n', '', a)
    a = a.replace('# A small round hover craft carries the bartender on the space screens: drawn with his legs, its deck at his feet.\n', '')
    a = a.replace("# The disc rides with the bartender on the space screens: drawn with each of his pictures, a little below his feet.\n", '')
    k = a.index('#+ room screen_8021')
    lines = "# The disc rides with the bartender on the space screens: drawn with each of his pictures, a little below his feet.\n" + ''.join(f'#+ on {SCREENS} also {c} pieces/space/disc.png 52 36 -10 14\n' for c in codes)
    a = a[:k] + lines + a[k:]
    open(p, 'w').write(a)
    print('disc tied to', len(codes), 'pictures of the bartender; rails and standing discs removed')


if __name__ == '__main__':
    main()
