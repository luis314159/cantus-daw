//! Exemplar's streaming from disk (ADR 0050): the process's one reader thread, the pool it
//! fills, the streams it serves and the boards it reads cursors from, and how a sample
//! becomes a stream — decoded once into a cache file at its own depth, heads computed, then
//! only heads read at every later load.
//!
//! **One stream per file** as it is (path and [`Identity`]), whatever zones play it and
//! however (review 13, 14): it holds every mip level the file has, each zone takes the
//! levels its own range needs, and a zone's later start is kept ready by the reader from
//! the instrument's start board. The stream travels with its head ([`SampleData::stream`]):
//! no registry, nothing to fill up; it is released when nothing holds it any more.
//!
//! Nothing here runs on the audio thread except [`lookup`] (allocation-free). Plain `std`
//! threads and file I/O, like the sample cache's loader.

use super::stream_cache::{self, CacheFile, Identity};
use cantus_core::{SampleData, StreamRef};
use cantus_dsp::exemplar::resample::{halve_reach, halve_span, MAX_LEVELS};
use cantus_dsp::exemplar::stream::{Board, Pool, StartPin, Stream, BLOCK, HEAD_FRAMES};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

#[path = "stream_reader.rs"]
mod reader;

pub use reader::{
    inject_io_fault, inject_io_faults, level0_frames_read, set_read_delay, LOOKAHEAD,
};

/// The automatic size limit's ceiling (Settings may choose another limit).
pub const AUTO_CACHE_CAP: u64 = 16 << 30;
/// Space the cache leaves free on its disk: its automatic limit keeps under it, and no
/// cache file is written that would cross it (second review #10).
pub const FREE_FLOOR: u64 = 5 << 30;

/// The floor of free space cache writes keep to (tests on a nearly full machine set it 0).
static FLOOR: AtomicU64 = AtomicU64::new(FREE_FLOOR);

/// Set the floor of free space cache writes keep to (tests whose machine may be nearly
/// full: the cache files they need are small).
pub fn set_free_floor(bytes: u64) {
    FLOOR.store(bytes, Ordering::Relaxed);
}

/// The automatic limit for a cache on a disk of `total` bytes with `free` free, holding
/// `held` now: the smallest of 16 GB, a tenth of the disk, and what leaves 5 GB free.
pub fn auto_limit(space: Option<cantus_rt::disk::Space>, held: u64) -> u64 {
    match space {
        Some(s) => AUTO_CACHE_CAP
            .min(s.total / 10)
            .min((s.free + held).saturating_sub(FREE_FLOOR)),
        None => AUTO_CACHE_CAP,
    }
}

/// The cache's limit and the disk it is on, for Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheRoom {
    /// The limit in force, bytes.
    pub limit: u64,
    /// Whether it is the automatic one.
    pub auto: bool,
    /// The disk, when the system says.
    pub space: Option<cantus_rt::disk::Space>,
    /// Bytes the cache holds now.
    pub held: u64,
}

/// Where a stream's level 0 is read from: the decoded file in memory while its cache file
/// is being written (a first load plays at once), then the cache file.
#[derive(Debug)]
enum Backing {
    Memory(Arc<[f32]>),
    File(Open),
}

/// A cache file a stream reads, held open with a shared lock no sweep deletes it under —
/// this process's or another Cantus's (second review #6) — while the handle budget allows:
/// past it the least recently read are closed and opened again when read, the lock taken
/// again, a file gone by then recovered like a damaged one (third review #1).
#[derive(Debug)]
struct Open {
    c: CacheFile,
    f: Option<std::fs::File>,
    /// The reader's look that last read it (the least recent is closed first).
    used: u64,
}

impl Open {
    /// The file, checked openable and not being deleted; held open when the budget allows.
    fn new(c: CacheFile) -> Option<Open> {
        let f = if handles::take() {
            match stream_cache::open_locked(&c.path) {
                Some(f) => Some(f),
                None => {
                    handles::give();
                    return None;
                }
            }
        } else {
            stream_cache::open_locked(&c.path)?;
            None
        };
        Some(Open { c, f, used: 0 })
    }

    /// Run `read` on the held file, or on one opened for it (closed after).
    fn with_file<R>(
        &mut self,
        read: impl FnOnce(&CacheFile, &mut std::fs::File) -> R,
    ) -> Option<R> {
        match self.f.as_mut() {
            Some(f) => Some(read(&self.c, f)),
            None => {
                let mut f = stream_cache::open_locked(&self.c.path)?;
                Some(read(&self.c, &mut f))
            }
        }
    }

    /// Let go of the handle (the budget is spent elsewhere).
    fn close(&mut self) {
        if self.f.take().is_some() {
            handles::give();
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.close();
    }
}

/// The cache files' handle budget (third review #1): a quarter of the process's open-files
/// limit (raised at start), at most 1 024 — the rest is the application's.
pub(crate) mod handles {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static OPEN: AtomicUsize = AtomicUsize::new(0);
    static BUDGET: AtomicUsize = AtomicUsize::new(0);

    pub fn budget() -> usize {
        let b = BUDGET.load(Ordering::Relaxed);
        if b > 0 {
            return b;
        }
        let b = (cantus_rt::files::limit() / 4).clamp(16, 1_024) as usize;
        BUDGET.store(b, Ordering::Relaxed);
        b
    }

    /// A handle from the budget, if one is left.
    pub fn take() -> bool {
        let budget = budget();
        OPEN.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < budget).then_some(n + 1)
        })
        .is_ok()
    }

    /// A handle beyond the budget (the reader, when nothing could be closed).
    pub fn force() {
        OPEN.fetch_add(1, Ordering::AcqRel);
    }

    pub fn give() {
        OPEN.fetch_sub(1, Ordering::AcqRel);
    }

    pub fn open() -> usize {
        OPEN.load(Ordering::Acquire)
    }

    pub fn set_budget(n: usize) {
        BUDGET.store(n.max(1), Ordering::Relaxed);
    }
}

/// Cache-file handles held open now, and the budget (tests, the memory readout).
pub fn handles_open() -> (usize, usize) {
    (handles::open(), handles::budget())
}

/// Set the cache files' handle budget (tests of a small one).
pub fn set_handle_budget(n: usize) {
    handles::set_budget(n);
}

