//! The barrel routine $DA16 on 25m, for the play routine's cost
//! ([`crate::play`]), as [`crate::play_mario`] is for Mario's: what each
//! state's routine ($DA4C's dispatch) does to the slots' bytes as the frame
//! starts, and which way each of its branches goes, by the branch's
//! address ([`Route`]); the cycles are the walk of the lifted code along
//! that path (gp_cycles::walk, docs/cycles.md). A barrel's draw ($F080
//! from the roll, $EADB from the other states: the metasprite writer
//! $F096 at 2 by 2) is measured over the corpus: 603 and 621.
//!
//! Exact over the split's RAM on 49,240 of 49,245 25m play frames it
//! writes (worker rngH4, 2026-11-04, on `gp cycles split` after its fix
//! for a routine that pulls its return address); 235 aren't written: a
//! straight barrel ($C0 to $C2), a drop through a crowd ($40), a ladder
//! taken while Mario is on one ($DB8B, $96 2) and the hammer's search
//! ($DF40 with 5 or more barrels on Mario's girder) return None. Worker
//! rngH4 added the hammer's test, the ladder taken, the girder's end, the
//! oil drum, a landing from a fall, the roll off the screen and the let
//! go from Kong's hands.

use crate::play_mario::{Rom, Route};

/// A barrel slot's bytes as the frame starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Slot {
    /// The sprite's y is $FF.
    pub free: bool,
    /// The sprite's x and y ($0233, $0230 + 16k).
    pub x: u8,
    pub y: u8,
    /// $5E, $68, $8A, $72, $040D and $A3 + k: the state, the girder, the
    /// frame counter, the tile, the turn count and a ladder's foot.
    pub state: u8,
    pub girder: u8,
    pub time: u8,
    pub tile: u8,
    pub turn: u8,
    pub foot: u8,
    /// $0421 and $0417 + k: a straight drop's way (1 straight, 3 zigzag)
    /// and the zigzag's side.
    pub way: u8,
    pub side: u8,
    /// Set by [`barrels`] when the slot's branches were all written, so
    /// its frame counter is the routine's.
    pub ran: bool,
}

/// What $DA16 reads besides the slots.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Barrels {
    pub slots: [Slot; 9],
    /// The difficulty $EAF7 (0 to 4).
    pub d: u8,
    /// $19 of the random register as the frame starts.
    pub r19: u8,
    /// $C0, the drop through a crowd's flag.
    pub drop: u8,
    /// $36 and $37, Kong's timers as the frame starts.
    pub throw: u8,
    pub hold: u8,
    /// $AD, the oil drum lit (1, 2 while a fire comes out), and $43, the
    /// straight barrel's wait.
    pub drum: u8,
    pub straight_wait: u8,
    /// $042B, where the straight barrel landed.
    pub landed: u8,
    /// Mario's girder $59 and state $96 as his routine leaves them.
    pub girder: u8,
    pub mario: u8,
}

/// The draws, measured over the corpus from the first instruction after
/// their JSR (or the tail JMP) to their RTS: $F080 from the roll and $EADB
/// from the other states, the metasprite writer $F096 at 2 by 2.
pub(crate) const DRAW_ROLL: i64 = 601;
pub(crate) const DRAW: i64 = 621;
/// The hides, measured the same way: a barrel going into the oil drum
/// ($F08E) and one leaving the screen ($F092).
const INTO_DRUM: i64 = 270;
const HIDE: i64 = 337;

/// $EAF7: the difficulty, capped at 4 (`CPX #$04`, its BCC).
fn eaf7(d: u8, r: &mut Route) {
    r.br(0xEB01, d < 4);
}

