//! Saltus's whammy pedal (ADR 0135, provisional), drawn in side view beside the keyboard: a
//! treadle with its grip rocking on a wedge base from the heel to the toe, a scale at the
//! toe from the heel's interval to the toe's with the voice's interval now on it, the
//! interval large above, and a MIDI lamp when the notes play it. Dragged up and down.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::FxHost;
use super::words;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill};
use cantus_dsp::saltus::SaltusSettings;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

/// The treadle's tilt at the heel and at the toe, radians (screen y down: the toe end up
/// at the heel, down at the toe).
const HEEL_TILT: f32 = -0.30;
const TOE_TILT: f32 = 0.20;

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
    // The base: a housing with chamfered shoulders, a groove along it, rubber feet.
    let len = body.width() * 0.72;
    let left = body.left() + body.width() * 0.05;
    let pivot = pos2(left + len * 0.45, body.center().y);
    let floor = (pivot.y + 66.0 * k).min(body.bottom() - 34.0 * k);
    let shade = |t: f32| mix(ink.ground, ink.text, t);
    let base = [
        pos2(left, floor),
        pos2(left + len, floor),
        pos2(left + len, floor - 20.0 * k),
        pos2(left + len - 14.0 * k, floor - 30.0 * k),
        pos2(pivot.x + 26.0 * k, pivot.y + 14.0 * k),
        pos2(pivot.x - 26.0 * k, pivot.y + 14.0 * k),
        pos2(left + 12.0 * k, floor - 36.0 * k),
        pos2(left, floor - 28.0 * k),
    ];
    b.polygon(&base, shade(0.14));
    let mut ring = base.to_vec();
    ring.push(base[0]);
    b.polyline(&ring, 0.8 * k, shade(0.42));
    b.hline(left + 6.0 * k, left + len - 6.0 * k, floor - 9.0 * k, 0.7 * k, ink.grid);
    for x in [left + 14.0 * k, left + len - 14.0 * k] {
        b.rrect(
            Rect::from_center_size(pos2(x, floor + 2.5 * k), vec2(22.0 * k, 5.0 * k)),
            2.0 * k,
            Fill::Solid(shade(0.30)),
        );
    }
    // The treadle, tilted by the pedal: a thick plate, its underside darker, a rubber pad
    // with its ridges, the toe's lip, the voice's light along its edge.
    let tilt = HEEL_TILT + (TOE_TILT - HEEL_TILT) * pedal.clamp(0.0, 1.0);
    let (tl, tr) = (left - 10.0 * k, left + len + 12.0 * k);
    let plate = |x: f32, y: f32| rot(pos2(x, pivot.y + y), pivot, tilt);
    let deck = [
        plate(tl + 6.0 * k, -28.0 * k),
        plate(tr - 8.0 * k, -28.0 * k),
        plate(tr, -21.0 * k),
        plate(tr, -3.0 * k),
        plate(tl, -3.0 * k),
        plate(tl, -21.0 * k),
    ];
    b.polygon(&deck, shade(0.24));
    let under = [
        plate(tl, -10.0 * k),
        plate(tr, -10.0 * k),
        plate(tr, -3.0 * k),
        plate(tl, -3.0 * k),
    ];
    b.polygon(&under, shade(0.12));
    let mut outline = deck.to_vec();
    outline.push(deck[0]);
    b.polyline(&outline, 0.9 * k, shade(0.62));
    let (pl, pr) = (tl + 16.0 * k, tr - 22.0 * k);
    let pad = [
        plate(pl, -26.0 * k),
        plate(pr, -26.0 * k),
        plate(pr, -14.0 * k),
        plate(pl, -14.0 * k),
    ];
    b.polygon(&pad, shade(0.08));
    let ridges = ((pr - pl) / (5.0 * k)) as usize;
    for i in 1..ridges {
        let x = pl + (pr - pl) * i as f32 / ridges as f32;
        b.segment(
            plate(x, -25.0 * k),
            plate(x, -15.0 * k),
            0.7 * k,
            shade(0.30),
            false,
        );
    }
    canvas::trace(
        b,
        &[plate(tl + 8.0 * k, -28.6 * k), plate(tr - 10.0 * k, -28.6 * k)],
        1.4 * k,
        with_alpha(ink.hue, 0.45 + 0.55 * pedal),
        ink,
    );
    // The hinge: a ring on its bracket.
    b.polygon(
        &[
            pos2(pivot.x - 14.0 * k, pivot.y + 14.0 * k),
            pos2(pivot.x + 14.0 * k, pivot.y + 14.0 * k),
            pos2(pivot.x, pivot.y),
        ],
        shade(0.2),
    );
    b.circle(pivot, 8.0 * k, Fill::Solid(shade(0.35)));
    b.circle_stroke(pivot, 8.0 * k, 0.9 * k, shade(0.7));
    b.circle(pivot, 2.5 * k, Fill::Solid(ink.text));
    // The scale at the toe: the heel's interval at the top of the travel, the toe's below.
    let toe = |g: f32| {
        let a = HEEL_TILT + (TOE_TILT - HEEL_TILT) * g;
        rot(pos2(tr + 12.0 * k, pivot.y - 15.0 * k), pivot, a)
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
    for (g, text, al) in [(0.0, &hw, Align2::LEFT_BOTTOM), (1.0, &tw, Align2::LEFT_TOP)] {
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
        &Text::key("saltus.d.midi", Role::Micro, if by_notes { ink.text } else { ink.dim }),
        k,
        None,
    );
}
