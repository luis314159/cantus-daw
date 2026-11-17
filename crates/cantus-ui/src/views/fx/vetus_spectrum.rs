//! Vetus's output (ADR 0130): beside the record, **what comes out** — the
//! output's spectrum over the input's, with the three limits an old medium sets drawn on it
//! and dragged there:
//!
//! - the **converter's Nyquist** (half the new sample rate): what lies past it folds back
//!   when the hold is raw (hatched), or is filtered away (dimmed) — dragged across;
//! - the **quantiser's noise floor** (about −6 dB a bit): dragged up and down for the bits;
//! - the **medium's band**: the tone module's response, its two edges dragged across.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::FxHost;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::vetus::digital::RateFilter;
use cantus_dsp::vetus::VetusSettings;
use cantus_dsp::SpectrumAnalyser;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

const BINS: usize = 96;
const LO: f32 = 20.0;
const HI: f32 = 20_000.0;
const TOP: f32 = 6.0;
const BOT: f32 = -108.0;
/// The tone's response: 0 dB at this level of the plot, 12 dB over a sixth of it.
const RESP_AT: f32 = -12.0;
const FALL: f32 = 1.0;

/// What is being dragged.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Nyquist,
    Floor,
    Lo,
    Hi,
}

/// The spectrum's memory.
pub struct View {
    analyser: Option<SpectrumAnalyser>,
    analysed: Option<(usize, u64)>,
    scratch: Vec<f32>,
    pub spec_in: [f32; BINS],
    pub spec_out: [f32; BINS],
    grab: Option<Grab>,
}

impl Default for View {
    fn default() -> Self {
        View {
            analyser: None,
            analysed: None,
            scratch: Vec::new(),
            spec_in: [BOT; BINS],
            spec_out: [BOT; BINS],
            grab: None,
        }
    }
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View").field("grab", &self.grab).finish()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Map {
    pub plot: Rect,
}

impl Map {
    pub fn x(&self, hz: f32) -> f32 {
        let u = (hz.clamp(LO, HI) / LO).ln() / (HI / LO).ln();
        self.plot.left() + self.plot.width() * u
    }
    pub fn hz(&self, x: f32) -> f32 {
        let u = ((x - self.plot.left()) / self.plot.width()).clamp(0.0, 1.0);
        LO * (HI / LO).powf(u)
    }
    pub fn y(&self, db: f32) -> f32 {
        let v = (TOP - db) / (TOP - BOT);
        self.plot.top() + self.plot.height() * v.clamp(0.0, 1.0)
    }
    pub fn db(&self, y: f32) -> f32 {
        TOP - (y - self.plot.top()) / self.plot.height() * (TOP - BOT)
    }
    /// A gain of the tone's response, drawn on its own scale.
    fn yr(&self, db: f32) -> f32 {
        self.y(RESP_AT + db.clamp(-60.0, 18.0))
    }
}

fn bin_hz(i: usize) -> f32 {
    LO * (HI / LO).powf((i as f32 + 0.5) / BINS as f32)
}

/// The quantiser's noise floor for `bits`, dBFS (a step's uniform error, `q/√12`).
pub fn floor_db(bits: f32) -> f32 {
    -6.02 * bits - 4.77
}

fn bits_of_floor(db: f32) -> f32 {
    ((-db - 4.77) / 6.02).clamp(1.0, 24.0)
}

/// The spectrum of what the analyser tap brought, falling slowly.
pub fn update(v: &mut View, pairs: &std::collections::VecDeque<(f32, f32)>, sr: f32) {
    let a = v
        .analyser
        .get_or_insert_with(|| SpectrumAnalyser::new(4096));
    let n = a.size();
    let sig = (
        pairs.len(),
        pairs.back().map_or(0, |p| {
            (u64::from(p.0.to_bits()) << 32) | u64::from(p.1.to_bits())
        }),
    );
    if pairs.len() < n || v.analysed == Some(sig) {
        return;
    }
    v.analysed = Some(sig);
    v.scratch.clear();
    for (x, y) in pairs.iter().skip(pairs.len() - n) {
        v.scratch.push(*x);
        v.scratch.push(*y);
    }
    a.push(&v.scratch);
    a.compute();
    for i in 0..BINS {
        let hz = bin_hz(i);
        let tilt = 1.5 * (hz / 1_000.0).log2();
        let fin = a.level_db(0, sr, hz, 1.0 / 6.0) + tilt;
        let fout = a.level_db(1, sr, hz, 1.0 / 6.0) + tilt;
        v.spec_in[i] = fin.max(v.spec_in[i] - FALL).max(BOT);
        v.spec_out[i] = fout.max(v.spec_out[i] - FALL).max(BOT);
    }
}

/// The plot inside the panel.
pub fn map_of(r: Rect, k: f32) -> Map {
    Map {
        plot: Rect::from_min_max(
            pos2(r.left() + 34.0 * k, r.top() + 26.0 * k),
            pos2(r.right() - 14.0 * k, r.bottom() - 20.0 * k),
        ),
    }
}

