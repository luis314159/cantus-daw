//! Spatium's curve display: decay time against frequency, in seconds, with the post EQ on
//! a second axis. The display is the control: drag a band's handle, wheel for its width,
//! double-click to make one (its shape follows where), Alt-click to bypass, right-click
//! for its menu, Delete to remove it.
//!
//! Four things are drawn so they can be told apart without colour: the **curve** asked
//! for (the thick line, filled against the curve before the drawn bands), the curve
//! **before the bands** (dashed), what the **engine achieves** (dotted, from its fitted
//! filters), and what was **measured** on the last tail (hollow squares).

use super::{achieved_t60, band_key, band_label, band_used, seconds, Curve, View};
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use cantus_core::fx::space::{DECAY_BANDS, POST_BANDS};
use cantus_dsp::spatium::OCTAVES;
use cantus_dsp::Aequalis;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};

const LO_HZ: f32 = 20.0;
const HI_HZ: f32 = 20_000.0;
const POINTS: usize = 200;
const DOT_R: f32 = 7.0;
/// The post EQ's axis, ± dB.
const POST_DB: f32 = 18.0;

/// A handle being dragged: which band, and where it and the pointer started.
#[derive(Copy, Clone, Debug)]
pub struct Drag {
    curve: Curve,
    band: usize,
    freq: f32,
    y0: f32,
    origin: Pos2,
}

/// The seconds axis for a decay: a tenth of it to five times it, so the curve sits in
/// the same place whatever the decay and the labels say where that is.
pub fn axis_seconds(decay: f32) -> (f32, f32) {
    ((decay / 10.0).max(0.02), (decay * 5.0).min(120.0))
}

/// A band's colour: the decay bands from the start of the rotation, the post EQ's six
/// further on, so the two curves' handles never share one.
pub fn band_color(curve: Curve, n: usize) -> Color32 {
    match curve {
        Curve::Decay => crate::views::eq_editor::band_color(n),
        Curve::Post => crate::views::eq_editor::band_color(n + DECAY_BANDS),
    }
}

/// The post EQ's colour: the selection hue, which every palette keeps distinct from the
/// gold of the decay curve.
fn post_color() -> Color32 {
    theme::p().selection
}

#[derive(Copy, Clone)]
struct Geo {
    rect: Rect,
    lo: f32,
    hi: f32,
}

impl Geo {
    fn x(&self, hz: f32) -> f32 {
        let f = (hz.max(1.0) / LO_HZ).ln() / (HI_HZ / LO_HZ).ln();
        self.rect.left() + f * self.rect.width()
    }
    fn hz(&self, x: f32) -> f32 {
        let f = (x - self.rect.left()) / self.rect.width().max(1.0);
        LO_HZ * (HI_HZ / LO_HZ).powf(f)
    }
    fn y(&self, sec: f32) -> f32 {
        let f = (sec.max(1e-3) / self.lo).ln() / (self.hi / self.lo).ln();
        self.rect.bottom() - f.clamp(-0.05, 1.05) * self.rect.height()
    }
    fn sec(&self, y: f32) -> f32 {
        let f = (self.rect.bottom() - y) / self.rect.height().max(1.0);
        self.lo * (self.hi / self.lo).powf(f)
    }
    fn y_db(&self, db: f32) -> f32 {
        self.rect.center().y - (db / POST_DB).clamp(-1.1, 1.1) * self.rect.height() * 0.5
    }
    fn db(&self, y: f32) -> f32 {
        (self.rect.center().y - y) / (self.rect.height() * 0.5) * POST_DB
    }
}

/// A tick on the seconds axis: `50 ms`, `1 s`, `20 s`.
fn axis_label(v: f32) -> String {
    if v < 1.0 {
        format!("{:.0} ms", v * 1_000.0)
    } else {
        format!("{v:.0} s")
    }
}

fn point_hz(i: usize) -> f32 {
    LO_HZ * (HI_HZ / LO_HZ).powf(i as f32 / (POINTS - 1) as f32)
}

/// The post EQ's summed response at `hz`, dB (without the automatic gain).
fn post_db(view: &View, hz: f32) -> f32 {
    if !view.s.post_eq_on {
        return 0.0;
    }
    view.s
        .post_bands
        .iter()
        .filter(|b| b.active)
        .map(|b| Aequalis::band_response_db(b, 48_000.0, hz))
        .sum()
}

