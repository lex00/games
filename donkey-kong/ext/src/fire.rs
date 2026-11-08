//! The oil drum and its fires on 25m (the fire code at $E1E5-$E833, the
//! drum at $E19A and the timers at $F4AC).
//!
//! The drum's counters are the game's bytes: `drum` ($AD: 0 out, 1 a barrel
//! has just reached it, 2 lit, 3 burning), `drum_timer` ($38), `fire_wait`
//! ($40, a slow timer, stepped on the frames the model's `divider`, the
//! game's 11-frame divider $34, wraps to 0) and `fire_aim` ($3B).
//!
//! A fire's words: `ext[0]` its state ($AF) plus one, so a body just
//! spawned reads 0; `ext[1]` its move counter ($E4, 0 to 15); `ext[2]`
//! the game's bytes $E8 (its ladder pause), $E0 (its girder), $D2 (whether
//! it has aimed), $EC (the way it aimed: 1 right, 2 left), $B3 (the way
//! it last chose to walk, which its bob is drawn facing) and $DB (the y
//! its climb ends at off 25m), a byte each from the lowest, as a state
//! mapping reads them; `ext[3]` its slot ($AE) plus one, the lowest free
//! one when it comes out of the drum.
//!
//! The picture a fire shows is its first sprite's tile ($0211 + 16 a
//! slot), which the counter `fire_tile_<slot>` keeps: `tile_a` or
//! `tile_b`, plus `tile_flip` when it is drawn mirrored. The way out of
//! the drum keeps it; each step of a walk ($E3ED) or on a ladder ($E50C)
//! turns it to the other picture, and each step of a bob ($E325) to
//! `tile_b`, or to `tile_a` from an unmirrored `tile_b`.

//!
//! The same code runs every stage, choosing by the stage $53 where the
//! stages differ (the table below). The hooks take the stage from the
//! parameter `stage_<level>` of the fire entity for the level being played
//! (`stage_m50 = 2`, `stage_m75 = 3`, `stage_m100 = 4`), else 1, so a
//! stage the stages list's stand-in plays (25m in 75m's place) runs 25m's
//! fires. A parameter `<name>_<level>` replaces `<name>` on that level
//! ([`kp`]). The tables are read by their CPU address in the lifted tables
//! ([`crate::rom_at`]), as the code reads them by $53.
//!
//! | Part | 25m ($53 = 1) | 50m (2, Original Edition's hooks) | 75m (3) | 100m (4) |
//! |---|---|---|---|---|
//! | Slots | `fires` | C1F6[$53 - 1] ($E22F) | the same | the same |
//! | A new fire ($E1E5) | lit drum, wait `fire_wait_frames` ($E200) | wait C466[d], no lit test ($83E6 -> $E213) | none: the setup places them ($E1FD) | as 50m ($E213) |
//! | Where it starts ($E538) | `out_x`, `out_y`, state 8 ($E553) | the drum's `out_x_m50`, `out_y_m50` ($83F0 -> $83FE) | state 0 where the setup put it | state 0 at C3CE[2 r + side], r = $19 AND 3 (draw site `fire_spawn`), side 8 while Mario's sprite x < `spawn_side` ($E564) |
//! | Out of the drum ($E59F) | `rise_x`, `floor_y` ($E5B4) | `rise_x_m50`, `floor_y_m50` ($840B -> $8419) | nothing moves | nothing moves |
//! | Its girder ($E016) | C0BC's heights | the list at C493[2 ($53 - 1)] | the same | the same |
//! | Its ends ($E6A5) | slope steps, C3E6 / C3E8 ($E6C6) | `edges_at_m50` by the way, and two more on girder `edge_girder_m50` ($8489) | C3EA / C3EC on girder `ends_girder`, `turn_x` going left on `turn_girder`, else C3EE / C3F0 ($E702) | C3F2 by the way, `end_step` in a girder up ($E73C), and the gaps where rivets were ($E770) |
//! | A choice | $E2B6 (draw site `fire_way`) | $E2B6 | $19 + slot AND 3 ($E278, draw site `fire_way_75m`) | $E2B6 |
//! | A ladder | `ladder_*` by girder ($E626) | the lists at C49B / C4A1 [2 ($53 - 2)], the target in $DB ($E7A3) | the same | the same, the faster gates C3F8 from d = 3 ($E436) |
//! | Its pictures | `tile_a`, `tile_b` | the same | the same | `tile_a_m100`, `tile_b_m100` ($E330) |
//! | The aim's reset ($E237) | yes | yes | no ($E23B) | yes |

use gp_sim::{ExtCx, Fx};

use crate::p;

/// The girders' heights by row ($C0BC) and the slopes' steps ($C1C5), by
/// CPU address in the lifted tables.
const GIRDER_HEIGHTS: u16 = 0xC0BC;
const SLOPES: u16 = 0xC1C5;

/// The tables the fire code reads by the stage $53 or the difficulty, by
/// CPU address: the slots ($E22F), the waits by d ($E21A), the heights'
/// lists ($E01F), the ladders' index and entries ($E7B5, $E7D6), the climb
/// gates ($E429, $E441), 100m's starting points ($E57D), its ends ($E744)
/// and gaps ($E789), and 75m's ends ($E70C-$E728).
const COUNTS: u16 = 0xC1F6;
const WAITS: u16 = 0xC466;
const HEIGHT_LISTS: u16 = 0xC493;
const LADDER_INDEX: u16 = 0xC49B;
const LADDER_LISTS: u16 = 0xC4A1;
const CLIMB_GATES: u16 = 0xC3F4;
const CLIMB_GATES_FAST: u16 = 0xC3F8;
const SPAWN_POINTS: u16 = 0xC3CE;
const ENDS_100M: u16 = 0xC3F2;
const GAPS_100M: u16 = 0xC3DE;
const ENDS_75M: [u16; 4] = [0xC3EA, 0xC3EC, 0xC3EE, 0xC3F0];
/// The smash's pictures by step ($EE3F).
const SMASH_TILES: u16 = 0xC1EC;

/// The name of the level being played.
fn level_name<'a>(cx: &'a ExtCx<'_>) -> Option<&'a str> {
    cx.spec.levels.get(cx.world.level).map(|l| l.name.as_str())
}

