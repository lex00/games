//! The play routine's cycles ($CE7C), branch by branch, from model state,
//! for the handler's cost (core `cost.frame`, docs/cycles.md, "The cost
//! model"). Each routine is written as the branches it takes over the bytes
//! it reads, with the cycles of each straight run of its code counted from
//! the 6502's instruction times (a taken branch 1 more, 2 across a page).
//! The inputs are a struct the dk_idle hook fills from the model's state at
//! the frame's end ([`Touch::of`]), so a test can fill it from the
//! original's RAM instead and check it against `gp cycles split`.
//!
//! | Routine | What decides it | Exact over the split's RAM |
//! |---|---|---|
//! | $EC29, Mario against the barrels ($EC44), the hammer's head against them ($ECBF), the fires ($ED8A) and the oil drum ($EDC5) on 25m | Each box test $EFEF's first failing comparison (the boxes' corners from the sprite buffer and the box tables at $C03C), Mario's state $96, in a jump the pad's left or right ($56), $9E and the barrel's distance ($9C, $9D), with the hammer $A0, $9F and $57 | 51,093 of 51,155 corpus play frames on 25m (worker rngH3, 2026-11-04); the rest touch: a death, a jump's points ($CFC6) or a smash, which [`touch`] leaves to the median |

use crate::play_barrels::{Barrels, Slot};
use crate::play_fires::{Fire, Fires};
use crate::play_mario::{M, Rom, Route, mario};
use gp_sim::ExtCx;
use gp_sim::world::Body;

/// A box's corners, x0, y0, x1, y1, as bytes.
type Box4 = [u8; 4];

pub(crate) fn corners(x: u8, y: u8, b: Box4) -> Box4 {
    [
        x.wrapping_add(b[0]),
        y.wrapping_add(b[1]),
        x.wrapping_add(b[2]),
        y.wrapping_add(b[3]),
    ]
}

/// $EFEF's first failing comparison of box `a` ($46-$49) against `b`
/// ($4A-$4D), or 4 when they meet.
pub(crate) fn first_fail(a: Box4, b: Box4) -> usize {
    if a[3] < b[1] {
        0
    } else if b[3] < a[1] {
        1
    } else if b[2] < a[0] {
        2
    } else if a[2] < b[0] {
        3
    } else {
        4
    }
}

/// $EFE8 or $EFEB as a route: box `a` loaded into $46-$49 ($0C 0).
pub(crate) fn box_load(r: &mut Route) {
    r.always(0xEFED, true);
    r.br(0xEFFF, false);
}

/// $EFEF or $EFF3 as a route: box `b` against `a`, its comparisons up to
/// the first that fails ($F03E, $F044, $F04A, $F050). Returns
/// [`first_fail`]: 4 for a hit.
pub(crate) fn box_cmp(r: &mut Route, a: Box4, b: Box4) -> usize {
    r.always(0xEFF1, true);
    r.br(0xEFFF, true);
    let k = first_fail(a, b);
    for (i, pc) in [0xF03E, 0xF044, 0xF04A, 0xF050].into_iter().enumerate() {
        if i > k {
            break;
        }
        r.br(pc, i == k);
    }
    k
}

/// What $EC29 reads, as bytes of the original's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Touch {
    /// Mario's sprite, $0203 and $0200.
    pub mario: (u8, u8),
    /// Mario's state $96: 1 walking, 2 on a ladder, 4 jumping, 8 falling,
    /// 10 with the hammer.
    pub state: u8,
    /// The barrel slots' sprites ($0230 + 16k, y then x at +3), y $FF free.
    pub barrels: [(u8, u8); 9],
    /// The fires' first sprites ($0210 + 16k).
    pub fires: [(u8, u8); 2],
    /// Left or Right in $56 this frame (its bits 0 and 1).
    pub sideways: bool,
    /// $9E, the jump's way.
    pub jump_way: u8,
    /// The hammer: $A0 (touched or held), $9F (the swing) and $57 (1
    /// right).
    pub hammer_touch: u8,
    pub swing: u8,
    pub facing: u8,
}

impl Touch {
    /// From the model's state at the frame's end: the player's box (the
    /// sprite is 5 above and left of it, the mapping's offsets), Mario's
    /// state from the player and the dk_mario hook's flags, the barrels'
    /// and fires' bodies (a free slot's sprite at y $FF).
    pub fn of(cx: &ExtCx<'_>) -> Option<Self> {
        let player = crate::player(cx)?;
        let mario = cx
            .kind("mario")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k && b.alive));
        let state = mario_state(player, mario);
        let sprite = |b: &Body| (b.x.floor() as u8, b.y.floor() as u8);
        let mut barrels = [(0u8, 0xFFu8); 9];
        if let Some(k) = cx.kind("barrel") {
            let out = cx
                .world
                .bodies
                .iter()
                .filter(|b| b.kind == k && b.alive && !b.waiting);
            for (s, b) in barrels.iter_mut().zip(out) {
                *s = sprite(b);
            }
        }
        let mut fires = [(0u8, 0xFFu8); 2];
        if let Some(k) = cx.kind("fire") {
            for b in cx
                .world
                .bodies
                .iter()
                .filter(|b| b.kind == k && b.alive && !b.waiting)
            {
                let slot = (b.ext[3] - 1).clamp(0, 1) as usize;
                fires[slot] = sprite(b);
            }
        }
        let held = |a: &str| cx.held(a);
        Some(Self {
            mario: ((player.x.floor() - 5) as u8, (player.y.floor() - 5) as u8),
            state,
            barrels,
            fires,
            sideways: held("left") || held("right"),
            jump_way: player.ext[1] as u8,
            hammer_touch: cx.counter("hammer_touch").unwrap_or(0) as u8,
            swing: cx.counter("swing").unwrap_or(0) as u8,
            facing: if player.facing_left { 2 } else { 1 },
        })
    }
}

