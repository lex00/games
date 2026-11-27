"""The ballpark's bars, as one painting laid from the screen's corner: every bar's rows in painted wood.

    python3 runway/bar.py OUT.png LEG.png

A bar's top is at game row 112 and the bars stand 96 rows apart; all four are the same rows down. By row from a
bar's top (game pixels): 0 to 15 the top, four red painted planks; 16 to 17 its front edge and the shadow under it;
18 to 23 the frame's board; 24 to 27 shadow; 28 to 31 the shelf; 32 to 35 the footrest; 36 on, the dark under it.
The room's rules lay this on the pixels the board draws its bar in, so the bar keeps the board's exact shape.
"""
import sys
import numpy as np
from PIL import Image

H, W = 960, 1024
Y, X = np.mgrid[0:H, 0:W]
rng = np.random.default_rng(11)
r = np.mod(Y - 224, 192)                 # stage rows down from a bar's top
bar_no = (Y - 224) // 192
out = np.zeros((H, W, 3))
# long soft streaks of grain: noise stretched along the boards
streak = np.repeat(rng.normal(0, 1, (H, W // 24 + 2)), 24, axis=1)[:, :W]
streak = (streak + np.roll(streak, 7, axis=1)) / 2
fine = rng.normal(0, 1, (H, W))


def paint(mask, top, mid, low, t):
    """A board: light along its top, its own colour, shade along its bottom; t runs 0 to 1 down it."""
    for i in range(3):
        v = np.where(t < 0.5, top[i] + (mid[i] - top[i]) * (t / 0.5), mid[i] + (low[i] - mid[i]) * ((t - 0.5) / 0.5))
        out[..., i] = np.where(mask, v + streak * 5 + fine * 1.5, out[..., i])


# the top: four planks, 8 stage pixels each, each with a lit edge and a groove
top = r < 32
pl = np.mod(r, 8)
paint(top, (232, 84, 66), (206, 46, 36), (170, 30, 26), pl / 7.0)
m = top & (pl == 0); out[m] = (250, 128, 104)                                  # each plank's lit edge
m = top & (pl == 7); out[m] = (92, 14, 14)                                     # the groove
# plank ends staggered along the top, and a pair of nail heads by each
end = top & (np.mod(X + (r // 8) * 137 + bar_no * 61, 230) < 2); out[end] = (104, 18, 16)
nail = top & (np.mod(X + (r // 8) * 137 + bar_no * 61, 230) >= 6) & (np.mod(X + (r // 8) * 137 + bar_no * 61, 230) < 8) & ((pl == 2) | (pl == 5)); out[nail] = (70, 22, 20)
chip = top & (rng.random((H, W)) < 0.004) & (pl > 1) & (pl < 6); out[chip] = (196, 150, 96)   # worn paint, the wood showing
# its front edge, lit, and the shadow under it
m = (r >= 32) & (r < 34); out[m] = (236, 96, 76)
m = (r >= 34) & (r < 36); out[m] = (40, 14, 12)
# the frame's board
fr = (r >= 36) & (r < 48)
paint(fr, (196, 122, 62), (160, 92, 44), (116, 62, 30), (r - 36) / 11.0)
m = fr & (r == 36); out[m] = (224, 156, 90)
bolt = fr & (np.mod(X + bar_no * 61, 96) >= 46) & (np.mod(X + bar_no * 61, 96) < 50) & (r >= 40) & (r < 44); out[bolt] = (60, 46, 40)
m = fr & (np.mod(X + bar_no * 61, 96) == 46) & (r == 40); out[m] = (150, 140, 130)
# shadow under the frame
m = (r >= 48) & (r < 56); out[m] = (34, 16, 12)
# the shelf, set back in shade
sh = (r >= 56) & (r < 64)
paint(sh, (112, 40, 28), (92, 30, 22), (64, 20, 16), (r - 56) / 7.0)
# the footrest
ft = (r >= 64) & (r < 72)
paint(ft, (190, 116, 58), (150, 86, 42), (104, 56, 28), (r - 64) / 7.0)
m = ft & (r == 64); out[m] = (222, 152, 88)
# the dark under the bar, lifting a little toward the ground
m = (r >= 72); out[m] = (26, 14, 12)
m = (r >= 80) & (r < 88); out[m] = (38, 22, 16)
Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), 'RGB').save(sys.argv[1])
# the legs: upright grain, lit down the left
leg = np.zeros((H, W, 3))
vs = np.repeat(rng.normal(0, 1, (H // 24 + 2, W)), 24, axis=0)[:H]
for i, v in enumerate((132, 74, 36)):
    leg[..., i] = v + vs * 6 + fine * 1.5
Image.fromarray(np.clip(leg, 0, 255).astype(np.uint8), 'RGB').save(sys.argv[2])
