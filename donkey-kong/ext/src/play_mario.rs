//! Mario's routine $D1A4 on 25m, branch by branch, for the play
//! routine's cost ([`crate::play`]). Each function is a copy of a routine's
//! branches over the bytes it reads and writes ([`M`]), the cycles of each
//! straight run counted from the 6502's instruction times (a taken branch
//! 1 more, 2 across a page; `LDA ($04),Y` 1 more across a page). The
//! tables it reads ($C03C-$C4FF: the box tables, 25m's girder pieces, its
//! ladder and gap lists, the walk's beat bits) come from the lifted
//! `play_rom` by CPU address, never from the code. A draw's cost at the
//! end (the metasprite writer $F096 with a fixed size) is measured over
//! the corpus by the path that reaches it ([`Tail`]).
//!
//! Exact over the split's RAM on 48,228 of 48,596 25m play frames (worker
//! rngH3, 2026-11-04): walking 30,982 of 31,037, on a ladder 8537 of 8537,
//! with the hammer 7043 of 7052, jumping 1666 of 1905. A fall ($96 8), a
//! jump's touch of a hammer ($D800 past its x test) and the landing hold's
//! end return None.

/// Reads the lifted tables by CPU address: 0 past them.
pub trait Rom {
    fn b(&self, a: u16) -> u8;
    fn w(&self, a: u16) -> u16 {
        u16::from(self.b(a)) | u16::from(self.b(a.wrapping_add(1))) << 8
    }
    fn box4(&self, a: u16) -> [u8; 4] {
        std::array::from_fn(|i| self.b(a.wrapping_add(i as u16)))
    }
    /// The box at the pointer $C03C + x ($C847's load).
    fn ptr4(&self, x: u16) -> [u8; 4] {
        self.box4(self.w(0xC03C + x))
    }
    /// The cycles of the routine at `entry` along the path `path` answers,
    /// from its first instruction to its RTS, read from the lifted code
    /// ([`gp_cycles::walk`]); None where the code isn't lifted (an
    /// original's empty slots) or the path doesn't say.
    fn walk(&self, entry: u16, path: &mut dyn gp_cycles::Path) -> Option<i64> {
        gp_cycles::walk(&|a| self.b(a), entry, path, 50_000)
            .ok()
            .map(|c| c as i64)
    }
}

/// The path a routine takes, as the model's state decides it: each
/// branch's outcome queued by the branch's address in the order the code
/// meets it, branches the code always takes the same way, the indexes of
/// reads that can cross a page, and calls and tail jumps costed by a
/// measured fact. [`Route::walk`] sums the cycles from the lifted code and
/// fails unless every answer queued is used, so a path that strays from
/// what the model decided shows as no cost, not a wrong one. An indexed
/// read with no index queued is taken as not crossing a page, and a
/// (zp),Y read with no pointer as reading through one at a page's start.
#[derive(Debug, Default, Clone)]
pub struct Route {
    branches: std::collections::BTreeMap<u16, std::collections::VecDeque<bool>>,
    fixed: std::collections::BTreeMap<u16, bool>,
    indexes: std::collections::BTreeMap<u16, std::collections::VecDeque<u8>>,
    pointers: std::collections::BTreeMap<u16, std::collections::VecDeque<(u16, u8)>>,
    costs: std::collections::BTreeMap<u16, std::collections::VecDeque<i64>>,
    stray: bool,
}

impl Route {
    pub fn new() -> Self {
        Self::default()
    }

    /// The branch at `pc` is taken (`t`) the next time the code meets it.
    /// Returns `t`, so a test reads `if r.br(PC, cond) { ... }`.
    pub fn br(&mut self, pc: u16, t: bool) -> bool {
        self.branches.entry(pc).or_default().push_back(t);
        t
    }

    /// The branch at `pc` goes one way every time (after a load of a
    /// constant).
    pub fn always(&mut self, pc: u16, t: bool) -> &mut Self {
        self.fixed.insert(pc, t);
        self
    }

    /// The index register at the indexed read at `pc`, the next time.
    pub fn idx(&mut self, pc: u16, i: u8) {
        self.indexes.entry(pc).or_default().push_back(i);
    }

