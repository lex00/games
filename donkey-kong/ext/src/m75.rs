//! Donkey Kong's 75m: the elevators ($E834) and the springs ($E981), which
//! the stage dispatch $CF01 runs in that order every frame of play, after
//! the prizes ($EE79, from the play routine's shared front). 75m's prizes
//! are the level's `pickups` (platformer `level.pickups`, as 100m's), with
//! the `dk_taken` hook's points and jingle.
//!
//! The `dk_prizes` hook here is Original Edition's 50m's prize test: a
//! prize is taken ($EEF0) on a frame Mario's step beat $BE is set, when
//! his sprite stands exactly on one of the stage's places (the lifted
//! $C5AE and $C5C2 from the index $C5FA[$53], three entries) not taken yet
//! ($C9 + entry): its flag set, 800 points ($CFC6 with X = 3, the event
//! `prize.taken`) and the jingle request $FD = $20. The hook takes $BE as
//! a frame Mario moved across or a climb's step moved him, which the
//! general option doesn't (a climb takes no pickup), and 50m takes one at
//! a ladder's top (m50-girder3 clip 1, frame 3337). A stage's setup clears
//! the flags ($CC47).
//!
//! The elevators are six platforms, each two sprites from $0230 (offsets
//! from $C2CC: $30, $38 and $40 the left column, rising at x 48; $48, $50
//! and $58 the right, falling at x 112), empty while the first sprite's y
//! is $FF. They move a pixel on the frames the beat $DFE8 gives (its step
//! $8A, 1 to 15 then 0, against the lifted bits $C1BC and the pair of masks
//! $C45C and $C461 by the difficulty). A left platform at y $70 makes one
//! due at the left's foot, one at y $48 goes; a right one at $A8 makes one
//! due at the right's top, one at $D0 or more goes. After the move the due
//! left one takes the first empty left slot at y $D0, else the due right
//! one the first empty right slot at $48. The counters `lift_beat`,
//! `lift_due_left` and `lift_due_right` are $8A, $D8 and $D9; a platform's
//! `ext[3]` is its slot plus one.
//!
//! Mario rides them as the level's `ground.bodies` (platformer
//! `level.ground_bodies`): when the stage's boxes hold nothing, his ground
//! test ($D326) meets a platform's box and sets $DA by its column, or
//! clears it when it meets none, and the player's `riding` names the
//! platform. $E834 moves his sprite with that column on each of its steps,
//! before the platforms. The engine carries him with a platform the hook
//! moves; once the platform has gone (at the top or the foot) the hook
//! carries him on, since $DA holds until his next ground test. Then
//! $EC29's last test, $EDC5, kills him on 75m while he walks or stands
//! ($96 = 1) with his box ($C03C + $4C) meeting one of two boxes ($C42B's
//! by the stage): one at the corner $C423 + ($53 - 1) * 2, (40, 68) by the
//! left column's top, and one at the literals $EDFC and $EE00, (112, 201)
//! under the right column's foot.
//!
//! Three slots, each a spring's four sprites at $0260, $0270 and $0280
//! ($0445 the slot, the sprite at ((slot + 3) * 16) + $30, $EFD7), empty
//! while the first sprite's y is $FF. The loop takes the slots in order:
//!
//! | A slot that is | What $E981 does |
//! |---|---|
//! | Empty | $EA34: when the throw timer $36 is 0, a spring at x 16 plus the lifted $C1FF[$19 & 3] and y $30, its start x kept in $0446,slot, and $36 again from the lifted $C457 by the difficulty. Only the first empty slot can take it, since the timer is then reloaded |
//! | Within $B0 of its start x | The bounce: the picture $C0 above y $26, else $C4, and at y $2E or more a landing (y back to $2E, the arc's step counter to 0, $FE = 2); then 2 px right and a step of the arc $EF72, the jump's, with the gravity the setup put in its slot ($50 and $03, $CCA6) |
//! | Past it | The drop ($EA01): 3 px down a frame, $FE = 1 as it leaves y $26, and gone once it was at y $D8 or more |
//!
//! A spring's words: `ext[0]` its start x ($0446,slot), `ext[1]` the
//! arc's step counter ($042C,X), `ext[2]` the arc's fraction ($042D,X) and
//! speed ($0435,X and $0436,X), a byte each, and `ext[3]` its slot plus
//! one, X being (slot + 1) * 2. Its place is its first sprite's.

