//! Caelum's display (design sheet v2): an observatory at night, dark in every theme.
//!
//! - **The window** (the left, most of the width): the sky over the last eight seconds in
//!   a metal frame — the tail on the horizon over the tank's still water, the shimmer's
//!   octaves rising band by band above it (`caelum_sky.rs`).
//! - **The sill** under it: the endless controls as lit pushbuttons in their bezels —
//!   Infinite (its ∞ lamp), Freeze (its crystal lamp), and the two infinite choices
//!   (`caelum_desk.rs`).
//! - **The astrolabe** (top right): the two voices read on a pitch spiral
//!   (`caelum_astro.rs`).
//! - **The decay by octave** (bottom right): eight glass tubes in a rack, each lit to the
//!   decay the tank achieves at its octave, 63 Hz … 8 kHz, on 0.3 … 60 s
//!   (`caelum_desk.rs`).
//!
//! The metal follows the colourway (`caelum_art::metal`). Every number from the engine's
//! telemetry (`cantus_engine::fx::caelum::t`); how far the light has climbed is a model of
//! the tank's exchange fed by its measured energy (`View::climb`).

use super::super::canvas::{self, Ink};
use super::super::host::FxHost;
use super::{art, astro, desk, sky};
use crate::kit::color::with_alpha;
use crate::kit::{with_brush, Style};
use cantus_core::EffectSpec;
use cantus_engine::fx::caelum::t;
use egui::{pos2, vec2, Rect};

/// Readings kept: eight seconds at thirty a second.
pub const KEPT: usize = 240;

/// The bands of light: the tail (0), the first voice's four steps (1 … 4), the second's
/// (5 … 8).
pub const LAYERS: usize = 9;

/// One block's numbers as the display reads them.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// Any telemetry at all (an engine playing, or the demo).
    pub seen: bool,
    pub energy_db: f32,
    pub shimmer: f32,
    pub input: f32,
    pub duck_db: f32,
    pub freeze: f32,
    pub bloom: f32,
    pub predelay_ms: f32,
    pub line_ms: f32,
    pub achieved: [f32; 8],
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        let mut achieved = [0.0; 8];
        achieved.copy_from_slice(&tel[t::ACHIEVED..t::ACHIEVED + 8]);
        Reading {
            seen: tel[t::SAMPLE_RATE] > 0.0,
            energy_db: tel[t::ENERGY],
            shimmer: tel[t::SHIMMER].clamp(0.0, 1.0),
            input: tel[t::INPUT],
            duck_db: tel[t::DUCK],
            freeze: tel[t::FREEZE],
            bloom: tel[t::BLOOM],
            predelay_ms: tel[t::PREDELAY],
            line_ms: tel[t::LINE],
            achieved,
        }
    }
}

/// The display's memory: the readings of the last eight seconds, newest last, and how
/// bright each band of light is in each of them.
#[derive(Debug, Default)]
pub struct View {
    history: Vec<Reading>,
    /// Seconds since the last reading was kept.
    since: f32,
    /// The wisps' and the stars' phase, 0 … 1.
    pub phase: f32,
    /// Each reading's bands, 0 … 1 (`climb`).
    bright: Vec<[f32; LAYERS]>,
    /// Each band's brightest over the history.
    peak: [f32; LAYERS],
}

/// Energy in dB as a share of the sky's light, 0 … 1 (−90 dB nothing, −12 dB all).
pub fn light(db: f32) -> f32 {
    ((db + 90.0) / 78.0).clamp(0.0, 1.0)
}

/// Band `l`'s interval above the input (semitones) and whether it is the second voice's;
/// `None` when the band is not there (a voice at the unison, a silent second voice, no
/// shimmer).
pub fn layer_of(spec: &EffectSpec, l: usize) -> Option<(f32, bool)> {
    if l == 0 {
        return Some((0.0, false));
    }
    if spec.get("shim") <= 0.0 {
        return None;
    }
    let (iv, second) = if l <= 4 {
        (spec.get("pa"), false)
    } else {
        if spec.get("pblvl") <= 0.01 {
            return None;
        }
        (spec.get("pb"), true)
    };
    let step = ((l - 1) % 4 + 1) as f32;
    (iv.abs() >= 0.5).then_some((iv * step, second))
}

/// How fast a band's light climbs to the next at full shimmer, a second (drawn slower than
/// the tank's own pass, so each step reads).
const CLIMB: f32 = 1.4;

impl View {
    /// Keep a reading every thirtieth of a second.
    pub fn update(&mut self, r: &Reading, dt: f32) {
        if !r.seen {
            return;
        }
        self.since += dt;
        self.phase = (self.phase + dt * 0.35).fract();
        if self.since >= 1.0 / 30.0 || self.history.is_empty() {
            self.since = 0.0;
            if self.history.len() == KEPT {
                self.history.remove(0);
            }
            self.history.push(*r);
        }
    }

