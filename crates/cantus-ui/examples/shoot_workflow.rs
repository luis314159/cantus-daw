//! Screenshots of round 31's workflow (ADR 0072) without a screen: the command palette
//! (opened, and narrowed by a word), the undo history beside the inspector (the current
//! step in the middle, the saved one tagged), the start page (the starters, a template of
//! the user's, the recent projects), the name a template is saved under, and the device
//! picker's *Track templates* shelf.
//!
//! `cargo run -p cantus-ui --example shoot_workflow -- <out-dir> [es] [light] [z1.5] [kind=…]`

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::keymap::Action;
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

/// The beat starter with a lead and a history of a session's edits.
fn app(ctx: &egui::Context, light: bool, files: &std::path::Path) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut a = CantusApp::from_parts(Some(Starter::Beat.build()), EngineHandle::dummy(), false);
    a.settings_path = files.join("settings.json");
    a.layouts_path = files.join("layouts.json");
    a.keymap_path = files.join("keymap.toml");
    a.appearance_path = files.join("appearance.json");
    a.workflow.wire_at(files, files);
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

/// A session's steps: notes moved, an insert, a track renamed, saved, more edits, two
/// undone.
fn session(a: &mut CantusApp, dir: &std::path::Path) {
    let keys = a.project().tracks[2].id;
    let strip = a.project().tracks[2].strip.unwrap();
    let clip = a.project().tracks[2].clips[0].clone();
    let moved: Vec<Note> = clip
        .notes
        .iter()
        .take(3)
        .map(|n| {
            let mut n = n.clone();
            n.start += Ticks::EIGHTH;
            n
        })
        .collect();
    a.exec(Command::Batch {
        label_key: "cmd.nudge".into(),
        commands: vec![Command::SetNotes {
            track: keys,
            clip: clip.id,
            notes: moved,
        }],
    });
    let slot = a.bus.new_effect(EffectKind::Compressor);
    a.exec(Command::InsertEffect {
        strip,
        index: 0,
        slot: Box::new(slot),
    });
    a.exec(Command::SetTrackName {
        id: keys,
        name: "Lead".into(),
    });
    a.save_to_path(&dir.join("Night drive.cantus"));
    a.exec(Command::SetTempo { bpm: 96.0 });
    let slot = a.bus.new_effect(EffectKind::Saturator);
    a.exec(Command::InsertEffect {
        strip,
        index: 1,
        slot: Box::new(slot),
    });
    a.exec(Command::SetTrackColor {
        id: keys,
        color: Color::rgb(220, 80, 120),
    });
    a.undo();
    a.undo();
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
    for kind in [
        "palette",
        "palette-filter",
        "history",
        "start",
        "name",
        "picker-templates",
        "launch",
    ] {
        if only.is_some_and(|o| o != kind) {
            continue;
        }
        // Never the user's files: a folder of this shot's own.
        let files = std::env::temp_dir().join(format!("cantus-shoot-workflow-{kind}{suffix}"));
        let _ = std::fs::remove_dir_all(&files);
        std::fs::create_dir_all(&files).expect("a folder");
        let ctx = context();
        let mut a = app(&ctx, light, &files);
        session(&mut a, &files);
        // A second recent project, and a template and a track template of the user's.
        let other = files.join("Sunday service.cantus");
        cantus_project::save(&Starter::Band.build(), &other, "shoot").expect("saved");
        a.workflow.recent.note(&other);
        a.workflow.recent.note(&files.join("Night drive.cantus"));
        let base = a.audio_base().map(|p| p.to_path_buf());
        let _ = a
            .workflow
            .store
            .save_project(a.project(), base.as_deref(), "Night drive kit");
        let lead = a.project().tracks[2].id;
        let _ = a.workflow.store.save_track(a.project(), lead, "Lead keys");
        let drums = a.project().tracks[0].id;
        let _ = a
            .workflow
            .store
            .save_track(a.project(), drums, "Boom bap kit");
        a.workflow.templates_changed();
        match kind {
            "palette" | "palette-filter" => a.run_action(Action::CommandPalette),
            "history" => a.show_pane(cantus_ui::views::workspace::Pane::History),
            "start" => a.run_action(Action::NewFromTemplate),
            // As Cantus starts: the start page beside the verse's card (ADR 0072).
            "launch" => {
                cantus_ui::workflow::open_at_start(&mut a, false);
                let r = cantus_ui::verse::references()[0];
                a.verse = Some(cantus_ui::views::verse::Card::new(r));
            }
            "name" => a.run_action(Action::SaveTrackTemplate),
            "picker-templates" => {
                a.open_picker(cantus_ui::picker::PickPurpose::AddInstrument { index: None });
                a.picker_view
                    .set_shelf(cantus_ui::picker_query::Shelf::Source(
                        cantus_ui::picker_catalog::Source::Template,
                    ));
            }
            _ => {}
        }
        if kind == "name" {
            a.selected_track = Some(lead);
            a.run_action(Action::SaveTrackTemplate);
        }
        a.set_status(String::new());
        let word = if spanish { "mez" } else { "mix" };
        let path = dir.join(format!("workflow-{kind}{suffix}.png"));
        shoot_events(
            &ctx,
            egui::vec2(W, H),
            zoom,
            14,
            Some(&path),
            |frame, _| {
                if kind == "palette-filter" && frame == 4 {
                    vec![egui::Event::Text(word.into())]
                } else {
                    Vec::new()
                }
            },
            |root| {
                let mut frame = eframe::Frame::_new_kittest();
                eframe::App::ui(&mut a, root, &mut frame);
            },
        );
        println!("wrote {}", path.display());
    }
}
