//! A Tapper player for making long input logs: it reads gp-arcade's RAM
//! each frame, decides the controls, and backs up to an earlier frame and
//! tries again when a life is lost. It only writes input logs (the MAME
//! oracle's `gp-arcade-input 0`) and a list of the phases it saw; the logs
//! are what count, and MAME replays them.
//!
//!   gp mechanic autoplay --rom $GP_ROM_DIR/arcade/tapper.zip \
//!       --out DIR/NAME [--levels N] [--idle]
//!
//! It is the game's own driver, kept in the game's folder: gameprinter's
//! `gp mechanic autoplay` builds and runs it (checks.toml [autoplay]), and
//! `gp mechanic triage` runs that. `cargo run --release -- ...` here runs it
//! alone.
//!
//! With `--search`, a life lost isn't retried from the same choices: the
//! player backs up a few seconds (`--back`, 150 frames, further on each
//! loss there) and runs a beam search over short moves (pour, up, down,
//! wait, walk left or right) to two seconds past the loss, each position
//! valued by playing on with the plain policy for `--horizon` frames
//! (150): frames survived, customers served, levels cleared and score.
//! The beam is `--width` (4) times the try, the moves are tried in a fixed
//! order and ties keep that order, so a run is the same every time.
//! `--jobs` (4) threads value the moves. Only the input log is kept.
//!
//! It writes NAME.input and NAME.phases when it ends, and the log so far
//! as NAME-partial.input as it goes (after the `--from` log, every 600
//! frames and after each search), so a run that is stopped keeps its work.
//! `--from LOG` goes on from an earlier play's log, its presses kept as
//! they were. `--levels N` stops after the N-th level's end sequence;
//! `--idle` coins up, starts and then presses nothing until the game is
//! over (a game-over play). `--until COND` plays to a goal: a condition on
//! the board as `gp query` and `run_until` read it (`ram[0xE70F] == 0 &&
//! ram[0xE397] == 10`), checked after every frame. The player stops on the
//! frame it first holds, a search values a way that reaches it above any
//! other and stops there, and the log's notes say whether it held.
//!
//! What it reads, all in the main CPU's work RAM (found by experiment on
//! the `tapper` set, which shares its code with tapperg):
//!
//! | Address | What |
//! |---|---|
//! | `E262 + 0x20*bar + 4*i` | Bar `bar`'s objects (0 bottom to 3 top), 8 of them, each as a sprite: y, flags, code, x. y 0 is an empty slot |
//! | `E2E6 + 4*i` | The bartender's sprites and what flies from the taps, the same way; 0x18 entries to E345 |
//! | `E4A7` | Spare lives |
//! | `E4A8` | Levels cleared |
//! | `E3B8` | The bar shown: 0 the saloon, 1 the sports bar, ... |
//! | `E3BD` | 1 in attract, 0 in a game, 2 at game over |
//! | `E3F5` | The score, six decimal digits, one a byte |
//!
//! A bar's line is y = 0x39 + 0x30*bar: customers stand on it, a customer
//! drinking is at y - 4, a full mug sliding out is at y - 5, an empty mug
//! sliding back (code 0x17) or a tip lying still is at y - 3, and the
//! bartender at y - 9. Each bar's tap is a sprite at E346 + 4*bar, its
//! flags' bit 4 set when it is on the left.

use std::fmt::Write as _;
use std::path::PathBuf;

use gp_runtime::select::Condition;

use gp_arcade::{Controls, Mcr, Romset};

const BAR_Y: [u8; 4] = [0x39, 0x69, 0x99, 0xc9];

fn peek(t: &Mcr, a: u16) -> u8 {
    t.peek(a)
}

#[derive(Debug, Clone, Copy)]
struct Obj {
    y: u8,
    code: u8,
    x: u8,
}

/// Every object slot in use, with its slot address.
fn objects(t: &Mcr) -> Vec<(u16, Obj)> {
    let mut out = Vec::new();
    let mut a = 0xe262u16;
    while a < 0xe346 {
        let y = peek(t, a);
        if y != 0 {
            out.push((
                a,
                Obj {
                    y,
                    code: peek(t, a + 2),
                    x: peek(t, a + 3),
                },
            ));
        }
        a += 4;
    }
    out
}

/// What the player sees.
#[derive(Debug, Default, Clone)]
struct Seen {
    /// The bartender's bar and x, when he's on screen.
    bart: Option<(usize, u8)>,
    customers: [Vec<u8>; 4],
    drinking: [Vec<u8>; 4],
    mugs: [Vec<(u8, u8)>; 4],
    other: Vec<(u16, Obj)>,
}

fn see(t: &Mcr) -> Seen {
    let mut s = Seen::default();
    let objs = objects(t);
    for &(a, o) in &objs {
        // In u16: a slot's y can be near 255 (a made-up board's or an
        // object off the screen), and y + 9 would overflow a u8.
        let y = u16::from(o.y);
        let bar = BAR_Y
            .iter()
            .position(|&b| [0, 4, 5, 9].iter().any(|d| y + d == u16::from(b)));
        match bar {
            Some(k) if o.y == BAR_Y[k] && a < 0xe2e2 => s.customers[k].push(o.x),
            Some(k) if o.y + 4 == BAR_Y[k] && a < 0xe2e2 => s.drinking[k].push(o.x),
            Some(k) if o.y + 5 == BAR_Y[k] => s.mugs[k].push((o.code, o.x)),
            Some(k) if o.y + 9 == BAR_Y[k] && a == 0xe2e6 => s.bart = Some((k, o.x)),
            _ => s.other.push((a, o)),
        }
    }
    s
}

