//! CPU, RAM, voices and dropouts (ADR 0052), moved here from the status bar: the audio
//! callback's load (its histogram in the tooltip), the process's memory in use (the
//! working set, read through `cantus_rt::usage` about once a second, off the audio thread),
//! the voices sounding, and the dropouts counted (the latest one's blame in the tooltip).
//! Without an audio device the engine's numbers are a dash. The texts are rebuilt only
//! when a number changes.

use super::paint::{self as p, ink1, ink2, ink3};
use super::Style;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::opaque;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Role;
use crate::kit::with_brush;
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
    /// No dropout is said on the status line ([`hush_dropouts`]).
    hushed: bool,
}

/// For pictures only (round 39, `shots`): from now on no dropout is said on the status line.
/// A headless shot runs the engine on a machine that may be busy, and a block that missed
/// its deadline there would print a warning the picture must not carry. The engine still
/// counts every dropout.
#[doc(hidden)]
pub fn hush_dropouts(app: &mut CantusApp) {
    app.topbar.ram.hushed = true;
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

/// How many readings the history keeps, and how far apart: half a minute.
pub const HISTORY: usize = 60;
const EVERY: Duration = Duration::from_millis(500);

/// The last half minute of the engine's load, the memory and the voices, a reading every
/// half second while frames are drawn (the More panel's graphs, the bar's CPU graph).
#[derive(Debug)]
pub struct History {
    at: Option<Instant>,
    /// Ring buffers; `next` is where the next reading goes, `len` how many there are.
    pub cpu: [f32; HISTORY],
    pub ram: [f32; HISTORY],
    pub voices: [f32; HISTORY],
    next: usize,
    len: usize,
}

impl Default for History {
    fn default() -> Self {
        History {
            at: None,
            cpu: [0.0; HISTORY],
            ram: [0.0; HISTORY],
            voices: [0.0; HISTORY],
            next: 0,
            len: 0,
        }
    }
}

impl History {
    /// Take a reading when half a second has passed (or none was taken).
    pub fn tick(&mut self, cpu: f32, ram_mb: f32, voices: f32) {
        if self.at.is_some_and(|t| t.elapsed() < EVERY) {
            return;
        }
        self.at = Some(Instant::now());
        self.cpu[self.next] = cpu;
        self.ram[self.next] = ram_mb;
        self.voices[self.next] = voices;
        self.next = (self.next + 1) % HISTORY;
        self.len = (self.len + 1).min(HISTORY);
    }

    /// The `i`-th of the last `n` readings of `ring`, oldest first (0 where none was taken).
    pub fn at(&self, ring: &[f32; HISTORY], n: usize, i: usize) -> f32 {
        let n = n.min(HISTORY);
        if i + self.len < n {
            return 0.0;
        }
        let back = n - i;
        ring[(self.next + HISTORY - back) % HISTORY]
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Key {
    live: bool,
    load: u32,
    ram: Option<u64>,
    overruns: u64,
}

thread_local! {
    static CACHE: RefCell<(Option<Key>, [String; 3])> = RefCell::new((None, Default::default()));
}

/// Where the parts sit from the well's left at 100 %: the graph, the CPU's value, the
/// divider, the memory, the dropouts' pill (`dir-a`: 6, 46, 76, 84, 124).
pub fn width(ctx: &egui::Context, s: &Style) -> f32 {
    let ram = p::measure(ctx, "999 MB", Role::Body, 12.0, s);
    (84.0 * s.k + ram + 2.0 * s.k + 14.0 * s.k + 4.0 * s.k).max(142.0 * s.k)
}

/// Every frame, whether the group is on the bar or folded: the RAM reading, the history,
/// and a new dropout said once on the status line with what took the time (round 20's
/// blame) — the count stays in the group.
pub fn watch(app: &mut CantusApp) {
    app.topbar.ram.tick();
    let stats = *app.engine.stats();
    let live = app.engine.is_live();
    let ram = app.topbar.ram.bytes().map_or(0.0, |b| ram_mb(b) as f32);
    let load = if live { stats.load * 100.0 } else { 0.0 };
    let voices = if live {
        stats.active_voices as f32
    } else {
        0.0
    };
    app.topbar.history.tick(load, ram, voices);
    let Some(d) = stats.last_dropout.filter(|_| !app.topbar.ram.hushed) else {
        return;
    };
    if app.topbar.ram.dropout != Some(d.block) {
        app.topbar.ram.dropout = Some(d.block);
        let text = crate::perf_status::dropout_text(app.project(), &d);
        app.set_error(text);
    }
}

/// A load graph in `r`: the last `n` readings of the CPU as a line over a filled area, in
/// `ink` (amber when busy).
pub fn graph(
    b: &mut crate::kit::Brush,
    r: Rect,
    h: &History,
    ring: &[f32; HISTORY],
    top: f32,
    ink: egui::Color32,
    s: &Style,
) {
    let n = 18usize;
    let mut pts = [Pos2::ZERO; 18];
    for (i, pt) in pts.iter_mut().enumerate() {
        let v = (h.at(ring, n, i) / top.max(1e-3)).clamp(0.0, 1.0);
        *pt = Pos2::new(
            r.left() + r.width() * i as f32 / (n - 1) as f32,
            r.bottom() - r.height() * v,
        );
    }
    b.area(&pts, r.bottom(), ink.gamma_multiply(0.22));
    b.polyline(&pts, 1.1 * s.k, ink);
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let stats = *app.engine.stats();
    let live = app.engine.is_live();
    let key = Key {
        live,
        load: (stats.load * 100.0).round() as u32,
        ram: app.topbar.ram.bytes().map(ram_mb),
        overruns: stats.overruns,
    };
    let row = p::row(r, s);
    let well = Rect::from_min_size(row.min, Vec2::new(width(ui.ctx(), s), row.height()));
    let busy = live && stats.load >= 0.7;
    let drops = if live { stats.overruns } else { 0 };
    let cpu_r = Rect::from_min_max(well.min, Pos2::new(well.left() + 76.0 * k, well.bottom()));
    let ram_r = Rect::from_min_max(Pos2::new(well.left() + 76.0 * k, well.top()), well.max);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let (k_old, texts) = &mut *c;
        if *k_old != Some(key) {
            *k_old = Some(key);
            for t in texts.iter_mut() {
                t.clear();
            }
            let _ = write!(texts[0], "{} %", if live { key.load } else { 0 });
            ram_text(app.topbar.ram.bytes(), &mut texts[1]);
            count(&mut texts[2], drops);
        }
        let h = &app.topbar.history;
        with_brush(ui.painter(), |b| {
            b.register(well, "perf", crate::kit::cap::WELL_KEY_TOKENS, &[]);
            p::well(b, well, s, None);
            let ink = if busy { p::busy(s) } else { ink3(s) };
            let gr = Rect::from_min_size(
                Pos2::new(well.left() + 6.0 * k, well.center().y - 6.0 * k),
                Vec2::new(34.0 * k, 12.0 * k),
            );
            graph(b, gr, h, &h.cpu, 100.0, ink, s);
            let vr = Rect::from_min_max(
                Pos2::new(well.left() + 44.0 * k, well.top()),
                Pos2::new(cpu_r.right() - 4.0 * k, well.bottom()),
            );
            let vi = if busy {
                p::busy(s)
            } else if live {
                ink1(s)
            } else {
                ink2(s)
            };
            p::words(
                b,
                vr,
                &texts[0],
                Role::Body,
                12.0,
                vi,
                Align2::RIGHT_CENTER,
                s,
            );
            b.vline(
                b.snap_mid(cpu_r.right()),
                well.top() + 6.0 * k,
                well.bottom() - 6.0 * k,
                b.px(),
                opaque(s.t.inner_shadow),
            );
            let mr = Rect::from_min_max(
                Pos2::new(ram_r.left() + 8.0 * k, well.top()),
                Pos2::new(well.right() - 20.0 * k, well.bottom()),
            );
            p::words(
                b,
                mr,
                &texts[1],
                Role::Body,
                12.0,
                ink1(s),
                Align2::LEFT_CENTER,
                s,
            );
            if drops > 0 {
                let pr = Rect::from_min_size(
                    Pos2::new(well.right() - 18.0 * k, well.center().y - 6.0 * k),
                    Vec2::new(14.0 * k, 12.0 * k),
                );
                p::pill(b, pr, &texts[2], s.t.danger, s.t.light, s);
            }
        });
    });
    // Each reading's tooltip, made only while hovered.
    for (i, rr) in [cpu_r, ram_r].into_iter().enumerate() {
        let resp = ui.interact(rr, Id::new(("toolbar-perf", i)), Sense::hover());
        if !resp.hovered() {
            continue;
        }
        let tip = match i {
            0 if live => crate::perf_status::histogram_text(&stats),
            0 => t("topbar.no_device"),
            _ => match stats.last_dropout {
                Some(d) if live && drops > 0 => crate::perf_status::dropout_tip(app.project(), &d),
                _ => t("topbar.ram_tip"),
            },
        };
        let name = if i == 0 { "topbar.cpu" } else { "topbar.ram" };
        hint::set_text(ui.ctx(), &t(name), "", Caps::NONE);
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