    /// The pointer and Y of the (zp),Y read at `pc`, the next time.
    pub fn ptr(&mut self, pc: u16, p: u16, y: u8) {
        self.pointers.entry(pc).or_default().push_back((p, y));
    }

    /// The call or tail jump at `pc` costs `c` cycles from the callee's
    /// first instruction to its RTS, the next time the code meets it: a
    /// measured fact (a draw whose cost isn't modelled branch by branch).
    pub fn cost(&mut self, pc: u16, c: i64) -> &mut Self {
        self.costs.entry(pc).or_default().push_back(c);
        self
    }

    /// The cycles from `entry` to its RTS along the route, from the lifted
    /// code. None when the code isn't lifted, a branch has no answer, or an
    /// answer is left over.
    pub fn walk(mut self, rom: &dyn Rom, entry: u16) -> Option<i64> {
        let c = rom.walk(entry, &mut self)?;
        let left = self.branches.values().any(|q| !q.is_empty())
            || self.indexes.values().any(|q| !q.is_empty())
            || self.costs.values().any(|q| !q.is_empty())
            || self.pointers.values().any(|q| !q.is_empty());
        (!left && !self.stray).then_some(c)
    }
}

impl gp_cycles::Path for Route {
    fn ask(&mut self, a: gp_cycles::Ask) -> gp_cycles::Answer {
        use gp_cycles::{Answer, Ask, Call};
        match a {
            Ask::Branch { pc } => match self.branches.get_mut(&pc).and_then(|q| q.pop_front()) {
                Some(t) => Answer::Taken(t),
                None => match self.fixed.get(&pc) {
                    Some(t) => Answer::Taken(*t),
                    None => {
                        self.stray = true;
                        Answer::Unknown
                    }
                },
            },
            Ask::Index { pc, indirect, .. } => {
                let i = self
                    .indexes
                    .get_mut(&pc)
                    .and_then(|q| q.pop_front())
                    .unwrap_or(0);
                if indirect {
                    let (p, y) = self
                        .pointers
                        .get_mut(&pc)
                        .and_then(|q| q.pop_front())
                        .unwrap_or((0, i));
                    Answer::Pointer(p, y)
                } else {
                    Answer::Index(i)
                }
            }
            Ask::Call { pc, .. } | Ask::Jump { pc, .. } => {
                match self.costs.get_mut(&pc).and_then(|q| q.pop_front()) {
                    Some(c) => Answer::Call(Call::Cost(c as u64)),
                    None => Answer::Unknown,
                }
            }
        }
    }
}

impl<F: Fn(u16) -> u8> Rom for F {
    fn b(&self, a: u16) -> u8 {
        self(a)
    }
}

/// A draw's measured end, by the path that reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    None,
    /// The climb's picture ($F096 from $D42C or $D4CC), mirrored when the
    /// pose is $54.
    Up(bool),
    Down(bool),
    /// The jump's picture ($F070) in the air or on its first frame, by $57.
    Air(u8),
    First(u8),
    /// The hammer's frame ($F070 and $F078): the swing up, $57 and $A0.
    Hammer(bool, u8, u8),
}

/// The bytes Mario's routine reads and writes, by address, and its box
/// test's first box ($46-$49).
#[derive(Debug, Clone)]
pub struct M {
    pub ram: [u8; 0x460],
    pub a: [u8; 4],
    /// The last x $D8AD read (its $00).
    pub x0: u8,
    /// Mario's y after the jump's arc ($EF72), from the model's end.
    pub y1: u8,
    pub tail: Tail,
}

impl M {
    pub fn new() -> Self {
        Self {
            ram: [0; 0x460],
            a: [0; 4],
            x0: 0,
            y1: 0,
            tail: Tail::None,
        }
    }
    fn r(&self, a: u16) -> u8 {
        self.ram[usize::from(a)]
    }
    fn set(&mut self, a: u16, v: u8) {
        self.ram[usize::from(a)] = v;
    }
}

impl Default for M {
    fn default() -> Self {
        Self::new()
    }
}