/// Whether bar `k`'s tap is on the left (the tap's sprite at E346 +
/// 4*k is flipped, bit 4 of its flags), and where the bartender stands at
/// it: 16 pixels right of a tap on the left, 22 left of one on the right.
/// The saloon's taps are all on the right, the sports bar's all on the
/// left, and the punk bar has both.
fn tap(t: &Mcr, k: usize) -> (bool, i32) {
    let a = 0xe346 + 4 * k as u16;
    let left = peek(t, a + 1) & 0x10 != 0;
    let x = i32::from(peek(t, a + 3));
    (left, if left { x + 16 } else { x - 22 })
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The player's own state between frames: what it is doing now.
#[derive(Clone, Default)]
struct Mind {
    /// Frames the tap has been held, and how long to hold it.
    pour: Option<(u32, u32)>,
    /// Frames to keep the controls released after a pour or a move.
    rest: u32,
    /// A move under way: the bar it left and frames waited.
    moving: Option<(usize, u32)>,
    /// Last frame's mug x by slot, to tell full mugs (going left) from
    /// empty ones (coming back right).
    mug_x: Vec<(u16, u8)>,
    /// Last frame's customer x by slot, to leave out customers being
    /// pushed back to the door (they want nothing more).
    cust_x: Vec<(u16, u8)>,
    /// Decisions to wait before trying a pour again that was found to be
    /// wasted.
    hold_off: u32,
    /// A bar chosen ahead by trying each ([`decide`]), kept until the
    /// bartender is there.
    force: Option<usize>,
}

/// Whether pouring now (the tap held `want` frames) and then doing nothing
/// for 70 frames loses a life that doing nothing at all doesn't.
fn wasted(t: &Mcr, want: u32) -> bool {
    let none = Controls::default();
    let lost = |pour: bool| {
        let mut u = t.clone();
        let lives = peek(&u, 0xe4a7);
        for i in 0..(want + 72) {
            let c = if pour && i < want {
                Controls { tap: true, ..none }
            } else {
                none
            };
            u.step(c);
            if peek(&u, 0xe4a7) != lives || peek(&u, 0xe3bd) == 2 {
                return true;
            }
        }
        false
    };
    lost(true) && !lost(false)
}

/// One decision.
fn decide(t: &Mcr, m: &mut Mind, rng: &mut Rng, wild: u64, deep: bool) -> Controls {
    let none = Controls::default();
    let s = see(t);
    let taps: Vec<(bool, i32)> = (0..4).map(|k| tap(t, k)).collect();
    // Full mugs (on the bar line less 5, going away from the tap) and
    // empty ones (code 0x17 on the bar line less 3, coming back to it; a
    // tip lies still on the same line).
    let mut empties: [Vec<(i32, i32)>; 4] = Default::default();
    let mut fulls = [0usize; 4];
    let mut now_x = Vec::new();
    let mut a = 0xe262u16;
    while a < 0xe346 {
        let y = peek(t, a);
        let x = peek(t, a + 3);
        if let Some(k) = BAR_Y
            .iter()
            .position(|&b| y != 0 && u16::from(y) + 5 == u16::from(b))
        {
            fulls[k] += 1;
        } else if let Some(k) = BAR_Y
            .iter()
            .position(|&b| y != 0 && u16::from(y) + 3 == u16::from(b) && peek(t, a + 2) == 0x17)
        {
            now_x.push((a, x));
            let before = m.mug_x.iter().find(|(s, _)| *s == a).map(|p| p.1);
            if let Some(b) = before {
                let step = i32::from(x) - i32::from(b);
                if step != 0 && (step < 0) == taps[k].0 {
                    empties[k].push((i32::from(x), step.abs()));
                }
            }
        }
        a += 4;
    }
    m.mug_x = now_x;
    // Customers walking to the tap or standing; not those sliding back.
    let mut wanting: [Vec<i32>; 4] = Default::default();
    let mut cust_now = Vec::new();
    for k in 0..4 {
        for i in 0..8u16 {
            let a = 0xe262 + 0x20 * k as u16 + 4 * i;
            if peek(t, a) != BAR_Y[k] {
                continue;
            }
            let x = peek(t, a + 3);
            cust_now.push((a, x));
            let before = m.cust_x.iter().find(|(s, _)| *s == a).map(|p| p.1);
            let away = before.is_some_and(|b| if taps[k].0 { x > b } else { x < b });
            if !away {
                wanting[k].push(i32::from(x));
            }
        }
    }
    m.cust_x = cust_now;
    let Some((bar, _)) = s.bart else {
        m.pour = None;
        m.moving = None;
        return none;
    };
    if let Some((held, want)) = m.pour {
        if held < want {
            m.pour = Some((held + 1, want));
            return Controls { tap: true, ..none };
        }
        m.pour = None;
        m.rest = 2;
    }
    if m.rest > 0 {
        m.rest -= 1;
        return none;
    }
    if let Some((from, waited)) = m.moving {
        if bar == from && waited < 12 {
            m.moving = Some((from, waited + 1));
            return none;
        }
        m.moving = None;
    }
    // Each bar's trouble: frames until a customer reaches the end or an
    // empty mug falls off it.
    let mut trouble: Vec<(i32, usize, bool)> = Vec::new();
    let mut far: [Option<i32>; 4] = [None; 4];
    for k in 0..4 {
        let (left, end) = taps[k];
        let dist = |x: i32| if left { x - end } else { end - x };
        let mut cs: Vec<i32> = wanting[k].iter().map(|&x| dist(x)).collect();
        cs.sort_unstable();
        // Full mugs on the way serve the nearest customers first.
        let uncovered: Vec<i32> = cs.into_iter().skip(fulls[k]).collect();
        if let Some(&d) = uncovered.first() {
            // A customer takes the bartender about 10 pixels short of the
            // tap, and walks up to a pixel a frame.
            trouble.push(((d - 10).max(0) * 3 / 2, k, false));
        }
        // A customer near the door goes out of it when a mug hits them,
        // and the level ends when every customer has gone out.
        if let Some(&d) = uncovered.last() {
            far[k] = Some(d);
        }
        for &(x, speed) in &empties[k] {
            trouble.push((dist(x) / speed, k, true));
        }
    }
    if trouble.is_empty() {
        return none;
    }
    trouble.sort();
    // Nothing urgent: serve the customer nearest a door, on this bar if
    // there is one.
    if trouble[0].0 > 60 {
        let here = far[bar].map(|d| d + 40);
        let best = (0..4)
            .filter_map(|k| far[k].map(|d| (if k == bar { here.unwrap_or(d) } else { d }, k)))
            .max();
        if let Some((_, k)) = best
            && let Some(i) = trouble.iter().position(|p| p.1 == k && !p.2)
        {
            let p = trouble.remove(i);
            trouble.insert(0, p);
        }
    }
    // Now and then, when retrying, take the second most urgent.
    let mut pick = if trouble.len() > 1 && wild > 0 && rng.below(4) < wild.min(3) {
        trouble[1]
    } else {
        trouble[0]
    };
    if let Some(k) = m.force {
        if let Some(p) = trouble.iter().find(|p| p.1 == k) {
            pick = *p;
        }
        if k == bar {
            m.force = None;
        }
    } else if deep && trouble.len() > 1 && trouble[1].0 < 150 {
        // Two bars need the bartender soon: play each choice ahead with
        // this same policy and keep the one that loses no life longest.
        let mut bars: Vec<usize> = Vec::new();
        for p in &trouble {
            if !bars.contains(&p.1) && bars.len() < 3 {
                bars.push(p.1);
            }
        }
        let mut best: Option<(u32, usize)> = None;
        for &k in &bars {
            let mut u = t.clone();
            let mut mm = m.clone();
            mm.force = Some(k);
            let mut r = Rng(rng.0);
            let lives = peek(&u, 0xe4a7);
            let mut lasted = 0;
            while lasted < 240 {
                let c = decide(&u, &mut mm, &mut r, 0, false);
                u.step(c);
                if peek(&u, 0xe4a7) != lives || peek(&u, 0xe3bd) == 2 {
                    break;
                }
                lasted += 1;
            }
            if best.is_none_or(|(b, _)| lasted > b) {
                best = Some((lasted, k));
            }
        }
        if let Some((_, k)) = best
            && let Some(p) = trouble.iter().find(|p| p.1 == k)
        {
            pick = *p;
            if k != bar {
                m.force = Some(k);
            }
        }
    }
    if deep && std::env::var_os("AUTOPLAY_TROUBLE").is_some() {
        eprintln!(
            "t {} bar {bar} trouble {trouble:?} pick {pick:?} force {:?}",
            t.frame(),
            m.force
        );
    }
    let (_, target, empty) = pick;
    if target == bar {
        if empty && wanting[bar].len() <= fulls[bar] {
            return none;
        }
        let want = 8 + if wild > 0 { rng.below(6) as u32 } else { 0 };
        if m.hold_off > 0 {
            m.hold_off -= 1;
            return none;
        }
        // A mug nobody takes falls off the far end: try the pour ahead,
        // and leave it if it costs a life that waiting doesn't.
        if deep && wasted(t, want) {
            m.hold_off = 6;
            return none;
        }
        m.pour = Some((1, want));
        return Controls { tap: true, ..none };
    }
    m.moving = Some((bar, 0));
    // The bartender wraps from the top bar to the bottom and back, a bar
    // every 7 frames, so go the short way round.
    if (target + 4 - bar) % 4 <= 2 {
        Controls { up: true, ..none }
    } else {
        Controls { down: true, ..none }
    }
}

/// One move of the search ([`search`]): a few frames of controls the
/// bartender carries out whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    /// Hold the tap 8 frames (a full mug) and rest 2.
    Pour,
    /// Press up or down once and wait until the bartender is on the next
    /// bar (a bar every 7 frames), at most 12 frames.
    Up,
    Down,
    /// Stand still 6 frames.
    Wait,
    /// Walk 6 frames along the bar, to catch a mug or take a tip.
    Left,
    Right,
}

