"""The punk room's wall lamps: a bare bulb in a red enamel socket, in three states of light.

    python3 runway/lamp.py THEME_DIR

The board's lamp is a socket on the wall with its bulb out to the side, 18 by 14 game
pixels, and one of its colours steps through a cycle (the model's counter cycle_step:
1 dim, 2 mid, 3 bright, 4 mid, 9 frames each). Three pictures here, the same lamp with
its glass and its glow at each of those, 32 by 24 game pixels with the lamp in the
middle, drawn as the left wall has it (socket left, bulb right); the room rules mirror
it with its cell on the right wall.
"""
import sys

from PIL import Image, ImageDraw

O = (38, 24, 18, 255)
W, H = 64, 62
CX, CY = 32, 24


def lamp(state):
    im = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    glass, core, hi, glow, reach = {
        'dim': ((150, 34, 24, 255), (190, 52, 34, 255), (244, 150, 120, 255), None, 0),
        'mid': ((255, 170, 60, 255), (255, 214, 110, 255), (255, 250, 220, 255), (255, 190, 80, 255), 15),
        'bright': ((255, 226, 130, 255), (255, 248, 200, 255), (255, 255, 255, 255), (255, 224, 120, 255), 22),
    }[state]
    bx, by = CX + 8, CY          # the bulb's middle
    # the glow: a dithered round of warm light on the wall round the bulb
    if glow:
        for y in range(H):
            for x in range(W):
                r2 = ((x - bx) / 1.15) ** 2 + (y - by) ** 2
                if r2 <= reach * reach:
                    near = r2 <= (reach * 0.6) ** 2
                    if near and (x + y) % 2 == 0 or (not near and x % 2 == 0 and y % 2 == 0 and (x // 2 + y // 2) % 2 == 0):
                        im.putpixel((x, y), glow)
    # the lamp's own shadow on the wall, under the socket, as the board's lamp has
    d.polygon([(CX - 19, CY + 10), (CX - 3, CY + 10), (CX - 3, CY + 16), (CX - 7, CY + 22), (CX - 7, CY + 30), (CX - 15, CY + 30), (CX - 15, CY + 22), (CX - 19, CY + 16)], fill=(74, 78, 86, 255))
    # the socket: a red enamel dish on the wall, seen from its side, and a brass neck
    d.ellipse((CX - 18, CY - 13, CX - 6, CY + 13), fill=O)
    d.ellipse((CX - 16, CY - 11, CX - 8, CY + 11), fill=(206, 44, 36, 255))
    d.ellipse((CX - 16, CY - 11, CX - 11, CY + 5), fill=(240, 96, 70, 255))
    d.arc((CX - 16, CY - 11, CX - 8, CY + 11), 60, 120, fill=(128, 20, 20, 255), width=2)
    d.rectangle((CX - 9, CY - 6, CX - 1, CY + 6), fill=O)
    d.rectangle((CX - 8, CY - 5, CX - 2, CY + 5), fill=(193, 136, 36, 255))
    d.rectangle((CX - 8, CY - 5, CX - 2, CY - 3), fill=(255, 236, 150, 255))
    d.line((CX - 5, CY - 5, CX - 5, CY + 5), fill=(128, 82, 16, 255))
    # the bulb: a pear of glass, its bright core, a glint
    d.ellipse((bx - 11, by - 10, bx + 12, by + 10), fill=O)
    d.polygon([(CX - 2, CY - 6), (bx - 4, by - 10), (bx - 4, by + 10), (CX - 2, CY + 6)], fill=O)
    d.ellipse((bx - 9, by - 8, bx + 10, by + 8), fill=glass)
    d.polygon([(CX - 1, CY - 4), (bx - 4, by - 8), (bx - 4, by + 8), (CX - 1, CY + 4)], fill=glass)
    d.ellipse((bx - 5, by - 5, bx + 7, by + 5), fill=core)
    d.rectangle((bx + 1, by - 5, bx + 4, by - 3), fill=hi)
    d.point((bx + 6, by - 2), fill=hi)
    if state == 'dim':
        d.line((bx - 4, by, bx + 3, by), fill=(96, 20, 16, 255))     # the cold filament
    return im


def main():
    t = sys.argv[1]
    for s in ('dim', 'mid', 'bright'):
        lamp(s).save(f'{t}/pieces/punk/lamp_{s}.png')
    sheet = Image.new('RGBA', (W * 3 * 4, H * 4), (120, 126, 134, 255))
    for i, s in enumerate(('dim', 'mid', 'bright')):
        big = lamp(s).resize((W * 4, H * 4), Image.NEAREST)
        sheet.alpha_composite(big, (i * W * 4, 0))
    sheet.convert('RGB').save(sys.argv[2] if len(sys.argv) > 2 else '/dev/null', 'PNG') if len(sys.argv) > 2 else None
    print('lamps written', W, H)


if __name__ == '__main__':
    main()
