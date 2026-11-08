//! Donkey Kong: Original Edition's 50m: the conveyors and the pies (the
//! lower half's $84C2-$8754, which the play routine's stage dispatch
//! $CEEB -> $836A runs on $53 = 2 after the fires), then the moving
//! ladders ($8789, before the touch tests).
//!
//! The conveyors' counters are the game's bytes:
//!
//! | Counter | Byte | What it is |
//! |---|---|---|
//! | `conv_beat` | $048B | The step of the 16-step pattern, 1 more each frame and back to 0 at 16 ($8755) |
//! | `conv_moving` | $0484 | 1 on a frame the conveyors move: the pattern's bit for the step, from the lifted pair of bytes by the difficulty d (`ext_tables.pattern`, $858E; $C1BC) |
//! | `conv_phase` | $0485 | One less each moving frame, from $1F, back to $1F after 0; its low three bits 0 ask for a turn test and a pie |
//! | `conv_turn` | $048A | One less at each such test; at 0 the lowest conveyor turns and it starts again at the lifted `turns` by d ($850E, $8588) |
//! | `conv_low` | $0483 | The lowest conveyor's way, 0 right and 1 left. The middle one's halves always run to the middle ($0481 = 0, $0482 = 1) |
//!
//! A pie ($861C) comes onto a conveyor's end when a test finds a free slot
//! of 7: a random byte ($19) and the lifted `row_masks` by d pick row 1 (the middle
//! conveyor's left half), 2 (its right half) or 3 (the lowest one), any
//! other value none, and no pie of that row may be within 16 pixels of the
//! end it comes from ($8692). Each moving frame every pie goes a pixel the
//! way its row runs ($86D3); one at x 0 or 248 goes, and on the middle
//! conveyor one at x 120 (into the oil drum). A pie's row follows from its
//! place: the lowest conveyor is at `row3_y`, and the middle one's halves
//! never cross x 120.
//!
//! Mario rides ($85A9): on a moving frame, walking or with the hammer
//! ($96 1 or 10) on the lowest conveyor's girder ($59 = 2) or the middle
//! one's ($59 = 4, its half by his sprite's x), his sprite goes a pixel the
//! way it runs while it is inside `ride_lo` to `ride_hi`.

use gp_sim::systems::LAND_HOLD_WORD;
use gp_sim::world::Body;
use gp_sim::{ExtCx, Fx};

use crate::p;

/// Row `i` of the lifted table `name` the model gives the hook
/// (`ext_tables`), 0 past it.
fn t(cx: &ExtCx<'_>, name: &str, i: i64) -> i64 {
    usize::try_from(i)
        .ok()
        .and_then(|i| cx.table(name).get(i).copied())
        .unwrap_or(0)
}

fn counter(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.counter(name).unwrap_or(0)
}

/// The conveyors, their pies and Mario's ride: one body that runs the
/// whole routine each frame of play.
pub(crate) struct Conveyor;

/// A pie's row from its place: 3 on the lowest conveyor, else the middle
/// one's half.
fn row(cx: &ExtCx<'_>, b: &Body) -> i64 {
    if b.y.floor() == p(cx, "row3_y") {
        3
    } else if b.x.floor() < p(cx, "middle") {
        1
    } else {
        2
    }
}

/// The way a row runs, 0 right and 1 left ($0480,X).
fn way(row: i64, low: i64) -> i64 {
    match row {
        1 => 0,
        2 => 1,
        _ => low,
    }
}