/// The handles' places: the Nyquist's top, the floor's right end, the band's edges.
fn handles(map: Map, s: &VetusSettings, sr: f32) -> [(Grab, Pos2, bool); 4] {
    let ny = map.x(0.5 * s.rate_hz.min(sr));
    let fl = map.y(floor_db(s.bits));
    let lo = pos2(map.x(s.lo), map.yr(tone_db(s, s.lo, sr)));
    let hi = pos2(map.x(s.hi), map.yr(tone_db(s, s.hi, sr)));
    [
        (Grab::Nyquist, pos2(ny, map.plot.top() + 6.0), s.rate_on),
        (Grab::Floor, pos2(map.plot.right() - 10.0, fl), s.bits_on),
        (Grab::Lo, lo, s.tone_on),
        (Grab::Hi, hi, s.tone_on),
    ]
}

/// The tone module's response at `hz`, dB.
pub fn tone_db(s: &VetusSettings, hz: f32, sr: f32) -> f32 {
    cantus_dsp::vetus::tone_db(s, hz, sr)
}

/// The drags: across for the Nyquist and the edges, up and down for the floor.
pub fn edit(host: &mut dyn FxHost, v: &mut View, ui: &egui::Ui, map: Map, k: f32) {
    let sr = host.sample_rate();
    let set = cantus_engine::fx::vetus::settings(host.spec());
    let resp = ui.interact(
        map.plot.expand(6.0 * k),
        ui.id().with("vetus-spectrum"),
        Sense::drag(),
    );
    let near = |p: Pos2| -> Option<Grab> {
        let mut best: Option<(Grab, f32)> = None;
        for (g, at, on) in handles(map, &set, sr) {
            if !on {
                continue;
            }
            // The Nyquist and the floor are lines: grabbed anywhere along them.
            let d = match g {
                Grab::Nyquist => (p.x - at.x).abs(),
                Grab::Floor => (p.y - at.y).abs(),
                _ => (p - at).length(),
            };
            if d < 9.0 * k && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((g, d));
            }
        }
        best.map(|(g, _)| g)
    };
    if resp.drag_started() {
        v.grab = resp.interact_pointer_pos().and_then(near);
    }
    if let (Some(g), Some(p)) = (v.grab, resp.interact_pointer_pos()) {
        if resp.dragged() {
            match g {
                Grab::Nyquist => {
                    let hz = (2.0 * map.hz(p.x)).clamp(1_000.0, 48_000.0);
                    host.set("rate.hz", hz, Some("rate.hz"));
                }
                Grab::Floor => {
                    host.set("bits.depth", bits_of_floor(map.db(p.y)), Some("bits.depth"));
                }
                Grab::Lo => {
                    let hz = map.hz(p.x).clamp(20.0, (set.hi * 0.8).min(2_000.0));
                    host.set("tone.lo", hz, Some("tone.lo"));
                }
                Grab::Hi => {
                    let hz = map.hz(p.x).clamp((set.lo * 1.25).max(500.0), 20_000.0);
                    host.set("tone.hi", hz, Some("tone.hi"));
                }
            }
        }
    }
    if resp.drag_stopped() && v.grab.take().is_some() {
        host.end_gesture();
    }
    if let Some(p) = resp.hover_pos() {
        let (tip, how) = match v.grab.or_else(|| near(p)) {
            Some(Grab::Nyquist) => ("vetus.spec.nyquist_tip", "vetus.spec.nyquist_how"),
            Some(Grab::Floor) => ("vetus.spec.floor_tip", "vetus.spec.floor_how"),
            Some(Grab::Lo | Grab::Hi) => ("vetus.spec.band_tip", "vetus.spec.band_how"),
            None => ("vetus.spec_tip", "vetus.spec_how"),
        };
        hint::set_text(ui.ctx(), frame::tr(tip), frame::tr(how), Caps::NONE);
    }
}

