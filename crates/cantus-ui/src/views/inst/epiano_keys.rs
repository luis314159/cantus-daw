//! Lamina's keyboard-wide and time pictures, drawn with the kit's brush into a card's
//! display:
//!
//! - **Key curves**: hardness, decay and level across the keyboard, eight anchors (C1…C8)
//!   drawn over the keys they sit on, joined as the engine interpolates them. Drag an anchor;
//!   double-click it back to neutral. Every tier of the product has it (Pianoteq keeps its
//!   Note Edit for the dearest edition).
//! - **The pedal**: its travel, the zone where the dampers only half touch shaded from the
//!   actual lift point (drag it), and where the pedal is now.
//! - **The tremolo**: a second of what it does to the level (or to each side, panning), and
//!   where it is now.

use super::parts::label;
use super::set;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{on_fill, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::light;
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::inst::epiano::{CURVE_DECAY, CURVE_HARD, CURVE_LEVEL};
use cantus_core::TrackId;
use cantus_dsp::lamina::geometry::Engine;
use cantus_dsp::lamina::{curve_at, LaminaSettings, Trem, CURVE_KEYS};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// Which curve the strip edits.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    #[default]
    Hardness,
    Decay,
    Level,
}

impl Curve {
    const ALL: [Curve; 3] = [Curve::Hardness, Curve::Decay, Curve::Level];
}

/// The three curves as the switch in the card's title names them.
pub const CURVES: [Src<'static>; 3] = [
    Src::Key("epiano.curve_hard"),
    Src::Key("epiano.curve_decay"),
    Src::Key("epiano.curve_level"),
];

