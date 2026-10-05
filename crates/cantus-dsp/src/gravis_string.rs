//! Gravis's modelled electric bass: one bidirectional digital waveguide per string
//! (`research/instruments/bass.md` §4.1; Karjalainen, Välimäki & Tolonen, CMJ 1998; Rank &
//! Kubin, ICASSP 1997).
//!
//! **The string.** Two rails of displacement waves, each `half` samples long: the right-
//! going rail runs bridge → nut (or fret), the left-going one back. Both are read with
//! third-order Lagrange interpolation, which stays clean while the length moves (slides,
//! bends, glides) where an allpass would ring. The nut inverts; the bridge inverts through
//! a one-pole loss filter (sustain and high-frequency decay, both in seconds) and a cascade
//! of four first-order allpasses (the stiffness that makes partial *n* sharp by
//! `√(1 + B n²)`). The loop is tuned by its **phase delay at the fundamental**, filters and
//! interpolators included, so the pitch is right to a fraction of a cent from B0 to C5.
//!
//! **Where it is plucked and heard.** A pluck adds its shape to the rails at the pluck
//! position; a pickup reads the displacement at its own position through a Gaussian
//! aperture (a humbucker: two coils). The two comb filters of an electric bass — pluck and
//! pickup — are therefore not filters here but the geometry itself, as on the instrument.
//!
//! **Frets.** Where the string runs over the frets between the fretting finger and the
//! bridge, each fret is a conditional reflection: when the string's displacement there
//! passes the fret's height, the excess is taken out of both rails (Rank & Kubin). That is
//! slap, pop and fret buzz.

#[path = "gravis_string_engine.rs"]
mod engine;
pub use engine::{Played, StringEngine};

pub const MAX_STRINGS: usize = 6;
pub const MAX_FRETS: usize = 24;
/// First-order allpass sections of the dispersion filter.
const AP: usize = 4;
/// Taps of a pickup: two coils of five.
const TAPS: usize = 10;
/// Output scale: a finger pluck at full velocity peaks around −6 dBFS.
const OUT: f32 = 0.22;
/// What a slap's **thumb** is heard at, against a finger pluck. The thumb's strike is a
/// velocity step whose edge reaches the coil (a derivative) in a third of a millisecond: at
/// unity it peaked ~20 dB over a finger pluck of the same velocity (+12 dBFS) and ~10 dB
/// over its loudness. The physics — the step, the frets it slaps — is kept; only what the
/// pickups hand on is scaled (round 19, fixes-mix).
pub const THUMB_LEVEL: f32 = 0.2;
/// What a slap's **pop** is heard at: it pulls 3.5× a finger's displacement off the frets
/// (+4 … +8 dBFS at unity); scaled as the thumb is.
pub const POP_LEVEL: f32 = 0.3;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Technique {
    #[default]
    Finger,
    Pick,
    /// Thumb below the pop velocity, pop at or above it.
    Slap,
    PalmMute,
}

impl Technique {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => Technique::Pick,
            2 => Technique::Slap,
            3 => Technique::PalmMute,
            _ => Technique::Finger,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Fingering {
    /// The lowest fret that reaches the note (open strings first).
    Lowest,
    /// Stay near where the hand is.
    #[default]
    Position,
    /// The thickest string that reaches the note.
    LowStrings,
}

impl Fingering {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            0 => Fingering::Lowest,
            2 => Fingering::LowStrings,
            _ => Fingering::Position,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum StringType {
    #[default]
    Round,
    Half,
    Flat,
    Tape,
}

impl StringType {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => StringType::Half,
            2 => StringType::Flat,
            3 => StringType::Tape,
            _ => StringType::Round,
        }
    }

    /// (stiffness factor, high-frequency sustain factor).
    fn factors(self) -> (f64, f64) {
        match self {
            StringType::Round => (1.0, 1.0),
            StringType::Half => (0.85, 0.6),
            StringType::Flat => (0.7, 0.3),
            StringType::Tape => (0.4, 0.15),
        }
    }
}

