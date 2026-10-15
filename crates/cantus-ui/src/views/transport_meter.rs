//! The master meter (ADR 0052): two lanes in a well, with a PEAK / LUFS switch (kept in
//! the settings). **Peak**: the master's left and right sample peaks, the highest of the
//! last two seconds held as a line and as a number (a click on it lets go). **LUFS**: the
//! master's loudness from the engine (`Meters::master_loudness`, BS.1770 / R128):
//! momentary over short-term, a tick at −14 LUFS (the streaming services' level), and
//! the momentary, short-term, integrated (since play) and true-peak numbers.

use super::Style;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::meter::{self, Levels};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{segmented, shell, with_brush, State};
use cantus_core::linear_to_db;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// How long a peak is held.
const HOLD: Duration = Duration::from_secs(2);
/// The streaming services' loudness, marked on the LUFS lanes.
const TARGET_LUFS: f32 = -14.0;

/// The held peak (linear) and when it was reached.
#[derive(Debug, Default)]
pub struct Hold {
    peak: f32,
    at: Option<Instant>,
}

impl Hold {
    /// Take this block's peak; returns the one held.
    fn feed(&mut self, peak: f32) -> f32 {
        let stale = self.at.is_none_or(|t| t.elapsed() >= HOLD);
        if peak >= self.peak || stale {
            self.peak = peak;
            self.at = Some(Instant::now());
        }
        self.peak
    }

    fn clear(&mut self) {
        self.peak = 0.0;
        self.at = None;
    }
}

thread_local! {
    static WORDS: RefCell<(String, String)> =
        RefCell::new((String::with_capacity(24), String::with_capacity(48)));
}

