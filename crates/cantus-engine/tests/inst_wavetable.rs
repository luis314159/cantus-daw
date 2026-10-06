//! Unda (the wavetable synth) through the engine: its defaults change nothing, every block
//! size gives the same samples, its pitch is exact, its aliasing is measured at the notes
//! where it matters, imported tables play, modulation moves what it names, and a voice
//! costs what the document says. `cargo test -p cantus-engine --test inst_wavetable --
//! --nocapture` prints the measured numbers.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use std::sync::Arc;
use support::*;

const SR: f64 = 48_000.0;

/// A project whose only track plays Unda with `spec`, the notes given as
/// `(start beat, length in beats, key, velocity)`, at 120 BPM.
fn project(spec: &NativeSpec, notes: &[(f64, f64, u8, u8)]) -> Project {
    let mut p = Project::new("unda");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Unda");
    t.instrument = InstrumentSpec::Native(spec.clone());
    let end = notes.iter().fold(1.0f64, |m, n| m.max(n.0 + n.1));
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::from_beats_f64(end.ceil()));
    for &(start, len, key, vel) in notes {
        clip.notes.push(p.make_note(
            Ticks::from_beats_f64(start),
            Ticks::from_beats_f64(len),
            key,
            vel,
        ));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn unda() -> NativeSpec {
    NativeSpec::new(InstrumentKind::Wavetable)
}

fn with(values: &[(&str, f32)]) -> NativeSpec {
    let mut s = unda();
    for (k, v) in values {
        assert!(s.set(k, *v).is_some(), "{k} is not a Unda parameter");
    }
    s
}

/// A busy patch: two oscillators in unison with warps, the sub and noise, both filters,
/// an LFO and an envelope on the matrix, glide.
fn busy() -> NativeSpec {
    let mut s = with(&[
        ("osc1.uni", 7.0),
        ("osc1.detune", 30.0),
        ("osc1.warp1", 2.0),
        ("osc1.warp1.amt", 0.4),
        ("osc2.on", 1.0),
        ("osc2.wt", 2.0),
        ("osc2.uni", 3.0),
        ("osc2.warp1", 13.0),
        ("osc2.warp1.amt", 0.3),
        ("osc2.warp1.src", 0.0),
        ("sub.on", 1.0),
        ("noise.on", 1.0),
        ("noise.level", -30.0),
        ("flt1.on", 1.0),
        ("flt1.type", 6.0),
        ("flt1.res", 0.5),
        ("flt2.on", 1.0),
        ("flt.routing", 0.4),
        ("voice.glide", 0.05),
        ("voice.glide_mode", 1.0),
        ("voice.drift", 0.5),
    ]);
    for (slot, src, dst, amt) in [
        (1, "lfo1", "osc1.pos", 0.3),
        (2, "env2", "flt1.cutoff", 0.25),
        (3, "vel", "osc2.level", -0.2),
    ] {
        let k = cantus_core::inst::modulation::SLOTS[slot - 1];
        s.set_text(k.src, Some(src.into())).unwrap();
        s.set_text(k.dst, Some(dst.into())).unwrap();
        s.set(k.amt, amt);
    }
    s.set("lfo1.rate", 5.0);
    s
}

fn riff() -> Vec<(f64, f64, u8, u8)> {
    vec![
        (0.0, 0.5, 48, 100),
        (0.25, 1.0, 55, 80),
        (0.5, 0.5, 60, 127),
        (1.0, 1.5, 67, 60),
        (1.2, 0.3, 36, 110),
    ]
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

#[test]
fn every_block_size_gives_the_same_samples_on_a_busy_patch() {
    let p = project(&busy(), &riff());
    let reference = render_with(&p, 512);
    assert!(peak(&reference) > 0.01, "it plays");
    assert!(reference.iter().all(|x| x.is_finite()));
    for block in [1usize, 7, 64, 1000] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

#[test]
fn two_renders_are_identical_and_the_seed_changes_them() {
    let p = project(&busy(), &riff());
    assert_eq!(render_of(&p), render_of(&p));
    let mut s = busy();
    s.set("voice.seed", 7.0);
    let q = project(&s, &riff());
    assert_ne!(render_of(&p), render_of(&q), "another seed, other phases");
}

#[test]
fn every_parameter_set_to_its_default_changes_no_sample() {
    let notes = riff();
    let reference = render_of(&project(&unda(), &notes));
    let mut all = unda();
    for info in InstrumentKind::Wavetable.params() {
        all.set(info.key, info.default);
    }
    assert!(all.params.len() > 200);
    assert_null(
        &reference,
        &render_of(&project(&all, &notes)),
        0.0,
        "all defaults",
    );
}

#[test]
fn inert_settings_are_bit_identical_to_their_absence() {
    let notes = riff();
    let reference = render_of(&project(&unda(), &notes));
    // A warp mode with no amount, unison settings with one lane, a route at depth 0, a
    // muted route, a filter at mix 0 and an off filter's settings.
    let mut s = with(&[
        ("osc1.warp1", 5.0),
        ("osc1.warp2", 13.0),
        ("osc1.detune", 80.0),
        ("osc1.blend", 0.9),
        ("osc1.width", 0.2),
        ("osc1.stack", 3.0),
        ("osc1.ucurve", 3.0),
        ("flt1.on", 1.0),
        ("flt1.mix", 0.0),
        ("flt2.type", 7.0),
        ("flt2.cutoff", 300.0),
    ]);
    let k = cantus_core::inst::modulation::SLOTS;
    s.set_text(k[0].src, Some("lfo1".into())).unwrap();
    s.set_text(k[0].dst, Some("osc1.pos".into())).unwrap();
    s.set_text(k[1].src, Some("env2".into())).unwrap();
    s.set_text(k[1].dst, Some("flt1.cutoff".into())).unwrap();
    s.set(k[1].amt, 0.8);
    s.set(k[1].on, 0.0);
    assert_null(
        &reference,
        &render_of(&project(&s, &notes)),
        0.0,
        "inert settings",
    );
}

/// The frequency of the left channel between `from` and `to` seconds, from its upward
/// zero crossings (linearly interpolated).
fn frequency(x: &[f32], from: f64, to: f64) -> f64 {
    let (a, b) = ((from * SR) as usize, (to * SR) as usize);
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0usize;
    for i in a..b {
        let (y0, y1) = (x[2 * i] as f64, x[2 * i + 2] as f64);
        if y0 < 0.0 && y1 >= 0.0 {
            let t = i as f64 + y0 / (y0 - y1);
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    count as f64 * SR / (last - first.expect("crossings"))
}

#[test]
fn the_pitch_is_exact_across_the_keyboard() {
    // A sine (the last frame of the basic table), no random phase.
    let s = with(&[("osc1.pos", 1.0), ("osc1.rand", 0.0)]);
    let mut worst = 0.0f64;
    for key in [24u8, 36, 45, 57, 69, 81, 93, 105] {
        let out = render_of(&project(&s, &[(0.0, 2.4, key, 100)]));
        let hz = frequency(&out, 0.2, 1.2);
        let want = 440.0 * math::powf(2f64, (key as f64 - 69.0) / 12.0);
        let cents = 1200.0 * math::log2(hz / want);
        worst = worst.max(cents.abs());
        assert!(
            cents.abs() < 0.05,
            "key {key}: {hz:.4} Hz for {want:.4} ({cents:+.4} ct)"
        );
    }
    println!("pitch: worst error {worst:.5} cents (keys 24…105)");
}

/// Energy between the harmonics of `f0`, below 20 kHz, relative to the fundamental, in
/// dB: a Kaiser-windowed (β = 20) FFT of the left channel from 0.25 s, the bins within
/// ±12 of a harmonic (or of DC) left out.
fn alias_floor(x: &[f32], f0: f64) -> f64 {
    use cantus_dsp::aequalis::fft::Fft;
    let n = 65_536usize;
    let start = (0.25 * SR) as usize;
    let beta = 20.0f64;
    let i0 = |v: f64| {
        let (mut sum, mut term) = (1.0f64, 1.0f64);
        for k in 1..200 {
            term *= (v * v / 4.0) / (k * k) as f64;
            sum += term;
            if term < 1e-18 * sum {
                break;
            }
        }
        sum
    };
    let norm = i0(beta);
    let mut re: Vec<f32> = (0..n)
        .map(|i| {
            let r = 2.0 * i as f64 / (n - 1) as f64 - 1.0;
            (x[2 * (start + i)] as f64 * i0(beta * (1.0 - r * r).max(0.0).sqrt()) / norm) as f32
        })
        .collect();
    let mut im = vec![0.0f32; n];
    Fft::new(n).run(&mut re, &mut im, false);
    let bin = SR / n as f64;
    let power = |k: usize| math::powi(re[k] as f64, 2) + math::powi(im[k] as f64, 2);
    let near = |k: usize| {
        let h = (k as f64 * bin / f0).round();
        ((k as f64) - h * f0 / bin).abs() <= 12.0
    };
    let fund = (f0 / bin).round() as usize;
    let p1: f64 = (fund - 12..=fund + 12).map(power).sum();
    let top = (20_000.0 / bin) as usize;
    let alias: f64 = (1..top).filter(|k| !near(*k)).map(power).sum();
    10.0 * math::log10(alias.max(1e-30) / p1)
}

#[test]
fn aliasing_is_measured_where_it_matters() {
    // A full saw (the basic table's first frame), no random phase, velocity 127.
    let mut rows = Vec::new();
    for (q, label, budget) in [(1.0, "2x", -90.0), (0.0, "1x", -60.0), (2.0, "4x", -90.0)] {
        for key in [33u8, 96, 108] {
            let s = with(&[("osc1.rand", 0.0), ("quality", q)]);
            let out = render_of(&project(&s, &[(0.0, 3.0, key, 127)]));
            let f0 = 440.0 * math::powf(2f64, (key as f64 - 69.0) / 12.0);
            let floor = alias_floor(&out, f0);
            rows.push((format!("{label} key {key} ({f0:.1} Hz)"), floor, budget));
        }
    }
    println!("aliasing (energy between harmonics below 20 kHz, re the fundamental):");
    for (r, floor, _) in &rows {
        println!("  {r}: {floor:.1} dB");
    }
    for (r, floor, budget) in rows {
        assert!(floor < budget, "{r}: alias floor {floor:.1} dB");
    }
}

#[test]
fn the_init_patch_keeps_its_headroom() {
    let notes: Vec<_> = [36u8, 48, 55, 60, 64, 67, 72, 79]
        .iter()
        .map(|k| (0.0, 3.0, *k, 127))
        .collect();
    let chord = peak(&render_of(&project(&unda(), &notes)));
    let one = peak(&render_of(&project(&unda(), &[(0.0, 1.0, 60, 127)])));
    println!(
        "init: one note peaks at {:.1} dBFS, an eight-note chord at {:.1} dBFS",
        20.0 * math::log10(one),
        20.0 * math::log10(chord)
    );
    assert!(chord < 1.0, "peak {chord}");
}

#[test]
fn modulation_moves_what_it_names() {
    let notes = [(0.0, 2.0, 57u8, 100u8)];
    let base = with(&[("osc1.rand", 0.0), ("flt1.on", 1.0), ("flt1.cutoff", 400.0)]);
    let plain = render_of(&project(&base, &notes));
    let mut m = base.clone();
    let k = cantus_core::inst::modulation::SLOTS[0];
    m.set_text(k.src, Some("macro1".into())).unwrap();
    m.set_text(k.dst, Some("flt1.cutoff".into())).unwrap();
    m.set(k.amt, 0.5);
    // The macro at 0: nothing moves.
    assert_null(
        &plain,
        &render_of(&project(&m, &notes)),
        0.0,
        "macro at zero",
    );
    m.set("macro1", 1.0);
    let bright = render_of(&project(&m, &notes));
    // Half the cutoff range up (×35): much brighter, so the energy of the sample-to-sample
    // difference grows.
    let hf = |x: &[f32]| {
        x.windows(4)
            .step_by(2)
            .map(|w| (w[2] - w[0]) as f64 * (w[2] - w[0]) as f64)
            .sum::<f64>()
    };
    assert!(
        hf(&bright) > 10.0 * hf(&plain),
        "{} vs {}",
        hf(&bright),
        hf(&plain)
    );
}

#[test]
fn an_imported_table_plays_what_the_file_holds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("third.wav");
    // Two frames of 1024 samples: the third harmonic, then silence.
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for f in 0..2 {
        for i in 0..1024 {
            let x = i as f32 / 1024.0;
            let v = if f == 0 {
                math::sin(std::f32::consts::TAU * 3.0 * x)
            } else {
                0.0
            };
            w.write_sample(v).unwrap();
        }
    }
    w.finalize().unwrap();
    let mut s = with(&[("osc1.rand", 0.0), ("osc1.frame", 1024.0)]);
    s.set_text("osc1.table", Some(path.to_string_lossy().into_owned()))
        .unwrap();
    let p = project(&s, &[(0.0, 2.0, 45, 100)]);
    let mut snap = Snapshot::compile(&p, 1);
    let mut cache = cantus_engine::SampleCache::new();
    assert!(cache.attach(&mut snap).is_empty());
    let opts = cantus_engine::RenderOptions {
        tail_seconds: 0.25,
        ..Default::default()
    };
    let out = cantus_engine::render(Arc::new(snap), &opts)
        .unwrap()
        .samples;
    // A2 is 110 Hz; the table holds its third harmonic alone.
    let hz = frequency(&out, 0.2, 0.9);
    assert!((hz / 330.0 - 1.0).abs() < 1e-4, "{hz} Hz");
    // A file that cannot be read is silence, never an error.
    let mut gone = s.clone();
    gone.set_text(
        "osc1.table",
        Some(dir.path().join("nope.wav").to_string_lossy().into_owned()),
    )
    .unwrap();
    let p = project(&gone, &[(0.0, 1.0, 45, 100)]);
    let mut snap = Snapshot::compile(&p, 1);
    assert_eq!(cache.attach(&mut snap).len(), 1);
    let out = cantus_engine::render(Arc::new(snap), &opts)
        .unwrap()
        .samples;
    assert!(out.iter().all(|x| *x == 0.0));
}

#[test]
fn grana_plays_an_imported_sample_and_partes_its_table_at_every_block_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..48_000 {
        w.write_sample(math::sin(std::f32::consts::TAU * 261.63 * i as f32 / 48_000.0) * 0.8)
            .unwrap();
    }
    w.finalize().unwrap();
    let mut s = with(&[
        ("osc1.engine", 2.0),
        ("osc1.pos", 0.3),
        ("osc1.density", 50.0),
        ("osc1.gpitch", 20.0),
        ("osc2.on", 1.0),
        ("osc2.engine", 1.0),
        ("osc2.stretch", 0.02),
        ("osc2.level", -6.0),
    ]);
    s.set_text("osc1.table", Some(path.to_string_lossy().into_owned()))
        .unwrap();
    let p = project(&s, &[(0.0, 1.5, 60, 100), (0.5, 1.0, 67, 90)]);
    let snap = |p: &Project| {
        let mut snap = Snapshot::compile(p, 1);
        cantus_engine::SampleCache::new().attach(&mut snap);
        Arc::new(snap)
    };
    let run = |block: usize| {
        let opts = cantus_engine::RenderOptions {
            tail_seconds: 0.5,
            block_frames: block,
            ..Default::default()
        };
        cantus_engine::render(snap(&p), &opts).unwrap().samples
    };
    let reference = run(256);
    assert!(peak(&reference) > 0.01 && reference.iter().all(|x| x.is_finite()));
    for block in [1usize, 64, 1000] {
        assert_null(&reference, &run(block), 0.0, &format!("block {block}"));
    }
}

/// A wall-clock ratio, so it runs on demand in release (`cargo test --release -p
/// cantus-engine --test inst_wavetable -- --ignored`), as ADR 0024 §13 does for every
/// timing assertion: in a debug build on a machine shared with other builds it measured
/// the worst case anywhere from 15× to 121× Firmus.
#[test]
#[ignore = "timing: run in release with --ignored"]
fn a_voice_costs_what_the_document_says() {
    use std::time::Instant;
    let cases = [
        ("init (1 osc, 1 lane, 2x)", unda(), 16usize),
        (
            "supersaw (1 osc x 7, ladder, 2x)",
            with(&[("osc1.uni", 7.0), ("flt1.on", 1.0), ("flt1.type", 6.0)]),
            16,
        ),
        (
            "pad (2 osc x 7, SVF, LFO on position, 2x)",
            {
                let mut s = with(&[
                    ("osc1.uni", 7.0),
                    ("osc2.on", 1.0),
                    ("osc2.uni", 7.0),
                    ("flt1.on", 1.0),
                ]);
                let k = cantus_core::inst::modulation::SLOTS[0];
                s.set_text(k.src, Some("lfo1".into())).unwrap();
                s.set_text(k.dst, Some("osc1.pos".into())).unwrap();
                s.set(k.amt, 0.3);
                s
            },
            16,
        ),
        (
            "worst (3 osc x 16, 2 ladders, 4x)",
            with(&[
                ("osc1.uni", 16.0),
                ("osc2.on", 1.0),
                ("osc2.uni", 16.0),
                ("osc3.on", 1.0),
                ("osc3.uni", 16.0),
                ("flt1.on", 1.0),
                ("flt1.type", 6.0),
                ("flt2.on", 1.0),
                ("flt2.type", 6.0),
                ("quality", 2.0),
            ]),
            4,
        ),
    ];
    // Seconds of work per second of audio per voice, one run.
    let once = |p: &Project, voices: usize| {
        let t = Instant::now();
        let out = render_of(p);
        let secs = t.elapsed().as_secs_f64();
        secs / (out.len() as f64 / 2.0 / SR) / voices as f64
    };
    // The baseline, measured now on the same machine: Firmus, sixteen voices.
    let mut firmus = Project::new("firmus");
    firmus.tempo_bpm = 120.0;
    let mut t = firmus.make_track("Firmus");
    let mut clip = firmus.make_clip("Notes", Ticks::ZERO, Ticks::beats(4));
    for i in 0..16u8 {
        clip.notes
            .push(firmus.make_note(Ticks::ZERO, Ticks::beats(4), 40 + 3 * i, 100));
    }
    t.clips.push(clip);
    firmus.tracks.push(t);
    println!("CPU per voice (one core, 48 kHz), each case against Firmus measured beside it:");
    // Ceilings relative to Firmus, about twice what was measured (DSP bench: init 1.0×,
    // seven lanes 4×, the ladder 2.3×).
    let ceilings = [3.0, 12.0, 25.0, 120.0];
    for ((label, spec, voices), ceiling) in cases.into_iter().zip(ceilings) {
        let notes: Vec<_> = (0..voices)
            .map(|i| (0.0, 4.0, 40 + 3 * i as u8, 100))
            .collect();
        // The case and the baseline alternate, and each keeps its least disturbed run: a
        // load that comes and goes then weighs on both, not on one.
        let p = project(&spec, &notes);
        let (mut c, mut base) = (f64::MAX, f64::MAX);
        for _ in 0..5 {
            c = c.min(once(&p, voices));
            base = base.min(once(&firmus, 16));
        }
        println!("  {label}: {:.3} % ({:.1}x Firmus)", 100.0 * c, c / base);
        assert!(c / base < ceiling, "{label}: {:.1}x Firmus", c / base);
    }
}
