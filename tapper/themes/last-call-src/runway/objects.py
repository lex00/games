# Cuts the lettering and objects sheets into the theme's pictures and writes
# their pic lines into theme/art.txt between the "# --- objects" markers.
# Each object is fitted into the original sprite's own pixel box (measured
# from the player's lift, never copied), bottom-aligned, at the theme's
# scale 2: a 32 by 32 sprite is a 64 by 64 picture.
import sys, numpy as np, tomllib
from PIL import Image
sys.path.insert(0, '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway')
R = '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/samples'
T = '/Users/alex/.gameprinter/work/themes/tapper-pixel/repo/theme'
import importlib.util
spec = importlib.util.spec_from_file_location('b', '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/build_lib.py')
from build_lib import load, objects, shrink, crop

def fit(a, m, sl, box, flip=False):
    # box: the original sprite's pixel box x0, x1, y0, y1 in its 32 by 32.
    ca, cm = crop(a, m, sl)
    h, w = cm.shape
    bw, bh = (box[1] - box[0] + 1) * 2, (box[3] - box[2] + 1) * 2
    k = min(bw / w, bh / h)
    img = shrink(ca, cm, max(1, round(w * k)), max(1, round(h * k)))
    if flip: img = img.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    out = Image.new('RGBA', (64, 64))
    x = box[0] * 2 + (bw - img.width) // 2
    y = (box[3] + 1) * 2 - img.height
    out.alpha_composite(img, (x, y))
    return out

sprites = np.asarray(Image.open('/Users/alex/.gameprinter/work/assets/tapper@tapper/sprites.png')); per = sprites.shape[1] // 32
def box(code):
    r, c = divmod(code, per); s = sprites[r*32:(r+1)*32, c*32:(c+1)*32]
    ys, xs = np.nonzero(s); return (xs.min(), xs.max(), ys.min(), ys.max())

lines = []
# Objects: a 4 by 4 sheet, row by row.
oa, om = load('objects')
cells = {}
for sl in objects(oa, om, minarea=400):
    cy, cx = (sl[0].start + sl[0].stop) // 2 // 480, (sl[1].start + sl[1].stop) // 2 // 480
    if (cy, cx) in cells:
        o = cells[(cy, cx)]
        sl = (slice(min(o[0].start, sl[0].start), max(o[0].stop, sl[0].stop)), slice(min(o[1].start, sl[1].start), max(o[1].stop, sl[1].stop)))
    cells[(cy, cx)] = sl
obs = [cells[k] for k in sorted(cells)]
assert len(obs) == 16, len(obs)
# The board's glasses hold their handle on the left: the sheet's on the right.
want = {
    'tap-09': (0, 0x09, False), 'mug-0e': (2, 0x0e, True), 'mug-16': (3, 0x16, True),
    'mug-14': (4, 0x14, True), 'mug-17': (5, 0x17, True), 'mug-10': (6, 0x10, True),
    'mug-11': (7, 0x11, False), 'mug-12': (8, 0x12, False), 'mug-13': (9, 0x13, False),
    'tip-4d': (10, 0x4d, False),
}
def clear_handle(img):
    # The window inside a mug's handle shows what is behind it. In each of
    # the handle's rows (the mug's middle) the handle's ring is a lit run
    # at its outer edge, then its darker band; the next lit run, before the
    # glass's own wall, is the window: made transparent.
    a = np.asarray(img).copy()
    on = a[..., 3] > 0
    lit = on & (a[..., :3].min(axis=2) > 200)
    ys, xs = np.nonzero(on)
    top, bot = ys.min(), ys.max()
    for y in range(top + int(0.3 * (bot - top)), top + int(0.85 * (bot - top))):
        row = np.nonzero(on[y])[0]
        if not len(row):
            continue
        runs, x = [], row[0]
        while x <= row[-1]:
            if lit[y, x]:
                x0 = x
                while x <= row[-1] and lit[y, x]:
                    x += 1
                runs.append((x0, x))
            else:
                x += 1
        if len(runs) >= 2 and runs[0][0] <= row[0] + 3:
            x0, x1 = runs[1]
            if 3 <= x1 - x0 <= 8 and x0 - runs[0][1] <= 5:
                a[y, x0:x1, 3] = 0
    return Image.fromarray(a, 'RGBA')
