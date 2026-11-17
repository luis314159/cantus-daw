//! Vetus's six modules (ADR 0130), in the signal's order: each a card whose
//! lamp is its switch (the stepped parameter's control: its menu, automate, learn), its
//! knobs and choices inside — and the inspector's picture.

use super::super::canvas::{self, Ink};
use super::super::controls::{self, Size};
use super::super::frame;
use super::super::host::FxHost;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Style};
use cantus_core::EffectSpec;
use cantus_dsp::vetus::VetusSettings;
use egui::{pos2, vec2, Align2, Rect, Sense};

/// A module's card: `title` and its switch `on_key` in the lamp, `body` inside.
fn module(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    title: &'static str,
    on_key: &'static str,
    body: impl FnOnce(&mut dyn FxHost, &mut egui::Ui),
) {
    let on = host.spec().get(on_key) >= 0.5;
    let (_, clicked, lamp) =
        controls::card_lamp_live(ui, s, rect, Src::Key(title), on, |ui, _| body(host, ui));
    // The lamp is the switch's control every frame (its menu, learn), not only when clicked.
    let learnt = controls::card_lamp_offer(ui, s, host, on_key, lamp.as_ref());
    if clicked && !learnt {
        host.set(on_key, if on { 0.0 } else { 1.0 }, None);
    }
}

/// A grid of keys for a stepped parameter, `cols` across, each its value: the lit one the
/// value now. Every key is the parameter's control (its menu, learn).
#[allow(clippy::too_many_arguments)]
fn choice_keys(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    labels: &[&'static str],
    cols: usize,
    width: f32,
    apply: bool,
) -> Option<usize> {
    let k = s.k;
    let now = host.spec().get(key).round().max(0.0) as usize;
    let rows = labels.len().div_ceil(cols);
    let gap = s.d(3.0);
    let h = s.d(controls::ROW);
    let (rect, _) = ui.allocate_exact_size(
        vec2(width * k, rows as f32 * h + (rows - 1) as f32 * gap),
        Sense::hover(),
    );
    let w = (rect.width() - gap * (cols - 1) as f32) / cols as f32;
    let name = host
        .spec()
        .kind
        .param(key)
        .map_or("", |i| frame::tr(i.label_key));
    let mut chosen = None;
    for (i, label) in labels.iter().enumerate() {
        let (row, col) = (i / cols, i % cols);
        let r = Rect::from_min_size(
            pos2(
                rect.left() + col as f32 * (w + gap),
                rect.top() + row as f32 * (h + gap),
            ),
            vec2(w, h),
        );
        let resp = ui.interact(r, ui.id().with(("vetus-choice", key, i)), Sense::click());
        let st = crate::kit::State::of(&resp, i == now);
        if ui.is_rect_visible(r) {
            with_brush(ui.painter(), |b| {
                controls::paint_key(b, r, s, st, Src::Key(label));
            });
        }
        let learnt = controls::stepped_offer(ui, s, host, key, name, &resp);
        if resp.clicked() && !learnt && i != now {
            if apply {
                host.set(key, i as f32, None);
            }
            chosen = Some(i);
        }
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                frame::tr(label),
                frame::tr(tip_of(label)),
                crate::kit::hint::Caps::NONE,
            );
        }
    }
    chosen
}

