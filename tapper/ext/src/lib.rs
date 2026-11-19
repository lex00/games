//! Tapper's `ext` hooks, written against gp-sim's [`Ext`] trait. Every
//! number a hook uses is the model's `ext_params`, with its evidence in the
//! model (frames of the MAME oracle's captures, which gp-arcade matches on
//! every frame).
//!
//! | Hook | Entity | What it does |
//! |---|---|---|
//! | `tapper_bartender` | player | Walks along his bar, moves to the next bar's tap when the stick is pushed up or down, and steps back from the tap when a mug is sent |
//! | `tapper_sound` | sound | The main CPU's requests to the sound board: the queue the hooks fill as the game's code asks, sent to the board's latches at each frame's start ([`sound`]) |
//!
//! The bartender's words: `ext[0]` his bar (0 the bottom bar on the screen,
//! 3 the top; the game's E6F9; up adds one), `ext[1]` frames left before he lands at
//! the bar he is moving to, `ext[2]` that bar, `ext[3]` frames the tap has
//! been held at the tap.
//!
//! The taps are bodies of kind `tap`, one a bar in bar order, placed from
//! the game's objects at E346 (x and mirroring) when a clip starts: a tap
//! mirrored is on the bar's left end.

use gp_sim::mechanic::{Carrier, Param, Primitive, Record};
use gp_sim::{Ext, ExtCx, Exts, Fx};

mod sound;
use sound::{ask, ask_ld, ask_run};

/// Every hook, by the name the model's `ext` gives it.
pub fn exts() -> Exts {
    let mut out = Exts::new();
    out.insert("tapper_bartender".into(), Box::new(Bartender) as Box<dyn Ext>);
    out.insert("tapper_flow".into(), Box::new(Flow));
    out.insert("tapper_bars".into(), Box::new(Bars));
    out.insert("tapper_sound".into(), Box::new(sound::Sound));
    out.insert("tapper_bar_end".into(), Box::new(BarEnd));
    out.insert("tapper_bonus_object".into(), Box::new(BonusObject));
    out.insert("tapper_bartender_part".into(), Box::new(BartenderPart));
    out
}

fn p(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.param(name).floor()
}

/// MAME's 4-way joystick filter (ioport.cpp, digital_joystick::frame_update,
/// and gp_arcade::FourWay): the cabinet's stick moves four ways, so a
/// diagonal keeps the direction that just changed, the horizontal one from
/// rest, and opposing directions cancel. `held` is right, left, down, up;
/// its state (the directions held last frame and the ones chosen, a nibble
/// each as up, down, left, right bits) is kept in `word` from bit 8.
fn four_way(word: &mut i64, held: [bool; 4]) -> [bool; 4] {
    const UP: i64 = 1;
    const DOWN: i64 = 2;
    const LEFT: i64 = 4;
    const RIGHT: i64 = 8;
    let previous = (*word >> 8) & 15;
    let mut chosen = (*word >> 12) & 15;
    let mut current = 0;
    for (on, bit) in [(held[3], UP), (held[2], DOWN), (held[1], LEFT), (held[0], RIGHT)] {
        if on {
            current |= bit;
        }
    }
    if current & (UP | DOWN) == UP | DOWN {
        current &= !(UP | DOWN);
    }
    if current & (LEFT | RIGHT) == LEFT | RIGHT {
        current &= !(LEFT | RIGHT);
    }
    if current != previous {
        chosen = current;
        let diagonal = |v: i64| v & (UP | DOWN) != 0 && v & (LEFT | RIGHT) != 0;
        if diagonal(chosen) {
            chosen ^= chosen & previous;
        }
        if diagonal(chosen) {
            chosen &= !(UP | DOWN);
        }
    }
    // This frame's directions become the next frame's previous.
    *word = (*word & 0xff) | (current << 8) | (chosen << 12);
    [
        held[0] && chosen & RIGHT != 0,
        held[1] && chosen & LEFT != 0,
        held[2] && chosen & DOWN != 0,
        held[3] && chosen & UP != 0,
    ]
}

/// The bartender ($E2E6's object).
pub struct Bartender;

/// Which branch of the play routine's $06A4 ran for the bartender this
/// frame, for his pictures (draw_pose).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pose {
    /// The play routine didn't run.
    Still,
    /// The setup's own frame, on which the hook doesn't move him.
    SetupFrame,
    /// It ran and changed none of his objects (a move's last frame, a
    /// stick held after a move).
    Play,
    /// $214B's start of a move to another bar.
    SwitchStart,
    /// A move's frame with its step E731 (1 to 6; 6 lands, $0A8E).
    Switch(i64),
    /// The tap held ($1206).
    Pour,
    /// Neither stick nor tap: $13CC with a mug to send (E718), else $2085
    /// while the pour's count E715 runs, else $2231.
    Rest,
    /// The send ($13CC), when he steps back from the tap.
    Sent,
    /// The walk ($19EF left, $1C02 right), with the bob's new step on a
    /// frame E727 ran out.
    Walk { left: bool, step: Option<i64> },
    /// A level's end script (tapper_bars counts it in the flow's word 3):
    /// its frame as if $4742 ran once, the script, $51A9 in place of
    /// $50EF, and script 4's frames: $4546's start and the drop's landing.
    End { n: i64, script: i64, interlude_b: bool, drop: i64, land: i64 },
    /// A customer's slide ($1CE1, the flow's sub 10): its frame and the
    /// phase past the bar's end, as tapper_bars counts them.
    Slide { n: i64, phase: i64 },
}

/// One of the game's 4-byte objects: y, flags, picture, x.
#[derive(Clone, Copy, Default, Debug)]
struct Obj {
    y: i64,
    flags: i64,
    pic: i64,
    x: i64,
}

/// A byte of a routine the model lifts as a row of its code's bytes
/// (`table`, from tapper's address `start`): the byte at `at`, an
/// instruction's immediate.
fn imm(cx: &ExtCx<'_>, table: &str, start: i64, at: i64) -> i64 {
    usize::try_from(at - start).ok().and_then(|k| cx.table(table).get(k).copied()).unwrap_or(0)
}

/// $1351: his pose at the tap, his arms and head on him.
fn at_tap_pose(cx: &ExtCx<'_>, me: &mut Obj, parts: &mut [Obj; 5], tap_left: bool) {
    const P: i64 = 0x1351;
    let f = if tap_left { imm(cx, "bartender_pour", P, 0x1383) } else { imm(cx, "bartender_pour", P, 0x1371) };
    me.flags = f;
    me.pic = imm(cx, "bartender_pour", P, 0x136B);
    for k in [0, 1, 4] {
        parts[k].flags = f;
    }
    parts[4].x = me.x;
    let mut x1 = me.x + imm(cx, "bartender_pour", P, 0x1394);
    if tap_left {
        x1 -= imm(cx, "bartender_pour", P, 0x139A);
    }
    parts[0].x = x1 & 0xff;
    parts[1].x = x1 & 0xff;
    parts[0].pic = imm(cx, "bartender_pour", P, 0x13A4);
    parts[0].y = me.y;
    parts[4].y = (me.y - imm(cx, "bartender_pour", P, 0x13BA)) & 0xff;
    parts[1].y = (parts[4].y + imm(cx, "bartender_pour", P, 0x13BF)) & 0xff;
    parts[1].pic = imm(cx, "bartender_pour", P, 0x13C6);
    parts[4].pic = imm(cx, "bartender_pour", P, 0x13CA);
}

/// $2085: the tap let go before the mug is full.
fn let_go_pose(cx: &ExtCx<'_>, me: &mut Obj, parts: &mut [Obj; 5], tap_x: i64, tap_left: bool) {
    const S: i64 = 0x2085;
    me.pic = imm(cx, "bartender_wait", S, 0x2088);
    parts[0].pic = imm(cx, "bartender_wait", S, 0x208C);
    parts[1].pic = imm(cx, "bartender_wait", S, 0x2094);
    let (f1, f2) = if tap_left { (0x20C9, 0x20D0) } else { (0x2090, 0x2098) };
    parts[0].flags = imm(cx, "bartender_wait", S, f1);
    parts[1].flags = imm(cx, "bartender_wait", S, f2);
    parts[0].x = tap_x;
    parts[1].x = tap_x;
    parts[1].y = (me.y + imm(cx, "bartender_wait", S, 0x20D5)) & 0xff;
    parts[3].y = 0;
}

/// A customer's slide ($1CE1) at its frame `n`: his pictures as he is
/// taken ($1D72 to $1DD8) and 15 frames on ($1DF6 to $1E5F). tapper_bars,
/// which moves him after this hook runs, places his arm and head
/// (`place_parts`).
fn slide_pose(cx: &ExtCx<'_>, me: &mut Obj, parts: &mut [Obj; 5], left: bool, n: i64, phase: i64) {
    let c = |at: i64| imm(cx, "slide_pose", 0x1D72, at);
    let along = if left { -0x10 } else { 0x10 };
    // Set again while they aren't, so a start inside the slide's waits
    // (tapper.die.001) has them too.
    if (36..51).contains(&n) && me.pic != c(0x1D75) {
        me.pic = c(0x1D75);
        parts[4].pic = c(0x1D79);
        parts[0].pic = c(0x1D7D);
        let f = if left { c(0x1DD7) } else { c(0x1DBC) };
        me.flags = f;
        parts[4].flags = f;
        parts[0].flags = if left { c(0x1DCE) } else { c(0x1D89) };
        for k in 1..4 {
            parts[k].y = 0;
        }
    } else if n >= 51 && phase < 4 && me.pic != c(0x1DFD) {
        parts[0].y = 0;
        me.pic = c(0x1DFD);
        parts[4].pic = c(0x1E01);
        let f = if left { c(0x1E5E) } else { c(0x1E42) };
        me.flags = f;
        parts[4].flags = f;
    }
    let _ = along;
}

/// A level's end script's pictures at its frame `n` (counted as tapper_bars
/// counts it, as if $4742 ran once), set as the script sets them; between
/// its steps his objects stay. tapper_bars moves him. In rbtapper the end
/// waves his picture where tapper mirrors him ($47E7), which the model
/// doesn't follow.
fn end_script_pose(
    cx: &ExtCx<'_>,
    me: &mut Obj,
    parts: &mut [Obj; 5],
    tap_y: &mut Option<i64>,
    (tap_x, left): (i64, bool),
    (n, script, interlude_b, drop, land): (i64, i64, bool, i64, i64),
) {
    let a = |at: i64| imm(cx, "end_pose_a", 0x4603, at);
    let b = |at: i64| imm(cx, "end_pose_b", 0x467B, at);
    // The sprite's mirror bit, which the scripts flip ($466E, $4783, $492F).
    const MIRROR: i64 = 0x10;
    // $4603, 15 frames in: standing, his arms and the rest gone, his head
    // above him ($4625 to $4653), and the tap back ($0679).
    if n == 16 {
        me.pic = a(0x462C);
        me.flags = a(0x4630);
        parts[4].pic = a(0x4634);
        parts[4].flags = a(0x4638);
        parts[4].x = (me.x - 3) & 0xff;
        parts[4].y = (me.y - 0x10) & 0xff;
        for o in parts.iter_mut().take(4) {
            o.y = 0;
        }
        *tap_y = Some(1);
    }
    // $4665: mirrored 4 times, 10 frames apart.
    if [26, 36, 46, 56].contains(&n) {
        me.flags ^= MIRROR;
    }
    // $4742: the tap hidden ($1BD4), the pose at it ($1351) and the tap's
    // handle down beside him ($4749 to $476F: 2 further on a bar whose tap
    // is on the left).
    let at_tap = (tap_x + if left { 2 } else { 0 }) & 0xff;
    if n == 56 {
        *tap_y = Some(0);
        at_tap_pose(cx, me, parts, left);
        parts[3] = Obj { y: parts[0].y, flags: b(0x476B), pic: b(0x4767), x: at_tap };
    }
    // $477D: the handle mirrored 6 times, 2 frames apart.
    if (58..=68).contains(&n) && n % 2 == 0 {
        parts[3].flags ^= MIRROR;
        parts[3].y = parts[0].y;
    }
    // $47DF: the full mug, then the let-go ($2085) and the tap back.
    if n == 70 {
        parts[2] = Obj { y: parts[0].y, flags: b(0x4808), pic: b(0x4801), x: at_tap };
        let_go_pose(cx, me, parts, tap_x, left);
        *tap_y = Some(1);
    }
    // $4815, 15 frames on: the mug in his hand, his arm 16 toward the
    // far end, his head 2 back ($4844).
    if n == 85 {
        parts[1].y = 0;
        parts[2].y = 0;
        parts[3].y = 0;
        me.pic = b(0x4822);
        parts[0].x = (me.x + if left { -0x10 } else { 0x10 }) & 0xff;
        parts[0].pic = b(0x4839);
        parts[0].y = (me.y + 1) & 0xff;
        parts[4].x = (parts[4].x + if left { 2 } else { -2 }) & 0xff;
    }
    // Back in $4603 ($4687 to $46E7): the mug slid along the bar.
    if n == 109 {
        parts[0].y = 0;
        me.pic = b(0x468E);
    }
    if n == 124 {
        me.pic = b(0x46CF);
    }
    if n == 128 {
        me.pic = b(0x46EA);
    }
    // $4708, scripts 0 and 3 after $58C5 (172): his arm out by his head.
    if (script == 0 || script == 3) && n == 172 {
        me.pic = b(0x470B);
        parts[4].pic = b(0x470F);
        parts[0].pic = b(0x4713);
        parts[0].y = parts[4].y;
        if left {
            parts[0].x = (parts[4].x - 8) & 0xff;
            parts[0].flags = b(0x473B);
            parts[4].x = (parts[0].x + 0x10) & 0xff;
        } else {
            parts[0].x = (parts[4].x + 8) & 0xff;
            parts[4].x = (parts[0].x - 0x10) & 0xff;
        }
    }
    match script {
        // $4881: $45E3 (12 frames), then $48FC.
        0 => {
            if n == 184 {
                parts[0].y = 0;
                parts[4].x = (parts[4].x + if left { -8 } else { 8 }) & 0xff;
                me.pic = b(0x4910);
                parts[4].pic = b(0x4914);
            }
            if n == 199 {
                me.pic = imm(cx, "end_pose_c", 0x491B, 0x491E);
            }
            if n == 209 || n == 219 {
                me.flags ^= MIRROR;
            }
        }
        // $489A: after $5904 (170) his arm up, then $45E3.
        1 => {
            if n == 170 {
                me.pic = b(0x48A3);
                parts[4].pic = b(0x48A7);
                parts[0].pic = b(0x48AB);
                parts[0].y = parts[4].y;
                parts[4].x = (parts[4].x + if left { -4 } else { 4 }) & 0xff;
            }
            if n == 182 {
                parts[0].y = 0;
            }
        }
        // $48D1: the hops' pictures ($50EF, or $51A9's, from 192);
        // tapper_bars puts his head 16 above him as he hops.
        3 => {
            let h = |at: i64| imm(cx, "end_hops", 0x50EF, at);
            if n == 192 {
                let (p, f, hp, hf) = if interlude_b { (0x5201, 0x5205, 0x520D, 0x5211) } else { (0x5147, 0x514B, 0x5153, 0x5157) };
                me.pic = h(p);
                me.flags = h(f);
                parts[0].y = 0;
                parts[4].pic = h(hp);
                parts[4].flags = h(hf);
                parts[4].x = (me.x + 1) & 0xff;
            }
        }
        // $48F5: $594D lifts the object above him, then his pictures
        // ($598D, his head 3 toward the far end; $59CB 60 on), and $4546's
        // drop: its start ($455B) and its landing ($45B7).
        4 => {
            let l = |at: i64| imm(cx, "end_lift", 0x594D, at);
            let d = |at: i64| imm(cx, "end_drop", 0x4546, at);
            if n == drop - 100 {
                me.pic = l(0x5990);
                parts[4].pic = l(0x5994);
                parts[4].x = (parts[4].x + if left { -3 } else { 3 }) & 0xff;
            }
            if n == drop - 40 {
                me.pic = l(0x59CE);
                parts[4].pic = l(0x59D2);
            }
            if n == drop {
                me.pic = d(0x455E);
                parts[4].pic = d(0x4562);
            }
            if n == land {
                me.pic = d(0x45BA);
                parts[4].pic = d(0x45BE);
            }
        }
        _ => {}
    }
}

/// The bartender's six objects after the play routine's branch `ev` ran:
/// his own (E2E6, whose y and x the hook keeps and whose flags and
/// picture are word 2's bytes 2 and 3) and five bodies of kind
/// bartender_part (E2EA on), set as $06A4's routines set them. The
/// routines' state is word 2's bytes 4 to 7: E706 (standing's wait),
/// E715 (the pour's count), E714 (the mug full; bit 1 E72C, the frame
/// after a send, which $06B5 skips) and E718 (a mug to send). Out of play
/// they stay as they are, the scripts' pictures not being modelled, and
/// go when his object is emptied (a setup's $5F31) or out of a game.
fn draw_pose(cx: &mut ExtCx<'_>, ev: Pose, x_before: i64, spawned: usize) {
    let Some(pk) = cx.kind("bartender_part") else { return };
    let mut ids: [Option<usize>; 5] = [None; 5];
    let mut parts = [Obj::default(); 5];
    for (id, b) in cx.world.bodies.iter().enumerate() {
        if b.alive && b.kind == pk && (1..=5).contains(&b.ext[1]) {
            let k = usize::try_from(b.ext[1] - 1).unwrap_or(0);
            ids[k] = Some(id);
            parts[k] = Obj { y: b.y.floor(), flags: b.ext[0] & 0xff, pic: (b.ext[0] >> 8) & 0xff, x: b.x.floor() };
        }
    }
    let w = cx.me.ext[2];
    let byte = |n: u32| (w >> (8 * n)) & 0xff;
    let mut me = Obj { y: cx.me.y.floor(), flags: byte(2), pic: byte(3), x: cx.me.x.floor() };
    let (mut e706, mut e715, mut e714, mut e718) = (byte(4), byte(5), byte(6), byte(7));
    let bar = cx.me.ext[0] & 3;
    let tap = Bartender::tap(cx, bar);
    let (tap_x, tap_left) = tap.unwrap_or((0, false));
    // $0679 puts the bar's tap back (its y from $4DCC), $1BD4 hides it
    // while he pours: his objects draw it then.
    let mut tap_y: Option<i64> = None;
    let hide = out_of_play(cx) || me.y == 0;
    let hold = e714 & 2 != 0;
    e714 &= 1;
    if hide {
        for o in &mut parts {
            o.y = 0;
        }
    } else if hold && ev != Pose::Still {
        // $06B5: the frame after a send runs none of it.
    } else {
        match ev {
            Pose::Still | Pose::Play => {}
            Pose::SwitchStart => {
                const S: i64 = 0x214B;
                let f = if tap_left { imm(cx, "bartender_switch", S, 0x2175) } else { imm(cx, "bartender_switch", S, 0x2171) };
                me.flags = f;
                parts[4].flags = f;
                parts[2].y = 0;
                parts[4].pic = imm(cx, "bartender_switch", S, 0x2183);
                e715 = 0;
                e714 = 0;
                tap_y = Some(1);
                me.pic = cx.table("bartender_switch_pictures").first().copied().unwrap_or(0);
                parts[0].y = 0;
                parts[1].y = 0;
            }
            Pose::Switch(k) if k >= 6 => {
                // $0A8E: at the next bar's tap.
                const S: i64 = 0x0A8E;
                parts[4].y = (me.y - imm(cx, "bartender_land", S, 0x0B0C)) & 0xff;
                parts[4].pic = imm(cx, "bartender_land", S, 0x0B13);
                parts[4].x = me.x;
                me.pic = cx.table("bartender_switch_pictures").get(6).copied().unwrap_or(me.pic);
            }
            Pose::Switch(k) => {
                me.pic = usize::try_from(k).ok().and_then(|k| cx.table("bartender_switch_pictures").get(k).copied()).unwrap_or(me.pic);
                if me.pic == imm(cx, "bartender_switch", 0x214B, 0x21D6) {
                    parts[4].y = 0;
                }
            }
            Pose::Pour => {
                // $1206: $1BD4 hides the tap, $1351 the pose at it.
                tap_y = Some(0);
                at_tap_pose(cx, &mut me, &mut parts, tap_left);
                const T: i64 = 0x1206;
                let at_tap = |cx: &ExtCx<'_>, plus: i64| (tap_x + if tap_left { imm(cx, "bartender_tap", T, plus) } else { 0 }) & 0xff;
                if e715 != 0 {
                    e715 -= 1;
                    if e715 == 0 {
                        // $125E: the mug is full; the full mug on the bar.
                        e714 = 1;
                        e718 = 1;
                        parts[2] = Obj {
                            y: parts[0].y,
                            flags: imm(cx, "bartender_tap", T, 0x1294),
                            pic: imm(cx, "bartender_tap", T, 0x128D),
                            x: at_tap(cx, 0x1282),
                        };
                    } else {
                        // $129D: the handle's picture mirrored each frame.
                        parts[3].flags ^= imm(cx, "bartender_tap", T, 0x12A1);
                        parts[3].y = parts[0].y;
                    }
                } else if e714 == 0 {
                    // $121D: the pour starts, the tap's handle down.
                    e715 = imm(cx, "bartender_tap", T, 0x121E);
                    parts[3] = Obj {
                        y: parts[0].y,
                        flags: imm(cx, "bartender_tap", T, 0x1249),
                        pic: imm(cx, "bartender_tap", T, 0x1245),
                        x: at_tap(cx, 0x123E),
                    };
                }
            }
            Pose::Sent | Pose::Rest if e718 != 0 && e714 != 0 => {
                // $13CC: he steps back (the hook moves him) with the mug
                // gone from his hands. With E714 0 (he walked away from
                // the tap with the mug full) it runs $2231 instead.
                tap_y = Some(1);
                const S: i64 = 0x13CC;
                e714 = 0;
                e715 = 0;
                e718 = 0;
                parts[1].y = 0;
                parts[2].y = 0;
                parts[3].y = 0;
                if tap_left {
                    let f = imm(cx, "bartender_send", S, 0x1434);
                    me.flags = f;
                    parts[4].flags = f;
                    parts[0].flags = f;
                    parts[4].x = (me.x - imm(cx, "bartender_send", S, 0x1436)) & 0xff;
                    parts[0].x = (parts[4].x - imm(cx, "bartender_send", S, 0x143F)) & 0xff;
                } else {
                    parts[4].x = (me.x + imm(cx, "bartender_send", S, 0x1421)) & 0xff;
                    parts[0].x = (parts[4].x + imm(cx, "bartender_send", S, 0x1426)) & 0xff;
                }
                parts[4].pic = imm(cx, "bartender_send", S, 0x144A);
                me.pic = imm(cx, "bartender_send", S, 0x144C);
                parts[0].pic = imm(cx, "bartender_send", S, 0x1451);
                e714 |= 2;
            }
            Pose::Sent | Pose::Rest if e718 == 0 && e715 != 0 => {
                // $2085: let go before the mug is full.
                let_go_pose(cx, &mut me, &mut parts, tap_x, tap_left);
            }
            Pose::Slide { n, phase } => {
                slide_pose(cx, &mut me, &mut parts, tap_left, n, phase);
            }
            Pose::End { n, script, interlude_b, drop, land } => {
                end_script_pose(cx, &mut me, &mut parts, &mut tap_y, (tap_x, tap_left), (n, script, interlude_b, drop, land));
            }
            // The setup's frame in the hook is the play routine's first
            // in the original (tapper.poweron.001: E706 6 on 1101, the
            // frame after $4BCB put him at the tap).
            Pose::Sent | Pose::Rest | Pose::SetupFrame => {
                // $2231: standing, his picture and his head's changing
                // every E706 frames.
                const S: i64 = 0x2231;
                parts[3].y = 0;
                parts[2].y = 0;
                let due = if e706 == 0 {
                    true
                } else {
                    e706 -= 1;
                    e706 == 0
                };
                if due {
                    tap_y = Some(1);
                    parts[1].y = 0;
                    parts[0].y = 0;
                    parts[4].x = me.x;
                    parts[4].y = (me.y - imm(cx, "bartender_idle", S, 0x226E)) & 0xff;
                    let (a, b, t) = if me.pic == imm(cx, "bartender_idle", S, 0x227A) {
                        (0x227E, 0x2280, 0x2288)
                    } else {
                        (0x228E, 0x2290, 0x2298)
                    };
                    me.pic = imm(cx, "bartender_idle", S, a);
                    parts[4].pic = imm(cx, "bartender_idle", S, b);
                    e706 = imm(cx, "bartender_idle", S, t);
                }
            }
            Pose::Walk { left, step } => {
                tap_y = Some(1);
                e715 = 0;
                let dx = me.x - x_before;
                if let Some(next) = step {
                    // $1A24: the step's pictures from the row at $1AD5
                    // (four bytes a step, the last the bob), arms and head
                    // put on him again.
                    let (name, s): (&str, i64) = if left { ("bartender_walk_left", 0x19EF) } else { ("bartender_walk_right", 0x1C02) };
                    let row = |cx: &ExtCx<'_>, n: i64| imm(cx, "bartender_walk_left", 0x19EF, 0x1AD5 + 4 * next + n);
                    me.pic = row(cx, 0);
                    parts[0].pic = row(cx, 1);
                    parts[4].pic = row(cx, 2);
                    let dy = row(cx, 3);
                    let dy = if dy >= 0x80 { dy - 0x100 } else { dy };
                    if left {
                        let or = imm(cx, name, s, 0x1A3A);
                        me.flags |= or;
                        parts[0].flags = imm(cx, name, s, 0x1A46);
                        parts[4].flags |= or;
                    } else {
                        me.flags = imm(cx, name, s, 0x1C4C);
                        parts[0].flags = imm(cx, name, s, 0x1C55);
                        parts[4].flags = imm(cx, name, s, 0x1C5E);
                    }
                    parts[4].y = (parts[4].y + dy) & 0xff;
                    parts[0].y = (me.y + imm(cx, name, s, if left { 0x1A6F } else { 0x1C7A })) & 0xff;
                    parts[4].x = x_before;
                    parts[0].x = (x_before + imm(cx, name, s, if left { 0x1A7A } else { 0x1C85 })) & 0xff;
                    parts[1].y = 0;
                    parts[2].y = 0;
                    parts[3].y = 0;
                    e714 = 0;
                }
                parts[0].x = (parts[0].x + dx) & 0xff;
                parts[4].x = (parts[4].x + dx) & 0xff;
            }
        }
    }
    cx.me.ext[2] = (w & 0xffff) | (me.flags << 16) | (me.pic << 24) | (e706 << 32) | (e715 << 40) | (e714 << 48) | (e718 << 56);
    if let Some(v) = tap_y
        && let Some(tk) = cx.kind("tap")
        && let Some((id, t)) = cx.world.bodies.iter().enumerate().filter(|(_, b)| b.kind == tk).nth(usize::try_from(bar).unwrap_or(0))
    {
        // $0679: the tap's y from its bar's row of $4DCC.
        let y = if v == 0 {
            0
        } else {
            cx.table("bar_objects").get(usize::try_from(bar * 4).unwrap_or(0)).copied().unwrap_or(t.y.floor())
        };
        if t.y.floor() != y {
            let mut t = *t;
            t.y = Fx::from_int(y);
            cx.set_body(id, t);
        }
    }
    // Spawned bodies take their ids in order from the world's end, after
    // any mug the send put in this frame.
    let mut fresh = cx.world.bodies.len() + spawned;
    for (k, o) in parts.iter().enumerate() {
        let word = o.flags | (o.pic << 8);
        match ids[k] {
            Some(id) => {
                let b = cx.world.bodies[id];
                if b.y.floor() != o.y || b.x.floor() != o.x || b.ext[0] != word {
                    let mut b = b;
                    b.y = Fx::from_int(o.y);
                    b.x = Fx::from_int(o.x);
                    b.ext[0] = word;
                    cx.set_body(id, b);
                }
            }
            // A hidden one with a picture too, so a start that found it
            // hidden (tapper.die.001) has it to place.
            None if o.y != 0 || o.pic != 0 => {
                let mut b = gp_sim::systems::new_body(cx.spec, cx.world, pk, Fx::from_int(o.x), Fx::from_int(o.y), false);
                b.ext[0] = word;
                b.ext[1] = k as i64 + 1;
                cx.spawn(pk, b.x, b.y, false);
                cx.set_body(fresh, b);
                fresh += 1;
            }
            None => {}
        }
    }
}

/// One of the bartender's other five objects (E2EA on): tapper_bartender
/// sets them; on their own they stand still.
pub struct BartenderPart;

impl Ext for BartenderPart {
    fn step(&self, _cx: &mut ExtCx<'_>) {}

    fn describe(&self) -> Vec<Record> {
        vec![Record {
            name: "tapper.bartender_part",
            version: env!("CARGO_PKG_VERSION"),
            carrier: Carrier::Entity,
            system: "platformer.player",
            class: "flow",
            implementation: "gp-ext-tapper::BartenderPart",
            like: "",
            about: "One of the bartender's other five objects (E2EA to E2FD), which tapper_bartender sets as the play routine's $06A4 does",
            params: &[],
            state: &["ext[0] flags plus 256 times the picture", "ext[1] its object's place from E2EA, plus 1"],
            writes: &[],
            built_from: &[Primitive::Condition],
            ..Record::EMPTY
        }]
    }
}

const PARAMS: &[Param] = &[
    Param::new("walk_step", "fixed", "px/frame"),
    Param::new("switch_frames", "frames", "frames"),
    Param::new("switch_busy", "frames", "frames"),
    Param::new("row_height", "fixed", "px"),
    Param::new("row_bottom", "fixed", "px"),
    Param::new("stand_right", "fixed", "px"),
    Param::new("stand_left", "fixed", "px"),
    Param::new("send_step", "fixed", "px"),
    Param::new("pour_frames", "frames", "frames"),
    Param::new("bob_frames", "frames", "frames"),
    Param::new("four_way", "bool", ""),
    Param::new("mug_speed", "fixed", "px/frame"),
    Param::new("mug_out", "fixed", "px"),
    Param::new("mug_out_left", "fixed", "px"),
    Param::new("mug_above", "fixed", "px"),
];

impl Bartender {
    /// Bar `bar`'s tap: its x and whether it's on the bar's left end.
    fn tap(cx: &ExtCx<'_>, bar: i64) -> Option<(i64, bool)> {
        let kind = cx.kind("tap")?;
        cx.world
            .bodies
            .iter()
            .filter(|b| b.kind == kind)
            .nth(usize::try_from(bar).ok()?)
            .map(|b| (b.x.floor(), b.facing_left))
    }

    /// Where he stands at bar `bar`'s tap.
    fn stand(cx: &ExtCx<'_>, bar: i64) -> Option<i64> {
        let (x, left) = Self::tap(cx, bar)?;
        Some(if left {
            x + p(cx, "stand_left")
        } else {
            x - p(cx, "stand_right")
        })
    }
}

impl Ext for Bartender {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let mut ev = Pose::Still;
        let mut spawned = 0;
        let x_before = cx.me.x.floor();
        Self::act(cx, &mut ev, &mut spawned);
        draw_pose(cx, ev, x_before, spawned);
    }

    fn describe(&self) -> Vec<Record> {
        vec![Record {
            name: "tapper.bartender",
            version: env!("CARGO_PKG_VERSION"),
            carrier: Carrier::Entity,
            system: "platformer.player",
            class: "mover",
            implementation: "gp-ext-tapper::Bartender",
            like: "player-physics",
            about: "Walks along his bar, moves to the next bar's tap on up or down, and steps back from the tap when a mug is sent, by the stick or, in the attract demo, by the presses $59DD picks; his six objects' pictures as the play routine sets them",
            params: PARAMS,
            state: &["ext[0] bar", "ext[1] move frames left x8 plus the bar moving to x2 plus the stick latch", "ext[2] bob count plus 256 x bob phase; byte 2 his flags and byte 3 his picture (E2E7, E2E8), the board sprite's; bytes 4 to 7 E706, E715, E714 (bit 1: the frame after a send, E72C) and E718", "ext[3] tap held (low byte), the 4-way filter's state above"],
            reads: &["action", "tap.x", "tap.facing"],
            writes: &["self.x", "self.y", "self.facing", "bartender_part", "tap.y"],
            built_from: &[Primitive::InputBinding, Primitive::Timer, Primitive::Condition, Primitive::Quantity],
            ..Record::EMPTY
        }]
    }
}

