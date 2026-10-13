//! Cantus Night refined for long sessions, and the theme's gradients (ADR 0046 §8): every
//! theme says what its gradients are; a flat theme draws none, not a vertex; the glows
//! stay inside their ground; the text over every gradient stays legible — and Cantus
//! Night's text comfortable rather than glaring; the veil calms a track's large fills
//! and keeps its hue, leaving the bright colour cell as it was.

use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::color::{contrast, over, veiled, Hsl};
use cantus_ui::kit::{gallery, light, Style, Tokens};
use cantus_ui::theme_family::{cantus_night_tokens, FAMILY_TOKENS};
use egui::{Color32, Pos2, Rect, Shape, Vec2};

const GRADIENT_KEYS: [&str; 4] = [
    "depth.sheen",
    "depth.glow",
    "depth.glow_cool",
    "editor.track_veil",
];

/// Every theme's tokens: the family, and the older presets the kit ships.
fn every_tokens() -> Vec<(&'static str, Tokens)> {
    let mut v: Vec<(&'static str, Tokens)> =
        FAMILY_TOKENS.iter().map(|(id, f)| (*id, f())).collect();
    v.push(("slate", Tokens::slate()));
    v.push(("paper", Tokens::paper()));
    v.push(("ink_and_gold", Tokens::ink_and_gold()));
    v.push(("parchment", Tokens::parchment()));
    v
}

#[test]
fn every_theme_defines_its_gradients_and_only_cantus_night_is_lit() {
    for (key, make) in cantus_ui::theme::PRESETS {
        let theme = make();
        for k in GRADIENT_KEYS {
            assert!(theme.kit.contains_key(k), "{key}: no {k}");
        }
    }
    for (id, t) in every_tokens() {
        if id == "cantus_night" {
            assert!(!t.is_flat(), "Cantus Night has its gradients");
            assert!(t.sheen.a() > 0 && t.glow.a() > 0 && t.glow_cool.a() > 0);
            assert!(t.track_veil.a() > 0, "Cantus Night veils its track colours");
        } else {
            // The other themes stay as they were: flat, their track colours unveiled.
            assert!(t.is_flat(), "{id} paints a gradient");
            assert_eq!(t.track_veil.a(), 0, "{id} veils its track colours");
        }
    }
    // A theme file from before the kit derives flat tokens.
    let old = Tokens::derive_from_palette(&cantus_ui::theme::ink_and_gold().palette());
    assert!(old.is_flat() && old.track_veil.a() == 0);
}

/// Run one frame of `f` with a brush; the vertices `f` added, and every vertex position
/// the frame handed to the painter.
fn painted(f: impl FnOnce(&mut Brush)) -> (usize, Vec<Pos2>) {
    let ctx = gallery::offscreen_context();
    let mut added = 0;
    let mut f = Some(f);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(2000.0, 1200.0))),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        let mut b = Brush::default();
        b.begin(ui.ctx());
        let before = b.vertex_count();
        if let Some(f) = f.take() {
            f(&mut b);
        }
        added = b.vertex_count() - before;
        b.flush(ui.painter());
    });
    let mut pts = Vec::new();
    fn walk(s: &Shape, pts: &mut Vec<Pos2>) {
        match s {
            Shape::Mesh(m) => pts.extend(m.vertices.iter().map(|v| v.pos)),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, pts)),
            _ => {}
        }
    }
    for s in &out.shapes {
        walk(&s.shape, &mut pts);
    }
    out.textures_delta.clear();
    (added, pts)
}

#[test]
fn a_flat_theme_draws_no_gradient_and_the_glows_stay_in_their_ground() {
    let bar = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(1920.0, 52.0));
    let ground = Rect::from_min_max(Pos2::new(220.0, 140.0), Pos2::new(1640.0, 1050.0));
    for (id, t) in every_tokens() {
        let (n, _) = painted(|b| {
            light::sheen(b, bar, &t);
            light::ambient(b, ground, &t);
        });
        if t.is_flat() {
            assert_eq!(n, 0, "{id}: a flat theme painted {n} vertices of gradient");
        } else {
            assert!(n > 0, "{id}: its gradients painted nothing");
        }
    }
    // Cantus Night's glows: inside the ground, whatever its shape, and cheap — a few
    // hundred quads, built per frame without a texture.
    let night = cantus_night_tokens();
    for ground in [
        ground,
        Rect::from_min_size(Pos2::new(0.0, 560.0), Vec2::new(1640.0, 480.0)),
        Rect::from_min_size(Pos2::new(105.0, 445.0), Vec2::new(880.0, 165.0)),
        Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(40.0, 900.0)),
    ] {
        let (n, pts) = painted(|b| light::ambient(b, ground, &night));
        assert!(
            n > 0 && n < 4000,
            "{n} vertices for the glows of {ground:?}"
        );
        let room = ground.expand(0.01);
        for p in pts {
            assert!(
                room.contains(p),
                "a glow vertex at {p:?} is outside {ground:?}"
            );
        }
    }
    // The sheen is one gradient rect: a handful of vertices.
    let (n, _) = painted(|b| light::sheen(b, bar, &night));
    assert!(n <= 16, "the sheen took {n} vertices");
}

