//! Kits from outside (round 37, ADR 0100 §7): an SFZ drum kit published by others (CC0,
//! `tests/fixtures/sfz/virtuosity_drums`, byte for byte) becomes a mapped kit; a folder of
//! one-shots becomes a mapped kit in one undo step, through the app, and plays through the
//! real engine.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn boot() -> CantusApp {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut p = Project::new("Kits");
    p.tempo_bpm = 120.0;
    CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false)
}

#[test]
fn an_sfz_drum_kit_from_outside_becomes_a_mapped_kit() {
    let sfz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sfz/virtuosity_drums/Programs/01-basic-kit.sfz");
    let zones = cantus_ui::padout_ui::sfz_zones(&sfz).unwrap();
    assert!(zones.len() > 50, "the kit's regions: {}", zones.len());
    let (kit, log) = cantus_core::pads::import::kit_from_zones(&zones);
    eprintln!("virtuosity basic kit: {} pads, log {log:?}", kit.pads.len());
    let pad = |k: u8| kit.pads.iter().find(|p| p.pitch == k);
    let kick = pad(36).expect("a kick on 36");
    assert!(
        kick.layers.len() >= 4,
        "velocity layers and takes: {}",
        kick.layers.len()
    );
    assert_eq!(
        kick.round_robin,
        RoundRobin::Cycle,
        "its takes cycle (seq_length)"
    );
    let mut zones_v: Vec<(u8, u8)> = kick.layers.iter().map(|l| (l.vel_lo, l.vel_hi)).collect();
    zones_v.dedup();
    assert!(
        zones_v.len() >= 2,
        "more than one velocity zone: {zones_v:?}"
    );
    assert!(pad(38).is_some(), "a snare on 38");
    // Paths made absolute from the program's folder: the files the kit would read.
    assert!(kick
        .layers
        .iter()
        .all(|l| std::path::Path::new(&l.sample).is_absolute()));
    // A kit of several microphones: one position kept for each key, every velocity zone
    // still there.
    let dirs: Vec<&str> = kick
        .layers
        .iter()
        .map(|l| l.sample.rsplit_once(['/', '\\']).map_or("", |(d, _)| d))
        .collect();
    assert!(
        dirs.windows(2).all(|w| w[0] == w[1]),
        "one microphone: {dirs:?}"
    );
    assert!(
        log.iter().any(|l| l.contains("microphone positions")),
        "said: {log:?}"
    );
    assert!(kit.pads.len() >= 8);
    // The file's exclusive groups: the hats choke each other (the pedal cuts the closed hat,
    // the closed and pedal cut the open one), nothing else chokes the kick or the snare.
    let (c, p, o) = (
        pad(42).unwrap().choke,
        pad(44).unwrap().choke,
        pad(46).unwrap().choke,
    );
    assert!(
        c != 0 && c == p && p == o,
        "the hats choke together: {c} {p} {o}"
    );
    assert_eq!(kick.choke, 0, "the kick chokes nothing");
    assert_eq!(pad(38).unwrap().choke, 0, "the snare chokes nothing");
}

