//! The render of a warped clip's window (ADR 0061 §6): the source (or the pitch editor's
//! render of it) read through the clip's warp under the project's tempo map, by the clip's
//! mode (`cantus_dsp::stretch`).
//!
//! **Layout.** The render covers the clip's window only — frame `k` is heard `k` samples
//! after the window's start, `start` on the timeline — so the engine plays it as it plays
//! any source (`add_clips`, one frame a sample) with the window's start as its frame 0 (the
//! sample cache marks it `zero_at_start`). Output frame `k` is song time
//! `seconds(start) + k / rate`, which is timeline tick `T` through the tempo map, content
//! tick `T − start + offset`, and source frame `warp.frame_at(that)`. A loop's pieces share
//! one render wherever the tempo over them is the same (the key holds the map's `span_key`
//! over the window, not its place).
//!
//! **A warp that does not follow the tempo** (a transposition, ADR 0061) plays at natural
//! speed: output frame `k` reads the source frame under the window's start, plus `k`. That
//! start frame — to the whole frame, what the engine plays to anyway — and the window's
//! length are all its render depends on, so its key holds them and not the offset, the fine
//! offset or the tempo before the window that put the frame there (round 29's second
//! review: a tempo edit before the window left a stale render playing, two loop pieces
//! whose hidden parts differed shared one, and a tempo edit's re-anchoring re-rendered a
//! window whose audio it had kept).

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

    /// The source frame (the source's own rate, to the whole frame) under the window's
    /// start of a warp that does not follow the tempo: what the unwarped rule reads there
    /// (`audio_time::frame_at`, from the content origin with the fine offset).
    pub fn natural_start(&self) -> f64 {
        audio_time::frame_at(
            &self.map,
            self.start,
            self.offset,
            self.slip_frames,
            self.source_rate.max(1) as f64,
            self.start.0 as f64,
        )
        .round()
    }

    /// The input frame (at `rate`) output frame `k` reads.
    pub fn source_at(&self, k: f64) -> f64 {
        let scale = self.rate as f64 / self.source_rate.max(1) as f64;
        if !self.warp.follow {
            // Natural speed from the window's start (see the module docs).
            return self.natural_start() * scale + k;
        }
        let t = self.tick_at(k);
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

    /// The stretcher's map over the window: its ends and every warp marker inside it
    /// (**hard** points: the audio is pinned there), and, where the tempo moves under the
    /// window, every tempo point inside it (a step's corner, a ramp's ends) and a sample
    /// every [`RAMP_STEP`] frames (**soft** points: a smooth curve, which Rhythm never cuts
    /// at and a transient's hold may move — round 29's review).
    pub fn time_map(&self) -> Map {
        let len = self.len();
        if !self.warp.follow {
            return Map::linear(self.source_at(0.0), 1.0);
        }
        let s = self.start.0 as f64;
        let e = self.end.0 as f64;
        let held = self.map.held_at(s).is_some_and(|(_, _, to)| to >= e);
        // (frame, hard)
        let mut ks: Vec<(f64, bool)> = vec![(0.0, true), (len as f64, true)];
        if !held {
            ks.extend((1..len / RAMP_STEP + 1).map(|i| ((i * RAMP_STEP) as f64, false)));
            for p in self.map.points() {
                let t = p.tick.0 as f64;
                if t > s && t < e {
                    ks.push((self.frame_of(t), false));
                }
            }
        }
        if self.warp.follow {
            let origin = s - self.offset.0 as f64;
            for m in &self.warp.markers {
                let k = self.frame_of(origin + m.tick.0 as f64);
                if k > 0.0 && k < len as f64 {
                    // The warp's own markers (a ramp's samples, the tempo points) are
                    // soft, as the tempo's are (round 29's second review).
                    ks.push((k, !m.auto));
                }
            }
        }
        // In order; where two land together, the hard one stays.
        ks.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
        ks.dedup_by(|b, a| b.0 - a.0 < 1e-9);
        let pts: Vec<(f64, f64)> = ks.iter().map(|&(k, _)| (k, self.source_at(k))).collect();
        let soft: Vec<bool> = ks.iter().map(|&(_, hard)| !hard).collect();
        Map::with_soft(pts, soft).unwrap_or_else(|| Map::linear(self.source_at(0.0), 1.0))
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

/// The input's channels over frames `[lo, hi)` (clamped) — only the span a render reads,
/// never a whole long take for a short window (round 29's review).
fn channels(d: &SampleData, lo: usize, hi: usize) -> Vec<Vec<f32>> {
    let ch = d.channels.max(1) as usize;
    let frames = d.frames.len() / ch;
    let (lo, hi) = (lo.min(frames), hi.min(frames).max(lo.min(frames)));
    (0..ch)
        .map(|c| (lo..hi).map(|i| d.frames[i * ch + c]).collect())
        .collect()
}

/// The input's whole length, frames.
fn frames_of(d: &SampleData) -> usize {
    d.frames.len() / d.channels.max(1) as usize
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

/// The clip's render (see the module docs), at full quality. Deterministic. `pitch` is
/// the pitch base's analysis of the whole input (Melodic: made once per input and kept by
/// the service, never per render); only the span the window reads is taken out of the
/// input.
pub fn render(
    input: &SampleData,
    plan: &Plan,
    analysis: Option<&Analysis>,
    pitch: Option<&cantus_dsp::pitch::Analysis>,
) -> SampleData {
    let map = plan.time_map();
    let scale = plan.rate as f64 / plan.source_rate.max(1) as f64;
    let onsets: Vec<f64> = analysis
        .map(|a| a.onsets.iter().map(|o| o * scale).collect())
        .unwrap_or_default();
    let mode = mode_of(plan.warp.mode);
    let mut req = Request {
        mode,
        map: &map,
        len: plan.len(),
        semitones: plan.warp.semitones(),
        formants: plan.warp.formants,
        onsets: &onsets,
        pitch: pitch.filter(|_| mode == Mode::Melodic),
        tail: tail_of(plan.warp.tail),
        origin: 0,
    };
    let (lo, hi) = stretch::reach(&req, plan.rate as f32, frames_of(input));
    let chs = channels(input, lo, hi);
    let refs: Vec<&[f32]> = chs.iter().map(Vec::as_slice).collect();
    req.origin = lo;
    let out = stretch::render(&refs, plan.rate as f32, &req);
    interleave(&out, plan.rate)
}

/// The pitch base's analysis of the whole input (for a Melodic render).
pub fn pitch_of(input: &SampleData, rate: u32) -> cantus_dsp::pitch::Analysis {
    let chs = channels(input, 0, frames_of(input));
    let refs: Vec<&[f32]> = chs.iter().map(Vec::as_slice).collect();
    cantus_dsp::pitch::analyse(&crate::pitch::mixdown(&refs), rate as f32)
}

/// The bridge while the render is made, in milliseconds: the right speed and — unless the
/// clip is Re-pitch, where speed is pitch — the right pitch at once. A transposed clip is
/// sketched by long grains read at its transposition (`stretch::sketch`), so an edit never
/// drops it back to its source's pitch while the full render is made (round 29's review);
/// an untransposed one is read at varispeed through the same map. Only the span the
/// window reads is taken out of the input. `onsets`: the file's transients at the render's
/// rate, when its analysis is in (a transposed sketch puts them, and the markers, on their
/// frames).
pub fn bridge(input: &SampleData, plan: &Plan, onsets: &[f64]) -> SampleData {
    let map = plan.time_map();
    let len = plan.len();
    let (a, b) = (map.source(0.0), map.source(len as f64));
    let reach = 8_192.0 * plan.warp.semitones().abs().max(1.0);
    let lo = (a.min(b) - reach).max(0.0) as usize;
    let hi = (a.max(b) + reach).max(0.0) as usize + 2;
    let chs = channels(input, lo, hi);
    let shift = lo as f64;
    let transposed = plan.warp.semitones() != 0.0 && plan.warp.mode != WarpMode::Repitch;
    let out: Vec<Vec<f32>> = if transposed {
        let refs: Vec<&[f32]> = chs.iter().map(Vec::as_slice).collect();
        let mut anchors: Vec<f64> = map
            .hard_points()
            .map(|p| p.1)
            .chain(onsets.iter().copied())
            .collect();
        anchors.sort_by(f64::total_cmp);
        anchors.dedup_by(|b, a| (*b - *a).abs() < 1.0);
        let semis = plan.warp.semitones();
        stretch::sketch(&refs, plan.rate as f32, &map, len, semis, lo, &anchors)
    } else {
        chs.iter()
            .map(|x| {
                (0..len)
                    .map(|k| {
                        let s = map.source(k as f64) - shift;
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
            .collect()
    };
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
    fnv(&mut h, &w.cents.to_le_bytes());
    fnv(&mut h, &[w.formants as u8, w.follow as u8]);
    if !w.follow {
        // Natural speed: the frame under the window's start and the window's length.
        fnv(&mut h, &plan.natural_start().to_bits().to_le_bytes());
        fnv(&mut h, &(plan.len() as u64).to_le_bytes());
        return h;
    }
    fnv(&mut h, &w.bpm.to_bits().to_le_bytes());
    for m in &w.markers {
        fnv(&mut h, &m.frame.to_bits().to_le_bytes());
        fnv(&mut h, &m.tick.0.to_le_bytes());
        fnv(&mut h, &[m.auto as u8]);
    }
    fnv(&mut h, &plan.offset.0.to_le_bytes());
    fnv(&mut h, &(plan.end.0 - plan.start.0).to_le_bytes());
    fnv(
        &mut h,
        &plan.map.span_key(plan.start, plan.end).to_le_bytes(),
    );
    h
}
