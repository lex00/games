//! Donkey Kong's `ext` hooks: what the model's components can't say about
//! 25m's barrels, written against gp-sim's [`Ext`] trait. Every number a
//! hook uses is the model's `ext_params`, with its evidence in the model
//! (the barrel code at $DA1D-$DD8A and the user's play, worker notes in
//! the donkey-kong-ju mechanic sessions).
//!
//! | Hook | Entity | What it does |
//! |---|---|---|
//! | `dk_throw` | kong | Throws when the counter `throw_timer` has been 0 a frame and fewer than `max` barrels are out, and sets `throw_timer` and `drop_timer` again; with `throws = 0` (a stage past 25m) only beats his chest |
//! | `dk_barrel` | barrel | Held, then rolls along its girder, takes a ladder down 1 time in 4, falls off a girder's end with a bounce, and after a fall may roll off the screen's edge; drops straight from slot 0 at times; fires `barrel.jumped` when the player jumps over it |
//! | `dk_pauline` | pauline | Calls for help: turns her body's tile in `pauline_body` every 16 frames, `calls` times out and back, then waits; `pauline_shown` is the tile on the screen, a frame later |
//! | `dk_sound` | sound | The game's sound driver ($FA48) once a frame, its state in the `sound_*` counters ([`sound`]): it takes the requests other hooks and rules leave in `sound_ff`, `sound_fe` and `sound_fd` and keeps what the driver keeps |
//! | `dk_hammer` | hammer | Takes a hammer touched in a jump when the jump's landing hold ends, swings it while its timer runs and leaves its head box in its words (a barrel that meets it after moving is smashed: `dk_barrel` calls `smash_test`), and drops it when the timer runs out or Mario falls |
//! | `dk_drum` | drum | The oil drum: lit by a barrel, the timers the fires read, and a new fire in a free slot ([`fire`]) |
//! | `dk_mario` | mario | Mario's bytes his components don't keep ($9B) and the step, rung and jump sounds he asks for ([`mario`]) |
//! | `dk_demo` | demo | The attract demo's script ($EBDA): Mario's direction and jumps from the `demo_script` table while `attract` is set ([`demo`]) |
//! | `dk_fire` | fire | A fire out of the drum: bobs, walks its girder, climbs a ladder and chooses a way by a random byte ([`fire`]) |
//! | `dk_conveyor` | conveyor | Donkey Kong: Original Edition's 50m: the conveyors' pattern and turns, the pies they carry and Mario's ride ($84C2-$8754, [`m50`]) |
//! | `dk_elevators` | elevators | Donkey Kong's 75m: the elevators' loop ($E834): the platforms' beat, their moves, the ones that go and the ones that come ([`m75`]) |
//! | `dk_prizes` | prizes_50m | Original Edition's 50m: a prize taken where Mario's sprite stands on its place on a step or a climb's step ($EEF0): its flag, 800 points and the jingle ([`m75`]). 75m's and 100m's prizes are their levels' `pickups` |
//! | `dk_springs` | springs | Donkey Kong's 75m: the springs' loop ($E981): a spring on the throw timer from a random x, its bounce on the jump's arc, its drop and its sounds ([`m75`]) |
//! | `dk_idle` | idle | The idle loop ($C7E1) after the frame's work: steps the random register $18-$1F, the counter `random`, once a pass ([`idle`]) |
//! | `dk_carry` | kong_carry, pauline_carry | The stage-clear scene's Kong taking Pauline up ($8B03, by $43 from `clear_wait`): places each row of Kong's sprites and Pauline, and copies 50m's moving ladders as the scene leaves them ([`clear`]) |
//!
//! A barrel's words: `ext[0]` its state (the game's $5E: $80 held, $81
//! let go, 1 rolling, 2 on a ladder, 8 falling, $10 bouncing, $20 rolling
//! off, $C0 to $C2 the straight drop), `ext[1]` frames in the state,
//! `ext[2]` its girder (the game's $68: the top girder is 6, the bottom 1),
//! `ext[3]` pixels moved down in the state, or in a bounce the height it
//! landed at. Its `time` is the game's frame counter $8A,X. Girders roll right when their number is even.

pub mod demo;
pub mod idle;
pub mod play;
pub mod play_barrels;
pub mod play_fires;
pub mod play_mario;
pub mod sound;

use gp_sim::world::Body;
use gp_sim::{Ext, ExtCx, Exts, Fx};

mod clear;
mod fire;
mod m100;
mod m50;
mod m75;
mod mario;

const HELD: i64 = 0x80;
const LET_GO: i64 = 0x81;
const ROLL: i64 = 1;
const LADDER: i64 = 2;
const FALL: i64 = 8;
const BOUNCE: i64 = 0x10;
const OFF: i64 = 0x20;
/// Dropping through a crowded girder while Mario holds the hammer ($40,
/// $DF07).
const DROP: i64 = 0x40;
const STRAIGHT: i64 = 0xC0;
const STOPPED: i64 = 0xC1;
const LANDED: i64 = 0xC2;

/// Every hook, by the name the model's `ext` gives it.
pub fn exts() -> Exts {
    let mut out = Exts::new();
    out.insert("dk_throw".into(), Box::new(Throw) as Box<dyn Ext>);
    out.insert("dk_barrel".into(), Box::new(Barrel));
    out.insert("dk_sound".into(), Box::new(sound::SoundDriver));
    out.insert("dk_hammer".into(), Box::new(Hammer));
    out.insert("dk_pauline".into(), Box::new(Pauline));
    out.insert("dk_drum".into(), Box::new(fire::Drum));
    out.insert("dk_demo".into(), Box::new(demo::Demo));
    out.insert("dk_fire".into(), Box::new(fire::Fire));
    out.insert("dk_mario".into(), Box::new(mario::Mario));
    out.insert("dk_idle".into(), Box::new(idle::Idle));
    out.insert("dk_conveyor".into(), Box::new(m50::Conveyor));
    out.insert("dk_springs".into(), Box::new(m75::Springs));
    out.insert("dk_prizes".into(), Box::new(m75::Prizes));
    out.insert("dk_elevators".into(), Box::new(m75::Elevators));
    out.insert("dk_taken".into(), Box::new(m100::Taken));
    out.insert("dk_ending".into(), Box::new(m100::Ending));
    out.insert("dk_carry".into(), Box::new(clear::Carry));
    out
}

pub(crate) fn p(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.param(name).floor()
}

/// The byte at CPU address `at` of the lifted tables this hook's entity
/// names in its `ext_tables` (`heights` from $C0BC, `tables` from $C15F,
/// and the others of [`play::TABLES`]), or 0 past them. The tables are the
/// recipe's: the template's asset map lifts them from the person's ROM, or
/// an original authors its own.
pub(crate) fn rom_at(cx: &ExtCx<'_>, at: u16) -> i64 {
    rom_at_kind(cx, cx.me.kind, at)
}

/// [`rom_at`] over the lifted tables entity kind `kind` names, for a hook
/// that reads another entity's tables (the drum reads the fires').
pub(crate) fn rom_at_kind(cx: &ExtCx<'_>, kind: usize, at: u16) -> i64 {
    let tables = &cx.spec.kinds[kind].ext_tables;
    play::TABLES
        .iter()
        .rev()
        .find(|(name, start)| at >= *start && tables.contains_key(*name))
        .and_then(|(name, start)| tables.get(*name)?.get(usize::from(at - start)))
        .copied()
        .unwrap_or(0)
}

/// [`rom_at`] read as the 6502's signed byte.
pub(crate) fn rom_signed(cx: &ExtCx<'_>, at: u16) -> i64 {
    i64::from(rom_at(cx, at) as u8 as i8)
}

/// The barrels' tables in the play routine's ($C15F to $C4FF), by CPU
/// address: references, the values are the lifted tables'.
mod barrel_tables {
    /// A ladder's foot and top x by girder (2 to 6) for the first two
    /// ladders down from each girder ($E0B4's lists).
    pub const FOOT_A: u16 = 0xC172;
    pub const TOP_A: u16 = 0xC177;
    pub const FOOT_B: u16 = 0xC17C;
    pub const TOP_B: u16 = 0xC181;
    /// The straight drop's stops for ways 1 to 3, `STOPS_STEP` apart
    /// ($E112 by the pointers at $C03C + $34, $36 and $38).
    pub const STOPS: u16 = 0xC1D5;
    pub const STOPS_STEP: u16 = 6;
    /// The bounce's x where a barrel's y moves, rightward and leftward,
    /// and the move ($DCD0-$DD31).
    pub const BOUNCE_RIGHT: u16 = 0xC3FC;
    pub const BOUNCE_DY: u16 = 0xC407;
    pub const BOUNCE_LEFT: u16 = 0xC412;
    /// The straight drop's fast rows, from (inclusive) and to (exclusive)
    /// ($DE86).
    pub const FAST_LO: u16 = 0xC41D;
    pub const FAST_HI: u16 = 0xC420;
}

/// The game's difficulty d ($EAF7): game B's 1 plus the loops cleared,
/// from the model's counters `game_b` ($50 bit 0) and `loop` ($54),
/// capped at 4 (`CPX #$04`). Without them, 0.
fn difficulty(cx: &ExtCx<'_>) -> i64 {
    let b = cx.counter("game_b").unwrap_or(0);
    let l = cx.counter("loop").unwrap_or(0);
    (b + l).clamp(0, 4)
}

/// A number the game reads from a table by difficulty: `<name><d>`, or
/// `<name>` when the model gives no table.
fn by_difficulty(cx: &ExtCx<'_>, name: &str) -> i64 {
    let d = format!("{name}{}", difficulty(cx));
    if cx.has_param(&d) {
        p(cx, &d)
    } else {
        p(cx, name)
    }
}

/// Donkey Kong throwing ($DA1D-$DA3B). His timers are the model's
/// counters `throw_timer` ($36) and `drop_timer` ($37), which count down a
/// frame at a time before the hooks run, through a death too. With the
/// throw timer at 0 since the last frame and a barrel slot free, he takes
/// a barrel, and the timers restart at `interval` (C443[d]) and `hold`
/// less this frame's step. `ext[0]` and `ext[1]` keep the timers as the
/// hook's last run ended, `ext[2]` that run's frame, and `ext[3]` marks a
/// frame the drop timer ran out (1): the barrel in his hands is let go the
/// frame after; 2 marks a frame a stop held him. His pose, for
/// presentation, is the counter `kong_pose` ([`pose`]).
struct Throw;

