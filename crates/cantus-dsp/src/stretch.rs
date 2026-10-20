//! **The stretcher and shifter** of audio clips (round 29, ADR 0061): a whole clip rendered
//! offline through a **time map** (where each output frame reads the source) and a
//! transposition, by one of four methods — the clip's *mode*.
//!
//! | Mode | For | How |
//! |---|---|---|
//! | [`Mode::Complex`] | mixes, anything polyphonic (the default) | the input split into its harmonic and percussive parts (median filtering, Driedger–Müller); the harmonic part through a **phase-locked phase vocoder** (identity locking around peaks, Laroche–Dolson; the instantaneous frequency from two analysis frames one synthesis hop apart, so the time map may vary freely), shifted by **mapping bins** with the **spectral envelope** put back (formants kept, true envelope, Röbel–Rodet) and with **one phase for both channels** (the image never smears); the percussive part by short grains read through a map that is a pure shift around every transient, so a hit is copied, never stretched |
//! | [`Mode::Rhythm`] | drums, percussive loops (Ableton's *Beats*) | slices between transients played at their own speed from where the map puts their transient, cut or filled at the end ([`Tail`]) |
//! | [`Mode::Melodic`] | a voice, a solo instrument (Ableton's *Tones*) | TD-PSOLA on the pitch base's epochs (ADR 0053), formants kept; where the line is unvoiced or not one voice, the Complex render cross-faded in |
//! | [`Mode::Repitch`] | tape: speed and pitch together | the source read at the map's speed through windowed-sinc mip levels (Exemplar's resampler) |
//!
//! **Identity:** a map that reads the source sample for sample (slope 1 at a whole-sample
//! offset) and no transposition return the source **to the bit**, in every mode. Every
//! render is deterministic (`crate::math` only) and its output exactly `len` frames long.
//! Offline: renders allocate (they run on a worker, never on the audio thread).

use crate::pitch::Analysis;

#[path = "stretch_env.rs"]
pub(crate) mod env;
#[path = "stretch_hpss.rs"]
mod hpss;
#[path = "stretch_map.rs"]
mod map;
#[path = "stretch_measure.rs"]
pub mod measure;
#[path = "stretch_melodic.rs"]
mod melodic;
#[path = "stretch_pv.rs"]
pub(crate) mod pv;
#[path = "stretch_read.rs"]
mod read;
#[path = "stretch_repitch.rs"]
mod repitch;
#[cfg(test)]
#[path = "stretch_review_tests.rs"]
mod review_tests;
#[path = "stretch_rhythm.rs"]
mod rhythm;
#[path = "stretch_sketch.rs"]
mod sketch;
#[cfg(test)]
#[path = "stretch_tests.rs"]
mod tests;

pub use map::Map;

/// How a clip is stretched (see the module docs).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Complex,
    Rhythm,
    Melodic,
    Repitch,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Complex, Mode::Rhythm, Mode::Melodic, Mode::Repitch];

    /// Whether the mode transposes apart from the speed (Re-pitch cannot: its pitch *is*
    /// its speed).
    pub fn transposes(self) -> bool {
        self != Mode::Repitch
    }

    /// Whether the mode can keep the formants while it transposes.
    pub fn keeps_formants(self) -> bool {
        matches!(self, Mode::Complex | Mode::Melodic)
    }
}

/// What a slice does in [`Mode::Rhythm`] when its source ends before the next slice starts
/// (the clip slowed down).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tail {
    /// Fade out and leave the gap silent (Ableton's *Off* with a fade).
    #[default]
    Fade,
    /// Loop the slice's end forward until the next slice.
    Loop,
    /// Loop the slice's end back and forth.
    PingPong,
}

impl Tail {
    pub const ALL: [Tail; 3] = [Tail::Fade, Tail::Loop, Tail::PingPong];
}

