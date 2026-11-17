//! Fractus's grain cloud: grains read from the cloud's own buffer of the input, every draw
//! a hash of the seed and the grain's number on the song's grid.
//!
//! - **The buffer** is written in its own count `w`, which is the song sample while the
//!   transport plays (it is set there at every start and seek) and stands still while the
//!   cloud is frozen — so a frozen cloud keeps its last seconds for as long as it is
//!   frozen, and when it thaws the new input follows the frozen one.
//! - **Octaves for pitched-up grains**: beside the buffer, two copies at half and a quarter
//!   of the rate, each low-passed by a 63-tap Kaiser half-band (−80 dB) as it is written,
//!   zero-phase (a copy's sample *m* is the input's time `m·2^k`). A grain at or below the
//!   input's pitch reads the buffer; transposed up it reads the copy whose band, sped up,
//!   stays under Nyquist ([`level_of`]: the half-rate copy up to an octave, the quarter-rate
//!   one up to two), crossfaded in over two semitones. Four-point Hermite at every level.
//! - **Grains** start on the song's grid: grain number `⌊so · density / fs⌋`, a new one
//!   each time it counts on. Its position, pitch, pan and direction are drawn from hashes
//!   of (seed, number, purpose); its pitch is quantised to the scale; a grain never reads
//!   ahead of what the buffer holds (it starts further back the faster it reads forward).
//!   A fixed pool of [`MAX_GRAINS`], a grain's place its number modulo the pool (so the
//!   grains sum in one order whatever came before); a grain whose place is still taken is
//!   skipped (the same skip at every block size). Each grain's level is `1/√overlap`, so density and size move
//!   the texture, not the loudness.

use super::seq::{roll, unit};
use crate::math;
use crate::modfx_line::hermite;
use std::sync::OnceLock;

pub const MAX_GRAINS: usize = 128;
/// Seconds of input the cloud holds.
pub const CLOUD_S: f64 = 4.0;
/// Copies of the buffer: the input, a half and a quarter of its rate.
const LEVELS: usize = 3;
/// Points in each window's table.
const WIN: usize = 1024;
/// The half-band's `m` (63 taps).
const HB_M: usize = 16;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum GrainWindow {
    #[default]
    Hann,
    Gauss,
    /// Flat in the middle, a quarter of the grain each side to fade.
    Tukey,
    /// A fast attack and a decay: percussive grains.
    Expodec,
    /// The decay reversed: grains that swell.
    Rexpodec,
}

impl GrainWindow {
    pub const ALL: [GrainWindow; 5] = [
        GrainWindow::Hann,
        GrainWindow::Gauss,
        GrainWindow::Tukey,
        GrainWindow::Expodec,
        GrainWindow::Rexpodec,
    ];
    pub fn from_index(v: f32) -> GrainWindow {
        let i = v.round().clamp(0.0, 4.0) as usize;
        GrainWindow::ALL[i]
    }
}

/// The intervals a grain's pitch may take.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Scale {
    /// Any pitch (cents and all).
    Free,
    #[default]
    Chromatic,
    Major,
    Minor,
    /// The minor pentatonic.
    Pentatonic,
    /// Fifths and octaves.
    Fifths,
    Octaves,
}

impl Scale {
    pub const ALL: [Scale; 7] = [
        Scale::Free,
        Scale::Chromatic,
        Scale::Major,
        Scale::Minor,
        Scale::Pentatonic,
        Scale::Fifths,
        Scale::Octaves,
    ];
    pub fn from_index(v: f32) -> Scale {
        Scale::ALL[v.round().clamp(0.0, 6.0) as usize]
    }

