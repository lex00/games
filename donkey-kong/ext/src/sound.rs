//! Donkey Kong's sound driver as model state: the bytes the game's driver
//! keeps ($F0-$FF, $0102 and nine bytes at $0680-$06F1) and the step it
//! takes once a frame at the start of the NMI ($FA48, the driver proper at
//! $FBF2). The step reads the songs, the length tables and the effects'
//! tables from the lifted asset `sound_rom` (the ROM from `ROM_BASE` up),
//! never from the repo, and writes no APU register: what it keeps is the
//! state the driver keeps, so the mapping compares it byte for byte and the
//! retro `rng_per_frame` switch can read it. Presentation plays the songs
//! through `gp_sound` as before.
//!
//! The driver plays one effect on pulse 1 at a time ($FF asks, $F0 holds
//! the one playing, $F1 and $F2 its counters), one on pulse 2 ($FE bit 7,
//! $06A5), one on the triangle ($FE bits 0 and 1, $06A1, $F5 and $F6), a
//! looping jingle by $FC ($06A3, $0680, $0698) and the music by $FD ($FB
//! the song, $F7-$F8 its data, $F3, $FA and $F9 the pulse 1, pulse 2 and
//! triangle offsets, $0695, $0696 and $0698 their lengths, $068D the
//! length table, $0691 pulse 2's length, $0102 playing). $06F0 is a pitch
//! that sweeps between $90 and $D8, which one effect plays.

use crate::play_mario::Route;
use gp_sim::world::World;
use gp_sim::{Ext, ExtCx, Spec};

/// The ROM address the lifted `sound_rom` table starts at.
pub const ROM_BASE: u16 = 0xFAE5;

/// Where each of the driver's tables sits in the ROM, by the address its
/// code reads.
mod at {
    pub const BLIP: u16 = 0xFB67;
    pub const LENGTHS: u16 = 0xFB4C;
    pub const JINGLE_LENGTHS: u16 = 0xFB62;
    pub const SONGS: u16 = 0xFE59;
    pub const JINGLE_STARTS: u16 = 0xFFCD;
    /// The operands of the driver's `LDA #` loads of the jingles' address
    /// into $F5 and $F6 ($FD2B and $FD2F): $FFD4 in donkey-kong-ju, $8340
    /// in the lower bank in donkey-kong-oe, whose $FD2C and $FD30 differ.
    pub const JINGLE_LO: u16 = 0xFD2C;
    pub const JINGLE_HI: u16 = 0xFD30;
}

/// The driver's bytes, by the RAM address the game keeps each in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sound {
    pub f0: u8,
    pub f1: u8,
    pub f2: u8,
    pub f3: u8,
    pub f5: u8,
    pub f6: u8,
    pub f7: u8,
    pub f8: u8,
    pub f9: u8,
    pub fa: u8,
    pub fb: u8,
    /// The jingle asked for, which the game sets and clears itself.
    pub fc: u8,
    /// Music asked for this frame.
    pub fd: u8,
    /// Pulse 2 and triangle effects asked for this frame.
    pub fe: u8,
    /// Pulse 1 effects asked for this frame.
    pub ff: u8,
    pub playing: u8,
    pub jingle_at: u8,
    pub length_table: u8,
    pub p2_length: u8,
    pub p1_left: u8,
    pub p2_left: u8,
    pub tri_left: u8,
    pub tri_fx: u8,
    pub jingle_now: u8,
    pub p2_fx: u8,
    pub sweep: u8,
    pub sweep_down: u8,
}

/// The RAM address of each byte, in the order of [`Sound::bytes`].
pub const ADDRS: [u16; 27] = [
    0xF0, 0xF1, 0xF2, 0xF3, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
    0x0102, 0x0680, 0x068D, 0x0691, 0x0695, 0x0696, 0x0698, 0x06A1, 0x06A3, 0x06A5, 0x06F0, 0x06F1,
];

impl Sound {
    pub fn bytes(&self) -> [u8; 27] {
        [
            self.f0,
            self.f1,
            self.f2,
            self.f3,
            self.f5,
            self.f6,
            self.f7,
            self.f8,
            self.f9,
            self.fa,
            self.fb,
            self.fc,
            self.fd,
            self.fe,
            self.ff,
            self.playing,
            self.jingle_at,
            self.length_table,
            self.p2_length,
            self.p1_left,
            self.p2_left,
            self.tri_left,
            self.tri_fx,
            self.jingle_now,
            self.p2_fx,
            self.sweep,
            self.sweep_down,
        ]
    }

    pub fn from_bytes(b: [u8; 27]) -> Self {
        Self {
            f0: b[0],
            f1: b[1],
            f2: b[2],
            f3: b[3],
            f5: b[4],
            f6: b[5],
            f7: b[6],
            f8: b[7],
            f9: b[8],
            fa: b[9],
            fb: b[10],
            fc: b[11],
            fd: b[12],
            fe: b[13],
            ff: b[14],
            playing: b[15],
            jingle_at: b[16],
            length_table: b[17],
            p2_length: b[18],
            p1_left: b[19],
            p2_left: b[20],
            tri_left: b[21],
            tri_fx: b[22],
            jingle_now: b[23],
            p2_fx: b[24],
            sweep: b[25],
            sweep_down: b[26],
        }
    }

    /// One frame of the driver: the requests in `ff`, `fe` and `fd` are
    /// taken and cleared, and `fc` is left as the game set it. `rom` reads
    /// a ROM byte by its address.
    pub fn step(&mut self, rom: &dyn Fn(u16) -> u8) {
        let _ = self.step_apu(rom, &mut || 0);
    }

