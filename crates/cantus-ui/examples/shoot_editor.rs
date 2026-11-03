//! The audio editor without a screen (round 36, ADR 0096).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_editor -- [out-dir]
//! ```
//!
//! A voice-like take in hiss, in Cantus Night and Paper, English and Spanish, at 75 %, 100 %
//! and 200 %: the wave and the spectrogram with a selection, a spectral rectangle picked, the
//! noise learned, and the wave alone zoomed to the samples.

use cantus_core::audio_edit::Sel;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::editor::{self, Act, Band, Show};
use cantus_ui::raster::shoot;
use cantus_ui::CantusApp;
use std::path::PathBuf;

const RATE: u32 = 48_000;

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

/// Hiss, then three syllables of a gliding voice with its harmonics, then hiss.
fn take() -> Vec<f32> {
    let n = RATE as usize * 5;
    let mut s = 12345u32;
    let mut x: Vec<f32> = (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((s >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * 0.02
        })
        .collect();
    let (a, b) = (RATE as usize, RATE as usize * 4);
    for (i, v) in x[a..b].iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let env = (0.5 + 0.5 * (std::f32::consts::TAU * 1.5 * t).sin()).powi(2);
        let f0 = 160.0 + 40.0 * (std::f32::consts::TAU * 0.4 * t).sin();
        for h in 1..=8 {
            *v += env * 0.22 / h as f32 * (std::f32::consts::TAU * f0 * h as f32 * t).sin();
        }
    }
    x
}

fn pump(app: &mut CantusApp, ctx: &egui::Context) {
    for _ in 0..4000 {
        editor::frame(app, ctx);
        let s = app.editor.session.as_ref().unwrap();
        if s.busy.is_none() && s.buf.is_some() && s.spec.is_some() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the editor never settled");
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/editor"));
    std::fs::create_dir_all(&dir).expect("output dir");
    std::env::set_var("CANTUS_DATA_DIR", dir.join("data"));
    let wav = dir.join("voice.wav");
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(&wav, spec).unwrap();
        for v in take() {
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
    }
    let mut p = Project::new("editor");
    p.tempo_bpm = 120.0;
    let mut v = p.make_track("Voice");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "Voice".into(),
        path: wav.to_string_lossy().into_owned(),
        sample_rate: RATE,
        channels: 1,
        frames: RATE as u64 * 5,
        sha256: None,
        recorded: true,
    });
    let c = p.make_audio_clip("Voice.wav", id, Ticks::ZERO, Ticks::beats(10));
    v.clips.push(c);
    p.tracks.push(v);
    use cantus_ui::i18n::Locale;
    for state in ["both", "region", "noise", "samples"] {
        for dark in [true, false] {
            for lang in [Locale::En, Locale::Es] {
                for k in [0.75f32, 1.0, 2.0] {
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(dark);
                    let mut app =
                        CantusApp::from_parts(Some(p.clone()), EngineHandle::dummy(), false);
                    let (t, c) = {
                        let tr = &app.project().tracks[0];
                        (tr.id, tr.clips[0].id)
                    };
                    editor::open(&mut app, t, c);
                    pump(&mut app, &ctx);
                    let r = RATE as usize;
                    match state {
                        "region" => {
                            let s = app.editor.session.as_mut().unwrap();
                            s.sel = Some(Sel::new(r * 3 / 2, r * 3));
                            s.band = Some(Band {
                                lo_hz: 700.0,
                                hi_hz: 2400.0,
                            });
                        }
                        "noise" => {
                            app.editor.session.as_mut().unwrap().sel =
                                Some(Sel::new(0, r * 9 / 10));
                            editor::act(&mut app, Act::Learn);
                            pump(&mut app, &ctx);
                            app.editor.session.as_mut().unwrap().sel =
                                Some(Sel::new(r * 3 / 2, r * 3));
                        }
                        "samples" => {
                            app.editor.show = Show::Wave;
                            let s = app.editor.session.as_mut().unwrap();
                            s.sel = Some(Sel::new(r * 2 + 100, r * 2 + 240));
                            s.view = editor::View {
                                start: (r * 2) as f64,
                                per: 0.5,
                            };
                            s.fit = false;
                        }
                        _ => {
                            let s = app.editor.session.as_mut().unwrap();
                            s.sel = Some(Sel::new(r * 3 / 2, r * 3));
                            let id = s.source;
                            let marks = cantus_core::SourceMarks {
                                markers: vec![
                                    cantus_core::SourceMarker {
                                        frame: r as u64,
                                        name: "Verse".into(),
                                    },
                                    cantus_core::SourceMarker {
                                        frame: 4 * r as u64,
                                        name: String::new(),
                                    },
                                ],
                                regions: vec![cantus_core::SourceRegion {
                                    start: 2 * r as u64,
                                    end: 3 * r as u64,
                                    name: "Take".into(),
                                }],
                                loop_points: Some((r as u64 / 2, r as u64)),
                            };
                            app.exec(Command::SetAudioSourceMarks {
                                id,
                                marks: Some(Box::new(marks)),
                            });
                            editor::frame(&mut app, &ctx);
                            let s = app.editor.session.as_mut().unwrap();
                            s.sel = Some(Sel::new(r * 3 / 2, r * 3));
                        }
                    }
                    let name = format!(
                        "editor-{state}-{}-{}-k{}.png",
                        if dark { "night" } else { "paper" },
                        lang.code(),
                        (k * 100.0) as u32
                    );
                    let out = dir.join(name);
                    shoot(&ctx, egui::vec2(1500.0, 900.0), k, 6, Some(&out), |root| {
                        editor::view::show(&mut app, root.ctx());
                    });
                    println!("{}", out.display());
                }
            }
        }
    }
}
