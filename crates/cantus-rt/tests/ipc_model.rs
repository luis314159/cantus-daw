//! The lock-step exchange of `cantus_rt::ipc` model-checked (ADR 0063): two callers (two
//! plug-ins' audio halves in Cantus) and two workers (the helper's threads, sharing one
//! lobby) written as their sequence of atomic steps — exactly the order of
//! `Caller::{submit, wait}`, `ipc::serve` (`Exchange::take`, `Lobby::{arrive, leave}`,
//! `LobbyBell::ring_if_alone`) and `Completer::complete` — and **every interleaving**
//! explored, under the sequentially consistent order those operations use. Bells are
//! modelled both ways they are built: a Windows auto-reset event checked after the word,
//! and a Linux futex that compares and sleeps atomically. Spinning gives up at any point
//! (a nondeterministic choice), so every sleep path is reached.
//!
//! What must hold: no reachable state where nothing can move but a caller still waits (a
//! lost wake-up), a job is taken by one worker only, and a caller reads a result only after
//! a worker published it. Pure code, no OS calls: it runs under Miri too.

use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Sem {
    Event,
    Futex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum C {
    Submit,
    MaybeRing,
    Spin,
    Epoch,
    Announce,
    Recheck,
    CheckWord,
    Sleep,
    Unannounce,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum W {
    Start,
    Scan,
    Leave,
    RingIfAlone,
    Publish,
    RingCaller,
    Arrive,
    PEpoch,
    PLeave,
    PScan,
    PCheckWord,
    PSleep,
    PArrive,
}

const N: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Ex {
    job: u8,
    taken: u8,
    done: u8,
    waiting: bool,
    word: u8,
    event: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Cl {
    pc: C,
    n: u8,
    epoch: u8,
    left: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Wk {
    pc: W,
    /// The exchange and job it holds.
    holding: Option<(usize, u8)>,
    epoch: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct S {
    ex: [Ex; N],
    spinning: u8,
    lword: u8,
    levent: bool,
    c: [Cl; N],
    w: [Wk; N],
}

fn pending(s: &S) -> Vec<usize> {
    (0..N).filter(|&i| s.ex[i].job != s.ex[i].taken).collect()
}

fn ring_lobby(t: &mut S) {
    t.lword = t.lword.wrapping_add(1);
    t.levent = true;
}

/// Every state one step of caller `i` leads to (empty: blocked or finished).
fn caller(s: S, i: usize, sem: Sem) -> Vec<S> {
    let mut t = s;
    let c = s.c[i];
    let x = s.ex[i];
    match c.pc {
        C::Submit => {
            t.c[i].n = x.job.wrapping_add(1);
            t.ex[i].job = t.c[i].n;
            t.c[i].pc = C::MaybeRing;
        }
        C::MaybeRing => {
            if s.spinning == 0 {
                ring_lobby(&mut t);
            }
            t.c[i].pc = C::Spin;
        }
        C::Spin => {
            if x.done == c.n {
                t.c[i].pc = C::Finished;
                return vec![t];
            }
            let mut give_up = s;
            give_up.c[i].pc = C::Epoch;
            return vec![s, give_up];
        }
        C::Epoch => {
            t.c[i].epoch = x.word;
            t.c[i].pc = C::Announce;
        }
        C::Announce => {
            t.ex[i].waiting = true;
            t.c[i].pc = C::Recheck;
        }
        C::Recheck => {
            if x.done == c.n {
                t.ex[i].waiting = false;
                t.c[i].pc = C::Finished;
            } else {
                t.c[i].pc = C::CheckWord;
            }
        }
        C::CheckWord => {
            t.c[i].pc = if x.word != c.epoch {
                C::Unannounce
            } else {
                C::Sleep
            };
        }
        C::Sleep => match sem {
            Sem::Event if x.event => {
                t.ex[i].event = false;
                t.c[i].pc = C::Unannounce;
            }
            Sem::Futex if x.word != c.epoch => t.c[i].pc = C::Unannounce,
            _ => return Vec::new(),
        },
        C::Unannounce => {
            t.ex[i].waiting = false;
            t.c[i].pc = C::Epoch;
        }
        C::Finished => {
            assert_eq!(x.done, c.n, "a result read before it was published: {s:?}");
            if c.left == 0 {
                return Vec::new();
            }
            t.c[i].left -= 1;
            t.c[i].pc = C::Submit;
        }
    }
    vec![t]
}

/// The states where worker `w` took pending exchange `i` (one compare-and-swap).
fn take(s: S, w: usize, i: usize, next: W) -> S {
    let mut t = s;
    let job = s.ex[i].job;
    assert_ne!(job, s.ex[i].taken);
    t.ex[i].taken = job;
    t.w[w].holding = Some((i, job));
    t.w[w].pc = next;
    t
}

/// Every state one step of worker `w` leads to.
fn worker(s: S, w: usize, sem: Sem) -> Vec<S> {
    let mut t = s;
    let k = s.w[w];
    match k.pc {
        W::Start | W::Arrive | W::PArrive => {
            t.spinning += 1;
            t.w[w].pc = W::Scan;
        }
        W::Scan => {
            let p = pending(&s);
            if !p.is_empty() {
                return p.into_iter().map(|i| take(s, w, i, W::Leave)).collect();
            }
            let mut give_up = s;
            give_up.w[w].pc = W::PEpoch;
            return vec![s, give_up];
        }
        W::Leave | W::PLeave => {
            t.spinning -= 1;
            t.w[w].pc = if k.pc == W::Leave {
                W::RingIfAlone
            } else {
                W::PScan
            };
        }
        W::RingIfAlone => {
            if !pending(&s).is_empty() && s.spinning == 0 {
                ring_lobby(&mut t);
            }
            t.w[w].pc = W::Publish;
        }
        W::Publish => {
            let (i, n) = k.holding.expect("holds a job");
            // One worker per job: nobody else completed this one.
            assert!(s.ex[i].done != n, "a job finished twice: {s:?}");
            t.ex[i].done = n;
            t.w[w].pc = W::RingCaller;
        }
        W::RingCaller => {
            let (i, _) = k.holding.expect("holds a job");
            if s.ex[i].waiting {
                t.ex[i].word = t.ex[i].word.wrapping_add(1);
                t.ex[i].event = true;
            }
            t.w[w].holding = None;
            t.w[w].pc = W::Arrive;
        }
        W::PEpoch => {
            t.w[w].epoch = s.lword;
            t.w[w].pc = W::PLeave;
        }
        W::PScan => {
            let p = pending(&s);
            if !p.is_empty() {
                return p
                    .into_iter()
                    .map(|i| take(s, w, i, W::RingIfAlone))
                    .collect();
            }
            t.w[w].pc = W::PCheckWord;
        }
        W::PCheckWord => {
            t.w[w].pc = if s.lword != k.epoch {
                W::PArrive
            } else {
                W::PSleep
            };
        }
        W::PSleep => match sem {
            Sem::Event if s.levent => {
                t.levent = false;
                t.w[w].pc = W::PArrive;
            }
            Sem::Futex if s.lword != k.epoch => t.w[w].pc = W::PArrive,
            _ => return Vec::new(),
        },
    }
    vec![t]
}

/// Explores every interleaving of `jobs[i]` jobs by caller `i`; returns the number of
/// states.
fn explore(jobs: [u8; N], sem: Sem) -> usize {
    let cl = |j: u8| Cl {
        pc: C::Submit,
        n: 0,
        epoch: 0,
        left: j - 1,
    };
    let wk = Wk {
        pc: W::Start,
        holding: None,
        epoch: 0,
    };
    let ex = Ex {
        job: 0,
        taken: 0,
        done: 0,
        waiting: false,
        word: 0,
        event: false,
    };
    let start = S {
        ex: [ex; N],
        spinning: 0,
        lword: 0,
        levent: false,
        c: [cl(jobs[0]), cl(jobs[1])],
        w: [wk; N],
    };
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(s) = stack.pop() {
        if !seen.insert(s) {
            continue;
        }
        let mut next = Vec::new();
        for i in 0..N {
            next.extend(caller(s, i, sem));
        }
        for w in 0..N {
            next.extend(worker(s, w, sem));
        }
        let all_done = s.c.iter().all(|c| c.pc == C::Finished && c.left == 0);
        let moves = next.iter().any(|x| *x != s);
        assert!(
            all_done || moves,
            "lost wake-up: nothing can move and a caller still waits: {s:?}"
        );
        stack.extend(next);
    }
    seen.len()
}

#[test]
fn no_interleaving_of_two_callers_and_two_workers_loses_a_wake_up_with_an_event() {
    // The first caller's second job meets the workers after they served both (the spin
    // and park paths again); the second caller's one job races the first's.
    let states = explore([2, 1], Sem::Event);
    assert!(states > 10_000, "{states} states: too small a model to mean much");
}

#[test]
fn no_interleaving_of_two_callers_and_two_workers_loses_a_wake_up_with_a_futex() {
    let states = explore([2, 1], Sem::Futex);
    assert!(states > 10_000, "{states} states");
}

/// The model finds the bug it exists for: a worker that stops counting as a spinner only
/// *after* its last look parks while a job waits that nobody rang for.
#[test]
fn the_model_catches_a_worker_that_looks_before_it_stops_spinning() {
    // Swap PLeave and PScan's order by running the model with a broken worker.
    fn broken(s: S, w: usize) -> Vec<S> {
        let k = s.w[w];
        let mut t = s;
        match k.pc {
            W::PEpoch => {
                t.w[w].epoch = s.lword;
                t.w[w].pc = W::PScan;
                vec![t]
            }
            W::PScan if k.holding.is_none() && pending(&s).is_empty() => {
                t.w[w].pc = W::PLeave;
                vec![t]
            }
            W::PLeave => {
                t.spinning -= 1;
                t.w[w].pc = W::PCheckWord;
                vec![t]
            }
            _ => worker(s, w, Sem::Futex),
        }
    }
    let result = std::panic::catch_unwind(|| {
        let cl = Cl {
            pc: C::Submit,
            n: 0,
            epoch: 0,
            left: 0,
        };
        let wk = Wk {
            pc: W::Start,
            holding: None,
            epoch: 0,
        };
        let ex = Ex {
            job: 0,
            taken: 0,
            done: 0,
            waiting: false,
            word: 0,
            event: false,
        };
        let start = S {
            ex: [ex; N],
            spinning: 0,
            lword: 0,
            levent: false,
            c: [cl; N],
            w: [wk; N],
        };
        let mut seen = HashSet::new();
        let mut stack = vec![start];
        while let Some(s) = stack.pop() {
            if !seen.insert(s) {
                continue;
            }
            let mut next = Vec::new();
            for i in 0..N {
                next.extend(caller(s, i, Sem::Futex));
            }
            for w in 0..N {
                next.extend(broken(s, w));
            }
            let all_done = s.c.iter().all(|c| c.pc == C::Finished && c.left == 0);
            assert!(all_done || next.iter().any(|x| *x != s), "lost: {s:?}");
            stack.extend(next);
        }
    });
    assert!(result.is_err(), "the model should find the lost wake-up");
}

/// Two jobs each, both bells: about seven minutes in release
/// (`cargo test -p cantus-rt --release --test ipc_model -- --ignored`).
#[test]
#[ignore]
fn the_full_model_two_jobs_each() {
    for sem in [Sem::Event, Sem::Futex] {
        assert!(explore([2, 2], sem) > 100_000);
    }
}
