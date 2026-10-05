//! Waveforms (ADR 0025): peak data for audio sources and the one painter every view draws
//! them with — audio clips in the arrangement, takes as they are recorded, the inspector.
//!
//! **Contract (round 19):** the audio-clips builder owns this file (peak files computed on
//! a thread, levels of detail, the cache) and keeps [`paint_peaks`]'s signature, which the
//! recording builder draws live takes with.
//!
//! How it fits together:
//!
//! - [`SourcePeaks`] is one file's peaks: min/max pairs per channel and mono-summed, at
//!   three levels of detail ([`LODS`]: 64, 512 and 4096 frames per peak). Level 0 sees
//!   every sample, so the loudest sample of any range is known exactly (normalise).
//! - [`SourcePeaks::columns`] answers "what does column *i* of this clip show" at any zoom
//!   by folding the coarsest level that is still finer than a column, so a three-minute
//!   file zoomed out costs a few hundred pairs a frame, not millions of samples.
//! - [`PeakStore`] (in `waveform_store.rs`) computes them on a worker thread, keeps them
//!   in memory, and on disk next to the user's data keyed by the file's path, size and
//!   modification time, so a reopened project draws at once.
//! - [`fill_peaks`] draws columns as **one strip of the brush's mesh** per channel (two
//!   vertices a column), which is what keeps a screen of audio clips under budget.

use crate::kit::{self, Brush};
use egui::{pos2, Color32, Painter, Pos2, Rect};
use std::cell::RefCell;

#[path = "waveform_store.rs"]
mod store;
pub use store::{resolve, Lookup, PeakStore};

/// Min/max pairs, one per `frames_per_peak` frames, mono-summed, in -1..=1.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Peaks {
    pub frames_per_peak: usize,
    pub pairs: Vec<(f32, f32)>,
}

/// The levels of detail every source is reduced to, finest first. Each is 8× the one
/// before, so the next level is computed from the last, not from the samples.
pub const LODS: [usize; 3] = [64, 512, 4096];

/// One level of detail of one source.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Level {
    pub frames_per_peak: usize,
    /// One list per channel of the file (1 or 2), min/max pairs.
    pub channels: Vec<Vec<(f32, f32)>>,
    /// Min/max of `(l + r) / 2` (of the one channel for a mono file).
    pub mono: Vec<(f32, f32)>,
}

/// Which signal of a source to read.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Which {
    Mono,
    Channel(usize),
}

/// One file's peaks at every level of detail.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourcePeaks {
    pub rate: u32,
    pub channels: u8,
    pub frames: u64,
    /// Finest first, one per [`LODS`] entry.
    pub levels: Vec<Level>,
}

fn fold(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0.min(b.0), a.1.max(b.1))
}

const EMPTY: (f32, f32) = (f32::MAX, f32::MIN);

impl SourcePeaks {
    /// Reduce interleaved samples (`channels` of them per frame, 1 or 2 used) to peaks.
    pub fn compute(rate: u32, channels: u8, samples: &[f32]) -> SourcePeaks {
        let ch = channels.max(1) as usize;
        let frames = samples.len() / ch;
        let fpp = LODS[0];
        let n = frames.div_ceil(fpp);
        let used = ch.min(2);
        let mut per: Vec<Vec<(f32, f32)>> = vec![Vec::with_capacity(n); used];
        let mut mono = Vec::with_capacity(n);
        for block in 0..n {
            let a = block * fpp;
            let b = (a + fpp).min(frames);
            let mut c = [EMPTY; 2];
            let mut m = EMPTY;
            for f in a..b {
                let frame = &samples[f * ch..f * ch + ch];
                let mut sum = 0.0;
                for (i, v) in frame.iter().take(used).enumerate() {
                    c[i] = fold(c[i], (*v, *v));
                    sum += *v;
                }
                let v = sum / used as f32;
                m = fold(m, (v, v));
            }
            for (i, list) in per.iter_mut().enumerate() {
                list.push(c[i]);
            }
            mono.push(m);
        }
        let mut levels = vec![Level {
            frames_per_peak: fpp,
            channels: per,
            mono,
        }];
        for &fpp in &LODS[1..] {
            let prev = levels.last().expect("level 0");
            let k = fpp / prev.frames_per_peak;
            let coarse = |v: &Vec<(f32, f32)>| -> Vec<(f32, f32)> {
                v.chunks(k)
                    .map(|c| c.iter().copied().fold(EMPTY, fold))
                    .collect()
            };
            let next = Level {
                frames_per_peak: fpp,
                channels: prev.channels.iter().map(coarse).collect(),
                mono: coarse(&prev.mono),
            };
            levels.push(next);
        }
        SourcePeaks {
            rate,
            channels: used as u8,
            frames: frames as u64,
            levels,
        }
    }

    /// The level a view `frames_per_column` wide should read: the coarsest one still finer
    /// than a column (the finest when a column is narrower than every level).
    pub fn level_for(&self, frames_per_column: f64) -> Option<&Level> {
        self.levels
            .iter()
            .rev()
            .find(|l| l.frames_per_peak as f64 <= frames_per_column)
            .or_else(|| self.levels.first())
    }

