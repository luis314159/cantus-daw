//! The pitch editor without a screen (round 28, ADR 0054).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_pitch -- [out-dir]
//! ```
//!
//! The demo project's take (`cantus_ui::pitch::demo`) in the editor: corrected (a note
//! selected, its values in the panel) and as sung, then the Split tool and a marquee — in
//! Cantus Night and Paper, English and Spanish, at 75 %, 100 % and 200 %.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::PathBuf;
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

/// The app on the demo project with the take analysed and its peaks read.
fn app_on(project: &cantus_core::Project, wav: &std::path::Path, track: usize) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(project.clone()), EngineHandle::dummy(), false);
    app.engine
        .samples
        .pitch()
        .analyse_now(wav, 48_000)
        .expect("the take analyses");
    let src = app.project().audio_pool[0].clone();
    let t0 = Instant::now();
    while !matches!(
        app.peaks.lookup(&src, None, None),
        cantus_ui::waveform::Lookup::Ready(_)
    ) && t0.elapsed() < Duration::from_secs(20)
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    let (t, c) = {
        let tr = &app.project().tracks[track];
        (tr.id, tr.clips[0].id)
    };
    app.open_pitch_editor(t, c);
    app
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/pitch"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let demo_dir = dir.join("demo");
    let path = cantus_ui::pitch::demo::write_demo(&demo_dir).expect("the demo");
    let project = cantus_project::load(&path, true).expect("the demo opens").project;
    let wav = demo_dir.join("voice-take.wav");
    use cantus_ui::i18n::Locale;
    for state in ["corrected", "sung", "split"] {
        for dark in [true, false] {
            for lang in [Locale::En, Locale::Es] {
                for k in [0.75f32, 1.0, 2.0] {
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(dark);
                    let mut app = app_on(&project, &wav, if state == "sung" { 1 } else { 0 });
                    let name = format!(
                        "pitch-{state}-{}-{}-k{}.png",
                        if dark { "night" } else { "paper" },
                        lang.code(),
                        (k * 100.0) as u32
                    );
                    let out = dir.join(name);
                    let mut first = true;
                    shoot(&ctx, egui::vec2(1400.0, 640.0), k, 6, Some(&out), |root| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(root, |ui| views::piano_roll::show(&mut app, ui));
                        if first {
                            first = false;
                            let ids: Vec<u32> = app.pitch.view.blobs.iter().map(|b| b.0).collect();
                            match state {
                                "corrected" => {
                                    app.pitch.view.selected = ids.get(2).copied().into_iter().collect()
                                }
                                "split" => {
                                    app.pitch.view.tool = views::pitch::Tool::Split;
                                    app.pitch.view.selected = ids.iter().skip(3).take(3).copied().collect();
                                }
                                _ => {}
                            }
                        }
                    });
                    println!("{}", out.display());
                }
            }
        }
    }
}
