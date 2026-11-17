//! Loquax's display (design sheet v2): a vocoder's deck, dark in every theme.
//!
//! - **The object the voice speaks into** (the left; `obj`, chosen in the ⋯ menu, kept per
//!   insert): the gooseneck rising from its chrome socket, the talk-box's tube rising from its
//!   driver to where the mouth would be, or the desk microphone on its weighted base — the
//!   voice reaching it (the on-air lamp, the glow and the sound's arcs follow the voice's
//!   level), its source written under it.
//! - **The bank** (most of the width), a phosphor screen behind glass in a machined bezel:
//!   one column per band on a log frequency axis, each a ladder of fine cells lit from the
//!   bottom by what the band says now (the vocoded output,
//!   `cantus_engine::fx::loquax::T_BANDS`), the highest cell held a moment (the peak); the
//!   band's own level (`b0` … `b63`) a short bar across its column, **drawn** by dragging
//!   across the screen (one undo step a stroke); the formant shift as
//!   the synthesis bands' ticks moved against the analysis bands' along the top.
//! - **The meter bridge** (the right): the voice's needle and the carrier's in one bezel,
//!   dBFS; the carrier's source; pilot lamps for a consonant heard, the hold and the robot;
//!   the robot's note or the synthesiser's voices; the bank in words.
//!
//! The bezels take the colourway's finish (`loquax_art::finish`).

use super::super::canvas::{self, Ink};
use super::super::frame::tr;
use super::super::host::FxHost;
use super::art;
use super::objects;
use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_dsp::loquax::bank::layout;
use cantus_dsp::loquax::t;
use cantus_engine::fx::loquax::{SHOWN, T_BANDS};
use egui::{pos2, vec2, Align2, Rect};

/// One block's numbers as the display reads them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Reading {
    pub seen: bool,
    pub bands: usize,
    pub voices: f32,
    pub gate: f32,
    pub modulator_db: f32,
    pub carrier_db: f32,
    pub robot: bool,
    pub hold: bool,
    /// The bands' output levels in their groups, dBFS.
    pub out_db: [f32; SHOWN],
}

impl Default for Reading {
    fn default() -> Self {
        Reading {
            seen: false,
            bands: 0,
            voices: 0.0,
            gate: 0.0,
            modulator_db: -200.0,
            carrier_db: -200.0,
            robot: false,
            hold: false,
            out_db: [-200.0; SHOWN],
        }
    }
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        let mut out_db = [-200.0; SHOWN];
        out_db.copy_from_slice(&tel[T_BANDS..T_BANDS + SHOWN]);
        Reading {
            seen: tel[0] > 0.0,
            bands: tel[t::BANDS].max(0.0) as usize,
            voices: tel[t::VOICES],
            gate: tel[t::GATE],
            modulator_db: tel[t::MOD_DB],
            carrier_db: tel[t::CARRIER_DB],
            robot: tel[t::ROBOT] >= 0.5,
            hold: tel[t::HOLD] >= 0.5,
            out_db,
        }
    }
}

/// The display's memory: each group's lit height, falling slowly (the phosphor), and its
/// peak, held a moment; the meter bridge's two needles (the voice, the carrier), swinging
/// with a needle's weight.
#[derive(Debug, Default)]
pub struct View {
    lit: [f32; SHOWN],
    peak: [f32; SHOWN],
    hold: [f32; SHOWN],
    needle: [f32; 2],
    /// Where the pointer was last frame while drawing the band levels (a stroke between two
    /// frames reaches every band it crosses).
    drawing: Option<egui::Pos2>,
    /// Where the bands' plot was drawn last frame (tests point at it).
    pub plot: Option<Rect>,
}

/// A level in dBFS as the needle's place on the meters' scale, 0 … 1 (−40 … 0 dBFS).
fn swing(db: f32) -> f32 {
    ((db + 40.0) / 40.0).clamp(0.0, 1.0)
}

