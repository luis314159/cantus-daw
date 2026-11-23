//! Arcus's picture (a child of `views::inst::arcus`): the string as it is bowed, and the
//! bow's place in its region.
//!
//! - **The string**, side on: the bridge (left), the stopped length to the finger, the
//!   fingerboard and the nut (right); the bow across it at its contact point β, as thick
//!   as it presses, its arrow as long as it moves and pointing its way (down-bow →, up-bow
//!   ←); the Helmholtz kink of the string's shape while it sounds. A pluck is a finger.
//! - **The Schelleng diagram** under it: the bow's force against its distance from the
//!   bridge, both on log axes; the region where the string's Helmholtz motion holds (this
//!   model's, measured — `tests_gate`) in the rosin's amber, and the bow's point now in its
//!   garnet. **The diagram is the control**: a drag moves the contact point (across) and the
//!   force (up and down), one undo step a drag.

use crate::app::CantusApp;
use crate::kit::color::with_alpha;
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::TrackId;
use cantus_dsp::arcus::voice::{bowing, limits};
use cantus_dsp::arcus::{tel, ArcusSettings, Technique};
use cantus_dsp::math;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// The diagram's ranges: β and force (as a share of the upper limit at β 0.1).
const BETA: (f32, f32) = (0.04, 0.3);
const FORCE: (f32, f32) = (0.02, 3.0);

/// What the picture shows: the newest voice's bowing, or the settings' at their dynamic on
/// the instrument's third string (the cello's D) with no voice.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Now {
    pub live: bool,
    pub key: f32,
    pub string: usize,
    pub beta: f32,
    pub force: f32,
    pub speed: f32,
    pub dynamic: f32,
    pub level: f32,
    pub sticking: bool,
}

pub fn now(fs: &ArcusSettings, tele: &cantus_engine::Telemetry) -> Now {
    let spec = fs.instrument.spec();
    let live = tele[tel::ALIVE] > 0.5 && tele[tel::KEY] >= 0.0;
    if live {
        return Now {
            live,
            key: tele[tel::KEY],
            string: (tele[tel::STRING].max(0.0) as usize).min(3),
            beta: tele[tel::BETA].max(0.01),
            force: tele[tel::FORCE],
            speed: tele[tel::SPEED],
            dynamic: tele[tel::DYNAMIC],
            level: tele[tel::LEVEL].clamp(0.0, 1.0),
            sticking: tele[tel::STICK] > 0.5,
        };
    }
    let string = 2;
    let z0 = spec.strings[string].z0();
    let bw = bowing(
        spec,
        fs.dynamic as f64,
        fs.bow_pos as f64,
        fs.bow_force as f64,
        fs.bow_speed as f64,
        z0,
        48_000.0 / spec.strings[string].open_hz(),
    );
    Now {
        live,
        key: (spec.strings[string].open + 7) as f32,
        string,
        beta: bw.beta as f32,
        force: bw.force as f32,
        speed: bw.speed as f32,
        dynamic: fs.dynamic,
        level: 0.0,
        sticking: false,
    }
}

/// The force axis's unit for a bowing: the upper limit at β 0.1 and the bow's speed.
fn unit(fs: &ArcusSettings, n: &Now) -> f32 {
    let z0 = fs.instrument.spec().strings[n.string].z0();
    limits(z0, (n.speed.abs() as f64).max(0.02), 0.1).1 as f32
}

fn x_of(r: Rect, beta: f32) -> f32 {
    let u = (beta.max(1e-3).ln() - BETA.0.ln()) / (BETA.1.ln() - BETA.0.ln());
    r.left() + r.width() * u.clamp(0.0, 1.0)
}

fn y_of(r: Rect, f: f32) -> f32 {
    let u = (f.max(1e-4).ln() - FORCE.0.ln()) / (FORCE.1.ln() - FORCE.0.ln());
    r.bottom() - r.height() * u.clamp(0.0, 1.0)
}

