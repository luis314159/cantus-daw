//! Fractus's display (design sheet v2): the sequencer itself — a machined chassis in the
//! colourway's finish holding three windows, every screen dark in every theme.
//!
//! - **The lanes** (the big window): ten rows of step pads behind glass, one row per module,
//!   its name at the left (the lanes on the open page of cards marked, a name clicked opens
//!   its page); every painted step a pad lit from the floor **up to its block's amount**
//!   (the pad's whole height is 100 %), the block's start notched, its amount and its chance
//!   written when they are not whole; the playhead's column and the lane sounding now lit
//!   (`t::PHASE`, `t::ACTIVE`); a lane a held note plays tagged with its key (`t::NOTES`).
//!   Under the pads a square LED per step (the step playing lit) and a seven-segment counter.
//! - **The grain cloud** (`fractus_scope.rs`): a scope's tube and graticule.
//! - **The keys that play the lanes**: ten keys from the first lane's, pressed and lit while
//!   a note plays its lane.

use super::super::canvas::{self, Ink};
use super::super::frame::tr;
use super::super::host::FxHost;
use super::art;
use super::scope;
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::sequencers as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::fx::fractus::{Pattern, LANES};
use cantus_core::EffectSpec;
use cantus_dsp::fractus::t;
use egui::{pos2, vec2, Align2, Rect, Sense};

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

/// The page of cards each lane's module is set on (`super::PAGES`).
pub const PAGE_OF: [usize; LANES] = [1, 1, 1, 1, 2, 2, 2, 2, 0, 3];

/// One block's numbers as the display reads them.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub seen: bool,
    pub step: usize,
    pub steps: usize,
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
            step: tel[t::STEP].max(0.0) as usize,
            steps: tel[t::STEPS].max(0.0) as usize,
            phase: tel[t::PHASE],
            active: tel[t::ACTIVE].max(0.0) as u32,
            notes: tel[t::NOTES].max(0.0) as u32,
            grains: tel[t::GRAINS],
            frozen: tel[t::FROZEN] >= 0.5,
            grain,
        }
    }

    /// The grains shown (no more than the telemetry carries).
    pub fn shown(&self) -> &[(f32, f32)] {
        &self.grain[..(self.grains.max(0.0) as usize).min(t::SHOWN)]
    }
}

/// How many readings back a grain's trail reaches.
pub const TRAIL: usize = 16;

/// The display's memory: the grains of the last [`TRAIL`] readings that differed (the
/// trails the scope draws behind each grain).
#[derive(Clone, Debug)]
pub struct View {
    hist: [[(f32, f32); t::SHOWN]; TRAIL],
    count: [usize; TRAIL],
    /// Where the newest is.
    head: usize,
    len: usize,
}

impl Default for View {
    fn default() -> Self {
        View {
            hist: [[(0.0, 0.0); t::SHOWN]; TRAIL],
            count: [0; TRAIL],
            head: 0,
            len: 0,
        }
    }
}

impl View {
    /// A new reading: kept when its grains moved (the editor draws faster than the
    /// telemetry comes, and a reading seen twice is no motion); forgotten when the unit is
    /// silent.
    pub fn update(&mut self, r: &Reading) {
        if !r.seen {
            self.len = 0;
            return;
        }
        let g = r.shown();
        if self.len > 0 && &self.hist[self.head][..self.count[self.head]] == g {
            return;
        }
        self.head = (self.head + 1) % TRAIL;
        self.hist[self.head][..g.len()].copy_from_slice(g);
        self.count[self.head] = g.len();
        self.len = (self.len + 1).min(TRAIL);
    }

    /// The readings in order, the oldest first (the screenshots' demo).
    pub fn seed(&mut self, series: &[Reading]) {
        *self = View::default();
        for r in series {
            self.update(r);
        }
    }

    /// How many readings it holds.
    pub fn len(&self) -> usize {
        self.len
    }

    /// The grains `age` readings back (0 the newest).
    pub fn back(&self, age: usize) -> &[(f32, f32)] {
        if age >= self.len {
            return &[];
        }
        let i = (self.head + TRAIL - age) % TRAIL;
        &self.hist[i][..self.count[i]]
    }
}