/// Entity kind `kind`'s parameter `name`, as `<name>_<level>` when it has
/// one for the level being played, else `<name>`, else 0.
pub(crate) fn kp(cx: &ExtCx<'_>, kind: usize, name: &str) -> i64 {
    let params = &cx.spec.kinds[kind].ext_params;
    level_name(cx)
        .and_then(|l| params.get(&format!("{name}_{l}")))
        .or_else(|| params.get(name))
        .map_or(0, |v| v.floor())
}

/// The game's stage $53 for the fires of kind `fire`: its parameter
/// `stage_<level>`, else 1.
pub(crate) fn stage(cx: &ExtCx<'_>, fire: usize) -> i64 {
    let params = &cx.spec.kinds[fire].ext_params;
    level_name(cx)
        .and_then(|l| params.get(&format!("stage_{l}")))
        .map_or(1, |v| v.floor())
}

const BOB: i64 = 0x00;
const BOB_DOWN: i64 = 0x10;
const BOB_END: i64 = 0x20;
const RIGHT: i64 = 0x01;
const LEFT: i64 = 0x02;
const LADDER: i64 = 0x03;
const DOWN: i64 = 0x13;
const OUT: i64 = 0x08;
const CHOOSE: i64 = 0xFF;

/// The stage rules over the tables: what the fire code does off 25m by
/// $53, reading the lifted tables by CPU address (`rom`) and the fire's
/// parameters by name, keyed by the level (`param`). The hooks build one
/// from their context ([`Stage::of`]); tests from made-up tables.
pub(crate) struct Stage<'a> {
    pub stage: i64,
    pub rom: &'a dyn Fn(i64) -> i64,
    pub param: &'a dyn Fn(&str) -> i64,
}

impl Stage<'_> {
    fn byte(&self, at: i64) -> i64 {
        (self.rom)(at & 0xFFFF)
    }

    /// The little-endian word at `at`.
    fn word(&self, at: i64) -> i64 {
        self.byte(at) | self.byte(at + 1) << 8
    }

    /// The fire slots ($E22F: C1F6[$53 - 1]).
    pub fn slots(&self) -> i64 {
        self.byte(i64::from(COUNTS) + self.stage - 1)
    }

    /// The wait before the next fire at difficulty `d` ($E21A: C466[d]).
    pub fn wait(&self, d: i64) -> i64 {
        self.byte(i64::from(WAITS) + d)
    }

    /// Where a fire starts on 100m ($E564): the side away from Mario (the
    /// points from 8 on while his sprite's x is under `spawn_side`) and a
    /// girder by the random byte `r` AND 3, from the points C3CE.
    pub fn spawn_point(&self, mario_sprite_x: i64, r: i64) -> (i64, i64) {
        let side = if mario_sprite_x < (self.param)("spawn_side") {
            8
        } else {
            0
        };
        let at = i64::from(SPAWN_POINTS) + (r & 3) * 2 + side;
        (self.byte(at), self.byte(at + 1))
    }

    /// The girder under a row ($E016): the list C493 points at for the
    /// stage, 1 at the bottom, 7 at its end mark $FF.
    pub fn girder(&self, row: i64) -> i64 {
        let list = self.word(i64::from(HEIGHT_LISTS) + (self.stage - 1) * 2);
        for k in 0..16 {
            let v = self.byte(list + k);
            if v == 0xFF {
                return 7;
            }
            if v <= row {
                return k + 1;
            }
        }
        7
    }

    /// Whether a walk stops at `nx` ($E6A5's ends by the stage), from `x`,
    /// going `left`, on girder `g`. `flag` is the zero-page byte the gap
    /// test reads ([`zero_page_flag`]).
    pub fn at_end(&self, g: i64, left: bool, x: i64, nx: i64, flag: &dyn Fn(i64) -> i64) -> bool {
        let way = i64::from(left);
        let t = |at: i64| self.byte(at + way);
        let param = self.param;
        match self.stage {
            // $8489: the girders' ends by the way, and two more pairs on
            // one girder.
            2 => {
                let at = param("edges_at");
                nx == t(at) || (g == param("edge_girder") && (nx == t(at + 2) || nx == t(at + 4)))
            }
            // $E702.
            3 => {
                let ends = ENDS_75M.map(i64::from);
                if g == param("ends_girder") {
                    nx == t(ends[0]) || nx == t(ends[1])
                } else if left && g == param("turn_girder") {
                    nx == param("turn_x")
                } else {
                    nx == t(ends[2]) || nx == t(ends[3])
                }
            }
            // $E73C: the bottom girder's end, `end_step` in a girder up
            // (the loop counts $E0 - 1 down as a byte), then the gaps.
            4 => {
                let step = param("end_step") * if left { 1 } else { -1 };
                let ups = (g - 1).rem_euclid(256);
                let end = (t(i64::from(ENDS_100M)) + step * ups).rem_euclid(256);
                nx == end || self.at_gap(g, left, x, flag)
            }
            _ => false,
        }
    }

    /// $E770: whether a fire at `x` (its sprite's x before the step) on
    /// girder `g` is at a gap where a rivet was: the flags $C1 + 2 (g - 2)
    /// and the byte after (zero-page bytes, so girder 1 reads $BF and $C0),
    /// each against its gap's x in C3DE by the way.
    fn at_gap(&self, g: i64, left: bool, x: i64, flag: &dyn Fn(i64) -> i64) -> bool {
        let first = (0xC1 + 2 * (g - 2)).rem_euclid(256);
        let at = i64::from(GAPS_100M) + 2 * i64::from(left);
        (0..2).any(|k| flag(first + k) != 0 && self.byte(at + k) == x)
    }

    /// A climb's gate pattern ($E41B): C3F4 by the state (3 up, $13 down),
    /// C3F8 on 100m from difficulty 3 ($E436).
    pub fn climb_gates(&self, d: i64, state: i64) -> (i64, i64) {
        let fast = self.stage == 4 && d >= 3;
        let base = i64::from(if fast { CLIMB_GATES_FAST } else { CLIMB_GATES });
        let at = base + (state >> 3);
        (self.byte(at), self.byte(at + 1))
    }

    /// $E7A3's lookup: the entries C49B's index gives girder `g`, from
    /// C4A1's list for the stage, three bytes each (x, the y to stop at,
    /// the state); the first whose x is `x`.
    pub fn ladder(&self, g: i64, x: i64) -> Option<(i64, i64)> {
        if g == 0 {
            return None;
        }
        let index = self.word(i64::from(LADDER_INDEX) + (self.stage - 2) * 2);
        let list = self.word(i64::from(LADDER_LISTS) + (self.stage - 2) * 2);
        let (mut i, end) = (self.byte(index + g - 1), self.byte(index + g));
        // Y steps by 3 as a byte until it is the end.
        for _ in 0..256 {
            if i == end {
                return None;
            }
            if self.byte(list + i) == x {
                return Some((self.byte(list + i + 1), self.byte(list + i + 2)));
            }
            i = (i + 3) & 0xFF;
        }
        None
    }
}