const ACTS: [Act; 6] = [
    Act::Pour,
    Act::Up,
    Act::Down,
    Act::Wait,
    Act::Left,
    Act::Right,
];

/// Whether a life was lost (or the game ended) since `lives` was read.
fn lost(t: &Mcr, lives: u8) -> bool {
    peek(t, 0xe4a7) < lives || peek(t, 0xe3bd) == 2
}

/// Plays `act` on `t`, keeping its controls in `log`. False when a life is
/// lost on the way.
/// With a goal, the act stops (alive) on the frame the goal first holds.
fn play_act(
    t: &mut Mcr,
    log: &mut Vec<Controls>,
    act: Act,
    lives: u8,
    goal: Option<&Condition>,
) -> bool {
    let none = Controls::default();
    let mut at_goal = false;
    // Whether to go on: no life lost and the goal not reached.
    let mut step = |t: &mut Mcr, c: Controls| {
        t.step(c);
        log.push(c);
        at_goal = at_goal || reached(t, goal);
        !lost(t, lives) && !at_goal
    };
    // Between plays (a level's start or end, the bonus round) the
    // bartender isn't at the bars: a frame of nothing.
    let Some((bar, _)) = see(t).bart else {
        step(t, none);
        return !lost(t, lives);
    };
    match act {
        Act::Pour => {
            for i in 0..10 {
                let c = if i < 8 {
                    Controls { tap: true, ..none }
                } else {
                    none
                };
                if !step(t, c) {
                    return !lost(t, lives);
                }
            }
            !lost(t, lives)
        }
        Act::Up | Act::Down => {
            let c = if act == Act::Up {
                Controls { up: true, ..none }
            } else {
                Controls { down: true, ..none }
            };
            if !step(t, c) {
                return !lost(t, lives);
            }
            for _ in 0..12 {
                if see(t).bart.is_some_and(|(b, _)| b != bar) {
                    break;
                }
                if !step(t, none) {
                    return !lost(t, lives);
                }
            }
            !lost(t, lives)
        }
        Act::Wait | Act::Left | Act::Right => {
            let c = match act {
                Act::Left => Controls { left: true, ..none },
                Act::Right => Controls {
                    right: true,
                    ..none
                },
                _ => none,
            };
            let _ = (0..6).all(|_| step(t, c));
            !lost(t, lives)
        }
    }
}

