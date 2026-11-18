//! Lenis's control panel (design sheet v1): the column of the instrument's front panel
//! beside its screen — the Depth dial (the hero: drag it up or down, double-click for none,
//! its menu offers automation and MIDI learn), the two readouts behind smoked glass (the
//! most cut now, held a second; the latency the quality gives), and three rows of push
//! keys with lamp windows: Quality (Fast, Standard, Precise), Curve (Soft, Hard) beside
//! *Listen to what is removed*, Channels (Linked, Independent, Mid and side). A click on a
//! key pushes it in (its row's other keys come out); each key's menu offers automation and
//! MIDI learn (the kit's `stepped_offer`); the help line says what it does. Every word is
//! measured, so the column is as wide as its words in each language.

use super::super::controls;
use super::super::frame::tr;
use super::super::host::FxHost;
use super::art::{self, c};
use super::picture::Picture;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{Brush, Style};
use cantus_core::EffectSpec;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense, Vec2};

/// A row of keys: its parameter, its caption, its keys' words, its help.
#[derive(Copy, Clone, Debug)]
struct Row {
    key: &'static str,
    caption: &'static str,
    words: &'static [&'static str],
    tip: &'static str,
}

const QUALITY: Row = Row {
    key: "qual",
    caption: "fx.lenis.qual",
    words: &["fx.lenis.qual.0", "fx.lenis.qual.1", "fx.lenis.qual.2"],
    tip: "lenis.tip.qual",
};
const CURVE: Row = Row {
    key: "curve",
    caption: "fx.lenis.curve",
    words: &["fx.lenis.curve.0", "fx.lenis.curve.1"],
    tip: "lenis.tip.curve",
};
/// The delta: one key, pushed in to listen.
const LISTEN: Row = Row {
    key: "delta",
    caption: "lenis.k.listen",
    words: &["lenis.d.removed"],
    tip: "lenis.tip.delta",
};
const CHANNELS: Row = Row {
    key: "chan",
    caption: "fx.lenis.chan",
    words: &["fx.lenis.chan.0", "fx.lenis.chan.1", "fx.lenis.chan.2"],
    tip: "lenis.tip.chan",
};
/// The rows, top to bottom; the second line holds two groups.
const LINES: [&[Row]; 3] = [&[QUALITY], &[CURVE, LISTEN], &[CHANNELS]];

/// The dial's radius, the key cap's size, at 100 %.
const DIAL: f32 = 46.0;
const DIAL_MAX: f32 = 66.0;
const CAP: Vec2 = Vec2::new(40.0, 24.0);
/// The readouts' digit height and cells.
const DIGIT_H: f32 = 26.0;
const CELLS: usize = 3;
/// The dial's sweep, radians.
pub const SWEEP: f32 = 5.0;

fn micro(ctx: &egui::Context, key: &str, k: f32) -> Vec2 {
    ktext::measure(ctx, &Text::key(key, Role::Micro, Color32::TRANSPARENT), k)
}

/// One key laid out: its row, its value, its cap and its word.
#[derive(Copy, Clone, Debug)]
pub struct Key {
    row: Row,
    value: usize,
    pub cap: Rect,
    word: Rect,
}

impl Key {
    /// The parameter the key sets.
    pub fn param(&self) -> &'static str {
        self.row.key
    }

    /// The value it sets (the delta's one key: 1).
    pub fn value(&self) -> usize {
        self.value
    }

    /// Where the pointer presses it: its cap and its word.
    pub fn hit(&self, k: f32) -> Rect {
        self.cap.union(self.word).expand(2.0 * k)
    }
}

