"""The ballpark stand's tiers, drawn for the room's own view.

    python3 runway/bleachers.py OUT.png [TIER]

Tiers run down the screen leaning right, 1 pixel across for 3 down, as the stand's edge does. TIER is one tier's
width in stage pixels (58 by default): solid concrete steps: a riser in hard shadow and a lit tread. The concrete is samples/concrete16.png.
"""
import os
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
out_path = sys.argv[1]
T = float(sys.argv[2]) if len(sys.argv) > 2 else 48.0
con = np.asarray(Image.open(f'{HERE}/samples/concrete16.png').convert('RGB').crop((0, 0, 1365, 1280)).resize((1024, 960), Image.BOX)).astype(float)
Y, X = np.mgrid[0:960, 0:1024]
u = X - Y / 3.0
# The original's rows stand 24 game pixels apart, a dark line 4 wide at the start of each, the first on the stand's
# edge (u = 632 in stage pixels): the hoods' stepped feet stand on those lines.
BASE = 628.0   # 2 game pixels left of the board's line, so each line's shadow starts flush against a bar's end
t = np.mod(u - BASE, T) / T   # 0 to 1 across a step
k = np.floor((u - BASE) / T)
out = con * 0.92
O = np.array([38, 24, 18.0])


def band(lo, hi):
    return (t >= lo) & (t < hi)


line = 2.0 / T
# Solid concrete steps, no seats: each step's riser in hard shadow, its tread in the light with a bright front edge,
# a dark line where the next riser stands on it, and a joint across the steps now and then.
out[band(0, line)] = O
m = band(line, 8.0 / T); out[m] = con[m] * 0.50                      # the riser, in shadow: as wide as the original's line
m = band(8.0 / T, 8.0 / T + 1.5 * line); out[m] = np.minimum(255, con[m] * 1.22)   # the tread's lit edge
m = band(8.0 / T + 1.5 * line, 0.94); out[m] = np.minimum(255, con[m] * 1.04)   # the tread
m = band(0.94, 1.0); out[m] = con[m] * 0.88                          # the next riser's cast shadow on it
v = Y + k * 53
step = 3.6 * T
jn = (np.mod(v, step) < 2) & band(line, 1.0); out[jn] = out[jn] * 0.6
# The stand's top steps against the sky, where the board has a ledge on each: a dark top edge and a lit tread under it.
# Each runs the whole width of its step, from the step's own left edge to where the next one rises, so no part of a
# step's top is left in the plain pattern; the two at the very top have theirs too.
for (x0, x1, y) in ((458, 512, 0), (440, 458, 6), (419, 440, 18), (398, 419, 30), (380, 398, 42), (359, 380, 54), (338, 359, 66)):
    box = (X >= x0 * 2) & (X < x1 * 2)
    m = box & (Y >= y * 2) & (Y < y * 2 + 3); out[m] = O
    m = box & (Y >= y * 2 + 3) & (Y < y * 2 + 9); out[m] = np.minimum(255, con[m] * 1.2)
    m = box & (Y >= y * 2 + 9) & (Y < y * 2 + 11); out[m] = con[m] * 0.8
# The first step's face, toward the field, is the stand's front: twice as deep as the other risers so it reads all
# the way up the stand's edge, with its lit edge above it.
FACE = 16.0
below = Y >= 66 * 2 + 11                    # under the lowest step's top, so the face doesn't cut across that top
m = below & (u >= BASE + 2) & (u < BASE + FACE); out[m] = con[m] * 0.50
m = below & (u >= BASE + FACE) & (u < BASE + FACE + 3); out[m] = np.minimum(255, con[m] * 1.22)
# Left of the stand's edge the picture is clear, so what lies under it (the field) meets the first step's face on one
# straight line, where the board's own stepped edge left slivers of concrete.
rgba = np.dstack([np.clip(out, 0, 255).astype(np.uint8), np.where(u < BASE, 0, 255).astype(np.uint8)])
Image.fromarray(rgba, 'RGBA').save(out_path)