/// $E166: the barrels on each girder ($7E + g), and Mario's: once, and
/// twice while he holds the hammer.
fn e166(b: &Barrels, crowd: &mut [u8; 10], r: &mut Route) {
    *crowd = [0; 10];
    // The clear loop, $7E to $84 from Y = 6 down.
    for y in (0..7).rev() {
        r.br(0xE16E, y > 0);
    }
    let girders: [u8; 10] = std::array::from_fn(|k| b.slots.get(k).map_or(0, |s| s.girder));
    for (y, g) in girders.iter().enumerate() {
        if !r.br(0xE175, *g == 0)
            && let Some(n) = crowd.get_mut(usize::from(*g))
        {
            *n = n.wrapping_add(1);
        }
        r.br(0xE181, y == 9);
    }
    if r.br(0xE18B, b.girder == 7) {
        return;
    }
    if let Some(n) = crowd.get_mut(usize::from(b.girder)) {
        *n = n.wrapping_add(1);
    }
    if r.br(0xE193, b.mario != 0x0A) {
        return;
    }
    if let Some(n) = crowd.get_mut(usize::from(b.girder)) {
        *n = n.wrapping_add(1);
    }
}

/// $DFE4: the slot's frame counter steps (to 0 past 15), and its bit of
/// `a` ($C1BC by the counter) is the result.
pub(crate) fn dfe4(s: &mut Slot, a: u8, r: &mut Route) -> bool {
    let next = s.time.wrapping_add(1);
    if !r.br(0xDFEE, next & 0x80 != 0) {
        r.br(0xDFF2, next >= 16);
    }
    let v = if next & 0x80 != 0 || next >= 16 {
        0
    } else {
        next
    };
    s.time = v;
    r.br(0xDFFD, v >= 8);
    let bit = (1u8 << (v & 7)) & a;
    r.br(0xE011, bit == 0);
    bit != 0
}

/// $E016: the girder by y, from 25m's list at the pointer $C493: the first
/// entry at or above y, 7 at its end ($FF).
fn e016(rom: &dyn Rom, a: u8, r: &mut Route) -> Option<u8> {
    let list = rom.w(0xC493);
    for (k, y) in (1u8..).zip(0..16u16) {
        let v = rom.b(list.wrapping_add(y));
        if r.br(0xE033, v == 0xFF) {
            return Some(7);
        }
        if r.br(0xE037, v == a) || r.br(0xE039, v < a) {
            return Some(k);
        }
    }
    None
}

/// $E05A: 0 at a step of the slope ($C1C4's list from 0, from 4 on girders
/// 1 and 6), else 1.
fn e05a(rom: &dyn Rom, g: u8, x: u8, r: &mut Route) -> u8 {
    let ends = r.br(0xE062, g == 1) || r.br(0xE066, g == 6);
    let (hit, end) = if ends {
        (0xE080, 0xE085)
    } else {
        (0xE06F, 0xE074)
    };
    let mut i = if ends { 4 } else { 0 };
    loop {
        if r.br(hit, rom.b(0xC1C4 + i) == x) {
            return 0;
        }
        i += 1;
        if r.br(end, i == 9) {
            return 1;
        }
    }
}

/// $E0F1: a ladder's top at x on the girder ($C177, $C181 by girder), its
/// foot ($C172, $C17C) into `foot`.
fn e0f1(rom: &dyn Rom, g: u8, x: u8, foot: &mut u8, r: &mut Route) -> bool {
    let k = u16::from(g).wrapping_sub(2);
    if r.br(0xE0FC, rom.b(0xC177 + k) == x) {
        *foot = rom.b(0xC172 + k);
        return true;
    }
    if r.br(0xE104, rom.b(0xC181 + k) == x) {
        *foot = rom.b(0xC17C + k);
        return true;
    }
    false
}

