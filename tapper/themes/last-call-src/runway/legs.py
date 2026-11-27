"""Which of the bartender's pictures are his legs, and what the pad's counter is called.

    python3 runway/legs.py SPRITES_JSONL...

The board draws the bartender as two sprites, his top half and, 32 pixels lower at the
same x, his legs. From clips' sprite lists this prints the codes seen directly under one
of his top halves (the pictures named sprite-player-CODE in the kit's names), with the
name each has in the kit, and every counter name in the kit that mentions a pad.
"""
import json
import os
import re
import sys
from collections import Counter


def main():
    names = open(os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/names.toml')).read()
    by_code = {}
    for ent, code in re.findall(r'"sprite-([a-z_]+)-([0-9a-f]+)"', names):
        by_code.setdefault(code, []).append(ent)
    tops = {c for c, e in by_code.items() if 'player' in e}
    under = Counter()
    alone = Counter()
    under_top = {}
    for path in sys.argv[1:]:
        for line in open(path):
            row = json.loads(line)
            boxes = {(b[0], b[1]): n[-2:] for n, b in row['model']}
            for (x, y), code in boxes.items():
                if code in tops:
                    low = None
                    for dx in (0, -6, 6, -10, 10, -4, 4, -8, 8, -12, 12, -2, 2):
                        low = boxes.get((x + dx, y + 32))
                        if low and low not in tops and 'bartender_part' in by_code.get(low, []):
                            break
                        low = None
                    if low:
                        under[low] += 1
                        under_top[code] = under_top.get(code, 0) + 1
                    else:
                        alone[code] += 1
    print('legs under a top half:', [(c, n, by_code.get(c)) for c, n in under.most_common()])
    print('LEGS ' + ' '.join(sorted(under)))
    print('top halves seen with nothing under them:', alone.most_common(40))
    print('SINGLES ' + ' '.join(sorted(c for c in alone if alone[c] > 3 * under_top.get(c, 0))))
    print('counters with pad in the name:', sorted(set(re.findall(r'"(counter-[a-z0-9_-]*pad[a-z0-9_-]*)"', names)))[:20])


if __name__ == '__main__':
    main()