    /// Fill the history at once (the screenshots' demo).
    pub fn seed(&mut self, readings: &[Reading]) {
        let from = readings.len().saturating_sub(KEPT);
        self.history = readings[from..].to_vec();
        self.phase = 0.37;
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Band `l`'s brightness in reading `j`, 0 … 1.
    pub fn bright(&self, j: usize, l: usize) -> f32 {
        self.bright.get(j).map_or(0.0, |b| b[l])
    }

    /// Band `l`'s brightest over the history.
    pub fn peak(&self, l: usize) -> f32 {
        self.peak[l]
    }

    /// Light the bands over the history: what is new (the energy's rise beyond the decay
    /// the tank owes, while there is input) enters at the horizon; each step a share of every band climbs to the
    /// next (the shimmer's amount; the voices sharing it by the second's level; nothing
    /// climbs while frozen; the top band climbs out of sight); an infinite tank that lets
    /// the shimmer fade loses what has climbed. The shares light the bands by the tank's
    /// measured energy. (The telemetry's shimmer share is what the exchange moves in a
    /// block — a few per cent, and noisy — not how much has climbed, so it does not set
    /// them.)
    pub fn climb(&mut self, spec: &EffectSpec) {
        let n = self.history.len();
        self.bright.clear();
        self.bright.reserve(KEPT);
        self.peak = [0.0; LAYERS];
        let dt = 1.0 / 30.0;
        let inf = spec.get("inf") >= 0.5;
        let t60 = if inf {
            1.0e6
        } else {
            spec.get("decay").max(0.3)
        };
        let owed = cantus_dsp::math::db_to_gain(-60.0 * dt / t60).powi(2);
        let on = |l: usize| layer_of(spec, l).is_some();
        let lvl = spec.get("pblvl").clamp(0.0, 1.0);
        let wa = if on(1) { 1.0 - 0.5 * lvl } else { 0.0 };
        let wb = if on(5) { lvl } else { 0.0 };
        let wsum = wa + wb;
        let rate = spec.get("shim").clamp(0.0, 1.0) * CLIMB;
        let fades = inf && spec.get("ishim") < 0.5;
        let mut s = [0.0f32; LAYERS];
        s[0] = 1.0;
        let mut prev: Option<f32> = None;
        for j in 0..n {
            let rd = self.history[j];
            let e = cantus_dsp::math::db_to_gain(rd.energy_db.max(-150.0)).powi(2);
            let tau = if rd.freeze > 0.5 || wsum <= 0.0 {
                0.0
            } else {
                (rate * dt).min(0.5)
            };
            let mut nx = s;
            let out0 = s[0] * tau;
            nx[0] -= out0;
            if wsum > 0.0 {
                nx[1] += out0 * wa / wsum;
                nx[5] += out0 * wb / wsum;
            }
            for l in [1, 2, 3, 5, 6, 7] {
                let m = s[l] * tau;
                nx[l] -= m;
                nx[l + 1] += m;
            }
            for l in [4, 8] {
                nx[l] -= s[l] * tau * 0.6;
            }
            if fades {
                for v in nx.iter_mut().skip(1) {
                    *v *= 1.0 - 0.6 * dt;
                }
            }
            let sum: f32 = nx.iter().sum();
            if sum > 1e-9 {
                for v in nx.iter_mut() {
                    *v /= sum;
                }
            }
            // What is new enters at the horizon.
            // (Only while something comes in: a tail's own wobble is not new sound.)
            let fresh = match prev {
                _ if rd.input < 1e-3 && prev.is_some() => 0.0,
                Some(p) if e > 1e-15 => ((e - p * owed) / e).clamp(0.0, 1.0),
                _ => 1.0,
            };
            for v in nx.iter_mut() {
                *v *= 1.0 - fresh;
            }
            nx[0] += fresh;
            s = nx;
            prev = Some(e);
            let lit = light(rd.energy_db);
            let mut row = [0.0f32; LAYERS];
            for l in 0..LAYERS {
                if on(l) {
                    row[l] = (lit * cantus_dsp::math::powf(s[l].max(0.0), 0.65)).min(1.0);
                    self.peak[l] = self.peak[l].max(row[l]);
                }
            }
            self.bright.push(row);
        }
    }
}

/// Where the display's objects stand.
pub struct Parts {
    pub window: Rect,
    pub sill: Rect,
    pub astro: Rect,
    pub tubes: Rect,
}

pub fn parts(r: Rect, k: f32) -> Parts {
    let inner = r.shrink(12.0 * k);
    let side_w = (inner.width() * 0.27).clamp(290.0 * k, 360.0 * k);
    let side = Rect::from_min_max(pos2(inner.max.x - side_w, inner.min.y), inner.max);
    let left = Rect::from_min_max(inner.min, pos2(side.min.x - 18.0 * k, inner.max.y));
    let sill_h = 62.0 * k;
    let window = Rect::from_min_max(left.min, pos2(left.max.x, left.max.y - sill_h - 10.0 * k));
    let sill = Rect::from_min_max(pos2(left.min.x, left.max.y - sill_h), left.max);
    let split = side.min.y + side.height() * 0.57;
    Parts {
        window,
        sill,
        astro: Rect::from_min_max(side.min, pos2(side.max.x, split - 4.0 * k)),
        tubes: Rect::from_min_max(pos2(side.min.x, split + 4.0 * k), side.max),
    }
}

pub fn show(
    host: &mut dyn FxHost,
    view: &mut View,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    now: &Reading,
) {
    let k = s.k;
    let ink = Ink::of(s);
    let p = parts(r, k);
    let sill = desk::Sill::lay(ui.ctx(), s, p.sill);
    let press = desk::edit(host, ui, s, &sill);
    let spec = host.spec().clone();
    view.climb(&spec);
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, None);
        // The room's dark, a little light over the instruments.
        b.radial(
            r,
            p.window.center(),
            vec2(p.window.width() * 0.75, p.window.height() * 0.8),
            &[
                (0.0, with_alpha(ink.text, 0.035)),
                (1.0, with_alpha(ink.text, 0.0)),
            ],
            10,
        );
        let glass = art::frame(b, s, p.window, 15.0 * k);
        sky::show(b, s, &ink, p.window, glass, view, now, &spec);
        desk::sill(b, s, &ink, &sill, &press, &spec, now);
        astro::show(b, s, &ink, p.astro, &spec);
        desk::tubes(b, s, &ink, p.tubes, now, &spec);
        canvas::edge_b(b, r, &ink);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kv: &[(&str, f32)]) -> EffectSpec {
        let mut s = cantus_core::starting::starting_spec(cantus_core::EffectKind::Shimmer);
        for (k, v) in kv {
            let _ = s.set(k, *v);
        }
        s
    }

    fn reading(db: f32, shimmer: f32) -> Reading {
        Reading {
            seen: true,
            energy_db: db,
            shimmer,
            ..Reading::default()
        }
    }

    #[test]
    fn the_light_climbs_band_by_band_as_the_tail_ages() {
        let sp = spec(&[("shim", 0.6), ("pa", 12.0), ("pblvl", 0.0), ("decay", 8.0)]);
        let mut v = View::default();
        // A second of a held chord, then the tail left to ring for six.
        let mut rs = vec![reading(-30.0, 0.0); 30];
        rs.extend((0..180).map(|i| reading(-30.0 - i as f32 * 60.0 / 240.0, 0.0)));
        v.seed(&rs);
        v.climb(&sp);
        let n = v.len();
        // Early, the light is on the horizon; late, it has climbed past the first band.
        assert!(
            v.bright(20, 0) > v.bright(20, 2),
            "{} {}",
            v.bright(20, 0),
            v.bright(20, 2)
        );
        let a = v.bright(40, 2) / v.bright(40, 0);
        let z = v.bright(n - 1, 2) / v.bright(n - 1, 0);
        assert!(
            z > a,
            "the second band's share grows as the tail ages: {a} → {z}"
        );
        // The second voice is silent: its bands are not there.
        assert!(layer_of(&sp, 5).is_none() && v.peak(5) == 0.0);
    }

    #[test]
    fn nothing_climbs_without_shimmer_or_while_frozen() {
        let sp = spec(&[("shim", 0.0)]);
        assert!((1..LAYERS).all(|l| layer_of(&sp, l).is_none()));
        let sp = spec(&[("shim", 0.8), ("pa", 12.0)]);
        let mut v = View::default();
        let mut frozen = reading(-30.0, 0.0);
        frozen.freeze = 1.0;
        v.seed(&vec![frozen; 120]);
        v.climb(&sp);
        assert!((1..LAYERS).all(|l| v.peak(l) == 0.0));
    }

    #[test]
    fn the_bands_are_lit_by_the_measured_energy_and_dark_without_it() {
        let sp = spec(&[("shim", 0.5), ("pa", 12.0), ("pblvl", 0.0)]);
        let mut v = View::default();
        let mut rs = vec![reading(-24.0, 0.0); 60];
        rs.extend(vec![reading(-200.0, 0.0); 30]);
        v.seed(&rs);
        v.climb(&sp);
        let total = |j: usize| (0..LAYERS).map(|l| v.bright(j, l)).fold(0.0f32, f32::max);
        assert!(total(59) > 0.3 && total(59) <= 1.0);
        assert_eq!(total(v.len() - 1), 0.0);
    }
}