impl Ext for Throw {
    fn step(&self, cx: &mut ExtCx<'_>) {
        // `throws = 0`: a stage past 25m, where $EB06 takes Kong's beats
        // from his wait alone ($EB22: $53 >= 2) and nothing is thrown. The
        // throw timer then matches none of the throw's poses.
        let throws = !cx.has_param("throws") || p(cx, "throws") != 0;
        let throw = if throws {
            cx.counter("throw_timer").unwrap_or(0)
        } else {
            -1
        };
        let drop = cx.counter("drop_timer").unwrap_or(0);
        // The timers as the last frame ended. The hook doesn't run on a
        // frame a state stops the enemies (a smash, a death), but the
        // counters count on, a frame at a time down to 0, so from what
        // they were at the hook's last run (`ext[2]`, its frame) they are
        // that less the frames since. Before any run (a start), the last
        // frame's are taken as this frame's plus one.
        let first = cx.me.ext[2] == 0;
        // Through a stop (`hook_through_stops`) Kong does nothing but keep
        // the timers as they stand, so the frame after a smash or a start
        // inside one knows them as the game's routine does.
        if cx.system_stopped {
            cx.me.ext[0] = throw;
            cx.me.ext[1] = drop;
            cx.me.ext[2] = cx.frame as i64;
            if !first {
                cx.me.ext[3] = 2;
            }
            // $EB06 doesn't run; the pose it last drew reaches the screen
            // at the next frame's start ([`Throw::frame_start`]).
            if cx.has_param("pose_take_at") && first {
                let at = drawn_at_start(cx, throw);
                cx.set_counter("kong_drawn", at);
            }
            return;
        }
        // The barrel loop ($DA16-$DA49) runs its index $5D through the
        // slots to `max` on every frame it runs, the jump and hammer loops
        // after it too, ending there unless the hammer stops one.
        if cx.counter("barrel_loop").is_some() {
            let max = p(cx, "max");
            cx.set_counter("barrel_loop", max);
        }
        crowd_counts(cx);
        // Kong ($EB06) runs on every frame the timers count (through a
        // stop too, `hook_through_stops`), so what the hook last kept is
        // the last frame's; a frame neither runs on (a pause and its 64
        // frames after) counts nothing.
        let (last_throw, last_drop) = if first {
            (throw + 1, drop + 1)
        } else {
            (cx.me.ext[0], cx.me.ext[1])
        };
        cx.me.ext[2] = cx.frame as i64;
        cx.me.ext[3] = i64::from(drop == 0 && last_drop > 0);
        let (mut throw_end, mut drop_end) = (throw, drop);
        if throws
            && throw == 0
            && last_throw == 0
            && let Some(barrel) = cx.kind("barrel")
        {
            let out = cx
                .world
                .bodies
                .iter()
                .filter(|b| b.alive && !b.waiting && b.kind == barrel)
                .count() as i64;
            if out < p(cx, "max") {
                // The timers take a frame off at the frame's end ($F4AC),
                // but not on the pause's first frame.
                let step = i64::from(!cx.counters_stopped);
                throw_end = by_difficulty(cx, "interval") - step;
                drop_end = p(cx, "hold") - step;
                cx.set_counter("throw_timer", throw_end);
                cx.set_counter("drop_timer", drop_end);
                let (x, y) = (cx.param("held_x"), cx.param("held_y"));
                cx.spawn(barrel, x, y, false);
            }
        }
        cx.me.ext[0] = throw_end;
        cx.me.ext[1] = drop_end;
        if cx.has_param("pose_take_at") {
            // At a start, the pose on the screen is the one the game last
            // drew, and the start's own frame ran $EB54 on the timers as
            // the start has them (one more than this frame's).
            // $EB06 runs before the timers ($F4AC, at $CF27), so it reads
            // the throw timer and the wait as the last frame ended: the
            // throw timer one more, and the wait one more on a divider
            // wrap, whose step the model's counters have taken already.
            // The pose's own step is taken first and the wrap's after.
            let wrapped = cx.counter("divider") == Some(0);
            let now_wait = counter(cx, "kong_wait");
            let last_wait = now_wait + i64::from(wrapped && cx.counter("kong_wait").is_some());
            let (drawn, wait, thrown) = if first {
                let at = drawn_at_start(cx, throw);
                let t = counter(cx, "kong_thrown");
                let before = if throws { throw + 1 } else { throw };
                pose(cx, before, at, last_wait, t)
            } else {
                let d = counter(cx, "kong_drawn");
                pose(cx, last_throw, d, last_wait, counter(cx, "kong_thrown"))
            };
            // Each beat drawn asks for pulse 2's effect ($EB85, $EB8E).
            if (drawn == BEAT_A || drawn == BEAT_B)
                && wait != last_wait
                && cx.counter("sound_fe").is_some()
            {
                cx.set_counter("sound_fe", 0x80);
            }
            // With no throws a wait run out starts again at `beat_rest`
            // and draws nothing ($EB29 -> $EB4F).
            let wait = if !throws && last_wait == 0 {
                p(cx, "beat_rest")
            } else {
                wait
            };
            let wait = if wrapped { (wait - 1).max(0) } else { wait };
            if cx.counter("kong_wait").is_some() {
                cx.set_counter("kong_wait", wait.max(0));
                cx.set_counter("kong_thrown", thrown);
            }
            // Every pose, the throw too, reaches the screen the frame after
            // it is drawn ([`Throw::frame_start`]): the name table's $20C5
            // shows the throw a frame after the buffer at $0331 holds it
            // (barrels clip 1, frames 1586 and 1587).
            cx.set_counter("kong_drawn", drawn);
        }
    }

    /// The NMI sends the buffer at $0331 to the name table first thing
    /// in every frame, a Start pause's too, so the pose drawn last frame is
    /// the one on the screen: `kong_pose` takes `kong_drawn` (hammer clip
    /// 15, frame 1910: the hold drawn on the frame Start pauses shows
    /// through the pause).
    fn frame_start(&self, cx: &mut ExtCx<'_>) {
        if cx.has_param("pose_take_at")
            && let Some(d) = cx.counter("kong_drawn")
        {
            cx.set_counter("kong_pose", d);
        }
    }
}

/// Kong's pose ($EB54). The game draws Kong into the name table: $C815
/// puts the pose in the buffer the NMI sends, so a pose drawn on a frame
/// is on the screen the frame after. The counter `kong_drawn` is the pose
/// last drawn and `kong_pose` the one shown ([`STAND`] to [`BEAT_B`]),
/// for presentation (the name table isn't RAM, so no byte holds either).
/// The counters `kong_wait` ($44, a slow timer the model steps down on
/// each divider wrap) and `kong_thrown` ($0515, which a barrel let go
/// steps) are the game's own. With the throw timer at `pose_take_at` he
/// turns to take a barrel and the wait is `pose_take_wait`; at 0 he holds
/// it, the wait `pose_hold_wait`; a barrel let go makes the throw, clears
/// `kong_thrown` and sets the same wait; otherwise the wait at
/// `pose_beat_a1`, `pose_beat_b1`, `pose_beat_a2`, `pose_beat_b2` and
/// `pose_stand_at` draws a pose. Each pose drawn takes one off the wait at
/// once ($EBAB), the throw's excepted. Takes and gives the pose drawn, the
/// wait and `kong_thrown`.
fn pose(cx: &ExtCx<'_>, throw: i64, drawn: i64, wait: i64, thrown: i64) -> (i64, i64, i64) {
    let (mut pose, mut wait, mut thrown) = (drawn, wait, thrown);
    let mut show = |p: i64, wait: &mut i64| {
        pose = p;
        *wait -= 1;
    };
    if throw == p(cx, "pose_take_at") {
        wait = p(cx, "pose_take_wait");
        show(TAKE, &mut wait);
    } else if throw == 0 {
        wait = p(cx, "pose_hold_wait");
        show(HOLD, &mut wait);
    } else if thrown != 0 {
        pose = THROW;
        thrown = 0;
        wait = p(cx, "pose_hold_wait");
    } else if wait == p(cx, "pose_beat_a1") || wait == p(cx, "pose_beat_a2") {
        show(BEAT_A, &mut wait);
    } else if wait == p(cx, "pose_beat_b1") || wait == p(cx, "pose_beat_b2") {
        show(BEAT_B, &mut wait);
    } else if wait == p(cx, "pose_stand_at") {
        show(STAND, &mut wait);
    }
    (pose, wait.max(0), thrown)
}

/// The pose last drawn at a start: the counter `kong_drawn` as the start
/// loads it, which the state mapping reads from the name table and the
/// buffer the NMI sends (features `field.nametable`, `field.case.ram`),
/// else the pose [`start_pose`] works out from the timers.
fn drawn_at_start(cx: &ExtCx<'_>, throw: i64) -> i64 {
    cx.counter("kong_drawn")
        .filter(|d| (STAND..=BEAT_B).contains(d))
        .unwrap_or_else(|| start_pose(cx, throw))
}

/// Kong's pose at a start, which the game keeps only in the name table:
/// the one $EB54 last drew, read from the wait $44, which each pose sets.
/// Only the take sets it above `pose_hold_wait` (to `pose_take_wait`), so
/// a wait above that is the take if the throw timer has come down past
/// `pose_take_at` since, through a death too, and else the stage setup's
/// $27 with the setup's own picture of him standing. At or below it he
/// has held the barrel and thrown it once the drop timer is out, unless
/// the barrel dropped straight, which has no throw; lower waits say which
/// beat has been drawn.
fn start_pose(cx: &ExtCx<'_>, throw: i64) -> i64 {
    let wait = counter(cx, "kong_wait");
    // With no throws ($EB22) the wait alone says which pose was drawn
    // last: each beat takes one off it as it is drawn, so a wait just
    // under a beat's value is that beat, and above the first beat or
    // under the stand's he stands (the setup draws him standing).
    if cx.has_param("throws") && p(cx, "throws") == 0 {
        let below = |at: &str| wait < p(cx, at);
        return if !below("pose_beat_a1") || below("pose_stand_at") {
            STAND
        } else if !below("pose_beat_b1") {
            BEAT_A
        } else if !below("pose_beat_a2") {
            BEAT_B
        } else if !below("pose_beat_b2") {
            BEAT_A
        } else {
            BEAT_B
        };
    }
    // A smash stops $EB54, so a throw timer that passed `pose_take_at`
    // since the smash began took nothing: the smash is `smash_step` - 1
    // steps of 8 frames old at least (hammer clip 2 starts at step 4 with
    // the timer at 24, Kong still on his last beat).
    let smash = counter(cx, "smash_step");
    let took = smash == 0 || throw + 8 * (smash - 1) < p(cx, "pose_take_at");
    if wait > p(cx, "pose_hold_wait") {
        if throw <= p(cx, "pose_take_at") {
            TAKE
        } else {
            STAND
        }
    } else if throw > 0 && throw <= p(cx, "pose_take_at") && took {
        TAKE
    } else if throw == 0 {
        HOLD
    } else if wait >= p(cx, "pose_beat_a1") {
        // A straight drop ($C0 to $C2, slot 0) is let go without a throw.
        let straight = cx.kind("barrel").is_some_and(|k| {
            cx.world
                .bodies
                .iter()
                .any(|b| b.alive && b.kind == k && b.ext[0] >= STRAIGHT)
        });
        if counter(cx, "drop_timer") == 0 && counter(cx, "kong_thrown") == 0 && !straight {
            THROW
        } else {
            HOLD
        }
    } else if wait >= p(cx, "pose_beat_b1") {
        BEAT_A
    } else if wait >= p(cx, "pose_beat_a2") {
        BEAT_B
    } else if wait >= p(cx, "pose_beat_b2") {
        BEAT_A
    } else if wait >= p(cx, "pose_stand_at") {
        BEAT_B
    } else {
        STAND
    }
}

/// Kong's poses as `kong_pose` keeps them (the asset map's `kong_stand` to
/// `kong_beat_b`).
const STAND: i64 = 0;
const TAKE: i64 = 1;
const HOLD: i64 = 2;
const THROW: i64 = 3;
const BEAT_A: i64 = 4;
const BEAT_B: i64 = 5;

/// Whether the thrower has let the held barrel go: the drop timer is out,
/// and didn't run out this frame. Without a thrower, after `held_frames`.
fn let_go(cx: &ExtCx<'_>, t: i64) -> bool {
    let kong = cx.kind("kong");
    match cx
        .world
        .bodies
        .iter()
        .find(|b| b.alive && Some(b.kind) == kong)
    {
        Some(k) => cx.counter("drop_timer").unwrap_or(0) == 0 && k.ext[3] == 0,
        None => t >= p(cx, "held_frames"),
    }
}

struct Barrel;

/// How far above the girder's surface a barrel's sprite stands: `stand`,
/// or on girder `g` its `stand<g>` (the bottom girder's is 2 more, as the
/// user's play has it).
fn stand_of(cx: &ExtCx<'_>) -> i64 {
    let g = format!("stand{}", cx.me.ext[2]);
    if cx.has_param(&g) {
        p(cx, &g)
    } else {
        p(cx, "stand")
    }
}

/// The standing row under column `x` within `reach` pixels of `row`, the
/// nearest first: the top of a cell with a surface plus its surface.
fn surface_near(cx: &ExtCx<'_>, x: i64, row: i64, reach: i64) -> Option<i64> {
    let t = cx.world.tiles(cx.spec);
    let mut best: Option<i64> = None;
    for d in -reach..=reach {
        let y = row + d;
        if let Some(s) = t.level.surface(x, y) {
            let top = t.cell_top(y) + s;
            if (top - row).abs() <= reach
                && best.is_none_or(|b| (top - row).abs() < (b - row).abs())
            {
                best = Some(top);
            }
        }
    }
    best
}

/// The girder a standing row is on, counting girders below it: the bottom
/// girder is 1. A girder is a run of surfaces 8 or more pixels apart.
fn girder_at(cx: &ExtCx<'_>, x: i64, row: i64) -> i64 {
    let t = cx.world.tiles(cx.spec);
    let bottom = t.level.pixels()[1];
    let mut n = 1;
    let mut y = row + 8;
    while y < bottom {
        if t.level.surface(x, y).is_some() {
            n += 1;
            y = t.cell_top(y) + 16;
        } else {
            y += 1;
        }
    }
    n
}

