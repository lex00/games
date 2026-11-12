//! Tapper's `ext` hooks, written against gp-sim's [`Ext`] trait. Every
//! number a hook uses is the model's `ext_params`, with its evidence in the
//! model (frames of the MAME oracle's captures, which gp-arcade matches on
//! every frame).
//!
//! | Hook | Entity | What it does |
//! |---|---|---|
//! | `tapper_bartender` | player | Walks along his bar, moves to the next bar's tap when the stick is pushed up or down, and steps back from the tap when a mug is sent |
//!
//! The bartender's words: `ext[0]` his bar (0 the top row on the screen,
//! 3 the bottom; the game's E6F9), `ext[1]` frames left before he lands at
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
    out
}

fn p(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.param(name).floor()
}

/// The bartender ($E2E6's object).
pub struct Bartender;

const PARAMS: &[Param] = &[
    Param::new("walk_step", "fixed", "px/frame"),
    Param::new("switch_frames", "frames", "frames"),
    Param::new("row_height", "fixed", "px"),
    Param::new("row_top", "fixed", "px"),
    Param::new("stand_right", "fixed", "px"),
    Param::new("stand_left", "fixed", "px"),
    Param::new("reach", "fixed", "px"),
    Param::new("far_left", "fixed", "px"),
    Param::new("far_right", "fixed", "px"),
    Param::new("send_step", "fixed", "px"),
    Param::new("pour_frames", "frames", "frames"),
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
        // Moving to another bar: the stick's picture runs, then he is at
        // the new bar's tap.
        if cx.me.ext[1] > 0 {
            cx.me.ext[1] -= 1;
            if cx.me.ext[1] == 0 {
                let to = cx.me.ext[2];
                cx.me.ext[0] = to;
                if let Some(x) = Self::stand(cx, to) {
                    cx.me.x = Fx::from_int(x);
                }
                cx.me.y = Fx::from_int(p(cx, "row_top") + p(cx, "row_height") * to);
                cx.me.ext[3] = 0;
            }
            return;
        }
        let up = cx.pressed("up");
        let down = cx.pressed("down");
        if up != down {
            // Up takes him to the bar below on the screen, down to the one
            // above, round the four.
            cx.me.ext[2] = (bar + if up { 1 } else { 3 }) % 4;
            cx.me.ext[1] = p(cx, "switch_frames");
            return;
        }
        let tap = Self::tap(cx, bar);
        // Pouring: the tap held puts him at the tap, wherever on the bar he
        // was, and keeps him there; let go after a full mug, he steps back
        // from it.
        if cx.held("button1") {
            if let Some(x) = Self::stand(cx, bar) {
                cx.me.x = Fx::from_int(x);
            }
            cx.me.ext[3] += 1;
            return;
        }
        if cx.me.ext[3] >= p(cx, "pour_frames") {
            if let Some((_, left)) = tap {
                let step = p(cx, "send_step");
                cx.me.x = Fx::from_int(cx.me.x.floor() + if left { step } else { -step });
            }
        }
        cx.me.ext[3] = 0;
        let left = cx.held("left");
        let right = cx.held("right");
        if left == right {
            return;
        }
        let (lo, hi) = match tap {
            Some((x, true)) => (x + p(cx, "stand_left") - p(cx, "reach"), p(cx, "far_right")),
            Some((x, false)) => (p(cx, "far_left"), x - p(cx, "stand_right") + p(cx, "reach")),
            None => (p(cx, "far_left"), p(cx, "far_right")),
        };
        let step = p(cx, "walk_step");
        let x = cx.me.x.floor() + if right { step } else { -step };
        // max then min, not clamp: a bar with no tap in reach (the bonus
        // round's) may give lo over hi.
        cx.me.x = Fx::from_int(x.max(lo).min(hi));
        cx.me.facing_left = left;
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
            state: &["ext[0] bar", "ext[1] switch frames left", "ext[2] bar moving to", "ext[3] tap held"],
            reads: &["action", "tap.x", "tap.facing"],
            writes: &["self.x", "self.y", "self.facing"],
            built_from: &[Primitive::InputBinding, Primitive::Timer, Primitive::Condition, Primitive::Quantity],
            ..Record::EMPTY
        }]
    }
}
