//! The recording mode's baked master (round 36, ADR 0092 §3): the whole project — but the
//! strips and tracks that stay live — as one stereo render, played by the master stage in
//! place of its chain and fader, so the engine reads one file and adds the live strips to
//! it.
//!
//! **The render** is the master's output as an export makes it (compensated: frame *f* is
//! what is heard at frame *f*), with the live roots muted, made from the project with its
//! strips frozen (so it reads their renders: quick). **Its key** is SHA-256 of the project
//! so muted (every byte of it: anything that could change the master's sound changes the
//! key; a rename too, which only renders it again). **Playing it**, every node a freeze
//! holds rests and the master's chain is taken out of the snapshot; the live strips are
//! gathered into the master as ever, given the master's (locked) fader, pan and polarity,
//! and the render is added after them. With nothing live and the controls untouched, the
//! output is the live master's to the bit (its latency is in the render: what plays has
//! none to report).

use crate::freeze::{FrozenHandle, ReadPlace};
use crate::messages::MAX_BLOCK_FRAMES;
use cantus_core::freeze::FreezeTarget;
use cantus_core::Project;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

pub use cantus_core::record_mode::{master_key, muted, MASTER_VERSION};

/// A master render prepared on the interface's thread, run on a worker.
pub struct MasterJob {
    pub key: String,
    snapshot: Arc<cantus_core::Snapshot>,
    path: Option<PathBuf>,
    rate: u32,
}

impl std::fmt::Debug for MasterJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MasterJob").field("key", &self.key).finish()
    }
}

/// Prepare the master render of `project` (its strips frozen, their renders at hand) with
/// `live` muted, into the store's folder.
pub fn prepare_master(
    h: &mut crate::EngineHandle,
    project: &Project,
    live: &[FreezeTarget],
) -> MasterJob {
    let rate = h.sample_rate.round() as u32;
    let key = master_key(project, live);
    let p = muted(project, live);
    // Rendered as the master plays live: never from an older master render.
    let was = h.frozen.master().map(str::to_string);
    h.frozen.set_master(None);
    let snapshot = h.compile_for_render(&p, 1);
    h.frozen.set_master(was);
    let path = h.frozen.write_dir().map(|d| {
        d.join(crate::freeze::file_name(&crate::freeze::render_key(
            &key, rate,
        )))
    });
    MasterJob {
        key,
        snapshot,
        path,
        rate,
    }
}

impl MasterJob {
    /// Whether its render is on disk already (nothing to do).
    pub fn done(&self) -> bool {
        self.path.as_ref().is_some_and(|p| p.exists())
    }

    /// Render and write it. Its path.
    pub fn run(self, progress: &crate::RenderProgress) -> Result<PathBuf, String> {
        self.run_with(Vec::new(), Vec::new(), progress)
    }

    /// The snapshot it renders (its hosted plug-ins' render instances are made for it).
    pub fn snapshot(&self) -> &cantus_core::Snapshot {
        &self.snapshot
    }

    /// Render and write it with the hosted plug-ins' render instances — the master's and
    /// the live strips' inserts, the instruments (made from the states the project
    /// stores, as a freeze's are: round 36, the integrated review — without them a hosted
    /// limiter on the master vanished from the render).
    pub fn run_with(
        self,
        units: Vec<crate::EffectUnit>,
        hosted: Vec<crate::inst::NativeUnit>,
        progress: &crate::RenderProgress,
    ) -> Result<PathBuf, String> {
        let path = self.path.clone().ok_or("no folder for renders")?;
        if self.done() {
            return Ok(path);
        }
        let out = crate::render_with_progress(
            self.snapshot,
            &crate::RenderOptions {
                block_frames: 1024,
                tail_seconds: 30.0,
                compensate_latency: true,
                ..Default::default()
            },
            units,
            hosted,
            progress,
        )
        .map_err(|e| e.to_string())?;
        if progress.cancel.load(Relaxed) {
            return Err("cancelled".into());
        }
        // The tail that turned out silent goes.
        let mut frames = out.samples.len() / 2;
        while frames > 0 && out.samples[frames * 2 - 1] == 0.0 && out.samples[frames * 2 - 2] == 0.0
        {
            frames -= 1;
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let raw = path.with_extension("raw");
        {
            use std::io::Write as _;
            let mut f =
                std::io::BufWriter::new(std::fs::File::create(&raw).map_err(|e| e.to_string())?);
            for x in &out.samples[..frames * 2] {
                f.write_all(&x.to_le_bytes()).map_err(|e| e.to_string())?;
            }
            f.flush().map_err(|e| e.to_string())?;
        }
        let peak = out.samples[..frames * 2]
            .iter()
            .fold(0f32, |m, x| m.max(x.abs()));
        let tapped = crate::freeze::render::Tapped {
            raw,
            frames,
            sample_rate: self.rate,
            latency: 0,
            peak,
            cut_notes: 0,
        };
        let key = crate::freeze::render_key(&self.key, self.rate);
        crate::freeze::render::write_render(&tapped, &path, &key, progress)
            .map_err(|e| e.to_string())?;
        Ok(path)
    }
}

/// The master stage's reader of a baked master (on the processor).
#[derive(Debug, Clone)]
pub(crate) struct MasterBake {
    h: Option<Arc<FrozenHandle>>,
    place: ReadPlace,
    l: Vec<f32>,
    r: Vec<f32>,
    inter: Vec<f32>,
}

impl Default for MasterBake {
    fn default() -> Self {
        MasterBake {
            h: None,
            place: ReadPlace::default(),
            l: vec![0.0; MAX_BLOCK_FRAMES],
            r: vec![0.0; MAX_BLOCK_FRAMES],
            inter: vec![0.0; MAX_BLOCK_FRAMES * 2],
        }
    }
}

impl MasterBake {
    /// The snapshot's render (or none), its loop's start kept ready. Allocation-free: the
    /// snapshot holds the handle too, so the last owner is never this thread.
    pub fn set(&mut self, h: Option<Arc<FrozenHandle>>, loop_frame: Option<i64>) {
        let same = match (&self.h, &h) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some(old) = self.h.as_ref() {
                crate::freeze::release(old);
            }
            self.place.block = None;
        }
        self.h = h;
        self.place.loop_frame = loop_frame;
    }

    /// Add the render's frames at `from` (the pass's first render frame) into `out`
    /// (interleaved, `n` frames), the master's peaks taken with them. Allocation- and
    /// lock-free; in real time a block not read yet is silence, counted in `late`.
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        from: i64,
        playing: bool,
        offline: bool,
        out: &mut [f32],
        channels: usize,
        n: usize,
        peaks: &mut [f32; 2],
        late: &AtomicU64,
    ) {
        let Some(h) = self.h.as_ref() else {
            return;
        };
        crate::freeze::point(h, &mut self.place, from);
        let n = n.min(self.l.len());
        if !playing || from + n as i64 <= 0 || from >= h.frames as i64 {
            return;
        }
        let missed = crate::freeze::read(
            h,
            from,
            &mut self.l[..n],
            &mut self.r[..n],
            &mut self.inter,
            offline,
        );
        if missed > 0 {
            late.fetch_add(missed as u64, Relaxed);
        }
        for (i, frame) in out.chunks_exact_mut(channels).take(n).enumerate() {
            if channels > 1 {
                frame[0] += self.l[i];
                frame[1] += self.r[i];
                peaks[0] = peaks[0].max(frame[0].abs());
                peaks[1] = peaks[1].max(frame[1].abs());
            } else {
                frame[0] += self.l[i] + self.r[i];
                peaks[0] = peaks[0].max(frame[0].abs());
            }
        }
    }
}