/// Where a band's handle sits.
fn handle(view: &View, geo: &Geo, curve: Curve, n: usize) -> Pos2 {
    let spec = view.spec();
    let freq = spec.get(&band_key(curve, n, "freq"));
    match curve {
        Curve::Decay => {
            let target = view.s.target_curve();
            let y = if spec.get(&band_key(curve, n, "shape")).round() as usize == 3 {
                geo.rect.bottom() - DOT_R - 2.0
            } else {
                geo.y(target.base_t60(freq) * spec.get(&band_key(curve, n, "mult")))
            };
            pos2(geo.x(freq), y)
        }
        Curve::Post => {
            let shape = spec.get(&band_key(curve, n, "shape")).round() as usize;
            let gain = if shape < 3 {
                spec.get(&band_key(curve, n, "gain"))
            } else {
                0.0
            };
            pos2(geo.x(freq), geo.y_db(gain))
        }
    }
}

/// The bands that exist on a curve.
fn bands(view: &View, curve: Curve) -> Vec<usize> {
    let n = match curve {
        Curve::Decay => DECAY_BANDS,
        Curve::Post => POST_BANDS,
    };
    (0..n)
        .filter(|b| band_used(view.spec(), curve, *b))
        .collect()
}

/// The row above the display: which curve the gestures edit, each curve's bypass, and
/// the post EQ's gain compensation with the amount it applies.
fn header(app: &mut CantusApp, ui: &mut egui::Ui, view: &View) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        let mode = app.fx.space.mode;
        for (curve, key) in [
            (Curve::Decay, "space.curve.decay"),
            (Curve::Post, "space.curve.post"),
        ] {
            let text = RichText::new(t(key)).color(if curve == Curve::Decay {
                pal.gold
            } else {
                post_color()
            });
            if ui
                .selectable_label(mode == curve, text)
                .on_hover_text(t(&format!("{key}_tip")))
                .clicked()
            {
                app.fx.space.mode = curve;
            }
        }
        ui.separator();
        for (key, label) in [
            ("deq_on", "space.curve.decay_on"),
            ("peq_on", "space.curve.post_on"),
            ("peq_auto", "space.curve.auto"),
        ] {
            let mut on = view.spec().get(key) >= 0.5;
            if ui
                .checkbox(&mut on, t(label))
                .on_hover_text(t(&format!("{label}_tip")))
                .changed()
            {
                app.set_fx_param(
                    view.strip,
                    view.effect,
                    key,
                    if on { 1.0 } else { 0.0 },
                    false,
                );
            }
        }
        if view.s.post_eq_on && view.s.post_auto_gain {
            let (_, comp) = view.s.wet_eq();
            if comp.abs() >= 0.05 {
                ui.label(
                    RichText::new(format!("{comp:+.1} dB"))
                        .text_style(design::mono_small())
                        .color(pal.text_dim),
                );
            }
        }
    });
}

