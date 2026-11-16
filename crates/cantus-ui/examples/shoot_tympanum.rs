//! r40:tymp-ui — Tympanum's views and Cantus's mixer with an instrument's strips grouped
//! (round 40 phase 3, design item 4), rendered headless for checkpoint D: the Kit view, the
//! Mixer view, and the desk with Drums (Tympanum, three mics on strips of their own) open
//! and Piano (Chorda, three perspectives) folded. Slate Night and Slate Paper, the window at
//! 1280 × 720 points, at 100 % and 150 %.
//!
//! `cargo run -p cantus-ui --example shoot_tympanum -- <out_dir> [--es] [--zoom <z>] [case…]`
//! (cases: `kit`, `mixer`, `desk`, `install`; `--es` in Spanish, `-es` in the names).
//!
//! The recorded kits the list shows are the manifest's Tympanum packs; until `tymp-engine`
//! adds them, the shot names Gogodze Phu II's kit (a pack the manifest has) as one, so the
//! list's recorded section is seen as it will be.

use cantus_core::inst_out::{OutputFamily, OutputId, OutputSlot};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use std::path::PathBuf;

/// The song the shots are taken on (the strips numbered as the design's sheets).
fn song() -> (Project, TrackId, TrackId) {
    let mut p = Project::new("Neon Tide");
    let names = [
        "Drums",
        "Kick In",
        "Kick Out",
        "Snare Top",
        "Bass",
        "Piano",
        "Hammers",
        "Player",
        "Room",
        "Guitar",
        "Vox",
        "Pad",
    ];
    for (i, n) in names.iter().enumerate() {
        if let Some(s) = p.strips.get_mut(i) {
            s.name = if [1, 2, 3, 6, 7, 8].contains(&i) {
                n.to_string()
            } else {
                String::new()
            };
        }
    }
    let strip = |p: &Project, i: usize| p.strips[i].id;
    let mut tracks = Vec::new();
    for (name, si, color) in [
        ("Drums", 0usize, 0usize),
        ("Bass", 4, 2),
        ("Piano", 5, 1),
        ("Guitar", 9, 5),
        ("Vox", 10, 4),
        ("Pad", 11, 6),
    ] {
        let mut t = p.make_track(name);
        t.strip = Some(strip(&p, si));
        t.color = Color::PALETTE[color];
        tracks.push(t);
    }
    // Drums: Tympanum, three mics on strips of their own that sum back into its strip.
    let mut kit = NativeSpec::new(InstrumentKind::AcousticDrums);
    for (id, name, fam, si) in [
        (0u16, "Kick In", OutputFamily::Kick, 1usize),
        (1, "Kick Out", OutputFamily::Kick, 2),
        (3, "Snare Top", OutputFamily::Snare, 3),
    ] {
        kit.outputs.push(OutputSlot {
            id: OutputId(id),
            name: name.into(),
            family: fam,
            strip: strip(&p, si),
        });
    }
    let drums_strip = strip(&p, 0);
    for i in [1usize, 2, 3] {
        p.strips[i].kind = cantus_core::vca::StripKind::Pad;
        p.strips[i].output = Some(drums_strip);
    }
    tracks[0].instrument = InstrumentSpec::Native(kit);
    // Piano: Chorda, its three perspectives on strips of their own.
    let mut piano = NativeSpec::new(InstrumentKind::Chorda);
    for (id, name, si) in [(0u16, "Hammers", 6usize), (1, "Player", 7), (3, "Room", 8)] {
        piano.outputs.push(OutputSlot {
            id: OutputId(id),
            name: name.into(),
            family: OutputFamily::Perspective,
            strip: strip(&p, si),
        });
    }
    let piano_strip = strip(&p, 5);
    for i in [6usize, 7, 8] {
        p.strips[i].kind = cantus_core::vca::StripKind::Pad;
        p.strips[i].output = Some(piano_strip);
    }
    tracks[2].instrument = InstrumentSpec::Native(piano);
    let (d, pi) = (tracks[0].id, tracks[2].id);
    p.tracks.extend(tracks);
    (p, d, pi)
}