impl gp_sim::Ext for Conveyor {
    fn step(&self, cx: &mut ExtCx<'_>) {
        // $8956 -> $895F at the NMI's end, on every frame of 50m: the first
        // pie slot copied into the sprite table, one further a frame.
        if let Some(t) = cx.counter("sprite_turn") {
            cx.set_counter("sprite_turn", (t + 1) % 7);
        }
        if cx.system_stopped {
            death_wait(cx);
            smash_turn(cx);
            return;
        }
        let d = crate::difficulty(cx);
        // $8755: the pattern's step.
        let mut beat = counter(cx, "conv_beat") + 1;
        if !(0..16).contains(&beat) {
            beat = 0;
        }
        let pattern = t(cx, "pattern", 2 * d) | t(cx, "pattern", 2 * d + 1) << 8;
        let moving = pattern >> beat & 1;
        cx.set_counter("conv_beat", beat);
        cx.set_counter("conv_moving", moving);
        let pie = cx.kind("pie");
        let mut pies: Vec<(usize, Body)> = match pie {
            Some(k) => cx
                .world
                .bodies
                .iter()
                .enumerate()
                .filter(|(_, b)| b.alive && !b.waiting && b.kind == k)
                .map(|(i, b)| (i, *b))
                .collect(),
            None => Vec::new(),
        };
        // A pie made last frame takes the slot $861C found for it then.
        let new_slot = counter(cx, "pie_new_slot") + 1;
        for (_, b) in pies.iter_mut().filter(|(_, b)| b.ext[3] == 0) {
            b.ext[3] = new_slot;
        }
        let mut low = counter(cx, "conv_low");
        let mut spawned = None;
        if moving == 1 {
            let mut phase = counter(cx, "conv_phase");
            if phase & 7 == 0 {
                // $850E: the turn test, which writes the four end caps
                // ($F4C2) from the phase and the ways: from the stage's
                // first test the caps are these, not the stage string's
                // (`belt_drawn`, for presentation).
                if cx.counter("belt_drawn") == Some(0) {
                    cx.set_counter("belt_drawn", 1);
                }
                let mut turn = counter(cx, "conv_turn");
                if turn == 0 {
                    low = 1 - low;
                    turn = t(cx, "turns", d);
                }
                cx.set_counter("conv_turn", turn - 1);
                cx.set_counter("conv_low", low);
                // $861C: a pie, if a slot is free.
                if pies.len() < p(cx, "slots") as usize {
                    let r = i64::from(crate::draw(cx, "pie_row", 1)) & t(cx, "row_masks", d);
                    if (1..=3).contains(&r) {
                        let w = way(r, low);
                        let blocked = pies.iter().any(|(_, b)| {
                            row(cx, b) == r
                                && if w == 0 {
                                    b.x.floor() < p(cx, "start_gap_left")
                                } else {
                                    b.x.floor() >= p(cx, "start_gap_right")
                                }
                        });
                        if !blocked {
                            // The first free slot of the seven.
                            let held: Vec<i64> = pies.iter().map(|(_, b)| b.ext[3] - 1).collect();
                            let slot = (0..).find(|n| !held.contains(n)).unwrap_or(0);
                            cx.set_counter("pie_new_slot", slot);
                            let x = if w == 0 {
                                p(cx, "start_left")
                            } else {
                                p(cx, "start_right")
                            };
                            let y = if r == 3 {
                                p(cx, "row3_y")
                            } else {
                                p(cx, "row12_y")
                            };
                            spawned = Some((x, y, w));
                        }
                    }
                }
            }
            if phase == 0 {
                phase = 0x20;
            }
            cx.set_counter("conv_phase", phase - 1);
        }
        // $85A9: Mario rides.
        let rode = if moving == 1 { ride(cx, low) } else { 0 };
        // $86D3: the pies move, the new one too, and go at the ends.
        let middle = p(cx, "middle");
        // The pies where this frame leaves them, by slot, for the touch loop.
        let mut placed: Vec<Body> = Vec::new();
        for (i, mut b) in pies {
            let r = row(cx, &b);
            if moving == 1 {
                let dx = if way(r, low) == 1 { -1 } else { 1 };
                b.x = b.x.add(Fx::from_int(dx));
            }
            let x = b.x.floor();
            if x == 0 || x == p(cx, "end_right") || (r != 3 && x == middle) {
                cx.remove(i);
            } else {
                cx.set_body(i, b);
                placed.push(b);
            }
        }
        if let (Some(k), Some((x, y, w))) = (pie, spawned) {
            let x = if w == 1 { x - 1 } else { x + 1 };
            cx.spawn(k, Fx::from_int(x), Fx::from_int(y), w == 1);
            let mut b = Body {
                kind: k,
                x: Fx::from_int(x),
                y: Fx::from_int(y),
                alive: true,
                ..Body::default()
            };
            b.ext[3] = counter(cx, "pie_new_slot") + 1;
            placed.push(b);
        }
        ladders(cx);
        loop_slot(cx, &placed, rode);
        smash_test(cx, &placed);
    }
}

