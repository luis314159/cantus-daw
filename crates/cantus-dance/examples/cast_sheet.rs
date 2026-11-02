//! Contact sheets of the cast, to look at a character without the application:
//! `cargo run -p cantus-dance --example cast_sheet -- <out-dir> [cell] [character-id…]`.
//!
//! For each character (all of them when none is named) it writes:
//! - `<id>-moves.png`: every move (a row each, in `Move::ALL` order) at phases 0, ¼, ½, ¾
//!   and on its next cycle (the other side), over a checker (transparent);
//! - `<id>-moods.png`: every mood (in `Mood::ALL` order), standing and mid-bounce, on
//!   the checker, a dark stage (Cantus Night), Slate and Paper;
//! - `<id>-sizes.png`: the same pose at 48, 96 and 480 pixels tall.
//!
//! `cast_sheet one <out.png> <id> <move-index> <phase> <mood-index> [px] [cycle] [turn]`
//! draws one pose large, to look closely.

#[path = "common/sheet.rs"]
mod sheet_common;

use cantus_dance::{cast, Character};
use sheet_common::*;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("one") {
        return one(&args[2..]);
    }
    let out = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("cast-sheets"));
    let cell: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(220);
    let wanted: Vec<String> = args.iter().skip(3).cloned().collect();
    std::fs::create_dir_all(&out).expect("the output folder");
    for ch in cast::builtin() {
        let id = ch.info().id.trim_start_matches("builtin:").to_string();
        if !wanted.is_empty() && !wanted.contains(&id) {
            continue;
        }
        let ch: &dyn Character = ch.as_ref();
        moves_sheet(ch, cell, &[Bg::Checker])
            .save_png(out.join(format!("{id}-moves.png")))
            .expect("a PNG");
        moods_sheet(ch, cell, &BACKGROUNDS)
            .save_png(out.join(format!("{id}-moods.png")))
            .expect("a PNG");
        sizes_sheet(ch, &[48, 96, 480])
            .save_png(out.join(format!("{id}-sizes.png")))
            .expect("a PNG");
        println!("{id}: sheets in {}", out.display());
    }
}

fn one(a: &[String]) {
    let f = |i: usize, d: f32| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let id = a.get(1).cloned().unwrap_or_else(|| "songbird".into());
    let ch = cast::by_id(&format!("builtin:{id}")).expect("a character");
    let mv = cantus_dance::Move::from_index(f(2, 1.0) as usize);
    let mood = cantus_dance::Mood::from_index(f(4, 1.0) as usize);
    let px = f(5, 600.0) as u32;
    let mut pose = pose_of(ch.as_ref(), mv, f(3, 0.0), f(6, 0.0) as u32, mood);
    pose.turn = f(7, 0.0);
    let mut sheet = tiny_skia::Pixmap::new(px, px).expect("a stage");
    cell(&mut sheet, ch.as_ref(), &pose, Bg::Checker, 0, 0, px, px);
    sheet.save_png(&a[0]).expect("a PNG");
}