/// A stream and where its level 0 is read from.
#[derive(Debug)]
struct Entry {
    stream: Arc<Stream>,
    backing: Mutex<Backing>,
    /// The file it plays, as it was when the stream was made.
    source: PathBuf,
    id: Identity,
    /// A block could not be read: the source is being decoded again (second review #3,
    /// #5, #15). The reader reads nothing of it meanwhile; offline voices wait.
    recovering: std::sync::atomic::AtomicBool,
    /// Its cache file was opened again after a read error (once: fourth review #5).
    reopened: std::sync::atomic::AtomicBool,
}

impl Entry {
    fn file_path(&self) -> Option<PathBuf> {
        match &*self.backing.lock().unwrap_or_else(|e| e.into_inner()) {
            Backing::File(o) => Some(o.c.path.clone()),
            Backing::Memory(_) => None,
        }
    }
}

/// Bytes of decoded files the cache writer may hold before a loader waits for it: the
/// first load's memory, bounded.
const WRITE_BOUND: u64 = 1 << 30;

/// A pause before every cache write, ms (tests of the switch from memory to file).
static WRITE_DELAY_MS: AtomicU64 = AtomicU64::new(0);

/// Make every cache write wait `d` first — for the test of a first load's switch.
pub fn set_write_delay(d: Duration) {
    WRITE_DELAY_MS.store(d.as_millis() as u64, Ordering::Relaxed);
}

/// A cache file to write for a stream that plays from memory meanwhile.
struct WriteJob {
    dir: PathBuf,
    source: PathBuf,
    id: Identity,
    channels: usize,
    rate: u32,
    data: Arc<[f32]>,
    entry: Arc<Entry>,
}

/// The cache writers: a few threads, the jobs in order, the decoded bytes they hold bounded.
struct Writer {
    jobs: std::sync::mpsc::Sender<WriteJob>,
    held: Mutex<u64>,
    room: std::sync::Condvar,
}

fn writer() -> &'static Writer {
    static W: OnceLock<Writer> = OnceLock::new();
    W.get_or_init(|| {
        let (jobs, rx) = std::sync::mpsc::channel::<WriteJob>();
        // Two writers: writing in parallel helps a little; more of them only compete with
        // the loaders' reads (measured: 1, 2 and 8 writers, 17.4 / 15.2 / 16–18 s for the
        // 8-layer piano's first load here, disk-bound).
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..2 {
            let rx = rx.clone();
            let _ = std::thread::Builder::new()
                .name(format!("cantus-cache-writer-{i}"))
                .spawn(move || loop {
                    let next = rx.lock().ok().and_then(|r| r.recv().ok());
                    let Some(j) = next else {
                        break;
                    };
                    {
                        let delay = WRITE_DELAY_MS.load(Ordering::Relaxed);
                        if delay > 0 {
                            std::thread::sleep(Duration::from_millis(delay));
                        }
                        let bytes = j.data.len() as u64 * 4;
                        // Never the file that would leave the disk under its floor of free
                        // space (second review #10): it plays from memory, and says so.
                        let floor = FLOOR.load(Ordering::Relaxed);
                        let room =
                            cantus_rt::disk::space(&j.dir).is_none_or(|s| s.free >= bytes + floor);
                        let written = if room {
                            stream_cache::write(
                                &j.dir, &j.source, &j.id, j.channels, j.rate, &j.data,
                            )
                        } else {
                            Err(std::io::Error::other("the disk is nearly full"))
                        };
                        match written {
                            Ok(c) => {
                                // The switch: blocks read from here on come from the file, the
                                // same values (the depth is exact); the decode is let go. (A
                                // file a sweep took at once: it plays on from memory.)
                                if let Some(o) = Open::new(c) {
                                    *j.entry.backing.lock().unwrap_or_else(|e| e.into_inner()) =
                                        Backing::File(o);
                                }
                            }
                            Err(e) => {
                                // Review 8: it keeps playing from memory, and says so once.
                                log::warn!("sample cache {}: {e}", j.dir.display());
                                if let Ok(mut n) = hub().notes.lock() {
                                    n.push(LoadNote::CacheUnwritable(e.to_string()));
                                }
                            }
                        }
                        drop(j);
                        let w = writer();
                        if let Ok(mut h) = w.held.lock() {
                            *h = h.saturating_sub(bytes);
                        }
                        w.room.notify_all();
                    }
                });
        }
        Writer {
            jobs,
            held: Mutex::new(0),
            room: std::sync::Condvar::new(),
        }
    })
}

/// Queue a cache file to write; waits (a loader thread) while the writer holds more than
/// [`WRITE_BOUND`] of decoded files.
fn write_later(job: WriteJob) {
    reserve(job.data.len() as u64 * 4);
    send_reserved(job);
}

/// Count `bytes` of decoded samples against the writers' bound, waiting while they hold
/// more (a loader's, or a recovery's before it decodes — third review #4).
fn reserve(bytes: u64) {
    let w = writer();
    if let Ok(mut h) = w.held.lock() {
        while *h > 0 && *h + bytes > WRITE_BOUND {
            h = match w.room.wait(h) {
                Ok(h) => h,
                Err(e) => e.into_inner(),
            };
        }
        *h += bytes;
    }
}

/// Give back bytes reserved for a decode that will not be written.
fn unreserve(bytes: u64) {
    let w = writer();
    if let Ok(mut h) = w.held.lock() {
        *h = h.saturating_sub(bytes);
    }
    w.room.notify_all();
}

/// Queue a cache file whose bytes are reserved already.
fn send_reserved(job: WriteJob) {
    let _ = writer().jobs.send(job);
}

/// Whether the stream a head carries still plays from its decoded file (its cache file
/// not written yet).
pub fn memory_backed(head: &SampleData) -> bool {
    let Some(st) = lookup(head) else {
        return false;
    };
    let i = hub().inner.lock().unwrap_or_else(|e| e.into_inner());
    i.entries
        .get(st.id as usize - 1)
        .and_then(Option::as_ref)
        .is_some_and(|e| {
            matches!(
                &*e.backing.lock().unwrap_or_else(|e| e.into_inner()),
                Backing::Memory(_)
            )
        })
}

#[derive(Debug, Default)]
struct Inner {
    /// By stream id − 1; `None` once released.
    entries: Vec<Option<Arc<Entry>>>,
    /// Heads made, by file as it is, so two caches (or a reload) share one stream.
    by_file: HashMap<(PathBuf, Identity), Weak<SampleData>>,
    /// Every instrument's boards, as long as the instrument has them (second review #1:
    /// the hub never keeps one alive).
    boards: Vec<Weak<Board>>,
    cache_dir: Option<PathBuf>,
}

