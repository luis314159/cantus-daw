//! CPU, RAM, voices and dropouts (ADR 0052), moved here from the status bar: the audio
//! callback's load (its histogram in the tooltip), the process's memory in use (the
//! working set, read through `cantus_rt::usage` about once a second, off the audio thread),
//! the voices sounding, and the dropouts counted (the latest one's blame in the tooltip).
//! Without an audio device the engine's numbers are a dash. The texts are rebuilt only
//! when a number changes.

use super::Style;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{light, with_brush, Fill};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// The RAM reading and when it was taken.
#[derive(Debug, Default)]
pub struct Ram {
    at: Option<Instant>,
    bytes: Option<u64>,
    /// The block of the last dropout said on the status line.
    dropout: Option<u64>,
}

impl Ram {
    /// Read again when a second has passed (or never was).
    pub fn tick(&mut self) {
        if self
            .at
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(1))
        {
            self.at = Some(Instant::now());
            self.bytes = cantus_rt::usage::working_set_bytes();
        }
    }

    /// The last reading, bytes (`None` where the system cannot say).
    pub fn bytes(&self) -> Option<u64> {
        self.bytes
    }
}

/// "690 MB", "1.3 GB", or a dash — never wider than "999 MB".
/// The memory rounded to megabytes: what the readout shows, and what it is rebuilt on.
pub fn ram_mb(bytes: u64) -> u64 {
    (bytes + (1 << 19)) >> 20
}

pub fn ram_text(bytes: Option<u64>, out: &mut String) {
    out.clear();
    match bytes {
        // Decided on the rounded value: 999.5 MB and up reads in gigabytes.
        Some(b) if ram_mb(b) >= 1000 => {
            let gb = b as f64 / (1u64 << 30) as f64;
            if gb < 99.95 {
                let _ = write!(out, "{gb:.1} GB");
            } else {
                let _ = write!(out, "{gb:.0} GB");
            }
        }
        Some(b) => {
            let _ = write!(out, "{} MB", ram_mb(b));
        }
        None => out.push('\u{2014}'),
    }
}

const WORDS: [&str; 4] = [
    "topbar.cpu",
    "topbar.ram",
    "topbar.voices",
    "topbar.dropouts",
];
const WIDEST: [&str; 4] = ["100 %", "999 MB", "999+", "999+"];

#[derive(Copy, Clone, Debug, PartialEq)]
struct Key {
    live: bool,
    load: u32,
    ram: Option<u64>,
    voices: usize,
    overruns: u64,
}

thread_local! {
    static CACHE: RefCell<(Option<Key>, [String; 4])> = RefCell::new((None, Default::default()));
}

fn columns(ctx: &egui::Context, s: &Style) -> (f32, f32) {
    let tk = s.tk();
    let label = WORDS
        .iter()
        .map(|k| text::measure(ctx, &Text::key(k, Role::Micro, s.t.text).caps(true), tk).x)
        .fold(0.0, f32::max);
    let value = WIDEST
        .iter()
        .map(|v| text::measure(ctx, &Text::new(v, Role::Value, s.t.text), tk).x)
        .fold(0.0, f32::max);
    (label + 6.0 * s.k, value + 6.0 * s.k)
}

pub fn width(ctx: &egui::Context, s: &Style) -> f32 {
    let (l, v) = columns(ctx, s);
    2.0 * (l + v) + 22.0 * s.k
}

