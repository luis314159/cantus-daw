//! **Basic Pitch's note creation, in Rust** (round 34, ADR 0086): from the network's three
//! posteriorgrams — *note* (88 semitones, A0–C8), *onset* (the same) and *contour* (264
//! bins, a third of a semitone each) at 86 frames a second — to notes with their pitch
//! bends, as `basic_pitch/note_creation.py` (Spotify, Apache-2.0, commit fa5997af) makes
//! them, decision for decision: onsets inferred from the note posteriors' rises and joined
//! to the predicted ones, onset peaks walked forward while the note's energy stays above
//! the frame threshold (eleven frames of tolerance), the energy taken away with its two
//! neighbours, then the "Melodia trick" (the loudest energy left grown both ways into a
//! note until nothing above the threshold remains); bends as the contour's peak near the
//! note under a Gaussian. The arithmetic follows NumPy's types where they decide a result
//! (`float64` for the inferred onsets and the remaining energy, `float32` with NumPy's
//! pairwise summation for a note's mean, ties rounded to even), so a note list equals
//! Basic Pitch's own on the same posteriorgrams (`tests/transcribe_reference.rs`).
//!
//! The one change is speed, not result: Basic Pitch looks for the largest remaining energy
//! by scanning the whole matrix once per note (quadratic in a song's length); here a heap
//! of the cells above the threshold gives the same cell — the largest, the first in
//! row-major order among equals — because energy only ever falls to zero.

use cantus_dsp::math;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// The network's sample rate.
pub const SAMPLE_RATE: u32 = 22_050;
/// Samples between two frames.
pub const FFT_HOP: usize = 256;
/// Samples one inference takes (`AUDIO_N_SAMPLES`: two seconds less a hop).
pub const WINDOW: usize = 43_844;
/// Frames one inference gives (`ANNOT_N_FRAMES`).
pub const WINDOW_FRAMES: usize = 172;
/// Frames a second, as Basic Pitch counts them (`22050 // 256`).
pub const FPS: usize = 86;
/// Frames two consecutive windows share; half of them is dropped at each side.
pub const OVERLAP_FRAMES: usize = 30;
/// Semitone bins of the note and onset posteriorgrams.
pub const NOTES: usize = 88;
/// Bins of the contour posteriorgram (three a semitone).
pub const CONTOURS: usize = 264;
/// MIDI note of bin 0 (A0).
pub const MIDI_OFFSET: u8 = 21;
const MAX_FREQ_IDX: usize = NOTES - 1;
/// Frames under the threshold a note survives (`energy_tol`).
const ENERGY_TOL: usize = 11;
/// Contour bins either side of a note's pitch searched for its bend.
const BEND_TOLERANCE: usize = 25;

/// The three posteriorgrams of a whole file, row-major (`frames × bins`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Posteriors {
    pub frames: usize,
    pub note: Vec<f32>,
    pub onset: Vec<f32>,
    pub contour: Vec<f32>,
}

/// What `predict` takes, with its defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteParams {
    /// An onset peak must reach this (Basic Pitch's 0.5).
    pub onset_threshold: f64,
    /// A note lasts while its energy stays above this (0.3).
    pub frame_threshold: f64,
    /// Notes of this many frames or fewer are dropped (11: 127.7 ms).
    pub min_note_frames: usize,
    pub min_hz: Option<f64>,
    pub max_hz: Option<f64>,
    pub infer_onsets: bool,
    pub melodia_trick: bool,
}

impl Default for NoteParams {
    fn default() -> Self {
        NoteParams {
            onset_threshold: 0.5,
            frame_threshold: 0.3,
            min_note_frames: min_note_frames(127.70),
            min_hz: None,
            max_hz: None,
            infer_onsets: true,
            melodia_trick: true,
        }
    }
}

/// `int(np.round(ms / 1000 * (22050 / 256)))`.
pub fn min_note_frames(ms: f64) -> usize {
    (ms / 1000.0 * (SAMPLE_RATE as f64 / FFT_HOP as f64))
        .round_ties_even()
        .max(0.0) as usize
}

/// A note in frames: `start .. end`, its MIDI pitch, its mean note posterior.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameNote {
    pub start: usize,
    pub end: usize,
    pub pitch: u8,
    pub amplitude: f32,
}