/// A segmented control over editor state (not a parameter): `selected` changes on click.
pub fn choose(
    ui: &mut egui::Ui,
    rect: Rect,
    id: egui::Id,
    selected: &mut usize,
    labels: &[Src<'_>],
    name: &str,
    s: &Style,
) {
    let (_, chosen) = crate::kit::widgets::Segmented::new(*selected, labels)
        .id(id)
        .name(name)
        .style(*s)
        .show_at(ui, rect);
    if let Some(i) = chosen {
        *selected = i;
    }
}

/// The switch of which curve the strip edits, in `rect`.
pub fn curve_switch(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, rect: Rect, s: &Style) {
    let mut i = Curve::ALL
        .iter()
        .position(|c| *c == app.inst.epiano.curve)
        .unwrap_or(0);
    choose(
        ui,
        rect,
        egui::Id::new(("lamina-curve-switch", track.0)),
        &mut i,
        &CURVES,
        &t("epiano.key_curve"),
        s,
    );
    app.inst.epiano.curve = Curve::ALL[i.min(2)];
}

fn is_black(k: u8) -> bool {
    matches!(k % 12, 1 | 3 | 6 | 8 | 10)
}

/// Write a curve's value the way it reads.
fn write_value(which: Curve, v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = match which {
        Curve::Hardness => write!(b, "{:+.0} %", v * 100.0),
        Curve::Decay => write!(b, "\u{d7}{v:.2}"),
        Curve::Level => write!(b, "{v:+.1} dB"),
    };
}

/// The key curve strip over a keyboard, A0…C8, in the display `d`.
pub fn key_curves(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    ls: &LaminaSettings,
    d: Rect,
    s: &Style,
) {
    let which = app.inst.epiano.curve;
    let (keys, anchors, lo, hi): (&[&str; 8], [f32; 8], f32, f32) = match which {
        Curve::Hardness => (&CURVE_HARD, ls.curve_hard, -0.5, 0.5),
        Curve::Decay => (&CURVE_DECAY, ls.curve_decay, 0.25, 4.0),
        Curve::Level => (&CURVE_LEVEL, ls.curve_level, -12.0, 12.0),
    };
    let log = which == Curve::Decay;
    let pos = |v: f32| {
        if log {
            (v / lo).ln() / (hi / lo).ln()
        } else {
            (v - lo) / (hi - lo)
        }
    };
    let val = |p: f32| {
        let p = p.clamp(0.0, 1.0);
        if log {
            lo * (hi / lo).powf(p)
        } else {
            lo + (hi - lo) * p
        }
    };
    let kb_h = s.s(30.0);
    let plot = Rect::from_min_max(
        Pos2::new(d.left() + s.s(10.0), d.top() + s.s(10.0)),
        Pos2::new(d.right() - s.s(10.0), d.bottom() - kb_h - s.s(4.0)),
    );
    let kb = Rect::from_min_max(
        Pos2::new(plot.left(), plot.bottom() + s.s(2.0)),
        Pos2::new(plot.right(), d.bottom() - s.s(4.0)),
    );
    let (k0, k1) = (21u8, 108u8);
    let x_of = |k: f32| plot.left() + (k - k0 as f32 + 0.5) / (k1 - k0 + 1) as f32 * plot.width();
    let pad = s.s(8.0);
    let y_of = |p: f32| plot.bottom() - pad - p * (plot.height() - 2.0 * pad);
    let neutral = if log { 1.0 } else { 0.0 };
    // The anchors first: a drag moves them this frame.
    let mut values = anchors;
    let mut hot = [false; 8];
    for (i, k) in CURVE_KEYS.iter().enumerate() {
        let p = Pos2::new(x_of(*k as f32), y_of(pos(values[i])));
        let resp = ui.interact(
            Rect::from_center_size(p, Vec2::splat(s.s(16.0))),
            egui::Id::new(("lamina-curve", track.0, keys[i])),
            Sense::click_and_drag(),
        );
        if i == 3 {
            // Only where it can be grabbed: the Edit view scrolls.
            app.inst.epiano.anchor = ui.is_rect_visible(resp.rect).then_some(resp.rect);
        }
        if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
            let pv = (plot.bottom() - pad - q.y) / (plot.height() - 2.0 * pad);
            values[i] = val(pv);
            set(app, track, keys[i], values[i], true);
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        if resp.double_clicked() {
            values[i] = neutral;
            set(app, track, keys[i], neutral, false);
        }
        hot[i] = resp.hovered() || resp.dragged();
        if hot[i] {
            let mut v = String::with_capacity(12);
            write_value(which, values[i], &mut v);
            let tip = tf(
                "epiano.anchor_tip",
                &[("key", &cantus_core::pitch_name(*k)), ("v", &v)],
            );
            hint::set_text(ui.ctx(), &tip, "", Caps::PARAM);
        }
    }
    let last = app.inst_telemetry()[cantus_engine::inst::epiano::tel::KEY] as u8;
    with_brush(&ui.painter_at(d), |b| {
        let tk = &s.t;
        // The instrument's range, lit.
        let (r0, r1) = ls.range.keys();
        b.rect(
            Rect::from_min_max(
                Pos2::new(x_of(r0 as f32 - 0.5), plot.top()),
                Pos2::new(x_of(r1 as f32 + 0.5), plot.bottom()),
            ),
            Fill::Solid(with_alpha(s.hue, 0.07)),
        );
        for p in [0.25, 0.5, 0.75] {
            b.hline(plot.left(), plot.right(), y_of(p), b.px(), tk.grid);
        }
        b.hline(
            plot.left(),
            plot.right(),
            y_of(pos(neutral)),
            b.px(),
            tk.grid_strong,
        );
        // Per-key bars, as the engine reads the curve.
        let bar = with_alpha(s.hue, 0.45);
        let w = ((plot.width() / (k1 - k0 + 1) as f32) * 0.6).max(b.px());
        for k in k0..=k1 {
            let v = curve_at(&values, k);
            let x = x_of(k as f32);
            let (y0, y1) = (y_of(pos(neutral)), y_of(pos(v)));
            if (y1 - y0).abs() > 0.5 {
                b.rect(
                    Rect::from_min_max(
                        Pos2::new(x - w * 0.5, y0.min(y1)),
                        Pos2::new(x + w * 0.5, y0.max(y1)),
                    ),
                    Fill::Solid(bar),
                );
            }
        }
        let mut line = [Pos2::ZERO; 8];
        for (i, k) in CURVE_KEYS.iter().enumerate() {
            line[i] = Pos2::new(x_of(*k as f32), y_of(pos(values[i])));
        }
        b.polyline(&line, s.s(2.0), s.hue);
        for (i, p) in line.iter().enumerate() {
            let r = s.s(if hot[i] { 6.5 } else { 5.0 });
            b.circle(*p, r + b.px() * 1.5, Fill::Solid(tk.ink));
            b.circle(*p, r, Fill::Solid(s.hue));
            b.circle(*p, r * 0.4, Fill::Solid(on_fill(s.hue)));
            if hot[i] {
                let mut v = String::with_capacity(12);
                write_value(which, values[i], &mut v);
                let lr = Rect::from_center_size(
                    *p - Vec2::new(0.0, s.s(16.0)),
                    Vec2::new(s.s(64.0), s.s(14.0)),
                );
                let _ = ktext::paint(
                    b,
                    lr,
                    &Text::new(&v, Role::Label, tk.text).caps(false),
                    s.tk(),
                );
            }
        }
        // The keyboard under it, the last key struck lit.
        let kw = kb.width() / (k1 - k0 + 1) as f32;
        for k in k0..=k1 {
            let r = Rect::from_min_size(
                Pos2::new(kb.left() + (k - k0) as f32 * kw, kb.top()),
                Vec2::new(kw, kb.height()),
            );
            let fill = if k == last {
                s.hue
            } else if is_black(k) {
                tk.key_black
            } else {
                tk.key_white
            };
            b.rect(r.shrink(b.px() * 0.5), Fill::V(fill, shade(fill, -0.05)));
        }
        for k in (k0..=k1).filter(|k| k % 12 == 0) {
            let x = kb.left() + (k - k0) as f32 * kw;
            let r = Rect::from_min_max(
                Pos2::new(x, kb.bottom() - s.s(12.0)),
                Pos2::new(x + kw * 6.0, kb.bottom()),
            );
            let name = cantus_core::pitch_name(k);
            let _ = ktext::paint(
                b,
                r,
                &Text::new(&name, Role::Micro, on_fill(tk.key_white))
                    .caps(false)
                    .align(Align2::LEFT_CENTER),
                s.tk(),
            );
        }
    });
}

/// The pedal's travel with the half-pedal zone and the lift point, in the display `d`.
#[allow(clippy::too_many_arguments)]
pub fn pedal(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    ls: &LaminaSettings,
    pedal: f32,
    engage: f32,
    d: Rect,
    s: &Style,
) {
    let head = s.s(20.0);
    let bar = Rect::from_min_max(
        Pos2::new(d.left() + s.s(12.0), d.top() + head + s.s(2.0)),
        Pos2::new(d.right() - s.s(12.0), d.bottom() - s.s(18.0)),
    );
    let x_of = |p: f32| bar.left() + p.clamp(0.0, 1.0) * bar.width();
    let mut lift = ls.pedal_lift;
    let handle = Rect::from_center_size(
        Pos2::new(x_of(lift), bar.center().y),
        Vec2::new(s.s(14.0), bar.height() + s.s(10.0)),
    );
    let resp = ui.interact(
        handle,
        egui::Id::new(("lamina-lift", track.0)),
        Sense::drag(),
    );
    if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
        let p = ((q.x - bar.left()) / bar.width()).clamp(0.1, 0.9);
        lift = (p * 100.0).round() / 100.0;
        set(app, track, "pedal.lift", lift, true);
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    let hot = resp.hovered() || resp.dragged();
    if hot {
        hint::set_text(ui.ctx(), &t("epiano.lift_tip"), "", Caps::PARAM);
    }
    with_brush(&ui.painter_at(d), |b| {
        let tk = &s.t;
        let title = tf(
            "epiano.pedal_title",
            &[
                ("p", &format!("{:.0}", pedal * 100.0)),
                ("on", &format!("{:.0}", engage * 100.0)),
                ("lift", &format!("{:.0}", lift * 100.0)),
            ],
        );
        label(
            b,
            Rect::from_min_size(
                Pos2::new(d.left() + s.s(10.0), d.top() + s.s(3.0)),
                Vec2::new(d.width() - s.s(20.0), s.s(15.0)),
            ),
            &title,
            Role::Label,
            tk.text,
            s,
        );
        light::sunken(b, bar, s.r(3.0), tk.well, 1.0, tk.depth);
        // The half-pedal zone either side of the lift point.
        let zone = Rect::from_min_max(
            Pos2::new(x_of(lift - 0.1), bar.top() + b.px()),
            Pos2::new(x_of(lift + 0.1), bar.bottom() - b.px()),
        );
        b.rect(zone, Fill::Solid(with_alpha(s.hue, 0.28)));
        // Where the pedal is.
        let fill = Rect::from_min_max(
            Pos2::new(bar.left() + s.s(2.0), bar.top() + s.s(4.0)),
            Pos2::new(
                x_of(pedal).max(bar.left() + s.s(2.0)),
                bar.bottom() - s.s(4.0),
            ),
        );
        if pedal > 0.0 {
            b.rrect(
                fill,
                s.r(2.0),
                Fill::V(shade(s.hue, 0.06), shade(s.hue, -0.06)),
            );
        }
        b.segment(
            Pos2::new(x_of(lift), bar.top() - s.s(4.0)),
            Pos2::new(x_of(lift), bar.bottom() + s.s(4.0)),
            s.s(if hot { 3.5 } else { 2.5 }),
            tk.text,
            true,
        );
        if hot {
            b.circle_stroke(
                Pos2::new(x_of(lift), bar.top() - s.s(4.0)),
                s.s(4.5),
                s.s(1.5),
                s.hue,
            );
        }
        // Up and down, at the ends of the travel.
        let ends = Rect::from_min_max(
            Pos2::new(bar.left(), bar.bottom() + s.s(3.0)),
            Pos2::new(bar.right(), d.bottom() - s.s(2.0)),
        );
        for (key, align) in [
            ("epiano.pedal_up", Align2::LEFT_CENTER),
            ("epiano.pedal_down", Align2::RIGHT_CENTER),
        ] {
            let half = if align == Align2::LEFT_CENTER {
                Rect::from_min_max(ends.min, Pos2::new(ends.center().x, ends.bottom()))
            } else {
                Rect::from_min_max(Pos2::new(ends.center().x, ends.top()), ends.max)
            };
            let _ = ktext::paint(
                b,
                half,
                &Text::key(key, Role::Micro, tk.text_dim)
                    .caps(true)
                    .align(align)
                    .policy(Policy::Abbreviate),
                s.tk(),
            );
        }
    });
}

/// A second of the tremolo in the display `d`: the level it leaves (both sides when it
/// pans), and where it is now (`now`, −1…1).
pub fn tremolo(b: &mut Brush, ls: &LaminaSettings, now: f32, d: Rect, s: &Style) {
    let tk = &s.t;
    let pan = match ls.trem {
        Trem::Pan => true,
        Trem::Amplitude => false,
        Trem::Auto => ls.engine == Engine::Tine,
    };
    let plot = Rect::from_min_max(
        Pos2::new(d.left() + s.s(10.0), d.top() + s.s(8.0)),
        Pos2::new(d.right() - s.s(22.0), d.bottom() - s.s(8.0)),
    );
    let depth = ls.trem_depth.clamp(0.0, 1.0);
    let y_of = |g: f32| plot.bottom() - g.clamp(0.0, 1.0) * plot.height();
    b.hline(plot.left(), plot.right(), y_of(1.0), b.px(), tk.grid);
    b.hline(plot.left(), plot.right(), y_of(0.0), b.px(), tk.grid);
    let mut left = [Pos2::ZERO; 97];
    let mut right = [Pos2::ZERO; 97];
    for i in 0..97 {
        let tt = i as f32 / 96.0;
        let lfo = (tt * ls.trem_rate_hz * std::f32::consts::TAU).sin();
        let x = plot.left() + tt * plot.width();
        left[i] = Pos2::new(x, y_of(1.0 - depth * 0.5 * (1.0 + lfo)));
        right[i] = Pos2::new(x, y_of(1.0 - depth * 0.5 * (1.0 - lfo)));
    }
    b.area(&left, plot.bottom(), with_alpha(s.hue, 0.12));
    b.polyline(&left, s.s(2.0), s.hue);
    if pan {
        b.polyline(&right, s.s(1.5), tk.text_dim);
    }
    // Where it is now, at the right edge.
    let x = plot.right() + s.s(11.0);
    b.vline(x, plot.top(), plot.bottom(), b.px(), tk.grid_strong);
    let g = 1.0 - depth * 0.5 * (1.0 + now.clamp(-1.0, 1.0));
    b.circle(Pos2::new(x, y_of(g)), s.s(4.0), Fill::Solid(s.hue));
}
