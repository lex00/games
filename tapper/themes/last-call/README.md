# Last Call

A theme for Tapper on Game Printer: new pictures, drawn by you, for every body, stage and screen. The game plays as it always does; every body keeps its place and its hit box, and only the pictures change. A player picks it under Settings, Theme.

## What's here

- `package.toml` names the package, `lex00/last-call`, and its licence (CC-BY-4.0).
- `theme/look.txt` names the look and its rules file.
- `theme/art.txt` holds the rules (`gp-art 1`): which of the game's pictures each of yours replaces, and at what size. Each line marked TO DRAW is a picture still to make.
- `theme/art/` holds your pictures, as PNG, at 2 times the game's pixel size.
- `preview.png` is the first bar in this look, for lists of games: the stage, the sign, the casks, one full mug and the score, with no one at the bar. Every pixel in it is this theme's own; the people, who are still the board's pictures, are left out.

## The rules

- Your own art only: no character's likeness, name or logo from the original, and nothing traced.
- Nothing from the ROM: no picture, sound or data read from it. A stage is never a picture here; the rules build it from your pieces over the level picture the player's own ROM gives.
- No names from the original. The game's kit lists the names a theme may not use: tapper, bally, midway, budweiser, bud, root beer, rootbeer, beer, anheuser, busch, saloon, cowboy, punk, bandit, sports bar, space bar, keg, bartender, barman, mustache, moustache, bow tie, suspenders, stetson, cowboy hat.
- If an AI tool made any of it, name the tool here, and make sure its terms let you share the result.

## How the pictures were made

Many of the pictures began as images made with Runway's image models (gen4_image, gen4_image_turbo and gpt_image_2) from our own words and our own earlier pictures, then were cut, fitted and repainted by hand and by script to the game's sizes. The rest were drawn by script.

## Check it

```sh
gp theme look check . --game tapper
```

It reads the theme the way the page does and lists what is wrong and where, what is still to draw, how each picture sits on its body's box and feet row, names the theme may not use, and whether any file holds data from your ROM. Fix the problems first; warnings are worth a look.

## See it in the game

    gp theme look play /Users/alex/.gameprinter/work/themes/tapper-pixel/repo --game tapper --open
    gp theme look play /Users/alex/.gameprinter/work/themes/tapper-pixel/repo --game tapper --shots DIR   (pictures at the game's moments)

Look at every stage, every pose at game size, the deaths and the score display.

Keep the pictures in Git LFS (`.gitattributes` does that once you run `git lfs install`).
