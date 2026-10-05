//! The peak store: every source's [`SourcePeaks`], computed off the UI thread, kept in
//! memory for the session and on disk between sessions.
//!
//! A view asks with [`PeakStore::lookup`] every frame; the first ask queues the file on a
//! worker thread and answers [`Lookup::Pending`], and the view draws a placeholder until
//! the peaks arrive (the worker asks egui for a repaint when they do). The disk cache is
//! keyed by the file's path, size and modification time (FNV-1a, stable across builds),
//! so a reopened project draws at once and an edited file is read again.
//!
//! The store is used through `&self` — the arrangement paints with the application
//! borrowed shared — so its state sits behind a mutex only the UI thread ever takes.

use super::SourcePeaks;
use cantus_core::{AudioSource, AudioSourceId, SampleData};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where a source's file is: its path as stored, against the project's folder when it is
/// relative (ADR 0025 §2). `base` is the folder of the project file, `None` before the
/// first save.
pub fn resolve(base: Option<&Path>, path: &str) -> PathBuf {
    let p = Path::new(path);
    match base {
        Some(dir) if p.is_relative() => dir.join(p),
        _ => p.to_path_buf(),
    }
}

/// What the store knows about a file.
#[derive(Clone, Debug)]
pub enum Lookup {
    Ready(Arc<SourcePeaks>),
    /// Being read.
    Pending,
    /// There is no file at the path.
    Missing,
    /// There is a file, and it could not be decoded (the reason).
    Failed(String),
}

#[derive(Clone, Debug)]
enum Entry {
    Pending,
    Ready(Arc<SourcePeaks>),
    Missing,
    Failed(String),
}

enum Job {
    /// Decode the file (or read the disk cache) and reduce it.
    File(PathBuf),
    /// The samples are already decoded (an import): just reduce them.
    Data(PathBuf, Arc<SampleData>),
}

struct Done {
    path: PathBuf,
    entry: Entry,
}

#[derive(Default)]
struct Inner {
    by_path: HashMap<PathBuf, Entry>,
    /// A source's stored path and base, and where they resolved to: the per-frame lookup
    /// compares the two and hashes the resolved path, with no allocation.
    by_source: HashMap<AudioSourceId, (String, Option<PathBuf>, PathBuf)>,
    jobs: Option<Sender<(Job, Option<egui::Context>)>>,
    done: Option<Receiver<Done>>,
    pending: usize,
    cache_dir: Option<PathBuf>,
    /// The last context a view looked up with: the one asked to repaint when work done
    /// for a caller without one (an import) arrives.
    ctx: Option<egui::Context>,
}

/// See the module docs.
#[derive(Default)]
pub struct PeakStore {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for PeakStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PeakStore")
    }
}