/// Every frame, whether the group is on the bar or folded: the RAM reading, and a new
/// dropout said once on the status line with what took the time (round 20's blame) — the
/// count stays in the group.
pub fn watch(app: &mut CantusApp) {
    app.topbar.ram.tick();
    let stats = app.engine.stats();
    let Some(d) = stats.last_dropout else {
        return;
    };
    if app.topbar.ram.dropout != Some(d.block) {
        app.topbar.ram.dropout = Some(d.block);
        let text = crate::perf_status::dropout_text(app.project(), &d);
        app.set_error(text);
    }
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let stats = *app.engine.stats();
    let live = app.engine.is_live();
    let key = Key {
        live,
        load: (stats.load * 100.0).round() as u32,
        ram: app.topbar.ram.bytes().map(ram_mb),
        voices: stats.active_voices,
        overruns: stats.overruns,
    };
    let well = Rect::from_center_size(r.center(), Vec2::new(r.width(), 40.0 * k));
    let (lw, vw) = columns(ui.ctx(), s);
    let cell = |i: usize| {
        let col = (i / 2) as f32;
        let row = (i % 2) as f32;
        let h = (well.height() - 6.0 * k) * 0.5;
        Rect::from_min_size(
            Pos2::new(
                well.left() + 7.0 * k + col * (lw + vw + 8.0 * k),
                well.top() + 3.0 * k + row * h,
            ),
            Vec2::new(lw + vw, h),
        )
    };
    let warn = live && stats.overruns > 0;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let (k_old, texts) = &mut *c;
        if *k_old != Some(key) {
            *k_old = Some(key);
            for t in texts.iter_mut() {
                t.clear();
            }
            let dash = '\u{2014}';
            if live {
                let _ = write!(texts[0], "{} %", key.load);
                count(&mut texts[2], key.voices as u64);
                count(&mut texts[3], key.overruns);
            } else {
                texts[0].push(dash);
                texts[2].push(dash);
                texts[3].push(dash);
            }
            ram_text(app.topbar.ram.bytes(), &mut texts[1]);
        }
        with_brush(ui.painter(), |b| {
            light::sunken(b, well, s.r(4.0), s.t.ink, 1.0, s.t.depth);
            // The CPU's load as a thin bar under its row.
            if live {
                let c0 = cell(0);
                let bar = Rect::from_min_size(
                    Pos2::new(c0.left() + lw, c0.bottom() - 2.5 * k),
                    Vec2::new(vw * stats.load.clamp(0.0, 1.0), 2.0 * k),
                );
                let ink = if stats.load > 0.8 {
                    s.t.danger
                } else {
                    s.t.meter_low
                };
                b.rect(b.snap_rect(bar), Fill::Solid(with_alpha(ink, 0.85)));
            }
            for (i, word) in WORDS.iter().enumerate() {
                let cr = cell(i);
                let lr = Rect::from_min_size(cr.min, Vec2::new(lw, cr.height()));
                let vr = Rect::from_min_max(Pos2::new(cr.left() + lw, cr.top()), cr.max);
                let _ = text::paint(
                    b,
                    lr,
                    &Text::key(word, Role::Micro, s.t.text_dim)
                        .caps(true)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
                let ink = if i == 3 && warn { s.t.danger } else { s.t.text };
                let _ = text::paint(
                    b,
                    vr,
                    &Text::new(&texts[i], Role::Value, ink)
                        .align(Align2::RIGHT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            }
        });
    });
    // Each number's tooltip, made only while hovered.
    for (i, word) in WORDS.iter().enumerate() {
        let resp = ui.interact(cell(i), Id::new(("toolbar-perf", i)), Sense::hover());
        if !resp.hovered() {
            continue;
        }
        let tip = match i {
            0 if live => crate::perf_status::histogram_text(&stats),
            1 => t("topbar.ram_tip"),
            2 => t("topbar.voices_tip"),
            3 => match stats.last_dropout {
                Some(d) if live => crate::perf_status::dropout_tip(app.project(), &d),
                _ => t("topbar.dropouts_tip"),
            },
            _ => t("topbar.no_device"),
        };
        hint::set_text(ui.ctx(), &t(word), "", Caps::NONE);
        let _ = resp.on_hover_text(tip);
    }
}

/// A count, never wider than "999+".
fn count(out: &mut String, n: u64) {
    if n > 999 {
        out.push_str("999+");
    } else {
        let _ = write!(out, "{n}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_reads_in_megabytes_then_gigabytes() {
        let mut s = String::new();
        ram_text(Some(690 << 20), &mut s);
        assert_eq!(s, "690 MB");
        ram_text(Some((13u64 << 30) / 10), &mut s);
        assert_eq!(s, "1.3 GB");
        ram_text(Some(999 << 20), &mut s);
        assert_eq!(s, "999 MB");
        // Rounded to 1 000 MB: in gigabytes, never "1000 MB" (review 1).
        ram_text(Some((1000 << 20) - 1), &mut s);
        assert_eq!(s, "1.0 GB");
        ram_text(Some((999 << 20) + (1 << 18)), &mut s);
        assert_eq!(s, "999 MB");
        ram_text(Some(150 << 30), &mut s);
        assert_eq!(s, "150 GB");
        ram_text(None, &mut s);
        assert_eq!(s, "\u{2014}");
    }
}