impl Bartender {
    /// The level's end script at its frame (tapper_bars keeps the count
    /// and the draws in the flow's word 3): which script, by the list
    /// $5623 picks with the levels cleared (end_script_lists), and for
    /// script 4 the frames $4546 starts its drop and the drop lands
    /// ($4585 to $45AA: a frame, then 4 down, until within 5 of him).
    fn end_pose(cx: &ExtCx<'_>, w3: i64) -> Pose {
        let n = ((w3 >> 24) & 0xffff) + 1;
        // The second run of $4742 starts on the frame 109 draw, which
        // tapper_bars takes after this hook: on 109 he took the first
        // run's next step, so on 110 he takes the second run's first
        // (as 56: at the tap, the glass in his hand), a frame after the
        // original.
        let n = match (n, w3 & END_TWICE != 0) {
            (110, true) => 56,
            (n, true) if n >= 109 => n - 53,
            (n, _) => n,
        };
        let cleared = cx
            .kind("bars")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
            .map_or(0, |b| (b.ext[3] >> 16) & 0xff);
        let words: Vec<i64> = cx.table("end_script_lists").chunks(2).map(|w| w[0] + 256 * w.get(1).copied().unwrap_or(0)).collect();
        let base = words.iter().copied().min().unwrap_or(0);
        let list = cx.kind("tap").and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k)).map_or(0, |b| b.ext[1] & 1);
        let script = words.get(usize::try_from(list * 10 + cleared % 10).unwrap_or(0)).map_or(0, |w| (w - base) / 5);
        let shown = (w3 >> 8) & 3;
        let interlude_b = shown != 3 && w3 & END_INTERLUDE_B != 0;
        let by = cx.me.y.floor();
        let k = (238 - (by + 6) + 9).max(10) / 10;
        let drop = 109 + 19 + 2 * k + 100;
        let land = drop + (250 - by).max(0) / 4 + 1;
        Pose::End { n, script, interlude_b, drop, land }
    }

    /// The play routine's bartender ($06A4): his move, and in `ev` which
    /// of its branches ran, for his pictures.
    fn act(cx: &mut ExtCx<'_>, ev: &mut Pose, spawned: &mut usize) {
        // The play routine ($0660, from $22FC) runs only in play: in a life
        // lost (state 12, then 9) and its waits (15, 16) he stands still.
        let flow = cx
            .kind("flow")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
            .copied();
        let state = flow.map_or(10, |b| b.ext[3] & 0xff);
        // Out of a game (the attract mode, a game over) and on a game's
        // start (state 8) the play routine doesn't run; the attract
        // mode's demo game runs it in state 3 ($22D4).
        if out_of_play(cx) || state == 8 {
            return;
        }
        let demo = in_demo(cx);
        // State 11 too: a level's end ($0963 sets it the frame the last
        // customer is served), whose next frame doesn't run $0660
        // (tapper.l13.001: tap held on 20481, he stays at 187).
        if [9, 11, 12, 13, 15, 16].contains(&state) || (demo && state != 3) {
            if let Some(fb) = flow
                && (state == 15 || state == 16)
                && (fb.ext[3] >> 16) & 0xff == 8
            {
                *ev = Self::end_pose(cx, fb.ext[3]);
            }
            if let Some(fb) = flow
                && (state == 15 || state == 16)
                && (fb.ext[3] >> 16) & 0xff == 10
            {
                *ev = Pose::Slide { n: ((fb.ext[3] >> 24) & 0xff) + 1, phase: (fb.ext[2] >> 26) & 0x3f };
            }
            return;
        }
        // On the setup's own frame (the flow's word 2 at 4, set by
        // tapper_bars where the setup places him) the game is still in the
        // setup's state (E397 9, 11 or 8): $0660 runs from the next frame,
        // so a press on that frame is not seen (tapper.l13.001: up on
        // 19672, the setup's frame, moves nothing; up on 19685 lands on
        // 19692). That is the space bar's setup (E3B8 3), which waits a
        // frame more ($5E99); in the saloon $0660 runs on that frame
        // already (tapper-first: the tap held through the saloon's setup
        // after a life lost, on 1444, puts him at the tap on 1445 and
        // pours from there), so only the space bar's waits it.
        if flow.is_some_and(|b| b.ext[2] == 4 && (b.ext[3] >> 8) & 3 == 3) {
            *ev = Pose::SetupFrame;
            return;
        }
        *ev = Pose::Play;
        // The demo's presses: $59DD runs first on every frame of state 3,
        // whatever the play routine then does.
        let presses = if demo { Some(demo_presses(cx)) } else { None };
        let bar = cx.me.ext[0];
        // ext[1]: frames left of a bar move times 8, plus the bar it goes
        // to times 2, plus the stick's latch (E730): set when a move
        // starts, cleared only on a frame with no direction held and no
        // move running ($06CB), so a held or early stick moves him once.
        let latch = cx.me.ext[1] & 1;
        if cx.me.ext[1] >= 8 {
            // A move takes `switch_busy` frames (E717 from 7, $21AC) and
            // he lands at the new bar's tap `switch_frames` in (E731 at 6,
            // $21C2). It runs whatever the stick and the tap do.
            cx.me.ext[1] -= 8;
            let left = cx.me.ext[1] >> 3;
            let to = (cx.me.ext[1] >> 1) & 3;
            // $214B's step E731 counts up from 0 as E717 counts down from
            // switch_busy; on E717's last frame nothing changes.
            *ev = if left == 0 { Pose::Play } else { Pose::Switch(p(cx, "switch_busy") - left) };
            if left == p(cx, "switch_busy") - p(cx, "switch_frames") {
                // $21C2 calls $0A8E as he lands: $0D after a move up (E733
                // 1, $0711), $0E after one down ($0A97, $0A9E; the
                // `sound_customers` rows are the program from $090D).
                let up = to == (cx.me.ext[0] + 1) % 4;
                ask(cx, "sound_customers", &[if up { 0x192 } else { 0x18B }]);
                cx.me.ext[0] = to;
                if let Some(x) = Self::stand(cx, to) {
                    cx.me.x = Fx::from_int(x);
                }
                cx.me.y = Fx::from_int(p(cx, "row_bottom") + p(cx, "row_height") * to);
                cx.me.ext[3] &= !0xff;
            }
            return;
        }
        // The stick's directions in the order $06FE tries them: right,
        // left, down, up; the first held wins.
        // In the demo E38E's bits 0 to 3 ($06FE), which MAME's 4-way
        // filter doesn't see: it filters the stick, not the program's byte.
        let held: [bool; 4] = match presses {
            Some(b) => [0, 1, 2, 3].map(|k| b & (1 << k) != 0),
            None => ["right", "left", "down", "up"].map(|a| cx.held(a)),
        };
        let held = if presses.is_none() && p(cx, "four_way") != 0 {
            four_way(&mut cx.me.ext[3], held)
        } else {
            held
        };
        // The tap: E38E's bit 4 in the demo.
        let tap_held = presses.map_or_else(|| cx.held("button1"), |b| b & 0x10 != 0);
        let dir = held.iter().position(|&h| h);
        let tap = Self::tap(cx, bar);
        match dir {
            None => {
                cx.me.ext[1] &= !1;
                // The tap ($1206) puts him at the tap and pours; let go
                // with a full mug (E718), he sends it ($13CC) and steps back.
                if tap_held {
                    if let Some(x) = Self::stand(cx, bar) {
                        cx.me.x = Fx::from_int(x);
                    }
                    cx.me.ext[3] += 1;
                    // The pour's sounds ($1206): its first frame sets E715
                    // to 4; on each of the next three a stream picture
                    // asks for $38 the first time and $39 after ($12E2,
                    // $12DB), and on the fourth the mug is full: $30 and
                    // $3A ($125E, $1295). The `sound_bar` rows are the
                    // program from $125E. By E715 as his word 2 keeps it
                    // (loaded, so a start inside a pour asks on the
                    // game's frame); the frame after a send runs none.
                    let (e715, e714) = ((cx.me.ext[2] >> 40) & 0xff, (cx.me.ext[2] >> 48) & 0xff);
                    if e714 & 2 == 0 {
                        match e715 {
                            4 => ask(cx, "sound_bar", &[0x85]),
                            2 | 3 => ask(cx, "sound_bar", &[0x7E]),
                            1 => ask(cx, "sound_bar", &[0x01, 0x38]),
                            _ => {}
                        }
                    }
                    *ev = Pose::Pour;
                    return;
                }
                *ev = Pose::Rest;
                if cx.me.ext[3] & 0xff >= p(cx, "pour_frames") {
                    *ev = Pose::Sent;
                    if let Some((tap_x, left)) = tap {
                        let step = p(cx, "send_step");
                        cx.me.x = Fx::from_int(cx.me.x.floor() + if left { step } else { -step });
                        // The send ($13CC) puts a full mug in a free slot of
                        // E2FE: 4 above his y ($149E), mug_out from the tap,
                        // and sets its speed E6FE ($14D3); the mugs' move
                        // ($0742) moves it on the same frame.
                        if let Some(mug) = cx.kind("mug") {
                            let speed = if left { p(cx, "mug_speed") } else { -p(cx, "mug_speed") };
                            // $14C0 and $14CC: 24 left of a right-end tap,
                            // 27 right of a left-end one.
                            let x0 = if left { tap_x + p(cx, "mug_out_left") } else { tap_x - p(cx, "mug_out") };
                            let y0 = cx.me.y.floor() + p(cx, "mug_above");
                            let _ = speed;
                            // The mugs' move (tapper_bars) moves it on this
                            // frame, after the send.
                            // $1468: the first mug slot from E2FE whose
                            // y is 0 (the slot, plus one, in its word 2).
                            let used: Vec<i64> = cx
                                .world
                                .bodies
                                .iter()
                                .filter(|b| b.alive && b.kind == mug)
                                .map(|b| b.ext[2])
                                .collect();
                            if let Some(n) = (1..=9).find(|n| !used.contains(n)) {
                                let id = cx.world.bodies.len();
                                let mut b = gp_sim::systems::new_body(
                                    cx.spec,
                                    cx.world,
                                    mug,
                                    Fx::from_int(x0),
                                    Fx::from_int(y0),
                                    !left,
                                );
                                b.ext[2] = n;
                                // $1486 to $14C4: its flags, the bar above
                                // and bit 3 from a tap on the left.
                                b.ext[3] = (bar << 4) | if left { 8 } else { 0 };
                                // $14A1 to $14C8: its sprite's flags ($13
                                // moving left) and picture; $14D6: 6 frames
                                // to its next picture (+5).
                                let one = |n: &str| cx.table(n).first().copied().unwrap_or(0);
                                let flags = if left { one("mug_flags_left") } else { one("mug_flags") };
                                b.ext[0] = flags | (one("mug_picture_full") << 8);
                                b.ext[1] = one("mug_flip_frames") << 8;
                                cx.spawn(mug, b.x, b.y, b.facing_left);
                                cx.set_body(id, b);
                                // $14DB: the mug sent asks for $12.
                                ask(cx, "sound_bar", &[0x27E]);
                                *spawned += 1;
                            }
                            // $14D1: every full mug's speed, E6FE, is -5
                            // from now (tapper_bars keeps it, its fraction
                            // E6FD as it was).
                            if let Some(bk) = cx.kind("bars")
                                && let Some((id, b)) = cx
                                    .world
                                    .bodies
                                    .iter()
                                    .enumerate()
                                    .find(|(_, b)| b.kind == bk)
                                    .map(|(i, b)| (i, *b))
                            {
                                let mut b = b;
                                b.ext[2] = (-speed.abs() << 8) | (b.ext[2] & 0xff);
                                cx.set_body(id, b);
                            }
                        }
                    }
                }
                cx.me.ext[3] &= !0xff;
            }
            Some(d @ (2 | 3)) => {
                if latch == 0 {
                    *ev = Pose::SwitchStart;
                    // Up takes him to the bar above on the screen (one
                    // more), down to the one below, round the four.
                    let to = (bar + if d == 3 { 1 } else { 3 }) % 4;
                    cx.me.ext[1] = p(cx, "switch_busy") * 8 + to * 2 + 1;
                }
            }
            Some(d) => {
                let right = d == 0;
                // The walk's ends ($1A9D, $1CA8): the bar's row of the
                // table at $0977, its left end (+13 on a bar whose tap is
                // on the left, $1AB4) and its right end +16 (+10 more on
                // such a bar, $1CBC, $1CC2); a step that would reach past
                // one (the left: below it; the right: at it or past) isn't
                // taken.
                let tap_left = tap.is_some_and(|(_, l)| l);
                let row = usize::try_from(bar & 3).unwrap_or(0) * 4;
                let ends = cx.table("bar_ends");
                let (l_end, r_end) = (ends.get(row + 1).copied().unwrap_or(0), ends.get(row + 2).copied().unwrap_or(0));
                let lo = (l_end + if tap_left { imm(cx, "bartender_walk_left", 0x19EF, 0x1AB5) } else { 0 }) & 0xff;
                let hi = (r_end
                    + imm(cx, "bartender_walk_right", 0x1C02, 0x1CBD)
                    + if tap_left { imm(cx, "bartender_walk_right", 0x1C02, 0x1CC3) } else { 0 })
                    & 0xff;
                // His walk's bob: E727 counts 2, 1, 0 over the frames a
                // direction is held and E729 steps 0 to 3 when it runs out,
                // moving him 1 down, 1 down, 1 up, 1 up; both are kept when
                // he stops or changes bar (tapper-diagonal-4way-a, frames
                // 601 to 699).
                let (count, phase) = (cx.me.ext[2] & 0xff, (cx.me.ext[2] >> 8) & 3);
                *ev = Pose::Walk { left: !right, step: if count == 0 { Some((phase + 1) & 3) } else { None } };
                if count == 0 && phase == 3 {
                    sound::footstep(cx, !right);
                }
                if count == 0 {
                    let next = (phase + 1) & 3;
                    let dy = if next == 1 || next == 2 { 1 } else { -1 };
                    cx.me.y = Fx::from_int(cx.me.y.floor() + dy);
                    cx.me.ext[2] = (cx.me.ext[2] & !0xffff) | (p(cx, "bob_frames") - 1 + (next << 8));
                } else {
                    cx.me.ext[2] = (cx.me.ext[2] & !0xffff) | (count - 1 + (phase << 8));
                }
                let step = p(cx, "walk_step");
                let x = (cx.me.x.floor() + if right { step } else { -step }) & 0xff;
                if (right && x < hi) || (!right && x >= lo) {
                    cx.me.x = Fx::from_int(x);
                }
                cx.me.facing_left = !right;
            }
        }
    }
}

/// The game's flow between plays: for now, a level's start. The level
/// setup at $4B3F clears the level's RAM (E6F2 to E733, so E6F2 is 0 on
/// that frame and on no other in a game), waits a frame, then draws the
/// taps by the level's layout (E3AF: bit k set puts bar k's tap on its
/// left end) and puts the bartender at bar 2 ($4BCB: x $AB, or $3D when
/// bar 2's tap is on the left, y $90).
///
/// Words: `ext[0]` 1 on the frame the setup cleared the level, `ext[1]`
/// the layout in its low byte; above it, as the mapping loads them, the
/// state a wait was called in (E3A8), where it goes on (E3A9) and its B
/// (E39F), which `$232B` and `$5FE6` keep, and the lives (E4A7, bits 40
/// to 47), which tapper_bars counts down.
pub struct Flow;

const FLOW_PARAMS: &[Param] = &[
    Param::new("start_bar", "count", ""),
    Param::new("start_right", "fixed", "px"),
    Param::new("start_left", "fixed", "px"),
    Param::new("tap_right", "fixed", "px"),
    Param::new("tap_left", "fixed", "px"),
    Param::new("tap_step", "fixed", "px"),
];

/// The flow's word 2 from bit 40: the frames since the attract mode's
/// logo began (sub 0 of the attract, 16 bits), and at bit 60 a flag for
/// the frame a game started on, which tapper_bars reads. The flag is
/// cleared every frame, so it keeps clear of the bonus round's fields
/// (BONUS_MASK, bits 40 to 57), which share the word in a game.
const ATTRACT_SHIFT: u32 = 40;
const ATTRACT_MASK: i64 = 0xffff << ATTRACT_SHIFT;
const STARTED: i64 = 1 << 60;

impl Flow {
    /// The coin routine ($0213, four times a frame from the CTC's
    /// interrupt at $0158): the coin switch (port 0 bit 0, low while a
    /// coin is held) is taken as settled when two reads agree ($021A),
    /// and a credit is counted when it settles let go after a coin
    /// ($0229 to $0231), so on the frame the coin is let go. Coins go
    /// toward a credit by the defaults' coins a credit (E000) and give
    /// their credits a coin (E001, $0242 to $0255); at most 9 ($0164).
    /// With coin_on_press (Normal) the credit comes on the frame the coin
    /// goes in.
    fn coin(cx: &mut ExtCx<'_>) -> i64 {
        let held = cx.held("coin");
        let was = cx.counter("coin_held").unwrap_or(0) != 0;
        let on_press = p(cx, "coin_on_press") != 0;
        // A start on the boot's first frame loads E097 from RAM the boot
        // hasn't set yet (0, as if held); the boot then sets it let go,
        // which no coin made ($022C runs only for a coin): no credit
        // (tapper.poweron.001: E097 0 on 94, set on 95, E011 0).
        let boot = cx.frame <= 1 && cx.counter("mode") == Some(0) && cx.me.ext[3] & 0xff == 0;
        let counts = !boot && if on_press { held && !was } else { !held && was };
        if counts {
            let defaults = cx.table("defaults");
            let (per_credit, per_coin) = (
                defaults.first().copied().unwrap_or(1),
                defaults.get(1).copied().unwrap_or(1),
            );
            let coins = cx.counter("coins").unwrap_or(0) + 1;
            let mut credits = cx.counter("credits").unwrap_or(0);
            if coins < per_credit {
                cx.set_counter("coins", coins);
            } else {
                credits += per_coin;
                cx.set_counter("coins", 0);
            }
            cx.set_counter("credits", credits.min(9));
            cx.set_counter("coin_held", i64::from(held));
            return credits.min(9);
        }
        cx.set_counter("coin_held", i64::from(held));
        cx.counter("credits").unwrap_or(0)
    }

    /// Out of a game: the boot's wait, the attract mode and the start
    /// ($584F) that begins a game. True when the frame is done.
    /// `now` is the credits after this frame's coin; $584F reads them
    /// before it (the main loop's pass comes first in the frame).
    fn out_of_game(cx: &mut ExtCx<'_>, now: i64) -> bool {
        let mode = cx.counter("mode").unwrap_or(0);
        let state = cx.me.ext[3] & 0xff;
        cx.me.ext[2] &= !STARTED;
        // The main loop ($22A3) calls $584F before its dispatch, except
        // on a frame a routine goes on after a frame's wait of its own
        // ($60F6, not the main loop's $232B): the attract's logo after
        // $3D91, its demo after $2F45 and after $3324 and $3342, and the
        // game over's after $2B82's $5E99 and $2C76's and $2CA2's waits.
        let t = (cx.me.ext[2] & ATTRACT_MASK) >> ATTRACT_SHIFT;
        let demo = in_demo(cx);
        let main_loop = match mode {
            // The demo's setup goes on after waits of its own ($5E99 and
            // $60F6 in $4B3F); the rest of the demo runs from the main
            // loop, its high scores' wait too ($232B).
            1 if demo => !(state == 15 && (cx.me.ext[3] >> 16) & 0xff == DEMO_SETUP),
            1 => {
                // This frame's place in the attract mode.
                let t = t + 1;
                let demo_waits = attract_at(cx, "demo_waits_at");
                !(t == p(cx, "logo_wait_at") || t == attract_at(cx, "demo_wait_at") || t == demo_waits || t == demo_waits + 1)
            }
            // The game over: its count is word 3 from bit 24 (sub 2).
            2 => state != 13 || (cx.me.ext[3] >> 24) & 0xffff >= 3,
            _ => false,
        };
        let credits = cx.counter("credits").unwrap_or(0);
        if mode == 1 && !demo && Self::scores_resumed(cx) {
            return false;
        }
        if main_loop && credits > 0 && cx.held("start1") {
            // $5860: a credit for a one-player game, state 8 ($5874);
            // the dispatch then runs $4AEF, the game's start: E3ED from
            // a draw of 0 to 2 ($4AFA), the level row from the defaults'
            // E006 ($4B08), the lives from their E005 ($4B1F), out of the
            // attract mode (E3BD 0, $4B31), and $4AEF's clear of E3EC to
            // E6F1 ($4AF5) puts the levels cleared, the score and the
            // customers' records at 0. Then $4B3F, the level's setup:
            // the bar shown from no levels cleared ($3C48) and a frame's
            // wait in $5E99 ($4B47); tapper_bars goes on from the next.
            cx.set_counter("credits", now - 1);
            cx.set_counter("mode", 0);
            if demo {
                cx.set_counter("demo", 0);
            }
            cx.set_counter("cleared", 0);
            cx.set_counter("score", 0);
            let defaults = cx.table("defaults").to_vec();
            let lives = defaults.get(5).copied().unwrap_or(3);
            let row = defaults.get(6).copied().unwrap_or(1);
            cx.set_counter("lives", lives);
            let draw = draw_at_most(cx, "game_start", 2);
            cx.me.ext[3] = 8;
            cx.me.ext[2] = STARTED;
            // $4AEF's $4B34 and $4B39 ask for 2 and $2A, in state 8, on the
            // frame the main loop takes the start (not the press's: a frame
            // of the attract's own waits puts it a frame later).
            ask(cx, "sound_game", &[1, 6]);
            cx.me.ext[1] = (cx.me.ext[1] & 0xff) | (lives << 40);
            cx.me.ext[0] = 0;
            if let Some(tk) = cx.kind("tap")
                && let Some((id, b)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == tk)
            {
                let mut b = *b;
                b.ext[1] = draw;
                cx.set_body(id, b);
            }
            if let Some(bk) = cx.kind("bars")
                && let Some((id, b)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == bk)
            {
                let mut b = *b;
                // Word 3: the levels cleared 0 and the level row; word 0:
                // the tips setting is E006 too.
                b.ext[3] = row << 24;
                b.ext[0] = (b.ext[0] & 0xffff) | (row << 24);
                cx.set_body(id, b);
            }
            // The customers' records, 16 of 0x20 bytes from E4B2: blank,
            // so slot n's is the n-th until the setup copies the bar's in.
            if let Some(rk) = cx.kind("record") {
                let have = cx.world.bodies.iter().filter(|b| b.alive && b.kind == rk).count();
                for _ in have..16 {
                    cx.spawn(rk, Fx::ZERO, Fx::ZERO, false);
                }
            }
            return true;
        }
        match (mode, state) {
            // Power-on: the boot ($0000) tests the RAM, copies the
            // defaults when the battery-backed settings fail their check
            // ($048A), clears the RAM ($005E) and waits on the sound
            // board's answers ($CFC5, a frame each $DB69), then the main
            // loop starts in state 0 (the boot_wait's frames, the first
            // 94 of the board's being before the model's first). The tips
            // setting E006 comes from the defaults ($0517).
            (0, 0) => {
                // The model's first frame is 1.
                if cx.frame <= 1
                    && let Some(bk) = cx.kind("bars")
                    && let Some((id, b)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == bk)
                {
                    let row = cx.table("defaults").get(6).copied().unwrap_or(1);
                    let mut b = *b;
                    b.ext[0] = (b.ext[0] & 0xffff) | (row << 24);
                    cx.set_body(id, b);
                }
                if i64::try_from(cx.frame).unwrap_or(i64::MAX) >= p(cx, "boot_wait") {
                    Self::attract(cx, credits);
                }
                true
            }
            // State 0 after a game over ($4A27): $22B8 again.
            (2, 0) => {
                Self::attract(cx, credits);
                true
            }
            // The demo game: tapper_bars plays it, and this hook puts the
            // taps and the bartender in at its setup as at a level's.
            (1, _) if demo => false,
            // State 0 after the demo's high scores ($4A53): $22B8 again.
            (1, 0) => {
                Self::attract(cx, credits);
                true
            }
            // $22CF, state 2: the demo's game starts ($5A64, tapper_bars).
            (1, 2) => {
                cx.set_counter("demo", 1);
                false
            }
            (1, _) => {
                // The attract mode: the logo ($3D76, state 1) for
                // attract_demo frames, then $4AAA and the demo screen
                // ($3CD8, state 6) to attract_play, when $3CD8 sets state
                // 2 and the demo's game starts.
                let t = Self::logo_resumed(cx).unwrap_or(t) + 1;
                let st = if t >= attract_at(cx, "attract_play") {
                    2
                } else if t >= attract_at(cx, "attract_demo") {
                    6
                } else if t == attract_at(cx, "attract_demo") - 1 {
                    5
                } else {
                    1
                };
                cx.me.ext[3] = (cx.me.ext[3] & !0xff) | st;
                cx.me.ext[2] = (cx.me.ext[2] & !ATTRACT_MASK) | ((t.min(0xffff)) << ATTRACT_SHIFT);
                true
            }
            _ => false,
        }
    }

    /// A start in the logo's last wait ($232B at $3DBD, called in state 1,
    /// E3A8; it goes on at $3DC0, E3A9, and sets state 5): the attract
    /// mode's frame, from the frames left of the timer for $2346, the last
    /// frame before state 5 being attract_demo less 2 (tapper.rb.l8.001
    /// starts there, 2 frames left). The place is read once.
    fn logo_resumed(cx: &mut ExtCx<'_>) -> Option<i64> {
        let place = (cx.me.ext[1] >> 16) & 0xffff;
        if place == 0 || (cx.me.ext[1] >> 8) & 0xff != 1 {
            return None;
        }
        cx.me.ext[1] &= !(0xffff << 16);
        let code = cx.table("logo_code");
        let start = cx.table("logo_start");
        let base = start.first().copied()? | (start.get(1).copied()? << 8);
        // The last wait: the one $3DC2's ld a,5 (state 5) follows.
        let last = (1..)
            .map_while(|n| wait_return(cx, "logo_code", "logo_start", n))
            .find(|&r| {
                let at = usize::try_from(r - base).unwrap_or(usize::MAX);
                code.get(at).copied() == Some(0x3E) && code.get(at + 1).copied() == Some(5)
            })?;
        if place != last {
            return None;
        }
        let left = cx
            .kind("timer")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.alive && b.kind == k && b.ext[1] == 0x2346))
            .map_or(0, |b| b.ext[0]);
        Some(attract_at(cx, "attract_demo") - 2 - left)
    }

    /// A start in the high scores' wait after a demo ($232B at $4A50,
    /// called in state 4, E3A8; it goes on at $4A53, E3A9): the demo's
    /// end goes on, its count (tapper_bars, sub DEMO_SCORES) from the
    /// frames left of the timer for $2346 (tapper.demo.002: 4 left on
    /// 1630, state 0 on 1635). The place is read once.
    fn scores_resumed(cx: &mut ExtCx<'_>) -> bool {
        let place = (cx.me.ext[1] >> 16) & 0xffff;
        if place == 0 || (cx.me.ext[1] >> 8) & 0xff != 4 || wait_return(cx, "high_scores", "high_scores_start", 1) != Some(place) {
            return false;
        }
        cx.me.ext[1] &= !(0xffff << 16);
        const H: i64 = 0x4A37;
        let wait = imm(cx, "high_scores", H, 0x4A4E) | (imm(cx, "high_scores", H, 0x4A4F) << 8);
        let left = cx
            .kind("timer")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.alive && b.kind == k && b.ext[1] == 0x2346))
            .map_or(0, |b| b.ext[0]);
        cx.me.ext[3] = (cx.me.ext[3] & 0xff00) | 15 | (DEMO_SCORES << 16) | (((wait - left).max(0) & 0xffff) << 24);
        cx.me.ext[2] = 0;
        cx.set_counter("demo", 1);
        true
    }

    /// $22B8 in state 0: with no credits $4E2C, the attract mode's logo
    /// (E3BD 1, state 1); with credits $4AAA, straight to the demo (state
    /// 6). Every object goes ($5F31).
    fn attract(cx: &mut ExtCx<'_>, credits: i64) {
        cx.set_counter("mode", 1);
        let t = if credits > 0 { attract_at(cx, "attract_demo") } else { 0 };
        cx.me.ext[3] = (cx.me.ext[3] & !0xff) | if credits > 0 { 6 } else { 1 };
        cx.me.ext[2] = (t << ATTRACT_SHIFT) & ATTRACT_MASK;
        cx.me.ext[0] = 0;
    }
}

/// The frame of the attract mode (from the logo's first, 0) that its part
/// `name` starts on: the flow's param, counted for a logo ($3D76) that
/// takes attract_demo frames, and shifted by as much as the set's own
/// logo takes more or fewer (logo_frames).
fn attract_at(cx: &ExtCx<'_>, name: &str) -> i64 {
    let param = |n: &str| {
        cx.kind("flow")
            .and_then(|k| cx.spec.kinds[k].ext_params.get(n))
            .map_or(0, |v| v.floor())
    };
    let base = param("attract_demo");
    let demo = logo_frames(cx).map_or(base, |n| n + 1);
    param(name) - base + demo
}

/// The logo's time ($3D76, state 1): the frame from the logo's first on
/// which it sets state 5, from its lifted code: one frame to its start,
/// one for its call of the frame wait ($60F6, the setup's call at $4B71
/// names it), DE frames for each call of $232B, and its loop of B
/// ($3DB7's djnz; rbtapper's also tests B with cp and jr nz). 215 frames
/// in tapper, 192 in rbtapper (tapper.rb.l8.001: state 1 on 13652, 5 on
/// 13844). None for code it can't follow.
fn logo_frames(cx: &ExtCx<'_>) -> Option<i64> {
    let code = flow_table(cx, "logo_code");
    let wait = flow_table(cx, "wait_call");
    let frame_wait = flow_table(cx, "frame_wait_call");
    let at = |k: usize| code.get(k).copied();
    let rel = |k: usize| at(k).map(|e| if e > 127 { e - 256 } else { e });
    let (mut pc, mut a, mut b, mut z, mut de, mut t) = (0usize, 0i64, 0i64, false, 0i64, 1i64);
    for _ in 0..10_000 {
        let op = at(pc)?;
        match op {
            0x01 | 0x21 | 0x32 => pc += 3,
            0x11 => {
                de = at(pc + 1)? | (at(pc + 2)? << 8);
                pc += 3;
            }
            0x06 => {
                b = at(pc + 1)?;
                pc += 2;
            }
            0x0E | 0x36 => pc += 2,
            0x3E => {
                a = at(pc + 1)?;
                pc += 2;
            }
            0xED => pc += if at(pc + 1)? == 0x53 { 4 } else { 2 },
            0x23 | 0x2B | 0x71 | 0x77 => pc += 1,
            0xAF => {
                a = 0;
                pc += 1;
            }
            0x78 => {
                a = b;
                pc += 1;
            }
            0xFE => {
                z = a == at(pc + 1)?;
                pc += 2;
            }
            0x20 => {
                let e = rel(pc + 1)?;
                pc += 2;
                if !z {
                    pc = usize::try_from(i64::try_from(pc).ok()? + e).ok()?;
                }
            }
            0x10 => {
                let e = rel(pc + 1)?;
                pc += 2;
                b = (b - 1) & 0xff;
                if b != 0 {
                    pc = usize::try_from(i64::try_from(pc).ok()? + e).ok()?;
                }
            }
            0xCD => {
                let call = code.get(pc..pc + 3)?;
                if call == wait.as_slice() {
                    t += de;
                } else if call == frame_wait.as_slice() {
                    t += 1;
                }
                pc += 3;
            }
            0xC9 => return Some(t),
            _ => return None,
        }
    }
    None
}

/// The counter attract_screen, which picks the attract mode's screen: the
/// logo's until attract_demo frames in, then the demo's ($4AAA), and 0 out
/// of the attract mode.
fn set_attract_screen(cx: &mut ExtCx<'_>) {
    let t = (cx.me.ext[2] & ATTRACT_MASK) >> ATTRACT_SHIFT;
    let state = cx.me.ext[3] & 0xff;
    let s = match cx.counter("mode") {
        // The demo's game: the demo screen until its setup draws the
        // stage ($5DF6), on the frame after $5A64.
        Some(1) if state == 2 => 2,
        // The high scores after the demo game ($4A37, the flow's sub
        // DEMO_SCORES): the screen emptied, its heading.
        Some(1) if in_demo(cx) && state == 15 && (cx.me.ext[3] >> 16) & 0xff == DEMO_SCORES => 3,
        Some(1) if in_demo(cx) => 0,
        Some(1) if t >= attract_at(cx, "attract_demo") => 2,
        Some(1) => 1,
        _ => 0,
    };
    if cx.counter("attract_screen") != Some(s) {
        cx.set_counter("attract_screen", s);
    }
    let f = if s == 0 { 0 } else { t };
    if cx.counter("attract_frames") != Some(f) {
        cx.set_counter("attract_frames", f);
    }
}