const MODES: [Src<'static>; 2] = [Src::Key("topbar.meter.peak"), Src::Key("topbar.meter.lufs")];

fn toggle_w(ctx: &egui::Context, s: &Style) -> f32 {
    MODES
        .iter()
        .map(|m| match m {
            Src::Key(k) => {
                text::measure(ctx, &Text::key(k, Role::Micro, s.t.text).caps(true), s.tk()).x
            }
            Src::Str(_) => 0.0,
        })
        .fold(0.0, f32::max)
        * 2.0
        + 20.0 * s.k
}

/// The meter's least and most width.
pub fn widths(ctx: &egui::Context, s: &Style) -> (f32, f32) {
    let readout =
        text::measure(ctx, &Text::new("\u{2212}88.8 LUFS", Role::Value, s.t.text), s.tk()).x;
    let min = toggle_w(ctx, s) + readout + 12.0 * s.k;
    (min.max(150.0 * s.k), min.max(280.0 * s.k))
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let m = app.engine.meters();
    let peaks = m.master;
    let loud = m.master_loudness;
    let lufs = app.settings.topbar.meter_lufs;
    let held = app.topbar.hold.feed(peaks[0].max(peaks[1]));
    let row_h = 15.0 * k;
    let top = Rect::from_min_size(
        Pos2::new(r.left(), r.center().y - 21.0 * k),
        Vec2::new(r.width(), row_h),
    );
    let lanes = Rect::from_min_size(
        Pos2::new(r.left(), top.bottom() + 3.0 * k),
        Vec2::new(r.width(), 13.0 * k),
    );
    let foot = Rect::from_min_size(
        Pos2::new(r.left(), lanes.bottom() + 1.0 * k),
        Vec2::new(r.width(), 12.0 * k),
    );
    // The PEAK / LUFS switch.
    let tr = Rect::from_min_size(top.min, Vec2::new(toggle_w(ui.ctx(), s), row_h));
    let tresp = ui.interact(tr, Id::new("toolbar-meter-mode"), Sense::click());
    let hovered = tresp
        .hover_pos()
        .and_then(|p| segmented::hit(tr, MODES.len(), p.x));
    if tresp.clicked() {
        if let Some(i) = hovered {
            let want = i == 1;
            if want != lufs {
                app.settings.topbar.meter_lufs = want;
                app.store_settings();
            }
        }
    }
    if tresp.hovered() {
        hint::set_text(ui.ctx(), &t("topbar.meter.mode_tip"), "", Caps::NONE);
    }
    let lufs = app.settings.topbar.meter_lufs;
    // The readout at the right: a click lets the held peak go.
    let rr = Rect::from_min_max(
        Pos2::new(tr.right() + 6.0 * k, top.top()),
        Pos2::new(top.right() - 2.0 * k, top.bottom()),
    );
    let rresp = ui.interact(rr, Id::new("toolbar-meter-readout"), Sense::click());
    if rresp.clicked() {
        app.topbar.hold.clear();
    }
    let clip = held >= 1.0;
    let db = |v: f32| linear_to_db(v);
    let levels = if lufs {
        Levels {
            level: [loud.momentary, loud.short_term],
            hold: [loud.momentary, loud.short_term],
        }
    } else {
        Levels {
            level: [db(peaks[0]), db(peaks[1])],
            hold: [db(held); 2],
        }
    };
    let lresp = ui.interact(lanes, Id::new("toolbar-master"), Sense::hover());
    WORDS.with(|w| {
        let mut w = w.borrow_mut();
        let (readout, footer) = &mut *w;
        readout.clear();
        footer.clear();
        let num = |out: &mut String, v: f32| {
            if v.is_finite() {
                if v < 0.0 {
                    let _ = write!(out, "\u{2212}{:.1}", -v);
                } else {
                    let _ = write!(out, "{v:.1}");
                }
            } else {
                out.push('\u{2014}');
            }
        };
        if lufs {
            num(readout, loud.momentary);
            readout.push_str(" LUFS");
            footer.push_str("S ");
            num(footer, loud.short_term);
            footer.push_str("  I ");
            num(footer, loud.integrated);
            footer.push_str("  TP ");
            let tp = if loud.true_peak > 0.0 {
                db(loud.true_peak)
            } else {
                f32::NEG_INFINITY
            };
            num(footer, tp);
        } else {
            num(readout, if held > 0.0 { db(held) } else { f32::NEG_INFINITY });
            readout.push_str(" dB");
        }
        if rresp.hovered() {
            let name = if lufs {
                t("topbar.meter.lufs_tip")
            } else {
                t("topbar.meter.peak_tip")
            };
            hint::set_text(ui.ctx(), &name, readout, Caps::NONE);
        }
        if let Some(p) = lresp.hover_pos() {
            let pos = (p.x - lanes.left()) / lanes.width().max(1.0);
            hint::set(ui.ctx(), &t("kit.mixer.master"), Caps::NONE, false, |b| {
                write!(b, "{:.1} dB", meter::db_at(pos))
            });
        }
        with_brush(ui.painter(), |b| {
            let st = State::of(&tresp, false);
            segmented::paint(b, tr, s, st, &MODES, usize::from(lufs), hovered);
            let ink = if clip && !lufs { s.t.danger } else { s.t.text };
            let _ = text::paint(
                b,
                rr,
                &Text::new(readout, Role::Value, ink)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            b.register(lanes, "meter", meter::TOKENS, &[]);
            shell::horizontal_meter(b, lanes, s, &levels);
            let inner = lanes.shrink(b.px() * 1.5);
            let mark = |b: &mut crate::kit::Brush, v: f32, ink| {
                let x = inner.left() + inner.width() * meter::pos_of_db(v);
                b.vline(b.snap_mid(x), inner.top(), inner.bottom(), b.px(), ink);
            };
            if lufs {
                mark(b, TARGET_LUFS, with_alpha(s.t.text, 0.7));
            } else if held > 0.0 {
                mark(b, db(held), s.t.text);
            }
            let word = if lufs {
                Text::new(footer, Role::Micro, s.t.text_dim)
            } else if clip {
                Text::key("workspace.clip", Role::Micro, s.t.danger)
            } else {
                Text::key("kit.mixer.master", Role::Micro, s.t.text_dim)
            };
            let _ = text::paint(
                b,
                foot,
                &word
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
    });
}