/// Runs `f` on the stage rules of fire kind `fire` for the level being
/// played.
fn with_stage<R>(cx: &ExtCx<'_>, fire: usize, f: impl FnOnce(&Stage<'_>) -> R) -> R {
    let rom = |at: i64| crate::rom_at_kind(cx, fire, at as u16);
    let param = |name: &str| kp(cx, fire, name);
    f(&Stage {
        stage: stage(cx, fire),
        rom: &rom,
        param: &param,
    })
}

/// The oil drum: the timers the fires read, its lighting, and a new fire
/// in the lowest free slot while it is lit.
pub(crate) struct Drum;

fn counter(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.counter(name).unwrap_or(0)
}

impl gp_sim::Ext for Drum {
    fn step(&self, cx: &mut ExtCx<'_>) {
        // The drum runs first in the frame, and does the whole frame's work
        // on the counters, so each holds the game's value at the frame's
        // end. Fires and barrels read none of it after this.
        //
        // While a state stops the enemies (a death, a clear, a smash), the
        // game runs only its timer routine ($F4AC): the timers count, and
        // nothing resets or reads them.
        // A frame a state stops the counters on (the pause's first frame:
        // the Start check at $CF42 comes after the frame's work and skips
        // $F4AC) steps none of the timers.
        let timers = !cx.counters_stopped;
        if cx.system_stopped {
            // A fire breaking under the hammer ($EE0C, before the timers).
            smash_turn(cx);
            if !timers {
                return;
            }
            let down = |n: i64| if n > 0 { n - 1 } else { n };
            for name in ["drum_timer", "fire_aim"] {
                let v = down(counter(cx, name));
                cx.set_counter(name, v);
            }
            if counter(cx, "divider") == 0 {
                let v = down(counter(cx, "fire_wait"));
                cx.set_counter("fire_wait", v);
            }
            return;
        }
        let aim_frames = p(cx, "aim_frames");
        let mut fire_aim = counter(cx, "fire_aim");
        let st = cx.kind("fire").map_or(1, |f| stage(cx, f));
        // The last frame's aim reset ($E237, after the fires) clears every
        // fire's aim before they choose again. 75m has none ($E23B).
        if st != 3 && fire_aim == aim_frames - 1 {
            clear_aims(cx);
        }
        let lit = p(cx, "lit_frames");
        let mut drum = counter(cx, "drum");
        let mut drum_timer = counter(cx, "drum_timer");
        let mut fire_wait = counter(cx, "fire_wait");
        // The fires: the lowest free slot takes a fire while the drum is
        // lit and the wait is out ($E200).
        if let Some(fire) = cx.kind("fire") {
            let out = fire_slots(cx, fire).len() as i64;
            // 25m's fire waits for the lit drum ($E200); 50m's and 100m's
            // for the wait alone, which starts at C466[d] ($E213); 75m's
            // are the setup's and none comes after ($E1FD).
            let place = match st {
                1 => (out < p(cx, "fires") && fire_wait == 0 && drum == 2).then(|| {
                    fire_wait = p(cx, "fire_wait_frames");
                    (cx.param("out_x").floor(), cx.param("out_y").floor())
                }),
                3 => None,
                _ => {
                    let slots = with_stage(cx, fire, |s| s.slots());
                    if out < slots && fire_wait == 0 {
                        let d = crate::difficulty(cx);
                        fire_wait = with_stage(cx, fire, |s| s.wait(d));
                        Some(if st == 4 {
                            // $E574 reads $19 (draw site `fire_spawn`).
                            let r = i64::from(crate::draw(cx, "fire_spawn", 1));
                            let mario = crate::player_x(cx) - kp(cx, fire, "player_oam");
                            with_stage(cx, fire, |s| s.spawn_point(mario, r))
                        } else {
                            (kp(cx, cx.me.kind, "out_x"), kp(cx, cx.me.kind, "out_y"))
                        })
                    } else {
                        None
                    }
                }
            };
            if let Some((x, y)) = place {
                // The spawn ($E553, $E58D) draws it as `tile_a` in the
                // lowest free slot.
                let held = fire_slots(cx, fire);
                let slot = (0..).find(|n| !held.contains(n)).unwrap_or(0);
                // The fire loop runs up the slots, so a fire in the slot
                // a smashed one left takes that one's place in the bodies'
                // order, and its turn (and its draws) before the fires in
                // higher slots. It moves from the next frame, as a new
                // body does ([`REVIVED`]).
                let dead = cx
                    .world
                    .bodies
                    .iter()
                    .position(|b| b.kind == fire && !b.alive && b.ext[3] == slot + 1);
                if let Some(i) = dead {
                    let mut nb = gp_sim::systems::new_body(
                        cx.spec,
                        cx.world,
                        fire,
                        Fx::from_int(x),
                        Fx::from_int(y),
                        false,
                    );
                    nb.ext[0] = REVIVED;
                    nb.ext[3] = slot + 1;
                    cx.set_body(i, nb);
                } else {
                    cx.spawn(fire, Fx::from_int(x), Fx::from_int(y), false);
                }
                let tile = format!("fire_tile_{slot}");
                if cx.counter(&tile).is_some() {
                    let a = kp(cx, fire, "tile_a");
                    cx.set_counter(&tile, a);
                }
            }
        }
        if st != 3 && fire_aim == 0 {
            fire_aim = aim_frames;
        }
        // The drum ($E19A, after the barrels). A barrel that reaches it
        // lights it itself (`dk_barrel`), as this frame's 1 and then 2.
        // Its flame ($02E1) is `flame_a` from the lighting and turns
        // between `flame_a` and `flame_b` each time the timer runs out, so
        // it holds while play stops.
        let mut flame = counter(cx, "drum_flame");
        let (fa, fb) = (p(cx, "flame_a"), p(cx, "flame_b"));
        if drum == 1 {
            drum = 2;
            drum_timer = lit;
            flame = fa;
        } else if drum != 0 && drum_timer == 0 {
            drum = 3;
            drum_timer = lit;
            flame = if flame == fa { fb } else { fa };
        }
        if cx.counter("drum_flame").is_some() {
            cx.set_counter("drum_flame", flame);
        }
        // The frame's end ($F4AC).
        let down = |n: i64| if timers && n > 0 { n - 1 } else { n };
        // The counters system has already stepped the divider this frame:
        // 0 is the frame it wrapped, when the game's slow timers step.
        if counter(cx, "divider") == 0 {
            fire_wait = down(fire_wait);
        }
        cx.set_counter("drum", drum);
        cx.set_counter("drum_timer", down(drum_timer));
        cx.set_counter("fire_wait", fire_wait);
        cx.set_counter("fire_aim", down(fire_aim));
    }
}

