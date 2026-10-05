//! The device picker's design review (round 20, ADR 0029): `cargo run -p cantus-ui
//! --example shoot_picker -- <out-dir> [what…]`.
//!
//! - `thumbs`: every native device's real-look thumbnail, timed, one PNG each.

use cantus_ui::views::picker_snap::{self, Subject};
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

fn thumbs(dir: &Path, dark: bool) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    for (name, s) in subjects() {
        let t0 = Instant::now();
        let Some(frame) = picker_snap::capture(&ctx, s, 0.8) else {
            println!("{name}: no editor");
            continue;
        };
        let t1 = Instant::now();
        let img = picker_snap::finish(frame, 416);
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

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let what: Vec<String> = args.collect();
    if what.is_empty() || what.iter().any(|w| w == "thumbs") {
        thumbs(&dir, true);
        thumbs(&dir, false);
    }
}