/// Mario's state $96 at the frame's end from the model: 2 on a ladder, 10
/// with the hammer (the dk_mario hook's flag), 4 in a jump or its landing
/// hold (the hook's jump mark), 8 off the ground otherwise, else 1.
pub(crate) fn mario_state(player: &Body, mario: Option<&Body>) -> u8 {
    let flags = mario.map_or(0, |m| m.ext[2]);
    let jump = mario.map_or(0, |m| m.ext[3]);
    if player.climbing {
        2
    } else if flags & 8 != 0 {
        10
    } else if jump == 1 || jump == 2 {
        4
    } else if !player.on_ground {
        8
    } else {
        1
    }
}

/// $EC29's cycles on 25m from its JSR to its last RTS, or None on a frame
/// it touches something (a box test that meets, which kills Mario or
/// smashes a barrel) or awards a jump's points ($CFC6).
pub fn touch(t: &Touch, rom: &dyn Rom) -> Option<i64> {
    let mut r = Route::new();
    let mario = corners(t.mario.0, t.mario.1, rom.ptr4(0x4C));
    let barrel_box = rom.ptr4(0x3A);
    // $EC29 on 25m: Mario's box by $EFE8, then $EC44.
    box_load(&mut r);
    r.always(0xEC35, false).always(0xEC39, false);
    r.always(0xEC54, true);
    let count = rom.b(0xC1FD);
    for (s, &(x, y)) in t.barrels.iter().enumerate() {
        let b = corners(x, y, barrel_box);
        let k = box_cmp(&mut r, mario, b);
        if r.br(0xEC61, k == 4) {
            return None;
        }
        if !r.br(0xEC67, t.state != 4) {
            // A jump over it: its points when it is under Mario's box.
            let dx = b[0].wrapping_sub(mario[0]);
            let dy = b[1].wrapping_sub(mario[1]);
            let over = if r.br(0xEC6D, t.sideways) {
                !r.br(0xEC7A, dx >= 3) && !r.br(0xEC7E, t.jump_way != 0)
            } else {
                r.br(0xEC71, dx == 0)
            };
            if over && !r.br(0xEC84, dy >= 0x18) {
                return None;
            }
        }
        if r.br(0xECA2, s as u8 + 1 == count) {
            break;
        }
    }
    r.always(0xECB3, false);
    let mut a = mario;
    if !r.br(0xECB9, t.state != 10) && r.br(0xECC1, t.hammer_touch != 0) {
        // $ECC6: the hammer's head above Mario or before him.
        let (mx, my) = t.mario;
        let up = t.swing >> 2 == 0;
        r.br(0xECCA, up);
        let (hx, hy) = if !r.br(0xECD3, !up) {
            (mx.wrapping_add(4), my.wrapping_sub(16))
        } else if r.br(0xECEC, t.facing == 1) {
            (mx.wrapping_add(16), my.wrapping_add(6))
        } else {
            (mx.wrapping_sub(16), my.wrapping_add(6))
        };
        box_load(&mut r);
        r.always(0xED10, false);
        a = corners(hx, hy, rom.ptr4(0x3C));
        for (s, &(x, y)) in t.barrels.iter().enumerate() {
            let k = box_cmp(&mut r, a, corners(x, y, barrel_box));
            if r.br(0xED24, k == 4) {
                return None;
            }
            r.br(0xED2F, s == 8);
        }
    }
    // $ED8A: the fires, against Mario's box or the head's.
    for (s, &(x, y)) in t.fires.iter().enumerate() {
        let k = box_cmp(&mut r, a, corners(x, y, barrel_box));
        if r.br(0xED9C, k == 4) {
            return None;
        }
        r.br(0xEDA8, s == 1);
    }
    if !r.br(0xEDB9, t.state != 10) {
        r.always(0xEDBF, true);
    }
    // $EDC5: the oil drum, its corner at $C423 and its box at $C42B's
    // pointer.
    r.always(0xEDC9, true).always(0xEDF4, true);
    let drum = corners(rom.b(0xC423), rom.b(0xC424), rom.box4(rom.w(0xC42B)));
    let k = box_cmp(&mut r, a, drum);
    if r.br(0xEDEE, k == 4) {
        return None;
    }
    r.walk(rom, 0xEC29).map(|c| c + 6)
}

/// What the play routine's small callees read, as the original's bytes:
/// at the frame's start (`*_0`, before the routine changes it) or its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Small {
    /// Mario's state $96 as the frame started.
    pub state_0: u8,
    /// The attract demo ($58).
    pub demo: bool,
    /// $15,X as $D175 reads it: the directions held (bits 0-3) and A newly
    /// pressed (bit 7).
    pub pad: u8,
    /// Pauline: $39 (pauline_wait), $B8 (pauline_step), $02F1
    /// (pauline_body) and $B7 (pauline_calls) as the frame started.
    pub pauline_wait_0: u8,
    pub pauline_step_0: u8,
    pub pauline_body_0: u8,
    pub pauline_calls_0: u8,
    /// The oil drum: $AD (drum) as the frame started and ended, $38
    /// (drum_timer) and $02E1 (drum_flame) as it started.
    pub drum_0: u8,
    pub drum_1: u8,
    pub drum_timer_0: u8,
    pub drum_flame_0: u8,
}

/// $D175, the pad to $56 and $57 and a jump's start, from its JSR, or 0 on
/// a frame it doesn't run (Mario jumping or falling, or the demo, whose
/// script $EBDA runs instead).
pub fn pad(s: &Small, rom: &dyn Rom) -> Option<i64> {
    if s.demo || s.state_0 == 4 || s.state_0 == 8 {
        return Some(0);
    }
    let mut r = Route::new();
    let v = s.pad & 0x0F;
    if !r.br(0xD17F, v == 0) {
        r.br(0xD183, v >> 2 != 0);
    }
    if !r.br(0xD18D, s.state_0 != 1) {
        r.br(0xD193, s.pad & 0x80 == 0);
    }
    r.walk(rom, 0xD175).map(|c| c + 6)
}