/// The open strings (MIDI notes, low to high) for 4, 5 or 6 strings and a tuning: 0
/// standard (B0 E1 A1 D2 G2 C3), 1 drop (the lowest a tone down), 2 a half step down.
pub fn open_strings(strings: usize, tuning: usize) -> [f32; MAX_STRINGS] {
    let mut o = match strings {
        5 => [23.0, 28.0, 33.0, 38.0, 43.0, 0.0],
        6 => [23.0, 28.0, 33.0, 38.0, 43.0, 48.0],
        _ => [28.0, 33.0, 38.0, 43.0, 0.0, 0.0],
    };
    let n = strings.clamp(4, 6);
    match tuning {
        1 => o[0] -= 2.0,
        2 => o.iter_mut().take(n).for_each(|x| *x -= 1.0),
        _ => {}
    }
    o
}

/// The inharmonicity coefficient B of an open string tuned to `note`, at 100 % stiffness on
/// a 34″ roundwound set: `π³ E d⁴ / (64 T L²)` for the core wire, about 4.4·10⁻⁴ for the
/// E string, halving about every five semitones up the set.
pub fn b_open(note: f32) -> f64 {
    4.4e-4 * (-(note as f64 - 28.0) / 5.2).exp2()
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StringSettings {
    pub strings: usize,
    pub open: [f32; MAX_STRINGS],
    pub scale_mm: f32,
    pub frets: usize,
    pub fretless: bool,
    pub kind: StringType,
    /// 0…2 of the computed B.
    pub stiffness: f32,
    /// T60 of the fundamental, seconds.
    pub sustain_s: f32,
    /// 0…1: the high frequencies' sustain relative to the fundamental's.
    pub bright: f32,
    pub age: f32,
    /// String height over the 12th fret, mm.
    pub action_mm: f32,
    /// 0…1: how hard the frets are (0: no collisions at all).
    pub buzz: f32,
    pub technique: Technique,
    pub pluck_mm: f32,
    pub hardness: f32,
    /// Velocity 0…1 at or above which a slap is a pop.
    pub pop_vel: f32,
    pub mute: f32,
    /// T60 once the note is let go (the fingers damp it), ms.
    pub release_ms: f32,
    pub noise: f32,
    pub fingering: Fingering,
    pub pu_mm: [f32; 2],
    pub pu_level: [f32; 2],
    pub humbucker: bool,
    pub pu_height_mm: f32,
    pub piezo: f32,
    pub tone: f32,
    pub humanize: f32,
}

impl Default for StringSettings {
    fn default() -> Self {
        StringSettings {
            strings: 4,
            open: open_strings(4, 0),
            scale_mm: 864.0,
            frets: 21,
            fretless: false,
            kind: StringType::Round,
            stiffness: 1.0,
            sustain_s: 6.0,
            bright: 0.45,
            age: 0.2,
            action_mm: 2.2,
            buzz: 0.3,
            technique: Technique::Finger,
            pluck_mm: 180.0,
            hardness: 0.5,
            pop_vel: 110.0 / 127.0,
            mute: 0.0,
            release_ms: 150.0,
            noise: 0.3,
            fingering: Fingering::Position,
            pu_mm: [60.0, 159.0],
            pu_level: [1.0, 1.0],
            humbucker: false,
            pu_height_mm: 3.0,
            piezo: 0.0,
            tone: 1.0,
            humanize: 0.0,
        }
    }
}

// ---- the tuning maths, shared with the editor --------------------------------------------

/// Phase delay (samples) of the loss filter `g(1−a)/(1 − a z⁻¹)` at `w` rad/sample.
pub fn loss_delay(a: f64, w: f64) -> f64 {
    (a * w.sin()).atan2(1.0 - a * w.cos()) / w
}

/// Phase delay of one first-order allpass `(c + z⁻¹)/(1 + c z⁻¹)`.
pub fn ap_delay(c: f64, w: f64) -> f64 {
    let num = (-w.sin()).atan2(c + w.cos());
    let den = (-c * w.sin()).atan2(1.0 + c * w.cos());
    -(num - den) / w
}

/// Third-order Lagrange taps for a delay `d ≥ 2`: `(first tap's delay, weights)`.
pub fn lagrange(d: f64) -> (usize, [f32; 4]) {
    let m0 = (d.floor() as usize).saturating_sub(1).max(1);
    let x = d - m0 as f64; // 1 ≤ x < 2 in the normal case
    let h = [
        -(x - 1.0) * (x - 2.0) * (x - 3.0) / 6.0,
        x * (x - 2.0) * (x - 3.0) / 2.0,
        -x * (x - 1.0) * (x - 3.0) / 2.0,
        x * (x - 1.0) * (x - 2.0) / 6.0,
    ];
    (m0, [h[0] as f32, h[1] as f32, h[2] as f32, h[3] as f32])
}

/// Phase delay of the Lagrange read for delay `d` at `w`.
pub fn lagrange_delay(d: f64, w: f64) -> f64 {
    let (m0, h) = lagrange(d);
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (j, hj) in h.iter().enumerate() {
        let a = -w * j as f64;
        re += *hj as f64 * a.cos();
        im += *hj as f64 * a.sin();
    }
    m0 as f64 - im.atan2(re) / w
}

/// The loss filter `(g, a)` that makes the loop lose 60 dB in `t60` seconds at `f0` and in
/// `t60_high` at `f_high`.
pub fn loss_design(f0: f64, t60: f64, t60_high: f64, f_high: f64, sr: f64) -> (f64, f64) {
    let g0 = 10f64.powf(-3.0 / (f0 * t60.max(1e-3)));
    let gh = 10f64.powf(-3.0 / (f0 * t60_high.max(1e-4)));
    let (w0, wh) = (
        std::f64::consts::TAU * f0 / sr,
        std::f64::consts::TAU * f_high / sr,
    );
    let r = (gh / g0).min(1.0);
    let a = if r >= 0.999_999 {
        0.0
    } else {
        let a2 = r * r - 1.0;
        let b = 2.0 * (w0.cos() - r * r * wh.cos());
        let disc = (b * b - 4.0 * a2 * a2).max(0.0);
        // Of the two roots (their product is 1), the one inside the unit circle.
        let x1 = (-b + disc.sqrt()) / (2.0 * a2);
        let x2 = (-b - disc.sqrt()) / (2.0 * a2);
        let pick = if (0.0..1.0).contains(&x1) { x1 } else { x2 };
        pick.clamp(0.0, 0.995)
    };
    let mag = ((1.0 - a * w0.cos()).powi(2) + (a * w0.sin()).powi(2)).sqrt();
    let g = (g0 * mag / (1.0 - a)).min(0.999_99);
    (g, a)
}

/// The dispersion allpass coefficient (four sections) that makes partial `n_fit` as sharp as
/// a stiff string with coefficient `b`, given the loop's other frequency-dependent delay
/// `other(w)`. `period` is the loop in samples; the allpasses may take at most 40 % of it.
pub fn dispersion_fit(b: f64, f0: f64, sr: f64, period: f64, other: impl Fn(f64) -> f64) -> f64 {
    if b <= 1e-9 {
        return 0.0;
    }
    let n_fit = ((0.3 * sr).min(5_000.0) / f0).floor().clamp(2.0, 12.0);
    let w0 = std::f64::consts::TAU * f0 / sr;
    let wn = w0 * n_fit * ((1.0 + b * n_fit * n_fit) / (1.0 + b)).sqrt();
    let tau_n = period / ((1.0 + b * n_fit * n_fit) / (1.0 + b)).sqrt();
    let need = (period - tau_n) - (other(w0) - other(wn));
    if need <= 0.0 {
        return 0.0;
    }
    let spread = |c: f64| AP as f64 * (ap_delay(c, w0) - ap_delay(c, wn));
    // The most negative coefficient allowed by the length budget.
    let (mut lo, mut hi) = (-0.97f64, 0.0f64);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if AP as f64 * ap_delay(mid, w0) > 0.4 * period {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let c_min = hi;
    if spread(c_min) <= need {
        return c_min;
    }
    let (mut lo, mut hi) = (c_min, 0.0f64);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if spread(mid) > need {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The comb a position makes on the harmonics: `|sin(nπβ)|` for a pluck or pickup at the
/// fraction `beta` of the vibrating length from the bridge (the editor draws these).
pub fn comb(n: usize, beta: f64) -> f64 {
    (n as f64 * std::f64::consts::PI * beta).sin().abs()
}

/// Where fret `k` is, in mm from the bridge, on a scale of `scale_mm`.
pub fn fret_mm(scale_mm: f64, k: f64) -> f64 {
    scale_mm * (-k / 12.0).exp2()
}

/// A pickup's taps: (fractional position in samples from the bridge, weight).
fn taps_for(center: f64, half_width: f64, humbucker: bool, coil_gap: f64) -> [(f64, f32); TAPS] {
    const W: [f32; 5] = [0.13, 0.2, 0.34, 0.2, 0.13];
    let mut t = [(0.0, 0.0); TAPS];
    let coils: &[f64] = if humbucker {
        &[-coil_gap, coil_gap]
    } else {
        &[0.0]
    };
    let scale = 1.0 / coils.len() as f32;
    for (c, off) in coils.iter().enumerate() {
        for (i, w) in W.iter().enumerate() {
            let x = center + off + half_width * (i as f64 - 2.0) * 0.5;
            t[c * 5 + i] = (x, w * scale);
        }
    }
    t
}

#[inline]
fn rd(buf: &[f32], mask: usize, w: usize, delay: usize) -> f32 {
    buf[w.wrapping_sub(delay) & mask]
}

/// Linear-interpolated read at a fractional delay.
#[inline]
fn rdf(buf: &[f32], mask: usize, w: usize, delay: f64) -> f32 {
    let d = delay.max(1.0);
    let i = d.floor() as usize;
    let f = (d - i as f64) as f32;
    let a = rd(buf, mask, w, i);
    let b = rd(buf, mask, w, i + 1);
    a + (b - a) * f
}

/// A smooth minimum: the pluck's triangle with its apex rounded by the finger's width.
#[inline]
fn smin(a: f64, b: f64, k: f64) -> f64 {
    0.5 * (a + b - ((a - b) * (a - b) + k * k).sqrt())
}

/// One string: its two rails and everything the loop needs.
#[derive(Clone, Debug)]
struct Guide {
    r: Vec<f32>,
    l: Vec<f32>,
    mask: usize,
    w: usize,
    /// Each rail's delay in samples (half the loop's pure delay).
    half: f64,
    half_target: f64,
    half_step: f64,
    ramp_left: u32,
    m0: usize,
    lg: [f32; 4],
    loss_b0: f32,
    loss_a: f32,
    loss_y: f32,
    ap_c: f32,
    ap_on: bool,
    ap_s: [f32; AP],
    /// Frets under the vibrating string: (rail offset, other rail offset, height in mm).
    frets: [(usize, usize, f32); MAX_FRETS],
    n_frets: usize,
    taps: [[(f64, f32); TAPS]; 2],
    pu_prev: [f32; 2],
    /// What the pickups hand on, by the last excitation's kind (1, or a slap's thumb or
    /// pop level).
    level: f32,
    piezo_prev: f32,
    /// How much of the motion is towards the frets (finger 0.35 … slap 1).
    vfac: f32,
    /// A pluck just changed the displacement at once: the pickups take it as where the
    /// string already was (the finger held it there), not as a jump to differentiate.
    resync: bool,
    held: bool,
    released: bool,
    active: bool,
    quiet: u32,
    note: f64,
    open: f32,
    vib_mm: f64,
    f0: f64,
    k_norm: f32,
    pluck_x: f64,
    /// The key and id this string sounds, in Strings mode.
    key: u8,
    id: u64,
}

impl Guide {
    fn new(len: usize) -> Self {
        Guide {
            r: vec![0.0; len],
            l: vec![0.0; len],
            mask: len - 1,
            w: 0,
            half: 100.0,
            half_target: 100.0,
            half_step: 0.0,
            ramp_left: 0,
            m0: 99,
            lg: [0.0, 1.0, 0.0, 0.0],
            loss_b0: 0.0,
            loss_a: 0.0,
            loss_y: 0.0,
            ap_c: 0.0,
            ap_on: false,
            ap_s: [0.0; AP],
            frets: [(1, 1, 0.0); MAX_FRETS],
            n_frets: 0,
            taps: [[(1.0, 0.0); TAPS]; 2],
            pu_prev: [0.0; 2],
            level: 1.0,
            piezo_prev: 0.0,
            vfac: 0.35,
            resync: false,
            held: false,
            released: false,
            active: false,
            quiet: 0,
            note: 40.0,
            open: 28.0,
            vib_mm: 864.0,
            f0: 82.4,
            k_norm: 1.0,
            pluck_x: 10.0,
            key: 0,
            id: 0,
        }
    }

    fn clear(&mut self) {
        self.r.iter_mut().for_each(|x| *x = 0.0);
        self.l.iter_mut().for_each(|x| *x = 0.0);
        self.loss_y = 0.0;
        self.ap_s = [0.0; AP];
        self.pu_prev = [0.0; 2];
        self.piezo_prev = 0.0;
        self.active = false;
        self.held = false;
        self.quiet = 0;
    }

    /// Scale everything in flight (a finger stopping the string before it plucks again).
    fn damp(&mut self, k: f32) {
        self.r.iter_mut().for_each(|x| *x *= k);
        self.l.iter_mut().for_each(|x| *x *= k);
        self.resync = true;
    }

    /// Tune the loop to `note` (a fractional MIDI note on this string), with the damping
    /// of a held or a let-go string. `ramp`: samples over which the length moves there.
    fn tune(&mut self, s: &StringSettings, note: f64, sr: f64, a4: f64, ramp: u32) {
        self.note = note;
        let fret = note - self.open as f64;
        let scale = s.scale_mm as f64;
        self.vib_mm = fret_mm(scale, fret);
        let f0 = super::voice::hz(note, a4).clamp(8.0, 0.2 * sr);
        self.f0 = f0;
        let period = sr / f0;
        let w0 = std::f64::consts::TAU * f0 / sr;
        // Damping: the sustain, a mute, and the fingers once it is let go.
        let (b_type, hf_type) = s.kind.factors();
        let mute = if s.technique == Technique::PalmMute {
            s.mute.max(0.75)
        } else {
            s.mute
        } as f64;
        let mut t60 = (s.sustain_s as f64).powf(1.0 - mute) * 0.12f64.powf(mute);
        if self.released {
            t60 = t60.min(s.release_ms as f64 * 0.001);
        }
        let ratio = ((0.02 + 0.98 * s.bright as f64)
            * hf_type
            * (1.0 - 0.8 * s.age as f64)
            * (1.0 - 0.7 * mute))
            .clamp(0.004, 1.0);
        let f_high = (3.0 * f0).max(2_000.0).min(0.4 * sr);
        let (g, a) = loss_design(f0, t60, t60 * ratio, f_high, sr);
        self.loss_b0 = (g * (1.0 - a)) as f32;
        self.loss_a = a as f32;
        // Stiffness.
        let b = if s.stiffness > 0.0 {
            b_open(self.open) * (scale / self.vib_mm).powi(2) * s.stiffness as f64 * b_type
        } else {
            0.0
        };
        let other = |w: f64| loss_delay(a, w) + 2.0 * lagrange_delay(0.5 * period, w);
        let c = dispersion_fit(b, f0, sr, period, other);
        self.ap_c = c as f32;
        self.ap_on = c != 0.0;
        // Tune by the phase delay at the fundamental, interpolators included.
        let mut tau = loss_delay(a, w0);
        if self.ap_on {
            tau += AP as f64 * ap_delay(c, w0);
        }
        let mut half = 0.5 * (period - tau);
        for _ in 0..2 {
            half -= lagrange_delay(half, w0) - half;
        }
        let half = half.clamp(2.0, (self.mask - 8) as f64);
        if ramp == 0 || !self.active {
            self.half = half;
            self.ramp_left = 0;
            let (m0, lg) = lagrange(half);
            self.m0 = m0;
            self.lg = lg;
        } else {
            self.half_step = (half - self.half) / ramp as f64;
            self.ramp_left = ramp;
        }
        self.half_target = half;
        self.k_norm = OUT * (period / std::f64::consts::TAU) as f32;
        // Geometry, in samples from the bridge.
        let per_mm = half / self.vib_mm;
        self.pluck_x = (s.pluck_mm as f64 * per_mm).clamp(1.5, half - 1.5);
        let h_bridge = 2.0 * s.action_mm as f64;
        self.n_frets = 0;
        if !s.fretless && s.buzz > 0.0 {
            let first = fret.max(0.0).floor() as usize + 1;
            for k in first..=s.frets.min(MAX_FRETS) {
                let d = fret_mm(scale, k as f64);
                if d >= self.vib_mm {
                    continue;
                }
                let x = d * per_mm;
                let ro = (x.round() as usize).max(1);
                let lo = ((half - x).round() as usize).max(1);
                let h = (h_bridge * (1.0 - d / self.vib_mm)) as f32;
                self.frets[self.n_frets] = (ro, lo, h.max(0.05));
                self.n_frets += 1;
            }
        }
        for p in 0..2 {
            let center = s.pu_mm[p] as f64 * per_mm;
            let mut taps = taps_for(center, 9.0 * per_mm, s.humbucker, 10.0 * per_mm);
            for t in &mut taps {
                // Off the vibrating string (beyond the fretting finger): nothing to hear.
                if t.0 >= half - 1.0 || t.0 < 1.0 {
                    t.1 = 0.0;
                    t.0 = 1.0;
                }
            }
            self.taps[p] = taps;
        }
    }

    /// Add a pluck: the displacement triangle with its apex at `x` (rounded by `width`
    /// samples), `height` mm, released at rest.
    fn pluck(&mut self, x: f64, height: f32, width: f64) {
        let half = self.half;
        let xp = x.clamp(1.5, half - 1.5);
        let k = (width / xp.min(half - xp)).max(1e-3);
        let n = (half.floor() as usize).saturating_sub(1);
        for i in 1..=n {
            let xf = i as f64;
            let y0 = height as f64 * smin(xf / xp, (half - xf) / (half - xp), k);
            let y = (0.5 * y0) as f32;
            let ri = self.w.wrapping_sub(i) & self.mask;
            let li = self.w.wrapping_sub(((half - xf).round() as usize).max(1)) & self.mask;
            self.r[ri] += y;
            self.l[li] += y;
        }
        self.active = true;
        self.quiet = 0;
        self.resync = true;
    }

    /// Add a strike: a velocity pulse `width` samples wide at `x` (the thumb, a hammer).
    fn strike(&mut self, x: f64, v: f32, width: f64) {
        let half = self.half;
        let n = (half.floor() as usize).saturating_sub(1);
        let w = width.max(1.0);
        for i in 1..=n {
            let u = ((i as f64 - (x - w)) / (2.0 * w)).clamp(0.0, 1.0);
            let s = (v as f64 * u * u * (3.0 - 2.0 * u)) as f32 * 0.5;
            let ri = self.w.wrapping_sub(i) & self.mask;
            let li = self
                .w
                .wrapping_sub(((half - i as f64).round() as usize).max(1))
                & self.mask;
            self.r[ri] += s;
            self.l[li] -= s;
        }
        self.active = true;
        self.quiet = 0;
    }

    /// One sample; returns the two pickups' and the piezo's signals.
    #[inline]
    fn next(&mut self, buzz: f32, pu_height: f32) -> [f32; 3] {
        if self.ramp_left > 0 {
            self.ramp_left -= 1;
            self.half = if self.ramp_left == 0 {
                self.half_target
            } else {
                self.half + self.half_step
            };
            let (m0, lg) = lagrange(self.half);
            self.m0 = m0;
            self.lg = lg;
        }
        let (w, mask, m0) = (self.w, self.mask, self.m0);
        let lg = self.lg;
        let read = |buf: &[f32]| {
            lg[0] * rd(buf, mask, w, m0)
                + lg[1] * rd(buf, mask, w, m0 + 1)
                + lg[2] * rd(buf, mask, w, m0 + 2)
                + lg[3] * rd(buf, mask, w, m0 + 3)
        };
        let rn = read(&self.r);
        let lb = read(&self.l);
        // The bridge: invert, lose, disperse.
        let mut v = self.loss_b0 * -lb + self.loss_a * self.loss_y;
        if v.abs() < 1e-30 {
            v = 0.0;
        }
        self.loss_y = v;
        if self.ap_on {
            let c = self.ap_c;
            for s in &mut self.ap_s {
                let y = c * v + *s;
                *s = v - c * y;
                v = y;
            }
        }
        self.r[w] = v;
        self.l[w] = -rn;
        // The frets: conditional reflections.
        if buzz > 0.0 {
            let inv = 1.0 / self.vfac;
            for &(ro, lo, h) in &self.frets[..self.n_frets] {
                let ri = w.wrapping_sub(ro) & mask;
                let li = w.wrapping_sub(lo) & mask;
                let y = self.r[ri] + self.l[li];
                let lim = -h * inv;
                if y < lim {
                    let e = (y - lim) * 0.5 * buzz;
                    self.r[ri] -= e;
                    self.l[li] -= e;
                }
            }
        }
        // The pickups: displacement through the aperture, the magnet's nonlinearity, the
        // coil's derivative.
        let mut out = [0.0f32; 3];
        let h = pu_height;
        for (p, o) in out.iter_mut().take(2).enumerate() {
            let mut y = 0.0f32;
            for &(x, wt) in &self.taps[p] {
                if wt != 0.0 {
                    y += wt * (rdf(&self.r, mask, w, x) + rdf(&self.l, mask, w, self.half - x));
                }
            }
            // Nearer the pole piece, more flux: y / (1 + y/2h), which doubles the slope at
            // the pole (where the string stops) and halves it as far away above — even
            // harmonics that grow with the pickup's closeness and the string's swing,
            // bounded (Paiva, Pakarinen & Välimäki 2012, simplified).
            let y = y.max(-h);
            let f = y / (1.0 + 0.5 * y / h);
            if self.resync {
                self.pu_prev[p] = f;
            }
            *o = (f - self.pu_prev[p]) * self.k_norm * self.level;
            self.pu_prev[p] = f;
        }
        if self.resync {
            self.piezo_prev = lb;
            self.resync = false;
        }
        out[2] = (lb - self.piezo_prev) * self.k_norm * self.level;
        self.piezo_prev = lb;
        self.w = (w + 1) & mask;
        if rn.abs() < 1e-6 && lb.abs() < 1e-6 {
            self.quiet += 1;
            if self.quiet as f64 > 2.0 * self.half + 16.0 {
                self.clear();
            }
        } else {
            self.quiet = 0;
        }
        out
    }
}