    /// [`Self::step`], with the APU register writes the driver makes, in
    /// order, each as the register's offset from $4000 and the value.
    /// `random` gives the game's random byte $18, which effect 8's pitch
    /// reads ($FC62), called only on a frame that reads it; nothing the
    /// driver keeps depends on it.
    pub fn step_apu(
        &mut self,
        rom: &dyn Fn(u16) -> u8,
        random: &mut dyn FnMut() -> u8,
    ) -> Vec<(u8, u8)> {
        self.step_cost(rom, random).0
    }

    /// [`Self::step_apu`], with the CPU cycles the game's driver takes for
    /// the step: from the handler's `JSR $FA48` to the RTS that ends it,
    /// the JSR included (core `cost.frame`, docs/cycles.md). The step
    /// answers each of the driver's branches by the 27 bytes (and on a
    /// frame effect 8 plays, bit 3 of `random`), and the cycles are the
    /// walk of the lifted code along that path ([`Route`]): 0 where the
    /// code isn't lifted.
    pub fn step_cost(
        &mut self,
        rom: &dyn Fn(u16) -> u8,
        random: &mut dyn FnMut() -> u8,
    ) -> (Vec<(u8, u8)>, i64) {
        let mut o = Apu {
            rom,
            random,
            writes: vec![(0x17, 0xC0)],
            r: Route::new(),
        };
        self.driver(&mut o);
        self.ff = 0;
        self.fe = 0;
        self.fd = 0;
        // $FA58: the sweep between $90 and $D8.
        let a = self.sweep;
        if !o.r.br(0xFA5D, a >= 0x90) {
            self.sweep_down = 0;
        }
        if !o.r.br(0xFA66, a < 0xD8) {
            self.sweep_down = self.sweep_down.wrapping_add(1);
        }
        let t = a >> 6;
        let mut down = self.sweep_down != 0;
        let mut v = a;
        if !o.r.br(0xFA78, down) {
            v = a.wrapping_add(t).wrapping_add(1);
            down = v == 0;
            o.r.br(0xFA7D, !down);
        }
        if down {
            v = v.wrapping_sub(t).wrapping_sub(1);
        }
        self.sweep = v;
        let read = |a: u16| rom(a);
        let cy = o.r.walk(&read, 0xFA48).map_or(0, |c| c + 6);
        (o.writes, cy)
    }

