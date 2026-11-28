//! The Tuning window's fixes of round 42's mid-round review (`docs/round42/fix-ui.md`), on
//! the boot path (`CantusApp::assemble`): the window is modal for keys — only Undo, Redo and
//! its own shortcut pass, and Esc closes it without clearing the selection behind it; a
//! replaced project closes it with its cadence; the cadence plays on a track that follows
//! the project and not at all for a scale that is not twelve to the octave; Equal drops a
//! leftover keyboard map; *Follow the song's key* can always be unticked; Chorda's card shows
//! the key a followed temperament resolves to, and keeps a chosen key per track; *Be the
//! MTS-ESP master* resigns when the song chooses MTS-ESP and can always be switched off (the
//! fake `LIBMTS` of `cantus-rt`'s tests). Its own binary: `CANTUS_DATA_DIR` points at a
//! temporary folder.

use cantus_core::scale::{Key as SongKey, ScaleKind};
use cantus_core::tuning::{Temperament, TrackTuning, Tuning, TuningSystem};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::keymap::Action;
use cantus_ui::views::tuning::{self, Target};
use cantus_ui::CantusApp;
use egui::{Key, Modifiers};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn data() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn app() -> (CantusApp, egui::Context) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    // Two instrument tracks, added as the picker adds them.
    for kind in [InstrumentKind::Chorda, InstrumentKind::Analog] {
        app.apply_pick(
            cantus_ui::picker::PickPurpose::AddInstrument { index: None },
            cantus_ui::picker::PickChoice::Instrument(kind),
        );
    }
    assert!(instrument_tracks(&app).len() >= 2, "two instrument tracks");
    (app, ctx)
}

/// One frame with one key press, as a keyboard sends it.
fn press(ctx: &egui::Context, a: &mut CantusApp, key: Key, modifiers: Modifiers) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        events: vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        a.global_shortcuts(ui.ctx());
        tuning::show(a, ui.ctx());
    });
    out.textures_delta.clear();
}

fn werck(key: u8, follow: bool) -> Tuning {
    Tuning {
        a4_hz: 415.0,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key,
            follow_song_key: follow,
        },
        map: None,
    }
}

fn instrument_tracks(a: &CantusApp) -> Vec<TrackId> {
    a.project()
        .tracks
        .iter()
        .filter(|t| t.instrument.native().is_some())
        .map(|t| t.id)
        .collect()
}

