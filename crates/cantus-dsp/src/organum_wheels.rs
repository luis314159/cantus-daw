//! The tone generator: 91 tonewheels on one synchronous shaft, and the contacts that
//! connect a key to them.
//!
//! **Tuning.** A wheel's pitch is the motor's speed × its gear ratio × its number of teeth.
//! The motor turns at 1200 rpm on 60 Hz mains (20 turns a second); twelve gear pairs, one
//! per pitch class, and teeth doubling each octave (2, 4, … 128) give wheels 1–84; the top
//! seven (85–91) have 192 teeth and borrow the gears of F…B to land on C8…F♯8. So the
//! organ is *almost* equal-tempered — within 0.71 cent below C8, and up to +2 cents in the
//! top seven — and that small, fixed mistuning is part of its sound (`Tuning::Gear`).
//! `Tuning::Equal` is exact equal temperament instead. `a4` scales either (for Gear, as if
//! the motor ran faster or slower).
//!
//! **Phase.** Each wheel is a 32-bit phase accumulator: the increment is `hz / sr · 2³²`,
//! so the frequency is exact to 1.1e-5 Hz at 48 kHz (0.0006 cent at 32.7 Hz), the wheels
//! never stop and are never retriggered, and adding the increment `n` times is the same
//! as adding `n ·` it once — block-size independent by integer arithmetic.
//!
//! **Waveform.** Near-pure sines from a 2048-point table, linearly interpolated (error
//! below −118 dB). The lowest octave's wheels are not pure sines on the instrument; we
//! give them a third harmonic at −22 dB (our estimate, not a measurement).

// Parallel per-wheel / per-mode arrays read clearest by index (as in `spatium_*`).
#![allow(clippy::needless_range_loop)]

/// Tonewheels in the generator.
use crate::math;

pub const WHEELS: usize = 91;
/// MIDI note of wheel 1 (C1, 32.7 Hz).
pub const FIRST_WHEEL_MIDI: i32 = 24;
/// MIDI note of the top wheel (F♯8).
pub const LAST_WHEEL_MIDI: i32 = FIRST_WHEEL_MIDI + WHEELS as i32 - 1;

/// Driving/driven teeth of the twelve gear pairs, C first (the ratios as commonly
/// published for the B-3's generator).
pub const GEAR_RATIOS: [(u32, u32); 12] = [
    (85, 104),
    (71, 82),
    (67, 73),
    (105, 108),
    (103, 100),
    (84, 77),
    (74, 64),
    (98, 80),
    (96, 74),
    (88, 64),
    (67, 46),
    (108, 70),
];

/// Turns a second of the synchronous motor on 60 Hz mains.
pub const MOTOR_RPS: f64 = 20.0;

/// How the wheels are tuned.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Tuning {
    /// The gear ratios (the instrument's own, slightly off equal temperament).
    #[default]
    Gear,
    /// Exact equal temperament.
    Equal,
}

impl Tuning {
    pub fn from_index(i: f32) -> Tuning {
        if i.round() as i32 == 1 {
            Tuning::Equal
        } else {
            Tuning::Gear
        }
    }
}

/// Teeth on wheel `w` (0-based).
pub fn teeth(w: usize) -> u32 {
    if w < 84 {
        2 << (w / 12)
    } else {
        192
    }
}

/// The gear pair driving wheel `w` (0-based): its pitch class's, except the top seven,
/// which take the gear five classes up (F for C8 … B for F♯8).
pub fn gear(w: usize) -> (u32, u32) {
    if w < 84 {
        GEAR_RATIOS[w % 12]
    } else {
        GEAR_RATIOS[(w % 12) + 5]
    }
}

/// Wheel `w`'s pitch from its gear at 60 Hz mains, Hz.
pub fn gear_hz(w: usize) -> f64 {
    let (a, b) = gear(w);
    MOTOR_RPS * a as f64 / b as f64 * teeth(w) as f64
}

/// Equal temperament for wheel `w` at `a4`.
pub fn equal_hz(w: usize, a4: f64) -> f64 {
    let midi = FIRST_WHEEL_MIDI + w as i32;
    a4 * math::powf(2f64, (midi - 69) as f64 / 12.0)
}

/// Wheel `w`'s pitch under a tuning and a reference.
pub fn wheel_hz(w: usize, tuning: Tuning, a4: f64) -> f64 {
    match tuning {
        Tuning::Gear => gear_hz(w) * a4 / 440.0,
        Tuning::Equal => equal_hz(w, a4),
    }
}

/// Cents of `hz` from `reference`.
pub fn cents(hz: f64, reference: f64) -> f64 {
    1200.0 * math::log2(hz / reference)
}

