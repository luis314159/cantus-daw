//! Frozen renders (round 29, ADR 0064): where they live on disk, how a snapshot finds them,
//! and how a frozen strip or track reads its render on the audio thread — streamed from
//! disk on ADR 0050's streaming (a cache-format file, heads only in memory), never whole in
//! memory, never I/O on the audio thread or the pool's workers.
//!
//! **A render's key** ([`render_key`]) is the target's content key (`cantus_core::freeze`)
//! with the rate, `DSP_VERSION` and this file's [`FORMAT`]: the same sound at the same rate
//! from the same build is the same file, wherever it is asked for. Files are named by it
//! (`<key>.cfrz`) in the store's folders — the project's own (beside the project, written
//! there when the project has a folder) and the application's (written there otherwise,
//! and read from always) — so a render made for one project serves any other with the same
//! sound, and a render that is not anywhere is made again: *Collect* does not need to copy
//! them (they are regenerable).
//!
//! **Reading.** A frozen strip or track holds its render's [`FrozenHandle`]; each pass it
//! copies its frames out of the stream's head and the shared pool ([`read`]), and points
//! its board's cursor at what comes next, so the reader thread keeps up to sixteen blocks
//! ahead of the playhead (about 1.4 s at 48 kHz) — and the loop's start, in a second cell,
//! so a wrap finds its blocks. In real time a block not read yet is silence, counted
//! (`EngineStats::stream_late`); offline (a render, an export) the pass waits for it.

use cantus_core::freeze::{FreezeTarget, FrozenAudio, FrozenPlay};
use cantus_core::{Project, SampleData, Snapshot, StreamRef};
use cantus_dsp::exemplar::stream::{Board, Cursor, Stream, BLOCK, HEAD_FRAMES};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[path = "freeze_render.rs"]
pub mod render;

/// Bumped when a render's layout or what it holds changes: older files are then other
/// keys, made again.
pub const FORMAT: u32 = 1;
/// A render file's extension.
pub const EXTENSION: &str = "cfrz";
/// Blocks a frozen reader asks the reader thread to keep ready beyond the one it reads
/// (the most a cursor can ask: [`Cursor::ahead`] holds 0…15).
const AHEAD: usize = 15;
/// How long a missing render is believed missing before its folders are looked at again.
const RECHECK: Duration = Duration::from_secs(2);

/// The key a render of content `content` at `rate` is stored under: lower-case hex
/// SHA-256 of the content key, the rate, `DSP_VERSION` and [`FORMAT`].
pub fn render_key(content: &str, rate: u32) -> String {
    let text = format!("{content}|{rate}|{}|{FORMAT}", cantus_dsp::DSP_VERSION);
    cantus_core::freeze::sha256_hex(text.as_bytes())
}

/// A render's file name.
pub fn file_name(render_key: &str) -> String {
    format!("{render_key}.{EXTENSION}")
}

/// What a render file names itself by: its key and the latency its frames arrive after.
pub fn label(render_key: &str, latency: usize) -> String {
    format!("{render_key}#{latency}")
}

/// A render file's label read back: `Some(latency)` when it is `key`'s.
fn latency_in(label: &str, key: &str) -> Option<usize> {
    let (k, l) = label.split_once('#')?;
    (k == key).then(|| l.parse().ok()).flatten()
}

/// The engine's handle on an opened render: its head (which keeps the stream alive in the
/// hub), its stream when it streams (a short render is whole in `head`), and its board.
pub struct FrozenHandle {
    pub head: Arc<SampleData>,
    pub stream: Option<Arc<Stream>>,
    /// Cell 0: where the playhead reads; cell 1: the loop's start.
    pub board: Arc<Board>,
    pub frames: usize,
    /// The latency its frames arrive after: what the frozen strip or track reports as its
    /// own (its chain's, kept, so the mix is the live mix to the bit).
    pub latency: usize,
}

impl std::fmt::Debug for FrozenHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenHandle")
            .field("frames", &self.frames)
            .field("streams", &self.stream.is_some())
            .finish()
    }
}

impl Drop for FrozenHandle {
    fn drop(&mut self) {
        // The reader lets go of its copy of the board at its next look.
        crate::inst::stream::hub().boards_changed();
    }
}

