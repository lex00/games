//! The sound requests: what the main CPU asks the sound board for, a byte
//! at a time, through the board's four latches (gp_sim::Ext::sound_requests).
//!
//! The game asks for a sound with $606B (C the request): it puts the byte
//! in the queue at E0E1 (E0DF the next place, E114 how many) unless the
//! game is in state 6 (E397) and the byte isn't 2, 8 or 9 ($6070 to
//! $607F; with the DIP switches at their defaults, demo sounds on, $6081
//! lets the rest through). The frame's interrupt ($00D8) sends the queue
//! at the start of the next frame ($0379): up to three bytes to latches 1,
//! 2 and 3, zeros for the rest ($03CA), the others kept for the frame
//! after ($03AD), and then E111 turned over (bit 7) to latch 0 ($03D9 to
//! $03E5), which tells the board a request is in. Before it sends, a coin
//! counted ($0241, E099) asks for 2 and 9 ($0380 to $038E).
//!
//! Every request byte is read from the player's romset: the `ld c,N` the
//! code asks with, by the routine's place in the program (the `sound_*`
//! rows), or the arrival routine's own first instruction. The model holds
//! no request byte of its own.
//!
//! The words of the `sound` body: `ext[0]` E111 (the turn bit), E099 (the
//! coins to answer) at bit 8 and E011 (credits) at 16, which a clip's start
//! loads; `ext[1]` the queue: its length (E114) in the low byte, then its
//! bytes from E0E1 (6 at most), the model's own, never loaded, so every
//! request sent is one a hook made; `ext[2]` its own: bit 0 the coin held on the frame
//! before, and from bit 8 the frames of the boot's sound check run so far
//! (255 when it is done); `ext[3]` the frame's latch writes, 16 bits each
//! from the low end (bit 10 set, the latch at bit 8, the byte below).

use gp_sim::ext::Request;
use gp_sim::mechanic::{Carrier, Param, Primitive, Record};
use gp_sim::spec::Spec;
use gp_sim::world::World;
use gp_sim::{Ext, ExtCx};

/// The sound board's requests ($606B, $0379).
pub struct Sound;

const PARAMS: &[Param] = &[
    Param::new("check_first", "frames", "frames"),
    Param::new("check_second", "frames", "frames"),
    Param::new("check_patterns", "frames", "frames"),
    Param::new("check_step", "frames", "frames"),
    Param::new("check_ready", "frames", "frames"),
];

/// The queue holds this many bytes in the body's word (the game's has 16).
const QUEUE: i64 = 6;
/// The boot's sound check done.
const DONE: i64 = 255;

/// A byte of one of the `sound` entity's rows, the program's code lifted
/// from the player's romset.
pub(crate) fn code(cx: &ExtCx<'_>, rows: &str, at: i64) -> Option<i64> {
    let k = cx.kind("sound")?;
    let t = cx.spec.kinds[k].ext_tables.get(rows)?;
    t.get(usize::try_from(at).ok()?).copied()
}

/// Puts `byte` in a body's queue as $606B does, dropping it in state 6
/// unless it is 2, 8 or 9.
fn queue(word1: &mut i64, byte: i64, state: i64) {
    if state == 6 && ![2, 8, 9].contains(&byte) {
        return;
    }
    let n = *word1 & 0xff;
    if n >= QUEUE {
        return;
    }
    let at = 8 * (n + 1);
    *word1 = (*word1 & !(0xff << at) & !0xff) | ((byte & 0xff) << at) | (n + 1);
}

/// The game's state, E397, as the flow's word 3 keeps it.
fn state(cx: &ExtCx<'_>) -> i64 {
    cx.kind("flow")
        .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
        .map_or(10, |b| b.ext[3] & 0xff)
}

/// Asks the sound board for the bytes at `at` in the `rows` (a hook's
/// call of $606B): queued in the `sound` body, sent at the next frame's
/// start. A byte the rows haven't, or a model with no `sound` body, asks
/// for nothing.
pub(crate) fn ask(cx: &mut ExtCx<'_>, rows: &str, at: &[i64]) {
    let Some(k) = cx.kind("sound") else { return };
    let Some(id) = cx.world.bodies.iter().position(|b| b.kind == k && b.alive) else {
        return;
    };
    let bytes: Vec<i64> = at.iter().filter_map(|&a| code(cx, rows, a)).collect();
    if bytes.is_empty() {
        return;
    }
    let st = state(cx);
    // This step's change to the body so far, if it made one.
    let mut b = cx
        .requests()
        .iter()
        .rev()
        .find_map(|r| match r {
            Request::SetBody(i, b) if *i == id => Some(*b),
            _ => None,
        })
        .unwrap_or(cx.world.bodies[id]);
    for byte in bytes {
        queue(&mut b.ext[1], byte, st);
    }
    cx.set_body(id, b);
}

/// A latch write as `ext[3]` keeps it.
fn write(latch: i64, byte: i64) -> i64 {
    0x400 | ((latch & 3) << 8) | (byte & 0xff)
}

fn p(cx: &ExtCx<'_>, name: &str) -> i64 {
    cx.param(name).floor()
}