impl Ext for Flow {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let credits = Self::coin(cx);
        let done = Self::out_of_game(cx, credits);
        set_attract_screen(cx);
        if done {
            return;
        }
        // The setup ($4B3F) is a script that waits through states 15 and
        // 16 and comes back in its own state (9 or 11, kept at E3A8). A
        // clip that starts as it comes back, with no bartender at a bar
        // yet, gets him and the taps placed now ($4BCB), its timers being
        // set already.
        let player_y = cx
            .kind("player")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
            .map_or(1, |b| b.y.floor());
        if cx.me.ext[2] == 4 {
            cx.me.ext[2] = 0;
        }
        let state = cx.me.ext[3] & 0xff;
        // In play the loaded wait (E3A8, E3A9, E39F, read only at a start
        // inside a script) is spent: cleared, so a level's end later in a
        // clip is a level's end (tapper_bars gives its points only when the
        // wait isn't one of the end's, $232B in state 11).
        if state == 10 && cx.me.ext[1] & 0xff_ffff_ff00 != 0 {
            cx.me.ext[1] &= !0xff_ffff_ff00;
        }
        // A level's setup has set its timers by then ($4E50: the waits'
        // speed-up, $2023, is counting); a bonus round's or a level end's
        // script hasn't.
        let level_timers = cx.kind("timer").is_some_and(|k| {
            cx.world
                .bodies
                .iter()
                .any(|b| b.kind == k && b.alive && b.ext[1] == 0x2023 && b.ext[0] > 0)
        });
        let resumed =
            cx.me.ext[0] == 0 && (state == 11 || state == 9) && player_y == 0 && level_timers;
        if cx.me.ext[0] == 0 && !resumed {
            return;
        }
        if resumed {
            cx.me.ext[3] = (cx.me.ext[3] & !0xff) | 10;
            // For tapper_bars: the level's first customers ($4C19).
            cx.me.ext[2] = 2;
        } else {
            cx.me.ext[0] = 0;
            // For tapper_bars, which runs later this frame: the level's
            // timers and first customers ($4E50, $4C19).
            cx.me.ext[2] = 1;
        }
        // The layout is the word's low byte; above it the mapping loads the
        // wait's state and place (E3A8, E3A9, E39F) and the lives (E4A7)
        // for tapper_bars.
        let layout = cx.me.ext[1] & 0xff;
        let (Some(tap_kind), Some(player)) = (cx.kind("tap"), cx.kind("player")) else {
            return;
        };
        let step = p(cx, "tap_step");
        // $4D13 (the setup's $5DF6): each bar's tap (E346 + 4 a bar) and
        // far end (E366 + 4 a bar) from the bar_objects row of its side,
        // y, flags, picture and x; in the sports and the space bar (E3B8
        // 1, 3) the far end 7 further out, and 2 more in the sports bar
        // ($4D89 to $4DBE). The tap keeps its flags and picture in word
        // 2 for its board sprite, the far end's x in word 0.
        let objects = cx.table("bar_objects").to_vec();
        let shown = (cx.me.ext[3] >> 8) & 3;
        let mut k = 0;
        for (id, b) in cx.world.bodies.iter().enumerate() {
            if b.kind == tap_kind && k < 4 {
                let left = layout & (1 << k) != 0;
                let mut t = *b;
                t.x = Fx::from_int(if left {
                    p(cx, "tap_left") + step * k
                } else {
                    p(cx, "tap_right") - step * k
                });
                t.facing_left = left;
                let row = usize::from(left) * 48 + usize::try_from(k).unwrap_or(0) * 4;
                if let (Some(tap), Some(end)) = (objects.get(row..row + 4), objects.get(row + 32..row + 36)) {
                    t.y = Fx::from_int(tap[0]);
                    t.ext[2] = (t.ext[2] & !0xffff) | tap[1] | (tap[2] << 8);
                    let mut far = end[3];
                    if shown == 1 || shown == 3 {
                        if shown == 1 {
                            far += 2;
                        }
                        far = if end[1] & 0x10 != 0 { far + 7 } else { far - 7 };
                    }
                    t.ext[0] = far & 0xff;
                }
                cx.set_body(id, t);
                k += 1;
            }
        }
        // The end pieces and far ends ($4D13's second and third groups, to
        // E356 and E366), as bodies of kind bar_end in that order; the far
        // ends 7 further out in the sports and the space bar, as the taps'
        // word 0 has them.
        let mut spawned = 0;
        if let Some(ek) = cx.kind("bar_end") {
            let old: Vec<usize> = cx
                .world
                .bodies
                .iter()
                .enumerate()
                .filter(|(_, b)| b.alive && b.kind == ek)
                .map(|(i, _)| i)
                .collect();
            for id in old {
                cx.remove(id);
            }
            let mut ends: Vec<gp_sim::world::Body> = Vec::new();
            for group in [16usize, 32] {
                for bar in 0..4usize {
                    let left = layout & (1 << bar) != 0;
                    let at = usize::from(left) * 48 + group + bar * 4;
                    let Some(o) = objects.get(at..at + 4) else { continue };
                    if o[0] == 0 {
                        continue;
                    }
                    let mut x = o[3] + if group == 32 && shown == 1 { 2 } else { 0 };
                    if group == 32 && (shown == 1 || shown == 3) {
                        x = if o[1] & 0x10 != 0 { x + 7 } else { x - 7 };
                    }
                    let mut b = gp_sim::systems::new_body(cx.spec, cx.world, ek, Fx::from_int(x & 0xff), Fx::from_int(o[0]), false);
                    b.ext[0] = o[1] | (o[2] << 8);
                    b.ext[1] = i64::try_from((group - 16) / 4 + bar + 1).unwrap_or(0);
                    ends.push(b);
                }
            }
            // Spawned bodies take their ids in order from the world's end.
            let first = cx.world.bodies.len();
            for (n, b) in ends.iter().enumerate() {
                cx.spawn(ek, b.x, b.y, false);
                cx.set_body(first + n, *b);
            }
            spawned = ends.len();
        }
        let bar = p(cx, "start_bar");
        let left = layout & (1 << bar) != 0;
        if let Some((id, b)) = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .find(|(_, b)| b.kind == player)
        {
            let mut b = *b;
            b.x = Fx::from_int(p(cx, if left { "start_left" } else { "start_right" }));
            let bottom = cx.spec.kinds[player].ext_params.get("row_bottom").map_or(48, |v| v.floor());
            let h = cx.spec.kinds[player].ext_params.get("row_height").map_or(48, |v| v.floor());
            b.y = Fx::from_int(bottom + h * bar);
            // $4BCB: his flags and picture (word 2's bytes 2 and 3) and
            // his head (E2FA) 16 above him, flags mirrored when the bar's
            // tap is on the left; the play routine's state after them is
            // the level's RAM, cleared.
            const S: i64 = 0x4BCB;
            let flags = imm(cx, "bartender_setup", S, if left { 0x4C03 } else { 0x4BE0 });
            let pic = imm(cx, "bartender_setup", S, 0x4BF8);
            b.ext = [bar, 0, (flags << 16) | (pic << 24), 0];
            cx.set_body(id, b);
            let head = Obj {
                y: imm(cx, "bartender_setup", S, 0x4BEC),
                flags: imm(cx, "bartender_setup", S, if left { 0x4C0F } else { 0x4BF0 }),
                pic: imm(cx, "bartender_setup", S, 0x4BF4),
                x: b.x.floor(),
            };
            if let Some(pk) = cx.kind("bartender_part") {
                let mut have = [false; 5];
                let parts: Vec<(usize, gp_sim::world::Body)> =
                    cx.world.bodies.iter().enumerate().filter(|(_, p)| p.alive && p.kind == pk).map(|(i, p)| (i, *p)).collect();
                for (pid, mut p) in parts {
                    let k = p.ext[1] - 1;
                    if let Some(h) = usize::try_from(k).ok().and_then(|k| have.get_mut(k)) {
                        *h = true;
                    }
                    if k == 4 {
                        p.y = Fx::from_int(head.y);
                        p.x = Fx::from_int(head.x);
                        p.ext[0] = head.flags | (head.pic << 8);
                    } else {
                        p.y = Fx::ZERO;
                    }
                    cx.set_body(pid, p);
                }
                if !have[4] {
                    let mut p = gp_sim::systems::new_body(cx.spec, cx.world, pk, Fx::from_int(head.x), Fx::from_int(head.y), false);
                    p.ext[0] = head.flags | (head.pic << 8);
                    p.ext[1] = 5;
                    cx.spawn(pk, p.x, p.y, false);
                    cx.set_body(cx.world.bodies.len() + spawned, p);
                }
            }
        }
    }

    fn describe(&self) -> Vec<Record> {
        vec![Record {
            name: "tapper.flow",
            version: env!("CARGO_PKG_VERSION"),
            carrier: Carrier::Entity,
            system: "platformer.enemies",
            class: "flow",
            implementation: "gp-ext-tapper::Flow",
            like: "",
            about: "The boot's wait, the attract mode's logo and demo screen and the start of its demo game, the coins and a game's start; a level's start: the taps by the level's layout and the bartender at his starting bar",
            params: FLOW_PARAMS,
            state: &["ext[0] the level was set up this frame", "ext[1] the layout (low byte), the wait's state E3A8, place E3A9 and B E39F, and the lives E4A7 above it"],
            writes: &["tap.x", "tap.facing", "player.x", "player.y", "player.ext"],
            built_from: &[Primitive::Condition, Primitive::Quantity],
            ..Record::EMPTY
        }]
    }
}

/// A bar's end piece or far end (E356 on): tapper_flow puts them in with
/// the setup's $4D13 and tapper_bars takes them away at a level's end and
/// at the next setup; between those they stand still.
pub struct BarEnd;

/// A sprite object of the bonus round (the cans, the bandit and his
/// hand, the bartender, the opened can): placed and moved by tapper_bars
/// as $2500 to $296F write them, by its object's place from E18E (its
/// word 1, plus 1).
pub struct BonusObject;

impl Ext for BonusObject {
    fn step(&self, _cx: &mut ExtCx<'_>) {}

    fn describe(&self) -> Vec<Record> {
        vec![Record {
            name: "tapper.bonus_object",
            version: env!("CARGO_PKG_VERSION"),
            carrier: Carrier::Entity,
            system: "platformer.enemies",
            class: "flow",
            implementation: "gp-ext-tapper::BonusObject",
            like: "",
            about: "A sprite object of the bonus round, placed and moved by the bars' hook as the round's code writes it",
            params: &[],
            state: &["ext[0] flags plus 256 times the picture", "ext[1] its object's place from E18E, plus 1"],
            writes: &[],
            built_from: &[Primitive::Condition],
            ..Record::EMPTY
        }]
    }
}

impl Ext for BarEnd {
    fn step(&self, _cx: &mut ExtCx<'_>) {}

    fn describe(&self) -> Vec<Record> {
        vec![Record {
            name: "tapper.bar_end",
            version: env!("CARGO_PKG_VERSION"),
            carrier: Carrier::Entity,
            system: "platformer.enemies",
            class: "flow",
            implementation: "gp-ext-tapper::BarEnd",
            like: "",
            about: "A bar's end piece or far end, standing where the setup put it ($4D13) until a level's end or the next setup empties it",
            params: &[],
            state: &["ext[0] flags plus 256 times the picture", "ext[1] its object's place from E356, plus 1"],
            writes: &[],
            built_from: &[Primitive::Condition],
            ..Record::EMPTY
        }]
    }
}

/// The bars' play each frame after the bartender's, in the order the
/// play routine ($0660) calls it: the bartender catching empty mugs
/// ($0B63), the mugs' move and customers catching full ones ($0742), the
/// customers' states ($0B43, dispatched by state through $100A) and their
/// ends of the bar ($0890). Mugs and customers are bodies without hooks of
/// their own; this one body moves them all, so the order is the game's.
///
/// Words: `ext[0]` the customers' walk (E3B3), `ext[1]` an empty mug's
/// speed (E3B0), `ext[2]` a full mug's (E6FE:E6FD, 8.8, signed).
///
/// A mug's words: `ext[1]` the fraction of its x (its slot's +6), `ext[2]`
/// its slot plus one, `ext[3]` its flags (+4: bit 0 empty, bit 3 moving
/// left, its bar above).
///
/// A customer's words (its record at E4B2 + 0x20 a customer): `ext[0]` its
/// state plus 256 times its facing (+0, +0D: 1 on a bar whose tap is on the
/// left), `ext[1]` its timer (+4), `ext[2]` its wait plus 256 times its
/// drink (+2, +9), `ext[3]` its drink's turn plus 256 times the turns left
/// (+3, +5).
pub struct Bars;

const BARS_PARAMS: &[Param] = &[
    Param::new("line", "fixed", "px"),
    Param::new("line_step", "fixed", "px"),
    Param::new("catch_reach", "fixed", "px"),
    Param::new("walk_frames", "frames", "frames"),
    Param::new("pushed_base", "frames", "frames"),
    Param::new("pushed_spread", "count", ""),
    Param::new("drink_turns", "count", ""),
    Param::new("throw_base", "frames", "frames"),
    Param::new("throw_spread", "count", ""),
    Param::new("after_throw", "frames", "frames"),
    Param::new("hand_reach", "fixed", "px"),
    Param::new("catch_once", "flag", ""),
    Param::new("setup_late_bars", "count", ""),
];

/// A level's end (the flow's sub 8): the draws it has taken, kept in the
/// flow's word 3 above the script's frame count. `END_TWICE_KNOWN`: $467E
/// has been reached (or wasn't, on the first level); `END_TWICE`: its bit
/// ran $4742 a second time. `END_INTERLUDE_KNOWN`: $48DA has picked the
/// interlude; `END_INTERLUDE_B`: it is $51A9, not $50EF.
const END_TWICE_KNOWN: i64 = 1 << 40;
const END_TWICE: i64 = 1 << 41;
const END_INTERLUDE_KNOWN: i64 = 1 << 42;
const END_INTERLUDE_B: i64 = 1 << 43;
/// The level's end began in the run (its first frame, or a start inside
/// $51A9 counted from it): its requests are asked. A clip that starts in
/// a script's later waits counts a level's end of its own, which asks for
/// nothing.
const END_SOUNDS: i64 = 1 << 44;

/// The random routine $5FAC's number for a site: the original's when the
/// run has its draws, else one from the generator from 0 to `n`, the A
/// it is called with ($5FB1 to $5FB4 return a number below A or equal to
/// it, else halve the rest and try again).
fn draw_at_most(cx: &mut ExtCx<'_>, site: &str, n: i64) -> i64 {
    match cx.scripted_draw(site) {
        Some(v) => i64::from(v),
        None => i64::from(cx.draw(site)) % (n.max(0) + 1),
    }
}

/// The show a tip starts ($14E7) and $0C25 runs each frame of play: the
/// bars hook's word 1 above the empty mug's speed.
#[derive(Clone, Copy)]
struct Show {
    /// E70F: the show is on.
    on: bool,
    /// E712: frames left.
    left: i64,
    /// E70E: frames to the dancers' next picture.
    tick: i64,
    /// E710: the next picture's place in the bar's list, 0 to 7.
    picture: i64,
}

impl Show {
    fn word(&self) -> i64 {
        (i64::from(self.on) << 8) | (self.left << 16) | (self.tick << 24) | (self.picture << 32)
    }
}

/// A little-endian word of a lifted table.
fn word_at(t: &[i64], at: usize) -> i64 {
    t.get(at).copied().unwrap_or(0) + 256 * t.get(at + 1).copied().unwrap_or(0)
}

/// A picture of the dancers, 36 bytes (9 objects of y, flags, picture, x)
/// for bar shown `bar`: the show's first ($1500 to $1515, through the
/// words at $15F5 by E3B8) when `n` is None, else the n-th of the bar's
/// list ($0C38 to $0C68, through the words at $0C99 by E3B8 to a list of
/// eight words). Each table is the program's, lifted from the player's
/// romset; a word is an address, taken from the first word of its table
/// to find the bytes lifted from there on.
fn show_picture(cx: &ExtCx<'_>, bar: i64, n: Option<i64>) -> Option<Vec<i64>> {
    let bar = usize::try_from(bar & 3).ok()?;
    let at = |ptrs: &[i64], bytes: &[i64], addr: i64| -> Option<Vec<i64>> {
        let off = usize::try_from(addr - word_at(ptrs, 0)).ok()?;
        bytes.get(off..off + 36).map(<[i64]>::to_vec)
    };
    match n {
        None => {
            let ptrs = cx.table("show_first_ptrs");
            at(ptrs, cx.table("show_first"), word_at(ptrs, 2 * bar))
        }
        Some(n) => {
            let ptrs = cx.table("show_lists");
            let bytes = cx.table("show_pictures");
            let list = usize::try_from(word_at(ptrs, 2 * bar) - word_at(ptrs, 0)).ok()?;
            let addr = word_at(bytes, list + 2 * usize::try_from(n & 7).ok()?);
            at(ptrs, bytes, addr)
        }
    }
}

/// The flow's flag for the frame a game started on (tapper_flow's
/// STARTED), 0 without a flow.
/// The attract mode's demo game (E3BD 1, the counter demo): from $5A64
/// (state 2) through its play (state 3), its life lost (12) and the high
/// scores after it ($4A37, state 4), to the state 0 that starts the logo
/// again.
fn in_demo(cx: &ExtCx<'_>) -> bool {
    cx.counter("mode") == Some(1) && cx.counter("demo") == Some(1)
}

/// Out of play: the attract mode but its demo game, and the game over.
fn out_of_play(cx: &ExtCx<'_>) -> bool {
    cx.counter("mode").is_some_and(|m| m != 0) && !in_demo(cx)
}

/// The flow's sub (word 3 from bit 16) through the demo's setup: $5A64 on
/// its first frame, $4B3F's clear of the level on its second.
const DEMO_SETUP: i64 = 13;
/// The flow's sub through the high scores' wait after the demo ($4A50).
const DEMO_SCORES: i64 = 14;

/// $59DD, the demo's presses, run by state 3 before the play routine
/// ($22D4): E3C4 counts down, and at 0 it is 16 again and a draw of 0 to
/// 3 picks one of the lists the words at $5A5C point at; the list's byte
/// at E3C3 becomes the presses (E38E) and E3C3 one more, but at 15 E3C3
/// goes back to 0 and the presses stay as they were ($5A04). E3C3 is kept
/// from one demo to the next.
fn demo_presses(cx: &mut ExtCx<'_>) -> i64 {
    const D: i64 = 0x59DD;
    let mut press = cx.counter("demo_press").unwrap_or(0);
    let count = (cx.counter("demo_count").unwrap_or(0) - 1) & 0xff;
    if count != 0 {
        cx.set_counter("demo_count", count);
        return press;
    }
    cx.set_counter("demo_count", imm(cx, "demo_input", D, 0x59EF));
    let pick = draw_at_most(cx, "demo_pick", imm(cx, "demo_input", D, 0x59F1));
    let ptrs: Vec<i64> = cx
        .table("demo_press_ptrs")
        .chunks(2)
        .map(|w| w[0] + 256 * w.get(1).copied().unwrap_or(0))
        .collect();
    let first = ptrs.iter().copied().min().unwrap_or(0);
    let mut place = cx.counter("demo_place").unwrap_or(0);
    if place == imm(cx, "demo_input", D, 0x5A05) {
        place = 0;
    } else {
        let list = ptrs.get(usize::try_from(pick).unwrap_or(0)).map_or(0, |w| w - first);
        press = usize::try_from(list + place)
            .ok()
            .and_then(|k| cx.table("demo_presses").get(k).copied())
            .unwrap_or(0);
        place = (place + 1) & 0xff;
        cx.set_counter("demo_press", press);
    }
    cx.set_counter("demo_place", place);
    press
}

/// The 16 records of 32 bytes the setup's $4A65 copies to E4B2 for the
/// bar shown (E3B8, the flow's word 3 from bit 8): the block of the words
/// at $4AA0, record_blocks for the four bars and, for the 4 $3C48 sets
/// out of a game, demo_records.
fn record_block(cx: &ExtCx<'_>, shown: i64) -> Vec<i64> {
    if shown >= 4 {
        return cx.table("demo_records").to_vec();
    }
    let at = usize::try_from(shown).unwrap_or(0) * 512;
    cx.table("record_blocks").get(at..at + 512).map(<[i64]>::to_vec).unwrap_or_default()
}

fn fb_started(flow: Option<(usize, gp_sim::world::Body)>) -> i64 {
    flow.map_or(0, |(_, b)| b.ext[2] & STARTED)
}

/// The bonus round's state in the flow's word 2 from bit 40: the can
/// with no shake (3 bits, E262 + 8 a can, +5 0), the last pick of the
/// shuffle (E719, 3 bits), the pointer (E6FB, 3 bits) and the cans shaken
/// (6 bits, a can a bit) and the can the shake now under way took (3
/// bits, from bit 55); the round's frame
/// count is word 3 from bit 24, the loop's count B from bit 40 and its
/// next frame from bit 48.
const BONUS_MASK: i64 = 0x3_ffff << 40;

/// The frames the bandit walks off after the last shake ($2669 to
/// $2680): 2 a frame from that can's place (bonus_bandit_places) until
/// his x is below bonus_bandit_stop.
fn bandit_walk(cx: &ExtCx<'_>, shaking: i64) -> i64 {
    let k = usize::try_from(shaking).unwrap_or(0);
    let tx = cx.table("bonus_bandit_places").get(2 * k + 1).copied().unwrap_or(0);
    let stop = byte_of(cx, "bandit_stop");
    let mut steps = 1;
    while tx - 2 * steps >= stop && steps < 128 {
        steps += 1;
    }
    steps
}

/// The round's frame the shuffle starts on: the last shake ends at 198,
/// then the walk off and the blink's 40 ($2684).
fn shuffle_start(cx: &ExtCx<'_>, shaking: i64) -> i64 {
    198 + bandit_walk(cx, shaking) + 40
}

/// The bonus round ($2500), sub 12 of a level's end, a frame: true on the
/// frame it returns. Its frames are counted from the frame $5681 counted
/// the level (0): $5E99 and $60F6 wait a frame each and $252C 20, the
/// cans' objects go in ($255F, from $2A15 to E262) and $2579 waits 30;
/// then five shakes 29 frames apart from 53 ($2585 to $2656): each draws
/// a can of 0 to 5 ($258A, again while it is one already shaken, +5 of
/// its object), rises 16 frames, waits 5 and two of 2 and 2. The bandit
/// walks off from the last can's place ($2669 to $2680, bandit_walk) and
/// blinks 40 ($2684 to $26A7), and the shuffle starts after
/// (shuffle_start): 16 swaps of a pair of cans, 9 frames each with fewer than 6
/// levels cleared ($4F17), else 5 ($5003); the first pair is the one the
/// unshaken can starts ($2AE5: its number, 5 taken as 4), each later one
/// a draw of 0 to 4 not the last ($4F35 or $5021, again while it is the
/// same), naming in bonus_pairs two cans each swapped with the next
/// ($4F17) or one ($5003); a swap trades the cans' places and shakes. Then $26E3 waits 10, the
/// pointer goes to 5 ($2720) and waits 8, and the choice ($26F7): a read
/// of the stick each 2 frames, B counting 255 down; right moves the
/// pointer one on (round from 5 to 0, $2720) and left one back ($2714),
/// each waiting 8 and taking one from B; the button, or B at 0, chooses
/// ($2787): 20 frames on, the unshaken can wins ($27C2: 70 frames to
/// $296F) and another loses ($284D: 160, tapper-bonus.002's second
/// round); $296F waits 15 and returns.
fn bonus_round(cx: &mut ExtCx<'_>, fb: &mut gp_sim::world::Body, cleared: i64, points: &mut i64) -> bool {
    let c = ((fb.ext[3] >> 24) & 0xffff) + 1;
    fb.ext[3] = (fb.ext[3] & !(0xffff << 24)) | (c << 24);
    let field = |w: i64, at: u32| (w >> at) & 7;
    let mut unshaken = field(fb.ext[2], 40);
    let mut pick = field(fb.ext[2], 43);
    let mut ptr = field(fb.ext[2], 46);
    let mut shaken = (fb.ext[2] >> 49) & 0x3f;
    let mut shaking = field(fb.ext[2], 55);
    let fast = cleared >= 6;
    let d = if fast { 5 } else { 9 };
    let start = if c >= 198 { shuffle_start(cx, shaking) } else { i64::MAX / 2 };
    let shuffle_end = start + 16 * d;
    let mut done = false;
    // $256A numbers the cans (E700 on): the bonus round's stage is up,
    // until the next setup's $5DF6.
    if c == 23 {
        cx.set_counter("bonus", 1);
    }
    // $2533: $0B after the wait of 20 ($252C).
    if c == 22 {
        ask(cx, "sound_bonus_open", &[1]);
    }
    // $2603: each shake, after its rise of 16 and wait of 5, asks for $25.
    if (53..53 + 5 * 29).contains(&c) && (c - 53) % 29 == 21 {
        ask(cx, "sound_bonus_shake", &[1]);
    }
    // $2684: the bandit's blink, $1C each 8 frames, five times.
    if c >= 198 && c >= start - 40 && c < start && (c - (start - 40)) % 8 == 0 {
        ask(cx, "sound_bonus", &[1]);
    }
    // $26BA: 2 as the shuffle ends; $26EE: $2D 10 frames on.
    if c == shuffle_end {
        ask(cx, "sound_bonus", &[0x37]);
    } else if c == shuffle_end + 10 {
        ask(cx, "sound_bonus", &[0x6B]);
    }
    if (53..53 + 5 * 29).contains(&c) && (c - 53) % 29 == 0 {
        loop {
            let k = draw_at_most(cx, "bonus_shake", 5);
            if shaken & (1 << k) == 0 {
                shaken |= 1 << k;
                shaking = k;
                break;
            }
        }
        if (c - 53) / 29 == 4 {
            unshaken = (0..6).find(|k| shaken & (1 << k) == 0).unwrap_or(0);
        }
    } else if c >= start && c < shuffle_end && (c - start) % d == 0 {
        let j = (c - start) / d;
        // Each swap asks for $26 ($4F1C, $5008), and each draw of a later
        // pair too, again when it is the last one ($4F29, $5015; the
        // `sound_shuffle` rows are the program from $4F1C).
        let (first_ask, draw_ask) = if fast { (0xEC, 0xF9) } else { (0, 0x0D) };
        pick = if j == 0 {
            ask(cx, "sound_shuffle", &[first_ask + 1]);
            if unshaken == 5 { 4 } else { unshaken }
        } else {
            let site = if fast { "attract_pick_2" } else { "attract_pick" };
            loop {
                ask(cx, "sound_shuffle", &[draw_ask + 1]);
                let v = draw_at_most(cx, site, 4);
                if v != pick {
                    break v;
                }
            }
        };
        let pairs = flow_table(cx, "bonus_pairs");
        let can = |w: usize| -> i64 {
            let a = pairs.get(2 * w).copied().unwrap_or(0) + 256 * pairs.get(2 * w + 1).copied().unwrap_or(0);
            (a - 0xE262).div_euclid(8)
        };
        // Each word names a can whose object trades places and shakes
        // with the next can's ($4FB1 to $4FF6: ix and ix+8, iy and
        // iy+8; $50C1 to $50E2: ix and ix+8).
        let p = usize::try_from(pick).unwrap_or(0);
        let firsts = if fast { vec![can(p)] } else { vec![can(2 * p), can(2 * p + 1)] };
        for a in firsts {
            if unshaken == a {
                unshaken = a + 1;
            } else if unshaken == a + 1 {
                unshaken = a;
            }
        }
    } else if c == shuffle_end + 10 {
        ptr = 5;
        // The first read of the stick, 8 on, B 254.
        fb.ext[3] = (fb.ext[3] & !(0xff << 40 | 0xffff << 48)) | (254 << 40) | ((c + 8) << 48);
    } else if c > shuffle_end + 10 {
        let b = (fb.ext[3] >> 40) & 0xff;
        let next = (fb.ext[3] >> 48) & 0xffff;
        // Once chosen, B is 0 and next is the choice's frame plus 0x8000.
        if c == next && b != 0 {
            let (b2, then, choose) = if cx.held("button1") {
                (b, c, true)
            } else if cx.held("right") || cx.held("left") {
                ptr = if cx.held("right") { (ptr + 1) % 6 } else { (ptr + 5) % 6 };
                (b - 1, c + 8, b - 1 == 0)
            } else {
                (b - 1, c + 2, b - 1 == 0)
            };
            let at = if choose { then } else { 0 };
            // $2787: the choice asks for 2 and $27.
            if choose && at == c {
                ask(cx, "sound_bonus", &[0x104, 0x109]);
            }
            fb.ext[3] = (fb.ext[3] & !(0xff << 40 | 0xffff << 48))
                | (if choose { 0 } else { b2 } << 40)
                | (if choose { at + 0x8000 } else { then } << 48);
        } else if next >= 0x8000 {
            // Chosen at next less 0x8000: 20 frames to the check, then
            // the win's 70 or the loss's 160, and $296F's 15.
            let at = next - 0x8000;
            let length = 20 + if ptr == unshaken { 70 } else { 160 } + 15;
            if c == at {
                ask(cx, "sound_bonus", &[0x104, 0x109]);
            }
            // 20 frames on, the win asks for $2A ($2823), the loss for $28
            // ($284D); in the loss, $29 and $2B ($292C, $3BFF) 56 frames
            // later, $2B again 40 on (the second $3BFF) and $2C 40 after
            // that ($2950) (measured on tapper.bonus.002's second round).
            let won = ptr == unshaken;
            if c == at + 20 {
                ask(cx, "sound_bonus", &[if won { 0x1A0 } else { 0x1CA }]);
            } else if !won && c == at + 76 {
                ask(cx, "sound_bonus", &[0x2A9]);
                ask(cx, "sound_bonus_2b", &[1]);
            } else if !won && c == at + 116 {
                ask(cx, "sound_bonus_2b", &[1]);
            } else if !won && c == at + 156 {
                ask(cx, "sound_bonus", &[0x2CD]);
            }
            // $2820: the win's points, on the frame of the check.
            if c == at + 20 && ptr == unshaken {
                *points += points_of(cx.table("points_bonus"));
            }
            if c == at + length {
                done = true;
            }
        }
    }
    fb.ext[2] = (fb.ext[2] & !BONUS_MASK) | (unshaken << 40) | (pick << 43) | (ptr << 46) | (shaken << 49) | (shaking << 55);
    done
}

/// A sprite object of the bonus round as the game keeps it: its address
/// (E18E on), y, flags, picture and x. A y of 0 is no picture.
#[derive(Clone, Copy)]
struct BonusObj {
    at: i64,
    y: i64,
    flags: i64,
    pic: i64,
    x: i64,
}

/// The board's sprite objects from E18E that hooks place by address (the
/// bonus round's, the pour's, the instruction scene's) as bodies of kind
/// `ok`: one kept is the same body, one gone is removed, a new one spawned.
fn place_objects(cx: &mut ExtCx<'_>, ok: usize, want: &[BonusObj]) {
    use gp_sim::world::Body;
    let have: Vec<(usize, Body)> =
        cx.world.bodies.iter().enumerate().filter(|(_, b)| b.alive && b.kind == ok).map(|(i, b)| (i, *b)).collect();
    // Word 1 is the object's place from E18E, plus 1: its sprite RAM
    // entry (present.sprite_order).
    let key = |at: i64| (at - 0xE18E) / 4 + 1;
    for (id, b) in &have {
        match want.iter().find(|o| key(o.at) == b.ext[1]) {
            Some(o) => {
                let mut b = *b;
                b.x = Fx::from_int(o.x);
                b.y = Fx::from_int(o.y);
                b.ext[0] = o.flags | (o.pic << 8);
                cx.set_body(*id, b);
            }
            None => cx.remove(*id),
        }
    }
    for o in want.iter().filter(|o| !have.iter().any(|(_, b)| b.ext[1] == key(o.at))) {
        let mut b = gp_sim::systems::new_body(cx.spec, cx.world, ok, Fx::from_int(o.x), Fx::from_int(o.y), false);
        b.ext[0] = o.flags | (o.pic << 8);
        b.ext[1] = key(o.at);
        // Its id: the world's end plus every spawn asked for this step,
        // mugs and customers included.
        let id = cx.world.bodies.len()
            + cx.requests().iter().filter(|r| matches!(r, gp_sim::ext::Request::Spawn { .. })).count();
        cx.spawn(ok, b.x, b.y, false);
        cx.set_body(id, b);
    }
}

/// The instruction scene's frame (demo_scene), 0 on its first, while the
/// attract mode is in state 6 ($3CD8): from the flow's count of the
/// attract mode, from the frame attract_demo names.
fn scene_frame(cx: &ExtCx<'_>) -> Option<i64> {
    if cx.counter("mode") != Some(1) {
        return None;
    }
    let k = cx.kind("flow")?;
    let fb = cx.world.bodies.iter().find(|b| b.alive && b.kind == k)?;
    // State 2's frame ($3CD8 set it as the scene ended): the board shows
    // the scene's last objects, which $5A64 empties on the next.
    if fb.ext[3] & 0xff == 2 {
        return Some(i64::MAX / 2);
    }
    if fb.ext[3] & 0xff != 6 || in_demo(cx) {
        return None;
    }
    let t = (fb.ext[2] & ATTRACT_MASK) >> ATTRACT_SHIFT;
    // The board shows a frame's objects the frame after the code wrote
    // them: the scene's frame before this one.
    Some(t - attract_at(cx, "attract_demo") - 2).filter(|n| *n >= 0)
}

/// The attract mode's instruction scene ($2EE0, called by $3CD8 in state 6)
/// on its frame `n` (0 the frame it starts, the texts' first): the objects
/// it shows, from E18E on. The scene plays the game's own objects with no
/// input and no draw: the bartender (E2E6, six parts) pours a mug at bar 1
/// and slides it to a customer (E1BE, from $33A3), walks to bar 2 for a tip
/// (E25E) and back, and the customer sends the empty mug (E2FE) back,
/// between waits of $232B. Each number is read from the scene's code by its
/// place (demo_screen_a, demo_screen), from the tables its calls read
/// ($C4A1 stand_x, $0977 bar_ends, $1AD5 in bartender_walk_left, $4DCC
/// bar_objects) and from the routines it calls ($1351 bartender_pour,
/// $2085 bartender_wait). After a wait A is what the text routine left
/// (scene_a, E39D: gp element inputs on tapper.demo.001 frames 414, 522
/// and 656).
fn demo_scene(cx: &ExtCx<'_>, n: i64) -> Vec<BonusObj> {
    const BASE: i64 = 0xE18E;
    let d = |at: i64| {
        if at < 0x2F37 { imm(cx, "demo_screen_a", 0x2EE0, at) } else { imm(cx, "demo_screen", 0x2F37, at) }
    };
    // A ld de,NN before a call of the wait: its frames.
    let w = |at: i64| d(at) | (d(at + 1) << 8);
    let stand = |bar: i64| imm(cx, "stand_x", 0xC4A1, 0xC4A1 + bar);
    let end = |bar: i64, k: i64| imm(cx, "bar_ends", 0x0977, 0x0977 + 4 * bar + k);
    let pour = |at: i64| imm(cx, "bartender_pour", 0x1351, at);
    let wait_pose = |at: i64| imm(cx, "bartender_wait", 0x2085, at);
    let step = |k: i64| imm(cx, "bartender_walk_left", 0x19EF, 0x1AD5 + k);
    let taps = |k: i64| imm(cx, "bar_objects", 0x4DCC, 0x4DCC + k);
    let after_wait = p(cx, "scene_a");
    let mut m = [0i64; 0x200];
    let (mut e727, mut e729) = (0i64, 0i64);
    let mut f = 0i64;
    // The objects as they stand after the writes of frame n.
    let shown = |m: &[i64; 0x200]| -> Vec<BonusObj> {
        (0..0x80)
            .filter(|k| m[4 * k] & 0xff != 0)
            .map(|k| BonusObj { at: BASE + 4 * k as i64, y: m[4 * k] & 0xff, flags: m[4 * k + 1] & 0xff, pic: m[4 * k + 2] & 0xff, x: m[4 * k + 3] & 0xff })
            .collect()
    };
    macro_rules! wait {
        ($k:expr) => {
            f += $k;
            if f > n {
                return shown(&m);
            }
        };
    }
    let at = |a: i64| (a - BASE) as usize;
    const IX: i64 = 0xE2E6;
    // The bartender at bar `bar`'s tap ($1351 with E3AF 0, so $0FFB's bit
    // is clear): his parts by the tables at $C4A1 and $0977.
    let pour_pose = |m: &mut [i64; 0x200], bar: i64| {
        let o = at(IX);
        let x = stand(bar);
        m[o + 3] = x;
        m[o + 2] = pour(0x136B);
        for k in [1, 5, 9, 0x15] {
            m[o + k] = pour(0x1371);
        }
        m[o + 0x17] = x;
        m[o + 7] = (x + pour(0x1394)) & 0xff;
        m[o + 0xB] = m[o + 7];
        m[o + 6] = pour(0x13A4);
        let y = end(bar, 0);
        m[o] = y;
        m[o + 4] = y;
        m[o + 0x14] = (y - pour(0x13BA)) & 0xff;
        m[o + 8] = (m[o + 0x14] + pour(0x13BF)) & 0xff;
        m[o + 0xA] = pour(0x13C6);
        m[o + 0x16] = pour(0x13CA);
    };
    // $0679: the tap's handle back, from $4DCC by bar.
    let handle_back = |m: &mut [i64; 0x200], bar: i64| m[at(0xE346) + 4 * bar as usize] = taps(4 * bar);
    // The scene's first frame: its colour and lines (the board's texts),
    // bar 1, the taps' objects from $4DD0, the bartender's first parts and
    // the customer's two objects from $33A3.
    let bar = d(0x2F00);
    for k in 0..d(0x2F13) {
        m[at(d(0x2F0D) | (d(0x2F0E) << 8)) + k as usize] = taps(4 + k);
    }
    let o = at(IX);
    m[o] = d(0x2F1A);
    m[o + 1] = d(0x2F1E);
    m[o + 3] = d(0x2F22);
    m[o + 0x17] = d(0x2F26);
    m[o + 0x14] = d(0x2F2A);
    m[o + 0x15] = d(0x2F2E);
    m[o + 0x16] = d(0x2F32);
    m[o + 2] = d(0x2F36);
    let cust = at(d(0x2F38) | (d(0x2F39) << 8));
    for k in 0..d(0x2F3E) {
        m[cust + k as usize] = d(0x33A3 + k);
    }
    // $60F6's wait of a frame, then $232B's.
    wait!(1);
    wait!(w(0x2F49));
    // The handle down ($1BD4) and the pour: he stands at the tap ($1351),
    // his arm on the handle (+$10 to +$13), and every other frame of six
    // the arm turns and the first free object of six from E18E takes a
    // layer of beer, 2 above the last.
    m[at(0xE346) + 4 * bar as usize] = 0;
    pour_pose(&mut m, bar);
    let hx = (stand(bar) + d(0x2F5F)) & 0xff;
    m[o + 0x13] = hx;
    m[o + 0x12] = d(0x2F66);
    m[o + 0x11] = d(0x2F6A);
    m[o + 0x10] = m[o + 4];
    wait!(w(0x2F72));
    for _ in 0..d(0x2F78) {
        m[o + 0x11] ^= d(0x2F7E);
        m[o + 0x10] = m[o + 4];
        let free = (0..d(0x2F8E)).find(|k| m[at(d(0x2F89) | (d(0x2F8A) << 8)) + 4 * *k as usize] == 0);
        let Some(k) = free else { continue };
        let b = at(BASE) + 4 * k as usize;
        m[b] = (m[o + 4] + 2 * k) & 0xff;
        m[b + 1] = d(0x2FA5);
        m[b + 2] = if k == 0 { d(0x2FB2) } else { d(0x2FAD) };
        m[b + 3] = (stand(bar) + d(0x2FC2)) & 0xff;
        wait!(w(0x2FC5));
    }
    // The full mug in his hand ($2085's pose at the tap) and the handle up.
    m[o + 0xF] = (stand(bar) + d(0x2FD8)) & 0xff;
    m[o + 0xC] = m[o + 4];
    m[o + 0xE] = d(0x2FE3);
    m[o + 0xD] = d(0x2FEA);
    m[o + 2] = wait_pose(0x2088);
    m[o + 6] = wait_pose(0x208C);
    m[o + 5] = wait_pose(0x2090);
    m[o + 0xA] = wait_pose(0x2094);
    m[o + 9] = wait_pose(0x2098);
    let tx = m[at(0xE349) + 4 * bar as usize];
    if m[at(0xE347) + 4 * bar as usize] & 0x10 == 0 {
        m[o + 3] = (tx - wait_pose(0x20AE)) & 0xff;
        m[o + 7] = (m[o + 3] + wait_pose(0x20B3)) & 0xff;
        m[o + 0xB] = m[o + 7];
    } else {
        m[o + 3] = (tx + wait_pose(0x20BD)) & 0xff;
        m[o + 7] = (m[o + 3] - wait_pose(0x20C2)) & 0xff;
        m[o + 5] = wait_pose(0x20C9);
        m[o + 0xB] = m[o + 7];
        m[o + 9] = wait_pose(0x20D0);
    }
    m[o + 8] = (m[o] + wait_pose(0x20D5)) & 0xff;
    m[o + 0x10] = 0;
    handle_back(&mut m, bar);
    wait!(w(0x2FF2));
    wait!(w(0x300A));
    // The send: his parts as A left them, the throw's pose, the beer gone
    // and the mug from his hand to the left, 3 a frame.
    m[o + 8] = after_wait;
    m[o + 0xC] = after_wait;
    m[o + 0x10] = after_wait;
    handle_back(&mut m, bar);
    m[o + 3] = (stand(bar) + d(0x3026)) & 0xff;
    m[o + 0x17] = (m[o + 3] + d(0x302B)) & 0xff;
    m[o + 7] = (m[o + 0x17] + d(0x3030)) & 0xff;
    m[o + 0x16] = d(0x3037);
    m[o + 2] = d(0x3039);
    m[o + 6] = d(0x303E);
    for k in 0..d(0x3047) {
        m[at(BASE) + 4 * k as usize] = 0;
    }
    let mug = at(0xE2FE);
    m[mug] = (m[o] + d(0x3059)) & 0xff;
    m[mug + 1] = d(0x3060);
    m[mug + 2] = d(0x3062);
    m[mug + 3] = (stand(bar) + d(0x3071)) & 0xff;
    for _ in 0..d(0x3076) {
        // Three dec (ix+$03) a frame ($3077).
        m[mug + 3] = (m[mug + 3] - 3) & 0xff;
        wait!(w(0x3081));
    }
    // The catch: the customer's arm and the mug gone, and both slide back
    // 2 a frame.
    m[cust + 6] = d(0x308F);
    m[cust + 4] = d(0x3093);
    m[mug] = d(0x3097);
    for _ in 0..d(0x3099) {
        // Two dec a frame each ($309A, $30A0).
        m[cust + 7] = (m[cust + 7] - 2) & 0xff;
        m[cust + 3] = (m[cust + 3] - 2) & 0xff;
        wait!(w(0x30A7));
    }
    wait!(w(0x30AF));
    wait!(w(0x30C9));
    // To bar 2 for the tip: his switch's pictures, then he stands at its
    // far end ($0977's +2).
    let switch = |m: &mut [i64; 0x200], a: i64, p2: i64, p16: i64| {
        for k in [4, 8, 0xC, 0x10] {
            m[o + k] = a;
        }
        m[o + 2] = p2;
        m[o + 3] = (m[o + 3] + d(0x30E3)) & 0xff;
        m[o + 0x16] = p16;
    };
    switch(&mut m, after_wait, d(0x30E1), d(0x30ED));
    wait!(w(0x30EF));
    let bar2 = d(0x30F5);
    let mut h = d(0x30FF);
    m[o + 0x14] = 0;
    for _ in 0..d(0x3105) {
        m[o + 2] = h;
        h += 1;
        wait!(w(0x310B));
    }
    let stand_far = |m: &mut [i64; 0x200], bar: i64, p2: i64, p16: i64, f1: i64, f15: i64, below: i64, add: i64| {
        m[o + 2] = p2;
        m[o + 0x16] = p16;
        m[o + 1] = f1;
        m[o + 0x15] = f15;
        let y = end(bar, 0);
        m[o] = y;
        m[o + 0x14] = (y - below) & 0xff;
        m[o + 3] = (end(bar, 2) + add) & 0xff;
        m[o + 0x17] = m[o + 3];
    };
    stand_far(&mut m, bar2, d(0x3115), d(0x3119), d(0x311D), d(0x3121), d(0x3134), d(0x313C));
    wait!(w(0x3144));
    // The tip on bar 2.
    let tip = at(d(0x315C) | (d(0x315D) << 8));
    m[tip] = d(0x315F);
    m[tip + 1] = 0;
    m[tip + 2] = d(0x3165);
    m[tip + 3] = d(0x3169);
    wait!(w(0x316B));
    // He walks left to it, 2 a frame, his step's pictures from $1AD5
    // every third frame (E727, E729).
    for _ in 0..d(0x3171) {
        wait!(w(0x3173));
        if e727 != 0 {
            e727 -= 1;
        } else {
            e729 = if e729 == 3 { 0 } else { e729 + 1 };
            let k = 4 * e729;
            m[o + 2] = step(k);
            m[o + 1] |= d(0x31A2);
            m[o + 6] = step(k + 1);
            m[o + 5] = d(0x31AE);
            m[o + 0x16] = step(k + 2);
            m[o + 0x15] |= d(0x31B8);
            let c = step(k + 3);
            m[o] = (m[o] + c) & 0xff;
            m[o + 4] = (m[o + 4] + c) & 0xff;
            m[o + 0x14] = (m[o + 0x14] + c) & 0xff;
            m[o + 4] = (m[o] + d(0x31D7)) & 0xff;
            m[o + 7] = (m[o + 3] + d(0x31DF)) & 0xff;
            e727 = d(0x31E4);
        }
        for k in [3, 7, 0x17] {
            m[o + k] = (m[o + k] + d(0x31E9)) & 0xff;
        }
    }
    m[tip] = d(0x3208);
    wait!(w(0x320A));
    wait!(w(0x3222));
    // Back to bar 1.
    for k in [4, 8, 0xC, 0x10] {
        m[o + k] = after_wait;
    }
    m[o + 2] = d(0x3236);
    m[o + 1] = d(0x323A);
    m[o + 0x16] = d(0x323E);
    m[o + 0x15] = d(0x3242);
    wait!(w(0x3244));
    let bar1 = d(0x324A);
    let mut h = d(0x3254);
    m[o + 0x14] = 0;
    for _ in 0..d(0x325A) {
        m[o + 2] = h;
        h += 1;
        wait!(w(0x3260));
    }
    stand_far(&mut m, bar1, d(0x326A), d(0x326E), d(0x3272), d(0x3276), d(0x3289), d(0x3291));
    // The customer drinks: his pictures, and six bubbles from E18E above
    // him, which go from the top every 4 frames.
    m[cust + 2] = d(0x329D);
    m[cust + 4] = m[cust];
    m[cust + 6] = d(0x32A7);
    let bub = at(d(0x32A9) | (d(0x32AA) << 8));
    let mut y = m[cust];
    for k in 0..d(0x32AC) as usize {
        m[bub + 4 * k] = y & 0xff;
        m[bub + 4 * k + 1] = d(0x32B9);
        m[bub + 4 * k + 2] = d(0x32BC);
        m[bub + 4 * k + 3] = (m[cust + 3] + d(0x32B1)) & 0xff;
        y += 1;
    }
    let mut hl = bub + 4 * d(0x32AC) as usize;
    for _ in 0..d(0x32C4) {
        wait!(w(0x32C6));
        if hl < 0x200 {
            m[hl] = 0;
        }
        hl = hl.saturating_sub(4);
    }
    // The empty mug back to the right, 1 a frame.
    m[cust + 2] = d(0x32D8);
    m[cust + 6] = d(0x32DC);
    m[mug] = (m[cust + 4] - 3) & 0xff;
    m[mug + 3] = m[cust + 7];
    m[mug + 2] = d(0x32F6);
    m[mug + 1] = d(0x32FA);
    for _ in 0..d(0x32FC) {
        m[mug + 3] = (m[mug + 3] + 1) & 0xff;
        wait!(w(0x3301));
    }
    m[mug] = d(0x330B);
    wait!(w(0x330D));
    // The last lines, the high scores' line and its wait, then state 2.
    wait!(1);
    wait!(1);
    wait!(w(0x3391));
    shown(&m)
}

/// The beer in the mug at the tap ($129D to $1308): each frame of the
/// pour after its first ($121D set E715 to 4), while E715 counts down,
/// the first free object of six from E18E takes a layer: at the
/// bartender's y plus 4 for each layer before it, flags 3, picture $7F
/// for the first and $21 above it, at the tap's handle's x. Three layers
/// fill the mug (E715 at 1, then 0 with E714 1, $125E); a send ($13CC),
/// a move ($214B) or a walk empties both counts, and the layers with them.
/// The level's end pours its own mug the same way.
fn pour_layers(cx: &ExtCx<'_>) -> Vec<BonusObj> {
    const T: i64 = 0x1206;
    let Some(pk) = cx.kind("player") else { return Vec::new() };
    let Some(me) = cx.world.bodies.iter().find(|b| b.alive && b.kind == pk) else { return Vec::new() };
    if me.y.floor() == 0 || out_of_play(cx) {
        return Vec::new();
    }
    let Some((tap_x, tap_left)) = Bartender::tap(cx, me.ext[0] & 3) else { return Vec::new() };
    let y = me.y.floor();
    // A level's end ($4742, from the end script's frame 56 as
    // Bartender::end_pose counts it): the handle down, then each 2 frames
    // ($477F, six times) a layer 2 above the last in the first free
    // object, picture $0B then $0C, until $4852 empties eight objects 15
    // frames after the full mug ($4815, frame 85).
    let flow = cx.kind("flow").and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k));
    if let Some(fb) = flow
        && [15, 16].contains(&(fb.ext[3] & 0xff))
        && (fb.ext[3] >> 16) & 0xff == 8
    {
        let w3 = fb.ext[3];
        let n = ((w3 >> 24) & 0xffff) + 1;
        let n = if n >= 109 && w3 & END_TWICE != 0 { n - 53 } else { n };
        let b = |at: i64| imm(cx, "end_pose_b", 0x467B, at);
        let x = (tap_x + if tap_left { b(0x47D4) } else { 0 }) & 0xff;
        let mut objs: Vec<BonusObj> = (0..6)
            .filter(|k| n >= 58 + 2 * k && n < 85)
            .map(|k| BonusObj {
                at: 0xE18E + 4 * k,
                y: (y + 2 * k) & 0xff,
                flags: b(0x47AB),
                pic: if k == 0 { b(0x47B8) } else { b(0x47B3) },
                x,
            })
            .collect();
        objs.extend(kicked_glass(cx, me, n, tap_left, fb.ext[3]));
        objs.extend(drink_bubbles(cx, me, n, tap_left));
        return objs;
    }
    let w = me.ext[2];
    let (e715, e714) = ((w >> 40) & 0xff, (w >> 48) & 1);
    // The board shows a frame's objects the frame after the code wrote
    // them, so a layer written as E715 counts down shows on the next one.
    let n = match e715 {
        1..=3 => 3 - e715,
        0 if e714 != 0 => 3,
        _ => 0,
    };
    let x = (tap_x + if tap_left { imm(cx, "bartender_tap", T, 0x1304) } else { 0 }) & 0xff;
    (0..n)
        .map(|i| BonusObj {
            at: 0xE18E + 4 * i,
            y: (y + 4 * i) & 0xff,
            flags: imm(cx, "bartender_tap", T, 0x12D1),
            pic: if i == 0 { imm(cx, "bartender_tap", T, 0x12E8) } else { imm(cx, "bartender_tap", T, 0x12D9) },
            x,
        })
        .collect()
}

