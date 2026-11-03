//! The thread ARA renders run on (ADR 0097): never the interface's, never the audio
//! thread. A job renders one clip's region (or the part the plug-in said changed) through
//! the clip's bound instance; a newer job for the same clip cancels it between blocks, and
//! the clip plays its last render until the new one arrives.
//!
//! **ARA's threads.** Rendering (VST3 `process`) may run on any thread, one at a time per
//! instance: the instance's audio half is behind a lock the job holds while it renders.
//! Model edits (`beginEditing`…) stay on the document thread; the plug-in reads audio from
//! this thread through the host's reader, which is thread-safe. A plug-in instance is torn
//! down on the document thread only after its job has let go of it.
//!
//! **What a render is.** Only the region's stretch of the source (with the plug-in's head
//! and tail and a margin) — never a copy of the whole recording — converted here to the
//! engine's rate, and placed by its start in the source ([`ClipRender`]).

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use cantus_ara::host::RegionFrames;
use cantus_core::model::ClipId;
use cantus_core::tempo::TempoMap;
use cantus_core::SampleData;
pub use cantus_engine::ClipRender;

use crate::backend::{Audio, AudioThread, ParamOut, TransportInfo};

/// The largest block an ARA render is processed in.
pub const RENDER_BLOCK: usize = 1024;

/// Source frames kept on each side of a region's render beyond its head and tail: what a
/// crossfade or the resampler reads past the window.
pub const MARGIN_SECONDS: f64 = 0.1;

pub type SharedAudio = Arc<Mutex<Box<dyn AudioThread>>>;

/// One render to make.
pub struct Job {
    pub clip: ClipId,
    /// The clip's render generation this job is for; a newer one cancels it.
    pub generation: u64,
    pub current: Arc<AtomicU64>,
    pub audio: SharedAudio,
    /// The source at its own rate (shared with the sample cache).
    pub source: Arc<SampleData>,
    pub frames: RegionFrames,
    /// The instance's latency in samples: its output is read that much later (pre-roll).
    pub latency: usize,
    /// The part to render again, in frames from the region's start, over `base` (the
    /// clip's last render); `None` renders the whole region.
    pub range: Option<(i64, i64)>,
    pub base: Option<ClipRender>,
    /// The project's tempo map: the transport the plug-in sees.
    pub tempo: Arc<TempoMap>,
    /// The engine's rate: the render is converted to it here.
    pub engine_rate: u32,
    /// Progress, 0..=1000.
    pub progress: Arc<AtomicU32>,
}

/// A render made (or given up), with how long it took.
pub struct Done {
    pub clip: ClipId,
    pub generation: u64,
    pub render: Option<ClipRender>,
    pub took: std::time::Duration,
}

/// The render thread and its two queues; what runs now and since when (the watchdog).
pub struct Worker {
    jobs: Option<Sender<Job>>,
    done: Receiver<Done>,
    pending: Arc<AtomicUsize>,
    running: Arc<Mutex<Option<(ClipId, Instant)>>>,
    thread: Option<JoinHandle<()>>,
}

impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

