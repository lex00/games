"""The guests' drinking pictures with our glass mug in place of the board's.

    python3 runway/drinkmug.py THEME_DIR SHEET_PNG

How the board draws a guest drinking: his picture holds the mug's left part in its corner
(its own pixels x 22 to 31, y 3 to 11: BOX), a second picture beside him (code 24) is the
mug's right part, and the drink is a stack of up to six thin bands (bonus_object 15, each
2 pixels above the last, at his x + 33 to 41 and y + 6 to 21) that go one by one as he
drinks. So the drink going down is the game's own, in step on every level.

Ours: the board's mug is left out of his picture (`#+ recolor`, every colour number as
nothing in BOX, and the handle's two blues under his hand), picture 24 is drawn as nothing,
and an empty glass (mug-23 with its drink taken out, at the board's mug's size) is drawn
over him (`also`) where the board's mug was: its handle at his hand, its body round the
bands. The bands are drawn as our amber (beer-15.png, and beer-15-left.png in the ballpark
where the board draws them on his other side), after him, so they fill the glass and
drain as the game takes them away. The glass has a hole wherever his own picture has a
pixel from the hand's rows down, so his hand holds it in front.

The check: a guest passes when at most 2 pixels of his own (a colour that is not the mug's
black, blues, red or white) lie in BOX. One that fails is left as the board draws him and
is named. SHEET_PNG shows every guest before and after (with four bands of drink), made
here from the same rules, to look at before the game is.
"""
import os
import re
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
from drinkers import DRINK, SHEET, cell, codes  # noqa: E402

BOX = (22, 3, 31, 11)
HAND_ROW = 12                      # from this row down his own pixels stay in front
BACK = (0x60, 0x60, 0x68)
MUG = {(0, 0, 0), (0, 0x92, 0xff), (0x24, 0x92, 0xff), (255, 0, 0), (255, 255, 255), (0, 0x49, 0x92)}
DEMO = ['1f']                      # the guests the attract demo shows drinking, as bonus_objects
ARM = {'ed': (22, 1, 31, 8)}       # guests whose own picture crosses the mug's corner: the corner's box
PLAYER_BOX = (22, 0, 31, 8)      # the mug's corner in the bartender's drinking picture
PLAYER_HAND_ROW = 9
ART_W = 48                         # game pixels of the art's width: 16 past the picture's right edge
# The glass's box in the guest's own pixels. The board's mug lies on its side, its mouth at
# his face and its base to the right (the owner, 2026-11-24: 'the original mug was sideways
# while they drink', of a first try that stood it upright): ours is mug-23 turned a quarter
# left, mouth at the box's left, handle under it at his hand.
GLASS = (22, 2, 45, 24)
# The drink's bands inside it: each a strip 2 game pixels high, as far apart as the board
# stacks them, so six fill the glass's inside and the top one goes first. In a band's own
# cell (which the board puts 12 right of the guest, or 22 left of him where it mirrors him).
BAND_ROWS = (30, 34)               # art pixels: a game pixel inside the glass's top wall
BAND_X = (26, 60)                  # art pixels, the guest unmirrored: between the mouth's rim and the base
BAND_X_LEFT = (22, 56)             # art pixels, the guest mirrored


def emptied(mug):
    """mug-23 with no drink and no head: the amber and the foam turned to clear glass."""
    a = np.asarray(mug).astype(float).copy()
    r, g, b, al = a[..., 0], a[..., 1], a[..., 2], a[..., 3]
    ys = np.arange(a.shape[0])[:, None] * np.ones((1, a.shape[1]))
    amber = (r > g + 25) & (g > b + 25) & (al > 0)
    foam = (r > 200) & (g > 190) & (b > 150) & (al > 0) & (ys < 22)
    # The page draws the drink's bands under the glass, so the glass's inside is nearly clear
    # (at 104 of 255 the amber washed out); what makes it read as a mug is its rim, drawn
    # dark in glass_art (with a clear inside and no rim the owner saw the drink with no mug).
    glass = np.array([196, 222, 246, 46.0])
    for c in range(3):
        a[..., c] = np.where(amber | foam, glass[c] + (a[..., c] - glass[c]) * 0.1, a[..., c])
    a[..., 3] = np.where(amber | foam, glass[3], a[..., 3])
    a[:9, :, 3] = np.where(foam[:9], 0, a[:9, :, 3])
    out = Image.fromarray(np.clip(a, 0, 255).astype(np.uint8), 'RGBA')
    return out.crop(out.getbbox())


