//! *Convert to MIDI* without a screen (round 34, ADR 0086).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_transcribe -- [out-dir]
//! ```
//!
//! The dialog on a sung take: notes and chords (the model not yet downloaded), melody,
//! drums with their classes, and mid-conversion — in Cantus Night and Paper, English and
//! Spanish, at 75 %, 100 % and 200 %.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ml::transcribe::spec::Mode;
use cantus_ui::raster::shoot;
use cantus_ui::transcribe_ui::run;
use cantus_ui::{views, CantusApp};
use std::path::PathBuf;

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

fn project(wav: &std::path::Path) -> Project {
    let mut p = Project::new("transcribe");
    p.tempo_bpm = 120.0;
    let keys = p.make_track("Keys");
    p.tracks.push(keys);
    let mut v = p.make_track("Lead");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "Vox".into(),
        path: wav.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 4,
        sha256: None,
        recorded: true,
    });
    let c = p.make_audio_clip("Vox.wav", id, Ticks::ZERO, Ticks::beats(8));
    v.clips.push(c);
    p.tracks.push(v);
    p
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/transcribe"));
    std::fs::create_dir_all(&dir).expect("output dir");
    std::env::set_var("CANTUS_DATA_DIR", dir.join("data"));
    let wav = dir.join("vox.wav");
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(&wav, spec).unwrap();
        for i in 0..48_000 * 4 {
            let t = i as f32 / 48_000.0;
            w.write_sample(0.2 * (std::f32::consts::TAU * 220.0 * t).sin())
                .unwrap();
        }
        w.finalize().unwrap();
    }
    let p = project(&wav);
    use cantus_ui::i18n::Locale;
    for state in ["poly", "melody", "drums", "running"] {
        for dark in [true, false] {
            for lang in [Locale::En, Locale::Es] {
                for k in [0.75f32, 1.0, 2.0] {
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(dark);
                    let mut app =
                        CantusApp::from_parts(Some(p.clone()), EngineHandle::dummy(), false);
                    let (t, c) = {
                        let tr = &app.project().tracks[1];
                        (tr.id, tr.clips[0].id)
                    };
                    run::open(&mut app, t, c);
                    {
                        let s = &mut app.transcribe.dialog.as_mut().unwrap().settings;
                        s.mode = match state {
                            "melody" => Mode::Melody,
                            "drums" => Mode::Drums,
                            _ => Mode::Polyphonic,
                        };
                    }
                    if state == "running" {
                        run::pretend_running(&mut app, run::Stage::Listening, 0.42);
                    }
                    let name = format!(
                        "transcribe-{state}-{}-{}-k{}.png",
                        if dark { "night" } else { "paper" },
                        lang.code(),
                        (k * 100.0) as u32
                    );
                    let out = dir.join(name);
                    shoot(&ctx, egui::vec2(1200.0, 720.0), k, 6, Some(&out), |root| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(root, |ui| views::arrangement::show(&mut app, ui));
                        cantus_ui::transcribe_ui::view::show(&mut app, root.ctx());
                    });
                    println!("{}", out.display());
                }
            }
        }
    }
}
