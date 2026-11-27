"""Where a clip's sprite list draws any of some picture codes: each code's stretches of frames and where.

    python3 runway/find_codes.py SPRITES_JSONL CODE...

Codes as the list has them (three hex digits). Prints, for each code found, its first stretches
(first frame, last frame, x and y on the first).
"""
import json
import sys


def main():
    path, want = sys.argv[1], set(sys.argv[2:])
    runs = {}
    for line in open(path):
        j = json.loads(line)
        f = j['frame']
        for c, b in j['model']:
            if c in want:
                r = runs.setdefault(c, [])
                if r and r[-1][1] == f - 1:
                    r[-1][1] = f
                elif not r or r[-1][1] != f:
                    r.append([f, f, b[0], b[1]])
    for c in sorted(runs):
        print(c, [tuple(r) for r in runs[c][:4]])
    if not runs:
        print('none of', sorted(want))


if __name__ == '__main__':
    main()