impl Inner {
    /// Streams only the hub holds now (every head dropped) go.
    fn let_go_unheld(&mut self) {
        for slot in self.entries.iter_mut() {
            if slot
                .as_ref()
                .is_some_and(|e| Arc::strong_count(&e.stream) == 1)
            {
                *slot = None;
            }
        }
        self.by_file.retain(|_, w| w.strong_count() > 0);
    }
}

/// The process's streaming state.
#[derive(Debug)]
pub struct Hub {
    inner: Mutex<Inner>,
    /// What the cache writer had to say (taken by the sample cache for the status line).
    notes: Mutex<Vec<LoadNote>>,
    pool: OnceLock<Arc<Pool>>,
    reader: OnceLock<std::thread::Thread>,
    /// Bumped when entries or boards change: the reader refreshes its copies then.
    generation: AtomicU64,
    /// The limit Settings chose, bytes; 0: automatic ([`auto_limit`]).
    limit: AtomicU64,
    /// One sweep at a time in this process (second review #8).
    sweeping: Mutex<()>,
    /// Sweeps started (tests: the start sweep ran).
    sweeps: AtomicU64,
}

static HUB: OnceLock<Hub> = OnceLock::new();

pub fn hub() -> &'static Hub {
    HUB.get_or_init(|| {
        // Every streamed file may hold a handle: the open-files limit raised first (third
        // review #1).
        cantus_rt::files::raise_limit();
        new_hub()
    })
}

fn new_hub() -> Hub {
    Hub {
        inner: Mutex::new(Inner::default()),
        notes: Mutex::new(Vec::new()),
        pool: OnceLock::new(),
        reader: OnceLock::new(),
        generation: AtomicU64::new(0),
        limit: AtomicU64::new(0),
        sweeping: Mutex::new(()),
        sweeps: AtomicU64::new(0),
    }
}

/// A voice moved to a new block (on the audio thread): the reader looks now. `unpark` takes
/// no lock and allocates nothing.
fn wake_reader() {
    if let Some(t) = HUB.get().and_then(|h| h.reader.get()) {
        t.unpark();
    }
}

/// What an offline voice does while it waits: wake the reader, sleep a moment.
fn idle() {
    if let Some(t) = hub().reader.get() {
        t.unpark();
    }
    std::thread::sleep(Duration::from_micros(30));
}

/// The stream a head carries, if it is one. Allocation-free: `configure` calls it on the
/// audio thread (an `Arc` clone).
pub fn lookup(head: &SampleData) -> Option<Arc<Stream>> {
    head.stream.as_ref()?.0.clone().downcast::<Stream>().ok()
}

impl Hub {
    /// Where cache files go (the app's data folder), and a tidy-up of it beside the window
    /// (stray parts, gone sources, the size limit).
    pub fn set_cache_dir(&self, dir: PathBuf) {
        if let Ok(mut i) = self.inner.lock() {
            i.cache_dir = Some(dir);
        }
        let _ = std::thread::Builder::new()
            .name("cantus-cache-sweep".into())
            .spawn(|| {
                let _ = hub().sweep();
            });
    }

    /// Where cache files go: the folder set ([`Self::set_cache_dir`]) — the app's data
    /// folder, a test's own temporary one. With none set, a release build falls back to
    /// `%TEMP%/cantus-stream-cache`; a debug build (every test run) refuses, loudly: no test
    /// may write the shared fallback (it outlives the run, and others' runs read it).
    pub fn cache_dir(&self) -> PathBuf {
        let set = self.inner.lock().ok().and_then(|i| i.cache_dir.clone());
        if let Some(dir) = set {
            return dir;
        }
        if cfg!(debug_assertions) {
            panic!("no sample cache folder set: a test or tool that streams sets its own");
        }
        std::env::temp_dir().join("cantus-stream-cache")
    }

    /// Whether a cache folder was set (the fallback is not used).
    pub fn cache_dir_set(&self) -> bool {
        self.inner.lock().is_ok_and(|i| i.cache_dir.is_some())
    }

    /// The folder to look at or tidy: the one set, the fallback in a release build, none in
    /// a debug build with none set (nothing to read there, nothing to sweep).
    fn cache_dir_to_read(&self) -> Option<PathBuf> {
        (self.cache_dir_set() || !cfg!(debug_assertions)).then(|| self.cache_dir())
    }

    /// The cache folder's size limit, bytes (Settings' choice).
    pub fn set_cache_limit(&self, bytes: u64) {
        self.limit.store(bytes.max(1), Ordering::Relaxed);
    }

    /// The automatic limit: the smallest of 16 GB, a tenth of the disk, and what leaves
    /// 5 GB free — computed when it is used.
    pub fn set_cache_auto(&self) {
        self.limit.store(0, Ordering::Relaxed);
    }

    /// The limit in force, bytes (the automatic one reads the disk: off the audio thread).
    pub fn cache_limit(&self) -> u64 {
        self.cache_room().limit
    }

    /// The limit in force, whether it is automatic, the disk and what the cache holds.
    pub fn cache_room(&self) -> CacheRoom {
        let dir = self.cache_dir_to_read();
        let held = dir.as_deref().map_or(0, stream_cache::size_of);
        let space = cantus_rt::disk::space(dir.as_deref().unwrap_or(&std::env::temp_dir()));
        let chosen = self.limit.load(Ordering::Relaxed);
        CacheRoom {
            limit: if chosen == 0 {
                auto_limit(space, held)
            } else {
                chosen
            },
            auto: chosen == 0,
            space,
            held,
        }
    }

    /// Keep `pins` ready: a board of them the reader reads (as long as it is held), handed
    /// back at once — nothing waits here (third review #6: the interface's thread compiles).
    pub fn keep_ready(&self, pins: &[StartPin]) -> Arc<Board> {
        let board = Arc::new(Board::new_pins(pins.len().max(1)));
        for (i, p) in pins.iter().enumerate() {
            board.set(i, p.pack());
        }
        self.register_board(&board);
        self.boards_changed();
        board
    }

    /// Whether every block `pins` keep ready is in the pool (or its stream failed or went).
    /// Never waits.
    pub fn pins_resident(&self, pins: &[StartPin]) -> bool {
        let Ok(i) = self.inner.lock() else {
            return true;
        };
        pins.iter().all(|p| {
            let Some(e) = i
                .entries
                .get(p.stream as usize - 1)
                .and_then(Option::as_ref)
            else {
                return true;
            };
            let st = &e.stream;
            st.failed()
                || (0..p.levels.min(st.levels())).all(|k| {
                    let head = st.head_frames(k) / BLOCK;
                    p.blocks(k)
                        .filter(|b| *b >= head && *b < st.blocks(k))
                        .all(|b| st.resident(k, b))
                })
        })
    }

