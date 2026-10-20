//! **Warping's engine side** (round 29, ADR 0061): the analysis of an audio file at import —
//! its transients, tempo, beat grid, downbeat, whether it is a loop, and the WAV `acid`
//! chunk's tempo — cached in memory and on disk; and the renders of warped clips, made off
//! the audio thread and off the interface thread and handed to [`crate::SampleCache`],
//! which attaches them in the source's place (after the pitch editor's render: the chain
//! is source → pitch → warp), so the same snapshot serves playback, the bounce, the stems
//! and freezing.
//!
//! A **sibling** of `crate::pitch::PitchService` (not one generalised service): the two
//! keep their own worker and disk cache, and the sample cache asks both — `loading()` and
//! [`crate::SampleCache::wait_renders`] count both, so an export waits for every render.
//!
//! **Two workers.** The final renders (the clip's mode, at full quality) are made on
//! `cantus-warp`; a **bridge** — the same map read at varispeed with linear interpolation,
//! a few milliseconds for a clip's window — on `cantus-warp-bridge`, so while a marker or
//! the tempo is dragged the clip plays at the right speed at once and the full render
//! takes its place when it is ready (ADR 0061 §6).

use cantus_core::{ClipId, SampleData};
use cantus_dsp::beat::Grid;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[path = "warp_attach.rs"]
pub(crate) mod attach;
#[path = "warp_disk.rs"]
pub mod disk;
#[path = "warp_render.rs"]
pub mod render;

pub use render::{bridge, key as render_key, pitch_of, render as render_clip, Plan};

/// The analyser's version: a better one analyses every file again (the disk cache keys
/// on it).
pub const ANALYSER: u32 = 1;

/// What the analysis finds in a file (at the file's own rate).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    pub rate: u32,
    pub frames: u64,
    /// The transients, source frames, in order.
    pub onsets: Vec<f64>,
    pub strengths: Vec<f32>,
    /// The tempo, the beats, the downbeat, a loop's bars.
    pub grid: Grid,
    /// The tempo a WAV's `acid` chunk states, when it has one.
    pub acid_bpm: Option<f64>,
}

impl Analysis {
    /// The tempo to warp by: the `acid` chunk's when the file has one, else the detected.
    pub fn bpm(&self) -> f64 {
        self.acid_bpm.unwrap_or(self.grid.bpm)
    }
}

/// Analyse one file's channels (at `rate`): a mono mixdown's onsets and grid in 4/4; with
/// an `acid` tempo, the grid's tempo is the chunk's (and a loop's bars counted by it).
pub fn analyse_channels(channels: &[&[f32]], rate: u32, acid_bpm: Option<f64>) -> Analysis {
    let mono = crate::pitch::mixdown(channels);
    let onsets = cantus_dsp::onset::detect(&mono, rate as f32);
    let mut grid = cantus_dsp::beat::analyse(&onsets, rate as f32, mono.len(), 4);
    if let Some(bpm) = acid_bpm.filter(|b| b.is_finite() && *b > 0.0) {
        grid.bpm = bpm;
        let bars = mono.len() as f64 / rate.max(1) as f64 * bpm / 240.0;
        grid.bars = ((bars - bars.round()).abs() < 0.02 && bars.round() >= 1.0)
            .then_some(bars.round() as u32);
        grid.confidence = 1.0;
    }
    Analysis {
        rate,
        frames: mono.len() as u64,
        onsets: onsets.frames,
        strengths: onsets.strengths,
        grid,
        acid_bpm,
    }
}