/// $EA5F, Pauline's call, from its JSR: nothing while $39 runs, else
/// $EAA1's step of $B8 and its bit of $C1BC (with $0A from $EA64's
/// immediate, and $0B 0), and the call drawn when the bit is set ($F080 by
/// her body's tile), the 4th call setting $B7 back to 0.
pub fn pauline(s: &Small, rom: &dyn Rom) -> Option<i64> {
    let mut r = Route::new();
    if !r.br(0xEA61, s.pauline_wait_0 == 0) {
        return r.walk(rom, 0xEA5F).map(|c| c + 6);
    }
    let next = s.pauline_step_0.wrapping_add(1);
    if !r.br(0xEAA5, next & 0x80 != 0) {
        r.br(0xEAA9, next >= 16);
    }
    let v = if next & 0x80 != 0 || next >= 16 {
        0
    } else {
        next
    };
    r.br(0xEAB4, v >= 8);
    let bits = if v < 8 { rom.b(0xEA65) } else { 0 };
    let bit = rom.b(0xC1BC + u16::from(v & 7)) & bits;
    r.br(0xEAC8, bit == 0);
    if r.br(0xEA6F, bit != 0) {
        let other = !r.br(0xEA7F, s.pauline_body_0 == 0xDB);
        r.cost(0xEA8F, crate::play_barrels::DRAW_ROLL);
        let calls = s.pauline_calls_0.wrapping_add(u8::from(other));
        r.br(0xEA96, calls != 4);
    }
    r.walk(rom, 0xEA5F).map(|c| c + 6)
}

/// $E19A, the oil drum's flame, from its JSR: 17 unlit, 494 the frame it
/// is lit ($AD 1, its sprite drawn), else 29 while $38 runs, and the
/// flame's flicker ($E1C3) when it runs out: 67 with $02E1 at $FE, 65 at
/// $FC.
pub fn drum(s: &Small) -> i64 {
    // Only $E19A's lit branch sets 2, from 1, which a barrel at the drum
    // set this frame ($AD 1 from $DA16's routines) or the last.
    let at = if s.drum_1 == 2 && s.drum_0 != 2 {
        1
    } else {
        s.drum_0.max(s.drum_1)
    };
    match at {
        0 => 17,
        1 => 494,
        _ if s.drum_timer_0 != 0 => 29,
        _ if s.drum_flame_0 == 0xFE => 67,
        _ => 65,
    }
}

/// The play routine's own code from $CE7C to $CEFE, with the JSRs its
/// callees' cycles count: 87 with $D175 run, 82 for a jump ($96 4) and 78
/// for a fall (8), which skip it. The demo's own is the median's.
pub fn own(s: &Small) -> Option<i64> {
    if s.demo {
        return None;
    }
    Some(match s.state_0 {
        4 => 82,
        8 => 78,
        _ => 87,
    })
}

impl Small {
    /// From the model's state: the counter `mario_96`, which [`add`] keeps
    /// from frame to frame (loaded from $96 at a start), and the counters
    /// the mapping reads from these bytes.
    pub fn of(cx: &ExtCx<'_>) -> Self {
        let start = |name: &str| cx.counter_at_start(name).unwrap_or(0) as u8;
        let end = |name: &str| cx.counter(name).unwrap_or(0) as u8;
        let mut pad = 0u8;
        for (bit, a) in [(1, "right"), (2, "left"), (4, "down"), (8, "up")] {
            if cx.held(a) {
                pad |= bit;
            }
        }
        if cx.pressed("jump") {
            pad |= 0x80;
        }
        Self {
            state_0: (cx.counter_at_start("mario_96").unwrap_or(0) & 0xFF) as u8,
            demo: cx.counter_at_start("attract").unwrap_or(0) != 0,
            pad,
            pauline_wait_0: start("pauline_wait"),
            pauline_step_0: start("pauline_step"),
            pauline_body_0: start("pauline_body"),
            pauline_calls_0: start("pauline_calls"),
            drum_0: start("drum"),
            drum_1: end("drum"),
            drum_timer_0: start("drum_timer"),
            drum_flame_0: start("drum_flame"),
        }
    }
}

/// The routines' commonest values, which the `[cost]` term for play
/// counts: $EC29, $D175, $EA5F, $E19A, the routine's own code, $D1A4,
/// $D04C, $CFA8, $DA16, $E1E5, $EB06 and $EBB6.
const MEDIANS: [(&str, i64); 12] = [
    ("touch_median", 4148),
    ("pad_median", 58),
    ("pauline_median", 17),
    ("drum_median", 29),
    ("own_median", 87),
    ("mario_median", 788),
    ("goal_median", 40),
    ("points_median", 97),
    ("barrels_median", 2237),
    ("fires_median", 186),
    ("kong_median", 106),
    ("bonus_median", 17),
];

/// Where 100m's ending's assets lifted on their own start, by CPU
/// address: `palette_ending` (the $3F1D update's count byte and its three
/// colours) and fallen Kong's two poses `kong_fallen_a` and `kong_fallen_b`
/// (24 tiles each). The code's spans in [`TABLES`] go on after each.
const ENDING_PALETTE: u16 = 0xC703;
const ENDING_PALETTE_LEN: u16 = 4;
const KONG_FALLEN_A: u16 = 0xC71D;
const KONG_FALLEN_B: u16 = 0xC736;
const KONG_FALLEN_LEN: u16 = 24;

