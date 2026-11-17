//! Fractus end to end (fx side round, wave A, ADR 0137), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle; a riff on its track and a
//! keyboard whose track is muted (its notes still sent, ADR 0022). Fractus is inserted on the
//! riff's strip as the picker inserts it — on its starting stutter: the last beat of each bar
//! repeats a thirty-second, **heard on its step in time with the song** (the bar's first
//! three beats the dry riff to the bit, the repeats from beat 3 and a thirty-second). **What
//! plays is the offline render**: the engine the app drives, played block by block in sizes
//! that do not divide the bar, equals the render to the bit. **A lane painted** as the
//! pointer paints it (`Command::SetFractusLane`) heard on its steps; **notes from another
//! track** play a lane while held; an edit heard and undone to the bit; and at its neutral
//! defaults, nothing painted, the dry master to the bit. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder. // fxa:fractus

use cantus_core::fx::fractus::Block;
use cantus_core::*;
use cantus_engine::{render, EffectUnit, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::Arc;

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

/// One bar at 120 BPM: 96 000 frames.
const BAR: usize = 96_000;
/// A beat's frames at 120 BPM.
const BEAT: usize = 24_000;

fn rendered(app: &mut CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        end: Some(Ticks::beats(4)),
        tail_seconds: 0.5,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

/// The first frame where two stereo renders differ.
fn first_diff(a: &[f32], b: &[f32]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
        .map(|i| i / 2)
}

/// The largest difference between two renders.
fn diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0f32, |m, (x, y)| m.max((x - y).abs()))
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs()))
}

/// A track playing `notes` (pitch, start in eighths, eighths).
fn line(p: &mut Project, name: &str, notes: &[(u8, i64, i64)]) -> Track {
    let mut track = p.make_track(name);
    let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(4));
    let eighth = |n: i64| Ticks::from_beats_f64(n as f64 * 0.5);
    for &(pitch, at, len) in notes {
        let n = p.make_note(eighth(at), eighth(len), pitch, 100);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    track
}

