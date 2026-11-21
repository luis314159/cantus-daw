//! Fides through the engine (round 41, ADR 0150): every exciter and resonator renders the
//! same at every block size and twice over, voices on the pool are the serial bits, the
//! defaults' extras change nothing, MPE's pressure presses the bow and its slide moves it,
//! a note dies to exact silence and the unit comes to rest, an eight-note chord keeps its
//! headroom, and a heavy patch's 32 voices render well inside real time.

mod support;
use cantus_core::*;
use cantus_dsp::fides::tel;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::par::Reversed;
use cantus_engine::inst::fides::Unit;
use cantus_engine::inst::{InstrumentUnit, NoteExpression, NoteOn};
use cantus_engine::*;

const SR: f32 = 48_000.0;

fn fides(set: &[(&str, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Resonator);
    for (k, v) in set {
        assert!(s.set(k, *v).is_some(), "no key {k}");
    }
    s
}

fn with_text(mut s: NativeSpec, text: &[(&str, &str)]) -> NativeSpec {
    for (k, v) in text {
        s.set_text(k, Some(v.to_string())).expect("a text key");
    }
    s
}

/// A phrase with overlapping notes, a repeated key and a chord, six beats at 120 BPM.
fn phrase_project(spec: NativeSpec) -> Project {
    let mut p = Project::new("fides");
    let mut t = p.make_track("Fides");
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Phrase", Ticks::ZERO, Ticks::beats(8));
    let q = Ticks::QUARTER;
    for (start, len, pitch, vel) in [
        (0i64, 2i64, 48u8, 100u8),
        (1, 2, 55, 80),
        (2, 1, 60, 127),
        (3, 1, 60, 90), // the same key again: the same resonator struck again
        (4, 4, 64, 110),
        (8, 4, 43, 70),
    ] {
        clip.notes.push(p.make_note(q * start, q * len, pitch, vel));
    }
    for pitch in [52u8, 59, 67] {
        clip.notes.push(p.make_note(q * 12, q * 6, pitch, 100));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn render_block(p: &Project, block: usize, tail: f64) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: tail,
        block_frames: block,
        ..Default::default()
    };
    render(std::sync::Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("render")
        .samples
}

fn shapes() -> Vec<(&'static str, NativeSpec)> {
    vec![
        ("default (a plucked nylon string)", fides(&[])),
        (
            "bowed string with vibrato",
            fides(&[("exc.type", 2.0), ("vib.depth", 20.0), ("body.type", 3.0)]),
        ),
        (
            "mallet on a marimba bar into its tube",
            fides(&[
                ("exc.type", 3.0),
                ("a.type", 2.0),
                ("b.on", 1.0),
                ("couple", 1.0),
                ("body.type", 0.0),
                ("release", 60.0),
            ]),
        ),
        (
            "noise into a tube, cut and tremolo",
            fides(&[
                ("exc.type", 4.0),
                ("a.type", 7.0),
                ("noise.sustain", 0.7),
                ("lowcut", 120.0),
                ("highcut", 6000.0),
                ("trem.depth", 0.5),
            ]),
        ),
        (
            "two voices stolen, plate and bell in parallel",
            fides(&[
                ("poly", 2.0),
                ("exc.type", 1.0),
                ("a.type", 4.0),
                ("b.on", 1.0),
                ("b.type", 9.0),
                ("retrig", 0.0),
            ]),
        ),
        (
            "a macro on the body's size (the unit's own route) and velocity on the decay",
            with_text(
                fides(&[("macro1", 0.8), ("mod1.amt", 0.6), ("mod2.amt", 0.5)]),
                &[
                    ("mod1.src", "macro1"),
                    ("mod1.dst", "body.size"),
                    ("mod2.src", "vel"),
                    ("mod2.dst", "a.decay"),
                ],
            ),
        ),
    ]
}

#[test]
fn every_shape_is_block_size_independent_and_deterministic() {
    for (name, spec) in shapes() {
        let p = phrase_project(spec);
        let reference = render_block(&p, 512, 1.0);
        assert!(reference.iter().all(|x| x.is_finite()), "{name}");
        assert!(reference.iter().any(|x| *x != 0.0), "{name} is silent");
        assert_null(&reference, &render_block(&p, 512, 1.0), 0.0, &format!("{name} twice"));
        for block in [1usize, 37, 64, 4096] {
            assert_null(
                &reference,
                &render_block(&p, block, 1.0),
                0.0,
                &format!("{name} at block size {block}"),
            );
        }
    }
}

#[test]
fn the_extras_at_their_defaults_change_no_sample() {
    let base = render_block(&phrase_project(fides(&[])), 256, 0.5);
    for (name, spec) in [
        (
            "B off, whatever B says",
            fides(&[("b.type", 9.0), ("b.decay", 30.0), ("b.level", 6.0)]),
        ),
        ("cuts at their ends", fides(&[("lowcut", 10.0), ("highcut", 20_000.0)])),
        ("no tremolo at any rate", fides(&[("trem.rate", 9.0)])),
        ("no vibrato at any rate", fides(&[("vib.rate", 9.0), ("vib.delay", 0.0)])),
        ("no strike noise, no noise layer", fides(&[("exc.noise", 0.0), ("noise.level", 0.0)])),
        (
            "a route at depth 0",
            with_text(fides(&[("mod1.amt", 0.0)]), &[("mod1.src", "vel"), ("mod1.dst", "a.decay")]),
        ),
    ] {
        assert_null(&base, &render_block(&phrase_project(spec), 256, 0.5), 0.0, name);
    }
}

fn run(u: &mut Unit, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(128) {
        let n = 128.min(frames - at);
        u.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    (l, r)
}

fn note(key: u8, id: u64) -> NoteOn {
    NoteOn {
        key,
        velocity: 0.8,
        id,
        channel: 1,
    }
}

#[test]
fn mpe_pressure_presses_the_bow_and_slide_moves_it() {
    let spec = fides(&[("exc.type", 2.0), ("mpe.press", 0.8), ("mpe.slide", 0.8)]);
    let mut plain = Unit::new(SR);
    plain.configure(&spec, &[]);
    plain.note_on(note(52, 9));
    let a = run(&mut plain, 24_000);
    let mut pressed = Unit::new(SR);
    pressed.configure(&spec, &[]);
    pressed.note_on(note(52, 9));
    pressed.expression(9, NoteExpression::Pressure, 0.9);
    pressed.expression(9, NoteExpression::Brightness, 0.95);
    let b = run(&mut pressed, 24_000);
    let (tp, tq) = (plain.telemetry(), pressed.telemetry());
    assert!(tq[tel::PRESSURE] > tp[tel::PRESSURE] + 0.3, "{} {}", tq[tel::PRESSURE], tp[tel::PRESSURE]);
    assert!(tq[tel::POS] > tp[tel::POS] + 0.05, "{} {}", tq[tel::POS], tp[tel::POS]);
    assert!(a.0 != b.0, "pressure and slide change the sound");
    // Another note's expression touches nothing of this one.
    let mut other = Unit::new(SR);
    other.configure(&spec, &[]);
    other.note_on(note(52, 9));
    other.expression(10, NoteExpression::Pressure, 0.9);
    assert_eq!(run(&mut other, 24_000), a);
}

#[test]
fn a_note_dies_to_exact_silence_and_the_unit_rests_as_new() {
    for (name, spec) in shapes() {
        let mut u = Unit::new(SR);
        u.configure(&spec, &[]);
        u.note_on(note(60, 1));
        let mut ran = 9_600;
        let _ = run(&mut u, ran);
        u.note_off(60, 1, 0.0);
        let mut rested = false;
        for _ in 0..(40 * 48_000 / 4800) {
            let (l, r) = run(&mut u, 4800);
            ran += 4800;
            if u.at_rest() {
                let (l2, r2) = run(&mut u, 4800);
                ran += 4800;
                assert!(l2.iter().chain(&r2).all(|x| *x == 0.0), "{name}: at rest, not silent");
                let _ = (l, r);
                rested = true;
                break;
            }
        }
        assert!(rested, "{name}: never came to rest");
        // As good as new: a fresh unit skipped as far renders the next note's bits.
        let mut fresh = Unit::new(SR);
        fresh.configure(&spec, &[]);
        fresh.skip(ran);
        u.note_on(note(64, 2));
        fresh.note_on(note(64, 2));
        assert_eq!(run(&mut u, 9_600), run(&mut fresh, 9_600), "{name}");
    }
}

#[test]
fn an_eight_note_chord_at_full_velocity_keeps_its_headroom() {
    for (name, spec) in shapes().into_iter().take(3) {
        let mut p = Project::new("chord");
        let mut t = p.make_track("Fides");
        t.instrument = InstrumentSpec::Native(spec);
        let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(4));
        for pitch in [36u8, 48, 55, 60, 64, 67, 72, 79] {
            clip.notes.push(p.make_note(Ticks::ZERO, Ticks::beats(3), pitch, 127));
        }
        t.clips.push(clip);
        p.tracks.push(t);
        let out = render_block(&p, 512, 1.0);
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak < 1.0, "{name}: {peak}");
        println!("{name}: chord peak {:.1} dBFS", 20.0 * peak.log10());
    }
}

#[test]
fn voices_on_the_pool_are_the_serial_bits() {
    for (name, spec) in shapes() {
        let mut a = Unit::new(SR);
        let mut b = Unit::new(SR);
        a.configure(&spec, &[]);
        b.configure(&spec, &[]);
        for (i, k) in [48u8, 55, 60, 64, 67].iter().enumerate() {
            a.note_on(note(*k, i as u64));
            b.note_on(note(*k, i as u64));
        }
        let frames = 24_000;
        let (mut al, mut ar) = (vec![0.0; frames], vec![0.0; frames]);
        let (mut bl, mut br) = (vec![0.0; frames], vec![0.0; frames]);
        for at in (0..frames).step_by(256) {
            let n = 256.min(frames - at);
            a.process(&mut al[at..at + n], &mut ar[at..at + n]);
            b.process_par(&mut bl[at..at + n], &mut br[at..at + n], &Reversed(4));
        }
        assert_null(&al, &bl, 0.0, name);
        assert_null(&ar, &br, 0.0, name);
    }
}

#[test]
fn thirty_two_heavy_voices_render_well_inside_real_time() {
    // The heaviest shape: a 48-partial plate bowed, a bell in parallel, every voice held.
    let spec = fides(&[
        ("poly", 32.0),
        ("exc.type", 2.0),
        ("a.type", 4.0),
        ("a.modes", 48.0),
        ("b.on", 1.0),
        ("b.type", 9.0),
        ("noise.level", 0.3),
    ]);
    let mut u = Unit::new(SR);
    u.configure(&spec, &[]);
    for k in 0..32u8 {
        u.note_on(note(36 + k, k as u64));
    }
    let seconds = 2.0;
    let t0 = std::time::Instant::now();
    let _ = run(&mut u, (seconds * SR) as usize);
    let took = t0.elapsed().as_secs_f64();
    assert_eq!(u.active_voices(), 32);
    println!("32 bowed plates and bells: {:.1} % of real time", 100.0 * took / seconds);
    // Generous: a debug-optimised build on a shared machine.
    assert!(took < 2.0 * seconds, "{took} s for {seconds} s");
}
