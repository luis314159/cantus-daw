//! Cedo end to end (fx side round, wave B, ADR 0141), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle, Cedo inserted as the picker
//! inserts it (the classic pump: Tempo, quarters, 9 dB — it sounds with no routing), and:
//!
//! - **the tempo pump heard at the beat, to the sample**: a square shape drawn, the pad
//!   silent from each beat's first sample for a quarter of it and untouched after;
//! - **a kick strip as the key ducks a bass strip** (Sidechain), and nothing before it plays;
//! - **another track's notes fire it** (MIDI, that track muted): ducked at each note,
//!   untouched after the shape ends;
//! - **the rest check sees the key**: an insert whose own input is silent while its key
//!   plays keeps its strip awake (Listen at Key: the key comes out);
//! - **band mode ducks only the lows**: the bass under the crossover down, the pad's tone over
//!   it kept;
//! - **linear phase compensated against a dry twin, to the sample**, before and after the
//!   floor is changed (60 Hz Auto, then the fixed 40 Hz floor);
//! - a render from bar 3 equals the whole render there; an edit (the shape) heard and undone
//!   to the bit; live in odd blocks equal to the offline render; and at its defaults, the
//!   dry master to the bit (the key and the notes still routed).
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fxb:cedo

use cantus_core::fx::cedo::Shape;
use cantus_core::fx::lapsus::{Point, SHAPE_HOLD};
use cantus_core::*;
use cantus_engine::{render, EffectUnit, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::Arc;

const SR: usize = 48_000;
/// A beat at 120 BPM, frames.
const BEAT: usize = 24_000;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled() -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc_)
}

/// `beats` beats at 120 BPM from `start`, interleaved stereo.
fn rendered_from(app: &mut CantusApp, start: i64, beats: i64) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        start: Ticks::beats(start),
        end: Some(Ticks::beats(start + beats)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn rendered(app: &mut CantusApp) -> Vec<f32> {
    rendered_from(app, 0, 8)
}

fn left(x: &[f32]) -> Vec<f32> {
    x.iter().step_by(2).copied().collect()
}

/// Mean square of both channels from `a` to `b` seconds.
fn power(x: &[f32], a: f32, b: f32) -> f64 {
    let (i, j) = (
        (a * SR as f32) as usize * 2,
        ((b * SR as f32) as usize * 2).min(x.len()),
    );
    x[i..j]
        .iter()
        .map(|v| f64::from(*v) * f64::from(*v))
        .sum::<f64>()
        / (j - i).max(1) as f64
}

fn db(p: f64) -> f64 {
    10.0 * p.max(1e-30).log10()
}

/// The left channel's power at `hz` (one DFT bin over whole cycles) from `a` to `b` s, dB.
fn tone_db(x: &[f32], hz: f64, a: f64, b: f64) -> f64 {
    let l = left(x);
    let per = SR as f64 / hz;
    let (i, mut j) = ((a * SR as f64) as usize, (b * SR as f64) as usize);
    j = i + (((j - i) as f64 / per).floor() * per) as usize;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (n, v) in l[i..j].iter().enumerate() {
        let w = std::f64::consts::TAU * hz * n as f64 / SR as f64;
        re += f64::from(*v) * w.cos();
        im += f64::from(*v) * w.sin();
    }
    db((re * re + im * im) / ((j - i) as f64).powi(2))
}

fn minus(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

/// A track playing `notes` (pitch, start beat, beats, velocity).
fn line(p: &mut Project, name: &str, notes: &[(u8, f64, f64, u8)]) -> Track {
    let mut track = p.make_track(name);
    let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(16));
    for &(pitch, at, len, vel) in notes {
        let n = p.make_note(
            Ticks::from_beats_f64(at),
            Ticks::from_beats_f64(len),
            pitch,
            vel,
        );
        clip.notes.push(n);
    }
    track.clips.push(clip);
    track
}

/// The song: a pad chord (A4, C5, E5) held six beats and again from beat 8 (bar 3), a bass
/// (A1) held, a kick (C2, short) on every beat from beat 2, a second pad with no notes until
/// beat 4 (the quiet input). Each on a strip of its own, to the master.
fn song() -> Project {
    let mut p = Project::new("Cedo");
    p.tempo_bpm = 120.0;
    let pad = line(
        &mut p,
        "Pad",
        &[
            (69, 0.0, 6.0, 90),
            (72, 0.0, 6.0, 90),
            (76, 0.0, 6.0, 90),
            (69, 8.0, 4.0, 90),
            (76, 8.0, 4.0, 90),
        ],
    );
    p.tracks.push(pad);
    let bass = line(&mut p, "Bass", &[(33, 0.0, 8.0, 110)]);
    p.tracks.push(bass);
    let kicks: Vec<(u8, f64, f64, u8)> = (2..8).map(|b| (36, b as f64, 0.25, 127)).collect();
    let kick = line(&mut p, "Kick", &kicks);
    p.tracks.push(kick);
    let late = line(&mut p, "Late", &[(64, 4.0, 4.0, 90)]);
    p.tracks.push(late);
    p
}

struct Song {
    tracks: Vec<TrackId>,
    strips: Vec<StripId>,
}

fn ids(app: &CantusApp) -> Song {
    let p = app.project();
    Song {
        tracks: p.tracks.iter().map(|t| t.id).collect(),
        strips: p.tracks.iter().map(|t| t.strip.expect("a strip")).collect(),
    }
}

/// Only the tracks `on` sound.
fn solo(app: &mut CantusApp, s: &Song, on: &[usize]) {
    for (i, id) in s.tracks.iter().enumerate() {
        app.exec(Command::SetTrackMute {
            id: *id,
            muted: !on.contains(&i),
        });
    }
}

fn set(app: &mut CantusApp, strip: StripId, effect: EffectId, key: &str, value: f32) {
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: key.into(),
        value,
    });
}