/// A short tone, our own test file (ADR 0100 §7's folder test needs files on disk).
fn tone(path: &std::path::Path, hz: f32, len: usize) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for n in 0..len {
        let env = 1.0 - n as f32 / len as f32;
        let x = (std::f32::consts::TAU * hz * n as f32 / 48_000.0).sin() * 0.5 * env;
        w.write_sample(x).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn a_folder_becomes_a_mapped_kit_in_one_drop_and_plays() {
    let dir = tempfile::tempdir().unwrap();
    let names = [
        ("Kick 1.wav", 60.0),
        ("Kick 2.wav", 62.0),
        ("Snare soft.wav", 200.0),
        ("Snare hard.wav", 210.0),
        ("HH Closed.wav", 3000.0),
        ("charles abierto.wav", 3200.0),
        ("Texture.wav", 500.0),
    ];
    for (n, hz) in names {
        tone(&dir.path().join(n), hz, 4_800);
    }
    let mut app = boot();
    let before = app.project().tracks.len();
    let t = app
        .make_kit_from_folder(None, dir.path())
        .expect("a kit track");
    assert_eq!(app.project().tracks.len(), before + 1);
    let kit = app
        .project()
        .track(t)
        .unwrap()
        .instrument
        .pulsus()
        .unwrap()
        .clone();
    let pad = |k: u8| kit.pads.iter().find(|p| p.pitch == k).unwrap();
    assert_eq!(pad(36).layers.len(), 2);
    assert_eq!(pad(36).round_robin, RoundRobin::Cycle);
    assert_eq!(pad(38).layers.len(), 2, "two dynamics");
    assert!(pad(38).layers[0].sample.ends_with("Snare soft.wav"));
    assert_eq!((pad(42).choke, pad(46).choke), (1, 1));
    assert_eq!(pad(60).name, "Texture");
    assert!(app.status.text.contains("5 pads"), "{}", app.status.text);
    // One undo step.
    assert!(app.bus.can_undo());
    app.bus.undo();
    assert_eq!(app.project().tracks.len(), before);
    app.bus.redo();
    // It plays: a hit on each named pad sounds, through the engine with its samples loaded.
    let mut p = app.project().clone();
    let i = p.track_index(t).unwrap();
    let mut c = p.make_clip("hits", Ticks::ZERO, Ticks::beats(4));
    for (b, k) in [36u8, 38, 42, 46].iter().enumerate() {
        c.notes
            .push(p.make_note(Ticks::beats(b as i64), Ticks::QUARTER, *k, 120));
    }
    p.tracks[i].clips.push(c);
    app.sync_engine();
    let snap = app.engine.compile_for_render(&p, 1);
    let out = render(
        snap,
        &RenderOptions {
            tail_seconds: 0.2,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    let beat = 24_000usize; // 120 BPM at 48 kHz
    for b in 0..4 {
        let at = b * beat * 2;
        let peak = out[at..at + 4_000 * 2]
            .iter()
            .fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.05, "hit {b} sounds: {peak}");
    }
    // Onto an existing kit (a drop on its pads): the same kit, the same track.
    let again = app.make_kit_from_folder(Some(t), dir.path());
    assert_eq!(again, Some(t));
}

/// A folder dropped on the window from the system (`interop`'s drop → `padout_ui::drop_kit`),
/// read on its own thread and landed by the frame's poll: a kit track, one undo step
/// (review 2 #8). A folder that names no drum is refused when it lands on no kit; one too
/// big to be a kit is refused; the notice counts the log's notes, not files (#9, #11).
#[test]
fn a_dropped_folder_lands_as_a_kit_and_a_folder_that_is_no_kit_is_refused() {
    let mut app = boot();
    let ctx = egui::Context::default();
    let before = app.project().tracks.len();
    // Drums, with more takes of one than a pad holds: the log has a note.
    let dir = tempfile::tempdir().unwrap();
    for n in 1..=20 {
        tone(&dir.path().join(format!("Kick {n}.wav")), 60.0, 480);
    }
    tone(&dir.path().join("Snare.wav"), 200.0, 480);
    assert!(cantus_ui::padout_ui::drop_kit(
        &mut app,
        &ctx,
        dir.path(),
        None
    ));
    assert!(cantus_ui::padout_ui::poll_kit(&mut app, true), "it lands");
    assert_eq!(app.project().tracks.len(), before + 1);
    let t = app.project().tracks.last().unwrap();
    assert!(t.instrument.pulsus().is_some());
    let status = app.status.text.clone();
    assert!(
        status.contains("notes on what did not fit") && !status.contains("files"),
        "{status}"
    );
    app.bus.undo();
    assert_eq!(app.project().tracks.len(), before, "one undo step");
    // No drum named: refused (dropped on no kit's pads), nothing made.
    let plain = tempfile::tempdir().unwrap();
    tone(&plain.path().join("Texture.wav"), 500.0, 480);
    tone(&plain.path().join("Ambience.wav"), 300.0, 480);
    assert!(cantus_ui::padout_ui::drop_kit(
        &mut app,
        &ctx,
        plain.path(),
        None
    ));
    assert!(!cantus_ui::padout_ui::poll_kit(&mut app, true));
    assert_eq!(app.project().tracks.len(), before);
    assert!(
        app.status.text.contains("named like a drum"),
        "{}",
        app.status.text
    );
    // Too big to be a kit: refused.
    let big = tempfile::tempdir().unwrap();
    for n in 0..=cantus_ui::padout_ui::KIT_MAX_FILES {
        std::fs::write(big.path().join(format!("Kick {n}.wav")), b"").unwrap();
    }
    assert_eq!(
        cantus_ui::padout_ui::kit_files_within(big.path(), &|_| true),
        cantus_ui::padout_ui::Scan::TooBig
    );
    assert!(cantus_ui::padout_ui::drop_kit(
        &mut app,
        &ctx,
        big.path(),
        None
    ));
    assert!(!cantus_ui::padout_ui::poll_kit(&mut app, true));
    assert_eq!(app.project().tracks.len(), before);
    assert!(app.status.text.contains("too big"), "{}", app.status.text);
    // A file that is neither a folder nor an SFZ is not a kit's drop at all.
    assert!(!cantus_ui::padout_ui::drop_kit(
        &mut app,
        &ctx,
        &dir.path().join("Snare.wav"),
        None
    ));
}
