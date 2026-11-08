//! Donkey Kong: Original Edition's stage-clear scene ($8A81, called from
//! $8A71 on every frame while $53 isn't 1, so after 25m's, 50m's and 75m's
//! goals), from the slow timer $43 as the scene reads it. The model keeps
//! $43 as `clear_wait` (the mapping's pairs), so the hook reads $43 back
//! from it.
//!
//! | $43 | Routine | What it draws |
//! |---|---|---|
//! | $8A-$8B | $8AB0 | Kong standing ($C815 with 2), the other sprites hidden; the model's `clear_scene` `hide` |
//! | $89 | $8AF3 | Kong turned ($C815 with 0, `kong_throw`); the model's `kong_drawn` 3 |
//! | $78-$88 | $8B03 | Once (from then `clear_pic` is 1, which hides the player, Pauline and Kong): Mario's and Pauline's sprites hidden ($F08C from $00 and $E8), Kong cleared from the name table and sprite palette 0's colors ($8BD8, through $CD76), CHR bank 2, or 3 after 50m ($8000). Every frame: Kong as 30 sprites from tile $40, or $60 with $43 odd, 6 columns of 5 a column at a time, at x $46 and y $18 + 4 ($43 - $88) ($F080 at $0210), Pauline 2 by 2 from tile $80 at 29 right of and 15 below him ($F080 at $0200, $F086 mirrored with $43 odd), then every sprite of the first $22 whose y has bit 7 set moved off ($8BAA) |
//!
//! The `kong_carry` bodies are Kong's five rows of sprites (a row each, by
//! their order among the kind's bodies), the `pauline_carry` body Pauline;
//! each runs this hook at the frame's end (`hook_last`), places itself and
//! puts its picture in its word 0: 0 for none (the body then stands above
//! the screen, drawn as the blank tile $7F), else for a row 1 + row, plus 5 with $43 odd, and for
//! Pauline 1, or 2 mirrored. The first `kong_carry` also makes
//! `mladder_copy` bodies where 50m's moving ladders stand when the scene's
//! machine enters `lift`: the scene leaves their sprites ($02A0-$02BF, past
//! the $24 sprites $8AB0 hides) as they were until the stage setup.

use gp_sim::{Ext, ExtCx, Fx};

/// Kong's place across ($8B55).
const KONG_X: i64 = 0x46;
/// Kong's top at $43 = $88 ($8B50).
const KONG_Y: i64 = 0x18;
/// Pauline from Kong's place ($8B75-$8B84).
const PAULINE_DX: i64 = 0x1D;
const PAULINE_DY: i64 = 0x0F;
/// Where a body with nothing to show stands: its picture above the
/// screen's top line, and on no line the sprite limit counts.
const AWAY: i64 = -24;

/// $43 from `clear_wait` in the scene ($43 141 is 37; the tally takes $43
/// from 133 to 131 and the barrels' clear from 123 to 121, the mapping's
/// pairs), for the part of it $8B03 draws in.
fn timer(clear_wait: i64) -> Option<i64> {
    match clear_wait {
        29..=37 => Some(clear_wait + 104),
        20..=28 => Some(clear_wait + 103),
        18..=19 => Some(clear_wait + 102),
        _ => None,
    }
}

/// Whether `machine` is in `state` now.
fn in_state(cx: &ExtCx<'_>, machine: &str, state: &str) -> bool {
    cx.spec.machines.iter().enumerate().any(|(i, m)| {
        m.name == machine
            && cx
                .world
                .machines
                .get(i)
                .and_then(|s| m.states.get(s.state))
                .is_some_and(|st| st.name == state)
    })
}

/// The `dk_carry` hook.
pub struct Carry;

impl Ext for Carry {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let me = cx.me.kind;
        let own = cx.world.bodies[..cx.id]
            .iter()
            .filter(|b| b.kind == me && b.alive)
            .count() as i64;
        let pauline = cx.kind("pauline_carry") == Some(me);
        if !pauline && own == 0 {
            ladder_copies(cx);
        }
        // $8B03 draws while $43 is $78 to $88, after a goal on any stage
        // but 100m ($53 1 then, the ending's instead).
        // The scene reads $43 as the frame starts: the wrap that steps it
        // comes last in the frame ($F4AC).
        let scene = in_state(cx, "game", "cleared") && cx.counter("stage").unwrap_or(0) > 0;
        let t = scene
            .then(|| timer(cx.counter_at_start("clear_wait").unwrap_or(0)))
            .flatten();
        if !pauline && own == 0 {
            // `clear_pic` for presentation (present.hidden): 1 from the
            // frame $8B03 first runs, which hides Mario and Pauline and
            // clears Kong from the name table, to the stage's end.
            let pic = i64::from(t.is_some_and(|t| t <= 0x88));
            if cx.counter("clear_pic").is_some_and(|v| v != pic) {
                cx.set_counter("clear_pic", pic);
            }
        }
        let at = t.filter(|t| (0x78..=0x88).contains(t));
        let place = at.and_then(|t| {
            let top = KONG_Y + 4 * (t - 0x88);
            let odd = t & 1;
            let (x, y, word) = if pauline {
                (KONG_X + PAULINE_DX, top + PAULINE_DY, 1 + odd)
            } else {
                (KONG_X, top + 8 * own, 1 + own + 5 * odd)
            };
            // A sprite whose y has bit 7 set is moved off ($8BAA). Pauline
            // is two rows of sprites: with her top row's y below 0 her lower
            // one still shows, and the top one's place is in the 8 lines
            // above the picture the console hides.
            let lowest = if pauline { -8 } else { 0 };
            (lowest..0x80).contains(&y).then_some((x, y, word))
        });
        let (x, y, word) = place.unwrap_or((0, AWAY, 0));
        cx.me.x = Fx::from_int(x);
        cx.me.y = Fx::from_int(y);
        cx.me.ext[0] = word;
    }
}

/// 50m's moving ladders as the scene leaves them: a copy of each, made on
/// the frame the `clear_scene` machine enters `lift` (its next frame
/// clears the ladders, whose state the mapping reads from RAM the scene
/// changes), until the barrels' clear at `clear_wait` 19.
fn ladder_copies(cx: &mut ExtCx<'_>) {
    let (Some(ladder), Some(copy)) = (cx.kind("mladder"), cx.kind("mladder_copy")) else {
        return;
    };
    if !in_state(cx, "clear_scene", "lift") || cx.state_at_start("clear_scene", "lift") {
        return;
    }
    let at: Vec<(Fx, Fx, bool)> = cx
        .world
        .bodies
        .iter()
        .filter(|b| b.kind == ladder && b.alive && !b.waiting)
        .map(|b| (b.x, b.y, b.facing_left))
        .collect();
    for (x, y, left) in at {
        cx.spawn(copy, x, y, left);
    }
}