impl Ext for Sound {
    fn step(&self, _cx: &mut ExtCx<'_>) {}

    /// The frame's interrupt ($00D8 calls $0379), before the frame's work;
    /// then the work's own requests the hooks don't make: the boot's sound
    /// check, a coin counted, a game started.
    fn frame_start(&self, cx: &mut ExtCx<'_>) {
        let st = state(cx);
        let (mut w0, mut w1) = (cx.me.ext[0], cx.me.ext[1]);
        let mut writes: Vec<i64> = Vec::new();
        // $0379: a coin counted asks for 2 (E113 is never set) and 9.
        let coins = (w0 >> 8) & 0xff;
        if coins > 0 {
            w0 = (w0 & !0xff00) | ((coins - 1) << 8);
            for at in [1, 6] {
                if let Some(b) = code(cx, "sound_coin", at) {
                    queue(&mut w1, b, st);
                }
            }
        }
        // $0391: the queue sent, three bytes at a time.
        let n = w1 & 0xff;
        if n > 0 {
            let sent = n.min(3);
            for i in 0..3 {
                let b = if i < sent {
                    (w1 >> (8 * (i + 1))) & 0xff
                } else {
                    0
                };
                writes.push(write(i + 1, b));
            }
            let rest = (((w1 as u64) >> 8) >> (8 * sent)) as i64;
            w1 = (rest << 8) | (n - sent);
            let turn = (w0 & 0xff) ^ 0x80;
            w0 = (w0 & !0xff) | turn;
            writes.push(write(0, turn));
        }
        // The boot's sound check, from power-on to the game's first frame:
        // while the game's state is 0 (E397, cleared by the boot's RAM
        // test; a clip starts in a later state). $CFC5 asks for 2 at once
        // and for 2 and 1 after its waits ($DB69, a frame each), then
        // writes each byte of the table at $D06A to all four latches, 3
        // first ($D000 to $D00A), a pattern after each six waits, and reads
        // the board's answer; $00B9 to $00C6 ask for 2, 2 and 3 once the
        // board is ready.
        let mut boot = (cx.me.ext[2] >> 8) & 0xff;
        if st == 0 && boot != DONE {
            boot += 1;
            let (first, second) = (p(cx, "check_first"), p(cx, "check_second"));
            let (patterns, step, ready) = (
                p(cx, "check_patterns"),
                p(cx, "check_step"),
                p(cx, "check_ready"),
            );
            let asks: &[(&str, i64)] = if boot == first {
                &[("sound_check", 1)]
            } else if boot == second {
                &[("sound_check", 0x18), ("sound_check", 0x1D)]
            } else if boot == ready {
                &[("sound_boot", 1)]
            } else if boot == ready + 1 {
                &[("sound_boot", 1), ("sound_boot", 12)]
            } else {
                &[]
            };
            for &(rows, at) in asks {
                if let Some(b) = code(cx, rows, at) {
                    queue(&mut w1, b, st);
                }
            }
            if boot >= patterns
                && (boot - patterns) % step.max(1) == 0
                && (boot - patterns) / step.max(1) < 4
            {
                let k = (boot - patterns) / step.max(1);
                if let Some(b) = code(cx, "sound_check", 0xA5 + k) {
                    for latch in [3, 2, 1, 0] {
                        writes.push(write(latch, b));
                    }
                }
            }
            if boot > ready {
                boot = DONE;
            }
        }
        // The coin handler ($0213, from the timer's interrupt in the
        // frame): a coin is counted when it is let go ($0229 to $0241),
        // and a credit given ($024C, one a coin as the DIP switches are).
        let held = cx.held("coin");
        let w2 = (cx.me.ext[2] & !0xff01) | (boot << 8) | i64::from(held);
        if cx.me.ext[2] & 1 != 0 && !held {
            let coins = ((w0 >> 8) & 0xff) + 1;
            let credits = ((w0 >> 16) & 0xff) + 1;
            w0 = (w0 & !0xff_ff00) | ((coins & 0xff) << 8) | ((credits & 0xff) << 16);
        }
        // A game's start ($4AEF from the start's press with a credit):
        // $4B34 and $4B39 ask for 2 and 2A.
        let credits = (w0 >> 16) & 0xff;
        if cx.pressed("start1") && credits > 0 {
            w0 = (w0 & !0xff_0000) | ((credits - 1) << 16);
            for at in [1, 6] {
                if let Some(b) = code(cx, "sound_game", at) {
                    queue(&mut w1, b, st);
                }
            }
        }
        let mut w3 = 0;
        for (i, w) in writes.iter().take(4).enumerate() {
            w3 |= w << (16 * i);
        }
        cx.me.ext = [w0, w1, w2, w3];
    }

    fn sound_requests(&self, _spec: &Spec, world: &World, id: usize) -> Vec<(u8, u8)> {
        let w = world.bodies[id].ext[3];
        (0..4)
            .map(|i| (w >> (16 * i)) & 0xffff)
            .filter(|e| e & 0x400 != 0)
            .map(|e| (((e >> 8) & 3) as u8, (e & 0xff) as u8))
            .collect()
    }

    fn describe(&self) -> Vec<Record> {
        vec![Record {
            name: "tapper.sound",
            version: env!("CARGO_PKG_VERSION"),
            carrier: Carrier::Game,
            system: "platformer.enemies",
            class: "sound",
            implementation: "gp-ext-tapper::Sound",
            like: "",
            about: "The main CPU's requests to the sound board: queued by the hooks as the game's code asks, sent three bytes a frame to the board's latches with the turn bit on latch 0",
            params: PARAMS,
            state: &[
                "ext[0] E111 the turn bit, E099 coins to answer, E011 credits",
                "ext[1] the queue's length (E114) and its bytes from E0E1",
                "ext[2] the coin held last frame, the boot's sound check frames",
                "ext[3] the frame's latch writes",
            ],
            reads: &["action", "flow.ext"],
            writes: &["self.ext"],
            built_from: &[
                Primitive::InputBinding,
                Primitive::Timer,
                Primitive::Condition,
            ],
            ..Record::EMPTY
        }]
    }
}