use gp_sim::world::Body;
use gp_sim::{ExtCx, Fx};

use crate::{p, rom_at};

/// The arc's start: speed $0080 and fraction $F0 when its step counter is
/// 0 ($EF7B-$EF8E, a slot other than Mario's).
const ARC_SPEED: i64 = 0x80;
const ARC_FRACTION: i64 = 0xF0;

/// $EF72 for a spring: one step of the arc over the sprite's y. `arc` is
/// (step counter, fraction, speed low, speed high); returns the new y.
pub(crate) fn arc_step(y: i64, arc: &mut [i64; 4], g_lo: i64, g_hi: i64) -> i64 {
    let [count, frac, v_lo, v_hi] = arc;
    if *count == 0 {
        *v_hi = 0;
        *v_lo = ARC_SPEED;
        *frac = ARC_FRACTION;
    } else {
        // CPX #$00 leaves the carry set, so ADC #$30 adds one more.
        let lo = *v_lo + 0x30 + 1;
        *v_lo = lo & 0xFF;
        *v_hi = (*v_hi + (lo >> 8)) & 0xFF;
    }
    // SEC; SBC gravity's low byte from the fraction, its high from y with
    // the borrow; CLC; ADC the speed's low to the fraction, its high to y
    // with the carry.
    let f = *frac - g_lo;
    let borrow = i64::from(f < 0);
    let f = f & 0xFF;
    let y = (y - g_hi - borrow) & 0xFF;
    let f = f + *v_lo;
    let carry = f >> 8;
    *frac = f & 0xFF;
    let y = (y + *v_hi + carry) & 0xFF;
    *count = (*count + 1) & 0xFF;
    y
}

fn pack(arc: &[i64; 4]) -> (i64, i64) {
    (arc[0], arc[1] | arc[2] << 8 | arc[3] << 16)
}

fn unpack(count: i64, word: i64) -> [i64; 4] {
    [count, word & 0xFF, word >> 8 & 0xFF, word >> 16 & 0xFF]
}

/// What a spring does this frame, from its slot's bytes: its new place
/// (None once it's gone), its words and the sound it asks for ($FE).
pub(crate) fn spring_step(
    x: i64,
    y: i64,
    start: i64,
    arc: &mut [i64; 4],
    g: (i64, i64),
) -> (Option<(i64, i64)>, Option<i64>) {
    if (start + 0xB0) & 0xFF < x {
        // $EA01: the drop, by the y it had.
        let sound = (y == 0x26).then_some(1);
        if y >= 0xD8 {
            return (None, sound);
        }
        return (Some((x, (y + 3) & 0xFF)), sound);
    }
    let mut y = y;
    let mut sound = None;
    if y >= 0x2E {
        sound = Some(2);
        y = 0x2E;
        arc[0] = 0;
    }
    let x = (x + 2) & 0xFF;
    let y = arc_step(y, arc, g.0, g.1);
    (Some((x, y)), sound)
}

/// The picture $E981 draws a spring with this move, from its place before
/// it: true for $C0 (the state `shut`), false for $C4 (`open`). On the
/// bounce, $C0 while its y was below $26 ($E9A9); on the drop ($EA01), $C0
/// while its y was from $50 to $8F or from $C0 to $D7.
pub(crate) fn spring_shut(x: i64, y: i64, start: i64) -> bool {
    if (start + 0xB0) & 0xFF < x {
        (0x50..0x90).contains(&y) || (0xC0..0xD8).contains(&y)
    } else {
        y < 0x26
    }
}

/// Whether a body's hitbox meets the player's, edges included.
fn meets_player(cx: &ExtCx<'_>, b: &Body) -> bool {
    let Some(m) = crate::player(cx) else {
        return false;
    };
    let rect = |b: &Body| {
        let h = cx.spec.kind(b).hitbox;
        let (x, y) = (b.x.floor() + h.x.floor(), b.y.floor() + h.y.floor());
        [x, y, x + h.w.floor() - 1, y + h.h.floor() - 1]
    };
    let ([ml, mt, mr, mb], [bl, bt, br, bb]) = (rect(m), rect(b));
    ml <= br && bl <= mr && mt <= bb && bt <= mb
}

