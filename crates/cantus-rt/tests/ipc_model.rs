//! The lock-step exchange of `cantus_rt::ipc` model-checked (ADR 0063): both sides written
//! as their sequence of atomic steps — exactly the order of `Caller::{submit, wait}` and
//! `Worker::{next, complete}` — and **every interleaving** explored, under the sequentially
//! consistent order those operations use. The bell is modelled both ways it is built: a
//! Windows auto-reset event checked after the word, and a Linux futex that compares and
//! sleeps atomically. Spinning gives up at any point (a nondeterministic choice), so the
//! sleep paths are all reached.
//!
//! What must hold: no reachable state where nothing can move while the caller still waits
//! for a job (a lost wake-up), and the caller only ever reads a result after the worker
//! published it. Pure code, no OS calls: it runs under Miri too.

use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Sem {
    /// Check the word, then wait on an auto-reset event (stale signals allowed).
    Event,
    /// Compare-and-sleep atomically on the word.
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
    Spin,
    Epoch,
    Announce,
    Recheck,
    CheckWord,
    Sleep,
    Unpark,
    Look,
    Work(u32),
    Publish(u32),
    MaybeRing(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct S {
    // Shared words.
    job: u32,
    done: u32,
    caller_waiting: bool,
    worker_parked: bool,
    cword: u32,
    wword: u32,
    cevent: bool,
    wevent: bool,
    // The caller: its step, the job it waits for, its epoch, jobs left.
    c: C,
    n: u32,
    ce: u32,
    left: u32,
    // The worker: its step, the last job it finished, its epoch.
    w: W,
    last: u32,
    we: u32,
}

/// Every state one step of the caller can lead to.
fn caller(s: S, sem: Sem) -> Vec<S> {
    let mut t = s;
    match s.c {
        C::Submit => {
            t.n = s.job + 1;
            t.job = t.n;
            t.c = C::MaybeRing;
        }
        C::MaybeRing => {
            if s.worker_parked {
                t.wword += 1;
                t.wevent = true;
            }
            t.c = C::Spin;
        }
        C::Spin => {
            // A spin look: done, or keep spinning — or give up and go to sleep.
            if s.done == s.n {
                t.c = C::Finished;
                return vec![t];
            }
            let mut give_up = s;
            give_up.c = C::Epoch;
            return vec![s, give_up];
        }
        C::Epoch => {
            t.ce = s.cword;
            t.c = C::Announce;
        }
        C::Announce => {
            t.caller_waiting = true;
            t.c = C::Recheck;
        }
        C::Recheck => {
            if s.done == s.n {
                t.caller_waiting = false;
                t.c = C::Finished;
            } else {
                t.c = C::CheckWord;
            }
        }
        C::CheckWord => {
            t.c = if s.cword != s.ce {
                C::Unannounce
            } else {
                C::Sleep
            };
        }
        C::Sleep => match sem {
            Sem::Event if s.cevent => {
                t.cevent = false;
                t.c = C::Unannounce;
            }
            Sem::Futex if s.cword != s.ce => t.c = C::Unannounce,
            _ => return Vec::new(), // blocked
        },
        C::Unannounce => {
            t.caller_waiting = false;
            t.c = C::Epoch;
        }
        C::Finished => {
            // The caller reads the result: the worker must have published it.
            assert!(s.done == s.n, "a result read before it was published: {s:?}");
            if s.left == 0 {
                return Vec::new(); // done with every job
            }
            t.left -= 1;
            t.c = C::Submit;
        }
    }
    vec![t]
}

/// Every state one step of the worker can lead to.
fn worker(s: S, sem: Sem) -> Vec<S> {
    let mut t = s;
    match s.w {
        W::Spin => {
            if s.job != s.last {
                t.w = W::Work(s.job);
                return vec![t];
            }
            let mut give_up = s;
            give_up.w = W::Epoch;
            return vec![s, give_up];
        }
        W::Epoch => {
            t.we = s.wword;
            t.w = W::Announce;
        }
        W::Announce => {
            t.worker_parked = true;
            t.w = W::Recheck;
        }
        W::Recheck => {
            if s.job != s.last {
                t.worker_parked = false;
                t.w = W::Work(s.job);
            } else {
                t.w = W::CheckWord;
            }
        }
        W::CheckWord => {
            t.w = if s.wword != s.we { W::Unpark } else { W::Sleep };
        }
        W::Sleep => match sem {
            Sem::Event if s.wevent => {
                t.wevent = false;
                t.w = W::Unpark;
            }
            Sem::Futex if s.wword != s.we => t.w = W::Unpark,
            _ => return Vec::new(),
        },
        W::Unpark => {
            t.worker_parked = false;
            t.w = W::Look;
        }
        W::Look => {
            t.w = if s.job != s.last {
                W::Work(s.job)
            } else {
                W::Spin
            };
        }
        W::Work(n) => {
            assert_eq!(n, s.job, "the worker ran a job the caller did not publish");
            t.w = W::Publish(n);
        }
        W::Publish(n) => {
            t.done = n;
            t.w = W::MaybeRing(n);
        }
        W::MaybeRing(n) => {
            if s.caller_waiting {
                t.cword += 1;
                t.cevent = true;
            }
            t.last = n;
            t.w = W::Spin;
        }
    }
    vec![t]
}

/// Explores every interleaving of `jobs` jobs; returns the number of states seen.
fn explore(jobs: u32, sem: Sem) -> usize {
    let start = S {
        job: 0,
        done: 0,
        caller_waiting: false,
        worker_parked: false,
        cword: 0,
        wword: 0,
        cevent: false,
        wevent: false,
        c: C::Submit,
        n: 0,
        ce: 0,
        left: jobs - 1,
        w: W::Spin,
        last: 0,
        we: 0,
    };
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(s) = stack.pop() {
        if !seen.insert(s) {
            continue;
        }
        let a = caller(s, sem);
        let b = worker(s, sem);
        let finished = s.c == C::Finished && s.left == 0;
        // The worker may sleep for ever once the caller has nothing more to give; but the
        // caller may never be stuck while it waits for a job.
        if a.is_empty() && !finished {
            assert!(
                !b.is_empty() && b.iter().any(|x| *x != s),
                "lost wake-up: the caller sleeps and nothing can wake it: {s:?}"
            );
        }
        stack.extend(a);
        stack.extend(b);
    }
    seen.len()
}

#[test]
fn no_interleaving_loses_a_wake_up_with_an_event() {
    let states = explore(3, Sem::Event);
    assert!(states > 1_000, "{states} states: the model is too small to mean much");
}

#[test]
fn no_interleaving_loses_a_wake_up_with_a_futex() {
    let states = explore(3, Sem::Futex);
    assert!(states > 1_000, "{states} states");
}

/// The model finds the bug it exists for: announce *after* the last look, and a wake-up is
/// lost (the check that keeps the model honest).
#[test]
fn the_model_catches_an_announcement_made_too_late() {
    fn broken(s: S) -> Vec<S> {
        let mut t = s;
        match s.c {
            // Re-check first, announce second: the classic lost wake-up.
            C::Announce => {
                if s.done == s.n {
                    t.c = C::Finished;
                } else {
                    t.c = C::Recheck;
                }
                vec![t]
            }
            C::Recheck => {
                t.caller_waiting = true;
                t.c = C::CheckWord;
                vec![t]
            }
            _ => caller(s, Sem::Futex),
        }
    }
    let result = std::panic::catch_unwind(|| {
        let start = S {
            job: 0,
            done: 0,
            caller_waiting: false,
            worker_parked: false,
            cword: 0,
            wword: 0,
            cevent: false,
            wevent: false,
            c: C::Submit,
            n: 0,
            ce: 0,
            left: 1,
            w: W::Spin,
            last: 0,
            we: 0,
        };
        let mut seen = HashSet::new();
        let mut stack = vec![start];
        while let Some(s) = stack.pop() {
            if !seen.insert(s) {
                continue;
            }
            // The broken caller also reads the epoch *after* announcing, as the bug would.
            let a = broken(s);
            let b = worker(s, Sem::Futex);
            if a.is_empty() && !(s.c == C::Finished && s.left == 0) {
                assert!(!b.is_empty() && b.iter().any(|x| *x != s), "lost: {s:?}");
            }
            stack.extend(a);
            stack.extend(b);
        }
    });
    // With the epoch read before the (late) announcement the futex still saves it, so the
    // model must find the case where the word did not move: it does, or this fails.
    assert!(
        result.is_err(),
        "the model should find the lost wake-up of a late announcement"
    );
}
