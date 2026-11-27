# The pour's hands (samples/pour-parts.png, drawn from the original's frame
# and ours): on the glass under the tap (bartender parts 02 and 05, its
# hand and sleeve over our clear glass) and on the pulled lever (part 03,
# his hand, sleeve and the wooden lever, meeting our brass tap).
import numpy as np
from PIL import Image
R = '/Users/alex/.gameprinter/work/themes/tapper-pixel/runway/samples'
T = '/Users/alex/.gameprinter/work/themes/tapper-pixel/repo/theme/art/obj'
src = Image.open(f'{R}/pour-parts.png').convert('RGB')
a = np.asarray(src.convert('RGBA')).astype(int)
# The backdrop: the white reached from the picture's edges, so the white
# of a sleeve inside its outline stays.
from PIL import ImageDraw
fl = src.copy()
for seed in ((0, 0), (fl.width - 1, 0), (0, fl.height - 1), (fl.width - 1, fl.height - 1), (fl.width // 2, 0), (fl.width // 2, fl.height - 1)):
    ImageDraw.floodfill(fl, seed, (255, 0, 255), thresh=40)
on = ~(np.all(np.asarray(fl) == (255, 0, 255), axis=2))
r, g, b = a[..., 0], a[..., 1], a[..., 2]
skin = on & (r > 180) & (g > 100) & (g < 200) & (b > 50) & (b < 160) & (r - b > 60)
white = on & (a[..., :3].min(axis=2) > 170) & (abs(r - b) < 40)
wood = on & (r > 70) & (r < 170) & (g < 0.7 * r) & (b < 0.55 * r) & ~skin
dark = on & (a[..., :3].max(axis=2) < 70)
def grow(m, k):
    out = m.copy()
    for dy in range(-k, k + 1):
        for dx in range(-k, k + 1):
            out |= np.roll(np.roll(m, dy, 0), dx, 1)
    return out
def part(mask, box):
    x0, y0, x1, y1 = box
    m = np.zeros_like(mask); m[y0:y1, x0:x1] = mask[y0:y1, x0:x1]
    m |= dark & grow(m, 3)
    m[:y0] = False; m[y1:] = False; m[:, :x0] = False; m[:, x1:] = False
    out = a.copy(); out[..., 3] = np.where(m, 255, 0)
    return Image.fromarray(out.astype(np.uint8), 'RGBA')
def place(img, k, at, src, size=64):
    sm = img.resize((round(img.width * k), round(img.height * k)), Image.LANCZOS)
    out = Image.new('RGBA', (size, size))
    ox, oy = round(at[0] - src[0] * k), round(at[1] - src[1] * k)
    out.alpha_composite(sm.crop((max(0, -ox), max(0, -oy), sm.width, sm.height)), (max(0, ox), max(0, oy)))
    return out
# The glass: its hand (gen x 160..390, y 580..850) and sleeve (x 80..470,
# y 90..520). Scaled by the glass: gen's body x 385..790 is our glass's
# 11..44 (k = 34/405), its left edge at our 11.
k = 34 / 405
hand = part(skin, (150, 570, 400, 860))
sleeve = part(white, (70, 80, 480, 530))
glass = Image.open(f'{T}/glass-pour.png').convert('RGBA')
glass.alpha_composite(place(sleeve, k, (11, 12), (385, 290)))
glass.alpha_composite(place(hand, k, (11, 12), (385, 290)))
glass.save(f'{T}/glass-pour-hand.png')
# The lever: hand, sleeve and wooden lever (gen x 930..1500, y 220..920);
# its lever's foot (gen 1480, 590) at the brass tap's socket, 37, 47 of
# part 03 (measured on the original's frame); the hand 24 wide (k 0.075).
lever = part(skin | white | wood, (930, 220, 1500, 920))
place(lever, 0.075, (37, 47), (1480, 590)).save(f'{T}/pull-03.png')
print('ok')