impl PeakStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Keep peak files in `dir` (`None`: memory only, which is what tests use).
    pub fn set_disk_cache(&self, dir: Option<PathBuf>) {
        self.lock().cache_dir = dir;
    }

    /// The peaks of a source of the project whose file sits in `base`, queuing the file
    /// the first time. `ctx` is asked to repaint when the peaks arrive.
    pub fn lookup(
        &self,
        source: &AudioSource,
        base: Option<&Path>,
        ctx: Option<&egui::Context>,
    ) -> Lookup {
        let mut g = self.lock();
        g.receive();
        let fresh = match g.by_source.get(&source.id) {
            Some((p, b, _)) => p != &source.path || b.as_deref() != base,
            None => true,
        };
        if fresh {
            let resolved = resolve(base, &source.path);
            g.by_source.insert(
                source.id,
                (source.path.clone(), base.map(Path::to_path_buf), resolved),
            );
        }
        let path = match g.by_source.get(&source.id) {
            Some((_, _, p)) => p.clone(),
            None => return Lookup::Pending,
        };
        g.lookup_path(path, ctx)
    }

    /// The peaks of a file by its resolved path, queuing it the first time.
    pub fn lookup_path(&self, path: &Path, ctx: Option<&egui::Context>) -> Lookup {
        let mut g = self.lock();
        g.receive();
        g.lookup_path(path.to_path_buf(), ctx)
    }

    /// Reduce samples already decoded (an import, a finished take) instead of reading the
    /// file again.
    pub fn insert_data(&self, path: &Path, data: Arc<SampleData>) {
        let mut g = self.lock();
        g.by_path.insert(path.to_path_buf(), Entry::Pending);
        g.send(Job::Data(path.to_path_buf(), data), None);
    }

    /// Forget a file, so the next lookup reads it again (it was relinked or replaced).
    pub fn forget(&self, path: &Path) {
        let mut g = self.lock();
        g.by_path.remove(path);
        g.by_source.retain(|_, (_, _, p)| p != path);
    }

    /// A file moved from `before` to `after` (collected on save): its peaks go with it.
    pub fn follow(&self, before: &Path, after: &Path) {
        let mut g = self.lock();
        g.receive();
        if let Some(e) = g.by_path.get(before).cloned() {
            if matches!(e, Entry::Ready(_)) {
                g.by_path.insert(after.to_path_buf(), e);
            }
        }
        if !before.exists() {
            g.by_path.remove(before);
        }
    }

    /// Take finished work; `true` when something arrived.
    pub fn poll(&self) -> bool {
        self.lock().receive()
    }

    /// Whether the worker has nothing queued.
    pub fn idle(&self) -> bool {
        let mut g = self.lock();
        g.receive();
        g.pending == 0
    }

    /// Wait (polling) until the worker is idle or `timeout` passes — for tests and the
    /// offline screenshots, never for a frame.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        loop {
            if self.idle() {
                return true;
            }
            if Instant::now() > until {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

impl Inner {
    fn lookup_path(&mut self, path: PathBuf, ctx: Option<&egui::Context>) -> Lookup {
        match self.by_path.get(&path) {
            Some(Entry::Ready(p)) => Lookup::Ready(p.clone()),
            Some(Entry::Pending) => Lookup::Pending,
            Some(Entry::Missing) => Lookup::Missing,
            Some(Entry::Failed(e)) => Lookup::Failed(e.clone()),
            None => {
                // A missing file is known at once: there is nothing to queue.
                if !path.is_file() {
                    self.by_path.insert(path, Entry::Missing);
                    return Lookup::Missing;
                }
                self.by_path.insert(path.clone(), Entry::Pending);
                self.send(Job::File(path), ctx);
                Lookup::Pending
            }
        }
    }

    fn send(&mut self, job: Job, ctx: Option<&egui::Context>) {
        if let Some(c) = ctx {
            if self.ctx.as_ref() != Some(c) {
                self.ctx = Some(c.clone());
            }
        }
        let ctx = self.ctx.clone();
        if self.jobs.is_none() {
            let (jtx, jrx) = channel::<(Job, Option<egui::Context>)>();
            let (dtx, drx) = channel::<Done>();
            let cache = self.cache_dir.clone();
            let spawned = std::thread::Builder::new()
                .name("cantus-peaks".into())
                .spawn(move || worker(jrx, dtx, cache));
            if spawned.is_err() {
                return;
            }
            self.jobs = Some(jtx);
            self.done = Some(drx);
        }
        if let Some(tx) = &self.jobs {
            if tx.send((job, ctx)).is_ok() {
                self.pending += 1;
            }
        }
    }

    fn receive(&mut self) -> bool {
        let Some(rx) = &self.done else {
            return false;
        };
        let mut got = Vec::new();
        while let Ok(d) = rx.try_recv() {
            got.push(d);
        }
        let any = !got.is_empty();
        for d in got {
            self.pending = self.pending.saturating_sub(1);
            self.by_path.insert(d.path, d.entry);
        }
        any
    }
}

/// The worker: one file at a time, in the order asked.
fn worker(
    jobs: Receiver<(Job, Option<egui::Context>)>,
    done: Sender<Done>,
    cache: Option<PathBuf>,
) {
    while let Ok((job, ctx)) = jobs.recv() {
        let (path, entry) = match job {
            Job::File(path) => {
                let entry = read(&path, cache.as_deref());
                (path, entry)
            }
            Job::Data(path, data) => {
                let peaks = SourcePeaks::compute(data.rate, data.channels, &data.frames);
                if let Some(dir) = cache.as_deref() {
                    if let Some(key) = disk::key(&path) {
                        let _ = disk::write(&dir.join(key), &peaks);
                    }
                }
                (path, Entry::Ready(Arc::new(peaks)))
            }
        };
        if done.send(Done { path, entry }).is_err() {
            return;
        }
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    }
}

/// The disk cache first, then the file.
fn read(path: &Path, cache: Option<&Path>) -> Entry {
    if !path.is_file() {
        return Entry::Missing;
    }
    let key = disk::key(path);
    if let (Some(dir), Some(k)) = (cache, key.as_deref()) {
        if let Some(p) = disk::read(&dir.join(k)) {
            return Entry::Ready(Arc::new(p));
        }
    }
    match cantus_engine::SampleCache::new().load(path) {
        Ok(data) => {
            let peaks = SourcePeaks::compute(data.rate, data.channels, &data.frames);
            if let (Some(dir), Some(k)) = (cache, key) {
                let _ = std::fs::create_dir_all(dir);
                let _ = disk::write(&dir.join(k), &peaks);
            }
            Entry::Ready(Arc::new(peaks))
        }
        Err(e) => Entry::Failed(e),
    }
}

#[path = "waveform_disk.rs"]
pub(crate) mod disk;