/// The manifest with Gogodze Phu II's kit offered to Tympanum (see the module's note).
fn manifest() -> cantus_content::Manifest {
    let mut m = cantus_content::Manifest::bundled();
    let offered = m.packs.iter().any(|p| {
        p.instruments
            .iter()
            .any(|i| i.plays == cantus_content::manifest::Plays::Tympanum)
    });
    if let Some(pk) = m
        .packs
        .iter_mut()
        .find(|p| p.id == "gogodze-phu-vol-ii")
        .filter(|_| !offered)
    {
        pk.instruments
            .push(cantus_content::manifest::PackInstrument {
                id: "kit".into(),
                name: cantus_content::manifest::Text {
                    en: "Gogodze Phu II".into(),
                    es: "Gogodze Phu II".into(),
                },
                file: "kit.sfz".into(),
                shelf: Default::default(),
                layers: None,
                gain_db: 0.0,
                voices: None,
                plays: cantus_content::manifest::Plays::Tympanum,
            });
    }
    m
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-tympanum")
    });
    let rest: Vec<String> = args.collect();
    let es = rest.iter().any(|a| a == "--es");
    let zooms: Vec<f32> = match rest
        .iter()
        .position(|a| a == "--zoom")
        .and_then(|i| rest.get(i + 1))
        .and_then(|z| z.parse().ok())
    {
        Some(z) => vec![z],
        None => vec![1.0, 1.5],
    };
    let only: Vec<&String> = rest
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && (*i == 0 || rest[i - 1] != "--zoom"))
        .map(|(_, a)| a)
        .collect();
    cantus_ui::i18n::set_locale(if es {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    std::fs::create_dir_all(&dir).expect("create the output directory");
    // r40 integration — `CANTUS_SHOOT_PICTURES=photo`: the instruments' pictures by the
    // photographic renderer (Settings ▸ Appearance ▸ Instrument pictures ▸ Photographic).
    if std::env::var("CANTUS_SHOOT_PICTURES").as_deref() == Ok("photo") {
        cantus_ui::views::inst::chorda::scene::hybrid::register();
        cantus_ui::scene::backend::set_default(cantus_ui::scene::SceneBackend::PerPixel);
    }
    // The scenes built on the painting thread: a shot cannot wait for the worker.
    cantus_ui::scene::set_blocking(true);
    let tmp = std::env::temp_dir().join("cantus-shoot-tympanum");
    for case in ["kit", "mixer", "desk", "install"] {
        if !only.is_empty() && !only.iter().any(|o| o.as_str() == case) {
            continue;
        }
        // Round 40 polish — `TYMP_SHOT_THEMES=preset:dark,…` (e.g. `ivory:0,carbon:1`):
        // other themes than Slate Night and Paper.
        let themes: Vec<(String, bool)> = match std::env::var("TYMP_SHOT_THEMES") {
            Ok(list) => list
                .split(',')
                .filter_map(|t| t.split_once(':'))
                .map(|(p, d)| (p.to_string(), d == "1"))
                .collect(),
            Err(_) => vec![
                ("slate_night".to_string(), true),
                ("slate_paper".to_string(), false),
            ],
        };
        for (preset, dark) in themes.iter().map(|(p, d)| (p.as_str(), *d)) {
            for &zoom in &zooms {
                let theme = preset.trim_start_matches("slate_");
                let mut name = format!("tympanum-{case}-{theme}-{:.0}", zoom * 100.0);
                if es {
                    name.push_str("-es");
                }
                let ctx = egui::Context::default();
                cantus_ui::design::install_fonts(&ctx);
                cantus_ui::design::install_style(&ctx);
                let (project, drums, piano) = song();
                let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
                app.settings_path = tmp.join("settings.json");
                app.layouts_path = tmp.join("layouts.json");
                app.keymap_path = tmp.join("keymap.toml");
                app.appearance_path = tmp.join("appearance.json");
                app.samples.content_root = Some(tmp.join("content"));
                app.samples.manifest = Some(manifest());
                app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
                let mode = app.appearance.mode;
                app.appearance.assign(mode, format!("preset:{preset}"));
                app.attach(&ctx);
                let path = dir.join(format!("{name}.png"));
                let size = egui::vec2(1280.0, 720.0);
                match case {
                    "desk" => {
                        app.show_central(CentralView::Mixer);
                        app.workspace
                            .toggle_fold(cantus_ui::views::workspace::Pane::Arrangement);
                        cantus_ui::views::mixer::set_width_mode(
                            &mut app,
                            cantus_ui::views::mixer::layout::WidthMode::Wide,
                        );
                        app.select_track(drums);
                        cantus_ui::views::mixer::group::set_folded(&mut app, piano, true);
                        raster::shoot(&ctx, size, zoom, 8, Some(&path), |root| app.frame(root));
                    }
                    _ => {
                        cantus_ui::views::inst::demo_setup(
                            &mut app,
                            drums,
                            InstrumentKind::AcousticDrums,
                        );
                        // r40:drums-hyb — `TYMP_SHOT_KIT=model:jazz`, `TYMP_SHOT_VOICE=hybrid`:
                        // the kits list with a kit's Generated · Hybrid.
                        for (var, key) in [
                            ("TYMP_SHOT_KIT", cantus_core::inst::tympanum::KIT_KEY),
                            ("TYMP_SHOT_VOICE", cantus_core::inst::tympanum::VOICE_KEY),
                            // Round 40 polish — `TYMP_SHOT_ROOM=studio`: the kit in a room.
                            ("TYMP_SHOT_ROOM", cantus_core::inst::room_look::ROOM_KEY),
                        ] {
                            if let Ok(v) = std::env::var(var) {
                                app.set_inst_text(drums, key, Some(v));
                            }
                        }
                        // r40:fix-f — a kit installing from a file: the box shows its
                        // progress (the job is only named; nothing runs).
                        if case == "install" {
                            app.samples
                                .user_installs
                                .push(cantus_ui::samples::UserInstall {
                                    id: "user-drskit-basic-xml-00000000".into(),
                                    track: drums,
                                    plays: cantus_content::manifest::Plays::Tympanum,
                                    name: "DRSKit_basic.xml".into(),
                                    size: 2_803_397_710,
                                });
                        }
                        app.open_inst_editor(drums);
                        app.inst.view = if case == "kit" || case == "install" {
                            cantus_ui::views::inst::View::Play
                        } else {
                            cantus_ui::views::inst::View::Mixer
                        };
                        // Round 40 polish — `TYMP_SHOT_ROOM_OPEN=1`: the room card open
                        // from its chip.
                        if std::env::var("TYMP_SHOT_ROOM_OPEN").as_deref() == Ok("1") {
                            use cantus_ui::views::inst::tympanum;
                            let mut lk = tympanum::look(&ctx, drums);
                            lk.room_open = true;
                            tympanum::keep(&ctx, drums, lk);
                        }
                        raster::shoot(&ctx, size, zoom, 6, Some(&path), |ui| {
                            cantus_ui::views::inst::window_in_sized(&mut app, ui, size)
                        });
                    }
                }
                println!("{}", path.display());
            }
        }
    }
}