/// One render.
#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub mode: Mode,
    /// Where output frame `t` reads the source (both at the render's rate).
    pub map: &'a Map,
    /// The output's length, frames.
    pub len: usize,
    /// The transposition, semitones (cents as hundredths). Ignored by [`Mode::Repitch`].
    pub semitones: f64,
    /// Keep the formants while transposing (Complex, Melodic).
    pub formants: bool,
    /// The source's transients, in source frames, in order (the onset detector's): Rhythm
    /// slices there; Complex copies the percussive part there.
    pub onsets: &'a [f64],
    /// The pitch base's analysis of the source (mono mixdown, the render's rate), for
    /// [`Mode::Melodic`]; without it Melodic renders as Complex.
    pub pitch: Option<&'a Analysis>,
    /// Rhythm's slice ends.
    pub tail: Tail,
    /// The source frame `channels[..][0]` is (0: the channels are the whole source). The
    /// map, the onsets and the pitch analysis stay in the source's own frames; a caller may
    /// pass only [`reach`]'s span of a long file.
    pub origin: usize,
}

/// The widest transposition, semitones either way (two octaves).
pub const MAX_SEMITONES: f64 = 24.0;

/// Render `req` over `channels` (one or two, each the source at `rate`): the output, one
/// vector per channel, each exactly `req.len` frames. See the module docs.
pub fn render(channels: &[&[f32]], rate: f32, req: &Request) -> Vec<Vec<f32>> {
    let semis = if req.mode.transposes() {
        req.semitones.clamp(-MAX_SEMITONES, MAX_SEMITONES)
    } else {
        0.0
    };
    if let Some(shift) = (semis == 0.0)
        .then(|| req.map.plain_over(req.len))
        .flatten()
    {
        return copy(channels, req.len, shift - req.origin as i64);
    }
    if channels.is_empty() || req.len == 0 {
        return channels.iter().map(|_| vec![0.0; req.len]).collect();
    }
    // Every mode works on the span the window reads (with the context its analyses and
    // readers need), never the whole file: a short window of a long take costs its own
    // length (round 29's reviews).
    let src_end = req.origin + channels.iter().map(|c| c.len()).max().unwrap_or(0);
    let (lo, hi) = {
        let (lo, hi) = reach(req, rate, src_end);
        (lo.max(req.origin), hi.min(src_end).max(lo.max(req.origin)))
    };
    if lo != 0 || hi < src_end {
        let local: Vec<&[f32]> = channels
            .iter()
            .map(|c| {
                let a = (lo - req.origin).min(c.len());
                let b = (hi - req.origin).min(c.len()).max(a);
                &c[a..b]
            })
            .collect();
        let shift = lo as f64;
        let pts: Vec<(f64, f64)> = req
            .map
            .points()
            .iter()
            .map(|p| (p.0, p.1 - shift))
            .collect();
        let soft: Vec<bool> = (0..pts.len()).map(|i| req.map.is_soft(i)).collect();
        let map = Map::with_soft(pts, soft).unwrap_or_else(Map::identity);
        let map = if req.map.points().is_empty() {
            Map::linear(-shift, 1.0)
        } else {
            map
        };
        let onsets: Vec<f64> = req
            .onsets
            .iter()
            .filter(|s| **s >= lo as f64 && **s < hi as f64)
            .map(|s| s - shift)
            .collect();
        let pitch = req.pitch.map(|a| window_of(a, lo, hi));
        let local_req = Request {
            map: &map,
            onsets: &onsets,
            pitch: pitch.as_ref(),
            origin: 0,
            ..req.clone()
        };
        return dispatch(&local, rate, &local_req, semis);
    }
    dispatch(channels, rate, req, semis)
}

/// The render itself, the channels starting at source frame 0.
fn dispatch(channels: &[&[f32]], rate: f32, req: &Request, semis: f64) -> Vec<Vec<f32>> {
    let ratio = crate::math::exp2(semis / 12.0);
    match req.mode {
        Mode::Complex => complex(channels, rate, req, ratio),
        Mode::Rhythm => rhythm::render(channels, rate, req, ratio),
        Mode::Melodic => match req.pitch {
            Some(a) => melodic::render(channels, rate, req, ratio, a),
            None => complex(channels, rate, req, ratio),
        },
        Mode::Repitch => repitch::render(channels, req.map, req.len),
    }
}

