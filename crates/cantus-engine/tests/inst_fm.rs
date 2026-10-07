//! Modulus through the engine: the family's rules with a rich patch (block-size
//! independence at tolerance 0, determinism, the default changes nothing, exact silence,
//! headroom), pitch accuracy across the keyboard, aliasing measured at high indices, the
//! guard, feedback that sounds the same at every quality, MPE, and a CPU budget.
//!
//! Run with `--nocapture` to see the measured numbers `docs/instruments/modulus.md` quotes.

use cantus_dsp::math;

mod support;
use cantus_core::inst::fm::{dx_template, OP_KEYS};
use cantus_core::*;
use cantus_dsp::spectrum::fft;
use cantus_engine::inst::{fm as efm, Controller, InstrumentUnit, NoteExpression, NoteOn};
use cantus_engine::*;
use std::sync::Arc;

const FS: f32 = 48_000.0;

fn spec_with(values: &[(&str, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Fm);
    for (k, v) in values {
        assert!(s.set(k, *v).is_some(), "no key {k}");
    }
    s
}

/// A rich patch: DX algorithm 5 over six operators with feedback, a seventh operator in a
/// back edge, an LFO, unison, glide and two modulation routes.
fn rich() -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Fm);
    for (k, v) in dx_template(5, 2.5, 2.0) {
        s.set(k, v);
    }
    for (k, v) in [
        ("op.b.ratio", 3.5),
        ("op.d.ratio", 2.0),
        ("op.f.ratio", 0.5),
        ("op.g.ratio", 7.0),
        ("m.g.a", 1.0),
        ("m.a.g", 0.8),
        ("env.b.mode", 1.0),
        ("env.b.dr2", 40.0),
        ("env.b.dl2", 60.0),
        ("lfo1.pitch", 25.0),
        ("unison", 2.0),
        ("glide", 40.0),
        ("drift", 3.0),
        ("quality", 1.0),
    ] {
        s.set(k, v);
    }
    s.set_text("mod1.src", Some("lfo2".into())).unwrap();
    s.set_text("mod1.dst", Some("m.b.a".into())).unwrap();
    s.set("mod1.amt", 0.1);
    s.set_text("mod2.src", Some("vel".into())).unwrap();
    s.set_text("mod2.dst", Some("macro.bright".into())).unwrap();
    s.set("mod2.amt", 0.3);
    s
}

