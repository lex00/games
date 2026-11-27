"""The picture codes a clip's sprite list draws over a stretch of frames, a line every STEP frames.

    python3 runway/codes_over.py SPRITES_JSONL FIRST LAST [STEP]

Each line: the frame and every code drawn on it with its x and y, in the list's order.
"""
import json
import sys


def main():
    path, first, last = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    step = int(sys.argv[4]) if len(sys.argv) > 4 else 5
    for line in open(path):
        j = json.loads(line)
        f = j['frame']
        if first <= f <= last and (f - first) % step == 0:
            print(f, ' '.join(f'{c}@{b[0]},{b[1]}' for c, b in j['model'] if c != '009'))


if __name__ == '__main__':
    main()
