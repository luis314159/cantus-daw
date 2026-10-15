//! **The pitch editor's engine side** (round 28, ADR 0054): a take's analysis into notes
//! and a key ([`analyse_take`]), the render of a clip's edits ([`render_edit`]), and the
//! service that does both off the audio thread and off the UI thread ([`PitchService`]),
//! with the analyses cached in memory and on disk (beside the waveform peaks) and the
//! renders handed to [`crate::SampleCache`], which attaches them to the snapshot in place
//! of the source — so the same snapshot serves real-time playback, the bounce and the
//! stems, and they hear exactly what the editor plays.
//!
//! **While a render is on its way** the clip plays the last render it had, or its source:
//! never silence; and [`crate::SampleCache::loading`] says so, so an export waits for it.

use cantus_core::pitch::PitchEdit;
use cantus_core::{ClipId, SampleData};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[path = "pitch_attach.rs"]
pub(crate) mod attach;
#[path = "pitch_disk.rs"]
pub mod disk;
#[path = "pitch_render.rs"]
mod render;
#[path = "pitch_take.rs"]
mod take;

pub use render::{key as render_key, render_edit};
pub use take::{
    analyse_take, detect_key, from_analysis, mixdown, FrameKind, TakeAnalysis, ANALYSER,
};

/// What the service knows about a take.
#[derive(Clone, Debug)]
pub enum Lookup {
    Ready(Arc<TakeAnalysis>),
    /// Being analysed (or waiting for its turn).
    Pending,
    /// The file could not be read (the reason).
    Failed(String),
}

#[derive(Clone, Debug)]
enum Entry {
    Pending,
    Ready(Arc<TakeAnalysis>),
    Failed(String),
}

enum Job {
    Analyse(PathBuf, u32),
    Render {
        clip: ClipId,
        key: u64,
        path: PathBuf,
        edit: Arc<PitchEdit>,
        rate: u32,
    },
}

#[derive(Default)]
struct State {
    disk: Option<PathBuf>,
    takes: HashMap<(PathBuf, u32), Entry>,
    queue: VecDeque<Job>,
    /// Renders finished on the worker, for the sample cache to collect.
    finished: Vec<(u64, Result<SampleData, String>)>,
    worker: bool,
    closed: bool,
    /// Jobs taken by the worker and not finished.
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

/// See the module docs. Used through `&self` from the UI (the editor asks every frame)
/// and from the sample cache; its state sits behind a mutex no audio thread ever takes.
#[derive(Default)]
pub struct PitchService {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for PitchService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PitchService")
    }
}

impl Drop for PitchService {
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.wake.notify_all();
    }
}

/// A source decoded at `rate`, as channels.
fn channels_of(path: &Path, rate: u32) -> Result<Vec<Vec<f32>>, String> {
    let d = crate::samples::decode_audio(path, rate)?;
    let ch = d.channels.max(1) as usize;
    Ok((0..ch)
        .map(|c| d.frames.iter().skip(c).step_by(ch).copied().collect())
        .collect())
}

impl PitchService {
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

    /// The analysis of the file at `path` at `rate`, queued on the worker the first time.
    pub fn lookup(&self, path: &Path, rate: u32) -> Lookup {
        let mut g = self.shared.lock();
        match g.takes.get(&(path.to_path_buf(), rate)) {
            Some(Entry::Ready(a)) => return Lookup::Ready(a.clone()),
            Some(Entry::Failed(e)) => return Lookup::Failed(e.clone()),
            Some(Entry::Pending) => return Lookup::Pending,
            None => {}
        }
        g.takes.insert((path.to_path_buf(), rate), Entry::Pending);
        g.queue.push_back(Job::Analyse(path.to_path_buf(), rate));
        self.ensure_worker(&mut g);
        Lookup::Pending
    }

    /// Forget what is known about `path` (the file changed): the next lookup analyses it.
    pub fn forget(&self, path: &Path) {
        self.shared.lock().takes.retain(|(p, _), _| p != path);
    }

    /// The analysis of `path` at `rate`, here and now (tests, a render on the worker).
    pub fn analyse_now(&self, path: &Path, rate: u32) -> Result<Arc<TakeAnalysis>, String> {
        analyse_on(&self.shared, path, rate)
    }

    /// Render `edit` over the file at `path`, played at `rate`, here and now.
    pub fn render_now(
        &self,
        path: &Path,
        edit: &PitchEdit,
        rate: u32,
    ) -> Result<SampleData, String> {
        render_on(&self.shared, path, edit, rate)
    }