/// A level in dBFS as a share of a column, 0 … 1 (−72 dB nothing, 0 dB all).
fn share(db: f32) -> f32 {
    ((db + 72.0) / 72.0).clamp(0.0, 1.0)
}

impl View {
    pub fn update(&mut self, r: &Reading, dt: f32) {
        for g in 0..SHOWN {
            let v = share(r.out_db[g]);
            self.lit[g] = if v > self.lit[g] {
                v
            } else {
                (self.lit[g] - dt * 1.4).max(v)
            };
            if v >= self.peak[g] {
                self.peak[g] = v;
                self.hold[g] = 0.6;
            } else {
                self.hold[g] -= dt;
                if self.hold[g] <= 0.0 {
                    self.peak[g] = (self.peak[g] - dt * 0.8).max(v);
                }
            }
        }
        // A needle's ballistics: about 300 ms to reach where the level is.
        let a = (dt / 0.065).min(1.0);
        for (n, db) in self.needle.iter_mut().zip([r.modulator_db, r.carrier_db]) {
            let to = if r.seen { swing(db) } else { 0.0 };
            *n += (to - *n) * a;
        }
    }

    /// The readings as they stand (the screenshots' demo).
    pub fn seed(&mut self, r: &Reading) {
        for g in 0..SHOWN {
            self.lit[g] = share(r.out_db[g]);
            self.peak[g] = (self.lit[g] + 0.06).min(1.0);
            self.hold[g] = 0.6;
        }
        self.needle = [swing(r.modulator_db), swing(r.carrier_db)];
    }
}

/// Where the display's three objects stand.
struct Parts {
    mic: Rect,
    bank: Rect,
    side: Rect,
}

fn parts(r: Rect, k: f32) -> Parts {
    let inner = r.shrink(12.0 * k);
    let mic_w = (inner.height() * 0.36).clamp(124.0 * k, 168.0 * k);
    let side_w = (inner.width() * 0.26).clamp(260.0 * k, 344.0 * k);
    let mic = Rect::from_min_size(inner.min, vec2(mic_w, inner.height()));
    let side = Rect::from_min_max(pos2(inner.max.x - side_w, inner.min.y), inner.max);
    let bank = Rect::from_min_max(
        pos2(mic.max.x + 10.0 * k, inner.min.y),
        pos2(side.min.x - 16.0 * k, inner.max.y),
    );
    Parts { mic, bank, side }
}

/// The bands' plot inside the bank's rect `r` (the screen inside its bezel, the title row
/// and the frequency marks taken off): where the columns stand and the levels are drawn.
fn plot_of(r: Rect, k: f32) -> Rect {
    let screen = r.shrink(11.0 * k).shrink(1.0 * k);
    let area = Rect::from_min_max(
        pos2(screen.min.x + 10.0 * k, screen.min.y + 8.0 * k),
        screen.max - vec2(10.0, 6.0) * k,
    );
    let ticks_y = area.min.y + 24.0 * k;
    Rect::from_min_max(
        pos2(area.min.x, ticks_y + 12.0 * k),
        area.max - vec2(0.0, 16.0 * k),
    )
}

/// The plot's frequency axis: the bank's range with a margin, log — a frequency's x.
fn freq_x(plot: Rect, spec: &EffectSpec, hz: f32) -> f32 {
    let (f0, f1) = (spec.get("lo") / 1.25, spec.get("hi") * 1.25);
    let span = cantus_dsp::math::log2(f1 / f0);
    plot.min.x + plot.width() * cantus_dsp::math::log2(hz / f0) / span
}

/// A band level, −24 … +12 dB, as its height in the plot (0 … 1).
fn level_share(db: f32) -> f32 {
    ((db + 24.0) / 36.0).clamp(0.0, 1.0)
}