fn corners(x: u8, y: u8, b: [u8; 4]) -> [u8; 4] {
    [
        x.wrapping_add(b[0]),
        y.wrapping_add(b[1]),
        x.wrapping_add(b[2]),
        y.wrapping_add(b[3]),
    ]
}

/// The metasprite writer $F096 at 2 by 2 from its JSR or JMP to its RTS
/// (measured over the corpus), and mirrored; $F082 and $F088 from their
/// JMP, which reach it by a branch.
const F096: i64 = 593;
const F096_MIRRORED: i64 = 705;
pub(crate) const F082: i64 = 598;
const F088: i64 = 710;
/// The hammer's own sprite ($F078's $F096 at 2 by 1 up, 1 by 2 down), by
/// the way Mario faces (measured).
const HAMMER_UP: [i64; 2] = [HAMMER_UP_RIGHT, HAMMER_UP_LEFT];
const HAMMER_DOWN: [i64; 2] = [HAMMER_DOWN_RIGHT, HAMMER_DOWN_LEFT];
const HAMMER_UP_RIGHT: i64 = 405;
const HAMMER_UP_LEFT: i64 = 465;
const HAMMER_DOWN_RIGHT: i64 = 423;
const HAMMER_DOWN_LEFT: i64 = 501;

/// Loops end at the tables' $FE; a guard for a table that has none.
const MAX_LOOP: usize = 64;

/// $F096 by its mirror flag.
fn f096(mirror: bool) -> i64 {
    if mirror { F096_MIRRORED } else { F096 }
}

/// The beat ($D9E8, or $D9E6 with one pattern for both halves): the
/// beat counter steps and wraps at the count its code compares with, and
/// the result is the pattern's bit for the count, `lo` for the first half
/// and `hi` for the second, from the bit table (the code's own counts,
/// read from the lifted code). Returns A.
fn d9e8(m: &mut M, rom: &dyn Rom, hi: u8, lo: u8, r: &mut Route) -> u8 {
    let (wrap, half) = (rom.b(0xD9EF), rom.b(0xD9FA).max(1));
    let next = m.r(0x88).wrapping_add(1);
    let v = if r.br(0xD9F0, next >= wrap) { 0 } else { next };
    m.set(0x88, v);
    r.br(0xD9FB, v >= half);
    let bits = if v < half { lo } else { hi };
    let a = rom.b(0xC1BC + u16::from(v % half)) & bits;
    u8::from(!r.br(0xDA0F, a == 0))
}

/// $D990: the screen's edges ($C1B4 by stage). None on 75m.
fn d990(m: &M, rom: &dyn Rom, r: &mut Route) -> Option<bool> {
    let s = u16::from(m.r(0x53));
    let x = m.r(0x203);
    let d = m.r(0x56);
    if r.br(0xD994, d == 1) {
        let b = rom.b((0xC1B4 + 2 * s).wrapping_sub(1));
        return Some(r.br(0xD9A8, b == x) || r.br(0xD9AA, b < x));
    }
    if !r.br(0xD998, d == 2) {
        return Some(false);
    }
    let b = rom.b((0xC1B4 + 2 * s).wrapping_sub(2));
    if r.br(0xD9BB, b >= x) {
        return Some(true);
    }
    if r.br(0xD9C1, s == 4) {
        return Some(false);
    }
    if r.br(0xD9C7, s == 3) {
        return None;
    }
    if r.br(0xD9CB, m.r(0x59) != 6) {
        return Some(false);
    }
    Some(r.br(0xD9D9, x == 0x68) || r.br(0xD9DB, x < 0x68))
}

/// $E016: the girder by y from $C493's list by stage.
fn e016(m: &M, rom: &dyn Rom, a: u8, r: &mut Route) -> Option<u8> {
    let list = rom.w(0xC493 + 2 * u16::from(m.r(0x53)).wrapping_sub(1));
    let mut k = 1u8;
    for y in 0..MAX_LOOP as u16 {
        let v = rom.b(list.wrapping_add(y));
        if r.br(0xE033, v == 0xFF) {
            return Some(7);
        }
        if r.br(0xE037, v == a) || r.br(0xE039, v < a) {
            return Some(k);
        }
        k = k.wrapping_add(1);
    }
    None
}