/// The slots the living fires hold.
fn fire_slots(cx: &ExtCx<'_>, fire: usize) -> Vec<i64> {
    cx.world
        .bodies
        .iter()
        .enumerate()
        .filter(|(i, b)| *i != cx.id && b.kind == fire && b.alive && b.ext[3] > 0)
        .map(|(_, b)| b.ext[3] - 1)
        .collect()
}

/// A zero-page byte the gap test reads ($E783, $E790): the rivets' flags
/// $C1-$C8 are the counters `rivet_0` to `rivet_7` (0 in place, 1 taken),
/// $BF is `smash_step`; any other, or one the model doesn't keep, 0.
fn zero_page_flag(cx: &ExtCx<'_>, at: i64) -> i64 {
    match at {
        0xBF => counter(cx, "smash_step"),
        0xC1..=0xC8 => counter(cx, &format!("rivet_{}", at - 0xC1)),
        _ => 0,
    }
}

/// Clears every fire's aim, so each aims again when it next chooses.
fn clear_aims(cx: &mut ExtCx<'_>) {
    let Some(fire) = cx.kind("fire") else {
        return;
    };
    let fires: Vec<usize> = (0..cx.world.bodies.len())
        .filter(|&i| cx.world.bodies[i].kind == fire && cx.world.bodies[i].alive)
        .collect();
    for i in fires {
        let mut b = cx.world.bodies[i];
        b.ext[2] &= !AIMED;
        cx.set_body(i, b);
    }
}

const AIMED: i64 = 1 << 16;

/// A fire ($E250): out of the drum, then bobbing, walking, climbing and
/// choosing again.
pub(crate) struct Fire;

struct Words {
    state: i64,
    gate: i64,
    pause: i64,
    aimed: bool,
    aim: i64,
    girder: i64,
    faced: i64,
    /// $DB: the y a climb off 25m ends at, 0 while it climbs none ($E7A3).
    target: i64,
    tile: i64,
}

impl Words {
    fn read(b: &gp_sim::world::Body) -> Self {
        Words {
            state: b.ext[0] - 1,
            gate: b.ext[1],
            pause: b.ext[2] & 0xFF,
            aimed: b.ext[2] & AIMED != 0,
            aim: (b.ext[2] >> 24) & 0xFF,
            girder: (b.ext[2] >> 8) & 0xFF,
            faced: (b.ext[2] >> 32) & 0xFF,
            target: (b.ext[2] >> 40) & 0xFF,
            tile: 0,
        }
    }

    fn write(&self, b: &mut gp_sim::world::Body) {
        b.ext[0] = self.state + 1;
        b.ext[1] = self.gate;
        b.ext[2] = (self.pause & 0xFF)
            | ((self.girder & 0xFF) << 8)
            | if self.aimed { AIMED } else { 0 }
            | ((self.aim & 0xFF) << 24)
            | ((self.faced & 0xFF) << 32)
            | ((self.target & 0xFF) << 40);
    }
}

impl Fire {
    /// The picture after a walk's or a ladder's step: the other one,
    /// mirrored with `flip`.
    fn turn(cx: &ExtCx<'_>, w: &mut Words, flip: bool) {
        let (a, b) = (kp(cx, cx.me.kind, "tile_a"), kp(cx, cx.me.kind, "tile_b"));
        let base = if w.tile >= b { a } else { b };
        w.tile = base + if flip { p(cx, "tile_flip") } else { 0 };
    }

    /// The move counter's step ($E806): on with the pattern's bit.
    fn gate(cx: &ExtCx<'_>, w: &mut Words, pattern: &str) -> bool {
        let lo = p(cx, &format!("{pattern}_lo"));
        let hi = p(cx, &format!("{pattern}_hi"));
        Self::gate_bits(w, lo, hi)
    }

    /// [`Fire::gate`] with the pattern's two bytes.
    fn gate_bits(w: &mut Words, lo: i64, hi: i64) -> bool {
        w.gate += 1;
        if !(0..16).contains(&w.gate) {
            w.gate = 0;
        }
        let bits = if w.gate < 8 { lo } else { hi };
        bits >> (w.gate & 7) & 1 != 0
    }

    /// The girder under a row ($E016, over the girders' heights C0BC, the
    /// model's `ext_tables.heights`): 1 at the bottom.
    fn girder(cx: &ExtCx<'_>, row: i64) -> i64 {
        for k in 0..6 {
            if crate::rom_at(cx, GIRDER_HEIGHTS + k as u16) <= row {
                return k + 1;
            }
        }
        7
    }

    /// Whether `x` is a slope step on girder `g` ($E5E8): `slope_0`, then
    /// the 8 of the lifted table C1C5 (the model's `ext_tables.tables`); the
    /// bottom and top girders step only from `slope_ends` on.
    fn slope_step(cx: &ExtCx<'_>, g: i64, x: i64) -> bool {
        let from = if g == 1 || g == 6 {
            p(cx, "slope_ends")
        } else {
            0
        };
        let at = |k: i64| {
            if k == 0 {
                p(cx, "slope_0")
            } else {
                crate::rom_at(cx, SLOPES + (k - 1) as u16)
            }
        };
        (from..9).any(|k| at(k) == x)
    }

    /// A ladder's top and foot at `x` for girder `g` ($E626).
    fn ladder(cx: &ExtCx<'_>, g: i64, x: i64) -> Option<(i64, i64)> {
        let set = if g == 1 { "bottom" } else { "mid" };
        ["a", "b"].into_iter().find_map(|k| {
            (p(cx, &format!("ladder_{set}_{k}_x")) == x).then(|| {
                (
                    p(cx, &format!("ladder_{set}_{k}_top")),
                    p(cx, &format!("ladder_{set}_{k}_foot")),
                )
            })
        })
    }