/// The springs' loop ($E981 and $EA34): one body that runs it each frame
/// of play over the `spring` bodies by their slots.
pub(crate) struct Springs;

impl gp_sim::Ext for Springs {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let Some(kind) = cx.kind("spring") else {
            return;
        };
        // The throw timer as $EA34 reads it: the value the last frame
        // ended with ($F4AC steps it after the play routine), which the
        // hook kept in `ext[0]` (a start's first run takes this frame's
        // plus one, as dk_throw does).
        let now = cx.counter("throw_timer").unwrap_or(0);
        let first = cx.me.ext[1] == 0;
        let last = if first { now + 1 } else { cx.me.ext[0] };
        cx.me.ext[1] = 1;
        if cx.system_stopped {
            cx.me.ext[0] = now;
            return;
        }
        let mut springs: Vec<(usize, Body)> = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.alive && !b.waiting && b.kind == kind)
            .map(|(i, b)| (i, *b))
            .collect();
        // A spring made last frame takes the lowest slot free ($EA43).
        for k in 0..springs.len() {
            if springs[k].1.ext[3] == 0 {
                let held: Vec<i64> = springs.iter().map(|(_, b)| b.ext[3]).collect();
                let slot = (1..=3).find(|n| !held.contains(n)).unwrap_or(1);
                let b = &mut springs[k].1;
                b.ext[3] = slot;
                b.ext[0] = b.x.floor();
            }
        }
        let g = (p(cx, "gravity_lo"), p(cx, "gravity_hi"));
        let mut timer = now;
        let mut sound = None;
        let mut spawned = false;
        for slot in 1..=3 {
            match springs.iter_mut().find(|(_, b)| b.ext[3] == slot) {
                Some((i, b)) => {
                    let (x, y) = (b.x.floor(), b.y.floor());
                    let mut arc = unpack(b.ext[1], b.ext[2]);
                    let shut = spring_shut(x, y, b.ext[0]);
                    let (place, s) = spring_step(x, y, b.ext[0], &mut arc, g);
                    sound = s.or(sound);
                    match place {
                        Some((x, y)) => {
                            b.x = Fx::from_int(x);
                            b.y = Fx::from_int(y);
                            (b.ext[1], b.ext[2]) = pack(&arc);
                            let m = cx.spec.kinds[kind].machine.as_ref();
                            let name = if shut { "shut" } else { "open" };
                            if let Some(state) =
                                m.and_then(|m| m.states.iter().position(|s| s.name == name))
                                && b.state != state
                            {
                                b.state = state;
                                b.time = 0;
                            }
                            cx.set_body(*i, *b);
                        }
                        None => cx.remove(*i),
                    }
                }
                None if !spawned && last == 0 => {
                    // $EA34: x from $C1FF by the random byte, y $30.
                    let r = i64::from(crate::draw(cx, "spring_x", 1)) & 3;
                    let x = rom_at(cx, 0xC1FF + r as u16) + 0x10;
                    cx.spawn(kind, Fx::from_int(x), Fx::from_int(0x30), false);
                    let d = crate::difficulty(cx);
                    let step = i64::from(!cx.counters_stopped);
                    timer = rom_at(cx, 0xC457 + d as u16) - step;
                    cx.set_counter("throw_timer", timer);
                    spawned = true;
                }
                None => {}
            }
        }
        if let Some(s) = sound
            && cx.counter("sound_fe").is_some()
        {
            cx.set_counter("sound_fe", s);
        }
        cx.me.ext[0] = timer;
        // The touch test ($EC44 over the springs' slots, $EC56) leaves its
        // index $5D at the first slot whose box meets Mario's, else at the
        // count, 3, which `barrel_loop` holds (kong_75m's `max`).
        let hit = (1..=3).find(|slot| {
            springs
                .iter()
                .find(|(_, b)| b.ext[3] == *slot)
                .is_some_and(|(_, b)| meets_player(cx, b))
        });
        if let Some(slot) = hit
            && cx.counter("barrel_loop").is_some()
        {
            cx.set_counter("barrel_loop", slot - 1);
        }
    }
}

/// The prizes' test ($EE79 and $EEF0): one body that runs it each frame of
/// play. `ext[0]` and `ext[2]` keep Mario's x and y as the last frame
/// ended, `ext[3]` whether he was climbing.
pub(crate) struct Prizes;

