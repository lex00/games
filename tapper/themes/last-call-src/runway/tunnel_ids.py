"""Which tiles tell the four tunnels apart, so each can blink on its own.

    python3 runway/tunnel_ids.py

A room rule places a piece by a class of cells, and a class is chosen by tile, so two
tunnels can only be told apart by a tile one has and the others lack. For each tunnel
(by the row of its opening's top cell, 4, 10, 16 or 22) this lists the tiles that, in
every one of the four rooms, stand in that tunnel's cells and nowhere else on the
screen, with where the tile stands from the opening's top cell in the tunnel's own
unmirrored frame (columns toward the bar positive).
"""
import os
from collections import Counter

W = os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/')


def main():
    per = {}
    for s in ('screen_8021', 'screen_87a1', 'screen_8f21', 'screen_96a1'):
        g = []
        for line in open(W + f'flips-{s}.txt'):
            p = line.split()
            if len(p) == 33 and p[0].isdigit():
                g.append([(c[:3], c.endswith('<')) for c in p[1:]])
        count = Counter(t for row in g for t, _ in row)
        for r in range(30):
            for c in range(32):
                if g[r][c][0] != '0bd':
                    continue
                fl = g[r][c][1]
                here = {}
                for rr in range(r, min(30, r + 9)):
                    for k in range(-16, 3):
                        cc = c - k if fl else c + k
                        if 0 <= cc < 32:
                            t, tf = g[rr][cc]
                            if t not in ('000', '0bd') and count[t] == 1 and tf == fl:
                                here[t] = (k, rr - r)
                per.setdefault(r, []).append(here)
    # also: a tile that marks one tunnel in each room (not the same tunnel every time), always at the same place from
    # that tunnel's opening: enough to give "the tunnel that has it" a rule of its own
    by_tile = {}
    for r, rooms in per.items():
        for i, h in enumerate(rooms):
            for t, off in h.items():
                by_tile.setdefault(t, []).append((i, r, off))
    for t, v in sorted(by_tile.items()):
        offs = {o for _, _, o in v}
        if len(v) == 4 and len(offs) == 1:
            print('tile', t, 'marks one tunnel in every room, at', offs.pop(), 'rows', [r for _, r, _ in v])
    for r, rooms in sorted(per.items()):
        common = set(rooms[0])
        for h in rooms[1:]:
            common &= set(h)
        same = {t: rooms[0][t] for t in common if all(h[t] == rooms[0][t] for h in rooms)}
        print('tunnel at row', r, 'in', len(rooms), 'rooms; its own tiles:', sorted(same.items()))


if __name__ == '__main__':
    main()