/// The tempo a WAV file's `acid` chunk (ACIDized loops) states, if it has one.
pub fn read_acid(path: &Path) -> Option<f64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return None;
    }
    loop {
        let mut ch = [0u8; 8];
        f.read_exact(&mut ch).ok()?;
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        if &ch[0..4] == b"acid" && size >= 24 {
            let mut body = [0u8; 24];
            f.read_exact(&mut body).ok()?;
            let bpm = f32::from_le_bytes([body[20], body[21], body[22], body[23]]) as f64;
            return (bpm.is_finite() && (20.0..=999.0).contains(&bpm)).then_some(bpm);
        }
        // Chunks are word-aligned.
        f.seek(SeekFrom::Current((size + (size & 1)) as i64)).ok()?;
    }
}

/// What the service knows about a file.
#[derive(Clone, Debug)]
pub enum Lookup {
    Ready(Arc<Analysis>),
    Pending,
    Failed(String),
}

#[derive(Clone, Debug)]
enum Entry {
    Pending,
    Ready(Arc<Analysis>),
    Failed(String),
}

/// One render asked of a worker.
#[derive(Clone)]
pub(crate) struct Job {
    pub clip: ClipId,
    /// The piece of the clip it plays (its start on the timeline: a loop's repetitions
    /// are pieces of one clip).
    pub piece: cantus_core::Ticks,
    pub key: u64,
    pub input: Arc<SampleData>,
    pub plan: Arc<Plan>,
}

enum Work {
    Analyse(PathBuf, u32),
    Render(Job),
}

/// A render the workers finished.
pub(crate) struct Finished {
    pub clip: ClipId,
    pub piece: cantus_core::Ticks,
    pub key: u64,
    pub bridge: bool,
    pub result: Result<SampleData, String>,
}

/// How many inputs' pitch analyses (Melodic renders) are kept.
const PITCH_KEPT: usize = 8;

#[derive(Default)]
struct State {
    disk: Option<PathBuf>,
    files: HashMap<(PathBuf, u32), Entry>,
    queue: VecDeque<Work>,
    bridges: VecDeque<Job>,
    finished: Vec<Finished>,
    /// The pitch base's analysis of each input a Melodic render read, newest last.
    pitch: Vec<(u64, Arc<cantus_dsp::pitch::Analysis>)>,
    /// Final renders held back (tests: what plays while a final is on its way).
    hold: bool,
    worker: bool,
    bridger: bool,
    closed: bool,
    busy: usize,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    computed: AtomicUsize,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// See the module docs. Used through `&self` from the interface (the import asks, the
/// editor draws the transients) and the sample cache; its state sits behind a mutex no
/// audio thread ever takes.
#[derive(Default)]
pub struct WarpService {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for WarpService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WarpService")
    }
}

impl Drop for WarpService {
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.wake.notify_all();
    }
}