impl gp_sim::Ext for Prizes {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let Some(m) = crate::player(cx).copied() else {
            return;
        };
        let x = m.x.floor();
        // A climb's step sets $BE too: a step up or down the ladder, the
        // last one onto the girder with it (50m's m50-girder3 clip 1 takes
        // a prize at sprite (76, 127) on frame 3337 as the climb ends).
        let y = m.y.floor();
        let climbed = (m.climbing || cx.me.ext[3] != 0) && y != cx.me.ext[2];
        let moved = cx.me.ext[1] != 0 && (x != cx.me.ext[0] || climbed);
        cx.me.ext[0] = x;
        cx.me.ext[1] = 1;
        cx.me.ext[2] = y;
        cx.me.ext[3] = i64::from(m.climbing);
        if cx.system_stopped || !moved {
            return;
        }
        let (sx, sy) = (x - p(cx, "sprite_dx"), m.y.floor() - p(cx, "sprite_dy"));
        let first = rom_at(cx, 0xC5FA + p(cx, "stage") as u16);
        for k in 0..3 {
            let at = (first + k) as u16;
            let flag = format!("prize_{k}");
            if rom_at(cx, 0xC5AE + at) == sy
                && rom_at(cx, 0xC5C2 + at) == sx
                && cx.counter(&flag) == Some(0)
            {
                cx.set_counter(&flag, 1);
                // The points show at Mario's sprite ($EF18-$EF27): the event
                // names him, as the level's pickups do.
                let player = cx.world.bodies.iter().position(|b| {
                    b.alive && cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player
                });
                if let Some(e) = cx.event("prize.taken") {
                    cx.fire(e, player);
                }
                crate::award_points(cx);
                if cx.counter("sound_fd").is_some() {
                    cx.set_counter("sound_fd", 0x20);
                }
                return;
            }
        }
    }
}

/// A platform's column: 0 for the left (slots 1 to 3), 1 for the right.
fn column(slot: i64) -> usize {
    usize::from(slot > 3)
}

/// The elevators' loop ($E834): one body that runs it each frame of play
/// over the `elevator` bodies by their slots.
pub(crate) struct Elevators;