    /// Queue a render for `clip`, in place of any render still waiting for it (a drag asks
    /// for one per frame; only the newest is made). Returns the key it replaced, if any.
    pub(crate) fn queue_render(
        &self,
        clip: ClipId,
        key: u64,
        path: PathBuf,
        edit: Arc<PitchEdit>,
        rate: u32,
    ) -> Option<u64> {
        let mut g = self.shared.lock();
        let mut replaced = None;
        g.queue.retain(|j| match j {
            Job::Render {
                clip: c, key: k, ..
            } if *c == clip => {
                replaced = Some(*k);
                false
            }
            _ => true,
        });
        // Renders before analyses: what plays comes first.
        let at = g
            .queue
            .iter()
            .position(|j| matches!(j, Job::Analyse(..)))
            .unwrap_or(g.queue.len());
        g.queue.insert(
            at,
            Job::Render {
                clip,
                key,
                path,
                edit,
                rate,
            },
        );
        self.ensure_worker(&mut g);
        replaced
    }

    /// Renders the worker finished since the last call.
    pub(crate) fn take_finished(&self) -> Vec<(u64, Result<SampleData, String>)> {
        std::mem::take(&mut self.shared.lock().finished)
    }

    /// Whether the worker has anything left to do.
    pub fn busy(&self) -> bool {
        let g = self.shared.lock();
        !g.queue.is_empty() || g.busy > 0
    }

    /// Start the worker the first time, and wake it for the job just queued (an idle
    /// worker sleeps until told).
    fn ensure_worker(&self, g: &mut State) {
        if !g.worker {
            let shared = self.shared.clone();
            let spawned = std::thread::Builder::new()
                .name("cantus-pitch".into())
                .spawn(move || work(shared));
            g.worker = spawned.is_ok();
        }
        self.shared.wake.notify_all();
    }
}

fn work(shared: Arc<Shared>) {
    loop {
        let job = {
            let mut g = shared.lock();
            loop {
                if g.closed {
                    return;
                }
                if let Some(j) = g.queue.pop_front() {
                    g.busy += 1;
                    break j;
                }
                g = shared.wake.wait(g).unwrap_or_else(|e| e.into_inner());
            }
        };
        match job {
            Job::Analyse(path, rate) => {
                let _ = analyse_on(&shared, &path, rate);
            }
            Job::Render {
                key,
                path,
                edit,
                rate,
                ..
            } => {
                let r = render_on(&shared, &path, &edit, rate);
                shared.lock().finished.push((key, r));
            }
        }
        shared.lock().busy -= 1;
    }
}

fn analyse_on(shared: &Shared, path: &Path, rate: u32) -> Result<Arc<TakeAnalysis>, String> {
    let key = (path.to_path_buf(), rate);
    let disk = {
        let g = shared.lock();
        if let Some(Entry::Ready(a)) = g.takes.get(&key) {
            return Ok(a.clone());
        }
        g.disk.clone()
    };
    let file = disk.as_deref().and_then(|d| disk::file_for(d, path, rate));
    let read = file.as_deref().and_then(disk::read);
    let result = match read {
        Some((a, frames)) if a.sample_rate == rate as f32 => {
            Ok(Arc::new(take::from_analysis(a, frames, rate)))
        }
        _ => channels_of(path, rate).map(|chs| {
            let refs: Vec<&[f32]> = chs.iter().map(Vec::as_slice).collect();
            let t = analyse_take(&refs, rate);
            shared.computed.fetch_add(1, Ordering::Relaxed);
            if let Some(f) = &file {
                if let Err(e) = disk::write(f, &t.analysis, t.frames) {
                    log::warn!("pitch cache {}: {e}", f.display());
                }
            }
            Arc::new(t)
        }),
    };
    shared.lock().takes.insert(
        key,
        match &result {
            Ok(a) => Entry::Ready(a.clone()),
            Err(e) => Entry::Failed(e.clone()),
        },
    );
    result
}

fn render_on(
    shared: &Shared,
    path: &Path,
    edit: &PitchEdit,
    rate: u32,
) -> Result<SampleData, String> {
    let take = analyse_on(shared, path, edit.rate)?;
    let chs = channels_of(path, edit.rate)?;
    let refs: Vec<&[f32]> = chs.iter().map(Vec::as_slice).collect();
    let out = render_edit(&refs, &take, edit);
    let n = out.len().max(1);
    let len = out.first().map_or(0, Vec::len);
    let mut frames = Vec::with_capacity(len * n);
    for i in 0..len {
        for ch in &out {
            frames.push(ch[i]);
        }
    }
    let frames = if rate != edit.rate && rate > 0 {
        crate::samples::resample::resample(&frames, n, edit.rate, rate)
    } else {
        frames
    };
    Ok(SampleData {
        stream: None,
        rate: if rate > 0 { rate } else { edit.rate },
        channels: n as u8,
        frames: frames.into(),
    })
}
