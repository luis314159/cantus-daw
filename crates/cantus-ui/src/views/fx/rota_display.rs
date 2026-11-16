//! Rota's display (ADR 0136, provisional): the rotary cabinet three ways on a night-dark
//! screen — **cut away** from the front (the horn's two bells turning on their shaft in the
//! upper chamber behind the louvres, the motors and the amp's valve on the shelf, the drum's
//! scoop sweeping round the lower chamber over the bass speaker), **from above** (the horn's
//! arm and the drum's opening passing the two microphones, which are dragged round and away)
//! and **the ramps** (each rotor's speed since the switch was thrown, a dot where it is now).
//! The speed switch is lit along the top: Stop, Slow, Fast. Everything turns from the
//! engine's telemetry (`cantus_engine::fx::rota`): the angle each rotor has, its speed, the
//! switch as notes have left it.

use super::super::arca::valve2;
use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::skin::cabinets as pal;
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_engine::fx::rota as tel;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};
use std::f32::consts::{PI, TAU};

/// The switch's three places' names.
pub const SPEEDS: [&str; 3] = ["fx.rota.speed.0", "fx.rota.speed.1", "fx.rota.speed.2"];

/// The telemetry, read.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub seen: bool,
    /// Each rotor's angle (turns) and speed (rpm): the horn, the drum.
    pub horn: (f32, f32),
    pub drum: (f32, f32),
    /// The switch as it is now (notes may have thrown it): 0 stop, 1 slow, 2 fast.
    pub speed: usize,
}

impl Reading {
    pub fn of(t: &[f32; 64]) -> Reading {
        Reading {
            seen: t[tel::T_SAMPLE_RATE] > 0.0,
            horn: (t[tel::T_HORN], t[tel::T_HORN + 1]),
            drum: (t[tel::T_DRUM], t[tel::T_DRUM + 1]),
            speed: (t[tel::T_SPEED].round().max(0.0) as usize).min(2),
        }
    }
}

/// What the display remembers: the rotors as last drawn (still at rest), the microphone
/// being dragged.
#[derive(Debug)]
pub struct View {
    pub horn: (f32, f32),
    pub drum: (f32, f32),
    drag: Option<usize>,
}

impl Default for View {
    fn default() -> Self {
        // At rest the rotors stand where a picture reads them best: the horn's bell a little
        // toward the listener, the drum's scoop to the left.
        View {
            horn: (0.07, 0.0),
            drum: (0.62, 0.0),
            drag: None,
        }
    }
}

impl View {
    pub fn update(&mut self, r: &Reading, _dt: f32) {
        if r.seen {
            self.horn = r.horn;
            self.drum = r.drum;
        }
    }

    pub fn seed(&mut self, r: &Reading, _spec: &EffectSpec) {
        self.update(r, 0.0);
    }
}

struct Parts {
    band: Rect,
    cut: Rect,
    above: Rect,
    ramps: Rect,
}

fn parts(r: Rect, k: f32) -> Parts {
    let band = Rect::from_min_size(r.min, vec2(r.width(), 34.0 * k));
    let body = Rect::from_min_max(pos2(r.left(), band.bottom()), r.max);
    let cut_w = (body.height() * 0.78).min(body.width() * 0.32);
    let above_w = (body.width() - cut_w) * 0.46;
    Parts {
        band,
        cut: Rect::from_min_size(body.min, vec2(cut_w, body.height())),
        above: Rect::from_min_size(
            pos2(body.left() + cut_w, body.top()),
            vec2(above_w, body.height()),
        ),
        ramps: Rect::from_min_max(pos2(body.left() + cut_w + above_w, body.top()), body.max),
    }
}

/// The switch's places along the band.
fn speed_rects(ctx: &egui::Context, band: Rect, k: f32) -> [Rect; 3] {
    let mut x = band.left() + 12.0 * k;
    std::array::from_fn(|i| {
        let t = Text::key(SPEEDS[i], Role::Control, Color32::TRANSPARENT);
        let w = ktext::measure(ctx, &t, k).x + 34.0 * k;
        let r = Rect::from_min_size(
            pos2(x, band.top() + 5.0 * k),
            vec2(w.max(72.0 * k), band.height() - 10.0 * k),
        );
        x = r.right() + 6.0 * k;
        r
    })
}

/// The display in `r`.
pub fn show(
    host: &mut dyn FxHost,
    v: &mut View,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    rd: &Reading,
) {
    let k = s.k;
    let p = parts(r, k);
    let ink = Ink::of(s);
    let (rs, _) = cantus_engine::fx::rota::settings(host.spec());
    let speeds = speed_rects(ui.ctx(), p.band, k);
    speed_edit(host, ui, s, &speeds);
    let mics = mic_spots(p.above, &rs, k);
    mic_drag(host, v, ui, p.above, &mics, k);
    let now = if rd.seen {
        rd.speed
    } else {
        (host.spec().get("speed").round().max(0.0) as usize).min(2)
    };
    let horn_c = ink.hue;
    let drum_c = ink.voice(ink.hue, true);
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        paint_band(b, &ink, &speeds, p.band, now, rd, &rs, k, horn_c, drum_c);
        paint_cut(b, s, &ink, p.cut, v, &rs, k, horn_c, drum_c);
        paint_above(b, &ink, p.above, v, &rs, &mics, k, horn_c, drum_c);
        paint_ramps(b, &ink, p.ramps, now, rd, &rs, k, horn_c, drum_c);
        // v2 — the two instruments set in walnut, as windows in the same cabinet.
        for pr in [p.above, p.ramps] {
            bezel(b, pr.shrink2(vec2(6.0 * k, 4.0 * k)), k);
        }
        b.hline(
            r.left() + 10.0 * k,
            r.right() - 10.0 * k,
            p.band.bottom(),
            0.7 * k,
            ink.grid,
        );
        canvas::edge_b(b, r, &ink);
    });
}