fn dir_of(girder: i64) -> i64 {
    if girder % 2 == 0 { 1 } else { -1 }
}

/// The player's x, whole pixels, or 0 with no player.
pub(crate) fn player_x(cx: &ExtCx<'_>) -> i64 {
    player(cx).map_or(0, |m| m.x.floor())
}

/// A random byte the game reads at `site`, the register's byte at
/// `$18 + byte`: the original's from a random draws script when the run
/// has one, else the counter `random`'s, which the idle loop steps
/// ([`idle::draw`]).
pub(crate) fn draw(cx: &mut ExtCx<'_>, site: &str, byte: u32) -> u8 {
    idle::draw(cx, site, byte)
}

pub(crate) fn player<'a>(cx: &'a ExtCx<'_>) -> Option<&'a Body> {
    cx.world
        .bodies
        .iter()
        .find(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
}

/// The player's girder, from the standing row under it.
fn player_girder(cx: &ExtCx<'_>, size: i64, feet: i64) -> Option<i64> {
    let m = player(cx)?;
    let x = m.x.floor() + size / 2;
    // In a jump, its air and its landing hold ($96 = 4), the game's $59 is
    // the girder he took off from: only the walk and a climb's end write
    // it (dk-tas-zeros-2, original frame 2284: Mario jumps up at y 122
    // with $59 at 3, and a barrel landing from girder 4 isn't at or below
    // him, so $DCB3 isn't reached). The takeoff word holds the takeoff's
    // y until a drop of 16 marks it.
    use gp_sim::systems::{FATAL_MARK, LAND_HOLD_WORD, TAKEOFF_Y_WORD};
    let jumping = (!m.on_ground && !m.climbing && m.air == u32::MAX) || m.ext[LAND_HOLD_WORD] > 0;
    let base = if jumping && m.ext[TAKEOFF_Y_WORD] != FATAL_MARK {
        m.ext[TAKEOFF_Y_WORD]
    } else {
        m.y.floor()
    };
    // Standing or in a jump, $59 is $E016's girder for his y: the walk and
    // the climb's end store it from his sprite's y + 8 against the stage's
    // girder heights ($C0BC by $C493, the barrel's table `heights`), 1 more
    // for each height below that, and nothing reads his x. Past a girder's
    // end, at the screen's edge, the surface under his middle is the
    // girder below (dk.walk-sweep.001, original frame 1676: Mario at x 229
    // on girder 3 and a barrel landing on girder 2 rolls off). A table
    // without the list's end mark $FF (a stage that leaves it empty) isn't
    // one $E016 could read, and the surfaces answer.
    if !m.climbing
        && let Some(barrel) = cx.kind("barrel")
        && let Some(heights) = cx.spec.kinds[barrel].ext_tables.get("heights")
        && heights.contains(&0xFF)
    {
        // His sprite is `player_sprite_dy` above him; the callers add 8.
        let dy = 8 - cx.spec.kinds[barrel]
            .ext_params
            .get("player_sprite_dy")
            .map_or(0, |v| v.floor());
        let y = (base + dy).rem_euclid(256);
        let above = heights.iter().take_while(|&&h| h != 0xFF && h > y).count() as i64;
        // The list's end mark $FF counts as the girder past the last.
        return Some(above + 1);
    }
    let feet = base + feet;
    // Off a girder (climbing), the game's $59 is still a girder: the one
    // below him, found here as the nearest surface under his feet.
    let row = surface_near(cx, x, feet, 16)
        .or_else(|| (feet..feet + 64).find_map(|y| surface_near(cx, x, y, 0)))?;
    Some(girder_at(cx, x, row))
}

/// The game's girder counts ($E166) that the crowd drop, the ladder test
/// and the roll off read, taken where the barrel loop starts ($DA16),
/// before any barrel moves this frame: the counters `girder_count1` to
/// `girder_count5` ($7F to $83) are the barrels whose girder ($68,X) is
/// that one, plus one for Mario on it ($59) and one more while he holds
/// the hammer. Two barrels on one ladder top both go down in the same
/// frame, as neither sees the other's move (dk.untaken-crowded.002,
/// original frame 1674). A barrel that lands from
/// a fall keeps the girder it fell from until its first move after
/// ($DCE7 and $DD49 set $68,X from its y), where the model's word 2 has
/// the new girder from the landing.
fn crowd_counts(cx: &mut ExtCx<'_>) {
    if cx.counter("girder_count3").is_none() {
        return;
    }
    let Some(barrel) = cx.kind("barrel") else {
        return;
    };
    let girder = |b: &Body| {
        let first_move = match b.ext[0] {
            OFF => b.ext[1] >= 2 || (b.ext[1] == 1 && b.time.is_multiple_of(2)),
            BOUNCE => b.ext[1] >= 2 || (b.ext[1] == 1 && b.time % 4 != 3),
            _ => true,
        };
        if first_move { b.ext[2] } else { b.ext[2] + 1 }
    };
    let feet = cx.spec.kinds[barrel]
        .ext_params
        .get("player_feet")
        .map_or(0, |v| v.floor());
    let mario = player_girder(cx, 16, feet);
    let held = cx.kind("hammer").is_some_and(|k| {
        cx.world
            .bodies
            .iter()
            .any(|b| b.kind == k && b.alive && b.ext[0] == 1)
    });
    for g in 1..=5 {
        let name = format!("girder_count{g}");
        if cx.counter(&name).is_none() {
            continue;
        }
        let mut n = cx
            .world
            .bodies
            .iter()
            .filter(|b| b.alive && !b.waiting && b.kind == barrel && girder(b) == g)
            .count() as i64;
        if mario == Some(g) {
            n += 1 + i64::from(held);
        }
        cx.set_counter(&name, n);
    }
}

/// The game's $7E for a girder: the barrels on it, plus one for the player
/// standing there, two while he holds the hammer.
fn occupancy(cx: &ExtCx<'_>, girder: i64, me: usize) -> i64 {
    // As the loop started ([`crowd_counts`]), where the model keeps it.
    if let Some(n) = cx.counter(&format!("girder_count{girder}")) {
        return n;
    }
    let barrel = cx.spec.kinds[cx.me.kind].name.clone();
    let mut n = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .filter(|(i, b)| {
            *i != me
                && b.alive
                && !b.waiting
                && cx.spec.kinds[b.kind].name == barrel
                && b.ext[2] == girder
        })
        .count() as i64;
    let feet = p(cx, "player_feet");
    if player_girder(cx, 16, feet) == Some(girder) {
        n += 1;
        // Holding the hammer ($96 = 10) he counts twice ($E18F-$E197).
        let held = cx.kind("hammer").is_some_and(|k| {
            cx.world
                .bodies
                .iter()
                .any(|b| b.kind == k && b.alive && b.ext[0] == 1)
        });
        if held {
            n += 1;
        }
    }
    n
}

impl Barrel {
    /// Whether the barrel let go now drops straight ($C0, $DAB2-$DAFF):
    /// one in slot 0, while the oil drum is unlit, or once lit, when the
    /// slow timer `straight_wait` ($43) is out. Either way the timer
    /// restarts at `straight_wait` (C44D[d]).
    fn straight(cx: &mut ExtCx<'_>) -> bool {
        if cx.slot() != Some(0) {
            return false;
        }
        let ready = match cx.counter("drum") {
            Some(0) => true,
            Some(_) => cx.counter("straight_wait") == Some(0),
            None => false,
        };
        if ready {
            // $DAFD runs before the frame's divider wrap ($F4AC).
            let wait = by_difficulty(cx, "straight_wait");
            set_slow_timer(cx, "straight_wait", wait);
            // Its way down ($0421,X, $DACD-$DAF4): 1, straight, while the
            // drum is unlit; once lit, 3 (zigzag) after a 1 and 1 after
            // anything else. The slot keeps it, with the zigzag's side.
            let (way, side) = Self::slot_way(cx);
            let way = if cx.counter("drum") == Some(0) || way != 1 {
                1
            } else {
                3
            };
            cx.set_slot_word(way | side << 8);
        }
        ready
    }

    /// The straight drop's way ($0421,X) and its zigzag's side ($0417,X)
    /// as the barrel's slot keeps them in its word: way + 256 * side.
    fn slot_way(cx: &ExtCx<'_>) -> (i64, i64) {
        let w = cx.slot().map_or(0, |s| cx.world.slots[s].word);
        (w & 0xFF, (w >> 8) & 1)
    }

    /// A step of the straight drop to `ny` ($DE1A-$DE53): with way 3 the
    /// barrel moves 2 pixels across, right on side 0 and left on side 1,
    /// on each step to an odd y; with way 2, a pixel right each step.
    fn straight_across(cx: &mut ExtCx<'_>, way: i64, side: i64, ny: i64) {
        let dx = match way {
            2 => 1,
            3 if ny % 2 == 1 => {
                if side == 0 {
                    2
                } else {
                    -2
                }
            }
            _ => 0,
        };
        cx.me.x = cx.me.x.add(Fx::from_int(dx));
    }

    /// The foot of the ladder whose top is at `x` on girder `g`: the y a
    /// barrel going down it stops at.
    fn ladder_foot(cx: &ExtCx<'_>, g: i64, x: i64) -> Option<i64> {
        Self::ladder_tops(cx, g)
            .into_iter()
            .find_map(|(top, foot)| (top == Some(x)).then_some(foot).flatten())
    }

    /// The ladders down from girder `g`: each one's top x and foot y, the
    /// first two from the lifted tables (C177 and C172, C181 and C17C) and
    /// a third from `ladder_c<g>` and `ladder_cfoot<g>` (the code's own
    /// compares at $E0D1 and $E0DD on girders 4 and 5).
    fn ladder_tops(cx: &ExtCx<'_>, g: i64) -> [(Option<i64>, Option<i64>); 3] {
        use barrel_tables::*;
        let lifted = |top: u16, foot: u16| {
            let i = u16::try_from(g - 2).ok().filter(|i| *i < 5);
            (
                i.map(|i| rom_at(cx, top + i)),
                i.map(|i| rom_at(cx, foot + i)),
            )
        };
        let c = |name: String| cx.has_param(&name).then(|| p(cx, &name));
        [
            lifted(TOP_A, FOOT_A),
            lifted(TOP_B, FOOT_B),
            (c(format!("ladder_c{g}")), c(format!("ladder_cfoot{g}"))),
        ]
    }

    /// The next girder a straight drop of way `way` stops at, at or below
    /// `y`: the way's `straight_stops` from C1D5, C1DB or C1E1 (the
    /// pointers at C03C + $34, $36, $38 for ways 1 to 3, read by $E112);
    /// the last is the bottom.
    fn next_stop(cx: &ExtCx<'_>, y: i64, way: i64) -> (i64, bool) {
        use barrel_tables::*;
        let n = p(cx, "straight_stops");
        let from = STOPS + STOPS_STEP * (way.clamp(1, 3) as u16 - 1);
        let mut last = y;
        for k in 0..n {
            let s = rom_at(cx, from + k as u16);
            last = s;
            if s >= y {
                return (s, k + 1 == n);
            }
        }
        (last, true)
    }

    /// The slot's sprite tile ($72,X) and its rolling count ($040D,X), held
    /// in the counters `barrel_tile<slot>` and `barrel_anim<slot>`.
    fn tile_names(cx: &ExtCx<'_>) -> Option<(String, String)> {
        let s = cx.slot()?;
        let names = (format!("barrel_tile{s}"), format!("barrel_anim{s}"));
        cx.counter(&names.0).is_some().then_some(names)
    }

    fn set_tile(cx: &mut ExtCx<'_>, tile: i64) {
        if let Some((t, _)) = Self::tile_names(cx) {
            cx.set_counter(&t, tile);
        }
    }

    /// $DBEE: the rolling count steps, and on reaching 6 restarts and turns
    /// the tile a step within $80-$8C, up on an odd girder and down on an
    /// even one.
    fn roll_tile(cx: &mut ExtCx<'_>, g: i64) {
        let Some((t, a)) = Self::tile_names(cx) else {
            return;
        };
        let n = cx.counter(&a).unwrap_or(0) + 1;
        if n < 6 {
            cx.set_counter(&a, n);
            return;
        }
        cx.set_counter(&a, 0);
        let tile = cx.counter(&t).unwrap_or(0);
        let next = if g & 1 == 1 { tile + 4 } else { tile - 4 };
        let next = if (0x80..0x90).contains(&next) {
            next
        } else if g & 1 == 1 {
            0x80
        } else {
            0x8C
        };
        cx.set_counter(&t, next);
    }

    /// $DC43 and $DE5D: going down, the tile swaps between $90 and $94.
    fn down_tile(cx: &mut ExtCx<'_>) {
        if let Some((t, _)) = Self::tile_names(cx) {
            let tile = cx.counter(&t).unwrap_or(0);
            cx.set_counter(&t, if tile == 0x90 { 0x94 } else { 0x90 });
        }
    }

    fn set(cx: &mut ExtCx<'_>, state: i64) {
        cx.me.ext[0] = state;
        cx.me.ext[1] = 0;
        cx.me.ext[3] = 0;
    }

    /// Moves across by `dx` and onto the girder under its middle; false if
    /// there's no girder under it (a girder's end).
    fn roll(cx: &mut ExtCx<'_>, dx: i64) -> bool {
        let size = p(cx, "size");
        let stand = stand_of(cx);
        let x = cx.me.x.floor() + dx;
        let bottom = cx.me.y.floor() + stand;
        cx.me.x = Fx::from_int(x);
        // The game rolls a barrel to x `edge_left` or `edge_right` and it
        // falls from there, whatever the girder under it (the user's play:
        // 32 falls, all at x 19 or 219). On the way it follows the girder.
        if (dx < 0 && x <= p(cx, "edge_left")) || (dx > 0 && x >= p(cx, "edge_right")) {
            return false;
        }
        // Down the slope: the game adds a pixel to y where x reaches a
        // multiple of 24 within `step_lo<g>` to `step_hi<g>`, whichever way
        // it rolls (the user's play and clear: 32 steps on girders 2 to 6,
        // 5 on the bottom one). Elsewhere it follows the level's surface.
        let g = cx.me.ext[2];
        let (lo, hi) = (format!("step_lo{g}"), format!("step_hi{g}"));
        if cx.has_param(&lo) {
            if x.rem_euclid(24) == 0 && (p(cx, &lo)..=p(cx, &hi)).contains(&x) {
                cx.me.y = cx.me.y.add(Fx::from_int(1));
            }
        } else if let Some(row) = surface_near(cx, x + size / 2, bottom, 3) {
            cx.me.y = Fx::from_int(row - stand);
        }
        true
    }

    /// $DF07: a pixel down on its counter's even counts ($DFE4 with mask
    /// $55), the sprite's tile turning between $90 and $94 while $72,X
    /// keeps the rolling one; rolling again on the girder below once y
    /// reaches `barrel_drop` ($C0), which goes back to 0. Word 1 holds the
    /// picture for the presentation: -5 for $90, -6 for $94, 0 before the
    /// first move. `until` is `barrel_drop` as this frame's turn reads it.
    fn drop_step(cx: &mut ExtCx<'_>, until: i64) {
        let pic = cx.me.ext[1] - 1;
        cx.me.ext[1] = pic;
        if cx.me.time.is_multiple_of(2) {
            let y = cx.me.y.floor() + 1;
            cx.me.y = Fx::from_int(y);
            let drawn = if pic == -5 { -6 } else { -5 };
            cx.me.ext[1] = drawn;
            if until <= y {
                Self::set(cx, ROLL);
                cx.set_counter("barrel_drop", 0);
                // $DF29 draws the dropping tile before $DF39 sets the
                // state to rolling, so the landing frame still shows it
                // (dk.crowded-top.003, frame 921, original frame 2033).
                // Presentation only: `turn` takes the word back to the
                // roll's count on the next frame.
                cx.me.ext[1] = drawn;
            }
        }
    }

    /// Whether a barrel in a lower slot than this one, whose hook runs
    /// after this one's (the model runs hooks in body order, the game's
    /// loop $DA16 in slot order), rolls on this frame to $DF40, so that its
    /// search, not this barrel's own after its move, sends this one down
    /// (dk.crowded-top.003, original frame 1984: slot 1 rolling on girder 5
    /// sends slot 2 down from girder 3 before slot 2 moves). Such a barrel
    /// reaches $DF40 unless its move takes it past the girder's end
    /// ($DBB1) or onto a ladder top ($DB73), where its draw ($DB7B) isn't
    /// known yet: there the model leaves this barrel to roll and test
    /// itself, as before. A barrel let go this frame ($81) isn't looked at.
    fn lower_slot_drops_first(cx: &ExtCx<'_>) -> bool {
        let Some(mine) = cx.slot() else {
            return false;
        };
        let me = cx.id;
        cx.world.slots.iter().take(mine).any(|s| {
            let Some(id) = usize::try_from(s.body).ok().and_then(|b| b.checked_sub(1)) else {
                return false;
            };
            let Some(b) = cx.world.bodies.get(id).filter(|_| id > me) else {
                return false;
            };
            let g = b.ext[2];
            if !b.alive || b.kind != cx.me.kind || b.ext[0] != ROLL || g == 0 {
                return false;
            }
            let dx = dir_of(g);
            let x = b.x.floor() + dx;
            let past = (dx < 0 && x <= p(cx, "edge_left")) || (dx > 0 && x >= p(cx, "edge_right"));
            let top = g >= 2 && Self::ladder_tops(cx, g).iter().any(|(t, _)| *t == Some(x));
            !past && !top
        })
    }

    /// At a ladder top on its girder: whether it goes down. The game draws
    /// `($19 & $88) == 0`, 1 in 4; the girder below must hold fewer than
    /// `crowd`; and it doesn't go down onto a climbing player 1 to 15
    /// pixels below it.
    fn takes_ladder(cx: &mut ExtCx<'_>) -> bool {
        let g = cx.me.ext[2];
        if g < 2 {
            return false;
        }
        let x = cx.me.x.floor();
        // C177/C181 by girder, and a third top on girders 4 and 5 ($E0D1,
        // $E0DD: x $C4 and $B4).
        if !Self::ladder_tops(cx, g)
            .iter()
            .any(|(top, _)| *top == Some(x))
        {
            return false;
        }
        // The game ANDs its random byte with C448's mask ($88, bits 7 and
        // 3: 1 in 4) at $DB7B and goes down when the result is 0.
        let mask = p(cx, "ladder_mask") as u8;
        let draw = crate::draw(cx, "ladder", 1) & mask == 0;
        if !draw || occupancy(cx, g - 1, cx.id) >= p(cx, "crowd") {
            return false;
        }
        if let Some(m) = player(cx)
            && m.climbing
        {
            // The game compares sprite tops ($0200,X with $0200): Mario's
            // is `player_sprite_dy` above the model's y ($47).
            let (by, my) = (cx.me.y.floor(), m.y.floor() - p(cx, "player_sprite_dy"));
            if by < my && by + 15 >= my {
                return false;
            }
        }
        true
    }

    /// $DF40, which each barrel that rolls on runs after its move: while
    /// Mario holds the hammer and his girder ($59: 3, else taken as 5)
    /// counts at least `crowd_drop` ([`crowd_counts`]: its barrels, and
    /// Mario twice),
    /// the barrel of the lowest slot rolling on that girder, this one or
    /// another, drops through it ($40) to the girder below, which it is
    /// on from now ($DF93). Its drop ends at `barrel_drop` ($C0): its y,
    /// plus 21, plus 2 for each 24 px its x is past 25 ($DFA8-$DFC0). One
    /// drop at a time: none starts while `barrel_drop` is set.
    ///
    /// On girder 5 the game's slot loop ends on its value being 10 instead
    /// of its index ($DF8A compares A), so with no barrel to drop it goes
    /// on through the zero page until a byte reads 10, and a byte pair
    /// there can match. The model looks at the barrel slots only.
    fn crowd_drop(cx: &mut ExtCx<'_>) {
        if let Some((id, b, g)) = Self::drop_target(cx) {
            Self::send_down(cx, id, b, g);
        }
    }

    /// [`Self::crowd_drop`]'s test: the barrel $DF40 would send down this
    /// frame (its body id and body) and the crowded girder, or None.
    fn drop_target(cx: &ExtCx<'_>) -> Option<(usize, Body, i64)> {
        if !cx.has_param("crowd_drop") || counter(cx, "barrel_drop") != 0 {
            return None;
        }
        let held = cx.kind("hammer").is_some_and(|k| {
            cx.world
                .bodies
                .iter()
                .any(|b| b.kind == k && b.alive && b.ext[0] == 1)
        });
        if !held {
            return None;
        }
        let feet = p(cx, "player_feet");
        let g = if player_girder(cx, 16, feet) == Some(3) {
            3
        } else {
            5
        };
        if counter(cx, &format!("girder_count{g}")) < p(cx, "crowd_drop") {
            return None;
        }
        let me = cx.id;
        let mine = cx.slot();
        cx.world.slots.iter().enumerate().find_map(|(k, s)| {
            let id = usize::try_from(s.body).ok()?.checked_sub(1)?;
            let b = if id == me {
                &*cx.me
            } else {
                cx.world.bodies.get(id)?
            };
            // A barrel in a later slot whose hook has already run (the
            // model runs hooks in body order, the game's loop in slot
            // order) and which only began rolling on its turn this frame
            // (word 1 is 0 after `set`, or a drop's last picture below 0
            // after a drop's landing, `drop_step`) wasn't rolling yet when
            // the game's loop was at this slot (dk.crowded-top.004,
            // original frame 2029: slot 2's bounce ends after slots 0 and 1
            // have passed it, and slot 0 sends it down on 2030).
            let later_new = id < me && mine.is_some_and(|m| k > m) && b.ext[1] <= 0;
            (b.alive && b.kind == cx.me.kind && b.ext[0] == ROLL && b.ext[2] == g && !later_new)
                .then_some((id, *b, g))
        })
    }

    /// $DF8F-$DFC0: barrel `id` (this one or another) goes down through
    /// girder `g`.
    /// Returns where its drop ends.
    fn send_down(cx: &mut ExtCx<'_>, id: usize, mut b: Body, g: i64) -> i64 {
        let me = cx.id;
        let (x, y) = (b.x.floor(), b.y.floor());
        let mut v = 25;
        let mut n = 0;
        while v < x && n < 16 {
            v = (v + 24) & 0xFF;
            n += 1;
        }
        let until = (2 * n + 0x15 + y) & 0xFF;
        cx.set_counter("barrel_drop", until);
        b.ext[0] = DROP;
        b.ext[1] = 0;
        b.ext[3] = 0;
        b.ext[2] = g - 1;
        if id == me {
            *cx.me = b;
        } else {
            cx.set_body(id, b);
        }
        until
    }

    /// After a fall lands: whether it rolls off instead. A 1 in 2 draw if
    /// its girder is at or below the player's, or certainly if the girder
    /// below is crowded.
    fn rolls_off(cx: &mut ExtCx<'_>) -> bool {
        let g = cx.me.ext[2];
        let feet = p(cx, "player_feet");
        let at_or_below = player_girder(cx, 16, feet).is_some_and(|m| g <= m);
        // At $DCB3 the game tests its random byte's bit 0 (1 in 2), only
        // for a barrel at or below the player's girder.
        let mask = p(cx, "off_mask") as u8;
        if at_or_below && crate::draw(cx, "roll_off", 1) & mask != 0 {
            return true;
        }
        g > 1 && occupancy(cx, g - 1, cx.id) >= p(cx, "crowd")
    }
}

impl Barrel {
    /// A frame of a smash ($EE0C), for the barrel being smashed while the
    /// smash stops everything else: its frame counter steps ($DFE8), and
    /// on 7 or 15 the smash steps ($BF), or after its 11th step the barrel
    /// goes, `smash_step` is 0 again and `smash.done` fires (the score's
    /// 500, $EE51-$EE69).
    /// The picture of a barrel breaking at smash step `step`, for its
    /// sprite: word 1 (the frames in its state, which nothing reads once
    /// it is hit) holds minus the break-up picture, 1 to 4 for tiles $30,
    /// $34, $38 and $3C ($EE0C draws $30 and $34 in turn from step 2, $38
    /// at step 8 and $3C from 9). Step 1 is still the barrel.
    fn smash_picture(cx: &mut ExtCx<'_>, step: i64) {
        let pic = match step {
            2..=7 => 1 + step % 2,
            8 => 3,
            9.. => 4,
            _ => return,
        };
        cx.me.ext[1] = -pic;
    }

    fn smash_turn(cx: &mut ExtCx<'_>) {
        let step = counter(cx, "smash_step");
        let Some(mine) = cx.slot() else {
            return;
        };
        if step == 0 || counter(cx, "barrel_loop") != mine as i64 {
            return;
        }
        cx.me.time = (cx.me.time + 1) % 16;
        if cx.me.time % 8 != 7 {
            Self::smash_picture(cx, step);
            return;
        }
        if step < 11 {
            // The first step asks for the smash's sound again ($EE34).
            if step == 1 {
                cx.set_counter("sound_ff", 2);
            }
            cx.set_counter("smash_step", step + 1);
            Self::smash_picture(cx, step + 1);
            return;
        }
        cx.set_counter("smash_step", 0);
        // The score's 500 takes a points sprite ($EE64 calls $CFC6, X = 2).
        award_points(cx);
        cx.me.alive = false;
        let id = cx.id;
        cx.remove(id);
        if let Some(done) = cx.event("smash.done") {
            cx.fire(done, Some(id));
        }
    }
}

impl Ext for Barrel {
    fn step(&self, cx: &mut ExtCx<'_>) {
        // While a state stops the barrels (a smash, a death) only the
        // smashed barrel's smash runs (`hook_through_stops`).
        if cx.system_stopped {
            Self::smash_turn(cx);
            return;
        }
        Self::turn(cx);
        // After its move, as the game tests the hammer after the barrels
        // have moved ($EC29 runs after the barrel loop).
        smash_test(cx);
    }
}

impl Barrel {
    /// The barrel's own routine for one frame.
    fn turn(cx: &mut ExtCx<'_>) {
        if cx.me.ext[0] == 0 {
            Self::set(cx, HELD);
            cx.me.ext[2] = p(cx, "top_girder");
        }
        // A drop's landing left its last picture in word 1 (`drop_step`);
        // rolling counts from 0, as `set` leaves it.
        if cx.me.ext[0] == ROLL && cx.me.ext[1] < 0 {
            cx.me.ext[1] = 0;
        }
        cx.me.ext[1] += 1;
        // The barrel's own frame counter ($8A,X, 0 to 15), which a ladder
        // descent steps on the even counts of.
        cx.me.time = (cx.me.time + 1) % 16;
        let t = cx.me.ext[1];
        let g = cx.me.ext[2];
        match cx.me.ext[0] {
            HELD => {
                if let_go(cx, t) {
                    cx.me.time = 0;
                    if Self::straight(cx) {
                        Self::set(cx, STRAIGHT);
                    } else {
                        Self::set(cx, LET_GO);
                    }
                }
            }
            STRAIGHT => {
                // Down the column under his hands a pixel a frame, and a
                // pixel more where the new y is in one of the
                // `straight_fast_rows` ranges from C41D to C420, exclusive
                // (past a girder, $DE86); a 3-frame stop at
                // each girder.
                let y = cx.me.y.floor();
                let (way, side) = Self::slot_way(cx);
                let (stop, bottom) = Self::next_stop(cx, y, way.max(1));
                // The tile swaps every frame of the drop and its stops
                // ($DE5D), the frame it reaches a stop too, but not the
                // frame it reaches the bottom (walk clip 1, 4741).
                if y < stop || !bottom {
                    Self::down_tile(cx);
                }
                if y >= stop {
                    Self::set(cx, if bottom { LANDED } else { STOPPED });
                    if !bottom {
                        // A stop at a girder sets the frame counter to 1
                        // ($DDCF) after this frame's step, so it reads 2.
                        cx.me.time = 2;
                    }
                    if bottom {
                        // Where it landed ($042B, $DE7B).
                        cx.me.ext[3] = cx.me.x.floor();
                    }
                } else {
                    let mut ny = y + 1;
                    Self::straight_across(cx, way, side, ny);
                    // The fast rows are way 1's only ($DE86).
                    if way == 1 {
                        use barrel_tables::*;
                        let fast = (0..p(cx, "straight_fast_rows") as u16).any(|k| {
                            (rom_at(cx, FAST_LO + k)..rom_at(cx, FAST_HI + k)).contains(&ny)
                        });
                        if fast {
                            ny += 1;
                        }
                    }
                    cx.me.y = Fx::from_int(ny.min(stop));
                }
            }
            STOPPED => {
                Self::down_tile(cx);
                if t >= 3 {
                    // On again ($DDF5): way 3 swaps its zigzag's side.
                    Self::set(cx, STRAIGHT);
                    let (way, mut side) = Self::slot_way(cx);
                    if way == 3 {
                        side ^= 1;
                        cx.set_slot_word(way | side << 8);
                    }
                    let ny = cx.me.y.floor() + 1;
                    cx.me.y = Fx::from_int(ny);
                    Self::straight_across(cx, way, side, ny);
                }
            }
            LANDED => {
                // Toward the drum a pixel a frame from where it landed, L
                // (`ext[3]`, $DEA5): up a pixel at L-1 and L-2; way 1 rolls
                // from L-3 (staying at L-2); a zigzag goes on, down a pixel
                // at L-11 and L-12, and rolls from L-13 (staying at L-12).
                if t == 1 {
                    // $DEFB: the rolling tile from the frame after it lands.
                    Self::set_tile(cx, 0x84);
                }
                let (way, _) = Self::slot_way(cx);
                let land = cx.me.ext[3];
                let x = cx.me.x.floor() - 1;
                let d = land - x;
                if d == 3 && way.max(1) == 1 || d == 13 {
                    Self::set(cx, ROLL);
                    cx.me.ext[2] = 1;
                    let mut w = 0;
                    if let Some(s) = cx.slot() {
                        w = cx.world.slots[s].word & 0xFF;
                    }
                    cx.set_slot_word(w);
                } else {
                    cx.me.x = Fx::from_int(x);
                    if d == 1 || d == 2 {
                        cx.me.y = cx.me.y.sub(Fx::from_int(1));
                    } else if d == 11 || d == 12 {
                        cx.me.y = cx.me.y.add(Fx::from_int(1));
                    }
                }
            }
            LET_GO => {
                // $81: a frame in his hands, then at the girder, then
                // rolling. Each frame it stays steps Kong's `kong_thrown`
                // ($DB1B), which his throw pose clears; the frame its
                // count turns it to rolling ($DB00: $DFE4 with mask $55)
                // doesn't.
                if t < p(cx, "let_go_frames")
                    && let Some(n) = cx.counter("kong_thrown")
                {
                    cx.set_counter("kong_thrown", (n + 1) & 0xFF);
                }
                if t == 1 {
                    cx.me.x = cx.param("let_go_x");
                    cx.me.y = cx.param("let_go_y");
                }
                if t >= p(cx, "let_go_frames") {
                    Self::set(cx, ROLL);
                    // $DB27.
                    Self::set_tile(cx, 0x84);
                }
            }
            ROLL => 'roll: {
                // A barrel's first roll gives it its girder ($68): the top
                // one for a barrel let go, the bottom after a straight drop.
                let g = if g == 0 {
                    let size = p(cx, "size");
                    let stand = stand_of(cx);
                    let x = cx.me.x.floor() + size / 2;
                    let row = surface_near(cx, x, cx.me.y.floor() + stand, 3)
                        .unwrap_or(cx.me.y.floor() + stand);
                    cx.me.ext[2] = girder_at(cx, x, row);
                    cx.me.ext[2]
                } else {
                    g
                };
                // Sent down by a lower slot's $DF40 before its own turn: it
                // doesn't move, and runs the drop's first frame ($DF07) as
                // the loop reaches its slot.
                if let Some((id, b, crowded)) = Self::drop_target(cx)
                    && id == cx.id
                    && Self::lower_slot_drops_first(cx)
                {
                    let until = Self::send_down(cx, id, b, crowded);
                    cx.me.ext[1] = 1;
                    Self::drop_step(cx, until);
                    break 'roll;
                }
                // It moves, then checks for a ladder top where it now is:
                // the state and girder change on the frame it arrives
                // ($DB6E-$DBAB; DEC $68,X at $DBA9).
                let rolled = Self::roll(cx, dir_of(g));
                Self::roll_tile(cx, g);
                if g <= 1 && cx.me.x.floor() <= p(cx, "drum_x") {
                    // Into the oil drum, which lights ($DBCD; the drum
                    // routine holds it at 2 for 16 frames).
                    if cx.counter("drum").is_some() {
                        cx.set_counter("drum", 2);
                        // The timers ($F4AC) take a frame off at the
                        // frame's end, but not on the pause's first frame.
                        let step = i64::from(!cx.counters_stopped);
                        cx.set_counter("drum_timer", p(cx, "drum_frames") - step);
                    }
                    // The drum routine draws its flame from the first
                    // picture again ($E19A, `flame_a`).
                    if let (Some(_), Some(drum)) = (cx.counter("drum_flame"), cx.kind("oil_drum")) {
                        let a = cx.spec.kinds[drum]
                            .ext_params
                            .get("flame_a")
                            .map_or(0, |v| v.floor());
                        cx.set_counter("drum_flame", a);
                    }
                    // And asks for pulse 2's effect ($DBE4).
                    if cx.counter("sound_fe").is_some() {
                        cx.set_counter("sound_fe", 0x80);
                    }
                    cx.me.alive = false;
                    let id = cx.id;
                    cx.remove(id);
                    return;
                }
                if !rolled {
                    // Off the left end its first pixel down is in the same
                    // frame; off the right end it starts down the next.
                    Self::set(cx, FALL);
                    if dir_of(g) < 0 {
                        cx.me.y = cx.me.y.add(Fx::from_int(1));
                        cx.me.ext[3] = 1;
                    }
                } else if Self::takes_ladder(cx) {
                    Self::set(cx, LADDER);
                    cx.me.ext[2] = g - 1;
                } else {
                    Self::crowd_drop(cx);
                }
            }
            DROP => {
                let until = counter(cx, "barrel_drop");
                Self::drop_step(cx, until);
            }
            LADDER => {
                // A pixel down on its counter's even counts ($DC30), until y
                // is the ladder's foot ($A3,X, set at the top from C172,
                // C17C or $E0D1/$E0DD: [`Barrel::ladder_tops`]).
                if cx.me.time.is_multiple_of(2) {
                    cx.me.y = cx.me.y.add(Fx::from_int(1));
                    Self::down_tile(cx);
                }
                let foot = Self::ladder_foot(cx, g + 1, cx.me.x.floor());
                if foot.is_some_and(|f| cx.me.y.floor() >= f) {
                    Self::set(cx, ROLL);
                }
            }
            FALL => {
                // A pixel down a frame, and out a pixel when the new y is
                // odd ($DC77-$DC8E: INC $01, then AND #1). The y's parity,
                // not the pixels counted, so a start in a fall steps on
                // the game's frames.
                Self::roll_tile(cx, g);
                cx.me.y = cx.me.y.add(Fx::from_int(1));
                cx.me.ext[3] += 1;
                if cx.me.y.floor() % 2 == 1 {
                    cx.me.x = cx.me.x.add(Fx::from_int(dir_of(g)));
                }
                let size = p(cx, "size");
                let stand = stand_of(cx);
                let bottom = cx.me.y.floor() + stand;
                if cx.me.ext[3] >= 8
                    && let Some(row) = surface_near(cx, cx.me.x.floor() + size / 2, bottom, 0)
                {
                    cx.me.y = Fx::from_int(row - stand);
                    // The roll-off test reads the girder it fell from
                    // ($DCB3: $68 drops a frame later, $E130 compares it).
                    let off = Self::rolls_off(cx);
                    cx.me.ext[2] = g - 1;
                    Self::set(cx, if off { OFF } else { BOUNCE });
                }
                if cx.me.y.floor() > p(cx, "below") {
                    cx.me.alive = false;
                    let id = cx.id;
                    cx.remove(id);
                }
            }
            BOUNCE => {
                // Two hops along the new girder ($DCD0-$DD31): on the
                // frames its counter isn't 3 in 4 it moves a pixel the
                // girder's way, and where x is the i-th of C3FC (rightward)
                // or C412 (leftward), of `bounce_moves`, its y moves by
                // C407's i-th; at the last it rolls.
                if cx.me.time % 4 != 3 {
                    // The rolling count steps only on the frames it moves
                    // ($DD25).
                    Self::roll_tile(cx, g);
                    let d = dir_of(g);
                    let x = cx.me.x.floor() + d;
                    cx.me.x = Fx::from_int(x);
                    use barrel_tables::*;
                    let xs = if d > 0 { BOUNCE_RIGHT } else { BOUNCE_LEFT };
                    let n = p(cx, "bounce_moves");
                    if let Some(i) = (0..n).find(|i| rom_at(cx, xs + *i as u16) == x) {
                        let dy = rom_signed(cx, BOUNCE_DY + i as u16);
                        cx.me.y = cx.me.y.add(Fx::from_int(dy));
                        if i + 1 == n {
                            Self::set(cx, ROLL);
                        }
                    }
                }
            }
            OFF => {
                // Off the screen the way it fell, a pixel on its counter's
                // even counts ($DD32, mask $55), at the height it landed;
                // gone as x reaches 0 or `edge` + 1.
                let moves = cx.me.time.is_multiple_of(2);
                if moves {
                    cx.me.x = cx.me.x.add(Fx::from_int(dir_of(g + 1)));
                }
                let x = cx.me.x.floor();
                let gone = !(1..=p(cx, "edge")).contains(&x);
                if moves && !gone {
                    // $DD73, on the frames it moves, but not the one it
                    // leaves on ($DD6C; barrels clip 9, 6994).
                    Self::roll_tile(cx, g);
                }
                if gone {
                    cx.me.alive = false;
                    let id = cx.id;
                    cx.remove(id);
                }
            }
            _ => Self::set(cx, ROLL),
        }
        Self::jumped(cx);
        Self::touch_loop(cx);
    }
}