/// The bubbles as he drinks at a level's end ($130C, called from $4869
/// at frame 85): six objects from E1AE, each one line below the last from
/// his y plus 1 ($1338 to $1343: state 11), 6 along from his x (11 back on
/// a bar whose tap is on the left), picture $15; they go from the top, one
/// each 4 frames, the last at frame 109 (tapper-m3-five's first level's
/// end, frames 1301 to 1325).
fn drink_bubbles(cx: &ExtCx<'_>, me: &gp_sim::world::Body, n: i64, left: bool) -> Vec<BonusObj> {
    const T: i64 = 0x1206;
    let b = |at: i64| imm(cx, "bartender_tap", T, at);
    let count = b(0x1324);
    let x = if left { me.x.floor() - b(0x1336) } else { me.x.floor() + b(0x1332) };
    (0..count)
        .filter(|i| n >= 85 && n < 109 - 4 * i)
        .map(|i| BonusObj {
            at: 0xE1AE + 4 * i,
            y: (me.y.floor() + 1 + i) & 0xff,
            flags: b(0x1346),
            pic: b(0x1349),
            x: x & 0xff,
        })
        .collect()
}

/// The glass of a level's end, after he drinks it ($4687 to $4707): from
/// frame 109 the empty glass ($17) in E2E2 beside him, 2 above his y and
/// 14 out from where he stood (he steps 1 that way, $46BF); from 124, the
/// kick ($46CC), the glass tipped ($10) 8 lower; from 128 it is in E356
/// and the script moves it, a step each 2 frames, its flags from the
/// table at $56C2 by a count of the steps mod 8 (E706, from 0 at $46C8):
/// - scripts 0 and 3 ($58C5): 22 steps, up 10 at first and 1 less each
///   step, 1 along away from the tap each step. Script 0 ($4881): 3
///   lower, then $45E3 breaks it, a picture a frame from the table at
///   $24F4 (12), and empties it. Script 3 ($48D1): $50EF or $51A9 bounce
///   it 10 steps (up 3 at first, 1 less each, 1 along, the flags' count
///   mod 4), then stand it ($17); $51A9 puts him under it 4 frames on
///   and it rides on his head as he hops (5 frames a hop, 5 up and down
///   by turns, along by the table at $519F).
/// - scripts 1 and 2 ($5904): the same arc for 21 steps, 1 along every
///   other step. Script 1 ($489A): $45E3 breaks it where it came down
///   (on his head). Script 2 ($2DF7): it stands ($17) at his feet, 2 out
///   on a left-tap bar.
/// - script 4 ($594D): straight up 10 a step until at $EE or higher, then
///   gone.
fn kicked_glass(cx: &ExtCx<'_>, me: &gp_sim::world::Body, n: i64, left: bool, w3: i64) -> Vec<BonusObj> {
    let b = |at: i64| imm(cx, "end_pose_b", 0x467B, at);
    let arc = |at: i64| imm(cx, "glass_arcs", 0x58C5, at);
    let hop = |at: i64| imm(cx, "end_hops", 0x50EF, at);
    let turns = cx.table("glass_turns");
    let turn = |k: i64| turns.get(usize::try_from(k.rem_euclid(8)).unwrap_or(0)).copied().unwrap_or(0);
    let crash = cx.table("glass_crash");
    let script = match Bartender::end_pose(cx, w3) {
        Pose::End { script, .. } => script,
        _ => -1,
    };
    let interlude_b = (w3 >> 8) & 3 != 3 && w3 & END_INTERLUDE_B != 0;
    // Where he stood at the kick: script 3's interludes move him after it
    // (tapper_bars, on frames before this one's), so those moves come off.
    let (mut my, mut mx) = (me.y.floor(), me.x.floor());
    if script == 3 {
        let away = if left { 1 } else { -1 };
        let hops = cx.table("hop_steps");
        let step = |k: i64| hops.get(usize::try_from(k).unwrap_or(0)).map_or(0, |&v| if v >= 0x80 { v - 0x100 } else { v });
        let first = if interlude_b {
            if n > 194 {
                mx -= 5 * away;
                my -= 5;
            }
            if n > 196 {
                mx -= 14 * away;
                my += 9;
            }
            200
        } else {
            192
        };
        for k in (0..10).filter(|k| first + 5 * k < n) {
            mx -= step(k);
            my -= if k % 2 == 0 { 5 } else { -5 };
        }
        mx &= 0xff;
    }
    // Where he stood before the step: the glass is placed from there.
    let stood = if left { mx + 1 } else { mx - 1 };
    let gx = if left { stood + b(0x46A3) - b(0x46AC) } else { stood + b(0x46A3) };
    let flags = if left { b(0x46B0) } else { b(0x469E) };
    let gy = my - b(0x4693);
    let at_side = |y: i64, pic: i64| BonusObj { at: 0xE2E2, y: y & 0xff, flags, pic, x: gx & 0xff };
    if (109..124).contains(&n) {
        return vec![at_side(gy, b(0x469A))];
    }
    if (124..128).contains(&n) {
        return vec![at_side(gy + b(0x46D8), b(0x46D3))];
    }
    let t = n - 128;
    if t < 0 {
        return Vec::new();
    }
    let y0 = gy + b(0x46D8);
    let pic = b(0x46FC);
    let dir = if left { 1 } else { -1 };
    let glass = |y: i64, flags: i64, pic: i64, x: i64| vec![BonusObj { at: 0xE356, y: y & 0xff, flags, pic, x: x & 0xff }];
    if script == 4 {
        // $594D: up c a step; after each step's wait, gone at $EE.
        let (c, top) = (imm(cx, "end_lift", 0x594D, 0x5953), imm(cx, "end_lift", 0x594D, 0x5980));
        let last = (1..64).find(|&j| y0 + c * j >= top).unwrap_or(64);
        if t >= 2 * last {
            return Vec::new();
        }
        let k = t / 2 + 1;
        return glass(y0 + c * k, turn(k), pic, gx);
    }
    // The arc: $58C5 (scripts 0, 3) or $5904 (1, 2).
    let wide = script == 0 || script == 3;
    let (c0, steps) = if wide { (arc(0x58C6), arc(0x58CB)) } else { (arc(0x5905), arc(0x590A)) };
    let along = |k: i64| if wide { k } else { (k + 1) / 2 };
    let at_step = |k: i64| -> (i64, i64) { (y0 + (0..k).map(|j| c0 - j).sum::<i64>(), gx + dir * along(k)) };
    if t < 2 * steps {
        let k = t / 2 + 1;
        let (y, x) = at_step(k);
        return glass(y, turn(k), pic, x);
    }
    let (ye, xe) = at_step(steps);
    let fe = turn(steps);
    let u = t - 2 * steps;
    match script {
        0 if u < 12 => {
            // $4887: 3 lower; $45E3: a picture a frame.
            let p = crash.get(usize::try_from(u).unwrap_or(0)).copied().unwrap_or(pic);
            glass(ye - 3, fe, p, xe)
        }
        1 if u < 12 => {
            let p = crash.get(usize::try_from(u).unwrap_or(0)).copied().unwrap_or(pic);
            glass(ye, fe, p, xe)
        }
        2 => {
            // $2DF7: at his feet.
            let d = imm(cx, "glass_down", 0x2DF7, 0x2E08);
            let x = mx + if left { d } else { 0 };
            glass(my, imm(cx, "glass_down", 0x2DF7, 0x2E0F), imm(cx, "glass_down", 0x2DF7, 0x2E13), x)
        }
        3 => {
            // $50EF/$51A9's bounce: the same code from either start.
            let (c, nb, m4) = (hop(0x50F9), hop(0x50FB), hop(0x5116));
            let bounce = |m: i64| (ye + (0..m).map(|j| c - j).sum::<i64>(), xe + dir * m);
            if u < 2 * nb {
                let m = u / 2 + 1;
                let (y, x) = bounce(m);
                return glass(y, turn(m % m4), pic, x);
            }
            let (yb, xb) = bounce(nb);
            let (f, p) = if interlude_b { (hop(0x51F0), hop(0x51F4)) } else { (hop(0x5136), hop(0x513A)) };
            let w = u - 2 * nb;
            if !interlude_b || w < 8 {
                return glass(yb, f, p, xb);
            }
            // $5263: 10 hops of 5 frames from 8 on, the glass on his head.
            let (hc, hw) = (hop(0x5266), hop(0x529E));
            let k = ((w - 8) / hw + 1).min(10);
            let dy = if k % 2 == 1 { hc } else { 0 };
            let dx: i64 = (0..k)
                .map(|i| {
                    let v = hop(0x519F + i);
                    if v >= 0x80 { v - 0x100 } else { v }
                })
                .sum();
            glass(yb + dy, f, p, xb + dx)
        }
        _ => Vec::new(),
    }
}

/// A one-byte table's byte (an immediate the round's code writes, lifted
/// from the player's romset).
fn byte_of(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.table(name).first().copied().unwrap_or(0)
}

/// The bonus round's pictures on the round's frame (`bonus_round`'s
/// count), as $2500 to $296F write them, and the line of text it shows
/// (counter bonus_text). The objects from E262 come in at frame 23
/// ($255F: bonus_objects, the six cans of two sprites each and the
/// bartender's four); each shake (from 53, 29 frames) puts the bandit
/// (E226, E22A) at the shaken can's place from bonus_bandit_places
/// ($25A3), moves his first sprite 16 frames ($25D4), shows his hand
/// (E35A) 5 ($25E3) and shakes the can twice, hiding it 2 frames of
/// each 4 ($260B to $2645). Then he walks off 2 a frame ($2669) to
/// bonus_bandit_stop and blinks five times of 8 ($2684). Each swap of the
/// shuffle ($4F17 with 9 frames, $5003 with 5) lifts the next can, moves
/// the pair 8 a frame across and puts it down; their places come back as
/// the objects trade. The bartender stands at the pointer's can ($2720)
/// from 10 frames after the shuffle; a choice ($2787) changes his
/// pictures for 20 frames, then a win ($27C2) opens the can (E2AA) and a
/// loss ($284D) sprays it first.
fn bonus_scene(cx: &ExtCx<'_>, fb: &gp_sim::world::Body, cleared: i64, done: bool) -> (Vec<BonusObj>, i64) {
    let c = (fb.ext[3] >> 24) & 0xffff;
    if done || c < 23 {
        return (Vec::new(), 0);
    }
    let table = cx.table("bonus_objects");
    let mut objs: Vec<BonusObj> = (0..16)
        .map(|i| {
            let r = table.get(i * 4..i * 4 + 4).unwrap_or(&[0, 0, 0, 0]);
            BonusObj { at: 0xE262 + 4 * i as i64, y: r[0], flags: r[1], pic: r[2], x: r[3] }
        })
        .collect();
    let places = cx.table("bonus_bandit_places");
    let place = |k: i64| -> (i64, i64) {
        let k = usize::try_from(k).unwrap_or(0);
        (places.get(2 * k).copied().unwrap_or(0), places.get(2 * k + 1).copied().unwrap_or(0))
    };
    let field = |at: u32| (fb.ext[2] >> at) & 7;
    let (unshaken, pick, ptr, shaking) = (field(40), field(43), field(46), field(55));
    let fast = cleared >= 6;
    let d = if fast { 5 } else { 9 };
    let start = if c >= 198 { shuffle_start(cx, shaking) } else { i64::MAX / 2 };
    let shuffle_end = start + 16 * d;
    let mut text = 0;
    let bf = byte_of(cx, "bandit_flags");
    let hf = byte_of(cx, "hand_flags");
    // The bandit's two sprites and his hand.
    let mut bandit = [BonusObj { at: 0xE226, y: 0, flags: bf, pic: 0, x: 0 }, BonusObj { at: 0xE22A, y: 0, flags: bf, pic: 0, x: 0 }];
    let mut hand = BonusObj { at: 0xE35A, y: 0, flags: hf, pic: 0, x: 0 };
    if c < 53 {
        // $253D: the round's first line while the cans wait ($2579).
        text = 1;
    } else if c < 53 + 5 * 29 {
        let f = (c - 53) % 29;
        let (ty, tx) = place(shaking);
        let shake = matches!(f, 21 | 22 | 25 | 26);
        bandit[0] = BonusObj {
            y: ty + f.min(15) + 1,
            pic: byte_of(cx, if shake { "bandit_pic_shake" } else { "bandit_pic_top" }),
            x: tx,
            ..bandit[0]
        };
        if f < 16 {
            bandit[1] = BonusObj { y: ty, pic: byte_of(cx, "bandit_pic_bottom"), x: tx, ..bandit[1] };
        } else {
            hand = if shake {
                BonusObj { y: ty + 32, pic: byte_of(cx, "hand_pic_shake"), x: tx, ..hand }
            } else {
                BonusObj { y: ty + 16, pic: byte_of(cx, "hand_pic"), x: (tx + 16) & 0xff, ..hand }
            };
            if shake {
                objs[usize::try_from(2 * shaking).unwrap_or(0)].y = 0;
            }
        }
    } else {
        // The walk off ($2655 to $2680) from the last shake's place, then
        // the blink ($2684 to $26A7), then standing there.
        let (ty, tx) = place(shaking);
        let steps = bandit_walk(cx, shaking);
        let w = (c - 198 + 1).min(steps);
        let blink = c - 198 - steps;
        let off = (0..40).contains(&blink) && blink % 8 < 4;
        bandit[0] = BonusObj {
            y: ty + 16,
            pic: byte_of(cx, if off { "bandit_pic_shake" } else { "bandit_pic_top" }),
            x: (tx - 2 * w) & 0xff,
            ..bandit[0]
        };
        if !off {
            bandit[1] = BonusObj { y: ty + 16, pic: byte_of(cx, "bandit_pic_walk"), x: (tx + 16 - 2 * w) & 0xff, ..bandit[1] };
        }
    }
    // The shuffle's swap under way: the pair or pairs it names, each can
    // and the one after it.
    if (start..shuffle_end).contains(&c) {
        let f = (c - start) % d;
        let pairs = flow_table(cx, "bonus_pairs");
        let can = |w: usize| -> i64 {
            let a = pairs.get(2 * w).copied().unwrap_or(0) + 256 * pairs.get(2 * w + 1).copied().unwrap_or(0);
            (a - 0xE262).div_euclid(8)
        };
        let p = usize::try_from(pick).unwrap_or(0);
        let firsts = if fast { vec![can(p)] } else { vec![can(2 * p), can(2 * p + 1)] };
        let (lift, across) = if fast {
            (if f == 4 { 0 } else { 8 }, 8 * f.clamp(0, 3))
        } else {
            (4 * (f + 1).min(3).min(9 - f - 1).max(0), 8 * (f - 2).clamp(0, 3))
        };
        for a in firsts {
            let (ia, ib) = (usize::try_from(2 * a).unwrap_or(0), usize::try_from(2 * a + 2).unwrap_or(0));
            if ib < 12 {
                objs[ib].y -= lift;
                objs[ib].x = (objs[ib].x - across) & 0xff;
                objs[ia].x = (objs[ia].x + across) & 0xff;
            }
        }
    }
    if c >= shuffle_end {
        let next = (fb.ext[3] >> 48) & 0xffff;
        let at = (next >= 0x8000).then_some(next - 0x8000);
        if c >= shuffle_end + 10 {
            // $2720: the bartender at the pointer's can.
            let (ty, tx) = place(ptr);
            let names = match at {
                Some(at) if c >= at + 20 && ptr == unshaken => ["bartender_pic_win_1", "bartender_pic_win_2", "bartender_pic_win_3"],
                Some(at) if c >= at + 76 && ptr != unshaken => ["bartender_pic_lose_1", "bartender_pic_lose_2", "bartender_pic_lose_3"],
                Some(_) => ["bartender_pic_open_1", "bartender_pic_open_2", "bartender_pic_open_3"],
                None => ["bartender_pic_wait_1", "bartender_pic_wait_2", "bartender_pic_wait_3"],
            };
            let (fl, fr) = (byte_of(cx, "bartender_flags"), byte_of(cx, "bartender_flags_right"));
            let pics = names.map(|n| byte_of(cx, n));
            objs[12] = BonusObj { at: 0xE292, y: ty + 16, flags: fl, pic: pics[0], x: (tx + 5) & 0xff };
            objs[13] = BonusObj { at: 0xE296, y: ty + 16, flags: fl, pic: pics[1], x: (tx + 21) & 0xff };
            objs[14] = BonusObj { at: 0xE29A, y: ty + 32, flags: fl, pic: pics[2], x: (tx + 5) & 0xff };
            objs[15] = BonusObj { at: 0xE29E, y: ty + 32, flags: fr, pic: pics[2], x: (tx + 21) & 0xff };
        }
        text = 2;
        if let Some(at) = at
            && c >= at + 20
        {
            let k = usize::try_from(2 * ptr).unwrap_or(0);
            let (cy, cxp) = (objs[k].y, objs[k].x);
            let beer = |dy: i64, pic: &str, dx: i64| BonusObj {
                at: 0xE2AA,
                y: cy + dy,
                flags: byte_of(cx, "beer_flags"),
                pic: byte_of(cx, pic),
                x: (cxp + dx) & 0xff,
            };
            if ptr == unshaken {
                // $27C2: the can opened, the win's lines, then the second
                // blinking every 10 frames from 10 on ($282E).
                objs.push(beer(16, "beer_pic", -11));
                objs[k].y = 0;
                let t = c - at - 20;
                text = if t < 10 || ((t - 10) / 10) % 2 == 1 { 3 } else { 4 };
            } else {
                // $284D: the can's top for 2 frames, the spray, then the
                // can opened and the loss's line ($28F2).
                text = 0;
                if c < at + 22 {
                    objs.push(beer(2, "beer_pic_spray", -2));
                } else if c < at + 76 {
                    // $28AD: the burst, its 27 objects (bonus_burst) put in
                    // from E2B2 one a frame, x by the can's, then from
                    // $28D9 taken away one a frame from the last; the
                    // first stays until the can is opened.
                    let t = c - (at + 22);
                    let rows = cx.table("bonus_burst");
                    for m in 0..27i64 {
                        let shown = t > m && (m == 0 || t < 55 - m);
                        let r = rows.get(usize::try_from(4 * m).unwrap_or(0)..usize::try_from(4 * m + 4).unwrap_or(0));
                        if let (true, Some(r)) = (shown, r) {
                            objs.push(BonusObj { at: 0xE2B2 + 4 * m, y: r[0], flags: r[1], pic: r[2], x: (r[3] + cxp) & 0xff });
                        }
                    }
                } else {
                    objs.push(beer(16, "beer_pic", -11));
                    objs[k].y = 0;
                    text = 5;
                }
            }
        }
    }
    objs.extend(bandit);
    objs.push(hand);
    objs.retain(|o| o.y != 0);
    (objs, text)
}

/// The bars' end pieces and far ends (kind bar_end) go: a level's end
/// ($5615) and a setup's $5F31 empty E356 on.
fn bar_ends_go(cx: &mut ExtCx<'_>) {
    if let Some(ek) = cx.kind("bar_end") {
        let ids: Vec<usize> = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.alive && b.kind == ek)
            .map(|(i, _)| i)
            .collect();
        for id in ids {
            cx.remove(id);
        }
    }
}

