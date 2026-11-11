//! Aula's own impulse responses (ADR 0112): rooms, halls, plates, springs and chambers,
//! made by Cantus itself — the same bits on every machine, at any rate, nothing to
//! download. All true stereo (left→left, left→right, right→left, right→right), each path
//! its own draw of the room, so a source off centre is heard on both sides.
//!
//! - **Rooms, chambers and halls**: early reflections (a seeded pattern that thins out),
//!   then a late field of noise in three bands (under 300 Hz, 300 Hz – 3 kHz, above), each
//!   decaying at its own RT60, building up from the reflections.
//! - **Plates**: no reflections, a field that builds in a millisecond or two, bright.
//! - **Springs**: an impulse round a loop of a delay, a chain of first-order all-pass
//!   sections (the dispersion that makes the chirp) and a low-pass, two springs of
//!   different lengths.

use crate::aula_kernel::Format;
use crate::aula_shape::Ir;
use crate::math;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Family {
    Room,
    Chamber,
    Hall,
    Plate,
    Spring,
}

/// One response of the set.
#[derive(Copy, Clone, Debug)]
pub struct Factory {
    /// Its name's locale key: `aula.ir.{key}`.
    pub key: &'static str,
    pub family: Family,
    /// RT60 under 300 Hz, between, above 3 kHz, seconds (a spring's loop decay: `[0]`).
    pub rt: [f32; 3],
    /// Early reflections: how many, the first (ms), over how long (ms), their level.
    pub er: (usize, f32, f32, f32),
    /// The late field's build-up and its start, ms.
    pub build_ms: f32,
    pub late_ms: f32,
    /// How much the cross paths carry, 0 … 1.
    pub cross: f32,
    /// A spring's two loop delays, ms.
    pub spring_ms: [f32; 2],
    pub seed: u64,
}

#[allow(clippy::too_many_arguments)]
const fn room(
    key: &'static str,
    family: Family,
    rt: [f32; 3],
    er: (usize, f32, f32, f32),
    build_ms: f32,
    late_ms: f32,
    cross: f32,
    seed: u64,
) -> Factory {
    Factory {
        key,
        family,
        rt,
        er,
        build_ms,
        late_ms,
        cross,
        spring_ms: [0.0; 2],
        seed,
    }
}

const fn spring(key: &'static str, rt: f32, spring_ms: [f32; 2], seed: u64) -> Factory {
    Factory {
        key,
        family: Family::Spring,
        rt: [rt, rt, rt],
        er: (0, 0.0, 0.0, 0.0),
        build_ms: 0.0,
        late_ms: 0.0,
        cross: 0.45,
        spring_ms,
        seed,
    }
}

/// The set, in the order the `ir` parameter counts. **Forever once shipped**: a project
/// holds the index; a new response goes at the end.
pub const FACTORY: &[Factory] = &[
    room(
        "studio",
        Family::Room,
        [0.45, 0.4, 0.3],
        (14, 3.0, 25.0, 0.7),
        8.0,
        12.0,
        0.6,
        1,
    ),
    room(
        "small",
        Family::Room,
        [0.3, 0.25, 0.18],
        (12, 2.0, 15.0, 0.8),
        5.0,
        8.0,
        0.5,
        2,
    ),
    room(
        "wood",
        Family::Room,
        [0.7, 0.55, 0.35],
        (16, 4.0, 35.0, 0.7),
        10.0,
        15.0,
        0.6,
        3,
    ),
    room(
        "live",
        Family::Room,
        [1.0, 0.9, 0.6],
        (18, 5.0, 45.0, 0.6),
        14.0,
        20.0,
        0.7,
        4,
    ),
    room(
        "ambience",
        Family::Room,
        [0.25, 0.22, 0.15],
        (8, 1.5, 10.0, 0.5),
        3.0,
        4.0,
        0.6,
        5,
    ),
    room(
        "chamber",
        Family::Chamber,
        [1.4, 1.3, 0.9],
        (24, 6.0, 50.0, 0.6),
        10.0,
        18.0,
        0.75,
        6,
    ),
    room(
        "stone",
        Family::Chamber,
        [1.9, 1.8, 1.4],
        (24, 8.0, 60.0, 0.6),
        12.0,
        22.0,
        0.75,
        7,
    ),
    room(
        "concert",
        Family::Hall,
        [2.6, 2.2, 1.4],
        (20, 12.0, 80.0, 0.45),
        30.0,
        35.0,
        0.8,
        8,
    ),
    room(
        "large",
        Family::Hall,
        [3.4, 3.0, 2.0],
        (20, 18.0, 110.0, 0.4),
        40.0,
        45.0,
        0.85,
        9,
    ),
    room(
        "cathedral",
        Family::Hall,
        [7.0, 6.0, 3.5],
        (16, 25.0, 160.0, 0.35),
        60.0,
        70.0,
        0.9,
        10,
    ),
    room(
        "plate",
        Family::Plate,
        [1.6, 1.8, 1.6],
        (0, 0.0, 0.0, 0.0),
        1.5,
        0.0,
        0.7,
        11,
    ),
    room(
        "vocal",
        Family::Plate,
        [1.0, 1.2, 1.0],
        (0, 0.0, 0.0, 0.0),
        1.5,
        0.0,
        0.7,
        12,
    ),
    room(
        "dark",
        Family::Plate,
        [2.4, 2.5, 1.2],
        (0, 0.0, 0.0, 0.0),
        2.0,
        0.0,
        0.7,
        13,
    ),
    spring("spring", 1.8, [33.0, 41.0], 14),
    spring("long_spring", 3.2, [47.0, 59.0], 15),
];