/// $E0AE: whether the barrel is at a ladder's top: the lifted ladders on
/// girders 2 to 6, and on 4 and 5 a third the code compares with ($C4 and
/// $B4, feet $89 and $71).
fn e0ae(rom: &dyn Rom, g: u8, x: u8, foot: &mut u8, r: &mut Route) -> bool {
    for (pc, v) in [
        (0xE0B6, 2),
        (0xE0BA, 3),
        (0xE0BE, 4),
        (0xE0C2, 5),
        (0xE0C6, 6),
    ] {
        if r.br(pc, g == v) {
            break;
        }
    }
    if !(2..=6).contains(&g) {
        return false;
    }
    if e0f1(rom, g, x, foot, r) {
        return true;
    }
    let third = match g {
        4 => Some((0xE0D8, 0xC4, 0x89)),
        5 => Some((0xE0E4, 0xB4, 0x71)),
        _ => None,
    };
    match third {
        Some((pc, tx, ty)) if r.br(pc, x == tx) => {
            *foot = ty;
            true
        }
        _ => false,
    }
}

/// $E090: whether x is the girder's end ($C1CD, $C1CE by its way).
fn e090(rom: &dyn Rom, g: u8, x: u8, r: &mut Route) -> bool {
    let odd = !r.br(0xE098, g & 1 == 0);
    r.br(0xE0A6, rom.b(0xC1CD + u16::from(!odd)) == x)
}

/// $DBEE: the turn count steps, and the tile turns every 6th.
fn tile(s: &mut Slot, r: &mut Route) {
    s.turn = s.turn.wrapping_add(1);
    if !r.br(0xDBF8, s.turn >= 6) {
        return;
    }
    s.turn = 0;
    let t = s.tile;
    s.tile = if !r.br(0xDC04, s.girder & 1 == 0) {
        let v = t.wrapping_add(4);
        if r.br(0xDC0D, v < 0x80) || r.br(0xDC11, v >= 0x90) {
            0x80
        } else {
            v
        }
    } else {
        let v = t.wrapping_sub(4);
        if r.br(0xDC22, v < 0x80) || r.br(0xDC26, v >= 0x90) {
            0x8C
        } else {
            v
        }
    };
}

/// $E112: y in the list at `list`, ended by $FE: (found, the index).
fn e112(rom: &dyn Rom, list: u16, y: u8, r: &mut Route) -> Option<(bool, u16)> {
    for i in 0..32u16 {
        let v = rom.b(list.wrapping_add(i));
        if r.br(0xE11A, v == 0xFE) {
            return Some((false, i));
        }
        if r.br(0xE11E, v == y) {
            return Some((true, i));
        }
    }
    None
}

/// $DB2C, the roll. None where a ladder is taken while Mario is on one
/// ($DB8B, $96 2: the test reads his sprite's y, which this doesn't keep)
/// and where the hammer's search runs ([`df40`]).
fn roll(b: &Barrels, s: &mut Slot, crowd: &[u8; 10], rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let a = dfe4(s, 0xFF, r);
    if !r.br(0xDB31, a) {
        return Some(());
    }
    let g = e016(rom, s.y, r)?;
    s.girder = g;
    let x = if r.br(0xDB47, g & 1 != 0) {
        s.x.wrapping_sub(1)
    } else {
        s.x.wrapping_add(1)
    };
    let step = e05a(rom, g, x, r);
    // $E048: a step down the slope while rolling.
    if !r.br(0xE04E, s.state != 1) {
        r.br(0xE052, step != 0);
    }
    tile(s, r);
    r.cost(0xDB6B, DRAW_ROLL);
    let ladder = e0ae(rom, g, x, &mut s.foot, r);
    if !r.br(0xDB73, !ladder) {
        eaf7(b.d, r);
        let go = rom.b(0xC448 + u16::from(b.d)) & b.r19 == 0;
        if !r.br(0xDB7D, !go) {
            let below = crowd
                .get(usize::from(g.wrapping_sub(1)))
                .copied()
                .unwrap_or(0);
            if !r.br(0xDB89, below >= 4) {
                if !r.br(0xDB8F, b.mario != 2) {
                    return None;
                }
                s.state = 2;
                s.girder = g.wrapping_sub(1);
                return Some(());
            }
        }
    }
    let end = e090(rom, g, x, r);
    if !r.br(0xDBB1, !end) {
        // At the girder's end ($DBE7): off it.
        s.state = 8;
        return Some(());
    }
    df40(b, crowd, r)?;
    if r.br(0xDBBF, g != 1) {
        return Some(());
    }
    // $DFC3 on the bottom girder: the oil drum's flame test by x.
    r.always(0xDFC9, false);
    r.br(0xDFD3, x >= 0x30);
    if r.br(0xDBC8, x == 0x20) || r.br(0xDBCA, x < 0x20) {
        // Into the oil drum ($DBCD): hidden, the drum lit, the girder 0.
        r.cost(0xDBD5, INTO_DRUM);
        s.girder = 0;
    }
    Some(())
}