/// The frame's palette cycles ($01CC, from the interrupt every frame while
/// E410 is set): a step of the running lists when their wait (E121) goes
/// below 0, the step's colour written and its frames waited; a list's 0
/// starts it again ($01ED). The punk bar's lamps (cycle_lamps) and the
/// space bar's three lights, whose lists step together (cycle_space_1 gives
/// their frames). A setup's $5EA9 starts them on the frame after it asked
/// (cycle_on 2); the looks write the colours of the step last written.
fn palette_cycles(cx: &mut ExtCx<'_>) {
    let c = |cx: &ExtCx<'_>, n: &str| cx.counter(n).unwrap_or(0);
    let (mut on, mut list, mut index, mut wait, mut step) =
        (c(cx, "cycle_on"), c(cx, "cycle_list"), c(cx, "cycle_index"), c(cx, "cycle_wait"), c(cx, "cycle_step"));
    // A clip's start loads the pointer's low byte and the list's start:
    // the step the pointer stands at, from them.
    let start = c(cx, "cycle_start");
    if start != 0 {
        index = ((index - start) & 0xff) / 3;
        step = index;
        let shown = cx.kind("flow").and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k)).map_or(0, |b| (b.ext[3] >> 8) & 3);
        list = if on != 0 && shown >= 2 { shown } else { 0 };
        cx.set_counter("cycle_start", 0);
    }
    if on == 1 && list >= 2 {
        let rows = cx.table(if list == 2 { "cycle_lamps" } else { "cycle_space_1" }).to_vec();
        wait = (wait - 1) & 0xff;
        if wait >= 0x80 {
            let row = |i: i64| rows.get(usize::try_from(i * 3).unwrap_or(usize::MAX)).copied().unwrap_or(0);
            let mut i = index;
            if row(i) == 0 {
                i = 0;
            }
            wait = row(i);
            step = i + 1;
            index = i + 1;
        }
    } else if on == 2 {
        // $5EA9, the frame after the setup's $5E99: the lists from their
        // start, stepped from the next frame.
        on = 1;
        index = 0;
        wait = 0;
    }
    for (n, v) in [("cycle_on", on), ("cycle_list", list), ("cycle_index", index), ("cycle_wait", wait), ("cycle_step", step)] {
        if cx.counter(n) != Some(v) {
            cx.set_counter(n, v);
        }
    }
}

/// The counter screen from the counter stage: $5DF6 takes the stage's
/// word from the words at $61ED (stage_screens, lifted) and loads the
/// screen it names; the looks list the screens by address, so the screen
/// is the word's place among the table's words.
fn set_screen(cx: &mut ExtCx<'_>) {
    let Some(stage) = cx.counter("stage") else { return };
    let rows = cx.table("stage_screens");
    let words: Vec<i64> = rows.chunks(2).map(|w| w[0] | (w.get(1).copied().unwrap_or(0) << 8)).collect();
    let Some(&word) = usize::try_from(stage).ok().and_then(|n| words.get(n)) else { return };
    let mut below: Vec<i64> = words.iter().copied().filter(|&w| w < word).collect();
    below.sort_unstable();
    below.dedup();
    let screen = i64::try_from(below.len()).unwrap_or(0);
    if cx.counter("screen") != Some(screen) {
        cx.set_counter("screen", screen);
    }
}

/// Where a wait in a routine goes on (E3A9, as $232B keeps it): the return
/// from the `n`-th (from 1) call to $232B in the routine's lifted code,
/// counted from its start, the operand of the call that reaches it. Each
/// set's code and start are its own, so a set whose code sits elsewhere
/// resumes at its own places.
fn wait_return(cx: &ExtCx<'_>, code: &str, start: &str, n: usize) -> Option<i64> {
    let call = cx.table("wait_call");
    let window = cx.table(code);
    let s = cx.table(start);
    if call.len() != 3 || s.len() < 2 || n == 0 {
        return None;
    }
    let base = s[0] | (s[1] << 8);
    let mut seen = 0;
    for i in 0..window.len().saturating_sub(2) {
        if window[i..i + 3] == call[..] {
            seen += 1;
            if seen == n {
                return Some(base + i as i64 + 3);
            }
        }
    }
    None
}

/// An entry of a points table as a number: six digits a byte, the most
/// significant first.
fn points_of(row: &[i64]) -> i64 {
    row.iter().take(6).fold(0, |n, d| n * 10 + d)
}

/// A table the model gives the flow's hook (tapper_flow), for the bars'.
fn flow_table(cx: &ExtCx<'_>, name: &str) -> Vec<i64> {
    cx.kind("flow")
        .and_then(|k| cx.spec.kinds[k].ext_tables.get(name))
        .cloned()
        .unwrap_or_default()
}

fn bp(cx: &ExtCx<'_>, name: &str) -> i64 {
    let Some(k) = cx.kind("bars") else { return 0 };
    cx.spec.kinds[k].ext_params.get(name).map_or(0, |v| v.floor())
}

/// tapper_bars moves the bartender in the end scripts and the slide after
/// his own hook has drawn him; his parts (kind bartender_part, by place
/// `k` from E2EA) go with him here: `f` gets each part's x and y.
fn place_parts(cx: &mut ExtCx<'_>, ks: &[i64], hidden_too: bool, f: impl Fn(i64, i64, i64) -> (i64, i64)) {
    let Some(pk) = cx.kind("bartender_part") else { return };
    let found: Vec<(usize, gp_sim::world::Body)> = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.alive && b.kind == pk && ks.contains(&(b.ext[1] - 1)))
        .map(|(i, b)| (i, *b))
        .collect();
    for (id, mut b) in found {
        if b.y.floor() == 0 && !hidden_too {
            continue;
        }
        let (x, y) = f(b.ext[1] - 1, b.x.floor(), b.y.floor());
        b.x = Fx::from_int(x & 0xff);
        b.y = Fx::from_int(y & 0xff);
        cx.set_body(id, b);
    }
}

/// The bartender's six objects as a hook other than his own sets them
/// (tapper_bars, which runs after his): `f` gets his object and his five
/// parts by place, and what it leaves is written back to the bodies (a
/// part with no body is left out). The tap of his bar goes to `tap_y`'s
/// y when `f` gives one: 0 hides it, 1 puts it back ($0679).
fn bartender_objects(cx: &mut ExtCx<'_>, f: impl FnOnce(&ExtCx<'_>, &mut Obj, &mut [Obj; 5], &mut Option<i64>, (i64, bool))) {
    let Some(pk) = cx.kind("player") else { return };
    let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.alive && b.kind == pk).map(|(i, b)| (i, *b)) else {
        return;
    };
    let w = pb.ext[2];
    let mut me = Obj { y: pb.y.floor(), flags: (w >> 16) & 0xff, pic: (w >> 24) & 0xff, x: pb.x.floor() };
    let mut ids: [Option<usize>; 5] = [None; 5];
    let mut parts = [Obj::default(); 5];
    if let Some(qk) = cx.kind("bartender_part") {
        for (id, b) in cx.world.bodies.iter().enumerate() {
            if b.alive && b.kind == qk && (1..=5).contains(&b.ext[1]) {
                let k = usize::try_from(b.ext[1] - 1).unwrap_or(0);
                ids[k] = Some(id);
                parts[k] = Obj { y: b.y.floor(), flags: b.ext[0] & 0xff, pic: (b.ext[0] >> 8) & 0xff, x: b.x.floor() };
            }
        }
    }
    let bar = pb.ext[0] & 3;
    let tk = cx.kind("tap");
    let tap = tk.and_then(|tk| cx.world.bodies.iter().enumerate().filter(|(_, b)| b.kind == tk).nth(usize::try_from(bar).unwrap_or(0)).map(|(i, b)| (i, *b)));
    let tap_at = tap.map_or((0, false), |(_, t)| (t.x.floor(), t.facing_left));
    let mut tap_y = None;
    f(cx, &mut me, &mut parts, &mut tap_y, tap_at);
    let mut b = pb;
    b.y = Fx::from_int(me.y);
    b.x = Fx::from_int(me.x);
    b.ext[2] = (w & !(0xffff << 16)) | ((me.flags & 0xff) << 16) | ((me.pic & 0xff) << 24);
    cx.set_body(pid, b);
    for (k, o) in parts.iter().enumerate() {
        if let Some(id) = ids[k] {
            let mut q = cx.world.bodies[id];
            q.y = Fx::from_int(o.y);
            q.x = Fx::from_int(o.x);
            q.ext[0] = (o.flags & 0xff) | ((o.pic & 0xff) << 8);
            cx.set_body(id, q);
        }
    }
    if let (Some(v), Some((id, mut t))) = (tap_y, tap) {
        let y = if v == 0 { 0 } else { cx.table("bar_objects").get(usize::try_from(bar * 4).unwrap_or(0)).copied().unwrap_or(t.y.floor()) };
        t.y = Fx::from_int(y);
        cx.set_body(id, t);
    }
}

/// A life lost, as $1CE1 ($1D15 to $1D39) and $2389 ($23B5 to $23D9) both
/// start it: his picture `pic`, his head's `head`, 16 above him (`up`, the
/// `add a,$F0`'s byte) at his x, his other four objects hidden, and the
/// tap of his bar put back ($0679). The pictures are the code's own
/// immediates, read from `table` (lifted from `start`) at their places.
/// His own object is `pb`, which the caller sets.
fn lost_pose(cx: &mut ExtCx<'_>, pb: &mut gp_sim::world::Body, table: &str, start: i64, (pic, head, up): (i64, i64, i64)) {
    let (pic, head, up) = (imm(cx, table, start, pic), imm(cx, table, start, head), imm(cx, table, start, up));
    pb.ext[2] = (pb.ext[2] & !(0xff << 24)) | (pic << 24);
    let (mx, my) = (pb.x.floor(), pb.y.floor());
    if let Some(qk) = cx.kind("bartender_part") {
        let found: Vec<(usize, gp_sim::world::Body)> =
            cx.world.bodies.iter().enumerate().filter(|(_, b)| b.alive && b.kind == qk).map(|(i, b)| (i, *b)).collect();
        for (id, mut q) in found {
            if q.ext[1] == 5 {
                q.y = Fx::from_int((my + up) & 0xff);
                q.x = Fx::from_int(mx);
                q.ext[0] = (q.ext[0] & 0xff) | (head << 8);
            } else {
                q.y = Fx::ZERO;
            }
            cx.set_body(id, q);
        }
    }
    // $0679: the tap's y from its bar's row of $4DCC.
    let bar = pb.ext[0] & 3;
    if let Some(tk) = cx.kind("tap")
        && let Some((id, t)) = cx.world.bodies.iter().enumerate().filter(|(_, b)| b.kind == tk).nth(usize::try_from(bar).unwrap_or(0))
        && let Some(&y) = cx.table("bar_objects").get(usize::try_from(bar * 4).unwrap_or(0))
        && t.y.floor() != y
    {
        let mut t = *t;
        t.y = Fx::from_int(y);
        cx.set_body(id, t);
    }
}

/// The bartender above GET READY TO SERVE ($5404): `Some(false)` his
/// pose at the tap as $543D sets it, `Some(true)` the one $5489 sets 30
/// frames on, `None` all six objects emptied ($54DB's $5F31). The
/// pictures, flags and places are the code's own immediates, read from
/// the intro_pose rows (lifted from $543D) at their places. A part his
/// hook hasn't made yet (a start inside the intro) is made here.
fn intro_pose(cx: &mut ExtCx<'_>, pose: Option<bool>, spawn: &mut Vec<gp_sim::world::Body>) {
    let c = |cx: &ExtCx<'_>, at: i64| imm(cx, "intro_pose", 0x543D, at);
    // (y, x, flags, picture) of his own object and of parts 0, 1 and 4.
    let objs: [(i64, i64, i64, i64); 4] = match pose {
        Some(false) => {
            let x = c(cx, 0x545A);
            let x2 = x + c(cx, 0x546A);
            let hy = c(cx, 0x5472);
            let y = hy + c(cx, 0x5477);
            let y1 = y + c(cx, 0x547F);
            [
                (y, x, c(cx, 0x5448), c(cx, 0x5444)),
                (y, x2, c(cx, 0x5450), c(cx, 0x544C)),
                (y1, x2, c(cx, 0x5458), c(cx, 0x5454)),
                (hy, x, c(cx, 0x5468), c(cx, 0x5464)),
            ]
        }
        Some(true) => {
            let x = c(cx, 0x548E);
            let x2 = x + c(cx, 0x54AA);
            let y = c(cx, 0x54B6);
            let hy = y - c(cx, 0x54BE);
            let y1 = hy + c(cx, 0x54C3);
            [
                (y, x, c(cx, 0x5499), c(cx, 0x5495)),
                (y, x2, c(cx, 0x549D), c(cx, 0x54B4)),
                (y1, x2, c(cx, 0x54A1), c(cx, 0x54CA)),
                (hy, x, c(cx, 0x54A5), c(cx, 0x54CE)),
            ]
        }
        None => [(0, 0, 0, 0); 4],
    };
    if let Some(pk) = cx.kind("player")
        && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.alive && b.kind == pk)
    {
        let mut pb = *pb;
        if pose.is_some() {
            let (y, x, f, p) = objs[0];
            pb.y = Fx::from_int(y & 0xff);
            pb.x = Fx::from_int(x & 0xff);
            pb.ext[2] = (pb.ext[2] & !(0xffff << 16)) | ((f & 0xff) << 16) | ((p & 0xff) << 24);
        } else {
            pb.y = Fx::ZERO;
        }
        cx.set_body(pid, pb);
    }
    let Some(qk) = cx.kind("bartender_part") else { return };
    let mut have = [false; 5];
    let found: Vec<(usize, gp_sim::world::Body)> =
        cx.world.bodies.iter().enumerate().filter(|(_, b)| b.alive && b.kind == qk).map(|(i, b)| (i, *b)).collect();
    let of = |k: i64| match k {
        0 => Some(objs[1]),
        1 => Some(objs[2]),
        4 => Some(objs[3]),
        _ => None,
    };
    for (id, mut q) in found {
        let k = q.ext[1] - 1;
        if let Some(n) = usize::try_from(k).ok().filter(|&n| n < 5) {
            have[n] = true;
        }
        match of(k).filter(|_| pose.is_some()) {
            Some((y, x, f, p)) => {
                q.y = Fx::from_int(y & 0xff);
                q.x = Fx::from_int(x & 0xff);
                q.ext[0] = (f & 0xff) | ((p & 0xff) << 8);
            }
            None => q.y = Fx::ZERO,
        }
        cx.set_body(id, q);
    }
    if pose.is_some() {
        for k in [0i64, 1, 4] {
            if have[k as usize] {
                continue;
            }
            let Some((y, x, f, p)) = of(k) else { continue };
            let mut b = gp_sim::systems::new_body(cx.spec, cx.world, qk, Fx::from_int(x & 0xff), Fx::from_int(y & 0xff), false);
            b.ext[0] = (f & 0xff) | ((p & 0xff) << 8);
            b.ext[1] = k + 1;
            spawn.push(b);
        }
    }
}

impl Bars {
    /// The level's end script $5623 picks: the list by the tap's word 1
    /// (E3ED), the entry by the levels cleared mod 10 (end_script_lists).
    fn end_script(cx: &ExtCx<'_>, cleared: i64) -> i64 {
        let words: Vec<i64> = cx.table("end_script_lists").chunks(2).map(|w| w[0] + 256 * w.get(1).copied().unwrap_or(0)).collect();
        let base = words.iter().copied().min().unwrap_or(0);
        let list = cx.kind("tap").and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k)).map_or(0, |b| b.ext[1] & 1);
        words.get(usize::try_from(list * 10 + cleared % 10).unwrap_or(0)).map_or(0, |w| (w - base) / 5)
    }

    /// The level's end's requests on its frame `h` (as Bartender::end_pose
    /// counts it, $4742 once), by its script ($5623) and, in script 3, its
    /// interlude. $4603: $2A after its wait of 15 ($4609); $4742: $11 as
    /// the pour starts ($4778), a gulp each 4 frames as he drinks ($4874
    /// calls $1B83 six times, the `sound_drink` rows) and 2 after the last
    /// ($4879); back in $4603, $14 as he kicks the glass ($46E2, 128).
    /// Then each script's own: 0 and 1 $45E3's $1B, 2 $2DF7's $21, 3 the
    /// interlude's $34 and $33 and its hops' $32 ($50EF) or $21 ($51A9) on
    /// every other hop, 4 $594D's $36, 2 once the object is up, $1B and $2F
    /// after its waits of 20 and 40, and $4546's $35 40 later (its 2 and
    /// $37 as the object lands, with the drop). Frames measured on the
    /// corpus's level ends (tapper.l1.001, l2, l3, l4, l5, l13 and
    /// clear.004), each as the code's waits give it.
    fn end_sounds(cx: &mut ExtCx<'_>, h: i64, script: i64, interlude_b: bool) {
        match h {
            16 => ask(cx, "sound_end", &[0xA3]),
            58 => ask(cx, "sound_end_drink", &[0x97]),
            89 | 93 | 97 | 101 | 105 => ask(cx, "sound_drink", &[(109 - h) / 4]),
            109 => {
                ask(cx, "sound_drink", &[0]);
                ask(cx, "sound_end_drink", &[0x198]);
            }
            128 => ask(cx, "sound_end_drink", &[1]),
            _ => {}
        }
        match script {
            0 if h == 172 => ask(cx, "sound_end", &[0x7D]),
            0 if h == 199 => ask(cx, "sound_end_toss", &[1]),
            1 if h == 170 => ask(cx, "sound_end", &[0x7D]),
            2 if h == 170 => ask(cx, "sound_end_turn", &[1]),
            3 => {
                if h == 172 {
                    ask(cx, "sound_interlude", &[if interlude_b { 0xBB } else { 1 }]);
                } else if h == 192 {
                    ask(cx, "sound_interlude", &[if interlude_b { 0x107 } else { 0x4D }]);
                }
                let (first, at) = if interlude_b { (200, 0x17E) } else { (192, 0x7B) };
                if h >= first && (h - first) % 10 == 0 && (h - first) / 10 < 5 {
                    ask(cx, "sound_interlude", &[at]);
                }
            }
            4 => {
                // $594D lifts the object from 6 above him, 10 each 2
                // frames until at least $EE: k steps (as tapper_bars
                // places the drop).
                let by = cx
                    .kind("player")
                    .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
                    .map_or(0, |b| b.y.floor());
                let k = (238 - (by + 6) + 9).max(10) / 10;
                let up = 128 + 2 * k;
                if h == 128 {
                    ask(cx, "sound_lift", &[1]);
                } else if h == up {
                    ask(cx, "sound_lift", &[0x38]);
                } else if h == up + 20 {
                    ask(cx, "sound_lift", &[0x6F]);
                } else if h == up + 60 {
                    ask(cx, "sound_lift", &[0x7A]);
                } else if h == up + 100 {
                    ask(cx, "sound_end", &[1]);
                }
            }
            _ => {}
        }
    }

    /// The bar a body on line y is at: 0 the bottom.
    fn bar_of(cx: &ExtCx<'_>, y: i64) -> i64 {
        let line = bp(cx, "line");
        let step = bp(cx, "line_step");
        (y - line + step / 2).div_euclid(step)
    }
    /// Bar `bar`'s ends: the code's table at $0977, 4 bytes a bar, its
    /// left end at +1 and its right at +2 ($078B to $079D, $08B8 to
    /// $08D2), lifted from the player's romset.
    fn ends(cx: &ExtCx<'_>, bar: i64) -> (i64, i64) {
        let row = usize::try_from(bar & 3).unwrap_or(0) * 4;
        let t = cx.table("bar_ends");
        (t.get(row + 1).copied().unwrap_or(0), t.get(row + 2).copied().unwrap_or(0))
    }
    /// A picture copied over the dancers' 9 objects ($150F or $0C62,
    /// ldir): an object whose y is 0 is none.
    fn put_picture(cx: &ExtCx<'_>, dk: usize, dancers: &mut [Option<gp_sim::world::Body>; 9], pic: &[i64]) {
        for (s, d) in dancers.iter_mut().enumerate() {
            let Some(o) = pic.get(s * 4..s * 4 + 4) else { continue };
            if o[0] == 0 {
                *d = None;
                continue;
            }
            let (x, y) = (Fx::from_int(o[3]), Fx::from_int(o[0]));
            let mut b = d.unwrap_or_else(|| gp_sim::systems::new_body(cx.spec, cx.world, dk, x, y, false));
            b.x = x;
            b.y = y;
            b.ext[0] = s as i64 + 1;
            b.ext[1] = o[1];
            b.ext[2] = o[2];
            *d = Some(b);
        }
    }

    /// The last dancer is the tip's object, E25E: once a picture is
    /// copied over it, the tip is what it holds, a tip only while its
    /// picture is $4D ($0B8C).
    fn tip_from_dancers(
        cx: &ExtCx<'_>,
        tip_k: Option<usize>,
        dancers: &[Option<gp_sim::world::Body>; 9],
        tip: &mut Option<(Option<usize>, gp_sim::world::Body)>,
        tips_gone: &mut Vec<usize>,
    ) {
        if let Some((Some(id), _)) = tip.take() {
            tips_gone.push(id);
        }
        if let (Some(d), Some(tk)) = (dancers[8], tip_k)
            && d.ext[2] == 0x4D
        {
            *tip = Some((None, gp_sim::systems::new_body(cx.spec, cx.world, tk, d.x, d.y, false)));
        }
    }

    fn tap_left(cx: &ExtCx<'_>, bar: i64) -> bool {
        cx.kind("tap").is_some_and(|k| {
            cx.world
                .bodies
                .iter()
                .filter(|b| b.kind == k)
                .nth(usize::try_from(bar).unwrap_or(0))
                .is_some_and(|b| b.facing_left)
        })
    }
}