/// $5D (the counter `barrel_loop`) after 50m's loops: Kong's hook leaves
/// it at the touch loop's count, 7 ($893D, $8952). The touch loop ($EC44
/// over the pies' slots) stops at the first pie whose box meets Mario's,
/// which kills him, so through the death it holds that slot. With the
/// hammer up the hammer's loop over the nine sprite slots ($ED12-$ED31,
/// after the fires' $ED38) runs it on to `hammer_loop`, 9, when nothing
/// is hit. Mario's box is where the ride (`rode`, a pixel or none) left
/// him: the hook's own move of his body lands only after it returns
/// (diff-50m-d2 clip 1, original frame 10660: pie 2 at sprite x 30 met
/// his place before the ride and set $5D a frame early).
fn loop_slot(cx: &mut ExtCx<'_>, pies: &[Body], rode: i64) {
    let Some(now) = cx.counter("barrel_loop") else {
        return;
    };
    let Some(mut m) = crate::player(cx).copied() else {
        return;
    };
    m.x = m.x.add(Fx::from_int(rode));
    let rect = |b: &Body| {
        let h = cx.spec.kind(b).hitbox;
        let (x, y) = (b.x.floor() + h.x.floor(), b.y.floor() + h.y.floor());
        [x, y, x + h.w.floor() - 1, y + h.h.floor() - 1]
    };
    let [ml, mt, mr, mb] = rect(&m);
    let hit = pies
        .iter()
        .filter(|b| b.ext[3] > 0)
        .filter(|b| {
            let [bl, bt, br, bb] = rect(b);
            ml <= br && bl <= mr && mt <= bb && bt <= mb
        })
        .map(|b| b.ext[3] - 1)
        .min();
    if let Some(slot) = hit.filter(|s| *s < now) {
        cx.set_counter("barrel_loop", slot);
        return;
    }
    jumped(cx, &m, pies);
    let held = cx.kind("hammer").is_some_and(|k| {
        cx.world
            .bodies
            .iter()
            .any(|b| b.kind == k && b.alive && b.ext[0] == 1)
    });
    if held && cx.has_param("hammer_loop") && counter(cx, "smash_step") == 0 {
        cx.set_counter("barrel_loop", p(cx, "hammer_loop"));
    }
}

/// A pie's word 0 once the hammer has hit it.
const SMASHED: i64 = 1;

