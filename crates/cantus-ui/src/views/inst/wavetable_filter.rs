//! Unda's filters on the kit: each one's response curve (the transfer function the DSP
//! realises) in its card's display, the modulated cutoff of the newest voice drawn over it
//! as a ghost and the reach of its modulation as a band; drag on the curve for cutoff
//! (across) and resonance (up). The routing between them is a card of its own.

use super::parts::{self, c};
use super::{db_text, names, set_values};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::with_alpha;
use crate::kit::knob::KnobSize;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Style};
use cantus_core::inst::modulation::connections;
use cantus_core::inst::wavetable as model;
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::unda::filter::{magnitude_db, FilterType};
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use std::fmt::Write as _;

const F_LO: f32 = 20.0;
const F_HI: f32 = 20_000.0;
const DB_LO: f32 = -36.0;
const DB_HI: f32 = 24.0;
const N: usize = 97;

fn x_of(rect: Rect, hz: f32) -> f32 {
    rect.left() + rect.width() * (hz / F_LO).ln() / (F_HI / F_LO).ln()
}

fn hz_of(rect: Rect, x: f32) -> f32 {
    F_LO * (F_HI / F_LO).powf(((x - rect.left()) / rect.width()).clamp(0.0, 1.0))
}

fn y_of(rect: Rect, db: f32) -> f32 {
    rect.bottom() - rect.height() * ((db - DB_LO) / (DB_HI - DB_LO)).clamp(0.0, 1.0)
}

/// The curve of `ty` at `cutoff`/`res`, as screen points.
fn curve(rect: Rect, ty: FilterType, cutoff: f32, res: f32) -> [Pos2; N] {
    let mut out = [Pos2::ZERO; N];
    for (i, p) in out.iter_mut().enumerate() {
        let x = rect.left() + rect.width() * i as f32 / (N - 1) as f32;
        let hz = hz_of(rect, x);
        *p = Pos2::new(x, y_of(rect, magnitude_db(ty, cutoff, res, hz, 96_000.0)));
    }
    out
}

/// How far the matrix can move a filter's cutoff, as (down, up) fractions of its range.
fn reach(spec: &NativeSpec, key: &str) -> (f32, f32) {
    connections(spec)
        .filter(|c| c.on && c.dst == key)
        .fold((0.0, 0.0), |(lo, hi), c| {
            let bipolar = cantus_core::inst::modulation::source(c.src).is_some_and(|s| s.bipolar);
            let a = c.amount;
            let (l, h) = if bipolar {
                (-a.abs(), a.abs())
            } else if a < 0.0 {
                (a, 0.0)
            } else {
                (0.0, a)
            };
            (lo + l, hi + h)
        })
}

/// A filter: its type in the title's selector, its curve to drag, and its cells. `big`
/// (Play) uses M cells and leaves out the stereo offset.
#[allow(clippy::too_many_arguments)]
pub fn filter_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    f: usize,
) {
    let k = |field: &str| format!("flt{}.{field}", f + 1);
    let big = parts::row_width(6, KnobSize::S, s) + s.s(80.0) < rect.width() - s.s(16.0)
        && rect.height() > s.s(220.0);
    let size = if big { KnobSize::M } else { KnobSize::S };
    let cells_h = parts::cell_size(size, s).y;
    let disp = (rect.height() - cells_h - s.s(24.0 + 1.0 + 16.0)) / s.k;
    let on = spec.get(&k("on")) >= 0.5;
    let title = tf("unda.filter_n", &[("n", &(f + 1).to_string())]);
    let types = names("unda.flt", model::FILTERS);
    let ti = spec.get(&k("type")).round().max(0.0) as usize;
    let type_name = types.get(ti).cloned().unwrap_or_default();
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Str(&title),
        Some(on),
        Some(&type_name),
        Some(disp.max(40.0)),
    );
    parts::power(app, ui, track, spec, &k("on"), &cr);
    let id = ui.id().with(("unda-flt-type", f));
    let pick = parts::selector(ui, id, &cr, &t("unda.flt_type"));
    if let Some(i) = parts::step(ti, types.len(), pick) {
        app.set_inst_value(track, &k("type"), i as f32, false);
    }
    if pick.open {
        parts::toggle_popup(ui, id.with("menu"));
    }
    if let Some(sr) = cr.selector {
        let items = parts::radio_items(&types, ti);
        if let Some(i) = parts::popup(ui, id.with("menu"), sr, s, &items) {
            app.set_inst_value(track, &k("type"), i as f32, false);
        }
    }
    if let Some(d) = cr.display {
        display(app, ui, track, spec, s, d, f, on);
    }
    let body = parts::below_display(&cr, s);
    let mix = |v: f32, b: &mut String| {
        let _ = write!(b, "{:.0} %", v * 100.0);
    };
    let fields: &[&str] = if big {
        &["cutoff", "res", "drive", "key", "mix"]
    } else {
        &["cutoff", "res", "drive", "key", "stereo", "mix"]
    };
    let keys: Vec<String> = fields.iter().map(|f| k(f)).collect();
    let cells: Vec<parts::C<'_>> = keys
        .iter()
        .zip(fields)
        .map(|(key, f)| match *f {
            "cutoff" if !big => c(key).fmt(&super::hz_s),
            "drive" if !big => c(key).fmt(&super::db_s),
            "drive" => c(key).fmt(&db_text),
            "mix" => c(key).fmt(&mix),
            _ => c(key),
        })
        .collect();
    parts::cells_spread(app, ui, track, spec, s, size, body, &cells);
    if !on {
        with_brush(ui.painter(), |b| {
            if let Some(d) = cr.display {
                parts::chip(
                    b,
                    Rect::from_min_size(
                        Pos2::new(d.left() + s.s(6.0), d.top() + s.s(5.0)),
                        Vec2::new(d.width() * 0.5, s.s(16.0)),
                    ),
                    s,
                    &t("unda.filter_wire"),
                    s.t.text_dim,
                    Align2::LEFT_CENTER,
                );
            }
        });
    }
}

