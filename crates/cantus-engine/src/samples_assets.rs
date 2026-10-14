//! Instruments' files loaded off the interface's thread (ADR 0050 §7): a few loader threads
//! decoding in parallel, a queue the next snapshot can take files off again (an undo), and
//! the progress — files done of the files asked for in the current batch.

use crate::inst::stream::{self, Loaded};
use cantus_core::SampleData;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

/// A pause before each background load, ms: a slow disk or decoder, for tests (0: none).
static LOAD_DELAY_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Make every background load take `d` longer (tests of the interface while loading).
pub fn set_load_delay(d: std::time::Duration) {
    LOAD_DELAY_MS.store(d.as_millis() as u64, Ordering::Relaxed);
}

/// Background loads held back until released (tests that prove a caller does not wait for
/// a decode: logically, not by the clock).
static HELD: Mutex<bool> = Mutex::new(false);
static RELEASED: Condvar = Condvar::new();

/// Hold every background load before it decodes (`true`) or let them go (`false`).
pub fn hold_loads(held: bool) {
    if let Ok(mut h) = HELD.lock() {
        *h = held;
    }
    RELEASED.notify_all();
}

fn wait_while_held() {
    let Ok(mut h) = HELD.lock() else {
        return;
    };
    while *h {
        h = match RELEASED.wait(h) {
            Ok(h) => h,
            Err(_) => return,
        };
    }
}

/// A file an instrument asks for, and whether Exemplar may stream it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Want {
    pub path: PathBuf,
    pub stream: bool,
}

/// A load's result: the audio (a stream's head, or the whole file) and, when it played
/// from memory for a reason worth saying (the cache could not be written), why.
pub type Done = (
    Want,
    Result<(Arc<SampleData>, Option<stream::LoadNote>), String>,
);

/// Decode a file in full: Exemplar's decoders by extension, else WAV.
pub fn decode_any(path: &Path) -> Result<SampleData, String> {
    crate::inst::sampler::decode::decode(path).unwrap_or_else(|| super::decode_wav(path))
}

/// Load one file as it is wanted: whole, or (streamable and long enough) streamed.
pub fn load_one(w: &Want) -> Result<(Arc<SampleData>, Option<stream::LoadNote>), String> {
    if !w.stream {
        return decode_any(&w.path).map(|d| (Arc::new(d), None));
    }
    stream::load(&w.path, &decode_any).map(|l| match l {
        Loaded::Whole { data, note } => (Arc::new(data), note),
        Loaded::Streamed(head) => (head, None),
    })
}

/// Threads for decoding at once: the cores less two (the interface, the audio), 1 to 8.
pub fn threads() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .saturating_sub(2)
        .clamp(1, 8)
}

/// Load `wants` here, now, on scoped threads in parallel (tests, renders).
pub fn load_all(wants: &[Want]) -> Vec<Done> {
    if wants.len() <= 1 {
        return wants.iter().map(|w| (w.clone(), load_one(w))).collect();
    }
    let queue = Mutex::new(wants.iter().cloned().collect::<VecDeque<_>>());
    let out = Mutex::new(Vec::with_capacity(wants.len()));
    std::thread::scope(|s| {
        for _ in 0..threads().min(wants.len()) {
            s.spawn(|| {
                while let Some(w) = queue.lock().ok().and_then(|mut q| q.pop_front()) {
                    let r = load_one(&w);
                    if let Ok(mut o) = out.lock() {
                        o.push((w, r));
                    }
                }
            });
        }
    });
    out.into_inner().unwrap_or_default()
}

#[derive(Debug, Default)]
struct Shared {
    queue: Mutex<VecDeque<Want>>,
    ready: Condvar,
    quit: AtomicBool,
}

/// The background loader for instruments' files.
#[derive(Debug)]
pub struct AssetLoader {
    shared: Arc<Shared>,
    done: Receiver<Done>,
    /// Queued or being decoded.
    pending: HashSet<Want>,
    /// Being decoded but no longer asked for: its result is kept, not counted.
    unwanted: HashSet<Want>,
    total: usize,
    finished: usize,
}

