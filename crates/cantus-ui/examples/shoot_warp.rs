//! Warping without a screen (round 29, ADR 0061).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_warp -- [out-dir]
//! ```
//!
//! A 128 BPM project with a drum loop at 92 BPM (warped by the import rule, Rhythm, its
//! transients and bar markers shown) and a sung phrase (Melodic, −3 st, a marker moved):
//! the arrangement, the warp editor, the inspector's warp section, the clip menu's warp
//! entries and *Quantise audio…* — in Cantus Night and Paper, English and Spanish, at 75 %,
//! 100 % and 200 %, and the editor in a narrow window.

use cantus_core::warp::{Warp, WarpMode, WarpOp};
use cantus_core::{Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme_family::cantus_night()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn wav(path: &Path, rate: u32, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for v in x {
        w.write_sample((v.clamp(-1.0, 1.0) * 32767.0).round() as i16)
            .unwrap();
    }
    w.finalize().unwrap();
}

/// The demo's two files.
fn files(dir: &Path) -> (PathBuf, PathBuf) {
    std::fs::create_dir_all(dir).unwrap();
    let drums = dir.join("drums 92.wav");
    let mut m = cantus_dsp::beat::testkit::drum_loop(44_100.0, 92.0, 4, 0.2, 0.0, 7);
    m.audio
        .truncate((60.0 / 92.0 * 16.0 * 44_100.0_f64).round() as usize);
    wav(&drums, 44_100, &m.audio);
    let voice = dir.join("voice.wav");
    let f0 = |t: f64| {
        let notes = [(0.0, 220.0), (0.7, 246.9), (1.4, 261.6), (2.2, 196.0)];
        notes
            .iter()
            .rev()
            .find(|(s, _)| t >= *s)
            .map_or(220.0, |n| n.1) as f32
    };
    let x = cantus_dsp::pitch::testkit::voice(
        48_000.0,
        3.0,
        cantus_dsp::pitch::testkit::Vowel::A,
        0.5,
        f0,
    );
    wav(&voice, 48_000, &x);
    (drums, voice)
}

/// The app: both files imported, the loop warped by the import rule, the voice warped by
/// hand; the peaks read and the analyses in.
fn app_on(drums: &Path, voice: &Path) -> CantusApp {
    let mut p = Project::empty("Warp demo");
    p.tempo_bpm = 128.0;
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    app.import_audio(&[drums.to_path_buf()], Ticks::ZERO, None);
    app.import_audio(&[voice.to_path_buf()], Ticks::beats(4), None);
    let t0 = Instant::now();
    while app.poll_warp() && t0.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let tracks: Vec<_> = app.project().tracks.iter().map(|t| t.id).collect();
    let refs: Vec<(cantus_core::TrackId, cantus_core::ClipId)> = tracks
        .iter()
        .filter_map(|t| {
            let c = app.project().track(*t)?.clips.first()?.id;
            Some((*t, c))
        })
        .collect();
    let voice_r = *refs.last().unwrap();
    let mut w = Warp::identity(48_000, 128.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap();
    w = w.move_marker(1, Ticks::beats(3));
    w.mode = WarpMode::Melodic;
    w.cents = -300;
    app.commit_warp(voice_r, Some(w), WarpOp::Set, false);
    for r in &refs {
        while app.warp_analysis(*r).is_none() && t0.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    for src in app.project().audio_pool.clone() {
        while !matches!(
            app.peaks.lookup(&src, None, None),
            cantus_ui::waveform::Lookup::Ready(_)
        ) && t0.elapsed() < Duration::from_secs(60)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    app.select_clip(refs[0].0, refs[0].1);
    app
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/warp"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let (drums, voice) = files(&dir.join("demo"));
    use cantus_ui::i18n::Locale;
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            for k in [0.75f32, 1.0, 2.0] {
                cantus_ui::i18n::set_locale(lang);
                let ctx = context(dark);
                let mut app = app_on(&drums, &voice);
                let r = app.clip_selection()[0];
                let tag = format!(
                    "{}-{}-k{}",
                    if dark { "night" } else { "paper" },
                    lang.code(),
                    (k * 100.0) as u32
                );
                // The arrangement: the loop warped (its bars marked, its hits ticked).
                app.arrangement.px_per_beat = 1000.0 / 24.0;
                app.arrangement.scroll_beats = 0.0;
                let out = dir.join(format!("warp-arrangement-{tag}.png"));
                shoot(&ctx, egui::vec2(1200.0, 420.0), k, 6, Some(&out), |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::arrangement::show(&mut app, ui));
                });
                println!("{}", out.display());
                // The warp editor on the loop.
                app.open_warp_editor(r);
                let out = dir.join(format!("warp-editor-{tag}.png"));
                shoot(&context(dark), egui::vec2(1400.0, 520.0), k, 6, Some(&out), |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::piano_roll::show(&mut app, ui));
                });
                println!("{}", out.display());
                // The inspector's warp section.
                let out = dir.join(format!("warp-inspector-{tag}.png"));
                shoot(&context(dark), egui::vec2(360.0, 1100.0), k, 6, Some(&out), |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::inspector::show(&mut app, ui));
                });
                println!("{}", out.display());
                // The clip menu's warp entries.
                let out = dir.join(format!("warp-menu-{tag}.png"));
                shoot(&context(dark), egui::vec2(320.0, 420.0), k, 4, Some(&out), |root| {
                    egui::CentralPanel::default()
                        .frame(cantus_ui::kit::automate::frame())
                        .show(root, |ui| {
                            views::arrangement::warp_api::clip_menu(&mut app, ui, r)
                        });
                });
                println!("{}", out.display());
                // *Quantise audio…*.
                app.open_quantise(vec![r]);
                let out = dir.join(format!("warp-quantise-{tag}.png"));
                shoot(&context(dark), egui::vec2(900.0, 500.0), k, 4, Some(&out), |root| {
                    let ctx = root.ctx().clone();
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |_| {});
                    views::warp_quantise::frame(&mut app, &ctx);
                });
                println!("{}", out.display());
            }
        }
    }
    // Narrow: the editor's bar scrolls sideways.
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(true);
        let mut app = app_on(&drums, &voice);
        let r = app.clip_selection()[0];
        app.open_warp_editor(r);
        let out = dir.join(format!("warp-editor-narrow-night-{}.png", lang.code()));
        shoot(&ctx, egui::vec2(720.0, 480.0), 1.0, 6, Some(&out), |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| views::piano_roll::show(&mut app, ui));
        });
        println!("{}", out.display());
    }
}