/// The curve, its modulation, and the drag.
#[allow(clippy::too_many_arguments)]
fn display(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    d: Rect,
    f: usize,
    on: bool,
) {
    let k = |field: &str| format!("flt{}.{field}", f + 1);
    let plot = d.shrink2(Vec2::new(s.s(4.0), s.s(4.0)));
    let ty = FilterType::from_index(spec.get(&k("type")));
    let (cut, res) = (spec.get(&k("cutoff")), spec.get(&k("res")));
    let line_c = if on { s.hue } else { s.t.text_dim };
    // The reach of its modulation: a band between the extreme curves.
    let (lo, hi) = reach(spec, &k("cutoff"));
    if lo != 0.0 || hi != 0.0 {
        let span = (20_000.0f32 / 16.0).ln();
        let a = curve(
            plot,
            ty,
            (cut * (lo * span).exp()).clamp(16.0, 20_000.0),
            res,
        );
        let b = curve(
            plot,
            ty,
            (cut * (hi * span).exp()).clamp(16.0, 20_000.0),
            res,
        );
        parts::band(&ui.painter_at(d), &a, &b, with_alpha(line_c, 0.22));
    }
    let tel = app.inst_telemetry();
    let live = super::live(app);
    with_brush(&ui.painter_at(d), |b| {
        for hz in [100.0, 1_000.0, 10_000.0] {
            let x = b.snap_mid(x_of(plot, hz));
            b.vline(x, d.top(), d.bottom(), b.px(), s.t.grid);
            let label = if hz >= 1_000.0 {
                format!("{}k", hz / 1_000.0)
            } else {
                format!("{hz}")
            };
            parts::say(
                b,
                Rect::from_min_size(
                    Pos2::new(x + s.s(3.0), d.bottom() - s.s(14.0)),
                    Vec2::new(s.s(28.0), s.s(12.0)),
                ),
                s,
                &label,
                Role::Micro,
                s.t.text_dim,
                Align2::LEFT_CENTER,
            );
        }
        b.hline(
            d.left(),
            d.right(),
            b.snap_mid(y_of(plot, 0.0)),
            b.px(),
            s.t.grid_strong,
        );
        let base = curve(plot, ty, cut, res);
        b.area(
            &base,
            d.bottom(),
            with_alpha(line_c, if s.t.dark { 0.14 } else { 0.10 }),
        );
        b.polyline(&base, s.s(1.8), line_c);
        // The newest voice's modulated cutoff: a ghost in the "now" colour.
        if live && on {
            let mc = tel[cantus_engine::inst::wavetable::tel::CUTOFF + f];
            let mr = tel[cantus_engine::inst::wavetable::tel::RES + f];
            if mc > 0.0 && ((mc / cut) - 1.0).abs() > 0.01 {
                b.polyline(
                    &curve(plot, ty, mc, mr),
                    s.s(1.2),
                    with_alpha(s.t.highlight, 0.85),
                );
            }
        }
        let handle = Pos2::new(
            x_of(plot, cut),
            y_of(
                plot,
                magnitude_db(ty, cut, res, cut, 96_000.0).min(DB_HI - 2.0),
            ),
        );
        b.circle(handle, s.s(4.5), Fill::Solid(line_c));
        let mut txt = String::new();
        crate::views::inst::write_readout(cut, "Hz", &mut txt);
        let _ = write!(txt, " \u{b7} {:.0} %", res * 100.0);
        parts::chip(
            b,
            Rect::from_min_size(
                Pos2::new(d.right() - s.s(126.0), d.top() + s.s(5.0)),
                Vec2::new(s.s(120.0), s.s(16.0)),
            ),
            s,
            &txt,
            s.t.text,
            Align2::RIGHT_CENTER,
        );
    });
    let r = ui.interact(
        d,
        ui.id().with(("unda-flt-drag", f)),
        Sense::click_and_drag(),
    );
    if r.dragged() {
        if let Some(pos) = r.interact_pointer_pos() {
            let new_cut = hz_of(plot, pos.x).clamp(16.0, 20_000.0);
            let new_res = (res - r.drag_delta().y / plot.height()).clamp(0.0, 1.0);
            set_values(
                app,
                track,
                &k("drag"),
                &[(&k("cutoff"), new_cut), (&k("res"), new_res)],
            );
        }
    }
    if r.drag_stopped() {
        app.end_gesture();
    }
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    parts::display_hint(ui, &r, "unda.filter_tip", "");
}