/// Open the render at `path` made for `key`.
pub fn open(path: &Path, key: &str) -> Result<Arc<FrozenAudio>, String> {
    use crate::inst::stream::{self, Loaded};
    let (loaded, found) = stream::open_file(path)?;
    let latency = latency_in(&found, key).ok_or("another render")?;
    let head = match loaded {
        Loaded::Streamed(h) => h,
        Loaded::Whole { data, .. } => Arc::new(data),
    };
    let st = stream::lookup(&head);
    let frames = match &st {
        Some(s) => s.frames[0],
        None => head.len_frames(),
    };
    let board = Arc::new(Board::new(2));
    if st.is_some() {
        stream::hub().register_board(&board);
    }
    let handle = FrozenHandle {
        head: head.clone(),
        stream: st,
        board,
        frames,
        latency,
    };
    Ok(Arc::new(FrozenAudio {
        key: key.to_string(),
        rate: head.rate,
        frames: frames as u64,
        reader: StreamRef(Arc::new(handle)),
    }))
}

/// The same render with a board of its own (review 1): an offline render reads it with its
/// own cursors, never moving the live processor's (they share the stream and its head).
fn with_own_board(a: &Arc<FrozenAudio>) -> Arc<FrozenAudio> {
    let Ok(h) = a.reader.0.clone().downcast::<FrozenHandle>() else {
        return a.clone();
    };
    let board = Arc::new(Board::new(2));
    if h.stream.is_some() {
        crate::inst::stream::hub().register_board(&board);
    }
    Arc::new(FrozenAudio {
        key: a.key.clone(),
        rate: a.rate,
        frames: a.frames,
        reader: StreamRef(Arc::new(FrozenHandle {
            head: h.head.clone(),
            stream: h.stream.clone(),
            board,
            frames: h.frames,
            latency: h.latency,
        })),
    })
}

/// Whether an opened render can no longer be read (its file damaged or gone).
fn failed(a: &FrozenAudio) -> bool {
    a.reader
        .0
        .clone()
        .downcast::<FrozenHandle>()
        .ok()
        .and_then(|h| h.stream.as_ref().map(|s| s.failed()))
        .unwrap_or(false)
}

/// The engine's handle of a snapshot's frozen render (an `Arc` clone: allocation-free).
pub(crate) fn handle_of(play: Option<&FrozenPlay>) -> Option<Arc<FrozenHandle>> {
    let a = play?.audio.as_ref()?;
    a.reader.0.clone().downcast::<FrozenHandle>().ok()
}

/// Where renders are looked for and written, and the ones opened.
#[derive(Debug, Default)]
pub struct FrozenStore {
    /// The project's own folder (written to when set), then the application's.
    project_dir: Option<PathBuf>,
    app_dir: Option<PathBuf>,
    /// By render key: opened, or known missing since then.
    open: HashMap<String, Result<Arc<FrozenAudio>, Instant>>,
    /// Renders found damaged, by key, with their file's time then: never opened again
    /// while the file is that one (a render made again replaces it).
    damaged: HashMap<String, Option<std::time::SystemTime>>,
}

