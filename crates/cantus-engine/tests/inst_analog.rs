//! Calor through the engine (ADR 0023): the same bits at every block size and wherever
//! the render starts, every control at its default changing nothing, pitch in cents,
//! the aliasing floor of a high note, headroom, the pedal, per-note expression, the
//! matrix, the telemetry the editor reads, and a CPU budget.

use cantus_core::inst::modulation::SLOTS;
use cantus_core::*;
use cantus_dsp::math;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::inst as einst;
use cantus_engine::*;
use std::sync::Arc;

const KIND: InstrumentKind = InstrumentKind::Analog;

/// One Calor track playing `notes` (start beat, length in beats, pitch, velocity).
fn project(spec: NativeSpec, notes: &[(f64, f64, u8, u8)]) -> Project {
    let mut p = Project::new("calor");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Calor");
    t.instrument = InstrumentSpec::Native(spec);
    let end = notes.iter().fold(1.0f64, |m, n| m.max(n.0 + n.1)).ceil() as i64 + 1;
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(end));
    for (start, len, pitch, vel) in notes {
        let at = Ticks((start * Ticks::QUARTER.0 as f64) as i64);
        let len = Ticks((len * Ticks::QUARTER.0 as f64) as i64);
        clip.notes.push(p.make_note(at, len, *pitch, *vel));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn spec(values: &[(&str, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(KIND);
    for (k, v) in values {
        assert!(s.set(k, *v).is_some(), "Calor has no key {k}");
    }
    s
}

fn connect(s: &mut NativeSpec, slot: usize, src: &str, dst: &str, amt: f32) {
    let k = SLOTS[slot];
    s.set_text(k.src, Some(src.into())).unwrap();
    s.set_text(k.dst, Some(dst.into())).unwrap();
    s.set(k.amt, amt).unwrap();
}

fn render_opts(p: &Project, block: usize, tail: f64) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: tail,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("render")
        .samples
}

/// A patch that moves: drift, LFOs, glide, unison, sync, filter FM, the chorus, a route.
fn busy() -> NativeSpec {
    let mut s = spec(&[
        ("analog.condition", 0.8),
        ("glide.time", 0.06),
        ("voice.stack", 2.0),
        ("osc2.sync", 1.0),
        ("mix.osc3", 0.4),
        ("mix.sub", 0.3),
        ("mix.noise", 0.05),
        ("vcf.reso", 0.8),
        ("vcf.fm", 0.2),
        ("vcf.lfo", 0.5),
        ("lfo1.pitch", 15.0),
        ("chorus.mode", 2.0),
        ("macro1", 0.6),
    ]);
    connect(&mut s, 0, "lfo2", "vcf.cutoff", 0.1);
    connect(&mut s, 1, "macro1", "vcf.reso", 0.1);
    connect(&mut s, 2, "vel", "env2.a", -0.05);
    s
}

fn riff() -> Vec<(f64, f64, u8, u8)> {
    vec![
        (0.0, 1.5, 48, 100),
        (0.0, 1.5, 55, 90),
        (0.75, 0.5, 60, 127),
        (1.5, 1.0, 62, 70),
        (2.25, 0.3, 67, 110),
        (2.5, 1.2, 43, 100),
    ]
}

#[test]
fn calor_renders_the_same_bits_at_every_block_size_and_quality() {
    for quality in [0.0, 1.0, 2.0] {
        let mut s = busy();
        s.set("quality", quality);
        let p = project(s, &riff());
        let reference = render_opts(&p, 512, 0.5);
        assert!(reference.iter().any(|x| *x != 0.0), "Calor is silent");
        for block in [1usize, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_opts(&p, block, 0.5),
                0.0,
                &format!("quality {quality} at block size {block}"),
            );
        }
    }
}

#[test]
fn a_note_sounds_the_same_wherever_the_render_starts() {
    let mut s = busy();
    s.set("glide.time", 0.001); // gliding from the note before is history by definition
    s.set("chorus.mode", 0.0);
    s.set("env2.r", 50.0);
    s.set("lfo1.restart", 1.0);
    let p = project(s, &[(0.0, 1.0, 50, 100), (8.0, 2.0, 62, 100)]);
    let full = render_opts(&p, 256, 0.5);
    let opts = RenderOptions {
        start: Ticks::beats(8),
        tail_seconds: 0.5,
        block_frames: 256,
        ..Default::default()
    };
    let from_bar = render(Arc::new(Snapshot::compile(&p, 1)), &opts)
        .unwrap()
        .samples;
    let at = 4 * 48_000 * 2; // beat 8 at 120 BPM, interleaved
    let n = from_bar.len().min(full.len() - at);
    assert_null(
        &full[at..at + n],
        &from_bar[..n],
        0.0,
        "rendered from beat 8",
    );
}

#[test]
fn every_control_at_its_default_changes_nothing() {
    let notes = [(0.0, 0.5, 57, 100), (0.25, 0.5, 64, 80)];
    let reference = render_opts(&project(NativeSpec::new(KIND), &notes), 512, 0.3);
    for info in KIND.params() {
        let s = spec(&[(info.key, info.default)]);
        assert_null(
            &reference,
            &render_opts(&project(s, &notes), 512, 0.3),
            0.0,
            &format!("{} set to its default", info.key),
        );
    }
    // A route at depth 0 is not evaluated.
    let mut s = NativeSpec::new(KIND);
    connect(&mut s, 0, "lfo1", "vcf.cutoff", 0.0);
    assert_null(
        &reference,
        &render_opts(&project(s, &notes), 512, 0.3),
        0.0,
        "a route at depth 0",
    );
}

fn calibrated_saw() -> NativeSpec {
    spec(&[
        ("analog.condition", 0.0),
        ("mix.osc2", 0.0),
        ("osc1.rev", 0.0),
        ("vcf.cutoff", 20_000.0),
        ("vcf.reso", 0.0),
        ("vcf.env", 0.0),
        ("vcf.key", 0.0),
    ])
}

/// The left channel of an interleaved render.
fn left(x: &[f32]) -> Vec<f32> {
    x.iter().step_by(2).copied().collect()
}

#[test]
fn notes_are_on_pitch_across_the_keyboard() {
    for key in [21u8, 33, 45, 57, 69, 81, 93, 105] {
        let out = left(&render_opts(
            &project(calibrated_saw(), &[(0.0, 2.0, key, 100)]),
            512,
            0.0,
        ));
        let f = frequency(&out[12_000..44_000]);
        let want = 440.0 * math::powf(2f64, (key as f64 - 69.0) / 12.0);
        let cents = 1200.0 * math::log2(f / want);
        assert!(cents.abs() < 0.1, "key {key}: {cents:+.3} cents");
    }
}

/// Frequency from upward zero crossings.
fn frequency(x: &[f32]) -> f64 {
    let mean = x.iter().map(|v| *v as f64).sum::<f64>() / x.len() as f64;
    let (mut first, mut last, mut n) = (None, 0.0, 0usize);
    for i in 1..x.len() {
        let (a, b) = (x[i - 1] as f64 - mean, x[i] as f64 - mean);
        if a < 0.0 && b >= 0.0 {
            let t = (i - 1) as f64 + a / (a - b);
            first.get_or_insert(t);
            last = t;
            n += 1;
        }
    }
    (n - 1) as f64 * 48_000.0 / (last - first.unwrap())
}

#[test]
fn a_high_note_keeps_its_aliasing_under_the_floor() {
    // C7 (2093 Hz), a saw with the filter open: the strongest component in the audible
    // band that is not a harmonic, against the fundamental. Quietly into the filter the
    // oscillator's own floor shows; at the default level the filter's saturation makes
    // partials of its own, and the oversampling (with the mixer's 22 kHz bandwidth ahead
    // of the saturation) is what keeps their folds down.
    let floor = |values: &[(&str, f32)]| {
        let mut s = calibrated_saw();
        for (k, v) in values {
            s.set(k, *v);
        }
        let out = left(&render_opts(&project(s, &[(0.0, 3.0, 96, 100)]), 512, 0.0));
        aliasing_floor_db(&out[12_000..12_000 + (1 << 15)], 2093.0)
    };
    let clean = floor(&[("mix.osc1", 0.05)]);
    let driven_good = floor(&[]);
    let driven_best = floor(&[("quality", 2.0)]);
    let driven_draft = floor(&[("quality", 0.0)]);
    println!(
        "C7 saw, aliasing floor: clean {clean:.1} dB; driven: draft {driven_draft:.1}, good {driven_good:.1}, best {driven_best:.1} dB"
    );
    assert!(clean < -85.0, "clean: {clean:.1} dB");
    assert!(driven_good < -78.0, "driven at Good: {driven_good:.1} dB");
    assert!(driven_best < -92.0, "driven at Best: {driven_best:.1} dB");
}

/// A plain DFT-free floor: Goertzel at every bin would be slow, so an FFT (radix 2).
fn aliasing_floor_db(x: &[f32], f0: f64) -> f64 {
    let n = x.len();
    let mut re: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let p = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            let w = 0.35875 - 0.48829 * math::cos(p) + 0.14128 * math::cos(2.0 * p)
                - 0.01168 * math::cos(3.0 * p);
            *v as f64 * w
        })
        .collect();
    let mut im = vec![0.0; n];
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for s in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = (math::cos(ang * k as f64), math::sin(ang * k as f64));
                let (a, b) = (s + k, s + k + len / 2);
                let (tr, ti) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
    let mag: Vec<f64> = (0..n / 2)
        .map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt())
        .collect();
    let bin = |f: f64| f * n as f64 / 48_000.0;
    let fb = bin(f0).round() as usize;
    let fund = mag[fb - 3..fb + 4].iter().cloned().fold(0.0, f64::max);
    let mut worst = 0.0f64;
    for (i, m) in mag.iter().enumerate() {
        let f = i as f64 * 48_000.0 / n as f64;
        if !(20.0..=20_000.0).contains(&f) {
            continue;
        }
        let h = (f / f0).round().max(1.0);
        if (i as f64 - bin(h * f0)).abs() <= 8.0 {
            continue;
        }
        worst = worst.max(*m);
    }
    20.0 * math::log10(worst / fund)
}