/// The response the parameter's index names (clamped into the set), at `rate`.
pub fn factory(index: usize, rate: f32) -> Ir {
    let f = &FACTORY[index.min(FACTORY.len() - 1)];
    let channels = match f.family {
        Family::Spring => springs(f, rate),
        _ => (0..4).map(|c| field(f, rate, c)).collect(),
    };
    Ir {
        rate,
        format: Format::TrueStereo,
        channels,
    }
}

struct Noise(u64);

impl Noise {
    fn new(seed: u64) -> Noise {
        let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xa017_2b11_d3c5_4e09;
        if s == 0 {
            s = 1;
        }
        Noise(s)
    }
    /// Uniform −√3 … √3 (unit variance).
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 1.732_050_8
    }
    fn unit(&mut self) -> f32 {
        0.5 + self.next() / (2.0 * 1.732_050_8)
    }
}

/// A one-pole's coefficient for a corner at `hz`.
fn pole(hz: f32, rate: f32) -> f32 {
    1.0 - math::exp(-std::f32::consts::TAU * hz / rate)
}

/// Path `c` (0 L→L, 1 L→R, 2 R→L, 3 R→R) of a room, chamber, hall or plate.
fn field(f: &Factory, rate: f32, c: usize) -> Vec<f32> {
    let cross = c == 1 || c == 2;
    let longest = f.rt.iter().fold(0.0f32, |m, v| m.max(*v));
    let len = ((longest * 1.1 + f.late_ms / 1000.0 + 0.05) * rate).max(0.2 * rate) as usize;
    let mut out = vec![0.0f32; len];
    let mut rng = Noise::new(f.seed * 16 + c as u64);
    // Early reflections: a blob of three taps each, thinning out.
    let (count, first, spread, level) = f.er;
    for k in 0..count {
        let x = (k + 1) as f32 / count as f32;
        let jitter = (rng.unit() - 0.5) * spread / count as f32;
        let ms = first + spread * math::powf(x, 1.3) + jitter;
        let at = (ms.max(0.5) / 1000.0 * rate) as usize;
        let sign = if rng.next() >= 0.0 { 1.0 } else { -1.0 };
        let side = if cross { f.cross * 0.8 } else { 1.0 };
        let g = level
            * math::exp(-1.5 * k as f32 / count as f32)
            * sign
            * side
            * (0.6 + 0.4 * rng.unit());
        for (d, w) in [0.25f32, 0.5, 0.25].iter().enumerate() {
            if let Some(v) = out.get_mut(at + d) {
                *v += g * w;
            }
        }
    }
    // The late field.
    let start = (f.late_ms / 1000.0 * rate) as usize;
    let (a1, a2) = (pole(300.0, rate), pole(3_000.0, rate));
    let r = |rt: f32| math::exp(-6.907_755 / (rt.max(0.05) * rate));
    let (rl, rm, rh) = (r(f.rt[0]), r(f.rt[1]), r(f.rt[2]));
    let rb = math::exp(-1.0 / (f.build_ms.max(0.1) / 1000.0 * rate));
    let (mut el, mut em, mut eh, mut q) = (1.0f32, 1.0f32, 1.0f32, 1.0f32);
    let (mut lp1, mut lp2) = (0.0f32, 0.0f32);
    // The late field's level against the reflections: a plate is all field.
    let late =
        if f.family == Family::Plate { 1.0 } else { 0.35 } * if cross { f.cross } else { 1.0 };
    for v in out.iter_mut().skip(start) {
        let w = rng.next();
        lp1 += a1 * (w - lp1);
        let rest = w - lp1;
        lp2 += a2 * (rest - lp2);
        let high = rest - lp2;
        *v += late * (lp1 * el + lp2 * em + high * eh) * (1.0 - q);
        el *= rl;
        em *= rm;
        eh *= rh;
        q *= rb;
    }
    fade_end(&mut out, rate);
    out
}

