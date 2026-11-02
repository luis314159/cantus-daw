//! The frame pipeline: frames made on worker threads, handed over in order, with bounded
//! memory (ADR 0088 §6).
//!
//! Worker `j` of `k` makes frames `j, j + k, j + 2k, …` and sends each through its own
//! channel of two places; the consumer takes frame `n` from channel `n mod k`. So frames
//! arrive in order, a slow frame only holds back its own channel, and at most `3k` frames
//! exist at once (two waiting a worker, one being made). The consumer (the encoder and the
//! writer) runs on the caller's thread — Media Foundation's objects stay on the thread
//! that made them.
//!
//! Stopping: the consumer returning an error (or [`crate::Control::cancel`]) drops the
//! receivers; each worker's next send fails and it ends. A worker that panics ends its
//! channel, which the consumer reports as an error rather than waiting for ever.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;

/// Frames waiting in each worker's channel.
pub const DEPTH: usize = 2;

/// How many workers to use: the processor's threads less one for the encoder, 1…8.
pub fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .saturating_sub(1)
        .clamp(1, 8)
}

/// What went wrong in the pipeline itself (not in the consumer).
#[derive(Debug, PartialEq, Eq)]
pub enum PipelineError<E> {
    /// The consumer's own error, passed through.
    Consumer(E),
    /// A worker ended before making its frame (it panicked).
    WorkerLost(u32),
}

/// Make frames `0..total` with `make` on `workers` threads (each with its own state, made
/// by `state`), and give each to `consume` in order. `stop` is checked before each frame.
pub fn run<S, T, E>(
    total: u32,
    workers: usize,
    stop: &AtomicBool,
    state: impl Fn() -> S + Sync,
    make: impl Fn(&mut S, u32) -> T + Sync,
    mut consume: impl FnMut(u32, T) -> Result<(), E>,
) -> Result<(), PipelineError<E>>
where
    T: Send,
{
    let k = workers.clamp(1, total.max(1) as usize);
    std::thread::scope(|s| {
        let mut rx = Vec::with_capacity(k);
        for j in 0..k {
            let (tx, r) = sync_channel::<T>(DEPTH);
            rx.push(r);
            let (state, make) = (&state, &make);
            std::thread::Builder::new()
                .name(format!("cantus-video-{j}"))
                .spawn_scoped(s, move || {
                    let mut st = state();
                    let mut n = j as u32;
                    while n < total && !stop.load(Ordering::Relaxed) {
                        if tx.send(make(&mut st, n)).is_err() {
                            return;
                        }
                        n += k as u32;
                    }
                })
                .expect("a worker thread");
        }
        let result = (|| {
            for n in 0..total {
                let item = rx[n as usize % k]
                    .recv()
                    .map_err(|_| PipelineError::WorkerLost(n))?;
                consume(n, item).map_err(PipelineError::Consumer)?;
            }
            Ok(())
        })();
        // Dropping the receivers ends every worker at its next send.
        drop(rx);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn frames_arrive_in_order_whatever_each_takes() {
        let stop = AtomicBool::new(false);
        let mut seen = Vec::new();
        run(
            200,
            5,
            &stop,
            || (),
            |_, n| {
                // Uneven work: some frames take far longer than others.
                if n % 7 == 3 {
                    std::thread::sleep(std::time::Duration::from_millis(3));
                }
                n * 2
            },
            |n, v| {
                assert_eq!(v, n * 2);
                seen.push(n);
                Ok::<(), ()>(())
            },
        )
        .unwrap();
        assert_eq!(seen, (0..200).collect::<Vec<_>>());
    }

    #[test]
    fn memory_is_bounded() {
        struct Alive<'a>(&'a AtomicUsize);
        impl Drop for Alive<'_> {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let alive = AtomicUsize::new(0);
        let most = AtomicUsize::new(0);
        let stop = AtomicBool::new(false);
        let k = 4;
        run(
            300,
            k,
            &stop,
            || (),
            |_, _| {
                let now = alive.fetch_add(1, Ordering::SeqCst) + 1;
                most.fetch_max(now, Ordering::SeqCst);
                Alive(&alive)
            },
            |_, item| {
                // A slow consumer: the workers fill their channels and wait.
                std::thread::sleep(std::time::Duration::from_micros(300));
                drop(item);
                Ok::<(), ()>(())
            },
        )
        .unwrap();
        let most = most.load(Ordering::SeqCst);
        assert!(most <= k * (DEPTH + 1) + 1, "{most} frames at once");
        assert_eq!(alive.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_consumer_error_stops_everything() {
        let stop = AtomicBool::new(false);
        let made = AtomicUsize::new(0);
        let r = run(
            10_000,
            3,
            &stop,
            || (),
            |_, n| {
                made.fetch_add(1, Ordering::SeqCst);
                n
            },
            |n, _| if n == 20 { Err("full disk") } else { Ok(()) },
        );
        assert_eq!(r, Err(PipelineError::Consumer("full disk")));
        assert!(made.load(Ordering::SeqCst) < 100);
    }

    #[test]
    fn a_stop_ends_the_workers_and_a_lost_worker_is_said() {
        let stop = AtomicBool::new(true);
        let r = run(50, 2, &stop, || (), |_, n| n, |_, _| Ok::<(), ()>(()));
        assert_eq!(r, Err(PipelineError::WorkerLost(0)));
    }

    #[test]
    fn each_worker_keeps_its_own_state() {
        let stop = AtomicBool::new(false);
        let mut total = 0u64;
        run(
            64,
            4,
            &stop,
            || vec![0u8; 16],
            |buf: &mut Vec<u8>, n| {
                buf[0] = buf[0].wrapping_add(1);
                n as u64
            },
            |_, v| {
                total += v;
                Ok::<(), ()>(())
            },
        )
        .unwrap();
        assert_eq!(total, (0..64).sum::<u64>());
    }
}
