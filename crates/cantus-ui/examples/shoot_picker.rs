//! The device picker's design review (round 20, ADR 0029): `cargo run -p cantus-ui
//! --example shoot_picker -- <out-dir> [what…]`.
//!
//! - `thumbs`: every native device's real-look thumbnail, timed, one PNG each.
//! - `picker`: the picker over the whole window — instruments and effects, Slate and Paper,
//!   a search, the Spanish interface — with a few hosted plug-ins in the database.
//! - `panel`: the browsing panel docked in the workspace.

use cantus_engine::EngineHandle;
use cantus_host::{Format, PluginInfo, PluginKind};
use cantus_ui::picker::PickPurpose;
use cantus_ui::raster::{Canvas, Textures};
use cantus_ui::views::picker_snap::{self, Subject};
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn save(img: &egui::ColorImage, path: &Path) {
    let [w, h] = img.size;
    let Some(mut pm) = tiny_skia::Pixmap::new(w as u32, h as u32) else {
        return;
    };
    for (i, c) in img.pixels.iter().enumerate() {
        let d = pm.data_mut();
        d[i * 4..i * 4 + 4].copy_from_slice(&[c.r(), c.g(), c.b(), c.a()]);
    }
    let _ = pm.save_png(path);
}

fn subjects() -> Vec<(String, Subject)> {
    let mut v: Vec<(String, Subject)> = cantus_core::InstrumentKind::ALL
        .iter()
        .map(|k| (format!("{k:?}"), Subject::Instrument(*k)))
        .collect();
    v.push(("Pulsus".into(), Subject::Pulsus));
    for k in cantus_core::EffectKind::ALL {
        if let Some(s) = Subject::of_effect(k) {
            v.push((format!("{k:?}"), s));
        }
    }
    v
}

fn themed_ctx(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn thumbs(dir: &Path, dark: bool) {
    let ctx = themed_ctx(dark);
    for (name, s) in subjects() {
        let t0 = Instant::now();
        let Some(frame) = picker_snap::capture(&ctx, s, 0.8) else {
            println!("{name}: no editor");
            continue;
        };
        let t1 = Instant::now();
        let img = picker_snap::finish(frame, 416, 9.0);
        let t2 = Instant::now();
        println!(
            "{name}: capture {:.0} ms, finish {:.0} ms",
            (t1 - t0).as_secs_f64() * 1e3,
            (t2 - t1).as_secs_f64() * 1e3
        );
        let tag = if dark { "dark" } else { "light" };
        save(&img, &dir.join(format!("thumb-{name}-{tag}.png")));
    }
}

/// Every capture made inside another context's pass, as the app makes them.
fn nested(dark: bool) {
    let ctx = themed_ctx(dark);
    for round in 0..3 {
        for (name, s) in subjects() {
            eprintln!("round {round} {name} …");
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let f = picker_snap::capture(ui.ctx(), s, 0.8);
                eprintln!("  captured {}", f.is_some());
                if let Some(f) = f {
                    let img = picker_snap::finish(f, 416, 9.0);
                    eprintln!("  finished {:?}", img.size);
                }
            });
            out.textures_delta.clear();
        }
    }
}

fn hosted(format: Format, id: &str, name: &str, vendor: &str, kind: PluginKind, f: &[&str]) -> PluginInfo {
    PluginInfo {
        format,
        id: id.into(),
        name: name.into(),
        vendor: vendor.into(),
        version: "1.0".into(),
        path: PathBuf::from(format!("C:/Program Files/Common Files/CLAP/{name}.clap")),
        kind,
        main_inputs: if kind == PluginKind::Instrument { 0 } else { 2 },
        main_outputs: 2,
        sidechain: false,
        editor: true,
        features: f.iter().map(|s| s.to_string()).collect(),
    }
}

/// A plug-in database like a musician's: a few free instruments and effects.
fn database() -> cantus_host::PluginDb {
    use Format::*;
    use PluginKind::*;
    let list = [
        hosted(Clap, "org.surge-synth-team.surge-xt", "Surge XT", "Surge Synth Team", Instrument, &["instrument", "synthesizer"]),
        hosted(Vst3, "D0E1", "Dexed", "Digital Suburban", Instrument, &["Instrument|Synth"]),
        hosted(Clap, "com.sfztools.sfizz", "sfizz", "SFZTools", Instrument, &["instrument", "sampler"]),
        hosted(Vst3, "0B1D", "OB-Xd", "discoDSP", Instrument, &["Instrument|Synth"]),
        hosted(Clap, "org.odin2", "Odin 2", "The Wave Warden", Instrument, &["instrument", "synthesizer"]),
        hosted(Clap, "geonkick", "Geonkick", "Iurie Nistor", Instrument, &["instrument", "drum"]),
        hosted(Clap, "com.airwindows.consolidated", "Airwindows Consolidated", "Airwindows", Effect, &["audio-effect", "utility"]),
        hosted(Vst3, "0A11", "Dragonfly Hall Reverb", "Michael Willis", Effect, &["Fx|Reverb"]),
        hosted(Clap, "com.chowdsp.chowtapemodel", "CHOW Tape Model", "chowdsp", Effect, &["audio-effect", "distortion"]),
        hosted(Vst3, "0C0D", "TDR Nova", "Tokyo Dawn Labs", Effect, &["Fx|EQ"]),
        hosted(Clap, "org.lsp.compressor", "LSP Compressor Stereo", "LSP", Effect, &["audio-effect", "compressor"]),
        hosted(Vst3, "0D0E", "Valhalla Supermassive", "Valhalla DSP", Effect, &["Fx|Delay|Reverb"]),
    ];
    let mut db = cantus_host::PluginDb::default();
    for (i, p) in list.into_iter().enumerate() {
        db.put(cantus_host::DbEntry {
            path: PathBuf::from(format!("C:/plugins/{i}")),
            format: p.format,
            modified: 1,
            size: 1,
            status: cantus_host::ScanStatus::Ok,
            plugins: vec![p],
        });
    }
    db
}

