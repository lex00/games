"""Which sprites make up the bartender and his pad on a space level, from a clip's sprite list.

    python3 runway/who.py SPRITES_JSONL [FRAME]

Prints the model's sprites on one frame (name, box) and, over the whole clip, each
sprite name with how often it shows and the sizes and rows it shows at.
"""
import json
import sys
from collections import Counter, defaultdict


def main():
    rows = [json.loads(line) for line in open(sys.argv[1])]
    frame = int(sys.argv[2]) if len(sys.argv) > 2 else 200
    r = min(rows, key=lambda x: abs(x['frame'] - frame))
    print('frame', r['frame'])
    for name, box in r['model']:
        print('  ', name, box)
    seen = Counter()
    sizes = defaultdict(Counter)
    for x in rows:
        for name, box in x['model']:
            seen[name] += 1
            sizes[name][(box[2], box[3])] += 1
    for name, n in seen.most_common(40):
        print(name, n, dict(sizes[name]))


if __name__ == '__main__':
    main()