/// $DF40: with no drop through a crowd ($C0) and Mario holding the
/// hammer ($96 $0A), on his girder $59 3 the crowd there ($81), else on
/// girder 5 ($83); under 5 barrels it returns. With 5 or more on girder 3
/// it looks for a rolling barrel on girder 3 in the ten slots and returns
/// when there is none. None when it finds one (it sends that barrel down,
/// $DF8F) and on girder 5's search, whose loop ends on its CMP #$0A
/// ($DF8A) against the byte it read last rather than the slot's index.
fn df40(b: &Barrels, crowd: &[u8; 10], r: &mut Route) -> Option<()> {
    if !r.br(0xDF42, b.drop == 0) || !r.br(0xDF49, b.mario == 0x0A) {
        return Some(());
    }
    let three = r.br(0xDF50, b.girder == 3);
    let i = if three { 3 } else { 5 };
    let full = crowd[i] >= 5;
    r.br(if three { 0xDF5B } else { 0xDF78 }, full);
    if !full {
        return Some(());
    }
    if !three {
        return None;
    }
    for k in 0..10 {
        let slot = b.slots.get(k);
        if !r.br(0xDF64, slot.map_or(0, |s| s.state) != 1)
            && r.br(0xDF6A, slot.map_or(0, |s| s.girder) == 3)
        {
            return None;
        }
        r.br(0xDF6F, k != 9);
    }
    Some(())
}

/// $DC30, down a ladder.
fn ladder(s: &mut Slot, r: &mut Route) {
    let a = dfe4(s, 0x55, r);
    if r.br(0xDC35, !a) {
        return;
    }
    let y = s.y.wrapping_add(1);
    r.br(0xDC48, s.tile != 0x90);
    r.cost(0xDC57, DRAW);
    r.br(0xDC60, y != s.foot);
}

/// $DC69, off a girder's end, to its landing on the girder below ($E112's
/// list at the pointer $C03C + $32).
fn fall(b: &Barrels, s: &mut Slot, crowd: &[u8; 10], rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let a = dfe4(s, 0xFF, r);
    if !r.br(0xDC6E, a) {
        return Some(());
    }
    let y = s.y.wrapping_add(1);
    if !r.br(0xDC7F, y & 1 == 0) {
        r.br(0xDC87, s.girder & 1 == 0);
    }
    tile(s, r);
    r.cost(0xDC99, DRAW);
    let (landed, _) = e112(rom, rom.w(0xC03C + 0x32), y, r)?;
    if r.br(0xDCA6, !landed) {
        return Some(());
    }
    // Landed ($DCA8): a bounce (state $10), or a roll off the screen ($20)
    // when the barrel is at or below Mario's girder ($E130) and bit 0 of
    // $19 is set, or when four or more are on the girder below.
    s.state = 0x10;
    let d = s.girder.wrapping_sub(b.girder);
    let near = r.br(0xE137, d == 0) || r.br(0xE139, d & 0x80 != 0);
    let off = !r.br(0xDCB1, !near) && !r.br(0xDCB7, b.r19 & 1 == 0);
    if !off {
        let below = crowd
            .get(usize::from(s.girder.wrapping_sub(1)))
            .copied()
            .unwrap_or(0);
        if !r.br(0xDCC6, below >= 4) {
            return Some(());
        }
    }
    s.state = 0x20;
    Some(())
}

