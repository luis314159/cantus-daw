//! From the stored notes and the analysed pitch to the curves a render follows: the pitch
//! shift and the formant shift by source time, the gain by output time, and the time map
//! (ADR 0054 §3). Pure arithmetic; the editor draws with the same functions the engine
//! renders with.
//!
//! **Where nothing is asked, nothing moves.** A note at its defaults has a shift of
//! exactly 0, and so has the gap between two such notes; changes ramp inside the gaps
//! beside an edited note, keeping a margin ([`margin`]) from its neighbours so the
//! renderer's 5 ms cross-fade never reaches them. Only notes sung *legato* (a gap shorter
//! than two margins) share a short transition across their boundary.

use super::{PitchEdit, PitchNote};

/// The renderer's fade (`cantus_dsp::psola::FADE_S`) in frames at `rate`, plus two: the
/// distance an edit keeps from an untouched neighbour so the neighbour stays bit for bit.
pub fn margin(rate: u32) -> u64 {
    (cantus_dsp::psola::FADE_S as f64 * rate as f64).ceil() as u64 + 2
}

/// The longest ramp between an edited note and silence or another note, seconds.
pub const RAMP_S: f64 = 0.040;
/// The transition across a legato boundary, each side, seconds (a third of a note at
/// most).
pub const LEGATO_S: f64 = 0.015;
/// The drift is the pitch smoothed by two moving averages of these lengths, seconds: their
/// zeros sit at 5 and 6 Hz, where a singer's vibrato is, so what is left is the vibrato.
pub const DRIFT_WINDOWS_S: [f64; 2] = [0.200, 0.1667];

/// The analysed pitch, per hop: frame `i` is centred on source frame `i · hop`; `midi` is
/// NaN where the frame is unvoiced.
#[derive(Clone, Copy, Debug)]
pub struct Frames<'a> {
    pub hop: usize,
    pub midi: &'a [f32],
}

impl Frames<'_> {
    /// The frames whose centres lie in `[start, end)`.
    pub fn range(&self, start: u64, end: u64) -> (usize, usize) {
        let h = self.hop.max(1) as u64;
        let a = start.div_ceil(h) as usize;
        let b = (end.div_ceil(h) as usize).min(self.midi.len());
        (a.min(b), b)
    }

    /// The sung centre of `[start, end)`: the mean pitch of its voiced frames, leaving a
    /// tenth out at each end (the transitions). `None` when nothing in it is voiced.
    pub fn centre(&self, start: u64, end: u64) -> Option<f32> {
        let (a, b) = self.range(start, end);
        let n = b - a;
        let trim = n / 10;
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for &m in &self.midi[a + trim..b - trim] {
            if m.is_finite() {
                sum += m as f64;
                count += 1;
            }
        }
        if count == 0 {
            // A short note: every voiced frame.
            for &m in &self.midi[a..b] {
                if m.is_finite() {
                    sum += m as f64;
                    count += 1;
                }
            }
        }
        (count > 0).then(|| (sum / count as f64) as f32)
    }
}

/// A note's pitch taken apart: per frame from `first`, the drift and the vibrato
/// (semitones; 0 on unvoiced frames). The sung pitch is `sung + drift + vibrato`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parts {
    pub first: usize,
    pub drift: Vec<f32>,
    pub vibrato: Vec<f32>,
}

/// Take a note's pitch apart (see [`Parts`]).
pub fn decompose(note: &PitchNote, f: &Frames, rate: u32) -> Parts {
    let (a, b) = f.range(note.src_start, note.src_end);
    let dev: Vec<f32> = f.midi[a..b]
        .iter()
        .map(|m| {
            if m.is_finite() {
                m - note.sung
            } else {
                f32::NAN
            }
        })
        .collect();
    let hop_s = f.hop.max(1) as f64 / rate.max(1) as f64;
    let mut drift = dev.clone();
    for w in DRIFT_WINDOWS_S {
        let len = ((w / hop_s).round() as usize).max(1);
        drift = moving_average(&drift, len);
    }
    let mut vibrato = vec![0.0; dev.len()];
    for i in 0..dev.len() {
        if dev[i].is_finite() {
            vibrato[i] = dev[i] - drift[i];
        } else {
            drift[i] = 0.0;
        }
    }
    Parts {
        first: a,
        drift,
        vibrato,
    }
}

/// A centred moving average of `len` frames over the finite values (NaN stays NaN; the
/// window shrinks at the ends and over unvoiced frames).
fn moving_average(x: &[f32], len: usize) -> Vec<f32> {
    let half = len / 2;
    let mut out = vec![f32::NAN; x.len()];
    for (i, o) in out.iter_mut().enumerate() {
        if !x[i].is_finite() {
            continue;
        }
        let lo = i.saturating_sub(half);
        let hi = (i + len - half).min(x.len());
        let (mut s, mut n) = (0.0f64, 0usize);
        for v in &x[lo..hi] {
            if v.is_finite() {
                s += *v as f64;
                n += 1;
            }
        }
        *o = (s / n.max(1) as f64) as f32;
    }
    out
}