impl WarpService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep analyses in `dir` (`None`: memory only).
    pub fn set_disk_cache(&self, dir: Option<PathBuf>) {
        self.shared.lock().disk = dir;
    }

    pub fn disk_cache(&self) -> Option<PathBuf> {
        self.shared.lock().disk.clone()
    }

    /// How many analyses this service computed (not read from the disk).
    pub fn computed(&self) -> usize {
        self.shared.computed.load(Ordering::Relaxed)
    }

    /// The analysis of the file at `path` (its own `rate`), queued the first time.
    pub fn lookup(&self, path: &Path, rate: u32) -> Lookup {
        let mut g = self.shared.lock();
        match g.files.get(&(path.to_path_buf(), rate)) {
            Some(Entry::Ready(a)) => return Lookup::Ready(a.clone()),
            Some(Entry::Failed(e)) => return Lookup::Failed(e.clone()),
            Some(Entry::Pending) => return Lookup::Pending,
            None => {}
        }
        g.files.insert((path.to_path_buf(), rate), Entry::Pending);
        g.queue.push_back(Work::Analyse(path.to_path_buf(), rate));
        self.ensure_workers(&mut g);
        Lookup::Pending
    }

    /// The analysis if it is ready, without asking for one.
    pub fn ready(&self, path: &Path, rate: u32) -> Option<Arc<Analysis>> {
        match self.shared.lock().files.get(&(path.to_path_buf(), rate)) {
            Some(Entry::Ready(a)) => Some(a.clone()),
            _ => None,
        }
    }

    /// Forget what is known about `path` (the file changed).
    pub fn forget(&self, path: &Path) {
        self.shared.lock().files.retain(|(p, _), _| p != path);
    }

    /// The analysis of `path` at `rate`, here and now (tests, the render on the worker).
    pub fn analyse_now(&self, path: &Path, rate: u32) -> Result<Arc<Analysis>, String> {
        analyse_on(&self.shared, path, rate)
    }

    /// Render `job` here and now.
    pub(crate) fn render_now(&self, job: &Job) -> Result<SampleData, String> {
        render_on(&self.shared, job)
    }

    /// Queue the final render of `job` and its bridge (when `bridge`). Nothing else is
    /// cancelled here: a loop's pieces may each want their own render (round 29's review);
    /// [`Self::retain_queued`] drops what nothing plays any more.
    pub(crate) fn queue_render(&self, job: Job, bridge: bool) {
        let mut g = self.shared.lock();
        if bridge && !g.bridges.iter().any(|j| j.key == job.key) {
            g.bridges.push_back(job.clone());
        }
        // Renders before analyses: what plays comes first.
        let at = g
            .queue
            .iter()
            .position(|w| matches!(w, Work::Analyse(..)))
            .unwrap_or(g.queue.len());
        g.queue.insert(at, Work::Render(job));
        self.ensure_workers(&mut g);
    }

    /// Drop the waiting finals and bridges whose key is not in `used`; returns the finals'
    /// keys dropped. A render already being made is left to finish (it is kept if it comes
    /// back in time to be played again).
    pub(crate) fn retain_queued(&self, used: &std::collections::HashSet<u64>) -> Vec<u64> {
        let mut g = self.shared.lock();
        let mut dropped = Vec::new();
        g.queue.retain(|w| match w {
            Work::Render(j) if !used.contains(&j.key) => {
                dropped.push(j.key);
                false
            }
            _ => true,
        });
        g.bridges.retain(|j| used.contains(&j.key));
        dropped
    }

    /// Hold final renders back (on) or let them be made (off) — for tests of what plays
    /// while a final is on its way. Bridges and analyses go on.
    #[doc(hidden)]
    pub fn hold_finals(&self, on: bool) {
        let mut g = self.shared.lock();
        g.hold = on;
        drop(g);
        self.shared.wake.notify_all();
    }

    pub(crate) fn take_finished(&self) -> Vec<Finished> {
        std::mem::take(&mut self.shared.lock().finished)
    }

    /// Whether either worker has anything left to do.
    pub fn busy(&self) -> bool {
        let g = self.shared.lock();
        !g.queue.is_empty() || !g.bridges.is_empty() || g.busy > 0
    }

    fn ensure_workers(&self, g: &mut State) {
        if !g.worker {
            let shared = self.shared.clone();
            g.worker = std::thread::Builder::new()
                .name("cantus-warp".into())
                .spawn(move || work(shared, false))
                .is_ok();
        }
        if !g.bridger {
            let shared = self.shared.clone();
            g.bridger = std::thread::Builder::new()
                .name("cantus-warp-bridge".into())
                .spawn(move || work(shared, true))
                .is_ok();
        }
        self.shared.wake.notify_all();
    }
}