/// A file's modification time.
fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl FrozenStore {
    /// The application's folder (read from always; written to when the project has none).
    pub fn set_app_dir(&mut self, dir: PathBuf) {
        if self.app_dir.as_ref() != Some(&dir) {
            self.app_dir = Some(dir);
            self.forget_missing();
        }
    }

    /// The project's own folder (`None`: a project never saved).
    pub fn set_project_dir(&mut self, dir: Option<PathBuf>) {
        if self.project_dir != dir {
            self.project_dir = dir;
            self.forget_missing();
        }
    }

    pub fn app_dir(&self) -> Option<&Path> {
        self.app_dir.as_deref()
    }

    pub fn project_dir(&self) -> Option<&Path> {
        self.project_dir.as_deref()
    }

    /// Where a new render goes: the project's folder, else the application's.
    pub fn write_dir(&self) -> Option<&Path> {
        self.project_dir.as_deref().or(self.app_dir.as_deref())
    }

    /// Look again at once for the renders believed missing (a render was just made).
    pub fn forget_missing(&mut self) {
        self.open.retain(|_, v| v.is_ok());
    }

    /// Where the render of `content` at `rate` is, if anywhere.
    pub fn path_of(&self, content: &str, rate: u32) -> Option<PathBuf> {
        let name = file_name(&render_key(content, rate));
        [self.project_dir.as_deref(), self.app_dir.as_deref()]
            .into_iter()
            .flatten()
            .map(|d| d.join(&name))
            .find(|p| p.is_file())
    }

    /// The render of `content` at `rate`, opened (once), or `None` when it is not there
    /// (looked for again after a moment, or after [`Self::forget_missing`]).
    pub fn find(&mut self, content: &str, rate: u32) -> Option<Arc<FrozenAudio>> {
        let key = render_key(content, rate);
        match self.open.get(&key) {
            // A render whose stream failed is missing: made again (review 1).
            Some(Ok(a)) if failed(a) => {
                log::warn!("frozen render {key}: unreadable, made again");
                let at = self.path_of(content, rate);
                let time = at.as_deref().and_then(mtime);
                if let Some(p) = at {
                    let _ = std::fs::remove_file(p);
                }
                self.damaged.insert(key.clone(), time);
                self.open.insert(key, Err(Instant::now()));
                return None;
            }
            Some(Ok(a)) => return Some(a.clone()),
            Some(Err(at)) if at.elapsed() < RECHECK => return None,
            _ => {}
        }
        let damaged = self.damaged.get(&key).copied();
        let found = self
            .path_of(content, rate)
            .filter(|p| damaged.is_none_or(|t| mtime(p) != t))
            .and_then(|p| match open(&p, &key) {
                Ok(a) => Some(a),
                Err(e) => {
                    log::warn!("frozen render {}: {e}", p.display());
                    None
                }
            });
        match &found {
            Some(a) => self.open.insert(key, Ok(a.clone())),
            None => self.open.insert(key, Err(Instant::now())),
        };
        found
    }

    /// Whether `target`'s render is at hand at `rate`.
    pub fn plays(&mut self, project: &Project, target: FreezeTarget, rate: u32) -> bool {
        if project.pattern_mode {
            return false;
        }
        match project.frozen(target) {
            Some(f) => self.find(&f.content, rate).is_some(),
            None => false,
        }
    }

    /// What rests at `rate` (`cantus_core::freeze::dormant`): for the host session, which
    /// unloads the hosted plug-ins that do not run.
    pub fn dormant(&mut self, project: &Project, rate: u32) -> cantus_core::freeze::Dormant {
        let mut plays = HashMap::new();
        for t in project.frozen_targets() {
            plays.insert(t, self.plays(project, t, rate));
        }
        cantus_core::freeze::dormant(project, &|t| plays.get(&t).copied().unwrap_or(false))
    }

    /// Attach the renders at hand to `snapshot`'s frozen strips and tracks and settle it
    /// (`cantus_core::freeze::settle`): what plays frozen loses its chain, what rests is
    /// marked. A live snapshot shares the store's handles, and the renders the project no
    /// longer uses are let go (review 1: kept for the session, their blocks wanted and
    /// their files open); an offline one (`own_boards`) reads with boards of its own.
    pub fn attach(
        &mut self,
        project: &Project,
        snapshot: &mut Snapshot,
        rate: u32,
        own_boards: bool,
    ) {
        let mut any = false;
        let mut used: Vec<String> = Vec::new();
        let frozen = snapshot
            .strips
            .iter_mut()
            .filter_map(|s| s.frozen.as_mut())
            .chain(snapshot.tracks.iter_mut().filter_map(|t| t.frozen.as_mut()));
        for f in frozen {
            used.push(render_key(&f.content, rate));
            f.audio =
                self.find(&f.content, rate).map(
                    |a| {
                        if own_boards {
                            with_own_board(&a)
                        } else {
                            a
                        }
                    },
                );
            any |= f.audio.is_some();
        }
        if !own_boards {
            // The engine still holds what it plays through its snapshot, which goes back
            // to this thread to be freed: never the last owner on the audio thread.
            self.open
                .retain(|k, v| v.is_err() || used.iter().any(|u| u == k));
        }
        if any {
            cantus_core::freeze::settle(project, snapshot);
        }
    }

    /// Let every opened render go (nothing is frozen any more); the snapshots that still
    /// hold one free it on this thread.
    pub fn let_go_all(&mut self) {
        self.open.retain(|_, v| v.is_err());
    }

    /// How many renders are open (tests).
    pub fn open_count(&self) -> usize {
        self.open.values().filter(|v| v.is_ok()).count()
    }

    /// Every render file in the store's folders (for Settings' clean-up).
    pub fn files(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for d in [self.project_dir.as_deref(), self.app_dir.as_deref()]
            .into_iter()
            .flatten()
        {
            if let Ok(rd) = std::fs::read_dir(d) {
                out.extend(
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension().is_some_and(|x| x == EXTENSION)),
                );
            }
        }
        out
    }
}

/// The render frame a transport's pass starts on: its sample less where tick 0 falls —
/// the render plays as an unwarped audio clip at tick 0 would (frozen, the tempo map is
/// locked).
#[inline]
pub(crate) fn frame_at(transport: &crate::transport::Transport) -> i64 {
    transport.samples() as i64
        - transport.clip_zero(cantus_core::Ticks::ZERO, cantus_core::Ticks::ZERO, 0.0)
}