/// The band whose column is nearest `x`, of `n`.
fn band_at(plot: Rect, spec: &EffectSpec, n: usize, x: f32) -> usize {
    let (lo, hi, width) = (spec.get("lo"), spec.get("hi"), spec.get("width"));
    let dist = |i: usize| (freq_x(plot, spec, layout(i, n, lo, hi, width).0) - x).abs();
    (0..n)
        .min_by(|&a, &b| dist(a).total_cmp(&dist(b)))
        .unwrap_or(0)
}

/// The display: the object the voice speaks into, the bank's screen in its bezel (its band
/// levels drawn by the pointer), the meter bridge and the lamps beside it.
pub fn show(
    host: &mut dyn FxHost,
    view: &mut View,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    now: &Reading,
) {
    let k = s.k;
    let p = parts(r, k);
    draw_levels(host, view, ui, p.bank, k);
    let spec = host.spec().clone();
    let ink = Ink::of(s);
    let view: &View = view;
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, None);
        mic(b, s, &ink, p.mic, view, now, &spec);
        bank(b, s, &ink, p.bank, view, now, &spec);
        side(b, s, &ink, p.side, view, now, &spec);
        canvas::edge_b(b, r, &ink);
    });
}

/// The band levels drawn by the pointer: pressed on the bank's screen and dragged, each band
/// the stroke crosses takes the level at the pointer's height (−24 … +12 dB, half a decibel
/// apart; within a decibel of 0 it snaps to 0). One stroke is one step of undo.
fn draw_levels(host: &mut dyn FxHost, view: &mut View, ui: &mut egui::Ui, bank: Rect, k: f32) {
    let plot = plot_of(bank, k);
    view.plot = Some(plot);
    let resp = ui.interact(
        plot,
        ui.id().with("loquax-bands"),
        egui::Sense::click_and_drag(),
    );
    let pointer = resp
        .interact_pointer_pos()
        .filter(|_| resp.is_pointer_button_down_on());
    if let Some(pt) = pointer {
        let spec = host.spec().clone();
        let n = spec.get("bands").round().clamp(8.0, 64.0) as usize;
        let from = view.drawing.unwrap_or(pt);
        // Every band between last frame's point and this one, at the stroke's height there.
        let (a, z) = (
            band_at(plot, &spec, n, from.x),
            band_at(plot, &spec, n, pt.x),
        );
        for i in a.min(z)..=a.max(z) {
            let t = if a == z {
                1.0
            } else {
                (i as f32 - a as f32) / (z as f32 - a as f32)
            };
            let y = from.y + (pt.y - from.y) * t;
            let u = ((plot.max.y - y) / plot.height()).clamp(0.0, 1.0);
            let mut db = ((-24.0 + 36.0 * u) * 2.0).round() / 2.0;
            if db.abs() < 1.0 {
                db = 0.0;
            }
            let key = cantus_core::fx::loquax::BAND_KEYS[i];
            if spec.get(key) != db {
                host.set(key, db, Some("loquax-bands"));
            }
        }
        view.drawing = Some(pt);
    }
    if (resp.drag_stopped() || resp.clicked()) && view.drawing.is_some() {
        view.drawing = None;
        host.end_gesture();
    }
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            tr("loquax.hint.bands"),
            tr("loquax.hint.bands_tip"),
            crate::kit::hint::Caps::NONE,
        );
    }
}

/// Who speaks: the voice's source.
fn voice_key(spec: &EffectSpec) -> &'static str {
    if spec.get("swap") >= 0.5 {
        "loquax.d.voice.key"
    } else {
        "loquax.d.voice.track"
    }
}

