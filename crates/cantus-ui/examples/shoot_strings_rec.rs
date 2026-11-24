//! r42:strings-rec — shots of the piano roll's articulation lane (ADR 0158): a cello line
//! whose notes carry their articulations and bows (a legato pair, détaché down and up,
//! staccato, spiccato, pizzicato, tremolo, one left to the instrument), the marks in the
//! lane under the roll, and the note's menu open; and Exemplar's zone card on its *Playing*
//! page (a zone's articulation, bow, keyswitch and mod-wheel crossfade); in Night and Paper,
//! English and Spanish, at 100 and 200 %:
//! `cargo run -p cantus-ui --example shoot_strings_rec -- <dir> [filter…]`.

use cantus_core::note_art::{Articulation, Bow, NoteArt};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::CantusApp;
use egui::vec2;
use std::path::PathBuf;

#[path = "shots_common/studio.rs"]
mod studio;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for what in ["lane", "menu", "zone"] {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [1.0f32, 2.0] {
                    let name = format!(
                        "art-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let path = dir.join(format!("{name}.png"));
                    if what == "zone" {
                        shoot_zone(&path, theme, scale);
                    } else {
                        shoot(&path, what, theme, scale);
                    }
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}

/// A cello line, each note with how it is played.
pub fn project() -> Project {
    let mut p = Project::new("Strings");
    p.tempo_bpm = 72.0;
    let mut t = p.make_track("Cello");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler));
    let mut c = p.make_clip("Line", Ticks::ZERO, Ticks::beats(8));
    let a = |art, bow| {
        Some(NoteArt {
            articulation: art,
            bow,
        })
    };
    use Articulation::*;
    let line: &[(f64, f64, u8, Option<NoteArt>)] = &[
        (0.0, 1.0, 50, a(Sustain, Some(Bow::Down))),
        (1.0, 1.0, 53, a(Legato, None)),
        (2.0, 0.5, 57, a(Sustain, Some(Bow::Up))),
        (2.5, 0.5, 55, a(Sustain, Some(Bow::Down))),
        (3.0, 0.25, 53, a(Staccato, None)),
        (3.25, 0.25, 52, a(Staccato, None)),
        (3.5, 0.5, 50, a(Spiccato, Some(Bow::Up))),
        (4.0, 1.0, 45, a(Pizzicato, None)),
        (5.0, 1.0, 48, None),
        (6.0, 2.0, 50, a(Tremolo, None)),
    ];
    for &(at, len, key, art) in line {
        let mut n = p.make_note(
            Ticks::from_beats_f64(at),
            Ticks::from_beats_f64(len),
            key,
            90,
        );
        n.art = art;
        c.notes.push(n);
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn shoot(path: &std::path::Path, what: &str, theme: &str, scale: f32) {
    use cantus_ui::kit::appearance::Mode;
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let th = if theme == "paper" {
        cantus_ui::theme::default_light_theme()
    } else {
        cantus_ui::theme::default_theme()
    };
    cantus_ui::theme::apply(&th, &ctx);
    let mut app = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-strings-rec");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let (preset, mode) = if theme == "paper" {
        (cantus_ui::kit::appearance::DEFAULT_LIGHT, Mode::Light)
    } else {
        (cantus_ui::kit::appearance::DEFAULT_DARK, Mode::Dark)
    };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    studio::studio(&mut app, 256);
    let t = app.project().tracks[0].id;
    let c = app.project().tracks[0].clips[0].id;
    app.select_clip(t, c);
    app.focus_piano_roll();
    app.selected_notes = app.project().tracks[0].clips[0].notes[4..6]
        .iter()
        .map(|n| n.id)
        .collect();
    app.piano_roll.lanes.art = true;
    app.piano_roll.vel_h = 190.0;
    let mut frame = 0;
    let note = app.project().tracks[0].clips[0].notes[4].id;
    cantus_ui::raster::shoot(&ctx, vec2(1200.0, 720.0), scale, 8, Some(path), |root| {
        frame += 1;
        if frame == 2 {
            cantus_ui::views::piano_roll::fit_clip(&mut app, t, c);
        }
        // The note's menu, as a click on the fifth note's mark opens it.
        if what == "menu" && frame == 4 {
            cantus_ui::views::piano_roll::art::open_menu_for(
                root.ctx(),
                note,
                egui::pos2(520.0, 560.0),
            );
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| cantus_ui::views::piano_roll::show(&mut app, ui));
    });
}

/// Exemplar's Edit view with a strings zone selected and its card on the *Playing* page: a
/// sustain, down-bow, on the sustain keyswitch, fading out across the mod wheel's lower half.
fn shoot_zone(path: &std::path::Path, theme: &str, scale: f32) {
    use cantus_core::inst::sampler::zf;
    use cantus_ui::kit::appearance::Mode;
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let mut app = CantusApp::from_parts(
        Some(cantus_ui::welcome_project()),
        EngineHandle::dummy(),
        false,
    );
    let tmp = std::env::temp_dir().join("cantus-shoot-strings-rec");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if theme == "paper" {
        Mode::Light
    } else {
        Mode::Dark
    };
    app.attach(&ctx);
    app.demo_editor("sampler");
    let track = app.inst.open.expect("the editor is open");
    let spec = app.native_of(track).expect("an Exemplar");
    cantus_ui::views::inst::sampler::set_zone_fields(
        &mut app,
        track,
        &spec,
        0,
        &[
            (zf::ART, 1.0),
            (zf::BOW, 1.0),
            (zf::SW, 24.0),
            (zf::XFCC, 1.0),
            (zf::XFOUTLO, 0.0),
            (zf::XFOUTHI, 64.0),
        ],
        false,
    );
    app.inst.sampler.selected = Some(0);
    app.inst.sampler.zone_page = 1;
    app.inst.view = cantus_ui::views::inst::View::Edit;
    cantus_ui::raster::shoot(&ctx, vec2(1500.0, 1100.0), scale, 8, Some(path), |root| {
        app.frame(root)
    });
}
