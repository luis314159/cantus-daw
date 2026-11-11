//! The control families as the kit draws them (round 39, `el-controls`), laid out like
//! `des-controls`' build sheets so the two can be compared side by side: knobs, faders and
//! meters, fields and drop-downs, menus, switches, buttons and dialogs, the driven signs —
//! Slate Night and Paper, at 100 % and 150 %, through the CPU renderer.
//!
//! `cargo run -p cantus-ui --example shoot_controls -- <out_dir> [family…]`
//!
//! Writes `<family>-<night|paper>-<1x|1.5x>.png`.

#[path = "shoot_controls/sheets.rs"]
mod sheets;

use cantus_ui::kit::system::Face;
use cantus_ui::kit::{tokens, Style};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "target/shoot-controls".to_string()),
    );
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&out).expect("out dir");
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    for (theme, t) in [
        ("night", cantus_ui::theme_slate::slate_night_tokens()),
        ("paper", cantus_ui::theme_slate::slate_paper_tokens()),
    ] {
        tokens::install(t);
        let s = Style::new(tokens::current());
        for (scale, ppp) in [("1x", 1.0f32), ("1.5x", 1.5)] {
            for fam in sheets::FAMILIES {
                if !only.is_empty() && !only.iter().any(|o| fam.name.starts_with(o.as_str())) {
                    continue;
                }
                let ctx = cantus_ui::kit::gallery::offscreen_context();
                cantus_ui::design::use_face(&ctx, Face::Barlow);
                let canvas = cantus_ui::raster::shoot(&ctx, fam.size, ppp, 3, None, |ui| {
                    sheets::paint(ui, &s, fam.name)
                });
                let png = cantus_ui::kit::gallery::encode_png(&canvas).expect("png");
                let file = out.join(format!("{}-{theme}-{scale}.png", fam.name));
                std::fs::write(&file, png).expect("write");
                println!("{}", file.display());
            }
        }
    }
}
