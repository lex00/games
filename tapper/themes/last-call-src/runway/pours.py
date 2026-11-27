"""Where in a clip the bartender pours: the stretches of frames the pour's stream (sprite 0a) is drawn.

    python3 runway/pours.py SPRITES_JSONL...
"""
import json
import sys


def main():
    for path in sys.argv[1:]:
        frames = [json.loads(line)['frame'] for line in open(path) if any(n[-2:] == '0a' for n, b in json.loads(line)['model'])]
        runs, start, prev = [], None, None
        for f in frames:
            if start is None:
                start = f
            elif f != prev + 1:
                runs.append((start, prev))
                start = f
            prev = f
        if start is not None:
            runs.append((start, prev))
        print(path.split('/')[-1], 'pours:', runs[:6])


if __name__ == '__main__':
    main()
