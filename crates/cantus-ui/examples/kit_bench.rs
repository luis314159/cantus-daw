//! What the kit costs to paint (ADR 0024 §13): per part, and the cases
//! `rendering-performance.md` §5 measured — 256 knobs, 64 meters, 2,000 labels.
//!
//! `cargo run --release -p cantus-ui --example kit_bench`
//!
//! Each case runs 200 frames at 1920×1080, 1×, and reports the median time to build the
//! frame (painting into the brush, flushing into egui) and to tessellate it, with the
//! vertices and meshes it produced. One core, no GPU.

use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::gallery::{self, PARTS};
use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
use cantus_ui::kit::meter::{Levels, MeterBatch};
use cantus_ui::kit::text::{self, Role, Text};
use cantus_ui::kit::{with_brush, State, Style, Tokens};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use std::time::Instant;

const FRAMES: usize = 200;

struct Cost {
    build_ms: f64,
    tess_ms: f64,
    vertices: usize,
    meshes: usize,
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn measure(ctx: &egui::Context, mut paint: impl FnMut(&mut egui::Ui)) -> Cost {
    let input = || egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0))),
        ..Default::default()
    };
    let (mut build, mut tess) = (Vec::new(), Vec::new());
    let (mut vertices, mut meshes) = (0, 0);
    for i in 0..FRAMES + 10 {
        let t0 = Instant::now();
        let mut out = ctx.run_ui(input(), |ui| paint(ui));
        let t1 = Instant::now();
        out.textures_delta.clear();
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        let t2 = Instant::now();
        if i >= 10 {
            build.push((t1 - t0).as_secs_f64() * 1e3);
            tess.push((t2 - t1).as_secs_f64() * 1e3);
        }
        vertices = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                _ => 0,
            })
            .sum();
        meshes = prims.len();
    }
    Cost {
        build_ms: median(&mut build),
        tess_ms: median(&mut tess),
        vertices,
        meshes,
    }
}

fn row(name: &str, n: usize, c: &Cost) {
    let total = c.build_ms + c.tess_ms;
    println!(
        "{name:<34} {n:>5} {:>8.3} {:>8.3} {:>8.3} {:>9.2} {:>9} {:>6}",
        c.build_ms,
        c.tess_ms,
        total,
        total * 1000.0 / n as f64,
        c.vertices,
        c.meshes
    );
}

fn grid(i: usize, cols: usize, pitch: f32) -> Pos2 {
    Pos2::new(
        20.0 + (i % cols) as f32 * pitch,
        20.0 + (i / cols) as f32 * pitch,
    )
}