impl gp_sim::Ext for Elevators {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let Some(kind) = cx.kind("elevator") else {
            return;
        };
        // Each platform's priority as its place gives it, for one placed
        // at the start, made last frame or carried in, also while the
        // stage waits to start.
        // The clear scene's copies ($0290) keep the attributes the
        // platforms had, which the same rule gives from where they stand.
        let copy = cx.kind("elevator_copy");
        for i in 0..cx.world.bodies.len() {
            let mut b = cx.world.bodies[i];
            if b.alive
                && (b.kind == kind || Some(b.kind) == copy)
                && let Some(state) = priority(cx, b.kind, &b)
            {
                b.state = state;
                b.time = 0;
                cx.set_body(i, b);
            }
        }
        if cx.system_stopped {
            // The clear scene's step 2 ($8AB0): $53 is already 4, so it
            // copies the sprites $0230-$025F to $0290 ($8ADD) before it
            // hides 32 sprites from $0210, and the platforms stand there
            // until the barrels' clear (clear_wait 19). The copies are
            // `elevator_copy` bodies, made on the frame the scene's
            // machine enters `hide`, the one before it clears the
            // platforms.
            if let Some(copy) = cx.kind("elevator_copy")
                && entered(cx, "clear_scene", "hide")
            {
                let lifts: Vec<(Fx, Fx)> = cx
                    .world
                    .bodies
                    .iter()
                    .filter(|b| b.alive && b.kind == kind)
                    .map(|b| (b.x, b.y))
                    .collect();
                for (x, y) in lifts {
                    cx.spawn(copy, x, y, false);
                }
            }
            // A death's wait hides them with the barrels and draws the
            // stage's own again with Mario's start, on its 21st divider
            // wrap (`death_wait` 11), before the restart places them.
            let none = !cx.world.bodies.iter().any(|b| b.alive && b.kind == kind);
            // $CC34 draws the objects of the stage $53 holds then: after a
            // game over hands the turn to the other player, theirs.
            let ours = cx.kind("fire").is_none_or(|f| {
                cx.counter("stage") == Some(crate::fire::stage(cx, f) - 1)
            });
            if none
                && ours
                && cx.state_at_start("game", "dying")
                && cx.counter("death_wait") == Some(11)
            {
                for (x, y) in [
                    ("start_x0", "start_y0"),
                    ("start_x1", "start_y1"),
                    ("start_x2", "start_y2"),
                    ("start_x3", "start_y3"),
                ] {
                    if cx.has_param(x) {
                        let (x, y) = (Fx::from_int(p(cx, x)), Fx::from_int(p(cx, y)));
                        spawn_placed(cx, kind, x, y);
                    }
                }
                // The list draws 75m's two fires at their start too ($C4F6:
                // tile $98 at $0210 and $0220), the level's fire spawns.
                if let Some(fire) = cx.kind("fire") {
                    let fires: Vec<(Fx, Fx)> = cx.spec.levels[cx.world.level]
                        .spawns
                        .iter()
                        .filter(|s| s.kind == fire)
                        .map(|s| (s.x, s.y))
                        .collect();
                    // Each drawn as `tile_a`, unmirrored: the slots' pictures
                    // (`fire_tile_<slot>`, $0211 + 16 a slot) are the list's.
                    let tile = crate::fire::kp(cx, fire, "tile_a");
                    for (k, (x, y)) in fires.into_iter().enumerate() {
                        cx.spawn(fire, x, y, false);
                        cx.set_counter(&format!("fire_tile_{k}"), tile);
                    }
                }
            }
            return;
        }
        let mut lifts: Vec<(usize, Body)> = cx
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.alive && !b.waiting && b.kind == kind)
            .map(|(i, b)| (i, *b))
            .collect();
        // A platform placed at the stage's start, or made last frame,
        // takes the lowest slot free in its column.
        for k in 0..lifts.len() {
            if lifts[k].1.ext[3] == 0 {
                let held: Vec<i64> = lifts.iter().map(|(_, b)| b.ext[3]).collect();
                let col = i64::from(lifts[k].1.x.floor() >= p(cx, "right_x"));
                let slot = (1..=3)
                    .map(|n| n + 3 * col)
                    .find(|n| !held.contains(n))
                    .unwrap_or(1 + 3 * col);
                lifts[k].1.ext[3] = slot;
                // The copy was taken before this frame's priority pass
                // above, so it gets the state again, as that pass set it.
                if let Some(state) = priority(cx, kind, &lifts[k].1) {
                    lifts[k].1.state = state;
                    lifts[k].1.time = 0;
                }
                cx.set_body(lifts[k].0, lifts[k].1);
            }
        }
        // $DFE8 with $5D = 0: the step $8A, then its bit against the masks.
        let mut beat = cx.counter("lift_beat").unwrap_or(0) + 1;
        if !(0..16).contains(&beat) {
            beat = 0;
        }
        cx.set_counter("lift_beat", beat);
        let d = crate::difficulty(cx) as u16;
        let mask = if beat < 8 {
            rom_at(cx, 0xC45C + d)
        } else {
            rom_at(cx, 0xC461 + d)
        };
        let bit = rom_at(cx, 0xC1BC + (beat & 7) as u16);
        let carried = if bit & mask == 0 {
            0
        } else {
            lifts_step(cx, kind, &mut lifts)
        };
        kill_boxes(cx, carried);
    }
}

/// A platform's sprite priority ($E968 and $E971), when its state should
/// change: the move that brings a left platform (rising) to y $C8 sets
/// attribute $03, in front, and the one to $50 sets $23, behind the
/// background; a right one (falling) the reverse, $03 at $50 and $23 at
/// $C8. A new one is placed behind ($E92E, $E95B). So a left platform is
/// behind at y $50 or less and above $C8, a right one below $50 and at $C8
/// or more. The states `behind` and `front` of the platform's machine.
fn priority(cx: &ExtCx<'_>, kind: usize, b: &Body) -> Option<usize> {
    let m = cx.spec.kinds[kind].machine.as_ref()?;
    let at = |name: &str| m.states.iter().position(|s| s.name == name);
    let y = b.y.floor();
    let left = match b.ext[3] {
        0 => b.x.floor() < p(cx, "right_x"),
        slot => column(slot) == 0,
    };
    let behind = if left {
        y <= 0x50 || y > 0xC8
    } else {
        y < 0x50 || y >= 0xC8
    };
    let want = at(if behind { "behind" } else { "front" })?;
    (b.state != want).then_some(want)
}

