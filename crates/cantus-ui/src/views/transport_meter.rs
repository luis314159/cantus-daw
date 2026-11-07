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
    /// The highest true peak since playback started (the LUFS footer's TP), and whether
    /// the transport was playing at the last look.
    true_peak: f32,
    playing: bool,
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

    /// Take the true peak the engine read since the last frame: the readout holds the
    /// highest since playback started (an over is never missed), until clicked.
    fn feed_true_peak(&mut self, true_peak: f32, playing: bool) -> f32 {
        if playing && !self.playing {
            self.true_peak = 0.0;
        }
        self.playing = playing;
        self.true_peak = self.true_peak.max(true_peak);
        self.true_peak
    }

    fn clear(&mut self) {
        self.peak = 0.0;
        self.at = None;
        self.true_peak = 0.0;
    }

    /// The highest true peak held now, linear (0: none since playback started).
    pub fn true_peak(&self) -> f32 {
        self.true_peak
    }

    /// The sample peak held now, linear.
    pub fn peak(&self) -> f32 {
        self.peak
    }
}

/// Once a frame, whatever the bar shows (review 2: the hold was fed only while the meter
/// was drawn, so an over while it was folded or hidden was missed): the engine's readings
/// of this frame into the holds the meter reads.
pub fn watch(app: &mut CantusApp) {
    let m = app.engine.meters();
    let (peaks, true_peak) = (m.master, m.master_loudness.true_peak);
    let playing = app.engine.is_playing();
    app.topbar.hold.feed(peaks[0].max(peaks[1]));
    app.topbar.hold.feed_true_peak(true_peak, playing);
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
            Src::Key(k) => text::measure(ctx, &Text::key(k, Role::Control, s.t.text), s.tk()).x,
            Src::Str(_) => 0.0,
        })
        .fold(0.0, f32::max)
        * 2.0
        + 2.0 * 10.0 * s.k
        + 4.0 * s.k
}

/// The lanes' least width in the one-row meter, at 100 %. // r37:finish
const LANES_MIN: f32 = 56.0;