/// Whether the goal holds on the board now; never without one. A goal
/// that can't be read (checked at the start) counts as not held.
fn reached(t: &Mcr, goal: Option<&Condition>) -> bool {
    goal.is_some_and(|g| gp_arcade::protocol::holds_on(t, g).unwrap_or(false))
}

/// What a goal reached is worth to the search: more than any number of
/// frames survived, less the frames it took.
const GOAL_VALUE: i64 = 1 << 40;

/// Customers left to serve in the level (E711).
fn customers_left(t: &Mcr) -> u32 {
    u32::from(peek(t, 0xe711))
}

/// How good a position is: the heuristic player ([`decide`]) plays on
/// from it for `horizon` frames; frames survived count most, then the
/// customers served and the levels cleared, then the score. A goal that
/// holds on the way is worth more than all of it, the sooner the better.
fn value(t: &Mcr, horizon: u32, goal: Option<&Condition>) -> i64 {
    if reached(t, goal) {
        return GOAL_VALUE;
    }
    let mut u = t.clone();
    let lives = peek(&u, 0xe4a7);
    let mut m = Mind::default();
    let mut r = Rng(0x2545_f491_4f6c_dd1d);
    let mut survived = 0;
    while survived < horizon {
        let c = decide(&u, &mut m, &mut r, 0, false);
        u.step(c);
        if reached(&u, goal) {
            return GOAL_VALUE - i64::from(survived) - 1;
        }
        if lost(&u, lives) {
            break;
        }
        survived += 1;
    }
    let (cleared, _) = level(&u);
    i64::from(survived) * 1000 + i64::from(cleared) * 200_000 - i64::from(customers_left(&u)) * 300
        + i64::from(score(&u)) / 10
}

/// A position in the search: the machine and the controls that reached it
/// from the search's start.
#[derive(Clone)]
struct Node {
    t: Mcr,
    log: Vec<Controls>,
    value: i64,
}

/// A beam search from `start` until frame `until` with no life lost: at
/// each step every position in the beam tries each [`Act`], each result
/// is valued by [`value`], and the `width` best go on (ties kept in the
/// order tried, so the result is the same on every run). Positions are
/// compared by their state's hash, so two ways to the same state count
/// once. The controls from `start` to the first position at `until`, and
/// whether it got there: when every way loses a life, the controls to the
/// furthest position any way reached, so the next search starts from
/// further on. With a goal, the first position where it holds ends the
/// search as one that got there.
fn search(
    start: &Mcr,
    until: u64,
    width: usize,
    horizon: u32,
    jobs: usize,
    goal: Option<&Condition>,
) -> (Vec<Controls>, bool) {
    let lives = peek(start, 0xe4a7);
    let mut beam = vec![Node {
        t: start.clone(),
        log: Vec::new(),
        value: 0,
    }];
    let mut furthest: Vec<Controls> = Vec::new();
    loop {
        if let Some(done) = beam.iter().find(|n| n.t.frame() >= until) {
            return (done.log.clone(), true);
        }
        if let Some(n) = beam.iter().max_by_key(|n| n.log.len())
            && n.log.len() > furthest.len()
        {
            furthest.clone_from(&n.log);
        }
        let tries: Vec<(usize, Act)> = (0..beam.len())
            .flat_map(|i| ACTS.iter().map(move |&a| (i, a)))
            .collect();
        let mut kids: Vec<Option<Node>> = vec![None; tries.len()];
        let chunk = tries.len().div_ceil(jobs.max(1));
        std::thread::scope(|sc| {
            for (part, out) in tries.chunks(chunk).zip(kids.chunks_mut(chunk)) {
                let beam = &beam;
                sc.spawn(move || {
                    for (&(i, a), o) in part.iter().zip(out.iter_mut()) {
                        let mut n = beam[i].clone();
                        if play_act(&mut n.t, &mut n.log, a, lives, goal) {
                            n.value = value(&n.t, horizon, goal);
                            *o = Some(n);
                        }
                    }
                });
            }
        });
        let mut kids: Vec<Node> = kids.into_iter().flatten().collect();
        if let Some(done) = kids.iter().find(|n| reached(&n.t, goal)) {
            return (done.log.clone(), true);
        }
        if kids.is_empty() {
            return (furthest, false);
        }
        // Best first; a stable sort keeps the order tried among equals.
        kids.sort_by_key(|n| std::cmp::Reverse(n.value));
        let mut seen = std::collections::HashSet::new();
        kids.retain(|n| seen.insert(n.t.state_hash()));
        kids.truncate(width);
        beam = kids;
    }
}