    fn degrees(self) -> &'static [i32] {
        match self {
            Scale::Free | Scale::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Pentatonic => &[0, 3, 5, 7, 10],
            Scale::Fifths => &[0, 7],
            Scale::Octaves => &[0],
        }
    }

    /// `st` semitones moved to the nearest interval of the scale (ties upwards).
    pub fn quantise(self, st: f32) -> f32 {
        if self == Scale::Free {
            return st;
        }
        let o = (st / 12.0).floor();
        let r = st - 12.0 * o;
        let mut best = 0.0f32;
        let mut dist = f32::MAX;
        for &d in self.degrees().iter().chain(&[12]) {
            let x = (r - d as f32).abs();
            if x <= dist {
                dist = x;
                best = d as f32;
            }
        }
        12.0 * o + best
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CloudSettings {
    /// The cloud beside the chain, 0 … 1 (0: off).
    pub level: f32,
    /// How far back the grains start, ms (0 … 2000).
    pub position_ms: f32,
    /// A random distance further back, ms (0 … 1000).
    pub spray_ms: f32,
    /// A grain's length, ms (5 … 500).
    pub size_ms: f32,
    /// Grains a second (1 … 200).
    pub density_hz: f32,
    /// Transposition, semitones (−24 … 24).
    pub pitch_st: f32,
    /// A random transposition either way, semitones (0 … 24).
    pub pitch_rand_st: f32,
    pub scale: Scale,
    /// How far the grains spread across the stereo field (0 … 1).
    pub spread: f32,
    /// The chance a grain plays backwards (0 … 1).
    pub reverse: f32,
    pub window: GrainWindow,
    /// The buffer stops listening: the grains read what it held.
    pub freeze: bool,
}

impl Default for CloudSettings {
    fn default() -> Self {
        CloudSettings {
            level: 0.0,
            position_ms: 120.0,
            spray_ms: 60.0,
            size_ms: 90.0,
            density_hz: 24.0,
            pitch_st: 0.0,
            pitch_rand_st: 0.0,
            scale: Scale::Chromatic,
            spread: 0.5,
            reverse: 0.0,
            window: GrainWindow::Hann,
            freeze: false,
        }
    }
}

/// The windows as tables (one extra point, so a read at 1 needs no wrap), made once.
#[allow(clippy::needless_range_loop)] // five tables filled from one index
fn windows() -> &'static [[f32; WIN + 1]; 5] {
    static W: OnceLock<[[f32; WIN + 1]; 5]> = OnceLock::new();
    W.get_or_init(|| {
        let mut w = [[0.0f32; WIN + 1]; 5];
        let g_end = math::exp(-0.5f64 * (0.5 / 0.16) * (0.5 / 0.16));
        let decay = |x: f64| (math::exp(-5.0 * x) - math::exp(-5.0)) / (1.0 - math::exp(-5.0));
        for i in 0..=WIN {
            let x = i as f64 / WIN as f64;
            let hann = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * x);
            let d = (x - 0.5) / 0.16;
            let gauss = ((math::exp(-0.5 * d * d) - g_end) / (1.0 - g_end)).max(0.0);
            let edge = 0.25;
            let tukey = if x < edge {
                0.5 - 0.5 * math::cos(std::f64::consts::PI * x / edge)
            } else if x > 1.0 - edge {
                0.5 - 0.5 * math::cos(std::f64::consts::PI * (1.0 - x) / edge)
            } else {
                1.0
            };
            let att = |x: f64| {
                if x < 0.03 {
                    let s = math::sin(std::f64::consts::FRAC_PI_2 * x / 0.03);
                    s * s
                } else {
                    1.0
                }
            };
            let expodec = att(x) * decay(x);
            let rexpodec = att(1.0 - x) * decay(1.0 - x);
            for (k, v) in [hann, gauss, tukey, expodec, rexpodec].iter().enumerate() {
                w[k][i] = *v as f32;
            }
        }
        w
    })
}

#[inline]
fn window_at(shape: usize, x: f32) -> f32 {
    let t = &windows()[shape];
    let p = x * WIN as f32;
    let i = (p as usize).min(WIN - 1);
    let f = p - i as f32;
    t[i] + (t[i + 1] - t[i]) * f
}

/// The half-band's taps that are not zero, as (offset from the centre, coefficient).
fn halfband_taps() -> &'static [(i64, f32)] {
    static T: OnceLock<Vec<(i64, f32)>> = OnceLock::new();
    T.get_or_init(|| {
        let beta = 0.1102 * (80.0 - 8.7);
        let h = crate::exemplar::resample::halfband(HB_M, beta);
        let c = (h.len() / 2) as i64;
        h.iter()
            .enumerate()
            .filter(|(_, v)| **v != 0.0)
            .map(|(i, v)| (i as i64 - c, *v as f32))
            .collect()
    })
}