/// Paint the picture in `d` and take the drag on the diagram. `tele` is the newest voice's
/// telemetry (zeros with no engine).
#[allow(clippy::too_many_arguments)]
pub fn picture(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    fs: &ArcusSettings,
    tele: &cantus_engine::Telemetry,
    d: Rect,
    s: &Style,
) {
    let pad = s.s(10.0);
    let inner = d.shrink(pad);
    let split = inner.top() + inner.height() * 0.42;
    let top = Rect::from_min_max(inner.min, pos2(inner.right(), split - s.s(6.0)));
    let dia = Rect::from_min_max(
        pos2(inner.left() + s.s(46.0), split + s.s(6.0)),
        pos2(inner.right() - s.s(4.0), inner.bottom() - s.s(14.0)),
    );
    let n = now(fs, tele);
    // The drag: across, the contact point (`bow.pos` from the dynamic's own β); up and down,
    // the force's place (`bow.force`, an octave either way).
    let id = egui::Id::new(("arcus-bow", track.0));
    let resp = ui.interact(dia, id, Sense::click_and_drag());
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let ux = ((p.x - dia.left()) / dia.width()).clamp(0.0, 1.0);
            let uy = ((dia.bottom() - p.y) / dia.height()).clamp(0.0, 1.0);
            let pos = (1.0 - 2.0 * ux).clamp(-1.0, 1.0);
            let force = (2.0 * uy - 1.0).clamp(-1.0, 1.0);
            app.set_inst_value(track, "bow.pos", pos, true);
            app.set_inst_value(track, "bow.force", force, true);
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        app.end_gesture();
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let ink = s.t.text;
    let dim = s.t.text_dim;
    let bow = super::face::accent_on(s, s.hue);
    let rosin = super::face::voice2(s, s.hue);
    crate::kit::with_brush(&ui.painter_at(d), |b| {
        string(b, top, fs, &n, ink, dim, bow, s);
        diagram(b, dia, fs, &n, ink, dim, bow, rosin, s);
    });
}

/// The string side on, the bow across it.
#[allow(clippy::too_many_arguments)]
fn string(
    b: &mut Brush,
    r: Rect,
    fs: &ArcusSettings,
    n: &Now,
    ink: Color32,
    dim: Color32,
    bow: Color32,
    s: &Style,
) {
    let spec = fs.instrument.spec();
    let st = spec.strings[n.string];
    let y = r.center().y + s.s(4.0);
    let (x0, x1) = (r.left() + s.s(8.0), r.right() - s.s(8.0));
    // The bridge, the fingerboard (its last two thirds), the nut.
    b.rrect(
        Rect::from_center_size(pos2(x0, y + s.s(6.0)), vec2(s.s(7.0), s.s(22.0))),
        s.r(1.5),
        Fill::Solid(dim),
    );
    let board = Rect::from_min_max(
        pos2(x0 + (x1 - x0) * 0.30, y + s.s(3.0)),
        pos2(x1, y + s.s(8.0)),
    );
    b.rrect(board, s.r(2.0), Fill::Solid(with_alpha(dim, 0.35)));
    b.rrect(
        Rect::from_center_size(pos2(x1, y + s.s(2.0)), vec2(s.s(4.0), s.s(14.0))),
        s.r(1.0),
        Fill::Solid(dim),
    );
    // The stopped length: the finger where the key stops the string.
    let f = 440.0 * math::exp2((n.key as f64 - 69.0) / 12.0);
    let stop = (st.open_hz() / f).clamp(0.1, 1.0) as f32;
    let xf = x0 + (x1 - x0) * stop;
    let pizz = fs.technique == Technique::Pizzicato;
    let xb = x0 + (xf - x0) * n.beta.clamp(0.0, 0.5);
    // The string: straight at rest; the Helmholtz kink while it sounds.
    let lift = r.height() * 0.30 * n.level.sqrt().min(1.0);
    let kink = if pizz { xb } else { x0 + (xf - x0) * 0.62 };
    b.polyline(
        &[pos2(x0, y), pos2(kink, y - lift), pos2(xf, y)],
        s.s(1.6),
        ink,
    );
    if xf < x1 - s.s(1.0) {
        b.hline(xf, x1, y, s.s(1.2), dim);
    }
    if stop < 0.999 {
        b.circle(pos2(xf, y - s.s(4.0)), s.s(4.0), Fill::Solid(ink));
    }
    if pizz {
        // The finger that plucks.
        b.circle_stroke(pos2(xb, y - s.s(9.0)), s.s(5.0), s.s(1.4), bow);
    } else {
        // The bow: hair as thick as it presses (relative to its region), its arrow its speed.
        let unit = unit(fs, n).max(1e-3);
        let press = (n.force / unit).clamp(0.0, 2.0);
        let w = s.s(2.0 + 6.0 * press.sqrt());
        b.rrect(
            Rect::from_center_size(pos2(xb, y), vec2(w, r.height() * 0.9)),
            w * 0.5,
            Fill::Solid(with_alpha(bow, 0.85)),
        );
        let len = (n.speed.abs() / 0.4).clamp(0.0, 1.0) * r.width() * 0.18;
        if len > s.s(2.0) {
            let dir = if n.speed < 0.0 { -1.0 } else { 1.0 };
            let ay = r.top() + s.s(6.0);
            let (a, z) = (pos2(xb, ay), pos2(xb + dir * len, ay));
            b.segment(a, z, s.s(1.6), bow, true);
            b.segment(z, pos2(z.x - dir * s.s(5.0), ay - s.s(3.5)), s.s(1.6), bow, true);
            b.segment(z, pos2(z.x - dir * s.s(5.0), ay + s.s(3.5)), s.s(1.6), bow, true);
        }
    }
}