/// Where frame `f` of the unwrapped posteriorgram sits, seconds (`model_frames_to_time`,
/// its alignment constant included).
pub fn frame_time(f: usize) -> f64 {
    let window_offset = (FFT_HOP as f64 / SAMPLE_RATE as f64)
        * (WINDOW_FRAMES as f64 - (WINDOW as f64 / FFT_HOP as f64))
        + 0.0018;
    (f * FFT_HOP) as f64 / SAMPLE_RATE as f64 - window_offset * (f / WINDOW_FRAMES) as f64
}

/// `int(np.round(127 * amplitude))`, in `float32` as NumPy 2 computes it.
pub fn velocity(amplitude: f32) -> i32 {
    (127.0f32 * amplitude).round_ties_even() as i32
}

/// NumPy's pairwise summation of `float32` values (`pairwise_sum_FLOAT`), from zero.
pub fn pairwise_sum(a: &[f32]) -> f32 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0f32;
        for &x in a {
            res += x;
        }
        res
    } else if n <= 128 {
        let mut r = [0.0f32; 8];
        r.copy_from_slice(&a[..8]);
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_sum(&a[..n2]) + pairwise_sum(&a[n2..])
    }
}

/// `np.mean` of a column's frames `a .. b` (`float32`).
fn column_mean(m: &[f32], width: usize, a: usize, b: usize, col: usize) -> f32 {
    let v: Vec<f32> = (a..b).map(|t| m[t * width + col]).collect();
    pairwise_sum(&v) / (b - a) as f32
}

/// `librosa.hz_to_midi`.
fn hz_to_midi(hz: f64) -> f64 {
    12.0 * (math::log2(hz) - math::log2(440.0)) + 69.0
}

/// `constrain_frequency`: bins outside `[min, max)` silenced in both matrices.
fn constrain(onset: &mut [f32], note: &mut [f32], n: usize, p: &NoteParams) {
    let mut zero = |from: usize, to: usize| {
        for t in 0..n {
            for f in from..to.min(NOTES) {
                onset[t * NOTES + f] = 0.0;
                note[t * NOTES + f] = 0.0;
            }
        }
    };
    if let Some(hz) = p.max_hz {
        let i = (hz_to_midi(hz) - MIDI_OFFSET as f64).round_ties_even();
        zero(i.clamp(0.0, NOTES as f64) as usize, NOTES);
    }
    if let Some(hz) = p.min_hz {
        let i = (hz_to_midi(hz) - MIDI_OFFSET as f64).round_ties_even();
        zero(0, i.clamp(0.0, NOTES as f64) as usize);
    }
}

/// `get_infered_onsets`: the larger of the predicted onset and the note posterior's rise
/// (the smaller of its rise over one and over two frames, scaled to the onsets' top).
/// `float64`, as NumPy promotes it; all NaN when the posterior never rises (Basic Pitch
/// divides by that zero, and then finds no onset at all).
pub fn infer_onsets(onset: &[f32], note: &[f32], n: usize) -> Vec<f64> {
    let at = |t: isize, f: usize| -> f64 {
        if t < 0 {
            0.0
        } else {
            note[t as usize * NOTES + f] as f64
        }
    };
    let mut diff = vec![0.0f64; n * NOTES];
    for t in 0..n {
        for f in 0..NOTES {
            let now = at(t as isize, f);
            let d = (now - at(t as isize - 1, f)).min(now - at(t as isize - 2, f));
            diff[t * NOTES + f] = if t < 2 || d < 0.0 { 0.0 } else { d };
        }
    }
    let top_onset = onset.iter().fold(f32::NEG_INFINITY, |m, v| m.max(*v)) as f64;
    let top_diff = diff.iter().fold(f64::NEG_INFINITY, |m, v| m.max(*v));
    diff.iter()
        .zip(onset)
        .map(|(d, o)| {
            let scaled = top_onset * d / top_diff;
            if scaled.is_nan() {
                f64::NAN
            } else {
                (*o as f64).max(scaled)
            }
        })
        .collect()
}

/// A remaining-energy cell for the heap: the largest value first, then the first in
/// row-major order (`np.argmax`).
#[derive(PartialEq)]
struct Cell(f64, usize);

impl Eq for Cell {}

impl PartialOrd for Cell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cell {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0).then(other.1.cmp(&self.1))
    }
}