fn project(spec: NativeSpec, notes: &[(u8, i64, i64, u8)]) -> Project {
    let mut p = Project::new("modulus");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Modulus");
    t.instrument = InstrumentSpec::Native(spec);
    let end = notes.iter().map(|n| n.1 + n.2).max().unwrap_or(1);
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(end));
    for &(key, start, len, vel) in notes {
        clip.notes
            .push(p.make_note(Ticks::beats(start), Ticks::beats(len), key, vel));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn render_at(p: &Project, block: usize, rate: u32, tail: f64) -> Vec<f32> {
    let opts = RenderOptions {
        sample_rate: rate,
        tail_seconds: tail,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .unwrap()
        .samples
}

const RIFF: &[(u8, i64, i64, u8)] = &[
    (48, 0, 2, 100),
    (55, 1, 2, 80),
    (60, 2, 1, 127),
    (64, 2, 2, 60),
    (96, 3, 1, 110),
];

#[test]
fn a_rich_patch_is_identical_at_every_block_size_and_every_run() {
    let p = project(rich(), RIFF);
    let reference = render_at(&p, 512, 48_000, 0.5);
    assert!(reference.iter().any(|x| *x != 0.0) && reference.iter().all(|x| x.is_finite()));
    for block in [1usize, 7, 64, 1000] {
        let other = render_at(&p, block, 48_000, 0.5);
        assert!(reference == other, "block {block} differs");
    }
    assert!(
        reference == render_at(&p, 512, 48_000, 0.5),
        "a second run differs"
    );
}

#[test]
fn every_default_written_out_changes_no_sample() {
    let fresh = project(NativeSpec::new(InstrumentKind::Fm), RIFF);
    let mut spelled = NativeSpec::new(InstrumentKind::Fm);
    for info in InstrumentKind::Fm.params() {
        spelled.set(info.key, info.default);
    }
    assert!(spelled.params.len() > 500);
    let spelled = project(spelled, RIFF);
    assert!(render_at(&fresh, 256, 48_000, 0.3) == render_at(&spelled, 256, 48_000, 0.3));
}

#[test]
fn a_rich_patch_goes_exactly_silent_and_keeps_headroom() {
    let chord: Vec<_> = [36u8, 48, 55, 60, 64, 67, 72, 79]
        .iter()
        .map(|k| (*k, 0, 2, 127))
        .collect();
    for spec in [NativeSpec::new(InstrumentKind::Fm), rich()] {
        let out = render_at(&project(spec, &chord), 256, 48_000, 4.0);
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        println!(
            "eight-note chord at velocity 127: peak {:.1} dBFS",
            20.0 * math::log10(peak)
        );
        assert!(peak < 1.0, "peak {peak}");
        let last = &out[out.len() - 48_000 * 2..];
        assert!(last.iter().all(|x| *x == 0.0), "not silent at the end");
    }
}

/// The unit alone, for measurements: one note held, `secs` rendered (left channel).
fn unit_note(spec: &NativeSpec, key: u8, secs: f32) -> Vec<f32> {
    let mut u = efm::Unit::new(FS);
    u.configure(spec, &[]);
    u.note_on(NoteOn {
        key,
        velocity: 1.0,
        id: 1,
        channel: 0,
    });
    let n = (secs * FS) as usize;
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    for (lc, rc) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(lc, rc);
    }
    l
}

/// Frequency from upward zero crossings, interpolated.
fn frequency(x: &[f32], fs: f32) -> f32 {
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0;
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = (i - 1) as f64 + (-x[i - 1] / (x[i] - x[i - 1])) as f64;
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    (count as f64 / ((last - first.unwrap()) / fs as f64)) as f32
}

#[test]
fn pitch_is_accurate_across_the_keyboard() {
    let spec = spec_with(&[("env.a.l2", 1.0)]);
    let mut worst = 0f32;
    for key in (21u8..=108).step_by(3) {
        let x = unit_note(&spec, key, 1.2);
        let f = frequency(&x[4_800..], FS);
        let want = 440.0 * math::powf(2f32, (key as f32 - 69.0) / 12.0);
        let cents = 1200.0 * math::log2(f / want);
        worst = worst.max(cents.abs());
    }
    println!("pitch error across A0…C8: at most {worst:.4} cents");
    assert!(worst < 0.1, "{worst} cents");
}

/// Energy in bins that are not harmonics of `k` against the total, dB: over the whole band,
/// and below 20 kHz (what folds into the decimator's transition band lands above 21.6 kHz).
fn alias_db(x: &[f32], k: usize) -> (f32, f32) {
    let mut re = x.to_vec();
    let mut im = vec![0f32; x.len()];
    fft(&mut re, &mut im);
    let n = x.len();
    let audible = (20_000.0 / FS * n as f32) as usize;
    let (mut total, mut alias, mut heard) = (0f64, 0f64, 0f64);
    for b in 1..n / 2 {
        let p = (re[b] * re[b] + im[b] * im[b]) as f64;
        total += p;
        if b % k != 0 {
            alias += p;
            if b < audible {
                heard += p;
            }
        }
    }
    let db = |x: f64| (10.0 * math::log10((x / total).max(1e-30))) as f32;
    (db(alias), db(heard))
}

/// A 1:1 pair on exactly periodic fixed frequencies (714 cycles in 16 384 samples, about
/// C7), `index` radians, at a quality and guard.
fn periodic_pair(index: f32, quality: f32, guard: f32) -> (f32, f32) {
    let f0 = FS * 714.0 / 16_384.0;
    let spec = spec_with(&[
        ("op.a.freq_mode", 1.0),
        ("op.a.fixed", f0),
        ("op.b.freq_mode", 1.0),
        ("op.b.fixed", f0),
        ("env.a.l2", 1.0),
        ("env.b.l2", 1.0),
        ("m.b.a", index),
        ("quality", quality),
        ("guard", guard),
    ]);
    let x = unit_note(&spec, 96, 1.0);
    alias_db(&x[x.len() - 16_384..], 714)
}

#[test]
fn aliasing_at_high_index_is_measured_and_held_down() {
    println!("alias at C7, 1:1, dB (whole band / below 20 kHz)");
    println!("index | Eco 1×        | Auto          | Eco+Tame      | 8×");
    let mut worst_auto = f32::MIN;
    let mut worst_tame = f32::MIN;
    let cell = |(a, b): (f32, f32)| format!("{a:>6.1}/{b:>6.1}");
    for index in [1.0f32, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0] {
        let eco = periodic_pair(index, 0.0, 0.0);
        let auto = periodic_pair(index, 1.0, 1.0);
        let tame = periodic_pair(index, 0.0, 2.0);
        let x8 = periodic_pair(index, 4.0, 0.0);
        println!(
            "{index:>5} | {} | {} | {} | {}",
            cell(eco),
            cell(auto),
            cell(tame),
            cell(x8)
        );
        if index <= 8.0 {
            worst_auto = worst_auto.max(auto.0);
        }
        worst_tame = worst_tame.max(tame.1);
    }
    assert!(
        worst_auto <= -60.0,
        "Auto up to index 8 at C7: {worst_auto} dB"
    );
    assert!(
        worst_tame <= -60.0,
        "Tame at 1×, audible band: {worst_tame} dB"
    );
    assert!(
        periodic_pair(8.0, 0.0, 0.0).0 > -40.0,
        "Eco really does alias there"
    );
}

#[test]
fn feedback_sounds_the_same_at_every_quality() {
    let level = |q: f32| {
        let spec = spec_with(&[("fb.a", 2.0), ("env.a.l2", 1.0), ("quality", q)]);
        let x = unit_note(&spec, 57, 0.6);
        // The first harmonics' levels.
        let seg = &x[x.len() - 8_192..];
        let mut re: Vec<f32> = seg
            .iter()
            .enumerate()
            .map(|(i, v)| v * (0.5 - 0.5 * math::cos(std::f32::consts::TAU * i as f32 / 8_192.0)))
            .collect();
        let mut im = vec![0f32; re.len()];
        fft(&mut re, &mut im);
        (1..=8)
            .map(|h| {
                let b = (220.0 * h as f32 * 8_192.0 / FS).round() as usize;
                (b - 2..=b + 2)
                    .map(|k| re[k] * re[k] + im[k] * im[k])
                    .fold(0f32, f32::max)
            })
            .map(|p| 10.0 * math::log10(p))
            .collect::<Vec<_>>()
    };
    let one = level(0.0);
    for q in [2.0, 3.0, 4.0] {
        let other = level(q);
        for (h, (a, b)) in one.iter().zip(&other).enumerate() {
            assert!(
                (a - b).abs() < 0.5,
                "harmonic {}: {a} dB at 1×, {b} dB at quality {q}",
                h + 1
            );
        }
    }
}

#[test]
fn the_timbre_is_the_same_at_every_sample_rate() {
    // Indices are radians and envelopes seconds, so the spectral centroid below 16 kHz of
    // a pair agrees across host rates.
    let spec = spec_with(&[
        ("m.b.a", 3.0),
        ("op.b.ratio", 2.0),
        ("env.a.l2", 1.0),
        ("env.b.l2", 1.0),
    ]);
    let centroid = |rate: f32| {
        let mut u = efm::Unit::new(rate);
        u.configure(&spec, &[]);
        u.note_on(NoteOn {
            key: 57,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let n = (0.6 * rate) as usize;
        let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
        for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            u.process(a, b);
        }
        let len = 16_384;
        let mut re: Vec<f32> = l[n - len..]
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v * (0.5 - 0.5 * math::cos(std::f32::consts::TAU * i as f32 / len as f32))
            })
            .collect();
        let mut im = vec![0f32; len];
        fft(&mut re, &mut im);
        let (mut num, mut den) = (0f64, 0f64);
        for k in 1..len / 2 {
            let f = k as f64 * rate as f64 / len as f64;
            if f < 16_000.0 {
                let p = (re[k] * re[k] + im[k] * im[k]) as f64;
                num += f * p;
                den += p;
            }
        }
        (num / den) as f32
    };
    let c48 = centroid(48_000.0);
    for rate in [44_100.0, 96_000.0] {
        let c = centroid(rate);
        println!("spectral centroid at {rate} Hz: {c:.1} Hz (48 kHz: {c48:.1} Hz)");
        assert!((c / c48 - 1.0).abs() < 0.01, "{rate}: {c} against {c48}");
    }
}