/// A saved point to back up to.
#[derive(Clone)]
struct Point {
    t: Mcr,
    log_len: usize,
    mind: Mind,
}

/// The score: six decimal digits, one a byte, at E3F5.
fn score(t: &Mcr) -> u32 {
    (0..6).fold(0, |s, i| s * 10 + u32::from(peek(t, 0xe3f5 + i) % 10))
}

/// Levels cleared (E4A8) and the bar shown (E3B8: 0 the saloon, 1 the
/// sports bar, ...).
fn level(t: &Mcr) -> (u8, u8) {
    (peek(t, 0xe4a8), peek(t, 0xe3b8))
}

/// What is on the screen, coarsely, for the phase list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The bartender at the bars.
    Play,
    /// The bonus round's six cans.
    Bonus,
    /// Anything else in a game: a level's start or end, a life lost.
    Other,
    /// Attract mode, before a game (E3BD 1).
    Attract,
    /// Game over and the initials (E3BD 2).
    GameOver,
}

fn mode(t: &Mcr) -> Mode {
    match peek(t, 0xe3bd) {
        1 => return Mode::Attract,
        2 => return Mode::GameOver,
        _ => {}
    }
    if (0..6).any(|i| peek(t, 0xe262 + 8 * i) == 0x89 && peek(t, 0xe264 + 8 * i) == 0x57) {
        Mode::Bonus
    } else if see(t).bart.is_some() {
        Mode::Play
    } else {
        Mode::Other
    }
}

/// The bonus round is waiting for a choice: the bartender stands behind
/// the cans (codes 5C to 5E). The slots they're in vary: bar 0's in the
/// first bonus round, bar 1's in the second.
fn choosing(t: &Mcr) -> bool {
    mode(t) == Mode::Bonus && (0..0x39u16).any(|i| (0x5c..=0x5e).contains(&peek(t, 0xe264 + 4 * i)))
}

/// The bonus round's choice, found by trying each: hold left or right a while,
/// then press the tap, and keep what scores most.
fn bonus_choice(t: &Mcr) -> Vec<Controls> {
    let none = Controls::default();
    let mut best: Option<(u32, Vec<Controls>)> = None;
    // Left of where the bartender stands, and the can or two right of it.
    for k in -6..24i32 {
        let way = if k < 0 {
            Controls {
                right: true,
                ..none
            }
        } else {
            Controls { left: true, ..none }
        };
        let mut plan = vec![way; k.unsigned_abs() as usize * 4];
        plan.extend([none; 2]);
        plan.extend([Controls { tap: true, ..none }; 3]);
        let mut u = t.clone();
        for &c in &plan {
            u.step(c);
        }
        for _ in 0..400 {
            u.step(none);
        }
        let s = score(&u);
        if best.as_ref().is_none_or(|(b, _)| s > *b) {
            best = Some((s, plan));
        }
    }
    best.map(|b| b.1).unwrap_or_default()
}

/// What `--help` prints: the player's own flags, as `main_with` reads them.
const USAGE: &str = "\
Tapper's auto-player: plays the romset from power-on and writes NAME.input
(an oracle input log) and NAME.phases when it ends, and the log so far as
NAME-partial.input as it goes (every 600 frames and after each search).

Usage: gp mechanic autoplay --rom FILE --out NAME [FLAGS]

  --rom FILE            The Tapper romset (a .zip)
  --out NAME            Where to write NAME.input and NAME.phases (default play)
  --frames N            Stop when the log holds N frames (default 200000)
  --levels N            Stop after the N-th level's end sequence
  --tries N             Tries at each life lost before keeping the loss (default 24)
  --idle                Coin up, start, then press nothing until the game is over
  --search              After a life lost, back up and search for a way past it
  --jobs N              Threads that value the search's moves (default 4)
  --horizon N           Frames each searched position is played on for (default 150)
  --width N             The search's beam, times the try (default 4)
  --back N              Frames to back up on the first loss, more after (default 150)
  --from LOG            Go on from an earlier play's input log
  --until COND          Play to a goal and stop on the frame it first holds: a
                        condition on the board as gp query reads it, such as
                        'ram[0xE70F] == 0 && ram[0xE397] == 10'; the search
                        values a way that reaches it above all others
  --trace LOG FROM TO   Print what the player sees on frames FROM to TO of LOG, and stop
  -h, --help            Print this
";

