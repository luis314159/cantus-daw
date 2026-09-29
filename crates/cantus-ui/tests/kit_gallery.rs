//! The gallery's golden images (ADR 0024 §14): every kit part × every state, in Slate
//! and Paper, at 1×, 1.5× and 2×, rendered through the software renderer and compared
//! with `tests/golden/kit/<part>-<theme>-<scale>.png`.
//!
//! A change to a part's look is a reviewed change to its golden images. To re-bless them
//! after an intended change:
//!
//! ```text
//! CANTUS_BLESS=1 cargo test -p cantus-ui --test kit_gallery
//! ```
//!
//! then look at the new PNGs (and the diff in `git`) before committing them. When a
//! comparison fails, the rendered image is written next to the target directory as
//! `target/kit-gallery/<name>-actual.png` for review.
//!
//! Tolerance: anti-aliasing may move by float noise between machines, so a pixel counts
//! as different only when a channel differs by more than [`CHANNEL_TOLERANCE`], and a
//! sheet fails only when more than [`MAX_DIFFERENT`] of its pixels do (or any pixel is
//! off by more than [`HARD_LIMIT`], which no rounding can explain).

use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::kit::gallery::{self, PARTS};
use cantus_ui::kit::{Style, Tokens};
use std::path::PathBuf;

const CHANNEL_TOLERANCE: u8 = 4;
const MAX_DIFFERENT: f64 = 0.002;
const HARD_LIMIT: u8 = 96;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("kit")
}

fn actual_dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("kit-gallery");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn check(theme: &str, tokens: Tokens, scale: &str, ppp: f32) {
    default_thread_locale(Locale::En);
    let bless = std::env::var_os("CANTUS_BLESS").is_some();
    let dir = golden_dir();
    std::fs::create_dir_all(&dir).expect("golden dir");
    let style = Style::new(tokens);
    let mut failures = Vec::new();
    for (name, part) in PARTS {
        let canvas = gallery::render_sheet(&style, part, ppp);
        let file = dir.join(format!("{name}-{theme}-{scale}.png"));
        let png = gallery::encode_png(&canvas).expect("encode");
        if bless {
            std::fs::write(&file, &png).expect("write golden");
            continue;
        }
        let Ok(bytes) = std::fs::read(&file) else {
            failures.push(format!(
                "{name}: no golden image at {} (bless with CANTUS_BLESS=1)",
                file.display()
            ));
            continue;
        };
        let (w, h, want) = gallery::decode_png(&bytes).expect("decode golden");
        let keep = |why: String, failures: &mut Vec<String>| {
            let out = actual_dir().join(format!("{name}-{theme}-{scale}-actual.png"));
            let _ = std::fs::write(&out, &png);
            failures.push(format!(
                "{name}-{theme}-{scale}: {why} (rendered: {})",
                out.display()
            ));
        };
        if (w, h) != (canvas.width, canvas.height) {
            keep(
                format!(
                    "size {}×{} but the golden is {w}×{h}",
                    canvas.width, canvas.height
                ),
                &mut failures,
            );
            continue;
        }
        let (mut different, mut worst) = (0usize, 0u8);
        for y in 0..h {
            for x in 0..w {
                let got = canvas.pixel(x, y);
                let exp = want[y * w + x];
                let d = (0..3).map(|c| got[c].abs_diff(exp[c])).max().unwrap_or(0);
                worst = worst.max(d);
                if d > CHANNEL_TOLERANCE {
                    different += 1;
                }
            }
        }
        let share = different as f64 / (w * h) as f64;
        if share > MAX_DIFFERENT || worst > HARD_LIMIT {
            keep(
                format!(
                    "{different} pixels differ ({:.3} %), the worst by {worst}",
                    share * 100.0
                ),
                &mut failures,
            );
        }
    }
    assert!(
        failures.is_empty(),
        "the kit's look changed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn slate_at_1x() {
    check("slate", Tokens::slate(), "1x", 1.0);
}

#[test]
fn slate_at_1_5x() {
    check("slate", Tokens::slate(), "1.5x", 1.5);
}

#[test]
fn slate_at_2x() {
    check("slate", Tokens::slate(), "2x", 2.0);
}

#[test]
fn paper_at_1x() {
    check("paper", Tokens::paper(), "1x", 1.0);
}

#[test]
fn paper_at_1_5x() {
    check("paper", Tokens::paper(), "1.5x", 1.5);
}

#[test]
fn paper_at_2x() {
    check("paper", Tokens::paper(), "2x", 2.0);
}