/// Steps in the cycle the spec sets.
pub fn steps_of(spec: &EffectSpec) -> usize {
    let beats =
        cantus_dsp::fractus::STEP_BEATS[(spec.get("step").round().clamp(0.0, 6.0)) as usize];
    let per_bar = (4.0 / beats).round() as usize;
    (per_bar * spec.get("bars").round().clamp(1.0, 4.0) as usize).clamp(1, 256)
}

/// Where the display's parts stand.
pub struct Parts {
    pub plate: Rect,
    /// The engraved titles' row.
    pub header: Rect,
    pub lanes: Rect,
    pub scope: Rect,
    /// The keys' engraved title row, then their well.
    pub keys_title: Rect,
    pub keys: Rect,
}

pub fn parts(r: Rect, k: f32) -> Parts {
    let plate = r.shrink(6.0 * k);
    let inner = plate.shrink2(vec2(18.0 * k, 10.0 * k));
    let header = Rect::from_min_size(inner.min, vec2(inner.width(), 20.0 * k));
    let body = Rect::from_min_max(pos2(inner.left(), header.bottom()), inner.max);
    let right_w = (inner.width() * 0.27).clamp(300.0 * k, 360.0 * k);
    let gap = 16.0 * k;
    let lanes = Rect::from_min_max(body.min, pos2(body.right() - right_w - gap, body.bottom()));
    let right = Rect::from_min_max(pos2(lanes.right() + gap, body.top()), body.max);
    let keys_h = (body.height() * 0.2).clamp(58.0 * k, 78.0 * k);
    let keys = Rect::from_min_max(pos2(right.left(), right.bottom() - keys_h), right.max);
    let keys_title = Rect::from_min_max(
        pos2(right.left(), keys.top() - 20.0 * k),
        pos2(right.right(), keys.top()),
    );
    let scope = Rect::from_min_max(right.min, pos2(right.right(), keys_title.top() - 2.0 * k));
    Parts {
        plate,
        header,
        lanes,
        scope,
        keys_title,
        keys,
    }
}

/// An engraved word on the chassis: its ink, with the cut's lit lip under it.
fn engrave(b: &mut Brush, s: &Style, at: egui::Pos2, align: Align2, t: &Text<'_>) -> Rect {
    let f = art::finish(s);
    let lip = Text {
        color: with_alpha(art::c(f.edge), 0.25),
        ..*t
    };
    canvas::put(b, at + vec2(0.0, 0.8 * s.k), align, &lip, s.k, None);
    let ink = Text {
        color: art::c(f.engrave),
        ..*t
    };
    canvas::put(b, at, align, &ink, s.k, None)
}

/// The display. Returns the page a lane's name asked for when one was clicked. The pointer
/// paints the lanes (`super::paint`) and throws the Frozen switch.
#[allow(clippy::too_many_arguments)]
pub fn show(
    host: &mut dyn FxHost,
    view: &View,
    stroke: &mut super::paint::Stroke,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    now: &Reading,
    page: usize,
) -> Option<usize> {
    let spec = host.spec().clone();
    let k = s.k;
    let ink = Ink::of(s);
    let p = parts(r, k);
    let mut laid = None;
    let mut frozen = Rect::NOTHING;
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, None);
        art::chassis(b, s, p.plate);
        frozen = header(b, s, &p, now, &spec);
        laid = Some(lanes(b, s, &ink, p.lanes, now, &spec, page));
        scope::show(b, s, &ink, p.scope, view, now, &spec);
        keys(b, s, &ink, &p, now, &spec);
    });
    let laid = laid?;
    stroke.laid = Some(laid);
    stroke.frozen = Some(frozen);
    super::paint::pointer(host, stroke, ui, &laid, frozen);
    // A lane's name opens the page of cards its module is set on.
    let mut asked = None;
    for (lane, rect) in laid.names.iter().enumerate() {
        if !rect.is_positive() {
            continue;
        }
        let resp = ui.interact(*rect, ui.id().with(("fractus.lane", lane)), Sense::click());
        if resp.hovered() {
            crate::kit::hint::fallback(
                ui.ctx(),
                tr(NAMES[lane]),
                tr("fractus.h.lane"),
                crate::kit::hint::Caps::NONE,
            );
        }
        if resp.clicked() {
            asked = Some(PAGE_OF[lane]);
        }
    }
    asked
}