    /// `n` columns of `which`, column `i` covering the frames from `from + i·per` to
    /// `from + (i+1)·per`, into `out` (cleared first). A column outside the file is
    /// silence, `(0, 0)`.
    pub fn columns(&self, which: Which, from: f64, per: f64, n: usize, out: &mut Vec<(f32, f32)>) {
        out.clear();
        let Some(level) = self.level_for(per) else {
            out.resize(n, (0.0, 0.0));
            return;
        };
        let list = match which {
            Which::Mono => &level.mono,
            Which::Channel(c) => match level.channels.get(c).or(level.channels.first()) {
                Some(l) => l,
                None => {
                    out.resize(n, (0.0, 0.0));
                    return;
                }
            },
        };
        let fpp = level.frames_per_peak as f64;
        let total = self.frames as f64;
        for i in 0..n {
            let a = (from + i as f64 * per).max(0.0);
            let b = (from + (i + 1) as f64 * per).min(total);
            if b <= a || a >= total {
                out.push((0.0, 0.0));
                continue;
            }
            let ia = (a / fpp).floor() as usize;
            let ib = ((b / fpp).ceil() as usize).max(ia + 1).min(list.len());
            let pair = list
                .get(ia..ib)
                .map(|s| s.iter().copied().fold(EMPTY, fold))
                .unwrap_or((0.0, 0.0));
            out.push(if pair == EMPTY { (0.0, 0.0) } else { pair });
        }
    }

    /// The loudest sample (absolute value, any channel) between two frames, from the
    /// finest level — what normalising a clip's window calls for.
    pub fn peak_in(&self, from: f64, to: f64) -> f32 {
        let Some(level) = self.levels.first() else {
            return 0.0;
        };
        let fpp = level.frames_per_peak as f64;
        let a = (from.max(0.0) / fpp).floor() as usize;
        let b = (to.min(self.frames as f64) / fpp).ceil() as usize;
        let mut peak = 0.0f32;
        for list in &level.channels {
            for &(lo, hi) in list.get(a..b.min(list.len())).unwrap_or(&[]) {
                peak = peak.max(lo.abs()).max(hi.abs());
            }
        }
        peak
    }

    /// The loudest sample of the whole file.
    pub fn peak(&self) -> f32 {
        self.peak_in(0.0, self.frames as f64)
    }

    /// The mono-summed pairs of one level, in the contract's shape.
    pub fn mono(&self, level: usize) -> Peaks {
        self.levels
            .get(level)
            .map(|l| Peaks {
                frames_per_peak: l.frames_per_peak,
                pairs: l.mono.clone(),
            })
            .unwrap_or_default()
    }

    /// Seconds of audio.
    pub fn seconds(&self) -> f64 {
        self.frames as f64 / self.rate.max(1) as f64
    }
}

thread_local! {
    /// The strip's two edges, reused so painting allocates nothing once warm.
    static EDGES: RefCell<(Vec<Pos2>, Vec<Pos2>)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

/// Draw `pairs` (the columns to show, left to right) across `rect` as one filled strip,
/// centred on its middle and scaled by `gain(i)` (linear, per column, so a clip's gain and
/// fades shape the drawing). Every column is at least a physical pixel tall, so silence
/// still draws its line.
pub fn fill_peaks(
    b: &mut Brush,
    rect: Rect,
    pairs: &[(f32, f32)],
    gain: impl Fn(usize) -> f32,
    color: Color32,
) {
    if pairs.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let mid = rect.center().y;
    let half = rect.height() * 0.5;
    let px = b.px();
    let step = rect.width() / pairs.len() as f32;
    EDGES.with(|cell| {
        let Ok(mut edges) = cell.try_borrow_mut() else {
            return;
        };
        let (top, bottom) = &mut *edges;
        top.clear();
        bottom.clear();
        // A single column still needs two points to make a strip.
        let wide = pairs.len() == 1;
        for (i, &(lo, hi)) in pairs.iter().enumerate() {
            let g = gain(i);
            let y0 = mid - (hi * g).clamp(-1.0, 1.0) * half;
            let y1 = (mid - (lo * g).clamp(-1.0, 1.0) * half).max(y0 + px);
            let xs: &[f32] = if wide {
                &[rect.left(), rect.right()]
            } else {
                &[rect.left() + (i as f32 + 0.5) * step]
            };
            for &x in xs {
                top.push(pos2(x, y0));
                bottom.push(pos2(x, y1));
            }
        }
        b.between(top, bottom, color);
    });
}

/// Paint `pairs` (already the columns to show, oldest first) across `rect`, centred on
/// its middle, scaled by `gain` (linear). Contract signature; drawn as [`fill_peaks`] —
/// one mesh — through the kit's brush.
pub fn paint_peaks(painter: &Painter, rect: Rect, pairs: &[(f32, f32)], gain: f32, color: Color32) {
    if pairs.is_empty() || rect.width() <= 0.0 {
        return;
    }
    kit::with_brush(painter, |b| fill_peaks(b, rect, pairs, |_| gain, color));
}
