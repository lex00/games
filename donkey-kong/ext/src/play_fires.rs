//! The fire routine $E1E5 on 25m, for the play routine's cost
//! ([`crate::play`]), as [`crate::play_barrels`] is for the barrels: what
//! each fire's state routine ($E250's dispatch, which runs the way's choice
//! $E2B6 and dispatches again) does to the fire slots' bytes as the frame
//! starts and which way its branches go ([`Route`]); the cycles are the
//! walk of the lifted code. A fire's draw (the
//! metasprite writer $F096 at 2 by 2) is measured over the corpus: 593,
//! or 705 mirrored, 713 on a ladder ($F088) and 598 out of the drum
//! ($F082).
//!
//! Exact over the split's RAM on 49,473 of 49,480 25m play frames, every
//! one written (worker rngH4, 2026-11-04, on `gp cycles split` after its
//! fix for a routine that pulls its return address); the climb ($E41B)
//! and the drum ($E538) were added to the walk, the bob and the way's
//! choice (worker rngH3).

use crate::play_mario::{Rom, Route};

/// A fire slot's bytes as the frame starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fire {
    /// The first sprite's y is $FF.
    pub free: bool,
    /// The first sprite's x, y and tile ($0213, $0210, $0211 + 16k).
    pub x: u8,
    pub y: u8,
    pub tile: u8,
    /// $AF, $E4, $E0, $D2, $EC and $B3 + k: the state, the move counter,
    /// the girder, whether it has aimed, the way it aimed and the way it
    /// last walked.
    pub state: u8,
    pub count: u8,
    pub girder: u8,
    pub aimed: u8,
    pub aim: u8,
    pub way: u8,
    /// $E8 + k: the ladder's pause, 1 to 3 on the frames a climb waits at
    /// a y that is a multiple of 4.
    pub pause: u8,
    /// The slot, $AE.
    pub slot: u8,
}

/// What $E1E5 reads besides the slots.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fires {
    pub fires: [Fire; 2],
    /// $40 (fire_wait), $AD (drum) and $3B (fire_aim) as the frame starts.
    pub wait: u8,
    pub drum: u8,
    pub aim: u8,
    /// $19 and $1A of the random register.
    pub random: [u8; 2],
    /// Mario's x, $0203, as his routine leaves it.
    pub mario_x: u8,
}

/// The draws, measured over the corpus from the first instruction after
/// their JSR (or the tail JMP) to their RTS: the metasprite writer $F096
/// at 2 by 2, mirrored, the climb's $F088 and the drum's $F082.
const DRAW: i64 = 593;
const DRAW_MIRRORED: i64 = 705;
const DRAW_CLIMB: i64 = 710;
const DRAW_DRUM: i64 = 598;

/// $E806: the move counter steps (to 0 past 15) and its bit of the
/// pattern ($0A for the counts 0 to 7, $0B for 8 to 15, the bits $C1BC).
fn e806(f: &mut Fire, lo: u8, hi: u8, r: &mut Route) -> bool {
    let next = f.count.wrapping_add(1);
    if !r.br(0xE80C, next & 0x80 != 0) {
        r.br(0xE810, next >= 16);
    }
    let v = if next & 0x80 != 0 || next >= 16 {
        0
    } else {
        next
    };
    f.count = v;
    r.br(0xE81B, v >= 8);
    let bit = (1u8 << (v & 7)) & if v < 8 { lo } else { hi };
    r.br(0xE82F, bit == 0);
    bit != 0
}

/// $E016 (the barrels' too): the girder by y.
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

/// $E5E8: whether x is off every step of the fire's girder's slope ($18,
/// then $C1C4's list; from its 4th on girders 1 and 6).
fn e5e8(rom: &dyn Rom, g: u8, x: u8, r: &mut Route) -> bool {
    if r.br(0xE5F0, g == 1) || r.br(0xE5F4, g == 6) {
        let mut i = 4;
        loop {
            if r.br(0xE616, rom.b(0xC1C4 + i) == x) {
                return false;
            }
            i += 1;
            if r.br(0xE61B, i == 9) {
                return true;
            }
        }
    }
    let mut a = 0x18;
    let mut i = 0;
    loop {
        if r.br(0xE5FC, a == x) {
            return false;
        }
        i += 1;
        if r.br(0xE601, i == 9) {
            return true;
        }
        a = rom.b(0xC1C4 + i);
    }
}

