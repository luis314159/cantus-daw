//! (f) Configurability: the same mixer in four looks the user can make — Slate, Paper,
//! Slate with the hue knob turned, and Slate flattened (depth down, corners squarer,
//! compact) — and (g) inspect mode: the eyedropper on the selected fader, listing the
//! tokens it paints with.

use crate::mixer;
use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::color::with_alpha;
use cantus_ui::kit::text::{self, Role, Text};
use cantus_ui::kit::{gallery, inspect, Density, Fill, Form, Style, ThemeKnobs, Tokens};
use egui::{Align2, Pos2, Rect, Vec2};

/// The four looks, with their captions.
pub fn looks() -> [(&'static str, Tokens); 4] {
    let mut hue = cantus_ui::theme::slate();
    hue.knobs = ThemeKnobs {
        hue: 150.0,
        saturation: 0.6,
        brightness: 0.1,
        ..Default::default()
    };
    let mut flat = cantus_ui::theme::slate();
    flat.form = Form {
        depth: 0.1,
        roundness: 0.4,
        density: Density::Normal,
        text_size: 1.0,
    };
    flat.knobs = ThemeKnobs {
        contrast: 0.25,
        ..Default::default()
    };
    [
        ("Slate", cantus_ui::theme::slate().kit_tokens()),
        ("Paper", cantus_ui::theme::paper().kit_tokens()),
        ("Slate · Hue +150° · Saturation +60 %", hue.kit_tokens()),
        (
            "Slate · Depth 10 % · Roundness 40 % · Contrast +25 %",
            flat.kit_tokens(),
        ),
    ]
}

/// A caption pill at the foot of a render.
fn caption(b: &mut Brush, s: &Style, size: Vec2, label: &str) {
    let t = &s.t;
    let w = text::measure(b.ctx(), &Text::new(label, Role::Control, t.text), 1.0).x + 20.0;
    let r = Rect::from_min_size(
        Pos2::new(size.x - w - 10.0, size.y - 30.0),
        Vec2::new(w, 22.0),
    );
    b.rrect(b.snap_rect(r), 11.0, Fill::Solid(with_alpha(t.ink, 0.92)));
    let _ = text::paint(
        b,
        r,
        &Text::new(label, Role::Control, t.text).align(Align2::CENTER_CENTER),
        1.0,
    );
}

/// The four mixers, side by side, as one PNG.
pub fn variations(ppp: f32) -> Vec<u8> {
    let size = mixer::SIZE;
    let parts: Vec<cantus_ui::raster::Canvas> = looks()
        .iter()
        .map(|(label, t)| {
            let s = Style::new(*t);
            gallery::render(size, ppp, |b, p| {
                mixer::paint(b, p, &s, size);
                caption(b, &s, size, label);
            })
        })
        .collect();
    side_by_side(&parts)
}

fn side_by_side(parts: &[cantus_ui::raster::Canvas]) -> Vec<u8> {
    let gap = 8;
    let w: usize = parts.iter().map(|c| c.width).sum::<usize>() + gap * (parts.len() - 1);
    let h = parts.iter().map(|c| c.height).max().unwrap_or(0);
    let mut pix = tiny_skia::Pixmap::new(w as u32, h as u32).expect("pixmap");
    pix.fill(tiny_skia::Color::from_rgba8(30, 30, 30, 255));
    let mut x0 = 0;
    for c in parts {
        for y in 0..c.height {
            for x in 0..c.width {
                let [r, g, b, _] = c.pixel(x, y);
                if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255) {
                    pix.pixels_mut()[y * w + x0 + x] = p;
                }
            }
        }
        x0 += c.width + gap;
    }
    pix.encode_png().expect("png")
}

/// The mixer with inspect mode on, the selected strip's fader cap picked.
pub fn inspect_demo(ppp: f32, t: Tokens) -> cantus_ui::raster::Canvas {
    let size = mixer::SIZE + Vec2::new(250.0, 0.0);
    let s = Style::new(t);
    gallery::render(size, ppp, |b, p| {
        inspect::set_enabled(b.ctx(), true);
        b.rect(
            Rect::from_min_size(Pos2::ZERO, size),
            Fill::Solid(s.t.panel),
        );
        mixer::paint(b, p, &s, mixer::SIZE);
        // Pick the selected strip's fader cap, where a user would click.
        let at = mixer::selected_cap_centre();
        if let Some(item) = inspect::hit(b.ctx(), at) {
            let card_at = Pos2::new(mixer::SIZE.x + 6.0, 60.0);
            inspect::paint_pick(b, &s, &item, card_at, Rect::from_min_size(Pos2::ZERO, size));
            // The pointer, as a small crosshair.
            b.circle_stroke(at, 5.0, 1.5, s.t.text);
            b.segment(
                at + Vec2::new(-9.0, 0.0),
                at + Vec2::new(9.0, 0.0),
                1.0,
                s.t.text,
                false,
            );
            b.segment(
                at + Vec2::new(0.0, -9.0),
                at + Vec2::new(0.0, 9.0),
                1.0,
                s.t.text,
                false,
            );
        }
    })
}