impl Worker {
    pub fn new() -> Worker {
        let (jobs, rx) = channel::<Job>();
        let (tx, done) = channel::<Done>();
        let pending = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(Mutex::new(None));
        let (p, r) = (pending.clone(), running.clone());
        let thread = std::thread::Builder::new()
            .name("cantus-ara-render".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    let start = Instant::now();
                    *r.lock().unwrap_or_else(|e| e.into_inner()) = Some((job.clip, start));
                    let render = run(&job);
                    *r.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    let done = Done {
                        clip: job.clip,
                        generation: job.generation,
                        render,
                        took: start.elapsed(),
                    };
                    // The job (and its hold on the instance) goes before it is counted done.
                    drop(job);
                    let _ = tx.send(done);
                    p.fetch_sub(1, Ordering::SeqCst);
                }
            })
            .ok();
        Worker {
            jobs: Some(jobs),
            done,
            pending,
            running,
            thread,
        }
    }

    pub fn submit(&self, job: Job) {
        self.pending.fetch_add(1, Ordering::SeqCst);
        if let Some(j) = &self.jobs {
            if j.send(job).is_err() {
                self.pending.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    /// Renders made since the last call.
    pub fn take(&self) -> Vec<Done> {
        self.done.try_iter().collect()
    }

    /// Jobs queued or running.
    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    /// The clip rendering now and for how long.
    pub fn running(&self) -> Option<(ClipId, std::time::Duration)> {
        self.running.lock().ok()?.map(|(c, at)| (c, at.elapsed()))
    }

    /// A render thread that stopped answering: its thread is left to itself (never joined)
    /// and what it holds is never freed — a hung plug-in cannot be stopped from outside.
    pub fn abandon(mut self) {
        self.jobs = None;
        if let Some(t) = self.thread.take() {
            std::mem::forget(t);
        }
        std::mem::forget(self);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(job: &Job) -> Option<ClipRender> {
    if job.current.load(Ordering::SeqCst) != job.generation {
        return None;
    }
    let mut audio = job.audio.lock().unwrap_or_else(|e| e.into_inner());
    let current = &job.current;
    let progress = &job.progress;
    render(audio.as_mut(), job, &|done| {
        progress.store((done * 1000.0) as u32, Ordering::Relaxed);
        current.load(Ordering::SeqCst) == job.generation
    })
}

/// The stretch of the source a render of `f` covers: [first, end) in source frames.
pub fn span(f: &RegionFrames, source_frames: i64) -> (i64, i64) {
    let margin = (MARGIN_SECONDS * f64::from(f.rate)).ceil() as i64;
    let first = (f.source_start - f.head - margin).clamp(0, source_frames);
    let end = (f.source_start + f.len + f.tail + margin).clamp(first, source_frames);
    (first, end)
}

/// Render a job's region with its instance's audio half: the source over [`span`], the
/// region (with the plug-in's head and tail) replaced by what the plug-in plays there.
/// A plug-in with latency is run that much longer and its first samples dropped, so the
/// render lands where the region is: an offline render needs no other delay compensation.
/// `go` hears the progress (0..=1) and says whether to go on; `None` when it said no.
pub fn render(
    audio: &mut dyn AudioThread,
    job: &Job,
    go: &dyn Fn(f64) -> bool,
) -> Option<ClipRender> {
    let f = job.frames;
    let source = &*job.source;
    let ch = source.channels.max(1) as usize;
    let frames = (source.frames.len() / ch) as i64;
    let (first, end) = span(&f, frames);
    // The buffer at the source's rate: the last render's when only a part changes (and it
    // covers the same span at the same rate), else the source.
    // A part rendered again is laid over the last render only when that render covers the
    // same stretch at the same rate; otherwise the whole region is rendered (a part over the
    // raw source would leave the rest of the clip unprocessed).
    let base = job.base.as_ref().filter(|b| {
        job.range.is_some() && b.source_frames == (first, end) && b.source_rate == source.rate
    });
    let mut buf: Vec<f32> = match base {
        Some(b) => b.at_source_rate.as_ref().to_vec(),
        None => source.frames[first as usize * ch..end as usize * ch].to_vec(),
    };
    // What to render, in frames from the region's start.
    let (a, b) = match (job.range, base) {
        (Some((a, b)), Some(_)) => (a.max(-f.head), b.min(f.len + f.tail)),
        _ => (-f.head, f.len + f.tail),
    };
    let latency = job.latency as i64;
    let total = (b - a).max(0) + latency;
    let (mut left, mut right) = (vec![0f32; RENDER_BLOCK], vec![0f32; RENDER_BLOCK]);
    let mut params = ParamOut::new();
    audio.reset();
    let rate = f64::from(f.rate.max(1));
    let ppq = cantus_core::time::PPQ as f64;
    let mut done = 0i64;
    while done < total {
        if !go(done as f64 / total.max(1) as f64) {
            return None;
        }
        let n = ((total - done) as usize).min(RENDER_BLOCK);
        left[..n].fill(0.0);
        right[..n].fill(0.0);
        let at = f.start + a + done;
        let seconds = at as f64 / rate;
        let tick = job.tempo.tick_at_seconds(seconds);
        let transport = TransportInfo {
            playing: true,
            recording: false,
            tempo_bpm: job.tempo.bpm_at(tick),
            tempo_inc: 0.0,
            position_beats: tick / ppq,
            position_samples: at,
            position_seconds: seconds,
            project_samples: at,
            bar_start_beats: 0.0,
            bar_number: 0,
            time_sig_num: 4,
            time_sig_den: 4,
            loop_beats: None,
            loop_seconds: None,
        };
        params.clear();
        audio.process(
            Audio {
                left: &mut left[..n],
                right: &mut right[..n],
                key: None,
                transport: Some(&transport),
            },
            &[],
            &[],
            &mut params,
        );
        for i in 0..n {
            // Output sample `done + i` holds what belongs `latency` samples earlier.
            let k = a + done + i as i64 - latency;
            if k < a || k >= b {
                continue;
            }
            let s = f.source_start + k;
            if s < first || s >= end {
                continue;
            }
            let at = (s - first) as usize * ch;
            buf[at] = left[i];
            if ch > 1 {
                buf[at + 1] = right[i];
            }
        }
        done += n as i64;
    }
    go(1.0);
    let at_source_rate: Arc<[f32]> = buf.into();
    let data = if source.rate == job.engine_rate || job.engine_rate == 0 {
        SampleData {
            rate: source.rate,
            channels: source.channels,
            frames: at_source_rate.clone(),
            stream: None,
        }
    } else {
        SampleData {
            rate: job.engine_rate,
            channels: source.channels,
            frames: cantus_engine::samples::resample::resample(
                &at_source_rate,
                ch,
                source.rate,
                job.engine_rate,
            )
            .into(),
            stream: None,
        }
    };
    Some(ClipRender {
        data: Arc::new(data),
        start: first as f64 / f64::from(source.rate.max(1)),
        source_frames: (first, end),
        source_rate: source.rate,
        at_source_rate,
    })
}