/// Run the whole window until the picker's pictures are in, then write the last frame.
fn window(app: &mut CantusApp, ctx: &egui::Context, size: egui::Vec2, path: &Path, events: Vec<egui::Event>) {
    let mut textures = Textures::default();
    let start = Instant::now();
    let mut frame = 0;
    let o = loop {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(frame as f64 * 0.25),
            predicted_dt: 0.25,
            events: if frame == 2 { events.clone() } else { Vec::new() },
            ..Default::default()
        };
        let mut o = ctx.run_ui(input, |root| app.frame(root));
        if std::env::var_os("SHOOT_LOG").is_some() {
            eprintln!("frame {frame}: {:?}", app.picker_view.thumbs.stats);
        }
        textures.update(&o.textures_delta);
        o.textures_delta.clear();
        frame += 1;
        let done = app.picker_view.thumbs.idle() && frame > 6;
        if done || start.elapsed().as_secs() > 120 {
            break o;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    };
    let prims = ctx.tessellate(o.shapes, o.pixels_per_point);
    let mut canvas = Canvas::new(size.x as usize, size.y as usize, ctx.global_style().visuals.panel_fill);
    canvas.paint(&prims, &textures, o.pixels_per_point);
    let _ = canvas.write_png(path);
    println!("wrote {} ({frame} frames, {:?})", path.display(), app.picker_view.thumbs.stats);
}

fn picker(dir: &Path, dark: bool, what: &str, spanish: bool) {
    use cantus_ui::kit::appearance::Mode;
    let locale = if spanish { cantus_ui::i18n::Locale::Es } else { cantus_ui::i18n::Locale::En };
    cantus_ui::i18n::set_locale(locale);
    eprintln!("picker {what} dark={dark} es={spanish}");
    let ctx = themed_ctx(dark);
    let mut app = CantusApp::from_parts(Some(cantus_ui::demos::startup_project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-picker");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    app.attach(&ctx);
    app.plugins.db = database();
    app.picker_view.thumbs.enable(Some(tmp.join("thumbs")));
    app.picker_view.prefs.favourites = vec!["cantus:unda".into(), "cantus:culmen".into()];
    app.picker_view.prefs.recent = vec!["cantus:lamina".into(), "cantus:comprimo".into()];
    let strip = app.project().tracks.get(5).and_then(|t| t.strip).unwrap_or(cantus_core::StripId::MASTER);
    let track = app.project().tracks[0].id;
    let mut events = Vec::new();
    match what {
        "instruments" => app.open_picker(PickPurpose::AddInstrument { index: None }),
        "replace" => app.open_picker(PickPurpose::ReplaceInstrument { track }),
        "effects" => app.open_picker(PickPurpose::AddEffect { strip }),
        "panel" => {
            app.toggle_pane(cantus_ui::views::workspace::Pane::Devices);
            app.picker_view.panel.selected = Some("cantus:calor".into());
        }
        "search" => {
            app.open_picker(PickPurpose::AddEffect { strip });
            events.push(egui::Event::Text("verb".into()));
        }
        _ => {}
    }
    eprintln!("opened: {:?}", app.picker);
    let tag = format!("picker-{what}-{}{}", if dark { "dark" } else { "light" }, if spanish { "-es" } else { "" });
    window(&mut app, &ctx, egui::vec2(1920.0, 1080.0), &dir.join(format!("{tag}.png")), events);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let what: Vec<String> = args.collect();
    let all = what.is_empty();
    let wants = |w: &str| all || what.iter().any(|x| x == w);
    if what.iter().any(|w| w == "nested") {
        nested(true);
        nested(false);
        eprintln!("nested: all done");
        return;
    }
    if wants("thumbs") {
        thumbs(&dir, true);
        thumbs(&dir, false);
    }
    // One picture per run: run once per view and theme (`instruments dark`, `effects
    // light es`…); a second whole window in the same process is not reliable.
    let es = what.iter().any(|x| x == "es");
    for w in ["instruments", "effects", "search", "replace", "panel"] {
        if what.iter().any(|x| x == w) {
            if !what.iter().any(|x| x == "light") {
                picker(&dir, true, w, es);
            }
            if !what.iter().any(|x| x == "dark") {
                picker(&dir, false, w, es);
            }
        }
    }
}
