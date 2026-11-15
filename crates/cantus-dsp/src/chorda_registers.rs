//! Model 2's voicing by register (ADR 0098, third amendment): what a technician does when
//! the bass, the middle and the treble each want their own colour — the hammers of each
//! section needled or hardened apart. The keys' bridge forces are summed into three buses
//! (bass, middle, treble), each key crossfaded between the two nearest by its place, and
//! each bus passes a short chain of peaking filters before the body. The cost is fixed
//! (three buses × [`BANDS`] stereo filters), never per voice, and a bus whose gains are all
//! 0 dB is not filtered: at a flat voicing the sum is the keys' own, to the bit.

use crate::math;

/// The bands' centres (Hz), an octave apart (round 40: seven, from the bass's second
/// partials to the bite — round 38's four at 125, 500, 1 600 and 4 000 Hz could not reach
/// the bass's 63 Hz nor its 250 Hz apart from its 500).
pub const CENTRES: [f64; BANDS] = [63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0];
pub const BANDS: usize = 7;
/// The registers' centre keys (MIDI): the bass (A0…B2's middle), the middle (C3…B5's),
/// the treble (C6…C8's). A key between two centres feeds both, linearly.
pub const CENTRE_KEYS: [f64; 3] = [34.0, 65.0, 96.0];
/// Each band's width: an octave (a graphic equaliser's).
const Q: f64 = std::f64::consts::SQRT_2;

/// One peaking filter, stereo (RBJ's cookbook, normalised by `a0`).
#[derive(Clone, Copy, Debug, Default)]
struct Peak {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    /// Direct form II transposed, per channel.
    z: [[f64; 2]; 2],
}

impl Peak {
    fn new(fs: f64, f: f64, db: f64) -> Peak {
        Peak::with_q(fs, f, db, Q)
    }

    fn with_q(fs: f64, f: f64, db: f64, q: f64) -> Peak {
        let a = math::powf(10.0f64, db / 40.0);
        let w = std::f64::consts::TAU * f / fs;
        let (s, c) = (math::sin(w), math::cos(w));
        let alpha = s / (2.0 * q.max(0.1));
        let a0 = 1.0 + alpha / a;
        Peak {
            b0: (1.0 + alpha * a) / a0,
            b1: -2.0 * c / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: -2.0 * c / a0,
            a2: (1.0 - alpha / a) / a0,
            z: [[0.0; 2]; 2],
        }
    }

    /// Its response at `e^{jw}` given `(cos w, sin w, cos 2w, sin 2w)`: (re, im).
    fn at(&self, t: (f64, f64, f64, f64)) -> (f64, f64) {
        let (c1, s1, c2, s2) = t;
        let (nr, ni) = (
            self.b0 + self.b1 * c1 + self.b2 * c2,
            -self.b1 * s1 - self.b2 * s2,
        );
        let (dr, di) = (
            1.0 + self.a1 * c1 + self.a2 * c2,
            -self.a1 * s1 - self.a2 * s2,
        );
        let d = dr * dr + di * di;
        ((nr * dr + ni * di) / d, (ni * dr - nr * di) / d)
    }

    #[inline]
    fn tick(&mut self, ch: usize, x: f64) -> f64 {
        let z = &mut self.z[ch];
        let y = self.b0 * x + z[0];
        z[0] = self.b1 * x - self.a1 * y + z[1];
        z[1] = self.b2 * x - self.a2 * y;
        y
    }
}

/// The three buses' filters and their scratch.
#[derive(Clone, Debug)]
pub struct Registers {
    /// Per register: its bands (a 0 dB band is left out) and whether any band is not flat.
    bands: [[Peak; BANDS]; 3],
    live: [[bool; BANDS]; 3],
    /// Per register: its level (linear; exactly 1 at 0 dB, then not applied) — the
    /// registers' balance (round 40: the bass stood 3.6 LU over the middle against the
    /// reference's balance).
    level: [f64; 3],
    /// The bass register's formant (round 40): one more peak, its own Q; off at 0 dB.
    formant: Peak,
    formant_live: bool,
    /// The bass register's high-pass (round 40: under the lowest notes); off at 0 Hz.
    hp: Peak,
    hp_live: bool,
    /// Its second section (a fourth-order high-pass; off: second order).
    hp2: Peak,
    hp2_live: bool,
    any: bool,
}

