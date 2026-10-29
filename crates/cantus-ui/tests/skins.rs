//! Every unit wears a face of its own (round 32, BACKLOG §6h point 1, ADR 0074).
//!
//! The contract declared the sheets plain; each family's builder gave its units their
//! faces. `every_unit_has_a_face_of_its_own` fails for a unit left on the plain look.

use cantus_ui::kit::skin::{self, Unit};
use std::collections::BTreeSet;

#[test]
fn every_unit_has_a_face_of_its_own() {
    let plain: Vec<String> = Unit::all()
        .into_iter()
        .filter(|u| skin::of(*u).is_plain())
        .map(|u| format!("{u:?}"))
        .collect();
    assert!(
        plain.is_empty(),
        "units left on the plain look: {}",
        plain.join(", ")
    );
}

#[test]
fn every_unit_has_a_sheet_under_a_key_of_its_own() {
    let mut keys = BTreeSet::new();
    for u in Unit::all() {
        let key = skin::of(u).key;
        assert_ne!(key, "plain", "{u:?} has no sheet");
        assert!(keys.insert(key), "{u:?} shares the key {key}");
    }
    assert_eq!(keys.len(), Unit::all().len());
}

#[test]
fn the_plain_skin_leaves_the_theme_untouched() {
    let t = cantus_ui::kit::Tokens::for_theme(&cantus_ui::theme::default_theme());
    assert_eq!(skin::PLAIN.tokens(&t), t);
    let s = cantus_ui::kit::Style::new(t);
    assert_eq!(s.skinned(&skin::PLAIN), s);
}

#[test]
fn turned_off_every_unit_wears_the_plain_look() {
    skin::set_enabled(false);
    let all_plain = Unit::all().into_iter().all(|u| skin::of(u).key == "plain");
    skin::set_enabled(true);
    assert!(all_plain);
}

/// Cached knob bodies are keyed by the skin's colours and shape (round 32, ADR 0074): two
/// units' knobs in one window never share a layer, so one's colours never show in the
/// other's; the same unit's knobs share theirs.
#[test]
fn cached_bodies_are_one_per_face() {
    use cantus_ui::kit::brush::Brush;
    use cantus_ui::kit::color::Hsl;
    use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
    use cantus_ui::kit::skin::{KnobShape, Skin, Tint};
    use cantus_ui::kit::{cache, gallery, State, Style, Tokens};
    static WARM: Skin = Skin {
        surface: Tint {
            hue: 30.0,
            sat: 0.4,
            amount: 0.6,
        },
        knob: KnobShape::Skirted,
        ..Skin::plain("test.warm")
    };
    static COLD: Skin = Skin {
        surface: Tint {
            hue: 210.0,
            sat: 0.4,
            amount: 0.6,
        },
        knob: KnobShape::Skirted,
        ..Skin::plain("test.cold")
    };
    let _ = Hsl::of(egui::Color32::BLACK);
    let ctx = gallery::offscreen_context();
    let paint = |skin: &'static Skin| -> u64 {
        let before = cache::with_raw(&ctx, |c| c.stats.layers_rasterised);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(200.0, 100.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let mut b = Brush::default();
            b.begin(ui.ctx());
            let s = Style::new(Tokens::slate()).skinned(skin);
            for i in 0..4 {
                let look = KnobLook::new(KnobSize::M, 0.2 * i as f32, 0.0);
                knob::paint(
                    &mut b,
                    egui::pos2(30.0 + 40.0 * i as f32, 50.0),
                    &s,
                    State::REST,
                    &look,
                );
            }
            b.flush(ui.painter());
        });
        out.textures_delta.clear();
        cache::with_raw(&ctx, |c| c.stats.layers_rasterised) - before
    };
    assert_eq!(paint(&WARM), 1, "one body for the warm unit's four knobs");
    assert_eq!(paint(&WARM), 0, "and it is kept");
    assert_eq!(paint(&COLD), 1, "the cold unit's knobs rasterise their own");
}