/// `output_to_notes_polyphonic`: the notes, in Basic Pitch's order (the onset pass from
/// the last onset back, then the Melodia trick's, loudest first). `post.note` and
/// `post.onset` are taken as given; the frequency limits are applied to copies.
pub fn notes(post: &Posteriors, p: &NoteParams) -> Vec<FrameNote> {
    let n = post.frames;
    if n == 0 {
        return Vec::new();
    }
    let mut note = post.note.clone();
    let mut onset32 = post.onset.clone();
    constrain(&mut onset32, &mut note, n, p);
    let onset: Vec<f64> = if p.infer_onsets {
        infer_onsets(&onset32, &note, n)
    } else {
        onset32.iter().map(|v| *v as f64).collect()
    };
    // The onset peaks (`argrelmax` along time: strictly above both neighbours; the edges,
    // compared with themselves, never are) at or above the threshold, latest first and,
    // within a frame, highest first.
    let mut peaks: Vec<(usize, usize)> = Vec::new();
    for t in 1..n.saturating_sub(1) {
        for f in 0..NOTES {
            let v = onset[t * NOTES + f];
            if v > onset[(t - 1) * NOTES + f]
                && v > onset[(t + 1) * NOTES + f]
                && v >= p.onset_threshold
            {
                peaks.push((t, f));
            }
        }
    }
    peaks.reverse();
    let mut energy: Vec<f64> = note.iter().map(|v| *v as f64).collect();
    let thresh = p.frame_threshold;
    let mut out = Vec::new();
    for (start, f) in peaks {
        if start >= n - 1 {
            continue;
        }
        let mut i = start + 1;
        let mut k = 0usize;
        while i < n - 1 && k < ENERGY_TOL {
            if energy[i * NOTES + f] < thresh {
                k += 1;
            } else {
                k = 0;
            }
            i += 1;
        }
        i -= k;
        if i - start <= p.min_note_frames {
            continue;
        }
        for t in start..i {
            clear(&mut energy, t, f);
        }
        out.push(FrameNote {
            start,
            end: i,
            pitch: f as u8 + MIDI_OFFSET,
            amplitude: column_mean(&note, NOTES, start, i, f),
        });
    }
    if p.melodia_trick {
        melodia(&mut energy, &note, n, p, &mut out);
    }
    out
}

/// A cell's energy and its two neighbours' taken away.
fn clear(energy: &mut [f64], t: usize, f: usize) {
    energy[t * NOTES + f] = 0.0;
    if f < MAX_FREQ_IDX {
        energy[t * NOTES + f + 1] = 0.0;
    }
    if f > 0 {
        energy[t * NOTES + f - 1] = 0.0;
    }
}

/// The Melodia trick: while energy above the threshold remains, the largest left is grown
/// forward and back into a note, its energy (and its neighbours') taken as it goes.
fn melodia(energy: &mut [f64], note: &[f32], n: usize, p: &NoteParams, out: &mut Vec<FrameNote>) {
    let thresh = p.frame_threshold;
    let mut heap: BinaryHeap<Cell> = energy
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > thresh)
        .map(|(i, v)| Cell(*v, i))
        .collect();
    while let Some(Cell(v, idx)) = heap.pop() {
        // A cell cleared since it was pushed is no longer what it was (energy only falls,
        // to zero); the heap's top among live cells is `np.argmax`'s answer.
        if energy[idx] != v {
            continue;
        }
        let (mid, f) = (idx / NOTES, idx % NOTES);
        energy[idx] = 0.0;
        let mut i = mid + 1;
        let mut k = 0usize;
        while i < n - 1 && k < ENERGY_TOL {
            if energy[i * NOTES + f] < thresh {
                k += 1;
            } else {
                k = 0;
            }
            clear(energy, i, f);
            i += 1;
        }
        // `i - 1 - k` in Python's integers: -1 when the note sits on the last frame.
        let end = i as isize - 1 - k as isize;
        let mut i = mid as isize - 1;
        let mut k = 0usize;
        while i > 0 && k < ENERGY_TOL {
            let t = i as usize;
            if energy[t * NOTES + f] < thresh {
                k += 1;
            } else {
                k = 0;
            }
            clear(energy, t, f);
            i -= 1;
        }
        let start = i + 1 + k as isize;
        if end - start <= p.min_note_frames as isize {
            continue;
        }
        let (start, end) = (start as usize, end as usize);
        out.push(FrameNote {
            start,
            end,
            pitch: f as u8 + MIDI_OFFSET,
            amplitude: column_mean(note, NOTES, start, end, f),
        });
    }
}

/// `scipy.signal.windows.gaussian(51, std=5)`.
fn gaussian() -> [f64; 2 * BEND_TOLERANCE + 1] {
    let mut w = [0.0; 2 * BEND_TOLERANCE + 1];
    let sig2 = 2.0 * 5.0 * 5.0;
    for (j, v) in w.iter_mut().enumerate() {
        let x = j as f64 - BEND_TOLERANCE as f64;
        *v = math::exp(-(x * x) / sig2);
    }
    w
}