/// The lifted spans the play routine's tables come from, by the CPU
/// address each starts at (the model's `ext_tables` of the idle entity:
/// tables of their own, and the rows and tables other parts of the model
/// lift), $C03C to $C4FF. 75m's tables ($C20C to $C2EF) and 100m's ($C2F0
/// to $C3E1) are spans of their own, so the model can name them; the hooks
/// read across them by address. The tables after 100m's keep the name
/// `tables_c2f0` from before 100m's were split off.
pub(crate) const TABLES: [(&str, u16); 42] = [
    // Donkey Kong: Original Edition's lower half: 50m's girder heights
    // ($8286, $E016's list for $53 = 2) and its fires' ladder lists and
    // edges ($8435-$8488, $E7A3's lists and the hook $8489's ends).
    ("heights_50m", 0x8286),
    ("fire_lists_50m", 0x8435),
    ("pointers", 0xC03C),
    ("runs", 0xC08C),
    ("heights", 0xC0BC),
    ("shapes", 0xC0C3),
    ("gaps", 0xC0CF),
    ("ladders", 0xC0E3),
    ("ladders_end", 0xC10A),
    ("ladder_shapes", 0xC10B),
    ("climb", 0xC127),
    ("tables", 0xC15F),
    ("runs_75m", 0xC20C),
    ("runs_75m_end", 0xC24B),
    ("heights_75m", 0xC24C),
    ("shapes_75m", 0xC252),
    ("ladders_75m", 0xC26E),
    ("ladders_75m_end", 0xC28C),
    ("ladder_shapes_75m", 0xC28D),
    ("tops_75m", 0xC2A5),
    ("tables_75m", 0xC2C3),
    ("fire_ways_75m", 0xC2D8),
    ("runs_100m", 0xC2F0),
    ("runs_100m_end", 0xC2FF),
    ("heights_100m", 0xC300),
    ("shapes_100m", 0xC306),
    ("ladders_100m", 0xC316),
    ("ladders_100m_end", 0xC340),
    ("ladder_shapes_100m", 0xC341),
    ("tops_100m", 0xC349),
    ("tops_100m_end", 0xC373),
    ("fire_index_100m", 0xC374),
    ("fire_ways_100m", 0xC37A),
    ("fire_starts_100m", 0xC3CE),
    ("fire_gaps_100m", 0xC3DE),
    ("tables_c2f0", 0xC3E2),
    ("code_c500", 0xC500),
    // From $C6A6: 100m's hammer palette ($C6A2-$C6A5) is lifted on its own.
    ("code_c6a3", 0xC6A6),
    // 100m's ending's palette and Kong's fallen poses are lifted on their
    // own, so the code's tables go on after each of them.
    ("code_c707", ENDING_PALETTE + ENDING_PALETTE_LEN),
    ("code_c735", KONG_FALLEN_A + KONG_FALLEN_LEN),
    ("code_c74e", KONG_FALLEN_B + KONG_FALLEN_LEN),
    ("code_fa45", 0xFA45),
];

/// The bytes of Mario's that the model's state doesn't hold as the frame
/// starts, as the counter `mario_start` keeps them from the last frame's
/// end (loaded from these bytes at a start), low byte first.
const START_BYTES: [u16; 8] = [0x203, 0x200, 0x88, 0x59, 0x5A, 0xA1, 0x94, 0x95];

/// Mario's routine's bytes as the frame starts, from the model.
fn mario_in(cx: &ExtCx<'_>, s: &Small) -> M {
    let mut m = M::new();
    let start = |name: &str| cx.counter_at_start(name).unwrap_or(0) as u8;
    let kept = cx.counter_at_start("mario_start").unwrap_or(0) as u64;
    for (k, a) in START_BYTES.iter().enumerate() {
        m.ram[usize::from(*a)] = (kept >> (8 * k)) as u8;
    }
    m.ram[0x53] = 1;
    m.ram[0x96] = s.state_0;
    m.ram[0x9E] = (cx.counter_at_start("mario_96").unwrap_or(0) >> 8) as u8;
    for (a, name) in [
        (0x9B, "walk_beat"),
        (0x97, "walk_pose"),
        (0x85, "walk_turn"),
        (0x5B, "climb_top"),
        (0x5C, "climb_step"),
        (0x3F, "hammer"),
        (0x9F, "swing"),
        (0xA0, "hammer_touch"),
        (0xA2, "swing_idle"),
    ] {
        m.ram[a] = start(name);
    }
    // $D175 ran first: $56 the pad's directions, and A while walking
    // starts the jump.
    m.ram[0x56] = s.pad & 0x0F;
    if s.state_0 == 1 && s.pad & 0x80 != 0 {
        m.ram[0x96] = 4;
        m.ram[0x94] = 0;
    }
    if let Some(p) = crate::player(cx) {
        m.ram[0x57] = if p.facing_left { 2 } else { 1 };
        m.y1 = (p.y.floor() - 5) as u8;
    }
    m
}

/// $D04C from its JSR on 25m: the goal test, 30 with $5A clear, else 40
/// unless $59 is the top girder ($C1FA), the stage's clear (None).
pub fn goal(rom: &dyn Rom, girder: u8, on: u8) -> Option<i64> {
    if on == 0 {
        return Some(30);
    }
    if rom.b(0xC1FA) == girder {
        return None;
    }
    Some(40)
}

/// The smash term's frame ($CF13: a barrel breaking as the frame
/// started, not paused, on 25m): the play routine runs $EE0C.
fn smashes(cx: &ExtCx<'_>) -> bool {
    cx.counter("mario_96").is_some()
        && cx.has_param("touch_median")
        && cx.state_at_start("game", "smashing")
        && cx.state_at_start("pause", "running")
        && cx.counter_at_start("stage").is_none_or(|s| s == 0)
}

/// $EE0C from its JSR: the smashed barrel ($5D's slot) breaks a step each
/// time its frame counter's bit of $80 is set (its 8th and 16th frame), its
/// picture drawn ($EADB, measured) and the step $BF counted on. None on the
/// last step (11), which adds the points ($CFC6).
pub fn smash(time: u8, step: u8, rom: &dyn Rom) -> Option<i64> {
    let mut s = Slot {
        time,
        ..Slot::default()
    };
    let mut r = Route::new();
    let a = crate::play_barrels::dfe4(&mut s, 0x80, &mut r);
    if r.br(0xEE17, a) {
        // 25m's $53 (the smash term is 25m's), and the step's sound.
        r.br(0xEE1E, false);
        r.br(0xEE32, step != 1);
        if r.br(0xEE3A, step == 0x0B) {
            return None;
        }
        r.cost(0xEE44, crate::play_barrels::DRAW);
    }
    r.walk(rom, 0xEE0C).map(|c| c + 6)
}

/// The death term's frame ($CF19: the game dying as the frame started,
/// `over` 0, not paused, on 25m): the play routine runs the spin $D0C0.
/// The last death's spin too, while $4E (the counter `demo`) is clear:
/// after it the game over's routine $CA30 runs instead.
fn dies(cx: &ExtCx<'_>) -> bool {
    let last =
        cx.state_at_start("game", "last_dying") && cx.counter_at_start("demo").unwrap_or(0) < 1;
    cx.counter("mario_96").is_some()
        && cx.has_param("touch_median")
        && (cx.state_at_start("game", "dying") || last)
        && cx.state_at_start("pause", "running")
        && cx.counter_at_start("over").unwrap_or(0) < 1
        && cx.counter_at_start("stage").is_none_or(|s| s == 0)
}

