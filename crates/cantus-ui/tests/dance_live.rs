//! The dancers live on the interface (round 35, ADR 0087), on the boot path: the desktop
//! app goes `run` → `boot::Boot` → `CantusApp::assemble`, and every frame
//! (`CantusApp::frame`) reads each dancer's feed once and poses it at the heard position.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::dancer::{self, Move};
use cantus_core::*;
use cantus_dance::pose_at;
use cantus_engine::EngineHandle;
use cantus_engine::Processor;
use cantus_ui::app::Files;
use cantus_ui::dance_live::DanceLive;
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

fn assembled() -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc)
}

/// The first track turned into a dancer, `edit` applied to its spec, the engine told.
fn dancer_on_first(app: &mut CantusApp, edit: impl FnOnce(&mut NativeSpec)) -> TrackId {
    let track = app.project().tracks[0].id;
    let mut spec = NativeSpec::new(InstrumentKind::Dancer);
    edit(&mut spec);
    assert!(app.exec(Command::SetInstrument {
        track,
        spec: Box::new(InstrumentSpec::Native(spec)),
    }));
    app.sync_engine();
    track
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

#[test]
fn a_frame_of_the_shipped_app_poses_the_dancer_from_its_engine_feed() {
    let (mut app, mut proc) = assembled();
    let track = dancer_on_first(&mut app, |s| {
        s.set("move", dancer::move_value(Move::Clap)).unwrap();
    });
    app.engine.play();
    run(&mut app, &mut proc, 40);
    frame(&mut app);
    let d = DanceLive::dancers(&app);
    assert_eq!(d.len(), 1);
    let d = &d[0];
    assert_eq!(d.track, track);
    assert!(d.live, "from the engine's feed, not the transport alone");
    assert!(d.frame.playing && d.pose.playing);
    assert_eq!(d.frame.value("move"), Some(dancer::move_value(Move::Clap)));
    // The welcome song's first clip plays C4 on its first beat: a key of the move map
    // (Arms up), which wins over `move` while it plays.
    let n = d.frame.note.expect("the clip's note");
    assert_eq!(Move::from_key(n.key), Some(Move::ArmsUp));
    assert_eq!(
        d.pose.mv,
        cantus_dance::pose::resolve(d.character.as_ref(), Move::ArmsUp)
    );
    assert_eq!(d.missing, None);
    assert!(app.dance_live.reads() >= 1);
}

#[test]
fn the_pose_is_at_the_heard_position_once_a_frame() {
    let (mut app, mut proc) = assembled();
    dancer_on_first(&mut app, |_| {});
    app.engine.play();
    run(&mut app, &mut proc, 30);
    DanceLive::update(&mut app, 0.0);
    let heard = app.engine.position().as_beats_f64();
    let d = DanceLive::dancers(&app).remove(0);
    assert!(heard > 0.5, "the song moved: {heard}");
    assert!(
        (d.frame.beats - heard).abs() < 1e-9,
        "{} vs {heard}",
        d.frame.beats
    );
    // The pose is the pure function of that frame.
    assert_eq!(d.pose, pose_at(&d.frame, d.character.as_ref(), 0.0));
    // A frame later with nothing new from the engine: nothing more is read.
    let reads = app.dance_live.reads();
    DanceLive::update(&mut app, 0.0);
    assert_eq!(app.dance_live.reads(), reads);
    // Between the engine's reports the shown position runs on, a little and no more.
    DanceLive::update(&mut app, 1.0);
    let on = DanceLive::dancers(&app)[0].frame.beats;
    let tempo = f64::from(DanceLive::dancers(&app)[0].frame.tempo_bpm);
    assert!(on > heard && on <= heard + cantus_ui::dance_live::RUN_ON * tempo / 60.0 + 1e-9);
}

#[test]
fn stopped_it_dances_from_the_feed_and_breathes() {
    let (mut app, mut proc) = assembled();
    dancer_on_first(&mut app, |s| {
        s.set("hue", 30.0).unwrap();
    });
    run(&mut app, &mut proc, 4);
    DanceLive::update(&mut app, 2.0);
    let a = DanceLive::dancers(&app).remove(0);
    assert!(a.live && !a.pose.playing);
    assert_eq!(a.pose.mv, Move::Idle);
    assert_eq!(a.pose.hue, 30.0);
    DanceLive::update(&mut app, 2.5);
    assert_ne!(
        DanceLive::dancers(&app)[0].pose.phase,
        a.pose.phase,
        "it breathes"
    );
}

#[test]
fn with_no_unit_yet_it_dances_from_the_transport() {
    let mut app = CantusApp::from_parts(None, EngineHandle::dummy(), false);
    let track = dancer_on_first(&mut app, |s| {
        s.set("energy", 0.4).unwrap();
    });
    DanceLive::update(&mut app, 0.0);
    let d = DanceLive::dancers(&app);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].track, track);
    assert!(!d[0].live);
    assert_eq!(d[0].frame.value("energy"), Some(0.4));
}