#[test]
fn the_morph_pad_mixes_its_corners_and_does_nothing_without_them() {
    let mut s = spec_with(&[("m.b.a", 3.0), ("morph.x", 0.5)]);
    assert_eq!(
        efm::patch_of(&s).m[1][0],
        3.0,
        "no corner stored: the patch as it is"
    );
    s.set_text("morph.c1", Some("m.b.a=1;op.b.ratio=2".into()))
        .unwrap();
    s.set_text("morph.c2", Some("m.b.a=5".into())).unwrap();
    let p = efm::patch_of(&s);
    assert_eq!(p.m[1][0], 3.0, "halfway between 1 and 5");
    assert_eq!(p.ops[1].ratio, 1.5, "halfway between 2 and the default 1");
    s.set("morph.y", 1.0);
    let p = efm::patch_of(&s);
    assert_eq!(p.m[1][0], 3.0, "corners 3 and 4 unstored: the patch itself");
    s.set("morph.x", 1.0);
    s.set("morph.y", 0.0);
    assert_eq!(efm::patch_of(&s).m[1][0], 5.0);
}

#[test]
fn mpe_tuning_moves_one_note_and_volume_scales_it() {
    let spec = spec_with(&[("env.a.l2", 1.0)]);
    let mut u = efm::Unit::new(FS);
    u.configure(&spec, &[]);
    u.note_on(NoteOn {
        key: 57,
        velocity: 1.0,
        id: 7,
        channel: 1,
    });
    u.expression(7, NoteExpression::Tuning, 12.0);
    let (mut l, mut r) = (vec![0f32; 48_000], vec![0f32; 48_000]);
    u.process(&mut l, &mut r);
    let f = frequency(&l[4_800..], FS);
    assert!((f - 440.0).abs() < 0.5, "an octave up: {f}");
    u.expression(7, NoteExpression::Volume, 0.5);
    let (mut l2, mut r2) = (vec![0f32; 4_800], vec![0f32; 4_800]);
    u.process(&mut l2, &mut r2);
    let peak = l2[2_400..].iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!((peak - 0.5 * 0.125).abs() < 0.01, "half the volume: {peak}");
}

