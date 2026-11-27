"""The tap, one design drawn by this script into every picture the board shows it in.

    python3 runway/tap16.py THEME_DIR

The board draws the tap as one sprite at rest (tap 09) and, while he pours,
as parts of his own sprites: 06 (his hand reaching, the handle still up),
03 (the handle pulled), and 02 or 05 (the spout's end over the mug). All of
them put the tap at the same place on the screen; only the 32 by 32 cell it
is cut into moves. So the tap is drawn once here, in the rest sprite's cell
at twice the game's pixels, and cut for each of the others:

  art/obj/tap-09.png     the tap at rest, the whole picture
  art/obj/tap-06.png     the same tap 6 game pixels lower in its cell (03 and 06 sit 6 higher on the screen)
  art/obj/tap-03.png     the pulled tap, only where the original shows its tap and not his hand
  art/obj/pour-02.png    the pour glass (art/obj/glass-pour-body.png) with the spout's end above it
  pieces/saloon/idle_tap.png  the tap at rest, for the screens that draw the barrels with no tap sprite

The rules that go with them (art.txt) hide the original's tap inside the
same boxes and lay these over it, so his hand and arm stay the board's.
"""
import math
import os
import sys

from PIL import Image, ImageDraw

O = (38, 24, 18, 255)
B_HI, B_MID, B_LO, B_DK = (255, 236, 150, 255), (232, 190, 80, 255), (193, 136, 36, 255), (128, 82, 16, 255)
W_HI, W_MID, W_LO = (214, 120, 44, 255), (160, 72, 18, 255), (108, 44, 12, 255)

PIVOT = (28, 44)       # where the handle meets the valve, in the rest cell, stage pixels
UP = (28, 16)          # the knob's middle with the handle up
PULLED = (6, 22)       # and with it pulled: 11 game pixels left and 3 down, as the original's knob moves


def brass_v(d, box):
    """A brass bar lit from above: light, mid, low, dark bands down its height."""
    x0, y0, x1, y1 = box
    h = y1 - y0 + 1
    for i in range(h):
        c = B_HI if i < h * 0.2 else B_MID if i < h * 0.5 else B_LO if i < h * 0.8 else B_DK
        d.line((x0, y0 + i, x1, y0 + i), fill=c)


def tap(knob):
    im = Image.new('RGBA', (64, 64), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    # the flange on the barrel's face: a disc seen from the side
    d.ellipse((45, 35, 58, 61), fill=O)
    d.ellipse((47, 37, 56, 59), fill=B_LO)
    d.ellipse((47, 37, 53, 57), fill=B_MID)
    d.ellipse((48, 39, 50, 46), fill=B_HI)
    # the pipe from it to the valve
    d.rectangle((32, 43, 48, 53), fill=O)
    brass_v(d, (32, 44, 48, 52))
    # the valve's body and the spout down from it
    # the spout's middle stands at cell x 14, where the board's own is: the board never flips the stream, so only that
    # place puts the stream under the spout in a room it draws mirrored
    d.rounded_rectangle((22, 39, 35, 57), radius=4, fill=O)
    d.rounded_rectangle((23, 40, 34, 56), radius=3, fill=B_MID)
    d.rectangle((24, 41, 26, 54), fill=B_HI)
    d.rectangle((32, 42, 33, 55), fill=B_LO)
    d.polygon([(23, 56), (34, 56), (32, 63), (25, 63)], fill=O)
    d.polygon([(24, 56), (33, 56), (31, 62), (26, 62)], fill=B_LO)
    d.line((25, 57, 26, 61), fill=B_MID)
    d.line((27, 63, 30, 63), fill=(20, 14, 12, 255))
    # the handle: a turned wooden grip on a brass collar
    px, py = PIVOT
    kx, ky = knob
    ang = math.atan2(ky - py, kx - px)
    nx, ny = -math.sin(ang), math.cos(ang)
    def quad(t0, t1, half, fill):
        a = (px + (kx - px) * t0, py + (ky - py) * t0)
        b = (px + (kx - px) * t1, py + (ky - py) * t1)
        d.polygon([(a[0] + nx * half, a[1] + ny * half), (b[0] + nx * half, b[1] + ny * half),
                   (b[0] - nx * half, b[1] - ny * half), (a[0] - nx * half, a[1] - ny * half)], fill=fill)
    quad(0.0, 1.0, 4.5, O)
    quad(0.0, 0.22, 3.5, B_MID)                 # the collar
    quad(0.22, 1.0, 3.2, W_MID)                 # the grip
    quad(0.22, 1.0, 1.2, W_HI)
    d.ellipse((kx - 6, ky - 6, kx + 6, ky + 6), fill=O)
    d.ellipse((kx - 5, ky - 5, kx + 5, ky + 5), fill=W_MID)
    d.ellipse((kx - 4, ky - 4, kx, ky), fill=W_HI)
    d.ellipse((kx + 1, ky + 1, kx + 4, ky + 4), fill=W_LO)
    return im


def shifted(im, dy):
    out = Image.new('RGBA', im.size, (0, 0, 0, 0))
    out.paste(im, (0, dy))
    return out


def keep(im, boxes):
    """Only the pixels inside the boxes (game pixels, both ends counted)."""
    out = Image.new('RGBA', im.size, (0, 0, 0, 0))
    for x0, y0, x1, y1 in boxes:
        b = (x0 * 2, y0 * 2, x1 * 2 + 2, y1 * 2 + 2)
        out.paste(im.crop(b), b[:2])
    return out


# Where the original's tap, and none of his hand, stands in each of his sprites (game pixels in the sprite).
BOX_06 = [(11, 11, 16, 17), (12, 18, 31, 31)]
BOX_03 = [(0, 13, 7, 20), (12, 24, 31, 31)]
BOX_02 = [(11, 0, 31, 5)]


def main():
    t = sys.argv[1]
    up, pulled = tap(UP), tap(PULLED)
    up.save(f'{t}/art/obj/tap-09.png')
    up.save(f'{t}/pieces/saloon/idle_tap.png')
    keep(shifted(up, 12), BOX_06).save(f'{t}/art/obj/tap-06.png')
    keep(shifted(pulled, 12), BOX_03).save(f'{t}/art/obj/tap-03.png')
    glass = Image.open(f'{t}/art/obj/glass-pour-body.png').convert('RGBA')
    end = keep(shifted(up, -52), BOX_02)
    pour = Image.new('RGBA', (64, 64), (0, 0, 0, 0))
    pour.alpha_composite(glass)
    pour.alpha_composite(end)
    pour.save(f'{t}/art/obj/pour-02.png')
    print('tap pictures written; boxes', BOX_06, BOX_03, BOX_02)


if __name__ == '__main__':
    main()
