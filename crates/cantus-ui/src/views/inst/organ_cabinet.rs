//! Organum's live pictures, each drawn with the kit's brush into a card's display and each,
//! where it can be, also a control:
//!
//! - **The cabinet from above**: the horn and the drum at the angles the engine reports,
//!   so they turn at their real speed (state, not decoration — at rest, with no engine,
//!   they sit where a new speaker has them and the speeds read the targets); the two
//!   microphones where they stand (drag one to set the spread and the distance) and the
//!   brake's parking angle (drag it round).
//! - **The spin-up**: rpm against time for the speed change under way, from the time
//!   constants, with where the rotors are now.
//! - **The scanner**: the delay line's taps, the span V1/V2/V3 sweeps, and the pickup.
//! - **The percussion**: its decay to −60 dB, and the envelope now.
//! - **The splits**: which keys play the pedals, the lower and the upper manual; drag a
//!   boundary, double-click it to remove it.

use super::parts::{label, readout};
use super::set;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self as ktext, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::organum::rotary::{
    RotarySettings, RotorSpeed, CABINET_HALF_M, DRUM_RADIUS_M, HORN_RADIUS_M,
};
use cantus_dsp::organum::scanner::{SPAN, TAPS, TAP_MS};
use cantus_dsp::organum::OrganSettings;
use cantus_engine::inst::organ::tel;
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use std::f32::consts::TAU;

/// What the engine says the rotors are doing, or what they would be doing at rest.
pub struct Rotors {
    pub horn_angle: f32,
    pub drum_angle: f32,
    pub horn_rpm: f32,
    pub drum_rpm: f32,
}

impl Rotors {
    pub fn of(r: &RotarySettings, t: &cantus_engine::Telemetry) -> Rotors {
        if t[tel::ALIVE] > 0.5 {
            return Rotors {
                horn_angle: t[tel::HORN_ANGLE],
                drum_angle: t[tel::DRUM_ANGLE],
                horn_rpm: t[tel::HORN_RPM],
                drum_rpm: t[tel::DRUM_RPM],
            };
        }
        let (h, d) = targets(r);
        let park = r.brake_deg / 360.0;
        let stopped = r.speed == RotorSpeed::Stop;
        Rotors {
            horn_angle: if stopped { park } else { 0.0 },
            drum_angle: if stopped { park } else { 0.25 },
            horn_rpm: h,
            drum_rpm: d,
        }
    }
}

/// The speeds the rotors are heading for: (horn, drum) rpm.
fn targets(r: &RotarySettings) -> (f32, f32) {
    match r.speed {
        RotorSpeed::Stop => (0.0, 0.0),
        RotorSpeed::Slow => (r.horn_slow_rpm, r.drum_slow_rpm),
        RotorSpeed::Fast => (r.horn_fast_rpm, r.drum_fast_rpm),
    }
}

/// Metres on the cabinet plane to a screen point: the listener is at the bottom, so the
/// cabinet's front (+x in the model) is down and its left (+y) is screen-left.
fn to_screen(c: Pos2, scale: f32, x: f32, y: f32) -> Pos2 {
    Pos2::new(c.x - y * scale, c.y + x * scale)
}

fn from_screen(c: Pos2, scale: f32, p: Pos2) -> (f32, f32) {
    ((p.y - c.y) / scale, -(p.x - c.x) / scale)
}

/// A faint line colour on a display's ink.
fn faint(s: &Style, a: f32) -> egui::Color32 {
    with_alpha(s.t.text_dim, a)
}

