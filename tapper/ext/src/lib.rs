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
    out.insert("tapper_bars".into(), Box::new(Bars));
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
        // The play routine ($0660, from $22FC) runs only in play: in a life
        // lost (state 12, then 9) and its waits (15, 16) he stands still.
        let flow = cx
            .kind("flow")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
            .copied();
        let state = flow.map_or(10, |b| b.ext[3] & 0xff);
        // State 11 too: a level's end ($0963 sets it the frame the last
        // customer is served), whose next frame doesn't run $0660
        // (tapper.l13.001: tap held on 20481, he stays at 187).
        if [9, 11, 12, 15, 16].contains(&state) {
            return;
        }
        // On the setup's own frame (the flow's word 2 at 4, set by
        // tapper_bars where the setup places him) the game is still in the
        // setup's state (E397 9, 11 or 8): $0660 runs from the next frame,
        // so a press on that frame is not seen (tapper.l13.001: up on
        // 19672, the setup's frame, moves nothing; up on 19685 lands on
        // 19692).
        if flow.is_some_and(|b| b.ext[2] == 4) {
            return;
        }
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
                                cx.spawn(mug, b.x, b.y, b.facing_left);
                                cx.set_body(id, b);
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
    Param::new("bar_left", "fixed", "px"),
    Param::new("bar_right", "fixed", "px"),
    Param::new("bar_inset", "fixed", "px"),
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

/// The random routine $5FAC's number for a site: the original's when the
/// run has its draws, else one from the generator below `n`.
fn draw_below(cx: &mut ExtCx<'_>, site: &str, n: i64) -> i64 {
    match cx.scripted_draw(site) {
        Some(v) => i64::from(v),
        None => i64::from(cx.draw(site)) % n.max(1),
    }
}

fn bp(cx: &ExtCx<'_>, name: &str) -> i64 {
    let Some(k) = cx.kind("bars") else { return 0 };
    cx.spec.kinds[k].ext_params.get(name).map_or(0, |v| v.floor())
}