/// Half-open spans in order and a rule for the curve between them (see the module docs).
struct Spans {
    starts: Vec<f64>,
    ends: Vec<f64>,
    len: f64,
    margin: f64,
    ramp: f64,
    legato: f64,
}

impl Spans {
    fn short_gap(&self, a: f64, b: f64) -> bool {
        b - a < 2.0 * self.margin + 1.0
    }

    /// The curve at `t`, given each span's own value at a time inside it.
    fn eval(&self, t: f64, own: &dyn Fn(usize, f64) -> f32) -> f32 {
        let n = self.starts.len();
        let k = self.starts.partition_point(|s| *s <= t);
        let left = |i: usize| own(i, self.starts[i]);
        let right = |i: usize| own(i, (self.ends[i] - 1.0).max(self.starts[i]));
        if k > 0 && t < self.ends[k - 1] {
            // Inside span i: its own value, blended toward a legato neighbour's at a
            // shared boundary.
            let i = k - 1;
            let mut v = own(i, t);
            let r = self.legato.min((self.ends[i] - self.starts[i]) / 3.0);
            if r > 0.0 {
                if i > 0 && self.short_gap(self.ends[i - 1], self.starts[i]) {
                    let d = t - self.starts[i];
                    if d < r {
                        let mid = 0.5 * (right(i - 1) + left(i));
                        v = mid + (v - mid) * (d / r) as f32;
                    }
                }
                if i + 1 < n && self.short_gap(self.ends[i], self.starts[i + 1]) {
                    let d = self.ends[i] - t;
                    if d < r {
                        let mid = 0.5 * (right(i) + left(i + 1));
                        v = mid + (v - mid) * (d / r) as f32;
                    }
                }
            }
            return v;
        }
        // In the gap after span k-1 (or the take's start) and before span k (or its end).
        let (a, sa) = if k > 0 {
            (self.ends[k - 1], right(k - 1))
        } else {
            (0.0, 0.0)
        };
        let (b, sb) = if k < n {
            (self.starts[k], left(k))
        } else {
            (self.len, 0.0)
        };
        if sa == 0.0 && sb == 0.0 {
            return 0.0;
        }
        if self.short_gap(a, b) {
            return if k > 0 && k < n {
                0.5 * (sa + sb)
            } else {
                sa + sb
            };
        }
        let (lo, hi) = (a + self.margin, b - self.margin);
        let lerp = |x: f64, x0: f64, x1: f64, y0: f32, y1: f32| -> f32 {
            if x <= x0 {
                y0
            } else if x >= x1 {
                y1
            } else {
                y0 + (y1 - y0) * ((x - x0) / (x1 - x0)) as f32
            }
        };
        if hi - lo <= 2.0 * self.ramp {
            lerp(t, lo, hi, sa, sb)
        } else if t < lo + self.ramp {
            lerp(t, lo, lo + self.ramp, sa, 0.0)
        } else if t > hi - self.ramp {
            lerp(t, hi - self.ramp, hi, 0.0, sb)
        } else {
            0.0
        }
    }
}

/// The curves of one edit over one analysis (built once per render or per drawing).
#[derive(Debug)]
pub struct Curves {
    hop: f64,
    rate: u32,
    parts: Vec<Parts>,
    offsets: Vec<f32>,
    edited: Vec<bool>,
    drift: Vec<f32>,
    vibrato: Vec<f32>,
    formants: Vec<f32>,
    gains: Vec<f32>,
    src: Spans,
    out: Spans,
}

impl std::fmt::Debug for Spans {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Spans({})", self.starts.len())
    }
}

