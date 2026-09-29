//! The drawbars: the organ's registration, drawn as the instrument shows it and played the
//! same way — drag a bar out (down) or in (up), or click a stop; double-click resets it.
//! The footage is printed under each bar, the position (0–8) on its handle, and what a step
//! means in dB in the help line while you point at it. Bars keep the organist's colour code:
//! the sub-harmonics (16′, 5⅓′) brown (the dynamics hue), the octaves white, the mutations
//! (2⅔′, 1⅗′, 1⅓′) black — the keyboard's own two tokens.

use super::set;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{on_fill, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self as ktext, Policy, Role, Text};
use crate::kit::{with_brush, Fill, Function, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::organum::wheels::{drawbar_db, FOOTAGES};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// Which colour a bar wears.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Sub,
    Octave,
    Mutation,
}

/// The upper and lower manuals' bars, 16′ first.
pub const MANUAL_KINDS: [Kind; 9] = [
    Kind::Sub,
    Kind::Sub,
    Kind::Octave,
    Kind::Octave,
    Kind::Mutation,
    Kind::Octave,
    Kind::Mutation,
    Kind::Mutation,
    Kind::Octave,
];

/// The pedals' two bars.
pub const PEDAL_KINDS: [Kind; 2] = [Kind::Sub, Kind::Octave];
/// The pedals' footages.
pub const PEDAL_FOOTAGES: [&str; 2] = ["16\u{2032}", "8\u{2032}"];

/// Harmonic of the 8′ each manual bar sounds (for the help line).
pub const HARMONICS: [&str; 9] = ["½", "1½", "1", "2", "3", "4", "5", "6", "8"];

/// A registration as organists write it: `88 8000 000`.
pub fn registration(values: &[f32]) -> String {
    let mut out = String::with_capacity(12);
    for (i, v) in values.iter().enumerate() {
        let gap = if values.len() == 9 {
            i == 2 || i == 6
        } else {
            i > 0
        };
        if gap {
            out.push(' ');
        }
        let d = v.round().clamp(0.0, 9.0) as u32;
        out.push(char::from_digit(d, 10).unwrap_or('0'));
    }
    out
}

/// The manual footages, as printed.
pub fn manual_footages() -> [&'static str; 9] {
    FOOTAGES
}

/// Where a set of bars sits in a display rect: the slots and the step between stops.
struct Geometry {
    slots: Vec<Rect>,
    /// One bar's column: its footage is printed across it.
    col: f32,
    handle_h: f32,
    step: f32,
    foot: Rect,
}

fn geometry(d: Rect, n: usize, s: &Style) -> Geometry {
    let pad = s.s(10.0);
    let foot_h = s.s(18.0);
    let inner = Rect::from_min_max(
        Pos2::new(d.left() + pad, d.top() + s.s(8.0)),
        Pos2::new(d.right() - pad, d.bottom() - foot_h - s.s(2.0)),
    );
    let col = inner.width() / n.max(1) as f32;
    let bw = (col * 0.66).min(s.s(30.0));
    let handle_h = (inner.height() * 0.1).clamp(s.s(14.0), s.s(20.0));
    let step = (inner.height() - handle_h) / 8.0;
    let slots = (0..n)
        .map(|i| {
            let cx = inner.left() + col * (i as f32 + 0.5);
            Rect::from_min_max(
                Pos2::new(cx - bw * 0.5, inner.top()),
                Pos2::new(cx + bw * 0.5, inner.bottom()),
            )
        })
        .collect();
    let foot = Rect::from_min_max(
        Pos2::new(inner.left(), d.bottom() - foot_h - s.s(2.0)),
        Pos2::new(inner.right(), d.bottom() - s.s(2.0)),
    );
    Geometry {
        slots,
        col,
        handle_h,
        step,
        foot,
    }
}