impl Barrel {
    /// The jump loop ($EC44) runs $5D through the slots after the barrels
    /// move and stops at the first barrel whose box meets Mario's ($EC5E),
    /// which kills him ($ECA7), so through the death $5D holds that slot.
    /// The boxes are the ones the contact system kills with.
    fn touch_loop(cx: &mut ExtCx<'_>) {
        let (Some(slot), Some(now)) = (cx.slot(), cx.counter("barrel_loop")) else {
            return;
        };
        let Some(m) = player(cx) else {
            return;
        };
        let rect = |b: &Body| {
            let h = cx.spec.kind(b).hitbox;
            let (x, y) = (b.x.floor() + h.x.floor(), b.y.floor() + h.y.floor());
            [x, y, x + h.w.floor() - 1, y + h.h.floor() - 1]
        };
        let ([ml, mt, mr, mb], [bl, bt, br, bb]) = (rect(m), rect(cx.me));
        let meets = ml <= br && bl <= mr && mt <= bb && bt <= mb;
        // dk_throw put it past the last slot this frame; the lowest slot
        // that meets wins, as the loop counts up.
        if meets && cx.me.alive && (slot as i64) < now {
            cx.set_counter("barrel_loop", slot as i64);
        }
    }
}

impl Barrel {
    /// The jump over a barrel ($EC44-$EC95, run for each barrel slot after
    /// the touch test): while the player is in a jump and not touching the
    /// barrel, the barrel's box left (x + `box_x`) less the player's box
    /// left ($46, the model's x) is `$9C` and its box top (y + `box_y`) less
    /// the player's top ($47) is `$9D`, both as bytes. A straight-up jump
    /// ($56 & 3 = 0, no speed across) scores at `$9C` = 0; a jump across at
    /// `$9C` below 3 on a frame its toggle $9E is 0; either way only with
    /// `$9D` below `jump_below` (24). The game then adds 100 points
    /// ($CFC6 with X = 0) and plays a sound: the model's event
    /// `barrel.jumped`, for the barrel.
    fn jumped(cx: &mut ExtCx<'_>) {
        if cx.me.ext[0] == HELD || cx.me.ext[0] == LET_GO || !cx.has_param("jump_below") {
            return;
        }
        let Some(m) = player(cx) else {
            return;
        };
        if m.on_ground || m.climbing {
            return;
        }
        let dx = (cx.me.x.floor() + p(cx, "box_x") - m.x.floor()).rem_euclid(256);
        let dy = (cx.me.y.floor() + p(cx, "box_y") - m.y.floor()).rem_euclid(256);
        let across = m.vx.raw() != 0;
        let lined_up = if across {
            dx < 3 && m.ext[1] == 0
        } else {
            dx == 0
        };
        if lined_up && dy < p(cx, "jump_below") {
            // $CFC6 sets the points sprite's timer $41 to 3 before the
            // frame's divider wrap ($F4AC runs last); the model's counters
            // stepped before this hook, so on a wrap frame (divider back at
            // 0) the wrap's step is taken here.
            // It takes the first of its two sprites ($02C0, $02C8) that
            // $CFA8 has hidden, which it does the frame after the slot's
            // timer reads 0: the second ($42) while the first still shows.
            award_points(cx);
            cx.fire_own("barrel.jumped");
            // And asks for song 5 ($EC93).
            if cx.counter("sound_fd").is_some() {
                cx.set_counter("sound_fd", 0x20);
            }
        }
    }
}

