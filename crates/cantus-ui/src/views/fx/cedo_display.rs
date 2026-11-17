//! Cedo's display (design sheet v1): the ducking engine on its bed beside its screen.
//!
//! - **The bed** in the colourway's finish, its corner bolts; along its top **the cast
//!   nameplate** and **the readout flags** — the trigger, the cycle (or the key's level in
//!   Sidechain), the reduction now, the latency — and **the pilot lamp** that flashes on
//!   each hit (the beat, the key, the note).
//! - **The engine** (`super::engine`): the cam on its flywheel or the solenoid, the walking
//!   beam, the bellows it squeezes and the level gauge.
//! - **The screen** in its riveted frame (`super::screen`).
//!
//! Every number from the engine's telemetry (`cantus_engine::fx::cedo::t`).

use super::super::canvas::Ink;
use super::engine::{self, Driver, Pose};
use super::screen::{self, Feed};
use super::view::{Hit, Reading, View};
use super::{plate, Split};
use crate::kit::color::with_alpha;
use crate::kit::{with_brush, Style};
use cantus_core::EffectSpec;
use cantus_dsp::lapsus::Curve;
use egui::{pos2, Rect};

/// The strip along the bed's top, at 100 %.
const STRIP: f32 = 58.0;

/// Where the display's parts stand.
pub struct Parts {
    pub bed: Rect,
    pub engine: Rect,
    pub screen: Rect,
}

/// The display's layout in `r`.
pub fn parts(r: Rect, k: f32) -> Parts {
    let top = r.top() + STRIP * k;
    let avail_h = r.bottom() - 14.0 * k - top;
    let ew = (avail_h * engine::W / engine::H).min(r.width() * 0.40);
    let engine = Rect::from_min_max(
        pos2(r.left() + 18.0 * k, top),
        pos2(r.left() + 18.0 * k + ew, r.bottom() - 14.0 * k),
    );
    let screen = Rect::from_min_max(
        pos2(engine.right() + 14.0 * k, top + 2.0 * k),
        pos2(r.right() - 20.0 * k, r.bottom() - 20.0 * k),
    );
    Parts {
        bed: r,
        engine,
        screen,
    }
}

/// How lit the pilot lamp is: a hit in the last few points, fading.
fn lamp_level(view: &View, trig: usize) -> f32 {
    let n = view.len();
    let mut best = 0.0f32;
    for (i, p) in view.points().enumerate() {
        let age = (n - 1 - i) as f32;
        if age > 9.0 {
            continue;
        }
        let hit = matches!((trig, p.hit), (0, Hit::Key) | (1 | 2, Hit::Cycle));
        if hit {
            best = best.max(1.0 - age / 10.0);
        }
    }
    best
}

/// The display in `r`.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &egui::Ui,
    s: &Style,
    r: Rect,
    spec: &EffectSpec,
    now: &Reading,
    view: &View,
    curve: &Curve,
    split: Option<&Split>,
) {
    let k = s.k;
    let p = parts(r, k);
    let ink = Ink::of(s);
    let trig = screen::trigger(spec);
    let amount_at = |u: f32| screen::amount_at(curve, u);
    // At rest the cam stands where the shape lets go.
    let rest = (0..64)
        .map(|i| i as f32 / 64.0)
        .fold((0.0f32, f32::INFINITY), |best, u| {
            let a = amount_at(u);
            if a < best.1 {
                (u, a)
            } else {
                best
            }
        })
        .0;
    let running = now.seen && now.phase >= 0.0;
    let lamp = if now.seen {
        lamp_level(view, trig)
    } else {
        0.0
    };
    let (driver, lift) = if trig == 0 {
        (
            Driver::Solenoid {
                lit: if now.seen && now.triggered { 1.0 } else { lamp },
            },
            if now.seen { now.amount } else { 0.0 },
        )
    } else {
        let phase = if running { now.phase } else { rest };
        (Driver::Cam { phase }, amount_at(phase))
    };
    let pose = Pose {
        driver,
        lift,
        shape: &amount_at,
        depth: spec.get("depth"),
        mix: spec.get("mix"),
    };
    with_brush(&ui.painter_at(r), |b| {
        plate::bed(b, s, p.bed);
        let name_h = 40.0 * k;
        let mid = r.top() + 31.0 * k;
        let name = plate::nameplate(b, s, pos2(r.left() + 30.0 * k, mid), name_h);
        flags(b, s, &ink, r, name.right(), mid, spec, now, trig, lamp);
        engine::draw(b, s, p.engine, &pose);
        let glass = plate::frame(b, s, p.screen);
        let feed = Feed {
            spec,
            curve,
            now,
            view,
            split: split.map(|x| x.dsp()),
        };
        screen::show(b, s, &ink, glass, &feed);
        plate::glass(b, glass, k);
    });
}