/// A set of drawbars bound to `keys`, filling the display `d`: interaction first, then the
/// picture, so the bar moves under the pointer in the frame it was dragged.
#[allow(clippy::too_many_arguments)]
pub fn drawbars(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
    keys: &[&'static str],
    footages: &[&str],
    kinds: &[Kind],
) {
    let g = geometry(d, keys.len(), s);
    let mut values = [0.0f32; 9];
    let mut hot = [false; 9];
    for (i, key) in keys.iter().enumerate().take(9) {
        let slot = g.slots[i];
        let id = egui::Id::new(("organ-drawbar", track.0, *key));
        let resp = ui.interact(slot, id, Sense::click_and_drag());
        app.inst.organ.bars.insert(*key, slot);
        let mut v = spec.get(key);
        if let Some(p) = resp.interact_pointer_pos() {
            if resp.dragged() || resp.clicked() {
                let n = ((p.y - slot.top() - g.handle_h * 0.5) / g.step)
                    .round()
                    .clamp(0.0, 8.0);
                if n != v {
                    v = n;
                    set(app, track, key, v, true);
                }
            }
        }
        if resp.double_clicked() {
            if let Some(info) = spec.kind.param(key) {
                v = info.default;
                set(app, track, key, v, false);
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            app.end_gesture();
        }
        hot[i] = resp.hovered() || resp.dragged();
        if hot[i] {
            let foot = footages.get(i).copied().unwrap_or("");
            let db = if v <= 0.0 {
                t("organ.bar_off")
            } else {
                format!("{:+.0} dB", drawbar_db(v))
            };
            let tip = if keys.len() == 9 {
                tf(
                    "organ.bar_tip",
                    &[
                        ("foot", &foot),
                        ("h", &HARMONICS[i]),
                        ("n", &(v as i32)),
                        ("db", &db),
                    ],
                )
            } else {
                tf(
                    "organ.pedal_tip",
                    &[("foot", &foot), ("n", &(v as i32)), ("db", &db)],
                )
            };
            hint::set_text(ui.ctx(), &tip, "", Caps::PARAM);
        }
        values[i] = v;
    }
    with_brush(&ui.painter_at(d), |b| {
        let t = &s.t;
        for (i, slot) in g.slots.iter().enumerate() {
            let kind = kinds.get(i).copied().unwrap_or(Kind::Octave);
            paint_bar(b, *slot, values[i], &g, kind, hot[i], s);
            let label = Text::new(footages.get(i).copied().unwrap_or(""), Role::Label, t.label)
                .caps(false)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(ktext::Elide::End));
            let col = Rect::from_center_size(
                Pos2::new(slot.center().x, g.foot.center().y),
                Vec2::new(g.col, g.foot.height()),
            );
            let _ = ktext::paint(b, col, &label, s.tk());
        }
    });
}

fn paint_bar(
    b: &mut crate::kit::Brush,
    slot: Rect,
    v: f32,
    g: &Geometry,
    kind: Kind,
    hot: bool,
    s: &Style,
) {
    let t = &s.t;
    let fill = match kind {
        Kind::Sub => t.hue(Function::Dynamics),
        Kind::Octave => t.key_white,
        Kind::Mutation => t.key_black,
    };
    // The slot: a narrow groove in the display, with the eight stops ticked beside it.
    let groove =
        Rect::from_center_size(slot.center(), Vec2::new(slot.width() * 0.36, slot.height()));
    light::sunken(b, groove, s.r(2.0), t.well, 1.0, t.depth);
    let tick = shade(t.ink, if t.dark { 0.22 } else { -0.22 });
    for n in 1..=8 {
        let y = slot.top() + g.handle_h * 0.5 + n as f32 * g.step;
        b.hline(
            slot.left() - s.s(4.0),
            slot.left() - s.s(1.0),
            y,
            b.px(),
            tick,
        );
    }
    // The shaft pulled out of the slot, then the handle with its position.
    let top = slot.top() + v.clamp(0.0, 8.0) * g.step;
    if v > 0.0 {
        let shaft = Rect::from_min_max(
            Pos2::new(groove.left() + b.px(), slot.top()),
            Pos2::new(groove.right() - b.px(), top + s.s(2.0)),
        );
        b.rect(
            b.snap_rect(shaft),
            Fill::V(with_alpha(fill, 0.55), with_alpha(fill, 0.85)),
        );
    }
    let handle = Rect::from_min_size(
        Pos2::new(slot.left(), top),
        Vec2::new(slot.width(), g.handle_h),
    );
    let base = if hot { shade(fill, 0.06) } else { fill };
    light::raised(
        b,
        handle,
        s.r(3.0),
        base,
        RaisedOpts {
            k: s.k,
            depth: t.depth,
            groove: Some(0.5),
            ..Default::default()
        },
    );
    if hot {
        b.rrect_stroke(handle.expand(s.s(1.0)), s.r(4.0), s.s(1.5), s.hue);
    }
    let digit = ["0", "1", "2", "3", "4", "5", "6", "7", "8"][v.round().clamp(0.0, 8.0) as usize];
    let _ = ktext::paint(
        b,
        handle,
        &Text::new(digit, Role::Value, on_fill(base)).align(Align2::CENTER_CENTER),
        s.tk(),
    );
}