for name, (k, code, flip) in want.items():
    im = fit(oa, om, obs[k], box(code), flip)
    if name in ('mug-0e', 'mug-16', 'mug-17'):
        im = clear_handle(im)
    im.save(f'{T}/art/obj/{name}.png')
    lines.append(f'pic sprite-{name} art/obj/{name}.png 32 32 0 0')
# The glass in flight (10: the board's glass tilted 45 degrees clockwise,
# its handle up and left, rim up and right; the board turns it with its
# flips as it tumbles, and our picture with it): our empty mug turned the
# same way, in that sprite's own box.
_e = Image.open(f'{T}/art/obj/mug-17.png').convert('RGBA')
_e = _e.crop(_e.getbbox()).rotate(-45, expand=True, resample=Image.BICUBIC)
_e = _e.crop(_e.getbbox())
_b = box(0x10); _bw, _bh = (_b[1] - _b[0] + 1) * 2, (_b[3] - _b[2] + 1) * 2
_k = min(_bw / _e.width, _bh / _e.height)
_e = _e.resize((round(_e.width * _k), round(_e.height * _k)), Image.LANCZOS)
_o = Image.new('RGBA', (64, 64))
_o.alpha_composite(_e, (_b[0] * 2 + (_bw - _e.width) // 2, _b[2] * 2 + (_bh - _e.height) // 2))
_o.save(f'{T}/art/obj/mug-10.png')
# The tap pulled, over the pose that holds the lever (the bartender's arm
# at the tap, part 02), at the tap's own place: 13 below the arm's sprite.
fit(oa, om, obs[1], box(0x09)).save(f'{T}/art/obj/tap-pulled.png')

# The idle screen's barrels: the brass tap at rest, as a room piece.
fit(oa, om, obs[0], box(0x09)).save(f'{T}/pieces/saloon/idle_tap.png')

# Letters the sheet has that the board never draws.
NOT_ON_BOARD = ('letter-4', 'letter-6', 'letter-8', 'letter-9', 'letter-Q', 'letter-Z', 'letter-x027', 'letter-x02c', 'letter-x02d', 'letter-x03f')
# The letters the game's texts write (the board names no other).
import glob
used = set()
for f in glob.glob('/Users/alex/.gameprinter/work/assets/tapper@tapper/*text*.toml'):
    for row in tomllib.load(open(f, 'rb')).get('rows', []):
        for b in row:
            c = chr(b)
            if c.isascii() and (c.isupper() or c.isdigit()):
                used.add(f'letter-{c}')
            elif b:
                used.add(f'letter-x{b:03x}')
used -= {'letter-x02e', 'letter-x021'}  # in the tables, never on screen
# Lettering: A-J, K-T, U-Z, 0-9, then . , ! ? - : ' (c) and the glass.
la, lm = load('lettering')
gl = objects(la, lm, minarea=150)
rows = {}
for sl in gl:
    rows.setdefault(round((sl[0].start + sl[0].stop) / 2 / 320), []).append(sl)
order = [sorted(rows[k], key=lambda s: s[1].start) for k in sorted(rows)]
flat = [s for r in order for s in r]
bottom = {id(s): max(x[0].stop for x in r) for r in order for s in r}
names = [f'letter-{c}' for c in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'] + ['letter-x02e', 'letter-x02c', 'letter-x021', 'letter-x03f', 'letter-x02d', 'letter-x03a', 'letter-x027', 'copyright', 'glass']
print(len(flat), 'glyphs found,', len(names), 'wanted')
assert len(flat) == len(names), [(s[0].start, s[1].start) for s in flat]
# One scale for the font: the letters' height to 26 of the cell's 32.
cap = max(s[0].stop - s[0].start for s in flat[:26])
k = 26 / cap
for name, sl in zip(names, flat):
    ca, cm = crop(la, lm, sl)
    h, w = cm.shape
    kk = min(k, 30 / h, 30 / w)
    img = shrink(ca, cm, max(1, round(w * kk)), max(1, round(h * kk)))
    out = Image.new('RGBA', (32, 32))
    # On the row's line: 29 is the bottom of a letter.
    y = 29 - round((bottom[id(sl)] - sl[0].stop) * k) - img.height
    out.alpha_composite(img, ((32 - img.width) // 2, max(0, y)))
    out.save(f'{T}/art/font/{name}.png')
    # Every letter and digit the sheet has: the points screen's numbers
    # come from the legend's own table, not the text tables `used` reads.
    if name in used or (name.startswith('letter-') and name not in ('letter-x02e', 'letter-x021') + NOT_ON_BOARD):
        lines.append(f'pic {name} art/font/{name}.png 16 16 0 0')
counters = ['score', 'score_2', 'cleared', 'credits']
for c in counters:
    for d in range(10):
        lines.append(f'pic counter-{c}-{d} art/font/letter-{d}.png 16 16 0 0')
lines.append('pic counter-lives_shown art/font/glass.png 16 16 0 0')

# --- Mugs everywhere the game shows one, all as the theme's glass ---
# The glass under the tap as he pours (bartender parts 02 and 05: the board
# draws the empty mug 17 at the same place in them): the frosted empty mug.
O = f'{T}/art/obj'
empty = Image.open(f'{O}/mug-17.png').convert('RGBA')
full = Image.open(f'{O}/mug-0e.png').convert('RGBA')
# The board draws this glass over the beer that rises in it, its inside
# clear: the frosted inside made faint (its walls, rim and base kept), so
# the beer shows through it.
ga = np.asarray(empty).copy()
inner = np.zeros(ga.shape[:2], bool); inner[14:58, 16:40] = True
ga[inner, 3] = (ga[inner, 3] * 0.06).astype(np.uint8)
Image.fromarray(ga, 'RGBA').save(f'{O}/glass-pour.png')
# His hand on the glass and his sleeve: runway/pour.py writes
# glass-pour-hand.png from samples/pour-parts.png over glass-pour.png.
import subprocess as _sp
_sp.run(['python3', '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/pour.py'], check=True)
# The pour's frames (bartender parts 02 and 05: the glass under the tap
# with his hand; 03 and 06: his hand on the lever, the pipe and spout)
# stay the board's own pictures, so his hand and arm stay where the game
# draws them, pixel for pixel, with their tap and glass recoloured: the
# pipe (13) and spout (10) in our tap's brass, the lever (14) in its wood,
# and the glass's logo (4, 2) drawn as nothing, so the glass is clear.
for code in ('03', '06'):
    lines.append(f'#+ recolor sprite-bartender_part-{code} 13=c1821c 10=eab647 14=7b4506')
# The glass under the tap in 02 and 05 is the original's at x 7 to 22,
# rows 6 to 31, right of his hand (x 0 to 6): that box of it drawn as
# nothing, and our clear glass drawn there (its handle cut away: his hand
# is on that side); the tap's pipe and flange along their top rows (13,
# 10) in our tap's brass.
gp = Image.open(f'{O}/glass-pour.png').convert('RGBA')
ga2 = np.asarray(gp).copy(); ga2[:, :12, 3] = 0
Image.fromarray(ga2, 'RGBA').save(f'{O}/glass-pour-body.png')
for code in ('02', '05'):
    lines.append(f'#+ recolor sprite-bartender_part-{code} box=7,6,22,31 9=- 4=- 2=- 10=-')
    lines.append(f'#+ recolor sprite-bartender_part-{code} box=9,0,31,5 13=c1821c 10=eab647')
    lines.append(f'also sprite-bartender_part-{code} art/obj/glass-pour-body.png 32 32 0 0')
# The points screen's mug (the legend's 17): the same glass.
lines.append('pic sprite-legend-17 art/obj/mug-17.png 32 32 0 0')
# The beer as it rises in that glass: each layer the board stacks (at the
# tap 7F then 21s, 4 apart; at a level's end 0B then 0Cs, 2 apart) cut from
# the full mug's beer, inside the empty glass's walls (its body x 11 to 44,
# its base from row 58), over the rows the board's layer covers.
fa = np.asarray(full).astype(int)
patch = full.crop((36, 26, 57, 50))
def beer(y0, y1, round_bottom):
    out = Image.new('RGBA', (64, 64))
    # The full mug's beer, column by column (each column its median amber
    # across the mug's middle), so the layers stack with no seams; a few
    # bubbles from the full mug's own, by row.
    am = (fa[..., 0] > 170) & (fa[..., 2] < 90) & (fa[..., 3] > 0)
    cols = [fa[26:50, x][am[26:50, x]] for x in range(64)]
    med = [np.median(c[:, :3], axis=0) if len(c) else None for c in cols]
    have = [x for x in range(64) if med[x] is not None]
    x0b, x1b = min(have), max(have)
    t = np.zeros((64, 64, 4), np.uint8)
    for x in range(64):
        # The glass's beer columns (15 to 40) across the full mug's own.
        sx = x0b + round((min(max(x, 15), 40) - 15) * (x1b - x0b) / 25)
        c = med[sx] if med[sx] is not None else med[have[len(have) // 2]]
        t[:, x, :3] = c; t[:, x, 3] = 255
    rng = np.random.default_rng(7)
    for _ in range(18):
        bx, by = rng.integers(18, 38), rng.integers(0, 64)
        t[by, bx, :3] = np.minimum(t[by, bx, :3].astype(int) + 60, 255)
    tex = Image.fromarray(t, 'RGBA')
    m = np.zeros((64, 64), bool)
    m[y0:y1 + 1, 14:43] = True
    if round_bottom:
        for k in range(3):
            m[y1 - k, 15:15 + 3 - k] = False
            m[y1 - k, 41 - 3 + k:41] = False
    a = np.asarray(tex).copy(); a[~m, 3] = 0
    return Image.fromarray(a.astype(np.uint8), 'RGBA')
# Each layer reaches higher than the board's (one colour, so the overlap
# doesn't show): the board's are stacked 4 and 2 apart and ours, cut at its
# rows, left rows of the floor showing between them.
for code, y0, round_bottom in (('7f', 38, True), ('21', 38, False), ('0b', 45, True), ('0c', 45, False)):  # beer x 14 to 42, the glass's inside
    beer(y0, 57, round_bottom).save(f'{O}/beer-{code}.png')
    lines.append(f'pic sprite-bonus_object-{code} art/obj/beer-{code}.png 32 32 0 0')
# The foam on top (0D: x 6 to 22, rows 3 to 10): the full mug's head.
head = full.crop((26, 0, 64, 24))
hb = np.asarray(head)[..., 3] > 0
ys, xs = np.nonzero(hb)
head = head.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1)).resize((34, 16), Image.LANCZOS)
fo = Image.new('RGBA', (64, 64)); fo.alpha_composite(head, (12, 6)); fo.save(f'{O}/foam-0d.png')
lines.append('pic sprite-bartender_part-0d art/obj/foam-0d.png 32 32 0 0')
# A customer's full mug, its foam spilling (23, x 0 to 23): the full mug
# with its head, in that box.
c23 = Image.new('RGBA', (64, 64))
fm = full.crop(full.getbbox())
k = min(48 / fm.width, 64 / fm.height)
fm = fm.resize((round(fm.width * k), round(fm.height * k)), Image.LANCZOS)
c23.alpha_composite(fm, ((48 - fm.width) // 2, 64 - fm.height)); c23.save(f'{O}/mug-23.png')
lines.append('pic sprite-customer-23 art/obj/mug-23.png 32 32 0 0')
# The mug on its side as he kicks it at a level's end (59, x 0 to 30,
# rows 13 to 31): the empty glass laid down.
lay = empty.crop(empty.getbbox()).rotate(90, expand=True)
k = min(62 / lay.width, 38 / lay.height)
lay = lay.resize((round(lay.width * k), round(lay.height * k)), Image.LANCZOS)
c59 = Image.new('RGBA', (64, 64)); c59.alpha_composite(lay, ((62 - lay.width) // 2, 64 - lay.height)); c59.save(f'{O}/mug-59.png')
lines.append('pic sprite-bonus_object-59 art/obj/mug-59.png 32 32 0 0')
# His hand on the pulled lever (part 03): runway/pour.py.
# The beer running from the tap into the glass (bartender part 0A, a
# dotted line at x 12 to 19, rows 0 to 30): a steady amber stream with a
# bright edge, ending in the beer.
st = np.zeros((64, 64, 4), np.uint8)
c0 = fa[26:50][((fa[26:50, :, 0] > 170) & (fa[26:50, :, 2] < 90))][:, :3]
amber = np.median(c0, axis=0).astype(np.uint8) if len(c0) else np.array([230, 160, 40], np.uint8)
for y in range(0, 60):
    for x in (31, 32):
        st[y, x, :3] = amber; st[y, x, 3] = 190
    st[y, 31, :3] = np.minimum(amber.astype(int) + 50, 255)
Image.fromarray(st, 'RGBA').save(f'{O}/stream-0a.png')
lines.append('pic sprite-bartender_part-0a art/obj/stream-0a.png 32 32 0 0')
# The glass at his mouth as he drinks at a level's end: in his own
# picture (player 41, its top right corner: the logo's blue 4, the rim 14,
# the drink 10) and the glass's edge (bartender part 24: 4 and 10),
# recoloured so the rest of him stays the game's, pixel for pixel.
# The original's glass (its colours in his picture, all of the glass's
# edge) drawn as nothing, and our full mug turned on its side, rim at his
# mouth, drawn with him instead (its place measured on the original's
# frame: the glass from 19 to 37 across his picture, beside his face; the
# original's glass at x 22 to 31, rows 0 to 14 of his picture, all hidden).
# The original's glass: in his picture x 22 to 31, rows 1 to 8 (all of it),
# its left wall at x 22 to 23 down to row 13, and the glass's colours (4,
# 10, 14) by his hand; and the glass's right half, bartender part 24, all
# of it (it shows only here). His hand (x 25 to 31, rows 9 to 14) and arm
# stay the game's, in front of the glass as in the original.
lines.append('#+ recolor sprite-player-41 box=22,0,31,8 2=- 3=- 4=- 7=- 9=- 10=- 12=- 14=- 15=-')
lines.append('#+ recolor sprite-player-41 box=22,9,31,14 4=- 10=- 14=-')
lines.append('#+ recolor sprite-player-41 box=22,9,23,13 9=-')
lines.append('#+ recolor sprite-bartender_part-24 9=- 4=- 10=-')
# Our clear glass (the empty mug, its inside faint, as under the tap)
# standing in the original glass's box, rows 2 to 20 of his picture, its
# body's right side on the original's (x 41): its handle on the left, in
# his hand (the hand and arm drawn in front of it). Drawn at x 14 of his
# picture, 32 wide.
# The beer in it is the board's bands (bonus object 15), which the page
# draws over him: each band ours, only inside the glass and right of his
# hand, so the glass drains as the original's does and his hand stays in
# front.
sp41 = sprites[4*32:5*32, 1*32:2*32]
R = '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/samples'
hid = np.zeros((32, 32), bool)
hid[0:9, 22:32] = sp41[0:9, 22:32] != 0
hid[9:15, 22:32] |= np.isin(sp41[9:15, 22:32], (4, 10, 14))
hid[9:14, 22:24] |= sp41[9:14, 22:24] == 9
front = (sp41 != 0) & ~hid
# The glass at his mouth: Runway's drawing (samples/drink-glass.png, the
# empty one), our glass tipped back toward his mouth, its tilt eased to 10
# degrees, 46 high (23 of his picture's rows: a little bigger than the
# original's 19), its right side on the original glass's (x 41), its
# bottom at row 21; his hand and arm in front of it.
DX, DY = 14, -4
from PIL import ImageDraw as _ID
dsrc = Image.open(f'{R}/drink-glass.png').convert('RGB').crop((960, 0, 1920, 1280))
_ID.floodfill(dsrc, (5, 5), (255, 0, 255), thresh=40)
dg0 = np.asarray(dsrc.convert('RGBA')).copy()
dg0[np.all(dg0[..., :3] == (255, 0, 255), axis=2), 3] = 0
dgi = Image.fromarray(dg0, 'RGBA')
dgi = dgi.crop(dgi.getbbox()).rotate(-15, expand=True, resample=Image.BICUBIC)
dgi = dgi.crop(dgi.getbbox())
k = 46 / dgi.height
dgi = dgi.resize((round(dgi.width * k), 46), Image.LANCZOS)
def _inside(a, seed):
    # The glass's inside: its light pixels reached from the body's middle.
    light = (a[..., 3] > 0) & (a[..., :3].min(axis=2) > 165)
    h, w = light.shape
    m = np.zeros((h, w), bool)
    stack = [seed]
    while stack:
        y, x = stack.pop()
        if 0 <= y < h and 0 <= x < w and light[y, x] and not m[y, x]:
            m[y, x] = True
            stack += [(y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)]
    return m
gi = _inside(np.asarray(dgi).astype(int), (23, int(dgi.width * 0.62)))
# Its inside's bottom on the lowest beer band's last row (his row 20).
gx = (41 + 1 - DX) * 2 - dgi.width
gy = (20 - DY) * 2 + 1 - int(np.nonzero(gi.any(axis=1))[0].max())
dg = Image.new('RGBA', (64, 64))
dg.alpha_composite(dgi, (gx, gy))
da = np.asarray(dg).copy()
inside = np.zeros((64, 64), bool)
ys_, xs_ = np.nonzero(gi)
for y, x in zip(ys_ + gy, xs_ + gx):
    if 0 <= y < 64 and 0 <= x < 64:
        inside[y, x] = True
# Its inside nearly clear, as the pour glass's is, so the beer bands drawn
# under it show through.
da[inside, 3] = (da[inside, 3] * 0.12).astype(np.uint8)
for y in range(32):
    for x in range(32):
        Y = (y - DY) * 2
        if front[y, x] and 0 <= (x - DX) * 2 < 64 and 0 <= Y < 64:
            da[Y:Y+2, (x-DX)*2:(x-DX)*2+2, 3] = 0
Image.fromarray(da, 'RGBA').save(f'{O}/mug-drink.png')
lines.append(f'also sprite-player-41 art/obj/mug-drink.png 32 32 {DX} {DY}')
# Each beer band (its rows 17 to 20 of its picture, at x 11 of his): the
# glass's inside at the lowest band's rows (his rows 17 to 20), in our
# beer's colour, 6 to the right in the band's picture (x 11 of his, the
# glass's picture at 14); the higher bands sit 4 rows up each.
# Smooth amber: our beer's colour, lighter at the glass's middle and darker
# at its walls; a row over the band's top so the bands meet.
c0 = fa[26:50][((fa[26:50, :, 0] > 170) & (fa[26:50, :, 2] < 90))][:, :3]
base = np.median(c0, axis=0) if len(c0) else np.array([230, 160, 40])
bd = np.zeros((64, 64, 4), np.uint8)
for r in range(33, 42):
    ys = r - 2 * DY
    xs = np.nonzero(inside[ys])[0] if 0 <= ys < 64 else []
    if not len(xs):
        continue
    for x in xs:
        X = x + 6
        if 0 <= X < 64:
            u = (x - xs[0]) / max(1, xs[-1] - xs[0])
            shade = 0.82 + 0.3 * (1 - abs(u - 0.4) * 2)
            bd[r, X, :3] = np.clip(base * shade, 0, 255).astype(np.uint8)
            bd[r, X, 3] = 255
Image.fromarray(bd, 'RGBA').save(f'{O}/beer-15.png')
lines.append('pic sprite-bonus_object-15 art/obj/beer-15.png 32 32 0 0')
# His mug after he drinks at a level's end, kicked and broken (bonus
# objects 17, then 10 to 13): the theme's frosted mug and its pieces, as
# the mugs sent back along the bars are.
for code in ('17', '10', '11', '12', '13'):
    lines.append(f'pic sprite-bonus_object-{code} art/obj/mug-{code}.png 32 32 0 0')

art = open(f'{T}/art.txt').read()
a0, a1 = '# --- objects (runway/objects.py) ---\n', '# --- end objects ---\n'
block = a0 + '\n'.join(lines) + '\n' + a1
if a0 in art:
    art = art[:art.index(a0)] + block + art[art.index(a1) + len(a1):]
else:
    art = art.replace('scale 2\n', 'scale 2\n' + block, 1)
art = art.replace('# The tap at rest, in brass (the board\'s sprite 09).\npic sprite-tap-09 art/tap09.png 32 32 0 0\n', '')
open(f'{T}/art.txt', 'w').write(art)
print(len(lines), 'lines')
