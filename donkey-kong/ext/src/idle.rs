//! The idle loop ($C7E1) and the random register it steps ($18-$1F).
//!
//! After the NMI handler returns, Donkey Kong waits for the next NMI in a
//! loop that calls $F4ED: it takes bit 1 of $18 and bit 1 of $19, sets the
//! carry when they differ, and rotates the eight bytes right through it,
//! $18 first. A pass is 76 cycles when the bits are equal (the BEQ at
//! $F4FA taken) and 77 when not (it falls through to SEC), so the frame's
//! spare time decides how many passes fit: 67 to 369 in the user's play
//! (docs/cycles.md). The register is the model's counter `random`, its
//! bytes $18 (the top byte) to $1F (the bottom), and the draws read it
//! ([`draw`]).
//!
//! The `dk_idle` hook runs last in every frame (`entity.hook_last`), as
//! the loop runs after the frame's work whatever it was. With the model's
//! `ext_params.idle_loop` (the retro side of the `rng_per_frame` switch) it
//! runs the loop's instructions on the register from where the last NMI
//! stopped them to where the next one stops them: the handler's cost of
//! the frame ([`handler`]), then passes until the vblank grid's next NMI.
//! Without it (the normal side) the register steps once a frame.
//!
//! Its words: `ext[0]` the NMI's entry, in PPU dots after the frame's
//! vblank; `ext[1]` where the NMI stopped the loop ([`Paused`]); `ext[2]`
//! the frame's parity (bit 0, set on an odd frame) and the CPU cycle's
//! parity at the NMI's entry (bit 1); `ext[3]` the passes of the frame
//! just run, for a frame-overrun test and for display.

use gp_sim::{Ext, ExtCx};

use crate::p;

/// The idle loop's hook.
pub struct Idle;

/// The register's byte at `$18 + byte`, from the counter's value.
pub fn byte_of(reg: u64, byte: u32) -> u8 {
    (reg >> (56 - 8 * byte.min(7))) as u8
}

/// The register after one whole pass of $F4ED.
pub fn step(reg: u64) -> u64 {
    let carry = ((reg >> 57) ^ (reg >> 49)) & 1;
    (reg >> 1) | (carry << 63)
}

/// A random byte for the draw site `site`: the run's draws script's when
/// it has one there, else the register's byte at `$18 + byte` as the
/// original reads it (the ladder choice reads $19, a fire's way
/// `$19 + slot`). A model without the counter `random` draws from the
/// generator.
pub fn draw(cx: &mut ExtCx<'_>, site: &str, byte: u32) -> u8 {
    if let Some(v) = cx.scripted_draw(site) {
        return v;
    }
    match cx.counter("random") {
        Some(r) => byte_of(r as u64, byte),
        None => cx.draw(site),
    }
}

/// Where in the loop an NMI stopped it: the next instruction's index in
/// [`CYCLES`] order (0 is the JSR at $C7E1), the carry the RORs pass on,
/// the bit taken from $18 at $F4ED and the outcome of the EOR at $F4F7.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Paused {
    pub at: u8,
    pub carry: bool,
    pub bit18: bool,
    pub differ: bool,
}

/// Bit 11 of a [`Paused`] word: the hook wrote it, so its `bit18` and
/// `differ` are the loop's own.
const KEPT: i64 = 1 << 11;

impl Paused {
    /// Where the NMI stopped the loop, from the address it pushed and the
    /// carry in the status it pushed (counters `nmi_resume` and
    /// `nmi_carry`, which a lockstep's start loads from the console as it
    /// takes the NMI, feature `field.machine`). The bit of $18 and the EOR's
    /// outcome are the register's: nothing between $F4ED and the RORs
    /// changes it. An address outside the loop (an NMI that came inside
    /// the handler) reads as the loop's start.
    pub fn at_address(pc: i64, carry: bool, reg: u64) -> Self {
        let at = ADDRS.iter().position(|a| i64::from(*a) == pc).unwrap_or(0) as u8;
        let bit18 = byte_of(reg, 0) & 2 != 0;
        Self {
            at,
            carry,
            bit18,
            differ: (byte_of(reg, 1) & 2 != 0) != bit18,
        }
    }

    /// The address the NMI pushed for this place.
    pub fn address(self) -> i64 {
        i64::from(ADDRS[usize::from(self.at % LOOP_LEN)])
    }

    pub fn word(self) -> i64 {
        i64::from(self.at)
            | i64::from(self.carry) << 8
            | i64::from(self.bit18) << 9
            | i64::from(self.differ) << 10
    }

    pub fn of(w: i64) -> Self {
        Self {
            at: (w & 0xFF) as u8 % LOOP_LEN,
            carry: w >> 8 & 1 != 0,
            bit18: w >> 9 & 1 != 0,
            differ: w >> 10 & 1 != 0,
        }
    }
}

