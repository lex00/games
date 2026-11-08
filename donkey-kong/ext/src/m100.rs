//! Donkey Kong's 100m and 75m: what a rivet or a prize taken shows and
//! asks for.
//!
//! The touch itself is the level's `pickups` (platformer `level.pickups`,
//! $EE79): 100m's 8 rivets ($C1 to $C8, the counters `rivet_0` to
//! `rivet_7`) and each stage's 3 prizes ($C9 to $CB, `prize_0` to
//! `prize_2`). The score comes
//! with their events. On the frame one is taken the routine shows the
//! points ($CFC6: X = 0, 100, for a rivet at $EED1; X = 3, 800, for a
//! prize at $EF27) and asks for the jingle $FD = $20 ($EED6, $EF2C); this
//! hook does those two, from the counters that went from 0 to 1.
//!
//! The same frame, after the touch, the clear test ($CF24 -> $D04C, $D063)
//! finds every rivet byte $C1-$C8 set and clears 100m ($D086): this hook
//! fires the model's event `rivets.cleared` then, which starts the ending.

use gp_sim::{Ext, ExtCx};

/// The counters the level's pickups keep, rivets then prizes.
const TAKEN: [&str; 11] = [
    "rivet_0", "rivet_1", "rivet_2", "rivet_3", "rivet_4", "rivet_5", "rivet_6", "rivet_7",
    "prize_0", "prize_1", "prize_2",
];

/// Whether one of `names` went from 0 to 1 this frame.
fn taken_now(cx: &ExtCx<'_>, names: &[&str]) -> bool {
    names
        .iter()
        .any(|n| cx.counter_at_start(n) == Some(0) && cx.counter(n) == Some(1))
}

/// The points and the jingle of a rivet or prize taken: a body with no box
/// that runs after the player's systems.
pub(crate) struct Taken;

impl Ext for Taken {
    fn step(&self, cx: &mut ExtCx<'_>) {
        if !taken_now(cx, &TAKEN) {
            return;
        }
        crate::award_points(cx);
        if cx.counter("sound_fd").is_some() {
            cx.set_counter("sound_fd", 0x20);
        }
        // $D063: the clear needs all 8 rivets, a rivet taken this frame.
        let rivets = &TAKEN[..8];
        if taken_now(cx, rivets) && rivets.iter().all(|n| cx.counter(n) == Some(1)) {
            if let Some(e) = cx.event("rivets.cleared") {
                cx.fire(e, None);
            }
        }
    }
}

/// 100m's ending ($C8A8 -> $CCF4 while $044F is 8), its pictures, by the
/// counter `ending_wait` (the divider wraps left, the game's $43 from $A0)
/// and the frames since it last changed (`ext[1]`; `ext[0]` holds the
/// wait last seen, `ext[3]` 1 while an ending runs). On the wrap to 65
/// ($43 $9F) and the four frames after it ($9E to $9B, $CD45-$CD89) the
/// game queues one update string a frame from the pointer table $C03C
/// ($C823: entries $0A, $0C, $06, $08, $0E), which clear the girders and
/// ladders ($24) and lay a girder ($62) under Kong; the hook changes those
/// cells' kinds (empty, top), which `present.cells` draws. From the frame
/// after, to $90, Kong blinks: the chest beat's poses by $43's parity
/// ($CD9D: $EB89 odd, $EB92 even). At $8F ($CDB1) his picture is cleared
/// (entry $10) and he falls as 24 sprites from (`fall_x`, `fall_y`), 2
/// pixels a frame ($CDD7), until his first sprite is at `land_y`; the
/// frame after, the thud ($FE $80), the sprites go and he lies in the name
/// table at $228D (`kong_fallen`, the counter `ending_kong` picks entry
/// $14 or $16). At $85 ($CE2F) a girder under Pauline (entry $18), the
/// heart (`heart_x`, `heart_y`), Mario facing left at (`mario_x`,
/// `mario_y`) and Pauline down to `pauline_y` ($CE63-$CE78); from $84 to
/// $70 Kong's two poses by $43's parity ($CE0E). The counter `ending_pic`
/// says what `present.hidden` hides: 1 the status rows blanked (entry
/// $0A, $C6E4), 2 the sprites moved off ($CBAE at $9E), 3 Kong falling, 4
/// Kong down, 5 the meeting. The frame after the
/// wait runs out the ending ends ($CD13): the hook takes its bodies away.
pub(crate) struct Ending;

/// The update strings' pointer table ($C823 and $C815 read it).
const POINTERS: u16 = 0xC03C;

fn param(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.param(name).floor()
}

/// The bodies of kind `name` alive.
fn bodies_of(cx: &ExtCx<'_>, name: &str) -> Vec<(usize, gp_sim::world::Body)> {
    let Some(k) = cx.kind(name) else {
        return Vec::new();
    };
    cx.world
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.alive && b.kind == k)
        .map(|(i, b)| (i, *b))
        .collect()
}

fn spawn_at(cx: &mut ExtCx<'_>, name: &str, x: i64, y: i64, left: bool) {
    if let Some(k) = cx.kind(name) {
        cx.spawn(k, gp_sim::Fx::from_int(x), gp_sim::Fx::from_int(y), left);
    }
}