/// $E6A5 on 25m: the girder from y + 11, the slope's step and the girder's
/// ends ($C3E6, $C3E8 by the way), where the fire stops (false).
fn e6a5(rom: &dyn Rom, f: &mut Fire, x: u8, y: u8, d: u8, r: &mut Route) -> Option<bool> {
    let g = e016(rom, y.wrapping_add(11), r)?;
    f.girder = g;
    r.br(0xE6B7, false);
    let off = e5e8(rom, g, x, r);
    if !r.br(0xE6CB, off) {
        r.br(0xE6D3, g & 1 == 0);
    }
    let end = if r.br(0xE6E9, g == 1) {
        r.br(0xE6F8, rom.b(0xC3E8 + u16::from(d)) == x)
    } else {
        r.br(0xE6F0, rom.b(0xC3E6 + u16::from(d)) == x)
    };
    Some(!end)
}

/// $E368, a walk ($99 the way), to its end with the draw.
fn walk(rom: &dyn Rom, f: &mut Fire, d: u8, r: &mut Route) -> Option<()> {
    let a = e806(f, 0x55, 0x55, r);
    if !r.br(0xE371, a) {
        return Some(());
    }
    let (mut x, mut y) = (f.x, f.y);
    x = if r.br(0xE37E, d != 0) {
        x.wrapping_sub(1)
    } else {
        x.wrapping_add(1)
    };
    let lo = x & 0x0F;
    let edge = r.br(0xE38D, lo == 4) || r.br(0xE391, lo == 0x0C);
    let mut to_e3af = true;
    if edge {
        y = y.wrapping_add(1);
    } else if r.br(0xE3A0, rom.b(0xC3E2 + u16::from(d)) == lo)
        || r.br(0xE3A5, rom.b(0xC3E4 + u16::from(d)) == lo)
    {
        y = y.wrapping_sub(1);
        to_e3af = false;
    }
    if to_e3af {
        let _ = r.br(0xE3B1, lo == 4) || r.br(0xE3B5, lo == 0x0C);
    }
    let go = e6a5(rom, f, x, y, d, r)?;
    if !r.br(0xE3C5, go) {
        return Some(());
    }
    if !r.br(0xE3D0, d == 0) && !r.br(0xE3D6, x == 0x0C) && r.br(0xE3D8, x < 0x0C) {
        return Some(());
    }
    r.always(0xE3F8, false);
    r.br(0xE3FC, f.tile >= 0x9C);
    r.cost(0xE418, if d != 0 { DRAW_MIRRORED } else { DRAW });
    Some(())
}

/// $E2B6: the way chosen by $19 plus the slot ($E2DD), toward Mario the
/// first time ($EC).
fn choose(f: &mut Fire, rnd: u8, mario_x: u8, r: &mut Route) {
    if !r.br(0xE2BA, f.aimed != 0) {
        f.aim = if r.br(0xE2D0, f.x >= mario_x) { 2 } else { 1 };
        f.aimed = 1;
    }
    let v = rnd & 7;
    f.state = if !r.br(0xE2E6, v >= 4) {
        v
    } else if !r.br(0xE2EF, v >= 7) {
        f.aim
    } else {
        3
    };
}

/// $E2F9, the bob out of the drum ($10 up, $20 down), with the draw.
fn bob(f: &mut Fire, r: &mut Route) {
    let a = e806(f, 0x55, 0x55, r);
    if !r.br(0xE302, a) || !r.br(0xE313, f.state != 0x20) {
        return;
    }
    r.br(0xE31C, f.state == 0x10);
    r.always(0xE330, false);
    r.br(0xE334, f.tile == 0x9C);
    r.cost(0xE353, if f.way >> 1 != 0 { DRAW_MIRRORED } else { DRAW });
    r.br(0xE35C, f.state == 0x10);
}

