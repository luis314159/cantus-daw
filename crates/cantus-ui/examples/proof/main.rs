//! The proof render of ADR 0024 Phase A: mock screens drawn **with the kit only** — not
//! the real views — for the user to approve the look before anything is migrated.
//!
//! `cargo run -p cantus-ui --example proof [--gallery] [--release]`
//!
//! Writes `target/proof/*.png`: the mixer desk, the channel inspector, one device in
//! the shell, the splash and the logo sheet, in Slate and Paper at 1× and 2×; with
//! `--gallery`, every part's sheet as well.

mod common;
mod device;
mod eq;
mod inspector;
mod mixer;
mod screens;
mod splash;
mod variations;
use screens as proof_screens;

use cantus_ui::kit::gallery::{self, PARTS};
use cantus_ui::kit::{Style, Tokens};
use std::path::PathBuf;

fn out_dir() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let dir = root.join("target").join("proof");
    std::fs::create_dir_all(&dir).expect("create target/proof");
    dir
}

fn write(dir: &std::path::Path, name: &str, canvas: &cantus_ui::raster::Canvas) {
    let path = dir.join(format!("{name}.png"));
    let bytes = gallery::encode_png(canvas).expect("encode png");
    std::fs::write(&path, bytes).expect("write png");
    println!("{}", path.display());
}

fn main() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = out_dir();
    let themes = [("slate", Tokens::slate()), ("paper", Tokens::paper())];
    if args.iter().any(|a| a == "--gallery") {
        for (tn, t) in themes {
            let s = Style::new(t);
            for (name, part) in PARTS {
                let c = gallery::render_sheet(&s, part, 2.0);
                write(&dir, &format!("gallery-{name}-{tn}"), &c);
            }
        }
        return;
    }
    let only: Vec<&str> = args
        .iter()
        .map(|s| s.as_str())
        .filter(|a| !a.starts_with("--"))
        .collect();
    let wanted = |name: &str| only.is_empty() || only.iter().any(|o| name.starts_with(o));
    if wanted("f-variations") {
        for ppp in [1.0f32, 2.0] {
            let png = variations::variations(ppp);
            let path = dir.join(format!("f-variations-{}x.png", ppp as u32));
            std::fs::write(&path, png).expect("write png");
            println!("{}", path.display());
        }
    }
    if wanted("g-inspect") {
        for (tn, t) in themes {
            for ppp in [1.0f32, 2.0] {
                let c = variations::inspect_demo(ppp, t);
                write(&dir, &format!("g-inspect-{tn}-{}x", ppp as u32), &c);
            }
        }
    }
    for (tn, t) in themes {
        // The shell reads the installed tokens, as a widget does.
        cantus_ui::kit::tokens::install(t);
        let s = Style::new(t);
        for ppp in [1.0f32, 2.0] {
            for (screen, size, paint) in proof_screens::SCREENS {
                if wanted(screen) {
                    let c = gallery::render(size, ppp, |b, p| paint(b, p, &s, size));
                    write(&dir, &format!("{screen}-{tn}-{}x", ppp as u32), &c);
                }
            }
            for (screen, size, paint) in proof_screens::UI_SCREENS {
                if wanted(screen) {
                    let c = gallery::render_ui(size, ppp, |ui| paint(ui, &s));
                    write(&dir, &format!("{screen}-{tn}-{}x", ppp as u32), &c);
                }
            }
        }
    }
}