impl Bars {
    /// The bar a body on line y is at: 0 the bottom.
    fn bar_of(cx: &ExtCx<'_>, y: i64) -> i64 {
        let line = bp(cx, "line");
        let step = bp(cx, "line_step");
        (y - line + step / 2).div_euclid(step)
    }
    /// Bar `bar`'s ends: the code's table at $0977, 4 bytes a bar.
    fn ends(cx: &ExtCx<'_>, bar: i64) -> (i64, i64) {
        let inset = bp(cx, "bar_inset") * bar;
        (bp(cx, "bar_left") + inset, bp(cx, "bar_right") - inset)
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
        let (timer_k, rec_k) = (cx.kind("timer"), cx.kind("record"));
        let player_k = cx.kind("player");
        let line = bp(cx, "line");
        let step = bp(cx, "line_step");
        let (mut walk, mut empty_speed, full_speed) = (cx.me.ext[0] & 0xff, cx.me.ext[1], cx.me.ext[2]);
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
        let cleared = (cx.me.ext[3] >> 16) & 0xff;
        let level = (cx.me.ext[3] >> 24) & 0xff;
        let layout = cx
            .kind("flow")
            .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
            .map_or(0, |b| b.ext[1]);
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
            // playing all the while).
            fb.ext[3] = (fb.ext[3] & !0xff) | 10;
            // $4A65: the records of the bar shown, each slot's wait, drink,
            // bar and number, and its facing from the layout.
            let shown = usize::try_from((fb.ext[3] >> 8) & 3).unwrap_or(0);
            let blocks = cx.table("record_blocks").to_vec();
            // The records are carried in slot order; before a game's first
            // setup their bytes are blank, so the n-th is slot n.
            for (k, (_, r)) in recs.iter_mut().enumerate() {
                let Some(rec) = blocks.get((shown * 16 + k) * 32..(shown * 16 + k + 1) * 32) else {
                    continue;
                };
                let bar = rec[8];
                let facing = (layout >> bar) & 1;
                r.ext[0] = rec[2] | (rec[9] << 8);
                r.ext[1] = bar | (facing << 8);
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
        let flow = cx
            .kind("flow")
            .and_then(|k| cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == k))
            .map(|(i, b)| (i, *b));
        let state = flow.map_or(10, |(_, b)| b.ext[3] & 0xff);
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
        let ending = state == 11 && !setting_up && bartender_y != 0 && !level_timers;
        let finishing = state == 11 && !setting_up && bartender_y != 0 && level_timers;
        if (ending || finishing) && let Some((fid, mut fb)) = flow {
            fb.ext[3] = if ending {
                // The level's end script ($55F4, then the level's own, as
                // $4603) starts on this frame: its frames are counted from
                // 1 in the flow's word 3 (sub 8).
                (fb.ext[3] & 0xff00) | 15 | (8 << 16) | (1 << 24)
            } else {
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
        let mut life_lost = state == 12 || state == 9;
        if state == 12 && let Some((fid, mut fb)) = flow {
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
                    e706 = 13;
                    next_sub = 1;
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
                    }
                }
                2 => {
                    e706 = 1;
                    e709 = (e709 - 1) & 0xff;
                    if e709 == 0 {
                        next_sub = 3;
                    }
                }
                3 if (fb.ext[3] >> 8) & 3 != 1 => {
                    // $24B9: unless the bar shown (E3B8) is the sports bar,
                    // $24C0 calls $5E99, which empties the list at E11A and
                    // waits for the next frame ($60F6, the main loop's own
                    // frame wait), so $24CC comes a frame later
                    // (tapper.l13.001: E6FF 3 on 19599, state 9 on 19601).
                    next_sub = 6;
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
        } else if state == 9 && let Some((fid, mut fb)) = flow {
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
            if let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let mut pb = *pb;
                pb.y = Fx::ZERO;
                pb.x = Fx::ZERO;
                cx.set_body(pid, pb);
            }
            let wait = 2 + 30 + 5 + if cleared == 0 { 100 } else { 0 };
            fb.ext[3] = (fb.ext[3] & !0xffff_00ff) | 15 | (5 << 16) | (wait << 24);
            cx.set_body(fid, fb);
            life_lost = true;
        } else if (state == 15 || state == 16) && sub == 5 && let Some((fid, mut fb)) = flow {
            let left = ((fb.ext[3] >> 24) & 0xff) - 1;
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
            // $1FB9) on the next (tapper.l6.001 from 8846). What follows (a
            // life lost's last step or game over, $2441) isn't modelled.
            let n = ((fb.ext[3] >> 24) & 0xff) + 1;
            let ry = (fb.ext[2] >> 16) & 0xff;
            let rbar = (fb.ext[2] >> 24) & 3;
            let mut phase = (fb.ext[2] >> 26) & 7;
            let rslot = (fb.ext[2] >> 29) & 0xf;
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
                    if n == 36 {
                        pb.y = Fx::from_int(ry);
                        pb.x = Fx::from_int(if left { tx + 12 } else { tx - 8 });
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
                        }
                    } else if (1..4).contains(&phase) {
                        pb.x = Fx::from_int((x + step) & 0xff);
                        phase += 1;
                    } else if phase == 4 {
                        pb.y = Fx::ZERO;
                        phase = 5;
                    }
                    // Each step of the slide ($1E68 to $1EBB) knocks over
                    // the mugs of his bar he has reached: 8 short of him
                    // ($1E97), or 16 past him on a left-tap bar ($1EA3);
                    // a knocked mug's picture is $11 and it drops 3.
                    if let Some(bx) = knock_at {
                        for (_, m) in bodies.iter_mut().filter(|(_, b)| b.kind == mug_k) {
                            let mx = m.x.floor();
                            let hit = if left { mx <= bx + 16 } else { bx - 8 < mx };
                            if m.ext[3] & 0x200 == 0 && (m.ext[3] >> 4) & 0xf == rbar && m.y.floor() != 0 && hit {
                                m.ext[3] |= 0x200;
                                m.y = Fx::from_int(m.y.floor() - 3);
                            }
                        }
                    }
                    cx.set_body(pid, pb);
                }
            }
            if n <= 255 {
                fb.ext[3] = (fb.ext[3] & !0xff00_0000) | (n << 24);
            }
            fb.ext[2] = (fb.ext[2] & !(7 << 26)) | (phase << 26);
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
            // From the second level a random bit ($467E, the Z80's refresh
            // register) can run $4742 a second time first; the model
            // doesn't: tapper.l2.001 and l3.001 didn't.
            // The count takes 16 bits from bit 24: the scripts run past 255
            // frames.
            let n = ((fb.ext[3] >> 24) & 0xffff) + 1;
            if n <= 0xffff {
                fb.ext[3] = (fb.ext[3] & !(0xffff << 24)) | (n << 24);
            }
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
                            b.ext[1] = n;
                        }
                    } else if n == b.ext[1] + 60 {
                        removed.push(bodies[j].0);
                    }
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
                    pb.x = Fx::from_int(x);
                    cx.set_body(pid, pb);
                }
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
        let playing = !ending && !life_lost && ![15, 16].contains(&state);
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
                // $1CE1 leaves the customers and mugs where they are.
                let reached = [15, 16].contains(&state) && end_sub == 10;
                if !reached && (b.kind != mug_k || !in_end) {
                    removed.push(*id);
                }
            }
            if let Some((Some(id), _)) = tip.take() {
                tips_gone.push(id);
            }
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
                }
            }
            // $0B86 to $0BD4, after the mugs: a tip (picture $4D at
            // E260) within reach is picked up and E72F, the tips left,
            // goes down by one. Its points ($5EE7) and the show it starts
            // ($14E7) aren't modelled.
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
            let x16 = b.x.floor() * 256 + (b.ext[1] & 0xff);
            let speed = if empty { empty_speed * 256 } else { full_speed };
            let x16 = if left_tap { x16 - speed } else { x16 + speed };
            b.x = Fx::from_int((x16 >> 8) & 0xff);
            b.ext[1] = x16 & 0xff;
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
                // $09C7: pushed back, for 14 frames plus a draw below 20.
                let r = draw_below(cx, "customer_wait_start", bp(cx, "pushed_spread"));
                c.ext[0] = (c.ext[0] & !0xff) | 1;
                c.ext[1] = bp(cx, "pushed_base") + r;
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
                        // $106B: a draw below 6, then drinking.
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
                            c.ext[1] = c.ext[3] & 0xff;
                            c.ext[3] = (c.ext[3] & !0xffff) | (c.ext[3] & 0xff) | ((turns - 1) << 8);
                        } else {
                            // $10C0: the empty mug, 3 above the line where
                            // the customer holds it, 16 toward the tap.
                            let r = draw_below(cx, "throw_wait", bp(cx, "throw_spread"));
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
                                } else {
                                    let mut b = gp_sim::systems::new_body(cx.spec, cx.world, mug_k, x, y, facing == 1);
                                    b.ext[2] = n + 1;
                                    b.ext[3] = flags;
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
                        // at a draw below 3 plus 4 ($213B).
                        if tips_left != 0 {
                            tip_count = (tip_count - 1) & 0xff;
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
                                let r = draw_below(cx, "tip_wait", 3);
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
                _ => {}
            }
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
                served_any = true;
                removed.push(*id);
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
            fb.ext[3] = (fb.ext[3] & !0xffff_00ff) | 15 | (10 << 16);
            fb.ext[2] = (fb.ext[2] & 0xffff) | ((ry & 0xff) << 16) | ((rbar & 3) << 24) | (rslot << 29);
            cx.set_body(fid, fb);
            if let Some(pk) = player_k
                && let Some((pid, pb)) = cx.world.bodies.iter().enumerate().find(|(_, b)| b.kind == pk)
            {
                let mut pb = *pb;
                pb.ext[0] = rbar;
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
                // $1B58: waiting, for 16 frames and a draw below 16.
                let r = draw_below(cx, "arrival_wait", 16);
                c.ext[0] = i64::from(left) << 8;
                c.ext[1] = r + 16;
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
        cx.me.ext[0] = walk | (tip_count << 8) | (tips_left << 16) | (tip_setting << 24);
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
        for (id, r) in &recs {
            cx.set_body(*id, *r);
        }
        // The tip last, so the ids given above stay as counted.
        for id in tips_gone {
            cx.remove(id);
        }
        match tip {
            Some((Some(id), t)) => cx.set_body(id, t),
            Some((None, t)) => {
                let n = cx.world.bodies.len();
                cx.spawn(t.kind, t.x, t.y, t.facing_left);
                cx.set_body(n, t);
            }
            None => {}
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
            about: "Each frame after the bartender: empty mugs caught, mugs moved and caught by customers, the customers' waits, walks, drinks and throws, and customers served off the bar's end",
            params: BARS_PARAMS,
            state: &["ext[0] walk, plus E72E, E72F and E006 a byte each above it", "ext[1] empty mug speed", "ext[2] full mug speed, 8.8"],
            reads: &["mug", "customer", "player", "tap.facing", "tip"],
            writes: &["mug", "customer", "tip"],
            built_from: &[Primitive::Timer, Primitive::Condition, Primitive::Quantity],
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
