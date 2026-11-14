//! Libra's display (ADR 0133, provisional): the **tuner** — a needle on a fine scale of
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
use crate::kit::{Brush, Fill, Style};
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
    let x = 69.0 + 12.0 * (f0 / reference).log2();
    let note = x.round();
    (note as i32, (x - note) * 100.0)
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

/// The display's three parts in its rect.
pub fn parts(r: Rect, k: f32) -> (Rect, Rect, Rect) {
    let tuner = Rect::from_min_size(r.min, vec2(r.width() * 0.42, r.height()));
    let gonio_w = (r.height() - 40.0 * k).min(r.width() * 0.36);
    let gonio = Rect::from_min_size(
        pos2(tuner.right() + 12.0 * k, r.top() + 8.0 * k),
        vec2(gonio_w, gonio_w),
    );
    let meters = Rect::from_min_max(pos2(gonio.right() + 14.0 * k, r.top()), r.max);
    (tuner, gonio, meters)
}

/// The drags: on the goniometer, across for the balance and up and down for the width; on
/// the tuner, up and down for its reference A.
pub fn edit(host: &mut dyn FxHost, v: &mut View, ui: &egui::Ui, r: Rect, k: f32) {
    let (tuner, gonio, _) = parts(r, k);
    let spec = host.spec();
    let (pan, width, reference) = (spec.get("pan"), spec.get("width"), spec.get("tuner.ref"));
    let g = ui.interact(gonio, ui.id().with("libra-gonio"), Sense::click_and_drag());
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
            host.set(
                "pan",
                (p0 + d.x / (gonio.width() * 0.5)).clamp(-1.0, 1.0),
                Some("pan"),
            );
            host.set(
                "width",
                (w0 - d.y / (gonio.height() * 0.5)).clamp(0.0, 2.0),
                Some("width"),
            );
        }
    }
    if g.drag_stopped() && v.drag.take().is_some() {
        host.end_gesture();
    }
    if g.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("libra.gonio_tip"),
            frame::tr("libra.gonio_how"),
            Caps::NONE,
        );
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
    let (tuner, gonio, meters) = parts(r, k);
    tuner_paint(b, s, &ink, tuner, v, spec.get("tuner.ref"), k);
    gonio_paint(b, s, &ink, gonio, v, spec, k);
    meters_paint(b, &ink, meters, gonio, v, spec, tl, k);
}

