//! Chorda's second model end to end (round 37, ADR 0098), on the boot path: a Chorda track
//! added the way the instrument picker adds one plays model 2 through the real engine (the
//! live processor the app drives, which equals the offline render to the bit); an old song
//! opened through the app plays model 1, to the bits the round-36 code rendered it to.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

#[path = "chorda_e2e/old_song.rs"]
mod old_song;

use cantus_core::*;
use cantus_dsp::chorda::design::Tuning;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use std::sync::Arc;

/// The round-36 code's hash of [`old_song::chorda_only`] of the v53 fixture (computed on
/// the contract commit, c95ee8a9, before model 2 existed).
const OLD_SONG: u64 = 0x0005_22d0_c2e4_add0;

fn data() -> &'static std::path::Path {
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

fn chorda_of(app: &CantusApp, track: TrackId) -> NativeSpec {
    app.project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .filter(|n| n.kind == InstrumentKind::Chorda)
        .cloned()
        .expect("a Chorda")
}

fn offline(p: &Project, frames: usize) -> Vec<f32> {
    let r = render(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .unwrap();
    r.samples[..frames * 2].to_vec()
}

#[test]
fn a_new_chorda_plays_model_2_live_and_an_old_song_plays_model_1_to_its_bits() {
    // Nothing is designed on the audio thread: the boot path asked for the reference
    // keyboard; wait for it as a user waits the second it takes.
    let (mut app, mut proc_) = assembled();
    cantus_engine::inst::chorda::ensure(Tuning::default(), 48_000.0);

    // A Chorda track, the way the picker adds one: model 2.
    let before = app.project().tracks.len();
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    assert_eq!(app.project().tracks.len(), before + 1, "one track more");
    let track = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the new Chorda track");
    let spec = chorda_of(&app, track);
    assert!(spec.is_stored("model"));
    assert_eq!(spec.get("model"), 2.0, "a new Chorda is model 2");

    // A clip of notes on it, one undo step.
    let mut clip = app.bus.new_clip("Phrase", Ticks::ZERO, Ticks::beats(4));
    for (b, k, v) in [(0i64, 48u8, 90u8), (0, 55, 80), (1, 64, 100), (2, 72, 70)] {
        clip.notes
            .push(app.bus.new_note(Ticks::beats(b), Ticks::beats(2), k, v));
    }
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    }));
    // The start-up project's other tracks are taken away (one undo step each, as a user
    // deletes them): the master is the piano alone.
    let others: Vec<TrackId> = app
        .project()
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|t| *t != track)
        .collect();
    for id in others {
        assert!(app.exec(Command::RemoveTrack { id }));
    }
    assert_eq!(app.project().tracks.len(), 1);

    // Live: the app sends the unit and the snapshot to the engine it drives; play.
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let frames = 48_000;
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
    let peak = live.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01, "the piano sounds live: peak {peak}");

    // What it played is model 2: the offline render of the project equals it to the bit,
    // and the same project at model 1 does not.
    let p = app.project().clone();
    let two = offline(&p, frames);
    assert!(two == live, "live equals the offline render of model 2");
    let mut one = p.clone();
    if let Some(InstrumentSpec::Native(n)) = one.track_mut(track).map(|t| &mut t.instrument) {
        n.set("model", 1.0);
    }
    assert!(offline(&one, frames) != live, "and not model 1");

    // An old song, opened through the app: its Chorda is model 1, said explicitly, and it
    // renders to the bits round 36 gave it.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cantus-project/fixtures/v53.cantus");
    let copy = data().join("old-song.cantus");
    std::fs::copy(&fixture, &copy).unwrap();
    app.open(&copy);
    let old = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the old song's Chorda");
    let spec = chorda_of(&app, old);
    assert!(spec.is_stored("model"));
    assert_eq!(spec.get("model"), 1.0, "an old song keeps model 1");
    let h = old_song::hash_render(&old_song::chorda_only(app.project().clone()));
    eprintln!("old song hash: {h:#018x}");
    assert_eq!(h, OLD_SONG, "the old song renders as round 36 rendered it");
}
