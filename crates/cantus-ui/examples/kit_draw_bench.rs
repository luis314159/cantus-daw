//! How the kit draws Slate's parts (round 39, ADR 0108): the two ways measured side by side
//! on the same keys — **vectors** of the kit's own (`cap::well_key`, tessellated each
//! frame) against **SVG rasterised per scale and cached** (each key drawn once by resvg into
//! the layer atlas, then one textured quad a frame).
//!
//! `cargo run --release -p cantus-ui --example kit_draw_bench`
//!
//! 240 keys (a top bar and twelve strips' worth, five times over), at 1× and 1.5×, warm
//! frames (200, median) and the cold frame (the first after the atlas is cleared — a start,
//! a theme switch, a change of scale). One core, no GPU.

use cantus_ui::kit::cap::{self, KeyFace};
use cantus_ui::kit::glyph::Glyph;
use cantus_ui::kit::{with_brush, State, Style};
use egui::{Pos2, Rect, Vec2};
use std::time::Instant;

const FRAMES: usize = 200;
const KEYS: usize = 240;
const GLYPHS: [Glyph; 6] = [
    Glyph::Play,
    Glyph::Stop,
    Glyph::Record,
    Glyph::Loop,
    Glyph::Metronome,
    Glyph::Magnet,
];

fn state(i: usize) -> State {
    State {
        hovered: i % 17 == 3,
        lit: i.is_multiple_of(5),
        ..State::REST
    }
}

fn cell(i: usize) -> Rect {
    let (cols, pitch) = (24, 30.0);
    Rect::from_min_size(
        Pos2::new(
            20.0 + (i % cols) as f32 * pitch,
            20.0 + (i / cols) as f32 * pitch,
        ),
        Vec2::splat(24.0),
    )
}

/// One key as the design source writes it (`dir-a/src/base.py` `well`, a glyph path), in
/// physical pixels.
fn svg(px: f32, s: &Style, st: State, g: Glyph) -> String {
    let hex = |c: egui::Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    let fill = cap::well_fill(s, st);
    let ink = cap::well_ink(s, st);
    let (w, r) = (24.0 * px, 4.0 * px);
    let glyph = match g {
        Glyph::Play => format!(
            "<path d='M{a} {b}L{c} {m}L{a} {d}Z' fill='{}'/>",
            hex(ink),
            a = 9.0 * px,
            b = 7.0 * px,
            c = 17.0 * px,
            m = 12.0 * px,
            d = 17.0 * px
        ),
        Glyph::Record => format!(
            "<circle cx='{m}' cy='{m}' r='{}' fill='{}'/>",
            4.0 * px,
            hex(s.t.danger),
            m = 12.0 * px
        ),
        _ => format!(
            "<rect x='{a}' y='{a}' width='{b}' height='{b}' rx='{}' fill='none' stroke='{}' \
             stroke-width='{}'/>",
            1.5 * px,
            hex(ink),
            1.5 * px,
            a = 7.0 * px,
            b = 10.0 * px
        ),
    };
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{w}'>\
         <rect width='{w}' height='{w}' rx='{r}' fill='{}'/>\
         <rect width='{w}' height='{}' rx='{r}' fill='{}'/>\
         <rect x='{p}' y='{p}' width='{}' height='{}' rx='{}' fill='{}'/>{glyph}</svg>",
        hex(s.t.lip),
        w - px,
        hex(s.t.inner_shadow),
        w - 2.0 * px,
        w - 2.0 * px,
        r - px,
        hex(fill),
        p = px,
    )
}

fn raster(svg: &str, pix: &mut tiny_skia::Pixmap) {
    use resvg::usvg;
    let Ok(tree) = usvg::Tree::from_str(svg, &usvg::Options::default()) else {
        return;
    };
    let Some(mut out) = resvg::tiny_skia::Pixmap::new(pix.width(), pix.height()) else {
        return;
    };
    resvg::render(&tree, Default::default(), &mut out.as_mut());
    pix.data_mut().copy_from_slice(out.data());
}

struct Cost {
    cold_ms: f64,
    warm_ms: f64,
    vertices: usize,
    draws: usize,
}

fn measure(ctx: &egui::Context, ppp: f32, mut paint: impl FnMut(&mut egui::Ui)) -> Cost {
    let input = || egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(1920.0 / ppp, 1080.0 / ppp),
        )),
        viewports: std::iter::once((
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                native_pixels_per_point: Some(ppp),
                ..Default::default()
            },
        ))
        .collect(),
        ..Default::default()
    };
    let mut frame = |ctx: &egui::Context| {
        let t0 = Instant::now();
        let out = ctx.run_ui(input(), |ui| paint(ui));
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let v = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                _ => 0,
            })
            .sum();
        (ms, v, prims.len())
    };
    // Settle the context (fonts, the atlas) on a frame that paints nothing.
    for _ in 0..3 {
        let _ = ctx.run_ui(input(), |_| {});
    }
    cantus_ui::kit::cache::with(ctx, |c| c.clear_layers());
    let (cold_ms, _, _) = frame(ctx);
    let mut warm = Vec::new();
    let (mut vertices, mut draws) = (0, 0);
    for _ in 0..FRAMES {
        let (ms, v, d) = frame(ctx);
        warm.push(ms);
        vertices = v;
        draws = d;
    }
    warm.sort_by(f64::total_cmp);
    Cost {
        cold_ms,
        warm_ms: warm[warm.len() / 2],
        vertices,
        draws,
    }
}

fn main() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let ctx = cantus_ui::kit::gallery::offscreen_context();
    let theme = cantus_ui::theme::slate_night();
    cantus_ui::theme::apply(&theme, &ctx);
    let s = Style::new(theme.kit_tokens());
    println!(
        "{:<44} {:>6} {:>9} {:>9} {:>9} {:>6}",
        "case", "scale", "cold ms", "warm ms", "vertices", "draws"
    );
    for ppp in [1.0f32, 1.5] {
        let v = measure(&ctx, ppp, |ui| {
            with_brush(ui.painter(), |b| {
                for i in 0..KEYS {
                    let g = GLYPHS[i % GLYPHS.len()];
                    cap::well_key(b, cell(i), &s, state(i), KeyFace::Glyph(g));
                }
            })
        });
        let r = measure(&ctx, ppp, |ui| {
            with_brush(ui.painter(), |b| {
                let px = b.ppp();
                let side = (24.0 * px).round() as usize;
                for i in 0..KEYS {
                    let g = GLYPHS[i % GLYPHS.len()];
                    let st = state(i);
                    let key = cantus_ui::kit::cache::key((
                        "bench-svg-key",
                        g as u32,
                        st.lit,
                        st.hovered,
                        side,
                    ));
                    let doc = || svg(px, &s, st, g);
                    if let Some(uv) = b.layer(key, side, side, |pix| raster(&doc(), pix)) {
                        b.layer_quad(cell(i), uv, 1.0);
                    }
                }
            })
        });
        for (name, c) in [
            ("240 keys: vectors of the kit's own", v),
            ("240 keys: SVG rasterised, cached quads", r),
        ] {
            println!(
                "{name:<44} {ppp:>6.1} {:>9.3} {:>9.3} {:>9} {:>6}",
                c.cold_ms, c.warm_ms, c.vertices, c.draws
            );
        }
    }
}
