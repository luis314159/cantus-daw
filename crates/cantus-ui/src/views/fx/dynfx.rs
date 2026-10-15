//! What the dynamics family's editors share (ADR 0056): Tonsor, Impetus, Discrimen.
//!
//! - [`Rates`] turns the running counts a unit's telemetry carries (samples seen, samples
//!   clipped, overs…) into rates over the last second **of audio** — never the last block
//!   only, so nothing between two frames is lost (the family's review: meters fold
//!   everything that came since the last frame).
//! - [`History`] keeps one value per new telemetry block for a strip graph (the last few
//!   seconds of gain reduction), and paints it.
//! - [`stat`] paints a label and its value on one line of a meter card, and [`bar`] a
//!   level bar under it.

use super::canvas::{self, Ink};
use crate::kit::text::{Role, Text};
use crate::kit::Brush;
use egui::{pos2, Align2, Color32, Pos2, Rect};
use std::collections::VecDeque;

/// Counters wrap here (the DSP's `2²⁴`, where `f32` still counts by one).
const WRAP: f64 = 16_777_216.0;

/// Rates from running counters over the last second of audio.
#[derive(Clone, Debug, Default)]
pub struct Rates {
    /// (samples, counts…) as the telemetry carried them, one entry per new block.
    seen: VecDeque<(f64, [f64; 3])>,
    /// Samples counted in all, unwrapped.
    total: f64,
    last: Option<f64>,
}

impl Rates {
    /// A new reading: `samples` and up to three counts, as the telemetry carries them
    /// (each wrapping at 2²⁴). The same reading twice is ignored.
    pub fn push(&mut self, samples: f32, counts: [f32; 3]) {
        let s = f64::from(samples);
        if self.last == Some(s) {
            return;
        }
        let step = match self.last {
            Some(prev) => (s - prev).rem_euclid(WRAP),
            None => 0.0,
        };
        self.last = Some(s);
        self.total += step;
        self.seen.push_back((self.total, counts.map(f64::from)));
        while self.seen.len() > 512 {
            self.seen.pop_front();
        }
    }

    /// Over the last `window` samples (or what is known of them): the share of samples
    /// each count is of them, and the counts per second at `sample_rate`.
    pub fn over(&self, window: f64, sample_rate: f32) -> [(f32, f32); 3] {
        let mut out = [(0.0, 0.0); 3];
        let (Some(&(now, c_now)), Some(_)) = (self.seen.back(), self.seen.front()) else {
            return out;
        };
        let then = self
            .seen
            .iter()
            .find(|(s, _)| now - *s <= window)
            .copied()
            .unwrap_or((now, c_now));
        let span = now - then.0;
        if span <= 0.0 {
            return out;
        }
        for (i, o) in out.iter_mut().enumerate() {
            let d = (c_now[i] - then.1[i]).rem_euclid(WRAP);
            *o = (
                (d / span) as f32,
                (d / span * f64::from(sample_rate)) as f32,
            );
        }
        out
    }

    pub fn clear(&mut self) {
        *self = Rates::default();
    }
}

/// The last values of a reading, one per new telemetry block.
#[derive(Clone, Debug)]
pub struct History {
    values: VecDeque<f32>,
    cap: usize,
    last_block: f32,
}

impl Default for History {
    fn default() -> Self {
        History::new(300)
    }
}

impl History {
    pub fn new(cap: usize) -> History {
        History {
            values: VecDeque::with_capacity(cap),
            cap,
            last_block: -1.0,
        }
    }

    /// `value` for telemetry block `block` (a repeated block is ignored).
    pub fn push(&mut self, block: f32, value: f32) {
        if block == self.last_block {
            return;
        }
        self.last_block = block;
        if self.values.len() == self.cap {
            self.values.pop_front();
        }
        self.values
            .push_back(if value.is_finite() { value } else { 0.0 });
    }

    pub fn values(&self) -> impl Iterator<Item = f32> + '_ {
        self.values.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The history as bars hanging from the top of `r`, the newest at the right; `range`
    /// is the value at the bottom.
    pub fn paint_down(&self, b: &mut Brush, r: Rect, range: f32, color: Color32, k: f32) {
        let n = self.cap.max(1);
        let w = r.width() / n as f32;
        let skip = n - self.values.len().min(n);
        for (i, v) in self.values.iter().enumerate() {
            if *v <= 0.0 {
                continue;
            }
            let x = r.left() + (skip + i) as f32 * w;
            let h = (v / range).clamp(0.0, 1.0) * r.height();
            b.rect(
                Rect::from_min_max(pos2(x, r.top()), pos2(x + w.max(k), r.top() + h)),
                crate::kit::Fill::Solid(color),
            );
        }
    }
}