/// Serial ↔ parallel between the two filters: a small picture of the paths and its cell.
pub fn routing_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cell_h = parts::cell_size(KnobSize::M, s).y;
    let disp = (rect.height() - cell_h - s.s(24.0 + 1.0 + 16.0)) / s.k;
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Key("unda.routing_title"),
        None,
        None,
        Some(disp.max(30.0)),
    );
    let v = spec.get("flt.routing");
    if let Some(d) = cr.display {
        with_brush(&ui.painter_at(d), |b| {
            let r = d.shrink2(Vec2::new(s.s(10.0), s.s(8.0)));
            let bw = (r.width() * 0.32).min(s.s(26.0));
            let bh = s.s(14.0);
            let cy = r.center().y;
            let (top, bot) = (cy - r.height() * 0.25, cy + r.height() * 0.25);
            let x1 = r.left() + r.width() * 0.3;
            let x2 = r.left() + r.width() * 0.7;
            let ser = with_alpha(s.hue, 0.25 + 0.75 * (1.0 - v));
            let par = with_alpha(s.hue, 0.25 + 0.75 * v);
            // Serial: in → 1 → 2 → out, on the middle line.
            b.segment(
                Pos2::new(r.left(), cy),
                Pos2::new(r.right(), cy),
                s.s(1.5),
                ser,
                false,
            );
            // Parallel: in splits to 1 (up) and 2 (down), both out.
            for y in [top, bot] {
                b.segment(
                    Pos2::new(r.left() + s.s(4.0), cy),
                    Pos2::new(r.left() + s.s(10.0), y),
                    s.s(1.2),
                    par,
                    false,
                );
                b.segment(
                    Pos2::new(r.left() + s.s(10.0), y),
                    Pos2::new(r.right() - s.s(10.0), y),
                    s.s(1.2),
                    par,
                    false,
                );
                b.segment(
                    Pos2::new(r.right() - s.s(10.0), y),
                    Pos2::new(r.right() - s.s(4.0), cy),
                    s.s(1.2),
                    par,
                    false,
                );
            }
            for (x, y, n) in [(x1, cy, "1"), (x2, cy, "2")] {
                let yy = if v > 0.5 {
                    if n == "1" {
                        top
                    } else {
                        bot
                    }
                } else {
                    y
                };
                let xx = if v > 0.5 { r.center().x } else { x };
                let bx = Rect::from_center_size(Pos2::new(xx, yy), Vec2::new(bw, bh));
                b.rrect(bx, s.r(3.0), Fill::Solid(s.t.deck));
                b.rrect_stroke(bx, s.r(3.0), s.s(1.0), s.hue);
                parts::say(b, bx, s, n, Role::Micro, s.t.text, Align2::CENTER_CENTER);
            }
        });
    }
    let fmt = |v: f32, b: &mut String| {
        let _ = if v <= 0.005 {
            write!(b, "{}", t("unda.serial"))
        } else if v >= 0.995 {
            write!(b, "{}", t("unda.parallel"))
        } else {
            write!(b, "{:.0} %", v * 100.0)
        };
    };
    let body = parts::below_display(&cr, s);
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::M,
        body,
        &[c("flt.routing").fmt(&fmt)],
    );
}
