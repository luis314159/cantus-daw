//! The browser's preview level as the sound plays (round 39, fixes; BACKLOG §8): a click
//! along the preview strip's level changes the audition's level at once — ramped by the
//! engine, the sound going on from where it was — instead of starting it again. Through
//! `CantusApp::assemble` and the real engine wiring (a processor driven block by block),
//! the strip reached by clicks as the user reaches it.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder, and the preview
//! level is the process's.

use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::views::{samples_browser, samples_thumbs};
use cantus_ui::CantusApp;
use egui::{pos2, vec2, Event, Pos2, Rect};
use std::path::{Path, PathBuf};

const SIZE: egui::Vec2 = egui::vec2(1500.0, 900.0);
/// One frame of the window, in samples.
const BLOCK: usize = 800;
/// The sound's level: a constant, so any restart's fade-in shows as a dip.
const DC: f32 = 0.5;

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

/// Three seconds of a constant level.
fn wav(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..3 * 48_000 {
        w.write_sample((DC * 32_768.0) as i16).unwrap();
    }
    w.finalize().unwrap();
}

struct Rig {
    app: CantusApp,
    proc_: Processor,
    ctx: egui::Context,
    /// Every block's left channel.
    out: Vec<f32>,
    _dir: tempfile::TempDir,
    sound: PathBuf,
}

impl Rig {
    fn new() -> Rig {
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let project = cantus_core::Project::new("audition");
        let mut app =
            CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("Sounds");
        let sound = user.join("Level.wav");
        wav(&sound);
        app.settings.sample_folders = vec![user.clone()];
        app.samples.open.insert(user);
        Rig {
            app,
            proc_,
            ctx,
            out: Vec::new(),
            _dir: dir,
            sound,
        }
    }

    /// One block of audio, then one frame of the browser as the app draws it.
    fn frame(&mut self, events: Vec<Event>) {
        let mut buf = vec![0f32; BLOCK * 2];
        self.proc_.process(&mut buf, 2);
        self.out.extend(buf.chunks(2).map(|f| f[0]));
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
            events,
            ..Default::default()
        };
        let a = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            a.poll_samples(&ctx);
            let left = Rect::from_min_size(Pos2::ZERO, vec2(320.0, SIZE.y));
            let mut l = ui.new_child(egui::UiBuilder::new().max_rect(left));
            samples_browser::show(a, &mut l);
            samples_browser::frame(a, &ctx);
        });
        out.textures_delta.clear();
    }

    fn click(&mut self, at: Pos2) {
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![Event::PointerMoved(at), press(true)]);
        self.frame(vec![Event::PointerMoved(at), press(false)]);
    }
}

#[test]
fn a_new_preview_level_is_heard_as_it_plays_without_starting_again() {
    samples_thumbs::set_preview_volume(1.0);
    let mut r = Rig::new();
    let mut row = None;
    for _ in 0..200 {
        r.frame(vec![]);
        row = r
            .app
            .samples
            .geometry
            .as_ref()
            .and_then(|g| g.row_of(&r.sound).map(|row| row.rect));
        if row.is_some() {
            break;
        }
    }
    let row = row.expect("the sound is listed");
    // A click on its row auditions it.
    r.click(row.center());
    let mut heard = false;
    for _ in 0..400 {
        r.frame(vec![]);
        if r.out.iter().rev().take(BLOCK).all(|s| *s > 0.01) {
            heard = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(heard, "the audition plays");
    let full = *r.out.last().unwrap();
    let started = r.app.samples.preview.playing.as_ref().map(|p| p.1);
    // The level at a quarter, by a click along the strip.
    let level = r
        .app
        .samples
        .geometry
        .as_ref()
        .and_then(|g| g.preview_level)
        .expect("the preview strip's level");
    let from = r.out.len();
    r.click(pos2(level.left() + level.width() * 0.25, level.center().y));
    let v = samples_thumbs::preview_volume();
    assert!((v - 0.25).abs() < 0.03, "{v}");
    for _ in 0..4 {
        r.frame(vec![]);
    }
    let after = &r.out[from..];
    let last = *after.last().unwrap();
    assert!(
        (last - full * v).abs() < 1e-3 * full,
        "the level is the new one: {last} against {}",
        full * v
    );
    // Never a dip under it (a start again fades in from silence), never a rise.
    for (i, w) in after.windows(2).enumerate() {
        assert!(w[1] <= w[0] + 1e-6, "it only goes down, at {i}: {w:?}");
        assert!(w[1] >= last - 1e-6, "no restart's fade-in, at {i}: {w:?}");
    }
    assert_eq!(
        r.app.samples.preview.playing.as_ref().map(|p| p.1),
        started,
        "the audition was not started again"
    );
    samples_thumbs::set_preview_volume(1.0);
}