/// The play routine's head in the attract demo ($58, the counter
/// `attract`, set), against the terms' frame with it clear: $CE7C's BEQ
/// falls through to the song test ($0102, `sound_music_on`: its BNE taken,
/// or the stores to $4015 and $0100) and the Start test at $CE8B, 14 more
/// with a song and 21 without, and $CC04 returns at its BNE, 18 against
/// the 47 of a game's frame. 0 off the demo.
pub fn demo_head(cx: &ExtCx<'_>) -> i64 {
    if cx.counter_at_start("attract").unwrap_or(0) == 0 {
        return 0;
    }
    let song = cx.counter("sound_music_on").unwrap_or(0) != 0;
    (if song { 14 } else { 21 }) - 29
}

/// $D0C0 from its JSR: the death's spin, a step on the beat $D9E6 ($88
/// at 4 and 12 of 16), its first step's wait $3A, each turn of the
/// picture $0201 ($6C to $7C) drawn ($F082, measured: 598). None on the
/// spin's last turn and after it ($98 $FF).
pub fn spin(beat: u8, step: u8, wait: u8, demo: bool, pose: u8, rom: &dyn Rom) -> Option<i64> {
    let mut r = Route::new();
    // The beat $D9E6 with the pattern $10 in both halves.
    let next = beat.wrapping_add(1);
    let v = if r.br(0xD9F0, next >= rom.b(0xD9EF)) {
        0
    } else {
        next
    };
    r.br(0xD9FB, v >= rom.b(0xD9FA));
    let bit = rom.b(0xC1BC + u16::from(v % rom.b(0xD9FA).max(1))) & rom.b(0xD0C5);
    r.br(0xDA0F, bit == 0);
    if r.br(0xD0C9, bit == 0) {
        return r.walk(rom, 0xD0C0).map(|c| c + 6);
    }
    if r.br(0xD0CF, step == 0xFF) {
        return None;
    }
    if !r.br(0xD0D3, step != 0) {
        r.br(0xD0D7, demo);
        return r.walk(rom, 0xD0C0).map(|c| c + 6);
    }
    if !r.br(0xD0E6, wait == 0) {
        if r.br(0xD0EA, wait < rom.b(0xD0E9)) {
            return r.walk(rom, 0xD0C0).map(|c| c + 6);
        }
        r.br(0xD0EE, demo);
    }
    let first = rom.b(0xD0FC);
    r.br(0xD0FD, pose >= first);
    let t = pose.max(first).wrapping_add(rom.b(0xD103));
    let mut step = step;
    if !r.br(0xD106, t < rom.b(0xD105)) {
        step += 1;
        if r.br(0xD10E, step == rom.b(0xD10D)) {
            return None;
        }
    }
    r.cost(0xD127, crate::play_mario::F082);
    r.br(0xD12E, step != rom.b(0xD12D));
    r.walk(rom, 0xD0C0).map(|c| c + 6)
}

/// $EB06 from its JSR on 25m, Kong shown ($0503 set): by the throw
/// timer $36 and the slow timer $44 as the frame starts, a pose drawn
/// ($C815, measured: 1805, or 1839 with the throw's) at the throw's
/// start ($36 $18), as it runs out (0) and on the beats $44 reaches ($13
/// and $0B one pose, $0F and $08 the other). None just after a throw
/// ($0515) or on the beat at 4.
pub fn kong(throw: u8, wait: u8, thrown: u8) -> Option<i64> {
    let c = 56;
    if throw == 0x18 {
        return Some(c + 16 + 1805);
    }
    if throw == 0 {
        return Some(c + 4 + 19 + 1839);
    }
    if thrown != 0 {
        return None;
    }
    let c = c + 4 + 6 + 9;
    Some(match wait {
        0x13 => c + 17 + 1805,
        0x0F => c + 22 + 1805,
        0x0B => c + 27 + 1805,
        0x08 => c + 32 + 1805,
        0x04 => return None,
        _ => c + 31,
    })
}

/// $CFA8 from its JSR: each points sprite whose timer ($41, $42) is out
/// is hidden, 11 cycles more than one whose timer runs.
pub fn points(timers: [u8; 2]) -> i64 {
    97 - 11 * timers.iter().filter(|t| **t != 0).count() as i64
}