/// $DCD0, a bounce off the girder below: x through C3FC (rightward) or
/// C412 (leftward), the move C407.
fn bounce(s: &mut Slot, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let a = dfe4(s, 0x77, r);
    if !r.br(0xDCD5, a) {
        return Some(());
    }
    let g = e016(rom, s.y, r)?;
    s.girder = g;
    let even = !r.br(0xDCEB, g & 1 != 0);
    let (x, tab, cmp, hit, end) = if even {
        (s.x.wrapping_add(1), 0xC3FCu16, 0xDCF3, 0xDCF6, 0xDCFB)
    } else {
        (s.x.wrapping_sub(1), 0xC412u16, 0xDD06, 0xDD09, 0xDD0E)
    };
    for i in 0..11u8 {
        r.idx(cmp, i);
        if r.br(hit, rom.b(tab + u16::from(i)) == x) {
            r.idx(0xDD16, i);
            r.br(0xDD1D, i != 10);
            break;
        }
        if r.br(end, i == 10) {
            break;
        }
    }
    tile(s, r);
    r.cost(0xDD2E, DRAW);
    Some(())
}

/// $DD32, rolling off the screen's edge; on the edge ($DD7F) its sprite
/// hidden and its girder cleared.
fn off(s: &mut Slot, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let a = dfe4(s, 0x55, r);
    if !r.br(0xDD37, a) {
        return Some(());
    }
    let g = e016(rom, s.y, r)?;
    s.girder = g;
    let gone = if !r.br(0xDD4D, g & 1 != 0) {
        r.br(0xDD55, s.y != 0x14);
        !r.br(0xDD5B, s.x.wrapping_sub(1) != 0)
    } else {
        r.br(0xDD66, s.y != 0xEC);
        !r.br(0xDD6E, s.x.wrapping_add(1) != 0xF4)
    };
    if gone {
        r.cost(0xDD81, HIDE);
    } else {
        tile(s, r);
        r.cost(0xDD7C, DRAW);
    }
    Some(())
}

/// $DA89, the straight drop ($C0 falling, $C1 stopped on a girder, $C2
/// landed), with its draw. None as it lands ($C2 from $C0) and as it rolls
/// on (1 from $C2).
fn straight(b: &Barrels, s: &mut Slot, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let way = s.way;
    if !r.br(0xDD93, s.state != 0xC2) {
        // $DEA5: where it landed against x.
        let x0 = s.x.wrapping_sub(1);
        let mut v = b.landed;
        let mut hit = None;
        for (i, (pc, d)) in [
            (0xDEB7, 1u8),
            (0xDEBE, 1),
            (0xDEC5, 1),
            (0xDECC, 8),
            (0xDED3, 1),
        ]
        .into_iter()
        .enumerate()
        {
            v = v.wrapping_sub(d);
            if r.br(pc, v == x0) {
                hit = Some(i);
                break;
            }
        }
        match hit {
            None if !r.br(0xDEDA, v.wrapping_sub(1) != x0) => return None,
            Some(2) if r.br(0xDEF9, way == 1) => return None,
            _ => {}
        }
        r.cost(0xDF03, DRAW);
        return Some(());
    }
    if !r.br(0xDD9A, s.state == 0xC1) {
        if !r.br(0xDDA0, way == 2) {
            r.br(0xDDA4, way == 3);
        }
        let list = match way {
            2 => 0x36,
            3 => 0x38,
            _ => 0x34,
        };
        let (stop, at) = e112(rom, rom.w(0xC03C + list), s.y, r)?;
        if !r.br(0xDDC4, at != 4) {
            return None;
        }
        if !r.br(0xDDCB, !stop) {
            s.time = 1;
            s.state = 0xC1;
        }
    }
    let mut y = s.y;
    if !r.br(0xDDE2, s.state != 0xC1) {
        let go = dfe4(s, 0x20, r);
        if r.br(0xDDE9, go) {
            s.state = 0xC0;
            if !r.br(0xDDFF, way != 3) {
                let was = s.side;
                r.br(0xDE04, was == 0);
                s.side = u8::from(was == 0);
            }
            moved(&mut y, way, rom, r);
        }
    } else {
        let go = dfe4(s, 0xFF, r);
        if r.br(0xDE18, !go) {
            return Some(());
        }
        moved(&mut y, way, rom, r);
    }
    if r.br(0xDE2E, way != 2) && !r.br(0xDE38, way != 3) && !r.br(0xDE3E, y & 1 == 0) {
        r.br(0xDE45, s.side != 0);
    }
    r.br(0xDE61, s.tile != 0x90);
    r.cost(0xDE70, DRAW);
    Some(())
}