    fn bob(cx: &ExtCx<'_>, w: &mut Words, y: &mut i64) {
        if !Self::gate(cx, w, "walk_gate") {
            return;
        }
        if w.state == BOB_END {
            w.state = CHOOSE;
            return;
        }
        if w.state == BOB_DOWN {
            *y += 1;
            w.state = BOB_END;
        } else {
            *y -= 1;
            w.state = BOB_DOWN;
        }
        // $E325 tests the tile for `tile_b` itself, so a mirrored one
        // turns to `tile_b` too. The bob faces the way it last walked.
        let (a, b) = (kp(cx, cx.me.kind, "tile_a"), kp(cx, cx.me.kind, "tile_b"));
        let base = if w.tile == b { a } else { b };
        w.tile = base
            + if w.faced == LEFT {
                p(cx, "tile_flip")
            } else {
                0
            };
    }

    /// A step along the girder ($E368), `left` for state 2.
    fn walk(cx: &ExtCx<'_>, w: &mut Words, left: bool, slot: usize, x: &mut i64, y: &mut i64) {
        if !Self::gate(cx, w, "walk_gate") {
            return;
        }
        let nx = (*x + if left { -1 } else { 1 }).rem_euclid(256);
        let mut ny = *y;
        let low = nx & 0xF;
        let (a, b) = (p(cx, "dip_a"), p(cx, "dip_b"));
        let back = if left { -1 } else { 1 };
        if low == a || low == b {
            ny += 1;
            w.state = CHOOSE;
        } else if low == a + back || low == b + back {
            ny -= 1;
        }
        let st = stage(cx, cx.me.kind);
        if st != 1 {
            // $E6A5 off 25m: the girder and its ends, then $E3CE's wall,
            // where a fire going left stops, and at a smaller x doesn't
            // move. Every stop leaves Z set ($E735, $E769, $84B6): no move.
            let row = ny + p(cx, "probe");
            w.girder = with_stage(cx, cx.me.kind, |s| s.girder(row));
            let flag = |at: i64| zero_page_flag(cx, at);
            let end = with_stage(cx, cx.me.kind, |s| s.at_end(w.girder, left, *x, nx, &flag));
            if end {
                w.state = BOB;
                return;
            }
            let wall = p(cx, "wall_left");
            if left && nx <= wall {
                w.state = BOB;
                if nx < wall {
                    return;
                }
            }
            *x = nx;
            *y = ny;
            Self::turn(cx, w, left);
            return;
        }
        // $E6A5: the girder, its slope steps and its ends.
        w.girder = Self::girder(cx, ny + p(cx, "probe"));
        let g = w.girder;
        if Self::slope_step(cx, g, nx) {
            let up_right = g % 2 == 1;
            ny += if up_right == left { 1 } else { -1 };
        }
        let edge = match (left, g == 1) {
            (true, _) => p(cx, "edge_left"),
            (false, true) => p(cx, "edge_right_bottom"),
            (false, false) => p(cx, "edge_right"),
        };
        // At the girder's end ($E6F3, against $C3E8 by the way it walks)
        // it bobs. $E6FD loads the slot into X before the return, so the
        // BNE at $E3C5 reads the slot's zero flag, not the 0 returned:
        // slot 0 stops where it was ($E3C7), slot 1 takes the step and
        // draws it ($E3ED).
        if nx == edge {
            w.state = BOB;
            if slot == 0 {
                return;
            }
        }
        if left && nx < p(cx, "edge_left") {
            w.state = BOB;
            return;
        }
        *x = nx;
        *y = ny;
        Self::turn(cx, w, left);
    }

    /// On a ladder ($E41B), up for state 3 and down for $13.
    fn climb(cx: &ExtCx<'_>, w: &mut Words, slot: usize, x: i64, y: &mut i64) {
        let gate = if w.state == DOWN {
            "down_gate"
        } else {
            "up_gate"
        };
        if !Self::gate(cx, w, gate) {
            return;
        }
        // Every step on a ladder draws it, a pause too, mirrored.
        Self::turn(cx, w, true);
        if w.pause == p(cx, "ladder_pause") {
            w.pause = 0;
        } else if *y & p(cx, "ladder_pause_mask") == 0 {
            w.pause += 1;
            return;
        }
        let Some((top, foot)) = Self::ladder(cx, w.girder, x) else {
            return;
        };
        if w.state == DOWN {
            *y += 1;
            if *y == foot {
                w.state = RIGHT;
            }
        } else {
            *y -= 1;
            if slot != 0 || w.girder == 2 {
                if *y == top + p(cx, "climb_back") {
                    w.state = DOWN;
                }
            } else if *y == top {
                w.state = LEFT;
            }
        }
    }

    /// On a ladder off 25m ($E41B, then $E7A3): the gate C3F4 by the state
    /// (C3F8 on 100m from d = 3), the pause, then the girder's entry in the
    /// stage's list at the fire's x, which gives the y to stop at ($DB) and
    /// the way (3 up, $13 down); no entry, it bobs. At the y it walks right.
    fn climb_list(cx: &ExtCx<'_>, w: &mut Words, x: i64, y: &mut i64) {
        // $E436 reads $50 and $54 as the frame began: on 100m's clear
        // the fires run before $D086 raises $54 (game B's round 2, the
        // clear's frame stays at C3F4).
        let d = (cx.counter_at_start("game_b").unwrap_or(0)
            + cx.counter_at_start("loop").unwrap_or(0))
        .clamp(0, 4);
        let (lo, hi) = with_stage(cx, cx.me.kind, |s| s.climb_gates(d, w.state));
        if !Self::gate_bits(w, lo, hi) {
            return;
        }
        Self::turn(cx, w, true);
        if w.pause == p(cx, "ladder_pause") {
            w.pause = 0;
        } else if *y & p(cx, "ladder_pause_mask") == 0 {
            w.pause += 1;
            return;
        }
        if w.target == 0 {
            match with_stage(cx, cx.me.kind, |s| s.ladder(w.girder, x)) {
                Some((target, state)) => {
                    w.target = target;
                    w.state = state;
                }
                None => w.state = BOB,
            }
        }
        let step = match w.state {
            LADDER => -1,
            DOWN => 1,
            _ => return,
        };
        *y = (*y + step).rem_euclid(256);
        if *y == w.target {
            w.state = RIGHT;
            w.target = 0;
        }
    }

