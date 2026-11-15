//! r40:pads — a pad's own mixer track in the channel rack (ADR 0117), rendered headless: the
//! demo song's hats kit with its pads listed, each with its mixer track's well (the kit's
//! number until one is chosen, des-views' "3" under the kit's "3"); the open hat sent to a
//! strip of its own (its well reads it); and the list a pad's well opens. Slate Night and
//! Slate Paper, the whole window at 1920 × 1080 (125 %).
//!
//! `cargo run -p cantus-ui --example shoot_pads -- <out_dir> [case…] [--es] [--zoom <z>]`
//! (`--es`: in Spanish, `-es` in the name; `--zoom 1.0`: at 100 %, `-100` in the name — the
//! mid-round review's C-9 asked for the inspector's rows in Spanish at 100 and 150 %).

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::workspace::Pane;
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq)]
enum Scene {
    /// Every pad on the kit's strip.
    Kit,
    /// The open hat on a strip of its own.
    Own,
    /// The list the open hat's well opens.
    List,
    /// r40:pads — the inspector's Sidechain section: a compressor keyed from the kick, its
    /// key heard (ADR 0118).
    Hear,
    /// The inspector's Recording section: a track with an input recording *After FX*.
    Record,
}

/// The demo song set up for the inspector's scenes: the track to select, and the keyed
/// compressor (Hear).
fn taps_song(
    p: &mut cantus_core::Project,
    scene: Scene,
) -> Option<(cantus_core::TrackId, Option<cantus_core::EffectId>)> {
    use cantus_core::*;
    match scene {
        Scene::Hear => {
            // The song's own keyed compressor, when it has one (the bass from the kick).
            for t in &p.tracks {
                let Some(fx) = t
                    .strip
                    .and_then(|s| p.strip(s))
                    .and_then(|s| s.effects.iter().find(|e| e.sidechain.is_some()))
                else {
                    continue;
                };
                return Some((t.id, Some(fx.id)));
            }
            let key = p.tracks.first()?.strip?;
            let t = p
                .tracks
                .iter()
                .find(|t| t.name.to_lowercase().contains("bass"))?;
            let (tid, strip) = (t.id, t.strip?);
            let mut fx = p.make_effect(EffectKind::Compressor);
            fx.sidechain = Some(key);
            let id = fx.id;
            p.strip_mut(strip)?.effects.push(fx);
            Some((tid, Some(id)))
        }
        Scene::Record => {
            let t = p
                .tracks
                .iter_mut()
                .find(|t| t.name.to_lowercase().contains("lead"))?;
            t.input = Some(TrackInput::mono(0));
            t.armed = true;
            t.record_tap = RecordTap::AfterFx;
            Some((t.id, None))
        }
        _ => None,
    }
}