#[test]
fn the_pedal_holds_a_released_note() {
    let spec = NativeSpec::new(InstrumentKind::Fm);
    let mut u = efm::Unit::new(FS);
    u.configure(&spec, &[]);
    u.controller(Controller::Sustain, 1.0);
    u.note_on(NoteOn {
        key: 60,
        velocity: 1.0,
        id: 1,
        channel: 0,
    });
    u.note_off(60, 1, 0.0);
    let (mut l, mut r) = (vec![0f32; 48_000], vec![0f32; 48_000]);
    u.process(&mut l, &mut r);
    assert_eq!(u.active_voices(), 1, "held by the pedal");
    u.controller(Controller::Sustain, 0.0);
    u.process(&mut l, &mut r);
    assert_eq!(u.active_voices(), 0, "released when it lifts");
}

#[test]
fn sixteen_eight_operator_voices_render_within_budget() {
    // Eight operators in two stacks of four, every one sounding or modulating.
    let mut spec = NativeSpec::new(InstrumentKind::Fm);
    for (s, t) in [(7, 6), (6, 5), (5, 4), (3, 2), (2, 1), (1, 0)] {
        spec.set(cantus_core::inst::fm::M_KEYS[s][t], 1.5);
    }
    spec.set(OP_KEYS[4].out, 0.0);
    spec.set(OP_KEYS[7].fb, 0.5);
    let keys: Vec<_> = (0..16).map(|i| (36 + 3 * i as u8, 0, 8, 100)).collect();
    for (q, label) in [
        (0.0, "1×"),
        (1.0, "Auto"),
        (2.0, "2×"),
        (3.0, "4×"),
        (4.0, "8×"),
    ] {
        let mut s = spec.clone();
        s.set("quality", q);
        let p = project(s, &keys);
        let start = std::time::Instant::now();
        let out = render_at(&p, 512, 48_000, 0.0);
        let secs = start.elapsed().as_secs_f64();
        let audio = out.len() as f64 / 2.0 / 48_000.0;
        let per_voice = secs / audio / 16.0 * 100.0;
        println!("16 voices × 8 operators at {label}: {per_voice:.3} % of a core per voice");
        assert!(
            secs < audio * 4.0,
            "{label}: {secs:.2} s for {audio:.1} s of audio"
        );
    }
}
