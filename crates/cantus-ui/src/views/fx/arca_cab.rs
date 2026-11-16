//! Arca's cabinet (ADR 0137, provisional): a dark screen with the cabinet chosen (a list:
//! none, Arca's five, or the loaded file) and the microphone's kind along its top; the
//! cabinet's front in small, the speaker the microphone faces lit; that speaker's cone drawn
//! large with **the microphone on it**, dragged from the dust cap to the cone's edge; beside
//! it **the side view**, the microphone dragged back from the grille cloth (1 to 30 cm); and
//! under them **the response** the choice makes, as the DSP designs it (`cab::design_db`).
//! Moving the microphone makes a new response off the audio thread, swapped in whole.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::words;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::skin::cabinets as pal;
use crate::kit::text::{self as ktext, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_dsp::arca::cab::{design_db, CabSettings, Cabinet, Mic};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// What the cabinet's screen remembers: which microphone handle is being dragged.
#[derive(Debug, Default)]
pub struct View {
    drag: Option<Handle>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Handle {
    /// On the cone: the position.
    Cone,
    /// In the side view: the distance (and the position, up and down).
    Side,
}

/// The screen's parts.
struct Parts {
    band: Rect,
    /// The cabinet's front in small.
    front: Rect,
    /// The cone, large: its centre and radius.
    cone: (Pos2, f32),
    side: Rect,
    response: Rect,
}

fn parts(r: Rect, k: f32) -> Parts {
    let band = Rect::from_min_size(r.min, vec2(r.width(), 34.0 * k));
    let resp_h = 112.0 * k;
    let side_h = 74.0 * k;
    let response = Rect::from_min_max(pos2(r.left(), r.bottom() - resp_h), r.max);
    let side = Rect::from_min_max(
        pos2(r.left(), response.top() - side_h),
        pos2(r.right(), response.top()),
    );
    let main = Rect::from_min_max(pos2(r.left(), band.bottom()), pos2(r.right(), side.top()));
    let front = Rect::from_min_size(
        pos2(main.left() + 14.0 * k, main.top() + 26.0 * k),
        vec2(104.0 * k, 104.0 * k),
    );
    let rad = ((main.height() - 24.0 * k) * 0.5).min(116.0 * k);
    let cx = (front.right() + main.right()) * 0.5 + 6.0 * k;
    Parts {
        band,
        front,
        cone: (pos2(cx, main.center().y + 2.0 * k), rad),
        side,
        response,
    }
}

/// The cone's playing area (inside its surround) for a radius.
fn cone_r(rad: f32) -> f32 {
    rad * 0.80
}

/// The side view's span of distances: the grille's x and the far end's.
fn side_span(side: Rect, k: f32) -> (f32, f32) {
    (side.left() + 70.0 * k, side.right() - 74.0 * k)
}

/// The x of a distance (cm) in the side view: a log scale, 1 to 30 cm.
fn side_x(side: Rect, k: f32, cm: f32) -> f32 {
    let (a, z) = side_span(side, k);
    a + (z - a) * (cm.clamp(1.0, 30.0).ln() / 30f32.ln())
}

fn mic_names_rects(ctx: &egui::Context, band: Rect, k: f32) -> [Rect; 3] {
    let mut x = band.right() - 10.0 * k;
    let mut out = [Rect::NOTHING; 3];
    for i in (0..3).rev() {
        let t = Text::key(words::MICS[i], Role::Control, Color32::TRANSPARENT);
        let w = ktext::measure(ctx, &t, k).x + 30.0 * k;
        out[i] = Rect::from_min_size(
            pos2(x - w, band.top() + 5.0 * k),
            vec2(w, band.height() - 10.0 * k),
        );
        x -= w + 4.0 * k;
    }
    out
}

/// The settings the screen draws: the spec's, with Arca's cabinet when a file plays.
fn cab_settings(spec: &cantus_core::EffectSpec) -> (usize, CabSettings) {
    let c = (spec.get("cab").round().max(0.0) as usize).min(6);
    let set = CabSettings {
        cabinet: Cabinet::from_index(c.clamp(1, 5) - 1),
        mic: Mic::from_index((spec.get("mic").round().max(0.0) as usize).min(2)),
        position: spec.get("micpos").clamp(0.0, 1.0),
        distance_cm: spec.get("micdist").clamp(1.0, 30.0),
    };
    (c, set)
}

/// The cabinet's screen in `r`.
pub fn show(host: &mut dyn FxHost, v: &mut View, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let k = s.k;
    let p = parts(r, k);
    let ink = Ink::of(s);
    let mics = mic_names_rects(ui.ctx(), p.band, k);
    // The field and the microphone's kind answer first, the drawing goes under them.
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
    });
    cab_field(host, ui, s, p.band, mics[0].left());
    mic_edit(host, ui, s, &mics);
    let (c, set) = cab_settings(host.spec());
    let factory = (1..=5).contains(&c);
    if factory {
        mic_drag(host, v, ui, &p, k);
    }
    let file = host.spec().ir.as_ref().map(|f| f.name().to_string());
    with_brush(&ui.painter_at(r), |b| {
        paint_mics(b, &ink, &mics, set.mic, factory, k);
        paint_front(b, &ink, p.front, c, k);
        match c {
            0 => note(b, &ink, p.cone.0, "arca.cab.none", k),
            6 => {
                let key = if file.is_some() {
                    "arca.cab.file_own_mic"
                } else {
                    "arca.cab.file_none"
                };
                note(b, &ink, p.cone.0, key, k);
                if let Some(name) = &file {
                    canvas::put(
                        b,
                        p.cone.0 + vec2(0.0, 22.0 * k),
                        Align2::CENTER_CENTER,
                        &Text::new(name, Role::Value, ink.hue).caps(false),
                        k,
                        None,
                    );
                }
            }
            _ => {
                paint_cone(b, p.cone.0, p.cone.1, k);
                paint_mic_front(b, &ink, p.cone, &set, k);
            }
        }
        paint_side(b, &ink, p.side, &set, factory, k);
        paint_response(b, &ink, p.response, &set, factory, k);
        b.hline(
            r.left() + 10.0 * k,
            r.right() - 10.0 * k,
            p.band.bottom(),
            0.7 * k,
            ink.grid,
        );
        b.hline(
            r.left() + 10.0 * k,
            r.right() - 10.0 * k,
            p.side.top(),
            0.7 * k,
            ink.grid,
        );
        canvas::edge_b(b, r, &ink);
    });
}