/// Runs the player with command-line style arguments (without the
/// program's name): `--rom FILE --out NAME` and the flags above.
fn main_with(args: impl IntoIterator<Item = String>) {
    let mut args = args.into_iter();
    let mut rom = PathBuf::new();
    let mut frames = 200_000usize;
    let mut out = PathBuf::from("play");
    let mut idle = false;
    let mut levels = u8::MAX;
    let mut trace = None;
    let mut tries = 24u32;
    let mut from: Option<PathBuf> = None;
    let mut search_on = false;
    let mut jobs = 4usize;
    let mut horizon = 150u32;
    let mut width0 = 4usize;
    let mut back_step = 150u64;
    let mut until: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rom" => rom = args.next().unwrap().into(),
            "--frames" => frames = args.next().unwrap().parse().unwrap(),
            "--out" => out = args.next().unwrap().into(),
            "--levels" => levels = args.next().unwrap().parse().unwrap(),
            "--tries" => tries = args.next().unwrap().parse().unwrap(),
            "--idle" => idle = true,
            "--search" => search_on = true,
            "--jobs" => jobs = args.next().unwrap().parse().unwrap(),
            "--horizon" => horizon = args.next().unwrap().parse().unwrap(),
            "--width" => width0 = args.next().unwrap().parse().unwrap(),
            "--back" => back_step = args.next().unwrap().parse().unwrap(),
            "--from" => from = Some(PathBuf::from(args.next().unwrap())),
            "--until" => until = Some(args.next().unwrap()),
            "--help" | "-h" => {
                print!("{USAGE}");
                return;
            }
            "--trace" => {
                trace = Some((
                    PathBuf::from(args.next().unwrap()),
                    args.next().unwrap().parse::<u64>().unwrap(),
                    args.next().unwrap().parse::<u64>().unwrap(),
                ))
            }
            _ => panic!("unknown {a}"),
        }
    }
    let goal = match until.as_deref().map(Condition::parse).transpose() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("--until {}: {e}", until.unwrap_or_default());
            return;
        }
    };
    let set = Romset::load_zip(&rom).unwrap();
    if let Some((file, from, to)) = trace {
        print_trace(&set, &file, from, to);
        return;
    }
    let mut t = Mcr::power_on(&set);
    // A goal that names nothing the board has is refused before any play.
    if let Some(g) = &goal
        && let Err(e) = gp_arcade::protocol::holds_on(&t, g)
    {
        eprintln!("--until {}: {e}", until.unwrap_or_default());
        return;
    }
    let goal = goal.as_ref();
    let partial = PathBuf::from(format!("{}-partial", out.display()));
    let none = Controls::default();
    let mut log: Vec<Controls> = Vec::new();
    let push = |t: &mut Mcr, log: &mut Vec<Controls>, c: Controls, n: usize| {
        for _ in 0..n {
            t.step(c);
            log.push(c);
        }
    };
    if let Some(file) = &from {
        // Go on from an earlier play: its presses first, as they were.
        let old = gp_arcade::oracle::InputLog::parse(&std::fs::read_to_string(file).unwrap()).unwrap();
        for n in 1..=old.frames.len() {
            push(&mut t, &mut log, old.controls(n), 1);
        }
    } else {
        // Power-on and 30 seconds of attract, a coin, then start 1.
        push(&mut t, &mut log, none, 900);
        push(&mut t, &mut log, Controls { coin: true, ..none }, 6);
        push(&mut t, &mut log, none, 54);
        push(
            &mut t,
            &mut log,
            Controls {
                start: true,
                ..none
            },
            6,
        );
    }
    // The log so far, written as it goes so a run that is stopped keeps
    // its work: after the start, every 600 frames and after each search.
    let mut notes = String::new();
    write_log(&partial, &log, &notes);
    let mut written = log.len();
    let mut goal_at = reached(&t, goal).then(|| t.frame());
    let mut mind = Mind::default();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut points: Vec<Point> = Vec::new();
    let mut lives = peek(&t, 0xe4a7);
    let mut fails_at: Option<(u64, u32)> = None;
    let mut plan: std::collections::VecDeque<Controls> = Default::default();
    let mut chose_at = 0u64;
    let mut cleared_at: Option<u64> = None;
    // The frame the current level's play began: backing up stops there.
    let mut level_start = t.frame();
    let mut seen_level = level(&t);
    let debug = std::env::var_os("AUTOPLAY_DEBUG").is_some();
    let mut recent: std::collections::VecDeque<String> = Default::default();
    while log.len() < frames && goal_at.is_none() {
        let f = t.frame();
        if f.is_multiple_of(20) && points.last().is_none_or(|p| p.t.frame() < f) {
            points.push(Point {
                t: t.clone(),
                log_len: log.len(),
                mind: mind.clone(),
            });
            if points.len() > 90 {
                points.remove(0);
            }
        }
        if !idle && plan.is_empty() && choosing(&t) && f > chose_at + 600 {
            plan.extend(bonus_choice(&t));
            chose_at = f;
            let _ = writeln!(notes, "# bonus round choice made at frame {f}");
        }
        let wild = fails_at.map_or(0, |(_, n)| u64::from(n));
        let c = if idle {
            none
        } else if let Some(c) = plan.pop_front() {
            c
        } else {
            decide(&t, &mut mind, &mut rng, wild, true)
        };
        if debug {
            let s = see(&t);
            recent.push_back(format!(
                "{} {:07b} b{:?} c{:?} d{:?} m{:x?}",
                t.frame() + 1,
                c.bits(),
                s.bart,
                s.customers,
                s.drinking,
                s.other
                    .iter()
                    .filter(|(_, o)| o.code == 0x17)
                    .map(|(a, o)| (a - 0xe000, o.y, o.x))
                    .collect::<Vec<_>>()
            ));
            if recent.len() > 150 {
                recent.pop_front();
            }
        }
        t.step(c);
        log.push(c);
        if reached(&t, goal) {
            goal_at = Some(t.frame());
            break;
        }
        if level(&t) != seen_level && mode(&t) == Mode::Play {
            seen_level = level(&t);
            level_start = t.frame();
        }
        let now = peek(&t, 0xe4a7);
        // E3BD goes to 2 at game over (0 in a game, 1 in attract).
        let over = peek(&t, 0xe3bd) == 2;
        if over && idle {
            let _ = writeln!(notes, "# game over at frame {}", t.frame());
            // Let the game-over screens and the initials entry time out.
            push(&mut t, &mut log, none, 1800);
            break;
        }
        if (now < lives || over) && !idle {
            // A life lost: back up and try again, further back each time.
            let n = match fails_at {
                Some((at, n)) if t.frame() <= at + 300 => n + 1,
                _ => 1,
            };
            if n <= tries && search_on {
                // Back up a few seconds (further each time) and search
                // from there to two seconds past the loss.
                let back = back_step * u64::from(n.min(4));
                let target = t.frame().saturating_sub(back).max(level_start);
                while points.len() > 1 && points.last().unwrap().t.frame() > target {
                    points.pop();
                }
                let p = points.last().unwrap().clone();
                let until = t.frame() + 60;
                let width = (width0 * n as usize).min(48);
                eprintln!(
                    "life lost at frame {}, search {n}: from frame {} to {until}, beam {width}",
                    t.frame(),
                    p.t.frame()
                );
                let (cs, whole) = search(&p.t, until, width, horizon, jobs, goal);
                t = p.t;
                log.truncate(p.log_len);
                plan.clear();
                recent.clear();
                if chose_at > t.frame() {
                    chose_at = 0;
                }
                fails_at = Some((fails_at.map_or(until, |(a, _)| a.max(until)), n));
                // The way found, or the furthest any way got: play on from
                // its end either way, so a hard stretch is searched a
                // piece at a time.
                for c in cs {
                    t.step(c);
                    log.push(c);
                    if reached(&t, goal) {
                        goal_at = Some(t.frame());
                        break;
                    }
                    // Points along the way found, so a loss soon after
                    // backs up a little, not to before this search.
                    if t.frame().is_multiple_of(20) {
                        points.push(Point {
                            t: t.clone(),
                            log_len: log.len(),
                            mind: Mind::default(),
                        });
                    }
                }
                if points.len() > 90 {
                    points.drain(..points.len() - 90);
                }
                mind = Mind::default();
                lives = peek(&t, 0xe4a7);
                if goal_at.is_some() {
                    eprintln!("  found a way to the goal at frame {}", t.frame());
                } else if whole {
                    eprintln!("  found a way to frame {}", t.frame());
                } else {
                    eprintln!("  no way past frame {}; playing on from there", t.frame());
                }
                write_log(&partial, &log, &notes);
                written = log.len();
                continue;
            }
            if n <= tries {
                if debug && n == 1 {
                    eprintln!("--- the frames before it:");
                    for r in &recent {
                        eprintln!("{r}");
                    }
                }
                let back = 40 * u64::from(n.min(20));
                let target = t.frame().saturating_sub(back).max(level_start);
                while points.len() > 1 && points.last().unwrap().t.frame() > target {
                    points.pop();
                }
                let p = points.last().unwrap().clone();
                eprintln!(
                    "life lost at frame {}, try {n}: back to frame {}",
                    t.frame(),
                    p.t.frame()
                );
                t = p.t;
                log.truncate(p.log_len);
                mind = p.mind;
                plan.clear();
                recent.clear();
                if chose_at > t.frame() {
                    chose_at = 0;
                }
                fails_at = Some((
                    fails_at.map_or(t.frame() + back, |(a, _)| a.max(t.frame())),
                    n,
                ));
                rng.next();
                continue;
            }
            if over {
                let _ = writeln!(
                    notes,
                    "# the last life lost at frame {}, after {tries} tries: game over",
                    t.frame()
                );
                push(&mut t, &mut log, none, 1800);
                break;
            }
            let _ = writeln!(
                notes,
                "# life lost at frame {}: kept after {tries} tries",
                t.frame()
            );
            fails_at = None;
        }
        if fails_at.is_some_and(|(at, _)| t.frame() > at + 600) {
            fails_at = None;
        }
        lives = now;
        let (cleared, _) = level(&t);
        if cleared >= levels && cleared_at.is_none() {
            cleared_at = Some(t.frame());
        }
        // Stop once the next level is under way after the last one asked
        // for, so its end sequence and whatever follows are in the log.
        if let Some(at) = cleared_at
            && mode(&t) == Mode::Play
            && t.frame() > at + 60
        {
            push(&mut t, &mut log, none, 30);
            break;
        }
        if log.len() >= written + 600 {
            write_log(&partial, &log, &notes);
            written = log.len();
        }
        if t.frame().is_multiple_of(600) {
            eprintln!(
                "frame {} lives {} cleared {:?} score {} mode {:?}",
                t.frame(),
                now,
                level(&t),
                score(&t),
                mode(&t)
            );
        }
    }
    if let Some(text) = &until {
        let _ = match goal_at {
            Some(f) => writeln!(notes, "# the goal {text} held at frame {f}"),
            None => writeln!(
                notes,
                "# the goal {text} never held: stopped at frame {}",
                t.frame()
            ),
        };
    }
    eprint!("{notes}");
    write_log(&out, &log, &notes);
    write_phases(&set, &out, &log);
}