impl Ext for Bars {
    fn step(&self, cx: &mut ExtCx<'_>) {
        use gp_sim::world::Body;
        let (Some(mug_k), Some(cust_k)) = (cx.kind("mug"), cx.kind("customer")) else {
            return;
        };
        set_screen(cx);
        palette_cycles(cx);
        // The attract mode (E3BD 1): nothing moves but in its demo game
        // (tapper_flow keeps the logo's time).
        let demo = in_demo(cx);
        if cx.counter("mode") == Some(1) && !demo {
            // The extra life's scores the scene's close draws (E00D to
            // E010, the defaults the boot copies to E000 on).
            let defaults = cx.table("defaults").to_vec();
            let digit = |k: usize| defaults.get(k).copied().unwrap_or(0);
            for (name, k) in [("extra_first", 0xD), ("extra_every", 0xF)] {
                let v = 10 * digit(k) + digit(k + 1);
                if cx.counter(name) != Some(v) {
                    cx.set_counter(name, v);
                }
            }
            // The instruction scene's objects ($2EE0, state 6); none
            // elsewhere in the attract mode.
            if let Some(ok) = cx.kind("bonus_object") {
                let want = scene_frame(cx).map(|n| demo_scene(cx, n)).unwrap_or_default();
                place_objects(cx, ok, &want);
            }
            return;
        }
        let (timer_k, rec_k) = (cx.kind("timer"), cx.kind("record"));
        // The points this frame adds to the score ($5EE7).
        let mut points = 0;
        // The lives as this frame leaves them, when it takes one: the flow
        // the HUD counters read at the end is the last frame's.
        let mut lives_now: Option<i64> = None;
        let player_k = cx.kind("player");
        let line = bp(cx, "line");
        let step = bp(cx, "line_step");
        let (mut walk, mut empty_speed, full_speed) = (cx.me.ext[0] & 0xff, cx.me.ext[1] & 0xff, cx.me.ext[2]);
        // Word 1 above the empty mug's speed: the show a tip starts
        // ($14E7, run by $0C25): E70F (on), E712 (frames left), E70E
        // (frames to the dancers' next picture) and E710 (that picture's
        // place in the bar's list), a byte each.
        let mut show = Show {
            on: (cx.me.ext[1] >> 8) & 0xff != 0,
            left: (cx.me.ext[1] >> 16) & 0xff,
            tick: (cx.me.ext[1] >> 24) & 0xff,
            picture: (cx.me.ext[1] >> 32) & 0xff,
        };
        // The dancers, the 9 objects from E23E (y, flags, picture, x), by
        // slot; the last is the tip's object, E25E.
        let dancer_k = cx.kind("dancer");
        let mut dancer_ids: [Option<usize>; 9] = [None; 9];
        // The bonus round's pictures this frame (bonus_scene), with its line
        // of text; None out of the round.
        let mut bonus_objs: Option<(Vec<BonusObj>, i64)> = None;
        let mut dancers: [Option<Body>; 9] = [None; 9];
        if let Some(dk) = dancer_k {
            for (i, b) in cx.world.bodies.iter().enumerate().filter(|(_, b)| b.alive && b.kind == dk) {
                if let Some(n) = usize::try_from(b.ext[0] - 1).ok().filter(|&n| n < 9) {
                    dancer_ids[n] = Some(i);
                    dancers[n] = Some(*b);
                }
            }
        }
        // Word 0 above the walk: E72E (customers' throws left to the next
        // tip), E72F (tips left this level) and E006 (the tips setting,
        // copied from the program's defaults at boot by $0517).
        let mut tip_count = (cx.me.ext[0] >> 8) & 0xff;
        let mut tips_left = (cx.me.ext[0] >> 16) & 0xff;
        let tip_setting = (cx.me.ext[0] >> 24) & 0xff;
        let tip_k = cx.kind("tip");
        // Word 3: E3B1 (the respawn wait), E4A8 (levels cleared), E4A9 (the
        // level's row of level_times, from 1).
        let mut w_word = cx.me.ext[3] & 0xffff;
        let mut cleared = (cx.me.ext[3] >> 16) & 0xff;
        let mut level = (cx.me.ext[3] >> 24) & 0xff;
        let layout = cx
            .kind("flow")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
            .map_or(0, |b| b.ext[1] & 0xff);
        let starts = cx.table("customer_starts").to_vec();
        let arrivals: Vec<i64> = cx
            .table("arrivals")
            .chunks(2)
            .map(|w| w[0] + 256 * w.get(1).copied().unwrap_or(0))
            .collect();
        let times = cx.table("level_times").to_vec();
        let row_word = |row: i64, at: usize| -> i64 {
            let base = usize::try_from((row - 1).max(0)).unwrap_or(0) * 12 + at;
            times.get(base).copied().unwrap_or(0) + 256 * times.get(base + 1).copied().unwrap_or(0)
        };
        let of_kind = |cx: &ExtCx<'_>, k: Option<usize>| -> Vec<(usize, Body)> {
            k.map(|k| {
                cx.world
                    .bodies
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.alive && b.kind == k)
                    .map(|(i, b)| (i, *b))
                    .collect()
            })
            .unwrap_or_default()
        };
        let mut timers = of_kind(cx, timer_k);
        let mut recs = of_kind(cx, rec_k);
        let mut bodies: Vec<(usize, Body)> = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.alive && (b.kind == mug_k || b.kind == cust_k))
            .map(|(i, b)| (i, *b))
            .collect();
        let mut removed: Vec<usize> = Vec::new();
        // The tip, the object at E25E (one at most): kept apart from the
        // mugs and customers, and written back when the step ends.
        let mut tip: Option<(Option<usize>, Body)> = of_kind(cx, tip_k).into_iter().next().map(|(i, b)| (Some(i), b));
        let mut tips_gone: Vec<usize> = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.alive && Some(b.kind) == tip_k)
            .map(|(i, _)| i)
            .skip(1)
            .collect();
        // Bodies this step adds, with the ids they will have.
        let mut next_id = cx.world.bodies.len();
        let mut added: Vec<usize> = Vec::new();
        let mut timers_changed: Vec<usize> = Vec::new();
        let mut new_timers: Vec<Body> = Vec::new();
        let rec_of = |recs: &[(usize, Body)], k: i64| recs.iter().position(|(_, r)| r.ext[2] == k);

        // $603C: a timer for `routine` after `delay` frames, in the first
        // idle entry of the list at E417.
        let schedule = |timers: &mut Vec<(usize, Body)>,
                            changed: &mut Vec<usize>,
                            new_timers: &mut Vec<Body>,
                            cx: &ExtCx<'_>,
                            delay: i64,
                            routine: i64| {
            if let Some(n) = timers.iter().position(|(_, t)| t.ext[0] == 0) {
                timers[n].1.ext[0] = delay & 0xffff;
                timers[n].1.ext[1] = routine;
                changed.push(n);
            } else if let Some(k) = timer_k {
                // A new entry: counted with the others from now on, and
                // spawned when the step ends.
                let mut t = gp_sim::systems::new_body(cx.spec, cx.world, k, Fx::ZERO, Fx::ZERO, false);
                t.ext[0] = delay & 0xffff;
                t.ext[1] = routine;
                timers.push((usize::MAX, t));
                let _ = new_timers;
            }
        };

        // A level's start, the frame after the setup cleared its RAM: the
        // full mug's speed back to -3 ($4C10), the level's timers ($4E50)
        // and its first customers' arrivals 10 frames on ($4C19 to $4C43).
        let mut full_speed = full_speed;
        if let Some(fk) = cx.kind("flow")
            && let Some((fid, fb)) = cx
                .world
                .bodies
                .iter()
                .enumerate()
                .find(|(_, b)| b.kind == fk)
                .map(|(i, b)| (i, *b))
            && (fb.ext[2] == 1 || fb.ext[2] == 2)
        {
            // 1: the whole setup; 2: a setup coming back after its waits,
            // whose records and level timers are in already.
            let whole = fb.ext[2] == 1;
            let mut fb = fb;
            // The setup's sounds, on this frame either way (after the
            // space bar's waits at $4B8B): $4B9A asks for 2, then after the
            // arrivals $4C4E for 2 and $4C56 for the bar shown's byte of
            // the table at $4C71 (the `sound_game` rows are the program
            // from $4B34).
            let shown_now = (fb.ext[3] >> 8) & 3;
            // E3B8 as it is: 4 in the demo ($3C48), which picks the fifth
            // byte of $4C71's table and the fifth block of records.
            let shown_byte = (fb.ext[3] >> 8) & 0xff;
            // The punk bar's and the space bar's $4937 ($4C4B) wait for the
            // next frame first ($5E99), so their last two come then.
            if shown_now >= 2 {
                ask(cx, "sound_game", &[0x67]);
                sound::ask_setup_next(cx);
            } else {
                ask(cx, "sound_game", &[0x67, 0x11B, 0x13D + shown_byte]);
            }
            // $4C4B's $4937: the punk bar's and the space bar's setups
            // empty the palette cycles ($5E99) and add their own on the
            // next frame ($5EA9); the others leave them as they are.
            if shown_now >= 2 {
                cx.set_counter("cycle_on", 2);
                cx.set_counter("cycle_list", shown_now);
            }
            // 4: the setup's frame, for tapper_bartender (play from the
            // next); tapper_flow clears it.
            fb.ext[2] = 4;
            // When the setup's timers count: the punk bar's and the space
            // bar's setups (bar shown 2, 3) call $5E99 through $4937, which
            // waits for the next frame ($60F6), so their timers count from
            // the next frame; the saloon's and the sports bar's on this one
            // (setup_late_bars, a bit a bar shown). A setup coming back
            // after its waits sets them after the frame's count too.
            let ptrs: Vec<i64> = cx
                .table("start_list_ptrs")
                .chunks(2)
                .map(|w| w[0] + 256 * w.get(1).copied().unwrap_or(0))
                .collect();
            let lists = cx.table("start_lists").to_vec();
            let first = ptrs.get(usize::try_from(cleared.min(9)).unwrap_or(0)).copied().unwrap_or(0)
                - ptrs.first().copied().unwrap_or(0);
            let first = usize::try_from(first).unwrap_or(usize::MAX);
            let shown_bit = 1 << ((fb.ext[3] >> 8) & 3);
            let late = i64::from(!whole || bp(cx, "setup_late_bars") & shown_bit != 0);
            // Play from here (the game reaches state 10 a few frames on,
            // playing all the while). Out of a game $4C63 sets state 3, the
            // demo's play ($22D4), with the level's RAM cleared (E6FF and
            // E706 0, $4B65).
            fb.ext[3] = if demo { (fb.ext[3] & 0xff00) | 3 } else { (fb.ext[3] & !0xff) | 10 };
            let shown = usize::try_from(shown_now).unwrap_or(0);
            // $4A65: the records of the bar shown, each slot's wait, drink,
            // bar and number, and its facing from the layout.
            let blocks = record_block(cx, (fb.ext[3] >> 8) & 0xff);
            // The records are carried in slot order; before a game's first
            // setup their bytes are blank, so the n-th is slot n.
            // A run from power-on has no records yet (a clip's start
            // carries them): the setup's copy makes all 16, new bodies
            // spawned when the step ends.
            if let Some(rk) = rec_k {
                while recs.len() < 16 {
                    let r = gp_sim::systems::new_body(cx.spec, cx.world, rk, Fx::ZERO, Fx::ZERO, false);
                    recs.push((usize::MAX, r));
                }
            }
            for (k, (_, r)) in recs.iter_mut().enumerate() {
                let Some(rec) = blocks.get(k * 32..(k + 1) * 32) else {
                    continue;
                };
                let bar = rec[8];
                let facing = (layout >> bar) & 1;
                r.ext[0] = rec[2] | (rec[9] << 8);
                r.ext[1] = bar | (facing << 8) | (rec[14] << 16);
                r.ext[2] = rec[12];
                r.ext[3] = rec[16];
            }
            cx.set_body(fid, fb);
            full_speed = -3 * 256;
            cx.me.ext[2] = full_speed;
            // $4B78 and $4E46: the customers' walk and an empty mug's speed
            // back to 1.
            walk = 1;
            empty_speed = 1;
            cx.me.ext[1] = 1;
            // $4B65 clears E6F2 to E733, the show's bytes among them.
            show = Show { on: false, left: 0, tick: 0, picture: 0 };
            // $4BD6: the first tip comes with the second throw.
            tip_count = 2;
            if whole {
                // $588A: the level's tips, from the levels cleared and
                // the setting E006.
                tips_left = if cleared == 0 {
                    1
                } else {
                    let b = match tip_setting {
                        0 | 1 => cleared << 1,
                        2..=5 => cleared,
                        _ => cleared >> 1,
                    } & 0xff;
                    if b >= 5 { 4 } else if cleared == 1 { b + 1 } else { b }
                };
            }
            // $4E46 sets the level's timers at $4B80, before $575B: in the
            // space bar (shown 3) that draws and waits 30 frames ($232B)
            // and $5E99 one more before the bartender and the arrivals
            // ($4BCB, $4C2D), and the main loop counts the timers ($60C1)
            // through those waits, so they have counted 32 frames by then
            // (tapper.l13.001: set on 19641, the arrivals on 19672).
            let ahead = if shown == 3 { 32 } else { 0 };
            if whole {
                w_word = row_word(level, 0);
                for (at, routine) in [(2, 0x2023), (4, 0x1B02), (6, 0x2209)] {
                    let d = row_word(level, at);
                    schedule(&mut timers, &mut timers_changed, &mut new_timers, cx, (d + late - ahead).max(1), routine);
                }
            }
            if whole && cleared >= 5 {
                w_word = row_word(level, 8);
                let d = row_word(level, 10);
                schedule(&mut timers, &mut timers_changed, &mut new_timers, cx, (d + late - ahead).max(1), 0x1B2E);
            }
            let mut i = first;
            while let Some(&slot) = lists.get(i) {
                if slot == 0xFF {
                    break;
                }
                if let Some(&routine) = arrivals.get(usize::try_from(slot).unwrap_or(usize::MAX)) {
                    schedule(&mut timers, &mut timers_changed, &mut new_timers, cx, 10 + late, routine);
                }
                i += 1;
            }
        }

        // The play routine ($0660) runs only in the game's play state
        // (E397 10); in a life lost (15, 16) the bars
        // stand still while the timers count. The model doesn't change
        // the state yet: it keeps the one a clip started in.
        let mut flow = cx
            .kind("flow")
            .and_then(|k| cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == k))
            .map(|(i, b)| (i, *b));
        let mut state = flow.map_or(10, |(_, b)| b.ext[3] & 0xff);
        // A start inside the bonus round ($2500): $232B's place (E3A9)
        // gives its frame. The wait of 20 at $252C ends on the round's
        // frame 22, the cans' objects coming in on the next ($255F); a start
        // inside it is that less its frames left, the timer for $2346 (0
        // once E397 is back to the state it was called in, 16). The
        // Budweiser clips start as it ends; Root Beer's 14 frames before.
        if let Some((fid, mut fb)) = flow
            && (fb.ext[3] >> 16) & 0xff != 12
            && Some((fb.ext[1] >> 16) & 0xffff) == wait_return(cx, "bonus_code", "bonus_start", 1)
        {
            let left = if state == 16 {
                0
            } else {
                timers.iter().find(|(_, t)| t.ext[1] == 0x2346).map_or(0, |(_, t)| t.ext[0])
            };
            let n = (22 - left).max(0);
            fb.ext[3] = (fb.ext[3] & 0xff00) | 15 | (12 << 16) | (n << 24);
            fb.ext[2] &= !BONUS_MASK;
            fb.ext[1] &= !(0xffff << 16);
            cx.set_body(fid, fb);
            flow = Some((fid, fb));
            state = 15;
        }
        // A start inside $1CE1's first waits (tapper.die.001 to die.003
        // start on the last of its three waits of 2): $232B keeps the
        // state the wait was called in at E3A8 (10, the play: $1CE1 runs
        // from $0890), where it goes on at E3A9 and B at E39F ($5FE6), and
        // its frames left are the timer for $2346. The script's frame
        // count (the flow's word 3 from bit 24, sub 10) is where it goes on
        // less the frames left: $1D4B after the wait of 30, $1D62 after
        // each wait of 2, B counting them down from 3. The customer is the
        // one at the bartender's end of his bar (E6F9, $08D9).
        if (state == 15 || state == 16)
            && let Some((fid, mut fb)) = flow
            && (fb.ext[3] >> 16) & 0xff != 10
            && (fb.ext[1] >> 8) & 0xff == 10
            && let ret @ (0x1D4B | 0x1D62) = (fb.ext[1] >> 16) & 0xffff
        {
            let b = (fb.ext[1] >> 32) & 0xff;
            let left = if state == 16 {
                0
            } else {
                timers.iter().find(|(_, t)| t.ext[1] == 0x2346).map_or(0, |(_, t)| t.ext[0])
            };
            let resume = if ret == 0x1D4B { 30 } else { 30 + 2 * (4 - b) };
            let n = (resume - 1 - left).max(0);
            let rbar = player_k
                .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
                .map_or(0, |b| b.ext[0] & 3);
            let left_tap = Self::tap_left(cx, rbar);
            let cust = bodies
                .iter()
                .filter(|(_, c)| c.kind == cust_k && Self::bar_of(cx, c.y.floor()) == rbar)
                .map(|(_, c)| *c)
                .reduce(|a, c| {
                    let further = if left_tap { c.x.floor() < a.x.floor() } else { c.x.floor() > a.x.floor() };
                    if further { c } else { a }
                });
            if let Some(c) = cust {
                let ry = c.y.floor();
                let rslot = (c.ext[3] >> 16) & 0xf;
                fb.ext[3] = (fb.ext[3] & !(0xffff_00ff | (0xffff << 24))) | 15 | (10 << 16) | (n << 24);
                fb.ext[2] = (fb.ext[2] & 0xffff) | ((ry & 0xff) << 16) | (rbar << 24) | (rslot << 32);
                fb.ext[1] &= !0xff_ffff_ff00;
                cx.set_body(fid, fb);
                flow = Some((fid, fb));
            }
        }
        // A start inside $51A9 (tapper.clear.004 starts in its wait at
        // $521D): $232B's place (E3A9) and B (E39F) give its frame, as for
        // $1CE1 above, counted as a level's end (sub 8) from $51A9's
        // first, 172; the draws before it are taken.
        if (state == 15 || state == 16)
            && let Some((fid, mut fb)) = flow
            && (fb.ext[3] >> 16) & 0xff != 8
            && let ret = (fb.ext[1] >> 16) & 0xffff
            && let Some(k) = (1..=5).find(|&k| wait_return(cx, "interlude_code", "interlude_start", k) == Some(ret))
        {
            let b = (fb.ext[1] >> 32) & 0xff;
            // The interlude's five waits, in order: a loop of 2 a step,
            // then 22, 24, 28, and a loop of 5 a step.
            let resume = match k {
                1 => Some(2 * (10 - b + 1)),
                2 => Some(22),
                3 => Some(24),
                4 => Some(28),
                _ => Some(28 + 5 * (10 - b + 1)),
            };
            if let Some(m) = resume {
                let left = timers.iter().find(|(_, t)| t.ext[1] == 0x2346).map_or(0, |(_, t)| t.ext[0]);
                let n = (172 + m - 1 - left).max(0);
                fb.ext[3] = (fb.ext[3] & 0xff00)
                    | state
                    | (8 << 16)
                    | (n << 24)
                    | END_TWICE_KNOWN
                    | END_INTERLUDE_KNOWN
                    | END_INTERLUDE_B
                    | END_SOUNDS;
                // The place is read once, at the start.
                fb.ext[1] &= !(0xffff << 16);
                cx.set_body(fid, fb);
                flow = Some((fid, fb));
            }
        }
        // State 11 outside a level's setup is a level's end ($2304): its
        // script ($55F4) waits through states 15 and 16 ($232B, $2346,
        // $231A), and the game's clear ($5DB8) has emptied the bars by the
        // first wait. In those waits (a level's end, a life lost) the slots
        // hold the script's pictures, not customers or mugs.
        let setting_up = flow.is_some_and(|(_, b)| b.ext[0] == 1 || b.ext[2] != 0);
        // A setup coming back in state 11 has no bartender at a bar yet
        // (tapper_flow places him); a level's end has him there.
        let bartender_y = player_k
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
            .map_or(0, |b| b.y.floor());
        // A level's end has emptied the timers ($5DEC at $0969); a setup
        // that waited for the next frame (the punk bar's $4942, called
        // through $4937 at $4C4B, waits in $5E99 at $60F6) is still in state 11 with
        // the bartender placed and the level's timers set: it reaches play
        // ($4C6D) on the next frame.
        let level_timers = timers.iter().any(|(_, t)| t.ext[1] == 0x2023 && t.ext[0] > 0);
        // A start just after end script 3's last hop: its wait ($232B,
        // the second in $50EF's code) returned to the hop loop with B at
        // 1, so $50EF returns, the script ends, and the setup runs on this
        // frame (tapper.clear.015: $570C's colours on its first frame,
        // $5404 on its second). The place (E3A9) and B (E39F) are read
        // once, as a clip starts.
        let hops_done = state == 11
            && flow.is_some_and(|(_, b)| {
                (b.ext[1] >> 16) & 0xffff != 0
                    && wait_return(cx, "hops_code", "hops_start", 2) == Some((b.ext[1] >> 16) & 0xffff)
                    && (b.ext[1] >> 32) & 0xff == 1
            });
        // A start in an end script's last wait when a bonus round comes
        // next ($5681 with the levels cleared plus one at 2, 5 or 9 mod
        // 13, or 0): script 1's last of $45E3's waits of 1 (B at 1), script
        // 2's $2DF7 wait of 15, script 3's last hop ($50EF's or $51A9's,
        // B at 1). In state 16 the wait has run out: the script returns on
        // this frame, $5681 counts the level and the bonus round ($2500)
        // starts, its frames counted from 0 here (tapper.clear.002,
        // clear.006). Back in state 11 it has returned already, the level
        // counted, and the round is on its frame 1 here: its first waits
        // ($5E99, $60F6) keep the state at 11 (tapper.clear.013, clear.018,
        // clear.021).
        let count = if state == 16 { cleared + 1 } else { cleared };
        let bonus_next = matches!(count % 13, 2 | 5 | 9) || count % 13 == 0;
        let last_wait = bonus_next
            && (state == 11 || state == 16)
            && flow.is_some_and(|(_, b)| {
                let (place, b_reg, sub) = ((b.ext[1] >> 16) & 0xffff, (b.ext[1] >> 32) & 0xff, (b.ext[3] >> 16) & 0xff);
                // The script by the levels cleared before this one.
                let script = Self::end_script(cx, count - 1);
                place != 0
                    && ![8, 10, 12].contains(&sub)
                    && (b.ext[1] >> 8) & 0xff == 11
                    && match script {
                        1 => b_reg == 1 && wait_return(cx, "end_gulp_code", "end_gulp_start", 1) == Some(place),
                        2 => wait_return(cx, "end_turn_code", "end_turn_start", 1) == Some(place),
                        3 => {
                            b_reg == 1
                                && (wait_return(cx, "hops_code", "hops_start", 2) == Some(place)
                                    || wait_return(cx, "interlude_code", "interlude_start", 5) == Some(place))
                        }
                        _ => false,
                    }
            });
        let hops_done = hops_done && !last_wait;
        let bonus_entered = last_wait && state == 16;
        if last_wait && let Some((fid, mut fb)) = flow {
            // As the script's return at $5681 below: a level more, and the
            // bonus round, sub 12, its frames from bit 24.
            cleared = count & 0xff;
            fb.ext[3] = (fb.ext[3] & 0xff00) | 15 | (12 << 16);
            fb.ext[2] &= !BONUS_MASK;
            fb.ext[1] &= !(0xffff << 16);
            cx.set_body(fid, fb);
            flow = Some((fid, fb));
            state = 15;
        }
        let ending = state == 11 && !setting_up && bartender_y != 0 && !level_timers && !hops_done;
        let finishing = state == 11 && !setting_up && bartender_y != 0 && level_timers;
        let mut end_heard = false;
        if ending {
            // $55FF: the level's points, once: a clip that starts inside
            // the end's script (its waits put the game back in state 11,
            // $231A) has had them, and its bar ends are gone ($5615 empties
            // them as the script starts, so none load).
            let ends_up = cx.kind("bar_end").is_some_and(|ek| cx.world.bodies.iter().any(|b| b.alive && b.kind == ek));
            // A start in the setup after the script (its last wait, $232B,
            // called in state 11, E3A8, with where it went on, E3A9, loaded
            // into the flow's word 1) has had them too: the bar ends are
            // back there ($4D13).
            let resumed = flow.is_some_and(|(_, b)| (b.ext[1] >> 8) & 0xff == 11 && (b.ext[1] >> 16) & 0xffff != 0);
            bar_ends_go(cx);
            if ends_up && !resumed {
                points += points_of(cx.table("points_level"));
                // $55F4: the level's end script asks for 2 as it starts.
                ask(cx, "sound_end_start", &[1]);
                end_heard = true;
            }
        }
        if (ending || finishing) && let Some((fid, mut fb)) = flow {
            fb.ext[3] = if ending {
                // The level's end script ($55F4, then the level's own, as
                // $4603) starts on this frame: its frames are counted from
                // 1 in the flow's word 3 (sub 8).
                (fb.ext[3] & 0xff00) | 15 | (8 << 16) | (1 << 24) | if end_heard { END_SOUNDS } else { 0 }
            } else {
                // A start in the setup's wait at $4C4B: its last two
                // requests come now ($4C4E, $4C5C; tapper.l9.001).
                let shown = (fb.ext[3] >> 8) & 3;
                ask(cx, "sound_game", &[0x11B, 0x13D + shown]);
                (fb.ext[3] & !0xff) | 10
            };
            cx.set_body(fid, fb);
        }
        // A mug fallen off a bar's end ($07A5, $081B) puts the game in state
        // 12, a life lost: $236E steps it by E6FF. The bars stand still.
        // 0 ($2389): the timers emptied ($5DEC) and the tip taken (E25E),
        // E706 13. 1 ($23F5): the fallen mug drops 2 a frame while E706
        // counts down. 2 ($241D): 12 more frames of its picture (E709).
        // 3 ($2441): state 9, the level's setup again ($22F7, $4B3F), whose
        // $5F31 empties the objects on the next frame.
        let mut end_spawn: Vec<Body> = Vec::new();
        // State 8, a game's start ($4AEF, then the setup $4B3F), is a
        // setup as after a life lost.
        // Not on the frame a setup places the bartender (the flow's word
        // 2 at 1 or 2 from tapper_flow, this hook's step above having
        // set state 10 for the frame after): a clip can start there, in
        // state 8 (tapper.l1.001).
        let game_start = state == 8 && flow.is_some_and(|(_, b)| b.ext[2] & 0xff == 0);
        let mut life_lost = state == 12 || state == 9 || game_start;
        let lost_sub = flow.map_or(0, |(_, b)| (b.ext[3] >> 16) & 0xff);
        // The lives (E4A7), in the flow's word 1 from bit 40.
        let lives = flow.map_or(1, |(_, b)| (b.ext[1] >> 40) & 0xff);
        // $2441 asks for 2 as it starts, lives left or not.
        if state == 12 && lost_sub == 3 {
            ask(cx, "sound_life", &[0x58]);
        }
        if demo && state == 12 && lost_sub == 3 && lives != 0 && let Some((fid, mut fb)) = flow {
            // $24A8: out of a game $24AE asks for 4 and the game goes to
            // state 4 ($22DC), the demo's end (tapper.poweron.001: E6FF 3
            // on 7626, state 4 on 7627).
            ask_ld(cx, "sound_life", &[0x24AF - 0x23EA]);
            fb.ext[3] = (fb.ext[3] & !0xff) | 4;
            cx.set_body(fid, fb);
        } else if state == 12 && lost_sub == 3 && lives == 0 && let Some((fid, mut fb)) = flow {
            // $2441 with no lives left ($244B): GAME OVER is drawn and
            // the game waits 60 frames ($2467) and 60 more ($24D8); unless
            // the bar shown is the sports bar, $24E2 calls $5E99, which
            // waits a frame, and $24EE puts the game in state 13, the game
            // over (tapper.l6.001: state 12 on 9009, 13 on 9131). Sub 11
            // counts its frames from bit 24.
            fb.ext[3] = (fb.ext[3] & !(0xffff_00ff | (0xffff << 24))) | 15 | (11 << 16) | (1 << 24);
            cx.set_body(fid, fb);
        } else if (state == 15 || state == 16 || state == 12) && lost_sub == 11 && let Some((fid, mut fb)) = flow {
            let c = ((fb.ext[3] >> 24) & 0xffff) + 1;
            let sports = (fb.ext[3] >> 8) & 3 == 1;
            // $246A: 2 after the first wait of 60.
            if c == 61 {
                ask(cx, "sound_life", &[0x81]);
            }
            if c == 121 && !sports {
                // $24E2's $5E99 empties the palette cycles.
                cx.set_counter("cycle_on", 0);
            }
            let st = if c == 122 || (c == 121 && sports) {
                13
            } else if c == 121 {
                12
            } else {
                state
            };
            // State 13's first frame: sub 1 (the objects stay one more
            // frame, then $2B82 empties them).
            let next_sub = if st == 13 { 1 } else { 11 };
            fb.ext[3] = (fb.ext[3] & !(0xffff_00ff | (0xffff << 24))) | st | (next_sub << 16) | ((c & 0xffff) << 24);
            cx.set_body(fid, fb);
        } else if state == 12 && let Some((fid, mut fb)) = flow {
            let sub = (fb.ext[3] >> 16) & 0xff;
            let mut e706 = (fb.ext[3] >> 24) & 0xff;
            let mut e709 = (fb.ext[2] >> 8) & 0xff;
            let mut next_sub = sub;
            match sub {
                0 => {
                    for (n, (_, t)) in timers.iter_mut().enumerate() {
                        t.ext[0] = 0;
                        t.ext[1] = 0;
                        timers_changed.push(n);
                    }
                    if let Some((Some(id), _)) = tip.take() {
                        tips_gone.push(id);
                    }
                    tip = None;
                    // $238C to $2398: the 9 objects from E23E, the dancers.
                    dancers = [None; 9];
                    e706 = 13;
                    next_sub = 1;
                    // $23B5 to $23DC: his hands to his head.
                    if let Some(pk) = player_k
                        && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.alive && b.kind == pk)
                    {
                        let mut pb = *pb;
                        lost_pose(cx, &mut pb, "lost_pose_fall", 0x23B5, (0x23BC, 0x23C0, 0x23C5));
                        cx.set_body(pid, pb);
                    }
                    // $23B0 to $23E6: the fallen mug's picture, $10
                    // empty, $14 full (bit 0 of its +4).
                    let one = |n: &str| cx.table(n).first().copied().unwrap_or(0);
                    let (empty_pic, full_pic) = (one("fall_picture_empty"), one("fall_picture_full"));
                    for (_, b) in bodies.iter_mut().filter(|(_, b)| b.kind == mug_k && b.ext[3] & 0x100 != 0) {
                        let pic = if b.ext[3] & 1 != 0 { empty_pic } else { full_pic };
                        b.ext[0] = (b.ext[0] & !0xff00) | (pic << 8);
                    }
                    // $23EA: the mug's fall asks for $1A (the
                    // `sound_life` rows are the program from $23EA).
                    ask(cx, "sound_life", &[0x01]);
                    // $23AF: a life less, a byte's (dec (hl): 0 less 1 is
                    // 255), counted on this frame.
                    let l = (((fb.ext[1] >> 40) & 0xff) - 1) & 0xff;
                    fb.ext[1] = (fb.ext[1] & !(0xff << 40)) | (l << 40);
                    lives_now = Some(l);
                }
                1 => {
                    e706 = (e706 - 1) & 0xff;
                    if e706 != 0 {
                        for (_, b) in bodies.iter_mut().filter(|(_, b)| b.kind == mug_k && b.ext[3] & 0x100 != 0) {
                            b.y = Fx::from_int(b.y.floor() - 2);
                        }
                    } else {
                        e706 = 1;
                        e709 = 12;
                        next_sub = 2;
                        // $2407: after its rise, picture $11.
                        let pic = cx.table("fall_picture_crash").first().copied().unwrap_or(0);
                        for (_, b) in bodies.iter_mut().filter(|(_, b)| b.kind == mug_k && b.ext[3] & 0x100 != 0) {
                            b.ext[0] = (b.ext[0] & !0xff00) | (pic << 8);
                        }
                        // $2402: its crash asks for $1B.
                        ask(cx, "sound_life", &[0x19]);
                    }
                }
                2 => {
                    e706 = 1;
                    e709 = (e709 - 1) & 0xff;
                    if e709 == 0 {
                        next_sub = 3;
                    } else {
                        // $242A: the crash, a picture a frame from the
                        // table at $24F4 (its +5 counts them, from 0 as
                        // E709 counts 11 down).
                        let k = usize::try_from(11 - e709).unwrap_or(0);
                        if let Some(&pic) = cx.table("fall_crash_pictures").get(k) {
                            for (_, b) in bodies.iter_mut().filter(|(_, b)| b.kind == mug_k && b.ext[3] & 0x100 != 0) {
                                b.ext[0] = (b.ext[0] & !0xff00) | (pic << 8);
                            }
                        }
                    }
                }
                3 if (fb.ext[3] >> 8) & 3 != 1 => {
                    // $24B9: unless the bar shown (E3B8) is the sports bar,
                    // $24C0 calls $5E99, which empties the list at E11A and
                    // waits for the next frame ($60F6, the main loop's own
                    // frame wait), so $24CC comes a frame later
                    // (tapper.l13.001: E6FF 3 on 19599, state 9 on 19601).
                    next_sub = 6;
                    cx.set_counter("cycle_on", 0);
                }
                _ => {
                    // $24CC: back to the level's setup (lives left; game
                    // over isn't modelled).
                    fb.ext[3] = (fb.ext[3] & !0xff) | 9;
                    next_sub = 3;
                }
            }
            fb.ext[3] = (fb.ext[3] & !0xffff_0000) | (next_sub << 16) | (e706 << 24);
            fb.ext[2] = (fb.ext[2] & !0xff00) | (e709 << 8);
            cx.set_body(fid, fb);
        }
        let sub = flow.map_or(0, |(_, b)| (b.ext[3] >> 16) & 0xff);
        if state == 9 && sub == 3 && let Some((fid, mut fb)) = flow {
            // The main loop reaches the setup ($22F7) a frame after $24D1.
            fb.ext[3] = (fb.ext[3] & !0x00ff_0000) | (4 << 16);
            cx.set_body(fid, fb);
            life_lost = true;
        } else if (state == 9 || hops_done || (game_start && fb_started(flow) == 0)) && let Some((fid, mut fb)) = flow {
            if hops_done {
                // The place is read once.
                fb.ext[1] &= !(0xffff << 16);
            }
            // $4B4D: the setup's $5F31 empties the objects, the bartender's
            // among them; its waits ($232B) are states 15 and 16. The
            // level's RAM is cleared ($4B65) after the bartender's walk on
            // ($5404: waits of 2, 30 and 5 frames, and 100 more before a
            // game's first level, $5575), so the setup's second half comes
            // that many frames on.
            for (id, _) in &bodies {
                removed.push(*id);
            }
            if let Some((Some(id), _)) = tip.take() {
                tips_gone.push(id);
            }
            tip = None;
            dancers = [None; 9];
            bar_ends_go(cx);
            // The taps go with the other objects, until the stage's $4D13
            // puts them back (tapper_flow).
            if let Some(tk) = cx.kind("tap") {
                let taps: Vec<(usize, Body)> =
                    cx.world.bodies.iter().enumerate().filter(|(_, b)| b.kind == tk).map(|(i, b)| (i, *b)).collect();
                for (id, mut t) in taps {
                    t.y = Fx::ZERO;
                    cx.set_body(id, t);
                }
            }
            if let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let mut pb = *pb;
                pb.y = Fx::ZERO;
                pb.x = Fx::ZERO;
                cx.set_body(pid, pb);
            }
            // His other objects with him.
            place_parts(cx, &[0, 1, 2, 3, 4], false, |_, px, _| (px, 0));
            let wait = 2 + 30 + 5 + if cleared == 0 { 100 } else { 0 };
            // $4B44's $3C48: the bar shown by the levels cleared, counted
            // in 13s ($3C55 to $3C76: 0 and 1 the saloon, 2 to 4 the sports
            // bar, 5 to 8 the punk bar, 9 to 12 the space bar). $4B4A's
            // $5DEC empties the timers. $4B50's $2EC3: the level's layout
            // from the layouts rows by the levels cleared, from 26 on
            // 13 to 25 again ($2EC6 to $2ED3), into E3AF.
            let in13 = cleared.rem_euclid(26) % 13;
            let shown = match in13 {
                0 | 1 => 0,
                2..=4 => 1,
                5..=8 => 2,
                _ => 3,
            };
            fb.ext[3] = (fb.ext[3] & !0xffff_ffff) | 15 | (shown << 8) | (5 << 16) | (wait << 24);
            // The setup's frame: the intro's first ($5404 runs on the next).
            cx.set_counter("intro", 1);
            let at = if cleared < 26 { cleared } else { in13 + 13 };
            let layouts = flow_table(cx, "layouts");
            if let Some(&l) = layouts.get(usize::try_from(at).unwrap_or(0)) {
                fb.ext[1] = (fb.ext[1] & !0xff) | l;
            }
            for (n, (_, t)) in timers.iter_mut().enumerate() {
                t.ext[0] = 0;
                t.ext[1] = 0;
                timers_changed.push(n);
            }
            cx.set_body(fid, fb);
            life_lost = true;
        } else if (state == 15 || state == 16) && sub == 5 && let Some((fid, mut fb)) = flow {
            let left = ((fb.ext[3] >> 24) & 0xff) - 1;
            // The level's intro ($5404, from the setup's next frame): the
            // screen cleared ($5FFD) and its lines written ($5F44), until
            // the stage is drawn; the counter intro is the frames since the
            // setup, plus 1, and 0 once the stage is up.
            let total = 2 + 30 + 5 + if cleared == 0 { 100 } else { 0 };
            let intro = if left > 0 { total - left + 1 } else { 0 };
            if cx.counter("intro") != Some(intro) {
                cx.set_counter("intro", intro);
            }
            // $5512, a game's first level: on the intro's frame 2 it copies
            // six sprites into the customer slots ($5567, from $55DC: y,
            // flags, picture, x each), and 100 frames on ($5575) $5F31
            // empties them.
            if cleared == 0
                && let Some(lk) = cx.kind("legend")
            {
                if intro == 2 {
                    let objs = cx.table("instr_objects").to_vec();
                    for o in objs.chunks(4).filter(|o| o.len() == 4 && o[0] != 0) {
                        let mut b = gp_sim::systems::new_body(cx.spec, cx.world, lk, Fx::from_int(o[3]), Fx::from_int(o[0]), false);
                        b.ext[1] = o[1];
                        b.ext[2] = o[2];
                        end_spawn.push(b);
                    }
                } else if intro == 102 || intro == 0 {
                    let ids: Vec<usize> =
                        cx.world.bodies.iter().enumerate().filter(|(_, b)| b.alive && b.kind == lk).map(|(i, _)| i).collect();
                    for id in ids {
                        cx.remove(id);
                    }
                }
            }
            // $5404's bartender at the tap above GET READY TO SERVE: 35
            // frames before the stage ($543D, after its wait of 2), the
            // pour's last picture 30 frames on ($5489), and $54DB's $5F31
            // empties him as the stage comes (tapper.l5.001: 5642, 5672
            // and 5677).
            if left == 35 || left == 5 || left <= 0 {
                let pose = match left {
                    35 => Some(false),
                    5 => Some(true),
                    _ => None,
                };
                intro_pose(cx, pose, &mut end_spawn);
            }
            if left <= 0 {
                // $4B6E's $5DF6 draws the level's stage, its screen's
                // place among the words at $61ED by the levels cleared.
                let stage = if cleared < 26 { cleared } else { cleared.rem_euclid(26) % 13 + 13 };
                cx.set_counter("stage", stage);
                cx.set_counter("bonus", 0);
                // The screen's copy ($5CED) draws the lives after it
                // ($5D21): the HUD shows them as of this load.
                cx.set_counter("lives_shown", (fb.ext[1] >> 40) & 0xff);
            }
            if left <= 0 && (fb.ext[3] >> 8) & 3 == 3 {
                // The setup clears the level's RAM on this frame ($4B65) and
                // waits for the next ($4B71). In the space bar (E3B8 3) $575B,
                // called at $4B8B, then draws and waits 30 frames ($232B)
                // and $5E99 waits one more ($60F6): the bartender comes 32
                // frames after the clear, not 1 (tapper.l13.001: E6F2 0 on
                // 19640, the bartender at his bar on 19672).
                fb.ext[3] = (fb.ext[3] & !0xffff_0000) | (7 << 16) | (31 << 24);
            } else if left <= 0 {
                // The setup clears the level's RAM on this frame: tapper_flow
                // places the bartender on the next, as at a level's start.
                fb.ext[3] &= !0xffff_0000;
                fb.ext[0] = 1;
            } else {
                fb.ext[3] = (fb.ext[3] & !0xff00_0000) | (left << 24);
            }
            cx.set_body(fid, fb);
        } else if (state == 15 || state == 16) && sub == 10 && let Some((fid, mut fb)) = flow {
            // $1CE1 goes on: after waits of 30 and 3 of 2 the bartender is
            // put where the customer stands ($1D8A: its y; $1DAC: 8 in
            // from his tap, or 12 out from a tap on the left end, $1DC3)
            // and his life is taken (E4A7, $1D71); 15 frames on he is 5
            // lower and 16 further along ($1E02, $1E2D); then a frame at
            // a time he slides 3 along ($1ED5 to $1EE7) while 3 more stays
            // on the bar ($1F60, the bar's end from the table at $0977, or
            // $1F6B on a left-tap bar), slides 3 on the same frame and 3
            // more a frame for 3 frames ($1F80), and leaves the bar (y 0,
            // $1FB9) on the next (tapper.l6.001 from 8846). Where the slide
            // ends, $1F75 copies the four objects of the program's table at
            // $2003 into the mug slots from E326 (the slide_objects rows,
            // lifted from the player's romset). After he leaves, $1FBD
            // moves his second sprite 4 frames and the customers' pictures
            // turn 10 times, 4 frames apart ($1FE6 to $2000); $1CE1 returns
            // 48 frames after the copy and the game goes on in state 12
            // with E6FF 3 ($1D66, $1D6B), $2441 on the next frame
            // (tapper.die.001: the copy on 1730, state 12 on 1778). The
            // flow's word 2 keeps the customer's y (bit 16), the bar (24),
            // the frames from the copy (26) and the customer's slot (32).
            let n = ((fb.ext[3] >> 24) & 0xff) + 1;
            let ry = (fb.ext[2] >> 16) & 0xff;
            let rbar = (fb.ext[2] >> 24) & 3;
            let mut phase = (fb.ext[2] >> 26) & 0x3f;
            let phase_before = phase;
            let rslot = (fb.ext[2] >> 32) & 0xf;
            let mut lives_word = fb.ext[1];
            if let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let mut pb = *pb;
                let tap = cx
                    .world
                    .bodies
                    .iter()
                    .filter(|b| Some(b.kind) == cx.kind("tap"))
                    .nth(usize::try_from(rbar).unwrap_or(0))
                    .map(|b| (b.x.floor(), b.facing_left));
                let (lo, hi) = Self::ends(cx, rbar);
                if let Some((tx, left)) = tap {
                    let step = if left { 3 } else { -3 };
                    let x = pb.x.floor();
                    // The customer who took him: 8 behind him from his
                    // second place ($1E34, $1E50), sliding with him in the
                    // loop ($1EE7, $1F04).
                    let cust = bodies
                        .iter()
                        .position(|(_, b)| b.kind == cust_k && (b.ext[3] >> 16) & 0xf == rslot);
                    let mut knock_at: Option<i64> = None;
                    // $1D50: after the wait of 30 his head goes and he
                    // turns, a picture each 2 frames from $28 (inc h).
                    if [30, 32, 34].contains(&n) {
                        let pic = imm(cx, "lost_pose_slide", 0x1D15, 0x1D51) + (n - 30) / 2;
                        pb.ext[2] = (pb.ext[2] & !(0xff << 24)) | ((pic & 0xff) << 24);
                        if n == 30 {
                            place_parts(cx, &[4], false, |_, px, _| (px, 0));
                        }
                    }
                    // $1D4B: $0D after the wait of 30; $1DF1: $18 as he
                    // is lowered (51).
                    if n == 30 {
                        ask(cx, "sound_lost", &[0x68]);
                    } else if n == 51 {
                        ask(cx, "sound_lost", &[0x10E]);
                    }
                    if n == 36 {
                        pb.y = Fx::from_int(ry);
                        pb.x = Fx::from_int(if left { tx + 12 } else { tx - 8 });
                        // $1D71: a life less (E4A7), counted on this frame
                        // (the HUD counters below read the flow as it was).
                        let l = (((lives_word >> 40) & 0xff) - 1) & 0xff;
                        lives_word = (lives_word & !(0xff << 40)) | (l << 40);
                        lives_now = Some(l);
                    } else if n == 51 {
                        pb.y = Fx::from_int(pb.y.floor() - 5);
                        let nx = x + if left { 16 } else { -16 };
                        pb.x = Fx::from_int(nx);
                        if let Some(k) = cust {
                            bodies[k].1.x = Fx::from_int(if left { nx - 8 } else { nx + 8 });
                        }
                        knock_at = Some(nx);
                    } else if n > 51 && phase == 0 {
                        let nx = (x + step) & 0xff;
                        let on = if left { nx - 24 < hi } else { nx - 3 >= lo };
                        pb.x = Fx::from_int(if on { nx } else { (nx + step) & 0xff });
                        if let Some(k) = cust {
                            bodies[k].1.x = Fx::from_int((bodies[k].1.x.floor() + step) & 0xff);
                        }
                        if on {
                            knock_at = Some(nx);
                        } else {
                            phase = 1;
                            // $1F75: the table at $2003, 8 bytes an
                            // object, over the mug slots 5 to 8 (E326 on);
                            // a mug there is overwritten.
                            let rows = cx.table("slide_objects").to_vec();
                            for k in 0..4 {
                                let Some(row) = rows.get(k * 8..k * 8 + 8) else { continue };
                                let index = 6 + k as i64;
                                let (oy, ox) = (Fx::from_int(row[0]), Fx::from_int(row[3]));
                                if let Some((_, m)) = bodies
                                    .iter_mut()
                                    .find(|(id, b)| b.kind == mug_k && b.ext[2] == index && !removed.contains(id))
                                {
                                    m.x = ox;
                                    m.y = oy;
                                    m.ext[3] = 0x1000;
                                } else {
                                    let mut m = gp_sim::systems::new_body(cx.spec, cx.world, mug_k, ox, oy, false);
                                    m.ext[2] = index;
                                    m.ext[3] = 0x1000;
                                    end_spawn.push(m);
                                }
                            }
                        }
                    } else if (1..4).contains(&phase) {
                        pb.x = Fx::from_int((x + step) & 0xff);
                        phase += 1;
                    } else if phase == 4 {
                        pb.y = Fx::ZERO;
                        phase = 5;
                    } else if phase >= 5 {
                        phase += 1;
                    }
                    // Each step of the slide ($1E68 to $1EBB) knocks over
                    // the mugs of his bar he has reached: 8 short of him
                    // ($1E97), or 16 past him on a left-tap bar ($1EA3);
                    // a knocked mug's picture is $11 and it drops 3.
                    let mut knocked = 0;
                    if let Some(bx) = knock_at {
                        for (_, m) in bodies.iter_mut().filter(|(_, b)| b.kind == mug_k) {
                            let mx = m.x.floor();
                            let hit = if left { mx <= bx + 16 } else { bx - 8 < mx };
                            if m.ext[3] & 0x200 == 0 && (m.ext[3] >> 4) & 0xf == rbar && m.y.floor() != 0 && hit {
                                m.ext[3] |= 0x200;
                                m.y = Fx::from_int(m.y.floor() - 3);
                                // $1EAD: each mug knocked over asks for $1B.
                                knocked += 1;
                            }
                        }
                    }
                    for _ in 0..knocked {
                        ask(cx, "sound_lost", &[0x1CA]);
                    }
                    // $1FE1: $19 after the copy's 4 frames and 4 more
                    // ($1F80, $1FB7).
                    if phase == 9 && phase_before == 8 {
                        ask(cx, "sound_lost", &[0x2FE]);
                    }
                    cx.set_body(pid, pb);
                    // His arm and head ($1D8A to $1DD8 as he is taken, 16
                    // up; from $1E02 his head 16 along at his height,
                    // sliding with him), and past the bar's end ($1FB9 to
                    // $1FDD) his head 3 a frame on for 4 frames, then gone.
                    let along = if left { -0x10 } else { 0x10 };
                    let (mx, my) = (pb.x.floor(), pb.y.floor());
                    if (36..51).contains(&n) {
                        place_parts(cx, &[0, 4], true, |k, _, _| (if k == 4 { mx + along } else { mx }, my - 0x10));
                    } else if n >= 51 && phase_before < 4 && my != 0 {
                        place_parts(cx, &[4], true, |_, _, _| (mx + along, my));
                    } else if (4..8).contains(&phase_before) {
                        place_parts(cx, &[4], false, |_, px, py| (px + step, py));
                    } else if phase_before == 8 {
                        place_parts(cx, &[4], false, |_, px, _| (px, 0));
                    }
                }
            }
            if n <= 255 {
                fb.ext[3] = (fb.ext[3] & !0xff00_0000) | (n << 24);
            }
            fb.ext[1] = lives_word;
            if phase >= 49 {
                // $1CE1 returns: state 12, E6FF 3; the slide's words go.
                fb.ext[3] = (fb.ext[3] & !(0xffff_00ff | (0xffff << 24))) | 12 | (3 << 16);
                fb.ext[2] &= 0xffff;
            } else {
                fb.ext[2] = (fb.ext[2] & !(0x3f << 26)) | (phase << 26);
            }
            cx.set_body(fid, fb);
        } else if (state == 15 || state == 16) && sub == 8 && let Some((fid, mut fb)) = flow {
            // The level's end script moves the bartender ($4603, the
            // script after the levels tapper.l1.001 to l3.001 end): it
            // waits 15 frames, then 4 of 10 ($232B), then $4742 calls
            // $1351, which puts him where he stands at his bar's tap (the
            // table at $C4A1 by his bar, E6F9), 56 frames from the end's
            // first. $4742 then waits 2, 6 of 2, 15 and 6 of 4, and back in
            // $4603 he steps 1 away from the tap ($46B4 to $46BF: +1, or -1
            // when the tap is on the bar's left end, E3AD) 53 frames on.
            // From the second level (E4A8 not 0, $4678) a draw from the
            // Z80's refresh register ($467E, bit 0 at $4680) on the frame
            // $4742 returns, 109 from the end's first, runs $4742 a second
            // time at 0 ($4684): his step away from the tap and all that
            // comes after it are 53 frames later. From bit 40 the flow's
            // word 3 keeps the draws taken in this end (END_*).
            // The count takes 16 bits from bit 24: the scripts run past 255
            // frames.
            let n = ((fb.ext[3] >> 24) & 0xffff) + 1;
            if n <= 0xffff {
                fb.ext[3] = (fb.ext[3] & !(0xffff << 24)) | (n << 24);
            }
            if n == 109 && fb.ext[3] & END_TWICE_KNOWN == 0 {
                fb.ext[3] |= END_TWICE_KNOWN;
                if cleared != 0 && cx.draw("end_twice") & 1 == 0 {
                    fb.ext[3] |= END_TWICE;
                }
            }
            // The script's frame as if $4742 ran once: past the second
            // run's start the frames of the second run come off, so the
            // second run's $1351 (109, as 56) puts him where he stands.
            let raw = n;
            let n = if n >= 109 && fb.ext[3] & END_TWICE != 0 { n - 53 } else { n };
            // After $4603 some lists go on ($5623: the list by E3ED, the
            // entry by the levels cleared mod 10, each a 5-byte step). The
            // last step, $5651, calls $48F5: after $4603 returns (15 and 4
            // more frames from the step, 128 from the end's first) $594D
            // lifts the object at E356, 6 above the bartender, 10 a time
            // every 2 frames until it is at least $EE, waits 20, 40 and 40,
            // and calls $4546, which puts an object in the mug slot at E326
            // at y 255 and his x, drops it 4 a frame until 5 above him
            // ($45A5), and empties it 60 frames later ($45BF to $45D7)
            // (tapper.l7.001: the end on 11860, the object on 12126).
            let words: Vec<i64> = cx
                .table("end_script_lists")
                .chunks(2)
                .map(|w| w[0] + 256 * w.get(1).copied().unwrap_or(0))
                .collect();
            let base = words.iter().copied().min().unwrap_or(0);
            let list = cx
                .kind("tap")
                .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
                .map_or(0, |b| b.ext[1] & 1);
            let script = words
                .get(usize::try_from(list * 10 + cleared % 10).unwrap_or(0))
                .map_or(0, |w| (w - base) / 5);
            if script == 4
                && n > 109
                && let Some(pk) = player_k
                && let Some(pb) = cx.world.bodies.iter().find(|b| b.kind == pk)
            {
                let (bx, by) = (pb.x.floor(), pb.y.floor());
                let y0 = by + 6;
                let k = (238 - y0 + 9).max(10) / 10;
                let t = 109 + 19 + 2 * k + 100;
                let slot5 = bodies.iter().position(|(id, b)| b.kind == mug_k && b.ext[2] == 6 && !removed.contains(id));
                if n == t {
                    if let Some(j) = slot5 {
                        let b = &mut bodies[j].1;
                        b.x = Fx::from_int(bx);
                        b.y = Fx::from_int(255);
                        b.ext[3] = 0x400;
                    } else {
                        let mut b = gp_sim::systems::new_body(cx.spec, cx.world, mug_k, Fx::from_int(bx), Fx::from_int(255), false);
                        b.ext[2] = 6;
                        b.ext[3] = 0x400;
                        end_spawn.push(b);
                    }
                } else if n > t
                    && let Some(j) = slot5
                    && bodies[j].1.ext[3] & 0x400 != 0
                {
                    let b = &mut bodies[j].1;
                    let y = b.y.floor();
                    if b.ext[3] & 0x800 == 0 {
                        let ny = y - 4;
                        b.y = Fx::from_int(ny);
                        if ny - 5 < by {
                            b.ext[3] |= 0x800;
                            // $45AD: it lands, asking for 2 and $37.
                            if fb.ext[3] & END_SOUNDS != 0 {
                                ask(cx, "sound_end", &[0x47, 0x4C]);
                            }
                            b.ext[1] = n;
                        }
                    } else if n == b.ext[1] + 60 {
                        removed.push(bodies[j].0);
                    }
                }
            }
            // Script 3 ($564C, $48D1): after $4603, $58C5 moves another
            // object for 22 waits of 2 and $4708 turns the bartender's
            // pictures; then in the space bar (E3B8 3, $48DA) $50EF: 10
            // waits of 2 for another object, then the bartender hops 10
            // times, 5 frames apart ($5160 to $519C): 5 up, then 5 down,
            // and along by the program's table at $519F (signed, the
            // hop_steps rows). 128 frames from the end's first $58C5
            // starts, so the hops are on frames 192 to 237
            // (tapper.l10.001: the first on 15874). In the other bars, on
            // the frame $58C5 returns (172), $2B58 looks for a mug on the
            // bartender's bar (E6F9) in the 9 slots from E2FE (y not 0,
            // its bar in the high nibble of +4) and runs $50EF if one is
            // there; else a draw from the Z80's refresh register ($48E7,
            // bit 0 at $48E9) picks $51A9 at 1 and $50EF at 0.
            let shown = (fb.ext[3] >> 8) & 3;
            if script == 3 && shown != 3 && n == 172 && fb.ext[3] & END_INTERLUDE_KNOWN == 0 {
                fb.ext[3] |= END_INTERLUDE_KNOWN;
                let his_bar = player_k
                    .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
                    .map_or(0, |b| b.ext[0]);
                let mug_there = bodies
                    .iter()
                    .any(|(id, b)| b.kind == mug_k && !removed.contains(id) && b.y.floor() != 0 && (b.ext[3] >> 4) & 0xf == his_bar);
                if !mug_there && cx.draw("end_interlude") & 1 != 0 {
                    fb.ext[3] |= END_INTERLUDE_B;
                }
            }
            let interlude_b = shown != 3 && fb.ext[3] & END_INTERLUDE_B != 0;
            // The script's requests by its frame as if $4742 ran once,
            // where the first run's own last frame (109) is its own.
            let h = if raw <= 109 { raw } else { n };
            if fb.ext[3] & END_SOUNDS != 0 {
                Self::end_sounds(cx, h, script, interlude_b);
            }
            // $51A9, from 172: 10 waits of 2 for the other object (E356),
            // then the bartender's pictures turn; 2 frames on ($5220) he
            // is 5 higher and 5 along, away from the tap (E3AD); 2 more
            // ($5249) he is where that object is, 16 above it, which is 9
            // lower than before and 14 further along; 4 more ($5263) he
            // hops 10 times, 5 frames apart: 5 up, then 5 down, and along
            // by the hop_steps rows ($5286), as in $50EF
            // (tapper.clear.004: $51A9 on 3286, the hops from 3314).
            if script == 3
                && interlude_b
                && let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let m = n - 172;
                let left = Self::tap_left(cx, pb.ext[0]);
                let away = if left { 1 } else { -1 };
                let (dx, dy) = if m == 22 {
                    (5 * away, 5)
                } else if m == 24 {
                    (14 * away, -9)
                } else if m >= 28 && (m - 28) % 5 == 0 && (m - 28) / 5 < 10 {
                    let k = (m - 28) / 5;
                    let steps = cx.table("hop_steps").to_vec();
                    let dx = steps.get(usize::try_from(k).unwrap_or(0)).map_or(0, |&v| if v >= 0x80 { v - 0x100 } else { v });
                    (dx, if k % 2 == 0 { 5 } else { -5 })
                } else {
                    (0, 0)
                };
                if (dx, dy) != (0, 0) {
                    let mut pb = *pb;
                    pb.x = Fx::from_int((pb.x.floor() + dx) & 0xff);
                    pb.y = Fx::from_int(pb.y.floor() + dy);
                    cx.set_body(pid, pb);
                    // $5228, $5255, $5279: his head 16 above him.
                    let (hx, hy) = (pb.x.floor(), pb.y.floor() - 0x10);
                    place_parts(cx, &[4], false, |_, _, _| (hx, hy));
                }
            }
            if script == 3
                && !interlude_b
                && n >= 192
                && (n - 192) % 5 == 0
                && (n - 192) / 5 < 10
                && let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let k = (n - 192) / 5;
                let steps = cx.table("hop_steps").to_vec();
                let dx = steps.get(usize::try_from(k).unwrap_or(0)).map_or(0, |&v| if v >= 0x80 { v - 0x100 } else { v });
                let dy = if k % 2 == 0 { 5 } else { -5 };
                let mut pb = *pb;
                pb.x = Fx::from_int((pb.x.floor() + dx) & 0xff);
                pb.y = Fx::from_int(pb.y.floor() + dy);
                cx.set_body(pid, pb);
                // $5176, $5191: his head 16 above him.
                let (hx, hy) = (pb.x.floor(), pb.y.floor() - 0x10);
                place_parts(cx, &[4], false, |_, _, _| (hx, hy));
            }
            // The script's step returns to $5656 (the game back in state
            // 11, $231A): script 1 ($489A) after $5904's 21 waits of 2 and
            // $45E3's 12 of 1, 182 from the end's first; script 3 after
            // the interlude, 242 ($50EF) or 250 ($51A9). In the punk bar
            // and the space bar $5E99 waits a frame ($5663). Then E4A8
            // counts the level ($5681), and when the count mod 13 ($5F6C)
            // is 2, 5, 9, or 0 past the first, $2500 starts the bonus round
            // and its $5F31 empties every object, the bartender's too
            // (tapper.l9.001, l13.001, l15.001: their last frame). Script
            // 0 ($563D) returns 219 from the end's first and script 2
            // ($5647) 185 (tapper-cycle: the levels cleared 0 and 10, and
            // 1, 7 and 11 with $4742 twice); script 4 when $4546's object
            // goes, 60 frames after it lands (tapper-cycle: levels cleared
            // 3, 6 and 13).
            let by = player_k
                .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
                .map_or(0, |b| b.y.floor());
            let done = match script {
                0 => Some(219),
                1 => Some(182),
                2 => Some(185),
                3 if interlude_b => Some(250),
                3 => Some(242),
                4 => {
                    let k = (238 - (by + 6) + 9).max(10) / 10;
                    Some(109 + 19 + 2 * k + 100 + (250 - by).div_euclid(4) + 1 + 60)
                }
                _ => None,
            };
            let count = cleared + 1;
            let bonus = matches!(count % 13, 2 | 5 | 9) || count % 13 == 0;
            let returns = done.is_some_and(|d| n == d + i64::from(shown == 2 || shown == 3));
            if returns {
                // $5681: a level more. Without a bonus round $56A0's $2EA1:
                // from 6 levels cleared, on an even count, the level row
                // (E4A9) one more, at most 9 (E70C, $4E4B). Then $2304
                // goes on to the setup ($22F7, $4B3F), whose $5E99 waits
                // this frame: state 8 here, as a game's start, so the
                // setup runs from the next.
                cleared = count & 0xff;
                if !bonus {
                    if count >= 6 && count % 2 == 0 {
                        level = (level + 1).min(9);
                    }
                    fb.ext[3] = (fb.ext[3] & 0xff00) | 8;
                } else {
                    // $2500, the bonus round (sub 12, bonus_round counts
                    // it from bit 24).
                    fb.ext[3] = (fb.ext[3] & 0xff00) | 15 | (12 << 16);
                    fb.ext[2] &= !BONUS_MASK;
                    // $2500's $5F31 empties the objects, the taps among
                    // them, until the next setup puts them back.
                    if let Some(tk) = cx.kind("tap") {
                        let taps: Vec<(usize, Body)> =
                            cx.world.bodies.iter().enumerate().filter(|(_, b)| b.kind == tk).map(|(i, b)| (i, *b)).collect();
                        for (id, mut t) in taps {
                            t.y = Fx::ZERO;
                            cx.set_body(id, t);
                        }
                    }
                }
            }
            if bonus && returns {
                for (id, _) in &bodies {
                    removed.push(*id);
                }
                if let Some(pk) = player_k
                    && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
                {
                    let mut pb = *pb;
                    pb.x = Fx::ZERO;
                    pb.y = Fx::ZERO;
                    cx.set_body(pid, pb);
                }
            }
            if (n == 56 || n == 109)
                && let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let mut pb = *pb;
                let bar = pb.ext[0];
                let tap = cx
                    .world
                    .bodies
                    .iter()
                    .filter(|b| Some(b.kind) == cx.kind("tap"))
                    .nth(usize::try_from(bar).unwrap_or(0))
                    .map(|b| (b.x.floor(), b.facing_left));
                let param = |name: &str| cx.spec.kinds[pk].ext_params.get(name).map_or(16, |v| v.floor());
                if let Some((tx, left)) = tap {
                    let x = if n == 56 {
                        if left { tx + param("stand_left") } else { tx - param("stand_right") }
                    } else {
                        pb.x.floor() + if left { -1 } else { 1 }
                    };
                    let dx = x - pb.x.floor();
                    pb.x = Fx::from_int(x);
                    cx.set_body(pid, pb);
                    // $1351 put his arms and head on him where he stands
                    // at the tap.
                    if n == 56 {
                        place_parts(cx, &[0, 1, 4], false, |_, px, py| (px + dx, py));
                    }
                }
            }
            if raw == 109 && fb.ext[3] & END_TWICE != 0 {
                // $4684 runs $4742 again on this frame, after his hook took
                // the first run's next step ($4603's at 109): its first
                // frame is drawn here, as end_script_pose's 56 (at the tap,
                // the tap hidden, its handle down beside him), so the glass
                // he drank is in his hand on the original's frame.
                let stand = |cx: &ExtCx<'_>, name: &str| {
                    cx.kind("player").and_then(|pk| cx.spec.kinds[pk].ext_params.get(name)).map_or(16, |v| v.floor())
                };
                bartender_objects(cx, |cx, me, parts, tap_y, (tap_x, left)| {
                    me.x = if left { tap_x + stand(cx, "stand_left") } else { tap_x - stand(cx, "stand_right") };
                    end_script_pose(cx, me, parts, tap_y, (tap_x, left), (56, 0, false, 0, 0));
                });
            }
            cx.set_body(fid, fb);
        } else if (state == 15 || state == 16) && sub == 12 && !bonus_entered && let Some((fid, mut fb)) = flow {
            let next = bonus_round(cx, &mut fb, cleared, &mut points);
            bonus_objs = Some(bonus_scene(cx, &fb, cleared, next));
            if next {
                // $2500 returns to $569D: $56A0's $2EA1, then the setup
                // ($2304, $22F7), state 8 as at a level's end.
                if cleared >= 6 && cleared % 2 == 0 {
                    level = (level + 1).min(9);
                }
                fb.ext[3] = (fb.ext[3] & 0xff00) | 8;
                fb.ext[2] &= !BONUS_MASK;
            }
            cx.set_body(fid, fb);
        } else if (state == 15 || state == 16) && sub == 7 && let Some((fid, mut fb)) = flow {
            let left = ((fb.ext[3] >> 24) & 0xff) - 1;
            if left <= 0 {
                fb.ext[3] &= !0xffff_0000;
                fb.ext[0] = 1;
            } else {
                fb.ext[3] = (fb.ext[3] & !0xff00_0000) | (left << 24);
            }
            cx.set_body(fid, fb);
        }
        // The attract mode's demo game (tapper.poweron.001, from 7259).
        // $3CD8 sets state 2 (tapper_flow); on the next frame $5A64 empties
        // the objects ($5F31), asks for 3 and sets the levels cleared, E3C4
        // and the level row as its code has them, then state 3; the setup
        // ($4B3F) sets the bar shown, 4 out of a game ($3C48), and waits for
        // the next frame ($5E99). There, with no intro out of a game ($4B62),
        // it empties the timers ($5DEC), takes the level's layout ($2EC3),
        // clears the level's RAM ($4B65) and draws the stage ($5DF6), and
        // tapper_flow puts the taps and the bartender in on the next frame
        // as at a level's start; the play ($59DD, $0660) runs from the frame
        // after that. The model keeps the setup's two frames in state 15
        // (sub DEMO_SETUP), the original's E397 being 3 already.
        let mut demo_still = false;
        if demo && let Some((fid, mut fb)) = flow {
            let sub = (fb.ext[3] >> 16) & 0xff;
            let count = (fb.ext[3] >> 24) & 0xffff;
            let empties = state == 2 || (state == 4 && sub != DEMO_SCORES);
            if empties {
                // $5F31 (from $5A64, and from $4A37's $3CE1): every object.
                for (id, _) in &bodies {
                    removed.push(*id);
                }
                if let Some((Some(id), _)) = tip.take() {
                    tips_gone.push(id);
                }
                tip = None;
                dancers = [None; 9];
                bar_ends_go(cx);
                if let Some(tk) = cx.kind("tap") {
                    let taps: Vec<(usize, Body)> =
                        cx.world.bodies.iter().enumerate().filter(|(_, b)| b.kind == tk).map(|(i, b)| (i, *b)).collect();
                    for (id, mut t) in taps {
                        t.y = Fx::ZERO;
                        cx.set_body(id, t);
                    }
                }
                if let Some(pk) = player_k
                    && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
                {
                    let mut pb = *pb;
                    pb.y = Fx::ZERO;
                    pb.x = Fx::ZERO;
                    cx.set_body(pid, pb);
                }
                place_parts(cx, &[0, 1, 2, 3, 4], false, |_, px, _| (px, 0));
            }
            if state == 2 {
                const S: i64 = 0x5A64;
                ask_ld(cx, "demo_setup", &[0x5A68 - S]);
                cleared = imm(cx, "demo_setup", S, 0x5A6D);
                cx.set_counter("demo_count", imm(cx, "demo_setup", S, 0x5A72));
                level = imm(cx, "demo_setup", S, 0x5A77);
                fb.ext[3] = 15 | (4 << 8) | (DEMO_SETUP << 16) | (1 << 24);
                // The logo's time and the wait's registers go; the lives
                // stay as the last game left them.
                fb.ext[2] = 0;
                fb.ext[1] &= !0xff_ffff_ffff;
                fb.ext[0] = 0;
                cx.set_body(fid, fb);
                demo_still = true;
            } else if state == 15 && sub == DEMO_SETUP && count == 1 {
                for (n, (_, t)) in timers.iter_mut().enumerate() {
                    t.ext[0] = 0;
                    t.ext[1] = 0;
                    timers_changed.push(n);
                }
                let layouts = flow_table(cx, "layouts");
                if let Some(&l) = layouts.get(usize::try_from(cleared).unwrap_or(0)) {
                    fb.ext[1] = (fb.ext[1] & !0xff) | l;
                }
                cx.set_counter("stage", cleared);
                cx.set_counter("bonus", 0);
                cx.set_counter("lives_shown", (fb.ext[1] >> 40) & 0xff);
                // The clear: tapper_flow places the taps and him next.
                fb.ext[0] = 1;
                fb.ext[3] = (fb.ext[3] & 0xff00) | 15 | (DEMO_SETUP << 16) | (2 << 24);
                cx.set_body(fid, fb);
                demo_still = true;
            } else if state == 4 && sub != DEMO_SCORES {
                // $22DC: with credits $4AAA, the demo screen again (as
                // $22B8 with credits); else $4A37, the high scores (not
                // drawn by the model) for a wait of $96 frames ($232B).
                let credits = cx.counter("credits").unwrap_or(0);
                if credits > 0 {
                    let at = attract_at(cx, "attract_demo");
                    fb.ext[3] = (fb.ext[3] & !0xff) | 6;
                    fb.ext[2] = (at << ATTRACT_SHIFT) & ATTRACT_MASK;
                    fb.ext[0] = 0;
                    cx.set_counter("demo", 0);
                } else {
                    fb.ext[3] = (fb.ext[3] & 0xff00) | 15 | (DEMO_SCORES << 16) | (1 << 24);
                }
                cx.set_body(fid, fb);
                demo_still = true;
            } else if (state == 15 || state == 16) && sub == DEMO_SCORES {
                const H: i64 = 0x4A37;
                let wait = imm(cx, "high_scores", H, 0x4A4E) | (imm(cx, "high_scores", H, 0x4A4F) << 8);
                let c = count + 1;
                if c > wait {
                    // $4A53: state 0, whose $22B8 starts the logo again.
                    fb.ext[3] &= 0xff00;
                    cx.set_counter("demo", 0);
                } else {
                    fb.ext[3] = (fb.ext[3] & !(0xffff << 24)) | (c << 24);
                }
                cx.set_body(fid, fb);
                demo_still = true;
            } else if state == 3 {
                // $59DD, before $0660 on every frame of the demo's play:
                // the empty mug's speed E3B0 and the respawn wait E3B1.
                const D: i64 = 0x59DD;
                empty_speed = imm(cx, "demo_input", D, 0x59DE);
                w_word = imm(cx, "demo_input", D, 0x59E3) | (imm(cx, "demo_input", D, 0x59E4) << 8);
            }
        }
        let life_lost = life_lost || demo_still;
        let playing = !ending && !life_lost && ![13, 15, 16].contains(&state) && !(demo && state != 3);
        if state == 13
            && let Some((fid, mut fb)) = flow
            && (fb.ext[3] >> 16) & 0xff == 1
        {
            // The count from bit 24 starts again for the game over's wait.
            fb.ext[3] = (fb.ext[3] & !(0x00ff_0000 | (0xffff << 24))) | (2 << 16);
            // $230F's $49E5 asks for 2 and 3.
            ask(cx, "sound_over", &[1, 6]);
            cx.set_body(fid, fb);
        } else if state == 13
            && let Some((fid, mut fb)) = flow
            && (fb.ext[3] >> 16) & 0xff == 2
        {
            // $2B82's $5E99 empties the palette cycles.
            cx.set_counter("cycle_on", 0);
            // $230F: $49E5 and its $2B82, which waits a frame ($5E99),
            // puts the game in its game over (E3BD 2, $2B90, on the frame
            // after the objects go) and, the score having a place in the
            // high scores (a fresh board's are all 0), waits for the
            // initials 1200 frames ($2CA7, the timer for $2D59, with a
            // wait of 1 each frame at $2CF7) after two frames of its own
            // ($2C76, $2CA2); then state 0 ($4A27), whose $22B8 starts
            // the attract mode. The initials aren't modelled: a stick or
            // button in them is not seen.
            let c = ((fb.ext[3] >> 24) & 0xffff) + 1;
            if c == 1 {
                cx.set_counter("mode", 2);
            }
            let wait = cx
                .kind("flow")
                .and_then(|k| cx.spec.kinds[k].ext_params.get("game_over_wait"))
                .map_or(1203, |v| v.floor());
            fb.ext[3] = if c >= wait {
                fb.ext[3] & !(0xffff_00ff | (0xffff << 24))
            } else {
                (fb.ext[3] & !(0xffff << 24)) | ((c & 0xffff) << 24)
            };
            cx.set_body(fid, fb);
        }
        if !playing && !life_lost {
            // A level's end ($55F4) empties the tip (E25E) and the
            // customers have all gone, but the mugs' slots (E2FE on) keep
            // what they hold, standing still, until the next setup empties
            // the objects ($5F31) (tapper.l5.001: two full mugs at E306
            // and E30E through the end's waits).
            for (id, b) in &bodies {
                // Only in a level's end the model saw begin (sub 9 on its
                // state 11 frame, then 8): a state 11 a clip starts in can
                // be the next level's setup, which empties them.
                let end_sub = flow.map_or(0, |(_, f)| (f.ext[3] >> 16) & 0xff);
                let in_end = (state == 11 && end_sub == 9) || ([15, 16].contains(&state) && end_sub == 8);
                // $1CE1 leaves the customers and mugs where they are, and
                // so do $2441's waits before a game over (sub 11) and the
                // game over's first frame (state 13, sub 1): $230F's
                // $2B82 empties the objects on its second (tapper.l6.001:
                // state 13 on 9131, the objects gone on 9133).
                let reached = ([15, 16].contains(&state) && (end_sub == 10 || end_sub == 11)) || (state == 13 && end_sub == 1);
                if !reached && (b.kind != mug_k || !in_end) {
                    removed.push(*id);
                }
            }
            if let Some((Some(id), _)) = tip.take() {
                tips_gone.push(id);
            }
            // The level's end empties the 9 objects from E23E with the tip
            // ($5604), $1CE1 and $2389 do too, and the setup's $5F31 every
            // object: the dancers are gone, and the show stands still
            // until the setup clears it with the level's RAM ($4B65).
            dancers = [None; 9];
        }
        let mut throws: Vec<Body> = Vec::new();
        if playing {
        // $0B63: the bartender catches an empty mug within 15 each way.
        if let Some(pk) = player_k
            && let Some(pb) = cx.world.bodies.iter().find(|b| b.kind == pk)
        {
            let (px, py) = (pb.x.floor(), pb.y.floor());
            let reach = bp(cx, "hand_reach");
            // The mugs in slot order (E2FE on), then the tip. After a catch
            // the points call ($0BF2, $5EE7) leaves DE, which held his y and
            // x ($0B6D, $0B6E), at $073C, so no later object that frame is
            // within reach: one catch a frame, the tip after none
            // (catch_once; tapper.l7.001: two empty mugs in reach on
            // 11224, the second caught on 11225).
            let once = bp(cx, "catch_once") != 0;
            let mut mugs: Vec<&(usize, Body)> = bodies.iter().filter(|(_, b)| b.kind == mug_k).collect();
            mugs.sort_by_key(|(_, b)| if b.ext[2] == 0 { 99 } else { b.ext[2] });
            let mut caught = false;
            for (id, b) in mugs {
                if caught && once {
                    break;
                }
                let (x, y) = (b.x.floor(), b.y.floor());
                // $0BDC: an empty mug is one whose flags (+4) have bit 0.
                let empty = b.ext[3] & 1 != 0;
                if empty && (y - py).abs() < reach && (x - px).abs() < reach {
                    removed.push(*id);
                    caught = true;
                    points += points_of(cx.table("points_catch"));
                    // $0BED: an empty mug caught asks for $17 (the
                    // `sound_catch` rows are the program from $0BCF).
                    ask_ld(cx, "sound_catch", &[0x1F]);
                }
            }
            // $0B86 to $0BD4, after the mugs: a tip (picture $4D at
            // E260) within reach is picked up and E72F, the tips left,
            // goes down by one. Its points ($5EE7 with DE $0C07) aren't
            // modelled: nothing in the model scores yet.
            if let Some((tid, t)) = tip
                && !(caught && once)
                && (t.y.floor() - py).abs() < reach
                && (t.x.floor() - px).abs() < reach
            {
                if let Some(id) = tid {
                    tips_gone.push(id);
                }
                tip = None;
                tips_left = (tips_left - 1) & 0xff;
                points += points_of(cx.table("points_tip"));
                // $0BCF asks for $13, then the show ($14E7) for 2 and $2E;
                // rbtapper's show asks for all three, its tip for none.
                ask_ld(cx, "sound_catch", &[0x01]);
                ask_run(cx, "sound_bar", 0x289);
                // $0BD8 calls the show, $14E7: the dancers' first picture
                // ($14F1 to $1515, see show_picture) and eight draws at $1523,
                // each 0 to 2 ($5FAC returns at most its A), that step
                // through the customer slots from E262 and E4B2 ($1526 to
                // $1538: 8 bytes and 32 a step). A customer in the slot
                // reached that isn't pushed back, drinking or about to
                // throw (states 1, 2 and 3, $153A to $1547) turns to watch:
                // state 6, its timer 255 ($154F, $1553). A slot reached
                // twice is set again; a slot with no customer is set too,
                // and its arrival writes state 0 ($1B66).
                let mut slot = 0;
                for _ in 0..8 {
                    slot += draw_at_most(cx, "tip_show_pick", 2);
                    if let Some(j) = bodies.iter().position(|(id, b)| {
                        b.kind == cust_k && !removed.contains(id) && (b.ext[3] >> 16) & 0xff == slot
                    }) {
                        let c = &mut bodies[j].1;
                        if !matches!(c.ext[0] & 0xff, 1..=3) {
                            c.ext[0] = (c.ext[0] & !0xff) | 6;
                            c.ext[1] = 0xFF;
                        }
                    }
                }
                // $14F1 to $1515: the show on for 255 frames, the
                // dancers' next picture in 6, and their first picture,
                // the bar shown's, over the 9 objects from E23E.
                show = Show { on: true, left: 0xFF, tick: 6, picture: show.picture };
                let shown = flow.map_or(0, |(_, b)| (b.ext[3] >> 8) & 3);
                if let Some(dk) = dancer_k
                    && let Some(pic) = show_picture(cx, shown, None)
                {
                    Self::put_picture(cx, dk, &mut dancers, &pic);
                    Self::tip_from_dancers(cx, tip_k, &dancers, &mut tip, &mut tips_gone);
                }
            }
        }

        // $0742: each mug moves, in slot order; a full one is caught by the
        // first customer of its bar standing within reach ($0827 to $0871).
        // A mug past its bar's end falls: an empty one past the bartender's
        // end ($0790 to $07A2: 19 in from the left end moving left, 10 out
        // from the right end moving right), a full one past the far end
        // ($0804 to $0818: at the left end moving left, 28 out from the
        // right end moving right). The game is then in state 12 and the
        // mugs' move stops there ($07AE, $0824).
        let mut order: Vec<usize> = (0..bodies.len()).filter(|&k| bodies[k].1.kind == mug_k).collect();
        order.sort_by_key(|&k| if bodies[k].1.ext[2] == 0 { 99 } else { bodies[k].1.ext[2] });
        let mut fell = false;
        for k in order {
            let (id, mut b) = bodies[k];
            if b.kind != mug_k || removed.contains(&id) {
                continue;
            }
            // $074F to $0766: the mug's flags (+4): bit 0 empty, bit 3
            // moving left, its bar above.
            let bar = (b.ext[3] >> 4) & 3;
            let left_tap = b.ext[3] & 8 != 0;
            let empty = b.ext[3] & 1 != 0;
            if !empty {
                let left = ((b.ext[1] >> 8) - 1) & 0xff;
                let left = if left == 0 {
                    let one = |n: &str| cx.table(n).first().copied().unwrap_or(0);
                    let full = one("mug_picture_full");
                    let pic = (b.ext[0] >> 8) & 0xff;
                    let next = if pic == full { one("mug_picture_slosh") } else { full };
                    b.ext[0] = (b.ext[0] & 0xff) | (next << 8);
                    one("mug_flip_frames")
                } else {
                    left
                };
                b.ext[1] = (b.ext[1] & 0xff) | (left << 8);
            }
            let x16 = b.x.floor() * 256 + (b.ext[1] & 0xff);
            let speed = if empty { empty_speed * 256 } else { full_speed };
            let x16 = if left_tap { x16 - speed } else { x16 + speed };
            b.x = Fx::from_int((x16 >> 8) & 0xff);
            b.ext[1] = (b.ext[1] & !0xff) | (x16 & 0xff);
            let h = b.x.floor();
            let (lo, hi) = Self::ends(cx, bar);
            let falls = match (empty, left_tap) {
                (true, true) => (lo + 0x13) & 0xff >= h,
                (true, false) => h > (hi + 0x0A) & 0xff,
                (false, true) => h > (hi + 0x1C) & 0xff,
                (false, false) => lo >= h,
            };
            if falls {
                b.ext[3] |= 0x100;
                bodies[k].1 = b;
                fell = true;
                break;
            }
            bodies[k].1 = b;
            if empty {
                continue;
            }
            let mx = b.x.floor();
            // $0827: the customers' slots in order from E262, so of two
            // customers in reach the lower slot takes the mug
            // (tapper.l8.001, frame 12731: slot 8 at x 179 before slot 11
            // at 182).
            let mut by_slot: Vec<usize> = (0..bodies.len()).filter(|&j| bodies[j].1.kind == cust_k).collect();
            by_slot.sort_by_key(|&j| (bodies[j].1.ext[3] >> 16) & 0xff);
            for j in by_slot {
                let (cid, mut c) = bodies[j];
                if c.kind != cust_k || removed.contains(&cid) {
                    continue;
                }
                if Self::bar_of(cx, c.y.floor()) != bar {
                    continue;
                }
                let d = (c.x.floor() + 8 - mx).rem_euclid(256);
                if d >= bp(cx, "catch_reach") {
                    continue;
                }
                let state = c.ext[0] & 0xff;
                if ![5, 3, 4, 0].contains(&state) {
                    continue;
                }
                // $09C7: pushed back, for 14 frames plus a draw of 0 to 20.
                let r = draw_at_most(cx, "customer_wait_start", bp(cx, "pushed_spread"));
                c.ext[0] = (c.ext[0] & !0xff) | 1;
                c.ext[1] = bp(cx, "pushed_base") + r;
                // $09E5: pushed back, the record's +14 picture.
                let slot = (c.ext[3] >> 16) & 0xff;
                let block = record_block(cx, flow.map_or(0, |(_, b)| (b.ext[3] >> 8) & 0xff));
                if let Some(p) = usize::try_from(slot).ok().and_then(|k| block.get(k * 32 + 0x14).copied()) {
                    c.ext[0] = (c.ext[0] & !(0xff << 24)) | (p << 24);
                }
                // $09ED to $0A1F: the mug it caught, its second sprite:
                // the record's +19 picture, 4 above it ($DE's level with
                // it, its picture set +10), 16 toward the tap.
                if let Some(base) = usize::try_from(slot).ok().map(|k| k * 32) {
                    let rows = &block;
                    let pic2 = rows.get(base + 0x19).copied().unwrap_or(0);
                    let tall = rows.get(base + 0x10).copied() == Some(0xDE);
                    let y2 = (c.y.floor() - if tall { 0 } else { 4 }) & 0xff;
                    c.ext[2] = (c.ext[2] & 0xffff) | (pic2 << 16) | (y2 << 24);
                }
                bodies[j].1 = c;
                removed.push(id);
                break;
            }
        }

        // $0B43: each customer's state, in slot order from E262 with its
        // record from E4B2. Its wait and drink are its slot's record's
        // (E4B4, E4BB), which the speed-up changes.
        let mut cust_order: Vec<usize> = (0..bodies.len()).filter(|&k| bodies[k].1.kind == cust_k).collect();
        cust_order.sort_by_key(|&k| (bodies[k].1.ext[3] >> 16) & 0xff);
        // The bar shown's records (record_blocks), whose bytes give a
        // customer's pictures as its state changes ($1016 to $11EA).
        let blocks = record_block(cx, flow.map_or(0, |(_, b)| (b.ext[3] >> 8) & 0xff));
        let block_of = |slot: i64| -> Option<&[i64]> {
            let k = usize::try_from(slot).ok()?;
            blocks.get(k * 32..(k + 1) * 32)
        };
        for k in cust_order {
            let (id, mut c) = bodies[k];
            if c.kind != cust_k || removed.contains(&id) {
                continue;
            }
            let slot = (c.ext[3] >> 16) & 0xff;
            let rec_bar = rec_of(&recs, slot).map_or_else(|| Self::bar_of(cx, c.y.floor()), |n| recs[n].1.ext[1] & 0xff);
            let (wait, drink) = match rec_of(&recs, slot) {
                Some(n) => (recs[n].1.ext[0] & 0xff, (recs[n].1.ext[0] >> 8) & 0xff),
                None => (c.ext[2] & 0xff, (c.ext[2] >> 8) & 0xff),
            };
            let state = c.ext[0] & 0xff;
            let facing = (c.ext[0] >> 8) & 1;
            let t = (c.ext[1] - 1) & 0xff;
            c.ext[1] = t;
            match state {
                0 => {
                    if t == 0 {
                        c.ext[0] = (c.ext[0] & !0xff) | 5;
                        c.ext[1] = bp(cx, "walk_frames");
                    }
                }
                5 => {
                    if t == 0 {
                        c.ext[0] &= !0xff;
                        c.ext[1] = wait;
                    } else {
                        let dx = if facing == 1 { -walk } else { walk };
                        c.x = Fx::from_int((c.x.floor() + dx) & 0xff);
                    }
                }
                1 => {
                    if t == 0 {
                        // $106B: a draw of 0 to 6, then drinking.
                        let _ = cx.draw("pushed_drink");
                        c.ext[0] = (c.ext[0] & !0xff) | 2;
                        c.ext[1] = drink;
                        c.ext[3] = (c.ext[3] & !0xffff) | drink | (bp(cx, "drink_turns") << 8);
                    } else {
                        let a = ((full_speed >> 8) + 2) & 0xff;
                        let a = if facing == 1 { (256 - a) & 0xff } else { a };
                        c.x = Fx::from_int((c.x.floor() + a) & 0xff);
                    }
                }
                2 => {
                    if t == 0 {
                        let turns = (c.ext[3] >> 8) & 0xff;
                        if turns != 0 {
                            // $10BA calls $1B83: a gulp, the byte of the
                            // table at $1BCE by the turns left less one
                            // ($1B8D to $1B95, the `sound_drink` rows).
                            ask(cx, "sound_drink", &[turns - 1]);
                            c.ext[1] = c.ext[3] & 0xff;
                            c.ext[3] = (c.ext[3] & !0xffff) | (c.ext[3] & 0xff) | ((turns - 1) << 8);
                        } else {
                            // $10C0: the empty mug, 3 above the line where
                            // the customer holds it, 16 toward the tap.
                            let r = draw_at_most(cx, "throw_wait", bp(cx, "throw_spread"));
                            c.ext[0] = (c.ext[0] & !0xff) | 3;
                            c.ext[1] = r + bp(cx, "throw_base");
                            let hx = c.x.floor() + if facing == 1 { -16 } else { 16 };
                            // $10E2: the first of the 9 mug slots from E2FE
                            // whose y is 0, none when all are taken. A slot
                            // emptied earlier this frame is the same object
                            // written again (its x's fraction, +6, kept).
                            let held = |n: i64| {
                                bodies.iter().any(|(id, b)| {
                                    b.kind == mug_k && b.ext[2] == n + 1 && !removed.contains(id)
                                }) || throws.iter().any(|b| b.ext[2] == n + 1)
                            };
                            // $1108 to $1123: its flags, the record's bar
                            // above, empty, and moving left on a bar whose
                            // tap is on the left.
                            let flags = (rec_bar << 4) | 1 | if facing == 1 { 8 } else { 0 };
                            // $1100 to $111F: its sprite's picture and
                            // flags ($13 moving left).
                            let one = |n: &str| cx.table(n).first().copied().unwrap_or(0);
                            let sprite = if facing == 1 { one("mug_flags_left") } else { one("mug_flags") }
                                | (one("mug_picture_empty") << 8);
                            if let Some(n) = (0..9).find(|&n| !held(n)) {
                                let (x, y) = (Fx::from_int(hx & 0xff), Fx::from_int(c.y.floor() - 3));
                                if let Some(j) = bodies.iter().position(|(id, b)| {
                                    b.kind == mug_k && b.ext[2] == n + 1 && removed.contains(id)
                                }) {
                                    let id = bodies[j].0;
                                    removed.retain(|r| *r != id);
                                    let b = &mut bodies[j].1;
                                    b.x = x;
                                    b.y = y;
                                    b.facing_left = facing == 1;
                                    b.ext[3] = flags;
                                    b.ext[0] = sprite;
                                } else {
                                    let mut b = gp_sim::systems::new_body(cx.spec, cx.world, mug_k, x, y, facing == 1);
                                    b.ext[2] = n + 1;
                                    b.ext[3] = flags;
                                    b.ext[0] = sprite;
                                    throws.push(b);
                                }
                            }
                        }
                    }
                }
                3 => {
                    if t == 0 {
                        c.ext[0] = (c.ext[0] & !0xff) | 4;
                        c.ext[1] = bp(cx, "after_throw");
                        // $20DE, called at $1142: while the level has tips
                        // left (E72F), each throw counts E72E down; at 0 a
                        // tip is left on the bar ($20F4: 8 above the
                        // customer, 12 further from the far end's picture
                        // when within 12 of it) and the count starts again
                        // at a draw of 0 to 3 plus 4 ($213B).
                        if tips_left != 0 {
                            tip_count = (tip_count - 1) & 0xff;
                            // $20EF: a tip left asks for $16.
                            if tip_count == 0 {
                                ask(cx, "sound_tip", &[1]);
                            }
                            if tip_count == 0
                                && let Some(tk) = tip_k
                            {
                                let bar = Self::bar_of(cx, c.y.floor());
                                let left = Self::tap_left(cx, bar);
                                // $2107 to $211D: the x of the bar's far
                                // end, the object at E366 plus 4 a bar (the
                                // tap's word 0).
                                let end = cx.kind("tap").map_or(0, |k| {
                                    cx.world
                                        .bodies
                                        .iter()
                                        .filter(|b| b.kind == k)
                                        .nth(usize::try_from(bar).unwrap_or(0))
                                        .map_or(0, |b| b.ext[0])
                                });
                                let cxp = c.x.floor();
                                // The sums are a byte's: a far end at 253
                                // plus 12 is 9 ($2122, $212D).
                                let x = if left {
                                    if (end - 12) & 0xff >= cxp { cxp } else { cxp - 12 }
                                } else if (end + 12) & 0xff < cxp {
                                    cxp
                                } else {
                                    cxp + 12
                                };
                                // The same object, written again: a tip
                                // still on the bar moves to the new place.
                                let old = tip.take().and_then(|(id, _)| id);
                                let nb = gp_sim::systems::new_body(
                                    cx.spec,
                                    cx.world,
                                    tk,
                                    Fx::from_int(x & 0xff),
                                    Fx::from_int(c.y.floor() - 8),
                                    false,
                                );
                                tip = Some((old, nb));
                                let r = draw_at_most(cx, "tip_wait", 3);
                                tip_count = r + 4;
                            }
                        }
                    }
                }
                4 => {
                    if t == 0 {
                        c.ext[0] &= !0xff;
                        c.ext[1] = wait;
                    }
                }
                // $11AA: watching the show, standing, its picture
                // mirrored while bit 2 of its timer is 0 ($11B2 to $11C5);
                // at 0 back to waiting with its slot's wait ($11CA to
                // $11EA), 255 frames after the pickup.
                6 => {
                    // Mirrored while bit 2 of the timer is 0: it turns as
                    // that bit changes (the timer counting down from 255,
                    // at 251, 247 and on), and turns back as it ends.
                    let mirrored = |t: i64| t < 252 && t & 4 == 0;
                    if mirrored(t) != mirrored(t + 1) || (t == 0 && mirrored(1)) {
                        c.ext[0] ^= 0x10 << 16;
                    }
                    if t == 0 {
                        c.ext[0] &= !0xff;
                        c.ext[1] = wait;
                    }
                }
                _ => {}
            }
            // The picture ($1016 to $11EA): each state's from the
            // record's bytes as it starts; while waiting (bit 0 flipped
            // every +0A frames) and walking (+0F), counted in the record's
            // +0E, which the record keeps while its customer is away.
            if let Some(b) = block_of(slot) {
                let ns = c.ext[0] & 0xff;
                let mut pic = (c.ext[0] >> 24) & 0xff;
                let n = rec_of(&recs, slot);
                let mut ctr = n.map_or(0, |n| (recs[n].1.ext[1] >> 16) & 0xff);
                match (state, ns) {
                    (0, 5) => {
                        pic = b[0x10];
                        ctr = b[0x0F];
                    }
                    (5 | 4 | 6, 0) => {
                        pic = b[0x12];
                        ctr = b[0x0A];
                    }
                    // Watching the show: its record's +1B ($154F's caller,
                    // the picture it takes as it turns to watch).
                    (_, 6) => pic = b[0x1B],
                    (1, 2) => pic = b[0x15],
                    (2, 3) => pic = b[0x14],
                    (3, 4) => pic = b[0x16],
                    (0, 0) | (5, 5) => {
                        ctr = (ctr - 1) & 0xff;
                        if ctr == 0 {
                            pic ^= 1;
                            ctr = if state == 0 { b[0x0A] } else { b[0x0F] };
                        }
                    }
                    _ => {}
                }
                c.ext[0] = (c.ext[0] & !(0xff << 24)) | (pic << 24);
                if let Some(n) = n {
                    recs[n].1.ext[1] = (recs[n].1.ext[1] & !(0xff << 16)) | (ctr << 16);
                }
                // Its second sprite, the mug it holds (+4 to +7): $106B
                // shows it as it starts drinking, the record's +1A at its
                // y (+4) and 16 toward the tap (+7); $10C0 changes it to
                // +18 as it throws; $1130 hides it (+4 to 0). Word 2 keeps
                // its picture (byte 2) and y (byte 3, 0 hidden).
                let (mut pic2, mut y2) = ((c.ext[2] >> 16) & 0xff, (c.ext[2] >> 24) & 0xff);
                match (state, ns) {
                    (1, 2) => {
                        pic2 = b[0x1A];
                        y2 = c.y.floor() & 0xff;
                    }
                    (2, 3) => pic2 = b[0x18],
                    (3, 4) => y2 = 0,
                    _ => {}
                }
                c.ext[2] = (c.ext[2] & 0xffff) | (pic2 << 16) | (y2 << 24);
            }
            // The held mug as drawn: its picture in byte 4 facing right
            // (16 right of the customer, $1098) or byte 5 facing left (16
            // left, $109C), 0 while it is hidden.
            // Bytes 4 and 5 level with it, 6 and 7 4 above (as it is
            // pushed back), so each draws at its own offset.
            let y2 = (c.ext[2] >> 24) & 0xff;
            let held = if y2 != 0 { (c.ext[2] >> 16) & 0xff } else { 0 };
            let left = (c.ext[0] >> 8) & 1 == 1;
            let above = y2 != 0 && (c.y.floor() - y2) & 0xff == 4;
            let at = 32 + 8 * (i64::from(left) + 2 * i64::from(above));
            c.ext[2] = (c.ext[2] & 0xffff_ffff) | (held << at);
            bodies[k].1 = c;
        }

        let mut served_any = false;
        let mut reach_end: Option<(i64, i64, i64)> = None;
        // $0890: a customer pushed past the bar's far end is served and
        // goes, and its slot's arrival comes again after the respawn wait
        // and 16 frames a level cleared ($091D to $094B).
        for (id, c) in &bodies {
            if c.kind != cust_k || removed.contains(id) {
                continue;
            }
            let bar = Self::bar_of(cx, c.y.floor());
            let (lo, hi) = Self::ends(cx, bar);
            let x = c.x.floor();
            // $08C3 and $08FD: a $DE customer's end is 4 further in.
            let slot = (c.ext[3] >> 16) & 0xff;
            let tall = rec_of(&recs, slot).is_some_and(|n| recs[n].1.ext[3] == 0xDE);
            let (lo, hi) = if tall { (lo - 4, hi + 4) } else { (lo, hi) };
            let served = if (c.ext[0] >> 8) & 1 == 0 { x < lo } else { x - 31 >= hi };
            // $08D2 and $08E8: one that isn't served and has reached the
            // bartender's end (facing right, x at the end, E3AF's bar
            // end; facing left, 31 short of the other) sets E6F9 to its
            // bar ($08D9) and starts $1CE1; the rest of $0890 doesn't run.
            let (lo0, hi0) = Self::ends(cx, bar);
            let at_end = if (c.ext[0] >> 8) & 1 == 0 { x >= hi0 } else { x - 31 < lo0 };
            if !served && at_end {
                reach_end = Some((bar, c.y.floor(), (c.ext[3] >> 16) & 0xf));
                break;
            }
            if served {
                // $090D: a customer served asks for $1E.
                ask(cx, "sound_customers", &[0x01]);
                served_any = true;
                removed.push(*id);
                let shown = usize::try_from(flow.map_or(0, |(_, b)| (b.ext[3] >> 8) & 3)).unwrap_or(0);
                points += cx.table("points_served").get(shown * 6..shown * 6 + 6).map_or(0, points_of);
                let slot = usize::try_from((c.ext[3] >> 16) & 0xff).unwrap_or(0);
                if let Some(&routine) = arrivals.get(slot) {
                    let d = w_word + 16 * cleared.min(11);
                    schedule(&mut timers, &mut timers_changed, &mut new_timers, cx, d, routine);
                }
            }
        }

        // $0963: each customer served takes one from E711, the customers
        // on the bars (one more at each arrival, $160F); at 0 the level is
        // cleared: the timers are emptied ($5DEC at $0969) and the game goes
        // to state 11 ($096C), whose level's end starts on the next frame.
        if served_any
            && !bodies.iter().any(|(id, b)| b.kind == cust_k && !removed.contains(id))
            && let Some((fid, mut fb)) = flow
        {
            for (n, (_, t)) in timers.iter_mut().enumerate() {
                t.ext[0] = 0;
                t.ext[1] = 0;
                timers_changed.push(n);
            }
            fb.ext[3] = (fb.ext[3] & !0xffff_00ff) | 11 | (9 << 16);
            cx.set_body(fid, fb);
        }
        // $0C25, after $0890: the show. A tip left on the bar in it ($20F4)
        // is the last dancer's object until the next picture is copied over
        // it. $1CE1 doesn't come back to the play routine ($232B goes to
        // the main loop, $22A3), so a frame that starts it doesn't count.
        if show.on
            && reach_end.is_none()
            && let Some(dk) = dancer_k
        {
            if let Some((_, t)) = tip {
                let mut d = dancers[8]
                    .unwrap_or_else(|| gp_sim::systems::new_body(cx.spec, cx.world, dk, t.x, t.y, false));
                d.x = t.x;
                d.y = t.y;
                d.ext[0] = 9;
                d.ext[2] = 0x4D;
                dancers[8] = Some(d);
            }
            show.left = (show.left - 1) & 0xff;
            if show.left == 0 {
                // $0C71 to $0C81: the show ends, the 9 objects' y 0, the
                // tip's with them.
                show.on = false;
                dancers = [None; 9];
                // $0C83 asks for 2, then $0C91 for the bar shown's byte of
                // the table at $4C71, as a level's setup does.
                ask_ld(cx, "sound_catch", &[0xB5]);
                ask(cx, "sound_game", &[0x13D + flow.map_or(0, |(_, b)| (b.ext[3] >> 8) & 3)]);
                // $0C95: the show's end loads the stage's screen again, and
                // its copy draws the lives ($5D21).
                if let Some((_, fb)) = flow {
                    cx.set_counter("lives_shown", (fb.ext[1] >> 40) & 0xff);
                }
                if let Some((Some(id), _)) = tip.take() {
                    tips_gone.push(id);
                }
                tip = None;
            } else {
                show.tick = (show.tick - 1) & 0xff;
                if show.tick == 0 {
                    // $0C38 to $0C6C: the bar's next picture, every 6
                    // frames, its place in the list counting 0 to 7.
                    show.picture = (show.picture + 1) & 7;
                    let shown = flow.map_or(0, |(_, b)| (b.ext[3] >> 8) & 3);
                    if let Some(pic) = show_picture(cx, shown, Some(show.picture)) {
                        Self::put_picture(cx, dk, &mut dancers, &pic);
                        Self::tip_from_dancers(cx, tip_k, &dancers, &mut tip, &mut tips_gone);
                    }
                    show.tick = 6;
                }
            }
        }
        // $1CE1, a customer at the bartender's end: it empties the timers
        // ($5DEC), the objects from E23E with the tip ($1CE9 to $1CF5) and
        // waits ($232B): the game is in states 15 and 16 from this frame,
        // the customers and mugs standing as they are. The flow's word 3
        // counts its frames (sub 10), word 2 keeps the customer's y and bar.
        if let Some((rbar, ry, rslot)) = reach_end
            && let Some((fid, mut fb)) = flow
        {
            for (n, (_, t)) in timers.iter_mut().enumerate() {
                t.ext[0] = 0;
                t.ext[1] = 0;
                timers_changed.push(n);
            }
            if let Some((Some(id), _)) = tip.take() {
                tips_gone.push(id);
            }
            tip = None;
            dancers = [None; 9];
            // $1CE4: it asks for 2 as it starts (the `sound_lost` rows are
            // the program from $1CE4).
            ask(cx, "sound_lost", &[1]);
            fb.ext[3] = (fb.ext[3] & !0xffff_00ff) | 15 | (10 << 16);
            fb.ext[2] = (fb.ext[2] & 0xffff) | ((ry & 0xff) << 16) | ((rbar & 3) << 24) | (rslot << 32);
            cx.set_body(fid, fb);
            if let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let mut pb = *pb;
                pb.ext[0] = rbar;
                // $1D15 to $1D3C: his hands to his head.
                lost_pose(cx, &mut pb, "lost_pose_slide", 0x1D15, (0x1D1C, 0x1D20, 0x1D25));
                cx.set_body(pid, pb);
            }
        }
        if fell && let Some((fid, mut fb)) = flow {
            fb.ext[3] = (fb.ext[3] & !0xffff_00ff) | 12;
            cx.set_body(fid, fb);
        }
        }

        // $60C1, after the play routine: each timer counts down; at 0 its
        // routine runs, so a customer arriving waits from the next frame. A customer
        // slot's arrival ($15FF on), the waits' speed-up ($2023) and the
        // respawn wait's changes ($1B2E, $1AE5); the rest aren't modelled.
        let mut fired: Vec<i64> = Vec::new();
        for n in 0..timers.len() {
            let t = &mut timers[n].1;
            if t.ext[0] > 0 {
                t.ext[0] -= 1;
                timers_changed.push(n);
                if t.ext[0] == 0 {
                    fired.push(t.ext[1]);
                }
            }
        }
        for routine in fired {
            if let Some(k) = arrivals.iter().position(|&a| a == routine) {
                // The arrival routine's first instruction asks for a
                // sound ($15FF: `ld c,N`, then $606B); the
                // `sound_arrivals` rows are the program from the first
                // arrival routine.
                let first = arrivals.iter().copied().min().unwrap_or(routine);
                ask(cx, "sound_arrivals", &[routine - first + 1]);
                // The slot's object from its start row; on a bar whose tap
                // is on the left, mirrored ($190F: x = $EE - x).
                let row = &starts[k * 8..k * 8 + 8];
                // $1B74: a customer whose record's picture set is $DE
                // stands 3 higher.
                let tall = recs
                    .iter()
                    .find(|(_, r)| r.ext[2] == k as i64)
                    .is_some_and(|(_, r)| r.ext[3] == 0xDE);
                let (y, x0) = (row[0] - if tall { 3 } else { 0 }, row[3]);
                let bar = Self::bar_of(cx, y);
                let left = layout & (1 << bar) != 0;
                let x = if left { 0xEE - x0 } else { x0 };
                let mut c = gp_sim::systems::new_body(cx.spec, cx.world, cust_k, Fx::from_int(x), Fx::from_int(y), left);
                // $1B58: waiting, for 16 frames and a draw of 0 to 16.
                let r = draw_at_most(cx, "arrival_wait", 16);
                // Its object's flags and picture as the start row has them
                // (+1, +2, copied in by the arrival), mirrored on a bar
                // whose tap is on the left (bit 4), in bytes 2 and 3 for
                // its board sprite; how a customer's picture changes as it
                // walks and drinks isn't modelled.
                // $1B58: its flags' low nibble is its record's +07 (its
                // colours), over the start row's, mirrored ($190F).
                let block = record_block(cx, flow.map_or(0, |(_, b)| (b.ext[3] >> 8) & 0xff));
                let rec7 = block.get(k * 32 + 7).copied().unwrap_or(row[1] & 0xf);
                let flags = ((row[1] | if left { 0x10 } else { 0 }) & 0xf0) | rec7;
                // $161B: its picture is its record's +10, not the start
                // row's.
                let pic = block.get(k * 32 + 0x10).copied().unwrap_or(row[2]);
                c.ext[0] = (i64::from(left) << 8) | (flags << 16) | (pic << 24);
                c.ext[1] = r + 16;
                // The start row's second sprite (+4 to +7), copied in with
                // the first ($160D): its picture and y, the held mug.
                c.ext[2] = (row[6] << 16) | (row[4] << 24);
                c.ext[3] = (k as i64) << 16;
                bodies.push((next_id, c));
                added.push(next_id);
                next_id += 1;
            } else if routine == 0x2023 {
                // $2023: every customer's wait shortens by 2, 4, 7 or 10 by
                // its bar, unless it would reach 0; then again later.
                for (_, r) in &mut recs {
                    let wait = r.ext[0] & 0xff;
                    let e = [2, 4, 7, 10][usize::try_from((r.ext[1] & 0xff).min(3)).unwrap_or(3)];
                    let a = wait - e;
                    if wait != 1 && a > 0 {
                        r.ext[0] = (r.ext[0] & !0xff) | a;
                    }
                }
                let d = row_word(level, 2);
                schedule(&mut timers, &mut timers_changed, &mut new_timers, cx, d, 0x2023);
            } else if routine == 0x1B2E {
                w_word = (row_word(level, 0) + 0x5A) & 0xffff;
                schedule(&mut timers, &mut timers_changed, &mut new_timers, cx, 0x1FE, 0x1AE5);
            } else if routine == 0x1AE5 {
                w_word = row_word(level, 0);
            }
        }

        cx.me.ext[3] = w_word | (cleared << 16) | (level << 24);
        // $5EE7 adds only in a game (E3BD 0, $5EEB).
        if points != 0 && cx.counter("mode") == Some(0) {
            let score = cx.counter("score").unwrap_or(0);
            let new = (score + points) % 1_000_000;
            cx.set_counter("score", new);
            // $5F23: past 999,999 the extra lives given count from 0 again.
            let mut given = if score + points >= 1_000_000 { 0 } else { cx.counter("lives_given").unwrap_or(0) };
            // $05D1 calls $6104 after a frame's points: the score in
            // thousands, less the first setting (E05C), and then each step
            // of the second (E05D), is the extra lives due; $6143 gives
            // those not given yet, one more life each (E4A7) and a request
            // to the sound board.
            let defaults = cx.table("defaults").to_vec();
            let (first, step) = (defaults.get(0x5C).copied().unwrap_or(0), defaults.get(0x5D).copied().unwrap_or(0));
            let thousands = new / 1000;
            let due = if first == 0 || thousands < first {
                0
            } else if step == 0 {
                1
            } else {
                1 + (thousands - first) / step
            };
            if due > given
                && let Some(fk) = cx.kind("flow")
                && let Some(fid) = cx.world.bodies.iter().position(|b| b.kind == fk)
            {
                let mut fb = cx
                    .requests()
                    .iter()
                    .rev()
                    .find_map(|r| match r {
                        gp_sim::ext::Request::SetBody(i, b) if *i == fid => Some(*b),
                        _ => None,
                    })
                    .unwrap_or(cx.world.bodies[fid]);
                while due > given {
                    given += 1;
                    let l = (((fb.ext[1] >> 40) & 0xff) + 1) & 0xff;
                    fb.ext[1] = (fb.ext[1] & !(0xff << 40)) | (l << 40);
                    lives_now = Some(l);
                    // E391 has the HUD draw the lives again ($6160).
                    cx.set_counter("lives_shown", l);
                    ask(cx, "sound_extra_life", &[0]);
                }
                cx.set_body(fid, fb);
            }
            if cx.counter("lives_given") != Some(given) {
                cx.set_counter("lives_given", given);
            }
        }
        // The counters the board's HUD and its looks read: the levels
        // cleared, the lives and the bar shown as the hooks keep them.
        if cx.counter("mode") == Some(0) || demo {
            cx.set_counter("cleared", cleared);
            if let Some(fb) = cx.kind("flow").and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k)) {
                let (lives, shown) = (lives_now.unwrap_or((fb.ext[1] >> 40) & 0xff), (fb.ext[3] >> 8) & 3);
                cx.set_counter("lives", lives);
                cx.set_counter("shown", shown);
                // A clip that starts in play has the lives its screen
                // shows: those it has.
                if cx.counter("lives_shown").is_none_or(|v| v < 0) {
                    cx.set_counter("lives_shown", lives);
                }
            }
        }
        cx.me.ext[0] = walk | (tip_count << 8) | (tips_left << 16) | (tip_setting << 24);
        cx.me.ext[1] = (cx.me.ext[1] & 0xff) | show.word();
        for (id, b) in &bodies {
            if added.contains(id) {
                continue;
            }
            if removed.contains(id) {
                cx.remove(*id);
            } else {
                cx.set_body(*id, *b);
            }
        }
        // New bodies, in the order their ids were given: each spawned,
        // then given its words.
        let mut spawned: Vec<Body> = bodies
            .iter()
            .filter(|(id, _)| added.contains(id))
            .map(|(_, b)| *b)
            .collect();
        spawned.extend(throws);
        spawned.extend(end_spawn);
        spawned.extend(timers.iter().filter(|(id, _)| *id == usize::MAX).map(|(_, t)| *t));
        spawned.extend(recs.iter().filter(|(id, _)| *id == usize::MAX).map(|(_, r)| *r));
        let first = cx.world.bodies.len();
        for b in &spawned {
            cx.spawn(b.kind, b.x, b.y, b.facing_left);
        }
        for (n, b) in spawned.iter().enumerate() {
            let mut b = *b;
            if removed.contains(&(first + n)) {
                b.alive = false;
            }
            cx.set_body(first + n, b);
        }
        timers_changed.sort_unstable();
        timers_changed.dedup();
        for n in timers_changed {
            if timers[n].0 != usize::MAX {
                cx.set_body(timers[n].0, timers[n].1);
            }
        }
        for (id, r) in recs.iter().filter(|(id, _)| *id != usize::MAX) {
            cx.set_body(*id, *r);
        }
        // The tip last, so the ids given above stay as counted.
        for id in tips_gone {
            cx.remove(id);
        }
        // Bodies spawned this step take their ids in order from here.
        let mut n = first + spawned.len();
        match tip {
            Some((Some(id), t)) => cx.set_body(id, t),
            Some((None, t)) => {
                cx.spawn(t.kind, t.x, t.y, t.facing_left);
                cx.set_body(n, t);
                n += 1;
            }
            None => {}
        }
        // The dancers, by slot: an object kept is the same body, one whose
        // y became 0 goes.
        for (id, d) in dancer_ids.iter().zip(dancers.iter()) {
            match (id, d) {
                (Some(id), Some(b)) => cx.set_body(*id, *b),
                (Some(id), None) => cx.remove(*id),
                (None, Some(b)) => {
                    cx.spawn(b.kind, b.x, b.y, b.facing_left);
                    cx.set_body(n, *b);
                    n += 1;
                }
                (None, None) => {}
            }
        }
        // The bonus round's objects, by address: one kept is the same
        // body, one gone (y 0, or out of the round) is removed.
        if let Some(ok) = cx.kind("bonus_object") {
            let in_round = bonus_objs.is_some();
            let (mut want, text) = bonus_objs.unwrap_or_default();
            if !in_round {
                want.extend(pour_layers(cx));
                if let Some(n) = scene_frame(cx) {
                    want.extend(demo_scene(cx, n));
                }
            }
            if cx.counter("bonus_text") != Some(text) {
                cx.set_counter("bonus_text", text);
            }
            place_objects(cx, ok, &want);
        }
    }

    fn describe(&self) -> Vec<Record> {
        vec![Record {
            name: "tapper.bars",
            version: env!("CARGO_PKG_VERSION"),
            carrier: Carrier::Entity,
            system: "platformer.enemies",
            class: "flow",
            implementation: "gp-ext-tapper::Bars",
            like: "",
            about: "Each frame after the bartender: empty mugs caught, a tip picked up and the show it starts, mugs moved and caught by customers, the customers' waits, walks, drinks, throws and watching, and customers served off the bar's end; the attract mode's demo game, its start ($5A64), its life lost's end and the high scores' wait after it",
            params: BARS_PARAMS,
            state: &["ext[0] walk, plus E72E, E72F and E006 a byte each above it", "ext[1] empty mug speed, then the show's E70F, E712, E70E and E710 a byte each above it", "ext[2] full mug speed, 8.8"],
            reads: &["mug", "customer", "player", "tap.facing", "tip", "dancer"],
            writes: &["mug", "customer", "tip", "dancer"],
            built_from: &[Primitive::Timer, Primitive::Condition, Primitive::Quantity],
            ..Record::EMPTY
        }]
    }
}

