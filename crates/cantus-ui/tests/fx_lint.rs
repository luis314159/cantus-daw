//! The effect editors keep the look (ADR 0024 §3, §4, §14): no bare words and no stock
//! egui widget in the files that draw them, no text painted unmeasured, no font named by
//! size, no colour spelled out or read from the old palette — everything goes through the
//! kit (`views/fx/controls.rs`, `views/fx/canvas.rs`) and its tokens.

use std::path::PathBuf;

/// Every file of the seven effect editors, the device frame and the hosted plug-ins'
/// editor: all on the kit. A new file of theirs goes here.
const ON_THE_KIT: &[&str] = &[
    "views/comp_editor.rs",
    "views/comp_editor/desk.rs",
    "views/comp_editor/history.rs",
    "views/comp_editor/key.rs",
    "views/comp_editor/transfer.rs",
    "views/eq_editor.rs",
    "views/eq_editor_display.rs",
    "views/eq_editor_panel.rs",
    "views/eq_editor_parse.rs",
    "views/fx/anulus.rs",
    "views/fx/canvas.rs",
    "views/fx/concentus.rs",
    "views/fx/controls.rs",
    "views/fx/deesser.rs",
    "views/fx/discrimen.rs",
    "views/fx/flexus.rs",
    "views/fx/fluctus.rs",
    "views/fx/frame.rs",
    "views/fx/gate.rs",
    "views/fx/host.rs",
    "views/fx/impetus.rs",
    "views/fx/limiter.rs",
    "views/fx/maximizer.rs",
    "views/fx/maximizer_bands.rs",
    "views/fx/maximizer_cards.rs",
    "views/fx/maximizer_curve.rs",
    "views/fx/maximizer_demo.rs",
    "views/fx/maximizer_display.rs",
    "views/fx/maximizer_learn.rs",
    "views/fx/maximizer_meters.rs",
    "views/fx/mod.rs",
    "views/fx/multiband.rs",
    "views/fx/phasis.rs",
    "views/fx/saturator.rs",
    "views/fx/saturator_curve.rs",
    "views/fx/saturator_harm.rs",
    "views/fx/space.rs",
    "views/fx/space_curve.rs",
    "views/fx/space_tail.rs",
    "views/fx/speculum.rs",
    "views/fx/stereo.rs",
    "views/fx/stereo_map.rs",
    "views/fx/stereo_scope.rs",
    "views/fx/tonsor.rs",
    "views/fx/tonus.rs",
    "views/fx/translatio.rs",
    "views/fx/tremor.rs",
    "views/fx/vibrans.rs",
    "views/plugins.rs",
];

/// What a file on the kit never says.
const BANNED: &[(&str, &str)] = &[
    (
        "selectable_label",
        "a bare text toggle: controls::led or controls::segmented",
    ),
    (
        "selectable_value",
        "a bare text toggle: controls::segmented",
    ),
    ("toggle_value", "a bare text toggle: controls::led"),
    ("ui.button(", "a stock button: controls::key"),
    ("small_button", "a stock button: controls::key"),
    ("egui::Button", "a stock button: controls::key"),
    (
        "ComboBox",
        "a stock combo box: controls::field and controls::list",
    ),
    ("menu_button", "a stock menu: frame::popup_list"),
    ("radio_value", "a stock radio: controls::segmented"),
    ("checkbox(", "a stock checkbox: controls::led"),
    ("RichText", "loose words: kit text in a box"),
    ("ui.label(", "loose words: kit text in a box"),
    ("painter.text(", "text painted unmeasured: canvas::put"),
    ("p.text(", "text painted unmeasured: canvas::put"),
    ("FontId", "a font by size: a text role"),
    ("theme::p()", "the old palette: the kit's tokens"),
    ("Color32::from_rgb", "a colour spelled out: a token"),
    ("Color32::from_gray", "a colour spelled out: a token"),
    ("Color32::WHITE", "a colour spelled out: a token"),
    ("Color32::BLACK", "a colour spelled out: a token"),
    (
        "on_hover_text",
        "an egui tooltip: the help line (kit::hint)",
    ),
];

#[test]
fn the_editors_on_the_kit_say_no_bare_word_and_draw_no_stock_widget() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for rel in ON_THE_KIT {
        let text = std::fs::read_to_string(src.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let mut in_tests = false;
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("#[cfg(test)]") {
                in_tests = true;
            }
            if in_tests || code.starts_with("//") {
                continue;
            }
            for (bad, why) in BANNED {
                if line.contains(bad) {
                    found.push(format!("src/{rel}:{}: `{bad}` — {why}", n + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}