/// The microphone, its word and its source under it.
fn mic(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    view: &View,
    now: &Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let glow = ink.voice(ink.hue, false);
    let words = 36.0 * k;
    let stand = Rect::from_min_max(r.min, pos2(r.max.x, r.max.y - words));
    let level = if now.seen { view.needle[0] } else { 0.0 };
    // A pool of the room's light behind the microphone, so it stands in front of the deck.
    b.radial(
        r,
        pos2(stand.center().x, stand.top() + stand.height() * 0.3),
        vec2(stand.width() * 0.75, stand.height() * 0.6),
        &[
            (0.0, with_alpha(ink.text, 0.07)),
            (1.0, with_alpha(ink.text, 0.0)),
        ],
        10,
    );
    // The object the voice speaks into, as the insert keeps it (`obj`).
    match spec.get("obj").round() as i32 {
        1 => {
            objects::talk_box(b, s, stand, level, glow);
        }
        2 => {
            objects::desk_mic(b, s, stand, level, glow);
        }
        _ => {
            art::microphone(b, s, stand, level, glow);
        }
    }
    let x = stand.left() + stand.width() * 0.36;
    let y = stand.bottom() + 6.0 * k;
    canvas::put(
        b,
        pos2(x, y),
        Align2::CENTER_TOP,
        &Text::key("loquax.d.voice", Role::Micro, glow),
        k,
        None,
    );
    canvas::put(
        b,
        pos2(x, y + 15.0 * k),
        Align2::CENTER_TOP,
        &Text::key(voice_key(spec), Role::Micro, ink.text).caps(false),
        k,
        None,
    );
}