/// The Complex render (also Melodic's fallback).
pub(crate) fn complex(channels: &[&[f32]], rate: f32, req: &Request, ratio: f64) -> Vec<Vec<f32>> {
    let (harmonic, percussive) = hpss::split(channels, rate);
    let h: Vec<&[f32]> = harmonic.iter().map(Vec::as_slice).collect();
    // The harmonic part held around transients for a whole window before each (no frame
    // that reaches a transient reads it stretched) and 30 ms after.
    let after = 0.030 * rate as f64;
    let n = pv::frame_for(rate, req.map, req.onsets, after);
    let (hmap, holds) = req.map.hold_long(req.onsets, (n / 2) as f64, after);
    let (mut out, meant) = pv::render(&h, rate, &hmap, &holds, req.len, ratio, req.formants, n);
    let p: Vec<&[f32]> = percussive.iter().map(Vec::as_slice).collect();
    let shifted = req.map.held_at(req.onsets, rate);
    let hits = hpss::grains(&p, rate, &shifted, req.len, ratio, req.onsets);
    // The level the vocoder's frames mean: each its own (what it carries, the
    // transposition's doing included), and the power the two parts shared where it read —
    // correlated in the source, not after: the split's parts summed lose it (round 29's
    // review: noise sank a decibel). Matched on the vocoder's output alone, the copied hits
    // added after as they are: a hit's power is never counted again at every frame that
    // reads near it (round 29's second review: slowed hits came out louder, sped-up ones
    // quieter).
    let win = pv::window(n);
    let w2: f64 = win.iter().map(|w| (*w as f64) * (*w as f64)).sum();
    let target: Vec<(i64, f64)> = meant
        .iter()
        .map(|m| {
            let k = if m.analysed > 1e-20 {
                m.meant / m.analysed
            } else {
                1.0
            };
            let whole = pv::power_of(channels, m.read, &win, w2);
            let parts = pv::power_of(&h, m.read, &win, w2) + pv::power_of(&p, m.read, &win, w2);
            (m.centre, m.meant + (whole - parts).max(0.0) * k)
        })
        .collect();
    pv::level_match(&mut out, &target, n);
    for (o, h) in out.iter_mut().zip(&hits) {
        for (a, b) in o.iter_mut().zip(h) {
            *a += *b;
        }
    }
    out
}