fn work(shared: Arc<Shared>, bridges: bool) {
    loop {
        let job = {
            let mut g = shared.lock();
            loop {
                if g.closed {
                    return;
                }
                let next = if bridges {
                    g.bridges.pop_front().map(Work::Render)
                } else {
                    let hold = g.hold;
                    g.queue
                        .iter()
                        .position(|w| !hold || matches!(w, Work::Analyse(..)))
                        .and_then(|i| g.queue.remove(i))
                };
                if let Some(j) = next {
                    g.busy += 1;
                    break j;
                }
                g = shared.wake.wait(g).unwrap_or_else(|e| e.into_inner());
            }
        };
        match job {
            Work::Analyse(path, rate) => {
                let _ = analyse_on(&shared, &path, rate);
            }
            Work::Render(job) => {
                let r = if bridges {
                    Ok(bridge(&job.input, &job.plan))
                } else {
                    render_on(&shared, &job)
                };
                shared.lock().finished.push(Finished {
                    clip: job.clip,
                    piece: job.piece,
                    key: job.key,
                    bridge: bridges,
                    result: r,
                });
            }
        }
        shared.lock().busy -= 1;
        shared.wake.notify_all();
    }
}

fn analyse_on(shared: &Shared, path: &Path, rate: u32) -> Result<Arc<Analysis>, String> {
    let key = (path.to_path_buf(), rate);
    let dir = {
        let g = shared.lock();
        if let Some(Entry::Ready(a)) = g.files.get(&key) {
            return Ok(a.clone());
        }
        g.disk.clone()
    };
    let file = dir.as_deref().and_then(|d| disk::file_for(d, path, rate));
    let result = match file
        .as_deref()
        .and_then(disk::read)
        .filter(|a| a.rate == rate)
    {
        Some(a) => Ok(Arc::new(a)),
        None => crate::samples::decode_audio(path, rate).map(|d| {
            let ch = d.channels.max(1) as usize;
            let chans: Vec<Vec<f32>> = (0..ch)
                .map(|c| d.frames.iter().skip(c).step_by(ch).copied().collect())
                .collect();
            let refs: Vec<&[f32]> = chans.iter().map(Vec::as_slice).collect();
            let a = analyse_channels(&refs, rate, read_acid(path));
            shared.computed.fetch_add(1, Ordering::Relaxed);
            if let Some(f) = &file {
                if let Err(e) = disk::write(f, &a) {
                    log::warn!("warp cache {}: {e}", f.display());
                }
            }
            Arc::new(a)
        }),
    };
    shared.lock().files.insert(
        key,
        match &result {
            Ok(a) => Entry::Ready(a.clone()),
            Err(e) => Entry::Failed(e.clone()),
        },
    );
    result
}

fn render_on(shared: &Shared, job: &Job) -> Result<SampleData, String> {
    let plan = &job.plan;
    // The transients (Rhythm slices, Complex holds) come from the file's analysis.
    let analysis = if plan.path.as_os_str().is_empty() {
        None
    } else {
        analyse_on(shared, &plan.path, plan.source_rate).ok()
    };
    // Melodic reads the pitch base's analysis of the whole input: made once per input and
    // kept, never again for every window and every edit (round 29's review).
    let pitch =
        (plan.warp.mode == cantus_core::warp::WarpMode::Melodic).then(|| pitch_on(shared, job));
    Ok(render_clip(
        &job.input,
        plan,
        analysis.as_deref(),
        pitch.as_deref(),
    ))
}

/// The pitch analysis of a job's input, kept per input (its file, what it is, the rate).
fn pitch_on(shared: &Shared, job: &Job) -> Arc<cantus_dsp::pitch::Analysis> {
    let plan = &job.plan;
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in plan
        .path
        .to_string_lossy()
        .as_bytes()
        .iter()
        .chain(&plan.input_key.to_le_bytes())
        .chain(&plan.rate.to_le_bytes())
        .chain(&(job.input.frames.len() as u64).to_le_bytes())
    {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    if let Some((_, a)) = shared.lock().pitch.iter().find(|(k, _)| *k == h) {
        return a.clone();
    }
    let a = Arc::new(pitch_of(&job.input, plan.rate));
    let mut g = shared.lock();
    g.pitch.retain(|(k, _)| *k != h);
    g.pitch.push((h, a.clone()));
    if g.pitch.len() > PITCH_KEPT {
        g.pitch.remove(0);
    }
    a
}