    fn driver(&mut self, o: &mut Apu<'_>) {
        let req = self.ff;
        let mut ff = self.ff;
        let mut f0 = self.f0;
        // Each test of $F0's next bit is a LSR A and a BCS, of $FF's a LSR
        // $FF and a BCS, at `pc`.
        let mut shift_ff = |o: &mut Apu<'_>, pc: u16| {
            let c = ff & 1 != 0;
            ff >>= 1;
            o.r.br(pc, c)
        };
        let mut shift_f0 = |o: &mut Apu<'_>, pc: u16| {
            let c = f0 & 1 != 0;
            f0 >>= 1;
            o.r.br(pc, c)
        };
        // $FBF2: pulse 1's effects, the lowest bit first, one playing kept
        // before one asked for.
        if shift_f0(o, 0xFBF7) {
            return self.p1_blip(o);
        }
        if shift_ff(o, 0xFBFB) {
            self.f0 = req;
            self.f1 = 0x71;
            self.f2 = 0;
            o.pulse1(0x9F, 0x7F);
            return self.p1_blip(o);
        }
        if o.r.br(0xFBFF, self.fa != 0) {
            // $FC4B: no effect while the music's second channel plays.
            o.r.always(0xFC4F, true);
            self.f0 = 0;
        } else if shift_f0(o, 0xFC02) {
            self.p1_slide(o);
        } else if shift_ff(o, 0xFC06) {
            self.f0 = req;
            self.f1 = 0x54;
            self.f2 = 0x6A;
            o.pulse1(0x9C, 0x7F);
            self.p1_slide(o);
        } else if shift_f0(o, 0xFC09) {
            self.p1_steps(o);
        } else if shift_ff(o, 0xFC0D) {
            self.f0 = req;
            self.f1 = 0x22;
            self.f2 = 0x0B;
            o.note(0, 0x20);
            self.p1_steps(o);
        } else if shift_f0(o, 0xFC10) {
            self.p1_buzz(o);
        } else if shift_ff(o, 0xFC14) {
            self.f0 = req;
            self.f1 = 0x0A;
            o.w(0x02, self.sweep);
            o.w(0x03, 0x88);
            self.p1_buzz(o);
        }
        self.rest(o);
    }

    /// $FB89: effect 1, which ends by starting song 8 over the music.
    fn p1_blip(&mut self, o: &mut Apu<'_>) {
        let x = self.f2;
        let y = (o.rom)(at::BLIP + u16::from(x));
        o.r.idx(0xFB8B, x);
        self.f1 = self.f1.wrapping_sub(1);
        if o.r.br(0xFB92, self.f1 == 0) {
            self.f0 = 0;
            self.fb = 0;
            self.start_song(8, o);
            return;
        }
        let a = self.f1;
        // (Y, carry) when the period is written ($FBA7).
        let mut write: Option<(u8, bool)> = None;
        if !o.r.br(0xFB96, a & 7 != 0) {
            o.r.idx(0xFB9A, x);
            let (s, c) = add(y >> 1, y, y & 1);
            if o.r.br(0xFB9E, s != 0) {
                write = Some((s, c));
            } else {
                o.r.br(0xFBA2, false);
                self.f2 = self.f2.wrapping_add(1);
                write = Some((0, false));
            }
        } else if !o.r.br(0xFBA2, a & 3 != 0) {
            self.f2 = self.f2.wrapping_add(1);
            write = Some((y, false));
        }
        if let Some((period, c)) = write {
            o.r.br(0xFBAC, !c);
            o.w(0x02, period);
            o.w(0x03, 0x28 + u8::from(c));
        }
        // $FBB2 to $FE00's end.
        o.w(0x08, 0);
        o.w(0x04, 0x10);
    }

    /// $FBC2: effect 2.
    fn p1_slide(&mut self, o: &mut Apu<'_>) {
        let mut y = self.f2;
        let a = self.f1 & 3;
        let carry = if o.r.br(0xFBC8, a == 0) || !o.r.br(0xFBCC, a != 3) {
            if a == 3 {
                self.f2 = self.f2.wrapping_sub(self.f2 >> 3);
                y = self.f2;
            }
            let (s, c) = add(y >> 1, self.f2, y & 1);
            y = s;
            c
        } else {
            false
        };
        // $FBD9: three ROLs through the carry into $4002, a fourth into
        // $4003.
        let (mut v, mut c) = (y, carry);
        for _ in 0..3 {
            let out = v & 0x80 != 0;
            v = v << 1 | u8::from(c);
            c = out;
        }
        o.w(0x02, v);
        o.w(0x03, v << 1 | u8::from(c));
        if !o.r.br(0xFBE8, self.f1 >= 0x18) {
            o.r.always(0xFBF0, true);
            o.w(0x00, self.f1 >> 1 | 0x90);
        }
        self.count_down(o);
    }

    /// $FC28: effect 4.
    fn p1_steps(&mut self, o: &mut Apu<'_>) {
        self.f2 = self.f2.wrapping_sub(1);
        if !o.r.br(0xFC2A, self.f2 != 0) {
            self.f2 = 7;
        }
        let y = (o.rom)(0xFAF5 + u16::from(self.f2));
        o.r.idx(0xFC32, self.f2);
        let x = if !o.r.br(0xFC3B, self.f1 >= 0x14) {
            self.f1 >> 1 | 0x50
        } else {
            0x5A
        };
        o.pulse1(x, y);
        self.count_down(o);
    }

    /// $FC62: effect 8.
    fn p1_buzz(&mut self, o: &mut Apu<'_>) {
        let x = ((o.random)() & 8).wrapping_add(self.f1).wrapping_add(0xFE);
        let y = (o.rom)(0xFAE5u16.wrapping_add(u16::from(x)));
        o.r.idx(0xFC6C, x);
        o.r.always(0xFC71, true);
        o.pulse1(0x41, y);
        self.count_down(o);
    }

    /// $FC44, where effects 2, 4 and 8 count down.
    fn count_down(&mut self, o: &mut Apu<'_>) {
        self.f1 = self.f1.wrapping_sub(1);
        if !o.r.br(0xFC46, self.f1 != 0) {
            o.r.always(0xFC4F, true);
            o.w(0x00, 0x90);
            self.f0 = 0;
        }
    }

    /// $FC90 on: pulse 2's effect, the triangle's, the jingle and the music.
    fn rest(&mut self, o: &mut Apu<'_>) {
        self.pulse2(o);
        if !o.r.br(0xFC9F, self.fc != 0) && !o.r.br(0xFCA3, self.f9 != 0) {
            self.triangle(o);
        }
        if !o.r.br(0xFD0D, self.fa != 0) {
            self.jingle(o);
        }
        if o.r.br(0xFD5A, self.fd != 0) {
            let y = top_bit(self.fd, &mut o.r);
            self.fb = y;
            self.start_song(y, o);
        } else if o.r.br(0xFD5F, self.playing != 0) {
            self.music(false, o);
        }
    }

    /// $FC90.
    fn pulse2(&mut self, o: &mut Apu<'_>) {
        if o.r.br(0xFC92, self.f3 != 0) {
            return;
        }
        if !o.r.br(0xFC97, self.p2_fx != 0) {
            if !o.r.br(0xFC9B, self.fe & 0x80 != 0) {
                return;
            }
            // $FC73: effect 7's start.
            self.p2_fx = 0x0E;
            o.w(0x05, 0x85);
            o.note(4, 0x46);
        }
        // $FC7F.
        self.p2_fx = self.p2_fx.wrapping_sub(1);
        if !o.r.br(0xFC82, self.p2_fx == 0) {
            o.r.always(0xFC8E, true);
            o.w(0x04, (self.p2_fx | 0x90).wrapping_sub(1));
        }
    }

    /// $FCA5.
    fn triangle(&mut self, o: &mut Apu<'_>) {
        let y = self.fe;
        let a = self.tri_fx;
        let mut fe = self.fe;
        // A LSR $FE and its BCS at `pc`.
        let mut shift_fe = |o: &mut Apu<'_>, pc: u16| {
            let c = fe & 1 != 0;
            fe >>= 1;
            o.r.br(pc, c)
        };
        let start = |s: &mut Self, o: &mut Apu<'_>, a: u8| {
            // $FCDD.
            o.r.always(0xFCEE, true);
            s.tri_fx = y;
            s.f5 = 0x0E;
            o.w(0x08, 0xFF);
            o.w(0x0B, 0x08);
            s.tri_end(o, a);
        };
        if shift_fe(o, 0xFCAC) {
            o.r.always(0xFCBC, true);
            return start(self, o, 0x28);
        }
        if o.r.br(0xFCAF, a & 1 != 0) {
            if !o.r.br(0xFCC0, self.f5 != 0) && shift_fe(o, 0xFCC4) {
                return start(self, o, 0xFE);
            }
            // $FCC6: five LSRs of $F6 and an ADC.
            let f6 = self.f6;
            let c = (f6 >> 4) & 1;
            let (v, carry) = add(f6 >> 5, f6, c);
            if o.r.br(0xFCCF, !carry) {
                return self.tri_end(o, v);
            }
            return self.tri_stop(o);
        }
        if o.r.br(0xFCB2, a & 2 != 0) {
            if o.r.br(0xFCF4, self.f5 == 0) {
                return self.tri_stop(o);
            }
            if o.r.br(0xFCF8, self.f5 == 7) {
                return self.tri_end(o, 0xFE);
            }
            let f6 = self.f6;
            return self.tri_end(o, f6.wrapping_sub(f6 >> 2));
        }
        if shift_fe(o, 0xFCB6) {
            start(self, o, 0xFE);
        } else {
            o.r.always(0xFCB8, true);
        }
    }

    /// $FCD1.
    fn tri_stop(&mut self, o: &mut Apu<'_>) {
        o.r.always(0xFCD9, true);
        self.tri_fx = 0;
        o.w(0x08, 0);
    }

    /// $FD00.
    fn tri_end(&mut self, o: &mut Apu<'_>, a: u8) {
        self.f6 = a;
        o.w(0x0A, a);
        if !o.r.br(0xFD07, self.f5 == 0) {
            self.f5 -= 1;
        }
    }

    /// $FD0F.
    fn jingle(&mut self, o: &mut Apu<'_>) {
        if !o.r.br(0xFD11, self.fc != 0) {
            o.r.always(0xFD16, true);
            self.jingle_now = 0;
            return;
        }
        let mut restart = self.fc != self.jingle_now;
        if o.r.br(0xFD1B, !restart) {
            self.tri_left = self.tri_left.wrapping_sub(1);
            if o.r.br(0xFD38, self.tri_left != 0) {
                return;
            }
        }
        loop {
            if restart {
                // $FD1D.
                self.jingle_now = self.fc;
                let y = top_bit(self.fc, &mut o.r);
                o.r.idx(0xFD25, y);
                o.r.always(0xFD33, true);
                self.jingle_at = (o.rom)(at::JINGLE_STARTS + u16::from(y));
                self.f5 = (o.rom)(at::JINGLE_LO);
                self.f6 = (o.rom)(at::JINGLE_HI);
            }
            // $FD3A.
            let y = self.jingle_at;
            self.jingle_at = self.jingle_at.wrapping_add(1);
            let jingle = u16::from(self.f5) | u16::from(self.f6) << 8;
            o.r.ptr(0xFD40, jingle, y);
            let note = (o.rom)(jingle.wrapping_add(u16::from(y)));
            if o.r.br(0xFD42, note == 0) {
                // A jingle that starts with its end byte (its data not
                // lifted, so read as 0) would loop here for ever, as the
                // game's driver would; the model leaves the frame instead.
                if restart {
                    return;
                }
                restart = true;
                continue;
            }
            let i = length_index(note);
            o.r.idx(0xFD4D, i);
            self.tri_left = (o.rom)(at::JINGLE_LENGTHS + u16::from(i));
            o.w(0x08, 0x10);
            o.note(8, note & 0x3E);
            return;
        }
    }

    /// $FD67: song `y` from the song table.
    fn start_song(&mut self, y: u8, o: &mut Apu<'_>) {
        let rom = o.rom;
        let off = rom(at::SONGS + u16::from(y));
        o.r.idx(0xFD67, y);
        for pc in [0xFD6B, 0xFD71, 0xFD76, 0xFD7B, 0xFD80] {
            o.r.idx(pc, off);
        }
        let h = at::SONGS + u16::from(off);
        self.length_table = rom(h);
        self.f7 = rom(h + 1);
        self.f8 = rom(h + 2);
        self.f9 = rom(h + 3);
        self.fa = rom(h + 4);
        self.p1_left = 1;
        self.p2_left = 1;
        self.tri_left = 1;
        self.playing = 1;
        self.f3 = 0;
        self.music(true, o);
    }

    fn data(&self, y: u8, o: &Apu<'_>) -> u8 {
        (o.rom)(self.song().wrapping_add(u16::from(y)))
    }

    /// The song's data, $F7-$F8.
    fn song(&self) -> u16 {
        u16::from(self.f7) | u16::from(self.f8) << 8
    }

    /// The length of the note byte `a` by the song's length table, $FAC4.
    fn length(&self, a: u8, o: &mut Apu<'_>) -> u8 {
        let i = length_index(a).wrapping_add(self.length_table);
        o.r.idx(0xFAD1, i);
        (o.rom)(at::LENGTHS + u16::from(i))
    }

    /// $FD97 (after a start) or $FD9B: the three channels' next notes.
    fn music(&mut self, started: bool, o: &mut Apu<'_>) {
        // The song's first channel, on pulse 1's registers ($FDA4).
        let mut y = 0;
        let mut go = started && o.r.br(0xFD99, self.fb == 0);
        if !go {
            y = self.fa;
            if !o.r.br(0xFD9D, y == 0) {
                self.p2_left = self.p2_left.wrapping_sub(1);
                go = !o.r.br(0xFDA2, self.p2_left != 0);
            }
        }
        if go {
            self.fa = self.fa.wrapping_add(1);
            o.r.ptr(0xFDA6, self.song(), y);
            let mut a = self.data(y, o);
            if o.r.br(0xFDA8, a == 0) {
                return self.song_end(o);
            }
            if !o.r.br(0xFDAA, a & 0x80 == 0) {
                let i = (a & 7).wrapping_add(self.length_table);
                o.r.idx(0xFAD1, i);
                self.p2_length = (o.rom)(at::LENGTHS + u16::from(i));
                let y2 = self.fa;
                self.fa = self.fa.wrapping_add(1);
                o.r.ptr(0xFDB6, self.song(), y2);
                a = self.data(y2, o);
            }
            let wrote = o.note(0, a);
            let x = if !o.r.br(0xFDBB, wrote) {
                o.r.always(0xFDBF, true);
                0
            } else if o.r.br(0xFDC5, self.fb == 0) {
                0x9F
            } else if o.r.br(0xFDCB, self.f9 != 0) {
                0x06
            } else {
                0x86
            };
            o.pulse1(x, 0x7F);
            self.p2_left = self.p2_length;
        }
        // The second, on pulse 2's ($FDD8).
        if !o.r.br(0xFDDA, self.fb == 0) {
            self.p1_left = self.p1_left.wrapping_sub(1);
            if !o.r.br(0xFDDF, self.p1_left != 0) {
                let y = self.f3;
                self.f3 = self.f3.wrapping_add(1);
                o.r.ptr(0xFDE5, self.song(), y);
                let a = self.data(y, o);
                if !o.r.br(0xFDE7, a != 0) {
                    return self.song_end(o);
                }
                self.p1_left = self.length(a, o);
                o.w(0x05, 0x7F);
                let wrote = o.note(4, a & 0x3E);
                let x = if !o.r.br(0xFE17, wrote) {
                    o.r.always(0xFE1B, true);
                    0x10
                } else if o.r.br(0xFE24, self.p1_left >= 0x18) {
                    0x89
                } else if o.r.br(0xFE2A, self.p1_left >= 0x10) {
                    0x86
                } else {
                    0x84
                };
                o.w(0x04, x);
            }
        }
        // The triangle ($FE31).
        let y = self.f9;
        if o.r.br(0xFE33, y == 0) {
            return;
        }
        self.tri_left = self.tri_left.wrapping_sub(1);
        if o.r.br(0xFE38, self.tri_left != 0) {
            return;
        }
        self.f9 = self.f9.wrapping_add(1);
        o.r.ptr(0xFE3C, self.song(), y);
        let a = self.data(y, o);
        self.tri_left = self.length(a, o);
        let l = self.tri_left.wrapping_add(0xFE).wrapping_shl(2);
        o.r.br(0xFE4B, l < 0x38);
        o.r.br(0xFE51, self.fb != 0);
        let l = if self.fb == 0 { 0xFF } else { l.min(0x38) };
        o.w(0x08, l);
        o.note(8, a & 0x3E);
    }

    /// $FDE9.
    fn song_end(&mut self, o: &mut Apu<'_>) {
        if !o.r.br(0xFDF9, self.fb == 0) {
            o.r.br(0xFDFE, self.tri_fx != 0);
        }
        o.w(0x00, 0x90);
        self.fa = 0;
        self.f3 = 0;
        self.f9 = 0;
        self.playing = 0;
        if self.fb == 0 || self.tri_fx == 0 {
            o.w(0x08, 0);
        }
        o.w(0x04, 0x10);
    }
}