    /// A new way ($E2B6): a random byte's low 3 bits, 4 to 6 taking the
    /// way toward the player it aimed at last.
    fn choose(cx: &mut ExtCx<'_>, w: &mut Words, x: i64) {
        let slot = (cx.me.ext[3] - 1).clamp(0, 6) as u32;
        // 75m's ($E278): $19 + slot AND 3, a way or a bob or a ladder.
        if stage(cx, cx.me.kind) == 3 {
            w.state = i64::from(crate::draw(cx, "fire_way_75m", 1 + slot)) & 3;
            if w.state == RIGHT || w.state == LEFT {
                w.faced = w.state;
            }
            return;
        }
        if !w.aimed {
            w.aimed = true;
            let mario = crate::player_x(cx) - p(cx, "player_oam");
            w.aim = if x >= mario { LEFT } else { RIGHT };
        }
        // $E2DD reads $19,X, X the fire's slot ($AE).
        let r = i64::from(crate::draw(cx, "fire_way", 1 + slot)) & 7;
        w.state = match r {
            0..4 => r,
            7 => LADDER,
            _ => w.aim,
        };
        // $E280: a walk's way is kept for the bob's facing ($B3).
        if w.state == RIGHT || w.state == LEFT {
            w.faced = w.state;
        }
    }
}

/// A fire's word 0 on the frame the drum put it in a smashed fire's
/// body: it doesn't move until the next frame.
const REVIVED: i64 = -1;

impl gp_sim::Ext for Fire {
    fn step(&self, cx: &mut ExtCx<'_>) {
        if cx.me.ext[0] == REVIVED {
            cx.me.ext[0] = 0;
            return;
        }
        let mut w = Words::read(cx.me);
        let st = stage(cx, cx.me.kind);
        // A new fire comes out of the drum on 25m and 50m; 100m's starts
        // in a bob ($E589), and so do 75m's, the setup's (TAS 5658S frame
        // 1610: $AF and $B0 are 0).
        if cx.me.ext[0] == 0 {
            w.state = if st >= 3 { BOB } else { OUT };
        }
        // A new fire takes the lowest slot the others don't hold.
        if cx.me.ext[3] == 0 {
            let held = fire_slots(cx, cx.me.kind);
            cx.me.ext[3] = (0..).find(|n| !held.contains(n)).unwrap_or(0) + 1;
        }
        let slot = (cx.me.ext[3] - 1) as usize;
        // The game sets a new fire's state and place only: its move
        // counter ($E4) and ladder pause ($E8) are what the slot's last
        // fire left, which no stage start clears. The model keeps them in
        // the counter `fire_left_<slot>`, $E4 plus 256 times $E8.
        let left = format!("fire_left_{slot}");
        let tile = format!("fire_tile_{slot}");
        w.tile = cx.counter(&tile).unwrap_or(0);
        if cx.me.ext[0] == 0 {
            let kept = cx.counter(&left).unwrap_or(0);
            w.gate = kept & 0xFF;
            w.pause = (kept >> 8) & 0xFF;
            // And the way its bob faces ($B3), kept the same way: only a
            // smashed fire leaves a slot to another.
            w.faced = (counter(cx, "fires_faced") >> (8 * slot)) & 0xFF;
            // The way out ($E553) draws it as `tile_a`.
            w.tile = kp(cx, cx.me.kind, "tile_a");
        }
        let (mut x, mut y) = (cx.me.x.floor(), cx.me.y.floor());
        loop {
            match w.state & 0xF {
                // Off 25m and 50m, $E59F only draws it ($E5B3).
                0x6 | 0x8 if st >= 3 => {}
                0x6 | 0x8 => {
                    // Out of the drum ($E59F): across, then down to the
                    // girder.
                    x += 1;
                    if x >= kp(cx, cx.me.kind, "rise_x") {
                        y += 1;
                        if y == kp(cx, cx.me.kind, "floor_y") {
                            w.state = BOB;
                            x -= 1;
                            y += if x >= p(cx, "mid_x") { -1 } else { 1 };
                        }
                    }
                }
                0x0 => Self::bob(cx, &mut w, &mut y),
                0x1 => Self::walk(cx, &mut w, false, slot, &mut x, &mut y),
                0x2 => Self::walk(cx, &mut w, true, slot, &mut x, &mut y),
                // Off 25m every ladder state goes to $E41B ($E2A5).
                0x3 if st != 1 => Self::climb_list(cx, &mut w, x, &mut y),
                0x3 => {
                    if w.state != DOWN {
                        w.state = if Self::ladder(cx, w.girder, x).is_some() {
                            LADDER
                        } else {
                            BOB
                        };
                    }
                    if w.state == BOB {
                        Self::bob(cx, &mut w, &mut y);
                    } else {
                        Self::climb(cx, &mut w, slot, x, &mut y);
                    }
                }
                _ => {
                    Self::choose(cx, &mut w, x);
                    continue;
                }
            }
            break;
        }
        cx.me.x = Fx::from_int(x);
        cx.me.y = Fx::from_int(y);
        w.write(cx.me);
        cx.set_counter(&left, (w.gate & 0xFF) | ((w.pause & 0xFF) << 8));
        if cx.counter(&tile).is_some() {
            cx.set_counter(&tile, w.tile);
        }
        smash_test(cx, st);
    }
}

/// A fire being smashed: its `time` while it breaks (a fire's own
/// routine doesn't read it).
const SMASHED: u32 = 1;

