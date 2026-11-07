//! The icon family (round 37, ADR 0103): every glyph is drawn to one stroke weight, one
//! corner radius and one optical size. The stroke and the radius are the family's by
//! construction (no op carries its own); this test holds the rest — every glyph's ink
//! within the live area, none smaller than the family's optical size, each centred in its
//! box (but the two placed on purpose), and each one's ink, rendered, within a band of
//! the family's median so no glyph reads heavier or fainter than the others.

use cantus_ui::kit::glyph::{self, Glyph, Op, ANCHORED, LIVE, OPTICAL_MIN};
use cantus_ui::kit::{gallery, Fill};
use egui::{Color32, Pos2, Rect, Vec2};

#[test]
fn every_glyph_keeps_to_the_live_area_and_the_optical_size() {
    let mut bad = Vec::new();
    for g in Glyph::ALL {
        let e = glyph::extent(g);
        assert!(e.is_positive(), "{g:?} draws nothing");
        let reach = e
            .min
            .x
            .abs()
            .max(e.max.x.abs())
            .max(e.min.y.abs())
            .max(e.max.y.abs());
        if reach > LIVE + 1e-3 {
            bad.push(format!("{g:?} reaches {reach:.2} units (live area {LIVE})"));
        }
        let size = e.width().max(e.height());
        if size < OPTICAL_MIN - 1e-3 {
            bad.push(format!(
                "{g:?} is {size:.2} units (the family's least is {OPTICAL_MIN})"
            ));
        }
        if !ANCHORED.contains(&g) {
            let c = e.center();
            if c.x.abs() > 1.0 || c.y.abs() > 1.0 {
                bad.push(format!("{g:?} sits off centre at ({:.2}, {:.2})", c.x, c.y));
            }
        }
    }
    assert!(bad.is_empty(), "outside the family:\n{}", bad.join("\n"));
}

#[test]
fn every_glyph_is_drawn_with_the_family_s_own_marks() {
    // Ops cannot carry a width or a radius; what is left to hold is that the closed shapes
    // given as `Loop` are convex (their tint is filled as one convex polygon).
    let convex = |p: &[(f32, f32)]| {
        let n = p.len();
        let mut sign = 0.0f32;
        for i in 0..n {
            let (a, b, c) = (p[i], p[(i + 1) % n], p[(i + 2) % n]);
            let z = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
            if z.abs() > 1e-4 {
                if sign != 0.0 && z.signum() != sign {
                    return false;
                }
                sign = z.signum();
            }
        }
        true
    };
    let mut bad = Vec::new();
    for g in Glyph::ALL {
        glyph::ops(g, &mut |op| match op {
            Op::Loop(p) | Op::Tint(p) | Op::Solid(p) if !convex(p) => {
                bad.push(format!("{g:?}: a filled shape that is not convex"))
            }
            Op::Line(p) if p.len() < 2 => bad.push(format!("{g:?}: a line of one point")),
            _ => {}
        });
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The ink each glyph lays down, rendered at `side` points in white on black: the sum of
/// its pixels' brightness over the box's area.
fn inks(side: f32, ppp: f32) -> Vec<(Glyph, f32)> {
    let cell = side + 8.0;
    let cols = 10usize;
    let rows = Glyph::ALL.len().div_ceil(cols);
    let size = Vec2::new(cols as f32 * cell, rows as f32 * cell);
    let canvas = gallery::render(size, ppp, |b, _| {
        b.rect(
            Rect::from_min_size(Pos2::ZERO, size),
            Fill::Solid(Color32::BLACK),
        );
        for (i, g) in Glyph::ALL.iter().enumerate() {
            let at = Pos2::new((i % cols) as f32 * cell, (i / cols) as f32 * cell);
            let r = Rect::from_min_size(at + Vec2::splat(4.0), Vec2::splat(side));
            glyph::paint(b, r, *g, Color32::WHITE);
        }
    });
    Glyph::ALL
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let x0 = ((i % cols) as f32 * cell * ppp) as usize;
            let y0 = ((i / cols) as f32 * cell * ppp) as usize;
            let n = (cell * ppp) as usize;
            let mut sum = 0.0f32;
            for y in y0..(y0 + n).min(canvas.height) {
                for x in x0..(x0 + n).min(canvas.width) {
                    sum += canvas.pixel(x, y)[0] as f32 / 255.0;
                }
            }
            (*g, sum / (side * ppp).powi(2))
        })
        .collect()
}

#[test]
fn no_glyph_reads_heavier_or_fainter_than_the_family() {
    for (side, ppp) in [(18.0, 1.0), (18.0, 1.5), (24.0, 2.0)] {
        let v = inks(side, ppp);
        let mut sorted: Vec<f32> = v.iter().map(|x| x.1).collect();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let median = sorted[sorted.len() / 2];
        let bad: Vec<String> = v
            .iter()
            .filter(|(_, ink)| *ink < median * 0.35 || *ink > median * 2.4)
            .map(|(g, ink)| format!("{g:?}: {:.0} % of the median", ink / median * 100.0))
            .collect();
        assert!(
            bad.is_empty(),
            "at {side} pt × {ppp}: the median ink is {median:.3}; outside the band:\n{}",
            bad.join("\n")
        );
    }
}

#[test]
fn the_stroke_is_whole_pixels_at_every_scale() {
    for ppp in [1.0, 1.25, 1.5, 2.0] {
        for side in [11.0, 14.0, 18.0, 22.0] {
            let w = glyph::stroke_width(side, ppp) * ppp;
            assert!(
                (w - w.round()).abs() < 1e-4 && w >= 1.0,
                "{side} pt × {ppp}: {w} px"
            );
        }
    }
}

/// Round 37 (finish, thirteenth delivery; ADR 0103 §59): the area keys' drawings have an
/// accent (the part in the highlight); the transport's glyphs have none.
#[test]
fn the_area_glyphs_have_an_accent_and_the_transport_none() {
    use cantus_ui::kit::glyph::{has_accent, Glyph};
    for g in [
        Glyph::Browser,
        Glyph::Arrangement,
        Glyph::PianoRoll,
        Glyph::Mixer,
        Glyph::Rack,
        Glyph::Session,
    ] {
        assert!(has_accent(g), "{g:?} has its accent");
    }
    for g in [Glyph::Play, Glyph::Stop, Glyph::Record, Glyph::Loop] {
        assert!(!has_accent(g), "{g:?} is one ink");
    }
}
