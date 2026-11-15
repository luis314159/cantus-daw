//! The Slate cast end to end (round 40, ADR 0122), on the boot path: in the app
//! `CantusApp::assemble` puts together, a dancer added the way the instrument picker adds
//! one is Compás; each character of the Slate cast chosen the way the picker chooses it
//! (one undo step) dances to the transport — the engine's feed posing it, the stage over
//! the workspace drawing it, a later position a different picture — and a song saved with
//! it and opened again shows it. The dancer's mood field speaks the character's own words.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::dancer::{CHARACTER_KEY, DEFAULT_CHARACTER};
use cantus_core::*;
use cantus_dance::Pose;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::dance_live::DanceLive;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::stage::Surface;
use cantus_ui::CantusApp;

/// The Slate cast, all twelve, in the picker's order.
const SLATE: &[&str] = &[
    "builtin:compas",
    "builtin:tono",
    "builtin:sincopa",
    "builtin:bajo",
    "builtin:onda",
    "builtin:voz",
    "builtin:glissando",
    "builtin:eco",
    "builtin:vinilo",
    "builtin:cinta",
    "builtin:metronomo",
    "builtin:espectro",
];

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
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc)
}

/// `blocks` blocks of 512 through the engine, its reports read.
fn run(app: &mut CantusApp, proc: &mut Processor, blocks: usize) {
    let mut buf = vec![0f32; 512 * 2];
    for _ in 0..blocks {
        proc.process(&mut buf, 2);
        app.engine.poll();
    }
}

/// One whole frame of the window, as `eframe` draws it.
fn frame(app: &mut CantusApp) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 800.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| app.frame(ui));
    out.textures_delta.clear();
}

fn drawn(app: &CantusApp, track: TrackId) -> Option<Vec<u8>> {
    app.stage
        .pixmap(track, Surface::Float)
        .map(|p| p.data().to_vec())
}

#[test]
fn every_slate_character_is_picked_dances_to_the_transport_and_is_saved() {
    let (mut app, mut proc) = assembled();
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Dancer),
    );
    let track = app.project().tracks.last().expect("the dancer").id;
    let spec = app
        .project()
        .track(track)
        .unwrap()
        .instrument
        .native()
        .unwrap();
    assert_eq!(spec.text(CHARACTER_KEY), Some(DEFAULT_CHARACTER));
    assert_eq!(DEFAULT_CHARACTER, "builtin:compas");
    app.sync_engine();
    app.engine.play();
    let dir = tempfile::tempdir().unwrap();
    for (i, id) in SLATE.iter().enumerate() {
        // As the picker's tile chooses it: one undo step.
        let text = |app: &CantusApp| {
            app.project()
                .track(track)
                .and_then(|t| t.instrument.native())
                .and_then(|n| n.text(CHARACTER_KEY))
                .map(str::to_string)
        };
        let before = text(&app);
        app.set_inst_text(track, CHARACTER_KEY, Some(id.to_string()));
        if before.as_deref() != Some(*id) {
            app.undo();
            assert_eq!(text(&app), before, "{id}: one undo step back");
            app.set_inst_text(track, CHARACTER_KEY, Some(id.to_string()));
        }
        assert_eq!(text(&app).as_deref(), Some(*id));
        app.sync_engine();
        run(&mut app, &mut proc, 30);
        frame(&mut app);
        let d = DanceLive::dancers(&app)
            .into_iter()
            .find(|d| d.track == track)
            .expect("the live dancer");
        assert_eq!(d.character.info().id, *id);
        assert!(d.missing.is_none(), "{id}: {:?}", d.missing);
        assert!(
            d.live && d.pose.playing,
            "{id}: danced from the engine's feed"
        );
        let a = drawn(&app, track).expect("drawn over the workspace");
        assert!(
            a.chunks(4).filter(|p| p[3] > 0).count() > 300,
            "{id}: the stage shows it"
        );
        // To the transport: later in the song, another pose and another picture (a
        // pose-to-pose character holds between its snaps: a few moments are looked at).
        let ch = cantus_dance::cast::by_id(id).unwrap();
        let st = cantus_dance::render::Stage::new(
            200,
            200,
            cantus_dance::render::Background::Transparent,
        );
        let draw = |p: Pose| {
            let p = Pose {
                x: 0.5,
                y: 0.9,
                ..p
            };
            cantus_dance::render(&st, &[(ch.as_ref(), p)]).unwrap()
        };
        let first = draw(d.pose);
        let mut moved = false;
        let mut beats = d.pose.beats;
        for _ in 0..8 {
            run(&mut app, &mut proc, 16);
            frame(&mut app);
            let later = DanceLive::dancers(&app)
                .into_iter()
                .find(|d| d.track == track)
                .map(|d| d.pose)
                .unwrap();
            assert!(later.beats != beats, "{id}: the song moved on (or looped)");
            beats = later.beats;
            moved |= draw(later).data() != first.data();
        }
        assert!(moved, "{id}: it danced");
        // Saved and opened again: the same character.
        let path = dir.path().join(format!("song-{i}.cantus"));
        app.save_to_path(&path);
        let p = cantus_project::load(&path, true).unwrap().project;
        let saved = p.track(track).and_then(|t| t.instrument.native()).unwrap();
        assert_eq!(saved.text(CHARACTER_KEY), Some(*id), "saved");
    }
    // The last one saved, opened through the app: it dances as itself.
    let last = dir.path().join(format!("song-{}.cantus", SLATE.len() - 1));
    app.open(&last);
    app.sync_engine();
    app.engine.play();
    run(&mut app, &mut proc, 30);
    frame(&mut app);
    let d = DanceLive::dancers(&app);
    assert_eq!(
        d.first().map(|d| d.character.info().id.clone()).as_deref(),
        Some(SLATE[SLATE.len() - 1])
    );
}

#[test]
fn the_mood_field_speaks_the_characters_own_words() {
    use cantus_ui::views::inst::dancer::controls::{mood_words, mood_words_of};
    let own = mood_words_of("builtin:metronomo");
    assert_eq!(own[0], "cast.metronomo.mood.neutral");
    assert_eq!(own[3], mood_words()[3], "Calm is everyone's word");
    assert_eq!(mood_words_of("builtin:ninja"), mood_words());
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for k in &own {
            assert!(cantus_ui::i18n::has(lang, k), "{k}");
        }
    }
}
