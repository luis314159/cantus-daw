//! Saltus's whammy pedal (ADR 0132), drawn in side view beside the keyboard: a
//! treadle with its grip rocking on a wedge base from the heel to the toe, a scale at the
//! toe from the heel's interval to the toe's with the voice's interval now on it, the
//! interval large above, and a MIDI lamp when the notes play it. Dragged up and down.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::FxHost;
use super::words;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill};
use cantus_dsp::saltus::SaltusSettings;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// The treadle's tilt at the heel and at the toe, radians (screen y down: the toe end up
/// at the heel, down at the toe).
const HEEL_TILT: f32 = -0.30;
const TOE_TILT: f32 = 0.20;

/// The pedal's metal: Saltus's graphite (its face's anodised body), a hair bluer; the
/// drawing sets its lightness face by face.
const METAL: crate::kit::color::Hsl = crate::kit::color::Hsl {
    h: 224.0,
    s: 0.07,
    l: 0.3,
};

/// Where the drawing goes in the panel `r`: the readout's band, the pedal's.
fn layout(r: Rect, k: f32) -> (Rect, Rect) {
    let top = Rect::from_min_size(r.min, vec2(r.width(), 74.0 * k));
    let body = Rect::from_min_max(
        pos2(r.left() + 14.0 * k, top.bottom()),
        pos2(r.right() - 14.0 * k, r.bottom() - 16.0 * k),
    );
    (top, body)
}

/// Rock the pedal: dragging down presses the toe.
pub fn edit(host: &mut dyn FxHost, ui: &mut egui::Ui, r: Rect, k: f32) {
    let (_, body) = layout(r, k);
    let g = ui.interact(r, ui.id().with("saltus-pedal"), Sense::click_and_drag());
    if g.dragged() {
        let now = host.spec().get("pedal");
        let v = (now + g.drag_delta().y / (body.height() * 0.8).max(1.0)).clamp(0.0, 1.0);
        if v != now {
            host.set("pedal", v, Some("pedal"));
        }
    }
    if g.drag_stopped() {
        host.end_gesture();
    }
    if g.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("saltus.d.pedal_tip"),
            frame::tr("saltus.d.pedal_how"),
            Caps::NONE,
        );
    }
}

fn rot(p: Pos2, c: Pos2, a: f32) -> Pos2 {
    let (s, co) = a.sin_cos();
    let d = p - c;
    pos2(c.x + d.x * co - d.y * s, c.y + d.x * s + d.y * co)
}

