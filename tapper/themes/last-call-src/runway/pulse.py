"""The disc's pulse: four pictures of Runway's disc, the rings swelling from dim to bright and back.

    python3 runway/pulse.py THEME_DIR [PREVIEW_PNG]

From pieces/space/disc.png: disc0 dim, disc1 as drawn, disc2 bright with a soft dithered
glow one pixel out from each ring, disc3 as drawn again. Nothing is added between the
rings but that thin glow, so the galaxy still shows through.
"""
import sys

from PIL import Image


def shade(im, k, glow):
    out = Image.new('RGBA', im.size, (0, 0, 0, 0))
    src = im.load()
    o = out.load()
    w, h = im.size
    if glow:
        for y in range(h):
            for x in range(w):
                if src[x, y][3]:
                    continue
                near = [src[x + dx, y + dy] for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)) if 0 <= x + dx < w and 0 <= y + dy < h and src[x + dx, y + dy][3]]
                if near and (x + y) % 2 == 0:
                    r, g, b, _ = near[0]
                    o[x, y] = (r // 2 + 20, g // 2 + 20, b // 2 + 40, 255)
    for y in range(h):
        for x in range(w):
            r, g, b, a = src[x, y]
            if a:
                if k > 1:
                    o[x, y] = (min(255, int(r + (255 - r) * (k - 1))), min(255, int(g + (255 - g) * (k - 1))), min(255, int(b + (255 - b) * (k - 1))), 255)
                else:
                    o[x, y] = (int(r * k), int(g * k), int(b * k), 255)
    return out


def main():
    theme = sys.argv[1]
    im = Image.open(f'{theme}/pieces/space/disc.png').convert('RGBA')
    frames = [shade(im, 0.55, False), shade(im, 0.85, False), shade(im, 1.55, True), shade(im, 0.85, False)]
    for i, f in enumerate(frames):
        f.save(f'{theme}/pieces/space/disc{i}.png')
    if len(sys.argv) > 2:
        w, h = im.size
        sheet = Image.new('RGBA', (w * 4 * 3, h * 3), (20, 10, 36, 255))
        for i, f in enumerate(frames):
            sheet.alpha_composite(f.resize((w * 3, h * 3), Image.NEAREST), (i * w * 3, 0))
        sheet.convert('RGB').save(sys.argv[2])
    print('four pulse pictures written', im.size)


if __name__ == '__main__':
    main()