pub fn display(app: &mut CantusApp, ui: &mut egui::Ui, view: &View, size: egui::Vec2) {
    let pal = theme::p();
    header(app, ui, view);
    let h = size.y - ui.spacing().interact_size.y - ui.spacing().item_spacing.y;
    let (rect, resp) = ui.allocate_exact_size(vec2(size.x, h), Sense::click_and_drag());
    let (lo, hi) = axis_seconds(view.s.decay_s);
    let geo = Geo { rect, lo, hi };
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));

    // Grid: decades and their 2/5 marks; seconds on a 1-2-5 ladder at the left; the post
    // EQ's decibels at the right, in its own colour.
    for decade in [10.0f32, 100.0, 1_000.0, 10_000.0] {
        for m in [1.0, 2.0, 5.0] {
            let hz = decade * m;
            if !(LO_HZ..=HI_HZ).contains(&hz) {
                continue;
            }
            let strong = m == 1.0;
            p.vline(
                geo.x(hz),
                rect.y_range(),
                Stroke::new(1.0, if strong { pal.grid_strong } else { pal.grid }),
            );
            p.text(
                pos2(geo.x(hz) + 3.0, rect.bottom() - 2.0),
                Align2::LEFT_BOTTOM,
                crate::widgets::knob::readout(hz, "Hz"),
                design::paint::ruler(),
                pal.text_dim,
            );
        }
    }
    for v in [
        0.01f32, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0,
    ] {
        if v < lo * 0.999 || v > hi * 1.001 {
            continue;
        }
        let y = geo.y(v);
        p.hline(rect.x_range(), y, Stroke::new(1.0, pal.grid));
        p.text(
            pos2(rect.left() + 3.0, y - 1.0),
            Align2::LEFT_BOTTOM,
            axis_label(v),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    if view.s.post_eq_on {
        for db in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
            p.text(
                pos2(rect.right() - 3.0, geo.y_db(db) - 1.0),
                Align2::RIGHT_BOTTOM,
                if db == 0.0 {
                    "0".to_string()
                } else {
                    format!("{db:+.0}")
                },
                design::paint::ruler(),
                post_color(),
            );
        }
    }

    // The post EQ: a dashed line in its own colour, faintly filled against 0 dB.
    if view.s.post_eq_on && view.s.post_bands.iter().any(|b| b.active) {
        let pts: Vec<Pos2> = (0..POINTS)
            .map(|i| pos2(geo.x(point_hz(i)), geo.y_db(post_db(view, point_hz(i)))))
            .collect();
        let zero = geo.y_db(0.0);
        let mut mesh = egui::Mesh::default();
        let fill = post_color().gamma_multiply(if pal.dark { 0.09 } else { 0.07 });
        for (k, pt) in pts.iter().enumerate() {
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(*pt, fill);
            mesh.colored_vertex(pos2(pt.x, zero), fill);
            if k > 0 {
                mesh.add_triangle(base - 2, base - 1, base);
                mesh.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(mesh);
        p.add(egui::Shape::dashed_line(
            &pts,
            Stroke::new(1.75, post_color()),
            7.0,
            3.0,
        ));
    }

    // The decay curve: before the bands (dashed), as asked (thick, filled against the
    // former), and as achieved (dotted).
    let curve = view.s.target_curve();
    let base: Vec<Pos2> = (0..POINTS)
        .map(|i| pos2(geo.x(point_hz(i)), geo.y(curve.base_t60(point_hz(i)))))
        .collect();
    let target: Vec<Pos2> = (0..POINTS)
        .map(|i| pos2(geo.x(point_hz(i)), geo.y(curve.t60(point_hz(i)))))
        .collect();
    let mut mesh = egui::Mesh::default();
    let fill = pal.gold.gamma_multiply(if pal.dark { 0.22 } else { 0.16 });
    for (k, (a, b)) in target.iter().zip(&base).enumerate() {
        let i0 = mesh.vertices.len() as u32;
        mesh.colored_vertex(*a, fill);
        mesh.colored_vertex(*b, fill);
        if k > 0 {
            mesh.add_triangle(i0 - 2, i0 - 1, i0);
            mesh.add_triangle(i0 - 1, i0, i0 + 1);
        }
    }
    p.add(mesh);
    p.add(egui::Shape::dashed_line(
        &base,
        Stroke::new(1.0, pal.text_dim),
        5.0,
        4.0,
    ));
    let st = &app.fx.space;
    let achieved: Vec<Pos2> = (0..POINTS)
        .step_by(2)
        .filter_map(|i| {
            let hz = point_hz(i);
            (31.0..=16_000.0)
                .contains(&hz)
                .then(|| achieved_t60(st, view, hz).map(|s| pos2(geo.x(hz), geo.y(s))))
                .flatten()
        })
        .collect();
    p.add(egui::Shape::line(
        target.clone(),
        Stroke::new(2.5, pal.gold),
    ));
    if achieved.len() > 1 {
        p.add(egui::Shape::dashed_line(
            &achieved,
            Stroke::new(1.25, pal.text),
            2.0,
            3.0,
        ));
    }
    // Measured on the last tail: hollow squares at the octaves.
    // An octave whose tail was cut before it fell far enough is extrapolated: dimmer.
    if let Some(m) = &st.capture.measured {
        for ((hz, t60), complete) in OCTAVES.iter().zip(m.t60).zip(m.complete) {
            if let Some(t60) = t60 {
                let c = pos2(geo.x(*hz), geo.y(t60));
                let color = if complete { pal.text } else { pal.text_dim };
                p.rect_stroke(
                    Rect::from_center_size(c, vec2(8.0, 8.0)),
                    1.0,
                    Stroke::new(1.5, color),
                    StrokeKind::Middle,
                );
            }
        }
    }

    // Handles: the decay bands as numbered circles, the post EQ's as lettered squares.
    let pointer = resp.hover_pos();
    let selected = st.selected;
    let mut hovered: Option<(Curve, usize)> = None;
    let mut handles = Vec::new();
    for curve_kind in [Curve::Post, Curve::Decay] {
        if (curve_kind == Curve::Decay && !view.s.decay_eq_on)
            || (curve_kind == Curve::Post && !view.s.post_eq_on)
        {
            continue;
        }
        for n in bands(view, curve_kind) {
            let c = handle(view, &geo, curve_kind, n);
            handles.push((curve_kind, n, c));
            if pointer.is_some_and(|pt| pt.distance(c) <= DOT_R + 3.0) {
                hovered = Some((curve_kind, n));
            }
        }
    }
    for (curve_kind, n, c) in &handles {
        let color = band_color(*curve_kind, *n);
        let on = view.spec().get(&band_key(*curve_kind, *n, "on")) >= 0.5;
        match curve_kind {
            Curve::Decay => {
                if on {
                    p.circle_filled(*c, DOT_R, color);
                } else {
                    p.circle_stroke(*c, DOT_R, Stroke::new(1.5, color));
                }
            }
            Curve::Post => {
                let r = Rect::from_center_size(*c, vec2(DOT_R * 2.0, DOT_R * 2.0));
                if on {
                    p.rect_filled(r, 3.0, color);
                } else {
                    p.rect_stroke(r, 3.0, Stroke::new(1.5, color), StrokeKind::Middle);
                }
            }
        }
        if selected == Some((*curve_kind, *n)) || hovered == Some((*curve_kind, *n)) {
            p.circle_stroke(*c, DOT_R + 3.0, Stroke::new(1.5, pal.text));
        }
        p.text(
            *c,
            Align2::CENTER_CENTER,
            band_label(*curve_kind, *n),
            design::paint::ruler(),
            if on {
                theme::on_color(&pal, color)
            } else {
                pal.text
            },
        );
    }

    // The readout where the pointer is: frequency, note, the curve, the engine, the EQ.
    if let Some(pt) = pointer {
        let hz = geo.hz(pt.x);
        p.vline(
            pt.x,
            rect.y_range(),
            Stroke::new(1.0, pal.text_dim.gamma_multiply(0.5)),
        );
        let mut text = format!(
            "{}  \u{b7}  {}  \u{b7}  {}",
            crate::widgets::knob::readout(hz, "Hz"),
            t("space.curve.decay"),
            seconds(curve.t60(hz))
        );
        if let Some(a) = achieved_t60(st, view, hz) {
            text.push_str(&format!("  \u{b7}  {} {}", t("space.engine"), seconds(a)));
        }
        if view.s.post_eq_on && view.s.post_bands.iter().any(|b| b.active) {
            text.push_str(&format!("  \u{b7}  EQ {:+.1} dB", post_db(view, hz)));
        }
        p.text(
            pos2(
                rect.left() + space::S2 + space::GRID * 9.0,
                rect.top() + space::S2,
            ),
            Align2::LEFT_TOP,
            text,
            design::paint::ruler(),
            pal.text,
        );
    } else if handles.is_empty() {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            t("space.curve.empty"),
            design::paint::empty_state(),
            pal.text_dim,
        );
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
    gestures(app, ui, view, &geo, &resp, hovered);
}

/// Make a band at the first free number on a curve, shaped by where the pointer is.
fn add_band(app: &mut CantusApp, view: &View, geo: &Geo, curve: Curve, pt: Pos2) {
    let limit = match curve {
        Curve::Decay => DECAY_BANDS,
        Curve::Post => POST_BANDS,
    };
    let Some(n) = (0..limit).find(|b| !band_used(view.spec(), curve, *b)) else {
        app.set_status(t("space.no_free_band"));
        return;
    };
    let fx = (pt.x - geo.rect.left()) / geo.rect.width();
    let fy = (pt.y - geo.rect.top()) / geo.rect.height();
    let freq = geo.hz(pt.x).clamp(20.0, 20_000.0);
    let key = |f: &str| band_key(curve, n, f);
    let mut fields = vec![(key("used"), 1.0), (key("freq"), freq)];
    match curve {
        Curve::Decay => {
            // Edges make shelves; the floor makes a notch; anywhere else a bell.
            let shape = if fx < 0.1 {
                1.0
            } else if fx > 0.9 {
                2.0
            } else if fy > 0.92 {
                3.0
            } else {
                0.0
            };
            fields.push((key("shape"), shape));
            if shape != 3.0 {
                let base = view.s.target_curve().base_t60(freq);
                fields.push((key("mult"), (geo.sec(pt.y) / base).clamp(0.25, 4.0)));
            } else {
                fields.push((key("q"), 4.0));
            }
        }
        Curve::Post => {
            let shape = if fx < 0.05 {
                3.0
            } else if fx > 0.95 {
                4.0
            } else if fx < 0.12 {
                1.0
            } else if fx > 0.88 {
                2.0
            } else {
                0.0
            };
            fields.push((key("shape"), shape));
            if shape < 3.0 {
                fields.push((key("gain"), geo.db(pt.y).clamp(-18.0, 18.0)));
            }
        }
    }
    app.set_space_params(view.strip, view.effect, &fields, false);
    app.fx.space.selected = Some((curve, n));
    app.fx.space.mode = curve;
}

fn gestures(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    view: &View,
    geo: &Geo,
    resp: &egui::Response,
    hovered: Option<(Curve, usize)>,
) {
    let fine = ui.input(|i| i.modifiers.shift);
    let alt = ui.input(|i| i.modifiers.alt);
    if resp.drag_started() {
        if let (Some((curve, n)), Some(origin)) = (hovered, resp.interact_pointer_pos()) {
            let c = handle(view, geo, curve, n);
            app.fx.space.drag = Some(Drag {
                curve,
                band: n,
                freq: view.spec().get(&band_key(curve, n, "freq")),
                y0: c.y,
                origin,
            });
            app.fx.space.selected = Some((curve, n));
            app.fx.space.mode = curve;
        }
    }
    if let (Some(d), Some(pt)) = (app.fx.space.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let scale = if fine { 0.2 } else { 1.0 };
            let delta = (pt - d.origin) * scale;
            let freq = geo.hz(geo.x(d.freq) + delta.x).clamp(20.0, 20_000.0);
            let y = d.y0 + delta.y;
            let key = |f: &str| band_key(d.curve, d.band, f);
            let shape = view.spec().get(&key("shape")).round() as usize;
            let mut fields = vec![(key("freq"), freq)];
            match d.curve {
                Curve::Decay if shape != 3 => {
                    let base = view.s.target_curve().base_t60(freq);
                    fields.push((key("mult"), (geo.sec(y) / base).clamp(0.25, 4.0)));
                }
                Curve::Post if shape < 3 => {
                    fields.push((key("gain"), geo.db(y).clamp(-18.0, 18.0)));
                }
                _ => {}
            }
            app.set_space_params(view.strip, view.effect, &fields, true);
        }
    }
    if resp.drag_stopped() {
        app.fx.space.drag = None;
        app.end_gesture();
    }
    if resp.clicked() {
        match hovered {
            Some((curve, n)) if alt => {
                let key = band_key(curve, n, "on");
                let on = view.spec().get(&key) >= 0.5;
                app.set_space_params(
                    view.strip,
                    view.effect,
                    &[(key, if on { 0.0 } else { 1.0 })],
                    false,
                );
            }
            Some((curve, n)) => {
                app.fx.space.selected = Some((curve, n));
                app.fx.space.mode = curve;
            }
            None => app.fx.space.selected = None,
        }
    }
    if resp.double_clicked() && hovered.is_none() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let curve = app.fx.space.mode;
            add_band(app, view, geo, curve, pt);
        }
    }
    // The wheel over a handle: its width (Q), or a cut's slope in 6 dB steps.
    if let Some((curve, n)) = hovered {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let shape = view.spec().get(&band_key(curve, n, "shape")).round() as usize;
            if curve == Curve::Post && shape >= 3 {
                let slope = view.spec().get(&band_key(curve, n, "slope"));
                let step = if scroll > 0.0 { 6.0 } else { -6.0 };
                app.set_space_params(
                    view.strip,
                    view.effect,
                    &[(band_key(curve, n, "slope"), (slope + step).clamp(6.0, 96.0))],
                    false,
                );
            } else {
                let q = view.spec().get(&band_key(curve, n, "q"));
                let k = if fine { 0.0015 } else { 0.006 };
                app.set_space_params(
                    view.strip,
                    view.effect,
                    &[(
                        band_key(curve, n, "q"),
                        (q * (scroll * k).exp2()).clamp(0.1, 10.0),
                    )],
                    true,
                );
            }
        }
    }
    let menu_band = hovered.or(app.fx.space.selected);
    resp.context_menu(|ui| {
        let Some((curve, n)) = menu_band.filter(|(c, n)| band_used(view.spec(), *c, *n)) else {
            ui.label(t("space.curve.empty"));
            return;
        };
        let title = match curve {
            Curve::Decay => tf("space.decay_band_n", &[("n", &band_label(curve, n))]),
            Curve::Post => tf("space.post_band_n", &[("n", &band_label(curve, n))]),
        };
        ui.label(RichText::new(title).color(band_color(curve, n)));
        let (count, prefix) = match curve {
            Curve::Decay => (4, "space.dshape"),
            Curve::Post => (5, "space.pshape"),
        };
        let shape = view.spec().get(&band_key(curve, n, "shape")).round() as usize;
        for i in 0..count {
            if ui
                .selectable_label(shape == i, t(&format!("{prefix}.{i}")))
                .clicked()
            {
                app.set_space_params(
                    view.strip,
                    view.effect,
                    &[(band_key(curve, n, "shape"), i as f32)],
                    false,
                );
                ui.close();
            }
        }
        ui.separator();
        if ui.button(t("space.remove_band")).clicked() {
            app.remove_space_band(view.strip, view.effect, curve, n);
            ui.close();
        }
    });
    if let Some((curve, n)) = app.fx.space.selected {
        if resp.hovered() && ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            app.remove_space_band(view.strip, view.effect, curve, n);
        }
    }
    if hovered.is_some() || app.fx.space.drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
}