/// The demo's hats kit: its track and the pad to route (the second, or the first).
fn hats(a: &CantusApp) -> Option<(cantus_core::TrackId, cantus_core::PadId)> {
    let t = a
        .project()
        .tracks
        .iter()
        .find(|t| t.name.to_lowercase().contains("hat") && t.instrument.pulsus().is_some())
        .or_else(|| {
            a.project()
                .tracks
                .iter()
                .find(|t| t.instrument.pulsus().is_some())
        })?;
    let k = t.instrument.pulsus()?;
    let pad = k.pads.get(1).or(k.pads.first())?.id;
    Some((t.id, pad))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-pads")
    });
    let rest: Vec<String> = args.collect();
    let es = rest.iter().any(|a| a == "--es");
    let zoom_arg: Option<f32> = rest
        .iter()
        .position(|a| a == "--zoom")
        .and_then(|i| rest.get(i + 1))
        .and_then(|z| z.parse().ok());
    let only: Vec<String> = rest
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && (*i == 0 || rest[i - 1] != "--zoom"))
        .map(|(_, a)| a.clone())
        .collect();
    cantus_ui::i18n::set_locale(if es {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let scenes = [
        ("rack-pads-kit", Scene::Kit),
        ("rack-pads-own", Scene::Own),
        ("rack-pads-list", Scene::List),
        ("insp-hear-key", Scene::Hear),
        ("insp-record-tap", Scene::Record),
    ];
    for (view, scene) in scenes {
        for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
            let (size, zoom) = ((1920.0f32, 1080.0f32), zoom_arg.unwrap_or(1.25));
            let mut name = format!("{view}-{}", preset.trim_start_matches("slate_"));
            if let Some(z) = zoom_arg {
                name.push_str(&format!("-{}", (z * 100.0).round()));
            }
            if es {
                name.push_str("-es");
            }
            if !only.is_empty() && !only.iter().any(|o| name.starts_with(o.as_str())) {
                continue;
            }
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let tmp = std::env::temp_dir().join("cantus-shoot-pads");
            let mut project = cantus_ui::demos::startup_project();
            let taps = taps_song(&mut project, scene);
            let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app.layouts_path = tmp.join("layouts.json");
            app.keymap_path = tmp.join("keymap.toml");
            app.appearance_path = tmp.join("appearance.json");
            app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
            let mode = app.appearance.mode;
            app.appearance.assign(mode, format!("preset:{preset}"));
            app.attach(&ctx);
            if let Some((t, fx)) = taps {
                // The inspector on the track, its section open (as a click on its title).
                let fold = if scene == Scene::Hear {
                    "inspector.sidechain"
                } else {
                    "rec.recording"
                };
                ctx.data_mut(|d| d.insert_temp(egui::Id::new(("insp-fold", fold)), true));
                app.select_track(t);
                app.engine.set_hear_key(fx);
                // Tall enough for the inspector to reach the section, folded below.
                let points = egui::vec2(size.0 / zoom, 2400.0 / zoom);
                let path = dir.join(format!("{name}.png"));
                raster::shoot(&ctx, points, zoom, 8, Some(&path), |root| app.frame(root));
                println!("{}", path.display());
                continue;
            }
            app.show_central(CentralView::ChannelRack);
            app.workspace.toggle_fold(Pane::Arrangement);
            let Some((t, pad)) = hats(&app) else {
                eprintln!("the demo song has no kit");
                return;
            };
            let clip = app
                .project()
                .track(t)
                .and_then(|x| x.clips.first().map(|c| c.id));
            app.selected_track = Some(t);
            app.selected_clip = clip;
            if scene == Scene::Own {
                let free = app.project().first_free_strip();
                app.set_pad_track(t, pad, free);
            }
            let points = egui::vec2(size.0 / zoom, size.1 / zoom);
            let path = dir.join(format!("{name}.png"));
            if scene == Scene::List {
                // One run (a run's textures are its own): the frames lay the window out,
                // the well is read from the rack as drawn, then clicked.
                let well = std::rc::Rc::new(std::cell::Cell::new(None::<egui::Pos2>));
                let seen = well.clone();
                raster::shoot_events(
                    &ctx,
                    points,
                    zoom,
                    14,
                    Some(&path),
                    |frame, _| {
                        let Some(at) = seen.get() else {
                            return Vec::new();
                        };
                        let press = |pressed| egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        };
                        match frame {
                            8 => vec![egui::Event::PointerMoved(at)],
                            9 => vec![press(true)],
                            10 => vec![press(false)],
                            _ => Vec::new(),
                        }
                    },
                    |root| {
                        app.frame(root);
                        // Where the well is now (the layout settles over the first frames).
                        if let Some(w) = app.rack.pad_wells.iter().find(|w| w.0 == t && w.1 == pad)
                        {
                            well.set(Some(w.2.center()));
                        }
                    },
                );
            } else {
                raster::shoot(&ctx, points, zoom, 8, Some(&path), |root| app.frame(root));
            }
            println!("{}", path.display());
        }
    }
}
