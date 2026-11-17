//! Libra's display (ADR 0133, provisional): at its centre **the balance**
//! (`libra_balance.rs`: the beam tilting with the L/R balance, the pans holding each
//! channel's level, polarity and delay, the arms' reach the width, the correlation along its
//! foot); around it, as secondary instruments, the **tuner** — a needle on a fine scale of
//! ±50 cents, the note and its octave large, the frequency, the reference A dragged up and
//! down — beside the **goniometer** (the output's mid and side as a fading cloud; dragged
//! across for the balance, up and down for the width) over the **correlation** meter, and
//! each channel's level with its polarity and its delay. Calm at rest: the needle settles in
//! the middle, dimmed; the cloud fades to a point.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::FxHost;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Style};
use cantus_dsp::libra::{t, POINTS};
use cantus_dsp::modfx_clock::tel;
use cantus_dsp::pitch::PitchTracker;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::collections::VecDeque;

/// The note names (sharps, the font's `#`).
const NOTES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
/// The goniometer's memory: this many blocks of points.
const TRAIL: usize = 24;

/// What the display remembers.
#[derive(Default)]
pub struct View {
    tracker: Option<(f32, PitchTracker)>,
    last: Option<(u32, u32)>,
    /// The frequency heard (Hz) and how voiced the last frames were (0 … 1, for the needle's
    /// light).
    pub f0: f32,
    pub voiced: f32,
    /// The needle's cents, eased.
    pub needle: f32,
    trail: VecDeque<[f32; 2 * POINTS]>,
    trail_block: f32,
    corr: f32,
    drag: Option<(f32, f32, Pos2)>,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View").field("f0", &self.f0).finish()
    }
}

/// A frequency read against the reference A: the MIDI note and the cents off it.
pub fn read(f0: f32, reference: f32) -> (i32, f32) {
    cantus_dsp::libra::tuner_reading(f0, reference)
}

impl View {
    /// The tap's new output samples into the tracker; the meters from the telemetry.
    pub fn update(&mut self, pairs: &VecDeque<(f32, f32)>, sr: f32, tl: &[f32; 64]) {
        let fresh = match &self.tracker {
            Some((r, _)) => *r != sr,
            None => true,
        };
        if fresh {
            self.tracker = Some((sr, PitchTracker::new(sr)));
            self.last = None;
        }
        if let Some(&(a, b)) = pairs.back() {
            let sig = (a.to_bits(), b.to_bits());
            if self.last != Some(sig) {
                let from = self
                    .last
                    .and_then(|l| {
                        pairs
                            .iter()
                            .rev()
                            .position(|p| (p.0.to_bits(), p.1.to_bits()) == l)
                            .map(|back| pairs.len() - back)
                    })
                    .unwrap_or(pairs.len().saturating_sub(4096));
                if let Some((_, tr)) = &mut self.tracker {
                    for p in pairs.iter().skip(from) {
                        if let Some(fr) = tr.push(p.1) {
                            if fr.voiced && fr.f0 > 0.0 {
                                self.f0 = fr.f0;
                                self.voiced = (self.voiced + 0.35).min(1.0);
                            } else {
                                self.voiced = (self.voiced - 0.15).max(0.0);
                            }
                        }
                    }
                }
                self.last = Some(sig);
            }
        }
        let blocks = tl[tel::BLOCKS];
        if blocks > 0.0 && blocks != self.trail_block {
            self.trail_block = blocks;
            let mut pts = [0.0f32; 2 * POINTS];
            pts.copy_from_slice(&tl[t::POINTS..t::POINTS + 2 * POINTS]);
            if self.trail.len() == TRAIL {
                self.trail.pop_front();
            }
            self.trail.push_back(pts);
            self.corr += (tl[t::CORR] - self.corr) * 0.25;
        }
    }

    /// Fill the goniometer and the tuner as a playing engine would (pictures with no engine).
    pub fn seed(&mut self, blocks: &[[f32; 2 * POINTS]], corr: f32, f0: f32) {
        self.trail = blocks.iter().copied().collect();
        self.corr = corr;
        self.f0 = f0;
        self.voiced = 1.0;
    }
}