/// The Schelleng diagram: the region, its limits, the bow's point.
#[allow(clippy::too_many_arguments)]
fn diagram(
    b: &mut Brush,
    r: Rect,
    fs: &ArcusSettings,
    n: &Now,
    ink: Color32,
    dim: Color32,
    bow: Color32,
    rosin: Color32,
    s: &Style,
) {
    let z0 = fs.instrument.spec().strings[n.string].z0();
    let v = (n.speed.abs() as f64).max(0.02);
    let unit = unit(fs, n).max(1e-3);
    let steps = 40;
    let mut upper: Vec<Pos2> = Vec::with_capacity(steps + 1);
    let mut lower: Vec<Pos2> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let beta = BETA.0 * (BETA.1 / BETA.0).powf(i as f32 / steps as f32);
        let (lo, hi) = limits(z0, v, beta as f64);
        let x = x_of(r, beta);
        upper.push(pos2(x, y_of(r, hi as f32 / unit)));
        lower.push(pos2(x, y_of(r, lo as f32 / unit)));
    }
    b.rrect_stroke(r, s.r(2.0), s.s(1.0), with_alpha(dim, 0.6));
    b.between(&upper, &lower, with_alpha(rosin, 0.28));
    b.polyline(&upper, s.s(1.2), rosin);
    b.polyline(&lower, s.s(1.2), rosin);
    // The axes' words: β across (the bridge at the left), force up.
    let label = |b: &mut Brush, rect: Rect, key: &str, align: Align2, c: Color32| {
        let t = Text::key(key, Role::Micro, c).align(align);
        let _ = ktext::paint(b, rect, &t, s.tk());
    };
    label(
        b,
        Rect::from_min_size(pos2(r.left(), r.bottom() + s.s(2.0)), vec2(r.width(), s.s(11.0))),
        "arcus.draw.beta",
        Align2::CENTER_CENTER,
        dim,
    );
    label(
        b,
        Rect::from_min_size(pos2(r.left() - s.s(46.0), r.top()), vec2(s.s(43.0), s.s(11.0))),
        "arcus.draw.force",
        Align2::RIGHT_TOP,
        dim,
    );
    label(
        b,
        Rect::from_min_size(pos2(r.left() + s.s(4.0), r.top() + s.s(3.0)), vec2(r.width() * 0.5, s.s(11.0))),
        "arcus.draw.region",
        Align2::LEFT_TOP,
        rosin,
    );
    // The bow's point now.
    let p = pos2(x_of(r, n.beta), y_of(r, n.force / unit));
    b.circle(p, s.s(4.5), Fill::Solid(bow));
    b.circle_stroke(p, s.s(6.5), s.s(1.0), if n.sticking { ink } else { bow });
}

/// The inspector's thumbnail: the string and the bow, small.
pub fn thumbnail(b: &mut Brush, r: Rect, fs: &ArcusSettings, s: &Style) {
    let bow = super::face::accent_on(s, s.hue);
    let n = now(fs, &[0.0; cantus_engine::TELEMETRY_LEN]);
    string(b, r, fs, &n, s.t.text, s.t.text_dim, bow, s);
}