/// Decay in seconds per octave, three ways, always on screen: the curve asked for, what
/// the engine's filters achieve, and what was measured on the last tail.
pub fn octave_table(app: &mut CantusApp, ui: &mut egui::Ui, view: &View, size: egui::Vec2) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, pal.panel_raised);
    let label_w = space::GRID * 20.0;
    let col = (rect.width() - label_w) / OCTAVES.len() as f32;
    let row = rect.height() / 4.0;
    let curve = view.s.target_curve();
    let st = &app.fx.space;
    let measured = st.capture.measured.as_ref();
    let rows: [(String, Color32); 4] = [
        ("Hz".into(), pal.text_dim),
        (t("space.table.curve"), pal.gold),
        (t("space.table.engine"), pal.text),
        (t("space.table.measured"), pal.text),
    ];
    for (r, (label, color)) in rows.iter().enumerate() {
        let y = rect.top() + row * (r as f32 + 0.5);
        p.text(
            pos2(rect.left() + space::S2, y),
            Align2::LEFT_CENTER,
            label,
            design::paint::ruler(),
            *color,
        );
        for (i, hz) in OCTAVES.iter().enumerate() {
            let x = rect.left() + label_w + col * (i as f32 + 0.5);
            // A measured octave whose tail was cut before it fell far enough is an estimate —
            // "≈", dimmer — or, when too little of it was heard to estimate, "cut" in
            // words rather than a bare dash (ADR 0022).
            let cut = r == 3 && measured.is_some_and(|m| !m.complete[i]);
            let text = match r {
                0 => crate::widgets::knob::readout(*hz, "Hz").replace(" Hz", ""),
                1 => format!("{:.2}", curve.t60(*hz)),
                2 => achieved_t60(st, view, *hz)
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "\u{2013}".into()),
                _ => match measured.and_then(|m| m.t60[i]) {
                    Some(v) if cut => format!("\u{2248}{v:.2}"),
                    Some(v) => format!("{v:.2}"),
                    None if cut => t("space.table.cut"),
                    None => "\u{2013}".into(),
                },
            };
            p.text(
                pos2(x, y),
                Align2::CENTER_CENTER,
                text,
                design::paint::ruler(),
                if cut { pal.text_dim } else { *color },
            );
        }
    }
    p.hline(
        rect.x_range(),
        rect.top() + row,
        Stroke::new(1.0, pal.grid_strong),
    );
    if measured.is_none() {
        p.text(
            pos2(rect.right() - space::S2, rect.top() + row * 3.5),
            Align2::RIGHT_CENTER,
            t("space.table.measured_hint"),
            design::paint::caption(),
            pal.text_dim,
        );
    }
    // Where the tail was cut, the table explains it on hover.
    if measured.is_some_and(|m| m.complete.iter().any(|c| !c)) {
        let _ = resp.on_hover_text(t("space.table.cut_tip"));
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
}