/// The hammer's test on 50m ($ECBF), after the touch test: its head (the
/// hammer's words, as the barrels read them) against each pie's box
/// ($ED12-$ED31, the slots by $5D, the box the hammer's `smash_*` from
/// the pie's place). The first that meets is smashed: `smash_step` ($BF)
/// 1, the smash's sound, its slot in $5D ($88B4 -> $ED69) and the event
/// `barrel.smashed` that stops play while it breaks. The fires' loop
/// before it ($ED38, with 50m's $ED74) is the fire hook's, which runs
/// after this one ([`fire_smashed`]).
fn smash_test(cx: &mut ExtCx<'_>, pies: &[Body]) {
    if counter(cx, "smash_step") != 0 {
        return;
    }
    let Some(hammer) = cx.kind("hammer") else {
        return;
    };
    let Some(h) = cx
        .world
        .bodies
        .iter()
        .find(|b| b.kind == hammer && b.alive && b.ext[0] == 1 && b.ext[3] > 0)
        .copied()
    else {
        return;
    };
    let ((x0, x1), (y0, y1)) = (crate::unpack(h.ext[1]), crate::unpack(h.ext[2]));
    let k = &cx.spec.kinds[hammer];
    let param = |name: &str| k.ext_params.get(name).map_or(0, |v| v.floor());
    let boxed = |b: &Body| {
        let (bx, by) = (b.x.floor(), b.y.floor());
        let body = [
            bx + param("smash_x0"),
            by + param("smash_y0"),
            bx + param("smash_x1"),
            by + param("smash_y1"),
        ];
        x1 >= body[0] && body[2] >= x0 && y1 >= body[1] && body[3] >= y0
    };
    let Some(event) = cx.event("barrel.smashed") else {
        return;
    };
    let mut order: Vec<&Body> = pies.iter().filter(|b| b.ext[3] > 0).collect();
    order.sort_by_key(|b| b.ext[3]);
    let Some(b) = order.into_iter().find(|b| boxed(b)) else {
        return;
    };
    let slot = b.ext[3];
    let Some(i) = cx
        .world
        .bodies
        .iter()
        .position(|w| w.alive && w.kind == b.kind && w.ext[3] == slot)
    else {
        return;
    };
    cx.set_counter("smash_step", 1);
    cx.set_counter("sound_ff", 2);
    cx.set_counter("barrel_loop", slot - 1);
    let mut w = *b;
    w.ext[0] = SMASHED;
    cx.set_body(i, w);
    cx.fire(event, Some(i));
}

/// Whether the pies' test ([`smash_test`]) smashed a pie this frame.
pub(crate) fn pie_smashed_now(cx: &ExtCx<'_>) -> bool {
    counter(cx, "smash_step") == 1
        && cx.counter_at_start("smash_step") == Some(0)
        && cx.kind("pie").is_some_and(|k| {
            cx.world
                .bodies
                .iter()
                .any(|b| b.alive && b.kind == k && b.ext[0] == SMASHED)
        })
}

/// A fire the hammer hit on 50m (the fire hook's `smash_test`, $ED57 ->
/// $ED74). The game tests the fires before the pies ($ED34, then $88A2 ->
/// $ED12 only when none was hit), but the model's fires move after this
/// hook, so a pie [`smash_test`] smashed this frame isn't, and $5D is what
/// the touch loop left, the pies' count `slots` (7), not the hammer's loop's
/// 9 (m50-hammer-smash clip 3, frame 4195: $5D 7, $AE 1).
pub(crate) fn fire_smashed(cx: &mut ExtCx<'_>) {
    if let Some(i) = cx.kind("pie").and_then(|k| {
        cx.world
            .bodies
            .iter()
            .position(|b| b.alive && b.kind == k && b.ext[0] == SMASHED)
    }) {
        let mut b = cx.world.bodies[i];
        b.ext[0] = 0;
        cx.set_body(i, b);
    }
    let Some(k) = cx.kind("conveyor") else {
        return;
    };
    if let Some(slots) = cx.spec.kinds[k].ext_params.get("slots")
        && cx.counter("barrel_loop").is_some()
    {
        cx.set_counter("barrel_loop", slots.floor());
    }
}

/// A frame of a smash on 50m ($EE0C, stopping everything else): the frame
/// counter of $5D's slot ($8A,X, the pie's `time`) steps, 0 to 15 ($DFE8
/// with $80 and $80); on 7 or 15 the smash steps, the first asking for
/// its sound again ($EE34); after its 11th step the pie goes with 500
/// points ($EE51-$EE69) and `smash.done`.
fn smash_turn(cx: &mut ExtCx<'_>) {
    let step = counter(cx, "smash_step");
    if step == 0 {
        return;
    }
    let Some(i) = cx.kind("pie").and_then(|k| {
        cx.world
            .bodies
            .iter()
            .position(|b| b.alive && b.kind == k && b.ext[0] == SMASHED)
    }) else {
        return;
    };
    let mut b = cx.world.bodies[i];
    b.time = (b.time + 1) % 16;
    cx.set_body(i, b);
    if b.time % 8 != 7 {
        return;
    }
    if step < 11 {
        if step == 1 {
            cx.set_counter("sound_ff", 2);
        }
        cx.set_counter("smash_step", step + 1);
        return;
    }
    cx.set_counter("smash_step", 0);
    crate::award_points(cx);
    cx.remove(i);
    if let Some(done) = cx.event("smash.done") {
        cx.fire(done, Some(i));
    }
}

