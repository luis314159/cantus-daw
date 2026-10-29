//! The user's Text size reaches the words a unit's face paints (round 32, the instruments
//! builder's finding and the review of delivery 2). These were painted at the window
//! scale `s.k`, not at `s.tk()`, so they ignored *Text size*, with faces on and off: a
//! card's title and its selector's name, the header's nameplate, the footer's *OUT* and
//! its chips, a menu's shortcut. At 140 % each must be painted 1.4 times as large as at
//! 100 %. And a dial prints no numbers that are not its parameter's own.

use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::kit::band::{self, CardLook};
use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::gallery;
use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
use cantus_ui::kit::menu::{self, Check, Item};
use cantus_ui::kit::meter::Levels;
use cantus_ui::kit::shell::{self, Footer};
use cantus_ui::kit::skin::{self, plates, Case, KnobShape, Lettering, Plate, Skin, Weight};
use cantus_ui::kit::text::Src;
use cantus_ui::kit::{State, Style, Tokens};
use egui::{Pos2, Rect, Vec2};
use std::collections::HashMap;

static FACE: Skin = Skin {
    plate: Plate::Engraved,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.08,
        weight: Weight::Medium,
        condensed: false,
        mono_values: false,
    },
    knob: KnobShape::Dial,
    ..Skin::plain("test.text_size")
};

/// The words to look for, lower-cased, and what paints them.
const WORDS: [(&str, &str); 6] = [
    ("width", "the card's title"),
    ("bell", "the card selector's name"),
    ("spatium", "the nameplate"),
    ("out", "the footer's OUT"),
    ("latency 2.0 ms", "a footer chip"),
    ("ctrl+z", "a menu shortcut"),
];

/// The painted height of every word painted, lower-cased, with text size `scale`.
fn painted(ctx: &egui::Context, sheet: &'static Skin, scale: f32) -> HashMap<String, f32> {
    let mut t = Tokens::slate();
    t.text_scale = scale;
    let s = Style::new(t).skinned(sheet);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        let mut b = Brush::default();
        b.begin(ui.ctx());
        let look = CardLook {
            title: Src::Str("Width"),
            switchable: false,
            selector: Some("Bell"),
            menu: false,
            display: None,
        };
        let _ = band::card(
            &mut b,
            Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(420.0, 200.0)),
            &s,
            State::LIT,
            &look,
        );
        let _ = plates::nameplate(
            &mut b,
            Rect::from_min_size(Pos2::new(10.0, 260.0), Vec2::new(600.0, 40.0)),
            &s,
            "Spatium",
            s.hue,
        );
        let chips = ["Latency 2.0 ms"];
        let _ = shell::paint_footer(
            &mut b,
            Rect::from_min_size(Pos2::new(0.0, 320.0), Vec2::new(900.0, 30.0)),
            &s,
            &Footer {
                chips: &chips,
                meter: Some(Levels::SILENT),
            },
            "",
        );
        menu::paint(
            &mut b,
            Rect::from_min_size(Pos2::new(10.0, 380.0), Vec2::new(300.0, 40.0)),
            &s,
            &[Item::Action {
                label: Src::Str("Undo"),
                check: Check::None,
                shortcut: Some("Ctrl+Z"),
                submenu: false,
                enabled: true,
            }],
            None,
        );
        b.flush(ui.painter());
    });
    out.textures_delta.clear();
    let mut words = HashMap::new();
    fn walk(shape: &egui::Shape, words: &mut HashMap<String, f32>) {
        match shape {
            egui::Shape::Text(t) => {
                let h = words
                    .entry(t.galley.text().to_lowercase())
                    .or_insert(0.0f32);
                *h = h.max(t.galley.size().y);
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, words)),
            _ => {}
        }
    }
    for c in &out.shapes {
        walk(&c.shape, &mut words);
    }
    words
}

#[test]
fn every_word_a_face_paints_follows_the_text_size_with_faces_on_and_off() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    let mut bad = Vec::new();
    for (what, sheet) in [("off", &skin::PLAIN), ("on", &FACE)] {
        // Warm up the fonts, then measure.
        let _ = painted(&ctx, sheet, 1.0);
        let one = painted(&ctx, sheet, 1.0);
        let big = painted(&ctx, sheet, 1.4);
        for (word, part) in WORDS {
            match (one.get(word), big.get(word)) {
                (Some(a), Some(b)) if *a > 0.0 => {
                    let ratio = b / a;
                    if !(1.3..1.5).contains(&ratio) {
                        bad.push(format!(
                            "faces {what}: {part} at 140 % is {ratio:.2}× its 100 % size"
                        ));
                    }
                }
                _ => bad.push(format!(
                    "faces {what}: {part} ({word:?}) not painted: {:?}",
                    one.keys().collect::<Vec<_>>()
                )),
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// A dial's printed "0–10" read as a second, wrong value round a knob whose value is in
/// its parameter's own units (Latitudo's width, 0–200 %): the dial prints a graduated
/// scale, no numbers, at every size.
#[test]
fn a_dial_prints_no_numbers_of_its_own() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    let s = Style::new(Tokens::slate()).skinned(&FACE);
    for size in [KnobSize::S, KnobSize::M, KnobSize::L, KnobSize::XL] {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 200.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let mut b = Brush::default();
            b.begin(ui.ctx());
            knob::paint(
                &mut b,
                Pos2::new(100.0, 100.0),
                &s,
                State::default(),
                &KnobLook::new(size, 0.575, 0.5),
            );
            b.flush(ui.painter());
        });
        out.textures_delta.clear();
        let texts = out
            .shapes
            .iter()
            .filter(|c| matches!(c.shape, egui::Shape::Text(_)))
            .count();
        assert_eq!(texts, 0, "{size:?}: a dial printed numbers");
    }
}