/// The render frame of tick `tick` (the loop's start), through the map.
pub(crate) fn frame_of_tick(
    map: &cantus_core::TempoMap,
    tick: cantus_core::Ticks,
    rate: f64,
) -> i64 {
    (map.seconds_at(tick.0 as f64) * rate).round() as i64
}

/// A frozen reader's place: the block its cursor points at (to wake the reader thread when
/// it moves) and the loop's start frame.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct ReadPlace {
    pub block: Option<usize>,
    pub loop_frame: Option<i64>,
}

/// Point `h`'s board at frame `at` (the next to read) and the loop's start: the reader
/// thread keeps those blocks ready. Allocation- and lock-free; wakes the reader when the
/// block moved.
#[inline]
pub(crate) fn point(h: &FrozenHandle, place: &mut ReadPlace, at: i64) {
    let Some(st) = h.stream.as_ref() else {
        return;
    };
    let block = (at.max(0) as usize / BLOCK).min(st.blocks(0).saturating_sub(1));
    let cursor = |block: usize| {
        Cursor {
            stream: st.id,
            level: 0,
            block,
            near_up: false,
            near_down: false,
            ahead: AHEAD,
        }
        .pack()
    };
    h.board.set(0, cursor(block));
    match place.loop_frame {
        Some(f) if f >= 0 && (f as usize) < h.frames => h.board.set(1, cursor(f as usize / BLOCK)),
        _ => h.board.set(1, 0),
    }
    if place.block != Some(block) {
        place.block = Some(block);
        st.pool.wake();
    }
}

/// Point `h`'s playhead cell at frame `at` only (an offline read waiting for a block: what
/// it waits for is what it asks for). Allocation- and lock-free.
#[inline]
fn point_at(h: &FrozenHandle, st: &Stream, at: usize) {
    let block = (at / BLOCK).min(st.blocks(0).saturating_sub(1));
    h.board.set(
        0,
        Cursor {
            stream: st.id,
            level: 0,
            block,
            near_up: false,
            near_down: false,
            ahead: AHEAD,
        }
        .pack(),
    );
}

/// A render no longer read by a node (unfrozen, another render, the node gone): its
/// cursors cleared, so the reader stops keeping its blocks (review 1). Atomic stores only:
/// on the audio thread.
#[inline]
pub(crate) fn release(h: &FrozenHandle) {
    h.board.set(0, 0);
    h.board.set(1, 0);
}

/// Frames `from..from + l.len()` of the render into `l`/`r` (silence outside it), `inter`
/// the interleaved scratch (two per frame). Returns the frames that were not there in time
/// (silence in their place): in real time, blocks the reader has not read yet; `offline`,
/// only blocks that never came within the pool's patience.
pub(crate) fn read(
    h: &FrozenHandle,
    from: i64,
    l: &mut [f32],
    r: &mut [f32],
    inter: &mut [f32],
    offline: bool,
) -> usize {
    let n = l.len().min(r.len());
    l[..n].iter_mut().for_each(|x| *x = 0.0);
    r[..n].iter_mut().for_each(|x| *x = 0.0);
    let a = from.max(0);
    let b = (from + n as i64).min(h.frames as i64);
    if a >= b {
        return 0;
    }
    let ch = (h.head.channels as usize).clamp(1, 2);
    let head = &h.head.frames;
    let head_frames = head.len() / ch;
    let mut at = a as usize;
    let end = b as usize;
    let put = |at: usize, src: &[f32], l: &mut [f32], r: &mut [f32]| {
        for (k, f) in src.chunks_exact(ch).enumerate() {
            let i = (at as i64 - from) as usize + k;
            l[i] = f[0];
            r[i] = f[ch - 1];
        }
    };
    // What the head holds (all of a render read whole).
    if at < head_frames {
        let to = end.min(head_frames);
        put(at, &head[at * ch..to * ch], l, r);
        at = to;
    }
    let Some(st) = h.stream.as_ref() else {
        return 0;
    };
    let mut waited: Option<Instant> = None;
    while at < end {
        let want = (end - at).min(inter.len() / ch).min(HEAD_FRAMES.max(BLOCK));
        let got = st.copy(0, at, &mut inter[..want * ch]);
        if got > 0 {
            put(at, &inter[..got * ch], l, r);
            at += got;
            continue;
        }
        if !offline || st.failed() {
            break;
        }
        // Offline: wait for the reader, as a streamed voice does (ADR 0050 §6), asking for
        // the very block waited for.
        let since = *waited.get_or_insert_with(Instant::now);
        if since.elapsed() > st.pool.patience() {
            break;
        }
        point_at(h, st, at);
        st.pool.wake();
        st.pool.idle();
    }
    end - at
}