/// The panel: its plate, the spectra, the limits and their words.
pub fn paint(b: &mut Brush, s: &Style, r: Rect, v: &View, set: &VetusSettings, sr: f32, k: f32) {
    let ink = Ink::of(s);
    let map = map_of(r, k);
    let plot = map.plot;
    let e = set.aged();
    let second = ink.voice(s.hue, true);
    // The grid: decades and every 24 dB, their numbers at the edges.
    for hz in [100.0f32, 1_000.0, 10_000.0] {
        b.vline(
            map.x(hz),
            plot.top(),
            plot.bottom(),
            0.8 * k,
            ink.grid_strong,
        );
    }
    for hz in [50.0f32, 200.0, 500.0, 2_000.0, 5_000.0] {
        b.vline(map.x(hz), plot.top(), plot.bottom(), 0.6 * k, ink.grid);
    }
    let mut db = 0.0;
    while db >= BOT {
        b.hline(plot.left(), plot.right(), map.y(db), 0.6 * k, ink.grid);
        let w = if db == 0.0 {
            "0".to_string()
        } else {
            format!("\u{2212}{}", -db as i32)
        };
        let t = Text::new(&w, Role::Micro, ink.dim).caps(false);
        canvas::put(
            b,
            pos2(plot.left() - 5.0 * k, map.y(db)),
            Align2::RIGHT_CENTER,
            &t,
            k,
            None,
        );
        db -= 24.0;
    }
    for (hz, w) in [(100.0f32, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        let t = Text::new(w, Role::Micro, ink.dim).caps(false);
        canvas::put(
            b,
            pos2(map.x(hz), plot.bottom() + 3.0 * k),
            Align2::CENTER_TOP,
            &t,
            k,
            None,
        );
    }
    // Past the Nyquist: hatched where it folds back, dimmed where it is filtered.
    if e.rate_on {
        let x = map.x(0.5 * e.rate_hz.min(sr));
        if x < plot.right() {
            let past = Rect::from_min_max(pos2(x, plot.top()), plot.right_bottom());
            if e.rate_filter == RateFilter::None {
                canvas::hatch(b, past, 6.0 * k, 0.6 * k, with_alpha(ink.danger, 0.28));
            } else {
                b.rect(past, Fill::Solid(with_alpha(ink.ground, 0.55)));
            }
        }
    }
    // The input, faint, and the output filled.
    let line = |spec: &[f32; BINS]| -> Vec<Pos2> {
        (0..BINS)
            .map(|i| pos2(map.x(bin_hz(i)), map.y(spec[i])))
            .collect()
    };
    let has = v.spec_out.iter().any(|d| *d > BOT + 1.0);
    if has {
        let input = line(&v.spec_in);
        let output = line(&v.spec_out);
        b.polyline(&input, 0.8 * k, with_alpha(ink.text, 0.30));
        b.area(&output, plot.bottom(), ink.fill(0.5));
        canvas::trace(b, &output, 1.2 * k, ink.hue, &ink);
    }
    // The medium's band: its response on its own scale, the edges' rings.
    if e.tone_on {
        let pts: Vec<Pos2> = (0..=160)
            .map(|j| {
                let hz = LO * (HI / LO).powf(j as f32 / 160.0);
                pos2(map.x(hz), map.yr(tone_db(&e, hz, sr)))
            })
            .collect();
        b.polyline(&pts, 1.0 * k, with_alpha(second, 0.9));
        for hz in [e.lo, e.hi] {
            let p = pos2(map.x(hz), map.yr(tone_db(&e, hz, sr)));
            b.circle_stroke(p, 4.5 * k, 1.1 * k, second);
            b.dot(p, 1.6 * k, second);
        }
        let mut w = String::new();
        w.push_str(frame::tr(super::MEDIUM_KEYS[e.medium as usize]));
        w.push_str("  ");
        super::super::controls::write_readout(e.lo, "Hz", &mut w);
        w.push_str(" \u{2013} ");
        super::super::controls::write_readout(e.hi, "Hz", &mut w);
        let t = Text::new(&w, Role::Micro, second).caps(false);
        let mid = map.x((e.lo * e.hi).sqrt());
        canvas::put(
            b,
            pos2(mid, map.yr(0.0) - 9.0 * k),
            Align2::CENTER_BOTTOM,
            &t,
            k,
            Some(ink.plate),
        );
    }
    // The quantiser's floor.
    if e.bits_on {
        let y = map.y(floor_db(e.bits));
        if y < plot.bottom() {
            canvas::dashed(
                b,
                pos2(plot.left(), y),
                pos2(plot.right(), y),
                1.0 * k,
                ink.hue,
                5.0 * k,
                3.0 * k,
            );
            let mut w = String::new();
            super::bits_pill(e.bits, &mut w);
            w.push_str("  ");
            super::super::controls::write_readout(floor_db(e.bits), "dB", &mut w);
            let t = Text::new(&w, Role::Micro, ink.text).caps(false);
            canvas::put(
                b,
                pos2(plot.right() - 4.0 * k, y - 3.0 * k),
                Align2::RIGHT_BOTTOM,
                &t,
                k,
                Some(ink.plate),
            );
        }
    }
    // The converter's Nyquist.
    if e.rate_on {
        let x = map.x(0.5 * e.rate_hz.min(sr));
        b.vline(x, plot.top(), plot.bottom(), 1.0 * k, ink.danger);
        b.dot(pos2(x, plot.top() + 6.0 * k), 2.4 * k, ink.danger);
        let mut w = String::from(frame::tr("vetus.spec.nyquist"));
        w.push(' ');
        super::super::controls::write_readout(0.5 * e.rate_hz, "Hz", &mut w);
        let t = Text::new(&w, Role::Micro, ink.text).caps(false);
        let left = x > plot.center().x;
        let at = pos2(x + if left { -6.0 } else { 6.0 } * k, plot.top() + 6.0 * k);
        let align = if left {
            Align2::RIGHT_CENTER
        } else {
            Align2::LEFT_CENTER
        };
        canvas::put(b, at, align, &t, k, Some(ink.plate));
    }
    let title = Text::key("vetus.spec.title", Role::Micro, ink.dim).caps(true);
    canvas::put(
        b,
        pos2(r.left() + 8.0 * k, r.top() + 6.0 * k),
        Align2::LEFT_TOP,
        &title,
        k,
        None,
    );
    let _ = vec2(0.0, 0.0);
}
