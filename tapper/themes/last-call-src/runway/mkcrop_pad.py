"""The crop of the space room's pad for Runway to look at, and the deny-list check of its prompt.

    python3 runway/mkcrop_pad.py

Writes refs/pad-crop.png: the board's pad and stem (game x 296..360, y 140..196) enlarged
on flat magenta, the void's black turned magenta.
"""
import os
import tomllib

from PIL import Image

HERE = os.path.dirname(__file__)


def main():
    im = Image.open(os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/screen_8021.png')).convert('RGB')
    c = im.crop((296, 140, 360, 196))
    px = c.load()
    for y in range(c.height):
        for x in range(c.width):
            if px[x, y] == (0, 0, 0):
                px[x, y] = (255, 0, 255)
    k = 20
    big = Image.new('RGB', (1920, 1280), (255, 0, 255))
    big.paste(c.resize((c.width * k, c.height * k), Image.NEAREST), ((1920 - c.width * k) // 2, (1280 - c.height * k) // 2))
    big.save(os.path.join(HERE, 'refs/pad-crop.png'))
    words = tomllib.load(open(os.path.expanduser('~/Documents/checkouts/lex00/games/tapper/themes/kit/words.toml'), 'rb'))
    text = open(os.path.join(HERE, 'prompts/disc16.txt')).read().lower()
    print('deny words in the prompt:', [d for d in words.get('deny', []) if d.lower() in text])


if __name__ == '__main__':
    main()
