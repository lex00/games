"""The theme's letters, as large as the board's own and on a dark plate.

    python3 runway/font16.py THEME_DIR

Cut from samples/lettering.png as objects.py cuts them (A-Z, 0-9, then
. , ! ? - : ' the copyright sign and the lives' glass), but:
- larger: a capital stands 30 of the cell's 32 and is drawn a third wider,
  31 at most, so a word fills its cells as the board's letters do;
- on a plate: the whole cell behind the letter is one dark colour, so a
  line of text reads over a floor or a counter. A space has no plate, as
  the board leaves its spaces clear.
Writes THEME_DIR/art/font/NAME.png, 32 by 32 each.
"""
import os
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from build_lib import crop, load, objects, shrink  # noqa: E402

PLATE = (22, 30, 58, 255)     # a deep navy, the sign's board in shadow
CAP = 30                      # a capital's height in the cell
WIDER = 1.34


def main():
    theme = sys.argv[1]
    la, lm = load('lettering')
    gl = objects(la, lm, minarea=150)
    rows = {}
    for sl in gl:
        rows.setdefault(round((sl[0].start + sl[0].stop) / 2 / 320), []).append(sl)
    order = [sorted(rows[k], key=lambda s: s[1].start) for k in sorted(rows)]
    flat = [s for r in order for s in r]
    bottom = {id(s): max(x[0].stop for x in r) for r in order for s in r}
    names = [f'letter-{c}' for c in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'] + [
        'letter-x02e', 'letter-x02c', 'letter-x021', 'letter-x03f', 'letter-x02d', 'letter-x03a', 'letter-x027',
        'copyright', 'glass']
    assert len(flat) == len(names), (len(flat), len(names))
    cap = max(s[0].stop - s[0].start for s in flat[:26])
    k = CAP / cap
    for name, sl in zip(names, flat):
        ca, cm = crop(la, lm, sl)
        h, w = cm.shape
        if name == 'glass':            # the lives' glass keeps its own shape, on the same plate
            kk = min(28 / cap, 31 / h, 31 / w)
            gh, gw = max(1, round(h * kk)), max(1, round(w * kk))
        else:
            gh = max(1, min(31, round(h * k)))
            gw = max(1, min(31, round(w * k * WIDER)))
        img = shrink(ca, cm, gw, gh)
        out = Image.new('RGBA', (32, 32), PLATE)
        # on the row's line: 31 is the bottom of a letter
        y = 31 - round((bottom[id(sl)] - sl[0].stop) * k) - img.height
        out.alpha_composite(img, ((32 - img.width) // 2, max(0, y)))
        out.save(f'{theme}/art/font/{name}.png')
    print(len(names), 'pictures in', f'{theme}/art/font')


if __name__ == '__main__':
    main()
