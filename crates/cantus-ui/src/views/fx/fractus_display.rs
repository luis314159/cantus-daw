//! Fractus's display: the step lanes and the grain cloud, on a graticule dark in every theme.
//!
//! - **The lanes** (most of the width): ten rows, one per module, its name whole at the
//!   left; the cycle's steps across, beats and bars ruled; each painted block a bar over
//!   its steps — as tall as its amount, a dotted underline when it plays only by chance
//!   (its probability written); the playhead from the engine (`t::PHASE`); a lane that
//!   sounds now lights its name and its row (`t::ACTIVE`), one a held note plays is marked
//!   (`t::NOTES`).
//! - **The cloud** (the right): the grains sounding, each where it reads (now at the right,
//!   the cloud's reach at the left) and at its pitch (±24 semitones), from `t::GRAIN0`;
//!   their count, and the freeze's hold.
//! - **The keys** (under the cloud): the ten lanes' keys from the first lane's, lit while a
//!   note plays its lane.

use super::super::canvas::{self, Ink};
use super::super::frame::tr;
use super::super::host::FxHost;
use crate::kit::color::with_alpha;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::fx::fractus::{Pattern, LANES, LANE_KEYS};
use cantus_core::EffectSpec;
use cantus_dsp::fractus::t;
use egui::{pos2, vec2, Align2, Rect};

/// The lanes' names, in lane order.
pub const NAMES: [&str; LANES] = [
    "fx.fractus.lane.repeat",
    "fx.fractus.lane.reverse",
    "fx.fractus.lane.tape",
    "fx.fractus.lane.shuffle",
    "fx.fractus.lane.pitch",
    "fx.fractus.lane.crush",
    "fx.fractus.lane.filter",
    "fx.fractus.lane.gate",
    "fx.fractus.lane.throw",
    "fx.fractus.lane.cloud",
];

/// One block's numbers as the display reads them.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub seen: bool,
    pub phase: f32,
    pub active: u32,
    pub notes: u32,
    pub grains: f32,
    pub frozen: bool,
    /// Up to sixteen grains: (where it reads 0 now … 1 the reach, pitch −1 … 1).
    pub grain: [(f32, f32); t::SHOWN],
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        let mut grain = [(0.0, 0.0); t::SHOWN];
        let n = (tel[t::GRAINS].max(0.0) as usize).min(t::SHOWN);
        for (g, slot) in grain.iter_mut().enumerate().take(n) {
            *slot = (tel[t::GRAIN0 + 2 * g], tel[t::GRAIN0 + 2 * g + 1]);
        }
        Reading {
            seen: tel[0] > 0.0,
            phase: tel[t::PHASE],
            active: tel[t::ACTIVE].max(0.0) as u32,
            notes: tel[t::NOTES].max(0.0) as u32,
            grains: tel[t::GRAINS],
            frozen: tel[t::FROZEN] >= 0.5,
            grain,
        }
    }
}

/// Steps in the cycle the spec sets.
fn steps_of(spec: &EffectSpec) -> usize {
    let beats =
        cantus_dsp::fractus::STEP_BEATS[(spec.get("step").round().clamp(0.0, 6.0)) as usize];
    let per_bar = (4.0 / beats).round() as usize;
    (per_bar * spec.get("bars").round().clamp(1.0, 4.0) as usize).clamp(1, 256)
}

pub fn show(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, r: Rect, now: &Reading) {
    let spec = host.spec().clone();
    let k = s.k;
    let ink = Ink::of(s);
    let inner = r.shrink(10.0 * k);
    let side = (inner.width() * 0.26).clamp(240.0 * k, 330.0 * k);
    let lanes_r = Rect::from_min_max(inner.min, pos2(inner.max.x - side - 12.0 * k, inner.max.y));
    let right = Rect::from_min_max(pos2(lanes_r.max.x + 12.0 * k, inner.min.y), inner.max);
    let split = right.max.y - 78.0 * k;
    let cloud_r = Rect::from_min_max(right.min, pos2(right.max.x, split - 6.0 * k));
    let keys_r = Rect::from_min_max(pos2(right.min.x, split + 6.0 * k), right.max);
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, None);
        lanes(b, s, &ink, lanes_r, now, &spec);
        cloud(b, s, &ink, cloud_r, now, &spec);
        keys(b, s, &ink, keys_r, now, &spec);
        canvas::edge_b(b, r, &ink);
    });
}

fn pane(b: &mut Brush, ink: &Ink, r: Rect, title: &str, k: f32) -> Rect {
    b.rrect_stroke(r, 4.0 * k, 1.0 * k, with_alpha(ink.grid_strong, 0.8));
    let t = Text::key(title, Role::Micro, ink.dim);
    canvas::put(
        b,
        pos2(r.min.x + 8.0 * k, r.min.y + 6.0 * k),
        Align2::LEFT_TOP,
        &t,
        k,
        None,
    );
    Rect::from_min_max(
        pos2(r.min.x + 8.0 * k, r.min.y + 22.0 * k),
        r.max - vec2(8.0, 8.0) * k,
    )
}