/// A hammer and Mario holding it (the game's $96 = 10 state, $D6C6-$D7BF,
/// and the touch test at $D800-$D8AA). Each hammer's hook runs after the
/// player has moved. The state is in the model's counters, which the
/// mapping reads from the game's bytes: `hammer_touch` the hammer touched
/// or held ($A0: 1 or 2, by the hammers' spawn order), `swing` the swing
/// ($9F: 1 to 3 the hammer up, 4 to 6 in front), `swing_idle` the
/// standing swing's shift ($A2) and `hammer` the timer ($3F, counted down
/// by each wrap of the divider). The hammer's own word 0 is 1 while Mario
/// holds it. The barrel it smashes is `barrel_loop`'s slot (the game's $5D),
/// and `smash_step` ($BF) is 1; the barrel's own hook
/// paces the smash and ends it ([`Barrel::smash_turn`]).
struct Hammer;

/// An inclusive box, `[x0, y0, x1, y1]`.
pub(crate) type Box4 = [i64; 4];

pub(crate) fn meet(a: Box4, b: Box4) -> bool {
    a[2] >= b[0] && b[2] >= a[0] && a[3] >= b[1] && b[3] >= a[1]
}

fn bx(cx: &ExtCx<'_>, at: (i64, i64), keys: [&str; 4]) -> Box4 {
    [
        at.0 + p(cx, keys[0]),
        at.1 + p(cx, keys[1]),
        at.0 + p(cx, keys[2]),
        at.1 + p(cx, keys[3]),
    ]
}

