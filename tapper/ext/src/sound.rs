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
//! first 6 bytes from E0E1, the model's own, never loaded, so every
//! request sent is one a hook made (its next 6 in `ext[2]` from bit 16,
//! 4 more in `ext[0]` from bit 32; `ext[0]` keeps the bartender's E728,
//! his walk's next foot, at bit 24); `ext[2]` its own: bit 0 the coin held
//! on the frame before, and from bit 8 the frames of the boot's sound
//! check run so far (255 when it is done); `ext[3]` the frame's latch writes, 16 bits each
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

/// The queue holds this many bytes: 6 in `ext[1]`, 6 more in `ext[2]`
/// from bit 16 and 4 in `ext[0]` from bit 32 (the game's has no bound;
/// a level's start asks for 16 at once).
const QUEUE: usize = 16;
/// The boot's sound check done.
const DONE: i64 = 255;

/// A byte of one of the `sound` entity's rows, the program's code lifted
/// from the player's romset.
pub(crate) fn code(cx: &ExtCx<'_>, rows: &str, at: i64) -> Option<i64> {
    let k = cx.kind("sound")?;
    let t = cx.spec.kinds[k].ext_tables.get(rows)?;
    t.get(usize::try_from(at).ok()?).copied()
}

/// Where the queue's byte `i` is kept: the word and its bit.
fn place(i: usize) -> (usize, i64) {
    match i {
        0..=5 => (1, 8 * (i as i64 + 1)),
        6..=11 => (2, 16 + 8 * (i as i64 - 6)),
        _ => (0, 32 + 8 * (i as i64 - 12)),
    }
}

/// A body's queue, its bytes in order (E114 of them from E0E1).
fn load(ext: &[i64; 4]) -> Vec<i64> {
    let n = usize::try_from(ext[1] & 0xff).unwrap_or(0).min(QUEUE);
    (0..n)
        .map(|i| {
            let (w, at) = place(i);
            (ext[w] >> at) & 0xff
        })
        .collect()
}

/// Puts a queue back in a body's words; the places past its end are 0.
fn store(ext: &mut [i64; 4], q: &[i64]) {
    ext[1] = q.len() as i64;
    ext[2] &= 0xffff;
    ext[0] &= 0xffff_ffff;
    for (i, &b) in q.iter().enumerate().take(QUEUE) {
        let (w, at) = place(i);
        ext[w] |= (b & 0xff) << at;
    }
}

/// Puts `byte` in a queue as $606B does, dropping it in state 6 unless it
/// is 2, 8 or 9.
fn queue(q: &mut Vec<i64>, byte: i64, state: i64) {
    if state == 6 && ![2, 8, 9].contains(&byte) {
        return;
    }
    if q.len() < QUEUE {
        q.push(byte & 0xff);
    }
}

/// The game's state, E397, as the flow's word 3 keeps it.
fn state(cx: &ExtCx<'_>) -> i64 {
    // The flow's own hook asks with the state it has just set.
    if cx.kind("flow") == Some(cx.me.kind) {
        return cx.me.ext[3] & 0xff;
    }
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
    let mut q = load(&b.ext);
    for byte in bytes {
        queue(&mut q, byte, st);
    }
    store(&mut b.ext, &q);
    cx.set_body(id, b);
}

/// As `ask`, but only the bytes that are an `ld c,N`'s (the row before is
/// $0E): a place where another set's program has other code asks for
/// nothing there.
pub(crate) fn ask_ld(cx: &mut ExtCx<'_>, rows: &str, at: &[i64]) {
    let at: Vec<i64> = at.iter().copied().filter(|&a| code(cx, rows, a - 1) == Some(0x0E)).collect();
    ask(cx, rows, &at);
}

/// The requests a routine starts with, from `at` in the `rows`: each
/// `ld c,N` then `call` (five bytes), until other code.
pub(crate) fn ask_run(cx: &mut ExtCx<'_>, rows: &str, at: i64) {
    let mut at = at;
    let mut bytes = Vec::new();
    while code(cx, rows, at) == Some(0x0E) && code(cx, rows, at + 2) == Some(0xCD) {
        bytes.push(at + 1);
        at += 5;
    }
    ask(cx, rows, &bytes);
}