/// A line of words in the cone's place.
fn note(b: &mut Brush, ink: &Ink, at: Pos2, key: &str, k: f32) {
    canvas::put(
        b,
        at,
        Align2::CENTER_CENTER,
        &Text::key(key, Role::Body, ink.dim),
        k,
        None,
    );
}

/// The cabinet's list: none, Arca's five, the loaded file (then a file is loaded).
fn cab_field(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, band: Rect, until: f32) {
    let k = s.k;
    // As wide as the room the microphones leave it (their names are longer in Spanish).
    let w = ((until - band.left() - 22.0 * k) / k).clamp(120.0, 200.0);
    let at = Rect::from_min_size(
        pos2(band.left() + 10.0 * k, band.top() + 6.0 * k),
        vec2(w * k, band.height() - 12.0 * k),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(at)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let ui = &mut child;
    let spec = host.spec();
    let now = (spec.get("cab").round().max(0.0) as usize).min(6);
    let value = words::cab_name(spec, now);
    let id = ui.id().with("arca-cab");
    let name = frame::tr("fx.arca.cab");
    if let Some(at) = controls::field_key(ui, s, host, "cab", id, &value, name, w) {
        let items: Vec<_> = words::CABS
            .iter()
            .enumerate()
            .map(|(i, key)| frame::radio(Src::Key(key), i == now))
            .collect();
        if let Some(i) = controls::list(ui, s, id, at, w + 40.0, &items) {
            if i == 6 && host.spec().ir.is_none() {
                load_file(host);
            }
            host.set("cab", i as f32, None);
        }
    }
}

/// Load a response from a file (in Cantus: the file dialog; Aula's store reads it).
fn load_file(host: &mut dyn FxHost) {
    let Some((app, strip, effect)) = host.daw() else {
        return;
    };
    if let Some(p) = rfd::FileDialog::new()
        .add_filter("Audio", &super::super::aula::EXTENSIONS)
        .pick_file()
    {
        super::super::aula::set_file(app, strip, effect, Some(p.to_string_lossy().to_string()));
    }
}

/// The microphone's kind: a click chooses one.
fn mic_edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rects: &[Rect; 3]) {
    let now = (host.spec().get("mic").round().max(0.0) as usize).min(2);
    let name = frame::tr("fx.arca.mic");
    for (i, r) in rects.iter().enumerate() {
        let resp = ui.interact(*r, ui.id().with(("arca-mic", i)), Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, "mic", name, &resp);
        if resp.clicked() && !learnt && i != now {
            host.set("mic", i as f32, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr(words::MICS[i]),
                frame::tr("arca.cab.mic_tip"),
                Caps::NONE,
            );
        }
    }
}

