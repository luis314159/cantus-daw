//! Arca's control plate (ADR 0137, provisional): the amp's front, in the face's hammered
//! enamel — at its left the nameplate with the pilot light (lit with an amp chosen,
//! breathing with how hard it works) and the input jack; then the knobs as an amp has them,
//! grouped by the circuit they turn: the preamp's gain, the tone stack, the power amp
//! (presence — the chime amp's cut, turned the other way — master and sag) and the output.
//! Without an amp the amp's knobs stay usable, dimmed, so it is set before it is chosen.

use super::super::controls::{self, Size};
use super::super::host::FxHost;
use super::super::valve_draw as valve;
use super::display::View;
use super::words;
use crate::kit::color::with_alpha;
use crate::kit::text::{self as ktext, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use egui::{pos2, vec2, Align2, Rect};

/// The plate's height at scale `k`: one row of knobs in their cards.
pub fn height(k: f32) -> f32 {
    controls::card_height(Size::M) * k
}

/// The nameplate's width, at 100 %.
const BADGE_W: f32 = 104.0;

/// A cell on `key` with a label of its own (the card says whose it is).
fn labelled(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, key: &'static str, label: &str) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let out = controls::Cell::param(info, spec.get(key))
        .size(Size::M)
        .label(Src::Key(label))
        .width(words::cell_width(ui, label))
        .show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// The plate in `r`.
pub fn show(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, r: Rect, v: &View) {
    let k = s.k;
    let gap = s.d(4.0);
    let model = words::model(host.spec());
    let badge = Rect::from_min_size(r.min, vec2(BADGE_W * k, r.height()));
    nameplate(ui, s, badge, model.is_some(), v.effort());
    let rest = Rect::from_min_max(pos2(badge.right() + gap, r.top()), r.max);
    let w = [
        controls::cells_width(&[Size::M]) + 16.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
        controls::cells_width(&[Size::M; 2]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let free = rest.width() - 3.0 * gap;
    let mut x = rest.left();
    let rects: Vec<Rect> = w
        .iter()
        .map(|wi| {
            let rr = Rect::from_min_size(pos2(x, rest.top()), vec2(free * wi / sum, r.height()));
            x = rr.right() + gap;
            rr
        })
        .collect();
    let amp = model.is_some();
    let dim = |ui: &mut egui::Ui| {
        if !amp {
            ui.multiply_opacity(0.55);
        }
    };
    controls::card(ui, s, rects[0], Src::Key("arca.card.preamp"), None, |ui, _| {
        dim(ui);
        labelled(host, ui, s, "gain", "arca.l.gain");
    });
    controls::card(ui, s, rects[1], Src::Key("arca.card.tone"), None, |ui, _| {
        dim(ui);
        labelled(host, ui, s, "bass", "arca.l.bass");
        labelled(host, ui, s, "mid", "arca.l.middle");
        labelled(host, ui, s, "treble", "arca.l.treble");
    });
    let cut = model.is_some_and(|m| words::character(m).cut);
    controls::card(ui, s, rects[2], Src::Key("arca.card.power"), None, |ui, _| {
        dim(ui);
        let presence = if cut { "arca.l.cut" } else { "arca.l.presence" };
        labelled(host, ui, s, "presence", presence);
        labelled(host, ui, s, "master", "arca.l.master");
        labelled(host, ui, s, "sag", "arca.l.sag");
    });
    controls::card(ui, s, rects[3], Src::Key("arca.card.output"), None, |ui, _| {
        labelled(host, ui, s, "mix", "arca.l.mix");
        labelled(host, ui, s, "out", "arca.l.output");
    });
}

/// The nameplate: the pilot light, the name engraved, the input jack.
fn nameplate(ui: &mut egui::Ui, s: &Style, r: Rect, on: bool, effort: f32) {
    let k = s.k;
    let t = &s.t;
    with_brush(&ui.painter_at(r.expand(2.0 * k)), |b| {
        // A plate screwed to the enamel: a darker inset with a lit lip.
        let plate = r.shrink(1.0 * k);
        b.rrect(plate, 4.0 * k, Fill::Solid(with_alpha(t.dark_shade, 0.22)));
        b.rrect_stroke(plate, 4.0 * k, 0.7 * k, with_alpha(t.light, 0.18));
        for (x, y) in [
            (plate.left() + 7.0 * k, plate.top() + 7.0 * k),
            (plate.right() - 7.0 * k, plate.top() + 7.0 * k),
            (plate.left() + 7.0 * k, plate.bottom() - 7.0 * k),
            (plate.right() - 7.0 * k, plate.bottom() - 7.0 * k),
        ] {
            b.circle(pos2(x, y), 2.2 * k, Fill::Solid(with_alpha(t.dark_shade, 0.5)));
            b.segment(
                pos2(x - 1.4 * k, y - 1.4 * k),
                pos2(x + 1.4 * k, y + 1.4 * k),
                0.6 * k,
                with_alpha(t.light, 0.4),
                false,
            );
        }
        let cx = plate.center().x;
        let lit = if on { 0.55 + 0.45 * effort } else { 0.0 };
        valve::jewel(b, s, pos2(cx, plate.top() + 30.0 * k), 9.0 * k, lit);
        let name = Rect::from_center_size(
            pos2(cx, plate.top() + 60.0 * k),
            vec2(plate.width() - 12.0 * k, 22.0 * k),
        );
        let _ = ktext::paint(
            b,
            name,
            &Text::key("arca.plate.name", Role::Section, s.hue).align(Align2::CENTER_CENTER),
            k,
        );
        // The input jack: a nut round a dark hole.
        let jc = pos2(cx, plate.bottom() - 30.0 * k);
        b.circle(jc, 8.0 * k, Fill::V(with_alpha(t.light, 0.5), with_alpha(t.dark_shade, 0.6)));
        b.circle(jc, 5.5 * k, Fill::Solid(with_alpha(t.dark_shade, 0.9)));
        b.circle_stroke(jc, 8.0 * k, 0.6 * k, with_alpha(t.light, 0.35));
        let jack = Rect::from_center_size(
            pos2(cx, plate.bottom() - 13.0 * k),
            vec2(plate.width() - 12.0 * k, 12.0 * k),
        );
        let _ = ktext::paint(
            b,
            jack,
            &Text::key("arca.plate.input", Role::Micro, t.text_dim).align(Align2::CENTER_CENTER),
            k,
        );
    });
}
