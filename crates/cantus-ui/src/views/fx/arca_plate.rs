//! Arca's control plate (ADR 0134; v2, the owner's "a real faceplate"): the
//! amp's front in the face's hammered enamel, one plate from end to end under a fine chrome
//! trim — at its left the jewel pilot lamp (lit with an amp chosen, breathing with how hard it
//! works) and the two input jacks; then the knobs as an amp has them, each in a printed ring
//! of fine ticks, grouped by the circuit they turn under engraved legends and parted by
//! engraved rules: the preamp's gain, the tone stack, the power amp (presence — the chime
//! amp's cut, turned the other way — master and sag) and the output. Without an amp the
//! amp's knobs stay usable, dimmed, so it is set before it is chosen.

use super::super::controls::{self, Size};
use super::super::host::FxHost;
use super::super::valve_draw as valve;
use super::display::View;
use super::words;
use crate::kit::color::{ensure_contrast_all, mix, over, with_alpha};
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Rect};

/// The plate's height at scale `k`: an engraved legend over one row of knobs.
pub fn height(k: f32) -> f32 {
    (controls::card_height(Size::M) + 6.0) * k
}

/// The lamp and jacks' end of the plate, at 100 %.
const BADGE_W: f32 = 112.0;

/// The knobs' labels' colour, held to the kit's 4.5:1 on the enamel as it is painted: the
/// ground, the plate's darker inset over it, and the hammered finish's lighter dimples. The
/// face's label tone is measured on its cards; on the light themes' mid-green enamel it fell
/// to about 3.4:1 (the site's pictures, 2026-11-19).
pub fn label_on_enamel(t: &crate::kit::Tokens) -> egui::Color32 {
    let inset = over(with_alpha(t.dark_shade, 0.16), t.device_ground);
    let dimple = mix(t.device_ground, t.light, 0.2);
    ensure_contrast_all(t.label, &[inset, dimple, t.device_ground], 4.5)
}