/// The names' column, as wide as the longest name.
fn names_width(b: &Brush, k: f32) -> f32 {
    NAMES
        .iter()
        .map(|n| {
            crate::kit::text::measure(
                b.ctx(),
                &Text::key(n, Role::Micro, egui::Color32::TRANSPARENT),
                k,
            )
            .x
        })
        .fold(0.0, f32::max)
        + 14.0 * k
}

fn lanes(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, now: &Reading, spec: &EffectSpec) {
    let k = s.k;
    let area = pane(b, ink, r, "fractus.d.lanes", k);
    let names_w = names_width(b, k);
    let grid = Rect::from_min_max(
        pos2(area.min.x + names_w, area.min.y),
        area.max - vec2(0.0, 14.0 * k),
    );
    let steps = steps_of(spec);
    let per_beat = (steps / (4 * spec.get("bars").round().clamp(1.0, 4.0) as usize)).max(1);
    let sw = grid.width() / steps as f32;
    let rh = grid.height() / LANES as f32;
    let hot = ink.voice(ink.hue, false);
    let note_col = ink.voice(ink.hue, true);
    // Rules: every step faint, every beat stronger, every bar strongest.
    for i in 0..=steps {
        let x = grid.min.x + i as f32 * sw;
        let (w, a) = if i % (per_beat * 4) == 0 {
            (1.2, 0.95)
        } else if i % per_beat == 0 {
            (1.0, 0.7)
        } else {
            (1.0, 0.35)
        };
        b.vline(
            x,
            grid.min.y,
            grid.max.y,
            w * k,
            with_alpha(ink.grid_strong, a),
        );
    }
    let pattern: Pattern = spec.fractus.as_deref().cloned().unwrap_or_default();
    for (lane, key) in NAMES.iter().enumerate() {
        let y0 = grid.min.y + lane as f32 * rh;
        let row = Rect::from_min_max(pos2(grid.min.x, y0), pos2(grid.max.x, y0 + rh));
        b.hline(
            area.min.x,
            grid.max.x,
            row.max.y,
            1.0 * k,
            with_alpha(ink.grid, 0.6),
        );
        let lit = now.active & (1 << lane) != 0;
        let noted = now.notes & (1 << lane) != 0;
        if lit {
            b.rect(row, Fill::Solid(with_alpha(hot, 0.10)));
        }
        let col = if lit { hot } else { ink.text };
        canvas::put(
            b,
            pos2(area.min.x, row.center().y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, col),
            k,
            None,
        );
        if noted {
            b.circle(
                pos2(grid.min.x - 6.0 * k, row.center().y),
                3.0 * k,
                Fill::Solid(note_col),
            );
        }
        for blk in pattern.lane(lane) {
            let from = usize::from(blk.at);
            if from >= steps {
                continue;
            }
            let to = (from + usize::from(blk.len)).min(steps);
            let x0 = grid.min.x + from as f32 * sw + 1.5 * k;
            let x1 = grid.min.x + to as f32 * sw - 1.5 * k;
            let full = rh - 6.0 * k;
            let h = full * (0.35 + 0.65 * blk.amount);
            let bar = Rect::from_min_max(
                pos2(x0, row.max.y - 3.0 * k - h),
                pos2(x1, row.max.y - 3.0 * k),
            );
            b.rrect(
                bar,
                2.0 * k,
                Fill::Solid(with_alpha(hot, if lit { 0.95 } else { 0.7 })),
            );
            // The start: a bright edge.
            b.vline(x0 + 1.0 * k, bar.min.y, bar.max.y, 2.0 * k, ink.text);
            if blk.prob < 1.0 {
                canvas::dashed(
                    b,
                    pos2(x0, row.max.y - 1.5 * k),
                    pos2(x1, row.max.y - 1.5 * k),
                    1.0 * k,
                    ink.text,
                    2.0 * k,
                    2.0 * k,
                );
                if x1 - x0 > 34.0 * k {
                    canvas::putf(
                        b,
                        pos2(x1 - 2.0 * k, bar.min.y + 1.0 * k),
                        Align2::RIGHT_TOP,
                        Role::Micro,
                        ink.ground,
                        k,
                        None,
                        format_args!("{:.0} %", 100.0 * blk.prob),
                    );
                }
            }
        }
    }
    // The playhead.
    if now.seen {
        let x = grid.min.x + now.phase.clamp(0.0, 1.0) * grid.width();
        b.vline(
            x,
            grid.min.y - 4.0 * k,
            grid.max.y + 4.0 * k,
            1.6 * k,
            ink.text,
        );
        b.polygon(
            &[
                pos2(x - 4.0 * k, grid.min.y - 6.0 * k),
                pos2(x + 4.0 * k, grid.min.y - 6.0 * k),
                pos2(x, grid.min.y),
            ],
            ink.text,
        );
    }
    // The grid in words, under it.
    const STEPS: [&str; 7] = [
        "fx.fractus.step.0",
        "fx.fractus.step.1",
        "fx.fractus.step.2",
        "fx.fractus.step.3",
        "fx.fractus.step.4",
        "fx.fractus.step.5",
        "fx.fractus.step.6",
    ];
    let step_i = spec.get("step").round().clamp(0.0, 6.0) as usize;
    let step_word = tr(STEPS[step_i]);
    canvas::putf(
        b,
        pos2(grid.min.x, grid.max.y + 2.0 * k),
        Align2::LEFT_TOP,
        Role::Micro,
        ink.dim,
        k,
        None,
        format_args!(
            "{} {step_word} \u{b7} {} {:.0} \u{b7} {} {:.0} %",
            tr("fx.fractus.step"),
            tr("fx.fractus.bars"),
            spec.get("bars"),
            tr("fx.fractus.swing"),
            100.0 * spec.get("swing"),
        ),
    );
    let _ = LANE_KEYS;
}

