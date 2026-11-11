# Level kit: Donkey Kong: Original Edition (`donkey-kong-oe`)

Written by `gp level kit donkey-kong-oe` from the model, the asset map and `level-kit/rules.toml` (docs/level-kit.md). Edit the rules file, not this one. It holds rules, sizes, counts and boxes, never a table of the game's ROM. `.claude/skills/level-design/SKILL.md` says how to design a stage from it by hand.

A stage is one screen of 256 by 240 pixels: six sloped girders and a short top platform, joined by ladders, full and broken. The thrower (`kong`) on the top girder rolls barrels down; each rolls along its girder, drops off its end or down a ladder to the one below, and ends in the oil drum at the bottom left, which lets out fires that wander the girders and climb their own ladders. The hero starts on the bottom girder, can take one of two hammers, and clears the stage by climbing to the top platform where the one to rescue (`pauline`) waits. The rules below are for a stage that runs on the game's own engine, as 25m's replacement: the model runs any layout, but the engine reads the stage from fixed places in a fixed format, and some of the stage is numbers in its code.

## The levels

| Level | Size | Bounds | Goal |
|---|---|---|---|
| `m100`: 100m: flat floors narrowing up to Pauline's, ladders between them, eight rivets, three prizes and the fires | 32 by 30 cells of 8 px | the level is 256 by 240 px; the screen 256 by 240; a body below row 232 is out | set by the rules or a hook, not a place |
| `m50`: 50m: four conveyors, flat girders, ladders and two that move | 32 by 30 cells of 8 px | the level is 256 by 240 px; the screen 256 by 240; a body below row 232 is out | the hero's left edge at x 0 or past, with its top above row 53 |
| `m75`: 75m: flat girders in steps, ladders, two columns of elevators, springs across the top and two fires | 32 by 30 cells of 8 px | the level is 256 by 240 px; the screen 256 by 240; a body below row 232 is out | the hero's left edge at x 0 or past, with its top above row 29 |
| `main`: 25m: girders sloping a pixel every 24 px, full and broken ladders | 32 by 30 cells of 8 px | the level is 256 by 240 px; the screen 256 by 240; a body below row 232 is out | the hero's left edge at x 0 or past, with its top above row 29 |

What `m100` starts with (the model's spawns, places in pixels from the level's top left):

| Thing | Role | x | y |
|---|---|---|---|
| `player` | player | 61 | 204 |
| `hammer` | power-up | 20 | 110 |
| `hammer` | power-up | 124 | 70 |
| `kong_100m` | enemy | 32 | 32 |
| `pauline` | npc | 80 | 25 |

What `m50` starts with (the model's spawns, places in pixels from the level's top left):

| Thing | Role | x | y |
|---|---|---|---|
| `player` | player | 61 | 204 |
| `hammer` | power-up | 124 | 150 |
| `hammer` | power-up | 20 | 110 |
| `kong_50m` | enemy | 32 | 32 |
| `pauline` | npc | 80 | 25 |
| `oil_drum` | hazard | 120 | 96 |
| `belt_ends` | npc | 8 | 104 |
| `mladder` | npc | 20 | 71 |
| `mladder` | npc | 220 | 71 |

What `m75` starts with (the model's spawns, places in pixels from the level's top left):

| Thing | Role | x | y |
|---|---|---|---|
| `player` | player | 21 | 188 |
| `kong_75m` | enemy | 32 | 32 |
| `pauline` | npc | 80 | 25 |
| `elevator` | npc | 48 | 120 |
| `elevator` | npc | 48 | 168 |
| `elevator` | npc | 112 | 112 |
| `elevator` | npc | 112 | 160 |

What `main` starts with (the model's spawns, places in pixels from the level's top left):

| Thing | Role | x | y |
|---|---|---|---|
| `player` | player | 53 | 204 |
| `hammer` | power-up | 32 | 127 |
| `hammer` | power-up | 32 | 70 |
| `kong` | enemy | 32 | 32 |
| `oil_drum` | hazard | 32 | 192 |
| `pauline` | npc | 80 | 25 |

