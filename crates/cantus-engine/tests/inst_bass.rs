//! Gravis through the engine: every engine renders the same at every block size and twice
//! over, its defaults change nothing, a depth-0 route is not evaluated, the pitch survives
//! the whole path, a high note's aliasing is measured, a chord of strings keeps its
//! headroom, telemetry reports what plays, and a CPU budget holds.

mod support;
use cantus_core::inst::bass::PARAMS;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::inst::bass::{tel, Unit};
use cantus_engine::inst::{InstrumentUnit, NoteOn};
use cantus_engine::*;

const SR: f32 = 48_000.0;

fn bass(set: &[(&str, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Bass);
    for (k, v) in set {
        assert!(s.set(k, *v).is_some(), "no key {k}");
    }
    s
}

/// A bass line with legato, touching notes, a slide and a chord, eight beats at 128 BPM,
/// on a track of its own.
fn line_project(spec: NativeSpec) -> Project {
    let mut p = Project::new("gravis");
    p.tempo_bpm = 128.0;
    let mut t = p.make_track("Gravis");
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Line", Ticks::ZERO, Ticks::beats(8));
    let q = Ticks::QUARTER;
    let notes = [
        (0i64, 2i64, 28u8, 100u8),
        (2, 1, 35, 90),
        (3, 1, 40, 127), // touching the one before
        (4, 3, 33, 110), // overlapping the next: legato
        (6, 2, 45, 80),
        (10, 1, 31, 127),
        (11, 1, 31, 127),
        (13, 3, 26, 100),
    ];
    for (start, len, pitch, vel) in notes {
        clip.notes.push(p.make_note(q * start, q * len, pitch, vel));
    }
    // A three-note chord at the end (Strings mode rings it).
    for pitch in [28u8, 35, 40] {
        clip.notes.push(p.make_note(q * 20, q * 4, pitch, 100));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn render_block(p: &Project, block: usize) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.5,
        block_frames: block,
        ..Default::default()
    };
    render(std::sync::Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("render")
        .samples
}

fn engines() -> Vec<(&'static str, NativeSpec)> {
    vec![
        (
            "808",
            bass(&[("drive", 0.5), ("808.harmonics", 0.4), ("sub.level", 0.3)]),
        ),
        (
            "synth",
            bass(&[
                ("engine", 1.0),
                ("syn.unison", 5.0),
                ("syn.width", 0.7),
                ("syn.filter", 1.0),
                ("syn.reso", 0.6),
                ("phase", 1.0),
                ("clip", 1.0),
            ]),
        ),
        (
            "string",
            bass(&[("engine", 2.0), ("str.technique", 2.0), ("str.buzz", 0.7)]),
        ),
        (
            "strings mode",
            bass(&[("engine", 2.0), ("mode", 2.0), ("str.strings", 5.0)]),
        ),
    ]
}

#[test]
fn every_engine_is_block_size_independent_and_deterministic() {
    for (name, spec) in engines() {
        let p = line_project(spec);
        let reference = render_block(&p, 512);
        assert!(reference.iter().all(|x| x.is_finite()), "{name}");
        assert!(reference.iter().any(|x| *x != 0.0), "{name} is silent");
        assert_null(
            &reference,
            &render_block(&p, 512),
            0.0,
            &format!("{name} twice"),
        );
        for block in [1usize, 7, 64, 480, 4096] {
            assert_null(
                &reference,
                &render_block(&p, block),
                0.0,
                &format!("{name} at block size {block}"),
            );
        }
    }
}

#[test]
fn modulation_steps_on_the_stream_grid() {
    // An LFO on the cutoff and a macro on the drive: still identical at every block size.
    let mut spec = bass(&[("engine", 1.0), ("lfo.rate", 3.3), ("macro1", 0.6)]);
    spec.set_text("mod1.src", Some("lfo1".into())).unwrap();
    spec.set_text("mod1.dst", Some("syn.cutoff".into()))
        .unwrap();
    spec.set("mod1.amt", 0.3);
    spec.set_text("mod2.src", Some("macro1".into())).unwrap();
    spec.set_text("mod2.dst", Some("drive".into())).unwrap();
    spec.set("mod2.amt", 0.5);
    let p = line_project(spec.clone());
    let reference = render_block(&p, 512);
    for block in [1usize, 31, 33, 480] {
        assert_null(
            &reference,
            &render_block(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
    // And the routes do something.
    let mut plain = spec;
    plain.set("mod1.amt", 0.0);
    plain.set("mod2.amt", 0.0);
    assert!(render_block(&line_project(plain), 512) != reference);
}

#[test]
fn defaults_and_depth_zero_routes_change_nothing() {
    for (name, spec) in engines() {
        let reference = render_block(&line_project(spec.clone()), 256);
        // Every parameter stored at its own default.
        let mut explicit = spec.clone();
        for info in PARAMS {
            if !spec.is_stored(info.key) {
                explicit.set(info.key, info.default);
            }
        }
        assert_null(
            &reference,
            &render_block(&line_project(explicit), 256),
            0.0,
            &format!("{name}: defaults stored"),
        );
        // A connection at depth 0.
        let mut routed = spec.clone();
        routed.set_text("mod1.src", Some("lfo1".into())).unwrap();
        routed.set_text("mod1.dst", Some("gain".into())).unwrap();
        assert_null(
            &reference,
            &render_block(&line_project(routed), 256),
            0.0,
            &format!("{name}: a depth-0 route"),
        );
    }
}

/// The fundamental near `f` from the phase slope of a Hann-windowed DFT.
fn measure_f0(x: &[f32], f: f64) -> f64 {
    let sr = SR as f64;
    let win = (8.0 * sr / f) as usize;
    let mut pts = Vec::new();
    let (mut last, mut acc) = (None::<f64>, 0.0);
    let mut start = 0;
    while start + win <= x.len() {
        let (mut re, mut im) = (0.0, 0.0);
        for i in 0..win {
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / win as f64).cos();
            let a = -std::f64::consts::TAU * f * (start + i) as f64 / sr;
            re += x[start + i] as f64 * w * a.cos();
            im += x[start + i] as f64 * w * a.sin();
        }
        let ph: f64 = im.atan2(re);
        if let Some(l) = last {
            let mut d = ph - l;
            while d > std::f64::consts::PI {
                d -= std::f64::consts::TAU;
            }
            while d < -std::f64::consts::PI {
                d += std::f64::consts::TAU;
            }
            acc += d;
        }
        last = Some(ph);
        pts.push((start as f64, acc));
        start += win / 2;
    }
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
    f + sxy / sxx * sr / std::f64::consts::TAU
}

/// Run a unit directly: one note, `secs` long, in blocks of 256; the left channel.
fn run_unit(spec: &NativeSpec, key: u8, secs: f32) -> Vec<f32> {
    let mut u = Unit::new(SR);
    u.configure(spec, &[]);
    u.note_on(NoteOn {
        key,
        velocity: 0.8,
        id: 1,
        channel: 0,
    });
    let n = (secs * SR) as usize;
    let mut out = Vec::with_capacity(n);
    let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
    while out.len() < n {
        l.iter_mut().for_each(|x| *x = 0.0);
        r.iter_mut().for_each(|x| *x = 0.0);
        u.process(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out.truncate(n);
    out
}

#[test]
fn pitch_survives_the_whole_path_on_every_engine() {
    let specs = [
        (
            "808",
            bass(&[("808.pitch", 0.0), ("808.knock", 0.0), ("808.click", 0.0)]),
        ),
        (
            "synth",
            bass(&[("engine", 1.0), ("syn.cutoff", 20_000.0), ("syn.env", 0.0)]),
        ),
        (
            "string",
            bass(&[("engine", 2.0), ("str.strings", 6.0), ("str.frets", 24.0)]),
        ),
    ];
    for (name, spec) in specs {
        let mut worst = 0.0f64;
        for key in (23u8..=72).step_by(7) {
            let x = run_unit(&spec, key, 1.0);
            let want = 440.0 * 2f64.powf((key as f64 - 69.0) / 12.0);
            let f = measure_f0(&x[4800..], want);
            let c = 1200.0 * (f / want).log2();
            worst = worst.max(c.abs());
            assert!(c.abs() < 0.5, "{name} key {key}: {c:+.3} cents");
        }
        eprintln!("{name}: worst pitch error through the unit {worst:.4} cents");
    }
}

/// The strongest component (Hann-windowed DFT, dB re the fundamental) that is not a
/// harmonic, below `max_hz`.
fn alias_floor_db(x: &[f32], f0: f64, max_hz: f64) -> f64 {
    let n = x.len();
    let sr = SR as f64;
    let dft = |f: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            let a = -std::f64::consts::TAU * f * i as f64 / sr;
            re += *v as f64 * w * a.cos();
            im += *v as f64 * w * a.sin();
        }
        (re * re + im * im).sqrt()
    };
    // The fundamental exactly at its frequency; the rest on a sparse sweep of the bins.
    let fund = dft(f0);
    let bin = sr / n as f64;
    let mut worst = 0.0f64;
    let mut k = 1;
    while (k as f64) * bin < max_hz.min(0.5 * sr) {
        let f = k as f64 * bin;
        let h = f / f0;
        if (h - h.round()).abs() > 0.1 && f > 0.5 * f0 {
            worst = worst.max(dft(f));
        }
        k += 3;
    }
    20.0 * (worst / fund).log10()
}

#[test]
fn a_high_note_aliases_little() {
    // A synth saw at C6 with the filter open: PolyBLEP keeps the folded partials down.
    let saw = bass(&[
        ("engine", 1.0),
        ("syn.cutoff", 20_000.0),
        ("syn.env", 0.0),
        ("syn.sustain", 1.0),
    ]);
    let x = run_unit(&saw, 84, 0.3);
    let f0 = 440.0 * 2f64.powf(15.0 / 12.0);
    let audible = alias_floor_db(&x[4800..4800 + 8192], f0, 12_000.0);
    let full = alias_floor_db(&x[4800..4800 + 8192], f0, 24_000.0);
    eprintln!(
        "synth saw at C6: aliasing {audible:.1} dB re the fundamental below 12 kHz, {full:.1} dB          over the whole band"
    );
    assert!(audible < -40.0, "{audible:.1} dB");
    // The 808's harmonics are generated below Nyquist only: nothing folds.
    let e808 = bass(&[
        ("808.pitch", 0.0),
        ("808.knock", 0.0),
        ("808.click", 0.0),
        ("808.harmonics", 1.0),
        ("808.sustain", 1.0),
    ]);
    let x = run_unit(&e808, 96, 0.3); // C7: harmonics 2…8 reach 16.7 kHz, above stop
    let f0 = 440.0 * 2f64.powf(27.0 / 12.0);
    let db = alias_floor_db(&x[4800..4800 + 8192], f0, 24_000.0);
    eprintln!("808 with harmonics at C7: aliasing floor {db:.1} dB");
    assert!(db < -90.0, "{db:.1} dB");
}

#[test]
fn a_chord_of_strings_keeps_its_headroom() {
    let spec = bass(&[("engine", 2.0), ("mode", 2.0), ("str.strings", 6.0)]);
    let mut p = Project::new("chord");
    let mut t = p.make_track("Gravis");
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(4));
    for pitch in [23u8, 28, 33, 38, 43, 48, 55, 60] {
        clip.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(3), pitch, 127));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    let out = render_block(&p, 256);
    let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
    eprintln!("six strings at 127: peak {peak:.3}");
    assert!(peak < 1.0, "peak {peak}");
}

#[test]
fn telemetry_reports_what_plays() {
    let spec = bass(&[("engine", 2.0)]);
    let mut u = Unit::new(SR);
    u.configure(&spec, &[]);
    u.note_on(NoteOn {
        key: 45,
        velocity: 0.9,
        id: 7,
        channel: 0,
    });
    let (mut l, mut r) = ([0.0f32; 480], [0.0f32; 480]);
    assert!(u.process(&mut l, &mut r));
    let t = u.telemetry();
    assert_eq!(t[0], 1.0, "one voice");
    assert_eq!(t[tel::ENGINE], 2.0);
    assert!((t[tel::PITCH] - 45.0).abs() < 1e-4);
    assert!(t[tel::STRING] >= 0.0, "a string is sounding");
    assert!(t[tel::PEAK] > 0.0);
    // A2 on a four-string, Position fingering from the first position: the G string at the
    // second fret — not an open string.
    assert!(t[tel::FRET] > 0.0);
}

#[test]
fn a_cpu_budget_holds() {
    // Six strings ringing in Strings mode with every feature on, for ten seconds of audio.
    let spec = bass(&[
        ("engine", 2.0),
        ("mode", 2.0),
        ("str.strings", 6.0),
        ("str.buzz", 1.0),
        ("str.stiffness", 2.0),
        ("str.piezo", 1.0),
        ("str.pu_type", 1.0),
        ("sub.level", 0.3),
        ("drive", 0.6),
        ("clip", 1.0),
        ("subsonic", 25.0),
        ("str.sustain", 20.0),
    ]);
    let mut u = Unit::new(SR);
    u.configure(&spec, &[]);
    for (i, key) in [23u8, 30, 35, 40, 45, 50].iter().enumerate() {
        u.note_on(NoteOn {
            key: *key,
            velocity: 1.0,
            id: i as u64,
            channel: 0,
        });
    }
    let secs = 10.0f64;
    let blocks = (secs * SR as f64 / 512.0) as usize;
    let (mut l, mut r) = ([0.0f32; 512], [0.0f32; 512]);
    let start = std::time::Instant::now();
    for _ in 0..blocks {
        l.iter_mut().for_each(|x| *x = 0.0);
        r.iter_mut().for_each(|x| *x = 0.0);
        u.process(&mut l, &mut r);
    }
    let took = start.elapsed().as_secs_f64();
    assert_eq!(u.active_voices(), 6, "all six still ring");
    let per_voice = took / secs / 6.0 * 100.0;
    eprintln!(
        "six strings, every feature: {took:.3} s for {secs} s of audio — {per_voice:.2} % of a \
         core per voice"
    );
    // The house rule for a budget on a shared, loaded build machine (as
    // `stress_project_renders_within_budget` and Lamina's): at least real time. The
    // measured figure is printed above; a quiet machine does it in well under a second.
    assert!(took < secs, "{took:.2} s for {secs} s of audio");
    // The 808 and the synth, one voice each.
    for spec in [
        bass(&[("drive", 1.0), ("808.harmonics", 1.0), ("clip", 1.0)]),
        bass(&[
            ("engine", 1.0),
            ("syn.unison", 7.0),
            ("syn.width", 1.0),
            ("syn.filter", 1.0),
        ]),
    ] {
        let mut u = Unit::new(SR);
        u.configure(&spec, &[]);
        u.note_on(NoteOn {
            key: 36,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let start = std::time::Instant::now();
        for _ in 0..blocks {
            l.iter_mut().for_each(|x| *x = 0.0);
            r.iter_mut().for_each(|x| *x = 0.0);
            u.process(&mut l, &mut r);
        }
        let took = start.elapsed().as_secs_f64();
        eprintln!(
            "one voice ({}): {:.2} % of a core",
            spec.params.len(),
            took / secs * 100.0
        );
        assert!(took < secs, "{took:.2} s for {secs} s of audio");
    }
}

#[test]
fn every_engine_goes_exactly_silent() {
    for (name, spec) in engines() {
        let p = line_project(spec);
        let opts = RenderOptions {
            tail_seconds: 4.0,
            block_frames: 256,
            ..Default::default()
        };
        let out = render(std::sync::Arc::new(Snapshot::compile(&p, 1)), &opts)
            .expect("render")
            .samples;
        let last = &out[out.len() - 48_000 * 2..];
        assert!(last.iter().all(|x| *x == 0.0), "{name} still sounds");
    }
}
