//! The new lead character end to end (round 37, ADR 0105), on the boot path: a dancer
//! added the way the instrument picker adds one, in the app `CantusApp::assemble` puts
//! together, names Fuga the ninja and the screen's live dancer draws Fuga; an old song
//! whose dancer named no character opens through the app with the songbird it always
//! showed (schema 59), and one that named Volta keeps Volta.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::dancer::{CHARACTER_KEY, DEFAULT_CHARACTER};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::dance_live::DanceLive;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled(project: Option<Project>) -> CantusApp {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    CantusApp::assemble(&ctx, files, project, engine, None, None, None, false)
}

/// The id of the character the screen draws for the first dancer.
fn drawn(app: &mut CantusApp) -> String {
    DanceLive::update(app, 0.5);
    let d = DanceLive::dancers(app);
    let first = d.first().expect("a live dancer");
    assert!(first.missing.is_none(), "{:?}", first.missing);
    first.character.info().id.clone()
}

#[test]
fn a_new_dancer_shows_fuga_the_ninja() {
    assert_eq!(DEFAULT_CHARACTER, "builtin:ninja");
    let mut app = assembled(None);
    let before = app.project().tracks.len();
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Dancer),
    );
    assert_eq!(app.project().tracks.len(), before + 1, "one track more");
    let t = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Dancer)
        })
        .expect("the dancer");
    let n = t.instrument.native().unwrap();
    assert_eq!(n.text(CHARACTER_KEY), Some("builtin:ninja"));
    assert_eq!(drawn(&mut app), "builtin:ninja");
    // The picker's first tile is Fuga too, and Volta follows.
    let cast = cantus_dance::cast::builtin();
    assert_eq!(cast[0].info().id, "builtin:ninja");
    assert_eq!(cast[1].info().id, "builtin:mecha");
    assert_eq!(cast[0].info().name, "Fuga");
    assert_eq!(cast[1].info().name, "Volta");
}

/// The v57 fixture with its dancer's text changed by `edit`, written where the app opens it.
fn old_song(edit: impl Fn(&mut serde_json::Map<String, serde_json::Value>)) -> PathBuf {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cantus-project/fixtures");
    let text = std::fs::read_to_string(fixtures.join("v57.cantus")).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let tracks = v["project"]["tracks"].as_array_mut().unwrap();
    let dancer = tracks
        .iter_mut()
        .find(|t| t["instrument"]["kind"] == "Dancer")
        .expect("the fixture's dancer");
    edit(dancer["instrument"].as_object_mut().unwrap());
    let dir = tempfile::tempdir().unwrap().keep();
    let path = dir.join("old.cantus");
    std::fs::write(&path, serde_json::to_string(&v).unwrap()).unwrap();
    path
}

#[test]
fn an_old_song_keeps_its_dancer() {
    // Named no character: it showed the songbird, and still does.
    let path = old_song(|inst| {
        inst.remove("text");
    });
    let mut app = assembled(None);
    app.open(&path);
    assert_eq!(drawn(&mut app), "builtin:songbird");
    // Named Volta: Volta.
    let path = old_song(|inst| {
        inst.insert(
            "text".into(),
            serde_json::json!({ "character": "builtin:mecha" }),
        );
    });
    app.open(&path);
    assert_eq!(drawn(&mut app), "builtin:mecha");
}

/// The project as saved to disk and read back: what the dancer track names.
fn saved_character(app: &mut CantusApp, track: TrackId) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.cantus");
    app.save_to_path(&path);
    let p = cantus_project::load(&path, true).unwrap().project;
    p.track(track)
        .and_then(|t| t.instrument.native())
        .filter(|n| n.kind == InstrumentKind::Dancer)
        .and_then(|n| n.text(CHARACTER_KEY))
        .map(str::to_string)
}

fn a_track(app: &mut CantusApp, kind: InstrumentKind) -> TrackId {
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(kind),
    );
    app.project().tracks.last().expect("a track").id
}

#[test]
fn every_way_to_a_dancer_names_its_character() {
    let mut app = assembled(None);
    // The inspector's instrument menu: it hands `set_instrument` a bare spec, as built
    // there (`views/inspector_menus.rs`), with its MPE routes.
    let t = a_track(&mut app, InstrumentKind::Chorda);
    let bare = cantus_core::model::note_expr::defaults::with_routes(NativeSpec::new(
        InstrumentKind::Dancer,
    ));
    assert_eq!(bare.text(CHARACTER_KEY), None, "the menu's spec names none");
    app.set_instrument(t, InstrumentSpec::Native(bare));
    assert_eq!(
        saved_character(&mut app, t).as_deref(),
        Some("builtin:ninja")
    );
    // *Swap instrument*, keeping matching settings (the default): rebuilt from a bare spec.
    let t = a_track(&mut app, InstrumentKind::Chorda);
    assert!(app.swap_keep.matching, "on by default");
    assert!(app.replace_instrument(t, &PickChoice::Instrument(InstrumentKind::Dancer)));
    assert_eq!(
        saved_character(&mut app, t).as_deref(),
        Some("builtin:ninja")
    );
    // And without keeping them.
    let t = a_track(&mut app, InstrumentKind::Chorda);
    app.swap_keep.matching = false;
    assert!(app.replace_instrument(t, &PickChoice::Instrument(InstrumentKind::Dancer)));
    assert_eq!(
        saved_character(&mut app, t).as_deref(),
        Some("builtin:ninja")
    );
    // A dancer swapped to a dancer with a character of its own keeps it.
    app.swap_keep.matching = true;
    let t = a_track(&mut app, InstrumentKind::Dancer);
    let mut volta = app
        .project()
        .track(t)
        .unwrap()
        .instrument
        .native()
        .unwrap()
        .clone();
    volta
        .set_text(CHARACTER_KEY, Some("builtin:mecha".into()))
        .unwrap();
    app.set_instrument(t, InstrumentSpec::Native(volta));
    assert_eq!(
        saved_character(&mut app, t).as_deref(),
        Some("builtin:mecha")
    );
}