impl Registers {
    /// The filters for `gains` (dB, per register and band) at `fs`, each register at 0 dB.
    pub fn new(fs: f64, gains: &[[f64; BANDS]; 3]) -> Registers {
        Registers::with_levels(fs, gains, &[0.0; 3])
    }

    /// [`Self::new`] with each register's level (dB).
    pub fn with_levels(fs: f64, gains: &[[f64; BANDS]; 3], levels: &[f64; 3]) -> Registers {
        Registers::voiced(fs, gains, levels, &[600.0, 0.0, 3.0])
    }

    /// [`Self::with_levels`] with the bass register's formant (Hz, dB, Q).
    pub fn voiced(
        fs: f64,
        gains: &[[f64; BANDS]; 3],
        levels: &[f64; 3],
        formant: &[f64; 3],
    ) -> Registers {
        let mut bands = [[Peak::default(); BANDS]; 3];
        let mut live = [[false; BANDS]; 3];
        for r in 0..3 {
            for b in 0..BANDS {
                live[r][b] = gains[r][b] != 0.0;
                bands[r][b] = Peak::new(fs, CENTRES[b], gains[r][b]);
            }
        }
        let level = levels.map(|db| {
            if db == 0.0 {
                1.0
            } else {
                math::powf(10.0f64, db / 20.0)
            }
        });
        let formant_live = formant[1] != 0.0;
        let formant = Peak::with_q(fs, formant[0], formant[1], formant[2]);
        let any =
            live.iter().flatten().any(|x| *x) || level.iter().any(|g| *g != 1.0) || formant_live;
        Registers {
            bands,
            live,
            level,
            formant,
            formant_live,
            hp: Peak::default(),
            hp_live: false,
            hp2: Peak::default(),
            hp2_live: false,
            any,
        }
    }

    /// The power gain (`|H|²`) at `f` Hz of a key whose shares of the registers are `w`
    /// (round 40: what the keyboard's evening weighs a partial with). 1 when none is
    /// voiced. Allocates nothing.
    pub fn power(&self, w: &[f64; 3], f: f64, fs: f64) -> f64 {
        if !self.any {
            return 1.0;
        }
        let x = std::f64::consts::TAU * f / fs;
        let t = (
            math::cos(x),
            math::sin(x),
            math::cos(2.0 * x),
            math::sin(2.0 * x),
        );
        let mul = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
        let (mut re, mut im) = (0.0, 0.0);
        for (r, share) in w.iter().enumerate() {
            if *share == 0.0 {
                continue;
            }
            let mut h = (share * self.level[r], 0.0);
            if r == 0 && self.formant_live {
                h = mul(h, self.formant.at(t));
            }
            if r == 0 && self.hp_live {
                h = mul(h, self.hp.at(t));
                if self.hp2_live {
                    h = mul(h, self.hp2.at(t));
                }
            }
            for (live, band) in self.live[r].iter().zip(&self.bands[r]) {
                if *live {
                    h = mul(h, band.at(t));
                }
            }
            re += h.0;
            im += h.1;
        }
        re * re + im * im
    }

    /// With the bass register's high-pass at `hz` (second order, Butterworth; 0: none).
    pub fn with_bass_hp(self, fs: f64, hz: f64) -> Registers {
        self.with_bass_hp_order(fs, hz, 2.0)
    }

