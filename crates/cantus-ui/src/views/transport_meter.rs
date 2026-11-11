//! The master meter (ADR 0052): two lanes in a well, with a PEAK / LUFS switch (kept in
//! the settings). **Peak**: the master's left and right sample peaks, the highest of the
//! last two seconds held as a line and as a number (a click on it lets go). **LUFS**: the
//! master's loudness from the engine (`Meters::master_loudness`, BS.1770 / R128):
//! momentary over short-term, a tick at −14 LUFS (the streaming services' level), and
//! the momentary, short-term, integrated (since play) and true-peak numbers.

use super::paint::{self as p, ink1, ink3};
use super::Style;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::mix;
use crate::kit::hint::{self, Caps};

use crate::kit::text::Role;
use crate::kit::{with_brush, Fill};
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
    static WORDS: RefCell<String> = RefCell::new(String::with_capacity(48));
}

const MODES: [&str; 2] = ["topbar.meter.peak", "topbar.meter.lufs"];

/// The bar meter's scale (`dir-a`): dB to a place along the lanes, the ticks at −48, −24,
/// −12, −6 and 0 where the sheet draws them.
const SCALE: [(f32, f32); 7] = [
    (-60.0, 0.0),
    (-48.0, 0.12),
    (-24.0, 0.42),
    (-12.0, 0.62),
    (-6.0, 0.76),
    (0.0, 0.9),
    (6.0, 1.0),
];

/// Where `db` sits along the lanes, 0…1.
pub fn pos_of(db: f32) -> f32 {
    if !db.is_finite() || db <= SCALE[0].0 {
        return 0.0;
    }
    for w in SCALE.windows(2) {
        let ((d0, p0), (d1, p1)) = (w[0], w[1]);
        if db <= d1 {
            return p0 + (p1 - p0) * (db - d0) / (d1 - d0);
        }
    }
    1.0
}

/// The level at a place along the lanes (the hint under the pointer).
pub fn db_at(pos: f32) -> f32 {
    let pos = pos.clamp(0.0, 1.0);
    for w in SCALE.windows(2) {
        let ((d0, p0), (d1, p1)) = (w[0], w[1]);
        if pos <= p1 {
            return d0 + (d1 - d0) * (pos - p0) / (p1 - p0).max(1e-6);
        }
    }
    SCALE[6].0
}

/// The mode word's width (10 Medium).
fn label_w(ctx: &egui::Context, s: &Style) -> f32 {
    MODES
        .iter()
        .map(|k| p::measure_key(ctx, k, Role::Control, 10.0, s))
        .fold(0.0, f32::max)
}

/// The meter's widths: compact (the lanes alone, 92) and full (the mode word before them,
/// 132), wider where a language's word needs it.
pub fn widths(ctx: &egui::Context, s: &Style) -> (f32, f32) {
    let full = (8.0 * s.k + label_w(ctx, s) + 4.0 * s.k + 88.0 * s.k + 10.0 * s.k).max(132.0 * s.k);
    (92.0 * s.k, full)
}

