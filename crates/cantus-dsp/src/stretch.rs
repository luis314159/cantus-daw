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
#[path = "stretch_repitch.rs"]
mod repitch;
#[path = "stretch_rhythm.rs"]
mod rhythm;
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
        return copy(channels, req.len, shift);
    }
    if channels.is_empty() || req.len == 0 {
        return channels.iter().map(|_| vec![0.0; req.len]).collect();
    }
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
    let (hmap, holds) = req
        .map
        .hold(req.onsets, (pv::frame_len(rate) / 2) as f64, 0.030 * rate as f64);
    let mut out = pv::render(&h, rate, &hmap, &holds, req.len, ratio, req.formants);
    let p: Vec<&[f32]> = percussive.iter().map(Vec::as_slice).collect();
    let shifted = req.map.held_at(req.onsets, rate);
    let hits = hpss::grains(&p, rate, &shifted, req.len, ratio);
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