    /// Sweeps started in this process.
    pub fn sweeps_started(&self) -> u64 {
        self.sweeps.load(Ordering::Relaxed)
    }

    /// Bytes the cache folder holds now.
    pub fn cache_size(&self) -> u64 {
        self.cache_dir_to_read()
            .map_or(0, |d| stream_cache::size_of(&d))
    }

    /// Tidy the cache folder under its limit; never a file a stream reads now. Off the
    /// audio thread (it reads headers and deletes files).
    pub fn sweep(&self) -> stream_cache::Swept {
        self.sweep_to(self.cache_limit())
    }

    /// *Clear cache*: every file nothing plays now (they are decoded again when needed).
    pub fn clear_cache(&self) -> stream_cache::Swept {
        self.sweep_to(0)
    }

    /// A pack's folder was deleted: the cache files of what was in it go now (its drive is
    /// not away: it is gone — second review #7).
    pub fn sweep_removed(&self, root: PathBuf) -> stream_cache::Swept {
        self.sweep_with(self.cache_limit(), &[root])
    }

    fn sweep_to(&self, limit: u64) -> stream_cache::Swept {
        self.sweep_with(limit, &[])
    }

    /// Let go of streams whose heads every holder dropped (a cache that went away): their
    /// cache files are closed (and unlocked) as the reader next looks.
    pub fn let_go_unheld(&self) {
        if let Ok(mut i) = self.inner.lock() {
            i.let_go_unheld();
        }
        self.generation.fetch_add(1, Ordering::Release);
        if let Some(t) = self.reader.get() {
            t.unpark();
        }
    }

    fn sweep_with(&self, limit: u64, removed: &[PathBuf]) -> stream_cache::Swept {
        // One at a time here (second review #8); across processes the folder's lock file.
        let _one = self.sweeping.lock().unwrap_or_else(|e| e.into_inner());
        self.let_go_unheld();
        self.sweeps.fetch_add(1, Ordering::Relaxed);
        // What streams read now, asked at each deletion — never a list taken before
        // (second review #6).
        let in_use = |p: &Path| {
            self.inner.lock().is_ok_and(|i| {
                i.entries
                    .iter()
                    .flatten()
                    .any(|e| e.file_path().as_deref() == Some(p))
            })
        };
        let Some(dir) = self.cache_dir_to_read() else {
            return stream_cache::Swept::default();
        };
        stream_cache::sweep(&dir, limit, &in_use, removed)
    }

    fn note(&self, n: LoadNote) {
        if let Ok(mut v) = self.notes.lock() {
            v.push(n);
        }
    }

    /// What the cache writer had to say since the last call.
    pub fn take_notes(&self) -> Vec<LoadNote> {
        self.notes
            .lock()
            .map(|mut n| std::mem::take(&mut *n))
            .unwrap_or_default()
    }

    /// Say where an instrument's layers (and zones' starts) publish what they read (with
    /// the instrument, off the audio thread). Boards nobody else holds are dropped.
    pub fn register_board(&self, board: &Arc<Board>) {
        if let Ok(mut i) = self.inner.lock() {
            i.boards.push(Arc::downgrade(board));
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// An instrument went: the reader lets go of its copies of its boards as it next looks.
    /// Allocation- and lock-free (it may be the audio thread).
    pub fn boards_changed(&self) {
        self.generation.fetch_add(1, Ordering::Release);
        if let Some(t) = self.reader.get() {
            t.unpark();
        }
    }

    pub fn pool(&self) -> &Arc<Pool> {
        self.pool
            .get_or_init(|| Arc::new(Pool::new(1, idle).with_wake(wake_reader)))
    }

    /// Streams alive now.
    pub fn streams(&self) -> usize {
        self.inner
            .lock()
            .map_or(0, |i| i.entries.iter().flatten().count())
    }

    fn start_reader(&self) {
        let t = self.reader.get_or_init(|| {
            std::thread::Builder::new()
                .name("cantus-stream-reader".into())
                .spawn(reader::run)
                .expect("the stream reader thread")
                .thread()
                .clone()
        });
        t.unpark();
    }
}

/// Let go of a stream nothing plays and no snapshot holds: `head` is the last holder of
/// its head (the sample cache's), and the stream's only other holder is this hub. Hands
/// `head` back when something still holds either. Off the audio thread.
pub fn release(head: Arc<SampleData>) -> Result<(), Arc<SampleData>> {
    let Some(st) = lookup(&head) else {
        return Err(head);
    };
    // Holders now: `st` here, the head's own handle, the hub's entry.
    if Arc::strong_count(&head) != 1 || Arc::strong_count(&st) != 3 {
        return Err(head);
    }
    let hub = hub();
    if let Ok(mut i) = hub.inner.lock() {
        if let Some(slot) = i.entries.get_mut(st.id as usize - 1) {
            *slot = None;
        }
        let me = Arc::downgrade(&head);
        i.by_file
            .retain(|_, w| w.strong_count() > 0 && !w.ptr_eq(&me));
    }
    hub.generation.fetch_add(1, Ordering::Release);
    // The reader lets go of its copy of the entry (and so the stream) when it looks again.
    if let Some(t) = hub.reader.get() {
        t.unpark();
    }
    Ok(())
}

/// Why a sample that could stream plays from memory, when it is worth saying (once, on
/// the status line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadNote {
    /// The cache folder cannot be written (the reason).
    CacheUnwritable(String),
    /// The file changed while it was read: what was read plays, nothing is cached.
    Changed(PathBuf),
    /// Its cache file could not be read (damaged, gone): the source is decoded again and
    /// plays on (second review #5).
    Damaged(PathBuf),
    /// Neither its cache file nor the source can be read (the reason): its notes end where
    /// the file could not be read, and an export fails naming it (second review #15).
    Unreadable(PathBuf, String),
    /// The source changed in its middle since it was cached, its size and time kept: found
    /// by the whole file's hash beside the reader, it is decoded again (second review #9).
    ChangedSinceCached(PathBuf),
}

/// Check, beside the reader and taking its time, that the whole of a stream's source is
/// what its cache file was written from (second review #9); when it is not, the stream is
/// let go — its notes end, the cache file goes — and the sample cache loads the file anew.
fn verify_later(entry: &Arc<Entry>, whole: u64) {
    use std::sync::mpsc::{channel, Sender};
    type Job = (Weak<Entry>, (PathBuf, Identity, u64));
    static VERIFIER: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    if whole == 0 {
        return;
    }
    // Once per file as it is, per session (third review #5): a stream opened again (a
    // preset switched back, an undo) is not hashed again.
    let key = (entry.source.clone(), entry.id, whole);
    if !checked().lock().is_ok_and(|mut c| c.insert(key.clone())) {
        return;
    }
    let tx = VERIFIER.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        let _ = std::thread::Builder::new()
            .name("cantus-cache-verify".into())
            .spawn(move || {
                while let Ok((entry, key)) = rx.recv() {
                    let whole = key.2;
                    let Some(e) = entry.upgrade().filter(|e| !e.stream.failed()) else {
                        // Gone before its turn: the next open checks it.
                        if let Ok(mut c) = checked().lock() {
                            c.remove(&key);
                        }
                        continue;
                    };
                    // About 500 MB a second at most: the reader's reads come first.
                    let pause = Duration::from_millis(2);
                    let h = stream_cache::source_hash(&e.source, pause);
                    SOURCES_HASHED.fetch_add(1, Ordering::Relaxed);
                    if h.is_ok_and(|h| h != whole) {
                        if let Ok(mut c) = checked().lock() {
                            c.remove(&key);
                        }
                        stale(&e);
                    }
                }
            });
        Mutex::new(tx)
    });
    if let Ok(tx) = tx.lock() {
        let _ = tx.send((Arc::downgrade(entry), key));
    }
}