/// The APU writes of a step, what the writes read, and the path the
/// step's code takes.
struct Apu<'a> {
    rom: &'a dyn Fn(u16) -> u8,
    random: &'a mut dyn FnMut() -> u8,
    writes: Vec<(u8, u8)>,
    r: Route,
}

impl Apu<'_> {
    fn w(&mut self, reg: u8, v: u8) {
        self.writes.push((reg, v));
    }

    /// $FA95: a pulse 1 setting.
    fn pulse1(&mut self, x: u8, y: u8) {
        self.w(0x00, x);
        self.w(0x01, y);
    }

    /// $FAA1: note `n`'s period to the channel at register `ch`, unless its
    /// low byte in the table ($FB01) is 0, a rest. Whether it wrote. The
    /// entries from $FAB3 and $FABA reach it by an always-taken BNE.
    fn note(&mut self, ch: u8, n: u8) -> bool {
        self.r.always(0xFAB8, true).always(0xFAC2, true);
        let lo = (self.rom)(0xFB01 + u16::from(n));
        self.r.idx(0xFAA2, n);
        if self.r.br(0xFAA5, lo == 0) {
            return false;
        }
        let hi = (self.rom)(0xFB00 + u16::from(n));
        self.r.idx(0xFAAA, n);
        self.w(ch + 2, lo);
        self.w(ch + 3, hi | 0x08);
        true
    }
}

