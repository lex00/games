//! Tapper's `ext` hooks, written against gp-sim's [`Ext`] trait. Every
//! number a hook uses is the model's `ext_params`, with its evidence in the
//! model (frames of the MAME oracle's captures, which gp-arcade matches on
//! every frame).
//!
//! | Hook | Entity | What it does |
//! |---|---|---|
//! | `tapper_bartender` | player | Walks along his bar, moves to the next bar's tap when the stick is pushed up or down, and steps back from the tap when a mug is sent |
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

/// Every hook, by the name the model's `ext` gives it.
pub fn exts() -> Exts {
    let mut out = Exts::new();
    out.insert("tapper_bartender".into(), Box::new(Bartender) as Box<dyn Ext>);
    out.insert("tapper_flow".into(), Box::new(Flow));
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

const PARAMS: &[Param] = &[
    Param::new("walk_step", "fixed", "px/frame"),
    Param::new("switch_frames", "frames", "frames"),
    Param::new("switch_busy", "frames", "frames"),
    Param::new("row_height", "fixed", "px"),
    Param::new("row_bottom", "fixed", "px"),
    Param::new("stand_right", "fixed", "px"),
    Param::new("stand_left", "fixed", "px"),
    Param::new("reach", "fixed", "px"),
    Param::new("far_left", "fixed", "px"),
    Param::new("far_right", "fixed", "px"),
    Param::new("send_step", "fixed", "px"),
    Param::new("pour_frames", "frames", "frames"),
    Param::new("bob_frames", "frames", "frames"),
    Param::new("four_way", "bool", ""),
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
            if left == p(cx, "switch_busy") - p(cx, "switch_frames") {
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
        let held: [bool; 4] = ["right", "left", "down", "up"].map(|a| cx.held(a));
        let held = if p(cx, "four_way") != 0 {
            four_way(&mut cx.me.ext[3], held)
        } else {
            held
        };
        let dir = held.iter().position(|&h| h);
        let tap = Self::tap(cx, bar);
        match dir {
            None => {
                cx.me.ext[1] &= !1;
                // The tap ($1206) puts him at the tap and pours; let go
                // with a full mug (E718), he sends it ($13CC) and steps back.
                if cx.held("button1") {
                    if let Some(x) = Self::stand(cx, bar) {
                        cx.me.x = Fx::from_int(x);
                    }
                    cx.me.ext[3] += 1;
                    return;
                }
                if cx.me.ext[3] & 0xff >= p(cx, "pour_frames") {
                    if let Some((_, left)) = tap {
                        let step = p(cx, "send_step");
                        cx.me.x = Fx::from_int(cx.me.x.floor() + if left { step } else { -step });
                    }
                }
                cx.me.ext[3] &= !0xff;
            }
            Some(d @ (2 | 3)) => {
                if latch == 0 {
                    // Up takes him to the bar above on the screen (one
                    // more), down to the one below, round the four.
                    let to = (bar + if d == 3 { 1 } else { 3 }) % 4;
                    cx.me.ext[1] = p(cx, "switch_busy") * 8 + to * 2 + 1;
                }
            }
            Some(d) => {
                let right = d == 0;
                let (lo, hi) = match tap {
                    Some((x, true)) => (x + p(cx, "stand_left") - p(cx, "reach"), p(cx, "far_right")),
                    Some((x, false)) => (p(cx, "far_left"), x - p(cx, "stand_right") + p(cx, "reach")),
                    None => (p(cx, "far_left"), p(cx, "far_right")),
                };
                // His walk's bob: E727 counts 2, 1, 0 over the frames a
                // direction is held and E729 steps 0 to 3 when it runs out,
                // moving him 1 down, 1 down, 1 up, 1 up; both are kept when
                // he stops or changes bar (tapper-diagonal-4way-a, frames
                // 601 to 699).
                let (count, phase) = (cx.me.ext[2] & 0xff, (cx.me.ext[2] >> 8) & 3);
                if count == 0 {
                    let next = (phase + 1) & 3;
                    let dy = if next == 1 || next == 2 { 1 } else { -1 };
                    cx.me.y = Fx::from_int(cx.me.y.floor() + dy);
                    cx.me.ext[2] = p(cx, "bob_frames") - 1 + (next << 8);
                } else {
                    cx.me.ext[2] = count - 1 + (phase << 8);
                }
                let step = p(cx, "walk_step");
                let x = cx.me.x.floor() + if right { step } else { -step };
                // max then min, not clamp: a bar with no tap in reach (the
                // bonus round's) may give lo over hi.
                cx.me.x = Fx::from_int(x.max(lo).min(hi));
                cx.me.facing_left = !right;
            }
        }
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
            about: "Walks along his bar, moves to the next bar's tap on up or down, and steps back from the tap when a mug is sent",
            params: PARAMS,
            state: &["ext[0] bar", "ext[1] move frames left x8 plus the bar moving to x2 plus the stick latch", "ext[2] bob count plus 256 x bob phase", "ext[3] tap held (low byte), the 4-way filter's state above"],
            reads: &["action", "tap.x", "tap.facing"],
            writes: &["self.x", "self.y", "self.facing"],
            built_from: &[Primitive::InputBinding, Primitive::Timer, Primitive::Condition, Primitive::Quantity],
            ..Record::EMPTY
        }]
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
/// the layout.
pub struct Flow;

const FLOW_PARAMS: &[Param] = &[
    Param::new("start_bar", "count", ""),
    Param::new("start_right", "fixed", "px"),
    Param::new("start_left", "fixed", "px"),
    Param::new("tap_right", "fixed", "px"),
    Param::new("tap_left", "fixed", "px"),
    Param::new("tap_step", "fixed", "px"),
];

impl Ext for Flow {
    fn step(&self, cx: &mut ExtCx<'_>) {
        if cx.me.ext[0] == 0 {
            return;
        }
        cx.me.ext[0] = 0;
        let layout = cx.me.ext[1];
        let (Some(tap_kind), Some(player)) = (cx.kind("tap"), cx.kind("player")) else {
            return;
        };
        let step = p(cx, "tap_step");
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
                cx.set_body(id, t);
                k += 1;
            }
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
            b.ext = [bar, 0, 0, 0];
            cx.set_body(id, b);
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
            about: "A level's start: the taps by the level's layout and the bartender at his starting bar",
            params: FLOW_PARAMS,
            state: &["ext[0] the level was set up this frame", "ext[1] the layout"],
            writes: &["tap.x", "tap.facing", "player.x", "player.y", "player.ext"],
            built_from: &[Primitive::Condition, Primitive::Quantity],
            ..Record::EMPTY
        }]
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
