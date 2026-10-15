//! The audio section's own parts: the file overview, a push button in the value column,
//! and a fade's curves as five drawn keys.

use super::super::sheet::Sheet;
use crate::app::CantusApp;
use crate::audio_clips::{self as ac, Fade};
use crate::kit::color::{shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cap, well, Fill, State};
use crate::views::arrangement::curve_icon;
use crate::waveform::{fill_peaks, Lookup, Which};
use cantus_core::{AudioRegion, Clip, FadeCurve};
use egui::{pos2, Align2, Id, Rect, Sense, Vec2};

/// The overview's height at 100 %.
const OVERVIEW_H: f32 = 46.0;

thread_local! {
    static COLS: std::cell::RefCell<Vec<(f32, f32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The whole file in a well, the clip's window lit on it and the rest dimmed — where in
/// the take this clip is, at a glance (Pro Tools' region overview).
pub(super) fn overview(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    clip: &Clip,
    a: &AudioRegion,
    looked: &Lookup,
) {
    let s = sh.s;
    let w = sh.width();
    let (full, _) = sh
        .ui
        .allocate_exact_size(Vec2::new(w, s.s(OVERVIEW_H)), Sense::hover());
    let m = s.s(8.0);
    let r = Rect::from_min_max(
        pos2(full.left() + m, full.top() + s.s(2.0)),
        pos2(full.right() - m, full.bottom() - s.s(2.0)),
    );
    well::paint(sh.b, r, &s, State::REST, None);
    let inner = r.shrink(s.s(3.0));
    let hue = s.function(crate::kit::Function::Source).hue;
    match looked {
        Lookup::Ready(p) => {
            let ppp = sh.ui.ctx().pixels_per_point();
            let n = ((inner.width() * ppp).ceil() as usize).clamp(1, 4096);
            let per = p.frames as f64 / n as f64;
            let gain = cantus_core::db_to_linear(a.gain_db);
            COLS.with(|cell| {
                if let Ok(mut cols) = cell.try_borrow_mut() {
                    p.columns(Which::Mono, 0.0, per, n, &mut cols);
                    if a.reverse {
                        cols.reverse();
                    }
                    fill_peaks(sh.b, inner, &cols, |_| gain, with_alpha(hue, 0.9));
                }
            });
            // The window: what lies outside it is veiled.
            let (from, to) = ac::window_frames(app.project(), clip, p.rate);
            let total = p.frames.max(1) as f64;
            let (mut x0, mut x1) = (from / total, to / total);
            if a.reverse {
                (x0, x1) = (1.0 - x1, 1.0 - x0);
            }
            let x = |u: f64| inner.left() + inner.width() * u.clamp(0.0, 1.0) as f32;
            let veil = with_alpha(s.t.shadow, 0.55);
            let (l, rr) = (x(x0), x(x1));
            if l > inner.left() {
                sh.b.rect(
                    Rect::from_min_max(inner.min, pos2(l, inner.bottom())),
                    Fill::Solid(veil),
                );
            }
            if rr < inner.right() {
                sh.b.rect(
                    Rect::from_min_max(pos2(rr, inner.top()), inner.max),
                    Fill::Solid(veil),
                );
            }
            let edge = with_alpha(s.t.selection, 0.9);
            sh.b.rrect_stroke(
                Rect::from_min_max(pos2(l, inner.top()), pos2(rr.max(l + 1.0), inner.bottom())),
                s.r(1.5),
                sh.b.px() * 1.5,
                edge,
            );
        }
        Lookup::Pending => {
            sh.b.hline(
                inner.left(),
                inner.right(),
                inner.center().y,
                sh.b.px(),
                with_alpha(hue, 0.6),
            );
        }
        Lookup::Missing | Lookup::Failed(_) => {
            let tx = Text::key("audioclips.missing", Role::Micro, s.t.danger)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(sh.b, inner, &tx, s.tk());
        }
    }
}

/// A push button across the value column, its label inside it. `strong` draws it lit
/// (the one thing to do here, as *Locate…* for a missing file).
pub(super) fn button(
    sh: &mut Sheet<'_>,
    id: impl std::hash::Hash + std::fmt::Debug,
    key: &str,
    strong: bool,
) -> bool {
    button_with(sh, id, key, strong, None, true)
}

/// [`button`] with a tip key for the help line (its value, after the name), drawn
/// disabled (and never clicked) when not `enabled` (round 28: *Edit pitch*).
pub(super) fn button_with(
    sh: &mut Sheet<'_>,
    id: impl std::hash::Hash + std::fmt::Debug,
    key: &str,
    strong: bool,
    tip: Option<&str>,
    enabled: bool,
) -> bool {
    let s = sh.s;
    let row = sh.row();
    let w = row.value.width().min(s.s(140.0));
    let br = Rect::from_min_size(row.value.min, Vec2::new(w, row.value.height()));
    let r = sh
        .ui
        .interact(br, Id::new(("insp-btn", id)), Sense::click());
    let st = State {
        disabled: !enabled,
        ..State::of(&r, strong)
    };
    let look = cap::CapLook {
        lit: Some(if strong { s.t.highlight } else { s.t.selection }),
        ..Default::default()
    };
    cap::paint(sh.b, br, &s, st, &look);
    // Text on the cap's own face, whatever the theme makes it.
    let ink = crate::kit::color::on_fill(cap::face(&s, st, &look));
    let tx = Text::key(key, Role::Control, ink)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(sh.b, br.shrink2(Vec2::new(s.s(6.0), 0.0)), &tx, s.tk());
    if r.hovered() {
        let name = sh.fmt.tr(key).to_string();
        let value = tip.map(|k| sh.fmt.tr(k).to_string()).unwrap_or_default();
        hint::set_text(sh.ui.ctx(), &name, &value, Caps::NONE);
    }
    r.clicked() && enabled
}

/// A fade's curve as five drawn keys in the value column, the current one lit. Returns
/// the one clicked.
pub(super) fn curves(
    sh: &mut Sheet<'_>,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    fade: Fade,
    current: FadeCurve,
) -> Option<FadeCurve> {
    let s = sh.s;
    let row = sh.row();
    sh.label(row.label, "audioclips.inspector.curve", false);
    let n = FadeCurve::ALL.len() as f32;
    let gap = s.s(2.0);
    let kw = ((row.value.width() - gap * (n - 1.0)) / n).min(s.s(30.0));
    let mut picked = None;
    for (i, (c, tip)) in FadeCurve::ALL
        .iter()
        .copied()
        .zip(crate::views::arrangement::CURVE_KEYS)
        .enumerate()
    {
        let x = row.value.left() + i as f32 * (kw + gap);
        let kr = Rect::from_min_size(pos2(x, row.value.top()), Vec2::new(kw, row.value.height()));
        let r = sh
            .ui
            .interact(kr, Id::new(("insp-curve", id, i)), Sense::click());
        let lit = c == current;
        let st = State::of(&r, lit);
        well::paint(sh.b, kr, &s, st, None);
        if lit {
            sh.b.rrect(
                kr.shrink(sh.b.px()),
                s.r(2.0),
                Fill::Solid(with_alpha(s.t.selection, 0.18)),
            );
            sh.b.rrect_stroke(kr, s.r(2.0), sh.b.px(), s.t.selection);
        }
        let ink = if lit {
            s.t.text
        } else {
            shade(s.t.text_dim, 0.0)
        };
        curve_icon(sh.b, kr, c, fade, ink, s.k);
        if r.hovered() {
            let name = sh.fmt.tr(tip).to_string();
            hint::set_text(sh.ui.ctx(), &name, "", Caps::NONE);
        }
        if r.clicked() && !lit {
            picked = Some(c);
        }
    }
    picked
}
