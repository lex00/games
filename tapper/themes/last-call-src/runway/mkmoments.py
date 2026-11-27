"""A moments file for theme shots: the power-on play (the run of last-call-floor/po-moments.toml) at chosen frames.

    python3 runway/mkmoments.py OUT_TOML NAME FRAME...
    python3 runway/mkmoments.py OUT_TOML NAME FIRST-LAST/STEP

Each shot is named NAME-FRAME.
"""
import os
import sys

PO = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/po-moments.toml')


def main():
    out, name = sys.argv[1], sys.argv[2]
    frames = []
    for a in sys.argv[3:]:
        if '/' in a:
            span, step = a.split('/')
            lo, hi = span.split('-')
            frames += list(range(int(lo), int(hi) + 1, int(step)))
        else:
            frames.append(int(a))
    text = open(PO).read()
    run = text[:text.index('[[shot]]')]
    shots = ''.join(f'[[shot]]\nname = "{name}-{f}"\nrun = "po"\nframe = {f}\nshows = "{name}"\n\n' for f in frames)
    open(out, 'w').write(run + shots)
    print(len(frames), 'shots')


if __name__ == '__main__':
    main()
