"""The pictures drawn near one picture code in a clip's sprite list, with their places from it.

    python3 runway/near_code.py SPRITES_JSONL CODE [FIRST_FRAME] [COUNT]

Prints, for the first COUNT (3) frames from FIRST_FRAME on that draw CODE (three hex digits,
as the list has them), each picture within 50 pixels of it: its code and its x and y from
CODE's own.
"""
import json
import sys


def main():
    path, code = sys.argv[1], sys.argv[2]
    first = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    count = int(sys.argv[4]) if len(sys.argv) > 4 else 3
    n = 0
    for line in open(path):
        j = json.loads(line)
        if j['frame'] < first:
            continue
        m = j['model']
        g = [b for c, b in m if c == code]
        if not g:
            continue
        g = g[0]
        print(j['frame'], 'at', g[:2], [(c, b[0] - g[0], b[1] - g[1]) for c, b in m if abs(b[0] - g[0]) < 50 and abs(b[1] - g[1]) < 50 and c != code])
        n += 1
        if n >= count:
            break


if __name__ == '__main__':
    main()