type Checked = std::collections::HashSet<(PathBuf, Identity, u64)>;

/// Files (as they are, with their cache file's whole hash) checked or queued this session.
fn checked() -> &'static Mutex<Checked> {
    static C: OnceLock<Mutex<Checked>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Checked::new()))
}

/// Sources hashed by the background check since the start (tests).
static SOURCES_HASHED: AtomicU64 = AtomicU64::new(0);

pub fn sources_hashed() -> u64 {
    SOURCES_HASHED.load(Ordering::Relaxed)
}

/// A stream whose cache file is not its source's any more: let go, the file deleted.
fn stale(entry: &Arc<Entry>) {
    log::warn!(
        "sample cache: {} changed since it was cached",
        entry.source.display()
    );
    entry.stream.set_failed();
    let hub = hub();
    if let Ok(mut i) = hub.inner.lock() {
        i.by_file.remove(&(entry.source.clone(), entry.id));
    }
    if let Some(p) = entry.file_path() {
        let _ = std::fs::remove_file(p);
    }
    hub.note(LoadNote::ChangedSinceCached(entry.source.clone()));
}

/// Why the reader could not read a block.
#[derive(Debug)]
enum Fault {
    /// Its checksum failed: the file is deleted (it is not what was written).
    Damaged,
    /// The cache file is not there any more.
    Gone,
    /// Any other error (a busy drive, an antivirus' lock): the file is kept.
    /// The error, and the first level-0 block of the read that failed.
    Io(String, usize),
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fault::Damaged => f.write_str("the cached samples are damaged"),
            Fault::Gone => f.write_str("the cache file is gone"),
            Fault::Io(e, _) => f.write_str(e),
        }
    }
}

/// A block of `entry`'s stream could not be read (the reader, off the audio thread): decode
/// the source again on a thread of its own and play on from the decode — then from a fresh
/// cache file when the old one was damaged or gone (second review #3, #5, #15). Voices in
/// real time that reach a block meanwhile fade (counted); offline ones wait. When the source
/// cannot be read either, or is another file now, the stream fails: its voices end, the
/// sample cache loads the file again (reporting it missing), and an export says so.
fn recover(entry: &Arc<Entry>, fault: &Fault) {
    use std::sync::atomic::Ordering::AcqRel;
    if entry.recovering.swap(true, AcqRel) || entry.stream.failed() {
        return;
    }
    let path = entry.file_path();
    // A frozen render (ADR 0064) is its own source: there is nothing to decode it from.
    // Damaged or gone, it fails at once and quietly — no "damaged sample" said — and the
    // interface makes it again (review 1); a read error that may pass (a busy drive, an
    // antivirus' lock) is read again first, its file kept (review 2).
    if is_frozen_render(entry) && !matches!(fault, Fault::Io(..)) {
        if let Fault::Damaged = fault {
            let _ = std::fs::remove_file(&entry.source);
        }
        log::warn!("frozen render {}: {fault}", entry.source.display());
        entry.stream.set_failed();
        entry
            .recovering
            .store(false, std::sync::atomic::Ordering::Release);
        if let Ok(mut i) = hub().inner.lock() {
            i.by_file.remove(&(entry.source.clone(), entry.id));
        }
        return;
    }
    log::warn!(
        "stream {}: {fault}; decoding {} again",
        path.as_deref().unwrap_or(Path::new("")).display(),
        entry.source.display()
    );
    let io = match fault {
        Fault::Io(_, block) => Some(*block),
        _ => None,
    };
    if let (Fault::Damaged, Some(p)) = (fault, &path) {
        let _ = std::fs::remove_file(p);
    }
    if io.is_none() {
        hub().note(LoadNote::Damaged(entry.source.clone()));
    }
    // One recovery at a time, in order (third review #4): never a decode per failing
    // stream at once.
    type Recovery = (Arc<Entry>, Option<usize>);
    static QUEUE: OnceLock<Mutex<std::sync::mpsc::Sender<Recovery>>> = OnceLock::new();
    let tx = QUEUE.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Recovery>();
        let _ = std::thread::Builder::new()
            .name("cantus-stream-recover".into())
            .spawn(move || {
                while let Ok((entry, io)) = rx.recv() {
                    RECOVERIES.fetch_add(1, Ordering::AcqRel);
                    let now = RECOVERING.fetch_add(1, Ordering::AcqRel) + 1;
                    RECOVERING_PEAK.fetch_max(now, Ordering::AcqRel);
                    recover_now(&entry, io);
                    RECOVERING.fetch_sub(1, Ordering::AcqRel);
                }
            });
        Mutex::new(tx)
    });
    let sent = tx
        .lock()
        .is_ok_and(|tx| tx.send((entry.clone(), io)).is_ok());
    if !sent {
        fail(entry, "no thread to decode it again".into());
    }
}