fn counter(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.counter(name).unwrap_or(0)
}

/// Sets one of the slow timers $3F-$45, which the game steps on each wrap
/// of the divider $34 in $F4AC, the frame's last routine ($CF27). A routine
/// that sets one runs before it, so on a wrap frame the game's timer
/// takes the wrap's step after the set; the model's counters stepped
/// before this hook, so the set takes that step here (down one, not below
/// 0). Barrels' $DAFD and $CFC6's points timer are two (hammer clip 18 on
/// Original Edition: the straight drop's 72 on a wrap frame is 71 when the
/// frame ends).
/// A points award ($CFC6, from a jumped barrel's $EC90 and a smash's
/// $EE64): it takes the first of the two points sprites ($02C0, $02C8)
/// that shows nothing and sets its timer ($41 or $42) to 3; with both
/// showing it sets none. A sprite shows from its award until $CFA8, the
/// first call of each play frame, hides it on reading its timer at 0: so
/// it shows on a frame that started with its timer above 0, though the
/// frame's divider wrap (last in the frame, $F4AC) takes it to 0, and on
/// a frame an earlier award took it.
pub(crate) fn award_points(cx: &mut ExtCx<'_>) {
    let shows = |cx: &ExtCx<'_>, name: &str| {
        cx.counter_at_start(name).unwrap_or(0) > 0 || counter(cx, name) > 0
    };
    let slot = ["points_timer", "points_timer_2"]
        .into_iter()
        .filter(|n| cx.counter(n).is_some())
        .find(|n| !shows(cx, n));
    if let Some(slot) = slot {
        set_slow_timer(cx, slot, 3);
    }
}

pub(crate) fn set_slow_timer(cx: &mut ExtCx<'_>, name: &str, value: i64) {
    let wrapped = cx.counter("divider") == Some(0);
    cx.set_counter(name, if wrapped { (value - 1).max(0) } else { value });
}

impl Ext for Hammer {
    /// A split Mario lasts one frame (`mario_split_x`).
    fn frame_start(&self, cx: &mut ExtCx<'_>) {
        for name in ["mario_split_x", "mario_split_y"] {
            if cx.counter(name).is_some_and(|v| v != 0) {
                cx.set_counter(name, 0);
            }
        }
    }