/// $E538 on 25m: a new fire (6) set at the drum's top, or one jumping
/// out of it (8): x one on, y one down past the arc's x, and at the arc's
/// end back up to land on the girder (state 0). To its draw's end ($F082).
/// The places and states are the immediates of the code, read from the
/// lifted code at their operands' addresses.
fn e538(rom: &dyn Rom, f: &mut Fire, r: &mut Route) {
    let op = |at: u16| rom.b(at);
    if r.br(0xE53E, f.state == 6) {
        r.always(0xE54C, true);
        f.x = op(0xE554);
        f.y = op(0xE558);
        f.state = op(0xE55E);
        r.cost(0xE59C, DRAW_DRUM);
        return;
    }
    if !r.br(0xE542, f.state == 8) {
        return;
    }
    r.always(0xE5B1, true);
    let mut x = f.x.wrapping_add(1);
    let up = r.br(0xE5BA, x == op(0xE5B9)) || !r.br(0xE5BC, x < op(0xE5B9));
    if up {
        let y = f.y.wrapping_add(1);
        if !r.br(0xE5C4, y != op(0xE5C3)) {
            f.state = 0;
            x = x.wrapping_sub(1);
            r.br(0xE5D2, x >= op(0xE5D1));
            r.br(0xE5DD, x != op(0xE5DC));
        }
        f.y = y;
    }
    f.x = x;
    r.cost(0xE5E5, DRAW_DRUM);
}

/// A ladder's top and foot by the fire's girder $E0 and x: the 25m ladders
/// a fire takes, which $E626 compares x with and sets by immediates in its
/// code; read from the lifted code (each the operand of its CMP or LDA).
fn ladder_ends(rom: &dyn Rom, g: u8, x: u8) -> Option<(u8, u8)> {
    let ladders: [(u16, u16, u16); 2] = if g == 1 {
        [(0xE643, 0xE652, 0xE657), (0xE647, 0xE662, 0xE667)]
    } else {
        [(0xE670, 0xE67F, 0xE684), (0xE674, 0xE68F, 0xE694)]
    };
    ladders
        .iter()
        .find(|(at, _, _)| rom.b(*at) == x)
        .map(|(_, top, foot)| (rom.b(*top), rom.b(*foot)))
}

/// $E626: on $13 (on the way down) nothing; on 3 the fire takes the ladder
/// at its x (state 3) or doesn't (state 0).
fn e626(rom: &dyn Rom, f: &mut Fire, r: &mut Route) {
    if !r.br(0xE62C, f.state != 0x13) {
        return;
    }
    let found = ladder_ends(rom, f.girder, f.x).is_some();
    let (a, b) = if r.br(0xE63B, f.girder == 1) {
        ((0xE644, 0xE643), (0xE648, 0xE647))
    } else {
        ((0xE671, 0xE670), (0xE675, 0xE674))
    };
    let _ = r.br(a.0, rom.b(a.1) == f.x) || r.br(b.0, rom.b(b.1) == f.x);
    f.state = if found { 3 } else { 0 };
}

/// $E41B, on a ladder: up on state 3, down on $13, the move counter's
/// pattern from $C3F4 by the state, a pause of three steps at each y that
/// is a multiple of 4 ($E8 + k), the top turning a fire back down (13
/// below the top, or at it for slot 0 off girder 2, which walks on) and
/// the foot ending the climb. To its draw's end ($F088). None off the
/// ladders [`ladder_ends`] knows.
fn climb(rom: &dyn Rom, f: &mut Fire, r: &mut Route) -> Option<()> {
    let (top, foot) = ladder_ends(rom, f.girder, f.x)?;
    let i = u16::from(f.state >> 3);
    r.always(0xE427, false);
    let a = e806(f, rom.b(0xC3F4 + i), rom.b(0xC3F5 + i), r);
    if !r.br(0xE44E, a) {
        return Some(());
    }
    let to_e46d = if r.br(0xE45D, f.pause == 0) {
        true
    } else if r.br(0xE461, f.pause == 3) {
        f.pause = 0;
        false
    } else {
        true
    };
    let skip = to_e46d && !r.br(0xE471, f.y & 3 != 0) && {
        f.pause += 1;
        true
    };
    if !skip {
        r.always(0xE47E, true);
        if r.br(0xE4BB, f.state == 0x13) {
            let y = f.y.wrapping_add(1);
            if !r.br(0xE4CB, foot != y) {
                f.state = 1;
            }
            f.y = y;
        } else {
            let y = f.y.wrapping_sub(1);
            let k0 = !r.br(0xE4DC, f.slot != 0) && !r.br(0xE4E4, f.girder == 2);
            if k0 {
                if !r.br(0xE4EE, top != y) {
                    f.state = 2;
                }
            } else if !r.br(0xE504, top.wrapping_add(0x0D) != y) {
                f.state = 0x13;
            }
            f.y = y;
        }
    }
    r.always(0xE517, false);
    r.br(0xE51B, f.tile >= 0x9C);
    r.cost(0xE535, DRAW_CLIMB);
    Some(())
}