/// $FA86: the highest bit set, 0 for bit 0 or none.
fn top_bit(v: u8, r: &mut Route) -> u8 {
    let mut a = v;
    let mut y = 7u8;
    loop {
        let c = a & 0x80 != 0;
        a <<= 1;
        if r.br(0xFA89, c) {
            return y;
        }
        y -= 1;
        if !r.br(0xFA8C, y != 0) {
            return 0;
        }
    }
}

/// $FAC4: a note byte's length, from its bits 0, 7 and 6.
fn length_index(b: u8) -> u8 {
    (b & 1) << 2 | (b >> 7) << 1 | (b >> 6) & 1
}

/// The 6502's ADC: the sum and the carry out.
fn add(a: u8, b: u8, c: u8) -> (u8, bool) {
    let s = u16::from(a) + u16::from(b) + u16::from(c);
    (s as u8, s > 0xFF)
}

/// The sound driver's counters, in [`ADDRS`]' order.
pub const SOUND_COUNTERS: [&str; 27] = [
    "sound_fx1",
    "sound_fx1_time",
    "sound_fx1_step",
    "sound_p1_at",
    "sound_tri_time",
    "sound_tri_pitch",
    "sound_song_lo",
    "sound_song_hi",
    "sound_tri_at",
    "sound_p2_at",
    "sound_song",
    "sound_fc",
    "sound_fd",
    "sound_fe",
    "sound_ff",
    "sound_music_on",
    "sound_jingle_at",
    "sound_length_table",
    "sound_p2_length",
    "sound_p1_left",
    "sound_p2_left",
    "sound_tri_left",
    "sound_tri_fx",
    "sound_jingle_now",
    "sound_fx2",
    "sound_sweep",
    "sound_sweep_down",
];