    fn step(&self, cx: &mut ExtCx<'_>) {
        use gp_sim::systems::{LAND_HOLD_WORD, STEP_WORD};
        let Some(pid) = cx
            .world
            .bodies
            .iter()
            .position(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
        else {
            return;
        };
        let mut mario = cx.world.bodies[pid];
        let at = (mario.x.floor(), mario.y.floor());
        // Mario's x before this frame's move: the dk_mario hook's copy from
        // the frame before, since the run has moved him already, or a
        // bound has held him where he was.
        let before = cx
            .kind("mario")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k && b.alive))
            .map(|b| b.ext[0])
            .filter(|x| *x > 0)
            .unwrap_or(at.0);
        // This hammer's number: 1 for the level's first.
        let me_kind = cx.me.kind;
        let own = cx.world.bodies[..cx.id]
            .iter()
            .filter(|b| b.kind == me_kind)
            .count() as i64
            + 1;
        // A death's 21st divider wrap ($43 $6D, $CAE7 -> $CC34 -> $CCC1)
        // draws the stage's objects from its list ($C5A6 by $53): both
        // hammers upright at their places (25m's, 50m's and 100m's lists
        // draw $F6 at $02D0 and $02D8), whether Mario held one or used it
        // up, though their flags ($0451, $0452) wait for the restart
        // ($CC47). Until the restart they stay there.
        if cx.system_stopped && cx.state_at_start("game", "dying") {
            let w = counter(cx, "death_wait");
            if (1..=11).contains(&w) {
                if w == 11 && cx.counter_at_start("death_wait") != Some(11) {
                    Self::redraw(cx);
                }
                return;
            }
        }
        let touch = counter(cx, "hammer_touch");
        let timer = counter(cx, "hammer");
        // A start from the original's state: a hammer touched with its
        // timer running is held.
        if cx.me.ext[0] == 0 && touch == own && timer > 0 {
            cx.me.ext[0] = 1;
            // A start in a state that stops Mario's routine (a smash) draws
            // it where it was held; nothing else runs until the state ends.
            if cx.system_stopped {
                Self::place(cx, at, counter(cx, "swing"), mario.facing_left);
            }
        }
        // A touch that kills Mario while he holds the hammer ($ED8A: the
        // touch with $96 at 10) hides it at once: $EF51 puts $FF in the y
        // of its two sprites ($02D0 + 8 * ($A0 - 1)) and clears its flag
        // ($0451,X). It stays hidden until the death's redraw of the
        // stage's objects (`redraw`, at `death_wait` 11). Presentation
        // only: nothing reads a hammer's place while Mario is down.
        let dying = cx.spec.kinds[mario.kind]
            .machine
            .as_ref()
            .is_some_and(|m| mario.state != m.initial);
        if dying && cx.me.ext[0] == 1 {
            let w = counter(cx, "death_wait");
            if w == 0 || w > 11 {
                cx.me.y = Fx::from_int(255);
            }
        }
        // While a state stops Mario's routine (a smash, a death, the
        // stage clear) the hammer's doesn't run either.
        if cx.system_stopped {
            return;
        }
        if cx.me.ext[0] == HOVER {
            Self::hover(cx, pid, mario);
            return;
        }
        if cx.me.ext[3] == GAP_DROP {
            cx.me.ext[0] = 0;
            let id = cx.id;
            cx.remove(id);
            return;
        }
        if cx.me.ext[0] == 0 {
            // Not held. A touch in a jump ($D800, called from the jump
            // routine) is taken when the landing hold ends ($D60D).
            let holding = mario.ext[LAND_HOLD_WORD] > 0;
            // A landing that killed him takes no hammer: $D627 tests $95
            // for a fall too far first, and the death leaves the hammer.
            let dying = cx.spec.kinds[mario.kind]
                .machine
                .as_ref()
                .is_some_and(|m| mario.state != m.initial);
            if touch == own {
                if mario.on_ground && !holding && !dying {
                    cx.me.ext[0] = 1;
                    // $D635 sets $3F to 75 in Mario's routine, and the
                    // timers ($F4AC, called at $CF27) run after it in the
                    // same frame. On a frame the divider wraps they take
                    // the new 75 to 74 at once. The model's counters step
                    // before the hooks, so a pickup on a wrap frame takes
                    // that step here (divider 0 is the wrap's frame).
                    let wrap = i64::from(counter(cx, "divider") == 0);
                    cx.set_counter("hammer", p(cx, "take") - wrap);
                    cx.set_counter("swing", 1);
                    // The hammer's tune ($D63F).
                    Self::jingle(cx, "jingle");
                }
                return;
            }
            let jumped = mario.air == u32::MAX;
            if touch != 0 || !(holding || (jumped && !mario.on_ground)) {
                return;
            }
            let body = bx(cx, at, ["touch_x0", "touch_y0", "touch_x1", "touch_y1"]);
            let h = (cx.me.x.floor(), cx.me.y.floor());
            let item = [h.0, h.1, h.0 + p(cx, "item_w"), h.1 + p(cx, "item_h")];
            if meet(body, item) {
                cx.set_counter("hammer_touch", own);
                // On 100m the touch also gives the fires' palette (sprite
                // palette 2) the hammer's colors ($D894-$D8A4: entry $46
                // of $C03C), until the hammer's end ($D7F2).
                if cx.has_param("blue_stage")
                    && cx.counter("stage") == Some(p(cx, "blue_stage"))
                    && cx.counter("fire_blue").is_some()
                {
                    cx.set_counter("fire_blue", 1);
                }
                // The touch test keeps the tune asked for so far this frame
                // in $0519 ($D88F), the hurry tune ($20) too once $D041 has
                // asked for it, and the hammer's end asks for it again
                // ($D7EC; dkA-h6, original frames 1332 and 25969).
                if let Some(fc) = cx.counter("sound_fc")
                    && cx.counter("hammer_jingle").is_some()
                {
                    cx.set_counter("hammer_jingle", fc);
                }
            }
            return;
        }
        // $D6DE walks only while the pad's latched byte $56 is exactly 1
        // or 2: Up or Down with a direction stands him (`run.lone_direction`).
        // Left and Right together latch none either (`run.both_left`).
        let both_left = cx.spec.kinds[mario.kind]
            .run
            .as_ref()
            .is_some_and(|r| r.both_left);
        let lone = !(cx.held("up") || cx.held("down"))
            && !(both_left && cx.held("left") && cx.held("right"));
        let right = lone && cx.held("right");
        let left = lone && !right && cx.held("left");
        // The walk's pattern, or with a smoothed walk the hammer's own
        // copy, which the run doesn't step (`own`).
        let (cycle, own) = hammer_beat(cx, mario.kind);
        let step_at =
            |n: i64| !cycle.is_empty() && cycle[n.rem_euclid(cycle.len() as i64) as usize] != 0;
        // Held, Mario is on his girder. A start from the original's state
        // loads him climbing, since the mapping's climbing flag is bit 1
        // of $96, which the hammer's state 10 sets too.
        if mario.climbing {
            mario.climbing = false;
            mario.on_ground = true;
            cx.set_body(pid, mario);
        }
        // Held. The timer ran out on an earlier frame (the game counts it
        // down after Mario's routine, so he still swings on the frame it
        // reaches 0), or Mario left the girder ($D737): the hammer is gone
        // ($D7BF clears its flag and hides its sprite) and Mario doesn't
        // move this frame.
        // The timer as Mario's routine read it is the last frame's: with
        // the frame's start kept (core `cost.frame`), 0 then; else 0 now
        // on a frame the divider didn't wrap. A smash can hold the hammer
        // past its run-out, and play resumes on a wrap frame with it at 0
        // all along (m50-hammer-smash clip 3, frame 4283).
        let ran_out = timer == 0
            && if cx.spec.cost.is_some() {
                cx.counter_at_start("hammer") == Some(0)
            } else {
                counter(cx, "divider") != 0
            };
        // Walked off the girder's end ($D737 sets $96 to 8, and $D7BF puts
        // it back to 1 with the hammer gone): this frame only drops the
        // hammer. On 25m Mario stays standing a pixel lower ($D72B adds
        // $D8EB's step to his y), and the fall starts with his next move
        // across, as the walk finds no ground then (hammer clips 5 and 6).
        // Off 25m $D729 skips that add ($53 isn't 1), so his y stays
        // (dk.untaken-hammer-edge.001 on 100m, original frame 6561).
        if !ran_out && !mario.on_ground && !mario.climbing && cx.me.ext[3] > 0 {
            // The stage's tune again ($D7EC).
            Self::jingle(cx, "stage_jingle");
            Self::colors_back(cx);
            cx.set_counter("hammer_touch", 0);
            cx.set_counter("swing", 0);
            let drop = if counter(cx, "stage") == 0 { p(cx, "edge_drop") } else { 0 };
            let y = cx.me.ext[3] - 1 + drop;
            // $D70A/$D710 and $D72B moved Mario's first sprite alone, and
            // $D7BF draws none of the others: they stay where the last
            // frame drew them (dk.hammer.013 frame 749, dk.untaken-hammer-
            // edge.001 frame 356). The player's `sprite.split` draws them
            // there; `frame_start` puts both back to 0. Presentation only.
            if cx.counter("mario_split_x").is_some() {
                cx.set_counter("mario_split_x", before - at.0);
                cx.set_counter("mario_split_y", -drop);
            }
            Self::stand(&mut mario, y);
            cx.set_body(pid, mario);
            cx.me.ext[0] = HOVER;
            cx.me.ext[1] = mario.x.floor();
            cx.me.ext[2] = y;
            cx.me.ext[3] = 0;
            // $D7BF hides its sprites at once ($02D0's y $FF in the next
            // frame's sprites; dk.hammer.013, original frames 1920 and
            // 1921). Presentation only: the hover reads its words.
            cx.me.y = Fx::from_int(255);
            return;
        }
        if ran_out || !mario.on_ground {
            Self::jingle(cx, "stage_jingle");
            Self::colors_back(cx);
            cx.me.ext[0] = 0;
            let id = cx.id;
            cx.remove(id);
            cx.set_counter("hammer_touch", 0);
            cx.set_counter("swing", 0);
            // With its own beat the run moved Mario by his speed and
            // stepped nothing, so there is nothing to undo.
            if mario.on_ground && (right || left) && !own {
                let n = mario.ext[STEP_WORD];
                // The run's move is undone, if it made one: at the screen's
                // edge it made none (dk.walk-sweep.002, frame 2634).
                if step_at(n) && at.0 != before {
                    let back = if right { -1 } else { 1 };
                    mario.x = mario.x.add(Fx::from_int(back));
                }
                mario.ext[STEP_WORD] = (n - 1).rem_euclid(cycle.len().max(1) as i64);
                cx.set_body(pid, mario);
            }
            return;
        }
        // $D9E8 steps the walk's counter on every frame of the hammer
        // state; the run stepped it only with a direction held, and not at
        // all with the hammer's own beat.
        if own || (!right && !left) {
            gp_sim::systems::cycle_step(&mut mario, &cycle);
            cx.set_body(pid, mario);
        }
        let step = step_at(mario.ext[STEP_WORD]);
        let mut swing = counter(cx, "swing");
        if step {
            // $D990: no move from the screen's edges. It tests Mario's
            // place before the move (`before`): his sprite x 224 or more
            // with Right (x 229), 16 or less with Left (x 21). Right held
            // at x 229 with the hammer idles the swing (dk.walk-sweep.001,
            // from frame 1652).
            // A level with its own edges ($C1B4 by $53: 50m's sprite x 8
            // and 232, level.camera.keep_in_view) takes them in place of
            // 25m's (m50-hammer-smash clip 4, frame 3485: x 15 to 13).
            let (left_edge, right_edge) = cx.spec.levels[cx.world.level]
                .camera
                .as_ref()
                .and_then(|c| c.keep_in_view)
                .map_or((p(cx, "left_edge"), p(cx, "right_edge")), |[l, r]| (l, r));
            let blocked = (right && (at.0 > right_edge || before >= right_edge))
                || (left && (at.0 < left_edge || before <= left_edge));
            if (right || left) && !blocked {
                swing = if swing == 0 || swing >= 6 {
                    1
                } else {
                    swing + 1
                };
            } else {
                let idle = (counter(cx, "swing_idle") << 1) & 0xFF;
                if idle == 0 {
                    cx.set_counter("swing_idle", p(cx, "idle_start"));
                    swing = if swing == 0 || swing >= 4 { 2 } else { 5 };
                } else {
                    cx.set_counter("swing_idle", idle);
                }
            }
            cx.set_counter("swing", swing);
        }
        // The hammer's head ($ECBF-$ED87): up while the swing is 0 to 3,
        // else in front of Mario. The first barrel it meets is smashed.
        let head = if swing < 4 {
            bx(cx, at, ["up_x0", "up_y0", "up_x1", "up_y1"])
        } else if mario.facing_left {
            bx(cx, at, ["back_x0", "front_y0", "back_x1", "front_y1"])
        } else {
            bx(cx, at, ["front_x0", "front_y0", "front_x1", "front_y1"])
        };
        // The barrels test it after they move (`smash_test`).
        cx.me.ext[1] = pack(head[0], head[2]);
        cx.me.ext[2] = pack(head[1], head[3]);
        // Mario's height after this frame, plus one so it isn't 0: a held
        // hammer's mark for the barrels, and where a walk off the girder
        // drops from.
        cx.me.ext[3] = mario.y.floor() + 1;
        // $D9E8 calls the hammer's walk $D6DE, which draws it ($D753), only
        // on the beat's step frames: on the others its sprites stay where
        // the last step drew them, while the conveyor's ride ($85A9, later
        // in the frame) can carry Mario on (m50-hammer-smash clip 2,
        // original frame 3804: no $D6DE, the hammer at x 113 with Mario a
        // pixel on).
        if step {
            Self::place(cx, at, swing, mario.facing_left);
        }
        // A step onto a hole this frame (platformer `level.pickup_holes`
        // leaves him standing with `air` 1, his fall to start on the next
        // frame): $EED9 drops the hammer first ($EF51: its flag $0451 and
        // its sprites go; $A0, the swing and the timer stay, and no tune
        // changes), then sets $96 = 8 (m100-hammer-gap, original frame
        // 3802).
        // Mario's routine ran with the hammer this frame, so the hammer
        // goes at the next frame's start ([`GAP_DROP`]).
        if mario.on_ground && mario.air == 1 && !cx.spec.levels[cx.world.level].pickups.is_empty() {
            cx.me.ext[3] = GAP_DROP;
            // Its sprites go this frame ($EF51 puts $FF in their y;
            // dk.m100-hammer.003, clip frame 407). Presentation only: the
            // head's box is in the words.
            cx.me.y = Fx::from_int(255);
        }
    }
}

/// The step pattern of the hammer state's beat, and whether it is the
/// hammer's own. $D9E8 steps the 15-frame counter $88 (the player's
/// `STEP_WORD`) on every frame of the hammer state, and the swing and the
/// held pose change on the pattern's step frames. That pattern is the
/// walk's, `run.step_cycle`, which the run steps itself with a direction
/// held. A profile that smooths the walk (`smooth_walk`) empties it and the
/// run steps nothing, so the hammer keeps the original's beat from its own
/// `beat` (bit n for count n) over `beat_count` counts, which no switch
/// turns off; then the second value is true and the hammer steps the
/// counter on every frame. A model with neither has no beat: the hammer
/// never swings.
pub(crate) fn hammer_beat(cx: &ExtCx<'_>, player_kind: usize) -> (Vec<i64>, bool) {
    if let Some(r) = &cx.spec.kinds[player_kind].run
        && !r.step_cycle.is_empty()
    {
        return (r.step_cycle.clone(), false);
    }
    let params = cx.kind("hammer").map(|k| &cx.spec.kinds[k].ext_params);
    let (Some(beat), Some(count)) = (
        params.and_then(|p| p.get("beat")),
        params.and_then(|p| p.get("beat_count")),
    ) else {
        return (Vec::new(), false);
    };
    let (beat, count) = (beat.floor(), count.floor());
    ((0..count).map(|n| (beat >> n) & 1).collect(), true)
}

