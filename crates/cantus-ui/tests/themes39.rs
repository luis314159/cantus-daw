//! Round 39, *themes*: the ten themes of rounds 24–38 back, redrawn in round 39's design
//! system, and four new ones (`theme_redrawn.rs`, `theme_added.rs`, made by
//! `tools/themes/themes.py`). What holds them, beyond what every shipped theme is held to
//! (`theme::tests`, `kit::tokens::tests`, the skins' and the views' own per-preset tests):
//!
//! - each keeps every promise of the kit as written, and resolves to exactly its tokens;
//! - each is of round 39's system: recessed keys, a face of a round-39 family, no gradient,
//!   no glow, the depth and corners of the recessed families;
//! - each is built on Slate's ladder in its light: the gutter, the base, the surface and the
//!   raised step in Slate's order, the well a shade off its bar;
//! - the restored ones keep their old light and their old id; the FL-derived pair carry new
//!   names; every theme is told from every other by its room and its accent;
//! - Slate and Slate Lapis are untouched (their tokens as their files write them).

use cantus_ui::kit::appearance::{CLASSIC, PRESETS};
use cantus_ui::kit::color::{contrast, luminance};
use cantus_ui::kit::system::{CableStyle, Face, System};
use cantus_ui::kit::tokens::{Tok, Tokens};
use cantus_ui::theme_added as added;
use cantus_ui::theme_redrawn as redrawn;
use egui::Color32;

type Entry = (
    &'static str,
    fn() -> cantus_ui::theme::Theme,
    fn() -> Tokens,
);

const RESTORED: [Entry; 10] = [
    (
        "cantus_night",
        redrawn::cantus_night,
        redrawn::cantus_night_tokens,
    ),
    (
        "cantus_paper",
        redrawn::cantus_paper,
        redrawn::cantus_paper_tokens,
    ),
    ("daylight", redrawn::daylight, redrawn::daylight_tokens),
    ("blue_hour", redrawn::blue_hour, redrawn::blue_hour_tokens),
    (
        "ultraviolet",
        redrawn::ultraviolet,
        redrawn::ultraviolet_tokens,
    ),
    ("lagoon", redrawn::lagoon, redrawn::lagoon_tokens),
    ("slate", redrawn::pewter, redrawn::pewter_tokens),
    ("paper", redrawn::chalk, redrawn::chalk_tokens),
    (
        "ink_gold",
        redrawn::ink_and_gold,
        redrawn::ink_and_gold_tokens,
    ),
    ("parchment", redrawn::parchment, redrawn::parchment_tokens),
];

const NEW: [Entry; 4] = [
    (
        "evergreen_night",
        added::evergreen_night,
        added::evergreen_night_tokens,
    ),
    (
        "evergreen_paper",
        added::evergreen_paper,
        added::evergreen_paper_tokens,
    ),
    ("bordeaux", added::bordeaux, added::bordeaux_tokens),
    ("print", added::print, added::print_tokens),
];

fn all() -> impl Iterator<Item = &'static Entry> {
    RESTORED.iter().chain(NEW.iter())
}

#[test]
fn every_theme_keeps_every_promise_as_written_and_resolves_to_its_tokens() {
    for (id, theme, tokens) in all() {
        let t = tokens();
        let bad = t.legibility_failures();
        assert!(bad.is_empty(), "{id}: {bad:#?}");
        let got = theme().kit_tokens();
        for tok in Tok::ALL {
            assert_eq!(got.get(*tok), t.get(*tok), "{id}: {}", tok.key());
        }
        assert_eq!(got.system, t.system, "{id}");
        // Offered under its id.
        let (_, make) = PRESETS.iter().find(|(k, _)| k == id).expect(id);
        assert_eq!(make(), theme(), "{id}");
    }
}

#[test]
fn every_theme_is_of_round_39_s_system() {
    for (id, _, tokens) in all() {
        let t = tokens();
        assert!(t.system.recessed(), "{id}: recessed keys are the standard");
        assert_ne!(t.system.face, Face::Plex, "{id}: a round-39 face");
        assert!(t.is_flat(), "{id}: no gradient, no glow");
        assert_eq!((t.depth, t.round), (0.5, 1.0), "{id}");
        assert!(
            luminance(t.inner_shadow) < luminance(t.well),
            "{id}: the well's edge"
        );
        // The user's colours a tint at rest, under the lane veil.
        assert!(t.track_veil.a() >= 128, "{id}: the veil");
    }
    // The systems chosen: Evergreen's real patch cords, Print's printed diagram.
    for t in [
        added::evergreen_night_tokens(),
        added::evergreen_paper_tokens(),
    ] {
        assert_eq!(t.system.cables, CableStyle::Jack);
        assert_eq!(t.system.face, System::GRAPHITE.face);
    }
    assert_eq!(added::print_tokens().system.cables, CableStyle::Subway);
}