/// Adds the play routine's modelled routines to the frame's cost, as each
/// one's difference from its commonest value, which the `[cost]` term for
/// play counts ([`MEDIANS`]), on a frame the term holds and the play
/// routine runs its whole frame ([`runs`]). A routine that can't be
/// costed (a touch, the demo's script, a fall) keeps the term's value.
/// Keeps Mario's state and the bytes of [`START_BYTES`] for the next
/// frame's start in the counters `mario_96` and `mario_start`.
pub fn add(cx: &mut ExtCx<'_>) {
    let t = Touch::of(cx);
    let spec = cx.spec;
    let kind = spec.kind(cx.me);
    let tables: Vec<(u16, &[i64])> = TABLES
        .iter()
        .filter_map(|(name, at)| kind.ext_tables.get(*name).map(|t| (*at, t.as_slice())))
        .collect();
    let rom = |a: u16| {
        tables
            .iter()
            .rev()
            .find(|(at, _)| a >= *at)
            .and_then(|(at, t)| t.get(usize::from(a - at)))
            .map_or(0, |v| *v as u8)
    };
    let mut after = None;
    let mut ran_barrels = None;
    let mut ran_fires = None;
    if runs(cx) {
        let s = Small::of(cx);
        let mut m = mario_in(cx, &s);
        let mario = if s.demo { None } else { mario(&mut m, &rom) };
        if mario.is_some() {
            after = Some(m.clone());
        }
        let goal = mario.and_then(|_| goal(&rom, m.ram[0x59], m.ram[0x5A]));
        let mut b = barrels_in(cx, t.as_ref().map_or(1, |t| t.state), m.ram[0x59]);
        // The barrels run for their bytes on every frame; their cost needs
        // Mario's girder and state as his routine leaves them.
        let mario_x = if mario.is_some() {
            m.ram[0x203]
        } else {
            crate::player(cx).map_or(0, |p| (p.x.floor() - 5) as u8)
        };
        let mut fs = fires_in(cx, mario_x);
        let fires = if s.demo {
            None
        } else {
            let n = crate::play_fires::fires(&mut fs, &rom);
            ran_fires = Some(fs);
            n
        };
        let barrels = if s.demo {
            None
        } else {
            let n = crate::play_barrels::barrels(&mut b, &rom);
            ran_barrels = Some(b);
            n.filter(|_| mario.is_some())
        };
        let start8 = |name: &str| cx.counter_at_start(name).unwrap_or(0) as u8;
        let timers = [
            cx.counter_at_start("points_timer").unwrap_or(0) as u8,
            cx.counter_at_start("points_timer_2").unwrap_or(0) as u8,
        ];
        let got = [
            t.as_ref().and_then(|t| touch(t, &rom)),
            pad(&s, &rom),
            pauline(&s, &rom),
            Some(drum(&s)),
            own(&s),
            mario,
            goal,
            Some(points(timers)),
            barrels,
            fires,
            kong(
                start8("throw_timer"),
                start8("kong_wait"),
                start8("kong_thrown"),
            ),
            (start8("bonus_tick") != 0).then_some(17),
        ];
        let mut n = 0;
        for (v, (name, median)) in got.iter().zip(MEDIANS) {
            if let Some(v) = v {
                n += v - if cx.has_param(name) {
                    crate::p(cx, name)
                } else {
                    median
                };
            }
        }
        cx.add_cycles(n);
    }
    if smashes(cx) {
        let k = cx.counter_at_start("barrel_loop").unwrap_or(0).clamp(0, 8);
        let kept = cx
            .counter_at_start(&format!("barrel_start{k}"))
            .unwrap_or(0) as u64;
        let n = smash(
            (kept >> 32) as u8,
            cx.counter_at_start("smash_step").unwrap_or(0) as u8,
            &rom,
        );
        if let Some(n) = n {
            let median = if cx.has_param("smash_median") {
                crate::p(cx, "smash_median")
            } else {
                86
            };
            cx.add_cycles(n - median);
        }
    }
    if dies(cx) {
        let start8 = |name: &str| cx.counter_at_start(name).unwrap_or(0) as u8;
        let kept = cx.counter_at_start("mario_start").unwrap_or(0) as u64;
        let spin = spin(
            (kept >> 16) as u8,
            start8("sound_death_seen"),
            start8("sound_death_wait"),
            start8("attract") != 0,
            start8("mario_pose"),
            &rom,
        );
        if let Some(n) = spin {
            let median = if cx.has_param("death_median") {
                crate::p(cx, "death_median")
            } else {
                81
            };
            cx.add_cycles(n - median);
        }
        // $CF24 runs the goal test $D04C on a death's frames too, over
        // Mario's $59 and $5A as his last frame left them (Mario's routine
        // doesn't run in a death): 40 with $5A set, 30 clear, which the
        // term holds.
        let girder = (kept >> 24) as u8;
        let on = (kept >> 32) as u8;
        if let Some(g) = goal(&rom, girder, on) {
            cx.add_cycles(g - 30);
        }
        cx.add_cycles(demo_head(cx));
        // $CFA8 runs on a death's frames too, both points sprites' timers
        // out: 97 on 3737 of 3737 corpus death frames, which the term
        // holds (the model's points timers aren't the game's then).
    }
    let Some(t) = t else {
        return;
    };
    let way = crate::player(cx).map_or(0, |p| p.ext[1] & 0xFF);
    if cx.counter("mario_96").is_some() {
        cx.set_counter("mario_96", i64::from(t.state) | way << 8);
    }
    if cx.counter("mario_start").is_some() {
        let kept = cx.counter_at_start("mario_start").unwrap_or(0) as u64;
        let byte = |k: usize| (kept >> (8 * k)) as u8;
        let p = crate::player(cx).copied();
        let mut v = [0u8; 8];
        for (k, a) in START_BYTES.iter().enumerate() {
            v[k] = match (a, &p, &after) {
                (0x203, Some(p), _) => (p.x.floor() - 5) as u8,
                (0x200, Some(p), _) => (p.y.floor() - 5) as u8,
                (0x88, Some(p), _) => p.ext[0] as u8,
                (a, _, Some(m)) => m.ram[usize::from(*a)],
                // $94 in a landing hold is $F4 less the hold's frames left,
                // $95 the takeoff's sprite y (the mapping's offsets).
                (0x94, Some(p), _) if p.ext[2] > 0 => (244 - p.ext[2]) as u8,
                (0x95, Some(p), _) => (p.ext[3] - 5) as u8,
                _ => byte(k),
            };
        }
        let packed = v.iter().rev().fold(0u64, |acc, b| acc << 8 | u64::from(*b));
        cx.set_counter("mario_start", packed as i64);
    }
    keep_barrels(cx, ran_barrels.as_ref());
    keep_fires(cx, ran_fires.as_ref());
}

/// The fire slots as the frame starts: the bytes the counters
/// `fire_start0` and `fire_start1` keep, the tiles the model's counters
/// hold, and the rest of what $E1E5 reads.
fn fires_in(cx: &ExtCx<'_>, mario_x: u8) -> Fires {
    let start = |name: &str| cx.counter_at_start(name).unwrap_or(0);
    let random = start("random") as u64;
    Fires {
        fires: std::array::from_fn(|k| {
            let v = start(&format!("fire_start{k}")) as u64;
            let byte = |i: u32| (v >> (8 * i)) as u8;
            Fire {
                free: byte(1) == 0xFF,
                x: byte(0),
                y: byte(1),
                tile: start(&format!("fire_tile_{k}")) as u8,
                state: byte(2),
                count: byte(3),
                girder: byte(4),
                aimed: byte(5),
                aim: byte(6),
                way: byte(7),
                pause: start(&format!("fire_pause{k}")) as u8,
                slot: k as u8,
            }
        }),
        wait: start("fire_wait") as u8,
        drum: start("drum") as u8,
        aim: start("fire_aim") as u8,
        random: [
            crate::idle::byte_of(random, 1),
            crate::idle::byte_of(random, 2),
        ],
        mario_x,
    }
}