fn main() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let ctx = gallery::offscreen_context();
    let s = Style::new(Tokens::slate());
    println!(
        "{:<34} {:>5} {:>8} {:>8} {:>8} {:>9} {:>9} {:>6}",
        "case", "n", "build", "tess", "total", "µs/item", "vertices", "draws"
    );
    println!("{}", "-".repeat(94));

    // 256 knobs: today's widget, the kit's painter with a cached body, with a vector body.
    let mut values = [0.5f32; 256];
    let c = measure(&ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (i, v) in values.iter_mut().enumerate() {
                *v = (i as f32 / 255.0) * 0.9 + 0.05;
                ui.add(cantus_ui::widgets::Knob::new(v, 0.0..=1.0, 0.0).label("Cut"));
            }
        });
    });
    row("256 knobs: today's widgets::Knob", 256, &c);
    let c = measure(&ctx, |ui| {
        with_brush(ui.painter(), |b| {
            for i in 0..256 {
                knob::paint(
                    b,
                    grid(i, 32, 56.0),
                    &s,
                    State::REST,
                    &KnobLook::new(KnobSize::M, i as f32 / 255.0, 0.0),
                );
            }
        })
    });
    row("256 knobs: kit, cached bodies", 256, &c);
    knob::set_cached_bodies(false);
    let c = measure(&ctx, |ui| {
        with_brush(ui.painter(), |b| {
            for i in 0..256 {
                knob::paint(
                    b,
                    grid(i, 32, 56.0),
                    &s,
                    State::REST,
                    &KnobLook::new(KnobSize::M, i as f32 / 255.0, 0.0),
                );
            }
        })
    });
    knob::set_cached_bodies(true);
    row("256 knobs: kit, bodies as vectors", 256, &c);
    let mut vals = [0.5f32; 256];
    let c = measure(&ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            for v in vals.iter_mut() {
                cantus_ui::kit::widgets::Param::new(v, 0.0..=1.0, 0.0)
                    .size(KnobSize::M)
                    .name("Cutoff")
                    .show(ui);
            }
        });
    });
    row("256 knobs: kit Param widgets", 256, &c);

    // 64 meters: batched, and the way today's mixer draws them (eight shapes each).
    let c = measure(&ctx, |ui| {
        with_brush(ui.painter(), |b| {
            let mut batch = MeterBatch::new(&s);
            for i in 0..64 {
                let r = Rect::from_min_size(
                    Pos2::new(10.0 + i as f32 * 24.0, 20.0),
                    Vec2::new(16.0, 240.0),
                );
                batch.paint(
                    b,
                    r,
                    &Levels {
                        level: [-3.0 - (i % 20) as f32, -5.0],
                        hold: [-1.0, -2.0],
                    },
                );
            }
        })
    });
    row("64 meters: one batched mesh", 64, &c);
    let c = measure(&ctx, |ui| {
        let p = ui.painter();
        for i in 0..64 {
            let r = Rect::from_min_size(
                Pos2::new(10.0 + i as f32 * 24.0, 20.0),
                Vec2::new(16.0, 240.0),
            );
            p.rect_filled(r, 2.0, s.t.meter_well);
            let bar = Rect::from_min_max(
                Pos2::new(r.left() + 2.0, r.top() + 60.0),
                Pos2::new(r.right() - 2.0, r.bottom()),
            );
            p.rect_filled(bar, 1.0, s.t.meter_low);
            p.line_segment(
                [
                    Pos2::new(r.left(), r.top() + 50.0),
                    Pos2::new(r.right(), r.top() + 50.0),
                ],
                Stroke::new(2.0, s.t.meter_high),
            );
            for k in 0..4 {
                let y = r.top() + 40.0 * k as f32 + 20.0;
                p.line_segment(
                    [Pos2::new(r.right() + 1.0, y), Pos2::new(r.right() + 4.0, y)],
                    Stroke::new(1.0, Color32::GRAY),
                );
            }
            p.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, s.t.grid_bar),
                egui::StrokeKind::Inside,
            );
        }
    });
    row("64 meters: eight shapes each", 64, &c);

    // 2,000 labels: boxed and cached, and egui's painter.text.
    let words = [
        "Cutoff",
        "Resonance",
        "Attack",
        "Release",
        "Mix",
        "Width",
        "Drive",
        "Level",
    ];
    let c = measure(&ctx, |ui| {
        with_brush(ui.painter(), |b| {
            for i in 0..2000 {
                let r = Rect::from_min_size(grid(i, 40, 46.0), Vec2::new(44.0, 16.0));
                let _ = text::paint(
                    b,
                    r,
                    &Text::new(words[i % 8], Role::Label, s.t.label).caps(true),
                    1.0,
                );
            }
        })
    });
    row("2000 labels: kit boxed text", 2000, &c);
    let c = measure(&ctx, |ui| {
        let p = ui.painter();
        for i in 0..2000 {
            let at = grid(i, 40, 46.0);
            p.text(
                at,
                egui::Align2::LEFT_TOP,
                words[i % 8],
                egui::FontId::proportional(11.0),
                s.t.label,
            );
        }
    });
    row("2000 labels: painter.text", 2000, &c);

    // Every part, 120 instances each (20 of each state).
    println!("{}", "-".repeat(94));
    for (name, part) in PARTS {
        let sl = gallery::slot(part);
        let cols = ((1900.0 / (sl.x + 4.0)) as usize).max(1);
        let c = measure(&ctx, |ui| {
            with_brush(ui.painter(), |b: &mut Brush| {
                for i in 0..120 {
                    let at = Pos2::new(
                        (i % cols) as f32 * (sl.x + 4.0),
                        (i / cols) as f32 * (sl.y + 4.0),
                    );
                    let st = State::ALL[i % 6].1;
                    gallery::paint_part(b, Rect::from_min_size(at, sl), &s, part, st);
                }
            })
        });
        row(&format!("part: {name}"), 120, &c);
    }
}