/// A unit driven by hand, 48 kHz.
fn unit(s: &NativeSpec) -> Box<dyn einst::InstrumentUnit> {
    let mut u = einst::make(KIND, 48_000.0);
    u.configure(s, &[]);
    u
}

fn run(u: &mut dyn einst::InstrumentUnit, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(cl, cr);
    }
    (l, r)
}

fn note(key: u8, id: u64) -> einst::NoteOn {
    einst::NoteOn {
        key,
        velocity: 0.8,
        id,
        channel: 0,
    }
}

#[test]
fn the_pedal_holds_a_released_note_until_it_lifts() {
    let s = spec(&[("env2.r", 20.0)]);
    let mut u = unit(&s);
    u.controller(einst::Controller::Sustain, 1.0);
    u.note_on(note(60, 1));
    run(u.as_mut(), 4800);
    u.note_off(60, 1, 0.0);
    run(u.as_mut(), 24_000);
    assert_eq!(u.active_voices(), 1, "the pedal holds the note");
    u.controller(einst::Controller::Sustain, 0.0);
    run(u.as_mut(), 24_000);
    assert_eq!(u.active_voices(), 0, "lifting it lets the note go");
}

#[test]
fn a_note_expression_reaches_only_its_note() {
    let s = spec(&[("analog.condition", 0.0), ("gain", -18.0)]);
    let both = {
        let mut u = unit(&s);
        u.note_on(note(48, 1));
        u.note_on(note(72, 2));
        u.expression(1, einst::NoteExpression::Tuning, 1.0);
        u.expression(1, einst::NoteExpression::Pan, -1.0);
        run(u.as_mut(), 12_000)
    };
    let one = {
        let mut u = unit(&s);
        u.note_on(note(48, 1));
        u.expression(1, einst::NoteExpression::Tuning, 1.0);
        u.expression(1, einst::NoteExpression::Pan, -1.0);
        run(u.as_mut(), 12_000)
    };
    let alone2 = {
        let mut u = unit(&s);
        u.note_on(note(72, 2));
        run(u.as_mut(), 12_000)
    };
    let worst = both
        .0
        .iter()
        .zip(one.0.iter().zip(alone2.0.iter()))
        .map(|(b, (x, y))| (b - (x + y)).abs())
        .fold(0f32, f32::max);
    assert!(
        worst < 1e-5,
        "the expression leaked to the other note ({worst})"
    );
}

