# Last Call: how its pictures were made

This folder is the source of the Last Call theme (`../last-call/`): the scripts that cut, fitted and drew its pictures, the words sent to the image models, and the images that came back. It is beside the theme and not inside it, because `gp registry publish` puts every file of the theme's folder in the package.

Nothing here is from the ROM. A picture of the original game is never kept here; the last section says how to make each one again from your own ROM.

## What is here

| Path | What it is |
|---|---|
| `runway/*.py` | 68 scripts, as they were run from 2026-11-16 to 2026-11-25. Each starts with what it does and how it is called |
| `runway/prompts/` | The words of each call to an image model, one file per picture |
| `runway/samples/` | The 82 images the calls gave back. The scripts cut the theme's pictures from these, and a new call would not give the same image, so they are kept |
| `runway/refs/` | Nine pictures of our own sent along with a prompt as a reference (an earlier result, or one of the theme's pictures) |
| `runway/spend.jsonl` | Each call: its name, model, credits and task id |
| `runway/gen16.sh` | The twelve paintings' calls in a row |
| `runway/po-moments.toml` | The play the theme's shots are taken from: power-on, then the presses of a run through the levels (`gp theme look play --moments`) |
| `ISSUES.md` | The owner's reports on the theme, each with its status and how it was checked |

`../last-call/theme/made-from.txt` says, for each picture of the theme, where it came from.

## The scripts are not a build

They were written one at a time during the work and each was run once or a few times, by hand, in the order the work went. No script runs them all. They are kept so that a picture can be made again or changed the way it was first made.

They fall into four kinds:

- Calls to the image models: `gen.py`, `gen16.sh`. The key is read from `RUNWAY_API_KEY`.
- Cutting a returned image into the theme's pictures: `build.py`, `objects.py`, `object16.py`, `font16.py`, `paintings16.py`, `paintings_front.py`, `craft.py`, `disc.py`, `ohno.py`, `refit_bay.py`, `spacebar.py`, `title.py`, `pour.py`, `pull.py`.
- Drawing a piece by rule, with no image model: `bar.py`, `pbar.py`, `bleachers.py`, `hood.py`, `hull.py`, `door.py`, `keg.py`, `lamp.py`, `neon.py`, `pulse.py`, `tap16.py`, `clear_mugs.py`, `drinkmug.py`, `heldmug.py`, `port.py`, `portfront.py`, `posters.py`, `tunnel_lamps.py`, `leftovers.py`, and the ones that write rules into `art.txt` (`wire_*.py`, `under.py`, `ride.py`, `behind.py`, `centre_sign.py`, `barends.py`, `bays.py`, `housekeep.py`).
- Looking at the game to measure it: where a clip draws a picture and what is near it (`find_codes.py`, `codes_over.py`, `near.py`, `near_code.py`, `who.py`, `legs.py`, `runs.py`, `pours.py`, `drink_timing.py`), a room's tiles (`pads.py`, `tunnel_ids.py`, `cycles.py`), and shots laid side by side (`strip.py`, `mkmoments.py`, `find_loud.py`, `probe_fill.py`, `preview_space.py`, `drinkers.py`, `cell_text.py`, `shared_part.py`).

## Paths the scripts expect

They were run from `~/.gameprinter/work/themes/tapper-pixel/`, as `python3 runway/NAME.py`, on a copy of the theme at `~/.gameprinter/work/themes/tapper-pixel/repo/theme`. Some name those places in full, and some read:

- the theme kit's local part, `~/.gameprinter/work/theme-kits/tapper/` (`gp theme kit`, `gp theme look sheet --game tapper`)
- the lifted pictures, `~/.gameprinter/work/assets/tapper@tapper/`
- a clip's sprite list (`SPRITES_JSONL` in a script's usage)
- the ROM itself (`cycles.py` only, for timing)

To run one, put this folder's `runway/` at that place or change the path at the script's top.

## Pictures of the original, and how to make them again

These were used during the work and are not kept, because each shows the original game's own pictures. All come from your own ROM.

| Picture | What it showed | How to make it |
|---|---|---|
| `refs/spacebar-crop.png` | The space room's first bar, game x 160 to 288, y 110 to 146, enlarged 15 times on flat magenta | `python3 runway/mkcrop_bar.py`, which reads the kit's `board/screen_8021.png` |
| `refs/pad-crop.png` | The space room's pad and stem, game x 296 to 360, y 140 to 196, enlarged on flat magenta | `python3 runway/mkcrop_pad.py` |
| `refs/bay-crop.png`, `refs/spacekeg-crop.png`, `refs/window-crop.png` | A bay, a keg and the left wall's window, cut the same way | The same as `mkcrop_bar.py` with another box. The boxes were not written down; `gp theme look cells` gives a room's cells to find them |
| `refs/stand-layout.png`, `refs/stand-layout2.png`, `refs/stands-layout.png` | The ballpark stand's tiers and bars as flat shapes | Not recorded. `bleachers.py` and `hood.py` hold the measures they were drawn to |
| `refs/pour-original.png`, `refs/drink-original.png` | The bartender at the tap, and a guest drinking, in the original's pictures | `gp theme look play ../last-call --game tapper --shots DIR --against original --moments FILE`, with FILE from `python3 runway/mkmoments.py FILE pour 1148-1184/2` (the pour of `po-moments.toml`'s play). The drink's frames were not written down; `pours.py` and `drink_timing.py` find them in a clip's sprite list |
| `refs/pour-theme.png` | The same pour with the theme's mug and tap | The left half of the pair the command above writes |
| The board's screens (`screen_8021.png` and the rest) | A whole room in the original's pictures | `gp theme look sheet --game tapper` writes them under `~/.gameprinter/work/theme-kits/tapper/board/` |
| A clip's sprite list | Every sprite a clip draws on every frame | `gp picture tapper --run RUN --clip CLIP --sprites FILE`: a line a frame, the original's sprites by code and box beside the model's |

Shots of the theme over the game are never committed either (`gp theme look play --shots` says so).
