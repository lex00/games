# Last Call: open issues

Each item is the owner's report, with its status and how it was checked. Done means checked
side by side with the original frame (`gp theme look play --shots --against original`).

## Mugs (redone from scratch: one table of every picture with a mug, below)

- [x] Old mug while he drinks (player 41, part 24), redone 2026-11-18: the sideways mug floated above his hand and the original's beer showed under it. Now our clear glass stands in the original glass's box (rows 2 to 20 of his picture, right side on the original's), held by its handle, his hand and arm in front; the original glass hidden whole (rows 0 to 8, its left wall to row 13, part 24). Checked every frame beside the original: level 1 1200 to 1232, level 2 1600 to 1632.
- [x] Old mug on the first frame of the pour (parts 02, 05): our glass from the first frame (checked every 2nd frame, 1148 to 1184).
- [x] Transparent right side of the glass during the pour: the beer fills the glass's inside (x 14 to 42).
- [x] Handle hole see-through on the mug pictures (mug-0e, 16, 17 and those made from them).
- [x] Pour glass: the original's glass hidden in its box (02, 05), ours drawn there; his hand and arm the game's; the tap's pipe brass.
- [ ] Mug-only pictures, all of them (mug, bonus_object, legend 17, customer 23, dd, f1, fb): our art, each checked.
- [ ] Bartender's mixed pictures (parts 02, 05, 24; player 41; check 01, 04): original glass hidden in a box, our mug drawn at the measured place.
- [x] Liquid floating in the air while he chugs: the beer in the drink is the board's bands (bonus object 15, 18 wide, 4 rows), which had no picture of ours, so the board's orange showed beside and below our mug. Each band is now our beer, only inside our glass and right of his hand, so the glass drains band by band with the original's. Checked every frame, level 1 and level 2.
- [ ] Levels 3 to 5 endings not seen: the clip replays (l3.001, l4.001, l5.001, l6.001, clear.004 to 007, shifted -94) never reach a level clear here (a lost life, or the attract screen). The drink uses the same pictures and offsets on every level, so the fix holds unless he drinks somewhere else; a route that clears level 3 is needed to see scripts 2 to 4 (end_script_lists), the double chug and the picture 59.
- (Later, with the character pass) Customers' drinking poses (about 25 pictures): the customers redrawn with our mug in hand. Until then they drink from the old mug (owner's yes, 2026-11-18).

## The bartender (every frame examined beside the original: walk, pour, send, catch, slide and loss, each celebration script)

- [ ] His lower half disappears after a life is lost.
- [ ] He runs too far through the opposite wall.
- [x] Running off a bar's left end into the wall: room_front draws the wall over him.
- [ ] Pull frame: arm and hand stay the game's; tap recoloured (03, 06). Checked at the pour; recheck after the mug plan.

## The celebration

- [x] Chugging: redone 2026-11-18 (the drink and the beer bands, under Mugs above).
- [ ] Double chug: the mug vanishes at the end of the second drink.

- [ ] A celebration with no kick and no broken mug that seemed to stop early: compare each end script with the original.
- [ ] A couple of frames of the mug throw switch back to the old mug: not seen in level 1's or level 2's throw (every frame, 2026-11-18); may be in the endings not reached (above).
- [x] The kicked mug in flight keeps one pose: its picture (bonus object 10, also mug 10) was our mug on its side, so the board's flips only turned it over. It is now our empty mug turned 45 degrees clockwise (handle up and left, rim up and right) as the board's glass is, and the board's flips tumble it. Checked every frame beside the original: level 1 1243 to 1262, level 2 1643 to 1691. The pieces (11 to 13) unchanged; 59 not shown in level 1 or 2's ending.
- [x] Level 1's celebration checked frame by frame (1196 to 1259): pour, drink and empty mug are ours.

## The bartender's sweep (2026-11-18)

Walk, pour, send, catch: level 1 run 1000 to 1195, every 2nd frame; the loss: die.001 1556 to 1824 (thrown down the bar, the wall at its end, the next life); the level 1 and 2 endings every frame. His own pictures are the board's and match the original in place and pose in every frame looked at. No slide was seen apart from the loss.

- [ ] The GET READY screen (and the first frames of a life): inside our glass under the tap the original glass's logo shows as a black shape. The pixels `#+ recolor` draws as nothing (9, 4, 2 in parts 02 and 05) come out black on that screen, where no room of ours is drawn behind, instead of the screen's navy. For the tools: a picture's pixels drawn as nothing should show the board's screen behind them.
- [ ] One frame late: the drink's last band (level 1 frame 1225) leaves a frame after the original's. Timing only.

## For the model

- The beer bands of the drink (bonus object 15) are drawn over the bartender on the page; the original draws them under him (his hand in front). The theme clips its bands to the right of his hand to hide it.

## Customers and dancers

- [x] Customers watching the dancers: the model gives them their record's +1B picture and the mirroring the original does ($11AA), checked beside the original (tapper.tip.002 frames 30 and 34); Tapper 0.1.8.
- [ ] Space bar, the dance (tapper.tip.002): an orange blob at the second bar's right end, and the blue customer on the third bar holding an orange mug, where the original has neither.

## The room

- [x] Walls bleed into the floor on both sides: room_mask lays the floor on the original's floor pixels in the door and cask cells (level 1 and the idle screen checked beside the original).
- [x] Floor boards every third one smooth; upper door leaf off the wall.
- [ ] Painting: less lean so the picture reads (1.6, 116 wide); check on screen.
- [ ] Root Beer (rbtapper) rooms: tiles differ from Budweiser's; needs rooms per romset.

## Done this round (checked)

- Black LAST CALL box over his hands (tools: also pictures loaded).
- Points over the faces on the legend screen (tools: text_center rows).
- Floating handles on the 2-bar idle screen (tools: unseen sprites).
- Stick: a pull pours a full glass and sends it.
- Vanishing mug on the second pour (model, Tapper 0.1.7).