/// A label at the left of `r` and a value at its right, on one line.
pub fn stat(
    b: &mut Brush,
    r: Rect,
    word: &str,
    color: Color32,
    ink: &Ink,
    k: f32,
    value: std::fmt::Arguments<'_>,
) {
    let t = Text::new(word, Role::Micro, ink.dim).caps(true);
    canvas::put(
        b,
        pos2(r.left(), r.center().y),
        Align2::LEFT_CENTER,
        &t,
        k,
        None,
    );
    canvas::putf(
        b,
        pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        Role::Value,
        color,
        k,
        None,
        value,
    );
}

/// A level bar: `frac` (0 … 1) of `r` filled from the left in `color` over a faint track.
pub fn bar(b: &mut Brush, r: Rect, frac: f32, color: Color32, ink: &Ink, k: f32) {
    b.rrect(r, 2.0 * k, crate::kit::Fill::Solid(ink.grid));
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        let fill = Rect::from_min_max(r.min, Pos2::new(r.left() + r.width() * f, r.bottom()));
        b.rrect(fill, 2.0 * k, crate::kit::Fill::Solid(color));
    }
}

/// A unit's columns (`cantus_dsp::dynfx_meter`) as the editor keeps them: every column the
/// audio made, in order, whatever the frame rate — appended from the telemetry each frame.
#[derive(Clone, Debug)]
pub struct ColumnHistory {
    cols: VecDeque<[f32; 3]>,
    cap: usize,
    last: Option<u32>,
}

impl Default for ColumnHistory {
    fn default() -> Self {
        ColumnHistory::new(320)
    }
}

impl ColumnHistory {
    pub fn new(cap: usize) -> ColumnHistory {
        ColumnHistory {
            cols: VecDeque::with_capacity(cap),
            cap,
            last: None,
        }
    }

    /// Take the columns of `tel` not seen yet.
    pub fn update(&mut self, tel: &[f32; 64]) {
        let cols = &mut self.cols;
        let cap = self.cap;
        self.last = Some(cantus_dsp::dynfx_meter::new_columns(tel, self.last, |c| {
            if cols.len() == cap {
                cols.pop_front();
            }
            cols.push_back(c);
        }));
    }

    /// The columns, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &[f32; 3]> + '_ {
        self.cols.iter()
    }

    pub fn len(&self) -> usize {
        self.cols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cols.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// A signed share as a percentage with its sign: `+80%`, `−35%`, `0%`.
pub fn signed_pct(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let p = (v * 100.0).round();
    if p > 0.0 {
        let _ = write!(b, "+{p:.0}%");
    } else if p < 0.0 {
        let _ = write!(b, "\u{2212}{:.0}%", -p);
    } else {
        b.push_str("0%");
    }
}

/// [`controls::param_with`] under a short label of its own (a part's *Gain* inside the
/// *Transient* card, where the parameter's full name would repeat the card's).
pub fn param_labelled(
    ui: &mut egui::Ui,
    s: &crate::kit::Style,
    host: &mut dyn super::host::FxHost,
    key: &'static str,
    size: super::controls::Size,
    format: Option<super::controls::Pill<'_>>,
    label: &'static str,
) -> egui::Response {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
    };
    let mut c = super::controls::Cell::param(info, spec.get(key))
        .size(size)
        .label(crate::kit::text::Src::Key(label))
        .name(super::frame::tr(info.label_key));
    if let Some(f) = format {
        c = c.format(f);
    }
    let out = c.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
    out.resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_count_over_the_last_second_of_audio_across_a_wrap() {
        let mut r = Rates::default();
        // 48 000 samples a second in blocks of 4 800, a clip every 100 samples.
        let mut s = 16_777_216.0f64 - 24_000.0;
        let mut c = 16_777_216.0f64 - 100.0;
        for _ in 0..20 {
            r.push(
                s.rem_euclid(WRAP) as f32,
                [c.rem_euclid(WRAP) as f32, 0.0, 0.0],
            );
            s += 4_800.0;
            c += 48.0;
        }
        let [(share, per_s), (none, _), _] = r.over(48_000.0, 48_000.0);
        assert!((share - 0.01).abs() < 1e-6, "{share}");
        assert!((per_s - 480.0).abs() < 1e-3, "{per_s}");
        assert_eq!(none, 0.0);
    }

    #[test]
    fn a_reading_seen_twice_counts_once() {
        let mut r = Rates::default();
        r.push(0.0, [0.0; 3]);
        r.push(100.0, [10.0, 0.0, 0.0]);
        r.push(100.0, [10.0, 0.0, 0.0]);
        let [(share, _), _, _] = r.over(1e9, 48_000.0);
        assert!((share - 0.1).abs() < 1e-6);
    }

    #[test]
    fn a_history_keeps_its_last_values_and_drops_nonsense() {
        let mut h = History::new(3);
        for (b, v) in [
            (1.0, 1.0),
            (2.0, f32::NAN),
            (2.0, 5.0),
            (3.0, 3.0),
            (4.0, 4.0),
        ] {
            h.push(b, v);
        }
        assert_eq!(h.values().collect::<Vec<_>>(), vec![0.0, 3.0, 4.0]);
    }
}
