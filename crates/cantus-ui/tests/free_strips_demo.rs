//! Round 39 (`fix2`): a bus fed only by other strips is not a free strip. Before the fix
//! `Project::strip_is_free` counted the house demo's drum bus as free (no track plays into
//! it), so the song's dancer — and any track a user added to the song the program opens
//! on — landed on it, renamed it, and Kick, Clap, Hats and Percussion read "→ Dancer".
//!
//! End to end, as a user meets it: the program's start-up song through
//! `CantusApp::assemble`, a track added the way *Add track* adds it; and every demo song
//! as *File ▸ Demo songs* builds it. Its own binary: it points `CANTUS_DATA_DIR` at a
//! temporary folder.

use cantus_core::{Project, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        d
    })
    .path()
}

/// The strips another strip routes into (its output or a send): buses and returns. (A
/// sidechain's key is a channel the compressor listens to — the house song's Kick — not a
/// bus.)
fn fed(p: &Project) -> Vec<StripId> {
    p.strips
        .iter()
        .flat_map(|s| s.feeds().collect::<Vec<_>>())
        .collect()
}

fn drum_bus(p: &Project) -> StripId {
    p.strips
        .iter()
        .find(|s| s.name == "Drum bus")
        .map(|s| s.id)
        .expect("the house song has its drum bus, named")
}

#[test]
fn a_track_added_to_the_startup_song_does_not_take_the_drum_bus() {
    let _ = data();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::demos::startup_project()),
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    let bus = drum_bus(app.project());
    let before = app.project().tracks.len();
    app.add_track();
    let p = app.project();
    assert_eq!(p.tracks.len(), before + 1, "Add track made a track");
    let new = p.tracks.last().unwrap();
    let s = new.strip.expect("a fresh strip for the new track");
    assert_ne!(s, bus, "the new track took the drum bus");
    assert!(
        !fed(p).contains(&s),
        "the new track took a strip other strips play into: {:?}",
        p.strip(s).map(|x| &x.name)
    );
    assert!(
        p.strip(s).unwrap().effects.is_empty() && p.strip(s).unwrap().name.is_empty(),
        "the new track's strip was untouched"
    );
    assert_eq!(
        p.strip(bus).unwrap().name,
        "Drum bus",
        "the bus keeps its name"
    );
    // A second and a third: none lands on the returns either.
    app.add_track();
    app.add_track();
    let p = app.project();
    for t in &p.tracks[before..] {
        assert!(!fed(p).contains(&t.strip.unwrap()), "{} on a bus", t.name);
    }
}

/// Every demo song: no track plays into a strip that other strips route into, unless the
/// song meant it — and the house song's dancer, added last, takes a strip of its own.
#[test]
fn no_demo_puts_a_track_on_a_bus() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for demo in cantus_ui::demos::ALL.iter() {
        let p = (demo.build)();
        let fed = fed(&p);
        for t in &p.tracks {
            if let Some(s) = t.strip {
                assert!(
                    !fed.contains(&s),
                    "{}: {} plays into {:?}, a strip other strips route into",
                    demo.id,
                    t.name,
                    p.strip(s).map(|x| &x.name)
                );
            }
        }
    }
    let p = (cantus_ui::demos::find("house").unwrap().build)();
    let bus = drum_bus(&p);
    let dancer = p
        .tracks
        .iter()
        .find(|t| t.name == "Dancer")
        .expect("the whole house song keeps its dancer");
    assert_ne!(dancer.strip, Some(bus), "the dancer took the drum bus");
    let into_bus: Vec<&str> = p
        .strips
        .iter()
        .filter(|s| s.output == Some(bus))
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(into_bus, ["Kick", "Clap", "Hats", "Percussion"]);
}
