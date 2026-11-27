"""How a drinker's liquid goes down over time, from a clip's sprite list.

    python3 runway/drink_timing.py SPRITES_JSONL

For each stretch of frames a drinking mug's second half (code 024) stays at one place, the number of
liquid bands (code 015) beside it on each frame since the stretch began, as runs: COUNT x FRAMES.
The same runs on every stretch mean the liquid's level follows the drinking picture's age.
"""
import json
import sys


def main():
    live = {}
    done = []
    for line in open(sys.argv[1]):
        j = json.loads(line)
        f = j['frame']
        halves = [(b[0], b[1]) for c, b in j['model'] if c == '024']
        bands = [(b[0], b[1]) for c, b in j['model'] if c == '015']
        seen = set()
        for x, y in halves:
            n = sum(1 for bx, by in bands if (abs(bx - x + 20) <= 1 or abs(bx - x - 10) <= 1) and -14 <= by - y <= 4)
            seen.add((x, y))
            live.setdefault((x, y), [f, []])[1].append(n)
        for k in [k for k in live if k not in seen]:
            done.append((k, *live.pop(k)))
    done += [(k, *v) for k, v in live.items()]
    for (x, y), first, counts in sorted(done, key=lambda d: d[1]):
        runs = []
        for n in counts:
            if runs and runs[-1][0] == n:
                runs[-1][1] += 1
            else:
                runs.append([n, 1])
        print(f'frame {first} at {x},{y} for {len(counts)}:', ' '.join(f'{n}x{k}' for n, k in runs))


if __name__ == '__main__':
    main()