/// The display's three parts in its rect: the tuner, the balance at the centre, the
/// goniometer.
pub fn parts(r: Rect, k: f32) -> (Rect, Rect, Rect) {
    let w = r.width();
    let tuner = Rect::from_min_max(r.min, pos2(r.left() + w * 0.28, r.bottom()));
    let balance = Rect::from_min_max(
        pos2(tuner.right(), r.top()),
        pos2(r.left() + w * 0.72, r.bottom()),
    );
    let side = (r.right() - balance.right() - 36.0 * k).min(r.height() - 70.0 * k);
    let gonio = Rect::from_center_size(
        pos2(
            (balance.right() + r.right()) * 0.5,
            r.top() + 24.0 * k + side * 0.5,
        ),
        vec2(side, side),
    );
    (tuner, balance, gonio)
}

/// The drags: on the balance, across for the balance and up and down for the width; on the
/// goniometer the same; on the tuner, up and down for its reference A.
pub fn edit(host: &mut dyn FxHost, v: &mut View, ui: &egui::Ui, r: Rect, k: f32) {
    let (tuner, balance, gonio) = parts(r, k);
    let spec = host.spec();
    let (pan, width, reference) = (spec.get("pan"), spec.get("width"), spec.get("tuner.ref"));
    for (id, area, span) in [
        ("libra-balance", balance, balance.width() * 0.35),
        ("libra-gonio", gonio, gonio.width() * 0.5),
    ] {
        let g = ui.interact(area, ui.id().with(id), Sense::click_and_drag());
        if g.double_clicked() {
            host.set_many(&[("pan", 0.0), ("width", 1.0)], "libra.cmd.centre");
        } else if g.drag_started() {
            if let Some(p) = g.interact_pointer_pos() {
                v.drag = Some((pan, width, p));
            }
        }
        if let (Some((p0, w0, at)), Some(p)) = (v.drag, g.interact_pointer_pos()) {
            if g.dragged() {
                let d = p - at;
                host.set("pan", (p0 + d.x / span).clamp(-1.0, 1.0), Some("pan"));
                host.set(
                    "width",
                    (w0 - d.y / (area.height() * 0.5)).clamp(0.0, 2.0),
                    Some("width"),
                );
            }
        }
        if g.drag_stopped() && v.drag.take().is_some() {
            host.end_gesture();
        }
        if g.hovered() {
            let (tip, how) = if id == "libra-balance" {
                ("libra.balance_tip", "libra.balance_how")
            } else {
                ("libra.gonio_tip", "libra.gonio_how")
            };
            hint::set_text(ui.ctx(), frame::tr(tip), frame::tr(how), Caps::NONE);
        }
    }
    let a = ui.interact(tuner, ui.id().with("libra-tuner"), Sense::drag());
    if a.dragged() {
        let dy = a.drag_delta().y;
        host.set(
            "tuner.ref",
            (reference - dy * 0.25).clamp(415.0, 466.0).round(),
            Some("tuner.ref"),
        );
    }
    if a.drag_stopped() {
        host.end_gesture();
    }
    if a.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("libra.tuner_tip"),
            frame::tr("libra.tuner_how"),
            Caps::NONE,
        );
    }
}