/// The update string at entry `entry` of $C03C, as the NMI writes it to
/// the name table: each update an address (high, low), a count byte (bit 7
/// down a column, bit 6 one tile repeated) and its tiles, to a 0. Each
/// cell written with the blank ($24) becomes empty and with the girder
/// ($62) a top, kept as written (`write_tile`) even where that is the
/// built kind, so the prizes, built as empty cells, are blanked ($C760)
/// and the rivets' places under Pauline show girder ($C74E); attributes
/// and palettes are left to the presentation.
fn draw_string(cx: &mut ExtCx<'_>, entry: u16) {
    use gp_sim::spec::Tile;
    let at = POINTERS + entry;
    let mut p = (crate::rom_at(cx, at) | crate::rom_at(cx, at + 1) << 8) as u16;
    for _ in 0..64 {
        let hi = crate::rom_at(cx, p);
        if hi == 0 {
            return;
        }
        let addr = hi << 8 | crate::rom_at(cx, p + 1);
        let ctl = crate::rom_at(cx, p + 2);
        let n = ctl & 0x3F;
        let (down, repeat) = (ctl & 0x80 != 0, ctl & 0x40 != 0);
        p += 3;
        for i in 0..n {
            let tile = crate::rom_at(cx, if repeat { p } else { p + i as u16 });
            let a = addr + if down { 32 * i } else { i };
            if (0x2000..0x23C0).contains(&a) {
                let (x, y) = ((a & 31) * 8, ((a - 0x2000) >> 5) * 8);
                match tile {
                    0x24 => cx.write_tile(x, y, Tile::Empty),
                    0x62 => cx.write_tile(x, y, Tile::Top),
                    _ => {}
                }
            }
        }
        p += if repeat { 1 } else { n as u16 };
    }
}

impl Ext for Ending {
    fn step(&self, cx: &mut ExtCx<'_>) {
        let w = cx.counter("ending_wait").unwrap_or(0);
        let last = cx.me.ext[0];
        let k = if w == last { cx.me.ext[1] + 1 } else { 0 };
        cx.me.ext[0] = w;
        cx.me.ext[1] = k;
        if w == 0 {
            // The frame after the wrap to $57 ($CD13): the ending is over.
            if cx.me.ext[3] != 0 && last == 0 {
                cx.me.ext[3] = 0;
                for name in ["kong_fall", "kong_fallen", "heart", "ending_mario"] {
                    for (i, _) in bodies_of(cx, name) {
                        cx.remove(i);
                    }
                }
                cx.set_counter("ending_pic", 0);
                cx.set_counter("ending_kong", 0);
            }
            return;
        }
        cx.me.ext[3] = 1;
        // A clear on a wrap frame reaches 65 two frames before $9F
        // (`ending_lag`).
        let k = if w == 65 { k - cx.counter("ending_lag").unwrap_or(0) } else { k };
        if (w, k) == (65, 0)
            && let Some(e) = cx.event("ending.hide")
        {
            cx.fire(e, None);
        }
        match (w, k) {
            (65, ..0) => {}
            // A clear off a wrap frame leaves $43 at $A0 until the wrap:
            // from the frame $0450 is 1 ($CCF7), $CD9D draws Kong by $43's
            // parity, $A0's the even picture ($EB92, entry $42), as at 65
            // to 55 below (game B's 100m at d = 1, the clear at divider 3).
            (66, 1..) => cx.set_counter("kong_drawn", 5),
            (65, 0) => {
                draw_string(cx, 0x0A);
                cx.set_counter("ending_pic", 1);
            }
            (65, 1) => {
                draw_string(cx, 0x0C);
                cx.set_counter("ending_pic", 2);
            }
            (65, 2) => draw_string(cx, 0x06),
            (65, 3) => draw_string(cx, 0x08),
            (65, 4) => draw_string(cx, 0x0E),
            (55..=65, _) => cx.set_counter("kong_drawn", if w % 2 == 1 { 5 } else { 4 }),
            (54, 0) => {
                draw_string(cx, 0x10);
                let (x, y) = (param(cx, "fall_x"), param(cx, "fall_y"));
                spawn_at(cx, "kong_fall", x, y, false);
                cx.set_counter("ending_pic", 3);
            }
            _ => {}
        }
        // $CDD7: the fall, a frame at a time, and the landing.
        for (i, mut b) in bodies_of(cx, "kong_fall") {
            if b.y.floor() >= param(cx, "land_y") {
                cx.remove(i);
                cx.set_counter("sound_fe", 0x80);
                let x = b.x.floor();
                let y = b.y.floor();
                spawn_at(cx, "kong_fallen", x, y, false);
                cx.set_counter("ending_pic", 4);
                cx.set_counter("ending_kong", 0);
            } else {
                b.y = b.y.add(gp_sim::Fx::from_int(2));
                cx.set_body(i, b);
            }
        }
        if (w, k) == (45, 0) {
            // $CE2F: the girder under Pauline, the heart, Mario, Pauline.
            draw_string(cx, 0x18);
            let (hx, hy) = (param(cx, "heart_x"), param(cx, "heart_y"));
            spawn_at(cx, "heart", hx, hy, false);
            let (mx, my) = (param(cx, "mario_x"), param(cx, "mario_y"));
            spawn_at(cx, "ending_mario", mx, my, true);
            let player = cx
                .world
                .bodies
                .iter()
                .position(|b| b.alive && cx.spec.kinds[b.kind].role == gp_sim::spec::Role::Player);
            if let Some(i) = player {
                // The sprite buffer's $0203 and $0200, plus 5, are his place.
                let mut b = cx.world.bodies[i];
                b.x = gp_sim::Fx::from_int(mx + 5);
                b.y = gp_sim::Fx::from_int(my + 5);
                cx.set_body(i, b);
            }
            for (i, mut b) in bodies_of(cx, "pauline") {
                b.y = gp_sim::Fx::from_int(param(cx, "pauline_y") + 1);
                cx.set_body(i, b);
            }
            cx.set_counter("mario_pose", 4);
            cx.set_counter("ending_pic", 5);
        } else if (25..=45).contains(&w) && cx.counter("ending_pic") == Some(5) {
            cx.set_counter("ending_kong", w % 2);
        }
    }
}
