//! Imported characters, loaded off the interface's thread (round 35, ADR 0087, ADR 0089):
//! a file may be large, slow or hostile, and the interface must never stall or fall on
//! one. A small worker thread asks import's loader (`cantus_dance::import::load`:
//! sandboxed — `paths::resolve` refuses absolute and `..` keys —, bounded, its own cache
//! checked against the files); meanwhile the default character dances and the screen says
//! it is loading.
//!
//! Nothing is kept for good — a miss no more than a hit: every [`RECHECK`] seconds a
//! character in use is looked at again by the worker. A loaded one is read again only when
//! its files changed (their size and modification time: the character file and its
//! `character.json`), else the same `Arc` stays; a miss is tried again. Answers are kept by
//! (project folder, key): an edit of its moves (a new package, a new key), a save that
//! carries the character to another folder (its key changing before the project's path
//! does), a file replaced — each reaches the screen.

use cantus_dance::character::Character;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::SystemTime;

/// How often an import in use is asked for again, in seconds of the wall clock.
pub const RECHECK: f64 = 0.5;

/// What a load is for: the project's folder and the key (`file:<rel>`).
type Key = (PathBuf, String);
/// The character file's and its package's size and modification time.
type Stamp = Vec<(u64, Option<SystemTime>)>;

/// What the worker answers: read (with the files' stamp), the same as before, or why not.
enum Reply {
    Read(Arc<dyn Character>, Stamp),
    Unchanged,
    Failed(String),
}

type Answer = (Key, Reply);

struct Worker {
    tx: Sender<(Key, Option<Stamp>)>,
    rx: Receiver<Answer>,
    /// Characters read from their files since it started (a test's measure).
    reads: Arc<AtomicU64>,
}

/// The stamp of the files a key names in `dir`, or `None` when it names nothing readable
/// there (the sandbox refuses it, or it is gone).
fn stamp_of(dir: &Path, rel: &str) -> Option<Stamp> {
    let file = cantus_dance::import::paths::resolve(dir, rel).ok()?;
    let package = file
        .parent()
        .unwrap_or(dir)
        .join(cantus_dance::import::package::PACKAGE_FILE);
    let one = |p: &Path| match std::fs::metadata(p) {
        Ok(m) => (m.len(), m.modified().ok()),
        Err(_) => (u64::MAX, None),
    };
    let s = vec![one(&file), one(&package)];
    (s[0].0 != u64::MAX).then_some(s)
}

impl Worker {
    fn start() -> Option<Worker> {
        let (tx, jobs) = channel::<(Key, Option<Stamp>)>();
        let (done, rx) = channel::<Answer>();
        let reads = Arc::new(AtomicU64::new(0));
        let counted = reads.clone();
        std::thread::Builder::new()
            .name("cantus-dance-load".into())
            .spawn(move || {
                while let Ok(((dir, id), had)) = jobs.recv() {
                    let rel = id.trim_start_matches("file:").to_string();
                    let now = stamp_of(&dir, &rel);
                    let reply = if had.is_some() && now == had {
                        Reply::Unchanged
                    } else {
                        counted.fetch_add(1, Ordering::Relaxed);
                        // A panic in a reader is an answer, never the interface's end.
                        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            cantus_dance::import::load(&dir, &rel).map_err(|e| e.to_string())
                        }))
                        .unwrap_or_else(|_| {
                            Err("the file could not be read (it broke the reader)".into())
                        });
                        match res {
                            Ok(c) => Reply::Read(c, now.unwrap_or_default()),
                            Err(e) => Reply::Failed(e),
                        }
                    };
                    if done.send(((dir, id), reply)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Worker { tx, rx, reads })
    }
}

#[derive(Default)]
struct State {
    result: Option<Result<Arc<dyn Character>, String>>,
    /// The files' stamp when it was read (a re-check compares it).
    stamp: Option<Stamp>,
    pending: bool,
    asked_at: f64,
}

/// What a `file:` key is now.
pub enum Got {
    Ready(Arc<dyn Character>),
    Loading,
    Failed(String),
}

/// The imports asked for, by folder and key, and the worker that reads them.
#[derive(Default)]
pub struct Loads {
    worker: Option<Worker>,
    states: HashMap<Key, State>,
}

impl Loads {
    fn ask(&mut self, key: &Key, wall: f64) {
        if self.worker.is_none() {
            self.worker = Worker::start();
        }
        let st = self.states.entry(key.clone()).or_default();
        st.pending = true;
        st.asked_at = wall;
        let had = match &st.result {
            Some(Ok(_)) => st.stamp.clone(),
            _ => None,
        };
        let sent = self
            .worker
            .as_ref()
            .is_some_and(|w| w.tx.send((key.clone(), had)).is_ok());
        if !sent {
            st.pending = false;
            st.result = Some(Err("the loader is not running".into()));
        }
    }

    fn answer(&mut self, key: Key, reply: Reply) {
        if let Some(st) = self.states.get_mut(&key) {
            match reply {
                Reply::Read(c, stamp) => {
                    st.result = Some(Ok(c));
                    st.stamp = Some(stamp);
                }
                Reply::Unchanged => {}
                Reply::Failed(e) => {
                    st.result = Some(Err(e));
                    st.stamp = None;
                }
            }
            st.pending = false;
        }
    }

    /// Characters read from their files since the worker started.
    pub fn reads(&self) -> u64 {
        self.worker
            .as_ref()
            .map_or(0, |w| w.reads.load(Ordering::Relaxed))
    }

    fn drain(&mut self) {
        let mut got = Vec::new();
        if let Some(w) = self.worker.as_ref() {
            while let Ok(a) = w.rx.try_recv() {
                got.push(a);
            }
        }
        for (k, r) in got {
            self.answer(k, r);
        }
    }

    /// The import at `id` (`file:<rel>`) in `dir`: ready, loading, or why it failed. Asks
    /// the worker the first time this folder and key are seen, and every [`RECHECK`]
    /// seconds after (a hit and a miss alike). While a new folder's answer is on its way,
    /// the same key's character from another folder stands in.
    pub fn get(&mut self, id: &str, dir: &Path, wall: f64) -> Got {
        self.drain();
        let key = (dir.to_path_buf(), id.to_string());
        let due = match self.states.get(&key) {
            None => true,
            Some(st) => !st.pending && (wall - st.asked_at >= RECHECK || wall < st.asked_at),
        };
        if due {
            self.ask(&key, wall);
        }
        match self.states.get(&key).and_then(|s| s.result.as_ref()) {
            Some(Ok(c)) => Got::Ready(c.clone()),
            Some(Err(e)) => Got::Failed(e.clone()),
            None => self
                .states
                .iter()
                .find_map(|((_, k), s)| match &s.result {
                    Some(Ok(c)) if k == id => Some(Got::Ready(c.clone())),
                    _ => None,
                })
                .unwrap_or(Got::Loading),
        }
    }

    /// Wait (at most `timeout`) until no load is under way: the tests'.
    pub fn wait(&mut self, timeout: std::time::Duration) {
        let end = std::time::Instant::now() + timeout;
        while self.states.values().any(|s| s.pending) && std::time::Instant::now() < end {
            let got = match self.worker.as_ref() {
                Some(w) => w.rx.recv_timeout(std::time::Duration::from_millis(20)).ok(),
                None => break,
            };
            if let Some((k, r)) = got {
                self.answer(k, r);
            }
        }
    }

    /// Forget every import (another project).
    pub fn clear(&mut self) {
        self.states.clear();
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}