/// Plays `log` and writes NAME.phases: each run of frames with the same
/// mode, levels cleared, bar and lives, with the frame numbers the MAME
/// oracle uses (frame 1 is the first stepped).
fn write_phases(set: &Romset, out: &std::path::Path, log: &[Controls]) {
    let mut t = Mcr::power_on(set);
    let mut s = String::from(
        "# frames FIRST-LAST (oracle frame numbers), mode, levels cleared, bar, spare lives, score at the end\n",
    );
    let mut run: Option<(u64, (Mode, u8, u8, u8))> = None;
    let mut last_score = 0;
    for &c in log {
        t.step(c);
        let (cleared, bar) = level(&t);
        let key = (mode(&t), cleared, bar, peek(&t, 0xe4a7));
        match run {
            Some((_, k)) if k == key => {}
            Some((from, k)) => {
                let _ = writeln!(
                    s,
                    "{from}-{} {:?} cleared {} bar {} lives {} score {last_score}",
                    t.frame() - 1,
                    k.0,
                    k.1,
                    k.2,
                    k.3 as i8
                );
                run = Some((t.frame(), key));
            }
            None => run = Some((t.frame(), key)),
        }
        last_score = score(&t);
    }
    if let Some((from, k)) = run {
        let _ = writeln!(
            s,
            "{from}-{} {:?} cleared {} bar {} lives {} score {last_score}",
            t.frame(),
            k.0,
            k.1,
            k.2,
            k.3 as i8
        );
    }
    std::fs::write(out.with_extension("phases"), s).unwrap();
}