/// The wheel (0-based) a MIDI note reaches, folding back an octave at a time past either
/// end of the generator, as the instrument's wiring does.
pub fn wheel_of(midi: i32) -> usize {
    let mut n = midi;
    while n < FIRST_WHEEL_MIDI {
        n += 12;
    }
    while n > LAST_WHEEL_MIDI {
        n -= 12;
    }
    (n - FIRST_WHEEL_MIDI) as usize
}

/// Where each drawbar sits above the key's 8′ pitch, semitones: 16′, 5⅓′, 8′, 4′, 2⅔′, 2′,
/// 1⅗′, 1⅓′, 1′ — harmonics ½, 1½, 1, 2, 3, 4, 5, 6, 8 of the 8′.
pub const BAR_SEMITONES: [i32; 9] = [-12, 7, 0, 12, 19, 24, 28, 31, 36];
/// The footages as organists print them.
pub const FOOTAGES: [&str; 9] = [
    "16\u{2032}",
    "5\u{2153}\u{2032}",
    "8\u{2032}",
    "4\u{2032}",
    "2\u{2154}\u{2032}",
    "2\u{2032}",
    "1\u{2157}\u{2032}",
    "1\u{2153}\u{2032}",
    "1\u{2032}",
];

/// The pedal's two drawbars are mixtures (as on the instrument): 16′ is the 16′ with a
/// little of its quint, 8′ the 8′ with a little 4′ and 2⅔′. Our voicing of them, as
/// `(semitones above the key, level)`; not measured.
pub const PEDAL_BARS: [[(i32, f32); 3]; 2] = [
    [(-12, 1.0), (7, 0.25), (0, 0.0)],
    [(0, 1.0), (12, 0.25), (19, 0.18)],
];

/// The drawbar's level for a position 0…8: 3 dB a step, 8 = unity, 0 = off; continuous
/// between the stops (a modulated drawbar glides).
pub fn drawbar_gain(position: f32) -> f32 {
    let p = position.clamp(0.0, 8.0);
    if p >= 1.0 {
        math::powf(10f32, -3.0 * (8.0 - p) / 20.0)
    } else {
        p * math::powf(10f32, -21.0 / 20.0)
    }
}

/// A drawbar's attenuation in dB (for printing), −∞ at 0.
pub fn drawbar_db(position: f32) -> f32 {
    20.0 * math::log10(drawbar_gain(position).max(1e-9))
}

/// Wheels that leak into a contact on wheel `w`: the other wheels of its compartment (same
/// pitch class, two and four octaves apart, and six), as `(wheel, relative level)`; the
/// level is multiplied by the leakage setting. The instrument's crosstalk comes from wheels
/// sharing a compartment and from the matching transformer; this is our model of the
/// first, not a measurement.
pub fn leak_partners(w: usize) -> [(usize, f32); 6] {
    let mut out = [(w, 0.0); 6];
    let mut n = 0;
    for (octaves, rel) in [
        (-2i32, 1.0f32),
        (2, 1.0),
        (-4, 0.5),
        (4, 0.5),
        (-6, 0.25),
        (6, 0.25),
    ] {
        let p = w as i32 + octaves * 12;
        if (0..WHEELS as i32).contains(&p) {
            out[n] = (p as usize, rel);
            n += 1;
        }
    }
    out
}

/// Points in the sine table (a power of two).
pub const TABLE_BITS: u32 = 11;
pub const TABLE: usize = 1 << TABLE_BITS;

/// The generator: phases, increments and the sine table.
#[derive(Clone, Debug)]
pub struct Generator {
    table: Box<[f32]>,
    pub phase: [u32; WHEELS],
    pub inc: [u32; WHEELS],
    /// This sample's value of each wheel.
    pub out: [f32; WHEELS],
    hz: [f64; WHEELS],
    sr: f64,
}

/// A fixed, arbitrary starting phase per wheel (the wheels were never aligned).
fn start_phase(w: usize) -> u32 {
    let mut z = (w as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z >> 32) as u32
}

impl Generator {
    pub fn new(sample_rate: f32) -> Self {
        let table: Box<[f32]> = (0..=TABLE)
            .map(|i| math::sin(std::f64::consts::TAU * i as f64 / TABLE as f64) as f32)
            .collect();
        let mut g = Generator {
            table,
            phase: [0; WHEELS],
            inc: [0; WHEELS],
            out: [0.0; WHEELS],
            hz: [0.0; WHEELS],
            sr: sample_rate.max(1.0) as f64,
        };
        g.reset();
        g.tune(Tuning::Gear, 440.0);
        g
    }

