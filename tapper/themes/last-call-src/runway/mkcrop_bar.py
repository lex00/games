"""The crop of the space room's bar for Runway to look at, and the deny-list check of its prompt.

    python3 runway/mkcrop_bar.py

Writes refs/spacebar-crop.png: a stretch of the board's first bar (its slab only, game
x 160..288, y 110..146) enlarged on flat magenta, the void's black turned magenta.
Prints the deny words found in prompts/spacebar16.txt (none means it may be sent).
"""
import os
import tomllib

from PIL import Image

HERE = os.path.dirname(__file__)


def main():
    im = Image.open(os.path.expanduser('~/.gameprinter/work/theme-kits/tapper/board/screen_8021.png')).convert('RGB')
    c = im.crop((160, 110, 288, 146))
    k = 15
    big = Image.new('RGB', (1920, 1280), (255, 0, 255))
    big.paste(c.resize((c.width * k, c.height * k), Image.NEAREST), (0, (1280 - c.height * k) // 2))
    big.save(os.path.join(HERE, 'refs/spacebar-crop.png'))
    big.resize((480, 320)).save(os.path.expanduser('~/.gameprinter/work/themes/last-call-floor/spacebar-crop.png'))
    words = tomllib.load(open(os.path.expanduser('~/Documents/checkouts/lex00/games/tapper/themes/kit/words.toml'), 'rb'))
    import sys
    name = sys.argv[1] if len(sys.argv) > 1 else 'spacebar16'
    text = open(os.path.join(HERE, f'prompts/{name}.txt')).read().lower()
    print('deny words in the prompt:', [d for d in words.get('deny', []) if d.lower() in text])


if __name__ == '__main__':
    main()