/// The sound driver, stepped by the one `sound` body at the start of every
/// frame (core `entity.hook_frame_start`), frozen or not, as the game's
/// NMI runs it before anything else ($C890): it takes the requests the
/// last frame left, from the rules and the hooks alike, and plays through
/// the Start pause, whose tune is song 6. Its table `rom` is the lifted
/// `sound_rom`, the ROM from [`ROM_BASE`].
///
/// Its `step`, every frame the counters run (`hook_through_stops`), keeps
/// the death's sound ($D0C8-$D0F6): the counter `sound_death_wait` ($3A,
/// a frame timer the routine at $F4AC counts down). On the spin's first
/// count (its step counter $88 reaching 4 or 12: the player's word 0,
/// which the player system steps before the hooks run) the game asks for
/// pulse 2's effect ($FE = $80) and sets $3A to $40; on a later count with
/// $3A at $0E or more it asks for pulse 1's effect 1 ($FF = 1) and clears
/// $3A. The counter `sound_death_seen` marks that the first count is past.
/// In the attract demo ($58, the counter `attract`) the game asks for
/// neither effect but keeps the timer, and the spin's last step sets $3A
/// to $7D ($D115-$D11B), which the demo waits out before it ends.
pub struct SoundDriver;

impl Ext for SoundDriver {
    fn frame_start(&self, cx: &mut ExtCx<'_>) {
        let counter = |n: &str| cx.counter(n);
        let mut s = driver_state(&counter);
        // $FA72 leaves $06F0's top two bits in $00 (the dk_idle hook's
        // `scratch_00`), from the sweep before this step's.
        let scratch = i64::from(s.sweep >> 6);
        let spec = cx.spec;
        let kind = spec.kind(cx.me);
        let Some(rom) = reader(kind) else {
            return;
        };
        // Effect 8's random byte: the original's in a lockstep with its
        // draws (the soft site `sound_buzz`), else the register's $18 as
        // the NMI found it, else 0.
        let reg = cx
            .counter("random")
            .map(|r| crate::idle::byte_of(r as u64, 0));
        let (_, cycles) = s.step_cost(&rom, &mut || cx.draw_soft(SITE).or(reg).unwrap_or(0));
        // Core `cost.frame`: the driver's cycles this frame, to the model's
        // terms, which hold none of the driver's.
        cx.add_cycles(cycles);
        if cx.counter("scratch_00").is_some_and(|v| v != scratch) {
            cx.set_counter("scratch_00", scratch);
        }
        cx.me.ext[1] = 1;
        for (name, v) in SOUND_COUNTERS.iter().zip(s.bytes()) {
            if cx.counter(name) != Some(i64::from(v)) {
                cx.set_counter(name, i64::from(v));
            }
        }
    }

    fn step(&self, cx: &mut ExtCx<'_>) {
        death(cx);
        title(cx);
    }

    /// The writes the next frame's step makes, from the same bytes on a
    /// copy. The game turns its channels on ($4015 = $0F) where it asks
    /// for songs 7 and 0 ($C90E, $CA1F), and nowhere else. The body's
    /// first step turned them on too until 2026-11-03, but the body is new
    /// at every start a lockstep loads and every stage restart after a
    /// death, where the original writes nothing: 1 frame of every corpus
    /// clip, and a second at the restart (hammer clip 20, model frame
    /// 2763). Effect 8's
    /// pitch reads the game's random byte $18, which the model doesn't
    /// hold yet: 0, or the original's when a lockstep's draws give it.
    fn apu(&self, spec: &Spec, world: &World, id: usize) -> Vec<(u8, u8)> {
        self.apu_peek(spec, world, id, &|_| None)
    }