/// The loop's instructions in order, with their cycles (MOS Technology,
/// MCS6500 Programming Manual, appendix A): JSR $F4ED; LDA $18; AND #2;
/// STA $00; LDA $19; AND #2; EOR $00; CLC; BEQ (3 taken, 2 not); SEC;
/// eight RORs of $18 to $1F; RTS; JMP $C7E1.
const CYCLES: [u64; 20] = [6, 3, 2, 3, 3, 2, 3, 2, 2, 2, 5, 5, 5, 5, 5, 5, 5, 5, 6, 3];
const LOOP_LEN: u8 = 20;
/// Each instruction's address, in [`CYCLES`] order: $C7E1, $F4ED to $F50D
/// and $C7E4.
const ADDRS: [u16; 20] = [
    0xC7E1, 0xF4ED, 0xF4EF, 0xF4F1, 0xF4F3, 0xF4F5, 0xF4F7, 0xF4F9, 0xF4FA, 0xF4FC, 0xF4FD, 0xF4FF,
    0xF501, 0xF503, 0xF505, 0xF507, 0xF509, 0xF50B, 0xF50D, 0xC7E4,
];
const LDA_18: u8 = 1;
const LDA_19: u8 = 4;
const EOR: u8 = 6;
const CLC: u8 = 7;
const BEQ: u8 = 8;
const SEC: u8 = 9;
const ROR: u8 = 10;

/// What the loop did between two NMIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waited {
    pub reg: u64,
    pub paused: Paused,
    /// The passes begun (the JSR at $C7E1 run).
    pub passes: u32,
    /// The next NMI's entry, in dots after its vblank.
    pub entry: i64,
}

/// Runs the loop on `reg` from `paused`, starting `start` dots after the
/// frame's vblank, until the NMI of the vblank `next` dots after it: the
/// NMI is taken after the first instruction that ends more than a cycle
/// after the vblank, and its entry is 7 cycles later (docs/cycles.md, "What
/// decides the count").
pub fn wait(reg: u64, paused: Paused, start: i64, next: i64) -> Waited {
    resume(reg, paused, start, next, None)
}

/// [`wait`] after a handler that left `scratch` in $00: a pass the NMI
/// stopped after its STA $00 ($F4F1) and before its EOR $00 ($F4F7) goes on
/// with the handler's $00, so the EOR's outcome is bit 1 of $19 against
/// all of `scratch` (A holds $19 AND 2, restored by the handler's PLA).
/// `None` keeps the pass's own.
pub fn resume(
    mut reg: u64,
    mut paused: Paused,
    start: i64,
    next: i64,
    scratch: Option<u16>,
) -> Waited {
    let mut eor = None;
    if let Some(v) = scratch {
        match paused.at {
            // The AND and the EOR are still to come; $19's bit is the one
            // the LDA read.
            5 | 6 => {
                let b19 = paused.differ != paused.bit18;
                paused.differ = u16::from(b19) * 2 != v;
            }
            LDA_19 => eor = Some(v),
            _ => {}
        }
    }
    let mut t = start;
    let mut passes = 0u32;
    loop {
        let at = paused.at;
        if at == 0 {
            passes += 1;
        }
        let mut cost = CYCLES[usize::from(at)];
        let mut then = at + 1;
        match at {
            LDA_18 => paused.bit18 = byte_of(reg, 0) & 2 != 0,
            LDA_19 => {
                let b19 = byte_of(reg, 1) & 2 != 0;
                paused.differ = match eor.take() {
                    Some(v) => u16::from(b19) * 2 != v,
                    None => b19 != paused.bit18,
                };
            }
            EOR | CLC => {
                if at == CLC {
                    paused.carry = false;
                }
            }
            BEQ => {
                if !paused.differ {
                    cost = 3;
                    then = ROR;
                }
            }
            SEC => paused.carry = true,
            _ if (ROR..ROR + 8).contains(&at) => {
                let k = u32::from(at - ROR);
                let shift = 56 - 8 * k;
                let b = (reg >> shift) & 0xFF;
                let out = b & 1 != 0;
                let nb = (b >> 1) | u64::from(paused.carry) << 7;
                reg = (reg & !(0xFF << shift)) | nb << shift;
                paused.carry = out;
            }
            _ => {}
        }
        paused.at = then % LOOP_LEN;
        t += 3 * cost as i64;
        if t > next + 3 {
            return Waited {
                reg,
                paused,
                passes,
                entry: t + 21 - next,
            };
        }
        if passes > 100_000 {
            // No vblank comes: a broken grid. Stop rather than spin.
            return Waited {
                reg,
                paused,
                passes,
                entry: 0,
            };
        }
    }
}