fn tuner_paint(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, v: &mut View, reference: f32, k: f32) {
    let c = pos2(r.center().x, r.top() + r.height() * 0.78);
    let rad = (r.width() * 0.44).min(r.height() * 0.66);
    let (note, cents) = if v.f0 > 0.0 {
        read(v.f0, reference)
    } else {
        (69, 0.0)
    };
    let target = if v.voiced > 0.2 { cents } else { 0.0 };
    v.needle += (target - v.needle) * 0.35;
    let ang = |ct: f32| -std::f32::consts::FRAC_PI_2 + ct / 50.0 * 1.05;
    // The scale: a tick every cent, longer every five and ten; the in-tune window lit.
    b.arc(
        c,
        rad,
        ang(-5.0),
        ang(5.0),
        3.0 * k,
        with_alpha(ink.ok, 0.5),
        false,
    );
    for ct in -50..=50 {
        let a = ang(ct as f32);
        let len = if ct % 10 == 0 {
            12.0
        } else if ct % 5 == 0 {
            8.0
        } else {
            4.0
        };
        let p0 = c + vec2(a.cos(), a.sin()) * (rad - len * k);
        let p1 = c + vec2(a.cos(), a.sin()) * rad;
        let w = if ct % 10 == 0 { 1.0 } else { 0.7 };
        b.segment(
            p0,
            p1,
            w * k,
            if ct % 10 == 0 {
                ink.grid_strong
            } else {
                ink.grid
            },
            false,
        );
    }
    for ct in [-50i32, -25, 0, 25, 50] {
        let a = ang(ct as f32);
        let p = c + vec2(a.cos(), a.sin()) * (rad + 10.0 * k);
        let w = if ct == 0 {
            "0".to_string()
        } else if ct > 0 {
            format!("+{ct}")
        } else {
            format!("\u{2212}{}", -ct)
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
    // The needle: lit while a note is held, calm in the middle otherwise.
    let lit = v.voiced;
    let in_tune = v.needle.abs() <= 3.0 && lit > 0.5;
    let col = if in_tune { ink.ok } else { ink.hue };
    let col = with_alpha(col, 0.35 + 0.65 * lit);
    let a = ang(v.needle.clamp(-55.0, 55.0));
    b.segment(
        c,
        c + vec2(a.cos(), a.sin()) * (rad - 2.0 * k),
        1.4 * k,
        col,
        true,
    );
    b.dot(c, 3.0 * k, col);
    // The note, its octave, the cents and the frequency.
    let name = if lit > 0.2 {
        format!(
            "{}{}",
            NOTES[note.rem_euclid(12) as usize],
            note.div_euclid(12) - 1
        )
    } else {
        "\u{2013}".to_string()
    };
    canvas::put(
        b,
        pos2(c.x, c.y - rad * 0.45),
        Align2::CENTER_CENTER,
        &Text::new(
            &name,
            Role::Hero,
            if lit > 0.2 { ink.text } else { ink.dim },
        )
        .caps(false),
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
        pos2(c.x, c.y - rad * 0.2),
        Align2::CENTER_CENTER,
        &Text::new(&w, Role::Value, ink.text).caps(false),
        k,
        None,
    );
    let mut rw = String::from("A = ");
    super::super::controls::write_readout(reference, "Hz", &mut rw);
    canvas::put(
        b,
        pos2(r.left() + 8.0 * k, r.bottom() - 8.0 * k),
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
    // The diamond of ±1 and its axes: M up, L and R at the diagonals.
    let dia = [
        c - vec2(0.0, h),
        c + vec2(h, 0.0),
        c + vec2(0.0, h),
        c - vec2(h, 0.0),
        c - vec2(0.0, h),
    ];
    b.polyline(&dia, 0.8 * k, ink.grid_strong);
    b.vline(c.x, g.top(), g.bottom(), 0.6 * k, ink.grid);
    b.hline(g.left(), g.right(), c.y, 0.6 * k, ink.grid);
    for (w, at, al) in [
        ("M", c - vec2(0.0, h + 2.0 * k), Align2::CENTER_BOTTOM),
        (
            "L",
            c + vec2(-h * 0.5 - 6.0 * k, -h * 0.5 - 6.0 * k),
            Align2::RIGHT_BOTTOM,
        ),
        (
            "R",
            c + vec2(h * 0.5 + 6.0 * k, -h * 0.5 - 6.0 * k),
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
        let alpha = 0.15 + 0.75 * (age + 1) as f32 / n as f32;
        for j in 0..POINTS {
            let (m, sd) = (pts[2 * j], pts[2 * j + 1]);
            let p = c + vec2(sd, -m) * (h * 1.4);
            if g.contains(p) {
                b.dot(p, 1.0 * k, with_alpha(ink.hue, alpha));
            }
        }
    }
    // The balance and the width as set: a tick where the balance leans, an ellipse of the
    // width.
    let (pan, width) = (spec.get("pan"), spec.get("width"));
    let px = c.x + pan * h;
    b.vline(
        px,
        g.bottom() - 6.0 * k,
        g.bottom(),
        1.2 * k,
        ink.voice(s.hue, true),
    );
    let ew = (width / 2.0).clamp(0.02, 1.0) * h;
    let pts: Vec<Pos2> = (0..=64)
        .map(|i| {
            let a = i as f32 / 64.0 * std::f32::consts::TAU;
            pos2(px + a.cos() * ew * 0.6, c.y + a.sin() * h * 0.6)
        })
        .collect();
    b.polyline(&pts, 0.8 * k, with_alpha(ink.voice(s.hue, true), 0.6));
}

#[allow(clippy::too_many_arguments)]
fn meters_paint(
    b: &mut Brush,
    ink: &Ink,
    r: Rect,
    gonio: Rect,
    v: &View,
    spec: &cantus_core::EffectSpec,
    tl: &[f32; 64],
    k: f32,
) {
    // The correlation under the goniometer: −1 … +1, the needle eased.
    let bar = Rect::from_min_size(
        pos2(gonio.left(), gonio.bottom() + 14.0 * k),
        vec2(gonio.width(), 6.0 * k),
    );
    b.rrect(bar, 3.0 * k, Fill::Solid(with_alpha(ink.grid, 0.6)));
    let x = bar.left() + bar.width() * (v.corr.clamp(-1.0, 1.0) + 1.0) * 0.5;
    let col = if v.corr < 0.0 { ink.danger } else { ink.ok };
    b.rrect(
        Rect::from_min_max(
            pos2(bar.center().x.min(x), bar.top()),
            pos2(bar.center().x.max(x), bar.bottom()),
        ),
        3.0 * k,
        Fill::Solid(with_alpha(col, 0.8)),
    );
    b.vline(
        bar.center().x,
        bar.top() - 2.0 * k,
        bar.bottom() + 2.0 * k,
        0.8 * k,
        ink.grid_strong,
    );
    for (w, at, al) in [
        ("\u{2212}1", bar.left_bottom(), Align2::LEFT_TOP),
        ("+1", bar.right_bottom(), Align2::RIGHT_TOP),
    ] {
        canvas::put(
            b,
            at + vec2(0.0, 2.0 * k),
            al,
            &Text::new(w, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    let mut cw = String::from(frame::tr("libra.corr"));
    cw.push(' ');
    let _ = std::fmt::Write::write_fmt(&mut cw, format_args!("{:+.2}", v.corr));
    canvas::put(
        b,
        pos2(bar.center().x, bar.bottom() + 3.0 * k),
        Align2::CENTER_TOP,
        &Text::new(&cw, Role::Micro, ink.text).caps(false),
        k,
        None,
    );
    // Each channel: its level, its polarity, its delay.
    let w = (r.width() - 12.0 * k) / 2.0;
    for (i, (name, peak, pol, dkey)) in [
        ("L", tl[t::PEAK_L], "pol.l", "delay.l"),
        ("R", tl[t::PEAK_R], "pol.r", "delay.r"),
    ]
    .into_iter()
    .enumerate()
    {
        let col = Rect::from_min_size(
            pos2(r.left() + i as f32 * (w + 12.0 * k), r.top() + 8.0 * k),
            vec2(w, r.height() - 16.0 * k),
        );
        let meter = Rect::from_min_max(
            pos2(col.center().x - 4.0 * k, col.top() + 18.0 * k),
            pos2(col.center().x + 4.0 * k, col.bottom() - 46.0 * k),
        );
        b.rrect(meter, 2.0 * k, Fill::Solid(with_alpha(ink.grid, 0.6)));
        let db = if tl[tel::BLOCKS] > 0.0 { peak } else { -200.0 };
        let u = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let top = meter.bottom() - meter.height() * u;
        b.rrect(
            Rect::from_min_max(pos2(meter.left(), top), meter.max),
            2.0 * k,
            Fill::Solid(if db > -0.1 { ink.danger } else { ink.hue }),
        );
        canvas::put(
            b,
            pos2(col.center().x, col.top()),
            Align2::CENTER_TOP,
            &Text::new(name, Role::Value, ink.text).caps(false),
            k,
            None,
        );
        let inverted = spec.get(pol) >= 0.5;
        let pw = if inverted { "\u{d8} " } else { "" };
        let mut dw = String::from(pw);
        let ms = spec.get(dkey);
        if spec.get("delay.unit") >= 0.5 {
            let smp = (ms * 0.001 * tl[tel::SAMPLE_RATE].max(1.0)).round();
            let _ = std::fmt::Write::write_fmt(
                &mut dw,
                format_args!("{smp:.0} {}", frame::tr("libra.samples")),
            );
        } else {
            super::super::controls::write_readout(ms, "ms", &mut dw);
        }
        canvas::put(
            b,
            pos2(col.center().x, col.bottom() - 30.0 * k),
            Align2::CENTER_CENTER,
            &Text::new(
                &dw,
                Role::Micro,
                if inverted { ink.danger } else { ink.text },
            )
            .caps(false),
            k,
            None,
        );
    }
}