/// The half-band's half length: a copy's sample lags the input by this (level 1), three
/// times it (level 2).
const HB_C: i64 = 2 * HB_M as i64 - 1;
/// How far behind the newest sample a grain keeps, so every copy has its sample.
const MARGIN: i64 = 3 * HB_C + 24;

/// The buffer and its two copies.
#[derive(Clone, Debug)]
struct Buf {
    lv: [Vec<[f32; 2]>; LEVELS],
    mask: [usize; LEVELS],
    /// The next position written (level 0).
    w: i64,
    /// The first position a read may see (level 0).
    from: i64,
    /// The newest sample each level holds.
    avail: [i64; LEVELS],
    /// Zero samples written in a row, each level.
    zeros: [u64; LEVELS],
    frozen: bool,
}

impl Buf {
    fn new(frames: usize) -> Buf {
        let n0 = frames.next_power_of_two();
        let lv = [
            vec![[0.0; 2]; n0],
            vec![[0.0; 2]; n0 / 2],
            vec![[0.0; 2]; n0 / 4],
        ];
        Buf {
            mask: [n0 - 1, n0 / 2 - 1, n0 / 4 - 1],
            lv,
            w: 0,
            from: 0,
            avail: [-1, -1, -1],
            zeros: [u64::MAX; LEVELS],
            frozen: false,
        }
    }

    fn cap(&self, k: usize) -> i64 {
        self.lv[k].len() as i64
    }

    fn clear(&mut self) {
        for l in &mut self.lv {
            l.iter_mut().for_each(|v| *v = [0.0; 2]);
        }
        self.restart(0);
        self.zeros = [u64::MAX; LEVELS];
    }

    /// Nothing before `at` is heard; the next write is at `at`.
    fn restart(&mut self, at: i64) {
        self.w = at;
        self.from = at;
        for k in 0..LEVELS {
            self.avail[k] = at.div_euclid(1 << k) - 1;
        }
    }

    #[inline]
    fn tap(&self, k: usize, i: i64) -> [f32; 2] {
        if i > self.avail[k] || i <= self.avail[k] - self.cap(k) || (i << k) < self.from {
            return [0.0; 2];
        }
        self.lv[k][i as usize & self.mask[k]]
    }

    #[inline]
    fn put(&mut self, k: usize, i: i64, x: [f32; 2]) {
        self.lv[k][i as usize & self.mask[k]] = x;
        self.avail[k] = self.avail[k].max(i);
        let z = x[0].to_bits() == 0 && x[1].to_bits() == 0;
        self.zeros[k] = if z {
            self.zeros[k].saturating_add(1)
        } else {
            0
        };
    }

    /// A copy's sample `m` from the level below, centred on its sample `2m`.
    fn decimate(&self, k: usize, m: i64) -> [f32; 2] {
        let mut s = [0.0f32; 2];
        for &(o, c) in halfband_taps() {
            let x = self.tap(k - 1, 2 * m + o);
            s[0] += c * x[0];
            s[1] += c * x[1];
        }
        s
    }

    #[inline]
    fn write(&mut self, x: [f32; 2]) {
        if self.frozen {
            return;
        }
        let w = self.w;
        self.put(0, w, x);
        if (w - HB_C).rem_euclid(2) == 0 {
            let m = (w - HB_C).div_euclid(2);
            let y = self.decimate(1, m);
            self.put(1, m, y);
            if (m - HB_C).rem_euclid(2) == 0 {
                let q = (m - HB_C).div_euclid(2);
                let z = self.decimate(2, q);
                self.put(2, q, z);
            }
        }
        self.w += 1;
    }

    /// `frames` of silence (the rings hold zeros already).
    fn skip(&mut self, frames: usize) {
        if self.frozen || frames == 0 {
            return;
        }
        self.w += frames as i64;
        let last = self.w - 1;
        self.avail[0] = last;
        let m = (last - HB_C).div_euclid(2);
        self.avail[1] = self.avail[1].max(m);
        let q = (self.avail[1] - HB_C).div_euclid(2);
        self.avail[2] = self.avail[2].max(q);
        for k in 0..LEVELS {
            self.zeros[k] = self.zeros[k].saturating_add((frames >> k) as u64 + 1);
        }
    }