/// A choice's help line: its key's `.tip`.
fn tip_of(label: &'static str) -> &'static str {
    match label {
        "vetus.bits_mode.0" => "vetus.bits_mode.0.tip",
        "vetus.bits_mode.1" => "vetus.bits_mode.1.tip",
        "vetus.bits_mode.2" => "vetus.bits_mode.2.tip",
        "vetus.rate_filter.0" => "vetus.rate_filter.0.tip",
        "vetus.rate_filter.1" => "vetus.rate_filter.1.tip",
        "vetus.rate_filter.2" => "vetus.rate_filter.2.tip",
        "vetus.medium.0" => "vetus.medium.0.tip",
        "vetus.medium.1" => "vetus.medium.1.tip",
        "vetus.medium.2" => "vetus.medium.2.tip",
        "vetus.medium.3" => "vetus.medium.3.tip",
        _ => "vetus.choice.tip",
    }
}

/// A cell on `key` labelled `label` (its own words: the groups say what they belong to).
fn cell(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    label: &'static str,
    size: Size,
    format: Option<controls::Pill<'_>>,
) {
    cell_w(host, ui, s, key, label, size, format, None);
}

/// [`cell`] wider than its knob, for a label that needs the room.
#[allow(clippy::too_many_arguments)]
fn cell_w(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    label: &'static str,
    size: Size,
    format: Option<controls::Pill<'_>>,
    width: Option<f32>,
) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let mut c = controls::Cell::param(info, spec.get(key))
        .size(size)
        .label(Src::Key(label));
    if let Some(f) = format {
        c = c.format(f);
    }
    if let Some(w) = width {
        c = c.width(w);
    }
    let out = c.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

pub fn bits(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, _cs: &VetusSettings) {
    module(
        host,
        ui,
        s,
        rect,
        "vetus.card.bits",
        "bits.on",
        |host, ui| {
            cell(
                host,
                ui,
                s,
                "bits.depth",
                "vetus.l.bits",
                Size::M,
                Some(&super::bits_pill),
            );
            let w = 164.0;
            controls::column(ui, s, w, |ui| {
                controls::caption(ui, s, Src::Key("fx.vetus.bits.mode"), w);
                choice_keys(
                    host,
                    ui,
                    s,
                    "bits.mode",
                    &[
                        "vetus.bits_mode.0",
                        "vetus.bits_mode.1",
                        "vetus.bits_mode.2",
                    ],
                    2,
                    w,
                    true,
                );
            });
            let w = 72.0;
            controls::column(ui, s, w, |ui| {
                controls::led_param(
                    ui,
                    s,
                    host,
                    "bits.dither",
                    Src::Key("fx.vetus.bits.dither"),
                    w,
                );
            });
        },
    );
}

pub fn rate(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, _cs: &VetusSettings) {
    module(
        host,
        ui,
        s,
        rect,
        "vetus.card.rate",
        "rate.on",
        |host, ui| {
            cell(host, ui, s, "rate.hz", "vetus.l.rate", Size::M, None);
            let w = 184.0;
            controls::column(ui, s, w, |ui| {
                controls::caption(ui, s, Src::Key("fx.vetus.rate.filter"), w);
                choice_keys(
                    host,
                    ui,
                    s,
                    "rate.filter",
                    &[
                        "vetus.rate_filter.0",
                        "vetus.rate_filter.1",
                        "vetus.rate_filter.2",
                    ],
                    2,
                    w,
                    true,
                );
            });
        },
    );
}

/// The width at 100 % of a cell whose label is two words.
const WIDE: f32 = 96.0;

/// A synced wow's length as its name.
fn div_pill(v: f32, out: &mut String) {
    const NAMES: [&str; 17] = [
        "8/1",
        "4/1",
        "2/1",
        "1/1",
        "1/2\u{b7}",
        "1/2",
        "1/4\u{b7}",
        "1/2T",
        "1/4",
        "1/8\u{b7}",
        "1/4T",
        "1/8",
        "1/16\u{b7}",
        "1/8T",
        "1/16",
        "1/16T",
        "1/32",
    ];
    out.push_str(NAMES[(v.round().max(0.0) as usize).min(16)]);
}

pub fn wobble(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, cs: &VetusSettings) {
    let synced = matches!(cs.wow_rate, cantus_dsp::modfx_lfo::Rate::Beats(_));
    module(
        host,
        ui,
        s,
        rect,
        "vetus.card.wobble",
        "wob.on",
        |host, ui| {
            cell(
                host,
                ui,
                s,
                "wow",
                "vetus.l.wow",
                Size::M,
                Some(&depth_pill),
            );
            if synced {
                cell_w(
                    host,
                    ui,
                    s,
                    "wow.div",
                    "vetus.l.wow_rate",
                    Size::M,
                    Some(&div_pill),
                    Some(WIDE),
                );
            } else {
                cell_w(
                    host,
                    ui,
                    s,
                    "wow.rate",
                    "vetus.l.wow_rate",
                    Size::M,
                    None,
                    Some(WIDE),
                );
            }
            cell(host, ui, s, "wow.drift", "vetus.l.drift", Size::M, None);
            cell(
                host,
                ui,
                s,
                "flut",
                "vetus.l.flutter",
                Size::M,
                Some(&depth_pill),
            );
            cell_w(
                host,
                ui,
                s,
                "flut.rate",
                "vetus.l.flutter_rate",
                Size::M,
                None,
                Some(WIDE),
            );
            let w = 66.0;
            controls::column(ui, s, w, |ui| {
                controls::led_param(ui, s, host, "wow.sync", Src::Key("vetus.l.sync"), w);
            });
        },
    );
}

/// A depth in cents, without the sign a pitch would carry: "12 ct".
fn depth_pill(v: f32, out: &mut String) {
    let _ = std::fmt::Write::write_fmt(out, format_args!("{v:.0} ct"));
}

pub fn age(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, _cs: &VetusSettings) {
    module(host, ui, s, rect, "vetus.card.age", "age.on", |host, ui| {
        cell(host, ui, s, "age.drop", "vetus.l.dropouts", Size::M, None);
        cell(host, ui, s, "age.wear", "vetus.l.wear", Size::M, None);
    });
}

pub fn noise(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, _cs: &VetusSettings) {
    module(
        host,
        ui,
        s,
        rect,
        "vetus.card.noise",
        "noise.on",
        |host, ui| {
            cell(host, ui, s, "crackle", "vetus.l.crackle", Size::M, None);
            cell(host, ui, s, "hiss", "vetus.l.hiss", Size::M, None);
            cell(host, ui, s, "hum", "vetus.l.hum", Size::M, None);
            cell(host, ui, s, "noise.follow", "vetus.l.follow", Size::M, None);
            let w = 98.0;
            controls::column(ui, s, w, |ui| {
                controls::caption(ui, s, Src::Key("vetus.l.hiss_kind"), w);
                let kinds = [Src::Key("vetus.hiss_kind.0"), Src::Key("vetus.hiss_kind.1")];
                controls::segmented_param(ui, s, host, "hiss.kind", &kinds, w);
                controls::caption(ui, s, Src::Key("vetus.l.mains"), w);
                let hz = [Src::Key("vetus.hum_hz.0"), Src::Key("vetus.hum_hz.1")];
                controls::segmented_param(ui, s, host, "hum.hz", &hz, w);
            });
        },
    );
}

pub fn tone(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, _cs: &VetusSettings) {
    module(
        host,
        ui,
        s,
        rect,
        "vetus.card.tone",
        "tone.on",
        |host, ui| {
            let w = 160.0;
            controls::column(ui, s, w, |ui| {
                controls::caption(ui, s, Src::Key("fx.vetus.tone.medium"), w);
                if let Some(i) =
                    choice_keys(host, ui, s, "tone.medium", &super::MEDIUM_KEYS, 2, w, false)
                {
                    // A medium brings its band: one step of undo with the choice.
                    let (lo, hi) = cantus_core::fx::vetus::MEDIUM_BANDS[i];
                    host.set_many(
                        &[("tone.medium", i as f32), ("tone.lo", lo), ("tone.hi", hi)],
                        "vetus.cmd.medium",
                    );
                }
            });
            cell_w(
                host,
                ui,
                s,
                "tone.lo",
                "vetus.l.low",
                Size::M,
                None,
                Some(WIDE - 6.0),
            );
            cell_w(
                host,
                ui,
                s,
                "tone.hi",
                "vetus.l.high",
                Size::M,
                None,
                Some(WIDE - 6.0),
            );
            cell_w(
                host,
                ui,
                s,
                "tone.res",
                "vetus.l.res",
                Size::M,
                None,
                Some(WIDE - 6.0),
            );
        },
    );
}

/// The inspector's picture: a small record, and the modules that are on in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(super::KIND));
    let ink = Ink::of(&st);
    let k = st.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0 * k), Sense::click());
    let v = super::heard(spec);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        let c = pos2(rect.left() + 28.0 * k, rect.center().y);
        let rr = 22.0 * k;
        let vinyl = crate::kit::skin::records::VINYL.rgb();
        b.circle(c, rr, crate::kit::Fill::Solid(vinyl));
        let mut r = rr * 0.45;
        while r < rr * 0.97 {
            b.circle_stroke(
                c,
                r,
                0.5 * k,
                crate::kit::color::with_alpha(crate::kit::skin::records::GROOVE.rgb(), 0.12),
            );
            r += 2.0 * k;
        }
        b.circle(
            c,
            rr * 0.36,
            crate::kit::Fill::Solid(crate::kit::skin::records::LABEL_NEW.rgb()),
        );
        b.dot(c, 1.4 * k, vinyl);
        let mut w = String::new();
        let names = [
            "vetus.card.bits",
            "vetus.card.rate",
            "vetus.card.wobble",
            "vetus.card.age",
            "vetus.card.noise",
            "vetus.card.tone",
        ];
        for (on, name) in v.switches().into_iter().zip(names) {
            if on {
                if !w.is_empty() {
                    w.push_str(" \u{b7} ");
                }
                w.push_str(frame::tr(name));
            }
        }
        if w.is_empty() {
            w.push_str(frame::tr("vetus.preview.none"));
        }
        canvas::put(
            b,
            pos2(rect.left() + 58.0 * k, rect.center().y),
            Align2::LEFT_CENTER,
            &Text::new(&w, Role::Micro, ink.text).caps(false),
            k,
            None,
        );
        canvas::edge_b(b, rect, &ink);
    });
    resp
}
