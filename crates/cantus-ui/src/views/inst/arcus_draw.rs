//! Arcus's pictures (a child of `views::inst::arcus`).
//!
//! - **The instrument** (Play): the cello or the double bass lying across the display, its
//!   scroll to the right — the body's outline with its bouts, purfling and f-holes, the
//!   tailpiece, the bridge, the fingerboard and the four strings, **the sounding one lit**
//!   and swinging as loud as it sounds; the left hand's **finger** where the note stops the
//!   string; **the bow** across the string at its real contact point (β of the stopped length
//!   from the bridge), tilted to reach that string over the bridge's arch, its hair pressed
//!   into the string as hard as it presses (darker and bent: the force over the region's
//!   top), its arrow as long and pointing as it moves; a pluck's finger for pizzicato, the
//!   short strokes of tremolo. A drag across it moves the contact (left and right) and the
//!   force (up and down), one undo step a drag.
//! - **The Schelleng diagram** (Edit): the bow's force against its distance from the bridge,
//!   log axes, the region of Helmholtz motion this model measures, the bow's point — the
//!   diagram is the control too.

#[path = "arcus_draw_body.rs"]
mod body;

use crate::app::CantusApp;
use crate::kit::color::with_alpha;
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::TrackId;
use cantus_dsp::arcus::voice::{bowing, limits};
use cantus_dsp::arcus::{tel, ArcusSettings};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// The diagram's ranges: β and force (as a share of the upper limit at β 0.1).
const BETA: (f32, f32) = (0.04, 0.3);
const FORCE: (f32, f32) = (0.02, 3.0);

/// What the pictures show: the newest voice's bowing, or the settings' at their dynamic on
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
    /// The region's limits for this bowing, N.
    pub lo: f32,
    pub hi: f32,
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
            lo: tele[tel::FMIN],
            hi: tele[tel::FMAX],
        };
    }
    let string = 2;
    let st = spec.strings[string];
    let bw = bowing(
        spec,
        fs.dynamic as f64,
        fs.bow_pos as f64,
        fs.bow_force as f64,
        fs.bow_speed as f64,
        st.z0(),
        48_000.0 / st.open_hz(),
    );
    Now {
        live,
        key: (st.open + 7) as f32,
        string,
        beta: bw.beta as f32,
        force: bw.force as f32,
        speed: bw.speed as f32,
        dynamic: fs.dynamic,
        level: 0.0,
        sticking: false,
        lo: bw.f_min as f32,
        hi: bw.f_max as f32,
    }
}

/// A drag in `r`: across, the contact (× 0.7 at the left — the bridge's side — … × 1.4 at
/// the right), up and down, the force (× 0.5 … × 2).
fn drag(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, r: Rect, salt: &str) {
    let id = egui::Id::new(("arcus-drag", salt, track.0));
    let resp = ui.interact(r, id, Sense::click_and_drag());
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let ux = ((p.x - r.left()) / r.width()).clamp(0.0, 1.0);
            let uy = ((r.bottom() - p.y) / r.height()).clamp(0.0, 1.0);
            app.set_inst_value(track, "bow.pos", 0.7 * 2f32.powf(ux), true);
            app.set_inst_value(track, "bow.force", 0.5 * 4f32.powf(uy), true);
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        app.end_gesture();
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
}

/// Paint the instrument in `d` (Play) and take its drag. `tele` is the newest voice's
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
    let inner = d.shrink(s.s(10.0));
    drag(app, ui, track, inner, "picture");
    let n = now(fs, tele);
    crate::kit::with_brush(&ui.painter_at(d), |b| {
        body::instrument(b, inner, fs, &n, s);
    });
}

/// Paint the Schelleng diagram in `d` (Edit) and take its drag.
#[allow(clippy::too_many_arguments)]
pub fn schelleng(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    fs: &ArcusSettings,
    tele: &cantus_engine::Telemetry,
    d: Rect,
    s: &Style,
) {
    let inner = d.shrink(s.s(10.0));
    let dia = Rect::from_min_max(
        pos2(inner.left() + s.s(46.0), inner.top() + s.s(4.0)),
        pos2(inner.right() - s.s(4.0), inner.bottom() - s.s(14.0)),
    );
    drag(app, ui, track, dia, "diagram");
    let n = now(fs, tele);
    let ink = s.t.text;
    let dim = s.t.text_dim;
    let bow = super::face::accent_on(s, s.hue);
    let rosin = super::face::voice2(s, s.hue);
    crate::kit::with_brush(&ui.painter_at(d), |b| {
        diagram(b, dia, fs, &n, ink, dim, bow, rosin, s);
    });
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
    let label = |b: &mut Brush, rect: Rect, key: &str, align: Align2, c: Color32| {
        let t = Text::key(key, Role::Micro, c).align(align);
        let _ = ktext::paint(b, rect, &t, s.tk());
    };
    label(
        b,
        Rect::from_min_size(
            pos2(r.left(), r.bottom() + s.s(2.0)),
            vec2(r.width(), s.s(11.0)),
        ),
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
        Rect::from_min_size(
            pos2(r.left() + s.s(4.0), r.top() + s.s(3.0)),
            vec2(r.width() * 0.5, s.s(11.0)),
        ),
        "arcus.draw.region",
        Align2::LEFT_TOP,
        rosin,
    );
    let p = pos2(x_of(r, n.beta), y_of(r, n.force / unit));
    b.circle(p, s.s(4.5), Fill::Solid(bow));
    b.circle_stroke(p, s.s(6.5), s.s(1.0), if n.sticking { ink } else { bow });
}

/// The inspector's thumbnail: the instrument, small.
pub fn thumbnail(b: &mut Brush, r: Rect, fs: &ArcusSettings, s: &Style) {
    let n = now(fs, &[0.0; cantus_engine::TELEMETRY_LEN]);
    body::instrument(b, r, fs, &n, s);
}
