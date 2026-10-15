//! The bench's *heavy* settings (`cantus-bench`'s `kernels::heavy`, copied here until the
//! bench crate merges into the tests' reach), its test signal, and what the contract tests
//! draw at random: parameter values across each kind's whole range and block splits.
//!
//! Keep [`heavy`] in step with `crates/cantus-bench/src/kernels.rs`: these are the settings
//! the per-node segmentation and at-rest suspension of ADR 0031 are proven at.

use cantus_core::{EffectKind, EffectParamInfo, EffectSpec, InstrumentKind, NativeSpec};

pub const SAMPLE_RATE: f32 = 48_000.0;

/// Heavy settings per kind: its most expensive modes (oversampling, linear phase, true
/// peak, many bands), as a mastering engineer would push them. A copy of the bench's.
pub fn heavy(kind: EffectKind) -> EffectSpec {
    let mut s = EffectSpec::new(kind);
    let set = |s: &mut EffectSpec, kv: &[(&str, f32)]| {
        for (k, v) in kv {
            s.set(k, *v);
        }
    };
    match kind {
        EffectKind::Eq => {
            // Eight bands in use, two of them dynamic, in linear phase at its finest.
            set(&mut s, &[("mode", 2.0), ("lp_res", 3.0)]);
            for b in 0..8 {
                let f = 60.0 * cantus_dsp::math::powi(2f32, b as i32);
                let gain = if b % 2 == 0 { 4.0 } else { -3.0 };
                for (field, v) in [("used", 1.0), ("freq", f), ("gain", gain), ("q", 1.2)] {
                    s.set(&cantus_core::eq_key(b, field), v);
                }
            }
            s.set(&cantus_core::eq_key(2, "dyn"), 1.0);
            s.set(&cantus_core::eq_key(5, "dyn"), 1.0);
        }
        EffectKind::Compressor => set(&mut s, &[("os", 2.0), ("thr", -30.0), ("ratio", 8.0)]),
        EffectKind::Limiter => set(&mut s, &[("tp", 1.0), ("look", 5.0), ("dither", 3.0)]),
        EffectKind::Space => set(&mut s, &[("decay", 8.0), ("predelay", 120.0)]),
        EffectKind::Multiband => {
            // Four bands working, split in linear phase at the steepest slope.
            set(&mut s, &[("mode", 2.0), ("slope", 48.0)]);
            for (b, from) in [0.0f32, 200.0, 1_000.0, 5_000.0].into_iter().enumerate() {
                for (field, v) in [("used", 1.0), ("from", from.max(20.0)), ("thr", -40.0)] {
                    s.set(&format!("b{b}.{field}"), v);
                }
            }
        }
        EffectKind::DeEsser => set(&mut s, &[("mode", 1.0), ("proc", 2.0), ("thr", -50.0)]),
        EffectKind::Gate => set(&mut s, &[("style", 5.0)]),
        EffectKind::Saturator => set(&mut s, &[("os", 3.0)]),
        EffectKind::Stereo => {}
        EffectKind::Maximizer => set(&mut s, &[("bands", 3.0), ("tp", 1.0), ("look", 5.0)]),
        EffectKind::Delay => set(&mut s, &[("time_ms", 1500.0), ("feedback", 0.9)]),
        EffectKind::Reverb => set(&mut s, &[("size", 1.0)]),
        EffectKind::External => {}
        // Round 28: each builder sets its own heavy line.
        EffectKind::Chorus => set(&mut s, &[("mode", 7.0), ("mix", 0.5), ("voices", 4.0), ("delay", 30.0), ("depth", 8.0), ("tone", 4_000.0)]),
        EffectKind::Flanger => set(&mut s, &[("mode", 1.0), ("tz", 1.0), ("mix", 0.5), ("fb", 0.95), ("depth", 10.0), ("spread", 90.0)]),
        EffectKind::Phaser => set(&mut s, &[("stages", 12.0), ("mix", 0.5), ("fb", 0.95), ("shape", 5.0), ("spread", 90.0), ("lo", 20.0), ("hi", 16_000.0)]),
        EffectKind::Tremolo => set(&mut s, &[("mode", 2.0), ("depth", 1.0), ("shape", 5.0), ("swing", 0.5), ("spread", 90.0)]),
        EffectKind::Vibrato => set(&mut s, &[("depth", 100.0), ("rate", 0.2), ("rise", 500.0), ("spread", 90.0), ("mix", 0.5)]),
        EffectKind::RingMod => set(&mut s, &[("wave", 2.0), ("lfo", 24.0), ("rate", 20.0), ("mix", 0.5), ("spread", 90.0), ("freq", 5_000.0)]),
        EffectKind::FreqShift => {}
        EffectKind::VolumeShaper => {}
        EffectKind::Clipper => {}
        EffectKind::Transient => {}
        EffectKind::TransientSplit => {}
        EffectKind::Analyzer => {}
        EffectKind::AutoTune => {}
    }
    s
}