fn shape(app: &mut CantusApp, strip: StripId, effect: EffectId, s: Option<Shape>) {
    app.exec(Command::SetCedoShape {
        strip,
        effect,
        shape: s.map(Box::new),
        pedal: None,
    });
}

/// Full depth over the first quarter of each cycle, nothing after.
fn square() -> Shape {
    Shape {
        points: vec![
            Point::new(0.0, 1.0).shape(SHAPE_HOLD),
            Point::new(0.25, 0.0).shape(SHAPE_HOLD),
        ],
    }
}

/// Insert Cedo on `strip` as the picker does: its starting preset.
fn insert(app: &mut CantusApp, strip: StripId) -> EffectId {
    app.add_effect(strip, EffectKind::Ducker);
    let e = app.project().strip(strip).unwrap().effects[0].clone();
    assert_eq!(
        (e.spec.get("trig"), e.spec.get("depth"), e.spec.get("out")),
        (1.0, 9.0, 1.4),
        "the classic pump: Tempo, 9 dB, its output fitted"
    );
    // Unity out for the measures that follow.
    app.exec(Command::SetEffectParam {
        strip,
        effect: e.id,
        key: "out".into(),
        value: 0.0,
    });
    e.id
}

fn latency_of(app: &CantusApp, strip: StripId) -> usize {
    let slot = app.project().strip(strip).unwrap().effects[0].clone();
    EffectUnit::new(&slot, SR as f32).latency()
}

