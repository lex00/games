//! Mario's bytes that the platformer's components don't keep, the sound
//! requests his routines make and the picture they last drew, read from
//! what his body did this frame. The `mario` body runs this hook after the
//! player's systems and after the hammers' hooks, so it sees where the
//! frame left him.
//!
//! | Game | Model | What it is |
//! |---|---|---|
//! | $9B | `walk_beat` | The walk's beat: a move across with it 0 sets it to 1; with it 1, asks for the step sound and clears it ($D233-$D244) |
//! | $97 | `walk_pose` | The walk's picture, 0, 4 or 8, stepped on the moves that ask for the step sound ($D246-$D273) |
//! | $85 | `walk_turn` | Which way the walk's picture goes from 4: to 8 when 0, to 0 when 1 |
//! | $5C | `climb_step` | The climb's step, 1 to 6, one a move ($D3E7, $D48B); its picture is `rom` at $C159 |
//! | $5B | `climb_top` | The step in a ladder's top zone, 1 to 16 going up and 13 down to 1 going down ($D3BB, $D463); its picture is `rom` at $C147 |
//! | $0201 | `mario_pose` | The picture the walk, the climb, the jump, the hammer or the death spin last drew; nothing else redraws him, so it stays (a fall redraws the same, $D697) |
//! | - | `pose_unface` | 1 when the last picture's mirroring is the game's own, not Mario's facing, and differs from it: a climb's picture drawn while he faced left, or the stage clear's standing picture, always mirrored, while he faced right |
//! | $FF | `sound_ff` | Pulse 1's request: 8 for a step ($D23E, and $D73E for every move with the hammer) or a rung ($D505), 4 for a jump ($D5E2) |
//!
//! Optional parameters, for a source whose program draws him otherwise
//! (Donkey Kong: Original Edition's hooks into its lower bank):
//!
//! | Parameter | What it does |
//! |---|---|
//! | `climb_end_pose`, `climb_end_from_a`, `climb_end_from_b` | When a climb's end ($D4E5) finds `mario_pose` at one of the two pictures `climb_end_from_a` and `climb_end_from_b` (the climb's, $0201 at $24 or $26), he is drawn at once with `climb_end_pose`, unmirrored ($EAE1, $EAD4, $EACD, $F082 from the hook at $8C0F) |
//! | `clear_mirror_x` | The stage clear's standing picture is mirrored only with his x at this or more (his sprite's x 128 and up, $0203 bit 7), else unmirrored ($F082 or $F088 from the hook at $839E); without it, always mirrored ($F088) |
//!
//! The hook's words: `ext[0]` and `ext[1]` Mario's x and y as the last
//! frame left them, `ext[2]` bit 0 on the ground then, bit 1 climbing
//! then, bit 2 set once the words hold a frame, bit 3 holding the hammer
//! then; `ext[3]` 1 in a jump's air and 2 in its landing hold.

use gp_sim::world::Body;
use gp_sim::{Ext, ExtCx};

/// The step and rung sound ($FF = 8).
const STEP: i64 = 8;
/// The jump's sound ($FF = 4).
const JUMP: i64 = 4;
/// The smash's sound ($FF = 2), which the barrels' routine asks for after
/// Mario's: nothing of his overwrites it.
const SMASH: i64 = 2;

const GROUND: i64 = 1;
const CLIMB: i64 = 2;
const HELD: i64 = 4;
const HAMMER: i64 = 8;

/// The CPU address the lifted table `rom` starts at.
const ROM_BASE: i64 = 0xC127;
/// 25m's ladder tops ($D50A: the pointer at $C48B for 25m): rows of x, y
/// and a shape, ended by $FE.
const TOPS: i64 = 0xC127;
/// The ladder tops' boxes, by the shape byte: left, top, right, bottom.
const TOP_SHAPES: i64 = 0xC143;
/// The pictures of the top zone's steps.
const TOP_POSES: i64 = 0xC147;
/// The pictures of the climb's steps.
const STEP_POSES: i64 = 0xC159;
/// The jump's picture in the air ($D5F1) and at the landing ($D5F9).
const JUMP_POSE: i64 = 0x28;
const LAND_POSE: i64 = 0x2C;
/// Standing, drawn at a landing hold's end ($D61B).
const STAND_POSE: i64 = 4;
/// The hammer's pictures by the swing $9F, 1 to 6: the lifted table $D6F6
/// reads ($C1A2, in the model's `ext_tables.tables`).
const SWING_POSES: u16 = 0xC1A2;
const SWINGS: i64 = 6;

/// $43 in a death's wait as `death_wait` counts it: 132 ($84) is 29 wraps
/// from the restart.
const WAIT_SKIP: i64 = 29;