def fitted(img, w, h):
    pre = np.asarray(img).astype(np.float32)
    al = pre[..., 3] / 255
    f = lambda ch: np.asarray(Image.fromarray(np.ascontiguousarray(ch), 'F').resize((w, h), Image.BOX))
    a = f(al)
    o = np.zeros((h, w, 4), np.uint8)
    for c in range(3):
        o[..., c] = np.clip(np.where(a > 0, f(pre[..., c] * al) / np.maximum(a, 1e-6), 0), 0, 255)
    o[..., 3] = np.where(a >= 0.4, np.clip(f(pre[..., 3]), 0, 255), 0)
    return Image.fromarray(o, 'RGBA')


def glass_art(mug23):
    g = fitted(emptied(mug23).transpose(Image.ROTATE_90), (GLASS[2] - GLASS[0]) * 2, (GLASS[3] - GLASS[1]) * 2)
    # A dark rim one pixel wide round the glass's outline, so it shows on any room's backdrop.
    a = np.asarray(g).copy()
    solid = a[..., 3] > 0
    pad = np.pad(solid, 1)
    inner = pad[:-2, 1:-1] & pad[2:, 1:-1] & pad[1:-1, :-2] & pad[1:-1, 2:]
    rim = solid & ~inner
    a[rim] = (58, 74, 100, 255)
    g = Image.fromarray(a, 'RGBA')
    art = Image.new('RGBA', (ART_W * 2, 64), (0, 0, 0, 0))
    art.alpha_composite(g, (GLASS[0] * 2, GLASS[1] * 2))
    return art