/// The switch: a click throws it (its menu offers automation and MIDI learn).
fn speed_edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rects: &[Rect; 3]) {
    let now = host.spec().get("speed").round().max(0.0) as usize;
    let name = frame::tr("fx.rota.speed");
    for (i, r) in rects.iter().enumerate() {
        let resp = ui.interact(*r, ui.id().with(("rota-speed", i)), Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, "speed", name, &resp);
        if resp.clicked() && !learnt && i != now {
            host.set("speed", i as f32, None);
        }
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr(SPEEDS[i]),
                frame::tr("rota.d.speed_tip"),
                Caps::NONE,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_band(
    b: &mut Brush,
    ink: &Ink,
    rects: &[Rect; 3],
    band: Rect,
    now: usize,
    rd: &Reading,
    rs: &cantus_dsp::rota::RotaSettings,
    k: f32,
    horn_c: Color32,
    drum_c: Color32,
) {
    for (i, r) in rects.iter().enumerate() {
        let on = i == now;
        let col = if on { ink.hue } else { ink.dim };
        if on {
            b.rrect(*r, 3.0 * k, Fill::Solid(with_alpha(ink.hue, 0.12)));
        }
        b.rrect_stroke(
            *r,
            3.0 * k,
            0.7 * k,
            if on { ink.hue } else { ink.grid_strong },
        );
        // The lamp: lit where the switch is.
        let lamp = pos2(r.left() + 12.0 * k, r.center().y);
        b.circle(
            lamp,
            3.4 * k,
            Fill::Solid(with_alpha(col, if on { 0.95 } else { 0.25 })),
        );
        if on {
            b.circle(lamp, 6.5 * k, Fill::Solid(with_alpha(col, 0.18)));
        }
        canvas::put(
            b,
            pos2(r.left() + 22.0 * k, r.center().y),
            Align2::LEFT_CENTER,
            &Text::key(SPEEDS[i], Role::Control, col),
            k,
            None,
        );
    }
    // At the right: each rotor's speed now (or the switch's, at rest).
    let (h, d) = if rd.seen {
        (rd.horn.1, rd.drum.1)
    } else {
        match now {
            2 => (rs.horn_fast_rpm, rs.drum_fast_rpm),
            1 => (rs.horn_slow_rpm, rs.drum_slow_rpm),
            _ => (0.0, 0.0),
        }
    };
    let unit = frame::tr("rota.d.rpm");
    let x = band.right() - 12.0 * k;
    let t = canvas::tag(
        b,
        pos2(x, band.center().y),
        Align2::RIGHT_CENTER,
        frame::tr("rota.d.drum"),
        drum_c,
        k,
        None,
        format_args!("{d:.0} {unit}"),
    );
    canvas::tag(
        b,
        pos2(t.left() - 18.0 * k, band.center().y),
        Align2::RIGHT_CENTER,
        frame::tr("rota.d.horn"),
        horn_c,
        k,
        None,
        format_args!("{h:.0} {unit}"),
    );
}

/// A filled ellipse.
fn ellipse(b: &mut Brush, c: Pos2, rx: f32, ry: f32, col: Color32) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let clip = Rect::from_center_size(c, vec2(rx * 2.2, ry * 2.2));
    b.radial(clip, c, vec2(rx, ry), &[(0.0, col), (1.0, col)], 16);
}

/// An ellipse's outline (or a part of it, `a0` … `a1`), `n` segments.
#[allow(clippy::too_many_arguments)]
fn ellipse_stroke(
    b: &mut Brush,
    c: Pos2,
    rx: f32,
    ry: f32,
    a0: f32,
    a1: f32,
    w: f32,
    col: Color32,
) {
    let mut pts = [Pos2::ZERO; 49];
    for (j, p) in pts.iter_mut().enumerate() {
        let a = a0 + (a1 - a0) * j as f32 / 48.0;
        *p = c + vec2(a.cos() * rx, a.sin() * ry);
    }
    b.polyline(&pts, w, col);
}

/// The cabinet cut away from the front (v2, the owner's "richer"): its walnut case with a
/// finer grain and a moulded top; the front panel left standing at both sides — louvres
/// above, grille cloth below — the middle cut away to show the louvred upper chamber with
/// the horn turning (blurred as it spins up), the shelf with the motors and the amp's
/// valves at the back, the drum with its staves, its scoop and the baffle inside it (the
/// scoop smearing at speed), the bass speaker under it firing up into it.
#[allow(clippy::too_many_arguments)]
fn paint_cut(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    v: &View,
    rs: &cantus_dsp::rota::RotaSettings,
    k: f32,
    horn_c: Color32,
    drum_c: Color32,
) {
    let _ = s;
    let h = r.height() - 52.0 * k;
    let w = (r.width() - 40.0 * k).min(h * 0.64);
    let cab = Rect::from_min_size(pos2(r.center().x - w * 0.5, r.top() + 24.0 * k), vec2(w, h));
    // The walnut case: its body and a fine grain (many thin lines, wavering).
    b.rrect(
        cab,
        5.0 * k,
        Fill::V(pal::WALNUT_LIGHT.rgb(), pal::WALNUT.rgb()),
    );
    for i in 0..22 {
        let x = cab.left() + cab.width() * (0.02 + 0.045 * i as f32);
        let mut pts = [Pos2::ZERO; 25];
        for (j, p) in pts.iter_mut().enumerate() {
            let t = j as f32 / 24.0;
            let wob =
                (t * 7.0 + i as f32 * 2.3).sin() * 1.2 * k + (t * 23.0 + i as f32).sin() * 0.4 * k;
            *p = pos2(x + wob, cab.top() + cab.height() * t);
        }
        let a = if i % 3 == 0 { 0.30 } else { 0.16 };
        b.polyline(&pts, 0.4 * k, with_alpha(pal::WALNUT_DARK.rgb(), a));
    }
    // The moulded top: an overhanging lid, a bead under it, its own grain along it.
    let lid = Rect::from_min_max(
        pos2(cab.left() - 6.0 * k, cab.top() - 10.0 * k),
        pos2(cab.right() + 6.0 * k, cab.top() + 2.0 * k),
    );
    b.rrect(
        lid,
        3.0 * k,
        Fill::V(pal::WALNUT_LIGHT.rgb(), pal::WALNUT_DARK.rgb()),
    );
    for j in 0..4 {
        let y = lid.top() + 2.5 * k + j as f32 * 2.4 * k;
        b.hline(
            lid.left() + 4.0 * k,
            lid.right() - 4.0 * k,
            y,
            0.4 * k,
            with_alpha(pal::WALNUT_DARK.rgb(), 0.3),
        );
    }
    b.hline(
        lid.left() + 3.0 * k,
        lid.right() - 3.0 * k,
        lid.top() + 0.6 * k,
        0.6 * k,
        with_alpha(pal::WALNUT_LIGHT.rgb(), 0.9),
    );
    b.hline(
        cab.left(),
        cab.right(),
        lid.bottom() + 1.5 * k,
        1.2 * k,
        pal::WALNUT_DARK.rgb(),
    );
    let plinth = Rect::from_min_max(
        pos2(cab.left() + 2.0 * k, cab.bottom() - 12.0 * k),
        pos2(cab.right() - 2.0 * k, cab.bottom()),
    );
    b.rrect(plinth, 2.0 * k, Fill::Solid(pal::WALNUT_DARK.rgb()));
    // Inside, in shadow; the shelf parts it.
    let inner = Rect::from_min_max(
        pos2(cab.left() + 9.0 * k, cab.top() + 10.0 * k),
        pos2(cab.right() - 9.0 * k, cab.bottom() - 18.0 * k),
    );
    b.rrect(
        inner,
        2.0 * k,
        Fill::V(pal::PLYWOOD.rgb(), pal::WALNUT_DARK.rgb()),
    );
    let upper = Rect::from_min_max(
        inner.min,
        pos2(inner.right(), inner.top() + inner.height() * 0.42),
    );
    let shelf = Rect::from_min_max(
        pos2(inner.left(), upper.bottom()),
        pos2(inner.right(), upper.bottom() + inner.height() * 0.09),
    );
    let lower = Rect::from_min_max(pos2(inner.left(), shelf.bottom()), inner.max);
    // The amp's valves at the back of the upper chamber, on their chassis.
    let amp = Rect::from_min_max(
        pos2(
            cab.right() - cab.width() * 0.16 - upper.width() * 0.30,
            shelf.top() - 6.0 * k,
        ),
        pos2(cab.right() - cab.width() * 0.16 - 6.0 * k, shelf.top()),
    );
    b.rect(amp, Fill::V(pal::CASTING.rgb(), pal::CHASSIS.rgb()));
    let glow = rs.drive.clamp(0.0, 1.0);
    let vh = upper.height() * 0.30;
    for (i, power) in [(0usize, false), (1, false), (2, true)] {
        let vw = vh * if power { 0.40 } else { 0.32 };
        let vhh = vh * if power { 1.0 } else { 0.78 };
        let x = amp.left() + 4.0 * k + i as f32 * (amp.width() - 8.0 * k) / 3.0;
        let vr = Rect::from_min_max(pos2(x, amp.top() - vhh), pos2(x + vw, amp.top()));
        valve2::valve(b, k, vr, 0.55, glow, power);
    }
    // The horn: its orbit, the blur of where its bells were, the far bell, the shaft, the
    // near bell.
    let hub = pos2(upper.center().x, upper.top() + upper.height() * 0.42);
    // The bells turn inside the chamber, clear of the louvres standing at its sides.
    let reach = (cab.width() * 0.5 - cab.width() * 0.16 - 9.0 * k) * 0.70;
    let fast = (v.horn.1 / 300.0).clamp(0.0, 1.0);
    ellipse_stroke(
        b,
        hub,
        reach,
        reach * 0.16,
        0.0,
        TAU,
        0.6 * k,
        with_alpha(horn_c, 0.14 + 0.3 * fast),
    );
    let theta = TAU * v.horn.0;
    // A bell sweeps this far in a sixtieth of a second: its trail behind it.
    let sweep = (v.horn.1 / 60.0 * TAU / 60.0).min(1.2);
    if sweep > 0.05 {
        // Streaks along the mouth's path, from where it was to where it is.
        for (base, col) in [(theta, horn_c), (theta + PI, pal::HORN_LIGHT.rgb())] {
            for f in [-0.3f32, 0.0, 0.3] {
                let mut pts = [Pos2::ZERO; 13];
                for (j, p) in pts.iter_mut().enumerate() {
                    let a = base - sweep * (1.0 - j as f32 / 12.0);
                    *p = hub
                        + vec2(
                            reach * a.sin(),
                            -a.cos() * 3.0 * k + f * upper.height() * 0.40,
                        );
                }
                b.polyline(&pts, 0.8 * k, with_alpha(col, 0.45));
            }
        }
        for j in (1..=6).rev() {
            let a = theta - sweep * j as f32 / 6.0;
            let alpha = 0.55 * (1.0 - j as f32 / 7.0);
            for (aa, real) in [(a, true), (a + PI, false)] {
                horn(
                    b,
                    hub,
                    reach,
                    aa,
                    upper.height() * 0.40,
                    real,
                    horn_c,
                    k,
                    alpha,
                );
            }
        }
    }
    let mut bells = [(theta, true), (theta + PI, false)];
    bells.sort_by(|a, b| a.0.cos().total_cmp(&b.0.cos()));
    let shaft_top = hub - vec2(0.0, 4.0 * k);
    for (i, (a, real)) in bells.iter().enumerate() {
        if i == 1 {
            b.segment(
                shaft_top,
                pos2(hub.x, shelf.top()),
                2.0 * k,
                pal::CHROME_SHADE.rgb(),
                false,
            );
            b.circle(
                hub,
                4.0 * k,
                Fill::V(pal::CHROME.rgb(), pal::CHROME_SHADE.rgb()),
            );
        }
        horn(
            b,
            hub,
            reach,
            *a,
            upper.height() * 0.40,
            *real,
            horn_c,
            k,
            1.0,
        );
    }
    // The shelf: the board, the horn's driver, the motors with their belts.
    b.rrect(
        shelf,
        1.5 * k,
        Fill::V(pal::WALNUT.rgb(), pal::WALNUT_DARK.rgb()),
    );
    let driver =
        Rect::from_center_size(pos2(hub.x, shelf.top() - 5.0 * k), vec2(18.0 * k, 10.0 * k));
    b.rrect(
        driver,
        2.0 * k,
        Fill::V(pal::CASTING.rgb(), pal::CHROME_SHADE.rgb()),
    );
    for x in [shelf.left() + 18.0 * k, shelf.left() + 38.0 * k] {
        let m = Rect::from_center_size(pos2(x, shelf.top() - 7.0 * k), vec2(15.0 * k, 14.0 * k));
        b.rrect(
            m,
            2.0 * k,
            Fill::V(pal::CASTING.rgb(), pal::CHROME_SHADE.rgb()),
        );
        let band = Rect::from_center_size(m.center(), vec2(m.width(), 4.0 * k));
        b.rect(band, Fill::V(pal::COPPER_LIGHT.rgb(), pal::COPPER.rgb()));
        b.segment(
            pos2(x + 7.0 * k, m.center().y),
            pos2(hub.x - 9.0 * k, driver.center().y),
            0.6 * k,
            with_alpha(pal::GASKET.rgb(), 0.9),
            false,
        );
    }
    // The bass speaker on its board, firing up into the drum; the drum above it.
    let dw = lower.width() * 0.60;
    let dh = lower.height() * 0.42;
    let drum = Rect::from_center_size(
        pos2(lower.center().x, lower.top() + lower.height() * 0.36),
        vec2(dw, dh),
    );
    let ry = dw * 0.09;
    let board_y = drum.bottom() + lower.height() * 0.16;
    b.rect(
        Rect::from_min_max(
            pos2(lower.left() + 2.0 * k, board_y),
            pos2(lower.right() - 2.0 * k, board_y + 5.0 * k),
        ),
        Fill::V(pal::WALNUT.rgb(), pal::WALNUT_DARK.rgb()),
    );
    let wc = pos2(lower.center().x, board_y);
    ellipse(b, wc, dw * 0.46, 6.0 * k, pal::GASKET.rgb());
    ellipse(b, wc, dw * 0.40, 5.0 * k, pal::SURROUND.rgb());
    ellipse(b, wc, dw * 0.34, 4.0 * k, pal::CONE_LIGHT.rgb());
    for f in [0.30f32, 0.22, 0.14] {
        ellipse_stroke(
            b,
            wc,
            dw * f,
            4.0 * k * f / 0.34,
            0.0,
            TAU,
            0.5 * k,
            with_alpha(pal::CONE_SHADE.rgb(), 0.8),
        );
    }
    ellipse(
        b,
        wc - vec2(0.0, 0.6 * k),
        dw * 0.08,
        1.2 * k,
        pal::DUST_CAP_LIGHT.rgb(),
    );
    // The sound rising from it into the drum: faint arcs.
    for j in 1..4 {
        let y = board_y - j as f32 * (board_y - drum.bottom()) / 4.0;
        ellipse_stroke(
            b,
            pos2(wc.x, y),
            dw * (0.28 - 0.04 * j as f32),
            2.0 * k,
            PI,
            TAU,
            0.5 * k,
            with_alpha(drum_c, 0.25),
        );
    }
    // The drum: a cylinder, its staves, its scoop (and its smear at speed).
    b.hrect(
        drum,
        &[
            (0.0, pal::DRUM.rgb()),
            (0.35, pal::DRUM_LIGHT.rgb()),
            (1.0, mix(pal::DRUM.rgb(), pal::WALNUT_DARK.rgb(), 0.5)),
        ],
    );
    for x in [drum.left(), drum.right()] {
        b.vline(
            x,
            drum.top(),
            drum.bottom(),
            0.7 * k,
            with_alpha(pal::CHROME_SHADE.rgb(), 0.9),
        );
    }
    ellipse(
        b,
        pos2(drum.center().x, drum.bottom()),
        dw * 0.5,
        ry,
        pal::DRUM.rgb(),
    );
    let phi = TAU * v.drum.0;
    let dsweep = (v.drum.1 / 60.0 * TAU / 60.0).min(1.0);
    if dsweep > 0.05 {
        for j in (1..=5).rev() {
            scoop(
                b,
                drum,
                phi - dsweep * j as f32 / 5.0,
                drum_c,
                k,
                0.25 * (1.0 - j as f32 / 6.0),
                false,
            );
        }
    }
    scoop(b, drum, phi, drum_c, k, 1.0, true);
    ellipse(
        b,
        pos2(drum.center().x, drum.top()),
        dw * 0.5,
        ry,
        pal::DRUM_LIGHT.rgb(),
    );
    ellipse_stroke(
        b,
        pos2(drum.center().x, drum.top()),
        dw * 0.5,
        ry,
        0.0,
        TAU,
        0.7 * k,
        with_alpha(pal::CHROME_SHADE.rgb(), 0.9),
    );
    b.segment(
        pos2(drum.center().x, drum.top() - ry),
        pos2(drum.center().x, shelf.bottom()),
        1.6 * k,
        pal::CHROME_SHADE.rgb(),
        false,
    );
    // The front panel left standing at both sides: louvres above, grille cloth below, the
    // cut's raw edge lit.
    let side_w = cab.width() * 0.16;
    for left in [true, false] {
        let x0 = if left {
            cab.left()
        } else {
            cab.right() - side_w
        };
        let panel = Rect::from_min_max(
            pos2(x0, cab.top() + 3.0 * k),
            pos2(x0 + side_w, plinth.top()),
        );
        b.rect(panel, Fill::V(pal::WALNUT_LIGHT.rgb(), pal::WALNUT.rgb()));
        let lv = Rect::from_min_max(
            pos2(panel.left() + 5.0 * k, upper.top() + 4.0 * k),
            pos2(panel.right() - 5.0 * k, upper.bottom() - 4.0 * k),
        );
        b.rect(lv, Fill::Solid(pal::WALNUT_DARK.rgb()));
        let mut y = lv.top() + 3.0 * k;
        while y < lv.bottom() - 2.0 * k {
            b.rect(
                Rect::from_min_max(pos2(lv.left(), y), pos2(lv.right(), y + 2.4 * k)),
                Fill::V(pal::WALNUT_LIGHT.rgb(), pal::SLAT.rgb()),
            );
            b.hline(
                lv.left(),
                lv.right(),
                y + 2.6 * k,
                0.5 * k,
                with_alpha(pal::GASKET.rgb(), 0.9),
            );
            y += 5.0 * k;
        }
        let gr = Rect::from_min_max(
            pos2(panel.left() + 5.0 * k, lower.top() + 6.0 * k),
            pos2(panel.right() - 5.0 * k, lower.bottom() - 6.0 * k),
        );
        b.rect(gr, Fill::Solid(pal::CLOTH.rgb()));
        let mut yy = gr.top() + 1.0 * k;
        while yy < gr.bottom() {
            b.hline(
                gr.left(),
                gr.right(),
                yy,
                0.45 * k,
                with_alpha(pal::CLOTH_THREAD.rgb(), 0.3),
            );
            yy += 1.7 * k;
        }
        let mut xx = gr.left() + 1.0 * k;
        while xx < gr.right() {
            b.vline(
                xx,
                gr.top(),
                gr.bottom(),
                0.4 * k,
                with_alpha(pal::CONE_SHADE.rgb(), 0.25),
            );
            xx += 1.7 * k;
        }
        b.rect(
            Rect::from_min_max(
                pos2(panel.left() + 5.0 * k, shelf.top()),
                pos2(panel.right() - 5.0 * k, shelf.bottom()),
            ),
            Fill::Solid(pal::WALNUT.rgb()),
        );
        let edge = if left { panel.right() } else { panel.left() };
        b.vline(
            edge,
            panel.top(),
            panel.bottom(),
            1.4 * k,
            pal::PLYWOOD_LIGHT.rgb(),
        );
        b.vline(
            edge + if left { 1.2 } else { -1.2 } * k,
            panel.top(),
            panel.bottom(),
            0.6 * k,
            with_alpha(pal::GASKET.rgb(), 0.8),
        );
    }
    // The names, in the rotors' colours.
    canvas::put(
        b,
        pos2(upper.left() + side_w + 8.0 * k, upper.top() + 10.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("rota.d.horn", Role::Micro, horn_c),
        k,
        Some(ink.plate),
    );
    canvas::put(
        b,
        pos2(lower.left() + side_w + 4.0 * k, lower.top() + 10.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("rota.d.drum", Role::Micro, drum_c),
        k,
        Some(ink.plate),
    );
    canvas::put(
        b,
        pos2(r.center().x, r.bottom() - 12.0 * k),
        Align2::CENTER_CENTER,
        &Text::key("rota.d.cut", Role::Micro, ink.dim),
        k,
        None,
    );
}

/// One of the horn's bells at angle `a` (0: toward the listener), seen from the front, at
/// `alpha` (a ghost of its blur below 1): a flared body from the hub to its mouth, the
/// mouth an ellipse as wide as it faces us. The sounding bell's rim in the horn's colour;
/// the other is the dummy that balances it.
#[allow(clippy::too_many_arguments)]
fn horn(
    b: &mut Brush,
    hub: Pos2,
    reach: f32,
    a: f32,
    size: f32,
    real: bool,
    col: Color32,
    k: f32,
    alpha: f32,
) {
    let (sx, depth) = (a.sin(), a.cos());
    let near = depth > 0.0;
    let hm = size * (0.86 + 0.14 * depth);
    let mouth = hub + vec2(reach * sx, -depth * 3.0 * k);
    let body = if near {
        mix(pal::HORN_LIGHT.rgb(), pal::HORN.rgb(), 0.45)
    } else {
        pal::HORN.rgb()
    };
    let pts = [
        hub + vec2(0.0, -2.5 * k),
        mouth + vec2(0.0, -hm * 0.5),
        mouth + vec2(0.0, hm * 0.5),
        hub + vec2(0.0, 2.5 * k),
    ];
    b.polygon(&pts, with_alpha(body, alpha));
    if alpha < 1.0 {
        return;
    }
    b.segment(
        pts[0],
        pts[1],
        0.6 * k,
        with_alpha(pal::HORN_LIGHT.rgb(), 0.8),
        false,
    );
    let rx = hm * 0.5 * depth.abs().max(0.10);
    let inside = if near {
        pal::CONE_SHADE.rgb()
    } else {
        pal::HORN.rgb()
    };
    ellipse(b, mouth, rx, hm * 0.5, inside);
    let rim = if real {
        col
    } else {
        with_alpha(pal::HORN_LIGHT.rgb(), 0.9)
    };
    ellipse_stroke(
        b,
        mouth,
        rx,
        hm * 0.5,
        0.0,
        TAU,
        if real { 1.1 } else { 0.6 } * k,
        rim,
    );
}

/// The drum's staves (when `whole`) and its scoop at `alpha`: the opening dark between two
/// edges in the drum's colour where it faces us, the baffle's curve lit inside it; its
/// outline faint while it is round the back.
fn scoop(b: &mut Brush, drum: Rect, phi: f32, col: Color32, k: f32, alpha: f32, whole: bool) {
    let c = drum.center().x;
    let rad = drum.width() * 0.5;
    let (top, bot) = (
        drum.top() + drum.height() * 0.10,
        drum.bottom() - drum.height() * 0.04,
    );
    if whole {
        for i in 0..32 {
            let a = phi + TAU * i as f32 / 32.0;
            if a.cos() > 0.05 {
                let x = c + rad * a.sin();
                b.vline(
                    x,
                    top,
                    bot,
                    0.5 * k,
                    with_alpha(pal::DRUM_LIGHT.rgb(), a.cos() * 0.35),
                );
            }
        }
    }
    let span = 0.62;
    let front = phi.cos() > 0.0;
    let (mut x0, mut x1) = (f32::MAX, f32::MIN);
    for j in 0..=24 {
        let a = phi - span + 2.0 * span * j as f32 / 24.0;
        if (a.cos() > 0.0) == front {
            let x = c + rad * a.sin();
            x0 = x0.min(x);
            x1 = x1.max(x);
        }
    }
    if x1 <= x0 {
        return;
    }
    let open = Rect::from_min_max(pos2(x0, top), pos2(x1, bot));
    if front {
        b.rect(
            open,
            Fill::Solid(with_alpha(pal::GASKET.rgb(), 0.9 * alpha)),
        );
        if alpha >= 1.0 {
            // The baffle inside: a curved deflector catching the light.
            let mut pts = [Pos2::ZERO; 13];
            for (j, p) in pts.iter_mut().enumerate() {
                let t = j as f32 / 12.0;
                *p = pos2(
                    x0 + (x1 - x0) * t,
                    bot - (bot - top) * (0.15 + 0.6 * (t * PI).sin()),
                );
            }
            b.polyline(&pts, 1.2 * k, with_alpha(pal::DRUM_LIGHT.rgb(), 0.9));
            for x in [x0, x1] {
                b.vline(x, top, bot, 1.1 * k, col);
            }
            b.hline(x0, x1, top, 0.7 * k, with_alpha(col, 0.7));
            b.hline(x0, x1, bot, 0.7 * k, with_alpha(col, 0.7));
        }
    } else if alpha >= 1.0 {
        canvas::dashed(
            b,
            pos2(x0, top),
            pos2(x0, bot),
            0.6 * k,
            with_alpha(col, 0.45),
            3.0 * k,
            3.0 * k,
        );
        canvas::dashed(
            b,
            pos2(x1, top),
            pos2(x1, bot),
            0.6 * k,
            with_alpha(col, 0.45),
            3.0 * k,
            3.0 * k,
        );
    }
}

/// The view from above: the cabinet's centre and half-size, and the microphones' places.
struct Above {
    c: Pos2,
    half: f32,
    /// The microphones' ring's inner and outer radii (10 cm and 300 cm).
    near: f32,
    far: f32,
    mics: [Pos2; 2],
}

fn mic_spots(r: Rect, rs: &cantus_dsp::rota::RotaSettings, k: f32) -> Above {
    let c = pos2(r.center().x, r.center().y - 4.0 * k);
    let room = (r.width().min(r.height()) * 0.5 - 30.0 * k).max(40.0 * k);
    let half = room * 0.42;
    let (near, far) = (half * 1.25, room);
    let t = (rs.distance_cm.clamp(10.0, 300.0) / 10.0).ln() / 30f32.ln();
    let d = near + (far - near) * t;
    let a = (rs.spread_deg.clamp(0.0, 180.0) * 0.5).to_radians();
    // The front (toward the listener) is down the screen; the microphones either side of it.
    let mics = [
        c + vec2(-a.sin(), a.cos()) * d,
        c + vec2(a.sin(), a.cos()) * d,
    ];
    Above {
        c,
        half,
        near,
        far,
        mics,
    }
}

/// A microphone dragged round the cabinet (the angle between the two) and away from it (the
/// distance).
fn mic_drag(host: &mut dyn FxHost, v: &mut View, ui: &mut egui::Ui, r: Rect, a: &Above, k: f32) {
    let g = ui.interact(r, ui.id().with("rota-above"), Sense::click_and_drag());
    let pointer = g.interact_pointer_pos().or(g.hover_pos());
    if g.drag_started() {
        v.drag = pointer.and_then(|pt| {
            (0..2)
                .map(|i| (i, a.mics[i].distance(pt)))
                .filter(|(_, d)| *d < 18.0 * k)
                .min_by(|x, y| x.1.total_cmp(&y.1))
                .map(|(i, _)| i)
        });
    }
    if let (Some(_), Some(pt), true) = (v.drag, pointer, g.dragged()) {
        let d = pt - a.c;
        let spread = (2.0 * d.x.abs().atan2(d.y).to_degrees())
            .clamp(0.0, 180.0)
            .round();
        let t = ((d.length() - a.near) / (a.far - a.near)).clamp(0.0, 1.0);
        let cm = (10.0 * (t * 30f32.ln()).exp()).round().clamp(10.0, 300.0);
        if (spread - host.spec().get("spread")).abs() > 0.5 {
            host.set("spread", spread, Some("spread"));
        }
        if (cm - host.spec().get("dist")).abs() > 0.5 {
            host.set("dist", cm, Some("dist"));
        }
    }
    if g.drag_stopped() && v.drag.take().is_some() {
        host.end_gesture();
    }
    if g.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("rota.d.above_name"),
            frame::tr("rota.d.above_tip"),
            Caps::NONE,
        );
    }
}

/// The cabinet from above: its lid, the drum's ring with its opening, the horn's arm with
/// its bell, the microphones either side of the front with the angle between them.
#[allow(clippy::too_many_arguments)]
fn paint_above(
    b: &mut Brush,
    ink: &Ink,
    r: Rect,
    v: &View,
    rs: &cantus_dsp::rota::RotaSettings,
    a: &Above,
    k: f32,
    horn_c: Color32,
    drum_c: Color32,
) {
    canvas::put(
        b,
        pos2(r.left() + 14.0 * k, r.top() + 14.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("rota.d.above", Role::Micro, ink.dim),
        k,
        None,
    );
    let c = a.c;
    // The distance rings: 10, 30, 100 and 300 cm, faint.
    for cm in [10.0f32, 30.0, 100.0, 300.0] {
        let t = (cm / 10.0).ln() / 30f32.ln();
        let rr = a.near + (a.far - a.near) * t;
        b.circle_stroke(c, rr, 0.5 * k, ink.grid);
        canvas::putf(
            b,
            c + vec2(rr * 0.71 + 3.0 * k, -rr * 0.71 - 3.0 * k),
            Align2::LEFT_BOTTOM,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{cm:.0} cm"),
        );
    }
    // The cabinet's lid, the front marked.
    let lid = Rect::from_center_size(c, vec2(a.half * 2.0, a.half * 1.7));
    b.rrect(
        lid,
        4.0 * k,
        Fill::V(pal::WALNUT_LIGHT.rgb(), pal::WALNUT.rgb()),
    );
    b.rrect_stroke(
        lid,
        4.0 * k,
        0.8 * k,
        with_alpha(pal::WALNUT_DARK.rgb(), 0.9),
    );
    canvas::put(
        b,
        pos2(c.x, lid.bottom() + 9.0 * k),
        Align2::CENTER_CENTER,
        &Text::key("rota.d.front", Role::Micro, ink.dim),
        k,
        None,
    );
    // The drum's ring and its opening; the horn's arm and bell. Angle 0 faces the front.
    let screen = |turns: f32| PI * 0.5 - TAU * turns;
    let rd = a.half * 0.72;
    b.circle_stroke(c, rd, 0.8 * k, with_alpha(drum_c, 0.45));
    let ad = screen(v.drum.0);
    b.arc(c, rd, ad - 0.55, ad + 0.55, 2.4 * k, drum_c, true);
    let ah = screen(v.horn.0);
    let dir = vec2(ah.cos(), ah.sin());
    let rh = a.half * 0.9;
    b.segment(
        c - dir * rh * 0.8,
        c + dir * rh * 0.6,
        1.2 * k,
        with_alpha(pal::CHROME.rgb(), 0.9),
        true,
    );
    let n = vec2(-dir.y, dir.x);
    let tip = c + dir * rh;
    let bell = [
        c + dir * rh * 0.55 + n * 1.5 * k,
        tip + n * 7.0 * k,
        tip - n * 7.0 * k,
        c + dir * rh * 0.55 - n * 1.5 * k,
    ];
    b.polygon(&bell, horn_c);
    let dummy = c - dir * rh * 0.8;
    b.circle(
        dummy,
        3.2 * k,
        Fill::Solid(with_alpha(pal::HORN_LIGHT.rgb(), 0.9)),
    );
    b.circle(
        c,
        3.0 * k,
        Fill::V(pal::CHROME.rgb(), pal::CHROME_SHADE.rgb()),
    );
    // The angle between the microphones, on an arc round the front.
    let half_a = (rs.spread_deg.clamp(0.0, 180.0) * 0.5).to_radians();
    let ra = a.near * 0.92;
    if half_a > 0.01 {
        b.arc(
            c,
            ra,
            PI * 0.5 - half_a,
            PI * 0.5 + half_a,
            0.7 * k,
            with_alpha(ink.text, 0.5),
            false,
        );
    }
    canvas::tag(
        b,
        pos2(c.x, c.y + ra + 12.0 * k),
        Align2::CENTER_CENTER,
        frame::tr("rota.d.angle"),
        ink.text,
        k,
        Some(ink.plate),
        format_args!("{:.0}\u{b0}", rs.spread_deg),
    );
    for (i, m) in a.mics.iter().enumerate() {
        canvas::dashed(
            b,
            *m,
            c,
            0.6 * k,
            with_alpha(ink.text, 0.25),
            3.0 * k,
            3.0 * k,
        );
        b.circle(
            *m,
            7.0 * k,
            Fill::V(pal::MIC_BODY_LIGHT.rgb(), pal::MIC_BODY.rgb()),
        );
        b.circle(*m, 4.4 * k, Fill::Solid(pal::MIC_MESH.rgb()));
        b.circle_stroke(
            *m,
            9.5 * k,
            if v.drag == Some(i) { 1.3 } else { 0.8 } * k,
            ink.hue,
        );
    }
    canvas::tag(
        b,
        a.mics[1] + vec2(0.0, 18.0 * k),
        Align2::CENTER_CENTER,
        frame::tr("rota.d.distance"),
        ink.text,
        k,
        Some(ink.plate),
        format_args!("{:.0} cm", rs.distance_cm),
    );
}

/// Each rotor's speed against the time since the switch was thrown — the way it goes now,
/// solid, the other way faint — on log scales, a dot where each rotor is; under it the
/// ramps' times.
#[allow(clippy::too_many_arguments)]
fn paint_ramps(
    b: &mut Brush,
    ink: &Ink,
    r: Rect,
    now: usize,
    rd: &Reading,
    rs: &cantus_dsp::rota::RotaSettings,
    k: f32,
    horn_c: Color32,
    drum_c: Color32,
) {
    canvas::put(
        b,
        pos2(r.left() + 14.0 * k, r.top() + 14.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("rota.d.ramps", Role::Micro, ink.dim),
        k,
        None,
    );
    let plot = Rect::from_min_max(
        pos2(r.left() + 46.0 * k, r.top() + 46.0 * k),
        pos2(r.right() - 16.0 * k, r.bottom() - 74.0 * k),
    );
    let (t0, t1) = (0.02f32, 20.0f32);
    let (v0, v1) = (10.0f32, 600.0f32);
    let fx =
        |t: f32| plot.left() + plot.width() * ((t.max(t0) / t0).ln() / (t1 / t0).ln()).min(1.0);
    let fy =
        |rpm: f32| plot.bottom() - plot.height() * ((rpm.clamp(v0, v1) / v0).ln() / (v1 / v0).ln());
    for (t, label) in [(0.1f32, "0.1 s"), (1.0, "1 s"), (10.0, "10 s")] {
        let x = fx(t);
        b.vline(x, plot.top(), plot.bottom(), 0.5 * k, ink.grid);
        canvas::put(
            b,
            pos2(x, plot.bottom() + 9.0 * k),
            Align2::CENTER_CENTER,
            &Text::new(label, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    for (rpm, label) in [(40.0f32, "40"), (100.0, "100"), (400.0, "400")] {
        let y = fy(rpm);
        b.hline(plot.left(), plot.right(), y, 0.5 * k, ink.grid);
        canvas::put(
            b,
            pos2(plot.left() - 5.0 * k, y),
            Align2::RIGHT_CENTER,
            &Text::new(label, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    canvas::put(
        b,
        pos2(plot.left() - 5.0 * k, plot.top() - 9.0 * k),
        Align2::RIGHT_CENTER,
        &Text::key("rota.d.rpm", Role::Micro, ink.dim),
        k,
        None,
    );
    // The two ways: up (slow to fast) and down (fast to slow, or to the brake).
    let rotors = [
        (
            horn_c,
            rs.horn_slow_rpm,
            rs.horn_fast_rpm,
            rs.horn_up_s,
            rs.horn_down_s,
            rd.horn.1,
        ),
        (
            drum_c,
            rs.drum_slow_rpm,
            rs.drum_fast_rpm,
            rs.drum_up_s,
            rs.drum_down_s,
            rd.drum.1,
        ),
    ];
    for (col, slow, fast, up, down, rpm) in rotors {
        for way in [false, true] {
            // `way`: true is the way the switch has it now.
            let rising = (now == 2) == way;
            let (from, to, tau) = if rising {
                (slow, fast, up)
            } else if now == 0 && way {
                (slow, v0, down)
            } else {
                (fast, slow, down)
            };
            let mut pts = [Pos2::ZERO; 64];
            for (j, p) in pts.iter_mut().enumerate() {
                let t = t0 * (t1 / t0).powf(j as f32 / 63.0);
                let s = to + (from - to) * (-t / tau.max(1e-3)).exp();
                *p = pos2(fx(t), fy(s));
            }
            if way {
                canvas::trace(b, &pts, 1.4 * k, col, ink);
                // Where the rotor is on it now (at the end, once it has arrived).
                let x = if rd.seen && (rpm - to).abs() > 0.5 {
                    let q = ((rpm - to) / (from - to)).clamp(1e-4, 1.0);
                    -tau * q.ln()
                } else {
                    t1
                };
                let at = pos2(fx(x), fy(if rd.seen { rpm } else { to }));
                b.circle(at, 4.0 * k, Fill::Solid(col));
                b.circle_stroke(at, 7.0 * k, 0.8 * k, with_alpha(col, 0.6));
            } else {
                b.polyline(&pts, 0.7 * k, with_alpha(col, 0.28));
            }
        }
    }
    // The ramps' times under the plot.
    let unit = "s";
    let y = plot.bottom() + 30.0 * k;
    for (i, (key, col, up, down)) in [
        ("rota.d.horn", horn_c, rs.horn_up_s, rs.horn_down_s),
        ("rota.d.drum", drum_c, rs.drum_up_s, rs.drum_down_s),
    ]
    .into_iter()
    .enumerate()
    {
        let yy = y + i as f32 * 20.0 * k;
        let t = canvas::put(
            b,
            pos2(plot.left(), yy),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, col),
            k,
            None,
        );
        let x = t.right().max(plot.left() + 56.0 * k) + 10.0 * k;
        let t = canvas::tag(
            b,
            pos2(x, yy),
            Align2::LEFT_CENTER,
            frame::tr("rota.d.up"),
            ink.text,
            k,
            None,
            format_args!("{up:.2} {unit}"),
        );
        canvas::tag(
            b,
            pos2(t.right() + 16.0 * k, yy),
            Align2::LEFT_CENTER,
            frame::tr("rota.d.down"),
            ink.text,
            k,
            None,
            format_args!("{down:.2} {unit}"),
        );
    }
}

/// v2 — a walnut bezel round one of the display's instruments: a fine frame of the
/// cabinet's wood with its grain, a dark inner lip, a brass screw in each corner — so the
/// view from above and the ramps read as windows in the same cabinet.
fn bezel(b: &mut Brush, r: Rect, k: f32) {
    let w = 5.0 * k;
    for (edge, horizontal) in [
        (
            Rect::from_min_max(r.min, pos2(r.right(), r.top() + w)),
            true,
        ),
        (
            Rect::from_min_max(pos2(r.left(), r.bottom() - w), r.max),
            true,
        ),
        (
            Rect::from_min_max(r.min, pos2(r.left() + w, r.bottom())),
            false,
        ),
        (
            Rect::from_min_max(pos2(r.right() - w, r.top()), r.max),
            false,
        ),
    ] {
        b.rect(edge, Fill::V(pal::WALNUT_LIGHT.rgb(), pal::WALNUT.rgb()));
        if horizontal {
            for j in 0..2 {
                let y = edge.top() + (1.5 + 2.0 * j as f32) * k;
                b.hline(
                    edge.left(),
                    edge.right(),
                    y,
                    0.4 * k,
                    with_alpha(pal::WALNUT_DARK.rgb(), 0.35),
                );
            }
        } else {
            for j in 0..2 {
                let x = edge.left() + (1.5 + 2.0 * j as f32) * k;
                b.vline(
                    x,
                    edge.top(),
                    edge.bottom(),
                    0.4 * k,
                    with_alpha(pal::WALNUT_DARK.rgb(), 0.35),
                );
            }
        }
    }
    b.rrect_stroke(
        r.shrink(w),
        1.0 * k,
        0.8 * k,
        with_alpha(pal::GASKET.rgb(), 0.9),
    );
    b.rrect_stroke(r, 1.5 * k, 0.6 * k, with_alpha(pal::WALNUT_DARK.rgb(), 0.9));
    for (x, y) in [
        (r.left() + w * 0.5, r.top() + w * 0.5),
        (r.right() - w * 0.5, r.top() + w * 0.5),
        (r.left() + w * 0.5, r.bottom() - w * 0.5),
        (r.right() - w * 0.5, r.bottom() - w * 0.5),
    ] {
        b.circle(
            pos2(x, y),
            1.8 * k,
            Fill::V(pal::COPPER_LIGHT.rgb(), pal::COPPER.rgb()),
        );
    }
}
