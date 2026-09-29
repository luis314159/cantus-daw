//! What Unda's and Modulus's look tests share (ADR 0024 §14): run the instrument window
//! headless with input, read back every text it painted with its box and clip, and say
//! what is clipped, elided or overlapping inside the editor's own area.
#![allow(dead_code)]

use cantus_core::TrackId;
use cantus_ui::views::inst::View;
use egui::{Pos2, Rect};

/// A painted text: what it says, where its letters are, and the clip it was painted in.
#[derive(Clone, Debug)]
pub struct Painted {
    pub text: String,
    pub rect: Rect,
    pub clip: Rect,
}

/// Every text of a frame's shapes.
pub fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<Painted> {
    fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<Painted>) {
        match shape {
            egui::Shape::Text(t) => out.push(Painted {
                text: t.galley.text().to_string(),
                rect: t.visual_bounding_rect(),
                clip,
            }),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut out);
    }
    out
}

/// Run one frame of the instrument window with `events`, at `size`, time `t`.
pub fn frame(
    ctx: &egui::Context,
    app: &mut cantus_ui::CantusApp,
    size: egui::Vec2,
    events: Vec<egui::Event>,
    t: f64,
) -> Vec<egui::epaint::ClippedShape> {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        time: Some(t),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        cantus_ui::views::inst::window_in(app, ui);
    });
    out.textures_delta.clear();
    out.shapes
}

/// The editor's own area (above the bands, below the header), for a window at the
/// origin at scale `k` in `view`: the shell's geometry (28-pt header, 8-pt padding, the
/// 88-pt band row, the keyboard in Play, the 24-pt footer).
pub fn area(k: f32, view: View) -> Rect {
    let keys = if view == View::Play { 60.0 } else { 0.0 };
    Rect::from_min_max(
        Pos2::new(8.0 * k, (28.0 + 8.0) * k),
        Pos2::new(
            (1040.0 - 8.0) * k,
            (680.0 - 24.0 - 8.0 - 88.0 - 4.0 - keys) * k,
        ),
    )
}

/// What is wrong with the texts inside `area`: a text cut by its clip, a text shortened
/// with an ellipsis, two texts overlapping.
pub fn problems(all: &[Painted], area: Rect) -> Vec<String> {
    let mut out = Vec::new();
    // The editor's texts: those in its area, and those a scrolling view holds below the
    // fold (their clip is the view's), which must fit as well as the visible ones.
    let below = |p: &Painted| {
        (p.clip.bottom() - area.bottom()).abs() < 2.0 && p.rect.center().y > area.bottom()
    };
    let shown: Vec<&Painted> = all
        .iter()
        .filter(|p| !p.text.trim().is_empty())
        .filter(|p| area.contains(p.rect.center()) || below(p))
        .collect();
    for p in &shown {
        // A card half scrolled out of a view that scrolls is cut by the view, not clipped
        // by its own box.
        let scrolled = (p.clip.bottom() - area.bottom()).abs() < 2.0
            && p.rect.bottom() > p.clip.bottom() - 1.0;
        if !scrolled && !p.clip.expand(1.0).contains_rect(p.rect) {
            out.push(format!(
                "clipped: {:?} at {:?} in {:?}",
                p.text, p.rect, p.clip
            ));
        }
        if p.text.contains('\u{2026}') {
            out.push(format!("elided: {:?} at {:?}", p.text, p.rect));
        }
    }
    for (i, a) in shown.iter().enumerate() {
        for b in &shown[i + 1..] {
            let x = a.rect.shrink(1.0).intersect(b.rect.shrink(1.0));
            if x.width() > 0.5 && x.height() > 0.5 {
                out.push(format!(
                    "overlap: {:?} at {:?} / {:?} at {:?}",
                    a.text, a.rect, b.text, b.rect
                ));
            }
        }
    }
    out
}

/// Open `track`'s editor in `view` at scale `k`, the window at the origin, and paint it a
/// few frames; the texts of the last.
pub fn paint_at(
    ctx: &egui::Context,
    app: &mut cantus_ui::CantusApp,
    track: TrackId,
    view: View,
    k: f32,
) -> Vec<Painted> {
    cantus_ui::kit::shell::Shell::new(("inst", track.0)).set_scale(ctx, k);
    app.inst.view = view;
    let size = egui::vec2(1040.0 * k + 40.0, 680.0 * k + 40.0);
    let mut last = Vec::new();
    for i in 0..3 {
        last = frame(ctx, app, size, Vec::new(), i as f64 * 0.25);
    }
    texts(&last)
}

/// An editor's Edit body drawn whole, not in the window (which scrolls it): on the device
/// ground, in a rect as wide as the window's area, at 100 % in the Source hue.
pub fn full_body(ui: &mut egui::Ui, draw: impl FnOnce(&mut egui::Ui, &cantus_ui::kit::Style)) {
    let s = cantus_ui::kit::Style::current().function(cantus_ui::kit::Function::Source);
    let full = ui.max_rect();
    cantus_ui::kit::with_brush(ui.painter(), |b| {
        cantus_ui::kit::shell::paint_ground(b, full, &s)
    });
    let area = Rect::from_min_size(
        full.min + egui::vec2(8.0, 8.0),
        egui::vec2(1024.0, full.height() - 16.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    draw(&mut child, &s);
}

/// Press at `from`, move to `to` in `steps`, release: the frames of a drag.
pub fn drag(
    ctx: &egui::Context,
    app: &mut cantus_ui::CantusApp,
    size: egui::Vec2,
    from: Pos2,
    to: Pos2,
    steps: usize,
    t0: f64,
) -> f64 {
    let mut t = t0;
    let press = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(ctx, app, size, vec![egui::Event::PointerMoved(from)], t);
    t += 0.05;
    frame(ctx, app, size, vec![press(from, true)], t);
    for i in 1..=steps {
        t += 0.05;
        let p = from + (to - from) * (i as f32 / steps as f32);
        frame(ctx, app, size, vec![egui::Event::PointerMoved(p)], t);
    }
    t += 0.05;
    frame(ctx, app, size, vec![press(to, false)], t);
    t += 0.05;
    frame(ctx, app, size, Vec::new(), t);
    t
}