/// Two springs, each an impulse round a loop: delay, a chain of all-passes, a low-pass.
fn springs(f: &Factory, rate: f32) -> Vec<Vec<f32>> {
    let len = (f.rt[0] * 1.1 * rate) as usize;
    let one = |delay_ms: f32, a: f32| -> Vec<f32> {
        const STAGES: usize = 80;
        let d = ((delay_ms / 1000.0 * rate) as usize).max(1);
        let g = math::powf(10.0f32, -3.0 * d as f32 / (f.rt[0] * rate));
        let mut line = vec![0.0f32; d];
        let mut pos = 0;
        let (mut xs, mut ys) = ([0.0f32; STAGES], [0.0f32; STAGES]);
        let lp_a = pole(4_500.0, rate);
        let (mut lp, mut hp_x, mut hp_y) = (0.0f32, 0.0f32, 0.0f32);
        let hp_r = math::exp(-std::f32::consts::TAU * 120.0 / rate);
        let mut out = vec![0.0f32; len];
        for (n, o) in out.iter_mut().enumerate() {
            lp += lp_a * (line[pos] - lp);
            let mut v = if n == 0 { 1.0 } else { 0.0 } + g * lp;
            for s in 0..STAGES {
                let y = a * v + xs[s] - a * ys[s];
                xs[s] = v;
                ys[s] = y;
                v = y;
            }
            line[pos] = v;
            pos = (pos + 1) % d;
            // A gentle high-pass: springs are thin.
            let y = hp_r * (hp_y + v - hp_x);
            hp_x = v;
            hp_y = y;
            *o = y;
        }
        fade_end(&mut out, rate);
        out
    };
    let a = one(f.spring_ms[0], 0.62);
    let b = one(f.spring_ms[1], 0.66);
    let lr: Vec<f32> = b.iter().map(|v| v * f.cross).collect();
    let rl: Vec<f32> = a.iter().map(|v| v * f.cross).collect();
    vec![a, lr, rl, b]
}

/// The last 10 % (at most 200 ms) faded out, so nothing ends on a cut.
fn fade_end(x: &mut [f32], rate: f32) {
    let n = x.len();
    let f = (n / 10).min((0.2 * rate) as usize).max(1);
    for i in 0..f {
        let g = math::cos(std::f32::consts::FRAC_PI_2 * (i + 1) as f32 / f as f32);
        x[n - f + i] *= g * g;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_response_is_finite_true_stereo_and_the_same_bits_twice() {
        for (i, f) in FACTORY.iter().enumerate() {
            let a = factory(i, 48_000.0);
            assert_eq!(a.format, Format::TrueStereo);
            assert_eq!(a.channels.len(), 4);
            assert!(
                a.channels.iter().flatten().all(|v| v.is_finite()),
                "{}",
                f.key
            );
            let peak = a
                .channels
                .iter()
                .flatten()
                .fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.01, "{} is silent", f.key);
            let b = factory(i, 48_000.0);
            assert!(a == b, "{} is not deterministic", f.key);
        }
    }

    #[test]
    fn longer_rooms_ring_longer() {
        let energy_after = |i: usize, s: f32| -> f64 {
            let ir = factory(i, 48_000.0);
            let from = (s * 48_000.0) as usize;
            ir.channels[0]
                .iter()
                .skip(from)
                .map(|v| (*v as f64) * (*v as f64))
                .sum()
        };
        let idx = |k: &str| FACTORY.iter().position(|f| f.key == k).unwrap();
        assert!(energy_after(idx("cathedral"), 1.0) > energy_after(idx("concert"), 1.0));
        assert!(energy_after(idx("concert"), 0.5) > energy_after(idx("small"), 0.5));
    }
}