/// The setup's last requests ($4C4E: 2, then the bar shown's byte of the
/// table at $4C71) on the next frame, after its wait ($4C4B's $5E99): bit
/// 1 of the body's `ext[2]` until the frame's start asks for them.
pub(crate) fn ask_setup_next(cx: &mut ExtCx<'_>) {
    let Some(k) = cx.kind("sound") else { return };
    let Some(id) = cx.world.bodies.iter().position(|b| b.kind == k && b.alive) else {
        return;
    };
    let mut b = cx
        .requests()
        .iter()
        .rev()
        .find_map(|r| match r {
            Request::SetBody(i, b) if *i == id => Some(*b),
            _ => None,
        })
        .unwrap_or(cx.world.bodies[id]);
    b.ext[2] |= 2;
    cx.set_body(id, b);
}

/// The bartender's walk's step ($19EF left, $1C02 right): as E727 runs
/// out with E729 at 3, $0F with E728's bit 0 clear, then $10, the bit
/// turned each time (E728 in the body's `ext[0]` from bit 24, loaded).
pub(crate) fn footstep(cx: &mut ExtCx<'_>, left: bool) {
    let Some(k) = cx.kind("sound") else { return };
    let Some(id) = cx.world.bodies.iter().position(|b| b.kind == k && b.alive) else {
        return;
    };
    let mut b = cx
        .requests()
        .iter()
        .rev()
        .find_map(|r| match r {
            Request::SetBody(i, b) if *i == id => Some(*b),
            _ => None,
        })
        .unwrap_or(cx.world.bodies[id]);
    let foot = (b.ext[0] >> 24) & 1;
    // The `sound_walk` rows are the program from $1A0C: $0F at $1A0C and
    // $1C1F, $10 at $1A18 and $1C2B.
    let at = match (left, foot) {
        (true, 0) => 0x01,
        (true, _) => 0x0D,
        (false, 0) => 0x214,
        (false, _) => 0x220,
    };
    b.ext[0] = (b.ext[0] & !(0xff << 24)) | ((foot ^ 1) << 24);
    cx.set_body(id, b);
    ask(cx, "sound_walk", &[at]);
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
        let mut q = load(&cx.me.ext);
        let mut w0 = cx.me.ext[0] & 0xffff_ffff;
        let mut writes: Vec<i64> = Vec::new();
        // $0379: a coin counted asks for 2 (E113 is never set) and 9.
        let coins = (w0 >> 8) & 0xff;
        if coins > 0 {
            w0 = (w0 & !0xff00) | ((coins - 1) << 8);
            for at in [1, 6] {
                if let Some(b) = code(cx, "sound_coin", at) {
                    queue(&mut q, b, st);
                }
            }
        }
        // $0391: the queue sent, three bytes at a time.
        if !q.is_empty() {
            let sent = q.len().min(3);
            for i in 0..3 {
                writes.push(write(i as i64 + 1, q.get(i).copied().filter(|_| i < sent).unwrap_or(0)));
            }
            q.drain(..sent);
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
        // A run that didn't start at power-on has had its boot:
        // tapper.attract.001 starts in state 0 as the boot ends. A run
        // resumed a few frames after power-on (the publish run starts
        // tapper.poweron.001 a frame in) is that far into the check.
        if boot == 0 && cx.frame > 1 {
            boot = if cx.frame <= 8 { i64::try_from(cx.frame).unwrap_or(1) - 1 } else { DONE };
        }
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
                    queue(&mut q, b, st);
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
        let w2 = (cx.me.ext[2] & 0xfc) | (boot << 8) | i64::from(held);
        // A setup's last requests, after its wait ($4C4E, $4C5C).
        if cx.me.ext[2] & 2 != 0 {
            let shown = cx
                .kind("flow")
                .and_then(|k| cx.world.bodies.iter().find(|b| b.kind == k))
                .map_or(0, |b| (b.ext[3] >> 8) & 3);
            for at in [0x11B, 0x13D + shown] {
                if let Some(b) = code(cx, "sound_game", at) {
                    queue(&mut q, b, st);
                }
            }
        }
        if cx.me.ext[2] & 1 != 0 && !held {
            let coins = ((w0 >> 8) & 0xff) + 1;
            let credits = ((w0 >> 16) & 0xff) + 1;
            w0 = (w0 & !0xff_ff00) | ((coins & 0xff) << 8) | ((credits & 0xff) << 16);
        }
        // A game's start asks from tapper_flow, where the main loop's
        // $584F takes it ($4B34, $4B39).
        let mut w3 = 0;
        for (i, w) in writes.iter().take(4).enumerate() {
            w3 |= w << (16 * i);
        }
        let mut ext = [w0, 0, w2, w3];
        store(&mut ext, &q);
        cx.me.ext = ext;
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
