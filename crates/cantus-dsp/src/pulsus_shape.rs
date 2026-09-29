//! Pulsus's small parts: seeded noise and choices, envelope coefficients, the saturator
//! (and its 2× oversampler), a state-variable filter with a real Q, and the pan taper.
//!
//! Everything here is deterministic and allocation-free; the oversampler's taps are built
//! once when the kit is constructed.

/// Deterministic white noise (xorshift32). Pulsus I seeded it from the pad and that pad's
/// hit count, which a seek resets; model 2 seeds it from the note ([`hit_seed`]).
#[derive(Copy, Clone, Debug)]
pub(crate) struct Noise(pub(crate) u32);

impl Noise {
    /// Pulsus I's seed, kept bit-exact for model 1 kits.
    pub(crate) fn seeded(pad: usize, counter: u64) -> Noise {
        // Any non-zero state; xorshift is stuck at zero.
        let mixed = (pad as u32).wrapping_mul(2_654_435_761)
            ^ (counter as u32).wrapping_mul(40_503)
            ^ 0x9E37_79B9;
        Noise(mixed | 1)
    }

    pub(crate) fn from_seed(seed: u64) -> Noise {
        Noise(((seed ^ (seed >> 32)) as u32) | 1)
    }

    #[inline]
    pub(crate) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32) * (2.0 / u32::MAX as f32) - 1.0
    }

    /// Uniform in 0..1.
    #[inline]
    pub(crate) fn unit(&mut self) -> f32 {
        0.5 * (self.next() + 1.0)
    }
}

/// SplitMix64: one well-mixed 64-bit word from any input.
#[inline]
pub(crate) fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The seed of one hit: the kit's seed, the pad's identity, and where the note is — its
/// tick and its id. A render that starts at bar 5 computes the same seed for a hit in bar
/// 5 as a render from the top, which is the fix for Pulsus I's hit counter.
#[inline]
pub(crate) fn hit_seed(kit_seed: u32, pad_id: u16, tick: i64, note_id: u64) -> u64 {
    mix64(
        mix64(kit_seed as u64 ^ ((pad_id as u64) << 32))
            ^ mix64(tick as u64)
            ^ note_id.rotate_left(17),
    )
}

/// Which of `n` layers a round-robin hit plays, from the hit's ordinal on its pad.
///
/// - `Cycle`: `ordinal % n`.
/// - `Random`: a hash of the seed, the pad, the zone and the ordinal.
/// - `NoRepeat`: a seeded permutation per round of `n` hits, with the first of each round
///   swapped away from the last of the round before — so it never repeats, and every
///   choice is computed from the ordinal alone (a seek cannot change it).
pub(crate) fn round_robin(mode: u8, n: usize, ordinal: u32, seed: u64) -> usize {
    if n <= 1 {
        return 0;
    }
    match mode {
        1 => ordinal as usize % n,
        2 => (mix64(seed ^ ordinal as u64) % n as u64) as usize,
        3 => {
            if n == 2 {
                // Never twice in a row, with two, is alternation from a seeded start.
                return ((seed as usize & 1) + ordinal as usize) % 2;
            }
            let round = ordinal as usize / n;
            let pos = ordinal as usize % n;
            // A round's permutation that would open on the last round's closing layer
            // swaps its first two; with n > 2 the last position is never swapped, so
            // this needs nothing but the round before.
            let swap =
                round > 0 && permuted(n, round, 0, seed) == permuted(n, round - 1, n - 1, seed);
            let pos = if swap && pos < 2 { 1 - pos } else { pos };
            permuted(n, round, pos, seed)
        }
        _ => 0,
    }
}

/// Element `pos` of a seeded permutation of `0..n` (n ≤ 16), without allocating.
fn permuted(n: usize, round: usize, pos: usize, seed: u64) -> usize {
    let mut p = [0u8; 16];
    let n = n.min(16);
    for (i, v) in p.iter_mut().enumerate().take(n) {
        *v = i as u8;
    }
    let mut s = mix64(seed ^ (round as u64).wrapping_mul(0xA24B_AED4_963E_E407));
    for i in (1..n).rev() {
        s = mix64(s);
        let j = (s % (i as u64 + 1)) as usize;
        p.swap(i, j);
    }
    p[pos.min(n - 1)] as usize
}

/// Decay coefficient reaching −60 dB after `seconds`.
#[inline]
pub(crate) fn decay_coef(seconds: f32, sample_rate: f32) -> f32 {
    let n = (seconds.max(0.0005) * sample_rate).max(1.0);
    (-6.907_755_3 / n).exp()
}

#[inline]
pub(crate) fn semitones(hz: f32, st: f32) -> f32 {
    hz * 2f32.powf(st.clamp(-48.0, 48.0) / 12.0)
}

/// Soft saturation, normalised so a full-scale input stays full-scale.
///
/// Peak-preserving normalisation means the *body* comes up rather than the top coming
/// down, which is what a driven kick actually does — and it is why the knob is kept
/// modest. At `k = 1 + 3·drive` the default 0.2 lifts the tail by about 4 dB and the
/// maximum by about 12. `drive == 0` returns the sample **untouched**.
#[inline]
pub(crate) fn saturate(x: f32, drive: f32) -> f32 {
    if drive <= 0.0 {
        return x;
    }
    let k = 1.0 + drive * 3.0;
    (k * x).tanh() / k.tanh()
}