/// The source moved by a whole number of frames: output frame `t` is source frame
/// `t + shift`, zero outside the source.
fn copy(channels: &[&[f32]], len: usize, shift: i64) -> Vec<Vec<f32>> {
    channels
        .iter()
        .map(|x| {
            (0..len as i64)
                .map(|t| {
                    let s = t + shift;
                    if s >= 0 && (s as usize) < x.len() {
                        x[s as usize]
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect()
}

/// What the band-limited reader and the interpolators reach past the span read, frames
/// (the deepest mip level's kernel and margin, 128 × 128), and the grid a span starts on
/// (the deepest level's step): so a window reads exactly as the whole file would.
const READ_PAD: f64 = 16_384.0 + 2_048.0;
const READ_ALIGN: usize = 128;

/// The span of the source, frames `[from, to)`, a render of `req` reads: for Complex and
/// Melodic the source under the window plus the context their analyses reach (the
/// harmonic/percussive split's median over 17 frames, the vocoder's frames, the grains),
/// its start on the split's and the pitch analysis' frame grid so a window renders as the
/// whole file would there; for Re-pitch the span its map reads, and for Rhythm the span
/// its slices read (round 29's second review: both took the whole file), each with its
/// readers' reach, on their grid. `src_len` is the source's length.
pub fn reach(req: &Request, rate: f32, src_len: usize) -> (usize, usize) {
    let span = |a: f64, b: f64| {
        let lo = ((a - READ_PAD).max(0.0) as usize / READ_ALIGN) * READ_ALIGN;
        let hi = ((b + READ_PAD).max(0.0).ceil() as usize).min(src_len);
        (lo.min(src_len), hi.max(lo.min(src_len)))
    };
    match req.mode {
        Mode::Repitch => {
            let (a, b) = (req.map.source(0.0), req.map.source(req.len as f64));
            return span(a.min(b), a.max(b));
        }
        Mode::Rhythm => {
            let semis = req.semitones.clamp(-MAX_SEMITONES, MAX_SEMITONES);
            let ratio = crate::math::exp2(semis / 12.0);
            let (a, b) = rhythm::reach(req, ratio, src_len);
            return span(a, b);
        }
        Mode::Complex | Mode::Melodic => {}
    }
    let a = req.map.source(0.0);
    let b = req.map.source(req.len as f64);
    let (a, b) = (a.min(b), a.max(b));
    let ctx = (3 * hpss::frame_len(rate) + 2 * pv::frame_len(rate) + 4_096) as f64;
    let mut align = hpss::frame_len(rate) / 4;
    if let Some(p) = req.pitch.filter(|p| p.hop > 0) {
        align = lcm(align, p.hop);
    }
    let lo = ((a - ctx).max(0.0) as usize / align) * align;
    let hi = ((b + ctx).max(0.0).ceil() as usize).min(src_len);
    (lo.min(src_len), hi.max(lo.min(src_len)))
}

fn lcm(a: usize, b: usize) -> usize {
    fn gcd(a: usize, b: usize) -> usize {
        if b == 0 {
            a
        } else {
            gcd(b, a % b)
        }
    }
    a / gcd(a, b).max(1) * b
}

/// The pitch analysis over source frames `[lo, hi)`, its frames and epochs counted from
/// `lo` (a multiple of its hop).
fn window_of(a: &Analysis, lo: usize, hi: usize) -> Analysis {
    let hop = a.hop.max(1);
    let first = (lo / hop).min(a.frames.len());
    let last = (hi.div_ceil(hop) + 1).clamp(first, a.frames.len());
    Analysis {
        sample_rate: a.sample_rate,
        hop: a.hop,
        frames: a.frames[first..last].to_vec(),
        epochs: a
            .epochs
            .iter()
            .filter(|e| e.at >= lo as f64 && e.at < hi as f64)
            .map(|e| crate::pitch::Epoch {
                at: e.at - lo as f64,
                period: e.period,
            })
            .collect(),
    }
}

/// A **sketch** of a render, made in milliseconds (WSOLA, `stretch_sketch`): the right
/// timing and the right pitch, a rougher sound — for what plays while the full render is
/// made (the engine's bridge). `channels` start at source frame `origin`, as in [`Request`];
/// `anchors` are the transients and markers (source frames, in order): a grain holding one
/// is read from it outward, so it lands on its frame.
pub fn sketch(
    channels: &[&[f32]],
    rate: f32,
    map: &Map,
    len: usize,
    semitones: f64,
    origin: usize,
    anchors: &[f64],
) -> Vec<Vec<f32>> {
    let semis = semitones.clamp(-MAX_SEMITONES, MAX_SEMITONES);
    let ratio = crate::math::exp2(semis / 12.0);
    let shift = origin as f64;
    let pts: Vec<(f64, f64)> = map.points().iter().map(|p| (p.0, p.1 - shift)).collect();
    let local = if pts.is_empty() {
        Map::linear(-shift, 1.0)
    } else {
        Map::new(pts).unwrap_or_else(Map::identity)
    };
    let local_anchors: Vec<f64> = anchors.iter().map(|s| s - shift).collect();
    sketch::render(channels, rate, &local, len, ratio, &local_anchors)
}