fn print_trace(set: &Romset, file: &std::path::Path, from: u64, to: u64) {
    let log = gp_arcade::oracle::InputLog::parse(&std::fs::read_to_string(file).unwrap()).unwrap();
    let mut t = Mcr::power_on(set);
    for n in 1..=to as usize {
        let c = log.controls(n);
        if n as u64 >= from {
            let s = see(&t);
            println!(
                "{} {:07b} L{} S{} {:?} {:?} b{:?} c{:?} d{:?} m{:x?} o{:x?}",
                n,
                c.bits(),
                peek(&t, 0xe4a7),
                score(&t),
                level(&t),
                mode(&t),
                s.bart,
                s.customers,
                s.drinking,
                s.mugs,
                s.other
                    .iter()
                    .map(|(a, o)| (a - 0xe000, o.y, o.code, o.x))
                    .collect::<Vec<_>>()
            );
        }
        t.step(c);
    }
}

fn write_log(out: &std::path::Path, log: &[Controls], notes: &str) {
    let mut s = String::from(
        "gp-arcade-input 0\n# Made by Tapper's auto-player (gp mechanic autoplay): a coin and start 1 after\n# 30 seconds of attract, then the player's own presses.\nsystem tapper\ncolumns CSRLDUT\n",
    );
    for l in notes.lines() {
        let _ = writeln!(s, "{l}");
    }
    let mut i = 0;
    while i < log.len() {
        let mut j = i;
        while j < log.len() && log[j] == log[i] {
            j += 1;
        }
        let bits = log[i].bits();
        let row: String = "CSRLDUT"
            .chars()
            .enumerate()
            .map(|(k, ch)| if bits & (1 << k) != 0 { ch } else { '.' })
            .collect();
        let _ = writeln!(s, "{} {row}", j - i);
        i = j;
    }
    std::fs::write(out.with_extension("input"), s).unwrap();
}

/// The game's own sets: gp passes the games checkout ($GP_GAMES_DIR), and
/// without one the driver reads the roms.toml of the game it belongs to.
fn main() {
    if gp_arcade::romset::known().is_empty() {
        let game = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        if let Err(e) = gp_arcade::romset::add_games_dir(&game) {
            eprintln!("autoplay: {e}");
        }
    }
    main_with(std::env::args().skip(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A goal on the made-up busy program (no game's bytes): the search
    /// stops on the frame it first holds and says it got there, a position
    /// that reaches it outvalues any other, and a goal that names nothing
    /// on the board isn't held.
    #[test]
    fn a_search_plays_to_its_goal() {
        let set = gp_arcade::testrom::busy();
        let t = Mcr::power_on(&set);
        let goal = Condition::parse("frame >= 7").unwrap();
        let (log, whole) = search(&t, 1000, 2, 5, 1, Some(&goal));
        assert!(whole);
        let mut u = t.clone();
        for c in &log {
            u.step(*c);
        }
        assert_eq!(u.frame(), 7);
        assert!(reached(&u, Some(&goal)));
        assert!(!reached(&t, Some(&goal)));
        assert!(!reached(&u, None));
        assert!(value(&t, 10, Some(&goal)) > GOAL_VALUE - 100);
        assert!(value(&t, 3, Some(&goal)) < GOAL_VALUE / 2);
        let wrong = Condition::parse("nothing[0] == 1").unwrap();
        assert!(!reached(&u, Some(&wrong)));
    }

    /// `--help` (as `gp mechanic autoplay --help` passes it) prints the flags
    /// and returns, before any ROM is read; it used to panic as an unknown flag.
    #[test]
    fn help_returns_without_a_rom() {
        main_with(["--help".to_string()]);
        main_with(["-h".to_string()]);
        for flag in [
            "--rom",
            "--out",
            "--frames",
            "--levels",
            "--tries",
            "--idle",
            "--search",
            "--jobs",
            "--horizon",
            "--width",
            "--back",
            "--from",
            "--until",
            "--trace",
        ] {
            assert!(USAGE.contains(flag), "the help leaves out {flag}");
        }
    }
}
