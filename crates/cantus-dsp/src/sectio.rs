//! Sectio's analysis (round 37, ADR 0099): where a loop is cut. Offline, deterministic.
//!
//! - **Markers first**: a WAV's own `cue ` points, its `smpl` loops' starts, and an ACID
//!   file's `acid` chunk (its beats, hence its tempo) — read from the file's bytes.
//! - **Transients**: the SuperFlux onsets of [`crate::onset`] (sustained material does not
//!   retrigger on a vibrato or a glide), kept above one sensitivity control.
//! - **A beat grid**: the loop's tempo (its ACID beats, or [`crate::beat`]'s grid over its
//!   onsets), so many steps a beat.

use crate::onset;

/// What a WAV file's chunks say about where to cut it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Markers {
    /// `cue ` points (frames), in order.
    pub cues: Vec<u32>,
    /// `smpl` loops' starts (frames).
    pub loops: Vec<u32>,
    /// The ACID chunk: beats in the loop, root note, tempo (BPM) as written.
    pub acid: Option<Acid>,
    /// The `fmt ` chunk: rate, channels, and the data's frames.
    pub rate: u32,
    pub channels: u16,
    pub frames: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Acid {
    pub one_shot: bool,
    pub root: u16,
    pub beats: u32,
    pub meter: (u16, u16),
    pub bpm: f32,
}

fn u16le(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}
fn u32le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}
fn f32le(b: &[u8], i: usize) -> Option<f32> {
    Some(f32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Read a RIFF/WAVE file's markers. `None` when it is not a WAV.
pub fn markers(bytes: &[u8]) -> Option<Markers> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut m = Markers::default();
    let mut block_align = 0u32;
    let mut data_len = 0u32;
    let mut i = 12usize;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32le(bytes, i + 4)? as usize;
        let body = i + 8;
        let end = (body + len).min(bytes.len());
        let c = &bytes[body..end];
        match id {
            b"fmt " => {
                m.channels = u16le(c, 2).unwrap_or(0);
                m.rate = u32le(c, 4).unwrap_or(0);
                block_align = u16le(c, 12).unwrap_or(0) as u32;
            }
            // What the file holds, not what its header claims (a streamed WAV writes
            // 0xFFFFFFFF; a cut file less than it says) — review 2 #5.
            b"data" => data_len = (end - body) as u32,
            b"cue " => {
                // No more points than the chunk has room for (a hostile count of 2³²−1
                // in a 12-byte chunk is read as what is there).
                let n = (u32le(c, 0).unwrap_or(0) as usize).min(c.len().saturating_sub(4) / 24);
                for k in 0..n {
                    // id, position, data chunk id, chunk start, block start, sample offset
                    if let Some(off) = u32le(c, 4 + k * 24 + 20) {
                        m.cues.push(off);
                    }
                }
            }
            b"smpl" => {
                let n = (u32le(c, 28).unwrap_or(0) as usize).min(c.len().saturating_sub(36) / 24);
                for k in 0..n {
                    // cue id, type, start, end, fraction, play count
                    if let Some(s) = u32le(c, 36 + k * 24 + 8) {
                        m.loops.push(s);
                    }
                }
            }
            b"acid" => {
                let flags = u32le(c, 0).unwrap_or(0);
                m.acid = Some(Acid {
                    one_shot: flags & 1 != 0,
                    root: u16le(c, 4).unwrap_or(60),
                    beats: u32le(c, 12).unwrap_or(0),
                    meter: (u16le(c, 18).unwrap_or(4), u16le(c, 16).unwrap_or(4)),
                    bpm: f32le(c, 20).unwrap_or(0.0),
                });
            }
            _ => {}
        }
        i = body + len + (len & 1);
    }
    m.frames = data_len.checked_div(block_align).unwrap_or(0);
    m.cues.sort_unstable();
    m.cues.dedup();
    Some(m)
}

impl Markers {
    /// The loop's tempo its ACID chunk implies: its beats over its length (more exact than
    /// the tempo it writes, which is rounded) when the two agree within 2 %, else the tempo
    /// it writes. A chunk that writes no tempo (0, or not a number) gives one from its beats
    /// only for a loop — a one-shot's beats are a default, not a tempo (Acidizer writes
    /// 12 beats and tempo 0 into an 808 hit). Outside 30 … 300 BPM: none.
    pub fn bpm(&self) -> Option<f32> {
        let a = self.acid?;
        let from_beats = (a.beats > 0 && self.frames > 0 && self.rate > 0)
            .then(|| (a.beats as f64 * 60.0 * self.rate as f64 / self.frames as f64) as f32);
        let written = (a.bpm.is_finite() && a.bpm > 0.0).then_some(a.bpm);
        let bpm = match (written, from_beats) {
            (Some(w), Some(b)) if (b - w).abs() <= 0.02 * w => Some(b),
            (Some(w), _) => Some(w),
            (None, Some(b)) if !a.one_shot => Some(b),
            _ => None,
        };
        bpm.filter(|b| (30.0..=300.0).contains(b))
    }

    /// The points the markers give, in order: the cues, else the loops' starts.
    pub fn points(&self) -> Vec<u32> {
        let mut p = if self.cues.is_empty() {
            self.loops.clone()
        } else {
            self.cues.clone()
        };
        p.sort_unstable();
        p.dedup();
        p
    }
}