/// The readout flags and the lamp, right to left from the bed's right; a flag that would
/// reach the nameplate is left out.
#[allow(clippy::too_many_arguments)]
fn flags(
    b: &mut crate::kit::Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    min_x: f32,
    mid: f32,
    spec: &EffectSpec,
    now: &Reading,
    trig: usize,
    lamp: f32,
) {
    use super::super::frame::tr;
    let k = s.k;
    let light = ink.hue;
    let mut x = r.right() - 32.0 * k;
    // The lamp and its word.
    let lamp_word = tr(match trig {
        0 => "cedo.lamp.key",
        1 => "cedo.lamp.beat",
        _ => "cedo.lamp.note",
    });
    plate::lamp(b, s, pos2(x - 8.0 * k, mid), 7.0 * k, ink.look.trace2, lamp);
    let w = super::super::canvas::put(
        b,
        pos2(x - 24.0 * k, mid),
        egui::Align2::RIGHT_CENTER,
        &crate::kit::text::Text::new(
            lamp_word,
            crate::kit::text::Role::Micro,
            with_alpha(super::art::c(crate::kit::skin::engines::ENAMEL), 0.85),
        )
        .caps(true),
        k,
        None,
    );
    x = w.left() - 22.0 * k;
    let h = 24.0 * k;
    let sr = if now.seen { now.sample_rate } else { 48_000.0 };
    let latency = if now.seen {
        now.latency * 1_000.0 / sr.max(1.0)
    } else {
        cantus_dsp::cedo::latency_for(&cantus_engine::fx::cedo::settings(spec), sr) as f32 * 1_000.0
            / sr
    };
    let gr = if now.seen { now.gr } else { 0.0 };
    let mut values: [(String, String); 4] = Default::default();
    values[0] = (tr("cedo.f.latency").to_owned(), format!("{latency:.1} ms"));
    values[1] = (
        tr("cedo.f.reduction").to_owned(),
        if gr < 0.05 {
            "0.0 dB".to_owned()
        } else if gr >= cantus_dsp::cedo::DEPTH_INF - 0.1 {
            "\u{2212}\u{221e} dB".to_owned()
        } else {
            format!("\u{2212}{gr:.1} dB")
        },
    );
    values[2] = if trig == 0 {
        let v = if now.seen && now.keyed && now.key_db > -95.0 {
            format!("{:.1} dB", now.key_db)
        } else {
            tr("cedo.f.no_key").to_owned()
        };
        (tr("cedo.f.key").to_owned(), v)
    } else {
        let div = spec.get("div").round().clamp(0.0, 16.0) as usize;
        (
            tr("fx.cedo.div").to_owned(),
            crate::i18n::t(&format!("modfx.div.{div}")),
        )
    };
    values[3] = (
        tr("fx.cedo.trig").to_owned(),
        tr(match trig {
            0 => "fx.cedo.trig.0",
            1 => "fx.cedo.trig.1",
            _ => "fx.cedo.trig.2",
        })
        .to_owned(),
    );
    for (name, value) in &values {
        let w = plate::flag_width(b, k, name, value);
        if x - w < min_x + 20.0 * k {
            break;
        }
        let f = plate::flag(b, s, pos2(x, mid), name, value, light, h);
        x = f.left() - 20.0 * k;
    }
}
