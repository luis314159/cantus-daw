//! A button's words read whole at 75–200 %, in both Slate Lapis themes and both languages
//! (r42 fix-ui): `audio_parts::button` sizes itself to its words and, in a recessed family,
//! paints the kit's dialog button, which narrows its margin to fit them — the size it
//! measures holds what it paints. Every label the tuning views put on a button, in a room
//! wide enough for it (a row that runs out of room is the view's to lay out).

use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::text::probe;

const LABELS: [&str; 10] = [
    "tuning.import",
    "tuning.export",
    "tuning.close",
    "tuning.reset",
    "tuning.reset_track",
    "tuning.mts.reinitialize",
    "tuning.audition.stop",
    "tuning.chorda.save_mine",
    "tuning.chorda.import",
    "tuning.audition.major",
];

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::default_theme()
    } else {
        cantus_ui::theme::default_light_theme()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

#[test]
fn a_buttons_words_are_never_cut_by_its_own_size() {
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        i18n::set_thread_locale(lang);
        let mut labels: Vec<String> = LABELS.iter().map(|k| i18n::t(k)).collect();
        // The cadence's label, the longest a tuning button says.
        labels.push(i18n::tf(
            "tuning.audition.play",
            &[("key", &"F#"), ("mode", &i18n::t("tuning.audition.minor"))],
        ));
        for dark in [true, false] {
            for k in [0.75f32, 1.0, 1.5, 2.0] {
                let ctx = context(dark);
                probe::start_clipped();
                cantus_ui::raster::shoot(&ctx, egui::vec2(900.0, 600.0), k, 2, None, |ui| {
                    for l in &labels {
                        for min in [0.0, 90.0] {
                            cantus_ui::views::audio_parts::button(ui, l, false, true, min);
                        }
                    }
                });
                for c in probe::take() {
                    bad.push(format!(
                        "{lang:?} dark {dark} {k}: {:?} -> {:?}",
                        c.full, c.shown
                    ));
                }
            }
        }
    }
    i18n::set_thread_locale(Locale::En);
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "cut:\n{}", bad.join("\n"));
}