/// The two lanes in `lanes` (each 3 high, 2 apart, the scale's ticks under them): grooves
/// at rest — never "−∞" — and the levels in green, amber from −6 dB, red from −1.
fn lanes_paint(
    b: &mut crate::kit::Brush,
    lanes: Rect,
    s: &Style,
    levels: [f32; 2],
    hold: f32,
    lufs: bool,
) {
    let k = s.k;
    let groove = mix(s.t.well, p::ink4(s), 0.28);
    let (amber, red) = (pos_of(-6.0), pos_of(-1.0));
    for (i, lv) in levels.into_iter().enumerate() {
        let lane = Rect::from_min_size(
            Pos2::new(lanes.left(), lanes.top() + i as f32 * 5.0 * k),
            Vec2::new(lanes.width(), 3.0 * k),
        );
        let lane = b.snap_rect(lane);
        b.rrect(lane, 1.0 * k, Fill::Solid(groove));
        let at = pos_of(lv);
        if at <= 0.0 {
            continue;
        }
        let seg = |b: &mut crate::kit::Brush, f0: f32, f1: f32, c: egui::Color32| {
            if f1 > f0 {
                let r = Rect::from_min_max(
                    Pos2::new(lane.left() + lane.width() * f0, lane.top()),
                    Pos2::new(lane.left() + lane.width() * f1, lane.bottom()),
                );
                b.rect(b.snap_rect(r), Fill::Solid(c));
            }
        };
        seg(b, 0.0, at.min(amber), s.t.meter_low);
        seg(b, amber, at.min(red), s.t.meter_high);
        seg(b, red, at, s.t.meter_clip);
    }
    let ticks_y = lanes.top() + 11.0 * k;
    for db in [-48.0f32, -24.0, -12.0, -6.0, 0.0] {
        let x = b.snap_mid(lanes.left() + lanes.width() * pos_of(db));
        let (h, c) = if db == 0.0 {
            (3.0, ink3(s))
        } else {
            (2.0, p::ink4(s))
        };
        b.vline(x, ticks_y, ticks_y + h * k, b.px(), c);
    }
    let mark = if lufs { TARGET_LUFS } else { hold };
    if mark.is_finite() && pos_of(mark) > 0.0 {
        let x = b.snap_mid(lanes.left() + lanes.width() * pos_of(mark));
        b.vline(
            x,
            lanes.top() - 1.0 * k,
            lanes.top() + 9.0 * k,
            b.px(),
            ink1(s),
        );
    }
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style, compact: bool) {
    let k = s.k;
    let m = app.engine.meters();
    let peaks = m.master;
    let loud = m.master_loudness;
    let lufs = app.settings.topbar.meter_lufs;
    // Fed once a frame by `watch`, drawn or not.
    let held = app.topbar.hold.peak();
    let true_peak = app.topbar.hold.true_peak();
    let (_, full) = widths(ui.ctx(), s);
    let row = p::row(r, s);
    let mw = if compact { 92.0 * k } else { full };
    let well = Rect::from_min_size(row.min, Vec2::new(mw, row.height()));
    let x0 = if compact {
        8.0 * k
    } else {
        mw - 88.0 * k - 10.0 * k
    };
    let lanes = Rect::from_min_max(
        Pos2::new(well.left() + x0, well.top() + 6.0 * k),
        Pos2::new(well.right() - 10.0 * k, well.top() + 15.0 * k),
    );
    let label_r = Rect::from_min_max(
        well.min,
        Pos2::new(well.left() + x0 - 2.0 * k, well.bottom()),
    );
    let lresp = ui.interact(well, Id::new("toolbar-master"), Sense::click());
    let mresp =
        (!compact).then(|| ui.interact(label_r, Id::new("toolbar-meter-mode"), Sense::click()));
    let toggle = mresp.as_ref().is_some_and(|r| r.clicked()) || lresp.secondary_clicked();
    if toggle {
        app.settings.topbar.meter_lufs = !lufs;
        app.store_settings();
    }
    if lresp.clicked() {
        app.topbar.hold.clear();
    }
    let lufs = app.settings.topbar.meter_lufs;
    let db = |v: f32| linear_to_db(v);
    let levels = if lufs {
        [loud.momentary, loud.short_term]
    } else {
        [db(peaks[0]), db(peaks[1])]
    };
    let clip = held >= 1.0;
    if mresp.as_ref().is_some_and(|r| r.hovered()) {
        hint::set_text(ui.ctx(), &t("topbar.meter.mode_tip"), "", Caps::NONE);
    } else if lresp.hovered() {
        WORDS.with(|w| {
            let mut w = w.borrow_mut();
            w.clear();
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
                w.push_str("M ");
                num(&mut w, loud.momentary);
                w.push_str("  S ");
                num(&mut w, loud.short_term);
                w.push_str("  I ");
                num(&mut w, loud.integrated);
                w.push_str("  TP ");
                let tp = if true_peak > 0.0 {
                    db(true_peak)
                } else {
                    f32::NEG_INFINITY
                };
                num(&mut w, tp);
            } else {
                num(
                    &mut w,
                    if held > 0.0 {
                        db(held)
                    } else {
                        f32::NEG_INFINITY
                    },
                );
                w.push_str(" dB");
                if let Some(p) = lresp.hover_pos() {
                    let pos = (p.x - lanes.left()) / lanes.width().max(1.0);
                    let _ = write!(w, "  \u{b7}  {:.1} dB", db_at(pos));
                }
            }
            // Its name short on the help line, the figures after it; what they mean and
            // what a click does in the tooltip.
            hint::set_text(ui.ctx(), &t("topbar.group.meter"), &w, Caps::NONE);
            let tip = if lufs {
                t("topbar.meter.lufs_tip")
            } else {
                t("topbar.meter.peak_tip")
            };
            let _ = lresp.clone().on_hover_text(tip);
        });
    }
    with_brush(ui.painter(), |b| {
        b.register(well, "meter", crate::kit::meter::TOKENS, &[]);
        p::well(b, well, s, None);
        if !compact {
            let hov = mresp.as_ref().is_some_and(|r| r.hovered());
            let ink = if hov { ink1(s) } else { ink3(s) };
            let lr = Rect::from_min_max(Pos2::new(well.left() + 8.0 * k, well.top()), label_r.max);
            p::words_key(
                b,
                lr,
                MODES[usize::from(lufs)],
                Role::Control,
                10.0,
                ink,
                Align2::LEFT_CENTER,
                s,
            );
        }
        lanes_paint(
            b,
            lanes,
            s,
            levels,
            if held > 0.0 {
                db(held)
            } else {
                f32::NEG_INFINITY
            },
            lufs,
        );
        if clip && !lufs {
            let lamp = Rect::from_min_size(
                Pos2::new(well.right() - 7.0 * k, well.top() + 5.0 * k),
                Vec2::new(3.0 * k, 9.0 * k),
            );
            b.rrect(b.snap_rect(lamp), 1.0 * k, Fill::Solid(s.t.meter_clip));
        }
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