    fn silent(&self) -> bool {
        (0..LEVELS).all(|k| {
            let span = (self.avail[k] - (self.from >> k) + 1).clamp(0, self.cap(k)) as u64;
            self.zeros[k] >= span
        })
    }

    /// The first and last sample of level `k` a read may see.
    #[inline]
    fn span(&self, k: usize) -> (i64, i64) {
        let lo = (self.avail[k] - self.cap(k) + 1).max((self.from + (1 << k) - 1) >> k);
        (lo, self.avail[k])
    }

    /// Level `k` at position `p` (in its own samples), Hermite; `span` is
    /// [`Buf::span`]'s.
    #[inline]
    fn read(&self, k: usize, p: f64, span: (i64, i64)) -> [f32; 2] {
        let i = p.floor();
        let f = (p - i) as f32;
        let i = i as i64;
        if i > span.0 && i + 2 <= span.1 {
            // Inside: four neighbours, no test each.
            let (l, m) = (&self.lv[k], self.mask[k]);
            let b = l[i as usize & m];
            if f == 0.0 {
                return b;
            }
            let a = l[(i - 1) as usize & m];
            let c = l[(i + 1) as usize & m];
            let d = l[(i + 2) as usize & m];
            return [
                hermite(a[0], b[0], c[0], d[0], f),
                hermite(a[1], b[1], c[1], d[1], f),
            ];
        }
        let b = self.tap(k, i);
        if f == 0.0 {
            return b;
        }
        let (a, c, d) = (self.tap(k, i - 1), self.tap(k, i + 1), self.tap(k, i + 2));
        [
            hermite(a[0], b[0], c[0], d[0], f),
            hermite(a[1], b[1], c[1], d[1], f),
        ]
    }
}

/// The copy a grain transposed by `st` semitones reads, and its weight towards the next:
/// the input itself at or below its pitch; from 2 st up the half-rate copy (its band, to
/// 12 kHz at 48 kHz, read at up to twice its speed never passes Nyquist); from 14 st the
/// quarter-rate one — each crossfaded in over the two semitones before, where only the
/// top of the band could fold.
pub fn level_of(st: f32) -> (u8, f32) {
    if st <= 0.0 {
        (0, 0.0)
    } else if st < 2.0 {
        (0, st / 2.0)
    } else if st <= 12.0 {
        (1, 0.0)
    } else if st < 14.0 {
        (1, (st - 12.0) / 2.0)
    } else {
        (2, 0.0)
    }
}

/// A position in the input's samples to one in each level's.
const SCALE: [f64; LEVELS] = [1.0, 0.5, 0.25];

#[derive(Copy, Clone, Debug, Default)]
struct Grain {
    on: bool,
    age: u32,
    len: u32,
    inv_len: f32,
    /// The position its first sample reads (level 0).
    base: i64,
    speed: f64,
    k: u8,
    wk: f32,
    gl: f32,
    gr: f32,
    gain: f32,
    shape: u8,
    st: f32,
}

#[derive(Clone, Debug)]
pub struct Cloud {
    sr: f32,
    set: CloudSettings,
    seed: u32,
    on: bool,
    buf: Buf,
    grains: Box<[Grain; MAX_GRAINS]>,
}

impl Cloud {
    /// A cloud at `sr` with its buffer (allocates: off the audio thread).
    pub fn new(sr: f32) -> Cloud {
        let _ = windows();
        let _ = halfband_taps();
        Cloud {
            sr,
            set: CloudSettings::default(),
            seed: 1,
            on: false,
            buf: Buf::new((CLOUD_S * f64::from(sr)) as usize),
            grains: Box::new([Grain::default(); MAX_GRAINS]),
        }
    }

    pub fn memory_bytes(&self) -> usize {
        self.buf.lv.iter().map(|l| l.len() * 8).sum()
    }