pub fn paint(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    v: &mut View,
    spec: &cantus_core::EffectSpec,
    tl: &[f32; 64],
    k: f32,
) {
    let ink = Ink::of(s);
    let (tuner, balance, gonio) = parts(r, k);
    tuner_paint(b, s, &ink, tuner, v, spec.get("tuner.ref"), k);
    gonio_paint(b, s, &ink, gonio, v, spec, k);
    let live = tl[tel::BLOCKS] > 0.0;
    let (pl, pr) = super::balance::peaks(tl);
    let delay = |key: &str| -> String {
        let ms = spec.get(key);
        let mut w = String::from(frame::tr("libra.delay"));
        w.push(' ');
        if spec.get("delay.unit") >= 0.5 {
            let smp = (ms * 0.001 * tl[tel::SAMPLE_RATE].max(1.0)).round();
            let _ = std::fmt::Write::write_fmt(
                &mut w,
                format_args!("{smp:.0} {}", frame::tr("libra.samples")),
            );
        } else {
            super::super::controls::write_readout(ms, "ms", &mut w);
        }
        w
    };
    let (dl, dr) = (delay("delay.l"), delay("delay.r"));
    let (nl, nr) = (frame::tr("libra.l.left"), frame::tr("libra.l.right"));
    super::balance::paint(
        b,
        &ink,
        balance,
        spec.get("pan"),
        spec.get("width"),
        v.corr,
        [
            super::balance::Pan {
                name: nl,
                level_db: pl,
                inverted: spec.get("pol.l") >= 0.5,
                delay: &dl,
            },
            super::balance::Pan {
                name: nr,
                level_db: pr,
                inverted: spec.get("pol.r") >= 0.5,
                delay: &dr,
            },
        ],
        live,
        k,
    );
    // Hairlines between the three instruments.
    for x in [balance.left(), balance.right()] {
        b.vline(
            x,
            r.top() + 12.0 * k,
            r.bottom() - 12.0 * k,
            0.7 * k,
            ink.grid,
        );
    }
}