/// Off 25m the hammer smashes fires ($ED8A, after every fire has moved:
/// no fire touched Mario, he holds the hammer ($96 = 10) and $53 isn't 1,
/// so $EDC1 calls $ECBF, which goes to $ED34 there). The head's box (the
/// hammer's words, as the barrels read them) against each fire's, the
/// hammer's `smash_*` from its place ($3A at $ED3E), up the slots ($AE);
/// the first that meets is smashed ($ED57-$ED82): the smash's sound
/// ($FF = 2), `fire_wait` ($40) 16, its girder ($E0) and its climb's end
/// ($DB) cleared, and `smash_step` ($BF) 1 with `barrel.smashed`, which
/// stops play while it breaks ([`smash_turn`]). The test runs once a
/// frame, in the last fire's hook, when all have moved. 25m's fires are
/// never smashed: there $ECBF tests the barrels ($ED0C).
fn smash_test(cx: &mut ExtCx<'_>, st: i64) {
    let kind = cx.me.kind;
    // On 50m a pie smashed this frame gives the smash to a fire: the game
    // tests the fires first ([`crate::m50::fire_smashed`]).
    if st == 1 || (counter(cx, "smash_step") != 0 && !(st == 2 && crate::m50::pie_smashed_now(cx)))
    {
        return;
    }
    let fires: Vec<(usize, gp_sim::world::Body)> = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| if i == cx.id { (i, *cx.me) } else { (i, *b) })
        .filter(|(_, b)| b.kind == kind && b.alive && b.ext[3] > 0)
        .collect();
    if fires.last().is_none_or(|(i, _)| *i != cx.id) {
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
    let boxed = |b: &gp_sim::world::Body| {
        let k = cx.spec.kinds[b.kind].hitbox;
        let (x, y) = (b.x.floor() + k.x.floor(), b.y.floor() + k.y.floor());
        [x, y, x + k.w.floor() - 1, y + k.h.floor() - 1]
    };
    // $ED8A tests every fire against Mario first: a touch kills him and
    // smashes none.
    let mario = cx
        .world
        .bodies
        .iter()
        .find(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive);
    if let Some(m) = mario
        && fires.iter().any(|(_, f)| crate::meet(boxed(m), boxed(f)))
    {
        return;
    }
    let ((x0, x1), (y0, y1)) = (crate::unpack(h.ext[1]), crate::unpack(h.ext[2]));
    let hp = |name: &str| {
        cx.spec.kinds[hammer]
            .ext_params
            .get(name)
            .map_or(0, |v| v.floor())
    };
    let mut order = fires;
    order.sort_by_key(|(_, b)| b.ext[3]);
    let Some((i, f)) = order.into_iter().find(|(_, b)| {
        let (x, y) = (b.x.floor(), b.y.floor());
        let body = [
            x + hp("smash_x0"),
            y + hp("smash_y0"),
            x + hp("smash_x1"),
            y + hp("smash_y1"),
        ];
        crate::meet([x0, y0, x1, y1], body)
    }) else {
        return;
    };
    let Some(event) = cx.event("barrel.smashed") else {
        return;
    };
    let mut w = Words::read(&f);
    w.girder = 0;
    w.target = 0;
    let mut b = f;
    w.write(&mut b);
    b.time = SMASHED;
    if i == cx.id {
        *cx.me = b;
    } else {
        cx.set_body(i, b);
    }
    if st == 2 {
        crate::m50::fire_smashed(cx);
    }
    cx.set_counter("smash_step", 1);
    cx.set_counter("sound_ff", 2);
    crate::set_slow_timer(cx, "fire_wait", 16);
    cx.fire(event, Some(i));
}