fn bank(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    view: &View,
    now: &Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let screen = art::bezel(b, s, r, 11.0 * k);
    let plot = plot_of(r, k);
    // The phosphor behind the glass: its ground, its scanlines.
    b.rect(
        screen,
        Fill::V(shade(ink.ground, 0.035), shade(ink.ground, -0.01)),
    );
    let mut y = screen.top() + 1.5 * k;
    while y < screen.bottom() {
        b.hline(
            screen.left(),
            screen.right(),
            y,
            0.5 * k,
            with_alpha(ink.grid, 0.18),
        );
        y += 3.0 * k;
    }
    let area = Rect::from_min_max(
        pos2(screen.min.x + 10.0 * k, screen.min.y + 8.0 * k),
        screen.max - vec2(10.0, 6.0) * k,
    );
    let n = spec.get("bands").round().clamp(8.0, 64.0) as usize;
    let (lo, hi, width) = (spec.get("lo"), spec.get("hi"), spec.get("width"));
    let formant = spec.get("formant");
    let second = ink.voice(ink.hue, true);
    // The title row: what the screen shows on the left; the bands and the formant's shift
    // on the right.
    canvas::put(
        b,
        area.min,
        Align2::LEFT_TOP,
        &Text::key("loquax.d.bank", Role::Micro, ink.dim),
        k,
        None,
    );
    let at = canvas::putf(
        b,
        pos2(area.max.x, area.min.y),
        Align2::RIGHT_TOP,
        Role::Micro,
        ink.text,
        k,
        None,
        format_args!("{} {n}", tr("loquax.l.bands")),
    );
    if formant != 0.0 {
        canvas::putf(
            b,
            pos2(at.min.x - 14.0 * k, area.min.y),
            Align2::RIGHT_TOP,
            Role::Micro,
            second,
            k,
            None,
            format_args!("{} {formant:+.1} st", tr("loquax.l.formant")),
        );
    }
    // The frequency axis: the bank's range with a margin, log.
    let (f0, f1) = (lo / 1.25, hi * 1.25);
    let ticks_y = area.min.y + 24.0 * k;
    let x_of = |hz: f32| freq_x(plot, spec, hz);
    for (hz, label) in [
        (100.0f32, "100 Hz"),
        (300.0, "300 Hz"),
        (1_000.0, "1 kHz"),
        (3_000.0, "3 kHz"),
        (10_000.0, "10 kHz"),
    ] {
        if hz < f0 || hz > f1 {
            continue;
        }
        let x = x_of(hz);
        b.vline(
            x,
            plot.min.y,
            plot.max.y,
            1.0 * k,
            with_alpha(ink.grid, 0.6),
        );
        // A mark near the screen's edge is written inward of it, never past the glass.
        let (lx, align) = if x > plot.max.x - 26.0 * k {
            (plot.max.x, Align2::RIGHT_TOP)
        } else if x < plot.min.x + 26.0 * k {
            (plot.min.x, Align2::LEFT_TOP)
        } else {
            (x, Align2::CENTER_TOP)
        };
        let t = Text::new(label, Role::Micro, ink.dim).caps(false);
        canvas::put(b, pos2(lx, plot.max.y + 2.0 * k), align, &t, k, None);
    }
    // The level lines across the plot: −24 and −48 dB, fine.
    for db in [-24.0f32, -48.0] {
        let y = plot.max.y - share(db) * plot.height();
        b.hline(
            plot.min.x,
            plot.max.x,
            y,
            0.6 * k,
            with_alpha(ink.grid, 0.45),
        );
    }
    let glow = ink.voice(ink.hue, false);
    let per = n.div_ceil(SHOWN).max(1);
    let cells = 24;
    let cell_h = plot.height() / cells as f32;
    for i in 0..n {
        let (c, _, _) = layout(i, n, lo, hi, width);
        let x = x_of(c);
        let next = if i + 1 < n {
            x_of(layout(i + 1, n, lo, hi, width).0)
        } else {
            x + 8.0 * k
        };
        let prev = if i > 0 {
            x_of(layout(i - 1, n, lo, hi, width).0)
        } else {
            x - 8.0 * k
        };
        let w = ((next - prev) * 0.5 * 0.72).clamp(2.0 * k, 16.0 * k);
        let g = (i / per).min(SHOWN - 1);
        let lit = if now.seen { view.lit[g] } else { 0.0 };
        let peak = if now.seen { view.peak[g] } else { 0.0 };
        // The light the lit cells throw on the glass round them.
        if lit > 0.02 {
            let top = plot.max.y - lit * plot.height();
            b.rrect_soft(
                Rect::from_min_max(pos2(x - w * 0.5, top), pos2(x + w * 0.5, plot.max.y))
                    .expand(2.0 * k),
                2.0 * k,
                Fill::Solid(with_alpha(glow, 0.16)),
                7.0 * k,
            );
        }
        for j in 0..cells {
            let y1 = plot.max.y - j as f32 * cell_h;
            let cell = Rect::from_min_max(
                pos2(x - w * 0.5, y1 - cell_h + 1.2 * k),
                pos2(x + w * 0.5, y1),
            );
            let u = (j as f32 + 0.5) / cells as f32;
            let on = u <= lit;
            let col = if on {
                with_alpha(glow, 0.35 + 0.65 * u)
            } else {
                with_alpha(ink.grid, 0.35)
            };
            b.rect(cell, Fill::Solid(col));
            // The lit cell's hot core along its top.
            if on && w > 4.0 * k {
                b.hline(
                    cell.left() + 0.5 * k,
                    cell.right() - 0.5 * k,
                    cell.top() + 0.5 * k,
                    0.5 * k,
                    with_alpha(ink.text, 0.10 + 0.25 * u),
                );
            }
        }
        if peak > 0.02 {
            let y = plot.max.y - peak * plot.height();
            b.hline(
                x - w * 0.5,
                x + w * 0.5,
                y,
                1.4 * k,
                with_alpha(ink.text, 0.85),
            );
        }
        // The band's own level, −24 … +12 dB: a bar across its column.
        let db = spec.get(cantus_core::fx::loquax::BAND_KEYS[i]);
        if db != 0.0 {
            let u = level_share(db);
            let y = plot.max.y - u * plot.height();
            b.hline(x - w * 0.6, x + w * 0.6, y, 2.0 * k, second);
        }
    }
    // The formant: the synthesis bands' ticks moved against the analysis bands'.
    let ratio = cantus_dsp::math::exp2(formant / 12.0);
    for i in 0..n {
        let (c, _, _) = layout(i, n, lo, hi, width);
        let xa = x_of(c);
        b.vline(
            xa,
            ticks_y - 3.0 * k,
            ticks_y + 3.0 * k,
            1.0 * k,
            with_alpha(ink.dim, 0.8),
        );
        if formant != 0.0 {
            let xs = x_of((c * ratio).clamp(f0, f1));
            b.vline(xs, ticks_y + 3.0 * k, ticks_y + 9.0 * k, 1.0 * k, second);
        }
    }
    art::glass(b, screen, k);
}