/// Review ui#1: Delete, Space, Ctrl+N and Esc's *Clear selection* never reach the project
/// behind the open window; Ctrl+Z and Ctrl+Y do; Ctrl+Shift+T (its own key) closes it.
#[test]
fn the_open_window_holds_the_keys_but_undo_redo_and_its_own() {
    let (mut a, ctx) = app();
    let ids: Vec<TrackId> = a.project().tracks.iter().map(|t| t.id).collect();
    assert!(!ids.is_empty());
    tuning::set(&mut a, Target::Project, werck(3, false));
    // Its own key opens it (the keymap's, on the boot path).
    press(&ctx, &mut a, Key::T, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(
        a.tuning.open,
        Some(Target::Project),
        "Ctrl+Shift+T opens it"
    );
    a.selected_tracks = ids.to_vec();
    a.selected_track = ids.first().copied();
    let before = a.project().clone();
    let name = a.project().name.clone();
    press(&ctx, &mut a, Key::Delete, Modifiers::NONE);
    press(&ctx, &mut a, Key::Space, Modifiers::NONE);
    press(&ctx, &mut a, Key::N, Modifiers::COMMAND);
    assert_eq!(
        a.project().tracks.len(),
        before.tracks.len(),
        "Delete kept away"
    );
    assert_eq!(a.project().name, name, "Ctrl+N kept away");
    assert!(!a.engine.is_playing(), "Space kept away");
    assert!(
        !a.selected_tracks.is_empty(),
        "nothing cleared the selection"
    );
    // Undo and Redo pass.
    press(&ctx, &mut a, Key::Z, Modifiers::COMMAND);
    assert!(a.project().tuning.is_default(), "Ctrl+Z undid the tuning");
    // Redo as the system binds it: Ctrl+Y on Windows and Linux, Cmd+Shift+Z on macOS.
    let redo = cantus_ui::keymap::cantus_keymap().bindings_for(Action::Redo)[0];
    let mut redo_mods = Modifiers::NONE;
    if redo.command {
        redo_mods = redo_mods | Modifiers::COMMAND;
    }
    if redo.shift {
        redo_mods = redo_mods | Modifiers::SHIFT;
    }
    if redo.alt {
        redo_mods = redo_mods | Modifiers::ALT;
    }
    press(&ctx, &mut a, redo.key, redo_mods);
    assert_eq!(a.project().tuning, werck(3, false), "Redo redid it");
    assert!(a.tuning.open.is_some(), "still open");
    // Esc closes the window, and only the window.
    press(&ctx, &mut a, Key::Escape, Modifiers::NONE);
    assert_eq!(a.tuning.open, None, "Esc closed it");
    assert!(
        !a.selected_tracks.is_empty(),
        "Esc did not also clear the selection behind it"
    );
    // Closed, the keys are the project's again.
    tuning::open(&mut a, Target::Project);
    press(&ctx, &mut a, Key::T, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(a.tuning.open, None, "its own key closes it");
    assert!(cantus_ui::keymap::cantus_keymap()
        .bindings_for(Action::OpenTuning)
        .iter()
        .all(|b| !(b.alt && b.command)));
}

/// Review ui#1: New (every replacement goes through `reset_bus_to`) closes a window on a
/// track and releases its cadence — the new project's ids start again at 1.
#[test]
fn a_replaced_project_closes_the_window_and_its_cadence() {
    let (mut a, _ctx) = app();
    let track = instrument_tracks(&a)[0];
    tuning::open(&mut a, Target::Track(track));
    tuning::start_audition(&mut a, Target::Track(track), 0.0);
    assert!(a.tuning.audition.is_some(), "the cadence plays");
    a.tuning.chorda_key.insert(track, 5);
    a.new_project();
    assert_eq!(a.tuning.open, None);
    assert!(a.tuning.audition.is_none(), "its keys released");
    assert!(
        a.tuning.chorda_key.is_empty(),
        "the old tracks' keys forgotten"
    );
}

/// Review ui#6, ui#7: the project's cadence plays on a track that follows the project (a
/// selected track with a tuning of its own would play its own), says why when none does,
/// and is off for a scale that is not twelve to the octave.
#[test]
fn the_cadence_plays_what_the_window_edits_or_says_why_not() {
    let (mut a, _ctx) = app();
    let tracks = instrument_tracks(&a);
    assert!(!tracks.is_empty());
    tuning::set(&mut a, Target::Project, werck(4, false));
    let own = tracks[0];
    a.exec(Command::SetTrackTuning {
        track: own,
        tuning: TrackTuning::Own {
            tuning: werck(0, false),
        },
    });
    a.selected_track = Some(own);
    let at = tuning::audition_place(&a, Target::Project);
    assert_ne!(at, Ok(own), "not the track that plays its own");
    if tracks.len() == 1 {
        assert_eq!(at, Err("tuning.audition.none_follows"));
    } else {
        let id = at.expect("a following track");
        assert!(a.project().track(id).unwrap().tuning.is_follow());
    }
    // Every instrument track its own: the window says so.
    for t in &tracks {
        a.exec(Command::SetTrackTuning {
            track: *t,
            tuning: TrackTuning::Equal,
        });
    }
    let rest: Vec<TrackId> = a
        .project()
        .tracks
        .iter()
        .filter(|t| t.tuning.is_follow() && !tracks.contains(&t.id))
        .map(|t| t.id)
        .collect();
    if rest.is_empty() {
        assert_eq!(
            tuning::audition_place(&a, Target::Project),
            Err("tuning.audition.none_follows")
        );
    }
    // A 31-note scale: no cadence.
    let scl = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../cantus-core/tests/data/tuning-library/31edo.scl");
    tuning::io::import(&mut a, Target::Track(own), &scl, None).expect("read");
    assert_eq!(
        tuning::audition_place(&a, Target::Track(own)),
        Err("tuning.audition.not_twelve")
    );
    tuning::start_audition(&mut a, Target::Track(own), 0.0);
    assert!(a.tuning.audition.is_none(), "no I–IV–V–I in 31-EDO");
}

/// Review ui#4: a keyboard map imported over equal temperament becomes a 12-note scale with
/// it (where the map row shows it), and Equal or Temperament chosen again drops it.
#[test]
fn equal_and_temperament_drop_a_keyboard_map() {
    let (mut a, _ctx) = app();
    let kbm = data().join("map.kbm");
    std::fs::write(
        &kbm,
        "12\n0\n127\n60\n69\n440.0\n12\n0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n",
    )
    .unwrap();
    tuning::io::import_map(&mut a, Target::Project, &kbm).expect("read");
    let tu = a.project().tuning.clone();
    assert!(tu.map.is_some());
    assert!(
        matches!(&tu.system, TuningSystem::Scale(s) if s.degrees.len() == 12),
        "{:?}",
        tu.system
    );
    tuning::choose_system(&mut a, Target::Project, TuningSystem::Equal);
    assert!(a.project().tuning.map.is_none(), "Equal dropped the map");
    assert!(!tuning::words::summary_of(&a.project().tuning, None).contains("mapped"));
    tuning::io::import_map(&mut a, Target::Project, &kbm).expect("read");
    tuning::choose_system(&mut a, Target::Project, werck(2, false).system);
    assert!(
        a.project().tuning.map.is_none(),
        "Temperament dropped the map"
    );
}

/// Review ui#8: Chorda's card shows (and rings) the key the project's followed temperament
/// resolves to, greyed; a key chosen in one track's card is that track's alone.
#[test]
fn chordas_card_shows_the_resolved_key_and_keeps_one_per_track() {
    let (mut a, _ctx) = app();
    let tracks = instrument_tracks(&a);
    tuning::set(&mut a, Target::Project, werck(3, false));
    assert_eq!(
        tuning::chorda::key_of(&a, tracks[0]),
        (3, true),
        "Eb, set by the project"
    );
    a.exec(Command::SetProjectKey {
        key: Some(SongKey::new(9, ScaleKind::Minor)),
    });
    tuning::set(&mut a, Target::Project, werck(3, true));
    assert_eq!(
        tuning::chorda::key_of(&a, tracks[0]),
        (9, true),
        "following the song's key: A"
    );
    // Its own equal temperament: the card's own key, per track.
    a.exec(Command::SetTrackTuning {
        track: tracks[0],
        tuning: TrackTuning::Equal,
    });
    a.tuning.chorda_key.insert(tracks[0], 7);
    assert_eq!(tuning::chorda::key_of(&a, tracks[0]), (7, false));
    if let Some(other) = tracks.get(1) {
        a.exec(Command::SetTrackTuning {
            track: *other,
            tuning: TrackTuning::Equal,
        });
        assert_eq!(
            tuning::chorda::key_of(&a, *other),
            (9, false),
            "another track: the song's key, not the first track's choice"
        );
    }
}

/// Review ui#24: *Slightly out* is found again after a language switch, and named in the
/// interface's language.
#[test]
fn slightly_out_is_found_in_either_language() {
    use cantus_ui::i18n::{set_thread_locale, Locale};
    let (mut a, _ctx) = app();
    let track = instrument_tracks(&a)[0];
    let spec = a
        .project()
        .track(track)
        .and_then(|t| t.instrument.native().cloned())
        .unwrap();
    let p = tuning::chorda::PRESETS
        .iter()
        .find(|p| p.name == "tuning.chorda.p.slightly_out")
        .unwrap();
    set_thread_locale(Locale::En);
    a.exec(tuning::chorda::preset_command(&spec, track, p, 0));
    let spec = a
        .project()
        .track(track)
        .and_then(|t| t.instrument.native().cloned())
        .unwrap();
    set_thread_locale(Locale::Es);
    let found = tuning::chorda::current(&a, track, &spec).map(|p| p.name);
    let words = tuning::words::track_mode_text(&a.project().track(track).unwrap().tuning, None);
    set_thread_locale(Locale::En);
    assert_eq!(found, Some("tuning.chorda.p.slightly_out"));
    assert!(words.contains("Un poco desafinado"), "{words}");
}

/// A click on the window's control `name`, as a hand gives it.
fn click(_ctx: &egui::Context, a: &mut CantusApp, name: &str) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    let mut at = None;
    cantus_ui::raster::shoot_events(
        &ctx,
        egui::vec2(1200.0, 800.0),
        1.0,
        6,
        None,
        |n, c| {
            if n != 3 {
                return Vec::new();
            }
            let p = tuning::geo(c, name)
                .unwrap_or_else(|| panic!("{name} drawn"))
                .center();
            at = Some(p);
            let button = |pressed| egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            vec![egui::Event::PointerMoved(p), button(true), button(false)]
        },
        |ui| tuning::show(a, &ui.ctx().clone()),
    );
    assert!(at.is_some(), "clicked {name}");
}

/// Review ui#3: *Follow the song's key* ticked on a song whose key was then removed can
/// still be unticked from the window.
#[test]
fn follow_the_songs_key_can_always_be_unticked() {
    let (mut a, ctx) = app();
    cantus_ui::design::install_fonts(&ctx);
    a.exec(Command::SetProjectKey {
        key: Some(SongKey::new(2, ScaleKind::Major)),
    });
    tuning::set(&mut a, Target::Project, werck(0, true));
    a.exec(Command::SetProjectKey { key: None });
    tuning::open(&mut a, Target::Project);
    click(&ctx, &mut a, "follow");
    assert!(
        matches!(
            a.project().tuning.system,
            TuningSystem::Temperament {
                follow_song_key: false,
                ..
            }
        ),
        "{:?}",
        a.project().tuning.system
    );
}

fn rustc() -> std::ffi::OsString {
    if let Some(r) = std::env::var_os("RUSTC") {
        return r;
    }
    if let Some(cargo) = std::env::var_os("CARGO") {
        let beside =
            PathBuf::from(cargo).with_file_name(if cfg!(windows) { "rustc.exe" } else { "rustc" });
        if beside.is_file() {
            return beside.into_os_string();
        }
    }
    "rustc".into()
}

/// The fake `LIBMTS` of `cantus-rt`'s tests (the same C symbols), built once.
fn fake_mts() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fake-mts-ui");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(if cfg!(windows) {
        "fake_mts.dll"
    } else if cfg!(target_os = "macos") {
        "libfake_mts.dylib"
    } else {
        "libfake_mts.so"
    });
    let src =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cantus-rt/tests/fake_mts/fake_mts.rs");
    let out = std::process::Command::new(rustc())
        .args(["--edition", "2021", "--crate-type", "cdylib"])
        .args(["--crate-name", "fake_mts", "-C", "opt-level=1", "-o"])
        .arg(&file)
        .arg(&src)
        .output()
        .expect("rustc runs");
    assert!(
        out.status.success(),
        "the fake LIBMTS did not build:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    file
}