/// $E250: the dispatch by the state's low bits, the way's choice and the
/// dispatch again. None where it isn't written.
fn e250(rom: &dyn Rom, f: &mut Fire, rnd: u8, mario_x: u8, r: &mut Route) -> Option<()> {
    for _ in 0..3 {
        let st = f.state & 0x0F;
        if r.br(0xE256, st == 0) {
            bob(f, r);
            return Some(());
        }
        if r.br(0xE25A, st == 6) || r.br(0xE25E, st == 8) {
            e538(rom, f, r);
            return Some(());
        }
        if r.br(0xE262, st == 1) {
            return walk(rom, f, 0, r);
        }
        if r.br(0xE266, st == 2) {
            return walk(rom, f, 1, r);
        }
        if r.br(0xE26A, st == 3) {
            r.always(0xE2A5, false);
            e626(rom, f, r);
            if r.br(0xE2AE, f.state != 0) {
                return climb(rom, f, r);
            }
            bob(f, r);
            return Some(());
        }
        r.always(0xE270, false);
        choose(f, rnd, mario_x, r);
        let v = f.state;
        if r.br(0xE284, v == 1) || !r.br(0xE288, v != 2) {
            f.way = v;
        }
    }
    None
}

/// $E1E5 from its JSR to its RTS. Leaves the slots as the routine does.
/// The cycles are the walk of $E1E5's code along the path the slots take
/// ([`Route`]).
pub fn fires(b: &mut Fires, rom: &dyn Rom) -> Option<i64> {
    let mut r = Route::new();
    r.always(0xE1F7, true);
    let mut all = true;
    for k in 0..2 {
        let mut f = b.fires[k];
        let n = if r.br(0xE1F1, !f.free) {
            e250(rom, &mut f, b.random[k], b.mario_x, &mut r)
        } else if r.br(0xE202, b.wait != 0)
            || r.br(0xE206, b.drum == 0)
            || r.br(0xE20A, b.drum != 2)
        {
            Some(())
        } else {
            // A new fire ($E20C): the wait set to $19 and the slot to 6,
            // then $E250 runs it.
            b.wait = 0x19;
            f.state = 6;
            f.free = false;
            e250(rom, &mut f, b.random[k], b.mario_x, &mut r)
        };
        all &= n.is_some();
        b.fires[k] = f;
        r.br(0xE232, k == 1);
    }
    r.always(0xE23B, false);
    r.br(0xE23F, b.aim != 0);
    all.then(|| r.walk(rom, 0xE1E5)).flatten().map(|c| c + 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run by hand, as the other play tests are: each 25m play frame's
    /// $E1E5 against [`fires`] on the RAM at its NMI, with Mario's x as
    /// the next NMI's.
    #[test]
    fn fire_cycles_match_the_split() {
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
                let (Some(want), Some(_)) = (num(line, "e1e5"), num(line, "ec29")) else {
                    continue;
                };
                let mut b = Fires {
                    fires: std::array::from_fn(|k| Fire {
                        free: r0[0x210 + 16 * k] == 0xFF,
                        x: r0[0x213 + 16 * k],
                        y: r0[0x210 + 16 * k],
                        tile: r0[0x211 + 16 * k],
                        state: r0[0xAF + k],
                        count: r0[0xE4 + k],
                        girder: r0[0xE0 + k],
                        aimed: r0[0xD2 + k],
                        aim: r0[0xEC + k],
                        way: r0[0xB3 + k],
                        pause: r0[0xE8 + k],
                        slot: k as u8,
                    }),
                    wait: r0[0x40],
                    drum: r0[0xAD],
                    aim: r0[0x3B],
                    random: [r0[0x19], r0[0x1A]],
                    mario_x: r1[0x203],
                };
                let Some(got) = fires(&mut b, &rom) else {
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