/// The onsets a sensitivity keeps (0 … 1): those within `40·sens + 6` dB of the strongest
/// (at 0.5, 26 dB), and never two closer than 30 ms. Slice 0 always starts at the file's
/// first frame (whatever comes before the first onset is the first slice's lead-in).
///
/// Two listenings, merged: the signal as it is, and its first difference (a high-pass
/// rising 6 dB an octave) — a closed hi-hat a few tens of dB under the kick has little
/// share of the whole's spectral flux and is missed by the first, never by the second. Each
/// onset's strength is against its own listening's strongest.
pub fn transients(mono: &[f32], rate: f32, sens: f32) -> Vec<u32> {
    transients_with(mono, rate, sens, &onset::Options::SLICER)
}

/// [`transients`] with the detector listening as `o` says (the measurement harness's).
pub fn transients_with(mono: &[f32], rate: f32, sens: f32, o: &onset::Options) -> Vec<u32> {
    ranked(mono, rate, sens, o)
        .into_iter()
        .map(|(f, _)| f)
        .collect()
}

/// [`transients`] with each onset's strength (0 … 1, against its listening's strongest; the
/// file's start 1), in order — what keeps the strongest when there are too many.
pub fn transients_ranked(mono: &[f32], rate: f32, sens: f32) -> Vec<(u32, f32)> {
    ranked(mono, rate, sens, &onset::Options::SLICER)
}

/// At most `max` of `ranked`: the file's start and the strongest of the rest, in order.
pub fn strongest(ranked: &[(u32, f32)], max: usize) -> Vec<u32> {
    if ranked.len() <= max {
        return ranked.iter().map(|(f, _)| *f).collect();
    }
    let mut rest: Vec<(u32, f32)> = ranked.iter().skip(1).copied().collect();
    rest.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    rest.truncate(max.saturating_sub(1));
    let mut out: Vec<u32> = std::iter::once(ranked[0].0)
        .chain(rest.into_iter().map(|(f, _)| f))
        .collect();
    out.sort_unstable();
    out
}

fn ranked(mono: &[f32], rate: f32, sens: f32, o: &onset::Options) -> Vec<(u32, f32)> {
    // A sample that is not a number reads as silence (one would blind the whole file).
    let clean: Vec<f32>;
    let mono = if mono.iter().all(|v| v.is_finite()) {
        mono
    } else {
        clean = mono
            .iter()
            .map(|v| if v.is_finite() { *v } else { 0.0 })
            .collect();
        &clean[..]
    };
    let range_db = 6.0 + 40.0 * sens.clamp(0.0, 1.0);
    let keep = crate::math::powf(10.0f32, -range_db / 20.0);
    let mut cand: Vec<(f64, f32)> = Vec::new();
    let high: Vec<f32> = std::iter::once(0.0)
        .chain(mono.windows(2).map(|w| w[1] - w[0]))
        .collect();
    for x in [mono, &high[..]] {
        let found = onset::detect_with(x, rate, o);
        let top = found.strengths.iter().copied().fold(0.0f32, f32::max);
        for (f, st) in found.frames.iter().zip(&found.strengths) {
            if *st > 0.0 && *st >= top * keep {
                cand.push((f.max(0.0), *st / top));
            }
        }
    }
    cand.sort_by(|a, b| a.0.total_cmp(&b.0));
    let gap = (0.03 * rate) as f64;
    let mut out: Vec<(u32, f32)> = vec![(0, 1.0)];
    for (at, st) in cand {
        let last = out.last().map_or(0.0, |l| l.0 as f64);
        if at - last < gap {
            // The first onset near the file's start becomes its start's own; of two
            // listenings' onsets for one hit, the earlier stands, the stronger strength.
            if let Some(l) = out.last_mut() {
                l.1 = l.1.max(st);
            }
            continue;
        }
        out.push((at as u32, st));
    }
    out
}

/// A grid of `steps` a beat at `bpm` across `len` frames, from `offset`.
pub fn grid(len: u32, rate: u32, bpm: f64, steps: u32, offset: u32) -> Vec<u32> {
    if rate == 0 || bpm <= 0.0 {
        return vec![0];
    }
    let step = rate as f64 * 60.0 / bpm / steps.max(1) as f64;
    let mut out = Vec::new();
    if offset > 0 {
        out.push(0);
    }
    let mut k = 0u64;
    loop {
        let f = offset as f64 + k as f64 * step;
        if f >= len as f64 {
            break;
        }
        out.push(f.round() as u32);
        k += 1;
    }
    out.dedup();
    out
}

/// A loop's tempo when nothing says it: the beat grid's over its onsets, folded into
/// 70 … 180 BPM; a loop of a whole number of bars at that tempo is preferred.
pub fn tempo(mono: &[f32], rate: f32) -> Option<f64> {
    let o = onset::detect(mono, rate);
    let g = crate::beat::analyse(&o, rate, mono.len(), 4);
    if g.bpm <= 0.0 {
        return None;
    }
    let mut bpm = g.bpm;
    while bpm < 70.0 {
        bpm *= 2.0;
    }
    while bpm > 180.0 {
        bpm /= 2.0;
    }
    // Snap to the tempo that makes the loop a whole number of beats, if one is near.
    let secs = mono.len() as f64 / rate as f64;
    let beats = (secs * bpm / 60.0).round().max(1.0);
    let exact = beats * 60.0 / secs;
    Some(if (exact - bpm).abs() / bpm < 0.04 {
        exact
    } else {
        bpm
    })
}

#[cfg(test)]
#[path = "sectio_tests.rs"]
mod tests;