/// $D91A: Mario's feet box against his girder's pieces ($C08C's runs,
/// shapes at the pointer $C044 + $1A). Returns $5A.
fn d91a(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<u8> {
    let g = e016(m, rom, m.r(0x200).wrapping_add(8), r)?;
    m.set(0x59, g);
    let mut x: u16 = 0;
    if !r.br(0xD927, g == 1) {
        x = 12;
        let mut k = 2;
        while !r.br(0xD92F, k == g) {
            x += 6;
            k += 1;
            if k > 8 {
                return None;
            }
        }
    }
    let shapes = rom.w(0xC044 + 0x1A);
    for _ in 0..MAX_LOOP {
        let at = 0xC08C + x;
        let (mut px, mut py) = (rom.b(at), rom.b(at + 1));
        let off = rom.box4(shapes.wrapping_add(u16::from(rom.b(at + 2))));
        let (step, count) = (rom.b(at + 3), rom.b(at + 4));
        x += 4;
        let mut n = 0u8;
        for _ in 0..256 {
            let k = crate::play::box_cmp(r, m.a, corners(px, py, off));
            if r.br(0xD967, k == 4) {
                return Some(1);
            }
            px = px.wrapping_add(step);
            py = py.wrapping_sub(1);
            n = n.wrapping_add(1);
            if !r.br(0xD978, n != count) {
                break;
            }
        }
        x += 1;
        if r.br(0xD980, rom.b(0xC08C + x) == 0xFE) {
            return Some(0);
        }
        x += 1;
    }
    None
}

/// $D2CB: Mario's feet box by $EFE8 (a jump's or a fall's box at $C03C +
/// $4A, else $C03C + $2C), then $D91A. Returns $0C.
fn d2cb(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<u8> {
    let st = m.r(0x96);
    let off = if r.br(0xD2D2, st == 4) || r.br(0xD2D6, st == 8) {
        rom.ptr4(0x4A)
    } else {
        rom.ptr4(0x2C)
    };
    m.a = corners(m.r(0x203), m.r(0x200), off);
    crate::play::box_load(r);
    r.always(0xD2E6, true);
    d91a(m, rom, r)
}

/// $D8EB: a step a pixel up (1) or down (255) at a piece's end.
fn d8eb(m: &M, r: &mut Route) -> u8 {
    if r.br(0xD8ED, m.r(0x5A) != 0) || r.br(0xD8F1, m.r(0x59) == 0) {
        return 0;
    }
    let d = m.r(0x56);
    if !r.br(0xD8F5, m.r(0x59) & 1 != 0) {
        if r.br(0xD8FB, d == 1) {
            return 1;
        }
        if r.br(0xD8FF, d == 2) {
            return 255;
        }
        return 0;
    }
    if r.br(0xD908, d == 1) {
        return 255;
    }
    if r.br(0xD90C, d == 2) {
        return 1;
    }
    0
}

/// $D8AD: the box $46-$49 against a list of (x, y, shape) at `list`, the
/// shapes at `base`, ended by $FE. Returns $0C.
fn d8ad(m: &mut M, rom: &dyn Rom, list: u16, base: u16, r: &mut Route) -> Option<u8> {
    let mut y: u16 = 0;
    r.ptr(0xD8B7, list, 0);
    for _ in 0..MAX_LOOP {
        let at = list.wrapping_add(y);
        let (px, py) = (rom.b(at), rom.b(at.wrapping_add(1)));
        let off = rom.box4(base.wrapping_add(u16::from(rom.b(at.wrapping_add(2)))));
        m.x0 = px;
        r.ptr(0xD8BC, list, (y + 1) as u8);
        r.ptr(0xD8C1, list, (y + 2) as u8);
        y += 2;
        let k = crate::play::box_cmp(r, m.a, corners(px, py, off));
        if r.br(0xD8D3, k == 4) {
            return Some(1);
        }
        y += 1;
        r.ptr(0xD8D8, list, y as u8);
        if r.br(0xD8DC, rom.b(list.wrapping_add(y)) == 0xFE) {
            return Some(0);
        }
    }
    None
}

/// $D36A on 25m: the gaps' list at $C03C + $1C by $D8AD.
fn d36a(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<u8> {
    r.always(0xD36E, true);
    d8ad(m, rom, rom.w(0xC03C + 0x1C), rom.w(0xC044 + 0x1C), r)
}

/// The walk's girder step and gap test after a move across ($D208, or
/// $D713 with the hammer), shared by the walk and the hammer: None when
/// Mario falls.
fn after_move(m: &mut M, rom: &dyn Rom, walk: bool, r: &mut Route) -> Option<()> {
    let (step_pc, stage_pc, fall_pc) = if walk {
        (0xD218, 0xD21E, 0xD22C)
    } else {
        (0xD723, 0xD729, 0xD735)
    };
    let r5a = d2cb(m, rom, r)?;
    m.set(0x5A, r5a);
    let g = e016(m, rom, m.r(0x200), r)?;
    m.set(0x59, g);
    let step = d8eb(m, r);
    if !r.br(step_pc, step == 0) {
        r.br(stage_pc, false);
        m.set(0x200, m.r(0x200).wrapping_add(step));
        let fall = d36a(m, rom, r)?;
        if !r.br(fall_pc, fall == 0) {
            return None;
        }
    }
    Some(())
}

/// $D1A4 from its JSR to its RTS, on 25m, by Mario's state as $D175
/// leaves it: the dispatch on $96 ($D1A4, and after the walk again from
/// $D1AA) and the state's routine, walked over the lifted code. None
/// where it isn't written.
pub fn mario(m: &mut M, rom: &dyn Rom) -> Option<i64> {
    m.tail = Tail::None;
    let mut r = Route::new();
    let chain = [
        (1u8, 0xD1A8u16),
        (2, 0xD1AC),
        (4, 0xD1B0),
        (8, 0xD1B4),
        (10, 0xD1B8),
    ];
    let mut from = 0;
    if r.br(0xD1A8, m.r(0x96) == 1) {
        walk(m, rom, &mut r)?;
        from = 1;
    }
    let st = m.r(0x96);
    let hit = chain[from.max(1)..]
        .iter()
        .find(|(v, pc)| r.br(*pc, st == *v))
        .map(|(v, _)| *v);
    match hit {
        None if from == 1 => {}
        Some(2) => ladder(m, rom, &mut r)?,
        Some(4) => jump(m, rom, &mut r)?,
        Some(10) => hammer(m, rom, &mut r)?,
        _ => return None,
    }
    r.walk(rom, 0xD1A4).map(|c| c + 6)
}

/// $D1CF, the walk ($96 1), or a ladder's grab ($D28B).
fn walk(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let d = m.r(0x56);
    if !(r.br(0xD1D3, d == 1) || r.br(0xD1D7, d == 2)) {
        if !(r.br(0xD1DB, d == 4) || r.br(0xD1DF, d == 8)) {
            return Some(());
        }
        // $D28B: the grab, Mario's box ($C186) against the ladders'
        // bottoms at $C47B by stage.
        m.a = corners(m.r(0x203), m.r(0x200), rom.box4(0xC186));
        crate::play::box_load(r);
        let x = 2 * u16::from(m.r(0x53)).wrapping_sub(1);
        let found = d8ad(m, rom, rom.w(0xC47B + x), rom.w(0xC483 + x), r)?;
        if !r.br(0xD2B7, found == 0) {
            m.set(0xA1, m.x0.wrapping_sub(4));
            m.set(0x96, 2);
            m.set(0x5B, 0);
            m.set(0x5C, 0);
        }
        return Some(());
    }
    let a = d9e8(m, rom, 0x36, 0xDB, r);
    if r.br(0xD1EE, a != 0) {
        let blocked = d990(m, rom, r)?;
        if !r.br(0xD1F6, !blocked) {
            return Some(());
        }
        if r.br(0xD1FD, d == 2) {
            m.set(0x203, m.r(0x203).wrapping_sub(1));
        } else {
            m.set(0x203, m.r(0x203).wrapping_add(1));
        }
        after_move(m, rom, true, r)?;
        if !r.br(0xD235, m.r(0x9B) != 0) {
            m.set(0x9B, 1);
        } else {
            let p = m.r(0x97);
            if !r.br(0xD248, p == 0) && !r.br(0xD24C, p == 8) {
                r.br(0xD254, m.r(0x85) == 0);
            }
        }
    }
    // $D275: the walk's picture, mirrored going left.
    if r.br(0xD283, d == 2) {
        r.cost(0xD288, F088);
    } else {
        r.cost(0xD285, F082);
    }
    Some(())
}

/// $D6C6, with the hammer ($96 = 10).
fn hammer(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    if !r.br(0xD6C8, m.r(0x3F) != 0) {
        return None;
    }
    let a = d9e8(m, rom, 0x36, 0xDB, r);
    if !r.br(0xD6D6, a != 0) {
        return Some(());
    }
    let blocked = d990(m, rom, r)?;
    let d = m.r(0x56);
    let mut moved = false;
    if !r.br(0xD6DC, blocked) {
        if r.br(0xD6E2, d == 1) {
            moved = true;
            m.set(0x203, m.r(0x203).wrapping_add(1));
        } else if r.br(0xD6E6, d == 2) {
            moved = true;
            m.set(0x203, m.r(0x203).wrapping_sub(1));
        }
    }
    if !moved {
        let a2 = m.r(0xA2) << 1;
        m.set(0xA2, a2);
        if r.br(0xD6ED, a2 == 0) {
            let s = m.r(0x9F);
            let sw = if r.br(0xD6F8, s == 0) || !r.br(0xD6FC, s < 4) {
                2
            } else {
                5
            };
            m.set(0x9F, sw);
        }
    } else {
        after_move(m, rom, false, r)?;
        let s = m.r(0x9F);
        let sw = if r.br(0xD744, s == 0) || r.br(0xD748, s >= 6) {
            1
        } else {
            s + 1
        };
        m.set(0x9F, sw);
    }
    // $D753: Mario's swing picture ($F070, mirrored by $57), then the
    // hammer's sprite above him or before him ($F078).
    let way = m.r(0x57);
    r.cost(0xF07D, f096(way & 3 >= 2));
    let up = m.r(0x9F) >> 2 == 0;
    r.br(0xD760, up);
    if r.br(0xD769, !up) {
        r.br(0xD78A, way != 1);
    }
    let touch = m.r(0xA0);
    r.br(0xD7B1, touch == 1);
    let side = usize::from(way != 1);
    r.cost(
        0xF07D,
        if up {
            HAMMER_UP[side]
        } else {
            HAMMER_DOWN[side]
        },
    );
    m.tail = Tail::Hammer(up, way, touch);
    Some(())
}

/// $D50A: the ladder test at Mario's place, 0 off, 1 on a ladder, 2 in a
/// ladder's top zone.
fn d50a(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<u8> {
    m.a = corners(m.r(0x203), m.r(0x200), rom.ptr4(0x2C));
    crate::play::box_load(r);
    let list = rom.w(0xC48B + 2 * u16::from(m.r(0x53)).wrapping_sub(1));
    let mut rr = d8ad(m, rom, list, 0xC143, r)?;
    r.always(0xD534, false);
    let top = d8ad(m, rom, rom.w(0xC03C + 0x1E), rom.w(0xC044 + 0x1E), r)?;
    if !r.br(0xD53E, top == 0) {
        rr = 2;
    }
    Some(rr)
}

/// $D4CF to the RTS: the girder test, and the walk again ($96 1) when
/// Mario's feet meet a piece.
fn d4cf(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let r5a = d2cb(m, rom, r)?;
    m.set(0x5A, r5a);
    if r.br(0xD4D4, r5a == 0) {
        return Some(());
    }
    let g = e016(m, rom, m.r(0x200).wrapping_add(8), r)?;
    m.set(0x59, g);
    Some(())
}

/// A pose's step through `max` (`hit`, `less` the BEQ and BCC of its
/// CMP), from `first` when the count is 0 (`zero` the BEQ); `wrap` past
/// it.
#[allow(clippy::too_many_arguments)]
fn pose_step(
    r: &mut Route,
    v: u8,
    zero: u16,
    hit: u16,
    less: u16,
    max: u8,
    first: u8,
    wrap: u8,
) -> u8 {
    if r.br(zero, v == 0) {
        return first;
    }
    let p = v + 1;
    if r.br(hit, p == max) || r.br(less, p < max) {
        p
    } else {
        wrap
    }
}

/// $D37E on a ladder ($96 = 2): up on $56 8, down on 4, the climb's
/// pictures by the top zone's ($5B, $C147) and the steps' ($5C, $C159)
/// counts, and $D4CF's girder test.
fn ladder(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let d = m.r(0x56);
    if r.br(0xD382, d == 8) {
        if !r.br(0xD390, m.r(0x5A) == 0) {
            let rr = d50a(m, rom, r)?;
            if r.br(0xD39A, rr != 0) {
                return d4cf(m, rom, r);
            }
        }
        let a = d9e8(m, rom, 0x49, 0x24, r);
        if !r.br(0xD3A5, a != 0) {
            return d4cf(m, rom, r);
        }
        let rr = d50a(m, rom, r)?;
        let pose = if r.br(0xD3B2, rr == 0) {
            let p = pose_step(r, m.r(0x5C), 0xD3E9, 0xD3F0, 0xD3F2, 6, 2, 1);
            m.set(0x5C, p);
            m.set(0x5A, 0);
            m.set(0x5B, 0);
            rom.b(0xC159 + u16::from(p) - 1)
        } else {
            if !r.br(0xD3B6, rr != 2) {
                return d4cf(m, rom, r);
            }
            let p = pose_step(r, m.r(0x5B), 0xD3BD, 0xD3C4, 0xD3C6, 16, 1, 16);
            m.set(0x5B, p);
            m.set(0x5A, 0);
            m.set(0x5C, 0);
            rom.b(0xC147 + u16::from(p) - 1)
        };
        let y = m.r(0x200).wrapping_sub(1);
        r.br(0xD503, y & 6 != 0);
        let mirror = pose == 0x54;
        r.br(0xD41F, mirror);
        r.cost(0xD42C, f096(mirror));
        m.tail = Tail::Up(mirror);
        m.set(0x203, m.r(0xA1));
        m.set(0x200, y);
        return d4cf(m, rom, r);
    }
    if !r.br(0xD386, d == 4) {
        return d4cf(m, rom, r);
    }
    if !r.br(0xD434, m.r(0x5A) == 0) {
        let rr = d50a(m, rom, r)?;
        if !r.br(0xD440, rr == 1) {
            return d4cf(m, rom, r);
        }
    }
    let a = d9e8(m, rom, 0x49, 0x49, r);
    if !r.br(0xD450, a != 0) {
        return d4cf(m, rom, r);
    }
    let rr = d50a(m, rom, r)?;
    let pose = if r.br(0xD45D, rr == 0) || r.br(0xD461, rr == 2) {
        let p = pose_step(r, m.r(0x5C), 0xD48D, 0xD494, 0xD496, 6, 1, 1);
        m.set(0x5C, p);
        m.set(0x5B, 0);
        rom.b(0xC159 + u16::from(p) - 1)
    } else {
        let v = m.r(0x5B);
        let p = if r.br(0xD465, v == 0) {
            13
        } else if r.br(0xD46C, v - 1 < 1) {
            1
        } else {
            v - 1
        };
        m.set(0x5B, p);
        m.set(0x5C, 3);
        rom.b(0xC147 + u16::from(p) - 1)
    };
    let y = m.r(0x200).wrapping_add(1);
    r.br(0xD503, y & 6 != 0);
    let mirror = pose == 0x54;
    r.br(0xD4BF, mirror);
    r.cost(0xD4CC, f096(mirror));
    m.tail = Tail::Down(mirror);
    m.set(0x203, m.r(0xA1));
    m.set(0x200, y);
    d4cf(m, rom, r)
}

/// $D800: a hammer touched in a jump. None past its x test.
fn d800(m: &M, r: &mut Route) -> Option<()> {
    if !r.br(0xD802, m.r(0xA0) == 0) {
        return Some(());
    }
    r.always(0xD809, true).always(0xD813, true);
    if r.br(0xD820, m.r(0x203) == 0x28) || r.br(0xD822, m.r(0x203) < 0x28) {
        return None;
    }
    Some(())
}

/// $D547, a jump ($96 = 4).
fn jump(m: &mut M, rom: &dyn Rom, r: &mut Route) -> Option<()> {
    let a = d9e8(m, rom, 0xFF, 0xFF, r);
    if !r.br(0xD54E, a != 0) {
        return Some(());
    }
    if !r.br(0xD555, m.r(0x94) < 0xF0) {
        // $D60D: the landing hold counts $94 on to $F4.
        m.set(0x94, m.r(0x94).wrapping_add(1));
        if r.br(0xD613, m.r(0x94) != 0xF4) {
            return Some(());
        }
        return None;
    }
    let blocked = d990(m, rom, r)?;
    if !r.br(0xD55D, !blocked) {
        let to = if r.br(0xD563, m.r(0x56) != 1) { 1 } else { 2 };
        m.set(0x56, to);
        m.set(0x57, to);
    }
    // The arc $EF72 with X 0: its start on the takeoff's frame ($94 0,
    // $042C still 0), else mid-jump.
    r.br(0xEF79, m.r(0x94) != 0);
    r.always(0xEF80, false).always(0xEF99, false);
    m.set(0x200, m.y1);
    let d = m.r(0x56);
    let go = if r.br(0xD583, d == 1) {
        Some(0xD58E)
    } else if r.br(0xD587, d == 2) {
        Some(0xD5A3)
    } else {
        None
    };
    if let Some(pc) = go
        && !r.br(pc, m.r(0x9E) == 0)
    {
        let x = m.r(0x203);
        m.set(
            0x203,
            if d == 1 {
                x.wrapping_add(1)
            } else {
                x.wrapping_sub(1)
            },
        );
    }
    d800(m, r)?;
    let way = m.r(0x57);
    if r.br(0xD5BD, m.r(0x94) == 0) {
        // $D5E2: the takeoff's sound, $94 1 and $95 the height.
        m.tail = Tail::First(way);
        m.set(0x94, 1);
        m.set(0x95, m.r(0x200));
        r.cost(0xF07D, f096(way & 3 >= 2));
        return Some(());
    }
    let low = m.r(0x200).wrapping_sub(16) < m.r(0x95);
    if !r.br(0xD5C6, low) {
        m.set(0x95, 0xFF);
    }
    let r5a = d2cb(m, rom, r)?;
    if r.br(0xD5D1, r5a == 0) {
        m.tail = Tail::Air(way);
        r.cost(0xF07D, f096(way & 3 >= 2));
        return Some(());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run by hand: `GP_PLAY_SPLIT` a folder of `gp cycles split
    /// --ram-out` outputs and `GP_PLAY_PRG` the program from `gp cycles
    /// slice`. Each 25m play frame's $D1A4 against [`mario`] on the RAM
    /// at the frame's NMI, with $56, $57 and Mario's y as the next NMI's.
    #[test]
    fn mario_cycles_match_the_split() {
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
                let (Some(want), Some(_)) = (num(line, "d1a4"), num(line, "ec29")) else {
                    continue;
                };
                let mut m = M::new();
                m.ram.copy_from_slice(&r0[..0x460]);
                m.set(0x56, r1[0x56]);
                m.set(0x57, r1[0x57]);
                m.y1 = r1[0x200];
                let pad = r1[0x15 + 2 * usize::from(r0[0x52])];
                if m.r(0x96) == 1 && pad & 0x80 != 0 {
                    m.set(0x96, 4);
                    m.set(0x94, 0);
                }
                let st = m.r(0x96);
                let Some(got) = mario(&mut m, &rom) else {
                    none += 1;
                    continue;
                };
                n += 1;
                if got == want {
                    ok += 1;
                } else if n - ok < 12 {
                    eprintln!(
                        "{name} {f0}: $96 {st} got {got} want {want} tail {:?}",
                        m.tail
                    );
                }
            }
        }
        eprintln!("exact {ok} of {n}, {none} not written");
        // 3 frames of 49,045 jump frames past the hammer's touch test are
        // off (the corpus, 2026-11-04).
        assert!(n - ok <= n / 1000, "{} of {n} off", n - ok);
    }
}