    pub fn set(&mut self, s: &CloudSettings, seed: u32) {
        self.set = CloudSettings {
            level: s.level.clamp(0.0, 1.0),
            position_ms: s.position_ms.clamp(0.0, 2_000.0),
            spray_ms: s.spray_ms.clamp(0.0, 1_000.0),
            size_ms: s.size_ms.clamp(5.0, 500.0),
            density_hz: s.density_hz.clamp(1.0, 200.0),
            pitch_st: s.pitch_st.clamp(-24.0, 24.0),
            pitch_rand_st: s.pitch_rand_st.clamp(0.0, 24.0),
            spread: s.spread.clamp(0.0, 1.0),
            reverse: s.reverse.clamp(0.0, 1.0),
            ..*s
        };
        self.seed = seed;
        self.buf.frozen = s.freeze;
    }

    pub fn settings(&self) -> &CloudSettings {
        &self.set
    }

    /// Whether grains are made (the unit's settings say whether anything could hear them).
    pub fn enable(&mut self, on: bool) {
        if !on {
            self.grains.iter_mut().for_each(|g| g.on = false);
        }
        self.on = on;
    }

    pub fn enabled(&self) -> bool {
        self.on
    }

    pub fn frozen(&self) -> bool {
        self.buf.frozen
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.grains.iter_mut().for_each(|g| g.on = false);
    }

    /// The transport started or moved to song sample `so`: the buffer starts again there
    /// (a frozen one keeps what it holds) and no grain carries on.
    pub fn restart(&mut self, so: i64) {
        if !self.buf.frozen {
            self.buf.restart(so);
        }
        self.grains.iter_mut().for_each(|g| g.on = false);
    }

    pub fn silent(&self) -> bool {
        self.buf.silent()
    }

    pub fn live(&self) -> usize {
        self.grains.iter().filter(|g| g.on).count()
    }

    /// How far back a grain may read, samples, with these settings.
    pub fn reach(&self) -> usize {
        let s = &self.set;
        let ms = f64::from(self.sr) * 0.001;
        let len = f64::from(s.size_ms) * ms;
        let up = math::exp2(f64::from((s.pitch_st + s.pitch_rand_st).min(24.0)) / 12.0);
        let r = f64::from(s.position_ms + s.spray_ms) * ms + len * up.max(1.0) + len;
        (r.ceil() as i64 + MARGIN) as usize
    }

    /// Up to `out.len()` grains: where each reads (0 now … 1 at half of [`CLOUD_S`]
    /// back) and its pitch (−1 … 1 for ±24 st). Returns how many.
    pub fn shown(&self, out: &mut [(f32, f32)]) -> usize {
        let span = CLOUD_S * 0.5 * f64::from(self.sr);
        let now = (self.buf.w - 1) as f64;
        let mut n = 0;
        for g in self.grains.iter().filter(|g| g.on) {
            if n == out.len() {
                break;
            }
            let p = g.base as f64 + f64::from(g.age) * g.speed;
            out[n] = (((now - p) / span).clamp(0.0, 1.0) as f32, g.st / 24.0);
            n += 1;
        }
        n
    }

    /// Whether a grain starts on song sample `so`, and its number.
    #[inline]
    fn due(&self, so: i64) -> Option<i64> {
        let k = f64::from(self.set.density_hz) / f64::from(self.sr);
        let a = (so as f64 * k).floor() as i64;
        let b = ((so - 1) as f64 * k).floor() as i64;
        (a > b).then_some(a)
    }