/// The NMI handler's cycles this frame, from its entry to its RTI's end:
/// the model's `cost` (core `cost.frame`, docs/cycles.md, "The cost
/// model"), its terms over the model's state and what hooks added, with
/// the OAM DMA's extra cycle when the write to $4014 lands on an odd CPU
/// cycle (`odd`, the CPU cycle's parity at the NMI's entry, against
/// `ext_params.dma_parity`). A model with no `cost` takes
/// `ext_params.handler`, the user's play's median.
pub fn handler(cx: &ExtCx<'_>, odd: bool) -> i64 {
    match cx.frame_cycles() {
        Some(c) => c + i64::from(odd == (p(cx, "dma_parity") != 0)),
        None => p(cx, "handler"),
    }
}

impl Ext for Idle {
    fn step(&self, cx: &mut ExtCx<'_>) {
        // A stage's start or a restart: Mario drawn unmirrored on its
        // frame, where his own hook doesn't run.
        crate::mario::unface_placed(cx);
        // Mario drawn standing by a wait he didn't die in.
        crate::mario::stand_placed(cx);
        // A held hammer hidden on the frame a touch kills Mario.
        crate::Hammer::hide_on_death(cx);
        let Some(r) = cx.counter("random") else {
            return;
        };
        let mut reg = r as u64;
        // A start of play seeds the top bytes ($C9C3: $18 = $0F, $19 =
        // $13), which the counter `random_seed` carries as its byte count
        // times 65536 plus the bytes, top byte first.
        if let Some(seed) = cx.counter("random_seed").filter(|s| *s > 0) {
            let n = (seed >> 16).clamp(0, 8) as u32;
            if n > 0 {
                let bits = 8 * n;
                let mask = if bits == 64 {
                    u64::MAX
                } else {
                    (1u64 << bits) - 1
                };
                let top = ((seed as u64) & mask) << (64 - bits);
                reg = (reg & !(mask << (64 - bits))) | top;
            }
            cx.set_counter("random_seed", 0);
        }
        if !cx.has_param("idle_loop") {
            cx.set_counter("random", step(reg) as i64);
            cx.me.ext[3] = 1;
            return;
        }
        // Core `cost.frame` with the NMI's place as counters (`nmi_resume`,
        // `nmi_carry`, `nmi_entry`, `nmi_cycle_odd`, `frame_odd`): a start
        // loads them from the console as it takes the NMI, and the hook
        // keeps them. Without them, its words.
        let nmi = NmiPlace::read(cx);
        let odd = nmi.map_or(cx.me.ext[2] & 1 != 0, |n| n.frame_odd);
        let cycle_odd = nmi.map_or(cx.me.ext[2] & 2 != 0, |n| n.cycle_odd);
        let entry = nmi.map_or(cx.me.ext[0], |n| n.entry);
        let paused = match nmi {
            Some(n)
                if Paused::of(cx.me.ext[1]).address() != n.resume || cx.me.ext[1] & KEPT == 0 =>
            {
                Paused::at_address(n.resume, n.carry, reg)
            }
            _ => Paused::of(cx.me.ext[1]),
        };
        let next = if odd && cx.has_param("rendering") {
            p(cx, "frame_dots") - 1
        } else {
            p(cx, "frame_dots")
        };
        crate::play::add(cx);
        let cost = handler(cx, cycle_odd);
        if cx.counter("frame_cost").is_some() {
            cx.set_counter("frame_cost", cost);
        }
        let start = entry + 3 * cost;
        let w = resume(reg, paused, start, next, scratch(cx));
        cx.set_counter("random", w.reg as i64);
        // With a `cost`, the CPU cycles from this NMI's entry to the next
        // one's, whose parity the next frame's DMA reads: whole, since both
        // entries fall on the CPU's cycle grid. A model without one keeps
        // only the frame's parity.
        let elapsed = (next + w.entry - entry).div_euclid(3);
        let next_odd = cx.frame_cycles().is_some() && cycle_odd != (elapsed & 1 != 0);
        cx.me.ext[0] = w.entry;
        cx.me.ext[1] = w.paused.word() | KEPT;
        cx.me.ext[2] = i64::from(!odd) | i64::from(next_odd) << 1;
        cx.me.ext[3] = i64::from(w.passes);
        if nmi.is_some() {
            NmiPlace {
                resume: w.paused.address(),
                carry: w.paused.carry,
                entry: w.entry,
                cycle_odd: next_odd,
                frame_odd: !odd,
            }
            .write(cx);
        }
    }
}

