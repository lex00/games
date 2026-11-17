# Neon Shift

A theme for the bar-serving arcade game `lex00/tapper` on Game Printer. The bar is a smart bar floating in a bright, playful 1990s retro-future city: warm orange and teal light, chrome and neon, flying traffic between tall towers. A chrome robot server on a hover base fills one kind of glowing drink and slides it down the counters to sixteen guests, humans and aliens side by side. The game plays as it always does: every body keeps its place and its hit box, and only the pictures change. A player picks it under Settings, Theme.

The game's levels and rules still come from the player's own romset, so it plays only for someone who has that romset.

## What's drawn

- The server: twelve poses of the robot's top half, its hover base, the drink dispensers and their levers, the foam, the streak of a sliding glass.
- Sixteen guests, four to each room, each with walking, waiting, surprised, drinking and pleased poses, and the glass a guest holds.
- The glass full, sloshing, empty, tipping and breaking; the coin a guest leaves.
- The between-rounds pieces: the cans, the trickster and the server's big close-up.
- The title screen and the score, level and credit digits, and the lives icon.
- All ten rooms, the demo's room and the between-rounds stage: neon counters with chrome tops, a dispenser at each counter's tap end, and a painted city, club, rooftop, orbital or stage view behind.

The game's own sound plays: this version gives no sound files.

## What's here

- `package.toml` names the package, `lex00/neon-shift`, and its licence (CC-BY-4.0).
- `theme/look.txt` names the look and its rules file.
- `theme/art.txt` holds the rules (`gp-art 1`): which of the game's pictures each of these replaces, and at what size.
- `theme/art/` holds the pictures, as PNG, at 2 times the board's pixel size.
- `preview.png` is the card picture, from the theme's own title art.

## How it was made

The pictures were made with Runway's API: the style board, the title backdrop and the rooms' backdrops with `gen4_image`, and every pose sheet, prop sheet and the lettering with `gpt_image_2`, from prompts written in plain words (no character, name or logo from the original, and no picture of the original sent). They were then keyed, cut and fitted to the board's pictures by small scripts. The rooms' counters, floors and glow, the motion streaks and the dispenser's pipe are drawn in code, at the places the game's rooms put them. Nothing in this folder comes from a ROM.

## Check it

```sh
gp theme look check . --game tapper
```

## See it in the game

```sh
gp theme look play . --game tapper --open
```
