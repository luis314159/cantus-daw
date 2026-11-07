//! Chorda's second model's mechanical noises (ADR 0098): what a grand's action adds to its
//! strings. Each is a force on the bridge's side of the instrument — it reaches the
//! listener through the body's responses, which turn an impulse into the board's knock.
//!
//! - **The hammer's blow**, the felt's force itself carried into the frame and the board
//!   (a share of it, `voice.rs`), the strike's broadband knock;
//! - **the key bottoming** on the keybed and the hammer's shank: a short, low thud — two
//!   damped modes of the action and keyframe (≈ 70 and 180 Hz) and a burst of felt-filtered
//!   noise — growing with the key's speed;
//! - **the damper landing** when a key comes up: a softer thud and a brush of felt on the
//!   strings.
//!
//! Every noise is seeded from the key and the note's id (never the unit's clock, never
//! random): the same song renders the same bits, and two strikes of a key never sound
//! exactly alike.

use crate::math;

/// A small xorshift: deterministic noise, −1…1.
#[derive(Clone, Copy, Debug)]
struct Rng(u64);

impl Rng {
    #[inline]
    fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// A seed from a key and a count.
pub fn seed(key: u8, count: u32) -> u64 {
    let mut z = (key as u64) << 32 | count as u64;
    z = z.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC0FF_EE11;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) | 1
}

/// −1…1 from a seed (one draw).
pub fn draw(seed: u64) -> f64 {
    Rng(seed).next()
}

/// Two damped modes and a noise burst, struck.
#[derive(Clone, Debug)]
pub struct Thud {
    /// Per mode: `2r cos w`, `−r²`, and the states.
    c: [[f64; 2]; 2],
    y: [[f64; 2]; 2],
    /// The noise: its envelope, the envelope's fall per sample, its two filters.
    env: f64,
    fall: f64,
    lp: f64,
    lp_a: f64,
    hp_x: f64,
    hp_y: f64,
    hp_a: f64,
    rng: Rng,
    noise_gain: f64,
    busy: bool,
}

/// Below this (N) a thud has ended.
const QUIET: f64 = 1.0e-12;

impl Thud {
    /// Modes at `f` Hz with T60 `t` s, the noise band `lo … hi` Hz falling over `tau` s.
    pub fn new(fs: f64, f: [f64; 2], t: [f64; 2], lo: f64, hi: f64, tau: f64) -> Thud {
        let mut c = [[0.0; 2]; 2];
        for m in 0..2 {
            let r = math::exp(-6.907_755 / (t[m] * fs));
            let w = std::f64::consts::TAU * f[m] / fs;
            c[m] = [2.0 * r * math::cos(w), -r * r];
        }
        let k = |hz: f64| math::exp(-std::f64::consts::TAU * hz / fs);
        Thud {
            c,
            y: [[0.0; 2]; 2],
            env: 0.0,
            fall: math::exp(-1.0 / (tau * fs)),
            lp: 0.0,
            lp_a: k(hi),
            hp_x: 0.0,
            hp_y: 0.0,
            hp_a: k(lo),
            rng: Rng(1),
            noise_gain: 0.0,
            busy: false,
        }
    }

    /// Strike: the modes kicked by `amp` (N·s-ish: an impulse), the noise at `noise`,
    /// seeded by `seed`.
    pub fn strike(&mut self, amp: [f64; 2], noise: f64, seed: u64) {
        for (y, a) in self.y.iter_mut().zip(amp) {
            y[0] += a;
        }
        self.env = 1.0;
        self.noise_gain = noise;
        self.rng = Rng(seed);
        self.busy = amp[0] != 0.0 || amp[1] != 0.0 || noise != 0.0;
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn silence(&mut self) {
        self.y = [[0.0; 2]; 2];
        self.env = 0.0;
        self.lp = 0.0;
        self.hp_x = 0.0;
        self.hp_y = 0.0;
        self.busy = false;
    }

    #[inline]
    pub fn tick(&mut self) -> f64 {
        if !self.busy {
            return 0.0;
        }
        let mut out = 0.0;
        for m in 0..2 {
            let [a, b] = self.c[m];
            let y = a * self.y[m][0] + b * self.y[m][1];
            out += self.y[m][0];
            self.y[m][1] = self.y[m][0];
            self.y[m][0] = y;
        }
        // The noise, then its filters (which ring down after it, so they run on).
        let n = if self.env > 1e-9 {
            let n = self.rng.next() * self.env * self.noise_gain;
            self.env *= self.fall;
            n
        } else {
            self.env = 0.0;
            0.0
        };
        self.lp = n + self.lp_a * (self.lp - n);
        let hp = self.hp_a * (self.hp_y + self.lp - self.hp_x);
        self.hp_x = self.lp;
        self.hp_y = hp;
        out += hp;
        let modes =
            self.y[0][0].abs() + self.y[0][1].abs() + self.y[1][0].abs() + self.y[1][1].abs();
        if modes < QUIET && self.env == 0.0 && self.hp_y.abs() < QUIET && self.lp.abs() < QUIET {
            self.silence();
        }
        out
    }
}
