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
        EffectKind::Chorus => set(
            &mut s,
            &[
                ("mode", 7.0),
                ("mix", 0.5),
                ("voices", 4.0),
                ("delay", 30.0),
                ("depth", 8.0),
                ("tone", 4_000.0),
            ],
        ),
        EffectKind::Flanger => set(
            &mut s,
            &[
                ("mode", 1.0),
                ("tz", 1.0),
                ("mix", 0.5),
                ("fb", 0.95),
                ("depth", 10.0),
                ("spread", 90.0),
            ],
        ),
        EffectKind::Phaser => set(
            &mut s,
            &[
                ("stages", 12.0),
                ("mix", 0.5),
                ("fb", 0.95),
                ("shape", 5.0),
                ("spread", 90.0),
                ("lo", 20.0),
                ("hi", 16_000.0),
            ],
        ),
        EffectKind::Tremolo => set(
            &mut s,
            &[
                ("mode", 2.0),
                ("depth", 1.0),
                ("shape", 5.0),
                ("swing", 0.5),
                ("spread", 90.0),
            ],
        ),
        EffectKind::Vibrato => set(
            &mut s,
            &[
                ("depth", 100.0),
                ("rate", 0.2),
                ("rise", 500.0),
                ("spread", 90.0),
                ("mix", 0.5),
            ],
        ),
        EffectKind::RingMod => set(
            &mut s,
            &[
                ("wave", 2.0),
                ("lfo", 24.0),
                ("rate", 20.0),
                ("mix", 0.5),
                ("spread", 90.0),
                ("freq", 5_000.0),
            ],
        ),
        EffectKind::FreqShift => set(
            &mut s,
            &[
                ("shift", -2_000.0),
                ("side", 2.0),
                ("fb", 0.95),
                ("lfo", 1_000.0),
                ("rate", 10.0),
                ("mix", 0.5),
            ],
        ),
        EffectKind::VolumeShaper => set(
            &mut s,
            &[
                ("depth", 1.0),
                ("div", 14.0),
                ("smooth", 0.0),
                ("p2.on", 1.0),
                ("p2.x", 0.7),
                ("p2.s", 4.0),
                ("p2.t", 1.0),
                ("p3.on", 1.0),
                ("p3.x", 0.9),
                ("p3.s", 1.0),
            ],
        ),
        // Tonsor: 16x with the true-peak guard, driven hard into the analog shoulder.
        EffectKind::Clipper => set(
            &mut s,
            &[
                ("drive", 12.0),
                ("ceil", -1.0),
                ("knee", 3.0),
                ("curve", 2.0),
                ("os", 4.0),
                ("tp", 1.0),
                ("match", 1.0),
            ],
        ),
        // Impetus: both amounts at work on the highs, the shortest onset, a long tail.
        EffectKind::Transient => set(
            &mut s,
            &[
                ("attack", 1.0),
                ("sustain", -0.7),
                ("speed", 2.0),
                ("length", 900.0),
                ("band", 1.0),
                ("xover", 600.0),
            ],
        ),
        // Discrimen: the look-ahead, both parts compressed and tilted.
        EffectKind::TransientSplit => set(
            &mut s,
            &[
                ("mode", 1.0),
                ("sens", 0.8),
                ("speed", 3.0),
                ("t.gain", 6.0),
                ("t.comp", 0.8),
                ("t.tilt", 4.0),
                ("b.gain", -4.0),
                ("b.comp", 0.8),
                ("b.tilt", -4.0),
            ],
        ),
        // Speculum: a pass-through, whatever is shown (the largest FFT, both channels).
        EffectKind::Analyzer => set(
            &mut s,
            &[("view", 1.0), ("fft", 4.0), ("chan", 4.0), ("sync", 1.0)],
        ),
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
        // r39:conv — Aula: the cathedral stretched half again (about 11 s, true stereo, four
        // levels of partitions), damped, cut, wide, half wet (the engine tests' heavy).
        EffectKind::Aula => set(
            &mut s,
            &[
                ("ir", 9.0),
                ("stretch", 1.5),
                ("damp", 0.4),
                ("low", 80.0),
                ("predelay", 30.0),
                ("width", 1.3),
                ("mix", 0.5),
            ],
        ),
        // fx:cribrum — both filters (a driven ladder at 24 dB/oct into three formants), 4×
        // oversampling, every source on every target.
        EffectKind::AutoFilter => set(
            &mut s,
            &[
                ("mix", 1.0),
                ("os", 2.0),
                ("a.model", 1.0),
                ("a.slope", 1.0),
                ("a.cut", 800.0),
                ("a.res", 0.7),
                ("a.drive", 12.0),
                ("b.on", 1.0),
                ("b.mode", 5.0),
                ("b.res", 0.5),
                ("spread", 0.5),
                ("lfo.shape", 6.0),
                ("lfo.stereo", 90.0),
                ("lfo.cut", 2.0),
                ("lfo.res", 0.2),
                ("lfo.vowel", 1.0),
                ("env.cut", 1.5),
                ("cur.cut", -1.0),
                ("cur.vowel", 1.0),
                ("cur.smooth", 10.0),
                ("out", -3.0),
            ],
        ),
        // fx:fragor — three pedals (the muff, the fuzz on a tired battery, the silicon
        // distortion), 8x, stereo, the gate on, half wet.
        EffectKind::Pedal => set(
            &mut s,
            &[
                ("s1", 5.0),
                ("s2", 4.0),
                ("s3", 3.0),
                ("mf.sustain", 0.8),
                ("ge.fuzz", 0.9),
                ("ge.bias", 0.5),
                ("si.dist", 0.8),
                ("gate", -70.0),
                ("os", 2.0),
                ("mix", 0.5),
                ("out", -6.0),
            ],
        ), // fx:fragor
        // fx:lapsus — the roll build (fifteen jumps a bar, each a join) under a trance gate,
        // mixed with the dry, a long crossfade and glide, and lookahead (long kernels).
        EffectKind::TimeShaper => set(
            &mut s,
            &[
                ("tslot", 16.0),
                ("vslot", 5.0),
                ("mix", 0.8),
                ("xfade", 6.0),
                ("smooth", 4.0),
                ("ahead", 5.0),
                ("out", 6.0),
            ],
        ),
        // fx:vetus — every module on: dithered mu-law bits, the hold behind both its filters,
        // a synced wow, dropouts and wear, every noise (following the music, so it rests),
        // the gramophone's band, Character half way.
        EffectKind::LoFi => set(
            &mut s,
            &[
                ("char", 0.5),
                ("bits.on", 1.0),
                ("bits.depth", 6.0),
                ("bits.dither", 1.0),
                ("bits.mode", 2.0),
                ("rate.on", 1.0),
                ("rate.hz", 9_000.0),
                ("rate.filter", 2.0),
                ("wob.on", 1.0),
                ("wow", 30.0),
                ("wow.sync", 1.0),
                ("flut", 8.0),
                ("age.on", 1.0),
                ("age.drop", 0.7),
                ("age.wear", 0.6),
                ("noise.on", 1.0),
                ("crackle", 0.8),
                ("hiss", 0.5),
                ("hum", 0.3),
                ("noise.follow", 1.0),
                ("tone.on", 1.0),
                ("tone.medium", 3.0),
                ("mix", 0.8),
                ("out", -3.0),
            ],
        ), // fx:vetus
        // fx:libra — every stage at work: DC removal, the sides swapped, a channel inverted,
        // both delayed, wider, balanced, gained.
        EffectKind::Utility => set(
            &mut s,
            &[
                ("dc", 1.0),
                ("chan", 1.0),
                ("pol.l", 1.0),
                ("delay.l", 12.5),
                ("delay.r", 40.0),
                ("width", 1.6),
                ("pan", -0.4),
                ("out", 6.0),
            ],
        ), // fx:libra
        // fx3:contract — wave 3's stubs: the placeholder's gain; each builder replaces its
        // own line with its unit's heaviest settings (tabula: a full board).
        // fx3:tabula — a full board: drive and a linear-phase EQ before a split (a chorus on
        // A; a phaser and an oversampled saturator on B, so the lanes' latencies differ),
        // then a linear-phase multiband and a room; the board's own levels moved too.
        EffectKind::Board => {
            use cantus_core::fx::tabula::{Board, Path, Pedal, PedalId};
            let pedal = |id: u16, kind: EffectKind, path: Path| {
                let mut p = Pedal::new(PedalId(id), heavy(kind));
                p.path = path;
                p
            };
            let mut room = Pedal::new(PedalId(9), EffectSpec::new(EffectKind::Space));
            room.path = Path::Post;
            room.spec.set("mix", 0.3);
            s.board = Some(Box::new(Board {
                pedals: vec![
                    pedal(0, EffectKind::Pedal, Path::Pre),
                    pedal(1, EffectKind::Eq, Path::Pre),
                    pedal(2, EffectKind::Chorus, Path::A),
                    pedal(3, EffectKind::Phaser, Path::B),
                    pedal(5, EffectKind::Saturator, Path::B),
                    pedal(7, EffectKind::Multiband, Path::Post),
                    room,
                ],
                macros: Vec::new(),
            }));
            set(
                &mut s,
                &[("in", -3.0), ("out", 2.0), ("mix", 0.85), ("ab", 0.4)],
            );
        }
        // fx3:saltus — the poly engine (its dearest) with both voices, spread, the dry
        // under them, formants moved and the throat shifted.
        EffectKind::PitchShift => set(
            &mut s,
            &[
                ("engine", 1.0),
                ("st1", 7.0),
                ("pan1", -0.5),
                ("st2", -12.0),
                ("ct2", 15.0),
                ("lvl2", 0.8),
                ("pan2", 0.6),
                ("dry", 0.5),
                ("formant", 0.0),
                ("throat", -3.0),
                ("out", 6.0),
            ],
        ), // fx3:saltus
        // fx3:rota — fast, driven, all wet, the drum ramping (a note's work).
        EffectKind::Rotary => set(
            &mut s,
            &[("mix", 1.0), ("speed", 2.0), ("drive", 0.6), ("out", 6.0)],
        ), // fx3:rota
        // fx3:arca — the dearest: the high gain at 8× into a cabinet.
        EffectKind::Amp => set(
            &mut s,
            &[
                ("model", 4.0),
                ("gain", 0.8),
                ("cab", 4.0),
                ("os", 2.0),
                ("out", 6.0),
            ],
        ), // fx3:arca
        // fxa:loquax — 64 bands of order 8, the synthesiser in seven copies and the
        // sidechain both carrying, formant and band shifted, spread, the consonants' help.
        EffectKind::Vocoder => set(
            &mut s,
            &[
                ("mix", 1.0),
                ("src", 2.0),
                ("bands", 64.0),
                ("order", 2.0),
                ("formant", 3.0),
                ("bshift", 0.5),
                ("spread", 0.7),
                ("uni", 7.0),
                ("robot", 1.0),
                ("cpass", 0.2),
                ("tilt", 2.0),
                ("out", 6.0),
            ],
        ), // fxa:loquax
        // fxa:fractus — every lane painted, the stutter build, swing, the cloud beside the
        // chain at a high density with pitched grains, a throw ringing.
        EffectKind::Glitch => {
            use cantus_core::fx::fractus::{Block, Pattern};
            let mut p = Pattern::default();
            for lane in 0..10u8 {
                let at = (u16::from(lane) * 3) % 14;
                p.replace(lane, &[Block::new(at, 2 + u16::from(lane % 3))]);
            }
            s.fractus = Some(Box::new(p));
            set(
                &mut s,
                &[
                    ("swing", 0.4),
                    ("rramp", 2.0),
                    ("rpitch", 1.0),
                    ("clevel", 0.6),
                    ("cdens", 120.0),
                    ("cpitch", 7.0),
                    ("crand", 12.0),
                    ("crev", 0.3),
                    ("wfb", 0.6),
                    ("out", 3.0),
                ],
            );
        } // fxa:fractus
        // fxa:caelum — sixteen lines, the vast size, both shimmer voices, bloom, ducking
        // keyed, modulated deep (finite: an infinite or frozen tank never rests).
        EffectKind::Shimmer => set(
            &mut s,
            &[
                ("mix", 0.6),
                ("size", 0.95),
                ("decay", 12.0),
                ("shim", 0.8),
                ("pblvl", 0.6),
                ("bloom", 1_500.0),
                ("duck", 12.0),
                ("mdepth", 0.8),
                ("width", 1.3),
                ("out", 6.0),
            ],
        ), // fxa:caelum
        // fxc:purus — all three modules: the gentle de-hum tracking sixteen harmonics, the
        // de-click at its cap with the de-crackle, the de-reverb estimating its decay; every
        // module's level moved (the latency is all three's, 87 ms at 48 kHz).
        EffectKind::Repair => set(
            &mut s,
            &[
                ("hon", 1.0),
                ("hmode", 1.0),
                ("hharm", 16.0),
                ("hgentle", 1.0),
                ("hout", -1.0),
                ("con", 1.0),
                ("csens", 0.7),
                ("crackle", 0.5),
                ("von", 1.0),
                ("vamount", 18.0),
                ("vdecay", 1.0),
                ("vmix", 0.8),
                ("out", 2.0),
            ],
        ), // fxc:purus
        // fxc:comparatio — the reference asked for, matched short-term with an offset (no
        // reference loaded here: the mix, to the bit).
        EffectKind::Reference => set(
            &mut s,
            &[
                ("listen", 1.0),
                ("match", 1.0),
                ("offset", -3.0),
                ("sync", 1.0),
            ],
        ), // fxc:comparatio
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
        EffectKind::Aula => "aula",            // r39:conv
        EffectKind::AutoFilter => "cribrum",   // fx:contract
        EffectKind::Pedal => "fragor",         // fx:contract
        EffectKind::TimeShaper => "lapsus",    // fx:contract
        EffectKind::LoFi => "vetus",           // fx:contract
        EffectKind::Utility => "libra",        // fx:contract
        EffectKind::Board => "tabula",         // fx3:contract
        EffectKind::PitchShift => "saltus",    // fx3:contract
        EffectKind::Rotary => "rota",          // fx3:contract
        EffectKind::Amp => "arca",             // fx3:contract
        EffectKind::Vocoder => "loquax",       // fxa:contract
        EffectKind::Glitch => "fractus",       // fxa:contract
        EffectKind::Shimmer => "caelum",       // fxa:contract
        EffectKind::Repair => "purus",         // fxc:contract
        EffectKind::Reference => "comparatio", // fxc:contract
    }
}

/// Every instrument, the two built in and the native ones, with `voices` held.
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
            // The dancer makes no sound: what its unit costs the audio thread (ADR 0087).
            InstrumentKind::Dancer => "saltator",
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
