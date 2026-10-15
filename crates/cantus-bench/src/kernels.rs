//! DSP kernels: each effect at its defaults and at heavy settings, alone; each
//! instrument with a few and with many voices.
//!
//! An effect is measured as the engine runs it — `EffectUnit::process_keyed` on 128-frame
//! stereo blocks at 48 kHz — on a signal that keeps every detector busy (noise and tones,
//! swelling and falling twice a second). The result is nanoseconds per stereo frame, and
//! from it the number of instances one core could run in a 128-sample buffer: the budget
//! divided by one instance's block time. That count is the ceiling, not the promise; the
//! DAWbench test (`dawbench.rs`) measures what actually fits.
//!
//! An instrument is measured through the processor (one track, `sessions::instrument`),
//! because its voices are fed by the sequencer; the processor's own share is a few
//! hundred nanoseconds a block, against the instrument's tens of microseconds.

use crate::report::{Better, Metric};
use crate::rig::{Rig, SAMPLE_RATE};
use crate::stats::{median, Summary};
use crate::Plan;
use cantus_core::{EffectKind, EffectSlot, EffectSpec, InstrumentKind, InstrumentSpec, Ticks};
use cantus_engine::EffectUnit;
use std::time::Instant;

pub const BLOCK: usize = 128;

/// Heavy settings per kind: its most expensive modes (oversampling, linear phase, true
/// peak, many bands), as a mastering engineer would push them.
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
                let f = 60.0 * 2f32.powi(b as i32);
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
        EffectKind::Chorus => {}
        EffectKind::Flanger => {}
        EffectKind::Phaser => {}
        EffectKind::Tremolo => {}
        EffectKind::Vibrato => {}
        EffectKind::RingMod => {}
        EffectKind::FreqShift => {}
        EffectKind::VolumeShaper => {}
        EffectKind::Clipper => {}
        EffectKind::Transient => {}
        EffectKind::TransientSplit => {}
        EffectKind::Analyzer => {}
        // Tonus: the hard effect an octave up with the throat down, humanised, a created
        // vibrato — every grain path working at once.
        EffectKind::AutoTune => set(
            &mut s,
            &[
                ("speed", 0.0),
                ("trans", 12.0),
                ("throat", -6.0),
                ("vdepth", 40.0),
                ("human", 1.0),
            ],
        ),
    }
    s
}

/// The test signal: noise and two tones, swelling and falling at 2 Hz, −12 dBFS peak.
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
        let t = i as f32 / SAMPLE_RATE as f32;
        // `cantus_dsp::math::sin`, not std's: the platform libm (MSVC, glibc) gives
        // different bits, and this signal feeds the Windows = Linux null hashes.
        let env = 0.5 + 0.5 * cantus_dsp::math::sin(std::f32::consts::TAU * 2.0 * t);
        let tone = cantus_dsp::math::sin(std::f32::consts::TAU * 220.0 * t) * 0.5
            + cantus_dsp::math::sin(std::f32::consts::TAU * 6_300.0 * t) * 0.2;
        l.push((noise() * 0.3 + tone) * env * 0.25);
        r.push((noise() * 0.3 + tone * 0.9) * env * 0.25);
    }
    (l, r)
}

/// One effect's block time, µs, as runs of `seconds` of signal: each run's median block.
fn effect_runs(spec: &EffectSpec, seconds: f64, runs: usize) -> Vec<f64> {
    let slot = EffectSlot {
        id: cantus_core::EffectId(1),
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: spec.clone(),
    };
    let blocks = (seconds * SAMPLE_RATE / BLOCK as f64).ceil() as usize;
    let (sl, sr) = signal(SAMPLE_RATE as usize);
    let mut unit = EffectUnit::new(&slot, SAMPLE_RATE as f32);
    let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
    let mut at = 0usize;
    let mut step = |unit: &mut EffectUnit| {
        if at + BLOCK > sl.len() {
            at = 0;
        }
        l.copy_from_slice(&sl[at..at + BLOCK]);
        r.copy_from_slice(&sr[at..at + BLOCK]);
        at += BLOCK;
        let t = Instant::now();
        unit.process_keyed(&mut l, &mut r, None);
        let us = t.elapsed().as_secs_f64() * 1e6;
        std::hint::black_box(&l);
        us
    };
    // A second of warm-up: tails fill, detectors settle, caches warm.
    for _ in 0..(SAMPLE_RATE as usize / BLOCK) {
        step(&mut unit);
    }
    (0..runs)
        .map(|_| {
            let v: Vec<f64> = (0..blocks).map(|_| step(&mut unit)).collect();
            median(&v)
        })
        .collect()
}