/// $DE1A and $DE86: y one down, and on the straight way once more through
/// three bands (C41D to C420).
fn moved(y: &mut u8, way: u8, rom: &dyn Rom, r: &mut Route) {
    *y = y.wrapping_add(1);
    if r.br(0xDE8A, way != 1) {
        return;
    }
    for k in 0..3 {
        if !r.br(0xDE93, *y < rom.b(0xC41D + k)) && !r.br(0xDE98, *y >= rom.b(0xC420 + k)) {
            *y = y.wrapping_add(1);
            return;
        }
        r.br(0xDEA2, k != 2);
    }
}

/// $DA9C, in Kong's hands, drawn; on the frame $37 runs out, let go ($81,
/// its frame counter 0), or for slot 0 a straight drop ($C0): at once
/// with the drum unlit ($AD 0), else when $43 is out, the way ($0421)
/// turning from 1 to 3 and back, and $43 set again ($DAF7).
fn held(b: &Barrels, s: &mut Slot, k: usize, r: &mut Route) {
    r.cost(0xDAAB, DRAW);
    if r.br(0xDAB0, b.hold != 0) {
        return;
    }
    s.state = 0x81;
    s.time = 0;
    if r.br(0xDABE, b.drum == 0) {
        if r.br(0xDAC5, k != 0) {
            return;
        }
        s.way = 1;
    } else {
        if r.br(0xDAD7, b.straight_wait != 0) || r.br(0xDADB, k != 0) {
            return;
        }
        let was = s.way;
        r.br(0xDAE8, was != 1);
        s.way = if was == 1 { 3 } else { 1 };
    }
    s.state = 0xC0;
    eaf7(b.d, r);
}

/// $DB00, let go: still for a beat, then rolling.
fn let_go(s: &mut Slot, r: &mut Route) {
    let a = dfe4(s, 0x55, r);
    if !r.br(0xDB05, a) {
        r.cost(0xDB18, DRAW);
    }
}

/// $DA4C: the dispatch on the state $5E and the state's routine. None
/// where it isn't written.
fn da4c(
    b: &Barrels,
    s: &mut Slot,
    k: usize,
    crowd: &[u8; 10],
    rom: &dyn Rom,
    r: &mut Route,
) -> Option<()> {
    const STATES: [(u8, u16); 11] = [
        (0x80, 0xDA52),
        (0x81, 0xDA56),
        (0x01, 0xDA5A),
        (0x02, 0xDA5E),
        (0xC0, 0xDA62),
        (0xC1, 0xDA66),
        (0xC2, 0xDA6A),
        (0x08, 0xDA6E),
        (0x10, 0xDA72),
        (0x20, 0xDA76),
        (0x40, 0xDA7A),
    ];
    if !STATES.iter().any(|(v, pc)| r.br(*pc, s.state == *v)) {
        return Some(());
    }
    match s.state {
        0x80 => held(b, s, k, r),
        0x81 => let_go(s, r),
        0x01 => roll(b, s, crowd, rom, r)?,
        0x02 => ladder(s, r),
        0x08 => fall(b, s, crowd, rom, r)?,
        0x10 => bounce(s, rom, r)?,
        0x20 => off(s, rom, r)?,
        0xC0..=0xC2 => straight(b, s, rom, r)?,
        _ => return None,
    }
    Some(())
}