#[cfg(test)]
mod bits {
    /// The game-start flag, cleared every frame, never touches a field of
    /// the bonus round in the same word (the can being shaken lost its
    /// middle bit, so the bandit shook cans 0 and 1 again for 2 and 3).
    #[test]
    fn the_start_flag_is_apart_from_the_bonus_fields() {
        assert_eq!(super::STARTED & super::BONUS_MASK, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::four_way;

    /// Right, left, down, up held each frame, through the filter.
    fn run(frames: &[[bool; 4]]) -> Vec<[bool; 4]> {
        let mut word = 0;
        frames.iter().map(|&h| four_way(&mut word, h)).collect()
    }

    const NONE: [bool; 4] = [false; 4];
    const RIGHT: [bool; 4] = [true, false, false, false];
    const LEFT: [bool; 4] = [false, true, false, false];
    const UP: [bool; 4] = [false, false, false, true];
    const UP_RIGHT: [bool; 4] = [true, false, false, true];
    const UP_LEFT: [bool; 4] = [false, true, false, true];

    #[test]
    fn a_diagonal_from_rest_goes_horizontal() {
        assert_eq!(run(&[NONE, UP_RIGHT, UP_RIGHT]), [NONE, RIGHT, RIGHT]);
    }

    #[test]
    fn a_diagonal_keeps_the_direction_that_changed() {
        assert_eq!(run(&[LEFT, UP_LEFT, UP_LEFT, UP]), [LEFT, UP, UP, UP]);
    }

    #[test]
    fn opposing_directions_cancel() {
        assert_eq!(run(&[[true, true, false, false]]), [NONE]);
    }
}