    /// Grain `id` starts (if the pool has room), its first read `MARGIN` or more behind
    /// the newest sample.
    fn spawn(&mut self, id: i64) {
        // A grain's place in the pool is its number's: the grains sum in the same order
        // whatever played before (a render from bar 5 adds them as the whole one does).
        let slot = id.rem_euclid(MAX_GRAINS as i64) as usize;
        if self.grains[slot].on {
            return;
        }
        let s = self.set;
        let seed = self.seed;
        let h = |p: u64| roll(seed, 9, id as u64, p);
        let bip = |p: u64| 2.0 * unit(h(p)) - 1.0;
        let sr = f64::from(self.sr);
        let len = (f64::from(s.size_ms) * 0.001 * sr).round().max(16.0);
        let st = s
            .scale
            .quantise(s.pitch_st + s.pitch_rand_st * bip(2))
            .clamp(-24.0, 24.0);
        let v = math::exp2(f64::from(st) / 12.0);
        let back = s.reverse > 0.0 && unit(h(4)) < s.reverse;
        let (k, wk) = level_of(st);
        let ahead = if back {
            0.0
        } else if self.buf.frozen {
            len * v
        } else {
            len * (v - 1.0)
        };
        let want = f64::from(s.position_ms + s.spray_ms * unit(h(1))) * 0.001 * sr;
        let most = (self.buf.cap(0) - MARGIN) as f64 - len * v.max(1.0);
        let off = want.max(ahead.max(0.0) + MARGIN as f64).min(most.max(0.0));
        let p = f64::from(s.spread) * f64::from(bip(3));
        let a = (p + 1.0) * std::f64::consts::FRAC_PI_4;
        let (sa, ca) = math::sin_cos(a);
        let overlap = f64::from(s.density_hz) * len / sr;
        self.grains[slot] = Grain {
            on: true,
            age: 0,
            len: len as u32,
            inv_len: 1.0 / len as f32,
            base: self.buf.w - 1 - off.round() as i64,
            speed: if back { -v } else { v },
            k,
            wk,
            gl: (ca * std::f64::consts::SQRT_2) as f32,
            gr: (sa * std::f64::consts::SQRT_2) as f32,
            gain: (1.0 / overlap.max(1.0).sqrt()) as f32,
            shape: s.window as u8,
            st,
        };
    }

    /// One input sample into the buffer, and (enabled) the cloud's output at song sample
    /// `so`.
    #[inline]
    pub fn tick(&mut self, x: [f32; 2], so: i64) -> [f32; 2] {
        self.buf.write(x);
        if !self.on {
            return [0.0; 2];
        }
        if let Some(id) = self.due(so) {
            self.spawn(id);
        }
        let mut y = [0.0f32; 2];
        let spans = [self.buf.span(0), self.buf.span(1), self.buf.span(2)];
        for g in self.grains.iter_mut() {
            if !g.on {
                continue;
            }
            let w = window_at(usize::from(g.shape), g.age as f32 * g.inv_len) * g.gain;
            let p = g.base as f64 + f64::from(g.age) * g.speed;
            let k = usize::from(g.k);
            let mut s = self.buf.read(k, p * SCALE[k], spans[k]);
            if g.wk > 0.0 {
                let t = self.buf.read(k + 1, p * SCALE[k + 1], spans[k + 1]);
                s[0] += (t[0] - s[0]) * g.wk;
                s[1] += (t[1] - s[1]) * g.wk;
            }
            y[0] += w * g.gl * s[0];
            y[1] += w * g.gr * s[1];
            g.age += 1;
            g.on = g.age < g.len;
        }
        y
    }

    /// `frames` of silence skipped at rest from song sample `so`: the grains start and
    /// age as they would have, in the order they would have.
    pub fn skip(&mut self, frames: usize, so: i64) {
        if !self.on {
            self.buf.skip(frames);
            return;
        }
        let k = f64::from(self.set.density_hz) / f64::from(self.sr);
        let end = so + frames as i64;
        let (mut at, mut wrote) = (so, so);
        let mut id = ((so - 1) as f64 * k).floor() as i64 + 1;
        loop {
            // The first sample whose grain number reaches `id`.
            let mut s = (id as f64 / k).ceil() as i64;
            while ((s - 1) as f64 * k).floor() as i64 >= id {
                s -= 1;
            }
            while ((s as f64 * k).floor() as i64) < id {
                s += 1;
            }
            if s >= end {
                break;
            }
            let s = s.max(so);
            // As a tick: the sample written, then the grain, which sounds from it.
            self.age(s - at);
            self.buf.skip((s + 1 - wrote) as usize);
            wrote = s + 1;
            at = s;
            self.spawn(id);
            id += 1;
        }
        self.age(end - at);
        self.buf.skip((end - wrote) as usize);
    }

    fn age(&mut self, n: i64) {
        if n <= 0 {
            return;
        }
        for g in self.grains.iter_mut().filter(|g| g.on) {
            let a = u64::from(g.age) + n as u64;
            g.on = a < u64::from(g.len);
            g.age = a.min(u64::from(u32::MAX)) as u32;
        }
    }
}
