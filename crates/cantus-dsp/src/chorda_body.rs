//! Chorda's second model's body and room (ADR 0098): what the strings' bridge force becomes
//! on its way to a listener at the keyboard.
//!
//! **The body** is a pair of radiation impulse responses — the soundboard, rim and lid as
//! heard by two microphones over the strings, the player's perspective — convolved with the
//! bridge force panned by the keys' places (`conv`). Each response is synthesised once, off
//! the audio thread and deterministically: a soundboard's dense, irregular modes (their
//! density, loss factors and radiation after Conklin 1996 and Berthaut, Ichchou & Jézéquel
//! 2003; amplitudes and phases hashed per microphone, so the two sides decorrelate as two
//! real microphones do), the board's broadband "knock" (the first milliseconds of its
//! response, where a hammer blow excites everything at once), and a direct path. It is a
//! filter after the strings — never a source of their coefficients (ADR 0059 §patents).
//!
//! **The room** is algorithmic: early reflections from a tapped line (a different pattern
//! per side), then an eight-line feedback delay network with a Householder matrix and a
//! frequency-dependent decay per line (Jot & Chaigne 1991). Its states are flushed to zero
//! on the instrument's grid once its input has been silent for longer than its lines, so the
//! instrument reaches an exact rest.

use super::conv::{Convolver, Prepared};
use crate::math;
use std::sync::{Arc, OnceLock};