/// The pedal at `pedal` (0 heel … 1 toe; `target` where it is going), in the panel `r`.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    b: &mut Brush,
    ink: &Ink,
    r: Rect,
    pedal: f32,
    target: f32,
    ss: &SaltusSettings,
    by_notes: bool,
    held: bool,
    k: f32,
) {
    let (top, body) = layout(r, k);
    let interval = cantus_dsp::saltus::whammy_interval(ss, pedal);
    // The interval now, large, and its name.
    let mut w = String::new();
    words::fine_st(interval, &mut w);
    canvas::put(
        b,
        pos2(top.center().x, top.top() + 12.0 * k),
        Align2::CENTER_TOP,
        &Text::new(&w, Role::Display, ink.hue).caps(false),
        k,
        None,
    );
    let mut n = String::new();
    words::interval(interval.round() as i32, &mut n);
    canvas::put(
        b,
        pos2(top.center().x, top.top() + 56.0 * k),
        Align2::CENTER_TOP,
        &Text::new(&n, Role::Label, ink.text).caps(false),
        k,
        None,
    );
    // Drawn in an oblique view: every face's depth runs back along `depth`; the pedal
    // grows with its panel (`g`, the drawing's own scale).
    let g = k * (body.width() / (250.0 * k)).clamp(1.0, 1.45);
    let depth = vec2(18.0 * g, -12.0 * g);
    let len = body.width() * 0.74;
    let left = body.left() + body.width() * 0.04;
    let pivot = pos2(left + len * 0.42, body.center().y - 6.0 * g);
    let floor = (pivot.y + 70.0 * g).min(body.bottom() - 34.0 * g);
    // The pedal's own metal (graphite, the face's), not the screen's light: lit by `t`.
    let shade = |t: f32| {
        crate::kit::color::Hsl {
            l: 0.05 + 0.62 * t,
            ..METAL
        }
        .rgb()
    };
    let quad = |b: &mut Brush, a: Pos2, c: Pos2, fill: Color32| {
        b.polygon(&[a, c, c + depth, a + depth], fill);
    };
    // The housing: its top and right side behind, its brushed front, the hinge block.
    let h = 24.0 * g;
    let right = left + len;
    b.hline(
        left - 6.0 * g,
        right + 26.0 * g,
        floor + 5.0 * g,
        0.7 * g,
        ink.grid,
    );
    quad(
        b,
        pos2(left, floor - h),
        pos2(right, floor - h),
        shade(0.30),
    );
    b.polygon(
        &[
            pos2(right, floor),
            pos2(right, floor - h),
            pos2(right, floor - h) + depth,
            pos2(right, floor) + depth,
        ],
        shade(0.10),
    );
    let front = Rect::from_min_max(pos2(left, floor - h), pos2(right, floor));
    b.rect(front, Fill::V(shade(0.24), shade(0.15)));
    for i in 1..8 {
        let y = front.top() + front.height() * i as f32 / 8.0;
        b.hline(
            left + 2.0 * g,
            right - 2.0 * g,
            y,
            0.5 * g,
            with_alpha(ink.text, 0.07),
        );
    }
    b.rrect_stroke(front, 1.0 * g, 0.8 * g, shade(0.5));
    b.hline(left, right, front.top(), 0.8 * g, shade(0.65));
    canvas::put(
        b,
        pos2(left + 10.0 * g, front.center().y),
        Align2::LEFT_CENTER,
        &Text::key("picker.dev.saltus.name", Role::Micro, shade(0.55)).caps(true),
        k,
        None,
    );
    // The pilot light: the voice's colour, brighter as the toe goes down.
    let led = pos2(right - 14.0 * g, front.center().y);
    b.circle(
        led,
        5.0 * g,
        Fill::Solid(with_alpha(ink.hue, 0.18 + 0.3 * pedal)),
    );
    b.circle(
        led,
        2.6 * g,
        Fill::Solid(with_alpha(ink.hue, 0.55 + 0.45 * pedal)),
    );
    b.circle_stroke(led, 3.4 * g, 0.6 * g, shade(0.6));
    // The toe switch under the toe, and the spring under the heel.
    let dome = pos2(right - 26.0 * g, floor - h) + depth * 0.5;
    b.circle(dome, 6.0 * g, Fill::Solid(shade(0.40)));
    b.circle_stroke(dome, 6.0 * g, 0.7 * g, shade(0.7));
    let block = [
        pos2(pivot.x - 30.0 * g, floor - h),
        pos2(pivot.x + 30.0 * g, floor - h),
        pos2(pivot.x + 14.0 * g, pivot.y + 8.0 * g),
        pos2(pivot.x - 14.0 * g, pivot.y + 8.0 * g),
    ];
    quad(b, block[3], block[2], shade(0.32));
    b.polygon(&block, shade(0.22));
    let mut edge = block.to_vec();
    edge.push(block[0]);
    b.polyline(&edge, 0.8 * g, shade(0.55));
    // The treadle, tilted by the pedal: its underside and toe end, its brushed side, the
    // rubber tread on its top with ridges across it, the toe's lip, the light on its edge.
    let tilt = HEEL_TILT + (TOE_TILT - HEEL_TILT) * pedal.clamp(0.0, 1.0);
    let (tl, tr) = (left - 12.0 * g, right + 12.0 * g);
    let plate = |x: f32, y: f32| rot(pos2(x, pivot.y + y), pivot, tilt);
    let heel_x = left + len * 0.1;
    let spring_top = plate(heel_x, -2.0 * g) + depth * 0.5;
    let spring_bot = pos2(heel_x, floor - h) + depth * 0.5;
    let coils = 7;
    let pts: Vec<Pos2> = (0..=coils * 2)
        .map(|i| {
            let t = i as f32 / (coils * 2) as f32;
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            spring_bot + (spring_top - spring_bot) * t + vec2(side * 5.0 * g, 0.0)
        })
        .collect();
    b.polyline(&pts, 1.1 * g, shade(0.55));
    let (a, c) = (
        plate(tl + 8.0 * g, -26.0 * g),
        plate(tr - 4.0 * g, -26.0 * g),
    );
    quad(b, a, c, shade(0.34));
    // The tread: a rubber pad inset on the top, ridges across it.
    let (pa, pc) = (
        plate(tl + 22.0 * g, -26.0 * g),
        plate(tr - 26.0 * g, -26.0 * g),
    );
    let inset = |p: Pos2, f: f32| p + depth * f;
    b.polygon(
        &[
            inset(pa, 0.15),
            inset(pc, 0.15),
            inset(pc, 0.85),
            inset(pa, 0.85),
        ],
        shade(0.07),
    );
    let ridges = ((pc - pa).length() / (4.5 * g)) as usize;
    for i in 1..ridges {
        let p = pa + (pc - pa) * (i as f32 / ridges as f32);
        b.segment(inset(p, 0.2), inset(p, 0.8), 0.9 * g, shade(0.22), false);
        b.segment(
            inset(p, 0.2) + vec2(0.9 * g, 0.0),
            inset(p, 0.8) + vec2(0.9 * g, 0.0),
            0.5 * g,
            shade(0.03),
            false,
        );
    }
    // The toe's lip and end face.
    let lip_a = plate(tr - 14.0 * g, -26.0 * g);
    quad(b, lip_a, c, shade(0.46));
    b.polygon(
        &[
            c,
            plate(tr, -19.0 * g),
            plate(tr, -19.0 * g) + depth,
            c + depth,
        ],
        shade(0.16),
    );
    let side = [
        a,
        c,
        plate(tr, -19.0 * g),
        plate(tr, -3.0 * g),
        plate(tl, -3.0 * g),
        plate(tl, -19.0 * g),
    ];
    b.polygon(&side, shade(0.22));
    b.polygon(
        &[
            plate(tl, -9.0 * g),
            plate(tr, -9.0 * g),
            plate(tr, -3.0 * g),
            plate(tl, -3.0 * g),
        ],
        shade(0.13),
    );
    for i in 1..4 {
        let y = -19.0 * g + 3.0 * g * i as f32;
        b.segment(
            plate(tl + 4.0 * g, y),
            plate(tr - 4.0 * g, y),
            0.5 * g,
            with_alpha(ink.text, 0.08),
            false,
        );
    }
    let mut outline = side.to_vec();
    outline.push(side[0]);
    b.polyline(&outline, 0.9 * g, shade(0.62));
    canvas::trace(
        b,
        &[a, c],
        1.5 * g,
        with_alpha(ink.hue, 0.45 + 0.55 * pedal),
        ink,
    );
    // The axle: a hex nut on the hinge.
    let hex: [Pos2; 6] = std::array::from_fn(|i| {
        let ang = std::f32::consts::FRAC_PI_3 * i as f32 + tilt;
        pivot + vec2(ang.cos(), ang.sin()) * 7.5 * g
    });
    b.polygon(&hex, shade(0.42));
    let mut hx = hex.to_vec();
    hx.push(hex[0]);
    b.polyline(&hx, 0.8 * g, shade(0.75));
    b.circle(pivot, 3.0 * g, Fill::Solid(shade(0.18)));
    b.circle_stroke(pivot, 3.0 * g, 0.6 * g, shade(0.8));
    // The scale at the toe: the heel's interval at the top of the travel, the toe's below.
    let toe = |g: f32| {
        let a = HEEL_TILT + (TOE_TILT - HEEL_TILT) * g;
        rot(pos2(tr + 16.0 * k, pivot.y - 15.0 * k), pivot, a)
    };
    let steps = 12;
    for i in 0..=steps {
        let g = i as f32 / steps as f32;
        let p = toe(g);
        let long = i == 0 || i == steps || i * 2 == steps;
        let d = vec2(if long { 9.0 } else { 5.0 } * k, 0.0);
        let c = if long { ink.grid_strong } else { ink.grid };
        b.segment(p, p + d, 0.8 * k, c, false);
    }
    let mut hw = String::new();
    words::semitone_pill(ss.heel, &mut hw);
    let mut tw = String::new();
    words::semitone_pill(ss.toe, &mut tw);
    for (g, text, al) in [
        (0.0, &hw, Align2::LEFT_BOTTOM),
        (1.0, &tw, Align2::LEFT_TOP),
    ] {
        canvas::put(
            b,
            toe(g) + vec2(12.0 * k, 0.0),
            al,
            &Text::new(text, Role::Label, ink.dim).caps(false),
            k,
            None,
        );
    }
    let now = toe(pedal.clamp(0.0, 1.0));
    canvas::trace(b, &[now, now + vec2(11.0 * k, 0.0)], 2.2 * k, ink.hue, ink);
    if (target - pedal).abs() > 0.01 {
        let t = toe(target.clamp(0.0, 1.0));
        b.circle_stroke(t + vec2(5.0 * k, 0.0), 2.6 * k, 0.8 * k, ink.hue);
    }
    // The MIDI lamp: lit when the notes play the pedal, bright while one is held.
    let lamp = Rect::from_min_size(
        pos2(body.left() + 2.0 * k, floor + 16.0 * k),
        vec2(42.0 * k, 16.0 * k),
    );
    let lc = if by_notes {
        ink.look.trace2
    } else {
        ink.grid_strong
    };
    if by_notes {
        let a = if held { 0.45 } else { 0.18 };
        b.rrect(lamp, 3.0 * k, Fill::Solid(with_alpha(lc, a)));
    }
    b.rrect_stroke(lamp, 3.0 * k, 0.8 * k, lc);
    canvas::put(
        b,
        lamp.center(),
        Align2::CENTER_CENTER,
        &Text::key(
            "saltus.d.midi",
            Role::Micro,
            if by_notes { ink.text } else { ink.dim },
        ),
        k,
        None,
    );
}