/// A death's wait on any stage but 25m: the wait routine ($CAC9) takes
/// one more from $43 on the frame it reaches $84 ($CAD2, $CB15), so the
/// wait is a divider wrap shorter than 25m's: `death_wait` goes from 29
/// to 28, and `setup_wait` with it, on the wrap that makes it 29 (m75
/// clip 11, frame 1797: $43 133 to 131).
fn death_wait_skip(cx: &mut ExtCx<'_>) {
    if cx.counter("stage").unwrap_or(0) != 0
        && cx.state_at_start("game", "dying")
        && cx.counter("death_wait") == Some(WAIT_SKIP)
    {
        cx.set_counter("death_wait", WAIT_SKIP - 1);
        // The wraps to the stage setup count the same $43.
        if let Some(n) = cx.counter("setup_wait").filter(|n| *n > 0) {
            cx.set_counter("setup_wait", n - 1);
        }
    }
}

/// The `dk_mario` hook.
pub struct Mario;

impl Ext for Mario {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let Some(mario) = cx
            .world
            .bodies
            .iter()
            .find(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
            .copied()
        else {
            cx.me.ext[2] = 0;
            return;
        };
        let mario = spin_start(cx, mario);
        spin_pose(cx, &mario);
        // While a state stops the bodies (a death, a smash, a pause, the
        // stage clear) Mario's routine doesn't run.
        if cx.system_stopped {
            death_wait_skip(cx);
            return;
        }
        // A body new this frame (the level's start or a restart, where
        // this one is placed again) has no last frame: Mario's was his
        // spawn, standing.
        let mut was = cx.me.ext;
        let fresh = was[2] & HELD == 0;
        if fresh
            && let Some(sp) = cx.spec.levels[cx.world.level]
                .spawns
                .iter()
                .find(|s| s.kind == mario.kind)
        {
            was = [sp.x.floor(), sp.y.floor(), HELD | GROUND, 0];
        }
        let mario = climb_by_stops(cx, mario, was);
        let mario = moving_ladder_stop(cx, mario, was);
        cx.me.ext[0] = seen_x(cx, &mario);
        cx.me.ext[1] = mario.y.floor();
        cx.me.ext[2] = HELD
            | if mario.on_ground { GROUND } else { 0 }
            | if mario.climbing { CLIMB } else { 0 }
            | if hammer(cx) == Some(1) { HAMMER } else { 0 };
        if fresh {
            // The stage setup draws him standing, unmirrored, whichever
            // way he faces ($57 is kept through a restart, run.face_way;
            // $CC34 -> $CCC1 draws tile 4 at $0200). A new body takes its
            // last frame from its spawn above, so this tests the hook's
            // own words.
            set_unface(cx, i64::from(mario.facing_left));
        }
        if was[2] & HELD == 0 {
            return;
        }
        keep_mirroring(cx, &mario, fresh);
        let mut walk_pose = cx.counter("walk_pose").unwrap_or(0);
        if let Some(v) = request(cx, &mario, was, &mut walk_pose)
            && cx.counter("sound_ff") != Some(SMASH)
        {
            cx.set_counter("sound_ff", v);
        }
        // Reaching the goal clears the stage, which draws him standing,
        // mirrored whatever way he faces, that frame ($D074-$D07F).
        // The climb that gets there ends on the girder ($D4E5-$D4EB).
        if at_goal(cx, &mario) {
            set_pose(cx, STAND_POSE);
            let mirrored =
                !cx.has_param("clear_mirror_x") || mario.x.floor() >= p(cx, "clear_mirror_x");
            set_unface(
                cx,
                i64::from(if mirrored {
                    !mario.facing_left
                } else {
                    mario.facing_left
                }),
            );
            cx.set_counter("climb_step", 0);
            cx.set_counter("climb_top", 0);
            cx.set_counter("walk_turn", 0);
            return;
        }
        pose(cx, &mario, was, walk_pose);
    }
}

/// A start inside the death spin: the spin ends on the 17th count of its
/// step word (4 or 12), 120 frames after the first ($D0C0: the first sets
/// $98 to 1, each later one draws the next picture at $0201, $70 to $78
/// and $6C, and $98 goes up as it comes back to $6C; at 5 the spin is
/// over). The model's `spinning` state ends 121 frames after it was
/// entered, so a start, which enters it then, sets its time from where the
/// spin is: the counter `spin_start` (the start's $98, which play never
/// sets) and the picture (`mario_pose`, $0201) give the counts done, the
/// step word the frames since the last. Within a count's 7 or 8 frames is
/// enough, as the end waits for a count.
fn spin_start(cx: &mut ExtCx<'_>, mut mario: Body) -> Body {
    let phase = cx.counter("spin_start").unwrap_or(0);
    if phase <= 0 {
        return mario;
    }
    cx.set_counter("spin_start", 0);
    let spinning = cx.spec.kinds[mario.kind]
        .machine
        .as_ref()
        .and_then(|m| m.states.get(mario.state))
        .is_some_and(|st| st.name == "spinning");
    if !spinning || phase > 4 {
        return mario;
    }
    let tile = cx.counter("mario_pose").unwrap_or(0);
    let counts = ((phase - 1) * 4 + 1 + ((tile - 0x6C) / 4).max(0)).max(1);
    let word = mario.ext[0].rem_euclid(15);
    let since = if (4..12).contains(&word) { word - 4 } else { (word - 12).rem_euclid(15) };
    mario.time = (1 + (15 * (counts - 1)) / 2 + since) as u32;
    if let Some(i) = cx.world.bodies.iter().position(|b| b.alive && b.kind == mario.kind) {
        cx.set_body(i, mario);
    }
    mario
}