/// A cell on `key` with a label of its own, as wide as its word; its knob's ring of ticks
/// printed on the plate around it.
fn labelled(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, key: &'static str, label: &str) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let value = spec.get(key);
    let norm = if info.max > info.min {
        ((value - info.min) / (info.max - info.min)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let out = controls::Cell::param(info, value)
        .size(Size::M)
        .label(Src::Key(label))
        .width(words::cell_width(ui, label))
        .show_in(ui, s, host, key);
    ticks(ui, s, out.resp.rect, norm);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// The ring of eleven fine ticks round a medium knob in `cell`, the ones up to `norm` in
/// the face's accent: printed on the plate, as an amp's numbers are.
fn ticks(ui: &egui::Ui, s: &Style, cell: Rect, norm: f32) {
    let k = s.k;
    let d = KnobSize::M.diameter();
    let h = crate::kit::cell::size(KnobSize::M).y;
    let c = pos2(cell.center().x, cell.top() + (h - 24.0 - d * 0.5) * k);
    let (r0, r1) = (d * 0.5 * k + 3.0 * k, d * 0.5 * k + 6.5 * k);
    with_brush(&ui.painter_at(cell.expand(4.0 * k)), |b| {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let a = std::f32::consts::PI * (0.75 + 1.5 * t);
            let dir = vec2(a.cos(), a.sin());
            let lit = t <= norm + 1e-3;
            let col = if lit {
                with_alpha(s.hue, 0.9)
            } else {
                with_alpha(s.t.text_dim, 0.55)
            };
            let long = i % 5 == 0;
            b.segment(
                c + dir * r0,
                c + dir * if long { r1 + 1.5 * k } else { r1 },
                if long { 0.8 } else { 0.6 } * k,
                col,
                false,
            );
        }
    });
}

/// An engraved word: dark into the enamel, its lower lip catching the light.
fn engraved(b: &mut Brush, s: &Style, r: Rect, key: &str, role: Role, align: Align2, k: f32) {
    let t = Text::key(key, role, with_alpha(s.t.light, 0.35)).align(align);
    let _ = ktext::paint(b, r.translate(vec2(0.0, 0.8 * k)), &t, k);
    let t = Text::key(key, role, s.t.text).align(align);
    let _ = ktext::paint(b, r, &t, k);
}

/// The plate in `r`.
pub fn show(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, r: Rect, v: &View) {
    let k = s.k;
    let model = words::model(host.spec());
    let amp = model.is_some();
    // The plate: a darker inset of the enamel under a chrome trim, screws at its ends.
    with_brush(&ui.painter_at(r.expand(2.0 * k)), |b| {
        let t = &s.t;
        b.rrect(r, 4.0 * k, Fill::Solid(with_alpha(t.dark_shade, 0.16)));
        for (y, a) in [(r.top() + 0.5 * k, 0.55), (r.bottom() - 0.5 * k, 0.30)] {
            b.hline(
                r.left() + 4.0 * k,
                r.right() - 4.0 * k,
                y,
                0.8 * k,
                with_alpha(t.light, a),
            );
        }
        b.hline(
            r.left() + 4.0 * k,
            r.right() - 4.0 * k,
            r.top() + 2.0 * k,
            0.6 * k,
            with_alpha(t.dark_shade, 0.4),
        );
        for x in [r.left() + 8.0 * k, r.right() - 8.0 * k] {
            for y in [r.top() + 10.0 * k, r.bottom() - 10.0 * k] {
                let p = pos2(x, y);
                b.circle(
                    p,
                    2.4 * k,
                    Fill::V(with_alpha(t.light, 0.6), with_alpha(t.dark_shade, 0.6)),
                );
                b.segment(
                    p - vec2(1.5, 1.5) * k,
                    p + vec2(1.5, 1.5) * k,
                    0.5 * k,
                    with_alpha(t.dark_shade, 0.8),
                    false,
                );
            }
        }
    });
    let badge = Rect::from_min_size(
        pos2(r.left() + 14.0 * k, r.top()),
        vec2(BADGE_W * k, r.height()),
    );
    lamp_and_jacks(ui, s, badge, amp, v.effort());
    let mut ls = *s;
    ls.t.label = label_on_enamel(&s.t);
    // The groups: an engraved legend over its knobs, an engraved rule between groups.
    type Group<'a> = (&'a str, &'a [(&'static str, &'a str)]);
    let groups: [Group<'_>; 4] = [
        ("arca.card.preamp", &[("gain", "arca.l.gain")]),
        (
            "arca.card.tone",
            &[
                ("bass", "arca.l.bass"),
                ("mid", "arca.l.middle"),
                ("treble", "arca.l.treble"),
            ],
        ),
        (
            "arca.card.power",
            &[
                ("presence", "arca.l.presence"),
                ("master", "arca.l.master"),
                ("sag", "arca.l.sag"),
            ],
        ),
        (
            "arca.card.output",
            &[("mix", "arca.l.mix"), ("out", "arca.l.output")],
        ),
    ];
    let cut = model.is_some_and(|m| words::character(m).cut);
    let width_of = |ui: &egui::Ui, keys: &[(&'static str, &str)]| {
        keys.iter()
            .map(|(_, l)| words::cell_width(ui, l) + 4.0)
            .sum::<f32>()
    };
    let widths: Vec<f32> = groups
        .iter()
        .map(|(_, ks)| width_of(ui, ks) + 12.0)
        .collect();
    let rest = Rect::from_min_max(pos2(badge.right() + 6.0 * k, r.top()), r.max);
    let total: f32 = widths.iter().sum();
    let spare = ((rest.width() / k - total) / groups.len() as f32).max(0.0);
    let mut x = rest.left();
    for (gi, ((title, keys), w)) in groups.iter().zip(&widths).enumerate() {
        let gw = (w + spare) * k;
        let g = Rect::from_min_size(pos2(x, r.top()), vec2(gw, r.height()));
        with_brush(&ui.painter_at(g), |b| {
            let legend =
                Rect::from_min_size(pos2(g.left(), g.top() + 6.0 * k), vec2(g.width(), 16.0 * k));
            engraved(b, s, legend, title, Role::Section, Align2::CENTER_CENTER, k);
            if gi > 0 {
                b.vline(
                    g.left(),
                    g.top() + 12.0 * k,
                    g.bottom() - 12.0 * k,
                    0.7 * k,
                    with_alpha(s.t.dark_shade, 0.45),
                );
                b.vline(
                    g.left() + 1.0 * k,
                    g.top() + 12.0 * k,
                    g.bottom() - 12.0 * k,
                    0.6 * k,
                    with_alpha(s.t.light, 0.25),
                );
            }
        });
        let body = Rect::from_min_max(pos2(g.left() + 6.0 * k, g.top() + 24.0 * k), g.max);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let gap = egui::Vec2::new(s.d(4.0), s.d(4.0));
        if child.spacing().item_spacing != gap {
            child.spacing_mut().item_spacing = gap;
        }
        let used = width_of(ui, keys) * k;
        child.add_space(((body.width() - used) * 0.5).max(0.0));
        if gi < 3 && !amp {
            child.multiply_opacity(0.55);
        }
        for (key, label) in keys.iter() {
            let label = if *key == "presence" && cut {
                "arca.l.cut"
            } else {
                label
            };
            labelled(host, &mut child, &ls, key, label);
        }
        x += gw;
    }
}

/// The lamp and the jacks: the jewel pilot lamp over its name, the two inputs below.
fn lamp_and_jacks(ui: &mut egui::Ui, s: &Style, r: Rect, on: bool, effort: f32) {
    let k = s.k;
    let t = &s.t;
    with_brush(&ui.painter_at(r.expand(2.0 * k)), |b| {
        let cx = r.center().x;
        let lit = if on { 0.55 + 0.45 * effort } else { 0.0 };
        valve::jewel(b, s, pos2(cx, r.top() + 30.0 * k), 10.0 * k, lit);
        let name = Rect::from_center_size(pos2(cx, r.top() + 56.0 * k), vec2(r.width(), 18.0 * k));
        engraved(
            b,
            s,
            name,
            "arca.plate.name",
            Role::Section,
            Align2::CENTER_CENTER,
            k,
        );
        // Two inputs, high and low: a hex nut round a dark hole, its word under it.
        for (i, key) in ["arca.plate.input_high", "arca.plate.input_low"]
            .iter()
            .enumerate()
        {
            let jx = cx + (i as f32 - 0.5) * 48.0 * k;
            let jc = pos2(jx, r.bottom() - 36.0 * k);
            for j in 0..6 {
                let a = std::f32::consts::TAU * j as f32 / 6.0;
                let p = jc + vec2(a.cos(), a.sin()) * 9.0 * k;
                let q = jc + vec2((a + 1.047).cos(), (a + 1.047).sin()) * 9.0 * k;
                b.segment(p, q, 0.7 * k, with_alpha(t.light, 0.45), false);
            }
            b.circle(
                jc,
                7.4 * k,
                Fill::V(with_alpha(t.light, 0.55), with_alpha(t.dark_shade, 0.5)),
            );
            b.circle(jc, 4.6 * k, Fill::Solid(with_alpha(t.dark_shade, 0.95)));
            let wr =
                Rect::from_center_size(pos2(jx, r.bottom() - 16.0 * k), vec2(46.0 * k, 12.0 * k));
            engraved(b, s, wr, key, Role::Micro, Align2::CENTER_CENTER, k);
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::kit::color::contrast;
    use crate::kit::color::{mix, over, with_alpha};
    use crate::kit::skin::{colourways, Unit};
    use cantus_core::EffectKind;

    /// Every colourway, in every shipped theme: the plate's labels at 4.5:1 or better on
    /// the enamel's ground, its inset and its dimples.
    #[test]
    fn the_plate_s_labels_read_on_the_enamel_in_every_theme() {
        let mut bad = Vec::new();
        for (key, make) in crate::theme::PRESETS {
            let ways = colourways(Unit::Effect(EffectKind::Amp));
            assert_eq!(ways.len(), 3, "Green enamel, Graphite and Oxblood");
            for cw in ways {
                let t = cw.sheet.tokens(&make().kit_tokens());
                let l = super::label_on_enamel(&t);
                let inset = over(with_alpha(t.dark_shade, 0.16), t.device_ground);
                let dimple = mix(t.device_ground, t.light, 0.2);
                for (what, bg) in [
                    ("ground", t.device_ground),
                    ("inset", inset),
                    ("dimple", dimple),
                ] {
                    let c = contrast(l, bg);
                    if c < 4.5 {
                        bad.push(format!("{key} {}: {what} {c:.2}", cw.id));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