#[test]
fn a_character_that_cannot_be_found_draws_the_default_and_says_so() {
    let mut app = CantusApp::from_parts(None, EngineHandle::dummy(), false);
    dancer_on_first(&mut app, |s| {
        s.set_text(dancer::CHARACTER_KEY, Some("builtin:nobody".into()))
            .unwrap();
    });
    DanceLive::update(&mut app, 0.0);
    let d = DanceLive::dancers(&app).remove(0);
    assert_eq!(d.missing.as_deref(), Some("builtin:nobody"));
    assert_eq!(
        d.character.info().id,
        cantus_dance::cast::default_character().info().id
    );
    // An import that is not there: the default, said.
    let dir = cantus_ui::dance_import::project_dir(&app);
    let (c, missing) = app.dance_live.character("file:Characters/x/x.json", &dir);
    assert_eq!(
        c.info().id,
        cantus_dance::cast::default_character().info().id
    );
    assert!(missing.is_some());
}

#[test]
fn a_project_switch_and_a_track_that_stops_dancing_drop_their_feeds() {
    let (mut app, mut proc) = assembled();
    let track = dancer_on_first(&mut app, |_| {});
    app.engine.play();
    run(&mut app, &mut proc, 8);
    DanceLive::update(&mut app, 0.0);
    assert_eq!(DanceLive::dancers(&app).len(), 1);
    assert!(app.engine.dance_feed(track).is_some());
    // The track hosts a synth again: no dancer, and the engine forgets its feed.
    assert!(app.exec(Command::SetInstrument {
        track,
        spec: Box::new(InstrumentSpec::default()),
    }));
    app.sync_engine();
    run(&mut app, &mut proc, 2);
    app.sync_engine();
    DanceLive::update(&mut app, 0.0);
    assert!(DanceLive::dancers(&app).is_empty());
    assert!(app.engine.dance_feed(track).is_none());
    // A dancer again, then another project: nothing carries over.
    dancer_on_first(&mut app, |_| {});
    run(&mut app, &mut proc, 2);
    DanceLive::update(&mut app, 0.0);
    assert_eq!(DanceLive::dancers(&app).len(), 1);
    app.replace_project(Project::new("Another"));
    DanceLive::update(&mut app, 0.0);
    assert!(DanceLive::dancers(&app).is_empty());
}

/// An imported character (ADR 0089) dances through import's loader, from the project's
/// folder — a saved project's, sandboxed — and *Edit moves…* (its package rewritten)
/// reaches the screen at the next frame: an import is never cached here for good.
#[test]
fn an_imported_character_dances_and_its_edited_moves_reach_the_screen() {
    use cantus_dance::import::{package, samples};
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::aseprite("pal")).unwrap();
    let project = tempfile::tempdir().unwrap();
    let p = src.path().join("pal.json");
    let found = cantus_dance::import::inspect(&p).unwrap();
    let settings = cantus_dance::import::guessed(&found);
    let licence = found.licence.clone();
    let rel = package::copy_into(project.path(), &p, &found, &settings, &licence).unwrap();
    let mut app = CantusApp::from_parts(None, EngineHandle::dummy(), false);
    app.path = Some(project.path().join("song.cantus"));
    let id = format!("file:{rel}");
    dancer_on_first(&mut app, |s| {
        s.set_text(dancer::CHARACTER_KEY, Some(id.clone())).unwrap();
    });
    assert_eq!(cantus_ui::dance_import::project_dir(&app), project.path());
    let wait = std::time::Duration::from_secs(20);
    // Read off this thread: the default dances meanwhile, and says it is loading.
    DanceLive::update(&mut app, 0.0);
    let d = DanceLive::dancers(&app).remove(0);
    if d.loading {
        assert_eq!(
            d.character.info().id,
            cantus_dance::cast::default_character().info().id
        );
    }
    app.dance_live.wait_for_loads(wait);
    DanceLive::update(&mut app, 0.1);
    let first = DanceLive::dancers(&app).remove(0);
    assert!(!first.loading && first.missing.is_none(), "{:?}", first.why);
    assert_eq!(first.character.info().id, id);
    let reads = app.dance_live.import_reads();
    // Asked again later, the files unchanged: the same character, nothing read again.
    DanceLive::update(&mut app, 1.0);
    app.dance_live.wait_for_loads(wait);
    DanceLive::update(&mut app, 1.1);
    assert!(std::sync::Arc::ptr_eq(
        &first.character,
        &DanceLive::dancers(&app)[0].character
    ));
    for i in 0..6 {
        DanceLive::update(&mut app, 3.0 + f64::from(i));
        app.dance_live.wait_for_loads(wait);
    }
    assert_eq!(
        app.dance_live.import_reads(),
        reads,
        "unchanged files are not read again"
    );
    // Edit moves…: its package rewritten; the next look reaches the screen.
    let mut edited = settings.clone();
    edited
        .mapping
        .moves
        .swap(Move::Clap.index(), Move::Wave.index());
    // Packages are immutable (ADR 0089): the edit is a new package, and the dancer is
    // re-keyed to it, as `dance_import::rekey` does.
    let new_rel = package::with_settings(project.path(), &rel, &edited).unwrap();
    assert_ne!(new_rel, rel, "an edit makes a new package");
    let new_id = format!("file:{new_rel}");
    dancer_on_first(&mut app, |s| {
        s.set_text(dancer::CHARACTER_KEY, Some(new_id.clone()))
            .unwrap();
    });
    DanceLive::update(&mut app, 2.0);
    app.dance_live.wait_for_loads(wait);
    DanceLive::update(&mut app, 2.1);
    let now = DanceLive::dancers(&app).remove(0);
    assert!(!std::sync::Arc::ptr_eq(&first.character, &now.character));
    assert_eq!(now.character.info().id, new_id);
}

