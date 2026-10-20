//! The render of a warped clip's window (ADR 0061 §6): the source (or the pitch editor's
//! render of it) read through the clip's warp under the project's tempo map, by the clip's
//! mode (`cantus_dsp::stretch`).
//!
//! **Layout.** The render covers the clip's window only — frame `k` is heard `k` samples
//! after the window's start, `start` on the timeline — so the engine plays it as it plays
//! any source (`add_clips`, one frame a sample) with the window's start as its frame 0 (the
//! sample cache sets the clip's `slip` to `−offset_seconds`, so `Transport::clip_zero` is
//! the window's start). Output frame `k` is song time `seconds(start) + k / rate`, which is
//! timeline tick `T` through the tempo map, content tick `T − start + offset`, and source
//! frame `warp.frame_at(that)`. A loop's pieces share one render wherever the tempo over
//! them is the same (the key holds the map's `span_key` over the window, not its place).

use super::Analysis;
use cantus_core::audio_time;
use cantus_core::tempo::TempoMap;
use cantus_core::time::{Ticks, PPQ};
use cantus_core::warp::{Warp, WarpMode, WarpTail};
use cantus_core::SampleData;
use cantus_dsp::stretch::{self, Map, Mode, Request, Tail};
use std::path::PathBuf;
use std::sync::Arc;

/// Breakpoints this far apart (output frames) where the tempo moves under the window.
const RAMP_STEP: usize = 256;

/// Everything a clip's render depends on.
#[derive(Clone, Debug)]
pub struct Plan {
    pub warp: Arc<Warp>,
    pub map: Arc<TempoMap>,
    /// The window on the timeline and the content tick at its start.
    pub start: Ticks,
    pub end: Ticks,
    pub offset: Ticks,
    /// The region's fine offset, source frames (a clip that does not follow the tempo).
    pub slip_frames: f64,
    /// The source file's own rate (the warp's frames count at it).
    pub source_rate: u32,
    /// The rate rendered at (the engine's; the input is at it too).
    pub rate: u32,
    /// The source file (its analysis: the transients).
    pub path: PathBuf,
    /// What the input is (the source, or a pitch render's key).
    pub input_key: u64,
}