#[test]
fn the_text_over_every_gradient_stays_legible_and_cantus_night_reads_comfortably() {
    let t = cantus_night_tokens();
    let pairs = t.pairings();
    assert!(
        pairs.iter().any(|(w, ..)| w.contains("sheen"))
            && pairs.iter().any(|(w, ..)| w.contains("glow")),
        "the gradients are measured"
    );
    assert!(
        t.legibility_failures().is_empty(),
        "{:#?}",
        t.legibility_failures()
    );
    // A flat theme measures nothing about gradients it does not paint.
    assert!(!Tokens::slate()
        .pairings()
        .iter()
        .any(|(w, ..)| w.contains("glow")));

    // Primary text: well past AAA (7:1) everywhere it is read, gradients included, but
    // not the maximum glare of ivory on near-black — about 12:1 on the panel.
    let on_panel = contrast(t.text, t.panel);
    assert!(
        (10.0..=13.5).contains(&on_panel),
        "text on panel {on_panel:.2}:1"
    );
    let lit = [
        over(t.sheen, t.toolbar),
        over(t.sheen, t.panel),
        over(t.glow, t.panel),
        over(t.glow_cool, over(t.glow, t.panel)),
        over(t.glow, t.strip),
    ];
    for bg in lit {
        let got = contrast(t.text, bg);
        assert!(got >= 7.0, "text on a gradient {got:.2}:1");
        assert!(contrast(t.text_dim, bg) >= 4.5);
    }
    // Secondary text: AA with room, and clearly a step below the primary.
    let dim = contrast(t.text_dim, t.panel);
    assert!(
        (4.5..=8.0).contains(&dim),
        "secondary text on panel {dim:.2}:1"
    );
    // What carries meaning still shows: meters on their well, the selection and the
    // playhead at 3:1 or more.
    for (what, fg, bg) in [
        ("meter low", t.meter_low, t.meter_well),
        ("meter high", t.meter_high, t.meter_well),
        ("meter clip", t.meter_clip, t.meter_well),
        ("selection", t.selection, t.strip),
        ("playhead", t.playhead, over(t.glow, t.row_black)),
    ] {
        assert!(contrast(fg, bg) >= 3.0, "{what}: {:.2}:1", contrast(fg, bg));
    }
}

#[test]
fn the_veil_calms_a_track_s_large_fills_and_keeps_its_hue() {
    use cantus_ui::views::clips_chrome::Tints;
    let night = cantus_night_tokens();
    let mut unveiled = night;
    unveiled.track_veil = Color32::TRANSPARENT;
    let (calm, plain) = (Style::new(night), Style::new(unveiled));
    let (mut a, mut b) = (Tints::default(), Tints::default());
    let colours = [
        Color32::from_rgb(0xe0, 0x8a, 0x3c),
        Color32::from_rgb(0x3c, 0xa0, 0xd0),
        Color32::from_rgb(0xa0, 0x50, 0xd0),
        Color32::from_rgb(0x40, 0xc0, 0x60),
        Color32::from_rgb(0xd0, 0x40, 0x60),
        Color32::from_rgb(0xd8, 0xc8, 0x40),
    ];
    for c in colours {
        let (v, p) = (a.of(c, &calm), b.of(c, &plain));
        // The colour cell keeps the bright hue; the large fill is calmer.
        assert_eq!(v.small, p.small);
        let (hv, hp) = (Hsl::of(v.large), Hsl::of(p.large));
        assert!(hv.s < hp.s * 0.9 || hv.l < hp.l * 0.9, "{c:?}: not calmer");
        let dh = (hv.h - hp.h).rem_euclid(360.0);
        assert!(dh.min(360.0 - dh) < 12.0, "{c:?}: hue moved {dh}°");
        // And its name stays legible on it.
        assert!(contrast(v.on_large, v.large) >= 4.5);
    }
    // A clear veil is the identity.
    let c = Color32::from_rgb(0x3c, 0xa0, 0xd0);
    assert_eq!(veiled(c, Color32::TRANSPARENT), c);
}