#[test]
fn every_theme_stands_on_slate_s_ladder_in_its_light() {
    for (id, _, tokens) in all() {
        let t = tokens();
        let l = luminance;
        if t.dark {
            // A dark gutter between panels, the base under the lanes, a lighter surface,
            // raised menus and number cells above it.
            assert!(l(t.ink) < l(t.panel), "{id}: the gutter");
            assert!(l(t.panel) < l(t.toolbar), "{id}: the surface");
            assert!(l(t.toolbar) < l(t.menu), "{id}: the raised step");
            assert!(l(t.well) < l(t.toolbar), "{id}: the well under its bar");
        } else {
            assert!(
                l(t.ink) > l(t.toolbar),
                "{id}: the paper's gutter is its lightest"
            );
            assert!(l(t.panel) > l(t.toolbar), "{id}: the base over the surface");
            assert!(l(t.well) < l(t.toolbar), "{id}: the well under its bar");
        }
        assert_eq!(t.toolbar, t.strip, "{id}: bars and strips are one surface");
        // Cards are the surface, or (Pewter, Chalk: round 24's own rule) a step apart from
        // it so the units' hues read on them.
        assert!(contrast(t.text, t.deck) >= 4.5, "{id}: cards");
        assert!(contrast(t.well, t.toolbar) >= 1.05, "{id}: the well shows");
    }
}

#[test]
fn the_restored_keep_their_light_and_id_and_the_fl_pair_is_renamed() {
    assert_eq!(RESTORED.len(), CLASSIC.len());
    for (id, theme, _) in RESTORED.iter() {
        let (_, classic) = CLASSIC.iter().find(|(c, _)| c == id).expect(id);
        assert_eq!(theme().dark, classic().dark, "{id}");
    }
    assert_eq!(redrawn::pewter().name, "Pewter");
    assert_eq!(redrawn::chalk().name, "Chalk");
    for (id, make) in PRESETS {
        let name = make().name;
        for mark in ["FL", "Studio One", "Ableton", "Logic", "Bitwig"] {
            assert!(!name.contains(mark), "{id}: {name}");
        }
    }
    // Names in both languages, none colliding.
    let mut names = std::collections::BTreeSet::new();
    for (id, make) in PRESETS {
        assert!(names.insert(make().name), "{id}: a name taken twice");
    }
}

/// CIELAB (D65) of an sRGB colour.
fn lab(c: Color32) -> [f32; 3] {
    let f = |u: u8| {
        let u = u as f32 / 255.0;
        if u <= 0.04045 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (f(c.r()), f(c.g()), f(c.b()));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let k = |t: f32| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    [
        116.0 * k(y) - 16.0,
        500.0 * (k(x) - k(y)),
        200.0 * (k(y) - k(z)),
    ]
}

fn delta(a: Color32, b: Color32) -> f32 {
    let (a, b) = (lab(a), lab(b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn every_theme_is_told_from_every_other() {
    // How far two themes of one light are apart, as the eye takes a whole window: the room
    // (the base and the bars, CIELAB ΔE) and half the accent. Slate and Slate Lapis, the
    // pair the owner first found alike and approved apart in `lapis2`, are 16.8 here; no
    // two themes may be closer than 14.
    let rooms: Vec<(&str, Tokens)> = PRESETS
        .iter()
        .map(|(id, m)| (*id, m().kit_tokens()))
        .collect();
    for (i, (a, ta)) in rooms.iter().enumerate() {
        for (b, tb) in &rooms[i + 1..] {
            if ta.dark != tb.dark {
                continue;
            }
            let d = delta(ta.panel, tb.panel)
                + delta(ta.toolbar, tb.toolbar)
                + 0.5 * delta(ta.highlight, tb.highlight);
            assert!(d >= 14.0, "{a} and {b} look alike ({d:.1})");
        }
    }
}

#[test]
fn slate_and_slate_lapis_are_untouched() {
    use cantus_ui::{theme_slate as slate, theme_slate_lapis as lapis};
    for (id, tokens) in [
        ("slate_night", slate::slate_night_tokens()),
        ("slate_paper", slate::slate_paper_tokens()),
        ("slate_lapis_night", lapis::slate_lapis_night_tokens()),
        ("slate_lapis_paper", lapis::slate_lapis_paper_tokens()),
    ] {
        let (_, make) = PRESETS.iter().find(|(k, _)| *k == id).unwrap();
        let got = make().kit_tokens();
        for tok in Tok::ALL {
            assert_eq!(got.get(*tok), tokens.get(*tok), "{id}: {}", tok.key());
        }
    }
    assert_eq!(
        slate::slate_night_tokens().panel,
        Color32::from_rgb(0x1d, 0x23, 0x29)
    );
}