    fn apu_peek(
        &self,
        spec: &Spec,
        world: &World,
        id: usize,
        peek: &dyn Fn(&str) -> Option<u8>,
    ) -> Vec<(u8, u8)> {
        let Some(me) = world.bodies.get(id) else {
            return Vec::new();
        };
        let counter = |n: &str| {
            let i = spec.counters.index_of(n)?;
            world.counters.get(i).copied()
        };
        let mut s = driver_state(&counter);
        let Some(rom) = reader(spec.kind(me)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if turns_channels_on(&s) {
            out.push((0x15, 0x0F));
        }
        out.extend(s.step_apu(&rom, &mut || peek(SITE).unwrap_or(0)));
        // After the NMI, the title's routine ($C8F3) and the attract
        // demo's play routine ($CE7C) turn every channel off ($4015 = 0)
        // while no song plays ($0102), so the demo is silent.
        let demo = counter("attract").unwrap_or(0) != 0;
        if s.playing == 0 && (demo || on_title(spec, world)) {
            out.push((0x15, 0x00));
        }
        out
    }
}

/// Whether the frame's driver step comes where the game turns every
/// channel on ($4015 = $0F): where it asks for song 7 or song 0 ($FD at
/// $80 or $01, $C90E and $CA1F). A new sound body (a lockstep's start, a
/// stage restarted after a death) doesn't.
fn turns_channels_on(s: &Sound) -> bool {
    s.fd == 0x80 || s.fd == 0x01
}

/// The soft draw site of effect 8's random byte ($FC62).
const SITE: &str = "sound_buzz";

/// The driver's bytes from the counters.
fn driver_state(counter: &dyn Fn(&str) -> Option<i64>) -> Sound {
    let bytes: [u8; 27] = std::array::from_fn(|i| counter(SOUND_COUNTERS[i]).unwrap_or(0) as u8);
    Sound::from_bytes(bytes)
}

/// A byte of the lifted `sound_rom` by its ROM address, or below
/// [`ROM_BASE`] of the optional table `rom_low` from the address
/// `rom_low_base` (donkey-kong-oe's jingles at $8340 in its lower bank).
fn rom_byte(table: &[i64], low: Option<(&[i64], u16)>, a: u16) -> u8 {
    let (table, base) = match low {
        Some((low, base)) if a < ROM_BASE => (low, base),
        _ => (table, ROM_BASE),
    };
    a.checked_sub(base)
        .and_then(|i| table.get(usize::from(i)))
        .map_or(0, |v| *v as u8)
}

/// The driver's code below the lifted `sound_rom`: $FA45 on, the asset
/// `code_fa45` as the kind's `ext_tables.code`.
const CODE_BASE: u16 = 0xFA45;

/// A reader of the driver's ROM bytes for a kind: `sound_rom`, the code
/// below it ($FA45 to $FAE4) and the optional low table.
fn reader<'a>(kind: &'a gp_sim::spec::Kind) -> Option<impl Fn(u16) -> u8 + 'a> {
    let table = kind.ext_tables.get("rom")?;
    let code = kind.ext_tables.get("code").map(Vec::as_slice);
    let low = low_rom(kind);
    Some(move |a: u16| match code {
        Some(code) if (CODE_BASE..ROM_BASE).contains(&a) => {
            code.get(usize::from(a - CODE_BASE)).map_or(0, |v| *v as u8)
        }
        _ => rom_byte(table, low, a),
    })
}

/// The kind's `rom_low` table and the address it starts at
/// (`ext_params.rom_low_base`), when it has both.
fn low_rom(kind: &gp_sim::spec::Kind) -> Option<(&[i64], u16)> {
    let low = kind.ext_tables.get("rom_low")?;
    let base = kind.ext_params.get("rom_low_base")?.floor();
    Some((low.as_slice(), u16::try_from(base).ok()?))
}

/// Whether the game rule is in `title` or `title_draw`, the frames the
/// title's routine ($C8F3) runs.
fn on_title(spec: &Spec, world: &World) -> bool {
    spec.machines.iter().zip(&world.machines).any(|(m, s)| {
        m.name == "game"
            && m.states
                .get(s.state)
                .is_some_and(|st| st.name == "title" || st.name == "title_draw")
    })
}

/// The title's song ($C8F3-$C911): on a frame the title's routine runs
/// with the counter `title_song` ($0518) at 0, the game asks for song 7
/// ($FD = $80) and sets the counter to 4. An attract demo's start takes
/// one off (the rules, $CA26), so the song comes back every fourth demo.
/// The NMI runs the title's routine ($C8C1) when $4E is set and $55 isn't
/// 0 as the frame starts, so on a frame the game rule is in `title` or
/// `title_draw` and was in one of them as the last frame ended (or the
/// body's first frame, at power-on). The frame that goes back to the title
/// after the demo's death ($CA4A) or a game over ($CA79) sets $55 itself
/// and draws nothing; the routine runs from the next frame (top-hammer
/// clip 1 on donkey-kong-oe, original frames 14261 and 14262). Word 2 of
/// the body: 2 once a frame is seen, plus 1 when it ended on the title.
fn title(cx: &mut ExtCx<'_>) {
    let Some(left) = cx.counter("title_song") else {
        return;
    };
    let now = on_title(cx.spec, cx.world);
    let was = cx.me.ext[2];
    let ran = now && (was & 1 != 0 || was & 2 == 0);
    if ran && left == 0 {
        cx.set_counter("sound_fd", 0x80);
        cx.set_counter("title_song", 4);
    }
    cx.me.ext[2] = 2 | i64::from(now);
}

