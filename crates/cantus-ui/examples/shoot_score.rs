//! The score's golden passages engraved and drawn without a screen (round 32, ADR 0075):
//! each as the Score pane wraps it, in Paper and Slate at 100 % and 200 %, and the first
//! page of each as the PDF lays it out (A4, black on white).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_score -- [out-dir] [passage]
//! ```

use cantus_score::build::{build, Song};
use cantus_score::layout::{engrave, Format, LayoutOptions, Paper};
use cantus_score::text::TextMeasure;
use cantus_ui::raster::shoot;
use cantus_ui::views::score_paint::{paint, Placement, ScoreInk};
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

/// A project holding a passage as one clip on one track (the song's tempo map and key).
fn project_of(p: &cantus_score::samples::Passage) -> cantus_core::Project {
    let mut pr = cantus_core::Project::new(p.title);
    pr.key = p.key;
    pr.tempo_bpm = p.tempo.bpm_at(0.0);
    pr.time_signature = p.tempo.signature_at(cantus_core::Ticks(0));
    pr.tempo_points = p.tempo.points().to_vec();
    pr.metre_changes = p.tempo.metres().to_vec();
    let part = &p.parts[0];
    let mut t = pr.make_track(part.name.clone());
    t.score = part.settings.clone();
    let end = part
        .notes
        .iter()
        .map(|n| n.start + n.len)
        .max()
        .unwrap_or(3840);
    let end = pr.tempo_map().bar_end(cantus_core::Ticks(end - 1));
    let mut c = pr.make_clip(p.title, cantus_core::Ticks(0), end);
    for n in &part.notes {
        let note = pr.make_note(
            cantus_core::Ticks(n.start),
            cantus_core::Ticks(n.len),
            n.pitch,
            n.velocity,
        );
        c.notes.push(note);
    }
    t.clips.push(c);
    pr.tracks.push(t);
    pr
}

/// The piano roll with its staff, and the Score pane, as the application draws them.
fn app_shots(out: &std::path::Path, p: &cantus_score::samples::Passage) {
    for (dark, theme) in [(false, "paper"), (true, "slate")] {
        for zoom in [1.0f32, 2.0] {
            let ctx = context(dark);
            let mut app = cantus_ui::CantusApp::from_parts(
                Some(project_of(p)),
                cantus_engine::EngineHandle::dummy(),
                false,
            );
            let t = app.project().tracks.last().unwrap().id;
            let c = app.project().tracks.last().unwrap().clips[0].id;
            app.selected_track = Some(t);
            app.selected_clip = Some(c);
            let first = app.project().tracks.last().unwrap().clips[0].notes[2].id;
            app.selected_notes = vec![first];
            app.score.split_open = true;
            app.score.split_h = 230.0;
            let path = out.join(format!(
                "{}-roll-{theme}-{}.png",
                p.name,
                (zoom * 100.0) as u32
            ));
            shoot(
                &ctx,
                egui::vec2(1400.0, 760.0),
                zoom,
                3,
                Some(&path),
                |ui| {
                    cantus_ui::views::piano_roll::show(&mut app, ui);
                },
            );
            println!("{}", path.display());
            // A context of its own: the textures one run uploads are kept by that run only.
            let ctx = context(dark);
            let path = out.join(format!(
                "{}-pane-{theme}-{}.png",
                p.name,
                (zoom * 100.0) as u32
            ));
            shoot(
                &ctx,
                egui::vec2(1100.0, 520.0),
                zoom,
                3,
                Some(&path),
                |ui| {
                    cantus_ui::views::score_view::show(&mut app, ui);
                },
            );
            println!("{}", path.display());
        }
    }
}

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/shoot-score".into()),
    );
    let only = std::env::args().nth(2);
    std::fs::create_dir_all(&out).expect("out dir");
    let text = TextMeasure::new(Some(cantus_ui::design::score_faces()));
    for p in cantus_score::samples::all() {
        if only.as_deref().is_some_and(|o| o != p.name) {
            continue;
        }
        let song = Song {
            title: p.title.into(),
            composer: "Cantus".into(),
            tempo: &p.tempo,
            key: p.key,
        };
        let score = build(&song, &p.parts, None);
        if matches!(p.name, "piano" | "melody") {
            app_shots(&out, &p);
        }
        // The pane: systems wrapped to its width.
        for (dark, theme) in [(false, "paper"), (true, "slate")] {
            for zoom in [1.0f32, 2.0] {
                let space = 7.0;
                let width_pt = 900.0;
                let opts = LayoutOptions {
                    format: Format::Wrap {
                        width: width_pt / space,
                    },
                    text: text.clone(),
                    header: false,
                };
                let e = engrave(&score, &opts);
                let page = &e.pages[0];
                let h = (page.list.height * space).max(120.0) + 20.0;
                let ctx = context(dark);
                let path = out.join(format!("{}-{theme}-{}.png", p.name, (zoom * 100.0) as u32));
                shoot(
                    &ctx,
                    egui::vec2(width_pt + 20.0, h),
                    zoom,
                    2,
                    Some(&path),
                    |ui| {
                        let s = cantus_ui::kit::Style::current();
                        let rect = ui.max_rect();
                        ui.painter().rect_filled(rect, 0.0, s.t.well);
                        let pl = Placement {
                            origin: rect.min + egui::vec2(10.0, 10.0),
                            space,
                        };
                        let ink = ScoreInk {
                            ink: s.t.text,
                            selected: s.t.selection,
                        };
                        paint(ui.painter(), &page.list, &pl, &ink, &|_| false, rect);
                    },
                );
                println!("{}", path.display());
            }
        }
        // The page: A4, as printed.
        let opts = LayoutOptions {
            format: Format::Pages(Paper::A4),
            text: text.clone(),
            header: true,
        };
        let e = engrave(&score, &opts);
        let page = &e.pages[0];
        let space = 5.0;
        let size = egui::vec2(page.list.width * space, page.list.height * space);
        let ctx = context(false);
        let path = out.join(format!("{}-page.png", p.name));
        shoot(&ctx, size, 1.5, 2, Some(&path), |ui| {
            let rect = ui.max_rect();
            ui.painter().rect_filled(rect, 0.0, egui::Color32::WHITE);
            let pl = Placement {
                origin: rect.min,
                space,
            };
            let ink = ScoreInk {
                ink: egui::Color32::BLACK,
                selected: egui::Color32::BLACK,
            };
            paint(ui.painter(), &page.list, &pl, &ink, &|_| false, rect);
        });
        println!("{}", path.display());
    }
}
