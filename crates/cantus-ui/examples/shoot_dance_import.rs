//! Screenshots of round 35's import card (ADR 0089) without a screen: a Lottie figure (its
//! licence stated), an Aseprite sheet (no licence: the choices), a DragonBones skeleton,
//! *Edit moves…* of a character already in a project, and a file that could not be read.
//!
//! `cargo run -p cantus-ui --example shoot_dance_import -- <out-dir> [es] [light] [z1.5] [kind=…]`

use cantus_dance::import::samples;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::templates::Starter;
use cantus_ui::CantusApp;
use std::path::PathBuf;

const W: f32 = 1400.0;
const H: f32 = 820.0;

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

fn app(ctx: &egui::Context, light: bool, files: &std::path::Path) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut a = CantusApp::from_parts(Some(Starter::Beat.build()), EngineHandle::dummy(), false);
    a.settings_path = files.join("settings.json");
    a.layouts_path = files.join("layouts.json");
    a.keymap_path = files.join("keymap.toml");
    a.appearance_path = files.join("appearance.json");
    a.appearance.mode = if light { Mode::Light } else { Mode::Dark };
    let preset = if light {
        "cantus_paper"
    } else {
        "cantus_night"
    };
    a.appearance
        .assign(a.appearance.mode, format!("preset:{preset}"));
    a.attach(ctx);
    a.focus_arrangement();
    a
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "shots".into()));
    let spanish = args.iter().any(|a| a == "es");
    let light = args.iter().any(|a| a == "light");
    let zoom: f32 = args
        .iter()
        .find_map(|a| a.strip_prefix('z').and_then(|z| z.parse().ok()))
        .unwrap_or(1.0);
    let only: Option<&str> = args
        .iter()
        .find_map(|a| a.strip_prefix("kind=").map(|k| k as &str));
    std::fs::create_dir_all(&dir).expect("output dir");
    let (locale, mut suffix) = if spanish {
        (cantus_ui::i18n::Locale::Es, "-es".to_string())
    } else {
        (cantus_ui::i18n::Locale::En, String::new())
    };
    if light {
        suffix.push_str("-paper");
    }
    if zoom != 1.0 {
        suffix.push_str(&format!("-z{zoom}"));
    }
    cantus_ui::i18n::set_locale(locale);
    for kind in ["lottie", "sheet", "dragonbones", "edit", "failed"] {
        if only.is_some_and(|o| o != kind) {
            continue;
        }
        let files = std::env::temp_dir().join(format!("cantus-shoot-dance-import-{kind}{suffix}"));
        let _ = std::fs::remove_dir_all(&files);
        std::fs::create_dir_all(&files).expect("a folder");
        let ctx = context();
        let mut a = app(&ctx, light, &files);
        let track = a.project().tracks[0].id;
        let src = files.join("source");
        let path = match kind {
            "lottie" => {
                samples::write(&src, &samples::dotlottie("Sunny")).expect("written");
                src.join("Sunny.lottie")
            }
            "sheet" => {
                samples::write(&src, &samples::aseprite("pixel-pal")).expect("written");
                src.join("pixel-pal.json")
            }
            "dragonbones" | "edit" => {
                samples::write(&src, &samples::dragonbones("Bones")).expect("written");
                std::fs::write(
                    src.join("LICENSE.txt"),
                    "Creative Commons Attribution 4.0 International\n",
                )
                .expect("written");
                src.join("Bones_ske.json")
            }
            _ => {
                std::fs::create_dir_all(&src).expect("a folder");
                let broken = br#"{"layers": [], "v": "5", "fr": 30, "ip": 0, "op": 0}"#;
                std::fs::write(src.join("broken.json"), broken).expect("written");
                src.join("broken.json")
            }
        };
        let edit = if kind == "edit" {
            // Already in a project: its copy, and the card to edit its moves.
            let project = files.join("project");
            let found = cantus_dance::import::inspect(&path).expect("read");
            let settings = cantus_dance::import::guessed(&found);
            let licence = found.licence.clone();
            let rel = cantus_dance::import::package::copy_into(
                &project, &path, &found, &settings, &licence,
            )
            .expect("copied");
            Some((project.join(&rel), rel))
        } else {
            None
        };
        match edit {
            Some((copy, rel)) => cantus_ui::dance_import::begin(&mut a, track, copy, Some(rel)),
            None => cantus_ui::dance_import::begin(&mut a, track, path, None),
        }
        cantus_ui::dance_import::poll(&mut a, true);
        if kind == "sheet" {
            if let Some(c) = a.dance_import.card.as_mut() {
                c.choice = Some(2);
            }
        }
        a.set_status(String::new());
        let out = dir.join(format!("dance-import-{kind}{suffix}.png"));
        shoot_events(
            &ctx,
            egui::vec2(W, H),
            zoom,
            40,
            Some(&out),
            |_, _| Vec::new(),
            |root| {
                let mut frame = eframe::Frame::_new_kittest();
                eframe::App::ui(&mut a, root, &mut frame);
            },
        );
        println!("wrote {}", out.display());
    }
}