/// The drive modes (0 soft, 1 tape, 2 clip, 3 raw = soft without oversampling), each
/// peak-preserving at full scale, so the small-signal lift is at most `k` (+12 dB).
#[inline]
pub(crate) fn shape(x: f32, drive: f32, mode: u8) -> f32 {
    let k = 1.0 + drive * 3.0;
    match mode {
        1 => (k * x).atan() / k.atan(),
        2 => (k * x).clamp(-1.0, 1.0),
        _ => (k * x).tanh() / k.tanh(),
    }
}

/// Equal-power taper for a pad's pan, unity at the centre. Deliberately **not** the
/// project's pan law (ADR 0012): a pad is one source inside an instrument that is itself
/// about to be placed on a channel. Centre returns exactly `(1, 1)`.
#[inline]
pub(crate) fn pan_gains(pan: f32) -> (f32, f32) {
    let p = if pan.is_finite() {
        pan.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    if p == 0.0 {
        return (1.0, 1.0);
    }
    let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
    (
        (angle.cos() * std::f32::consts::SQRT_2).min(1.0),
        (angle.sin() * std::f32::consts::SQRT_2).min(1.0),
    )
}

/// A state-variable filter (Simper's trapezoidal form) addressed by Q rather than a 0…1
/// resonance, for the clap's band, the cymbal's bands and the pad filter.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Band {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl Band {
    #[inline]
    pub(crate) fn set(&mut self, hz: f32, q: f32, sample_rate: f32) {
        let fc = hz.clamp(10.0, sample_rate * 0.45);
        let g = (std::f32::consts::PI * fc / sample_rate).tan();
        self.k = 1.0 / q.clamp(0.3, 40.0);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    #[inline]
    pub(crate) fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    /// `(low, band, high)`.
    #[inline]
    pub(crate) fn tick(&mut self, v0: f32) -> (f32, f32, f32) {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        if self.ic1.abs() < 1e-20 {
            self.ic1 = 0.0;
        }
        if self.ic2.abs() < 1e-20 {
            self.ic2 = 0.0;
        }
        (v2, v1, v0 - self.k * v1 - v2)
    }

    /// The pad filter's modes: 1 low-pass, 2 high-pass, 3 band-pass (unity at the centre).
    #[inline]
    pub(crate) fn mode(&mut self, v0: f32, mode: u8) -> f32 {
        let (lp, bp, hp) = self.tick(v0);
        match mode {
            1 => lp,
            2 => hp,
            _ => bp * self.k,
        }
    }
}

/// Taps of the half-band filter the 2× drive uses: a Blackman-windowed sinc, 31 long.
pub(crate) const HALF_TAPS: usize = 31;

pub(crate) fn half_band_taps() -> [f32; HALF_TAPS] {
    let mut h = [0.0f32; HALF_TAPS];
    let c = (HALF_TAPS / 2) as f64;
    for (i, tap) in h.iter_mut().enumerate() {
        let n = i as f64 - c;
        let sinc = if n == 0.0 {
            0.5
        } else if (n as i64) % 2 == 0 {
            // A half-band's even taps are exactly zero; say so rather than keep 1e-17.
            0.0
        } else {
            (std::f64::consts::PI * n * 0.5).sin() / (std::f64::consts::PI * n)
        };
        let x = i as f64 / (HALF_TAPS - 1) as f64;
        let w = 0.42 - 0.5 * (std::f64::consts::TAU * x).cos()
            + 0.08 * (2.0 * std::f64::consts::TAU * x).cos();
        *tap = (sinc * w) as f32;
    }
    h
}

/// A 2× oversampled waveshaper: zero-stuff, half-band, shape at the double rate,
/// half-band, decimate — computed polyphase, so each input sample costs the half-band's
/// 16 non-zero side taps twice and two shapes, not four full 31-tap filters. The centre
/// tap makes one up-sampled phase a pure delay and one term of the decimator. Fifteen
/// samples of latency at the base rate, the same for every voice and every block size.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Oversampler {
    /// Input history, shaped even-phase history and shaped odd-phase history: rings of 16.
    x: [f32; 16],
    even: [f32; 16],
    odd: [f32; 16],
    at: usize,
}

impl Oversampler {
    pub(crate) fn reset(&mut self) {
        *self = Oversampler::default();
    }

    #[inline]
    pub(crate) fn process(&mut self, x: f32, drive: f32, mode: u8, h: &[f32; HALF_TAPS]) -> f32 {
        let n = (self.at + 1) & 15;
        self.at = n;
        self.x[n] = x;
        // Up: the even phase is the side taps over the input (×2 for the zero-stuffing);
        // the odd phase is the centre tap alone, the input seven samples back.
        let mut up = 0.0;
        for j in 0..16 {
            up += h[2 * j] * self.x[(n + 16 - j) & 15];
        }
        let up_even = 2.0 * up;
        let up_odd = self.x[(n + 16 - 7) & 15];
        self.even[n] = shape(up_even, drive, mode);
        self.odd[n] = shape(up_odd, drive, mode);
        // Down: the side taps over the shaped even phase, and the centre tap on the odd.
        let mut out = 0.5 * self.odd[(n + 16 - 8) & 15];
        for j in 0..16 {
            out += h[2 * j] * self.even[(n + 16 - j) & 15];
        }
        if out.abs() < 1e-20 {
            0.0
        } else {
            out
        }
    }
}