/// The death spin's pictures ($D0F8-$D11F): the routine at $D0C8 draws
/// on a count of the spin (its step word at 4 or 12) after the first, when
/// the death's wait $3A is out or at 14 or more, the picture after the one
/// $0201 holds: $70 after any below $6C, then $74, $78 and $6C in turn,
/// unmirrored whichever way he faces. Until that count he shows the
/// picture he died with. The counts and the wait are the `dk_sound`
/// hook's (`sound_death_seen`, `sound_death_wait`), which runs before this
/// one in the frame: the first count leaves the wait at $3F, and a count
/// that draws leaves it at 0. Lying still ($7C) is the state `down`'s
/// picture.
fn spin_pose(cx: &mut ExtCx<'_>, mario: &Body) {
    let Some(m) = cx.spec.kinds[mario.kind].machine.as_ref() else {
        return;
    };
    let spinning = m
        .states
        .get(mario.state)
        .is_some_and(|st| st.name == "spinning");
    if !spinning || mario.time < 1 {
        return;
    }
    let count = matches!(mario.ext[0] % 15, 4 | 12);
    let seen = cx.counter("sound_death_seen").unwrap_or(0) != 0;
    let wait = cx.counter("sound_death_wait").unwrap_or(0);
    if !(count && seen && wait == 0) {
        return;
    }
    let was = cx.counter("mario_pose").unwrap_or(0).max(0x6C);
    let now = if was + 4 >= 0x7C { 0x6C } else { was + 4 };
    cx.set_counter("mario_pose", now);
    set_unface(cx, i64::from(mario.facing_left));
}

/// The request Mario's routine made this frame, if any, with the walk's
/// beat and picture stepped as the walk steps them (`walk_pose` is the
/// picture after this frame: a counter set in a hook reads back only on
/// the next frame).
fn request(cx: &mut ExtCx<'_>, mario: &Body, was: [i64; 4], walk_pose: &mut i64) -> Option<i64> {
    let moved_x = seen_x(cx, mario) != was[0];
    let moved_y = mario.y.floor() != was[1];
    let grounded = was[2] & GROUND != 0;
    let climbed = was[2] & CLIMB != 0;
    // A rung: each move up or down a ladder ($D4EE, $D4F9) asks when the
    // sprite's new y ($0200, Mario's y less 5) has bits 1 and 2 clear. The
    // grab's first move up comes with the snap across ($D2BE).
    if (climbed || mario.climbing) && moved_y {
        return ((mario.y.floor() - 5) & 6 == 0).then_some(STEP);
    }
    // The jump's first frame ($D5E2, with $94 still 0).
    if grounded && !mario.on_ground && !mario.climbing && mario.vy.floor() < 0 {
        return Some(JUMP);
    }
    // A move across that finds ground. A walk off the girder's end returns
    // before the sound ($D22E, $D737).
    if !(grounded && mario.on_ground && !mario.climbing && moved_x) {
        return None;
    }
    match hammer(cx) {
        Some(1) => Some(STEP),
        Some(_) => None,
        None => {
            let beat = cx.counter("walk_beat").unwrap_or(0);
            cx.set_counter("walk_beat", 1 - beat);
            if beat == 0 {
                return None;
            }
            // $D246-$D273: 4 goes to 8 or 0 by $85, 0 and 8 go to 4.
            let at = *walk_pose;
            let turn = cx.counter("walk_turn").unwrap_or(0);
            let (next, turn) = match at {
                0 => (4, 0),
                8 => (4, 1),
                _ => (if turn == 0 { 8 } else { 0 }, turn),
            };
            cx.set_counter("walk_pose", next);
            *walk_pose = next;
            cx.set_counter("walk_turn", turn);
            Some(STEP)
        }
    }
}