/// $00 as the frame's handler leaves it, for a pass its NMI stopped
/// between $F4F1 and $F4F7 (core `cost.frame`): the counter `scratch_00`,
/// which the dk_sound hook sets to what the driver leaves ($FA72 stores
/// $06F0's top two bits, before its sweep), unless the play routine ran
/// (the game rule in `playing`, not paused), whose last store there was
/// never 0 or 2 on 30 sampled corpus clips (worker rngH2, 2026-11-04: 193
/// frames, the register matching only with the EOR's outcome set). `None`
/// without the counter.
fn scratch(cx: &ExtCx<'_>) -> Option<u16> {
    let v = cx.counter("scratch_00")?;
    let state = |machine: &str, name: &str| {
        cx.spec
            .machines
            .iter()
            .zip(&cx.world.machines)
            .any(|(m, s)| {
                m.name == machine && m.states.get(s.state).is_some_and(|st| st.name == name)
            })
    };
    // The Start pause runs only $CF42 of the play routine, which stores
    // nothing there.
    let playing = state("game", "playing") && !state("pause", "paused");
    Some(if playing { 0x100 } else { v as u16 })
}

/// The next NMI as model state: where it stops the loop (the address it
/// pushes and the carry), its entry in PPU dots after its vblank, the CPU
/// cycle's parity at the entry and whether its frame is odd. A model holds
/// it as five counters, all or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NmiPlace {
    pub resume: i64,
    pub carry: bool,
    pub entry: i64,
    pub cycle_odd: bool,
    pub frame_odd: bool,
}

impl NmiPlace {
    fn read(cx: &ExtCx<'_>) -> Option<Self> {
        Some(Self {
            resume: cx.counter("nmi_resume")?,
            carry: cx.counter("nmi_carry")? & 1 != 0,
            entry: cx.counter("nmi_entry")?,
            cycle_odd: cx.counter("nmi_cycle_odd")? & 1 != 0,
            frame_odd: cx.counter("frame_odd")? & 1 != 0,
        })
    }

    fn write(self, cx: &mut ExtCx<'_>) {
        for (name, v) in [
            ("nmi_resume", self.resume),
            ("nmi_carry", i64::from(self.carry)),
            ("nmi_entry", self.entry),
            ("nmi_cycle_odd", i64::from(self.cycle_odd)),
            ("frame_odd", i64::from(self.frame_odd)),
        ] {
            if cx.counter(name) != Some(v) {
                cx.set_counter(name, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole pass of the instruction list steps the register as $F4ED
    /// does and costs 76 cycles with the bits equal, 77 with them apart.
    #[test]
    fn a_pass_is_one_step_and_76_or_77_cycles() {
        for reg in [
            0x0F13_0000_0000_0000u64,
            0x0213_4567_89AB_CDEF,
            0x8000_0000_0000_0001,
        ] {
            let equal = (reg >> 57 & 1) == (reg >> 49 & 1);
            let cost = if equal { 76 } else { 77 };
            // Start at the JSR, and let the NMI's vblank fall in the next
            // pass's JSR: one pass, then the second begun.
            let w = wait(reg, Paused::default(), 0, 3 * cost + 3);
            assert_eq!(w.passes, 2, "{reg:x}");
            assert_eq!(w.paused.at, 1);
            assert_eq!(w.reg, step(reg), "{reg:x}");
        }
    }

    /// Stopped in the middle of the RORs, the loop goes on from there
    /// with the carry it kept: two halves make one step.
    #[test]
    fn a_pass_stopped_by_the_nmi_goes_on_after_it() {
        let reg = 0x0213_4567_89AB_CDEFu64;
        let first = wait(reg, Paused::default(), 0, 3 * 30);
        assert!(first.paused.at > ROR && first.paused.at < ROR + 8);
        let mut rest = first;
        while rest.paused.at != 0 {
            rest = wait(rest.reg, rest.paused, 0, 0);
        }
        assert_eq!(rest.reg, step(reg));
    }

    /// From one NMI's entry to the next is whole CPU cycles, so the next
    /// frame's DMA reads the parity the hook carries.
    #[test]
    fn entries_are_whole_cycles_apart() {
        for (entry, handler, next) in [
            (21i64, 10207i64, 89342i64),
            (24, 9720, 89341),
            (22, 1649, 89342),
        ] {
            let w = wait(
                0x0F13_0000_0000_0000,
                Paused::default(),
                entry + 3 * handler,
                next,
            );
            assert_eq!(
                (next + w.entry - entry).rem_euclid(3),
                0,
                "{entry} {handler} {next}"
            );
        }
    }

    #[test]
    fn a_zero_register_stays_zero() {
        assert_eq!(step(0), 0);
        assert_eq!(byte_of(0x0F13_0000_0000_0000, 1), 0x13);
    }

    #[test]
    fn paused_words_round_trip() {
        let p = Paused {
            at: 13,
            carry: true,
            bit18: false,
            differ: true,
        };
        assert_eq!(Paused::of(p.word()), p);
    }
}