/// Review ui#2: choosing MTS-ESP for the song resigns Cantus as master (it would publish
/// 12-TET to every client), and a master can always be switched off.
#[test]
fn choosing_mts_esp_resigns_the_master_and_the_switch_always_turns_off() {
    let (mut a, _ctx) = app();
    tuning::install_mts(&mut a, cantus_engine::mts::MtsLink::open(&fake_mts()));
    tuning::set(&mut a, Target::Project, werck(3, false));
    tuning::set_master(&mut a, true, false);
    let link = a.tuning.mts.clone().unwrap();
    assert!(link.is_master(), "{:?}", a.tuning.master_refused);
    assert_eq!(tuning::master_switch(&a, Target::Project), (true, true));
    tuning::choose_system(&mut a, Target::Project, TuningSystem::External);
    assert!(!link.is_master(), "following MTS-ESP, Cantus resigned");
    assert_eq!(a.project().tuning.system, TuningSystem::External);
    // A master while the song follows MTS-ESP (another path) can still be switched off.
    tuning::set_master(&mut a, true, false);
    if link.is_master() {
        assert_eq!(tuning::master_switch(&a, Target::Project), (true, true));
    }
    tuning::set_master(&mut a, false, false);
    assert!(!link.is_master());
    assert!(!tuning::master_switch(&a, Target::Project).0);
}