/// The microphone dragged: on the cone, across it (the position); in the side view, along
/// (the distance) and up and down (the position).
fn mic_drag(host: &mut dyn FxHost, v: &mut View, ui: &mut egui::Ui, p: &Parts, k: f32) {
    let (c, rad) = p.cone;
    let cone = Rect::from_center_size(c, vec2(rad * 2.0, rad * 2.0));
    let g1 = ui.interact(cone, ui.id().with("arca-cone"), Sense::click_and_drag());
    let g2 = ui.interact(p.side, ui.id().with("arca-side"), Sense::click_and_drag());
    if g1.drag_started() || g1.clicked() {
        v.drag = Some(Handle::Cone);
    }
    if g2.drag_started() || g2.clicked() {
        v.drag = Some(Handle::Side);
    }
    let pointer = g1
        .interact_pointer_pos()
        .or(g2.interact_pointer_pos())
        .filter(|_| g1.is_pointer_button_down_on() || g2.is_pointer_button_down_on());
    let spec = host.spec();
    let (pos, dist) = (spec.get("micpos"), spec.get("micdist"));
    if let (Some(h), Some(pt)) = (v.drag, pointer) {
        let side_c = p.side.center().y;
        let new_pos = match h {
            Handle::Cone => ((pt - c).length() / cone_r(rad)).clamp(0.0, 1.0),
            Handle::Side => ((side_c - pt.y) / (p.side.height() * 0.36)).clamp(0.0, 1.0),
        };
        let quant = |x: f32| (x * 100.0).round() / 100.0;
        if (quant(new_pos) - pos).abs() > 1e-4 {
            host.set("micpos", quant(new_pos), Some("micpos"));
        }
        if h == Handle::Side {
            let (a, z) = side_span(p.side, k);
            let t = ((pt.x - a) / (z - a)).clamp(0.0, 1.0);
            let cm = (t * 30f32.ln()).exp();
            let cm = (cm * 10.0).round() / 10.0;
            if (cm - dist).abs() > 1e-4 {
                host.set("micdist", cm.clamp(1.0, 30.0), Some("micdist"));
            }
        }
    }
    let released = (g1.drag_stopped() || g2.drag_stopped() || g1.clicked() || g2.clicked())
        && v.drag.is_some();
    if released {
        v.drag = None;
        host.end_gesture();
    }
    if g1.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("fx.arca.micpos"),
            frame::tr("arca.cab.cone_tip"),
            Caps::NONE,
        );
    }
    if g2.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("fx.arca.micdist"),
            frame::tr("arca.cab.side_tip"),
            Caps::NONE,
        );
    }
}

/// The microphones' names along the band, a small sign of each beside its name.
fn paint_mics(b: &mut Brush, ink: &Ink, rects: &[Rect; 3], now: Mic, on: bool, k: f32) {
    for (i, r) in rects.iter().enumerate() {
        let lit = on && Mic::from_index(i) == now;
        let col = if lit { ink.hue } else { ink.dim };
        if lit {
            b.rrect(*r, 3.0 * k, Fill::Solid(with_alpha(ink.hue, 0.12)));
        }
        b.rrect_stroke(
            *r,
            3.0 * k,
            0.7 * k,
            if lit { ink.hue } else { ink.grid_strong },
        );
        mic_sign(
            b,
            Mic::from_index(i),
            pos2(r.left() + 11.0 * k, r.center().y),
            col,
            k,
        );
        canvas::put(
            b,
            pos2(r.left() + 21.0 * k, r.center().y),
            Align2::LEFT_CENTER,
            &Text::key(words::MICS[i], Role::Control, col),
            k,
            None,
        );
    }
}