/// The jump over a pie, the barrels' test ($EC44-$EC95 for each of the
/// seven slots, the pies' box at `box_x`, `box_y` from their place, as a
/// barrel's): Mario in a jump, the box's left less his ($9C) 0 straight
/// up, or below 3 across on a frame his toggle $9E is 0, and its top less
/// his ($9D) below `jump_below`. Each pie that passes scores 100 and asks
/// for song 5 (the event `barrel.jumped` at the pie): two pies under him
/// score twice (m50-pie-points clip 1, frames 4075 and 4076). Across, on
/// 50m, $8BF6 lets it score only while neither points sprite shows ($41,
/// $42); straight up it doesn't test them.
fn jumped(cx: &mut ExtCx<'_>, m: &Body, pies: &[Body]) {
    if m.on_ground || m.climbing || !cx.has_param("jump_below") {
        return;
    }
    let mut order: Vec<&Body> = pies.iter().filter(|b| b.ext[3] > 0).collect();
    order.sort_by_key(|b| b.ext[3]);
    let across = m.vx.raw() != 0;
    for b in order {
        let dx = (b.x.floor() + p(cx, "box_x") - m.x.floor()).rem_euclid(256);
        let dy = (b.y.floor() + p(cx, "box_y") - m.y.floor()).rem_euclid(256);
        let lined_up = if across {
            dx < 3 && m.ext[1] == 0
        } else {
            dx == 0
        };
        if !lined_up || dy >= p(cx, "jump_below") {
            continue;
        }
        let shows = |cx: &ExtCx<'_>, name: &str| {
            cx.counter_at_start(name).unwrap_or(0) > 0 || counter(cx, name) > 0
        };
        if across && (shows(cx, "points_timer") || shows(cx, "points_timer_2")) {
            continue;
        }
        crate::award_points(cx);
        if let Some(e) = cx.event("barrel.jumped") {
            let at = cx
                .world
                .bodies
                .iter()
                .position(|w| w.alive && w.kind == b.kind && w.ext[3] == b.ext[3]);
            cx.fire(e, at);
        }
        if cx.counter("sound_fd").is_some() {
            cx.set_counter("sound_fd", 0x20);
        }
    }
}

/// A death's wait on 50m: nothing moves, and the pies and the moving
/// ladders stay where they were until the 13th divider wrap moves every
/// sprite off (the game rule clears the pies). The ladders' bytes ($048E,
/// $0491) stay as the death left them until the restart ($CC47), so their
/// bodies wait (out of play and of the comparison) from that wrap, and on
/// the 21st ($CAE7 -> $CC34 -> $CCC1, 50m's list $82DE) come back at their
/// start, tile $94 at (20, 71) and (220, 71), the level's spawns. Only on
/// 50m's turn: after a game over hands the turn over, $53 is the other
/// player's stage.
fn death_wait(cx: &mut ExtCx<'_>) {
    if !cx.state_at_start("game", "dying") || cx.counter("stage") != Some(1) {
        return;
    }
    let w = cx.counter("death_wait");
    if w == cx.counter_at_start("death_wait") || !matches!(w, Some(19) | Some(11)) {
        return;
    }
    let Some(k) = cx.kind("mladder") else {
        return;
    };
    let mut places: Vec<(Fx, Fx)> = cx.spec.levels[cx.world.level]
        .spawns
        .iter()
        .filter(|s| s.kind == k)
        .map(|s| (s.x, s.y))
        .collect();
    places.sort_by_key(|(x, _)| x.floor());
    let mut ls: Vec<(usize, Body)> = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.alive && b.kind == k)
        .map(|(i, b)| (i, *b))
        .collect();
    ls.sort_by_key(|(_, b)| b.x.floor());
    for (n, (i, mut b)) in ls.into_iter().enumerate() {
        if w == Some(19) {
            b.waiting = true;
        } else if let Some(&(x, y)) = places.get(n) {
            b.waiting = false;
            b.x = x;
            b.y = y;
        }
        cx.set_body(i, b);
    }
}

