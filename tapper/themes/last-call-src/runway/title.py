# The title card in 14 stages, one for each look the game shows its title
# screen in (screen_attract_logo-1 to -14): our LAST CALL panel at the
# original panel's place, its letters empty at first and filling with drink
# from the bottom, letter after letter from the left, full by the last.
import numpy as np
from PIL import Image
R = '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/samples'
T = '/Users/alex/.gameprinter/work/themes/tapper-pixel/repo/theme'
src = np.asarray(Image.open(f'{R}/title.png').convert('RGB')).astype(int)
H, W, _ = src.shape
# The panel: everything not near black.
lit = src.max(axis=2) > 40
ys, xs = np.nonzero(lit)
panel = src[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
ph, pw, _ = panel.shape
r, g, b = panel[..., 0], panel[..., 1], panel[..., 2]
amber = (r > 170) & (g > 80) & (b < 110) & (g < r)
foam = (r > 200) & (g > 200) & (b > 170)
fill = amber | foam
# The panel's cream border isn't foam.
edge = 24
fill[:edge, :] = False; fill[-edge:, :] = False; fill[:, :edge] = False; fill[:, -edge:] = False
# The letters: columns where there is fill, in runs.
cols = fill.any(axis=0)
runs = []; start = None
for x, on in enumerate(list(cols) + [False]):
    if on and start is None: start = x
    if not on and start is not None:
        if x - start > 10: runs.append((start, x))
        start = None
# The foam joins the letters at the top, so each letter is found lower
# down, where they stand apart, and every column goes to the nearest one.
ry0 = np.nonzero(fill.any(axis=1))[0]
band = fill[int(ry0.min() + 0.45 * (ry0.max() - ry0.min())):int(ry0.min() + 0.9 * (ry0.max() - ry0.min()))]
bc = band.any(axis=0)
runs = []; start = None
for x, on in enumerate(list(bc) + [False]):
    if on and start is None: start = x
    if not on and start is not None:
        if x - start > 40: runs.append((start, x))
        start = None
print('runs', runs)
# A letter's legs (the A's) are one letter: runs closer than the narrowest
# gap between letters join.
gaps = sorted(runs[i + 1][0] - runs[i][1] for i in range(len(runs) - 1))
while len(runs) > 8:
    i = min(range(len(runs) - 1), key=lambda i: runs[i + 1][0] - runs[i][1])
    runs[i:i + 2] = [(runs[i][0], runs[i + 1][1])]
mids = [(a + b) / 2 for a, b in runs]
edges = [0] + [int((mids[i] + mids[i + 1]) / 2) for i in range(len(mids) - 1)] + [pw]
runs = [(edges[i], edges[i + 1]) for i in range(len(mids))]
print('letters', len(runs))
rows = fill.any(axis=1); ry = np.nonzero(rows)[0]; top, bot = ry.min(), ry.max()
empty = np.array([34, 12, 9])
# The original panel's box at the theme's scale 2: cells 3 to 28 by 7 to 16.
bx0, by0, bw, bh = 96, 224, 832, 320
k = min(bw / pw, bh / ph)
nw, nh = round(pw * k), round(ph * k)
for stage in range(1, 15):
    f = (stage - 1) / 13 * len(runs)
    out = panel.copy()
    for i, (x0, x1) in enumerate(runs):
        frac = max(0.0, min(1.0, f - i))
        # The letter's drink stands from its bottom up to frac of its height.
        level = bot - frac * (bot - top)
        sub = fill[:, x0:x1].copy()
        yy = np.arange(ph)[:, None]
        dry = sub & (yy < level)
        o = out[:, x0:x1]
        o[dry] = empty
        out[:, x0:x1] = o
    pic = Image.fromarray(out.astype(np.uint8)).resize((nw, nh), Image.LANCZOS if False else Image.BOX)
    screen = Image.new('RGB', (1024, 960), (0, 0, 0))
    screen.paste(pic, (bx0 + (bw - nw) // 2, by0 + (bh - nh) // 2))
    screen.save(f'{T}/art/title/title-{stage:02d}.png')
lines = ['pic screen_attract_logo art/title/title-14.png 512 480 0 0'] + [f'pic screen_attract_logo-{s} art/title/title-{s:02d}.png 512 480 0 0' for s in range(1, 15)]
art = open(f'{T}/art.txt').read()
a0, a1 = '# --- title (runway/title.py) ---\n', '# --- end title ---\n'
block = a0 + '\n'.join(lines) + '\n' + a1
art = art[:art.index(a0)] + block + art[art.index(a1) + len(a1):] if a0 in art else art.replace('scale 2\n', 'scale 2\n' + block, 1)
open(f'{T}/art.txt', 'w').write(art)