/// Every effect kind at its defaults and heavy: ns per stereo frame, and instances that
/// fit one core at 128 samples.
pub fn effects(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    let budget = BLOCK as f64 / SAMPLE_RATE * 1e6;
    for kind in EffectKind::ALL {
        for (label, spec) in [("default", EffectSpec::new(kind)), ("heavy", heavy(kind))] {
            let runs = effect_runs(&spec, plan.kernel_seconds, plan.runs);
            let ns: Vec<f64> = runs.iter().map(|us| us * 1e3 / BLOCK as f64).collect();
            let fit: Vec<f64> = runs.iter().map(|us| (budget / us).floor()).collect();
            let name = kind_id(kind);
            out.push(Metric::new(
                format!("fx.{name}.{label}"),
                "ns/frame",
                Better::Lower,
                Summary::of(&ns),
            ));
            out.push(
                Metric::new(
                    format!("fx.{name}.{label}.fit128"),
                    "instances",
                    Better::Higher,
                    Summary::of(&fit),
                )
                .ungated(),
            );
        }
    }
    out
}

/// The kind's name in a metric id.
pub fn kind_id(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Eq => "aequalis",
        EffectKind::Compressor => "comprimo",
        EffectKind::Limiter => "terminus",
        EffectKind::Space => "spatium",
        EffectKind::Multiband => "multiplex",
        EffectKind::DeEsser => "sibilus",
        EffectKind::Gate => "porta",
        EffectKind::Saturator => "fervor",
        EffectKind::Stereo => "latitudo",
        EffectKind::Maximizer => "culmen",
        EffectKind::Delay => "delay",
        EffectKind::Reverb => "reverb",
        EffectKind::External => "external",
        EffectKind::Chorus => "concentus",
        EffectKind::Flanger => "flexus",
        EffectKind::Phaser => "phasis",
        EffectKind::Tremolo => "tremor",
        EffectKind::Vibrato => "vibrans",
        EffectKind::RingMod => "anulus",
        EffectKind::FreqShift => "translatio",
        EffectKind::VolumeShaper => "fluctus",
        EffectKind::Clipper => "tonsor",
        EffectKind::Transient => "impetus",
        EffectKind::TransientSplit => "discrimen",
        EffectKind::Analyzer => "speculum",
        EffectKind::AutoTune => "tonus",
    }
}

/// Every instrument, the two built in and the seven native ones, with `voices` held.
pub fn instruments() -> Vec<(&'static str, InstrumentSpec)> {
    let mut v: Vec<(&'static str, InstrumentSpec)> = vec![
        ("firmus", InstrumentSpec::default()),
        (
            "pulsus",
            InstrumentSpec::Pulsus(cantus_core::PulsusParams::default()),
        ),
    ];
    for kind in InstrumentKind::ALL {
        let name = match kind {
            InstrumentKind::Wavetable => "unda",
            InstrumentKind::Analog => "calor",
            InstrumentKind::Fm => "modulus",
            InstrumentKind::Sampler => "exemplar",
            InstrumentKind::Bass => "gravis",
            InstrumentKind::ElectricPiano => "lamina",
            InstrumentKind::Organ => "organum",
            InstrumentKind::Chorda => "chorda",
        };
        v.push((
            name,
            InstrumentSpec::Native(cantus_core::NativeSpec::new(kind)),
        ));
    }
    v
}

/// Each instrument with 1, 8 and 32 voices: ns per stereo frame for the whole voice set,
/// and the voices actually sounding (an instrument caps its own polyphony).
pub fn instrument_metrics(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    let blocks = (plan.kernel_seconds * SAMPLE_RATE / BLOCK as f64).ceil() as usize;
    for (name, spec) in instruments() {
        for voices in [1usize, 8, 32] {
            let p = crate::sessions::instrument(spec.clone(), voices);
            let mut sounding = 0;
            let ns: Vec<f64> = (0..plan.runs)
                .map(|_| {
                    let mut rig = Rig::new(&p, Ticks::ZERO, BLOCK);
                    let run = rig.run(SAMPLE_RATE as usize / BLOCK / 4, blocks);
                    sounding = run.voices;
                    median(&run.block_us) * 1e3 / BLOCK as f64
                })
                .collect();
            out.push(
                Metric::new(
                    format!("inst.{name}.v{voices}"),
                    "ns/frame",
                    Better::Lower,
                    Summary::of(&ns),
                )
                .note(format!("{sounding} voices sounding")),
            );
        }
    }
    out
}
