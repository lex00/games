"""Where in a clip the bartender runs down a bar: the stretches of frames his legs move sideways.

    python3 runway/runs.py SPRITES_JSONL

His legs are the codes 0f, 19, 27, 36, 37, a2, a4. Prints each stretch of at least 20
frames in which their x keeps changing, with the row he is on and how far he went.
"""
import json
import sys

import os
import re

# every picture of the bartender's own entity in the kit's names: his top half or his whole body
LEGS = set(re.findall(r'"sprite-player-([0-9a-f]+)"', open(os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/names.toml')).read()))


def main():
    track = []
    rows = [json.loads(line) for line in open(sys.argv[1])]
    for row in rows:
        legs = [b for n, b in row['model'] if n[-2:] in LEGS]
        track.append((row['frame'], legs[0][0], legs[0][1]) if legs else (row['frame'], None, None))
    start = None
    still = 0
    for i in range(1, len(track)):
        f, x, y = track[i]
        px = track[i - 1][1]
        moving = x is not None and px is not None and x != px and y == track[i - 1][2]
        if moving:
            if start is None:
                start = i - 1
            still = 0
        elif start is not None:
            still += 1
            if still > 6:
                end = i - still
                if track[end][0] - track[start][0] >= 20:
                    codes = sorted({n[-2:] for line2 in rows[start:end] for n, b in line2['model'] if n[-2:] in LEGS})
                    below = sorted({(n[-2:], b[0] - track[start + 2][1], b[1] - track[start + 2][2]) for n, b in rows[start + 2]['model'] if 0 < b[1] - track[start + 2][2] <= 40 and abs(b[0] - track[start + 2][1]) <= 24})
                    print('frames', track[start][0], 'to', track[end][0], 'row y', track[start][2], 'x', track[start][1], 'to', track[end][1], 'his pictures', codes, 'under him', below)
                start = None
                still = 0


if __name__ == '__main__':
    main()
