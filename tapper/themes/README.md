# Themes for Tapper

A theme changes how the game looks and sounds. The game itself, its rules in `model.toml`, stays the same.

`kit/` is the theme kit: the words a theme's prompts start from and must not use (`words.toml`), with the kit file. The pictures a theme draws are named by the board's names (a screen's slot, `sprite-ENTITY-CODE`, `counter-NAME-D`); `gp theme look sheet --game tapper` lists them from your own romset.

Each theme is a folder here, published to the registry as its own package. Its pictures are in Git LFS.

| Theme | What it is | Where it lives |
|---|---|---|
| Original | The game's own art and sound, lifted from your ROM when you play. Never stored in a repo. | Your ROM |
| Neon Shift | A smart bar floating in a playful 1990s retro-future city: a chrome robot server, sixteen guests (humans and aliens), neon rooms. New pictures that hold nothing from the ROM. | `neon-shift/` (package `lex00/neon-shift`) |

On the page, pick one under Settings, Theme.

When a new theme for this game is published, add a row here.