fn tuner_paint(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, v: &mut View, reference: f32, k: f32) {
    let (note, cents) = if v.f0 > 0.0 {
        read(v.f0, reference)
    } else {
        (69, 0.0)
    };
    let target = if v.voiced > 0.2 { cents } else { 0.0 };
    v.needle += (target - v.needle) * 0.35;
    let lit = v.voiced;
    let in_tune = v.needle.abs() <= 3.0 && lit > 0.5;
    // The note and its octave large at the top, the cents and the frequency under it.
    let name = if lit > 0.2 {
        format!(
            "{}{}",
            NOTES[note.rem_euclid(12) as usize],
            note.div_euclid(12) - 1
        )
    } else {
        "\u{2013}".to_string()
    };
    let note_col = if lit > 0.2 {
        if in_tune {
            ink.ok
        } else {
            ink.text
        }
    } else {
        ink.dim
    };
    canvas::put(
        b,
        pos2(r.center().x, r.top() + 30.0 * k),
        Align2::CENTER_CENTER,
        &Text::new(&name, Role::Hero, note_col).caps(false),
        k,
        None,
    );
    let mut w = String::new();
    if lit > 0.2 {
        super::super::controls::write_readout(v.needle, "ct", &mut w);
        w.push_str("   ");
        super::super::controls::write_readout(v.f0, "Hz", &mut w);
    } else {
        w.push_str(frame::tr("libra.no_pitch"));
    }
    canvas::put(
        b,
        pos2(r.center().x, r.top() + 60.0 * k),
        Align2::CENTER_CENTER,
        &Text::new(&w, Role::Value, ink.text).caps(false),
        k,
        None,
    );
    // The scale: an arc of ±50 cents over the needle's pivot, a tick every cent, longer
    // every five and ten; the in-tune window (±3) lit.
    let c = pos2(r.center().x, r.bottom() - 26.0 * k);
    let rad = (r.width() * 0.46).min(r.height() - 110.0 * k).max(40.0 * k);
    let span = 0.95f32;
    let ang = |ct: f32| -std::f32::consts::FRAC_PI_2 + ct / 50.0 * span;
    b.arc(
        c,
        rad + 3.0 * k,
        ang(-3.0),
        ang(3.0),
        3.0 * k,
        with_alpha(ink.ok, 0.7),
        false,
    );
    b.arc(
        c,
        rad,
        ang(-50.0),
        ang(50.0),
        0.8 * k,
        ink.grid_strong,
        false,
    );
    for ct in -50..=50 {
        let a = ang(ct as f32);
        let (len, wd, col) = if ct % 10 == 0 {
            (11.0, 1.0, ink.grid_strong)
        } else if ct % 5 == 0 {
            (7.0, 0.8, ink.grid_strong)
        } else {
            (4.0, 0.6, ink.grid)
        };
        let u = vec2(a.cos(), a.sin());
        b.segment(c + u * (rad - len * k), c + u * rad, wd * k, col, false);
    }
    for ct in [-50i32, -25, 0, 25, 50] {
        let a = ang(ct as f32);
        let p = c + vec2(a.cos(), a.sin()) * (rad + 12.0 * k);
        let w = match ct.cmp(&0) {
            std::cmp::Ordering::Equal => "0".to_string(),
            std::cmp::Ordering::Greater => format!("+{ct}"),
            std::cmp::Ordering::Less => format!("\u{2212}{}", -ct),
        };
        canvas::put(
            b,
            p,
            Align2::CENTER_CENTER,
            &Text::new(&w, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    // The needle: lit while a note is held, resting calmly in the middle otherwise.
    let col = if in_tune { ink.ok } else { ink.hue };
    let col = with_alpha(col, 0.35 + 0.65 * lit);
    let a = ang(v.needle.clamp(-55.0, 55.0));
    let u = vec2(a.cos(), a.sin());
    b.segment(
        c + u * (rad * 0.18),
        c + u * (rad - 2.0 * k),
        1.3 * k,
        col,
        true,
    );
    b.circle_stroke(c, 4.0 * k, 1.0 * k, col);
    let mut rw = String::from(frame::tr("libra.reference"));
    rw.push_str(" = ");
    super::super::controls::write_readout(reference, "Hz", &mut rw);
    canvas::put(
        b,
        pos2(r.left() + 8.0 * k, r.bottom() - 6.0 * k),
        Align2::LEFT_BOTTOM,
        &Text::new(&rw, Role::Value, ink.dim).caps(false),
        k,
        Some(ink.plate),
    );
    let _ = s;
}

fn gonio_paint(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    g: Rect,
    v: &View,
    spec: &cantus_core::EffectSpec,
    k: f32,
) {
    let c = g.center();
    let h = g.width() * 0.5;
    // The diamond of full scale and its axes: the mid up, L and R on the diagonals.
    let dia = [
        c - vec2(0.0, h),
        c + vec2(h, 0.0),
        c + vec2(0.0, h),
        c - vec2(h, 0.0),
        c - vec2(0.0, h),
    ];
    b.polyline(&dia, 0.8 * k, ink.grid_strong);
    b.segment(c - vec2(0.0, h), c + vec2(0.0, h), 0.6 * k, ink.grid, false);
    b.segment(c - vec2(h, 0.0), c + vec2(h, 0.0), 0.6 * k, ink.grid, false);
    for (w, at, al) in [
        ("M", c - vec2(0.0, h + 3.0 * k), Align2::CENTER_BOTTOM),
        (
            "L",
            c + vec2(-h * 0.5 - 5.0 * k, -h * 0.5 - 5.0 * k),
            Align2::RIGHT_BOTTOM,
        ),
        (
            "R",
            c + vec2(h * 0.5 + 5.0 * k, -h * 0.5 - 5.0 * k),
            Align2::LEFT_BOTTOM,
        ),
    ] {
        canvas::put(
            b,
            at,
            al,
            &Text::new(w, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    // The cloud: the newest blocks brightest.
    let n = v.trail.len().max(1);
    for (age, pts) in v.trail.iter().enumerate() {
        let alpha = 0.12 + 0.78 * (age + 1) as f32 / n as f32;
        for j in 0..POINTS {
            let (m, sd) = (pts[2 * j], pts[2 * j + 1]);
            let p = c + vec2(sd, -m) * (h * 1.4);
            if g.contains(p) {
                b.dot(p, 0.9 * k, with_alpha(ink.hue, alpha));
            }
        }
    }
    // The balance and the width as set: the balance a tick under the diamond, the width two
    // brackets on the side axis.
    let second = ink.voice(s.hue, true);
    let (pan, width) = (spec.get("pan"), spec.get("width"));
    let px = c.x + pan * h;
    b.segment(
        pos2(px, c.y + h + 3.0 * k),
        pos2(px, c.y + h + 9.0 * k),
        1.4 * k,
        second,
        true,
    );
    let ew = (width * 0.5).clamp(0.0, 1.0) * h;
    for sx in [-1.0f32, 1.0] {
        let x = c.x + sx * ew;
        b.segment(
            pos2(x, c.y - 5.0 * k),
            pos2(x, c.y + 5.0 * k),
            1.2 * k,
            second,
            true,
        );
    }
}