impl Curves {
    pub fn new(edit: &PitchEdit, frames: &Frames) -> Self {
        let rate = edit.rate.max(1);
        let spans = |r: &dyn Fn(&PitchNote) -> (u64, u64)| Spans {
            starts: edit.notes.iter().map(|n| r(n).0 as f64).collect(),
            ends: edit.notes.iter().map(|n| r(n).1 as f64).collect(),
            len: edit.frames as f64,
            margin: margin(rate) as f64,
            ramp: RAMP_S * rate as f64,
            legato: LEGATO_S * rate as f64,
        };
        Curves {
            hop: frames.hop.max(1) as f64,
            rate,
            parts: edit
                .notes
                .iter()
                .map(|n| {
                    if n.drift != 1.0 || n.vibrato != 1.0 {
                        decompose(n, frames, rate)
                    } else {
                        Parts::default()
                    }
                })
                .collect(),
            offsets: edit.notes.iter().map(PitchNote::offset).collect(),
            edited: edit.notes.iter().map(PitchNote::pitch_edited).collect(),
            drift: edit.notes.iter().map(|n| n.drift - 1.0).collect(),
            vibrato: edit.notes.iter().map(|n| n.vibrato - 1.0).collect(),
            formants: edit.notes.iter().map(|n| n.formant).collect(),
            gains: edit.notes.iter().map(|n| n.gain_db).collect(),
            src: spans(&|n| (n.src_start, n.src_end)),
            out: spans(&|n| (n.start, n.end)),
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Note `i`'s own shift at source time `t` (semitones).
    fn note_shift(&self, i: usize, t: f64) -> f32 {
        if !self.edited[i] {
            return 0.0;
        }
        let mut s = self.offsets[i];
        let p = &self.parts[i];
        if !p.drift.is_empty() {
            let x = (t / self.hop - p.first as f64).clamp(0.0, (p.drift.len() - 1) as f64);
            let j = x.floor() as usize;
            let k = (j + 1).min(p.drift.len() - 1);
            let w = (x - j as f64) as f32;
            let at = |v: &[f32]| v[j] + (v[k] - v[j]) * w;
            s += self.drift[i] * at(&p.drift) + self.vibrato[i] * at(&p.vibrato);
        }
        s
    }

    /// The pitch shift at source time `t`, semitones: exactly 0 wherever nothing is asked.
    pub fn shift_at(&self, t: f64) -> f32 {
        self.src.eval(t, &|i, t| self.note_shift(i, t))
    }

    /// The formant shift at source time `t`, semitones.
    pub fn formant_at(&self, t: f64) -> f32 {
        self.src.eval(t, &|i, _| self.formants[i])
    }

    /// The gain at output time `t`, dB: inside a note its gain, ramped over the margin at
    /// its edges (inside it), 0 elsewhere.
    pub fn gain_db_at(&self, t: f64) -> f32 {
        let k = self.out.starts.partition_point(|s| *s <= t);
        if k == 0 || t >= self.out.ends[k - 1] {
            return 0.0;
        }
        let i = k - 1;
        let g = self.gains[i];
        if g == 0.0 {
            return 0.0;
        }
        let (a, b) = (self.out.starts[i], self.out.ends[i]);
        let r = self.out.margin.min((b - a) / 4.0).max(1.0);
        let edge = |gap_short: bool, other: f32| if gap_short { 0.5 * (g + other) } else { 0.0 };
        let left = if i > 0 {
            edge(
                self.out.short_gap(self.out.ends[i - 1], a),
                self.gains[i - 1],
            )
        } else {
            0.0
        };
        let right = if i + 1 < self.gains.len() {
            edge(
                self.out.short_gap(b, self.out.starts[i + 1]),
                self.gains[i + 1],
            )
        } else {
            0.0
        };
        if t - a < r {
            left + (g - left) * ((t - a) / r) as f32
        } else if b - t < r {
            right + (g - right) * ((b - t) / r) as f32
        } else {
            g
        }
    }

    /// Whether any note has a gain.
    pub fn has_gain(&self) -> bool {
        self.gains.iter().any(|g| *g != 0.0)
    }
}

/// The time map's breakpoints `(output, source)` for an edit: none when no note moved
/// (the identity). Every note's edges are anchors; a gap whose two sides moved by
/// different amounts is warped between margins kept next to each side, so the notes
/// beside it are read sample for sample.
pub fn time_points(edit: &PitchEdit) -> Vec<(f64, f64)> {
    if !edit.notes.iter().any(PitchNote::moved) {
        return Vec::new();
    }
    let m = margin(edit.rate) as i64;
    let len = edit.frames as i64;
    // Anchors in order: the take's start, each note's two edges, the take's end.
    let mut anchors: Vec<(i64, i64)> = vec![(0, 0)];
    for n in &edit.notes {
        anchors.push((n.start as i64, n.src_start as i64));
        anchors.push((n.end as i64, n.src_end as i64));
    }
    anchors.push((len, len));
    let mut pts: Vec<(i64, i64)> = Vec::new();
    for (i, w) in anchors.windows(2).enumerate() {
        let ((o1, s1), (o2, s2)) = (w[0], w[1]);
        pts.push((o1, s1));
        // Even windows are gaps (start → first note, note end → next start, …).
        let gap = i % 2 == 0;
        if gap && o1 - s1 != o2 - s2 && o2 - o1 > 2 * m && s2 - s1 > 2 * m {
            pts.push((o1 + m, s1 + m));
            pts.push((o2 - m, s2 - m));
        }
    }
    pts.push(*anchors.last().expect("two anchors at least"));
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for (o, s) in pts {
        match out.last() {
            Some(&(lo, ls)) if (o as f64) <= lo => {
                debug_assert!(s as f64 <= ls || o as f64 == lo);
            }
            _ => out.push((o as f64, s as f64)),
        }
    }
    out
}