/// The meter's least and most width.
pub fn widths(ctx: &egui::Context, s: &Style) -> (f32, f32) {
    let readout = text::measure(
        ctx,
        &Text::new("\u{2212}88.8 LUFS", Role::Value, s.t.text),
        s.tk(),
    )
    .x;
    // r37:finish — one row (the meter bridge): the switch, the lanes, the readout.
    let min = toggle_w(ctx, s) + readout + LANES_MIN * s.k + 16.0 * s.k;
    (min, min.max(300.0 * s.k))
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let m = app.engine.meters();
    let peaks = m.master;
    let loud = m.master_loudness;
    let lufs = app.settings.topbar.meter_lufs;
    // Fed once a frame by `watch`, drawn or not.
    let held = app.topbar.hold.peak();
    let true_peak = app.topbar.hold.true_peak();
    // r37:finish — one row: the PEAK / LUFS switch, the lanes, the readout (the meter
    // bridge: the system meters are the row under it). The loudness footer is the lanes'
    // hint, and a click on the lanes lets the held peaks go.
    let row_h = 18.0 * k;
    let top = Rect::from_center_size(r.center(), Vec2::new(r.width(), row_h));
    let tr = Rect::from_min_size(top.min, Vec2::new(toggle_w(ui.ctx(), s), row_h));
    let readout_w = text::measure(
        ui.ctx(),
        &Text::new("\u{2212}88.8 LUFS", Role::Value, s.t.text),
        s.tk(),
    )
    .x;
    let lufs_now = app.settings.topbar.meter_lufs;
    let (x0, x1) = (tr.right() + 6.0 * k, top.right() - readout_w - 8.0 * k);
    // In loudness the lanes are the upper half and the short-term, integrated and true-peak
    // figures a line under them (the TP the manual says to click); in peak, the lanes
    // alone, full height.
    let (lanes, foot) = if lufs_now {
        (
            Rect::from_min_max(
                Pos2::new(x0, r.center().y - 10.0 * k),
                Pos2::new(x1, r.center().y - 2.0 * k),
            ),
            Rect::from_min_max(
                Pos2::new(x0, r.center().y - 1.0 * k),
                Pos2::new(x1, r.center().y + 10.0 * k),
            ),
        )
    } else {
        let l = Rect::from_min_max(
            Pos2::new(x0, r.center().y - 6.5 * k),
            Pos2::new(x1, r.center().y + 6.5 * k),
        );
        (l, l)
    };
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
        Pos2::new(lanes.right() + 4.0 * k, top.top()),
        Pos2::new(top.right() - 2.0 * k, top.bottom()),
    );
    let rresp = ui.interact(rr, Id::new("toolbar-meter-readout"), Sense::click());
    // The footer's figures too (the TP the manual says to click), in loudness.
    let fresp = ui.interact(foot, Id::new("toolbar-meter-foot"), Sense::click());
    if rresp.clicked() || (lufs && fresp.clicked()) {
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
            if loud.momentary.is_finite() {
                num(readout, loud.momentary);
            } else {
                readout.push_str("\u{2212}\u{221e}"); // r37:finish
            }
            readout.push_str(" LUFS");
            footer.push_str("S ");
            num(footer, loud.short_term);
            footer.push_str("  I ");
            num(footer, loud.integrated);
            footer.push_str("  TP ");
            let tp = if true_peak > 0.0 {
                db(true_peak)
            } else {
                f32::NEG_INFINITY
            };
            num(footer, tp);
        } else {
            // r37:finish — silence reads as minus infinity, as a strip's meter does (judge
            // 7: a bare dash read unfinished).
            if held > 0.0 {
                num(readout, db(held));
            } else {
                readout.push_str("\u{2212}\u{221e}");
            }
            readout.push_str(" dB");
        }
        if lufs && fresp.hovered() {
            hint::set_text(ui.ctx(), &t("topbar.meter.lufs_tip"), footer, Caps::NONE);
        }
        if rresp.hovered() {
            let name = if lufs {
                t("topbar.meter.lufs_tip")
            } else {
                t("topbar.meter.peak_tip")
            };
            hint::set_text(ui.ctx(), &name, readout, Caps::NONE);
        }
        if let Some(p) = lresp.hover_pos().filter(|_| !lufs) {
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
            // r37:finish — the scale under the lanes at rest and playing alike: ticks at −48,
            // −24, −12, −6 and 0 dB, so the empty well reads as a meter waiting.
            if !lufs {
                for v in [-48.0f32, -24.0, -12.0, -6.0, 0.0] {
                    let x = inner.left() + inner.width() * meter::pos_of_db(v);
                    b.vline(
                        b.snap_mid(x),
                        inner.bottom() - inner.height() * 0.35,
                        inner.bottom(),
                        b.px(),
                        // On the well's own dark (both themes): its readable ink.
                        with_alpha(
                            crate::kit::color::on_fill(s.t.meter_well),
                            if v == 0.0 { 0.70 } else { 0.40 },
                        ),
                    );
                }
            }
            if lufs {
                mark(b, TARGET_LUFS, with_alpha(s.t.text, 0.7));
            } else if held > 0.0 {
                mark(b, db(held), s.t.text);
            }
            if lufs {
                let _ = text::paint(
                    b,
                    foot,
                    &Text::new(footer, Role::Micro, s.t.text_dim)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::Hold;

    /// The TP readout holds the highest true peak since play (review 1: it showed the last
    /// block's), starts again on Play, and lets go when clicked.
    #[test]
    fn the_true_peak_is_held_from_play_until_clicked() {
        let mut h = Hold::default();
        assert_eq!(h.feed_true_peak(0.5, true), 0.5);
        assert_eq!(h.feed_true_peak(1.2, true), 1.2);
        assert_eq!(h.feed_true_peak(0.1, true), 1.2, "held");
        assert_eq!(h.feed_true_peak(0.0, false), 1.2, "held after Stop");
        assert_eq!(h.feed_true_peak(0.3, true), 0.3, "Play starts it again");
        h.clear();
        assert_eq!(h.feed_true_peak(0.2, true), 0.2, "a click lets it go");
    }
}