/// A microphone's outline, small: a dynamic's round head on a stem, a ribbon's long box, a
/// condenser's capsule in its ring.
fn mic_sign(b: &mut Brush, m: Mic, c: Pos2, col: Color32, k: f32) {
    let w = 0.8 * k;
    match m {
        Mic::Dynamic => {
            b.circle_stroke(c - vec2(0.0, 2.0 * k), 3.6 * k, w, col);
            b.segment(
                c + vec2(0.0, 1.6 * k),
                c + vec2(0.0, 6.0 * k),
                w,
                col,
                false,
            );
        }
        Mic::Ribbon => {
            let r = Rect::from_center_size(c, vec2(6.0 * k, 11.0 * k));
            b.rrect_stroke(r, 2.0 * k, w, col);
            for j in 0..3 {
                let y = r.top() + (2.5 + 3.0 * j as f32) * k;
                b.hline(r.left() + 1.5 * k, r.right() - 1.5 * k, y, 0.6 * k, col);
            }
        }
        Mic::Condenser => {
            b.circle_stroke(c, 5.0 * k, w, col);
            b.circle(c, 2.4 * k, Fill::Solid(with_alpha(col, 0.6)));
        }
    }
}

/// The cabinet's front in small: its covering, the cloth, the speakers seen through it,
/// the one the microphone faces ringed in the display's colour.
fn paint_front(b: &mut Brush, ink: &Ink, r: Rect, c: usize, k: f32) {
    let (cols, rows, wide) = match c {
        1 => (1, 1, 1.0),
        2 => (2, 1, 1.55),
        3..=5 => (2, 2, 1.0),
        _ => (1, 1, 1.0),
    };
    let h = r.height() / if wide > 1.0 { 1.45 } else { 1.0 };
    let box_ = Rect::from_center_size(r.center(), vec2(h * wide, h).min(r.size() * vec2(1.6, 1.0)));
    let dim = !(1..=5).contains(&c);
    let a = if dim { 0.4 } else { 1.0 };
    b.rrect(
        box_,
        4.0 * k,
        Fill::V(
            with_alpha(pal::COVERING_TOP.rgb(), a),
            with_alpha(pal::COVERING.rgb(), a),
        ),
    );
    let cloth = box_.shrink(6.0 * k);
    b.rrect(cloth, 2.0 * k, Fill::Solid(with_alpha(pal::CLOTH.rgb(), a)));
    canvas::hatch(
        b,
        cloth.shrink(1.0 * k),
        3.0 * k,
        0.5 * k,
        with_alpha(pal::CLOTH_THREAD.rgb(), 0.22 * a),
    );
    b.rrect_stroke(
        cloth,
        2.0 * k,
        0.8 * k,
        with_alpha(pal::PIPING.rgb(), 0.8 * a),
    );
    let cw = cloth.width() / cols as f32;
    let ch = cloth.height() / rows as f32;
    let sr = cw.min(ch) * if c == 5 { 0.36 } else { 0.42 };
    for j in 0..rows {
        for i in 0..cols {
            let sc = pos2(
                cloth.left() + cw * (i as f32 + 0.5),
                cloth.top() + ch * (j as f32 + 0.5),
            );
            b.circle(
                sc,
                sr,
                Fill::Solid(with_alpha(pal::CONE_SHADE.rgb(), 0.55 * a)),
            );
            let faced = i == 0 && j == 0 && !dim;
            b.circle_stroke(
                sc,
                sr,
                if faced { 1.2 } else { 0.7 } * k,
                if faced {
                    ink.hue
                } else {
                    with_alpha(pal::CONE_SHADE.rgb(), 0.9 * a)
                },
            );
        }
    }
    canvas::put(
        b,
        pos2(r.center().x, r.bottom() + 14.0 * k),
        Align2::CENTER_CENTER,
        &Text::key("arca.cab.front", Role::Micro, ink.dim),
        k,
        None,
    );
}

