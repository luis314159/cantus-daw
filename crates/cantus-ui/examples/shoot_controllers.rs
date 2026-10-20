//! MIDI learn and controller mapping without a screen (round 29, ADR 0062).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_controllers -- [out-dir]
//! ```
//!
//! The mappings pane (both maps: transport keys and the master everywhere; a filter on a
//! knob with its range and bend, a 14-bit fader with feedback, an encoder, a pad on a mute,
//! one control on two targets, a mapping whose track is gone; a row lit by a message, one
//! waiting to learn again), a parameter's menu with *Learn MIDI controller* and *Forget*,
//! the learn mode over an effect's editor, the mixer and the arrangement, and the top
//! bar with its MIDI cell lit — in Cantus Night and Paper, English and Spanish, at 75 %, 100 % and
//! 200 %, and the pane in a narrow window. Review 2: the channel rack in the learn mode,
//! the mapped marks on the mixer's faders and lamps and on an effect's switches and choices,
//! a send's route menu with its level mapped, and the pane's rows for an action (no *Send*),
//! a plug-in not loaded and one learnt on another plug-in.

use cantus_core::controller::*;
use cantus_core::{AutoTarget, EffectKind, InstrumentSpec, Send, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::controllers::Scope;
use cantus_ui::i18n::Locale;
use cantus_ui::raster::{shoot, shoot_events};
use cantus_ui::{views, CantusApp};
use std::path::{Path, PathBuf};
use std::time::Instant;

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

fn src(port: &str, channel: Option<u8>, control: Control) -> MidiSource {
    MidiSource {
        port: PortRef {
            name: port.into(),
            ordinal: 0,
        },
        channel,
        control,
    }
}

/// The demo song with a controller's worth of mappings.
fn app() -> CantusApp {
    let project = cantus_ui::starter_project();
    let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
    app.midi_port_names = vec!["Keystation 49".into(), "nanoKONTROL2".into()];
    let track = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.expect("a strip");
    app.add_effect(strip, EffectKind::Compressor);
    let fx = app
        .project()
        .strip(strip)
        .unwrap()
        .effects
        .last()
        .unwrap()
        .id;
    let key = EffectKind::Compressor.params()[0].key.to_string();
    let mut knob = Mapping::new(
        MappingId(10),
        src("Keystation 49", Some(0), Control::Cc(74)),
        MappingTarget::Param(AutoTarget::Effect { effect: fx, key }),
        MapMode::Absolute,
    );
    knob.min = 0.15;
    knob.max = 0.85;
    knob.curve = 0.35;
    let mut fader = Mapping::new(
        MappingId(11),
        src("nanoKONTROL2", Some(0), Control::Cc14(0)),
        MappingTarget::Param(AutoTarget::StripGain(strip)),
        MapMode::Absolute,
    );
    fader.feedback = true;
    let mut pan = Mapping::new(
        MappingId(12),
        src("nanoKONTROL2", Some(0), Control::Cc(16)),
        MappingTarget::Param(AutoTarget::StripPan(strip)),
        MapMode::Relative(Encoding::TwosComplement),
    );
    pan.sensitivity = 2.0;
    let pad = Mapping::new(
        MappingId(13),
        src("Keystation 49", Some(9), Control::Note(36)),
        MappingTarget::TrackMute(track),
        MapMode::Toggle,
    );
    let mut shared = Mapping::new(
        MappingId(14),
        src("Keystation 49", Some(0), Control::Cc(74)),
        MappingTarget::Param(AutoTarget::StripWidth(strip)),
        MapMode::Absolute,
    );
    shared.takeover = Takeover::Scale;
    let gone = Mapping::new(
        MappingId(15),
        src("Keystation 49", Some(9), Control::Note(38)),
        MappingTarget::TrackSolo(TrackId(999_999)),
        MapMode::Toggle,
    );
    for m in [knob, fader, pan, pad, shared, gone] {
        app.add_mapping(Scope::Project, m);
    }
    // Review 2: the rack's pan, a send's level, the compressor's style and listen, a
    // hosted synth not loaded here and one learnt on another plug-in.
    let strips: Vec<_> = app.project().strips.iter().map(|s| s.id).collect();
    app.set_strip_send(
        strips[1],
        Send {
            level: 0.5,
            ..Send::new(strips[5])
        },
    );
    app.add_track();
    let synth_track = app.project().tracks.last().expect("a track").id;
    let vital = cantus_core::ExternalPlugin::new(
        cantus_core::external::PluginFormat::Clap,
        "audio.vital",
        "Vital",
    );
    app.set_instrument(synth_track, InstrumentSpec::External(vital.clone()));
    let hosted = MappingTarget::Param(AutoTarget::PluginInstrument {
        track: synth_track,
        id: 7,
    });
    let mut not_loaded = Mapping::new(
        MappingId(16),
        src("Keystation 49", Some(0), Control::Cc(71)),
        hosted.clone(),
        MapMode::Absolute,
    );
    not_loaded.plugin = hosted.plugin_of(app.project());
    let mut other = not_loaded.clone();
    other.id = MappingId(17);
    other.source = src("Keystation 49", Some(0), Control::Cc(72));
    other.plugin = Some(PluginRef {
        format: cantus_core::external::PluginFormat::Vst3,
        id: "SurgeXT".into(),
    });
    let style = EffectKind::Compressor
        .params()
        .iter()
        .find(|i| i.key == "style")
        .map(|i| i.key)
        .unwrap_or("style");
    for m in [
        Mapping::new(
            MappingId(18),
            src("nanoKONTROL2", Some(0), Control::Cc(17)),
            MappingTarget::Param(AutoTarget::TrackPan(track)),
            MapMode::Absolute,
        ),
        Mapping::new(
            MappingId(19),
            src("nanoKONTROL2", Some(0), Control::Cc(18)),
            MappingTarget::Param(AutoTarget::Send {
                strip: strips[1],
                to: strips[5],
            }),
            MapMode::Absolute,
        ),
        Mapping::new(
            MappingId(20),
            src("nanoKONTROL2", Some(0), Control::Cc(80)),
            MappingTarget::Param(AutoTarget::Effect {
                effect: fx,
                key: style.into(),
            }),
            MapMode::Relative(Encoding::TwosComplement),
        ),
        Mapping::new(
            MappingId(21),
            src("nanoKONTROL2", Some(0), Control::Cc(48)),
            MappingTarget::Param(AutoTarget::Effect {
                effect: fx,
                key: "listen".into(),
            }),
            MapMode::Toggle,
        ),
        Mapping::new(
            MappingId(22),
            src("nanoKONTROL2", Some(0), Control::Cc(64)),
            MappingTarget::StripMute(strip),
            MapMode::Toggle,
        ),
        not_loaded,
        other,
        // Review 3: a port not open (*Not connected*), the strip's effects switch.
        Mapping::new(
            MappingId(23),
            src("Launchkey Mini", Some(0), Control::Cc(21)),
            MappingTarget::Param(AutoTarget::MasterPan),
            MapMode::Absolute,
        ),
        Mapping::new(
            MappingId(24),
            src("nanoKONTROL2", Some(0), Control::Cc(65)),
            MappingTarget::StripFx(strip),
            MapMode::Toggle,
        ),
    ] {
        app.add_mapping(Scope::Project, m);
    }
    for (i, (action, n)) in [
        ("play-pause", 41u8),
        ("stop", 42),
        ("record", 45),
        ("toggle-loop", 46),
    ]
    .into_iter()
    .enumerate()
    {
        app.controllers.global.mappings.push(Mapping::new(
            MappingId(100 + i as u64),
            src("nanoKONTROL2", Some(0), Control::Cc(n)),
            MappingTarget::Action(action.into()),
            MapMode::Trigger,
        ));
    }
    // An action that is no state: its row has no *Send*.
    app.controllers.global.mappings.push(Mapping::new(
        MappingId(104),
        src("nanoKONTROL2", Some(0), Control::Cc(43)),
        MappingTarget::Action("undo".into()),
        MapMode::Trigger,
    ));
    let mut master = Mapping::new(
        MappingId(110),
        src("nanoKONTROL2", Some(0), Control::Cc(7)),
        MappingTarget::Param(AutoTarget::MasterGain),
        MapMode::Absolute,
    );
    master.feedback = true;
    app.controllers.global.mappings.push(master);
    app.controllers.global.mappings.push(Mapping::new(
        MappingId(111),
        src("", None, Control::Cc(20)),
        MappingTarget::DeviceParam(0),
        MapMode::Relative(Encoding::BinaryOffset),
    ));
    // A message just came on the knob.
    app.controllers
        .activity
        .insert((Scope::Project, MappingId(10)), Instant::now());
    app
}

fn name(dir: &Path, what: &str, dark: bool, lang: Locale, k: f32) -> PathBuf {
    dir.join(format!(
        "controllers-{what}-{}-{}-k{}.png",
        if dark { "night" } else { "paper" },
        lang.code(),
        (k * 100.0) as u32
    ))
}

fn panel(root: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui)) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(root, f);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/controllers"));
    std::fs::create_dir_all(&dir).expect("output dir");
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            for k in [0.75f32, 1.0, 2.0] {
                cantus_ui::i18n::set_locale(lang);
                // The pane, one row waiting to learn again.
                let ctx = context(dark);
                let mut a = app();
                a.learn_target(a.project().controllers.mappings[1].target.clone());
                if let Some(l) = a.controllers.learning.as_mut() {
                    l.replace = Some(MappingId(11));
                }
                let out = name(&dir, "pane", dark, lang, k);
                shoot(&ctx, egui::vec2(1500.0, 860.0), k, 6, Some(&out), |root| {
                    panel(root, |ui| views::controllers::show(&mut a, ui));
                });
                println!("{}", out.display());
                // The learn mode over an effect's editor: one knob mapped, one armed.
                let ctx = context(dark);
                let mut a = app();
                let strip = a.project().tracks[0].strip.unwrap();
                let fx = a.project().strip(strip).unwrap().effects.last().unwrap().id;
                a.open_effect_editor(strip, fx);
                a.toggle_learn_mode();
                let key = EffectKind::Compressor.params()[1].key.to_string();
                a.learn_target(MappingTarget::Param(AutoTarget::Effect { effect: fx, key }));
                let out = name(&dir, "learn-editor", dark, lang, k);
                shoot(&ctx, egui::vec2(1100.0, 640.0), k, 8, Some(&out), |root| {
                    panel(root, |_| {});
                    views::comp_editor::window(&mut a, root.ctx());
                    views::fx::window(&mut a, root.ctx());
                });
                println!("{}", out.display());
                // Review 2: the channel rack in the learn mode (its knobs and lamps).
                let ctx = context(dark);
                let mut a = app();
                a.toggle_learn_mode();
                // The rack edits the selected clip.
                let t0 = a.project().tracks[0].id;
                a.selected_track = Some(t0);
                a.selected_clip = a.project().tracks[0].clips.first().map(|c| c.id);
                let out = name(&dir, "learn-rack", dark, lang, k);
                shoot(&ctx, egui::vec2(1100.0, 420.0), k, 6, Some(&out), |root| {
                    panel(root, |ui| views::channel_rack::show(&mut a, ui));
                });
                println!("{}", out.display());
                // The mapped marks out of the learn mode: the mixer's fader, pan, lamps.
                let ctx = context(dark);
                let mut a = app();
                let out = name(&dir, "marks-mixer", dark, lang, k);
                shoot(&ctx, egui::vec2(1300.0, 560.0), k, 6, Some(&out), |root| {
                    panel(root, |ui| views::mixer::show(&mut a, ui));
                });
                println!("{}", out.display());
                // …and an effect's style and listen switches.
                let ctx = context(dark);
                let mut a = app();
                let strip = a.project().tracks[0].strip.unwrap();
                let fx = a.project().strip(strip).unwrap().effects.last().unwrap().id;
                a.open_effect_editor(strip, fx);
                let out = name(&dir, "marks-editor", dark, lang, k);
                shoot(&ctx, egui::vec2(1100.0, 640.0), k, 8, Some(&out), |root| {
                    panel(root, |_| {});
                    views::comp_editor::window(&mut a, root.ctx());
                    views::fx::window(&mut a, root.ctx());
                });
                println!("{}", out.display());
                // A send's route menu, its level mapped.
                let ctx = context(dark);
                let mut a = app();
                let strips: Vec<_> = a.project().strips.iter().map(|s| s.id).collect();
                views::route_menu::open(
                    &ctx,
                    egui::Id::new("mixer.route_menu"),
                    cantus_core::routing::Route::send(strips[1], strips[5]),
                    egui::pos2(420.0, 120.0),
                );
                let out = name(&dir, "route-menu", dark, lang, k);
                shoot(&ctx, egui::vec2(1300.0, 560.0), k, 6, Some(&out), |root| {
                    panel(root, |ui| views::mixer::show(&mut a, ui));
                });
                println!("{}", out.display());
            }
            // The mixer and the arrangement in learn mode; the top bar's key lit.
            for (what, size) in [
                ("learn-mixer", egui::vec2(1300.0, 560.0)),
                ("learn-arrangement", egui::vec2(1300.0, 420.0)),
                ("topbar", egui::vec2(1920.0, 70.0)),
            ] {
                cantus_ui::i18n::set_locale(lang);
                let ctx = context(dark);
                let mut a = app();
                a.toggle_learn_mode();
                let out = name(&dir, what, dark, lang, 1.0);
                shoot(&ctx, size, 1.0, 6, Some(&out), |root| {
                    panel(root, |ui| match what {
                        "learn-mixer" => views::mixer::show(&mut a, ui),
                        "learn-arrangement" => views::arrangement::show(&mut a, ui),
                        _ => views::transport::show(&mut a, ui),
                    });
                });
                println!("{}", out.display());
            }
            // Review 3: Tonus's scale keyboard in the learn mode — each key outlined where
            // it shows, one key armed.
            cantus_ui::i18n::set_locale(lang);
            let ctx = context(dark);
            let mut a = app();
            let strip = a.project().tracks[0].strip.unwrap();
            a.add_effect(strip, EffectKind::AutoTune);
            let fx = a.project().strip(strip).unwrap().effects.last().unwrap().id;
            a.open_effect_editor(strip, fx);
            a.toggle_learn_mode();
            a.learn_target(MappingTarget::Param(AutoTarget::Effect {
                effect: fx,
                key: "n1".into(),
            }));
            let out = name(&dir, "learn-tonus", dark, lang, 1.0);
            shoot(&ctx, egui::vec2(1100.0, 700.0), 1.0, 8, Some(&out), |root| {
                panel(root, |_| {});
                views::fx::window(&mut a, root.ctx());
            });
            println!("{}", out.display());
            // A parameter's menu: *Learn MIDI controller* again and *Forget*.
            cantus_ui::i18n::set_locale(lang);
            let ctx = context(dark);
            let at = egui::pos2(60.0, 60.0);
            let out = name(&dir, "menu", dark, lang, 1.0);
            let mut v = 0.4f32;
            shoot_events(
                &ctx,
                egui::vec2(420.0, 300.0),
                1.0,
                6,
                Some(&out),
                |frame, _| match frame {
                    1 => vec![egui::Event::PointerMoved(at)],
                    2 => vec![egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed: true,
                        modifiers: Default::default(),
                    }],
                    3 => vec![egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed: false,
                        modifiers: Default::default(),
                    }],
                    _ => Vec::new(),
                },
                |root| {
                    panel(root, |ui| {
                        ui.add_space(30.0);
                        ui.horizontal(|ui| {
                            ui.add_space(30.0);
                            let m = cantus_ui::kit::automate::ModOffer {
                                learn: Some(cantus_ui::kit::automate::LearnOffer {
                                    mapped: true,
                                    mode: false,
                                    armed: false,
                                }),
                                ..Default::default()
                            };
                            let _ = cantus_ui::kit::widgets::Param::new(&mut v, 0.0..=1.0, 0.5)
                                .automation(false)
                                .modulation(m)
                                .show_full(ui);
                        });
                    });
                },
            );
            println!("{}", out.display());
        }
    }
    // Narrow: the pane scrolls both ways.
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(true);
        let mut a = app();
        let out = dir.join(format!("controllers-pane-narrow-night-{}.png", lang.code()));
        shoot(&ctx, egui::vec2(620.0, 380.0), 1.0, 6, Some(&out), |root| {
            panel(root, |ui| views::controllers::show(&mut a, ui));
        });
        println!("{}", out.display());
    }
}
