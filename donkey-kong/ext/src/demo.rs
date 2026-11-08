//! The attract demo's script ($EBDA), the `dk_demo` hook. While the
//! counter `attract` ($58) is set, the game runs this routine in place of
//! the pad's ($D175), before Mario's own routine reads $56, on every frame
//! of play on which Mario isn't jumping or falling ($CEC1 skips it while
//! $96 is 4 or 8). Its first run starts the script ($050B); after that,
//! each entry of the model's `demo_script` table (`way`, `frames`) is
//! loaded on a frame of its own, which leaves $56 as it was, and then held
//! for its frames: a direction goes to $56, and a way of 5 is a jump
//! ($96 = 4), with $56 kept, or nothing while the hammer is held. The hook
//! keeps the routine's bytes in counters (`demo_on` $050B, `demo_index`
//! $050E, `demo_left` $050C, `demo_way` $050D, `demo_pad` $56) and holds
//! the model's input actions to match, with core `entity.hook_first` so
//! the player's system reads them this frame.

use gp_sim::{Ext, ExtCx};

pub struct Demo;

/// The way that is a jump.
const JUMP: i64 = 5;

/// The script's entry `i`: its way and frames. Past the table's end the
/// game reads on into the bytes after it; the demo's Mario dies long
/// before, so there the script holds nothing.
fn entry(cx: &ExtCx<'_>, i: i64) -> (i64, i64) {
    let way = cx.table("way");
    let frames = cx.table("frames");
    match usize::try_from(i) {
        Ok(i) if i < way.len() && i < frames.len() => (way[i], frames[i]),
        _ => (0, 255),
    }
}

impl Demo {
    /// Jumping or falling ($96 is 4 or 8), when $CEC1 runs neither the
    /// script nor the pad's routine: in the air, or in a jump's landing
    /// hold.
    fn in_air(mario: &gp_sim::world::Body) -> bool {
        let landing = mario.ext[gp_sim::systems::LAND_HOLD_WORD] > 0;
        (!mario.on_ground && !mario.climbing) || landing
    }
}

impl Ext for Demo {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let c = |cx: &ExtCx<'_>, n: &str| cx.counter(n).unwrap_or(0);
        // The play routine runs $D041 after the bonus's step and before
        // Mario's routine on every frame of play: with the bonus under
        // 1000 ($2E below $10) it asks for the hurry tune, $FC = $20,
        // which a hammer's tune asked for later in the frame overrides
        // for that frame only (dkA-h6, original frame 24975).
        if cx.counter("bonus").is_some_and(|b| b < 1000) && cx.counter("sound_fc").is_some() {
            cx.set_counter("sound_fc", 0x20);
        }
        if c(cx, "attract") == 0 {
            // Out of the demo the pad's routine ($D175) runs in its place,
            // on the same frames, and stores the held directions in $56
            // ($D17B). A demo that starts later keeps them there until its
            // script gives a direction (dk-walk-user-4, original frame
            // 8332: $56 is the last game's 2).
            if let Some(mario) = crate::player(cx).copied()
                && !Self::in_air(&mario)
            {
                let pad = i64::from(cx.held("right"))
                    | i64::from(cx.held("left")) << 1
                    | i64::from(cx.held("down")) << 2
                    | i64::from(cx.held("up")) << 3;
                cx.set_counter("demo_pad", pad);
            }
            return;
        }
        // Start ends the demo and begins a game ($CF42 -> $C98A), after the
        // rest of the frame's play: the model's event takes the game's
        // rules there, and the play systems still run this frame.
        if cx.held("pause")
            && let Some(e) = cx.event("demo.start")
        {
            cx.fire(e, None);
        }
        let Some(mario) = crate::player(cx).copied() else {
            return;
        };
        let air = Self::in_air(&mario);
        let mut pad = c(cx, "demo_pad");
        let mut jump = false;
        if !air {
            if c(cx, "demo_on") == 0 {
                cx.set_counter("demo_on", 1);
                cx.set_counter("demo_index", 0);
                cx.set_counter("demo_left", 0);
            } else if c(cx, "demo_left") != 0 {
                let way = c(cx, "demo_way");
                if way == JUMP {
                    if c(cx, "hammer") > 0 {
                        pad = 0;
                    } else {
                        jump = true;
                    }
                } else {
                    pad = way;
                }
                cx.set_counter("demo_left", c(cx, "demo_left") - 1);
            } else {
                let i = c(cx, "demo_index");
                let (way, frames) = entry(cx, i);
                cx.set_counter("demo_left", frames);
                cx.set_counter("demo_way", way);
                cx.set_counter("demo_index", i + 1);
            }
        }
        cx.set_counter("demo_pad", pad);
        cx.hold("right", pad & 1 != 0);
        cx.hold("left", pad & 2 != 0);
        cx.hold("down", pad & 4 != 0);
        cx.hold("up", pad & 8 != 0);
        cx.hold("jump", jump);
    }
}