/// Keeps each fire slot's bytes for the next frame's start, from the fire
/// bodies (the mapping's: the state $AF one less than `ext[0]`, the move
/// counter `ext[1]`, the girder, aimed flag, aimed way and last way in
/// `ext[2]`'s bytes 1 to 4), a free slot's sprite y at $FF.
fn keep_fires(cx: &mut ExtCx<'_>, ran: Option<&Fires>) {
    let Some(kind) = cx.kind("fire") else {
        return;
    };
    for k in 0..2 {
        let name = format!("fire_start{k}");
        let Some(_) = cx.counter(&name) else {
            return;
        };
        let kept = cx.counter_at_start(&name).unwrap_or(0) as u64;
        let mut v: [u8; 8] = std::array::from_fn(|i| (kept >> (8 * i)) as u8);
        let body = cx
            .world
            .bodies
            .iter()
            .find(|b| b.kind == kind && b.alive && !b.waiting && b.ext[3] - 1 == k as i64);
        let pause_now = body.map(|b| b.ext[2] & 0xFF);
        match body {
            Some(b) => {
                let w = b.ext[2] as u64;
                v = [
                    b.x.floor() as u8,
                    b.y.floor() as u8,
                    (b.ext[0] - 1) as u8,
                    b.ext[1] as u8,
                    (w >> 8) as u8,
                    (w >> 16) as u8,
                    (w >> 24) as u8,
                    (w >> 32) as u8,
                ];
            }
            None => {
                if let Some(f) = ran {
                    let f = f.fires[k];
                    v[2] = f.state;
                }
                v[1] = 0xFF;
            }
        }
        let packed = v.iter().rev().fold(0u64, |acc, b| acc << 8 | u64::from(*b));
        cx.set_counter(&name, packed as i64);
        // The ladder's pause $E8 + k, the low byte of the body's `ext[2]`.
        let pause = format!("fire_pause{k}");
        if let (Some(v), Some(_)) = (pause_now, cx.counter(&pause)) {
            cx.set_counter(&pause, v);
        }
    }
}

/// The barrel slots as the frame starts: the bytes the counters
/// `barrel_start0` to `barrel_start8` keep, the tiles and turn counts the
/// model's counters hold, and the rest of what $DA16 reads.
fn barrels_in(cx: &ExtCx<'_>, mario: u8, girder: u8) -> Barrels {
    let start = |name: &str| cx.counter_at_start(name).unwrap_or(0);
    let slots = std::array::from_fn(|k| {
        let v = start(&format!("barrel_start{k}")) as u64;
        let byte = |i: u32| (v >> (8 * i)) as u8;
        Slot {
            free: byte(1) == 0xFF,
            x: byte(0),
            y: byte(1),
            state: byte(2),
            girder: byte(3),
            time: byte(4),
            foot: byte(5),
            way: byte(6),
            side: byte(7),
            ran: false,
            tile: start(&format!("barrel_tile{k}")) as u8,
            turn: start(&format!("barrel_anim{k}")) as u8,
        }
    });
    Barrels {
        slots,
        d: (start("game_b") + start("loop")).clamp(0, 4) as u8,
        r19: crate::idle::byte_of(start("random") as u64, 1),
        drop: start("barrel_drop") as u8,
        landed: landed(cx),
        drum: start("drum") as u8,
        straight_wait: start("straight_wait") as u8,
        throw: start("throw_timer") as u8,
        hold: start("drop_timer") as u8,
        girder,
        mario,
    }
}

/// $042B, where the straight barrel landed: the slot 0 barrel's `ext[3]`
/// in its landed state (the mapping's shared word).
fn landed(cx: &ExtCx<'_>) -> u8 {
    let Some(kind) = cx.kind("barrel") else {
        return 0;
    };
    cx.world
        .slots
        .first()
        .and_then(|s| s.body.checked_sub(1))
        .and_then(|i| cx.world.bodies.get(i as usize))
        .filter(|b| b.kind == kind && b.alive && b.ext[0] == 0xC2)
        .map_or(0, |b| b.ext[3] as u8)
}

/// Keeps each barrel slot's bytes for the next frame's start: a body's
/// sprite, state, girder and frame counter from the model, a ladder's
/// foot from $DA16's copy when it ran, a free slot's sprite y at $FF.
fn keep_barrels(cx: &mut ExtCx<'_>, ran: Option<&Barrels>) {
    let Some(kind) = cx.kind("barrel") else {
        return;
    };
    for k in 0..9 {
        let name = format!("barrel_start{k}");
        let Some(_) = cx.counter(&name) else {
            return;
        };
        let kept = cx.counter_at_start(&name).unwrap_or(0) as u64;
        let mut v: [u8; 8] = std::array::from_fn(|i| (kept >> (8 * i)) as u8);
        if let Some(b) = ran {
            v[5] = b.slots[k].foot;
            v[7] = b.slots[k].side;
        }
        // The slot's word holds a straight drop's way and side.
        if let Some(word) = cx.world.slots.get(k).map(|s| s.word) {
            v[6] = (word & 0xFF) as u8;
            v[7] = (word >> 8 & 0xFF) as u8;
        }
        let body = cx
            .world
            .slots
            .get(k)
            .and_then(|s| s.body.checked_sub(1))
            .and_then(|i| cx.world.bodies.get(i as usize))
            .filter(|b| b.kind == kind && b.alive && !b.waiting);
        match body {
            Some(b) => {
                v[0] = b.x.floor() as u8;
                v[1] = b.y.floor() as u8;
                v[2] = b.ext[0] as u8;
                // Kong's barrel has no girder ($68 0) until it rolls, and
                // its frame counter starts at its let go ($DAB8).
                let held = b.ext[0] == 0x80;
                // Nor does a straight drop's ($C0 to $C2), which never
                // reads its girder.
                v[3] = if held || matches!(b.ext[0], 0x81 | 0xC0..=0xC2) {
                    0
                } else {
                    b.ext[2] as u8
                };
                // The frame counter as $DA16's copy steps it where its
                // branches ran: the hook's isn't compared ($8A is loaded,
                // SLOT_LOADS_17), and parts from it after a straight drop.
                v[4] = match ran.map(|r| r.slots[k]) {
                    Some(s) if s.ran && s.state == b.ext[0] as u8 => s.time,
                    _ if held => 0,
                    _ => b.time as u8,
                };
            }
            None => match ran.map(|r| r.slots[k]) {
                // The slot took Kong's barrel this frame ($DA2B), drawn at
                // $30, $30; the model's throw comes a frame later.
                Some(s) if s.state == 0x80 => {
                    v = [0x30, 0x30, 0x80, 0, 0, s.foot, v[6], v[7]];
                }
                // A barrel leaves its slot with its girder at 0: off the
                // screen ($DD88), into the oil drum ($DBE0).
                _ => {
                    v[1] = 0xFF;
                    v[3] = 0;
                }
            },
        }
        let packed = v.iter().rev().fold(0u64, |acc, b| acc << 8 | u64::from(*b));
        cx.set_counter(&name, packed as i64);
    }
}

