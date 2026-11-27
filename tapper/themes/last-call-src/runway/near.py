"""What is drawn near one of the bartender's pictures: the sprites within 48 pixels of a given code, over a clip.

    python3 runway/near.py SPRITES_JSONL CODE...

For each code, the first frame it shows and, over the clip, each other sprite code seen
within 48 pixels of it with its commonest offset (dx, dy) from it.
"""
import json
import sys
from collections import Counter, defaultdict


def main():
    rows = [json.loads(line) for line in open(sys.argv[1])]
    for code in sys.argv[2:]:
        first = None
        near = defaultdict(Counter)
        span = []
        for row in rows:
            me = [b for n, b in row['model'] if n[-2:] == code]
            if not me:
                continue
            if first is None:
                first = row['frame']
            span.append((row['frame'], me[0][0], me[0][1]))
            for n, b in row['model']:
                if n[-2:] != code and abs(b[0] - me[0][0]) <= 48 and abs(b[1] - me[0][1]) <= 48:
                    near[n[-2:]][(b[0] - me[0][0], b[1] - me[0][1])] += 1
        print(code, 'first frame', first, 'frames', len(span), 'e.g.', span[:3])
        for other, offs in sorted(near.items(), key=lambda kv: -sum(kv[1].values()))[:6]:
            print('    with', other, offs.most_common(2))


if __name__ == '__main__':
    main()
