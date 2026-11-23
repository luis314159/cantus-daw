//! Arcus through the engine (round 42, ADR 0157): every shape renders the same at every
//! block size and twice over, voices on the pool are the serial bits, the extras at their
//! defaults and the tuning at 12-TET change nothing, a CC1 swell and MPE's pressure and
//! slide move the bow, a note's own articulation and bow reach it, a note dies to exact
//! silence and the unit rests as new, a chord keeps its headroom.

mod support;
use cantus_core::note_art::{Articulation, Bow, NoteArt};
use cantus_core::*;
use cantus_dsp::arcus::tel;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::par::Reversed;
use cantus_engine::inst::arcus::Unit;
use cantus_engine::inst::{Controller, InstrumentUnit, NoteExpression, NoteOn};
use cantus_engine::*;

const SR: f32 = 48_000.0;

fn arcus(set: &[(&str, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::BowedString);
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

/// A line with a slur (overlapping notes), a re-bow, a repeated key and a double stop, at
/// 120 BPM.
fn phrase_project(spec: NativeSpec) -> Project {
    let mut p = Project::new("arcus");
    let mut t = p.make_track("Arcus");
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Phrase", Ticks::ZERO, Ticks::beats(8));
    let q = Ticks::QUARTER;
    for (start, len, pitch, vel) in [
        (0i64, 2i64, 50u8, 90u8),
        (2, 1, 53, 70), // overlaps nothing: a re-bow on the ringing string
        (3, 2, 55, 110),
        (4, 2, 57, 80), // overlaps the last: a slur
        (6, 1, 57, 60), // the same key again
        (8, 3, 45, 100),
    ] {
        clip.notes.push(p.make_note(q * start, q * len, pitch, vel));
    }
    for pitch in [43u8, 50] {
        clip.notes.push(p.make_note(q * 12, q * 3, pitch, 100));
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
        ("default (a cello, legato)", arcus(&[])),
        ("a double bass, re-bowed", arcus(&[("inst", 1.0), ("mode", 1.0)])),
        ("pizzicato, poly", arcus(&[("tech", 1.0), ("mode", 0.0)])),
        ("tremolo, thermal", arcus(&[("tech", 2.0), ("bow.friction", 1.0)])),
        (
            "portamento, no vibrato, cut",
            arcus(&[
                ("porta", 120.0),
                ("vib.depth", 0.0),
                ("lowcut", 80.0),
                ("highcut", 6000.0),
            ]),
        ),
        (
            "a macro on the body's size and the contact",
            with_text(
                arcus(&[("macro1", 0.8), ("mod1.amt", 0.6), ("mod2.amt", -0.5)]),
                &[
                    ("mod1.src", "macro1"),
                    ("mod1.dst", "body.size"),
                    ("mod2.src", "macro1"),
                    ("mod2.dst", "bow.pos"),
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
        assert_null(
            &reference,
            &render_block(&p, 512, 1.0),
            0.0,
            &format!("{name} twice"),
        );
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
    let base = render_block(&phrase_project(arcus(&[])), 256, 0.5);
    for (name, spec) in [
        (
            "cuts at their ends",
            arcus(&[("lowcut", 10.0), ("highcut", 20_000.0)]),
        ),
        ("no portamento", arcus(&[("porta", 0.0)])),
        (
            "a route at depth 0",
            with_text(
                arcus(&[("mod1.amt", 0.0)]),
                &[("mod1.src", "mw"), ("mod1.dst", "body.size")],
            ),
        ),
    ] {
        assert_null(
            &base,
            &render_block(&phrase_project(spec), 256, 0.5),
            0.0,
            name,
        );
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
        velocity: 0.7,
        id,
        channel: 1,
    }
}

#[test]
fn the_tuning_at_12_tet_is_the_same_bits_and_a_table_moves_the_pitch() {
    let spec = arcus(&[]);
    let mut plain = Unit::new(SR);
    plain.configure(&spec, &[]);
    plain.note_on(note(50, 1));
    let a = run(&mut plain, 24_000);
    let mut equal = Unit::new(SR);
    equal.configure(&spec, &[]);
    equal.set_tuning(&cantus_core::tuning::TuningTable::EQUAL);
    equal.note_on(note(50, 1));
    assert_eq!(run(&mut equal, 24_000), a, "12-TET at A440 adds nothing");
    let mut t = [0.0f32; 128];
    t[50] = 0.31;
    let mut tuned = Unit::new(SR);
    tuned.configure(&spec, &[]);
    tuned.set_tuning(&cantus_core::tuning::TuningTable(t));
    tuned.note_on(note(50, 1));
    let _ = run(&mut tuned, 24_000);
    let want = 440.0 * cantus_dsp::math::exp2((50.31 - 69.0) / 12.0);
    let got = tuned.telemetry()[tel::F0];
    assert!((got as f64 / want - 1.0).abs() < 0.01, "{got} {want}");
}

#[test]
fn cc1_mpe_pressure_and_slide_move_the_bow() {
    let spec = arcus(&[("mode", 0.0)]);
    let mut plain = Unit::new(SR);
    plain.configure(&spec, &[]);
    plain.note_on(note(50, 9));
    let a = run(&mut plain, 24_000);
    // CC1 up: a louder dynamic, a faster bow.
    let mut swelled = Unit::new(SR);
    swelled.configure(&spec, &[]);
    swelled.controller(Controller::ModWheel, 1.0);
    swelled.note_on(note(50, 9));
    let _ = run(&mut swelled, 24_000);
    let (tp, ts) = (plain.telemetry(), swelled.telemetry());
    assert!(ts[tel::DYNAMIC] > tp[tel::DYNAMIC] + 0.2, "{} {}", ts[tel::DYNAMIC], tp[tel::DYNAMIC]);
    assert!(ts[tel::SPEED].abs() > tp[tel::SPEED].abs() * 1.5);
    // MPE pressure on the dynamic, slide on the contact point.
    let mut pressed = Unit::new(SR);
    pressed.configure(&spec, &[]);
    pressed.note_on(note(50, 9));
    pressed.expression(9, NoteExpression::Pressure, 0.95);
    pressed.expression(9, NoteExpression::Brightness, 0.95);
    let b = run(&mut pressed, 24_000);
    let tq = pressed.telemetry();
    assert!(tq[tel::DYNAMIC] > tp[tel::DYNAMIC] + 0.2);
    assert!(tq[tel::BETA] < tp[tel::BETA] * 0.9, "{} {}", tq[tel::BETA], tp[tel::BETA]);
    assert!(a.0 != b.0);
    // Another note's expression touches nothing of this one.
    let mut other = Unit::new(SR);
    other.configure(&spec, &[]);
    other.note_on(note(50, 9));
    other.expression(10, NoteExpression::Pressure, 0.9);
    assert_eq!(run(&mut other, 24_000), a);
}

#[test]
fn a_note_s_own_articulation_and_bow_reach_it() {
    let spec = arcus(&[]);
    let plain = {
        let mut u = Unit::new(SR);
        u.configure(&spec, &[]);
        u.note_on(note(50, 4));
        run(&mut u, 24_000)
    };
    for (art, bow) in [
        (Articulation::Pizzicato, None),
        (Articulation::Staccato, None),
        (Articulation::Spiccato, None),
        (Articulation::Tremolo, None),
        (Articulation::Sustain, Some(Bow::Up)),
    ] {
        let mut u = Unit::new(SR);
        u.configure(&spec, &[]);
        u.note_art(4, NoteArt { articulation: art, bow });
        u.note_on(note(50, 4));
        let x = run(&mut u, 24_000);
        assert!(x != plain, "{art:?} {bow:?} changed nothing");
        // An art named for another note is not this one's.
        let mut v = Unit::new(SR);
        v.configure(&spec, &[]);
        v.note_art(5, NoteArt { articulation: art, bow });
        v.note_on(note(50, 4));
        assert_eq!(run(&mut v, 24_000), plain, "{art:?}: another note's art");
    }
}

#[test]
fn a_note_dies_to_exact_silence_and_the_unit_rests_as_new() {
    for (name, spec) in shapes() {
        let mut u = Unit::new(SR);
        u.configure(&spec, &[]);
        u.note_on(note(52, 1));
        let mut ran = 9_600;
        let _ = run(&mut u, ran);
        u.note_off(52, 1, 0.0);
        let mut rested = false;
        for _ in 0..(40 * 48_000 / 4800) {
            let _ = run(&mut u, 4800);
            ran += 4800;
            if u.at_rest() {
                let (l2, r2) = run(&mut u, 4800);
                ran += 4800;
                assert!(
                    l2.iter().chain(&r2).all(|x| *x == 0.0),
                    "{name}: at rest, not silent"
                );
                rested = true;
                break;
            }
        }
        assert!(rested, "{name}: never came to rest");
        let mut fresh = Unit::new(SR);
        fresh.configure(&spec, &[]);
        fresh.skip(ran);
        u.note_on(note(57, 2));
        fresh.note_on(note(57, 2));
        assert_eq!(run(&mut u, 9_600), run(&mut fresh, 9_600), "{name}");
    }
}

#[test]
fn a_chord_at_full_velocity_keeps_its_headroom() {
    for (name, spec) in [
        ("cello, poly", arcus(&[("mode", 0.0)])),
        ("bass pizzicato, poly", arcus(&[("inst", 1.0), ("tech", 1.0), ("mode", 0.0)])),
    ] {
        let mut p = Project::new("chord");
        let mut t = p.make_track("Arcus");
        t.instrument = InstrumentSpec::Native(spec);
        let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(4));
        for pitch in [36u8, 43, 50, 57] {
            clip.notes
                .push(p.make_note(Ticks::ZERO, Ticks::beats(3), pitch, 127));
        }
        t.clips.push(clip);
        p.tracks.push(t);
        let out = render_block(&p, 512, 1.0);
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak < 1.0, "{name}: {peak}");
        println!(
            "{name}: chord peak {:.1} dBFS",
            20.0 * cantus_dsp::math::log10(peak)
        );
    }
}

#[test]
fn voices_on_the_pool_are_the_serial_bits() {
    for (name, spec) in shapes() {
        let spec = {
            let mut s = spec;
            s.set("mode", 0.0);
            s
        };
        let mut a = Unit::new(SR);
        let mut b = Unit::new(SR);
        a.configure(&spec, &[]);
        b.configure(&spec, &[]);
        for (i, k) in [43u8, 50, 57, 62].iter().enumerate() {
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
