//! The cast on the themes (round 35, ADR 0090): for each included character (all of them
//! when none is named), drawn through the one renderer the stage, the companion window
//! and a video use:
//!
//! - `cast-<id>-moves.png`: every move (rows, in `Move::ALL` order) at phases 0, 1/4, 1/2,
//!   3/4 and the next cycle's 1/4 (the other side), transparent over a checker;
//! - `cast-<id>-themes.png`: every mood (columns, in `Mood::ALL` order) on a checker
//!   (transparent) and on the stage of every shipped theme (rows, in the Appearance
//!   page's order: Cantus Night, Cantus Paper, … Slate, Ink & Gold, Paper, Parchment);
//! - `cast-<id>-moods.png`: every mood standing and mid-bounce, larger;
//! - `cast-<id>-sizes.png`: one pose 48, 96 and 480 pixels tall.
//!
//! `cargo run -p cantus-ui --example shoot_cast [-- out_dir [id…]]` (default
//! `target/shoot-cast`).

#[path = "../../cantus-dance/examples/common/sheet.rs"]
mod sheet;

use cantus_dance::{cast, Character};
use sheet::*;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("target/shoot-cast"),
    );
    let wanted: Vec<String> = args.iter().skip(2).cloned().collect();
    std::fs::create_dir_all(&out).expect("the output folder");
    let mut stages = vec![Bg::Checker];
    for (_, theme) in cantus_ui::theme::PRESETS {
        let c = theme().palette().panel;
        stages.push(Bg::Solid([c.r(), c.g(), c.b(), 255]));
    }
    for ch in cast::builtin() {
        let id = ch.info().id.trim_start_matches("builtin:").to_string();
        if !wanted.is_empty() && !wanted.contains(&id) {
            continue;
        }
        let ch: &dyn Character = ch.as_ref();
        let save = |p: tiny_skia::Pixmap, what: &str| {
            p.save_png(out.join(format!("cast-{id}-{what}.png")))
                .expect("a PNG");
        };
        save(moves_sheet(ch, 200, &[Bg::Checker]), "moves");
        save(moods_rows_sheet(ch, 150, &stages), "themes");
        save(moods_sheet(ch, 240, &BACKGROUNDS), "moods");
        save(sizes_sheet(ch, &[48, 96, 480]), "sizes");
        println!("{id}: sheets in {}", out.display());
    }
}