    pub fn reset(&mut self) {
        for w in 0..WHEELS {
            self.phase[w] = start_phase(w);
        }
        self.out = [0.0; WHEELS];
    }

    /// Set every wheel's increment. Cheap enough for the audio thread (91 `powf` at most).
    pub fn tune(&mut self, tuning: Tuning, a4: f64) {
        for w in 0..WHEELS {
            let hz = wheel_hz(w, tuning, a4).min(self.sr * 0.49);
            self.hz[w] = hz;
            self.inc[w] = (hz / self.sr * 4_294_967_296.0).round() as u32;
        }
    }

    /// The frequency wheel `w` actually runs at (after the phase increment's rounding).
    pub fn actual_hz(&self, w: usize) -> f64 {
        self.inc[w] as f64 / 4_294_967_296.0 * self.sr
    }

    #[inline]
    fn lookup(&self, phase: u32) -> f32 {
        let i = (phase >> (32 - TABLE_BITS)) as usize;
        let frac = ((phase >> (32 - TABLE_BITS - 16)) & 0xFFFF) as f32 * (1.0 / 65_536.0);
        let a = self.table[i];
        a + (self.table[i + 1] - a) * frac
    }

    /// Advance every wheel one sample and compute its value.
    #[inline]
    pub fn tick(&mut self) {
        for w in 0..WHEELS {
            let p = self.phase[w].wrapping_add(self.inc[w]);
            self.phase[w] = p;
            self.out[w] = self.lookup(p);
        }
        // The lowest octave's complex wheels.
        for w in 0..12 {
            let p3 = self.phase[w].wrapping_mul(3);
            self.out[w] += 0.08 * self.lookup(p3);
        }
    }

    /// Advance every wheel `n` samples without computing values (silence): the same phases
    /// a per-sample `tick` would reach.
    pub fn skip(&mut self, n: usize) {
        let n = n as u32;
        for w in 0..WHEELS {
            self.phase[w] = self.phase[w].wrapping_add(self.inc[w].wrapping_mul(n));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gears_land_within_the_published_errors() {
        // The lowest octave (2 teeth, 20 turns a second) against equal temperament: every
        // class within 0.71 cent, A exact.
        let expected = [
            -0.58, -0.68, 0.20, -0.09, -0.14, -0.68, 0.03, 0.02, -0.71, 0.0, -0.29, -0.59,
        ];
        for (w, e) in expected.iter().enumerate() {
            let c = cents(gear_hz(w), equal_hz(w, 440.0));
            assert!(
                (c - e).abs() < 0.006,
                "wheel {} class {w}: {c:.3} ct, published {e}",
                w + 1
            );
        }
        assert_eq!(gear_hz(9), 55.0, "A1 is exact");
        // The top seven borrow F…B: wheel 91 is 5924.57 Hz.
        assert!((gear_hz(90) - 5924.571).abs() < 0.01, "{}", gear_hz(90));
        assert!((gear_hz(84) - 4189.091).abs() < 0.01, "{}", gear_hz(84));
    }

    #[test]
    fn the_phase_increment_keeps_the_gear_pitch_to_a_hundredth_of_a_cent() {
        for sr in [44_100.0f32, 48_000.0, 96_000.0] {
            let g = Generator::new(sr);
            for w in 0..WHEELS {
                let c = cents(g.actual_hz(w), gear_hz(w));
                assert!(c.abs() < 0.01, "wheel {} at {sr}: {c} ct", w + 1);
            }
        }
    }

    #[test]
    fn skipping_equals_ticking() {
        let mut a = Generator::new(48_000.0);
        let mut b = a.clone();
        for _ in 0..777 {
            a.tick();
        }
        b.skip(777);
        assert_eq!(a.phase, b.phase);
    }

    #[test]
    fn folding_keeps_notes_inside_the_generator() {
        assert_eq!(wheel_of(24), 0);
        assert_eq!(wheel_of(12), 0);
        assert_eq!(wheel_of(114), 90);
        assert_eq!(wheel_of(132), wheel_of(120));
        assert_eq!(wheel_of(120), 84);
    }

    #[test]
    fn drawbars_are_three_db_a_step() {
        assert_eq!(drawbar_gain(8.0), 1.0);
        assert_eq!(drawbar_gain(0.0), 0.0);
        for s in 1..8 {
            let d = drawbar_db(s as f32 + 1.0) - drawbar_db(s as f32);
            assert!((d - 3.0).abs() < 1e-4, "{s}: {d}");
        }
    }
}