/// The picture the frame's routine drew, and the climb's steps.
fn pose(cx: &mut ExtCx<'_>, mario: &Body, was: [i64; 4], walk_pose: i64) {
    let climbed = was[2] & CLIMB != 0;
    let moved_y = mario.y.floor() != was[1];
    // Up or Down at a ladder grabs it ($D28B) and the climb routine runs
    // the same frame; with no move it finds the ground and stands him
    // again ($D4CF), clearing the climb's steps and $85 as a climb's end
    // does.
    // Not in a jump's landing hold, which returns before the test, and
    // not with the hammer: its routine ($D6C6) runs in place of the walk's
    // and never tests a ladder, nor on the frame it drops the hammer
    // ($D7BF).
    if !climbed
        && cx.me.ext[3] == 0
        && hammer(cx) != Some(1)
        && was[2] & HAMMER == 0
        && !mario.climbing
        && mario.on_ground
        && was[2] & GROUND != 0
        && (cx.held("up") || cx.held("down"))
        && at_ladder(cx, was[0], was[1])
    {
        climb_end(cx, mario.facing_left);
    }
    if climbed || mario.climbing {
        if moved_y {
            // The game keeps y in a byte, so a climb down through the floor
            // wraps from 255 to 0 ($0200, climb.down_through): the move's
            // way is its sign modulo 256.
            let dy = (mario.y.floor() - was[1] + 128).rem_euclid(256) - 128;
            climb_move(cx, dy < 0, was[0], was[1], mario.facing_left);
        }
        // $D4CF: on a girder the climb ends and its steps are cleared.
        if !mario.climbing {
            climb_end(cx, mario.facing_left);
        }
        return;
    }
    use gp_sim::systems::LAND_HOLD_WORD;
    // A fall off a girder's end ($96 8) draws him every frame, mirrored by
    // the facing $57, from the first tile $0201 as it is: a picture drawn
    // with the other mirroring (`pose_unface` 1) is then read 2 tiles
    // over. dk.hammer.013: standing $04 unmirrored while $57 is 2 (the
    // hammer's drop draws nothing), then from original frame 1921 the
    // fall draws $04 $05 / $02 $03 mirrored (1922's sprites). The mapping
    // reads $0201 as the picture, 4 here as before, so the shift is
    // presentation: `pose_unface` 2 (2 tiles back, facing left) or 3 (2
    // on, facing right), which the player's sprite clips draw.
    if !mario.on_ground
        && mario.air != u32::MAX
        && cx.me.ext[3] == 0
        && hammer(cx) != Some(1)
        && was[2] & HAMMER == 0
        && cx.counter("pose_unface") == Some(1)
    {
        set_unface(cx, if mario.facing_left { 2 } else { 3 });
    }
    // A jump draws its picture every frame in the air ($D5F1) and the
    // landing's when it lands ($D5F9); the landing hold's end draws him
    // standing ($D61B), before a hammer touched in the jump is taken.
    if !mario.on_ground && mario.air == u32::MAX {
        cx.me.ext[3] = 1;
        set_pose(cx, JUMP_POSE);
        return;
    }
    if cx.me.ext[3] == 1 && mario.on_ground {
        cx.me.ext[3] = 2;
        set_pose(cx, LAND_POSE);
        return;
    }
    if cx.me.ext[3] == 2 {
        if mario.ext[LAND_HOLD_WORD] == 0 {
            cx.me.ext[3] = 0;
            set_pose(cx, STAND_POSE);
        }
        return;
    }
    let held = hammer(cx);
    // A move off the girder's end with the hammer ($D737, then $D7BF)
    // draws nothing; the hammer's hook sees the walk off on the next frame.
    if held == Some(1) && !mario.on_ground {
        return;
    }
    if held == Some(1) {
        // The hammer's routine draws only on the frames the walk's step
        // pattern moves ($D6D3 calls $D9E8, and returns at $D6D8 on a 0),
        // so the frame after a pickup can keep the standing picture. With
        // a smoothed walk the beat is the hammer's own (`hammer_beat`).
        let (cycle, _) = crate::hammer_beat(cx, mario.kind);
        let n = mario.ext[gp_sim::systems::STEP_WORD];
        if !cycle.is_empty() && cycle[n.rem_euclid(cycle.len() as i64) as usize] == 0 {
            return;
        }
        let swing = cx.counter("swing").unwrap_or(1);
        if (1..=SWINGS).contains(&swing) {
            let pose = crate::rom_at(cx, SWING_POSES + (swing - 1) as u16);
            set_pose(cx, pose);
        }
        return;
    }
    // The frame the hammer goes ($D7BF, by its timer or a walk off the
    // girder's end) draws nothing.
    if was[2] & HAMMER != 0 {
        return;
    }
    // The walk draws its picture on every frame Left or Right is held
    // alone ($D1CF, $D1E5-$D1F0, $D275), moved or not; standing it draws
    // nothing.
    // With `run.both_left`, Left and Right together latch no way ($56 0)
    // and draw nothing.
    let lone = !(cx.held("up") || cx.held("down"));
    let both = cx.held("left") && cx.held("right");
    let both_left = cx.spec.kinds[mario.kind]
        .run
        .as_ref()
        .is_some_and(|r| r.both_left);
    // Not on the frame a fall off a girder's end lands: the fall's routine
    // runs in its place ($96 8) and draws the fall's picture
    // (dk.hammer.013, original frames 1930 to 1940: the landing kills him
    // and $0201 stays $04 with Left held).
    let landed = was[2] & (GROUND | CLIMB) == 0;
    if mario.on_ground
        && !landed
        && lone
        && (cx.held("left") || cx.held("right"))
        && !(both && both_left)
    {
        set_pose(cx, walk_pose);
    }
}