/// $D0C8's sound for this frame, then $F4AC's count of $3A.
fn death(cx: &mut ExtCx<'_>) {
    let Some(wait) = cx.counter("sound_death_wait") else {
        return;
    };
    let spinning = cx.kind("player").and_then(|k| {
        let b = cx.world.bodies.iter().find(|b| b.alive && b.kind == k)?;
        let m = cx.spec.kinds[k].machine.as_ref()?;
        // The routine runs from the frame after the death: a death in
        // the player's turn (a fall) shows the spin a frame early here
        // (hammer clip 5: $88 at 4 as Mario dies at 9097, the first
        // count at 9105).
        (m.states.get(b.state)?.name == "spinning" && b.time >= 1).then_some(b.ext[0])
    });
    let demo = cx.counter("attract").unwrap_or(0) != 0;
    // The demo's spin just ended: its last step sets $3A to $7D.
    let spin_ended = demo
        && cx.kind("player").is_some_and(|k| {
            cx.world.bodies.iter().any(|b| {
                b.alive
                    && b.kind == k
                    && b.time == 0
                    && cx.spec.kinds[k]
                        .machine
                        .as_ref()
                        .and_then(|m| m.states.get(b.state))
                        .is_some_and(|st| st.name == "demo_down")
            })
        });
    let mut wait = wait;
    // Whether the spin's first count is past: the counter
    // `sound_death_seen`, the game's $98 at 1 to 4, which a start in a
    // death loads.
    let seen = cx.counter("sound_death_seen").unwrap_or(0);
    let mut now_seen = seen;
    // A spin Start stopped in the demo keeps $98 as it was.
    let held = cx.kind("player").is_some_and(|k| {
        cx.world.bodies.iter().any(|b| {
            b.alive
                && b.kind == k
                && cx.spec.kinds[k]
                    .machine
                    .as_ref()
                    .and_then(|m| m.states.get(b.state))
                    .is_some_and(|st| st.name == "spin_stopped")
        })
    });
    match spinning {
        None if held => {}
        None => now_seen = 0,
        Some(step) => {
            // $D0C0 stops the jingle on every frame of the death.
            if cx.counter("sound_fc").is_some_and(|v| v != 0) {
                cx.set_counter("sound_fc", 0);
            }
            let count = matches!(step % 15, 4 | 12);
            if count && seen == 0 {
                now_seen = 1;
                if !demo {
                    cx.set_counter("sound_fe", 0x80);
                }
                wait = 0x40;
            } else if count && wait >= 0x0E {
                if !demo {
                    cx.set_counter("sound_ff", 1);
                }
                wait = 0;
            }
        }
    }
    if spin_ended {
        wait = 0x7D;
    }
    if now_seen != seen {
        cx.set_counter("sound_death_seen", now_seen);
    }
    if wait > 0 {
        wait -= 1;
    }
    if cx.counter("sound_death_wait") != Some(wait) {
        cx.set_counter("sound_death_wait", wait);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The driver's cycles against the original's: each frame of
    /// `gp cycles split --ram-out` in `$GP_SOUND_SPLIT` (NAME.jsonl and
    /// NAME.ram), the 27 bytes and $18 from the RAM at the NMI's entry,
    /// the tables from the program ROM `$GP_SOUND_PRG` (`gp cycles slice`),
    /// against the split's cycles of $FA48. Game data, so it runs only by
    /// hand: `cargo test -p gp-ext-dk -- --ignored driver_cycles`.
    #[test]
    #[ignore = "needs a split of the user's ROM"]
    fn driver_cycles_match_the_split() {
        let (Ok(dir), Ok(prg)) = (
            std::env::var("GP_SOUND_SPLIT"),
            std::env::var("GP_SOUND_PRG"),
        ) else {
            return;
        };
        let prg = std::fs::read(prg).unwrap();
        let base = 0x10000 - prg.len();
        let rom = |a: u16| prg[usize::from(a) - base];
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let p = e.ok()?.path();
                let s = p.to_str()?.strip_suffix(".jsonl")?.to_string();
                Some(s)
            })
            .collect();
        names.sort();
        let (mut n, mut ok) = (0u64, 0u64);
        let mut diffs = std::collections::BTreeMap::<i64, u64>::new();
        let mut shown = 0;
        for name in names {
            let Ok(ram) = std::fs::read(format!("{name}.ram")) else {
                continue;
            };
            let lines = std::fs::read_to_string(format!("{name}.jsonl")).unwrap();
            for (i, line) in lines.lines().enumerate() {
                let Some(r) = ram.get(i * 2048..(i + 1) * 2048) else {
                    break;
                };
                let Some(at) = line.find("\"fa48\":") else {
                    continue;
                };
                let want: i64 = line[at + 7..]
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                let bytes: [u8; 27] = std::array::from_fn(|k| r[usize::from(ADDRS[k])]);
                let mut s = Sound::from_bytes(bytes);
                let (_, got) = s.step_cost(&rom, &mut || r[0x18]);
                n += 1;
                if got == want {
                    ok += 1;
                } else {
                    *diffs.entry(got - want).or_default() += 1;
                    if shown < 12 {
                        shown += 1;
                        eprintln!(
                            "{name} {i}: got {got} want {want} bytes {bytes:02x?} r18 {:02x}",
                            r[0x18]
                        );
                    }
                }
            }
        }
        eprintln!("exact {ok} of {n}; diffs {diffs:?}");
        assert_eq!(ok, n);
    }

    /// Only a request for song 7 or song 0 turns the channels on; a fresh
    /// driver's bytes (all 0, as a new body's step reads them after a
    /// restart) don't. Corpus clips wrote $4015 = $0F where the original
    /// didn't on their first frame and at each restart (hammer clip 20,
    /// model frame 2763).
    #[test]
    fn only_songs_7_and_0_turn_the_channels_on() {
        let with_fd = |fd: u8| {
            let mut b = [0u8; 27];
            b[SOUND_COUNTERS
                .iter()
                .position(|c| *c == "sound_fd")
                .unwrap()] = fd;
            Sound::from_bytes(b)
        };
        assert!(!turns_channels_on(&with_fd(0)));
        assert!(turns_channels_on(&with_fd(0x80)));
        assert!(turns_channels_on(&with_fd(0x01)));
        assert!(!turns_channels_on(&with_fd(0x08)));
    }
}