    /// [`Self::with_bass_hp`] of `order` 2 or 4 (4: two of its sections in series, a
    /// fourth-order Linkwitz–Riley-like slope; anything under 4 is second order).
    pub fn with_bass_hp_order(mut self, fs: f64, hz: f64, order: f64) -> Registers {
        self.hp_live = hz > 0.0;
        if self.hp_live {
            let w = std::f64::consts::TAU * hz / fs;
            let (s, c) = (math::sin(w), math::cos(w));
            let alpha = s / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
            let a0 = 1.0 + alpha;
            self.hp = Peak {
                b0: 0.5 * (1.0 + c) / a0,
                b1: -(1.0 + c) / a0,
                b2: 0.5 * (1.0 + c) / a0,
                a1: -2.0 * c / a0,
                a2: (1.0 - alpha) / a0,
                z: [[0.0; 2]; 2],
            };
            self.hp2_live = order >= 4.0;
            if self.hp2_live {
                self.hp2 = self.hp;
            }
            self.any = true;
        }
        self
    }

    /// Whether any register is voiced (otherwise the engine sums the keys directly).
    pub fn active(&self) -> bool {
        self.any
    }

    /// Key `m`'s (MIDI) share of each register: at most two non-zero, summing to 1.
    pub fn weights(m: u8) -> [f64; 3] {
        Registers::weights_at(m, &CENTRE_KEYS)
    }

    /// [`Self::weights`] with the registers' centre keys `centres` (ascending).
    pub fn weights_at(m: u8, centres: &[f64; 3]) -> [f64; 3] {
        let x = m as f64;
        let [b, mid, t] = *centres;
        if x <= b {
            [1.0, 0.0, 0.0]
        } else if x < mid {
            let u = (x - b) / (mid - b);
            [1.0 - u, u, 0.0]
        } else if x < t {
            let u = (x - mid) / (t - mid);
            [0.0, 1.0 - u, u]
        } else {
            [0.0, 0.0, 1.0]
        }
    }

    /// Register `r`'s bus, in place: its bands in series.
    #[inline]
    pub fn filter(&mut self, r: usize, l: &mut [f64], rt: &mut [f64]) {
        let g = self.level[r];
        if g != 1.0 {
            for x in l.iter_mut().chain(rt.iter_mut()) {
                *x *= g;
            }
        }
        if r == 0 && self.formant_live {
            let p = &mut self.formant;
            for x in l.iter_mut() {
                *x = p.tick(0, *x);
            }
            for x in rt.iter_mut() {
                *x = p.tick(1, *x);
            }
        }
        if r == 0 && self.hp_live {
            let p = &mut self.hp;
            for x in l.iter_mut() {
                *x = p.tick(0, *x);
            }
            for x in rt.iter_mut() {
                *x = p.tick(1, *x);
            }
            if self.hp2_live {
                let p = &mut self.hp2;
                for x in l.iter_mut() {
                    *x = p.tick(0, *x);
                }
                for x in rt.iter_mut() {
                    *x = p.tick(1, *x);
                }
            }
        }
        for b in 0..BANDS {
            if !self.live[r][b] {
                continue;
            }
            let p = &mut self.bands[r][b];
            for x in l.iter_mut() {
                *x = p.tick(0, *x);
            }
            for x in rt.iter_mut() {
                *x = p.tick(1, *x);
            }
        }
    }

    pub fn at_rest(&self) -> bool {
        self.bands.iter().flatten().all(|p| p.z == [[0.0; 2]; 2])
            && self.formant.z == [[0.0; 2]; 2]
            && self.hp.z == [[0.0; 2]; 2]
            && self.hp2.z == [[0.0; 2]; 2]
    }

    /// Every filter's state cleared.
    pub fn reset(&mut self) {
        for p in self.bands.iter_mut().flatten() {
            p.z = [[0.0; 2]; 2];
        }
        self.formant.z = [[0.0; 2]; 2];
        self.hp.z = [[0.0; 2]; 2];
        self.hp2.z = [[0.0; 2]; 2];
    }

    /// With nothing driving them: states under `floor` flushed to 0 (the engine's grid).
    pub fn flush(&mut self, floor: f64) {
        for p in self
            .bands
            .iter_mut()
            .flatten()
            .chain(std::iter::once(&mut self.formant))
            .chain(std::iter::once(&mut self.hp))
            .chain(std::iter::once(&mut self.hp2))
        {
            for z in p.z.iter_mut().flatten() {
                if z.abs() < floor {
                    *z = 0.0;
                }
            }
        }
    }
}