#[test]
fn a_route_moves_its_target_and_a_macro_drives_it() {
    let notes = [(0.0, 1.0, 45, 100)];
    let render_with_macro = |m: f32| {
        let mut s = spec(&[("macro2", m)]);
        connect(&mut s, 0, "macro2", "vcf.cutoff", -0.5);
        render_opts(&project(s, &notes), 512, 0.2)
    };
    let (open, closed) = (render_with_macro(0.0), render_with_macro(1.0));
    let rms = |x: &[f32]| {
        (x.iter().map(|v| math::powi(*v as f64, 2)).sum::<f64>() / x.len() as f64).sqrt()
    };
    assert!(
        rms(&closed) < 0.8 * rms(&open),
        "the macro closed the filter"
    );
}

#[test]
fn the_editor_is_told_which_cards_play_and_where_the_cutoff_is() {
    let mut u = unit(&NativeSpec::new(KIND));
    u.note_on(note(60, 11));
    u.note_on(note(64, 12));
    run(u.as_mut(), 2400);
    let t = u.telemetry();
    assert_eq!(t[0], 2.0, "two voices");
    assert_eq!(t[1], 8.0, "of eight");
    let lit: Vec<f32> = (8..24).filter(|i| t[*i] > 0.0).map(|i| t[i + 16]).collect();
    assert_eq!(lit.len(), 2, "two cards lit: {t:?}");
    assert!(lit.contains(&61.0) && lit.contains(&65.0));
    assert!(t[41] > 20.0 && t[41] < 20_000.0, "the live cutoff, Hz");
    assert_eq!(t[44], 2.0, "Good is 2×");
}

#[test]
fn eight_voices_render_within_budget() {
    // Eight voices for four seconds at Good, well inside real time even in a test build.
    let chord: Vec<(f64, f64, u8, u8)> = [36u8, 43, 48, 52, 55, 60, 64, 67]
        .iter()
        .map(|k| (0.0, 8.0, *k, 100))
        .collect();
    let p = project(busy(), &chord);
    let t0 = std::time::Instant::now();
    let out = render_opts(&p, 512, 0.0);
    let secs = out.len() as f64 / 2.0 / 48_000.0;
    let took = t0.elapsed().as_secs_f64();
    println!("eight voices (stack 2) for {secs:.1} s of audio took {took:.2} s");
    assert!(took < secs * 2.0, "{took:.2} s for {secs:.1} s of audio");
}