/// Recoveries running now, and the most there ever were at once (tests).
static RECOVERING: AtomicU64 = AtomicU64::new(0);
static RECOVERING_PEAK: AtomicU64 = AtomicU64::new(0);

pub fn recoveries_at_once_peak() -> u64 {
    RECOVERING_PEAK.load(Ordering::Acquire)
}

/// Recoveries made since the start (tests).
static RECOVERIES: AtomicU64 = AtomicU64::new(0);

pub fn recoveries() -> u64 {
    RECOVERIES.load(Ordering::Acquire)
}

/// Whether `entry` streams a frozen render (ADR 0064).
fn is_frozen_render(entry: &Entry) -> bool {
    entry
        .source
        .extension()
        .is_some_and(|x| x == crate::freeze::EXTENSION)
}

/// Frozen renders whose reads failed for a passing reason (the disk, a lock) and failed
/// again after waiting: not damaged — their store looks at them again later (review 3).
static IO_FAILED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Whether the frozen render at `path` failed only for passing read errors (taken: asked
/// once per failure).
pub fn take_io_failed(path: &std::path::Path) -> bool {
    let Ok(mut v) = IO_FAILED.lock() else {
        return false;
    };
    match v.iter().position(|p| p == path) {
        Some(i) => {
            v.swap_remove(i);
            true
        }
        None => false,
    }
}

/// How long a frozen render's passing read error is waited out, try after try (review 3:
/// one immediate retry, once per stream, failed a good render for the session).
const FROZEN_RETRIES: [u64; 4] = [0, 50, 200, 1_000];

fn recover_now(entry: &Arc<Entry>, io: Option<usize>) {
    let st = &entry.stream;
    // A frozen render's passing read error: read again, waiting longer each time; read, it
    // plays on and the next error gets its tries again. Still failing: failed for now, its
    // file kept and looked at again later — only a damaged or vanished render is made
    // again (review 3).
    if let (Some(block), true) = (io, is_frozen_render(entry)) {
        for wait in FROZEN_RETRIES {
            std::thread::sleep(std::time::Duration::from_millis(wait));
            if reopen(entry, block) {
                entry.reopened.store(false, Ordering::Release);
                entry.recovering.store(false, Ordering::Release);
                if let Some(t) = hub().reader.get() {
                    t.unpark();
                }
                return;
            }
        }
        log::warn!(
            "frozen render {}: read errors, looked at again later",
            entry.source.display()
        );
        if let Ok(mut v) = IO_FAILED.lock() {
            v.push(entry.source.clone());
        }
        st.set_failed();
        entry.recovering.store(false, Ordering::Release);
        if let Ok(mut i) = hub().inner.lock() {
            i.by_file.remove(&(entry.source.clone(), entry.id));
        }
        return;
    }
    // A read that failed for a passing reason (a busy drive): the kept file, opened again,
    // is read again — the very block that failed — before anything is decoded (third
    // review #4); once per stream: an error that comes back is the disk's, and the source
    // is decoded (fourth review #5).
    let first_time = !entry.reopened.swap(true, Ordering::AcqRel);
    if let (Some(block), true) = (io, first_time) {
        if reopen(entry, block) {
            entry.recovering.store(false, Ordering::Release);
            if let Some(t) = hub().reader.get() {
                t.unpark();
            }
            return;
        }
    }
    // A frozen render whose read failed again: failed, quietly, its file kept (it is made
    // again; review 2).
    if is_frozen_render(entry) {
        log::warn!("frozen render {}: unreadable", entry.source.display());
        st.set_failed();
        entry.recovering.store(false, Ordering::Release);
        if let Ok(mut i) = hub().inner.lock() {
            i.by_file.remove(&(entry.source.clone(), entry.id));
        }
        return;
    }
    if io.is_some() {
        hub().note(LoadNote::Damaged(entry.source.clone()));
    }
    // The decode is counted against the writers' bound before it is made (#4).
    let bytes = (st.frames[0] * st.channels) as u64 * 4;
    reserve(bytes);
    let decoded = crate::samples::assets::decode_any(&entry.source).and_then(|d| {
        let same = stream_cache::identity(&entry.source) == Some(entry.id)
            && d.channels.clamp(1, 2) as usize == st.channels
            && d.len_frames() == st.frames[0]
            && d.rate as f32 == st.rate;
        if same {
            Ok(d)
        } else {
            Err("the file changed".to_string())
        }
    });
    match decoded {
        Ok(d) => {
            *entry.backing.lock().unwrap_or_else(|e| e.into_inner()) =
                Backing::Memory(d.frames.clone());
            entry.recovering.store(false, Ordering::Release);
            if let Some(t) = hub().reader.get() {
                t.unpark();
            }
            // Always written anew: the decode never stays in memory for the session.
            send_reserved(WriteJob {
                dir: hub().cache_dir(),
                source: entry.source.clone(),
                id: entry.id,
                channels: st.channels,
                rate: d.rate,
                data: d.frames.clone(),
                entry: entry.clone(),
            });
        }
        Err(e) => {
            unreserve(bytes);
            fail(entry, e);
        }
    }
}

/// The stream's cache file opened again and the block that failed read again: whether it
/// reads now (its backing then the fresh handle).
fn reopen(entry: &Arc<Entry>, block: usize) -> bool {
    let mut b = entry.backing.lock().unwrap_or_else(|e| e.into_inner());
    let Backing::File(o) = &mut *b else {
        return false;
    };
    let Some(mut fresh) = Open::new(o.c.clone()) else {
        return false;
    };
    if reader::injected_io(&fresh.c.path) {
        return false;
    }
    let ch = fresh.c.channels;
    let mut buf = vec![0.0f32; BLOCK * ch];
    let read = fresh.with_file(|c, f| c.read_blocks(f, block, 1, &mut buf).is_ok());
    if read != Some(true) {
        return false;
    }
    *o = fresh;
    true
}

/// The stream cannot be played on: its voices end, a load of the file makes a new one.
fn fail(entry: &Arc<Entry>, why: String) {
    log::warn!("stream {}: {why}", entry.source.display());
    entry.stream.set_failed();
    entry.recovering.store(false, Ordering::Release);
    let hub = hub();
    if let Ok(mut i) = hub.inner.lock() {
        i.by_file.remove(&(entry.source.clone(), entry.id));
    }
    hub.note(LoadNote::Unreadable(entry.source.clone(), why));
}

/// The file a failed stream played (`None` while it plays on): for an export's error.
pub fn failed_source(head: &SampleData) -> Option<PathBuf> {
    let st = lookup(head)?;
    if !st.failed() {
        return None;
    }
    let i = hub().inner.lock().ok()?;
    let e = i.entries.get(st.id as usize - 1)?.as_ref()?;
    Some(e.source.clone())
}

/// Whether the stream a head carries has failed (the sample cache loads its file again).
pub fn failed(head: &SampleData) -> bool {
    lookup(head).is_some_and(|s| s.failed())
}

/// A sample, loaded for Exemplar: whole (short, the cache unwritable, the file changed
/// while it was read — `note` says why when it is worth saying) or streamed.
#[derive(Debug)]
pub enum Loaded {
    Whole {
        data: SampleData,
        note: Option<LoadNote>,
    },
    Streamed(Arc<SampleData>),
}

/// Levels a file of `frames0` frames has: level *k* exists while level *k − 1* holds 32
/// frames or more (the in-memory mip-maps' rule), at most `MAX_LEVELS`.
fn levels_of(frames0: usize) -> usize {
    let mut n = 1;
    let mut f = frames0;
    while n < MAX_LEVELS && f >= 32 {
        f = f.div_ceil(2);
        n += 1;
    }
    n
}

fn level_frames(frames0: usize, levels: usize) -> [usize; MAX_LEVELS] {
    let mut t = [0usize; MAX_LEVELS];
    let mut f = frames0;
    for slot in t.iter_mut().take(levels.min(MAX_LEVELS)) {
        *slot = f;
        f = f.div_ceil(2);
    }
    t
}

/// Frames of each level its head (and the levels above) need: the level's own head, and
/// the filter's reach into it for the head of the level above. At most the level.
fn needs(frames0: usize, levels: usize) -> [usize; MAX_LEVELS] {
    let totals = level_frames(frames0, levels);
    let reach = halve_reach();
    let mut out = [0usize; MAX_LEVELS];
    let mut need = 0usize;
    for k in (0..levels).rev() {
        let head = (HEAD_FRAMES >> k).min(totals[k]);
        need = head.max(if k + 1 < levels {
            2 * need + reach + 1
        } else {
            0
        });
        need = need.min(totals[k]);
        out[k] = need;
    }
    out
}

/// Load `source` for Exemplar: from its cache file when it has one for the file as it is
/// (heads only), else decoded — and, when long enough, written to the cache and streamed.
/// The file's identity is taken before it is read and checked again after (review 7).
pub fn load(
    source: &Path,
    decode_file: &dyn Fn(&Path) -> Result<SampleData, String>,
) -> Result<Loaded, String> {
    let hub = hub();
    // A debug build (a test run) never streams into the shared fallback: the load fails,
    // naming why, so the test does too.
    let dir = hub
        .cache_dir_to_read()
        .ok_or_else(|| "no sample cache folder set (stream::hub().set_cache_dir)".to_string())?;
    let id = stream_cache::identity(source).ok_or_else(|| "the file cannot be read".to_string())?;
    if let Some(head) = existing(source, &id) {
        return Ok(Loaded::Streamed(head));
    }
    // The cache file, open and locked for as long as the stream reads it.
    if let Some(mut o) = stream_cache::open(&dir, source, &id).and_then(Open::new) {
        if o.c.frames > HEAD_FRAMES + BLOCK {
            let need = needs(o.c.frames, levels_of(o.c.frames))[0];
            let mut prefix =
                vec![0.0f32; need.next_multiple_of(BLOCK).min(o.c.frames) * o.c.channels];
            let read = o
                .with_file(|c, f| c.read_blocks(f, 0, need.div_ceil(BLOCK), &mut prefix))
                .unwrap_or(Err(stream_cache::ReadError::Damaged));
            let c = &o.c;
            match read {
                Ok(_) => {
                    let n = need * c.channels;
                    let (ch, rate, frames, whole) = (c.channels, c.rate, c.frames, c.whole);
                    let (head, entry) = make(
                        source,
                        id,
                        (ch, rate, frames),
                        Backing::File(o),
                        &prefix[..n],
                    );
                    if let Some(e) = entry {
                        verify_later(&e, whole);
                    }
                    return Ok(Loaded::Streamed(head));
                }
                Err(e) => {
                    // Damaged: decoded again below, the file written anew.
                    let path = c.path.clone();
                    log::warn!("sample cache {}: {e}", path.display());
                    drop(o);
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
    let data = decode_file(source)?;
    let ch = data.channels.clamp(1, 2) as usize;
    let frames = data.len_frames();
    if frames <= HEAD_FRAMES + BLOCK {
        return Ok(Loaded::Whole { data, note: None });
    }
    if stream_cache::identity(source).as_ref() != Some(&id) {
        // Changed while it was read: what was read is played, nothing is cached.
        return Ok(Loaded::Whole {
            data,
            note: Some(LoadNote::Changed(source.to_path_buf())),
        });
    }
    // A first load plays at once from the decoded file; its cache file is written beside
    // it, and the stream switches to the file when it is whole (the same values).
    let need = needs(frames, levels_of(frames))[0];
    let backing = Backing::Memory(data.frames.clone());
    let (head, entry) = make(
        source,
        id,
        (ch, data.rate, frames),
        backing,
        &data.frames[..need * ch],
    );
    if let Some(entry) = entry {
        write_later(WriteJob {
            dir,
            source: source.to_path_buf(),
            id,
            channels: ch,
            rate: data.rate,
            data: data.frames.clone(),
            entry,
        });
    }
    Ok(Loaded::Streamed(head))
}

/// A frozen render (round 29, ADR 0064) at `path`, a cache-format file
/// ([`stream_cache::write_at`]): streamed from it, heads only in memory — or, when short,
/// read whole. With the label it was written with (the caller checks it: a file is never
/// played for another render).
pub fn open_file(path: &Path) -> Result<(Loaded, String), String> {
    let (c, found) = stream_cache::open_at(path).ok_or_else(|| "not a render".to_string())?;
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos().min(i64::MAX as u128) as i64);
    let id = Identity {
        size: meta.len(),
        mtime,
        print: 0,
    };
    if let Some(head) = existing(path, &id) {
        return Ok((Loaded::Streamed(head), found));
    }
    let mut o = Open::new(c).ok_or_else(|| "cannot be opened".to_string())?;
    let (ch, rate, frames) = (o.c.channels, o.c.rate, o.c.frames);
    if frames <= HEAD_FRAMES + BLOCK {
        let mut all = vec![0.0f32; frames.div_ceil(BLOCK) * BLOCK * ch];
        o.with_file(|c, f| c.read_blocks(f, 0, frames.div_ceil(BLOCK), &mut all))
            .unwrap_or(Err(stream_cache::ReadError::Damaged))
            .map_err(|e| e.to_string())?;
        all.truncate(frames * ch);
        return Ok((
            Loaded::Whole {
                data: SampleData {
                    rate,
                    channels: ch as u8,
                    frames: all.into(),
                    stream: None,
                },
                note: None,
            },
            found,
        ));
    }
    let need = needs(frames, levels_of(frames))[0];
    let mut prefix = vec![0.0f32; need.next_multiple_of(BLOCK).min(frames) * ch];
    o.with_file(|c, f| c.read_blocks(f, 0, need.div_ceil(BLOCK), &mut prefix))
        .unwrap_or(Err(stream_cache::ReadError::Damaged))
        .map_err(|e| e.to_string())?;
    let (head, _) = make(
        path,
        id,
        (ch, rate, frames),
        Backing::File(o),
        &prefix[..need * ch],
    );
    Ok((Loaded::Streamed(head), found))
}

/// The whole of `source` from its cache file, when one is there for the file as it is (an
/// editor's waveform of a streamed zone: a positioned read of the decoded samples, no
/// decode). Off the audio thread and off the interface's (it reads the whole file).
pub fn read_whole(source: &Path) -> Option<SampleData> {
    let id = stream_cache::identity(source)?;
    let c = stream_cache::open(&hub().cache_dir_to_read()?, source, &id)?;
    let mut f = std::fs::File::open(&c.path).ok()?;
    let mut out = vec![0.0f32; c.frames * c.channels];
    c.read_blocks(&mut f, 0, c.frames.div_ceil(BLOCK), &mut out)
        .ok()?;
    Some(SampleData {
        rate: c.rate,
        channels: c.channels as u8,
        frames: out.into(),
        stream: None,
    })
}

fn existing(source: &Path, id: &Identity) -> Option<Arc<SampleData>> {
    let i = hub().inner.lock().ok()?;
    let h = i.by_file.get(&(source.to_path_buf(), *id))?.upgrade()?;
    // A failed stream is never handed out again (second review #3).
    (!failed(&h)).then_some(h)
}

/// The stream for a file (`(channels, rate, frames)`, read from `backing`), its heads
/// computed from level 0's first frames. Returns the head and, when it made a new stream,
/// its entry (an existing stream for the same file is handed back as it is).
fn make(
    source: &Path,
    id: Identity,
    (ch, rate, frames): (usize, u32, usize),
    backing: Backing,
    prefix0: &[f32],
) -> (Arc<SampleData>, Option<Arc<Entry>>) {
    let levels = levels_of(frames);
    let totals = level_frames(frames, levels);
    let needs = needs(frames, levels);
    // Each level's prefix from the one below, as far as the levels above need.
    let mut prefixes: Vec<Vec<f32>> = vec![prefix0.to_vec()];
    for k in 1..levels {
        let mut out = vec![0.0f32; needs[k] * ch];
        halve_span(&prefixes[k - 1], 0, totals[k - 1] as i64, ch, 0, &mut out);
        prefixes.push(out);
    }
    let heads: Vec<Arc<[f32]>> = prefixes
        .iter()
        .enumerate()
        .map(|(k, p)| Arc::from(&p[..(HEAD_FRAMES >> k).min(totals[k]) * ch]))
        .collect();
    let hub = hub();
    let pool = hub.pool().clone();
    let mut inner = match hub.inner.lock() {
        Ok(i) => i,
        Err(p) => p.into_inner(),
    };
    let key = (source.to_path_buf(), id);
    if let Some(h) = inner
        .by_file
        .get(&key)
        .and_then(Weak::upgrade)
        .filter(|h| !failed(h))
    {
        return (h, None);
    }
    inner.let_go_unheld();
    let n = inner.entries.len() as u32 + 1;
    let stream = Arc::new(Stream::new(n, ch, rate as f32, frames, heads, pool));
    let head = Arc::new(SampleData {
        rate,
        channels: ch as u8,
        frames: stream.heads[0]
            .clone()
            .unwrap_or_else(|| Arc::from(&[][..])),
        stream: Some(StreamRef(stream.clone())),
    });
    let entry = Arc::new(Entry {
        stream,
        backing: Mutex::new(backing),
        source: source.to_path_buf(),
        id,
        recovering: std::sync::atomic::AtomicBool::new(false),
        reopened: std::sync::atomic::AtomicBool::new(false),
    });
    inner.entries.push(Some(entry.clone()));
    inner.by_file.insert(key, Arc::downgrade(&head));
    drop(inner);
    hub.generation.fetch_add(1, Ordering::Release);
    hub.start_reader();
    (head, Some(entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_automatic_limit_keeps_to_16_gb_a_tenth_of_the_disk_and_5_gb_free() {
        use cantus_rt::disk::Space;
        const G: u64 = 1 << 30;
        let disk = |total: u64, free: u64| {
            Some(Space {
                total: total * G,
                free: free * G,
            })
        };
        assert_eq!(
            auto_limit(disk(2_000, 1_000), 0),
            16 * G,
            "a big disk: 16 GB"
        );
        assert_eq!(auto_limit(disk(100, 90), 0), 10 * G, "a tenth of the disk");
        assert_eq!(auto_limit(disk(128, 12), 0), 7 * G, "5 GB left free");
        assert_eq!(
            auto_limit(disk(128, 12), 2 * G),
            9 * G,
            "what it holds counts"
        );
        assert_eq!(auto_limit(disk(128, 3), 0), 0, "a full disk: nothing kept");
        assert_eq!(auto_limit(None, 0), AUTO_CACHE_CAP);
    }

    #[test]
    fn the_prefix_covers_every_heads_filter_reach() {
        let reach = halve_reach();
        let n = needs(100_000, 2);
        assert_eq!(n[1], 8_192);
        assert_eq!(
            n[0],
            2 * 8_192 + reach + 1,
            "level 1's head reaches into level 0"
        );
        assert_eq!(needs(10_000, 2)[0], 10_000, "never past the sample");
        assert_eq!(levels_of(31), 1);
        assert_eq!(levels_of(32), 2);
        assert_eq!(levels_of(1 << 20), MAX_LEVELS);
    }
}