/// A speaker's cone from the front: the basket's gasket and bolts, the surround, the paper
/// cone with its ribs lit from above, the dust cap.
fn paint_cone(b: &mut Brush, c: Pos2, rad: f32, k: f32) {
    let clip = Rect::from_center_size(c, vec2(rad * 2.2, rad * 2.2));
    b.circle(c, rad, Fill::V(pal::BASKET.rgb(), pal::GASKET.rgb()));
    b.circle(c, rad * 0.95, Fill::Solid(pal::GASKET.rgb()));
    for i in 0..4 {
        let a = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
        let bp = c + vec2(a.cos(), a.sin()) * rad * 0.975;
        b.circle(bp, 2.0 * k, Fill::Solid(pal::BASKET.rgb()));
    }
    // The surround: a rolled ring, light on its upper side.
    b.radial(
        clip,
        c,
        vec2(rad * 0.92, rad * 0.92),
        &[
            (0.0, pal::SURROUND.rgb()),
            (0.86, pal::SURROUND.rgb()),
            (0.93, mix(pal::SURROUND.rgb(), pal::CONE_LIGHT.rgb(), 0.6)),
            (1.0, pal::SURROUND.rgb()),
        ],
        24,
    );
    // The cone: lighter toward the cap, a sheen from the top left.
    b.radial(
        clip,
        c,
        vec2(cone_r(rad), cone_r(rad)),
        &[
            (0.0, pal::CONE_LIGHT.rgb()),
            (0.45, pal::CONE.rgb()),
            (1.0, pal::CONE_SHADE.rgb()),
        ],
        24,
    );
    for i in 1..7 {
        let rr = cone_r(rad) * (0.30 + 0.115 * i as f32);
        b.circle_stroke(c, rr, 0.5 * k, with_alpha(pal::CONE_SHADE.rgb(), 0.55));
    }
    b.arc(
        c,
        cone_r(rad) * 0.7,
        std::f32::consts::PI * 1.08,
        std::f32::consts::PI * 1.42,
        cone_r(rad) * 0.28,
        with_alpha(pal::CONE_LIGHT.rgb(), 0.10),
        false,
    );
    let cap = rad * 0.24;
    b.radial(
        clip,
        c,
        vec2(cap, cap),
        &[(0.0, pal::DUST_CAP_LIGHT.rgb()), (1.0, pal::DUST_CAP.rgb())],
        16,
    );
    b.circle_stroke(c, cap, 0.7 * k, with_alpha(pal::CONE_SHADE.rgb(), 0.9));
}

/// The microphone on the cone, seen from the front at its position (to the right of the
/// cap, along the cone's radius): its shadow, its head as its kind has it, a fine ring where
/// it points and the cap-to-edge scale beside it.
fn paint_mic_front(b: &mut Brush, ink: &Ink, cone: (Pos2, f32), set: &CabSettings, k: f32) {
    let (c, rad) = cone;
    let reach = cone_r(rad);
    let at = c + vec2(set.position.clamp(0.0, 1.0) * reach, 0.0);
    // The scale: the cap, the middle and the edge, marked along the radius below.
    let y = c.y + rad + 10.0 * k;
    b.hline(c.x, c.x + reach, y, 0.7 * k, ink.grid_strong);
    for (t, key) in [(0.0, "arca.cab.cap"), (0.5, ""), (1.0, "arca.cab.edge")] {
        let x = c.x + t * reach;
        b.vline(x, y - 3.0 * k, y + 3.0 * k, 0.7 * k, ink.grid_strong);
        if !key.is_empty() {
            canvas::put(
                b,
                pos2(x, y + 11.0 * k),
                Align2::CENTER_CENTER,
                &Text::key(key, Role::Micro, ink.dim),
                k,
                None,
            );
        }
    }
    b.circle(pos2(at.x, y), 2.4 * k, Fill::Solid(ink.hue));
    // Where it points: a dashed ring at its radius from the cap.
    let d = (at - c).length();
    if d > 4.0 * k {
        let n = 72;
        for i in 0..n {
            if i % 2 == 0 {
                let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
                let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
                b.arc(c, d, a0, a1, 0.6 * k, with_alpha(ink.hue, 0.55), false);
            }
        }
    }
    // Its shadow on the cone, then its head.
    b.circle(
        at + vec2(5.0 * k, 7.0 * k),
        17.0 * k,
        Fill::Solid(with_alpha(pal::CONE_SHADE.rgb(), 0.55)),
    );
    let body = pal::MIC_BODY.rgb();
    let light = pal::MIC_BODY_LIGHT.rgb();
    let mesh = pal::MIC_MESH.rgb();
    match set.mic {
        Mic::Ribbon => {
            let head = Rect::from_center_size(at, vec2(22.0 * k, 36.0 * k));
            b.rrect(head, 6.0 * k, Fill::V(light, body));
            let inner = head.shrink(3.0 * k);
            b.rrect(inner, 4.0 * k, Fill::Solid(mesh));
            let mut yy = inner.top() + 2.5 * k;
            while yy < inner.bottom() - 1.5 * k {
                b.hline(
                    inner.left() + 1.5 * k,
                    inner.right() - 1.5 * k,
                    yy,
                    0.6 * k,
                    with_alpha(light, 0.45),
                );
                yy += 2.6 * k;
            }
        }
        m => {
            let rr = if m == Mic::Condenser { 17.0 } else { 14.0 } * k;
            b.circle(at, rr, Fill::V(light, body));
            b.circle(at, rr - 2.6 * k, Fill::Solid(mesh));
            // The grille's mesh: a fine cross-hatch inside the ring.
            let step = 2.4 * k;
            let inner = rr - 3.2 * k;
            let mut o = -inner;
            while o <= inner {
                let h = (inner * inner - o * o).max(0.0).sqrt();
                b.segment(
                    at + vec2(o, -h),
                    at + vec2(o, h),
                    0.45 * k,
                    with_alpha(light, 0.35),
                    false,
                );
                b.segment(
                    at + vec2(-h, o),
                    at + vec2(h, o),
                    0.45 * k,
                    with_alpha(light, 0.35),
                    false,
                );
                o += step;
            }
            if m == Mic::Condenser {
                b.circle(at, 3.0 * k, Fill::Solid(with_alpha(light, 0.5)));
            }
        }
    }
    b.circle_stroke(at, 21.0 * k, 0.8 * k, ink.hue);
    canvas::tag(
        b,
        pos2(c.x - 10.0 * k, y),
        Align2::RIGHT_CENTER,
        frame::tr("arca.cab.position"),
        ink.text,
        k,
        None,
        format_args!("{:.0} %", set.position * 100.0),
    );
}