/// Where everything in the column stands.
pub struct Panel {
    pub dial: Pos2,
    pub dial_rad: f32,
    /// The readouts' windows: the most cut, the latency.
    pub most: Rect,
    pub latency: Rect,
    /// The rows' captions, where they are printed.
    captions: [(Pos2, &'static str, f32); 4],
    pub keys: [Option<Key>; 9],
}

/// The width a row of keys takes: each key's column as wide as its cap or its word.
fn group_width(ctx: &egui::Context, k: f32, row: &Row) -> f32 {
    let cols: f32 = row
        .words
        .iter()
        .map(|w| micro(ctx, w, k).x.max(CAP.x * k))
        .sum();
    cols + 12.0 * k * (row.words.len() as f32 - 1.0)
}

fn line_width(ctx: &egui::Context, k: f32, line: &[Row]) -> f32 {
    let groups: f32 = line.iter().map(|r| group_width(ctx, k, r)).sum();
    groups + 26.0 * k * (line.len() as f32 - 1.0)
}

/// One readout's column: its caption, or its window and unit.
fn readout_width(ctx: &egui::Context, k: f32) -> f32 {
    let win = art::digits_width(CELLS, DIGIT_H * k) + 14.0 * k + 26.0 * k;
    ["lenis.r.most", "lenis.r.latency"]
        .iter()
        .map(|w| micro(ctx, w, k).x)
        .fold(win, f32::max)
}

/// The gap between the two readouts.
const READOUT_GAP: f32 = 28.0;

/// The width the column needs at scale `k` in the interface's language.
pub fn width(ctx: &egui::Context, k: f32) -> f32 {
    let dial = 2.0 * (DIAL_MAX + 40.0) * k;
    let readouts = 2.0 * readout_width(ctx, k) + READOUT_GAP * k;
    LINES
        .iter()
        .map(|l| line_width(ctx, k, l))
        .fold(dial.max(readouts), f32::max)
        + 8.0 * k
}

/// The column laid out in `r`: the dial at the top, the readouts under it, the rows of
/// keys at the foot, each line centred.
pub fn lay(ctx: &egui::Context, s: &Style, r: Rect) -> Panel {
    let k = s.k;
    let row_h = 14.0 * k + 6.0 * k + CAP.y * k + 6.0 * k + 12.0 * k;
    let keys_h = 3.0 * row_h + 2.0 * 12.0 * k;
    let win = vec2(
        art::digits_width(CELLS, DIGIT_H * k) + 14.0 * k,
        DIGIT_H * k + 12.0 * k,
    );
    let read_h = 20.0 * k + win.y;
    let keys_top = r.bottom() - keys_h;
    let read_top = keys_top - 22.0 * k - read_h;
    // The dial in what is left above, as large as fits.
    let dial_h = read_top - 16.0 * k - r.top();
    let dial_rad = ((dial_h - 2.0 * 44.0 * k) * 0.5).clamp(DIAL * k, DIAL_MAX * k);
    let dial = pos2(r.center().x, r.top() + dial_h * 0.5 + 4.0 * k);
    // The readouts side by side.
    let rw = readout_width(ctx, k);
    let x0 = r.center().x - (2.0 * rw + READOUT_GAP * k) * 0.5;
    let most = Rect::from_min_size(pos2(x0, read_top + 20.0 * k), win);
    let latency = Rect::from_min_size(pos2(x0 + rw + READOUT_GAP * k, read_top + 20.0 * k), win);
    // The rows of keys.
    let mut keys = [None; 9];
    let mut captions = [(Pos2::ZERO, "", 0.0); 4];
    let (mut n, mut m) = (0, 0);
    let mut y = keys_top;
    let widest = LINES
        .iter()
        .map(|l| line_width(ctx, k, l))
        .fold(0.0, f32::max);
    let left = r.center().x - widest * 0.5;
    for line in LINES {
        let mut x = left;
        for row in line {
            let gw = group_width(ctx, k, row);
            captions[m] = (pos2(x, y), row.caption, gw);
            m += 1;
            let mut kx = x;
            for (i, w) in row.words.iter().enumerate() {
                let ww = micro(ctx, w, k).x;
                let col = ww.max(CAP.x * k);
                let cap =
                    Rect::from_min_size(pos2(kx + (col - CAP.x * k) * 0.5, y + 20.0 * k), CAP * k);
                let word = Rect::from_min_size(
                    pos2(kx + (col - ww) * 0.5, cap.bottom() + 6.0 * k),
                    vec2(ww, 12.0 * k),
                );
                if n < keys.len() {
                    keys[n] = Some(Key {
                        row: *row,
                        value: if row.words.len() == 1 { 1 } else { i },
                        cap,
                        word,
                    });
                    n += 1;
                }
                kx += col + 12.0 * k;
            }
            x += gw + 26.0 * k;
        }
        y += row_h + 12.0 * k;
    }
    Panel {
        dial,
        dial_rad,
        most,
        latency,
        captions,
        keys,
    }
}

/// What the pointer does in the column this frame.
#[derive(Copy, Clone, Debug, Default)]
pub struct Press {
    pub dial: bool,
    pub keys: [bool; 9],
}

/// The column's controls answer the pointer (see the module's words).
pub fn edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, panel: &Panel) -> Press {
    let k = s.k;
    let mut out = Press::default();
    // The dial: a drag up turns it up (shift for fine), a double-click turns it to none.
    let id = ui.id().with("lenis-depth");
    let rect = Rect::from_center_size(panel.dial, Vec2::splat(2.0 * panel.dial_rad));
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let learnt = controls::stepped_offer(ui, s, host, "depth", tr("fx.lenis.depth"), &resp);
    if !learnt {
        let v = host.spec().get("depth");
        if resp.dragged() {
            let fine = if ui.input(|i| i.modifiers.shift) {
                0.2
            } else {
                1.0
            };
            let d = resp.drag_delta();
            let nv = (v + (d.x - d.y) / (220.0 * k) * fine).clamp(0.0, 1.0);
            if nv != v {
                host.set("depth", nv, Some("depth"));
            }
        }
        if resp.drag_stopped() {
            host.end_gesture();
        }
        if resp.double_clicked() {
            host.set("depth", 0.0, None);
        }
    }
    out.dial = resp.hovered() || resp.dragged();
    if out.dial {
        let v = host.spec().get("depth");
        hint::set(
            ui.ctx(),
            tr("fx.lenis.depth"),
            Caps::PARAM,
            resp.dragged(),
            |b| {
                use std::fmt::Write as _;
                write!(b, "{:.0} % \u{b7} {}", v * 100.0, tr("lenis.tip.depth"))
            },
        );
    }
    // The keys.
    for (j, key) in panel.keys.iter().enumerate() {
        let Some(key) = key else { continue };
        let row = key.row;
        let id = ui.id().with(("lenis-key", row.key, key.value));
        let resp = ui.interact(key.hit(k), id, Sense::click());
        let name = tr(row.caption);
        let learnt = controls::stepped_offer(ui, s, host, row.key, name, &resp);
        let now = host.spec().get(row.key).round().max(0.0) as usize;
        if resp.clicked() && !learnt {
            if row.words.len() == 1 {
                host.set(row.key, if now >= 1 { 0.0 } else { 1.0 }, None);
            } else if now != key.value {
                host.set(row.key, key.value as f32, None);
            }
        }
        if resp.hovered() {
            hint::set_text(ui.ctx(), name, tr(row.tip), Caps::NONE);
        }
        out.keys[j] = resp.hovered();
    }
    out
}

/// The column: the dial and its scale, the readouts, the keys and their words.
pub fn paint(
    b: &mut Brush,
    s: &Style,
    panel: &Panel,
    press: &Press,
    spec: &EffectSpec,
    pic: &Picture,
    latency_ms: f32,
) {
    let k = s.k;
    let f = art::finish(s);
    let trace = c(f.trace);
    let cut = c(f.cut);
    // The dial: its caption over it, its scale's numbers, its value under it.
    let depth = spec.get("depth").clamp(0.0, 1.0);
    let lamp = if art::pale(s) { c(f.print) } else { trace };
    art::dial(
        b,
        s,
        panel.dial,
        panel.dial_rad,
        depth,
        SWEEP,
        press.dial,
        lamp,
    );
    for (t, n) in [(0.0f32, 0), (0.5, 50), (1.0, 100)] {
        let a = art::dial_angle(t, SWEEP);
        let d = vec2(cantus_dsp::math::sin(a), -cantus_dsp::math::cos(a));
        let at = panel.dial + d * (panel.dial_rad + 22.0 * k);
        let align = if d.x < -0.3 {
            Align2::RIGHT_CENTER
        } else if d.x > 0.3 {
            Align2::LEFT_CENTER
        } else {
            Align2::CENTER_BOTTOM
        };
        let t = canvas_num(n);
        art::print(
            b,
            s,
            at,
            align,
            &Text::new(t, Role::Micro, Color32::TRANSPARENT),
            1.0,
        );
    }
    let top = panel.dial.y - panel.dial_rad - 38.0 * k;
    art::print(
        b,
        s,
        pos2(panel.dial.x, top),
        Align2::CENTER_BOTTOM,
        &Text::key("fx.lenis.depth", Role::Label, Color32::TRANSPARENT),
        1.0,
    );
    super::super::canvas::putf(
        b,
        pos2(panel.dial.x, panel.dial.y + panel.dial_rad + 14.0 * k),
        Align2::CENTER_TOP,
        Role::Value,
        art::ink(s),
        k,
        None,
        format_args!("{:.0} %", depth * 100.0),
    );
    // The readouts: their captions printed over the smoked windows, the units beside.
    let led = c(f.led);
    let most = if pic.held >= 99.95 { 99.9 } else { pic.held };
    for (win, caption, unit, value) in [
        (panel.most, "lenis.r.most", "dB", most),
        (panel.latency, "lenis.r.latency", "ms", latency_ms),
    ] {
        art::print(
            b,
            s,
            pos2(win.left(), win.top() - 6.0 * k),
            Align2::LEFT_BOTTOM,
            &Text::key(caption, Role::Micro, Color32::TRANSPARENT),
            1.0,
        );
        art::smoke(b, s, win);
        let digits = win.shrink2(vec2(7.0 * k, 6.0 * k));
        let mut buf = [0u8; 8];
        let text = fixed(value, &mut buf);
        art::digits(b, s, digits, text, CELLS, led);
        art::smoke_cover(b, s, win);
        art::print(
            b,
            s,
            pos2(win.right() + 7.0 * k, win.bottom() - 2.0 * k),
            Align2::LEFT_BOTTOM,
            &Text::new(unit, Role::Label, Color32::TRANSPARENT).caps(false),
            1.0,
        );
    }
    // The rows: a caption and a fine rule, the keys, their words.
    for (at, caption, w) in panel.captions {
        if caption.is_empty() {
            continue;
        }
        let t = art::print(
            b,
            s,
            at + vec2(0.0, 7.0 * k),
            Align2::LEFT_CENTER,
            &Text::key(caption, Role::Micro, Color32::TRANSPARENT),
            1.0,
        );
        if t.right() + 6.0 * k < at.x + w {
            art::rule(
                b,
                s,
                pos2(t.right() + 6.0 * k, t.center().y),
                pos2(at.x + w, t.center().y),
                0.6 * k,
            );
        }
    }
    for (key, hot) in panel.keys.iter().zip(press.keys) {
        let Some(key) = key else { continue };
        let row = key.row;
        let now = spec.get(row.key).round().max(0.0) as usize;
        let on = now == key.value;
        let lit = on.then_some(if row.key == "delta" { cut } else { trace });
        art::key(b, s, key.cap, lit, hot);
        let word = row.words[if row.words.len() == 1 { 0 } else { key.value }];
        art::print(
            b,
            s,
            key.word.center(),
            Align2::CENTER_CENTER,
            &Text::key(word, Role::Micro, Color32::TRANSPARENT),
            1.0,
        );
    }
    let _ = with_alpha;
}

/// A whole number's words without allocating ("0", "50", "100").
fn canvas_num(n: u32) -> &'static str {
    match n {
        0 => "0",
        50 => "50",
        _ => "100",
    }
}

/// `v` written with one decimal into `buf` ("7.4", "10.6", "0.0").
fn fixed(v: f32, buf: &mut [u8; 8]) -> &str {
    use std::io::Write as _;
    let mut cur = std::io::Cursor::new(&mut buf[..]);
    let _ = write!(cur, "{:.1}", v.clamp(0.0, 999.9));
    let n = cur.position() as usize;
    std::str::from_utf8(&buf[..n]).unwrap_or("")
}