#[test]
fn fractus_plays_its_lanes_in_time_with_the_song_live_as_offline_and_undoes() {
    let (mut app, mut proc_) = assembled();
    let mut p = Project::new("Fractus");
    p.tempo_bpm = 120.0;
    // A riff of eighths that moves every step, so a repeat is never the riff itself.
    let riff: Vec<(u8, i64, i64)> = (0..8)
        .map(|i| ([57u8, 60, 64, 67, 69, 67, 64, 60][i as usize], i, 1))
        .collect();
    // Each track added before the next is made: each takes the next free strip.
    let riff = line(&mut p, "Riff", &riff);
    p.tracks.push(riff);
    let mut keys = line(&mut p, "Keys", &[(55, 2, 2)]);
    keys.muted = true;
    p.tracks.push(keys);
    app.replace_project(p);
    let keys = app.project().tracks[1].id;
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(peak(&dry) > 0.01, "the riff sounds");

    // Inserted as the picker inserts it: the starting stutter, no latency.
    app.add_effect(strip, EffectKind::Glitch);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let slot = app.project().strip(strip).unwrap().effects[0].clone();
    assert_eq!(
        slot.spec.fractus.as_deref().map(|p| p.lane(0).to_vec()),
        Some(vec![Block::new(12, 4)]),
        "the last beat's repeat painted"
    );
    assert_eq!(EffectUnit::new(&slot, 48_000.0).latency(), 0);
    let stutter = rendered(&mut app);
    // Beat 3's first thirty-second plays live; the first repeat is the next thirty-second.
    let at = first_diff(&dry, &stutter).expect("the stutter is heard");
    eprintln!(
        "the stutter's first changed frame: {at} (beat 3 + 1/32 = {})",
        3 * BEAT + BEAT / 8
    );
    assert_eq!(
        at,
        3 * BEAT + BEAT / 8,
        "on its step, in time with the song"
    );
    assert!(diff(&dry, &stutter) > 0.01);

    // What plays is the offline render: the engine the app drives, played from the top in
    // blocks that do not divide the bar.
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let frames = BAR;
    let mut live = Vec::with_capacity(frames * 2);
    let sizes = [256usize, 333, 512, 17, 1024];
    let mut i = 0;
    while live.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - live.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc_.process(&mut buf, 2);
        live.extend_from_slice(&buf);
    }
    app.toggle_play();
    let offline = {
        let p = app.project().clone();
        let r = render(
            Arc::new(Snapshot::compile(&p, 1)),
            &RenderOptions {
                tail_seconds: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        r.samples[..frames * 2].to_vec()
    };
    assert!(peak(&live) > 0.01, "it plays live");
    assert_eq!(first_diff(&live, &offline), None, "live equals the render");

    // A lane painted as the pointer paints it: the gate over beat 2, in thirty-seconds.
    app.exec(Command::SetFractusLane {
        strip,
        effect,
        lane: 7,
        blocks: vec![Block::new(4, 4)],
    });
    let gated = rendered(&mut app);
    let at = first_diff(&stutter, &gated).expect("the gate is heard");
    eprintln!("the gate's first changed frame: {at} (beat 1 = {BEAT})");
    assert!(
        (BEAT..BEAT + BEAT / 8).contains(&at),
        "the gate on beat 2's step: {at}"
    );
    assert_eq!(
        first_diff(&stutter[2 * (2 * BEAT)..], &gated[2 * (2 * BEAT)..]),
        None,
        "and gone by beat 3"
    );
    app.undo();
    assert_eq!(rendered(&mut app), stutter, "the lane undone, to the bit");

    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        })
    };
    // Notes from another track play the lanes: from C3 (48), G3 (55) is the gate's key.
    // Played by notes alone the steps rest: with no notes, the riff dry.
    set(&mut app, "trig", 1.0);
    set(&mut app, "quant", 0.0);
    let no_notes = rendered(&mut app);
    assert_eq!(first_diff(&no_notes, &dry), None, "no notes: nothing plays");
    app.exec(Command::SetEffectMidiSource {
        strip,
        effect,
        source: Some(keys),
    });
    let by_notes = rendered(&mut app);
    let at = first_diff(&no_notes, &by_notes).expect("the held note plays the gate");
    eprintln!("the note's gate's first changed frame: {at} (the note at {BEAT})");
    assert!(
        (BEAT..BEAT + BEAT / 8).contains(&at),
        "from the note's sample: {at}"
    );
    assert!(diff(&no_notes, &by_notes) > 0.01);

    // An edit heard, undone to the bit.
    set(&mut app, "trig", 0.0);
    set(&mut app, "rpitch", 5.0);
    let moved = rendered(&mut app);
    assert!(diff(&stutter, &moved) > 0.001, "the repeats a fourth up");
    app.undo();
    assert_eq!(rendered(&mut app), stutter, "undone, to the bit");

    // Its neutral defaults, nothing painted: the dry master, to the bit (the notes still
    // routed).
    let mut defaults: Vec<Command> = EffectKind::Glitch
        .params()
        .iter()
        .map(|i| Command::SetEffectParam {
            strip,
            effect,
            key: i.key.into(),
            value: i.default,
        })
        .collect();
    defaults.push(Command::SetFractusLane {
        strip,
        effect,
        lane: 0,
        blocks: Vec::new(),
    });
    app.exec(Command::Batch {
        label_key: "cmd.fx_preset".into(),
        commands: defaults,
    });
    assert!(app
        .project()
        .effect(strip, effect)
        .unwrap()
        .spec
        .fractus
        .is_none());
    assert_eq!(rendered(&mut app), dry, "the null, to the bit");
}

/// The starting stutter is a factory preset: the picker's insert names it in the editor, and
/// loading that preset (its lanes in its values) gives the same spec.
#[test]
fn the_starting_stutter_is_its_factory_preset() {
    use cantus_ui::views::fx::frame::{read_presets, recognise};
    let start = cantus_core::starting::starting_spec(EffectKind::Glitch);
    let list = read_presets(EffectKind::Glitch, false);
    let named = recognise(&start, &list).expect("named");
    assert_eq!(
        named.name,
        cantus_ui::i18n::t("preset.fractus.last_beat_stutter")
    );
    let entry = list.iter().find(|e| e.name == named.name).unwrap();
    assert_eq!(
        cantus_core::fx::fractus::pattern_of_flat(&entry.params).map(Box::new),
        start.fractus
    );
    for info in EffectKind::Glitch.params() {
        let want = entry.params.get(info.key).copied().unwrap_or(info.default);
        assert_eq!(start.get(info.key), want, "{}", info.key);
    }
    // At its defaults (nothing painted), no preset is named.
    assert!(recognise(&EffectSpec::new(EffectKind::Glitch), &list).is_none());
}
