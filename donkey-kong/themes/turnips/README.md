# Turnips

A clay look for Donkey Kong (NES), played from a ROM you own on a Game Printer page. Pick it under Settings, Theme: Original or Turnips.

The game is unchanged. Every body keeps its place and hit box; only the pictures change. Kong is a scowling root vegetable, the hero is a snail in a straw hat, the barrels are dung balls, the fires are scarabs, the hammer is a fly swatter, and the stages are wooden planks and ladders over a cream backdrop.

## What's here

- `theme/look.txt` names the look and its rules file.
- `theme/art.txt` holds the rules (`gp-art 1`, read by `gp_present::art` in [gameprinter](https://github.com/lex00/gameprinter)): which of the game's pictures each clay picture replaces and at what size, timed extra frames, the scarabs' facing, the ball Kong holds, the stump and the stink over the drum, and how each stage is built from pieces.
- `theme/art/` holds the pictures, at 6 to 8 times the game's pixel size.
- `package.toml` names the package, `lex00/turnips`.
- `preview.png` is the clay title screen, for lists of games.

## No ROM data

None of these files holds bytes from a Donkey Kong ROM. The stages aren't pictures: the rules build them in the player's browser, from pieces (a plank, a ladder rail, a rope, a wheel), over the level picture read from the player's own ROM. The lettering is our own clay alphabet. `gp template served` checks every file against both ROMs before a publish.

## How the art was made

The characters, props and letters were generated on Runway (gpt_image_2 and gen4_image_turbo) from our own prompts and references, then cut and fitted by hand. Sizes and offsets were set by eye in the running game.

## License

The pictures, the lettering and the rules are licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/) (CC BY 4.0); the full text is in `LICENSE`. You may use, change and share them, in free or paid games, as long as you credit them. Put this, or the same facts, where your game lists its credits:

> Turnips art by lex00 (https://github.com/lex00/turnips), licensed under CC BY 4.0.

Say if you changed anything. The licence covers only this repo's files: Donkey Kong, its name and its ROM belong to Nintendo and aren't licensed here.
