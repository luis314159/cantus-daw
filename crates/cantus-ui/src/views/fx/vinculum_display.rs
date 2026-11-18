//! Vinculum's display (design sheet v1): the bus compressor in its rack, dark in every
//! theme.
//!
//! - **The rack**: two plain steel rails, the unit screwed to them by its ears.
//! - **The faceplate** (the colourway's finish): the maker's name and the model's own name
//!   engraved along the top, jewel lamps for the external key, the Auto release, the limit
//!   and the valve colour.
//! - **The meter bridge**: the gain reduction's needle (0 dB at the right) and the output's,
//!   each behind its window on a card lit from behind; between them a window on the model's
//!   gain element — the VCA's chip on its board, the opto's cell with its panel glowing, the
//!   vari-mu's valves.
//! - **The model's panel**: the model selector and the model's own stepped switches (the
//!   VCA's ratio, attack and release; the opto's and the vari-mu's Compress or Limit toggle;
//!   the vari-mu's six time positions) — each a real control: a click on a position chooses
//!   it, a click on the knob steps it, its menu offers automation and MIDI learn.
//! - **The screen**: the static transfer curve at the settings (make-up and mix included)
//!   with the signal's point on it, the reduction in large figures, and the gain reduction
//!   over the last eight seconds hanging from the top over the input and output levels (the
//!   key's too when one is routed).
//!
//! Every number from the engine's telemetry (`cantus_engine::fx::vinculum::t`).

use super::super::canvas::Ink;
use super::super::host::FxHost;
use super::face::{self, Panel};
use super::{art, screen};
use crate::kit::{with_brush, Style};
use cantus_engine::fx::vinculum::t;
use egui::{pos2, vec2, Rect};

/// One block's numbers as the display reads them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Reading {
    pub seen: bool,
    /// The reduction now and its deepest in the block, dB (≥ 0).
    pub gr: f32,
    pub gr_peak: f32,
    /// The input's, the output's and the key's peaks, dBFS.
    pub in_db: f32,
    pub out_db: f32,
    pub key_db: f32,
    pub keyed: bool,
}

/// The floor every level is clamped to, dBFS.
pub const FLOOR_DB: f32 = -96.0;

fn db(x: f32) -> f32 {
    if x > 1e-6 {
        (20.0 * cantus_dsp::math::log10(x)).max(FLOOR_DB)
    } else {
        FLOOR_DB
    }
}

impl Default for Reading {
    fn default() -> Self {
        Reading {
            seen: false,
            gr: 0.0,
            gr_peak: 0.0,
            in_db: FLOOR_DB,
            out_db: FLOOR_DB,
            key_db: FLOOR_DB,
            keyed: false,
        }
    }
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        if tel[t::SAMPLE_RATE] <= 0.0 {
            return Reading::default();
        }
        Reading {
            seen: true,
            gr: tel[t::GR].max(0.0),
            gr_peak: tel[t::GR_PEAK].max(tel[t::GR]).max(0.0),
            in_db: db(tel[t::IN_L].max(tel[t::IN_R])),
            out_db: db(tel[t::OUT_L].max(tel[t::OUT_R])),
            key_db: db(tel[t::KEY]),
            keyed: tel[t::KEYED] >= 0.5,
        }
    }
}

/// One point of the history: the deepest reduction since the point before, the levels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Point {
    pub gr: f32,
    pub in_db: f32,
    pub out_db: f32,
    pub key_db: f32,
}

impl Point {
    const QUIET: Point = Point {
        gr: 0.0,
        in_db: FLOOR_DB,
        out_db: FLOOR_DB,
        key_db: FLOOR_DB,
    };
}

/// The history's length: eight seconds at thirty points a second.
pub const HISTORY: usize = 240;
/// Points a second.
pub const RATE: f32 = 30.0;

/// The display's memory: the two needles, swinging with a meter's weight; the peak
/// marker, held a moment; the history of the reduction and the levels (a ring, filled at
/// [`RATE`], never allocated after it is made).
#[derive(Debug)]
pub struct View {
    pub needle_gr: f32,
    pub needle_out: f32,
    peak: f32,
    hold: f32,
    ring: [Point; HISTORY],
    head: usize,
    len: usize,
    acc: f32,
    pending: Point,
}

impl Default for View {
    fn default() -> Self {
        View {
            needle_gr: 0.0,
            needle_out: FLOOR_DB,
            peak: 0.0,
            hold: 0.0,
            ring: [Point::QUIET; HISTORY],
            head: 0,
            len: 0,
            acc: 0.0,
            pending: Point::QUIET,
        }
    }
}

impl View {
    /// The needles move toward the reading (about 300 ms to get there, as a meter's
    /// movement does), the peak holds 1.5 s, and the history takes a point every
    /// thirtieth of a second — the deepest reduction and the loudest levels since the last.
    pub fn update(&mut self, r: &Reading, dt: f32) {
        let a = (dt / 0.065).min(1.0);
        let (gr, out) = if r.seen {
            (r.gr, r.out_db)
        } else {
            (0.0, FLOOR_DB)
        };
        self.needle_gr += (gr - self.needle_gr) * a;
        self.needle_out += (out.max(-60.0) - self.needle_out.max(-60.0)) * a;
        if r.seen && r.gr_peak >= self.peak {
            self.peak = r.gr_peak;
            self.hold = 1.5;
        } else {
            self.hold -= dt;
            if self.hold <= 0.0 {
                self.peak = (self.peak - dt * 6.0).max(gr);
            }
        }
        if !r.seen {
            return;
        }
        let p = &mut self.pending;
        p.gr = p.gr.max(r.gr_peak);
        p.in_db = p.in_db.max(r.in_db);
        p.out_db = p.out_db.max(r.out_db);
        p.key_db = p.key_db.max(r.key_db);
        self.acc += dt;
        if self.acc >= 1.0 / RATE {
            let p = std::mem::replace(&mut self.pending, Point::QUIET);
            while self.acc >= 1.0 / RATE {
                self.acc -= 1.0 / RATE;
                self.push(p);
            }
        }
    }