/// A moving ladder's state ($048E,X): up and waiting, going down, down and
/// waiting, going up.
const UP: i64 = 0;
const GOING_DOWN: i64 = 1;
const DOWN: i64 = 2;
const GOING_UP: i64 = 3;

/// $8789, after the conveyors: the two ladders that move, `mladder`
/// bodies at their sprites' place, the left one first (ladder 0, its
/// random byte $19; the right one $1A). `ext[0]` is the state ($048E,X),
/// `ext[1]` the count ($0491,X); the counter `mladder_div` is $0490, 10
/// frames round. A count at 0 is loaded by the state ($87E9): up, a random
/// byte's low four bits and the lifted wait at the top by the difficulty
/// d; going down, `down_count`; down, the byte's low three bits and the
/// wait at the bottom; going up, `up_count`. A waiting ladder counts down
/// only when the divider is 0 ($8837, $8868). Going down it counts each
/// frame and its sprites go a pixel down on each count that is a multiple
/// of 8 ($884D); going up a pixel up on each multiple of 4 ($887E). At 0
/// it takes the next state.
fn ladders(cx: &mut ExtCx<'_>) {
    let Some(k) = cx.kind("mladder") else {
        return;
    };
    let mut div = counter(cx, "mladder_div");
    if div == 0 {
        div = p(cx, "ladder_div");
    }
    div -= 1;
    cx.set_counter("mladder_div", div);
    let mut ls: Vec<(usize, Body)> = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.alive && b.kind == k)
        .map(|(i, b)| (i, *b))
        .collect();
    ls.sort_by_key(|(_, b)| b.x.floor());
    ladder_cells(cx, &ls);
    let d = crate::difficulty(cx);
    for (n, (i, mut b)) in ls.into_iter().enumerate() {
        let byte = 1 + n as u32;
        let (mut state, mut count) = (b.ext[0], b.ext[1]);
        if count == 0 {
            count = match state {
                GOING_DOWN => p(cx, "down_count"),
                DOWN => {
                    (i64::from(crate::draw(cx, "mladder_down", byte)) & 7)
                        + t(cx, "ladder_waits", 5 + d)
                }
                GOING_UP => p(cx, "up_count"),
                _ => {
                    (i64::from(crate::draw(cx, "mladder_up", byte)) & 15) + t(cx, "ladder_waits", d)
                }
            };
        }
        let mut dy = 0;
        match state {
            GOING_DOWN => {
                count -= 1;
                if count == 0 {
                    state = DOWN;
                } else if count & 7 == 0 {
                    dy = 1;
                }
            }
            GOING_UP => {
                count -= 1;
                if count == 0 {
                    state = UP;
                } else if count & 3 == 0 {
                    dy = -1;
                }
            }
            _ => {
                if div == 0 {
                    count -= 1;
                    if count == 0 {
                        state = if state == DOWN { GOING_UP } else { GOING_DOWN };
                    }
                }
            }
        }
        b.ext[0] = state;
        b.ext[1] = count & 0xFF;
        b.y = b.y.add(Fx::from_int(dy));
        cx.set_body(i, b);
    }
}