def main():
    theme, sheet_out = sys.argv[1], sys.argv[2]
    sheet = Image.open(SHEET).convert('RGB')
    order = codes()
    mug23 = Image.open(f'{theme}/art/obj/mug-23.png').convert('RGBA')
    # The bands' pictures: an amber strip, darker at its ends and toward the mug's base, with a
    # lighter line along its top (the drink's surface on the top band, a faint layer below it)
    # and a bubble or two, the same in every band.
    for name, (x0, x1), base_right in (('band', BAND_X, True), ('band-left', BAND_X_LEFT, False)):
        strip = Image.new('RGBA', (64, 64), (0, 0, 0, 0))
        sp = strip.load()
        for y in range(BAND_ROWS[0], BAND_ROWS[1]):
            for x in range(x0, x1):
                edge = min(x - x0, x1 - 1 - x)
                along = (x - x0) / (x1 - 1 - x0)
                deep = along if base_right else 1 - along
                k = (0.78 if edge < 2 else 1.0) * (1.04 - 0.16 * deep)
                if y == BAND_ROWS[0]:
                    k *= 1.1
                c = (min(255, int(238 * k)), min(255, int(156 * k)), min(255, int(30 * k)), 255)
                if y == BAND_ROWS[0] + 2 and (x - x0) % 11 == 5:
                    c = (255, 214, 120, 255)
                sp[x, y] = c
        strip.save(f'{theme}/art/drink/{name}.png')
    band = Image.open(f'{theme}/art/drink/band.png').convert('RGBA')
    os.makedirs(f'{theme}/art/drink', exist_ok=True)
    for f in os.listdir(f'{theme}/art/drink'):
        if f.startswith('mug-'):
            os.unlink(f'{theme}/art/drink/{f}')
    base = glass_art(mug23)
    passed, failed = [], []
    for c in DRINK:
        if c not in order:
            failed.append((c, -1))
            continue
        im = cell(sheet, order, c).load()
        own = sum(1 for y in range(BOX[1], BOX[3] + 1) for x in range(BOX[0], BOX[2] + 1) if im[x, y] != BACK and im[x, y] not in MUG)
        (failed if own > 2 else passed).append((c, own))
    print('pass:', ' '.join(c for c, _ in passed))
    print("fail (his own pixels in the mug's corner, -1 not on the sheet):", ' '.join(f'{c}:{n}' for c, n in failed) or 'none')
    # each guest's glass: the hole where his own pixels are, from the hand's rows down
    arts = {}
    for c, _ in passed:
        im = cell(sheet, order, c).load()
        art = base.copy()
        px = art.load()
        for y in range(HAND_ROW, 32):
            for x in range(32):
                if im[x, y] != BACK:
                    for dy in range(2):
                        for dx in range(2):
                            px[x * 2 + dx, y * 2 + dy] = (0, 0, 0, 0)
        art.save(f'{theme}/art/drink/glass-{c}.png')
        arts[c] = art
    # the rules
    p = f'{theme}/art.txt'
    a = open(p).read()
    a = re.sub(r'^# (?:A drinking guest|The drink.s bands).*\n', '', a, flags=re.M)
    a = re.sub(r'^(?:#\+ recolor|also) sprite-(?:customer|bonus_object|scene_customer)-(?:' + '|'.join(DRINK) + r') .*\n', '', a, flags=re.M)
    a = re.sub(r'^pic sprite-(?:customer|bonus_object|scene_customer)-24 .*\n', '', a, flags=re.M)
    a = re.sub(r'^(?:#\+ on screen_69a1 )?pic sprite-bonus_object-15 .*\n', '', a, flags=re.M)
    lines = "# A drinking guest holds our empty glass where the board's mug was (runway/drinkmug.py).\n"
    nums = ' '.join(f'{n}=-' for n in range(1, 16))
    # The attract demo's guest is a scene_customer in the model as it is now, and was a
    # bonus_object in older builds of the page: both names take the same glass.
    names = [(f'sprite-customer-{c}', c) for c, _ in passed]
    names += [(f'sprite-{kind}-{c}', c) for kind in ('scene_customer', 'bonus_object') for c in DEMO if c in arts]
    for n, c in names:
        lines += f'#+ recolor {n} box={BOX[0]},{BOX[1]},{BOX[2]},{BOX[3]} {nums}\n'
        lines += f'#+ recolor {n} box=23,13,27,16 2=- 10=-\n'
        # In play a guest and the mug's right part (24) are one body, and the theme dresses
        # the last of a body's pictures it knows, which is 24: the glass is hung on 24 below.
        # The attract demo's guest is a body of his own and takes the glass himself.
        if not n.startswith('sprite-customer-'):
            lines += f'also {n} art/drink/glass-{c}.png {ART_W} 32 0 0\n'
    # A guest whose own picture crosses the mug's corner (ARM: the fourth alien, whose arm lies
    # over it) can't lose the whole corner. The board's mug is left out of his picture a row
    # at a time, each run of pixels that are not his: his are every colour that is not the
    # mug's, and the black pixels beside one (his outline). The glass, hung on picture 24 for
    # every guest alike, is then drawn over the arm where they cross, as a mug held in front.
    a = re.sub(r'^#\+ recolor sprite-customer-(?:' + '|'.join(ARM) + r') .*\n', '', a, flags=re.M)
    for c, (x0, y0, x1, y1) in ARM.items():
        if c not in order:
            continue
        im = cell(sheet, order, c).load()
        his = lambda x, y: 0 <= x < 32 and 0 <= y < 32 and im[x, y] != BACK and im[x, y] not in MUG
        kept = lambda x, y: his(x, y) or (im[x, y] == (0, 0, 0) and any(his(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))))
        runs = 0
        for y in range(y0, y1 + 1):
            x = x0
            while x <= x1:
                if kept(x, y) or im[x, y] == BACK:
                    x += 1
                    continue
                e = x
                while e + 1 <= x1 and not kept(e + 1, y) and im[e + 1, y] != BACK:
                    e += 1
                lines += f'#+ recolor sprite-customer-{c} box={x},{y},{e},{y} {nums}\n'
                runs += 1
                x = e + 1
        print(f'{c}: the mug left out in {runs} runs, his arm kept')
    # The glass of a guest in play, on picture 24, which the board draws 32 pixels to his
    # right: the same art 32 to its left. One glass for every guest, clear from the hand's
    # rows down where it would lie over his own picture, so his hand stays in front.
    shared = base.copy()
    sp = shared.load()
    for y in range(HAND_ROW * 2, 64):
        for x in range(64):
            sp[x, y] = (0, 0, 0, 0)
    shared.save(f'{theme}/art/drink/glass-24.png')
    a = re.sub(r'^also sprite-customer-24 .*\n', '', a, flags=re.M)
    lines += f'also sprite-customer-24 art/drink/glass-24.png {ART_W} 32 -32 0\n'
    # The bartender's own drink when a level is cleared (player 41): the same mug two pixels
    # higher, its right part his picture 24, the same bands.
    names_p = open(os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/names.toml')).read()
    order_p = sorted(set(re.findall(r'sprite-player-([0-9a-f]{2})', names_p)))
    psheet = Image.open(os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/sprites-player.png')).convert('RGB')
    k = order_p.index('41')
    him = psheet.crop(((k % 8) * 80 + 8, (k // 8) * 92 + 4, (k % 8) * 80 + 72, (k // 8) * 92 + 68)).resize((32, 32), Image.NEAREST).load()
    art = Image.new('RGBA', (ART_W * 2, 64), (0, 0, 0, 0))
    art.alpha_composite(base, (0, -4))
    px = art.load()
    for y in range(PLAYER_HAND_ROW, 32):
        for x in range(32):
            if him[x, y] != BACK:
                for dy in range(2):
                    for dx in range(2):
                        px[x * 2 + dx, y * 2 + dy] = (0, 0, 0, 0)
    art.save(f'{theme}/art/drink/glass-player-41.png')
    a = re.sub(r'^(?:#\+ recolor|also) sprite-player-41 .*\n', '', a, flags=re.M)
    a = re.sub(r'^(?:#\+ recolor|pic) sprite-bartender_part-24 .*\n', '', a, flags=re.M)
    b = PLAYER_BOX
    lines += f'#+ recolor sprite-player-41 box={b[0]},{b[1]},{b[2]},{b[3]} {nums}\n'
    lines += f'also sprite-player-41 art/drink/glass-player-41.png {ART_W} 32 0 0\n'
    # The mug's right part (24) is left out with `#+ recolor`, every colour number as nothing,
    # and never with a `pic` line: in play a guest and the pictures drawn with him are one
    # body, a theme dresses one picture of a body, and a `pic` for 24 made that the dressed
    # one, so the glass's `also` on the guest's own picture never ran (the owner, 2026-11-24:
    # 'the mug still VANISHES while he drinks'; the attract demo, whose pictures are bodies of
    # their own, showed it and hid the fault).
    a = re.sub(r'^(?:#\+ recolor|pic) sprite-(?:customer|bonus_object|scene_customer|bartender_part)-24 .*\n', '', a, flags=re.M)
    for kind in ('customer', 'bonus_object', 'scene_customer', 'bartender_part'):
        lines += f'#+ recolor sprite-{kind}-24 {nums}\n'
    # The attract demo slides its empty mug back as a bonus_object: our empty mug there too.
    a = re.sub(r'^pic sprite-bonus_object-(?:10|14|17) art/obj/mug-.*\n', '', a, flags=re.M)
    for code in ('10', '14', '17'):
        lines += f'pic sprite-bonus_object-{code} art/obj/mug-{code}.png 32 32 0 0\n'
    lines += "# The drink's bands, which the board stacks in the mug and takes away as he drinks: our amber.\n"
    lines += 'pic sprite-bonus_object-15 art/drink/band.png 32 32 0 0\n'
    lines += '#+ on screen_69a1 pic sprite-bonus_object-15 art/drink/band-left.png 32 32 0 0\n'
    k = a.index('pic letter-A ')
    a = a[:k] + lines + a[k:]
    open(p, 'w').write(a)
    # the sheet: each guest before and after
    s = 5
    out = Image.new('RGB', (4 * (ART_W * s + 6), ((len(DRINK) + 3) // 4) * 2 * (32 * s + 6)), (70, 110, 60))
    for r, c in enumerate(DRINK):
        if c not in order:
            continue
        im = cell(sheet, order, c).convert('RGBA')
        px = im.load()
        for y in range(32):
            for x in range(32):
                if px[x, y][:3] == BACK:
                    px[x, y] = (0, 0, 0, 0)
        x0, y0 = (r % 4) * (ART_W * s + 6), (r // 4) * 2 * (32 * s + 6)
        before = Image.new('RGBA', (ART_W, 32), (70, 110, 60, 255))
        before.alpha_composite(im)
        out.paste(before.resize((ART_W * s, 32 * s), Image.NEAREST).convert('RGB'), (x0, y0))
        if c not in arts:
            continue
        cut = im.copy()
        cp = cut.load()
        for y in range(BOX[1], BOX[3] + 1):
            for x in range(BOX[0], BOX[2] + 1):
                cp[x, y] = (0, 0, 0, 0)
        for y in range(13, 17):
            for x in range(23, 28):
                if cp[x, y][:3] in ((0, 0x92, 0xff), (0x24, 0x92, 0xff)):
                    cp[x, y] = (0, 0, 0, 0)
        big = Image.new('RGBA', (ART_W * 2, 64), (70, 110, 60, 255))
        g = Image.new('RGBA', (ART_W, 32), (0, 0, 0, 0))
        g.alpha_composite(cut)
        big.alpha_composite(g.resize((ART_W * 2, 64), Image.NEAREST))
        big.alpha_composite(arts[c])
        for k in range(2, 6):                       # four bands left, as the board stacks them
            big.alpha_composite(band, (12 * 2, (-10 + 2 * k) * 2))
        out.paste(big.resize((ART_W * s, 32 * s), Image.NEAREST).convert('RGB'), (x0, y0 + 32 * s + 6))
    out.save(sheet_out)


if __name__ == '__main__':
    main()