/// A small, fixed variation (−1…1) hashed from `i` and a seed.
fn hash(i: u64, seed: u64) -> f64 {
    let mut z = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ seed.wrapping_mul(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// How a body's response is made: every number a voicing pass may move, in one place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyVoicing {
    /// The response's length (s).
    pub length: f64,
    /// The board's T60 (s) at 100 Hz and at 1 kHz; between and beyond, a power law.
    pub t60_100: f64,
    pub t60_1k: f64,
    /// The lowest mode (Hz) and the modes' mean spacing (Hz) above `dense_hz`, and below it.
    pub lowest: f64,
    pub spacing: f64,
    pub spacing_low: f64,
    pub dense_hz: f64,
    /// The radiation's rise: first order from `rise_hz` (below it falls 6 dB/oct), then a
    /// fall from `fall_hz` (`fall_db` per octave), cut above `top_hz`.
    pub rise_hz: f64,
    pub fall_hz: f64,
    pub fall_db: f64,
    pub top_hz: f64,
    /// The direct path's share (the strings' and the bridge's own radiation, at t = 0).
    pub direct: f64,
    /// The low modes' weight: their amplitudes raised by `low_db` below `low_hz` (a grand's
    /// board is large: its lowest modes radiate the bass's fundamentals strongly).
    pub low_db: f64,
    pub low_hz: f64,
    /// Voicing pass two (round 37, the owner's ABX): the response's level per octave
    /// (dB at 63 Hz … 8 kHz, log-interpolated, flat beyond) over the radiation — a
    /// technician's voicing of the board, measured against the reference piano.
    pub eq: [f64; 8],
    /// 1: each third-octave's modes are scaled so the band carries the radiation's energy
    /// (a random draw of a few modes no longer makes a band honk); 0: the raw draw.
    pub even: f64,
    /// The microphones' coherence corner (Hz): below it the two sides hear the same modes
    /// (a solid centre), above it they part as a spaced pair's do; 0: independent.
    pub coh_hz: f64,
}

impl Default for BodyVoicing {
    fn default() -> Self {
        BodyVoicing {
            length: 0.30,
            t60_100: 0.12,
            t60_1k: 0.224,
            lowest: 48.0,
            spacing: 22.0,
            spacing_low: 14.0,
            dense_hz: 400.0,
            rise_hz: 39.5,
            fall_hz: 2_496.0,
            fall_db: 1.5,
            top_hz: 12_600.0,
            direct: 0.25,
            low_db: 6.0,
            low_hz: 150.0,
            eq: [-5.4, 3.45, -0.6, 4.3, -0.8, 2.76, 4.6, -3.56],
            even: 1.0,
            coh_hz: 200.0,
        }
    }
}

/// The board's T60 (s) at `f` Hz.
fn t60(v: &BodyVoicing, f: f64) -> f64 {
    let slope = math::log10(v.t60_1k / v.t60_100); // per decade
    (v.t60_100 * math::powf(10.0f64, slope * math::log10(f / 100.0))).clamp(0.02, 2.0)
}

/// The radiation's gain (linear) at `f`.
pub fn radiation(v: &BodyVoicing, f: f64) -> f64 {
    let rise = f / (f * f + v.rise_hz * v.rise_hz).sqrt();
    let fall = if f > v.fall_hz {
        math::powf(10.0f64, -v.fall_db * math::log2(f / v.fall_hz) / 20.0)
    } else {
        1.0
    };
    let top = 1.0 / (1.0 + math::powf(f / v.top_hz, 4.0));
    let low = 1.0
        + (math::powf(10.0f64, v.low_db / 20.0) - 1.0) / (1.0 + (f / v.low_hz) * (f / v.low_hz));
    rise * fall * top * low
}

/// The octaves the voicing's `eq` is given at (Hz).
pub const EQ_HZ: [f64; 8] = [
    63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0,
];

/// The voicing's `eq` (linear) at `f`: log-interpolated between its octaves, flat beyond.
pub fn eq_gain(v: &BodyVoicing, f: f64) -> f64 {
    let x = (math::log2(f.max(1.0) / EQ_HZ[0])).clamp(0.0, 7.0);
    let i = (x as usize).min(6);
    let t = x - i as f64;
    let d = v.eq[i] + t * (v.eq[i + 1] - v.eq[i]);
    math::powf(10.0f64, d / 20.0)
}

/// The body's resonant response for microphone `mic` (0 left, 1 right) at `fs`, without the
/// direct path: energy `(1 − direct)²`.
pub fn response(v: &BodyVoicing, fs: f64, mic: u64) -> Vec<f64> {
    let [l, r] = responses(v, fs);
    if mic == 0 {
        l
    } else {
        r
    }
}

/// Both microphones' responses at once: each mode's frequency and decay are the board's,
/// shared, so one recursion serves both; only its amplitude and phase are each
/// microphone's.
pub fn responses(v: &BodyVoicing, fs: f64) -> [Vec<f64>; 2] {
    // At most the frames 48 kHz gives the length: the convolution's work per sample stays
    // what it is at 48 kHz at any rate (its cost grows with the rate, as the voices', never
    // with its square); above 48 kHz the board's last ring is the shorter for it.
    let len = ((v.length * fs.min(48_000.0)) as usize).max(1);
    let mut h = [vec![0.0f64; len], vec![0.0f64; len]];
    let nyq = 0.45 * fs;
    // The modes: frequency, decay, and per microphone an amplitude and a phase.
    struct Mode {
        f: f64,
        decay: f64,
        amp: [f64; 2],
        ph: [f64; 2],
    }
    let mut modes: Vec<Mode> = Vec::new();
    let mut f = v.lowest;
    let mut i = 0u64;
    while f < v.top_hz.min(nyq) {
        let decay = 6.907_755 / t60(v, f);
        // A Rayleigh-like amplitude and a random phase per mode and microphone; with a
        // coherence corner, the right side shares the left's draw below it (its own above).
        let draw = |seed: u64| {
            let u = 0.5 * (hash(i, seed) + 1.0);
            (
                (-2.0 * math::ln(u.max(1e-6))).sqrt(),
                std::f64::consts::PI * hash(i, seed ^ 0x5A5A),
            )
        };
        let (a0, p0) = draw(0xB0D7);
        let (a1, p1) = draw(0xB0D8);
        let (a1, p1) = if v.coh_hz > 0.0 {
            let c = 1.0 / (1.0 + (f / v.coh_hz) * (f / v.coh_hz));
            (
                c * a0 + (1.0 - c * c).sqrt() * a1,
                p0 + (1.0 - c) * (p1 - p0),
            )
        } else {
            (a1, p1)
        };
        let g = radiation(v, f) * eq_gain(v, f);
        modes.push(Mode {
            f,
            decay,
            amp: [a0 * g, a1 * g],
            ph: [p0, p1],
        });
        let spacing = if f < v.dense_hz {
            v.spacing_low
        } else {
            v.spacing
        };
        f += spacing * (1.0 + 0.6 * hash(i, 0xF00D));
        i += 1;
    }
    // Even bands: each third-octave's modes carry, per microphone, what their mean draw
    // would (E[amp²] = 2 for the Rayleigh draw) at the band's own radiation.
    if v.even > 0.0 {
        let mut lo = 0;
        while lo < modes.len() {
            let top = modes[lo].f * 1.259_921;
            let hi = modes[lo..]
                .iter()
                .position(|m| m.f >= top)
                .map_or(modes.len(), |k| lo + k);
            for mic in 0..2 {
                let (mut got, mut want) = (0.0, 0.0);
                for m in &modes[lo..hi] {
                    let g = radiation(v, m.f) * eq_gain(v, m.f);
                    got += m.amp[mic] * m.amp[mic];
                    want += 2.0 * g * g;
                }
                let k = 1.0 + v.even * ((want / got.max(1e-30)).sqrt() - 1.0);
                for m in &mut modes[lo..hi] {
                    m.amp[mic] *= k;
                }
            }
            lo = hi;
        }
    }
    for m in &modes {
        let w = std::f64::consts::TAU * m.f / fs;
        // Each mode's energy is spread over its ring: scaled by its decay so a short mode is
        // not quieter in total than a long one (the board's mobility is what is flat).
        let gain: [(f64, f64); 2] = std::array::from_fn(|mic| {
            let a = m.amp[mic] * (2.0 * m.decay / fs).sqrt();
            // e^{−dt} sin(wt + φ) = e^{−dt} (sin wt cos φ + cos wt sin φ).
            (a * math::cos(m.ph[mic]), a * math::sin(m.ph[mic]))
        });
        let r = math::exp(-m.decay / fs);
        let (c, s) = (math::cos(w), math::sin(w));
        let (mut re, mut im) = (1.0, 0.0);
        let mut env = 1.0;
        let [hl, hr] = &mut h;
        for (xl, xr) in hl.iter_mut().zip(hr.iter_mut()) {
            let (es, ec) = (env * im, env * re);
            *xl += gain[0].0 * es + gain[0].1 * ec;
            *xr += gain[1].0 * es + gain[1].1 * ec;
            let (nr, ni) = (re * c - im * s, re * s + im * c);
            re = nr;
            im = ni;
            env *= r;
            if env < 1e-6 {
                break;
            }
        }
    }
    // A fade over the last fifth (a cut would click), then the energy `(1 − direct)²`.
    let fade = len / 5;
    for x in h.iter_mut() {
        for k in 0..fade {
            let t = k as f64 / fade as f64;
            x[len - 1 - k] *= 0.5 - 0.5 * math::cos(std::f64::consts::PI * t);
        }
        let e: f64 = x.iter().map(|y| y * y).sum();
        let g = (1.0 - v.direct) / e.max(1e-30).sqrt();
        for y in x.iter_mut() {
            *y *= g;
        }
    }
    h
}

/// A body's two responses (left, right), ready to convolve and shared.
type Pair = Arc<[Arc<Prepared>; 2]>;

/// The responses made this session, by sample rate and voicing, ready to convolve:
/// synthesised once, shared by every unit (made off the audio thread; a unit at a rate
/// already made shares them — round 39, fixes: their spectra too, not a copy each).
fn cached(v: &BodyVoicing, fs: f64) -> Pair {
    use std::sync::Mutex;
    type Made = Vec<(u64, BodyVoicing, Pair)>;
    static MADE: OnceLock<Mutex<Made>> = OnceLock::new();
    let made = MADE.get_or_init(|| Mutex::new(Vec::new()));
    let key = fs.to_bits();
    if let Ok(m) = made.lock() {
        if let Some((_, _, h)) = m.iter().find(|(k, b, _)| *k == key && b == v) {
            return h.clone();
        }
    }
    MADE_HERE.with(|n| n.set(n.get() + 1));
    let [l, r] = responses(v, fs);
    let h = Arc::new([Arc::new(Prepared::new(&l)), Arc::new(Prepared::new(&r))]);
    if let Ok(mut m) = made.lock() {
        if m.len() < 64 {
            m.push((key, *v, h.clone()));
        }
    }
    if *v == BodyVoicing::default() {
        publish(fs, &h);
    }
    h
}

thread_local! {
    /// The responses this thread has synthesised (the tests: none on the interface's).
    static MADE_HERE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many bodies' responses this thread has synthesised: what a test of a thread that
/// must never wait for them reads.
pub fn made_on_this_thread() -> u64 {
    MADE_HERE.with(|n| n.get())
}

/// The shipped voicing's responses by sample rate, for a body made waiting to take without
/// a lock or an allocation (the audio thread): each slot written once.
const READY: usize = 16;
static READY_AT: [OnceLock<(u64, Pair)>; READY] = [const { OnceLock::new() }; READY];

fn publish(fs: f64, h: &Pair) {
    let key = fs.to_bits();
    for slot in &READY_AT {
        match slot.get() {
            Some((k, _)) if *k == key => return,
            Some(_) => continue,
            None => {
                if slot.set((key, h.clone())).is_ok() {
                    return;
                }
                if slot.get().is_some_and(|(k, _)| *k == key) {
                    return;
                }
            }
        }
    }
}

/// The shipped voicing's responses at `fs`, if made. Lock-free, allocates nothing.
fn ready(fs: f64) -> Option<&'static Pair> {
    let key = fs.to_bits();
    READY_AT
        .iter()
        .filter_map(OnceLock::get)
        .find(|(k, _)| *k == key)
        .map(|(_, h)| h)
}

/// Whether a body made waiting at `fs` can be given its responses later: they are made,
/// or there is room to publish them.
fn can_wait(fs: f64) -> bool {
    ready(fs).is_some() || READY_AT.iter().any(|s| s.get().is_none())
}

/// Make the body's responses for `fs` now (the designer's worker, beside the keyboard's
/// design), so the first unit at that rate does not pay for them.
pub fn warm(fs: f64) {
    let _ = cached(&BodyVoicing::default(), fs);
}

/// Whether the shipped voicing's responses at `fs` are made.
pub fn is_warm(fs: f64) -> bool {
    ready(fs).is_some()
}

/// The body: two convolvers, the panned bridge forces in, the microphones out — the direct
/// path beside them, the resonance scaled by `level` (the `body` setting), and the tilt.
#[derive(Clone, Debug)]
pub struct Body {
    left: Convolver,
    right: Convolver,
    direct: f64,
    pub level: f64,
    /// The tilt: a high shelf from about 2.5 kHz (its gain minus one), per channel.
    shelf: f64,
    tilt_a: f64,
    tilt: [f64; 2],
    /// The rate it waits at for the shipped voicing's responses (made waiting), else 0.
    waiting_at: f64,
}

impl Body {
    /// Allocates and synthesises (off the audio thread).
    pub fn new(fs: f64, v: &BodyVoicing) -> Body {
        let h = cached(v, fs);
        Body::of(
            fs,
            v,
            Convolver::with(h[0].clone()),
            Convolver::with(h[1].clone()),
        )
    }

    fn of(fs: f64, v: &BodyVoicing, left: Convolver, right: Convolver) -> Body {
        Body {
            left,
            right,
            direct: v.direct,
            level: 1.0,
            shelf: 0.0,
            tilt_a: math::exp(-std::f64::consts::TAU * 2_500.0 / fs),
            tilt: [0.0; 2],
            waiting_at: 0.0,
        }
    }

    /// The shipped voicing's body, its responses taken if they are made and else waited
    /// for — never synthesised here (round 39, fixes: a unit made on the interface's thread
    /// at a rate not yet made paid 16 ms for them). Until [`Self::take`] gives them, its
    /// resonance is silent. With no room left to wait (sixteen rates this session) they are
    /// made at once. Allocates its lines.
    pub fn waiting(fs: f64) -> Body {
        let v = BodyVoicing::default();
        if let Some(h) = ready(fs) {
            return Body::of(
                fs,
                &v,
                Convolver::with(h[0].clone()),
                Convolver::with(h[1].clone()),
            );
        }
        if !can_wait(fs) {
            return Body::new(fs, &v);
        }
        let len = ((v.length * fs.min(48_000.0)) as usize).max(1);
        let p = Prepared::partitions_of(len);
        let mut b = Body::of(fs, &v, Convolver::waiting(p), Convolver::waiting(p));
        b.waiting_at = fs;
        b
    }

    /// Whether it waits for its responses.
    pub fn is_waiting(&self) -> bool {
        self.waiting_at != 0.0
    }

    /// Its responses, if they are made now: from the next sample it sounds as a body made
    /// with them would (its lines held the inputs meanwhile). Lock-free, allocates nothing:
    /// for the audio thread.
    pub fn take(&mut self) -> bool {
        if self.waiting_at == 0.0 {
            return true;
        }
        let Some(h) = ready(self.waiting_at) else {
            return false;
        };
        self.adopt(h);
        true
    }

    /// Its responses made now if they are not (offline: a render waits for them).
    /// Allocates.
    pub fn make(&mut self) {
        if self.waiting_at == 0.0 {
            return;
        }
        let h = cached(&BodyVoicing::default(), self.waiting_at);
        self.adopt(&h);
    }

    fn adopt(&mut self, h: &Pair) {
        let l = self.left.adopt(h[0].clone());
        let r = self.right.adopt(h[1].clone());
        debug_assert!(l && r, "a response of the length waited for");
        self.waiting_at = 0.0;
    }

    /// The tilt: the top raised or lowered by `db`.
    pub fn set_tilt(&mut self, db: f64) {
        self.shelf = math::powf(10.0f64, db / 20.0) - 1.0;
    }

    pub fn at_rest(&self) -> bool {
        self.left.at_rest() && self.right.at_rest() && self.tilt == [0.0; 2]
    }

    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
        self.tilt = [0.0; 2];
    }

    /// Every state cleared, the grid's phase kept (a model switch).
    pub fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
        self.tilt = [0.0; 2];
    }

    /// At a grid point with nothing driving it: the tilt's states flushed below `floor`.
    pub fn flush(&mut self, floor: f64) {
        for t in self.tilt.iter_mut() {
            if t.abs() < floor {
                *t = 0.0;
            }
        }
    }

    pub fn skip(&mut self, frames: usize) {
        self.left.skip(frames);
        self.right.skip(frames);
    }

    /// The direct path and the resonance, each stereo, before the tilt.
    #[inline]
    pub fn parts(&mut self, l: f64, r: f64) -> ([f64; 2], [f64; 2]) {
        (
            [self.direct * l, self.direct * r],
            [
                self.level * self.left.tick(l),
                self.level * self.right.tick(r),
            ],
        )
    }

    /// The tilt on the microphones' sum.
    #[inline]
    pub fn tilt(&mut self, out: &mut [f64; 2]) {
        if self.shelf != 0.0 || self.tilt != [0.0; 2] {
            for (c, o) in out.iter_mut().enumerate() {
                self.tilt[c] = *o + self.tilt_a * (self.tilt[c] - *o);
                *o += self.shelf * (*o - self.tilt[c]);
            }
        }
    }

    #[inline]
    pub fn tick(&mut self, l: f64, r: f64) -> (f64, f64) {
        let (d, b) = self.parts(l, r);
        let mut out = [d[0] + b[0], d[1] + b[1]];
        self.tilt(&mut out);
        (out[0], out[1])
    }
}