impl AssetLoader {
    pub fn spawn() -> Option<AssetLoader> {
        let shared = Arc::new(Shared::default());
        let (tx, done) = channel::<Done>();
        for i in 0..threads() {
            let (shared, tx): (Arc<Shared>, Sender<Done>) = (shared.clone(), tx.clone());
            std::thread::Builder::new()
                .name(format!("cantus-sample-loader-{i}"))
                .spawn(move || worker(&shared, &tx))
                .ok()?;
        }
        Some(AssetLoader {
            shared,
            done,
            pending: HashSet::new(),
            unwanted: HashSet::new(),
            total: 0,
            finished: 0,
        })
    }

    /// The files the snapshot asks for now: those `missing` not on their way are queued;
    /// queued files it no longer asks for leave the queue (an undo during a load).
    pub fn want(&mut self, wanted: &HashSet<Want>, missing: &[Want]) {
        if let Ok(mut q) = self.shared.queue.lock() {
            let before = q.len();
            q.retain(|w| {
                let keep = wanted.contains(w);
                if !keep {
                    self.pending.remove(w);
                }
                keep
            });
            self.total = self.total.saturating_sub(before - q.len());
            // Being decoded and no longer wanted: not counted any more.
            for w in self.pending.iter() {
                if !wanted.contains(w) && !q.contains(w) && self.unwanted.insert(w.clone()) {
                    self.total = self.total.saturating_sub(1);
                }
            }
            for w in missing {
                if self.unwanted.remove(w) {
                    self.total += 1; // asked for again while it decodes
                }
                if self.pending.insert(w.clone()) {
                    q.push_back(w.clone());
                    self.total += 1;
                }
            }
        }
        self.shared.ready.notify_all();
        self.settle();
    }

    /// What finished since the last call.
    pub fn take(&mut self) -> Vec<Done> {
        let mut out = Vec::new();
        while let Ok(d) = self.done.try_recv() {
            self.landed(&d.0);
            out.push(d);
        }
        self.settle();
        out
    }

    /// Wait for everything on its way (an offline render; ADR 0050 §7).
    pub fn wait(&mut self) -> Vec<Done> {
        let mut out = Vec::new();
        while !self.pending.is_empty() {
            match self.done.recv() {
                Ok(d) => {
                    self.landed(&d.0);
                    out.push(d);
                }
                Err(_) => break,
            }
        }
        self.settle();
        out
    }

    fn landed(&mut self, w: &Want) {
        self.pending.remove(w);
        if !self.unwanted.remove(w) {
            self.finished += 1;
        }
    }

    /// A batch ends when nothing is on its way.
    fn settle(&mut self) {
        if self.pending.is_empty() {
            self.total = 0;
            self.finished = 0;
            self.unwanted.clear();
        }
    }

    /// Files done of the files asked for, while a batch loads.
    pub fn progress(&self) -> Option<(usize, usize)> {
        (self.total > self.finished).then_some((self.finished, self.total))
    }

    pub fn busy(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn is_pending(&self, path: &Path) -> bool {
        self.pending
            .iter()
            .any(|w| w.path == path && !self.unwanted.contains(w))
    }
}

impl Drop for AssetLoader {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        self.shared.ready.notify_all();
    }
}

fn worker(shared: &Shared, tx: &Sender<Done>) {
    loop {
        let w = {
            let Ok(mut q) = shared.queue.lock() else {
                return;
            };
            loop {
                if shared.quit.load(Ordering::Acquire) {
                    return;
                }
                if let Some(w) = q.pop_front() {
                    break w;
                }
                q = match shared.ready.wait(q) {
                    Ok(q) => q,
                    Err(_) => return,
                };
            }
        };
        let delay = LOAD_DELAY_MS.load(Ordering::Relaxed);
        if delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
        wait_while_held();
        let r = load_one(&w);
        if tx.send((w, r)).is_err() {
            return;
        }
    }
}