/// The meters' scale, dBFS: the major marks (numbered), the minor ones.
const MAJOR: [f32; 5] = [-40.0, -30.0, -20.0, -10.0, 0.0];
const MINOR: [f32; 4] = [-35.0, -25.0, -15.0, -5.0];
/// Where the red begins.
const RED_DB: f32 = -6.0;

/// The meter bridge (the voice's needle and the carrier's), the carrier's source, the
/// lamps (a consonant heard, the hold, the robot), the robot's note or the voices, the
/// bank in words.
fn side(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    view: &View,
    now: &Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let glow = ink.voice(ink.hue, false);
    let second = ink.voice(ink.hue, true);
    let bridge_h = (r.width() * 0.5).min(r.height() * 0.5);
    let bridge = Rect::from_min_size(r.min, vec2(r.width(), bridge_h));
    let inner = art::bezel(b, s, bridge, 9.0 * k);
    // The two faces, a strip of the bezel's finish between them.
    let gap = 7.0 * k;
    let fw = (inner.width() - gap) * 0.5;
    let faces = [
        Rect::from_min_size(inner.min, vec2(fw, inner.height())),
        Rect::from_min_max(pos2(inner.max.x - fw, inner.min.y), inner.max),
    ];
    let f = art::finish(s);
    b.rect(
        Rect::from_min_max(
            pos2(faces[0].max.x, inner.min.y - 1.0 * k),
            pos2(faces[1].min.x, inner.max.y + 1.0 * k),
        ),
        Fill::V(f.face.rgb(), f.foot.rgb()),
    );
    let major = MAJOR.map(swing);
    let minor = MINOR.map(swing);
    let levels = [now.modulator_db, now.carrier_db];
    for (i, (face, (word, col))) in faces
        .into_iter()
        .zip([("loquax.d.voice", glow), ("loquax.d.carrier", second)])
        .enumerate()
    {
        let needle = if now.seen { view.needle[i] } else { 0.0 };
        let d = art::meter(b, k, face, col, &major, &minor, swing(RED_DB), needle);
        for (db, p) in MAJOR.iter().zip(major) {
            canvas::putf(
                b,
                d.at(p, 13.0 * k),
                Align2::CENTER_CENTER,
                Role::Micro,
                if *db > RED_DB { ink.danger } else { ink.dim },
                k,
                None,
                format_args!("{db:.0}"),
            );
        }
        // The meter's word and its level in the face's lower corners, clear of the
        // needle's swing and the pivot's cover.
        let wy = face.bottom() - 5.0 * k;
        canvas::put(
            b,
            pos2(face.left() + 7.0 * k, wy),
            Align2::LEFT_BOTTOM,
            &Text::key(word, Role::Micro, col),
            k,
            None,
        );
        if now.seen && levels[i] > -199.0 {
            canvas::putf(
                b,
                pos2(face.right() - 7.0 * k, wy),
                Align2::RIGHT_BOTTOM,
                Role::Micro,
                ink.text,
                k,
                None,
                format_args!("{:.0} dB", levels[i]),
            );
        }
    }
    let x0 = r.min.x + 2.0 * k;
    let mut y = bridge.bottom() + 10.0 * k;
    // The carrier's source, in words.
    let src = spec.get("src").round() as i32;
    let carrier_key = match (src, spec.get("swap") >= 0.5) {
        (_, true) => "loquax.d.carrier.track",
        (0, _) => "loquax.d.carrier.key",
        (2, _) => "loquax.d.carrier.both",
        _ => "loquax.d.carrier.synth",
    };
    let t = canvas::put(
        b,
        pos2(x0, y),
        Align2::LEFT_TOP,
        &Text::key("loquax.d.carrier", Role::Micro, second),
        k,
        None,
    );
    canvas::put(
        b,
        pos2(t.max.x + 8.0 * k, y),
        Align2::LEFT_TOP,
        &Text::key(carrier_key, Role::Micro, ink.text).caps(false),
        k,
        None,
    );
    y += 22.0 * k;
    // The lamps: a consonant heard, the hold, the robot.
    let robot = spec.get("robot") >= 0.5;
    let lamps = [
        ("loquax.d.consonant", glow, now.gate > 0.5),
        ("loquax.d.hold", glow, now.hold || spec.get("hold") >= 0.5),
        ("loquax.l.robot", second, now.robot || robot),
    ];
    let cw = r.width() / 3.0;
    for (j, (key, col, on)) in lamps.into_iter().enumerate() {
        let p = pos2(x0 + 6.0 * k + j as f32 * cw, y + 7.0 * k);
        art::lamp(b, p, 4.2 * k, col, if on { 1.0 } else { 0.0 });
        canvas::put(
            b,
            pos2(p.x + 10.0 * k, p.y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, if on { ink.text } else { ink.dim }).caps(false),
            k,
            None,
        );
    }
    y += 24.0 * k;
    // The robot's note, or the synthesiser's voices sounding.
    if robot {
        let note = spec.get("rnote").round() as i32;
        let names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let name = names[note.rem_euclid(12) as usize];
        canvas::putf(
            b,
            pos2(x0, y),
            Align2::LEFT_TOP,
            Role::Micro,
            if now.robot { second } else { ink.text },
            k,
            None,
            format_args!("{} {name}{}", tr("loquax.d.robot"), note.div_euclid(12) - 1),
        );
    } else {
        canvas::putf(
            b,
            pos2(x0, y),
            Align2::LEFT_TOP,
            Role::Micro,
            ink.text,
            k,
            None,
            format_args!("{} {:.0}", tr("loquax.d.voices"), now.voices),
        );
    }
    y += 26.0 * k;
    // The bank in words: its bands and slope, its range, its followers, the formant.
    b.hline(
        x0,
        r.max.x,
        y - 8.0 * k,
        1.0 * k,
        with_alpha(ink.grid_strong, 0.7),
    );
    let order = [4, 6, 8][(spec.get("order").round().clamp(0.0, 2.0)) as usize];
    let lines: [(&'static str, String); 4] = [
        (
            "loquax.l.bands",
            format!(
                "{:.0} \u{b7} {} {order}",
                spec.get("bands"),
                tr("loquax.d.order")
            ),
        ),
        (
            "loquax.d.range",
            format!(
                "{} \u{2013} {}",
                cantus_core::fx::readout::readout(spec.get("lo"), "Hz"),
                cantus_core::fx::readout::readout(spec.get("hi"), "Hz")
            ),
        ),
        (
            "loquax.card.follow",
            format!("{:.1} ms \u{b7} {:.0} ms", spec.get("att"), spec.get("rel")),
        ),
        (
            "loquax.l.formant",
            format!(
                "{:+.1} st \u{b7} {} {:+.1} dB",
                spec.get("formant"),
                tr("loquax.d.tilt"),
                spec.get("tilt")
            ),
        ),
    ];
    for (key, value) in lines {
        // A line that would run past the side waits for a taller window.
        if y + 14.0 * k > r.max.y {
            break;
        }
        canvas::put(
            b,
            pos2(x0, y),
            Align2::LEFT_TOP,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            None,
        );
        canvas::put(
            b,
            pos2(r.max.x, y),
            Align2::RIGHT_TOP,
            &Text::new(&value, Role::Micro, ink.text).caps(false),
            k,
            None,
        );
        y += 18.0 * k;
    }
}