/// A project file naming a character outside its folder (`file:../../x.json`, an
/// absolute path) is refused by the sandbox: the default dances and the screen says why.
#[test]
fn a_character_key_that_leaves_the_project_draws_the_default_and_says_why() {
    let project = tempfile::tempdir().unwrap();
    // A real file where the key points, so only the sandbox stands in the way.
    let outside = project.path().parent().unwrap().join("x.json");
    let _ = std::fs::write(&outside, b"{}");
    let mut app = CantusApp::from_parts(None, EngineHandle::dummy(), false);
    app.path = Some(project.path().join("song.cantus"));
    for key in [
        "file:../x.json",
        "file:../../x.json",
        "file:C:/Windows/win.ini",
        "file:/etc/passwd",
    ] {
        dancer_on_first(&mut app, |s| {
            s.set_text(dancer::CHARACTER_KEY, Some(key.into())).unwrap();
        });
        DanceLive::update(&mut app, 0.0);
        app.dance_live
            .wait_for_loads(std::time::Duration::from_secs(20));
        DanceLive::update(&mut app, 0.1);
        let d = DanceLive::dancers(&app).remove(0);
        assert_eq!(d.missing.as_deref(), Some(key), "{key}");
        assert!(
            d.why.as_deref().is_some_and(|w| !w.is_empty()),
            "{key}: why"
        );
        assert_eq!(
            d.character.info().id,
            cantus_dance::cast::default_character().info().id
        );
    }
    let _ = std::fs::remove_file(outside);
}

/// A save that carries an imported character to another folder (`Characters/<name>-2/`):
/// the key changes before the project's path does, so for a frame or two the key is
/// looked for in the old folder and missed; once the path changes, the dancer draws the
/// imported character again — a miss is never kept.
#[test]
fn a_renaming_save_finds_the_character_in_its_new_folder() {
    use cantus_dance::import::{package, samples};
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::aseprite("pal")).unwrap();
    let p = src.path().join("pal.json");
    let found = cantus_dance::import::inspect(&p).unwrap();
    let settings = cantus_dance::import::guessed(&found);
    let licence = found.licence.clone();
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    let rel_old = package::copy_into(old.path(), &p, &found, &settings, &licence).unwrap();
    // The save carried it to a folder of another name in the new place (as a carry beside
    // an edited same-named folder does): the key changes before the project's path does.
    let folder = rel_old.split('/').nth(1).unwrap().to_string();
    package::copy_into(new.path(), &p, &found, &settings, &licence).unwrap();
    std::fs::rename(
        new.path().join("Characters").join(&folder),
        new.path().join("Characters").join(format!("{folder}-2")),
    )
    .unwrap();
    let rel_new = rel_old.replace(
        &format!("Characters/{folder}/"),
        &format!("Characters/{folder}-2/"),
    );
    let mut app = CantusApp::from_parts(None, EngineHandle::dummy(), false);
    app.path = Some(old.path().join("song.cantus"));
    let wait = std::time::Duration::from_secs(20);
    let key = |rel: &str| format!("file:{rel}");
    let new_key = key(&rel_new);
    dancer_on_first(&mut app, |s| {
        s.set_text(dancer::CHARACTER_KEY, Some(key(&rel_old)))
            .unwrap();
    });
    DanceLive::update(&mut app, 0.0);
    app.dance_live.wait_for_loads(wait);
    DanceLive::update(&mut app, 0.1);
    assert!(DanceLive::dancers(&app)[0].missing.is_none());
    // The save rewrote the key first: the old folder has no `pal-2`.
    let track = app.project().tracks[0].id;
    app.set_inst_text(track, dancer::CHARACTER_KEY, Some(new_key.clone()));
    DanceLive::update(&mut app, 0.2);
    app.dance_live.wait_for_loads(wait);
    DanceLive::update(&mut app, 0.3);
    assert_eq!(
        DanceLive::dancers(&app)[0].missing.as_deref(),
        Some(new_key.as_str())
    );
    // Then the project's path: the next frames draw the character from its new folder.
    app.path = Some(new.path().join("song.cantus"));
    DanceLive::update(&mut app, 0.4);
    app.dance_live.wait_for_loads(wait);
    DanceLive::update(&mut app, 0.5);
    let d = DanceLive::dancers(&app).remove(0);
    assert!(d.missing.is_none(), "{:?}", d.why);
    assert_eq!(d.character.info().id, new_key);
}