/// `get_pitch_bends`: for each frame of the note, the contour bin with the most energy
/// within 25 bins of its pitch under a Gaussian, in thirds of a semitone from the pitch.
pub fn pitch_bends(contour: &[f32], notes: &[FrameNote]) -> Vec<Vec<i32>> {
    let g = gaussian();
    let tol = BEND_TOLERANCE as isize;
    notes
        .iter()
        .map(|nt| {
            // `round(36 · log2(hz / 27.5))` is exactly three bins a semitone from A0.
            let fi = 3 * (nt.pitch as isize - MIDI_OFFSET as isize);
            let lo = (fi - tol).max(0) as usize;
            let hi = (fi + tol + 1).min(CONTOURS as isize) as usize;
            let g0 = (tol - fi).max(0) as usize;
            let shift = (tol - (tol - fi).max(0)) as i32;
            (nt.start..nt.end)
                .map(|t| {
                    let mut best = 0usize;
                    let mut top = f64::NEG_INFINITY;
                    for (j, b) in (lo..hi).enumerate() {
                        let v = contour[t * CONTOURS + b] as f64 * g[g0 + j];
                        if v > top {
                            top = v;
                            best = j;
                        }
                    }
                    best as i32 - shift
                })
                .collect()
        })
        .collect()
}

/// `drop_overlapping_pitch_bends`'s rule: whether each note overlaps another in time (its
/// bends are dropped when one MIDI channel carries every note).
pub fn overlapping(notes: &[(f64, f64)]) -> Vec<bool> {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|a, b| {
        notes[*a]
            .0
            .total_cmp(&notes[*b].0)
            .then(notes[*a].1.total_cmp(&notes[*b].1))
    });
    let mut out = vec![false; notes.len()];
    for x in 0..order.len() {
        for y in x + 1..order.len() {
            let (i, j) = (order[x], order[y]);
            if notes[j].0 >= notes[i].1 {
                break;
            }
            out[i] = true;
            out[j] = true;
        }
    }
    out
}

/// Every window's posteriorgram joined into the whole file's (`unwrap_output`): half the
/// overlap dropped at each side of every window, the rest trimmed to the file's length.
/// `windows` holds each window's `WINDOW_FRAMES × bins` matrix.
pub fn unwrap(windows: &[Vec<f32>], bins: usize, samples: usize) -> Vec<f32> {
    let keep = kept_frames(samples);
    let mut out = Vec::with_capacity(keep * bins);
    for w in windows {
        join(&mut out, w, bins, keep);
    }
    out
}

/// The frames a file of `samples` keeps: `floor(samples · 86 / 22050)`.
pub fn kept_frames(samples: usize) -> usize {
    (samples as f64 * (FPS as f64 / SAMPLE_RATE as f64)).floor() as usize
}

/// One window's frames joined onto `out` as [`unwrap`] joins them (half the overlap
/// dropped at each side), never past `keep` frames — a window at a time, so the windows
/// need not be kept.
pub fn join(out: &mut Vec<f32>, window: &[f32], bins: usize, keep: usize) {
    let half = OVERLAP_FRAMES / 2;
    let rows = &window[half * bins..(WINDOW_FRAMES - half) * bins];
    let room = (keep * bins).saturating_sub(out.len());
    out.extend_from_slice(&rows[..rows.len().min(room)]);
}

/// Samples between two windows' starts.
pub fn window_hop() -> usize {
    WINDOW - OVERLAP_FRAMES * FFT_HOP
}

/// How many windows a file of `samples` takes (the file preceded by half the overlap of
/// silence, windows a hop apart, as long as one starts inside it).
pub fn window_count(samples: usize) -> usize {
    let padded = samples + OVERLAP_FRAMES * FFT_HOP / 2;
    padded.div_ceil(window_hop()).max(1)
}

/// Window `k` of a file: `WINDOW` samples of the padded file, zeros past its end.
pub fn window(x: &[f32], k: usize, out: &mut [f32]) {
    let pad = OVERLAP_FRAMES * FFT_HOP / 2;
    let start = k * window_hop();
    for (i, o) in out.iter_mut().enumerate() {
        let at = start + i;
        *o = if at >= pad && at - pad < x.len() {
            x[at - pad]
        } else {
            0.0
        };
    }
}
