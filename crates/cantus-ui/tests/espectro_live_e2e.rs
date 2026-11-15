//! Espectro hears the song live, end to end (round 40, ADR 0122; fix-c R2-C1 … C3), in the
//! app `CantusApp::assemble` puts together on a real processor driven block by block:
//!
//! - a project's Espectro, added and chosen as the picker does, makes `DanceLive` hear the
//!   spectrum: the engine's master tap goes on, the companion's analyser (its window closed)
//!   analyses the song and the dancer's pose carries its four bands;
//! - stopped, nothing is analysed and the tap is off (round 38: nothing computes at rest);
//! - its track deleted, no dancer is left: the tap goes off and the analyser stops;
//! - Espectro chosen as the companion window's own character hears the song too, the
//!   window's visualiser off or on.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::dancer::CHARACTER_KEY;
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::dance_live::DanceLive;
use cantus_ui::keymap::Action as Key;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::stage::companion::VisMode;
use cantus_ui::CantusApp;
use std::path::Path;

const RATE: f64 = 48_000.0;
/// One frame of the window, in samples: 60 frames a second.
const FRAME: usize = 800;
const ESPECTRO: &str = "builtin:espectro";

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

struct Rig {
    app: CantusApp,
    proc_: Processor,
    ctx: egui::Context,
    t: f64,
}

fn assembled() -> Rig {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc_) = EngineHandle::manual(RATE);
    let project = cantus_ui::welcome_project();
    let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
    Rig {
        app,
        proc_,
        ctx,
        t: 0.0,
    }
}

impl Rig {
    /// One block of audio and one frame of the window.
    fn frame(&mut self) {
        let mut buf = vec![0f32; FRAME * 2];
        self.proc_.process(&mut buf, 2);
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1600.0, 900.0),
            )),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| app.frame(ui))
            .textures_delta
            .clear();
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    fn tap_on(&mut self) -> bool {
        self.app.engine.master_tap().is_some_and(|t| t.is_on())
    }

    /// The companion's analyser: (updates, analyses).
    fn counts(&self) -> (u64, u64) {
        self.app.stage.companion.vis_counts()
    }

    /// A dancer added as the picker adds one, Espectro chosen as its tile chooses it.
    fn add_espectro(&mut self) -> TrackId {
        self.app.apply_pick(
            PickPurpose::AddInstrument { index: None },
            PickChoice::Instrument(InstrumentKind::Dancer),
        );
        let track = self.app.project().tracks.last().expect("the dancer").id;
        self.app
            .set_inst_text(track, CHARACTER_KEY, Some(ESPECTRO.to_string()));
        self.app.sync_engine();
        track
    }

    fn espectro_bands(&self, track: TrackId) -> [f32; 4] {
        DanceLive::dancers(&self.app)
            .into_iter()
            .find(|d| d.track == track)
            .expect("the live dancer")
            .pose
            .bands
    }
}

#[test]
fn a_projects_espectro_hears_the_song_and_nothing_is_analysed_at_rest() {
    let mut r = assembled();
    r.frames(5);
    assert!(!r.tap_on(), "no dancer that hears: the tap is off");
    let track = r.add_espectro();
    // Stopped: Espectro's bars dance to the beat; nothing is analysed and the tap is off.
    r.frames(30);
    let before = r.counts();
    r.frames(30);
    assert!(!r.tap_on(), "stopped: the tap is off");
    assert_eq!(r.counts(), before, "stopped: the analyser does nothing");
    // Playing: the master tap on, the closed companion's analyser hearing the song, and the
    // dancer's pose carrying its bands (DanceLive::update → hears → tap → bands).
    r.app.toggle_play();
    r.frames(90);
    assert!(r.app.engine.is_playing());
    assert!(r.tap_on(), "playing with Espectro: the tap is on");
    // (The closed companion analyses on the wall clock, at most 20 times a second.)
    assert!(r.counts().1 > before.1, "it analyses: {:?}", r.counts());
    let bands = r.espectro_bands(track);
    assert!(
        bands.iter().any(|b| *b > 0.05),
        "the song's bands: {bands:?}"
    );
    // Stopped again: the tap goes off and the analyser rests.
    r.app.toggle_play();
    r.frames(3);
    let stopped = r.counts();
    r.frames(30);
    assert!(!r.tap_on(), "stopped again: the tap is off");
    assert_eq!(r.counts(), stopped, "stopped again: nothing analysed");
    // Its track deleted, playing: no dancer is left, so none hears.
    r.app.delete_track(track);
    r.app.sync_engine();
    r.app.toggle_play();
    r.frames(30);
    assert!(r.app.engine.is_playing());
    assert!(DanceLive::dancers(&r.app).is_empty());
    assert!(!r.app.dance_live.hears_spectrum(), "no dancer hears");
    let gone = r.counts();
    r.frames(30);
    assert!(!r.tap_on(), "no dancer left: the tap is off");
    assert_eq!(r.counts(), gone, "no dancer left: the analyser rests");
}

#[test]
fn espectro_in_the_companion_window_hears_the_song() {
    for vis in [VisMode::Off, VisMode::Spectrum] {
        let mut r = assembled();
        r.app.settings.stage.follow_project = false;
        r.app.settings.stage.character = ESPECTRO.to_string();
        r.app.settings.stage.vis = vis;
        r.frames(5);
        // Opened by its key (the settings file the first pass saved may open it already).
        if !r.app.settings.stage.companion_open {
            r.app.run_action(Key::ToggleCompanion);
        }
        assert!(r.app.settings.stage.companion_open);
        r.app.toggle_play();
        r.frames(90);
        assert!(r.app.engine.is_playing(), "{vis:?}: playing");
        assert!(
            r.tap_on(),
            "{vis:?}: the companion's Espectro wants the tap"
        );
        let (ch, pose, seen) = {
            let sh = r.app.stage.companion.shared.lock().unwrap();
            let (ch, pose) = sh.dancers.first().cloned().expect("its own character");
            (ch, pose, (sh.vis.bands, sh.vis.playing, sh.frame.playing))
        };
        let counts = r.counts();
        assert_eq!(ch.info().id, ESPECTRO);
        assert!(
            pose.bands.iter().any(|b| *b > 0.05),
            "{vis:?}: the companion's Espectro hears the song: {:?} (analyser {counts:?},              shown {seen:?})",
            pose.bands
        );
    }
}