/// The side view: the speaker in section behind the grille cloth, the microphone in profile
/// at its distance (a log scale, 1 to 30 cm) and height (its position on the cone).
fn paint_side(b: &mut Brush, ink: &Ink, r: Rect, set: &CabSettings, on: bool, k: f32) {
    let cy = r.center().y + 4.0 * k;
    let (gx, zx) = side_span(r, k);
    let half = r.height() * 0.36;
    canvas::put(
        b,
        pos2(r.left() + 12.0 * k, r.top() + 11.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("arca.cab.side", Role::Micro, ink.dim),
        k,
        None,
    );
    // The speaker in section: the cone's two walls, the cap, the magnet behind.
    let back = gx - 34.0 * k;
    let col = if on {
        mix(pal::CONE_LIGHT.rgb(), ink.text, 0.2)
    } else {
        ink.dim
    };
    b.segment(
        pos2(gx - 4.0 * k, cy - half),
        pos2(back, cy - 6.0 * k),
        0.9 * k,
        col,
        false,
    );
    b.segment(
        pos2(gx - 4.0 * k, cy + half),
        pos2(back, cy + 6.0 * k),
        0.9 * k,
        col,
        false,
    );
    b.arc(
        pos2(back + 4.0 * k, cy),
        6.0 * k,
        -std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_2,
        0.9 * k,
        col,
        false,
    );
    b.rrect(
        Rect::from_center_size(pos2(back - 8.0 * k, cy), vec2(12.0 * k, 22.0 * k)),
        2.0 * k,
        Fill::Solid(with_alpha(pal::BASKET.rgb(), if on { 0.9 } else { 0.4 })),
    );
    // The grille cloth: a fine dashed line.
    canvas::dashed(
        b,
        pos2(gx, r.top() + 18.0 * k),
        pos2(gx, r.bottom() - 6.0 * k),
        0.8 * k,
        with_alpha(pal::CLOTH_THREAD.rgb(), 0.8),
        2.0 * k,
        2.0 * k,
    );
    // The distance scale: 1, 3, 10 and 30 cm.
    let ys = r.bottom() - 8.0 * k;
    b.hline(gx, zx, ys, 0.6 * k, ink.grid);
    for cm in [1.0f32, 3.0, 10.0, 30.0] {
        let x = side_x(r, k, cm);
        b.vline(x, ys - 3.0 * k, ys, 0.6 * k, ink.grid_strong);
        canvas::putf(
            b,
            pos2(x + 3.0 * k, ys - 6.0 * k),
            Align2::LEFT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{cm:.0}"),
        );
    }
    if !on {
        return;
    }
    // The microphone in profile, its head toward the cone, at its height.
    let mx = side_x(r, k, set.distance_cm);
    let my = cy - set.position.clamp(0.0, 1.0) * half;
    canvas::dashed(
        b,
        pos2(gx, my),
        pos2(mx, my),
        0.6 * k,
        with_alpha(ink.hue, 0.6),
        3.0 * k,
        2.0 * k,
    );
    let head = if set.mic == Mic::Ribbon { 9.0 } else { 7.0 } * k;
    let body = Rect::from_min_max(
        pos2(mx + head, my - 3.5 * k),
        pos2(mx + head + 46.0 * k, my + 3.5 * k),
    );
    b.rrect(
        body,
        2.0 * k,
        Fill::V(pal::MIC_BODY_LIGHT.rgb(), pal::MIC_BODY.rgb()),
    );
    b.circle(
        pos2(mx + head * 0.5, my),
        head * 0.62,
        Fill::V(pal::MIC_BODY_LIGHT.rgb(), pal::MIC_MESH.rgb()),
    );
    b.circle_stroke(
        pos2(mx + head * 0.5, my),
        head * 0.62 + 2.0 * k,
        0.8 * k,
        ink.hue,
    );
    canvas::tag(
        b,
        pos2(body.right() + 6.0 * k, my),
        Align2::LEFT_CENTER,
        frame::tr("arca.cab.distance"),
        ink.text,
        k,
        None,
        format_args!("{:.1} cm", set.distance_cm),
    );
}

/// The response the cabinet and the microphone make, as designed (0 dB: its mean between
/// 100 Hz and 5 kHz), 60 Hz to 12 kHz.
fn paint_response(b: &mut Brush, ink: &Ink, r: Rect, set: &CabSettings, on: bool, k: f32) {
    let plot = Rect::from_min_max(
        pos2(r.left() + 34.0 * k, r.top() + 22.0 * k),
        pos2(r.right() - 12.0 * k, r.bottom() - 18.0 * k),
    );
    canvas::put(
        b,
        pos2(r.left() + 12.0 * k, r.top() + 11.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("arca.cab.response", Role::Micro, ink.dim),
        k,
        None,
    );
    let (f0, f1) = (60f32, 12_000f32);
    let fx = |f: f32| plot.left() + plot.width() * ((f / f0).ln() / (f1 / f0).ln());
    let (top, bot) = (12.0f32, -30.0f32);
    let fy = |db: f32| plot.top() + plot.height() * ((top - db.clamp(bot, top)) / (top - bot));
    for (db, label) in [(0.0f32, "0"), (-12.0, "-12"), (-24.0, "-24")] {
        let y = fy(db);
        b.hline(plot.left(), plot.right(), y, 0.6 * k, ink.grid);
        canvas::put(
            b,
            pos2(plot.left() - 5.0 * k, y),
            Align2::RIGHT_CENTER,
            &Text::new(label, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    for (f, label) in [(100.0f32, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        let x = fx(f);
        b.vline(x, plot.top(), plot.bottom(), 0.6 * k, ink.grid);
        canvas::put(
            b,
            pos2(x, plot.bottom() + 9.0 * k),
            Align2::CENTER_CENTER,
            &Text::new(label, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    if !on {
        return;
    }
    const N: usize = 96;
    let mut db = [0.0f32; N];
    let mut mean = (0.0f32, 0usize);
    for (i, d) in db.iter_mut().enumerate() {
        let f = f0 * (f1 / f0).powf(i as f32 / (N - 1) as f32);
        *d = design_db(set, f);
        if (100.0..=5_000.0).contains(&f) {
            mean.0 += *d;
            mean.1 += 1;
        }
    }
    let m = mean.0 / mean.1.max(1) as f32;
    let mut pts = [Pos2::ZERO; N];
    for (i, p) in pts.iter_mut().enumerate() {
        let f = f0 * (f1 / f0).powf(i as f32 / (N - 1) as f32);
        *p = pos2(fx(f), fy(db[i] - m));
    }
    b.area(&pts, plot.bottom(), ink.fill(0.6));
    canvas::trace(b, &pts, 1.3 * k, ink.voice(ink.hue, true), ink);
}