#[test]
fn cedo_pumps_on_the_beat_ducks_by_a_key_and_by_notes_on_the_boot_path() {
    let (mut app, mut proc_) = assembled();
    app.replace_project(song());
    let s = ids(&app);
    let pad = s.strips[0];
    solo(&mut app, &s, &[0]);
    let pad_dry = rendered(&mut app);
    assert!(power(&pad_dry, 0.5, 3.5) > 1e-5, "the pad sounds");

    // Inserted on the pad: the classic pump sounds with no routing.
    let fx = insert(&mut app, pad);
    let pumped = rendered(&mut app);
    let dip = db(power(&pad_dry, 1.0, 1.1)) - db(power(&pumped, 1.0, 1.1));
    let after = db(power(&pad_dry, 1.4, 1.5)) - db(power(&pumped, 1.4, 1.5));
    eprintln!("the classic pump: {dip:.1} dB on the beat, {after:.2} dB late in it");
    assert!(dip > 5.0, "it pumps on the beat: {dip:.1} dB");
    assert!(after.abs() < 0.01, "and lets go: {after:.3} dB");

    // The tempo pump to the sample: a square shape, silence (80 dB is ∞) through the first
    // quarter of every beat, the pad untouched after.
    set(&mut app, pad, fx, "depth", 80.0);
    set(&mut app, pad, fx, "edge", 0.0);
    shape(&mut app, pad, fx, Some(square()));
    let sq = rendered(&mut app);
    let (l, dry) = (left(&sq), left(&pad_dry));
    for b in 1..6 {
        let at = b * BEAT;
        assert_ne!(l[at - 1], 0.0, "beat {b}: the sample before it sounds");
        assert_eq!(l[at - 1], dry[at - 1], "beat {b}: untouched before it");
        for (i, v) in l.iter().enumerate().take(at + BEAT / 4).skip(at) {
            assert_eq!(*v, 0.0, "beat {b}: sample {i} ducked");
        }
        for i in at + BEAT / 4..at + BEAT {
            assert_eq!(
                l[i].to_bits(),
                dry[i].to_bits(),
                "beat {b}: sample {i} untouched"
            );
        }
    }

    // Live in blocks that divide nothing equals the offline render.
    let play = |proc_: &mut Processor, frames: usize, out: &mut Vec<f32>| {
        let sizes = [256usize, 333, 512, 17, 1024];
        let mut i = 0;
        let mut done = 0;
        while done < frames {
            let n = sizes[i % sizes.len()].min(frames - done);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            proc_.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            done += n;
        }
    };
    app.seek(Ticks::ZERO);
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let mut live = Vec::new();
    play(&mut proc_, 2 * SR, &mut live);
    app.toggle_play();
    let offline = {
        let p = app.project().clone();
        let r = render(
            Arc::new(Snapshot::compile(&p, 1)),
            &RenderOptions {
                end: Some(Ticks::beats(8)),
                tail_seconds: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        r.samples[..live.len()].to_vec()
    };
    assert!(power(&live, 0.0, 1.0) > 1e-6, "it plays live");
    assert!(
        live.iter()
            .zip(&offline)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "live equals the render"
    );

    // An edit heard, undone to the bit: the shape redrawn, then the undo.
    let base = rendered(&mut app);
    shape(
        &mut app,
        pad,
        fx,
        Some(Shape {
            points: vec![Point::new(0.0, 0.0), Point::new(0.5, 1.0)],
        }),
    );
    let moved = rendered(&mut app);
    assert!(
        moved.iter().zip(&base).any(|(a, b)| (a - b).abs() > 1e-3),
        "a new shape is heard"
    );
    app.undo();
    assert_eq!(rendered(&mut app), base, "undone, to the bit");

    // A render from bar 3 hears what the whole render hears there (the pad held through).
    let whole = rendered_from(&mut app, 0, 12);
    let part = rendered_from(&mut app, 8, 4);
    let n = part.len().min(whole.len() - 2 * 8 * BEAT);
    assert!(n > 2 * BEAT, "bar 3 runs");
    assert!(
        part[..n]
            .iter()
            .zip(&whole[2 * 8 * BEAT..2 * 8 * BEAT + n])
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "from bar 3: the whole render's bar 3"
    );

    // Band mode: only the lows. A constant full duck under 150 Hz (minimum phase) on the pad
    // and the bass together: the bass (55 Hz) far down, the pad's A4 (440 Hz) kept.
    shape(
        &mut app,
        pad,
        fx,
        Some(Shape {
            points: vec![Point::new(0.0, 1.0)],
        }),
    );
    set(&mut app, pad, fx, "band", 1.0);
    set(&mut app, pad, fx, "xhz", 150.0);
    let mut p = app.project().clone();
    let mut c = p.tracks[1].clips[0].clone();
    c.id = ClipId(p.alloc_id());
    let pad_track = p.tracks.iter_mut().find(|t| t.strip == Some(pad)).unwrap();
    pad_track.clips.push(c);
    app.replace_project(p);
    let banded = rendered(&mut app);
    let mut q = app.project().clone();
    q.strip_mut(pad).unwrap().effects[0].bypass = true;
    app.replace_project(q);
    let unducked = rendered(&mut app);
    let lows = tone_db(&unducked, 55.0, 1.0, 3.0) - tone_db(&banded, 55.0, 1.0, 3.0);
    let tone = tone_db(&unducked, 440.0, 1.0, 3.0) - tone_db(&banded, 440.0, 1.0, 3.0);
    eprintln!("Lows under 150 Hz: the bass {lows:.1} dB down, the pad's A4 {tone:.2} dB");
    assert!(lows > 20.0, "the lows ducked: {lows:.1} dB");
    assert!(tone.abs() < 0.5, "the pad's tone kept: {tone:.2} dB");

    // The kick strip as the key ducks the bass strip (Sidechain), nothing before it plays.
    app.replace_project(song());
    let s = ids(&app);
    let (bass, kick) = (s.strips[1], s.strips[2]);
    solo(&mut app, &s, &[1, 2]);
    let both = rendered(&mut app);
    solo(&mut app, &s, &[2]);
    let kick_alone = rendered(&mut app);
    let bass_dry = minus(&both, &kick_alone);
    solo(&mut app, &s, &[1, 2]);
    let fx = insert(&mut app, bass);
    for (k, v) in [
        ("trig", 0.0),
        ("depth", 18.0),
        ("thr", -40.0),
        ("att", 1.0),
        ("hold", 60.0),
        ("rel", 100.0),
    ] {
        set(&mut app, bass, fx, k, v);
    }
    app.exec(Command::SetEffectSidechain {
        strip: bass,
        effect: fx,
        source: Some(kick),
    });
    let keyed = minus(&rendered(&mut app), &kick_alone);
    let under = db(power(&bass_dry, 1.0, 1.1)) - db(power(&keyed, 1.0, 1.1));
    let before = db(power(&bass_dry, 0.2, 0.95)) - db(power(&keyed, 0.2, 0.95));
    eprintln!("the bass under the kick: {under:.1} dB, {before:.3} dB before it plays");
    assert!(under > 10.0, "the key ducks the bass: {under:.1} dB");
    assert!(before.abs() < 1e-6, "untouched before the key: {before}");

    // The rest check sees the key: the late pad's strip, silent until beat 4, keyed by the
    // kick and listening to the key — the key comes out of it from beat 2.
    let late = s.strips[3];
    solo(&mut app, &s, &[2, 3]);
    let kick_and_late = rendered(&mut app);
    let fx_late = insert(&mut app, late);
    set(&mut app, late, fx_late, "trig", 0.0);
    set(&mut app, late, fx_late, "listen", 2.0);
    app.exec(Command::SetEffectSidechain {
        strip: late,
        effect: fx_late,
        source: Some(kick),
    });
    let out = minus(&rendered(&mut app), &kick_alone);
    let silent_input = power(&minus(&kick_and_late, &kick_alone), 1.0, 1.9);
    let heard = power(&out, 1.0, 1.9);
    eprintln!("the late strip before its notes: input {silent_input:e}, out {heard:e}");
    assert!(silent_input < 1e-12, "its own input is silent there");
    assert!(heard > 1e-5, "the key keeps it awake: {heard:e}");
    app.exec(Command::RemoveEffect {
        strip: late,
        effect: fx_late,
    });

    // Another track's notes fire it (MIDI): the kick muted, its notes still the trigger; the
    // classic pump on quarters ducks the bass at each note and lets go by 65 % of the beat.
    solo(&mut app, &s, &[1]);
    app.exec(Command::SetEffectSidechain {
        strip: bass,
        effect: fx,
        source: None,
    });
    set(&mut app, bass, fx, "trig", 2.0);
    set(&mut app, bass, fx, "depth", 24.0);
    app.exec(Command::SetEffectMidiSource {
        strip: bass,
        effect: fx,
        source: Some(s.tracks[2]),
    });
    let by_notes = rendered(&mut app);
    for b in [2.0f32, 3.0, 5.0] {
        let t = b * 0.5;
        let hit = db(power(&bass_dry, t, t + 0.05)) - db(power(&by_notes, t, t + 0.05));
        let late =
            db(power(&bass_dry, t + 0.36, t + 0.49)) - db(power(&by_notes, t + 0.36, t + 0.49));
        eprintln!("note at beat {b}: {hit:.1} dB at it, {late:.3} dB late in the beat");
        assert!(hit > 10.0, "beat {b}: ducked by the note ({hit:.1} dB)");
        assert!(late.abs() < 0.01, "beat {b}: let go ({late:.3} dB)");
    }
    let before = db(power(&bass_dry, 0.2, 0.95)) - db(power(&by_notes, 0.2, 0.95));
    assert!(before.abs() < 1e-6, "no note, no duck: {before}");

    // Its neutral defaults: the song as with Cedo bypassed, to the bit (the key and the notes
    // still routed).
    solo(&mut app, &s, &[0, 1, 2, 3]);
    app.exec(Command::SetEffectSidechain {
        strip: bass,
        effect: fx,
        source: Some(kick),
    });
    let defaults: Vec<Command> = EffectKind::Ducker
        .params()
        .iter()
        .map(|i| Command::SetEffectParam {
            strip: bass,
            effect: fx,
            key: i.key.into(),
            value: i.default,
        })
        .collect();
    app.exec(Command::Batch {
        label_key: "cmd.fx_preset".into(),
        commands: defaults,
    });
    shape(&mut app, bass, fx, None);
    let dry_again = {
        let mut p = app.project().clone();
        // Bypassed, not taken out: the key's routing keeps the strips' order (the master's
        // sum is the same sum).
        p.strip_mut(bass).unwrap().effects[0].bypass = true;
        let snap = app.engine.compile_for_render(&p, 1);
        render(
            snap,
            &RenderOptions {
                end: Some(Ticks::beats(8)),
                tail_seconds: 0.0,
                ..Default::default()
            },
        )
        .unwrap()
        .samples
    };
    let null = rendered(&mut app);
    assert_eq!(null.len(), dry_again.len());
    let first = null
        .iter()
        .zip(&dry_again)
        .position(|(a, b)| a.to_bits() != b.to_bits());
    assert_eq!(
        first,
        None,
        "the null, to the bit (first difference at {first:?}: {:?})",
        first.map(|i| (null[i], dry_again[i]))
    );
}

/// Linear phase against a dry twin: two identical pad tracks, Cedo (Lows, linear phase, at
/// depth 0) on one: the mixer delays the twin by Cedo's latency, so the song is the dry
/// song later by exactly that, to the bit — with the Auto floor (60 Hz, 36.8 ms) and after
/// the floor is changed to the fixed 40 Hz (55.3 ms).
#[test]
fn the_linear_phase_is_compensated_against_a_dry_twin_after_a_floor_change() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Twins");
    p.tempo_bpm = 120.0;
    for name in ["Pad A", "Pad B"] {
        let t = line(&mut p, name, &[(57, 0.0, 6.0, 100), (64, 1.0, 5.0, 90)]);
        p.tracks.push(t);
    }
    app.replace_project(p);
    let dry = rendered(&mut app);
    let a = app.project().tracks[0].strip.unwrap();
    let fx = insert(&mut app, a);
    set(&mut app, a, fx, "depth", 0.0);
    set(&mut app, a, fx, "band", 1.0);
    set(&mut app, a, fx, "xover", 1.0);
    for (floor, want) in [(0.0, 1_765usize), (1.0, 2_655)] {
        set(&mut app, a, fx, "lfloor", floor);
        assert_eq!(latency_of(&app, a), want, "floor {floor}");
        let out = rendered(&mut app);
        let shift = 2 * want;
        assert!(out[..shift].iter().all(|v| *v == 0.0), "nothing before");
        assert!(
            out[shift..]
                .iter()
                .zip(&dry)
                .all(|(x, y)| x.to_bits() == y.to_bits()),
            "floor {floor}: the twins aligned, the song {want} samples late to the bit"
        );
    }
}

/// The presets are the file's, in both languages, and cover every trigger and band.
#[test]
fn the_presets_name_real_keys_in_both_languages_and_every_trigger() {
    let file: Vec<_> = cantus_ui::preset_data::presets_of(
        cantus_ui::preset_data::FileUnit::Effect(EffectKind::Ducker),
    )
    .collect();
    assert!(file.len() >= 12, "twelve presets or more: {}", file.len());
    let mut trig = [false; 3];
    let mut band = [false; 3];
    let mut shaped = 0;
    for p in &file {
        let mut spec = EffectSpec::new(EffectKind::Ducker);
        for (k, v) in &p.values {
            if cantus_core::fx::cedo::flat_key(k).is_some() {
                continue;
            }
            assert!(spec.set(k, *v).is_some(), "{}: no parameter {k}", p.key);
        }
        if cantus_core::fx::cedo::shape_of_flat(&p.values).is_some() {
            shaped += 1;
        }
        trig[spec.get("trig") as usize] = true;
        band[spec.get("band") as usize] = true;
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(&p.key), p.key, "{} in {lang:?}", p.key);
        }
    }
    assert_eq!(trig, [true; 3], "every trigger");
    assert_eq!(band, [true; 3], "every band");
    assert!(shaped >= 4, "presets draw shapes of their own: {shaped}");
}