fn cloud(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, now: &Reading, spec: &EffectSpec) {
    let k = s.k;
    let area = pane(b, ink, r, "fractus.d.cloud", k);
    let plot = Rect::from_min_max(
        area.min + vec2(30.0 * k, 0.0),
        area.max - vec2(0.0, 14.0 * k),
    );
    // Pitch rows: the unison and the octaves.
    for st in [-24i32, -12, 0, 12, 24] {
        let y = plot.center().y - st as f32 / 24.0 * plot.height() * 0.5;
        let a = if st == 0 { 0.9 } else { 0.5 };
        b.hline(
            plot.min.x,
            plot.max.x,
            y,
            1.0 * k,
            with_alpha(ink.grid_strong, a),
        );
        canvas::putf(
            b,
            pos2(plot.min.x - 4.0 * k, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{st:+}"),
        );
    }
    let grain = ink.voice(ink.hue, true);
    let level = spec.get("clevel");
    for (pos, pitch) in now
        .grain
        .iter()
        .take((now.grains.max(0.0) as usize).min(t::SHOWN))
    {
        let x = plot.max.x - pos.clamp(0.0, 1.0) * plot.width();
        let y = plot.center().y - pitch.clamp(-1.0, 1.0) * plot.height() * 0.5;
        b.circle(pos2(x, y), 6.0 * k, Fill::Solid(with_alpha(grain, 0.18)));
        b.circle(
            pos2(x, y),
            2.6 * k,
            Fill::Solid(with_alpha(grain, 0.6 + 0.4 * level.max(0.3))),
        );
    }
    canvas::put_key(
        b,
        pos2(plot.min.x, plot.max.y + 2.0 * k),
        Align2::LEFT_TOP,
        "fractus.d.past",
        Role::Micro,
        ink.dim,
        k,
    );
    canvas::put_key(
        b,
        pos2(plot.max.x, plot.max.y + 2.0 * k),
        Align2::RIGHT_TOP,
        "fractus.d.now",
        Role::Micro,
        ink.dim,
        k,
    );
    canvas::putf(
        b,
        pos2(area.max.x, area.min.y - 16.0 * k),
        Align2::RIGHT_TOP,
        Role::Micro,
        ink.text,
        k,
        None,
        format_args!("{} {:.0}", tr("fractus.d.grains"), now.grains.max(0.0)),
    );
    if now.frozen || spec.get("cfreeze") >= 0.5 {
        canvas::put(
            b,
            pos2(plot.center().x, plot.min.y + 2.0 * k),
            Align2::CENTER_TOP,
            &Text::key("fractus.d.frozen", Role::Micro, ink.text),
            k,
            Some(ink.plate),
        );
    }
}

fn keys(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, now: &Reading, spec: &EffectSpec) {
    let k = s.k;
    let area = pane(b, ink, r, "fractus.d.keys", k);
    let first = spec.get("key").round() as i32;
    let w = area.width() / LANES as f32;
    let hot = ink.voice(ink.hue, true);
    for lane in 0..LANES {
        let note = first + lane as i32;
        let black = matches!(note.rem_euclid(12), 1 | 3 | 6 | 8 | 10);
        let x0 = area.min.x + lane as f32 * w;
        let key = Rect::from_min_max(
            pos2(x0 + 1.0 * k, area.min.y),
            pos2(x0 + w - 1.0 * k, area.max.y - 12.0 * k),
        );
        let on = now.notes & (1 << lane) != 0;
        let col = if on {
            hot
        } else if black {
            with_alpha(ink.grid_strong, 0.9)
        } else {
            with_alpha(ink.text, 0.22)
        };
        b.rrect(key, 2.0 * k, Fill::Solid(col));
        let names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        canvas::putf(
            b,
            pos2(key.center().x, area.max.y),
            Align2::CENTER_BOTTOM,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!(
                "{}{}",
                names[note.rem_euclid(12) as usize],
                note.div_euclid(12) - 1
            ),
        );
    }
}