/// $DA16 from its JSR to its RTS: the crowd counts, then each slot, a free
/// one taking a barrel when Kong's $36 is out. Leaves the slots as the
/// routine does (their frame counters, tiles and ladder feet). The cycles
/// are the walk of $DA16's code along the path the slots take
/// ([`Route`]).
pub fn barrels(b: &mut Barrels, rom: &dyn Rom) -> Option<i64> {
    let mut r = Route::new();
    let mut crowd = [0u8; 10];
    e166(b, &mut crowd, &mut r);
    // Every slot runs, so a slot's bytes step on a frame another's branch
    // isn't written.
    let mut all = true;
    for k in 0..9 {
        let mut s = b.slots[k];
        let n = if r.br(0xDA25, !s.free) {
            da4c(b, &mut s, k, &crowd, rom, &mut r)
        } else if r.br(0xDA29, b.throw != 0) {
            Some(())
        } else {
            s.state = 0x80;
            b.hold = 0x10;
            eaf7(b.d, &mut r);
            b.throw = rom.b(0xC443 + u16::from(b.d));
            da4c(b, &mut s, k, &crowd, rom, &mut r)
        };
        s.ran = n.is_some();
        all &= s.ran;
        b.slots[k] = s;
        r.br(0xDA49, k != 8);
    }
    // The JSR to $DA16.
    all.then(|| r.walk(rom, 0xDA16)).flatten().map(|c| c + 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run by hand, as `mario_cycles_match_the_split` is: each 25m play
    /// frame's $DA16 against [`barrels`] on the RAM at its NMI, with $59
    /// and $96 as the next NMI's.
    #[test]
    fn barrel_cycles_match_the_split() {
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
        let num = |line: &str, name: &str| -> Option<i64> {
            let at = line.find(&format!("\"{name}\":"))?;
            line[at + name.len() + 3..]
                .split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse()
                .ok()
        };
        let (mut n, mut ok, mut none) = (0u64, 0u64, 0u64);
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
                let (Some(r0), Some(r1)) = (
                    ram.get(i * 2048..(i + 1) * 2048),
                    ram.get((i + 1) * 2048..(i + 2) * 2048),
                ) else {
                    break;
                };
                let (Some(f0), Some(f1)) = (
                    num(line, "frame"),
                    lines.get(i + 1).and_then(|l| num(l, "frame")),
                ) else {
                    continue;
                };
                if f1 != f0 + 1 || r0[0x53] != 1 || r0[0x58] != 0 {
                    continue;
                }
                let (Some(want), Some(_)) = (num(line, "da16"), num(line, "ec29")) else {
                    continue;
                };
                let mut b = Barrels {
                    slots: std::array::from_fn(|k| Slot {
                        free: r0[0x230 + 16 * k] == 0xFF,
                        x: r0[0x233 + 16 * k],
                        y: r0[0x230 + 16 * k],
                        state: r0[0x5E + k],
                        girder: r0[0x68 + k],
                        time: r0[0x8A + k],
                        tile: r0[0x72 + k],
                        turn: r0[0x40D + k],
                        foot: r0[0xA3 + k],
                        way: r0[0x421 + k],
                        side: r0[0x417 + k],
                        ran: false,
                    }),
                    d: ((r0[0x50] & 1) + r0[0x54]).min(4),
                    r19: r0[0x19],
                    drop: r0[0xC0],
                    landed: r0[0x42B],
                    drum: r0[0xAD],
                    straight_wait: r0[0x43],
                    throw: r0[0x36],
                    hold: r0[0x37],
                    girder: r1[0x59],
                    mario: r1[0x96],
                };
                let Some(got) = barrels(&mut b, &rom) else {
                    none += 1;
                    continue;
                };
                n += 1;
                if got == want {
                    ok += 1;
                } else if n - ok < 12 {
                    eprintln!("{name} {f0}: got {got} want {want}");
                }
            }
        }
        eprintln!("exact {ok} of {n}, {none} not written");
        assert!(n - ok <= n / 1000, "{} of {n} off", n - ok);
    }
}