    fn push(&mut self, p: Point) {
        self.ring[self.head] = p;
        self.head = (self.head + 1) % HISTORY;
        self.len = (self.len + 1).min(HISTORY);
    }

    /// The screenshots' demo: the history the real unit made, the needles where it left
    /// them.
    pub fn seed(&mut self, points: &[Point], last: &Reading) {
        *self = View::default();
        for p in points.iter().rev().take(HISTORY).rev() {
            self.push(*p);
        }
        self.needle_gr = last.gr;
        self.needle_out = last.out_db.max(-60.0);
        self.peak = points
            .iter()
            .rev()
            .take(45)
            .fold(last.gr_peak, |m, p| m.max(p.gr));
        self.hold = 1.5;
    }

    /// The peak marker's reduction, dB.
    pub fn peak(&self) -> f32 {
        self.peak
    }

    /// How many points the history holds.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The history, oldest first.
    pub fn points(&self) -> impl Iterator<Item = &Point> + '_ {
        let start = (self.head + HISTORY - self.len) % HISTORY;
        (0..self.len).map(move |i| &self.ring[(start + i) % HISTORY])
    }
}

/// Where the display's parts stand.
pub struct Parts {
    /// The faceplate between its ears.
    pub face: Rect,
    /// The strip along the top: the names and the lamps.
    pub strip: Rect,
    pub gr_meter: Rect,
    pub element: Rect,
    pub out_meter: Rect,
    /// The model's panel (its switches) and the screen beside it.
    pub panel: Rect,
    pub screen: Rect,
}

/// The rails' width and the ears', at scale `k`.
const RAIL: f32 = 30.0;
const EAR: f32 = 46.0;

/// The display's layout in `r`: the faceplate, its strip, its meter bridge, its panel (as
/// wide as the model's switches, `panel_w`) and the screen.
pub fn parts(r: Rect, k: f32, panel_w: f32) -> Parts {
    let plate = Rect::from_min_max(
        pos2(r.left() + RAIL * k * 0.8, r.top() + 16.0 * k),
        pos2(r.right() - RAIL * k * 0.8, r.bottom() - 16.0 * k),
    );
    let face = plate.shrink2(vec2(EAR * k, 0.0));
    let inner = face.shrink2(vec2(14.0 * k, 12.0 * k));
    let strip = Rect::from_min_size(inner.min, vec2(inner.width(), 46.0 * k));
    let rest = Rect::from_min_max(pos2(inner.left(), strip.bottom() + 6.0 * k), inner.max);
    let gap = 16.0 * k;
    let row1 = Rect::from_min_size(rest.min, vec2(rest.width(), rest.height() * 0.55));
    let row2 = Rect::from_min_max(pos2(rest.left(), row1.bottom() + gap), rest.max);
    let mw = (row1.width() - 2.0 * gap) * 0.30;
    let gr_meter = Rect::from_min_size(row1.min, vec2(mw, row1.height()));
    let out_meter = Rect::from_min_max(pos2(row1.right() - mw, row1.top()), row1.max);
    let element = Rect::from_min_max(
        pos2(gr_meter.right() + gap, row1.top()),
        pos2(out_meter.left() - gap, row1.bottom()),
    );
    let pw = panel_w.min(row2.width() * 0.6);
    let panel = Rect::from_min_size(row2.min, vec2(pw, row2.height()));
    let screen = Rect::from_min_max(pos2(panel.right() + gap, row2.top()), row2.max);
    Parts {
        face,
        strip,
        gr_meter,
        element,
        out_meter,
        panel,
        screen,
    }
}

/// The display: the rack and the unit in it, its controls answering the pointer (the
/// lamps' are answered before, `face::lamp_edit`). The panel's switches as placed.
#[allow(clippy::too_many_arguments)]
pub fn show(
    host: &mut dyn FxHost,
    view: &View,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    now: &Reading,
    routed: bool,
    lamps: &[Option<face::Lamp>; 4],
) -> [Option<face::Switch>; 4] {
    let spec = host.spec().clone();
    let k = s.k;
    let panel = Panel::lay(ui.ctx(), s, &spec);
    let p = parts(r, k, panel.width);
    let panel = panel.place(p.panel);
    let press = face::edit(host, ui, s, &panel);
    // What the switches did this frame is drawn this frame.
    let spec = host.spec().clone();
    let ink = Ink::of(s);
    let keyed = now.keyed || routed;
    with_brush(&ui.painter_at(r), |b| {
        art::rack(b, s, r, RAIL * k);
        let plate = Rect::from_min_max(
            pos2(p.face.left() - EAR * k, p.face.top()),
            pos2(p.face.right() + EAR * k, p.face.bottom()),
        );
        art::faceplate(b, s, plate, EAR * k);
        face::strip(b, s, p.strip, &spec, now, lamps);
        face::bridge(b, s, &p, &spec, view, now);
        face::panel(b, s, &panel, &press, &spec);
        screen::show(b, s, &ink, p.screen, &spec, view, now, keyed);
    });
    panel.switches(k)
}