/// The hammer's word 0 while Mario stands at the girder's end after
/// dropping it.
const HOVER: i64 = 2;

/// A held hammer's word 3 on the frame Mario stepped onto a hole: it goes
/// as the next frame starts, before his fall's first move.
const GAP_DROP: i64 = -1;

impl Hammer {
    /// A touch that kills Mario while he holds the hammer ($ED8A, $EF51)
    /// hides it on the death's own frame. The hammer's hook ran before the
    /// touch killed him, so it hides it only from the next frame; the
    /// idle hook, last in the frame, calls this to hide it on the death's
    /// frame too (dk.top-hammer.001 frame 442, original frame 14006:
    /// $96 255 from 14005 and $02D0's y $FF in the next frame's sprites).
    /// Presentation only, as the hook's own hiding.
    pub(crate) fn hide_on_death(cx: &mut ExtCx<'_>) {
        let Some(mario) = cx
            .world
            .bodies
            .iter()
            .find(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
        else {
            return;
        };
        let dying = cx.spec.kinds[mario.kind]
            .machine
            .as_ref()
            .is_some_and(|m| mario.state != m.initial);
        let w = counter(cx, "death_wait");
        if !dying || (1..=11).contains(&w) {
            return;
        }
        let held: Vec<usize> = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.alive && b.ext[0] == 1 && cx.spec.kind(b).ext.as_deref() == Some("dk_hammer")
            })
            .map(|(i, _)| i)
            .collect();
        for i in held {
            let mut b = cx.world.bodies[i];
            b.y = Fx::from_int(255);
            cx.set_body(i, b);
        }
    }

    /// The hammer's end ($D7D5 or $D7CB, then $D7F2) puts the fires'
    /// palette back (entry $4E of $C03C, the stage's colors). A drop at a
    /// hole ($EF51) doesn't.
    fn colors_back(cx: &mut ExtCx<'_>) {
        if cx.counter("fire_blue").is_some_and(|v| v != 0) {
            cx.set_counter("fire_blue", 0);
        }
    }

    /// Where the game draws the held hammer ($D753-$D7BC), for
    /// presentation: nothing reads a held hammer's place. Up above Mario's
    /// head (at `at`) while the swing is 1 to 3, else level with him in
    /// front, mirrored when he faces left.
    /// The redraw on a death's 21st wrap: the first live hammer's hook
    /// makes each of the level's hammer bodies new at its spawn, not held.
    /// A level whose hammers have all gone gets none back until the
    /// restart (no body to run the hook).
    fn redraw(cx: &mut ExtCx<'_>) {
        let me_kind = cx.me.kind;
        let first = !cx.world.bodies[..cx.id]
            .iter()
            .any(|b| b.alive && b.kind == me_kind);
        if !first {
            return;
        }
        // Each hammer body of the level, live or gone, in the order the
        // level's start made them (the k-th is the k-th spawn, the flag at
        // $0451 + k), is made again at its place, so the k-th hammer is
        // the same body before and after the redraw.
        let mine: Vec<usize> = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == me_kind)
            .map(|(i, _)| i)
            .collect();
        let places: Vec<(gp_sim::Fx, gp_sim::Fx)> = cx.spec.levels[cx.world.level]
            .spawns
            .iter()
            .filter(|s| s.kind == me_kind)
            .map(|s| (s.x, s.y))
            .collect();
        for (n, (x, y)) in places.into_iter().enumerate() {
            match mine.get(n) {
                Some(&i) => {
                    let nb = gp_sim::systems::new_body(cx.spec, cx.world, me_kind, x, y, false);
                    cx.set_body(i, nb);
                }
                None => cx.spawn(me_kind, x, y, false),
            }
        }
    }

    fn place(cx: &mut ExtCx<'_>, at: (i64, i64), swing: i64, facing_left: bool) {
        if cx.has_param("draw_up_x") {
            let (dx, dy) = if swing < 4 {
                (p(cx, "draw_up_x"), p(cx, "draw_up_y"))
            } else if facing_left {
                (p(cx, "draw_back_x"), p(cx, "draw_front_y"))
            } else {
                (p(cx, "draw_front_x"), p(cx, "draw_front_y"))
            };
            cx.me.x = Fx::from_int(at.0 + dx);
            cx.me.y = Fx::from_int(at.1 + dy);
            cx.me.facing_left = facing_left;
        }
    }

    /// Sets the jingle the game keeps asking for ($FC) to the param
    /// `name`: `jingle`, the hammer's ($40), or `stage_jingle`, 25m's ($02,
    /// which the game keeps in $0519).
    fn jingle(cx: &mut ExtCx<'_>, name: &str) {
        if cx.has_param(name) && cx.counter("sound_fc").is_some() {
            // The stage's tune is the one kept at the pickup, when the
            // model keeps it (the counter `hammer_jingle`, $0519).
            let kept = (name == "stage_jingle")
                .then(|| cx.counter("hammer_jingle"))
                .flatten();
            let v = kept.unwrap_or_else(|| p(cx, name));
            cx.set_counter("sound_fc", v);
        }
    }

    /// Mario standing at `y`, the fall's first frame undone: on the ground,
    /// still up and down, and the step counter less the fall's own step.
    fn stand(mario: &mut gp_sim::world::Body, y: i64) {
        use gp_sim::systems::STEP_WORD;
        mario.y = Fx::from_int(y);
        mario.vy = Fx::ZERO;
        mario.on_ground = true;
        mario.air = 0;
        mario.ext[STEP_WORD] = (mario.ext[STEP_WORD] - 1).rem_euclid(15);
    }

    /// A frame at the girder's end after a walk off it with the hammer:
    /// with a move across the walk finds no ground and the fall goes on as
    /// the run started it, and the hammer goes; without one Mario stays.
    fn hover(cx: &mut ExtCx<'_>, pid: usize, mut mario: gp_sim::world::Body) {
        if mario.x.floor() != cx.me.ext[1] || mario.climbing {
            cx.me.ext[0] = 0;
            let id = cx.id;
            cx.remove(id);
            return;
        }
        if !mario.on_ground {
            Self::stand(&mut mario, cx.me.ext[2]);
            cx.set_body(pid, mario);
        }
    }
}

/// Two coordinates in one word, for the head box a hammer leaves for the
/// barrels.
pub(crate) fn pack(a: i64, b: i64) -> i64 {
    (a + 512) * 1024 + (b + 512)
}

pub(crate) fn unpack(v: i64) -> (i64, i64) {
    (v.div_euclid(1024) - 512, v.rem_euclid(1024) - 512)
}

/// A barrel meeting the head of a held hammer, the head this frame's swing
/// put where its hammer's words say, is smashed ($ECBF-$ED87): the first
/// barrel that meets it (a smash already this frame or under way takes
/// none), unless Kong still holds it. Its box is the hammer's `smash_*`.
fn smash_test(cx: &mut ExtCx<'_>) {
    let me = *cx.me;
    if !me.alive || me.ext[0] == HELD || me.ext[0] == LET_GO {
        return;
    }
    // A smash under way stops the barrels, so one set while they move was
    // set this frame, by a barrel whose hook ran first. The game tests the
    // head once every barrel has moved, up the slots from 0 ($ED8A), so
    // the lowest slot that meets it takes the smash.
    if counter(cx, "smash_step") != 0
        && cx
            .slot()
            .is_none_or(|s| counter(cx, "barrel_loop") <= s as i64)
    {
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
    else {
        return;
    };
    let ((x0, x1), (y0, y1)) = (unpack(h.ext[1]), unpack(h.ext[2]));
    let k = &cx.spec.kinds[hammer];
    let param = |name: &str| k.ext_params.get(name).map_or(0, |v| v.floor());
    let at = (me.x.floor(), me.y.floor());
    let body = [
        at.0 + param("smash_x0"),
        at.1 + param("smash_y0"),
        at.0 + param("smash_x1"),
        at.1 + param("smash_y1"),
    ];
    if !meet([x0, y0, x1, y1], body) {
        return;
    }
    let Some(event) = cx.event("barrel.smashed") else {
        return;
    };
    // The smash is paced by this barrel's own frame counter
    // ([`Barrel::smash_turn`]). The hammer's loop over the barrels stops
    // at this one, so $5D holds its slot until the barrel loop runs again
    // ($ED24).
    cx.set_counter("smash_step", 1);
    // The smash's sound, pulse 1's effect 2 ($ED57).
    cx.set_counter("sound_ff", 2);
    if let Some(slot) = cx.slot()
        && cx.counter("barrel_loop").is_some()
    {
        cx.set_counter("barrel_loop", slot as i64);
    }
    let id = cx.id;
    cx.fire(event, Some(id));
}

/// Pauline calling for help ($EA5F-$EACC), presentation the game keeps in
/// RAM: the counters `pauline_wait` ($39, a frame timer the model counts
/// down), `pauline_step` ($B8), `pauline_calls` ($B7) and `pauline_body`
/// ($02F1, her body's first tile). While the wait is 0 the step counts 0
/// to 15, and on each step whose bit is set in `call_mask` ($0A and $0B
/// against the bit table at $C1BC) her body turns from `body` to
/// `call_body` or back; the `calls`th turn back sets the wait to
/// `call_wait`. The body is written into the sprite table in RAM, which
/// the NMI sends to the screen the frame after: `pauline_shown` is the
/// body as the frame began, the one on the screen. $EA5F reads $39 before
/// the timer routine $F4AC steps it at the frame's end, while the model
/// counts it down before the hooks run: the hook tests the wait as the
/// last frame ended (`ext[0]`, kept with its frame in `ext[1]`), and the
/// wait it sets is `call_wait` less this frame's step, as $F4AC takes one
/// off the $BB the call's end stores (walk clip 1, frames 4701 and 4815).
struct Pauline;

impl Ext for Pauline {
    fn step(&self, cx: &mut ExtCx<'_>) {
        // In a death's wait the rules can draw her body again before this
        // runs (the 21st wrap's redraw), so the screen takes the body the
        // frame began with.
        let body = if cx.system_stopped && cx.state_at_start("game", "dying") {
            cx.counter_at_start("pauline_body")
        } else {
            cx.counter("pauline_body")
        };
        if let Some(b) = body {
            cx.set_counter("pauline_shown", b);
        }
        let Some(wait) = cx.counter("pauline_wait") else {
            return;
        };
        // The wait as the last frame ended: the hook's own record from the
        // frame before, else this frame's before its step down to 0.
        let last = if cx.me.ext[1] != 0 && cx.me.ext[1] + 1 == cx.frame as i64 {
            cx.me.ext[0]
        } else if wait > 0 {
            wait + 1
        } else {
            0
        };
        cx.me.ext[0] = wait;
        cx.me.ext[1] = cx.frame as i64;
        if cx.system_stopped || last != 0 {
            return;
        }
        let mut step = counter(cx, "pauline_step") + 1;
        if !(0..16).contains(&step) {
            step = 0;
        }
        cx.set_counter("pauline_step", step);
        if p(cx, "call_mask") >> step & 1 == 0 {
            return;
        }
        let body = p(cx, "body");
        if counter(cx, "pauline_body") == body {
            cx.set_counter("pauline_body", p(cx, "call_body"));
            return;
        }
        cx.set_counter("pauline_body", body);
        let calls = counter(cx, "pauline_calls") + 1;
        if calls == p(cx, "calls") {
            let wait = p(cx, "call_wait") - 1;
            cx.set_counter("pauline_calls", 0);
            cx.set_counter("pauline_wait", wait);
            cx.me.ext[0] = wait;
        } else {
            cx.set_counter("pauline_calls", calls);
        }
    }
}