/// The moving ladders' cells. The game's climb ($D37E-$D50A) reads no
/// background tiles: up or down it goes by the ladder lists and the tops'
/// boxes, so Mario climbs a moving ladder its whole height whatever its
/// state, but for $890F's stop. The screen draws only its lower part as
/// ladder tiles (y 88 to 103 on 50m); its upper part is sprites. So the
/// empty cells of the level's ladder box that holds each `mladder` body
/// (the last rows of `ladders_50m`, x 24 and 224 from y 64, 40 high) are
/// ladder cells for the model's climb, which goes by the tiles under the
/// feet (m50-mladder clip 1, frame 4132: the climb on past body y 75).
fn ladder_cells(cx: &mut ExtCx<'_>, ls: &[(usize, Body)]) {
    use gp_sim::spec::Tile;
    let level = &cx.spec.levels[cx.world.level];
    let ts = level.tile_size.max(1) as usize;
    let boxes: Vec<[i64; 4]> = ls
        .iter()
        .filter_map(|(_, b)| {
            let mid = b.x.floor() + p(cx, "ladder_mid");
            level
                .ladders
                .iter()
                .find(|l| l[0] <= mid && mid < l[2])
                .copied()
        })
        .collect();
    for l in boxes {
        for y in (l[1]..l[3]).step_by(ts) {
            for x in (l[0]..l[2]).step_by(ts) {
                if cx.tile_at(x, y) == Tile::Empty {
                    cx.set_tile(x, y, Tile::Ladder);
                }
            }
        }
    }
}

/// $85A9: Mario's sprite goes a pixel the way his conveyor runs. The
/// pixel it moved him, or 0.
fn ride(cx: &mut ExtCx<'_>, low: i64) -> i64 {
    let Some((i, mut m)) = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .find(|(_, b)| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
        .map(|(i, b)| (i, *b))
    else {
        return 0;
    };
    // $96 is 1 (the walk, standing too) or 10 (the hammer) only on the
    // ground, off a ladder and out of a jump's landing hold, and not once
    // he has died: a fall too far that lands on a conveyor ($D627 sets $96
    // to 255 before the ride runs) isn't carried (m50-girder3 clip 1,
    // frame 3383: x 64 as he lands on the lowest conveyor through girder
    // 3's gap).
    let dead = cx.spec.kinds[m.kind]
        .machine
        .as_ref()
        .is_some_and(|mm| m.state != mm.initial);
    if !m.on_ground || m.climbing || m.ext[LAND_HOLD_WORD] > 0 || dead {
        return 0;
    }
    // $59 from his sprite's y + 8 ($E016 over the lifted heights): 2 on
    // the lowest conveyor's girder, 4 on the middle's.
    let feet = m.y.floor() - p(cx, "sprite_dy") + 8;
    let floor = 1
        + (1..=5)
            .take_while(|n| feet < t(cx, "heights", n - 1))
            .count() as i64;
    let sx = m.x.floor() - p(cx, "sprite_dx");
    let w = match floor {
        2 => low,
        4 => i64::from(sx >= p(cx, "middle_sprite")),
        _ => return 0,
    };
    let dx = if w == 0 {
        if p(cx, "ride_hi") > sx { 1 } else { 0 }
    } else if p(cx, "ride_lo") < sx {
        -1
    } else {
        0
    };
    if dx == 0 {
        return 0;
    }
    m.x = m.x.add(Fx::from_int(dx));
    cx.set_body(i, m);
    // The dk_mario hook keeps Mario's x as the frame left it to tell a
    // move across of his own (the walk's beat and step sound); the ride
    // isn't one.
    let mario_hook = |b: &Body| cx.spec.kinds[b.kind].ext.as_deref() == Some("dk_mario");
    if let Some((j, mut h)) = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .find(|(_, b)| b.alive && mario_hook(b))
        .map(|(j, b)| (j, *b))
    {
        h.ext[0] += dx;
        cx.set_body(j, h);
    }
    // The hammer's test ($ECBF) puts the head by Mario's sprite as the
    // ride left it. A held hammer whose hook ran before this one this frame
    // put its head (its words 1 and 2) by his place before the ride, so it
    // goes with him (m50-hammer-smash clip 3, frame 4195: the head's left
    // at sprite x 172 + 4 meets a fire's box at 176).
    if let Some(k) = cx.kind("hammer")
        && let Some((j, mut h)) = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .find(|(j, b)| b.alive && b.kind == k && b.ext[0] == 1 && *j < cx.id)
            .map(|(j, b)| (j, *b))
    {
        let (x0, x1) = crate::unpack(h.ext[1]);
        h.ext[1] = crate::pack(x0 + dx, x1 + dx);
        cx.set_body(j, h);
    }
    dx
}