/// The engraved row: the lanes' title and the grid in words over the lanes, the cloud's
/// title and its frozen lamp over the scope.
/// Returns the Frozen switch's place (its lamp and its word): a click freezes the cloud.
fn header(b: &mut Brush, s: &Style, p: &Parts, now: &Reading, spec: &EffectSpec) -> Rect {
    let k = s.k;
    let y = p.header.center().y - 1.0 * k;
    engrave(
        b,
        s,
        pos2(p.lanes.left() + 2.0 * k, y),
        Align2::LEFT_CENTER,
        &Text::key("fractus.d.lanes", Role::Micro, art::c(pal::IVORY)),
    );
    const STEPS: [&str; 7] = [
        "fx.fractus.step.0",
        "fx.fractus.step.1",
        "fx.fractus.step.2",
        "fx.fractus.step.3",
        "fx.fractus.step.4",
        "fx.fractus.step.5",
        "fx.fractus.step.6",
    ];
    let step_word = tr(STEPS[spec.get("step").round().clamp(0.0, 6.0) as usize]);
    let words = format!(
        "{} {step_word} \u{b7} {} {:.0} \u{b7} {} {:.0} %",
        tr("fx.fractus.step"),
        tr("fx.fractus.bars"),
        spec.get("bars"),
        tr("fx.fractus.swing"),
        100.0 * spec.get("swing"),
    );
    engrave(
        b,
        s,
        pos2(p.lanes.right() - 2.0 * k, y),
        Align2::RIGHT_CENTER,
        &Text::new(&words, Role::Micro, art::c(pal::IVORY)).caps(false),
    );
    engrave(
        b,
        s,
        pos2(p.scope.left() + 2.0 * k, y),
        Align2::LEFT_CENTER,
        &Text::key("fractus.d.cloud", Role::Micro, art::c(pal::IVORY)),
    );
    // The frozen lamp: dark until the cloud holds.
    let frozen = now.frozen || spec.get("cfreeze") >= 0.5;
    let w = engrave(
        b,
        s,
        pos2(p.scope.right() - 2.0 * k, y),
        Align2::RIGHT_CENTER,
        &Text::key("fractus.d.frozen", Role::Micro, art::c(pal::IVORY)),
    );
    art::led(
        b,
        pos2(w.left() - 9.0 * k, y + 0.5 * k),
        7.0 * k,
        art::c(pal::ICE),
        if frozen { 1.0 } else { 0.0 },
    );
    Rect::from_min_max(
        pos2(w.left() - 16.0 * k, p.header.top()),
        pos2(w.right() + 2.0 * k, p.header.bottom()),
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
}

/// A MIDI note's name: "C3" (60 is C4).
pub fn note_name(n: i32) -> (&'static str, i32) {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    (NAMES[n.rem_euclid(12) as usize], n.div_euclid(12) - 1)
}

/// Where the lanes' window put its parts: each lane's name cell (a click opens its page),
/// the pads' grid and its steps (the pointer paints there).
#[derive(Copy, Clone, Debug)]
pub struct Laid {
    pub names: [Rect; LANES],
    pub grid: Rect,
    pub steps: usize,
    /// The pads' inset in their row, top and bottom: a pad's floor is amount 0, its top 1.
    pub inset: f32,
}

/// The lanes' window: the pads behind glass, the names, the ruler, the step LEDs and the
/// counter.
fn lanes(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    now: &Reading,
    spec: &EffectSpec,
    page: usize,
) -> Laid {
    let k = s.k;
    let g = art::window(b, s, r);
    b.rect(
        g,
        Fill::V(
            crate::kit::color::shade(ink.ground, 0.03),
            crate::kit::color::shade(ink.ground, -0.01),
        ),
    );
    let hot = ink.voice(ink.hue, false);
    let keyed = ink.voice(ink.hue, true);
    let inner = g.shrink2(vec2(8.0 * k, 5.0 * k));
    let ruler_h = 15.0 * k;
    let foot_h = 26.0 * k;
    let tag_w = 30.0 * k;
    let names_w = names_width(b, k) + 12.0 * k + tag_w;
    let grid = Rect::from_min_max(
        pos2(inner.left() + names_w, inner.top() + ruler_h),
        pos2(inner.right(), inner.bottom() - foot_h),
    );
    let steps = steps_of(spec);
    let bars = spec.get("bars").round().clamp(1.0, 4.0) as usize;
    let per_beat = (steps / (4 * bars)).max(1);
    let sw = grid.width() / steps as f32;
    let rh = grid.height() / LANES as f32;
    let cells = sw >= 7.0 * k;
    let gap = (sw * 0.14).clamp(1.0 * k, 3.0 * k);
    let playing = now.seen && now.steps == steps;
    // The step under the playhead: its column lit through every lane.
    if playing {
        let x0 = grid.left() + now.step as f32 * sw;
        b.rect(
            Rect::from_min_max(pos2(x0, grid.top()), pos2(x0 + sw, grid.bottom())),
            Fill::V(with_alpha(hot, 0.04), with_alpha(hot, 0.12)),
        );
    }
    // The ruler: each beat numbered, the bars' brighter; the beats ruled faintly.
    for beat in 0..(steps / per_beat).max(1) {
        let x = grid.left() + (beat * per_beat) as f32 * sw;
        let bar = beat % 4 == 0;
        b.vline(
            x,
            grid.top() - 2.0 * k,
            grid.bottom(),
            if bar { 1.0 * k } else { 0.6 * k },
            with_alpha(ink.grid_strong, if bar { 0.7 } else { 0.35 }),
        );
        if per_beat as f32 * sw >= 14.0 * k {
            canvas::putf(
                b,
                pos2(x + 3.0 * k, inner.top() + ruler_h * 0.5),
                Align2::LEFT_CENTER,
                Role::Micro,
                if bar { ink.text } else { ink.dim },
                k,
                None,
                format_args!("{}", beat + 1),
            );
        }
    }
    let pattern: Pattern = spec.fractus.as_deref().cloned().unwrap_or_default();
    let first = spec.get("key").round() as i32;
    let mut cells_of = [Rect::NOTHING; LANES];
    for (lane, key) in NAMES.iter().enumerate() {
        let y0 = grid.top() + lane as f32 * rh;
        let row = Rect::from_min_max(pos2(grid.left(), y0), pos2(grid.right(), y0 + rh));
        let lit = now.active & (1 << lane) != 0;
        let noted = now.notes & (1 << lane) != 0;
        let here = PAGE_OF[lane] == page;
        if lit {
            b.rect(
                Rect::from_min_max(pos2(inner.left(), y0), row.max),
                Fill::Solid(with_alpha(hot, 0.07)),
            );
        }
        // The open page's lanes: a bar at the left.
        if here {
            b.rrect(
                Rect::from_min_max(
                    pos2(inner.left(), y0 + 6.0 * k),
                    pos2(inner.left() + 2.0 * k, y0 + rh - 6.0 * k),
                ),
                1.0 * k,
                Fill::Solid(with_alpha(ink.text, 0.8)),
            );
        }
        let col = if lit {
            hot
        } else if here {
            ink.text
        } else {
            ink.dim
        };
        canvas::put(
            b,
            pos2(inner.left() + 7.0 * k, row.center().y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, col),
            k,
            None,
        );
        cells_of[lane] = Rect::from_min_max(
            pos2(inner.left(), y0),
            pos2(grid.left() - tag_w - 2.0 * k, y0 + rh),
        );
        // A held note plays it: its key, tagged.
        if noted {
            let (n, o) = note_name(first + lane as i32);
            canvas::putf(
                b,
                pos2(grid.left() - 6.0 * k, row.center().y),
                Align2::RIGHT_CENTER,
                Role::Micro,
                ink.ground,
                k,
                Some(keyed),
                format_args!("{n}{o}"),
            );
        }
        let blocks = pattern.lane(lane);
        let at = |i: usize| {
            blocks
                .iter()
                .find(|bl| usize::from(bl.at) <= i && i < bl.end())
                .copied()
        };
        if cells {
            for i in 0..steps {
                let x0 = grid.left() + i as f32 * sw;
                let cell = Rect::from_min_max(
                    pos2(x0 + gap * 0.5, y0 + 2.5 * k),
                    pos2(x0 + sw - gap * 0.5, y0 + rh - 2.5 * k),
                );
                let blk = at(i);
                let colr = match blk {
                    Some(bl) if bl.prob < 1.0 => mix(hot, ink.ground, 0.3),
                    _ => hot,
                };
                art::pad(
                    b,
                    cell,
                    k,
                    blk.map(|bl| bl.amount),
                    colr,
                    blk.is_some() && lit && playing && i == now.step,
                    (i / per_beat) % 2 == 1,
                );
            }
        }
        for blk in blocks {
            let from = usize::from(blk.at);
            if from >= steps {
                continue;
            }
            let to = blk.end().min(steps);
            let x0 = grid.left() + from as f32 * sw + gap * 0.5;
            let x1 = grid.left() + to as f32 * sw - gap * 0.5;
            let well = Rect::from_min_max(pos2(x0, y0 + 2.5 * k), pos2(x1, y0 + rh - 2.5 * k));
            if !cells {
                // Too many steps for pads: the block as one bar, as tall as its amount.
                let top = well.bottom() - well.height() * blk.amount.clamp(0.08, 1.0);
                b.rrect(
                    Rect::from_min_max(pos2(x0, top), well.max),
                    1.5 * k,
                    Fill::Solid(with_alpha(hot, if lit { 0.95 } else { 0.75 })),
                );
            }
            // The start: a notch, bright.
            b.vline(x0 + 0.8 * k, well.top(), well.bottom(), 1.6 * k, ink.text);
            if blk.prob < 1.0 {
                canvas::dashed(
                    b,
                    pos2(x0, y0 + rh - 1.0 * k),
                    pos2(x1, y0 + rh - 1.0 * k),
                    1.0 * k,
                    ink.text,
                    2.0 * k,
                    2.0 * k,
                );
            }
            block_words(b, k, ink, well, blk.amount, blk.prob);
        }
    }
    // The playhead's line through the lanes, its caret on the ruler.
    if now.seen {
        let x = grid.left() + now.phase.clamp(0.0, 1.0) * grid.width();
        b.vline(
            x,
            grid.top(),
            grid.bottom(),
            1.2 * k,
            with_alpha(ink.text, 0.8),
        );
        b.polygon(
            &[
                pos2(x - 4.0 * k, grid.top() - 6.0 * k),
                pos2(x + 4.0 * k, grid.top() - 6.0 * k),
                pos2(x, grid.top()),
            ],
            ink.text,
        );
    }
    foot(b, k, ink, inner, grid, steps, sw, now, playing, hot);
    art::glass(b, g, k);
    Laid {
        names: cells_of,
        grid,
        steps,
        inset: 2.5 * k,
    }
}

/// A block's amount and chance in words over its pad's dark top, when they are not whole and
/// fit: "50 %", "50 % · 75 % chance".
fn block_words(b: &mut Brush, k: f32, ink: &Ink, well: Rect, amount: f32, prob: f32) {
    if amount >= 1.0 && prob >= 1.0 {
        return;
    }
    let room = well.width() - 6.0 * k;
    let chance = tr("fractus.d.chance");
    let both = format!(
        "{:.0} % \u{b7} {:.0} % {chance}",
        100.0 * amount,
        100.0 * prob
    );
    let one = if amount < 1.0 {
        format!("{:.0} %", 100.0 * amount)
    } else {
        format!("{:.0} % {chance}", 100.0 * prob)
    };
    let fits = |w: &str| {
        crate::kit::text::measure(b.ctx(), &Text::new(w, Role::Micro, ink.text).caps(false), k).x
            <= room
    };
    let words = if amount < 1.0 && prob < 1.0 && fits(&both) {
        both
    } else if fits(&one) {
        one
    } else {
        return;
    };
    canvas::put(
        b,
        pos2(well.left() + 4.0 * k, well.top() + 2.0 * k),
        Align2::LEFT_TOP,
        &Text::new(&words, Role::Micro, ink.text).caps(false),
        k,
        Some(with_alpha(ink.ground, 0.85)),
    );
}

/// Under the pads: the seven-segment counter (the step playing, of the cycle's) under the
/// names, and a square LED per step.
#[allow(clippy::too_many_arguments)]
fn foot(
    b: &mut Brush,
    k: f32,
    ink: &Ink,
    inner: Rect,
    grid: Rect,
    steps: usize,
    sw: f32,
    now: &Reading,
    playing: bool,
    hot: egui::Color32,
) {
    let y = grid.bottom() + (inner.bottom() - grid.bottom()) * 0.55;
    let dh = 15.0 * k;
    let dw = 9.0 * k;
    let n = if playing { now.step + 1 } else { 0 };
    let digits = if steps >= 100 { 3 } else { 2 };
    let mut x = inner.left() + 6.0 * k;
    for d in (0..digits).rev() {
        let v = (n / 10usize.pow(d as u32)) % 10;
        let col = if playing { hot } else { with_alpha(hot, 0.1) };
        art::digit(
            b,
            Rect::from_min_size(pos2(x, y - dh * 0.5), vec2(dw, dh)),
            if playing { v as u8 } else { 8 },
            col,
        );
        x += dw + 3.0 * k;
    }
    canvas::putf(
        b,
        pos2(x + 2.0 * k, y + dh * 0.5),
        Align2::LEFT_BOTTOM,
        Role::Micro,
        ink.dim,
        k,
        None,
        format_args!("/ {steps}"),
    );
    if sw >= 6.0 * k {
        let side = (sw * 0.36).clamp(3.0 * k, 7.0 * k);
        for i in 0..steps {
            let cx = grid.left() + (i as f32 + 0.5) * sw;
            let on = if playing && i == now.step { 1.0 } else { 0.0 };
            art::led(b, pos2(cx, y), side, hot, on);
        }
    } else {
        let bar = Rect::from_min_max(
            pos2(grid.left(), y - 2.0 * k),
            pos2(grid.right(), y + 2.0 * k),
        );
        b.rrect(bar, 2.0 * k, Fill::Solid(with_alpha(ink.grid_strong, 0.5)));
        if now.seen {
            let to = grid.left() + now.phase.clamp(0.0, 1.0) * grid.width();
            b.rrect(
                Rect::from_min_max(bar.min, pos2(to, bar.bottom())),
                2.0 * k,
                Fill::Solid(hot),
            );
        }
    }
}

/// The keys' engraved title and their well: the ten lane keys from the first, pressed and
/// lit while a note plays its lane, each white key named.
fn keys(b: &mut Brush, s: &Style, ink: &Ink, p: &Parts, now: &Reading, spec: &EffectSpec) {
    let k = s.k;
    engrave(
        b,
        s,
        pos2(p.keys_title.left() + 2.0 * k, p.keys_title.center().y),
        Align2::LEFT_CENTER,
        &Text::key("fractus.d.keys", Role::Micro, art::c(pal::IVORY)),
    );
    let well = art::window(b, s, p.keys);
    b.rect(well, Fill::Solid(art::c(pal::GASKET)));
    // The felt the keys rest under.
    let felt = Rect::from_min_max(well.min, pos2(well.right(), well.top() + 3.0 * k));
    b.rect(
        felt,
        Fill::V(
            art::c(pal::FELT),
            mix(art::c(pal::FELT), art::c(pal::SHADOW), 0.5),
        ),
    );
    let first = spec.get("key").round() as i32;
    let bed = Rect::from_min_max(
        pos2(well.left() + 1.0 * k, felt.bottom()),
        pos2(well.right() - 1.0 * k, well.bottom()),
    );
    let mut laid = [None; 16];
    art::layout(bed, first, first + LANES as i32 - 1, &mut laid);
    let glow = ink.voice(ink.hue, true);
    for key in laid.iter().flatten() {
        let lane = key.note - first;
        let is_lane = (0..LANES as i32).contains(&lane);
        let on = is_lane && now.notes & (1 << lane) != 0;
        art::key(b, k, key, on, is_lane, glow);
        if !key.black {
            let (n, o) = note_name(key.note);
            canvas::putf(
                b,
                pos2(key.rect.center().x, key.rect.bottom() - 4.0 * k),
                Align2::CENTER_BOTTOM,
                Role::Micro,
                art::c(pal::EBONY),
                k,
                None,
                format_args!("{n}{o}"),
            );
        }
    }
    // The felt's shadow over the keys' backs.
    b.rect(
        Rect::from_min_max(bed.min, pos2(bed.right(), bed.top() + 3.0 * k)),
        Fill::V(
            with_alpha(art::c(pal::SHADOW), 0.5),
            with_alpha(art::c(pal::SHADOW), 0.0),
        ),
    );
}