/// A frame of a fire's smash ($EE0C off 25m): the frame counter at $8A
/// of the barrel loop's slot $5D (the counter `smash_beat`, which nothing
/// else steps off 25m) steps, 0 to 15 ($DFE8 with $80 and $80); on 7 and
/// 15 the smash steps, the fire's sprite drawn as C1EC[`smash_step` - 1]
/// ($EE3C-$EE44, the barrels' break-up pictures) and the first asking for
/// its sound again ($EE34); after
/// the 11th step the fire goes ($EAD1 hides it), 500 points ($EE62) and
/// `smash.done`.
fn smash_turn(cx: &mut ExtCx<'_>) {
    let step = counter(cx, "smash_step");
    let Some(fire) = cx.kind("fire") else {
        return;
    };
    if step == 0 || stage(cx, fire) == 1 {
        return;
    }
    let Some(i) = cx
        .world
        .bodies
        .iter()
        .position(|b| b.kind == fire && b.alive && b.time == SMASHED)
    else {
        return;
    };
    let beat = (counter(cx, "smash_beat") + 1) % 16;
    if cx.counter("smash_beat").is_some() {
        cx.set_counter("smash_beat", beat);
    }
    if beat % 8 != 7 {
        return;
    }
    let b = cx.world.bodies[i];
    if step < 11 {
        if step == 1 {
            cx.set_counter("sound_ff", 2);
        }
        let tile = format!("fire_tile_{}", b.ext[3] - 1);
        if cx.counter(&tile).is_some() {
            let pic = crate::rom_at_kind(cx, fire, SMASH_TILES + (step - 1) as u16);
            cx.set_counter(&tile, pic);
        }
        cx.set_counter("smash_step", step + 1);
        return;
    }
    cx.set_counter("smash_step", 0);
    crate::award_points(cx);
    // The slot's next fire bobs facing the way this one last walked
    // ($B3,X stays).
    if cx.counter("fires_faced").is_some() {
        let shift = 8 * (b.ext[3] - 1);
        let faced = Words::read(&b).faced & 0xFF;
        let kept = counter(cx, "fires_faced") & !(0xFF << shift);
        cx.set_counter("fires_faced", kept | (faced << shift));
    }
    cx.remove(i);
    if let Some(done) = cx.event("smash.done") {
        cx.fire(done, Some(i));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Made-up tables at the addresses the code reads, and made-up
    /// parameters: none of it is the game's.
    struct Made {
        bytes: HashMap<i64, i64>,
        params: HashMap<&'static str, i64>,
    }

    impl Made {
        fn new() -> Self {
            Made {
                bytes: HashMap::new(),
                params: HashMap::new(),
            }
        }

        fn put(&mut self, at: i64, values: &[i64]) {
            for (k, v) in values.iter().enumerate() {
                self.bytes.insert(at + k as i64, *v);
            }
        }

        fn word(&mut self, at: i64, w: i64) {
            self.put(at, &[w & 0xFF, w >> 8]);
        }

        fn run<R>(&self, stage: i64, f: impl FnOnce(&Stage<'_>) -> R) -> R {
            let rom = |at: i64| self.bytes.get(&at).copied().unwrap_or(0);
            let param = |name: &str| self.params.get(name).copied().unwrap_or(0);
            f(&Stage {
                stage,
                rom: &rom,
                param: &param,
            })
        }
    }

    #[test]
    fn slots_and_waits_are_read_by_stage_and_difficulty() {
        let mut m = Made::new();
        m.put(i64::from(COUNTS), &[2, 3, 1, 5]);
        m.put(i64::from(WAITS), &[50, 40, 30, 20, 10]);
        assert_eq!(m.run(2, |s| s.slots()), 3);
        assert_eq!(m.run(4, |s| s.slots()), 5);
        assert_eq!(m.run(4, |s| s.wait(0)), 50);
        assert_eq!(m.run(4, |s| s.wait(4)), 10);
    }

    #[test]
    fn the_girder_comes_from_the_stages_list_of_heights() {
        let mut m = Made::new();
        // Stage 2's list at a made-up address, through the pointer table.
        m.word(i64::from(HEIGHT_LISTS) + 2, 0x9000);
        m.put(0x9000, &[180, 140, 100, 60, 0xFF]);
        let g = |row| m.run(2, |s| s.girder(row));
        assert_eq!(g(200), 1);
        assert_eq!(g(180), 1);
        assert_eq!(g(179), 2);
        assert_eq!(g(100), 3);
        assert_eq!(g(61), 4);
        assert_eq!(g(10), 7);
    }

    #[test]
    fn a_ladder_is_the_girders_entry_at_the_fires_x() {
        let mut m = Made::new();
        // Stage 3: the index and the entries at made-up addresses.
        m.word(i64::from(LADDER_INDEX) + 2, 0x9100);
        m.word(i64::from(LADDER_LISTS) + 2, 0x9200);
        // Girder 1 has entries 0 and 1 (bytes 0 to 5), girder 2 none,
        // girder 3 entry 2.
        m.put(0x9100, &[0, 6, 6, 9]);
        m.put(0x9200, &[30, 150, 3, 70, 120, 3, 90, 200, 0x13]);
        let l = |g, x| m.run(3, |s| s.ladder(g, x));
        assert_eq!(l(1, 30), Some((150, 3)));
        assert_eq!(l(1, 70), Some((120, 3)));
        assert_eq!(l(1, 90), None);
        assert_eq!(l(2, 30), None);
        assert_eq!(l(3, 90), Some((200, 0x13)));
        // Girder 0 (none set yet) has no ladder.
        assert_eq!(l(0, 30), None);
    }

    #[test]
    fn climb_gates_go_by_the_state_and_100ms_difficulty() {
        let mut m = Made::new();
        m.put(i64::from(CLIMB_GATES), &[1, 2, 3, 4]);
        m.put(i64::from(CLIMB_GATES_FAST), &[5, 6, 7, 8]);
        assert_eq!(m.run(3, |s| s.climb_gates(4, LADDER)), (1, 2));
        assert_eq!(m.run(3, |s| s.climb_gates(4, DOWN)), (3, 4));
        assert_eq!(m.run(4, |s| s.climb_gates(2, LADDER)), (1, 2));
        assert_eq!(m.run(4, |s| s.climb_gates(3, LADDER)), (5, 6));
        assert_eq!(m.run(4, |s| s.climb_gates(3, DOWN)), (7, 8));
    }

    #[test]
    fn a_new_100m_fire_starts_on_the_side_away_from_mario() {
        let mut m = Made::new();
        m.params.insert("spawn_side", 100);
        m.put(
            i64::from(SPAWN_POINTS),
            &[1, 11, 2, 12, 3, 13, 4, 14, 5, 15, 6, 16, 7, 17, 8, 18],
        );
        let at = |mario, r| m.run(4, |s| s.spawn_point(mario, r));
        assert_eq!(at(150, 0), (1, 11));
        assert_eq!(at(150, 3), (4, 14));
        assert_eq!(at(150, 7), (4, 14));
        assert_eq!(at(99, 0), (5, 15));
        assert_eq!(at(99, 2), (7, 17));
        assert_eq!(at(100, 1), (2, 12));
    }

    #[test]
    fn stage_2_stops_at_its_table_and_one_girders_extra_ends() {
        let mut m = Made::new();
        m.params.insert("edges_at", 0x9300);
        m.params.insert("edge_girder", 2);
        // Right, left; then two more pairs.
        m.put(0x9300, &[200, 20, 60, 80, 120, 140]);
        let end = |g, left, nx| m.run(2, |s| s.at_end(g, left, nx, nx, &|_| 0));
        assert!(end(1, false, 200));
        assert!(end(1, true, 20));
        assert!(!end(1, true, 200));
        assert!(!end(1, false, 60));
        assert!(end(2, false, 60));
        assert!(end(2, false, 120));
        assert!(end(2, true, 80));
        assert!(end(2, true, 140));
        assert!(!end(2, true, 60));
    }

    #[test]
    fn stage_3_stops_by_its_girder_tables_and_one_turn() {
        let mut m = Made::new();
        m.params.insert("ends_girder", 2);
        m.params.insert("turn_girder", 4);
        m.params.insert("turn_x", 210);
        m.put(i64::from(ENDS_75M[0]), &[90, 70]);
        m.put(i64::from(ENDS_75M[1]), &[190, 170]);
        m.put(i64::from(ENDS_75M[2]), &[91, 71]);
        m.put(i64::from(ENDS_75M[3]), &[191, 171]);
        let end = |g, left, nx| m.run(3, |s| s.at_end(g, left, nx, nx, &|_| 0));
        assert!(end(2, false, 90));
        assert!(end(2, false, 190));
        assert!(end(2, true, 70));
        assert!(!end(2, true, 71));
        assert!(end(1, false, 91));
        assert!(end(3, true, 171));
        // Going left on the turn girder only its x stops it.
        assert!(end(4, true, 210));
        assert!(!end(4, true, 71));
        assert!(end(4, false, 191));
    }

    #[test]
    fn stage_4_ends_step_in_by_girder_and_turn_at_open_gaps() {
        let mut m = Made::new();
        m.params.insert("end_step", 10);
        m.put(i64::from(ENDS_100M), &[230, 5]);
        m.put(i64::from(GAPS_100M), &[50, 170, 60, 180]);
        let none = |_: i64| 0;
        let end = |g, left, x, nx| m.run(4, |s| s.at_end(g, left, x, nx, &none));
        assert!(end(1, false, 229, 230));
        assert!(end(3, false, 209, 210));
        assert!(end(1, true, 6, 5));
        assert!(end(4, true, 36, 35));
        assert!(!end(4, true, 36, 34));
        // Girder 3's flags are $C3 and $C4: with $C4 set, the right gap
        // turns a fire at its x going right, the left one doesn't.
        let flags = |at: i64| i64::from(at == 0xC4);
        let gap = |left, x| m.run(4, |s| s.at_end(3, left, x, x + 1, &flags));
        assert!(gap(false, 170));
        assert!(!gap(false, 50));
        assert!(gap(true, 180));
        assert!(!gap(true, 170));
        // Girder 1 reads the two bytes below $C1.
        let low = |at: i64| i64::from(at == 0xBF);
        assert!(m.run(4, |s| s.at_end(1, false, 50, 51, &low)));
    }
}