impl Plan {
    /// The render's length, frames: the window, rounded up, and one more.
    pub fn len(&self) -> usize {
        let secs = self
            .map
            .seconds_between_f64(self.start.0 as f64, self.end.0 as f64)
            .max(0.0);
        (secs * self.rate as f64).ceil() as usize + 1
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// The timeline tick heard at output frame `k`.
    pub fn tick_at(&self, k: f64) -> f64 {
        let s = self.start.0 as f64;
        if let Some((bpm, _, to)) = self.map.held_at(s) {
            let t = s + k / self.rate as f64 * bpm / 60.0 * PPQ as f64;
            if t <= to {
                return t;
            }
        }
        self.map.tick_after_seconds(s, k / self.rate as f64)
    }

    /// The input frame (at `rate`) output frame `k` reads.
    pub fn source_at(&self, k: f64) -> f64 {
        let t = self.tick_at(k);
        let scale = self.rate as f64 / self.source_rate.max(1) as f64;
        // The one mapping the interface draws through too (ADR 0061 §6).
        let f = audio_time::frame_through(
            &self.map,
            self.start,
            self.offset,
            Some(&self.warp),
            self.slip_frames,
            self.source_rate as f64,
            t,
        );
        f * scale
    }

    /// The output frame where timeline tick `t` is heard.
    fn frame_of(&self, t: f64) -> f64 {
        self.map.seconds_between_f64(self.start.0 as f64, t) * self.rate as f64
    }

    /// The stretcher's map over the window: its ends, every warp marker inside it, and,
    /// where the tempo moves under the window, a breakpoint every [`RAMP_STEP`] frames.
    pub fn time_map(&self) -> Map {
        let len = self.len();
        let s = self.start.0 as f64;
        let held = self
            .map
            .held_at(s)
            .is_some_and(|(_, _, to)| to >= self.end.0 as f64);
        let mut ks: Vec<f64> = vec![0.0, len as f64];
        if !held {
            ks.extend((1..len / RAMP_STEP + 1).map(|i| (i * RAMP_STEP) as f64));
        }
        if self.warp.follow {
            let origin = s - self.offset.0 as f64;
            for m in &self.warp.markers {
                let k = self.frame_of(origin + m.tick.0 as f64);
                if k > 0.0 && k < len as f64 {
                    ks.push(k);
                }
            }
        }
        ks.sort_by(f64::total_cmp);
        ks.dedup_by(|b, a| *b - *a < 1e-9);
        let pts: Vec<(f64, f64)> = ks.iter().map(|&k| (k, self.source_at(k))).collect();
        // A marker's own frame where it is exact (the formula's round trip aside).
        Map::new(pts).unwrap_or_else(|| Map::linear(self.source_at(0.0), 1.0))
    }
}

fn mode_of(m: WarpMode) -> Mode {
    match m {
        WarpMode::Complex => Mode::Complex,
        WarpMode::Rhythm => Mode::Rhythm,
        WarpMode::Melodic => Mode::Melodic,
        WarpMode::Repitch => Mode::Repitch,
    }
}

fn tail_of(t: WarpTail) -> Tail {
    match t {
        WarpTail::Fade => Tail::Fade,
        WarpTail::Loop => Tail::Loop,
        WarpTail::PingPong => Tail::PingPong,
    }
}

fn channels(d: &SampleData) -> Vec<Vec<f32>> {
    let ch = d.channels.max(1) as usize;
    (0..ch)
        .map(|c| d.frames.iter().skip(c).step_by(ch).copied().collect())
        .collect()
}

fn interleave(out: &[Vec<f32>], rate: u32) -> SampleData {
    let n = out.len().max(1);
    let len = out.first().map_or(0, Vec::len);
    let mut frames = Vec::with_capacity(len * n);
    for i in 0..len {
        for c in out {
            frames.push(c[i]);
        }
    }
    SampleData {
        stream: None,
        rate,
        channels: n as u8,
        frames: frames.into(),
    }
}

/// The clip's render (see the module docs), at full quality. Deterministic.
pub fn render(input: &SampleData, plan: &Plan, analysis: Option<&Analysis>) -> SampleData {
    let chs = channels(input);
    let refs: Vec<&[f32]> = chs.iter().map(Vec::as_slice).collect();
    let map = plan.time_map();
    let scale = plan.rate as f64 / plan.source_rate.max(1) as f64;
    let onsets: Vec<f64> = analysis
        .map(|a| a.onsets.iter().map(|o| o * scale).collect())
        .unwrap_or_default();
    let mode = mode_of(plan.warp.mode);
    let pitch = (mode == Mode::Melodic)
        .then(|| cantus_dsp::pitch::analyse(&crate::pitch::mixdown(&refs), plan.rate as f32));
    let out = stretch::render(
        &refs,
        plan.rate as f32,
        &Request {
            mode,
            map: &map,
            len: plan.len(),
            semitones: plan.warp.semitones(),
            formants: plan.warp.formants,
            onsets: &onsets,
            pitch: pitch.as_ref(),
            tail: tail_of(plan.warp.tail),
            origin: 0,
        },
    );
    interleave(&out, plan.rate)
}

/// The bridge while the render is made: the same map read at varispeed, linear
/// interpolation, untransposed — the right speed at once, a few milliseconds of work.
pub fn bridge(input: &SampleData, plan: &Plan) -> SampleData {
    let chs = channels(input);
    let map = plan.time_map();
    let out: Vec<Vec<f32>> = chs
        .iter()
        .map(|x| {
            (0..plan.len())
                .map(|k| {
                    let s = map.source(k as f64);
                    let i = s.floor();
                    let f = (s - i) as f32;
                    let get = |j: i64| {
                        if j >= 0 && (j as usize) < x.len() {
                            x[j as usize]
                        } else {
                            0.0
                        }
                    };
                    let i = i as i64;
                    get(i) + (get(i + 1) - get(i)) * f
                })
                .collect()
        })
        .collect();
    interleave(&out, plan.rate)
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// The render's key: the file, what the input is, the warp, the window's content and
/// length, the tempo over the window, the rates — any change makes a new render, and two
/// windows that would render the same (a loop's pieces under one tempo) share one.
pub fn key(plan: &Plan) -> u64 {
    let w = &plan.warp;
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, plan.path.to_string_lossy().as_bytes());
    fnv(&mut h, &plan.input_key.to_le_bytes());
    fnv(&mut h, &plan.rate.to_le_bytes());
    fnv(&mut h, &plan.source_rate.to_le_bytes());
    fnv(&mut h, &w.rate.to_le_bytes());
    fnv(&mut h, w.mode.id().as_bytes());
    fnv(&mut h, w.tail.id().as_bytes());
    fnv(&mut h, &w.bpm.to_bits().to_le_bytes());
    fnv(&mut h, &w.cents.to_le_bytes());
    fnv(&mut h, &[w.formants as u8, w.follow as u8]);
    for m in &w.markers {
        fnv(&mut h, &m.frame.to_bits().to_le_bytes());
        fnv(&mut h, &m.tick.0.to_le_bytes());
    }
    fnv(&mut h, &plan.offset.0.to_le_bytes());
    fnv(&mut h, &(plan.end.0 - plan.start.0).to_le_bytes());
    fnv(
        &mut h,
        &plan.map.span_key(plan.start, plan.end).to_le_bytes(),
    );
    if !w.follow {
        fnv(&mut h, &plan.slip_frames.to_bits().to_le_bytes());
    }
    h
}