/// The cabinet from above in the display `d`.
#[allow(clippy::too_many_arguments)]
pub fn cabinet(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    o: &OrganSettings,
    rotors: &Rotors,
    d: Rect,
    s: &Style,
) {
    let r = &o.rotary;
    let text_h = s.s(34.0);
    let area = Rect::from_min_max(
        d.min + Vec2::splat(s.s(6.0)),
        Pos2::new(d.right() - s.s(6.0), d.bottom() - text_h),
    );
    // Scale: the cabinet (0.6 m wide) and the microphones as far as 0.9 m out; a
    // microphone further away is drawn at 0.9 m with its distance printed.
    let dist = CABINET_HALF_M + r.mic_distance_cm * 0.01;
    let half = (r.mic_spread_deg * 0.5).to_radians();
    let shown = dist.min(0.9);
    let xspan = (shown * half.sin()).max(CABINET_HALF_M) + 0.07;
    let above = CABINET_HALF_M + 0.04;
    let below = (shown * half.cos()).max(CABINET_HALF_M) + 0.07;
    let scale = (area.width() * 0.5 / xspan).min(area.height() / (above + below));
    let c = Pos2::new(
        area.center().x,
        area.top() + (area.height() - (above + below) * scale) * 0.5 + above * scale,
    );

    // The brake's parking angle: a tick on the horn's circle, draggable.
    let pa = r.brake_deg.to_radians();
    let park = |rad: f32| to_screen(c, scale, rad * pa.cos(), rad * pa.sin());
    let pk = park(HORN_RADIUS_M + 0.035);
    let brake = ui.interact(
        Rect::from_center_size(pk, Vec2::splat(s.s(14.0))),
        egui::Id::new(("organ-brake", track.0)),
        Sense::drag(),
    );
    if let Some(p) = brake.interact_pointer_pos().filter(|_| brake.dragged()) {
        let (x, y) = from_screen(c, scale, p);
        let deg = (y.atan2(x).to_degrees() + 360.0)
            .rem_euclid(360.0)
            .round()
            .min(359.0);
        set(app, track, "rot.brake", deg, true);
    }
    if brake.drag_stopped() {
        app.end_gesture();
    }
    if brake.hovered() || brake.dragged() {
        let tip = tf(
            "organ.brake_tip",
            &[("deg", &format!("{:.0}", r.brake_deg))],
        );
        hint::set_text(ui.ctx(), &tip, "", Caps::PARAM);
    }
    // The microphones.
    let mut mics = [(Pos2::ZERO, false); 2];
    for (k, sign) in [(0usize, 1.0f32), (1, -1.0)] {
        let p = to_screen(c, scale, shown * half.cos(), sign * shown * half.sin());
        let resp = ui.interact(
            Rect::from_center_size(p, Vec2::splat(s.s(16.0))),
            egui::Id::new(("organ-mic", track.0, k)),
            Sense::drag(),
        );
        if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
            let (qx, qy) = from_screen(c, scale, q);
            let cm = ((qx * qx + qy * qy).sqrt() - CABINET_HALF_M).max(0.1) * 100.0;
            let spread = (2.0 * (qy * sign).atan2(qx).to_degrees()).clamp(0.0, 180.0);
            set(app, track, "rot.dist", cm.clamp(10.0, 200.0), true);
            set(app, track, "rot.spread", spread.round(), true);
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        let hot = resp.hovered() || resp.dragged();
        if hot {
            let tip = tf(
                "organ.mic_tip",
                &[
                    ("cm", &format!("{:.0}", r.mic_distance_cm)),
                    ("deg", &format!("{:.0}", r.mic_spread_deg)),
                ],
            );
            hint::set_text(ui.ctx(), &tip, "", Caps::PARAM);
        }
        mics[k] = (p, hot);
    }
    let brake_hot = brake.hovered() || brake.dragged();

    with_brush(&ui.painter_at(d), |b| {
        let t = &s.t;
        // The cabinet: its box, the baffle between the rotors drawn as a line.
        let cab = Rect::from_center_size(c, Vec2::splat(2.0 * CABINET_HALF_M * scale));
        b.rrect(
            b.snap_rect(cab),
            s.r(6.0),
            Fill::V(shade(t.ink, 0.06), shade(t.ink, 0.02)),
        );
        b.rrect_stroke(b.snap_rect(cab), s.r(6.0), b.px(), faint(s, 0.55));
        // The drum: a disc with its scoop toward its angle.
        let dr = DRUM_RADIUS_M * scale;
        b.circle(c, dr, Fill::Solid(shade(t.ink, 0.10)));
        b.circle_stroke(c, dr, b.px() * 1.5, faint(s, 0.6));
        let da = rotors.drum_angle * TAU;
        let mut scoop = [Pos2::ZERO; 14];
        scoop[0] = c;
        for (i, p) in scoop.iter_mut().enumerate().skip(1) {
            let a = da - 0.45 + 0.9 * (i - 1) as f32 / 12.0;
            *p = to_screen(
                c,
                scale,
                DRUM_RADIUS_M * 0.96 * a.cos(),
                DRUM_RADIUS_M * 0.96 * a.sin(),
            );
        }
        for w in scoop[1..].windows(2) {
            b.polygon(&[c, w[0], w[1]], with_alpha(s.hue, 0.35));
        }
        b.polyline(&scoop[1..], s.s(2.0), s.hue);
        // The horn: the live bell toward its angle, the dummy opposite.
        let ha = rotors.horn_angle * TAU;
        let along = |len: f32, off: f32| {
            let (x, y) = (len * ha.cos(), len * ha.sin());
            to_screen(c, scale, x - off * ha.sin(), y + off * ha.cos())
        };
        b.segment(
            c,
            along(-HORN_RADIUS_M * 0.85, 0.0),
            s.s(3.0),
            t.text_dim,
            true,
        );
        b.segment(c, along(HORN_RADIUS_M * 0.6, 0.0), s.s(4.0), s.hue, true);
        b.polygon(
            &[
                along(HORN_RADIUS_M * 0.55, 0.012),
                along(HORN_RADIUS_M, 0.06),
                along(HORN_RADIUS_M, -0.06),
                along(HORN_RADIUS_M * 0.55, -0.012),
            ],
            s.hue,
        );
        b.circle(c, s.s(3.5), Fill::Solid(t.text));
        // The brake's parking tick.
        b.segment(
            park(HORN_RADIUS_M - 0.01),
            pk,
            s.s(if brake_hot { 3.5 } else { 2.5 }),
            t.danger,
            true,
        );
        // The microphones, with the lines they hear along.
        for (k, (p, hot)) in mics.iter().enumerate() {
            b.segment(c, *p, b.px(), faint(s, 0.25), false);
            b.circle(*p, s.s(if *hot { 6.5 } else { 5.0 }), Fill::Solid(t.text));
            if *hot {
                b.circle_stroke(*p, s.s(8.0), s.s(1.5), s.hue);
            }
            let lr = Rect::from_center_size(
                *p + Vec2::new(0.0, s.s(13.0)),
                Vec2::new(s.s(16.0), s.s(12.0)),
            );
            let _ = ktext::paint(
                b,
                lr,
                &Text::new(if k == 0 { "L" } else { "R" }, Role::Micro, t.text_dim),
                s.tk(),
            );
        }
        // Readouts: speeds now, and where they are heading.
        let (ht, dt) = targets(r);
        let lines = [
            tf(
                "organ.horn_now",
                &[
                    ("rpm", &format!("{:.0}", rotors.horn_rpm)),
                    ("to", &format!("{ht:.0}")),
                ],
            ),
            tf(
                "organ.drum_now",
                &[
                    ("rpm", &format!("{:.0}", rotors.drum_rpm)),
                    ("to", &format!("{dt:.0}")),
                ],
            ),
        ];
        let row_h = s.s(15.0);
        let x0 = d.left() + s.s(8.0);
        let half_w = (d.width() - s.s(16.0)) * 0.5;
        for (i, line) in lines.iter().enumerate() {
            let y = d.bottom() - s.s(4.0) - (2 - i) as f32 * row_h;
            let lr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(half_w, row_h));
            label(
                b,
                lr,
                line,
                Role::Label,
                if i == 0 { t.text } else { t.text_dim },
                s,
            );
        }
        let right = Rect::from_min_max(
            Pos2::new(d.center().x, d.bottom() - s.s(4.0) - row_h),
            Pos2::new(d.right() - s.s(8.0), d.bottom() - s.s(4.0)),
        );
        if !o.rotary_on {
            let top = Rect::from_min_max(
                Pos2::new(d.left() + s.s(8.0), d.top() + s.s(4.0)),
                Pos2::new(d.right() - s.s(8.0), d.top() + s.s(4.0) + row_h),
            );
            let _ = ktext::paint(
                b,
                top,
                &Text::key("organ.rotary_bypassed", Role::Label, t.danger)
                    .caps(false)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
        if dist > 0.9 {
            let mut v = String::with_capacity(8);
            readout("rot.dist", "cm", r.mic_distance_cm, &mut v);
            let _ = ktext::paint(
                b,
                right,
                &Text::new(&v, Role::Label, t.text_dim)
                    .caps(false)
                    .align(Align2::RIGHT_CENTER),
                s.tk(),
            );
        }
    });
}

/// rpm against time for a change of speed from `from` to `to` with time constant `tau`.
fn lag(from: f32, to: f32, tau: f32, t: f32) -> f32 {
    to + (from - to) * (-t / tau.max(1e-3)).exp()
}

/// The spin-up (or spin-down) under way in the display `d`: both rotors' rpm over 8 s.
pub fn ramp(b: &mut Brush, o: &OrganSettings, rotors: &Rotors, d: Rect, s: &Style) {
    let t = &s.t;
    let r = &o.rotary;
    let fast = r.speed == RotorSpeed::Fast;
    let secs = 8.0;
    let max_rpm = r.horn_fast_rpm.max(r.drum_fast_rpm) * 1.08;
    let head = s.s(18.0);
    let plot = Rect::from_min_max(
        Pos2::new(d.left() + s.s(10.0), d.top() + head),
        Pos2::new(d.right() - s.s(10.0), d.bottom() - s.s(8.0)),
    );
    let at = |tt: f32, rpm: f32| {
        Pos2::new(
            plot.left() + tt / secs * plot.width(),
            plot.bottom() - rpm / max_rpm * plot.height(),
        )
    };
    for sec in 0..=8 {
        let x = b.snap_mid(plot.left() + sec as f32 / secs * plot.width());
        b.vline(x, plot.top(), plot.bottom(), b.px(), t.grid);
    }
    b.hline(
        plot.left(),
        plot.right(),
        plot.bottom(),
        b.px(),
        t.grid_strong,
    );
    let title = tf("organ.ramp_title", &[("s", &format!("{secs:.0}"))]);
    label(
        b,
        Rect::from_min_size(
            Pos2::new(plot.left(), d.top() + s.s(2.0)),
            Vec2::new(plot.width(), head - s.s(2.0)),
        ),
        &title,
        Role::Label,
        t.text_dim,
        s,
    );
    let pick = |slow: f32, fast_v: f32| if fast { (slow, fast_v) } else { (fast_v, slow) };
    let rows = [
        (
            pick(r.horn_slow_rpm, r.horn_fast_rpm),
            if fast { r.horn_up_s } else { r.horn_down_s },
            s.hue,
            rotors.horn_rpm,
            "organ.horn",
        ),
        (
            pick(r.drum_slow_rpm, r.drum_fast_rpm),
            if fast { r.drum_up_s } else { r.drum_down_s },
            t.text,
            rotors.drum_rpm,
            "organ.drum",
        ),
    ];
    let mut pts = [Pos2::ZERO; 65];
    for (i, ((from, to), tau, color, now, name)) in rows.into_iter().enumerate() {
        for (k, p) in pts.iter_mut().enumerate() {
            let tt = secs * k as f32 / 64.0;
            *p = at(tt, lag(from, to, tau, tt));
        }
        b.polyline(&pts, s.s(2.0), color);
        // The time constant: 63 % of the way.
        let p63 = at(tau.min(secs), lag(from, to, tau, tau));
        b.circle_stroke(p63, s.s(3.5), s.s(1.5), color);
        let mut secs_text = String::with_capacity(12);
        readout("", "s", tau, &mut secs_text);
        let v = tf(
            "organ.ramp_point",
            &[("name", &t_name(name)), ("time", &secs_text)],
        );
        // The key, clear of both curves (round 26: beside the ring it sat on the other
        // rotor's curve): in the corner the curves leave empty — top right while they fall
        // (spinning down, they end low), bottom right while they rise.
        let line = s.s(16.0);
        let w = s.s(170.0);
        let top = if fast {
            plot.bottom() - s.s(6.0) - line * (2 - i) as f32
        } else {
            plot.top() + s.s(4.0) + line * i as f32
        };
        let lr = Rect::from_min_size(
            Pos2::new(plot.right() - s.s(8.0) - w, top),
            Vec2::new(w, s.s(14.0)),
        )
        .intersect(plot);
        b.circle_stroke(
            Pos2::new(lr.left() - s.s(8.0), lr.center().y),
            s.s(3.5),
            s.s(1.5),
            color,
        );
        label(b, lr, &v, Role::Label, color, s);
        // Where the rotor is now, on the curve.
        if (to - from).abs() > 1.0 {
            let frac = ((now - from) / (to - from)).clamp(0.0, 0.9999);
            let tnow = -tau * (1.0 - frac).ln();
            if tnow <= secs {
                b.circle(at(tnow, now), s.s(4.0), Fill::Solid(color));
            }
        }
    }
}

fn t_name(key: &str) -> String {
    t(key)
}

/// The scanner in the display `d`: the line's taps (0.11 ms apart), the span the depth
/// sweeps, the pickup where the engine says it is.
pub fn scanner(b: &mut Brush, o: &OrganSettings, position: f32, d: Rect, s: &Style) {
    let t = &s.t;
    let head = s.s(16.0);
    let on = o.vib_upper || o.vib_lower;
    // When no manual goes through it, a note takes the bottom row and the line moves up.
    let foot = if on { 0.0 } else { s.s(14.0) };
    let y = d.top() + head + (d.height() - head - foot) * 0.5;
    let x0 = d.left() + s.s(14.0);
    let x1 = d.right() - s.s(14.0);
    let dx = (x1 - x0) / (TAPS - 1) as f32;
    let span = SPAN[o.vib_mode.depth()];
    let span_rect = Rect::from_min_max(
        Pos2::new(x0 - s.s(5.0), y - s.s(8.0)),
        Pos2::new(x0 + (span - 1) as f32 * dx + s.s(5.0), y + s.s(8.0)),
    );
    b.rrect(span_rect, s.r(4.0), Fill::Solid(with_alpha(s.hue, 0.22)));
    b.hline(x0, x1, y, b.px(), t.text_dim);
    for i in 0..TAPS {
        b.circle(
            Pos2::new(x0 + i as f32 * dx, y),
            s.s(2.5),
            Fill::Solid(t.text_dim),
        );
    }
    if on {
        let px = x0 + position.clamp(0.0, 1.0) * (x1 - x0);
        b.segment(
            Pos2::new(px, y - s.s(11.0)),
            Pos2::new(px, y + s.s(11.0)),
            s.s(2.5),
            s.hue,
            true,
        );
    }
    let title = tf(
        "organ.scanner_title",
        &[
            ("mode", &o.vib_mode.label()),
            ("ms", &format!("{:.2}", (span - 1) as f32 * TAP_MS)),
            ("hz", &format!("{:.2}", o.vib_rate_hz)),
        ],
    );
    let head_r = Rect::from_min_size(
        Pos2::new(d.left() + s.s(8.0), d.top() + s.s(2.0)),
        Vec2::new(d.width() - s.s(16.0), head),
    );
    label(b, head_r, &title, Role::Label, t.text_dim, s);
    if !on {
        let r = Rect::from_min_max(
            Pos2::new(d.left() + s.s(8.0), d.bottom() - s.s(15.0)),
            Pos2::new(d.right() - s.s(8.0), d.bottom() - s.s(1.0)),
        );
        let _ = ktext::paint(
            b,
            r,
            &Text::key("organ.vib_off", Role::Label, t.text_dim)
                .caps(false)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    }
}

/// The percussion's decay in the display `d`: level (dB) against time to −60 dB, and the
/// envelope now.
pub fn percussion(b: &mut Brush, o: &OrganSettings, env: f32, d: Rect, s: &Style) {
    let t = &s.t;
    let t60 = if o.perc_slow {
        o.perc_slow_ms
    } else {
        o.perc_fast_ms
    } * 0.001;
    let secs = t60 * 1.1;
    let head = s.s(16.0);
    let plot = Rect::from_min_max(
        Pos2::new(d.left() + s.s(10.0), d.top() + head + s.s(2.0)),
        Pos2::new(d.right() - s.s(10.0), d.bottom() - s.s(6.0)),
    );
    let level_db = if o.perc_soft { o.perc_soft_db } else { 0.0 };
    let at = |tt: f32, db: f32| {
        Pos2::new(
            plot.left() + tt / secs * plot.width(),
            plot.top() + (-db / 66.0).clamp(0.0, 1.0) * plot.height(),
        )
    };
    for db in [-20.0, -40.0, -60.0] {
        b.hline(plot.left(), plot.right(), at(0.0, db).y, b.px(), t.grid);
    }
    let color = if o.perc_on { s.hue } else { t.text_dim };
    let a = at(0.0, level_db);
    let z = at(t60, level_db - 60.0);
    let fill = [a, z];
    b.area(&fill, plot.bottom(), with_alpha(color, 0.16));
    b.segment(at(0.0, -66.0), a, s.s(2.0), color, false);
    b.segment(a, z, s.s(2.0), color, true);
    if env > 0.0 && o.perc_on {
        let db = 20.0 * env.log10();
        let tt = (-db / 60.0) * t60;
        b.circle(
            at(tt, level_db + db),
            s.s(4.0),
            Fill::Solid(mix(s.hue, t.text, 0.2)),
        );
    }
    let mut v = String::with_capacity(12);
    readout("", "ms", t60 * 1000.0, &mut v);
    let title = tf("organ.perc_title", &[("t", &v)]);
    let head_r = Rect::from_min_size(
        Pos2::new(d.left() + s.s(8.0), d.top() + s.s(2.0)),
        Vec2::new(d.width() - s.s(16.0), head),
    );
    label(b, head_r, &title, Role::Label, t.text_dim, s);
}

fn is_black(k: u8) -> bool {
    matches!(k % 12, 1 | 3 | 6 | 8 | 10)
}

/// The keyboard split into its divisions in the display `d`, boundaries draggable.
pub fn splits(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
) {
    let (lo, hi) = (36u8, 96u8);
    let names_h = s.s(18.0);
    let keys = Rect::from_min_max(
        Pos2::new(d.left() + s.s(8.0), d.top() + names_h + s.s(2.0)),
        Pos2::new(d.right() - s.s(8.0), d.bottom() - s.s(8.0)),
    );
    let whites: Vec<u8> = (lo..=hi).filter(|k| !is_black(*k)).collect();
    let w = keys.width() / whites.len() as f32;
    let x_of = |k: u8| {
        let i = whites.iter().position(|x| *x >= k).unwrap_or(whites.len());
        keys.left() + i as f32 * w
    };
    // Boundaries first: a drag moves them this frame.
    let mut pedal = spec.get("split.pedal") as u8;
    let mut upper = spec.get("split.upper") as u8;
    let mut hot = [false; 2];
    for (i, key) in ["split.pedal", "split.upper"].into_iter().enumerate() {
        let value = if i == 0 { pedal } else { upper };
        let x = if value > 0 {
            x_of(value.clamp(lo, hi))
        } else {
            keys.left()
        };
        let handle = Rect::from_center_size(
            Pos2::new(x, keys.center().y),
            Vec2::new(s.s(12.0), keys.height() + s.s(8.0)),
        );
        let resp = ui.interact(
            handle,
            egui::Id::new(("organ-split", track.0, key)),
            Sense::click_and_drag(),
        );
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
            let wi = ((p.x - keys.left()) / w)
                .round()
                .clamp(0.0, whites.len() as f32 - 1.0) as usize;
            let k = whites[wi];
            let v = if k <= lo { 0 } else { k };
            set(app, track, key, v as f32, true);
            if i == 0 {
                pedal = v;
            } else {
                upper = v;
            }
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        if resp.double_clicked() {
            set(app, track, key, 0.0, false);
        }
        hot[i] = resp.hovered() || resp.dragged();
        if hot[i] {
            let name = if value > 0 {
                cantus_core::pitch_name(value)
            } else {
                t("organ.split_none")
            };
            let tip_key = if i == 0 {
                "organ.split_pedal_tip"
            } else {
                "organ.split_upper_tip"
            };
            hint::set_text(ui.ctx(), &tf(tip_key, &[("key", &name)]), "", Caps::PARAM);
        }
    }
    with_brush(&ui.painter_at(d), |b| {
        let tk = &s.t;
        let zone = |k: u8| {
            if pedal > 0 && k < pedal {
                mix(tk.key_white, tk.text_dim, 0.45)
            } else if upper > 0 && k < upper {
                mix(tk.key_white, s.hue, 0.25)
            } else {
                mix(tk.key_white, s.hue, 0.5)
            }
        };
        for (i, k) in whites.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(keys.left() + i as f32 * w, keys.top()),
                Vec2::new(w - b.px(), keys.height()),
            );
            b.rrect(
                b.snap_rect(r),
                s.r(1.5),
                Fill::V(zone(*k), shade(zone(*k), -0.06)),
            );
        }
        for (i, k) in whites.iter().enumerate() {
            if *k < hi && is_black(k + 1) {
                let r = Rect::from_min_size(
                    Pos2::new(keys.left() + (i as f32 + 0.69) * w, keys.top()),
                    Vec2::new(w * 0.62, keys.height() * 0.6),
                );
                let c = mix(tk.key_black, zone(k + 1), 0.25);
                b.rrect(b.snap_rect(r), s.r(1.0), Fill::Solid(c));
            }
        }
        // The divisions' names over their keys.
        let p_end = if pedal > 0 { pedal.clamp(lo, hi) } else { lo };
        let u_start = if upper > 0 {
            upper.clamp(p_end, hi)
        } else {
            p_end
        };
        for (from, to, key) in [
            (lo, p_end, "organ.pedal"),
            (p_end, u_start, "organ.lower"),
            (u_start, hi, "organ.upper"),
        ] {
            if to > from {
                let r = Rect::from_min_max(
                    Pos2::new(x_of(from), d.top() + s.s(2.0)),
                    Pos2::new(x_of(to).max(x_of(from) + 1.0), d.top() + names_h),
                );
                let _ = ktext::paint(
                    b,
                    r,
                    &Text::key(key, Role::Micro, tk.text)
                        .caps(true)
                        .align(Align2::CENTER_CENTER)
                        .policy(Policy::Abbreviate),
                    s.tk(),
                );
            }
        }
        for (i, value) in [pedal, upper].into_iter().enumerate() {
            let x = if value > 0 {
                x_of(value.clamp(lo, hi))
            } else {
                keys.left()
            };
            let color = if value > 0 { tk.text } else { tk.grid_strong };
            let w = s.s(if hot[i] { 3.5 } else { 2.5 });
            b.segment(
                Pos2::new(x, keys.top() - s.s(3.0)),
                Pos2::new(x, keys.bottom() + s.s(3.0)),
                w,
                color,
                true,
            );
            if hot[i] {
                b.circle_stroke(
                    Pos2::new(x, keys.top() - s.s(3.0)),
                    s.s(4.0),
                    s.s(1.5),
                    s.hue,
                );
            }
        }
    });
}