/// The play term's frame ($C89F: the game in play as the frame started,
/// `over` 0 or the attract demo) with the play routine run from $CEA2 to
/// $CEFB: not paused, on 25m (the counter `stage` 0, $53 = 1).
fn runs(cx: &ExtCx<'_>) -> bool {
    cx.counter("mario_96").is_some()
        && cx.has_param("touch_median")
        && cx.state_at_start("game", "playing")
        && cx.state_at_start("pause", "running")
        && (cx.counter_at_start("over").unwrap_or(0) < 1
            || cx.counter_at_start("attract").unwrap_or(0) >= 1)
        && cx.counter_at_start("stage").is_none_or(|s| s == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run by hand: `GP_PLAY_SPLIT` is a folder of `gp cycles split
    /// --ram-out` outputs (CLIP.jsonl and CLIP.ram). Each play frame's
    /// $EC29 against [`touch`] on the RAM at the next NMI.
    #[test]
    fn touch_cycles_match_the_split() {
        let (Ok(dir), Ok(prg)) = (std::env::var("GP_PLAY_SPLIT"), std::env::var("GP_PLAY_PRG"))
        else {
            return;
        };
        let prg = std::fs::read(prg).unwrap();
        let base = 0x10000 - prg.len();
        let rom = |a: u16| prg[usize::from(a) - base];
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let p = e.ok()?.path();
                Some(p.to_str()?.strip_suffix(".jsonl")?.to_string())
            })
            .collect();
        names.sort();
        let (mut n, mut ok, mut skipped) = (0u64, 0u64, 0u64);
        let mut small_ok = std::collections::BTreeMap::<&str, (u64, u64)>::new();
        let routine = |line: &str, name: &str| -> i64 {
            line.find(&format!("\"{name}\":")).map_or(0, |at| {
                line[at + name.len() + 3..]
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
        };
        for name in names {
            let Ok(ram) = std::fs::read(format!("{name}.ram")) else {
                continue;
            };
            let lines: Vec<String> = std::fs::read_to_string(format!("{name}.jsonl"))
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect();
            for (i, line) in lines.iter().enumerate() {
                let Some(r) = ram.get((i + 1) * 2048..(i + 2) * 2048) else {
                    break;
                };
                let r0 = &ram[i * 2048..(i + 1) * 2048];
                let Some(at) = line.find("\"ec29\":") else {
                    continue;
                };
                let frame = |l: &str| -> i64 { l[9..].split(',').next().unwrap().parse().unwrap() };
                if lines.get(i + 1).is_none_or(|l| frame(l) != frame(line) + 1) {
                    continue;
                }
                if r[0x53] != 1 {
                    continue;
                }
                let want: i64 = line[at + 7..]
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                let spr = |k: usize| (r[0x203 + 16 * k], r[0x200 + 16 * k]);
                let t = Touch {
                    mario: spr(0),
                    state: r[0x96],
                    barrels: std::array::from_fn(|s| spr(s + 3)),
                    fires: [spr(1), spr(2)],
                    sideways: r[0x56] & 3 != 0,
                    jump_way: r[0x9E],
                    hammer_touch: r[0xA0],
                    swing: r[0x9F],
                    facing: r[0x57],
                };
                let small = Small {
                    state_0: r0[0x96],
                    demo: r0[0x58] != 0,
                    pad: r[0x15 + 2 * usize::from(r0[0x52])] & 0x8F,
                    pauline_wait_0: r0[0x39],
                    pauline_step_0: r0[0xB8],
                    pauline_body_0: r0[0x2F1],
                    pauline_calls_0: r0[0xB7],
                    drum_0: r0[0xAD],
                    drum_1: r[0xAD],
                    drum_timer_0: r0[0x38],
                    drum_flame_0: r0[0x2E1],
                };
                for (name, got) in [
                    ("d175", pad(&small, &rom).unwrap_or(-1)),
                    ("ea5f", pauline(&small, &rom).unwrap_or(-1)),
                    ("e19a", drum(&small)),
                ] {
                    let want = routine(line, name);
                    if got != 0 || want != 0 {
                        let e = small_ok.entry(name).or_insert((0u64, 0u64));
                        e.1 += 1;
                        if got == want {
                            e.0 += 1;
                        } else if e.1 - e.0 < 4 {
                            eprintln!("{name} {i}: got {got} want {want} {small:?}");
                        }
                    }
                }
                let Some(got) = touch(&t, &rom) else {
                    skipped += 1;
                    continue;
                };
                n += 1;
                if got == want {
                    ok += 1;
                } else if n - ok < 10 {
                    eprintln!("{name} {i}: got {got} want {want}");
                }
            }
        }
        eprintln!("exact {ok} of {n}, {skipped} touching; {small_ok:?}");
        assert_eq!(ok, n);
    }

    #[test]
    fn a_free_slot_fails_on_its_y() {
        let mario = corners(100, 150, [5, 5, 10, 10]);
        assert_eq!(first_fail(mario, corners(0, 0xFF, [4, 4, 12, 13])), 1);
    }
}