## What's data and what's fixed in code

A stage changes what is data. What the code fixes, a stage keeps to: change it and the stage no longer runs on the game's own engine.

| | What | Rule | Slot | Source |
|---|---|---|---|---|
| data | Girders | Runs of 6 bytes the walk's ground test reads ($D91A): x, y, the piece's shape, the step across (signed), the count of 24 px pieces, each a pixel higher than the last, and an end mark (0, or $FE to end the row) | `ground_runs` | the asset map's note; packP's pack, 2026-11-04 |
| data | Girder pieces' shapes | A box by shape, 4 bytes: left, top, right and bottom from the piece's place. 25m has 3 (a long flat piece, a 24 px piece and the top platform's) | `ground_shapes` | the asset map |
| data | Ladders | 3 bytes a ladder: x, y and a shape; each shape is a box of 4 bytes, left, top, right and bottom from the ladder's place. The list ends with $FE. A broken ladder is one whose box stops short of the girder above | `ladders` | the asset map; the ladder routine $D28B, $D8AD tests the hero's feet box against each |
| data | Ladder tops and the climb's pictures | The tops the climb finds (up to 9, then an end mark), the top zone's box, the 16 pictures of a climb over a top and the 6 of a step | `climb_rom` | the model's climb_rom slot ($C127) |
| data | Girder heights | The rows that say which girder a body is on (7 thresholds), read by the fires and the edge test | `play_heights` | the model's play_heights slot ($C0BC) |
| data | Gaps under the girders' ends | The places a walk off a girder's end drops through, and their boxes | `play_gaps` | the model's play_gaps slot ($C0CF, $C0DF) |
| data | The slope list | 9 x places where a girder steps a pixel, shared by barrels and fires: a barrel on girders 2 to 5 steps at each, on girders 1 and 6 from the fifth on; a fire steps at 24 (in code) and at the list from its second. Each is a piece's step less 8, the middle of a 16 px sprite | `play_tables` | $C1C4; gcFit's notes (2026-11-04) |
| data | Barrels' ladders | Two ladders a girder for girders 2 to 6, each an x and a foot row (20 bytes): the ladders a barrel may take down | `play_tables` | $C172 to $C185; packP's pack |
| data | Broken ladders' stops | A box at the top of each broken ladder that stops the hero's climb: x, y, 0, ended by $FE | `play_tables` | $C161; packP's pack |
| data | Straight drops | Three ways a barrel can drop straight down the stage, each 5 rows where it stops | `play_tables` | three tables after the slope list; the barrel's straight*_stop params |
| data | Hammers and the hero's start | Two hammers, an x and a y each; the hero's start as its sprite's x (the model's x less 5) and y | `play_tables_c2f0` | $C4CB, $C4D1, $C4E3; packP's pack |
| data | The stage's picture | PPU update strings that draw the girders, ladders and drum, with 25m's palettes: what the screen shows. Play never reads it, so it must agree with the girders and ladders above | `stage_screens` | the asset map: entry screen_25m |
| data | The attract demo | The demo's moves: up to 20 holds, a way and a frame count each | `demo_script` | the model's demo_script slot |
| code | Rows of runs | The ground test reads row 1 as two runs at the list's start and row n (2 to 7) as one run at byte 12 + 6(n - 2): the bottom girder has exactly 2 runs and every other row exactly 1, 8 runs in all |  | $D91A; packP's pack, gcFit's notes |
| code | Sloped ends | A run is one straight slope of 24 px pieces, each a pixel higher than the last in its direction, so a girder above the bottom slopes from end to end; a flat part needs a run of its own, which only the bottom row has (25m's: flat to x 128, then sloped) |  | gcFit's notes |
| code | Fires' ladders | Fires climb 4 ladders whose x, top and foot are numbers in the fire routine's code ($E626, $E640 to $E697): two from the bottom girder (x 92 and 196) and two from the second (x 44 and 108), the model's fire ladder_* params. A stage keeps ladders where fires climb, or the code changes |  | the model's note on entities.fire; packP's pack patched these as code |
| code | The third barrel ladder | Barrels take a third ladder only on girders 4 and 5, its x and foot in code ($E0D1, $E0DD; the model's barrel ladder_c4 and ladder_c5, foot 14 over girders 3 and 4) |  | packP's pack, gcFit's notes |
| code | The throw | The thrower lets a barrel go at x 77, y 50 (the barrel's let_go_x and let_go_y), so girder 6 lies 64 px under the thrower and steps a pixel only from the slope list's fifth place on |  | the model's barrel params; gcFit's notes |
| code | A pushed barrel's drop | A barrel the hammer holder's crowd pushes through girder 3 or 5 drops 21 px and 2 more each 24 across, so girders 2 and 4 must lie under 3 and 5 as 25m's do |  | the barrel's crowd and crowd_drop params; gcFit's notes |
| code | The drum and the fires' way out | The drum's flames are at x 32, y 192 and a fire comes out at x 32, y 184 (the drum's out_x and out_y): the bottom girder's left end holds the drum |  | the model's oil_drum spawn and params |
| code | Screen edges | The hero's walk stops at x 21 and 229 ($D990); barrels fall at x 19 and 219 and bounce on to x 31 when they land on an even girder; fires turn at x 12 and 212 (228 on the bottom girder) |  | the hammer's left_edge and right_edge, the barrel's edge_left and edge_right, the fire's edge params |
| code | The goal and the top | The stage is cleared when the hero's top is above row 29 (the model's goal under); only standing on the top platform does that: the hero stands 12 rows under its top, so the platform must stand at row 40 or above, and a full ladder must reach it from girder 6 |  | the model's levels.main.goal |
| code | The thrower's place | The thrower stands at x 32, y 32, where the throw's numbers (above) assume him; his six poses are fixed pictures of 24 tiles each |  | the kong spawn; the kong_* slots |

## Limits

| What | Limit | Why | Source |
|---|---|---|---|
| Girder runs | 8 runs, 48 bytes: 2 on the bottom row, 1 on each of 6 rows above | the row rule; 9 runs spill 6 bytes into the shapes | the asset map (ground_runs_25m: 8 rows of 6); packP's pack |
| Girder shapes | 3 shapes, 12 bytes | a fourth must move elsewhere with its pointer (packP moved heights and shapes into 100m's ladder list) | the asset map; packP's pack |
| Ladders | 13 ladders (39 bytes) and an end mark; 7 shapes (28 bytes) | the list is followed by other tables | the asset map (ladders_25m: 13 rows of 3; ladder_shapes_25m: 7 of 4) |
| Broken ladders | 4 | the stop boxes' list is 13 bytes | $C161; packP's pack |
| Ladder tops | 9 | the climb's top list | climb_rom |
| Barrel ladders | 2 on each of girders 2 to 6, a third on girders 4 and 5 only | the table and the code | packP's pack |
| Fire ladders | 4, at x 92 and 196 from the bottom girder and 44 and 108 from the second, unless the code changes |  | the fire's params |
| Hammers | 2 |  | $C4CB, $C4D1 |
| The screen | 256 by 240 px; girders between x 0 and 256, the hero's walk from x 21 to 229; a body below row 232 is out |  | the model |
| Space for the stage's picture | 1258 bytes hold all five screens: 25m 449, 75m 177, 100m 268, the title 322, the status box 42. A new 25m with the others kept has 449 bytes; with 75m and 100m dropped, 894 |  | the asset map (stage_screens: 1258 bytes, 5 entries); packP's pack, 2026-11-04 |
| Tiles | 256 background tiles and 256 sprite tiles, 8 KB of CHR in all |  | the asset map |

The data slots the levels read, as the asset map holds them:

| Slot | Kind | Format | Size | What |
|---|---|---|---|---|
| `climb_rom` | table | table | 56 rows of 1 column | 25m's ladder tops and the climb's pictures, from $C127: the top zones, their box and the pictures by $5B and $5C |
| `demo_script` | table | table | 20 rows of 2 columns | The attract demo's script: 20 entries of a direction and its frames ($C014, $C028) |
| `ground_runs` | rows | byte-rows | 8 rows of 6 bytes (48 bytes) | 25m's girders as runs of one-row pieces: x, y, shape, the step across and the count, each piece a pixel higher |
| `ground_runs_100m` | rows | byte-rows | 5 rows of 3 bytes (15 bytes) | 100m's ground as $D2E2 reads it on any stage but 25m: rows of x, y and a shape, each one box, through the pointer at $C46B ($C2F0) |
| `ground_runs_50m` | rows | byte-rows | 7 rows of 3 bytes (21 bytes) | 50m's ground as $D2E2 reads it on any stage but 25m: rows of x, y and a shape, each one box, through the pointer at $C46B ($8260) |
| `ground_runs_75m` | rows | byte-rows | 21 rows of 3 bytes (63 bytes) | 75m's ground as $D2E2 reads it on any stage but 25m: rows of x, y and a shape, each one box, through the pointer at $C46B ($C20C) |
| `ground_shapes` | rows | byte-rows | 3 rows of 4 bytes (12 bytes) | The girder pieces' boxes by shape: left, top, right and bottom from the piece's place |
| `ground_shapes_100m` | rows | byte-rows | 4 rows of 4 bytes (16 bytes) | 100m's ground boxes by shape through the pointer at $C473: left, top, right and bottom from the row's place ($C306) |
| `ground_shapes_50m` | rows | byte-rows | 4 rows of 4 bytes (16 bytes) | 50m's ground boxes by shape: left, top, right and bottom from the row's place ($8276) |
| `ground_shapes_75m` | rows | byte-rows | 7 rows of 4 bytes (28 bytes) | 75m's ground boxes by shape: left, top, right and bottom from the row's place ($C252) |
| `ladder_shapes` | rows | byte-rows | 7 rows of 4 bytes (28 bytes) | The ladders' boxes by shape: left, top, right and bottom from the ladder's place |
| `ladder_shapes_100m` | rows | byte-rows | 2 rows of 4 bytes (8 bytes) | 100m's ladders' boxes by shape (the pointer at $C483, $C341) |
| `ladder_shapes_50m` | rows | byte-rows | 2 rows of 4 bytes (8 bytes) | 50m's ladders' boxes by shape: left, top, right and bottom from the ladder's place |
| `ladder_shapes_75m` | rows | byte-rows | 6 rows of 4 bytes (24 bytes) | 75m's ladders' boxes by shape ($C28D) |
| `ladder_tops_100m` | rows | byte-rows | 14 rows of 3 bytes (42 bytes) | 100m's whole ladders' tops as $D50A reads them (the pointer at $C489 + $53 * 2): rows of x, y and a shape, with 25m's box at $C143 ($C349) |
| `ladder_tops_75m` | rows | byte-rows | 10 rows of 3 bytes (30 bytes) | 75m's whole ladders' tops as $D50A reads them: x, y and a shape each ($C2A5) |
| `ladders` | rows | byte-rows | 13 rows of 3 bytes (39 bytes) | 25m's ladders: x, y and a shape each |
| `ladders_100m` | rows | byte-rows | 14 rows of 3 bytes (42 bytes) | 100m's ladders as $D28B reads them (the pointer at $C47B): rows of x, y and a shape ($C316) |
| `ladders_50m` | rows | byte-rows | 12 rows of 3 bytes (36 bytes) | 50m's ladders: x, y and a shape each, the last two the ladders that move, at their top |
| `ladders_75m` | rows | byte-rows | 10 rows of 3 bytes (30 bytes) | 75m's ladders: x, y and a shape each ($C26E) |
| `palettes_50m` | palettes | palette | 8 palettes of 4 colors | 50m's palettes, the first update of screen_50m |
| `play_gaps` | table | table | 20 rows of 1 column | The play routine's tables: 25m's gaps under the girders' ends ($C0CF) and their boxes ($C0DF), $D36A's list, which the dk_idle hook's cost of the play routine r |
| `play_heights` | table | table | 7 rows of 1 column | The play routine's tables: 25m's girders' heights by row ($C0BC), $E016's list, which the dk_idle hook's cost of the play routine reads by CPU address (ext/src/ |
| `play_tables` | table | table | 173 rows of 1 column | The play routine's tables from $C15F to $C20B: Mario's boxes, the swing's pictures, the barrels' ladder lists, the slope list and the straight drops' stops, whi |
| `play_tables_c2f0` | table | table | 286 rows of 1 column | The play routine's tables from $C3E2 to $C4FF (the name keeps the first address from before 100m's were split off): the fires' girder ends, the barrels' bounce  |
| `stage_screens` | level-objects | ppu-strings | 1258 bytes in all, 5 entries share it | The stages as PPU update strings, which the draw routine at $F1F0 walks |
| `stage_screens_50m` | level-objects | ppu-strings | 281 bytes in all, 1 entries share it | 50m's screen as PPU update strings, string 2 at $8147 in the lower half |

## The hero and the things in a stage

From the model. A gap the hero must jump, a ladder it must reach and a fall it must survive are measured against these.

| Move | The model's numbers |
|---|---|
| climb | 0.3333 px a frame, about 90 frames for 30 px |
| fall | at most 4 px a frame |
| fatal drop | a fall of more than 16 px kills |
| jump | up at 1.3789 px a frame, gravity 0.06641 px a frame each frame: about 14 px high and 42 frames in the air on flat ground (v*v/2g and 2v/g) |
| jump across | about 28 px walking |
| walk | 0.6667 px a frame, 40 px a second at 60 frames |

| Thing | Role | Size | Touch box (x, y, w, h from its place) | A touch |
|---|---|---|---|---|
| `barrel` | enemy | 16 by 16 | 2, 10, 14, 4 | kill-player |
| `fire` | enemy | 16 by 16 | 2, 10, 14, 4 | kill-player |
| `hammer` | power-up | 8 by 16 | 0, 0, 8, 8 | a hook decides |
| `oil_drum` | hazard | 16 by 8 | -2, 6, 22, 3 | kill-player |
| `pie` | enemy | 16 by 16 | 2, 10, 14, 4 | kill-player |
| `player` | player | 16 by 16 | 3, 0, 1, 12 | the hero's box |
| `spring` | enemy | 16 by 16 | 2, 10, 14, 4 | kill-player |

The touch boxes are the model's, as its touch tests read them. The theme kit (`themes/kit/kit.toml`, each slot's `geometry`) gives each in its picture's pixels, as the screen shows it.

## Play measures to land near

`gp level measure --template T` prints each under its name. A stage of this kind should land within the tolerance of each target.

| Measure | What | Target | Within | Source |
|---|---|---|---|---|
| `girders` | Girders, and the top platform | 6 + 1 | 1 | docs/originals.md |
| `slope_px_per_24` | Slope, pixels per 24 across, bottom to top | -0.53, 0.88, -0.88, 0.88, -0.88, 0.83 | 0.25 each | docs/originals.md |
| `gaps` | Gaps between girders' middles, px | 31, 30, 30, 30, 29, 25 | 8 px each | docs/originals.md |
| `ladders, broken` | Ladders, and how many are broken | 13, 4 (0.31) | 1 each | docs/originals.md |
| `things` | Hammers' heights over the bottom girder; their distance to the nearest ladder | 72, 129; 16, 16 | 8 px | docs/originals.md |
| `climb_frames_min, climb_frames_median` | A climb, frames (fastest, median) | 68, 71 | not set: it follows the gaps | docs/originals.md |
| `route_frames` | The direct route to the top, frames | 1160 | 10% | docs/originals.md |
| `leave_by_ladder_share` | Barrels leaving a girder by a ladder | 0.48 | 0.1 | docs/originals.md |
| `throw_to_drum_median` | From the throw to the drum, median frames | 1003 | 10% | docs/originals.md |
| `jump_window_median` | The jump window over a barrel, median frames | 22 | 3 frames | docs/originals.md |
| `goal_height` | The goal over the bottom girder's middle, px | 186 | not set | docs/originals.md |

## Originality

`gp template check T --original` compares a stage derived from this game with the original's, lifted in memory from your ROM. It is refused at cells 0.6 or ladders 0.5. What counts as copying:

- the layout: 60% or more of the cells either level fills (ground, ladder) the same kind in both, at the best shift of up to 4 cells each way, mirrored or not
- the ladders: 50% or more of them within a tile across of one of the original's, between the same floors counted from the bottom, at that shift
- the data: any line of 24 characters or more of the stage's built files that the original's lifted files hold too (gp template served's rule)
- tracing: a stage drawn over the original's picture, or laid out from a guide made from the ROM, is a copy whatever it scores
- the art: a theme's sprites and pictures are held to the theme kit's originality check (docs/theme-kit.md, "The originality check")

## How to verify

| Step | How |
|---|---|
| 1. The model's own checks | Write the stage as data in a derived template's authored.toml, then gp template derive T --write, gp template build T --write and gp template check T: every slot filled, the pins current |
| 2. The play measures | gp level measure --template T: each measure above within its tolerance. --clears 3 plays the route with the barrels and fires in and prints the holds a baseline takes |
| 3. Originality | gp template check T --original with your ROM in $GP_ROM_DIR: cells and ladders under the thresholds, no shared data line |
| 4. Baselines | Write the holds from step 2 as a model baseline (docs/originals.md, "Model baselines") and run gp interp baselines T |
| 5. The real engine | Pack the stage's slots into a copy of your ROM at 25m's places in 25m's formats, outside the repo (packP's script did it by hand; gp assets pack writes slots that keep their sizes), then run the same inputs on the model and on the copy: gp interp compare T --rom COPY --input LOG --assets LIFT --mode free. A slot that doesn't fit its place, or a rule fixed in code, is where a run first splits from the model where Donkey Kong's own doesn't. Never commit the copy |
| 6. A theme | gp theme check T --theme DIR: a theme's floors and sprites must keep the stage's geometry (docs/theme-kit.md, "Gameplay geometry") |

## What fits on the real engine

| What | Finding | Source |
|---|---|---|
| Shapes and heights | A fourth ground shape has no room in 25m's 3: packP moved the heights and shapes into 100m's ladder list and pointed 25m at them, which breaks 100m | packP's pack |
| Ladders, tops, stop boxes, barrel ladders, slope list, hammers, start, demo | All fit 25m's places and formats as data: 13 ladders and 7 shapes, 9 tops, 4 stop boxes, 2 barrel ladders a girder, 9 slope places, 2 hammers | packP's pack |
| Fires' ladders and the third barrel ladder | They move only by changing code: packP rewrote 12 numbers of the fire routine and 4 of the barrels' third-ladder test, and swapped the girder 3 and 5 branches. A stage that keeps them where the code has them needs no code changes | packP's pack; gcFit's notes |

## Open

- The stage picture's bytes are counted by hand (packP); a tool that encodes a layout entry and reports the space left would replace it
- The rules fixed in code were read from the model's notes and packP's and gcFit's work, not from a detector; docs/research/deterministic-inference.md lists a code-immediate finder as the tool that would find them
- Whether the attract demo's script must stay within 20 holds when packed (its table's size) is not checked
- The 75m and 100m stages have no kit: the model holds only 25m