/// Spawns a platform at (`x`, `y`) in the priority state its place gives
/// it ([`priority`]), on its first frame: the death's redraw ($CC34 at
/// `death_wait` 11) draws the stage's platforms from the object list with
/// attribute $03 where they stand (dk.m75-elevator-top.001 frame 825,
/// recording OAM $0230-$024F), and a body made by a hook is drawn the
/// frame it appears, before the hook next sets its state. The new body is
/// the next index the requests add, so a `set_body` after the spawn
/// gives it the state.
fn spawn_placed(cx: &mut ExtCx<'_>, kind: usize, x: Fx, y: Fx) {
    let spawned = cx
        .requests()
        .iter()
        .filter(|r| matches!(r, gp_sim::ext::Request::Spawn { .. }))
        .count();
    let id = cx.world.bodies.len() + spawned;
    cx.spawn(kind, x, y, false);
    let mut nb = gp_sim::systems::new_body(cx.spec, cx.world, kind, x, y, false);
    if let Some(state) = priority(cx, kind, &nb) {
        nb.state = state;
        cx.set_body(id, nb);
    }
}

/// Whether the game machine `machine` entered `state` this frame: in it
/// now and not as the frame started.
fn entered(cx: &ExtCx<'_>, machine: &str, state: &str) -> bool {
    let now = cx.spec.machines.iter().enumerate().any(|(i, m)| {
        m.name == machine
            && cx
                .world
                .machines
                .get(i)
                .and_then(|s| m.states.get(s.state))
                .is_some_and(|st| st.name == state)
    });
    now && !cx.state_at_start(machine, state)
}

/// Where Mario rides a platform of `kind`: the player's index, the
/// platform's, its column and whether it is still there.
fn rider(cx: &ExtCx<'_>, kind: usize) -> Option<(usize, usize, usize, bool)> {
    let (p, m) = cx
        .world
        .bodies
        .iter()
        .enumerate()
        .find(|(_, b)| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive)?;
    let r = (m.riding as usize).checked_sub(1)?;
    let lift = cx.world.bodies.get(r).filter(|l| l.kind == kind)?;
    Some((p, r, column(lift.ext[3]), lift.alive))
}

/// $E834 on a frame of the beat: the platforms move and Mario with his
/// column, then a due one is placed. Returns Mario's step down (-1 up, 1
/// down, 0 when he rides nothing).
fn lifts_step(cx: &mut ExtCx<'_>, kind: usize, lifts: &mut [(usize, Body)]) -> i64 {
    let ride = rider(cx, kind);
    let mut ridden_gone = false;
    {
        let mut due = [
            cx.counter("lift_due_left").unwrap_or(0),
            cx.counter("lift_due_right").unwrap_or(0),
        ];
        for slot in 1..=6 {
            let Some((i, b)) = lifts.iter_mut().find(|(_, b)| b.ext[3] == slot) else {
                continue;
            };
            let col = column(slot);
            let y = b.y.floor() + if col == 0 { -1 } else { 1 };
            let gone = if col == 0 {
                if y == 0x70 {
                    due[0] = 1;
                }
                y <= 0x48
            } else {
                if y == 0xA8 {
                    due[1] = 1;
                }
                y >= 0xD0
            };
            if gone {
                ridden_gone |= ride.is_some_and(|(_, r, _, _)| r == *i);
                cx.remove(*i);
                b.alive = false;
            } else {
                b.y = Fx::from_int(y);
                if let Some(state) = priority(cx, kind, b) {
                    b.state = state;
                    b.time = 0;
                }
                cx.set_body(*i, *b);
            }
        }
        // $E90E: the due left one first; the right waits for a frame the
        // left has none due.
        let free = |col: usize| {
            (1..=3)
                .map(|n| n + 3 * col as i64)
                .find(|n| !lifts.iter().any(|(_, b)| b.alive && b.ext[3] == *n))
        };
        if due[0] == 1 {
            if free(0).is_some() {
                let (x, y) = (Fx::from_int(p(cx, "left_x")), Fx::from_int(0xD0));
                spawn_placed(cx, kind, x, y);
                due[0] = 0;
            }
        } else if due[1] == 1 && free(1).is_some() {
            let (x, y) = (Fx::from_int(p(cx, "right_x")), Fx::from_int(0x48));
            spawn_placed(cx, kind, x, y);
            due[1] = 0;
        }
        cx.set_counter("lift_due_left", due[0]);
        cx.set_counter("lift_due_right", due[1]);
    }
    let Some((p, _, col, alive)) = ride else {
        return 0;
    };
    let dy = if col == 0 { -1 } else { 1 };
    // $DA outlives the platform: the engine carries Mario with one it
    // moves, the hook once it has gone.
    if !alive || ridden_gone {
        let mut m = cx.world.bodies[p];
        m.y = m.y.add(Fx::from_int(dy));
        cx.set_body(p, m);
    }
    dy
}