/// A move up or down a ladder from (`x`, `y`): the step in the top zone
/// when Mario's feet are in one ($D50A), else the climb's step.
fn climb_move(cx: &mut ExtCx<'_>, up: bool, x: i64, y: i64, facing_left: bool) {
    let top = cx.counter("climb_top").unwrap_or(0);
    let step = cx.counter("climb_step").unwrap_or(0);
    if in_top_zone(cx, x, y) {
        let top = match (up, top) {
            (true, 0) => 1,
            (true, t) => (t + 1).min(16),
            (false, 0) => 13,
            (false, t) => (t - 1).max(1),
        };
        cx.set_counter("climb_top", top);
        cx.set_counter("climb_step", if up { 0 } else { 3 });
        let p = rom(cx, TOP_POSES + top - 1);
        set_climb_pose(cx, p, facing_left);
    } else {
        let step = match step {
            0 if up => 2,
            0 => 1,
            s if s + 1 > 6 => 1,
            s => s + 1,
        };
        cx.set_counter("climb_step", step);
        cx.set_counter("climb_top", 0);
        let p = rom(cx, STEP_POSES + step - 1);
        set_climb_pose(cx, p, facing_left);
    }
}

/// A climb's move as the game makes it ($D39C-$D3B8, $D445-$D45D): on a
/// count of the step counter that moves, up goes a pixel unless his feet
/// meet one of the stage's stop boxes ($D53B: the level's `ladder_stops`,
/// the list at $C161), where the broken ladders end; down always goes. The climb system holds him by the ladder tiles under
/// his feet, which end a few pixels from the boxes on some broken ladders,
/// so a move it made or didn't make that the game's rule doesn't is put
/// right here, before the rest of the hook reads where he is.
fn climb_by_stops(cx: &mut ExtCx<'_>, mut mario: Body, was: [i64; 4]) -> Body {
    let stops = !cx.spec.levels[cx.world.level].ladder_stops.is_empty();
    if was[2] & CLIMB == 0 || !mario.climbing || !stops || stage_tops(cx).is_some() {
        return mario;
    }
    let Some(cl) = cx.spec.kinds[mario.kind].climb.as_ref() else {
        return mario;
    };
    let [up, down] = gp_sim::systems::climb_ways(cx.spec, cx.world, &cx.spec.kinds[mario.kind]);
    if !up && !down {
        return mario;
    }
    let cycle = if up { &cl.up_cycle } else { &cl.down_cycle };
    if cycle.is_empty() {
        return mario;
    }
    let n = mario.ext[gp_sim::systems::STEP_WORD].rem_euclid(cycle.len() as i64);
    let step = cycle[n as usize];
    let y = if up && step != 0 && at_stop(cx, was[0], was[1]) {
        was[1]
    } else if up {
        was[1] - step
    } else {
        was[1] + step
    };
    let now = mario.y.floor();
    if now == y || (now - y).abs() > 1 || !(0..=255).contains(&y) {
        return mario;
    }
    mario.y = gp_sim::Fx::from_int(y);
    if let Some(pid) = cx
        .world
        .bodies
        .iter()
        .position(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
    {
        cx.set_body(pid, mario);
    }
    mario
}

/// Original Edition's 50m: the hook $890F at $D39C, in the climb up off a
/// girder's ladder ($5A 0, Up held). On a level with the parameter
/// `mladder_stop_<level>` (the code tests $53 = 2), while the moving ladder
/// on Mario's half of the screen (his sprite x below `mladder_half`, the
/// left one, $8930) isn't up (its state $048E,X, the `mladder` body's word
/// 0) and his sprite y ($0200, before the move) is below
/// `mladder_stop_<level>`, the climb goes to $D4CF as with no Up: he
/// doesn't move and the climb's count ($D9E8) doesn't step. So he stays
/// on the ladder until it is up again (worker dk50b's probe: up the left
/// moving ladder to sprite y 73, then Up as it starts down at frame 3871,
/// stays at 71 from 3877).
fn moving_ladder_stop(cx: &mut ExtCx<'_>, mut mario: Body, was: [i64; 4]) -> Body {
    let Some(name) = cx.spec.levels.get(cx.world.level).map(|l| &l.name) else {
        return mario;
    };
    let at = format!("mladder_stop_{name}");
    if !cx.has_param(&at) || was[2] & CLIMB == 0 || !mario.climbing {
        return mario;
    }
    let Some(cl) = cx.spec.kinds[mario.kind].climb.as_ref() else {
        return mario;
    };
    let [up, down] = gp_sim::systems::climb_ways(cx.spec, cx.world, &cx.spec.kinds[mario.kind]);
    if !up || down || was[1] - p(cx, "sprite_dy") >= p(cx, &at) {
        return mario;
    }
    let Some(k) = cx.kind("mladder") else {
        return mario;
    };
    let mut ladders: Vec<&Body> = cx
        .world
        .bodies
        .iter()
        .filter(|b| b.alive && b.kind == k)
        .collect();
    ladders.sort_by_key(|b| b.x.floor());
    let right = mario.x.floor() - p(cx, "sprite_dx") >= p(cx, "mladder_half");
    let Some(ladder) = ladders.get(usize::from(right)) else {
        return mario;
    };
    if ladder.ext[0] == 0 {
        return mario;
    }
    mario.y = gp_sim::Fx::from_int(was[1]);
    if !cl.up_cycle.is_empty() {
        let w = gp_sim::systems::STEP_WORD;
        mario.ext[w] = (mario.ext[w] - 1).rem_euclid(cl.up_cycle.len() as i64);
    }
    if let Some(pid) = cx
        .world
        .bodies
        .iter()
        .position(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
    {
        cx.set_body(pid, mario);
    }
    mario
}

/// Whether Mario's feet box at (`x`, `y`) meets one of the stage's stop
/// boxes, edges included ($D53B, $D8AD).
fn at_stop(cx: &ExtCx<'_>, x: i64, y: i64) -> bool {
    let feet = [
        p(cx, "feet_x0"),
        p(cx, "feet_y0"),
        p(cx, "feet_x1"),
        p(cx, "feet_y1"),
    ];
    let (fx0, fy0, fx1, fy1) = (x + feet[0], y + feet[1], x + feet[2], y + feet[3]);
    // The level's stops (`level.ladder_stops`: the list at $C161, its box
    // at $C16E).
    cx.spec.levels[cx.world.level]
        .ladder_stops
        .iter()
        .any(|l| fx1 >= l[0] && l[2] >= fx0 && fy1 >= l[1] && l[3] >= fy0)
}

/// The stage's own ladder tops, when the hook has them: the parameter
/// `tops_at_<level>` (the list's CPU address) and the table
/// `tops_<level>`, by the name of the level being played, as $D519 picks
/// the list by $53. Such a stage's ladders are whole, so no climb stops
/// short there. They go by the level, not the counter `stage`, so a stage
/// the stages list's stand-in plays (25m in 50m's place) climbs by the
/// stand-in's ladders.
fn stage_tops(cx: &ExtCx<'_>) -> Option<(i64, String)> {
    let stage = &cx.spec.levels.get(cx.world.level)?.name;
    let at = format!("tops_at_{stage}");
    cx.has_param(&at)
        .then(|| (p(cx, &at), format!("tops_{stage}")))
}

/// Whether Mario's feet box at (`x`, `y`) meets a ladder top's box, edges
/// included ($D50A, $D8AD).
fn in_top_zone(cx: &ExtCx<'_>, x: i64, y: i64) -> bool {
    // A stage with its own list (the pointer at $C48B for its $53,
    // $D519), lifted from that address; the boxes are $C143's on every
    // stage ($D523).
    if let Some((base, table)) = stage_tops(cx) {
        let list = |at: i64| {
            usize::try_from(at - base)
                .ok()
                .and_then(|i| cx.table(&table).get(i).copied())
                .unwrap_or(-1)
        };
        return meets_in(cx, &list, base, TOP_SHAPES, x, y);
    }
    meets(cx, TOPS, TOP_SHAPES, x, y)
}

/// Whether Mario's feet box at (`x`, `y`) meets a ladder's box, edges
/// included, from the level's ladders (`level.ladders`), as the ladder
/// routine's test does ($D28B, $D8AD).
fn at_ladder(cx: &ExtCx<'_>, x: i64, y: i64) -> bool {
    let feet = [
        p(cx, "feet_x0"),
        p(cx, "feet_y0"),
        p(cx, "feet_x1"),
        p(cx, "feet_y1"),
    ];
    let (fx0, fy0, fx1, fy1) = (x + feet[0], y + feet[1], x + feet[2], y + feet[3]);
    cx.spec.levels[cx.world.level]
        .ladders
        .iter()
        .any(|l| fx1 >= l[0] && l[2] >= fx0 && fy1 >= l[1] && l[3] >= fy0)
}

/// Whether Mario's feet box at (`x`, `y`) meets a box of the list at
/// `list` (rows of x, y and a shape byte into `shapes`, ended by $FE),
/// edges included ($D8AD).
fn meets(cx: &ExtCx<'_>, list: i64, shapes: i64, x: i64, y: i64) -> bool {
    meets_in(cx, &|at| rom(cx, at), list, shapes, x, y)
}

/// [`meets`], with the list's rows read through `row_byte`.
fn meets_in(
    cx: &ExtCx<'_>,
    row_byte: &dyn Fn(i64) -> i64,
    list: i64,
    shapes: i64,
    x: i64,
    y: i64,
) -> bool {
    let feet = [
        p(cx, "feet_x0"),
        p(cx, "feet_y0"),
        p(cx, "feet_x1"),
        p(cx, "feet_y1"),
    ];
    let (fx0, fy0, fx1, fy1) = (x + feet[0], y + feet[1], x + feet[2], y + feet[3]);
    let mut at = list;
    loop {
        let bx = row_byte(at);
        if bx == 0xFE || bx < 0 {
            return false;
        }
        let by = row_byte(at + 1);
        let shape = shapes + row_byte(at + 2);
        let (l, t) = (bx + rom(cx, shape), by + rom(cx, shape + 1));
        let (r, b) = (bx + rom(cx, shape + 2), by + rom(cx, shape + 3));
        if fx1 >= l && r >= fx0 && fy1 >= t && b >= fy0 {
            return true;
        }
        at += 3;
    }
}

/// A climb's end ($D4E5): its steps and the walk's turn cleared, and with
/// `climb_end_pose`, a climbing picture in $0201 replaced at once by that
/// one, unmirrored.
fn climb_end(cx: &mut ExtCx<'_>, facing_left: bool) {
    cx.set_counter("climb_step", 0);
    cx.set_counter("climb_top", 0);
    cx.set_counter("walk_turn", 0);
    if cx.has_param("climb_end_pose") {
        let pose = cx.counter("mario_pose");
        if pose == Some(p(cx, "climb_end_from_a")) || pose == Some(p(cx, "climb_end_from_b")) {
            set_pose(cx, p(cx, "climb_end_pose"));
            set_unface(cx, i64::from(facing_left));
        }
    }
}

fn set_pose(cx: &mut ExtCx<'_>, p: i64) {
    if cx.counter("mario_pose") != Some(p) {
        cx.set_counter("mario_pose", p);
    }
    set_unface(cx, 0);
}

/// A climb's picture, which the game draws with its own mirroring
/// whichever way Mario faces ($F096): `pose_unface` is 1 when he faces
/// left, so the presentation undoes the facing flip.
fn set_climb_pose(cx: &mut ExtCx<'_>, p: i64, facing_left: bool) {
    set_pose(cx, p);
    set_unface(cx, i64::from(facing_left));
}

/// A stage's start or a restart places the bodies again, and on that frame
/// the player's systems don't run, so neither does the `dk_mario` hook,
/// and the level's counters have put `pose_unface` back to 0. The stage
/// setup draws Mario standing unmirrored whichever way he faces ($CC34 ->
/// $CCC1: tile 4 at $0200, $57 kept through a restart), so a hook that runs
/// on every frame (`dk_idle`, `hook_last`) turns the facing flip over
/// then, as this hook does on the next frame. The `mario` body's words
/// hold no frame (`ext[2]` without HELD) only when it was just placed: a
/// clip's start loads HELD.
pub(crate) fn unface_placed(cx: &mut ExtCx<'_>) {
    let placed = cx.world.bodies.iter().any(|b| {
        b.alive && cx.spec.kind(b).ext.as_deref() == Some("dk_mario") && b.ext[2] & HELD == 0
    });
    if !placed {
        return;
    }
    let Some(left) = cx
        .world
        .bodies
        .iter()
        .find(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
        .map(|b| b.facing_left)
    else {
        return;
    };
    set_unface(cx, i64::from(left));
}

/// A death's wait that Mario didn't die in (Start in the demo, or a
/// game's start): its 21st-from-the-end wrap draws the stage's sprites
/// from its start ($CAE7 -> $CC34 -> $CCC1), Mario standing, tile 4,
/// unmirrored whichever way he faced, and nothing draws him again until
/// the stage starts. The `respawn` there moves the body; this sets his
/// picture, from the frame the wait is at 11 (`death_wait` 1 to 11) while
/// the game is `dying` and he isn't lying in `down`, which draws its own
/// (dk.title.009, original frames 1104 to 1225: the demo's walk picture
/// kept).
pub(crate) fn stand_placed(cx: &mut ExtCx<'_>) {
    let wait = cx.counter("death_wait").unwrap_or(0);
    if !(1..=11).contains(&wait) || !game_in(cx, "dying") {
        return;
    }
    let Some(mario) = cx
        .world
        .bodies
        .iter()
        .find(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)
        .copied()
    else {
        return;
    };
    let lying = cx.spec.kinds[mario.kind]
        .machine
        .as_ref()
        .and_then(|m| m.states.get(mario.state))
        .is_some_and(|st| st.name == "down");
    if lying {
        return;
    }
    if cx.counter("mario_pose") != Some(4) {
        cx.set_counter("mario_pose", 4);
    }
    let unface = i64::from(mario.facing_left);
    if cx.counter("pose_unface").is_some_and(|u| u != unface) {
        set_unface(cx, unface);
    }
}

/// Whether the game's machine is in `state` now.
fn game_in(cx: &ExtCx<'_>, state: &str) -> bool {
    cx.spec
        .machines
        .iter()
        .zip(&cx.world.machines)
        .any(|(m, s)| m.name == "game" && m.states.get(s.state).is_some_and(|st| st.name == state))
}

/// A turn with nothing drawn: Left or Right sets the facing $57 ($D175)
/// on frames whose routine draws no picture, which keeps the mirroring it
/// was drawn with ($0202): on a ladder without a move (the climb's picture
/// has its own, $F096; dk.m75.046, original frames 4867 on: $57 2, $0202
/// $00), or holding the hammer between the walk's step frames (its routine
/// draws only on those, $D6D3; dk.crowded-top.003, original frames 1358
/// and 1359: $57 1, $0202 $40 until 1360's swing). So when the facing
/// differs from the last frame's, `pose_unface` turns over here, before
/// this frame's picture, which sets it afresh if one is drawn. `pose_faced`
/// keeps the facing: 1 right, 2 left, 0 unknown (a start, a stage's
/// start, or a body placed this frame).
fn keep_mirroring(cx: &mut ExtCx<'_>, mario: &Body, fresh: bool) {
    let Some(faced) = cx.counter("pose_faced") else {
        return;
    };
    let now = 1 + i64::from(mario.facing_left);
    let unface = cx.counter("pose_unface").unwrap_or(0);
    if !fresh && faced != 0 && faced != now && (0..=1).contains(&unface) {
        set_unface(cx, 1 - unface);
    }
    if faced != now {
        cx.set_counter("pose_faced", now);
    }
}

fn set_unface(cx: &mut ExtCx<'_>, v: i64) {
    if cx.counter("pose_unface").is_some() {
        cx.set_counter("pose_unface", v);
    }
}

fn p(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.param(name).floor()
}

/// The lifted byte at CPU address `at`, or -1 past the table.
fn rom(cx: &ExtCx<'_>, at: i64) -> i64 {
    usize::try_from(at - ROM_BASE)
        .ok()
        .and_then(|i| cx.table("rom").get(i).copied())
        .unwrap_or(-1)
}

/// Mario's x as the frame will leave it: a camera's `keep_in_view` runs
/// after the hooks and holds him at its bound, where the walk routine
/// doesn't move him ($D990), the bound of his girder's row where
/// `keep_in_view_rows` gives one (Kong's girder on 25m), or in a jump
/// sent back from it on the same frame ($D547 calls $D990 before it moves).
fn seen_x(cx: &ExtCx<'_>, mario: &Body) -> i64 {
    gp_sim::systems::kept_x(cx.spec, cx.world, mario)
}

/// Whether Mario is at the level's goal, as the player's bounds test
/// (`gp_sim::systems::bounds`) has it this frame (`level.goal`).
fn at_goal(cx: &ExtCx<'_>, mario: &Body) -> bool {
    // A body climbing down through (`climb.down_through`) wraps instead,
    // and the bounds test skips it.
    let through = mario.climbing
        && cx.spec.kinds[mario.kind]
            .climb
            .as_ref()
            .is_some_and(|c| c.down_through > 0);
    if through {
        return false;
    }
    let level = &cx.spec.levels[cx.world.level];
    level.goal_x.is_some_and(|g| mario.x >= g) && !level.goal_under.is_some_and(|u| mario.y >= u)
}

/// The word 0 of a hammer that Mario holds (1) or has just dropped at a
/// girder's end (2), if any.
fn hammer(cx: &ExtCx<'_>) -> Option<i64> {
    let k = cx.kind("hammer")?;
    cx.world
        .bodies
        .iter()
        .find(|b| b.kind == k && b.alive && b.ext[0] != 0)
        .map(|b| b.ext[0])
}