/// The bench's heavy instrument load: this many voices held at once
/// (`cantus-bench`'s `instrument_metrics`, `inst.*.v32`).
pub const HEAVY_VOICES: usize = 32;

/// The bench's test signal: noise and two tones, swelling and falling at 2 Hz, −12 dBFS
/// peak. A copy of `kernels::signal`.
pub fn signal(frames: usize) -> (Vec<f32>, Vec<f32>) {
    let mut seed = 0x2545_F491u32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let (mut l, mut r) = (Vec::with_capacity(frames), Vec::with_capacity(frames));
    for i in 0..frames {
        let t = i as f32 / SAMPLE_RATE;
        let env = 0.5 + 0.5 * cantus_dsp::math::sin(std::f32::consts::TAU * 2.0 * t);
        let tone = cantus_dsp::math::sin(std::f32::consts::TAU * 220.0 * t) * 0.5
            + cantus_dsp::math::sin(std::f32::consts::TAU * 6_300.0 * t) * 0.2;
        l.push((noise() * 0.3 + tone) * env * 0.25);
        r.push((noise() * 0.3 + tone * 0.9) * env * 0.25);
    }
    (l, r)
}

/// A small deterministic generator (xorshift64*): the tests' splits, values and seeds.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next_u64() % (hi - lo + 1) as u64) as usize
    }
    /// Uniform in `0..1`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
    /// A block length: mostly short, sometimes up to 4096, sometimes a single sample — the
    /// splits per-node segmentation can produce.
    pub fn block(&mut self) -> usize {
        match self.range(0, 9) {
            0 => 1,
            1..=4 => self.range(1, 64),
            5..=7 => self.range(1, 512),
            _ => self.range(1, 4096),
        }
    }
}

/// A value drawn across a parameter's whole range (log-spaced where the knob is).
fn draw(info: &EffectParamInfo, rng: &mut Rng) -> f32 {
    let u = rng.unit();
    let v = if info.logarithmic && info.min > 0.0 && info.max > info.min {
        info.min * cantus_dsp::math::powf(info.max / info.min, u)
    } else {
        info.min + (info.max - info.min) * u
    };
    if info.stepped {
        v.round()
    } else {
        v
    }
}

/// An effect with every parameter drawn at random over its range, with a probability
/// `p` per parameter (the others keep their defaults).
pub fn random_effect(kind: EffectKind, p: f32, rng: &mut Rng) -> EffectSpec {
    let mut s = EffectSpec::new(kind);
    for info in kind.params() {
        if rng.chance(p) {
            s.set(info.key, draw(info, rng));
        }
    }
    s
}

/// An instrument with every parameter drawn at random over its range, with a probability
/// `p` per parameter.
pub fn random_instrument(kind: InstrumentKind, p: f32, rng: &mut Rng) -> NativeSpec {
    let mut s = NativeSpec::new(kind);
    for info in kind.params() {
        if rng.chance(p) {
            s.set(info.key, draw(info, rng));
        }
    }
    s
}

/// Denormals flushed to zero (FTZ and DAZ on x86-64, FZ on AArch64) while it lives, as the
/// engine's `process` will run (ADR 0031 §5.2): a copy of `cantus-plugins`' guard. Some
/// state decays by multiplication alone and stops in the subnormals without it — there a
/// unit can be exact and still never come to rest.
pub struct Flush {
    #[allow(dead_code)]
    saved: u64,
}

impl Flush {
    #[cfg(target_arch = "x86_64")]
    pub fn new() -> Flush {
        let mut csr: u32 = 0;
        // SAFETY: stmxcsr/ldmxcsr only read and write the SSE control register; the value
        // written is the current one with two mode bits set.
        unsafe {
            std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
            let flushed = csr | 0x8040;
            std::arch::asm!("ldmxcsr [{}]", in(reg) &flushed, options(nostack, preserves_flags));
        }
        Flush { saved: csr as u64 }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn new() -> Flush {
        let fpcr: u64;
        // SAFETY: reads and writes the floating-point control register only.
        unsafe {
            std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags));
            std::arch::asm!("msr fpcr, {}", in(reg) fpcr | (1 << 24), options(nomem, nostack, preserves_flags));
        }
        Flush { saved: fpcr }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn new() -> Flush {
        Flush { saved: 0 }
    }
}

impl Default for Flush {
    fn default() -> Self {
        Flush::new()
    }
}

impl Drop for Flush {
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        {
            let csr = self.saved as u32;
            // SAFETY: restores the value read in `new`.
            unsafe {
                std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, preserves_flags));
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            // SAFETY: restores the value read in `new`.
            unsafe {
                std::arch::asm!("msr fpcr, {}", in(reg) self.saved, options(nomem, nostack, preserves_flags));
            }
        }
    }
}