/// $EDC5 on 75m: Mario walking or standing ($96 = 1) with his box meeting
/// either of the stage's kill boxes dies. `dy` is his carry this frame,
/// which the engine applies after the hook.
fn kill_boxes(cx: &mut ExtCx<'_>, dy: i64) {
    let Some(m) = crate::player(cx).copied() else {
        return;
    };
    let mario = cx
        .world
        .bodies
        .iter()
        .find(|b| b.alive && cx.kind("mario") == Some(b.kind));
    if crate::play::mario_state(&m, mario) != 1 {
        return;
    }
    let at = |a: i64| rom_at(cx, a as u16) as u8;
    let w = |a: i64| at(a) as i64 | (at(a + 1) as i64) << 8;
    let box4 = |a: i64| [at(a), at(a + 1), at(a + 2), at(a + 3)];
    let (sx, sy) = (
        m.x.floor() - p(cx, "sprite_dx"),
        m.y.floor() + dy - p(cx, "sprite_dy"),
    );
    let mine = crate::play::corners(sx as u8, sy as u8, box4(w(0xC03C + 0x4C)));
    let k = (p(cx, "stage") - 1) * 2;
    let kill = box4(w(0xC42B + k));
    let places = [
        (at(0xC423 + k), at(0xC424 + k)),
        (p(cx, "kill_x1") as u8, p(cx, "kill_y1") as u8),
    ];
    if places
        .iter()
        .any(|&(x, y)| crate::play::first_fail(mine, crate::play::corners(x, y, kill)) == 4)
        && let Some(e) = cx.event("player.died")
    {
        let id = cx
            .world
            .bodies
            .iter()
            .position(|b| cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player && b.alive);
        cx.fire(e, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A spring's first frames from its spawn at y $30: it lands at once
    /// ($FE = 2, y back to $2E), then rises on the arc, 2 px right a frame.
    #[test]
    fn a_new_spring_lands_then_rises() {
        let mut arc = [7, 0x11, 0x22, 0x01];
        let (place, sound) = spring_step(0x10, 0x30, 0x10, &mut arc, (0x50, 0x03));
        assert_eq!(sound, Some(2));
        // $2E - 3 with no borrow ($F0 - $50), then $A0 + $80 carries 1.
        assert_eq!(place, Some((0x12, 0x2C)));
        assert_eq!(arc, [1, 0x20, 0x80, 0]);
        let (place, sound) = spring_step(0x12, 0x2C, 0x10, &mut arc, (0x50, 0x03));
        assert_eq!(sound, None);
        // Speed $80 + $31 = $B1; $20 - $50 borrows: $2C - 4, then $D0 + $B1
        // carries 1.
        assert_eq!(place, Some((0x14, 0x29)));
        assert_eq!(arc, [2, 0x81, 0xB1, 0]);
    }

    /// Past $B0 from its start x a spring drops 3 px a frame, asks for
    /// $FE = 1 as it leaves y $26, and is gone from y $D8.
    #[test]
    fn a_spring_past_its_run_drops_and_goes() {
        let mut arc = [0; 4];
        assert_eq!(
            spring_step(0xC2, 0x26, 0x10, &mut arc, (0x50, 0x03)),
            (Some((0xC2, 0x29)), Some(1))
        );
        assert_eq!(
            spring_step(0xC2, 0xD7, 0x10, &mut arc, (0x50, 0x03)),
            (Some((0xC2, 0xDA)), None)
        );
        assert_eq!(
            spring_step(0xC2, 0xDA, 0x10, &mut arc, (0x50, 0x03)),
            (None, None)
        );
    }
}
