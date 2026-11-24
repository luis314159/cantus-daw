//! Arcus's pictures (a child of `views::inst::arcus`).
//!
//! - **The instrument** (Play): the cello or the double bass standing on its endpin in its
//!   room — the scene engine's picture (r42:arcus-look, `scene::bowed*`), Drawn or
//!   Photographic as *Settings ▸ Appearance* says — and over it, live, **the sounding string lit**
//!   and swinging as loud as it sounds; the left hand's **finger** where the note stops the
//!   string; **the bow** across the string at its real contact point (β of the stopped length
//!   from the bridge), tilted to reach that string over the bridge's arch, its hair pressed
//!   into the string as hard as it presses (darker and bent: the force over the region's
//!   top), sliding along its hair as it moves (a blur behind it as fast); a pluck's finger for pizzicato, the
//!   short strokes of tremolo. A drag across it moves the contact (left and right) and the
//!   force (up and down), one undo step a drag.
//! - **The Schelleng diagram** (Edit): the bow's force against its distance from the bridge,
//!   log axes, the region of Helmholtz motion this model measures, the bow's point — the
//!   diagram is the control too.

#[path = "arcus_draw_arm.rs"]
mod arm; // r42:arcus-upper
#[path = "arcus_draw_body.rs"]
mod body;
#[path = "arcus_draw_hand.rs"]
pub mod hand; // r42:arcus-upper

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

/// Which of the scene's instruments `fs` plays.
pub fn bowed_of(fs: &ArcusSettings) -> crate::scene::bowed::Bowed {
    match fs.instrument {
        cantus_dsp::arcus::tables::Instrument::Bass => crate::scene::bowed::Bowed::Bass,
        cantus_dsp::arcus::tables::Instrument::Violin => crate::scene::bowed::Bowed::Violin, // r42:arcus-upper
        cantus_dsp::arcus::tables::Instrument::Viola => crate::scene::bowed::Bowed::Viola, // r42:arcus-upper
        _ => crate::scene::bowed::Bowed::Cello,
    }
}

/// The scene `fs`'s instrument is drawn as (r42:arcus-look): the cello or the double bass
/// standing in the studio's live room Chorda and Tympanum stand in, in the theme's light.
pub fn scene_view(
    fs: &ArcusSettings,
    s: &Style,
    holder: crate::scene::bowed_hold::Holder, // r42:arcus-upper
) -> crate::scene::SceneView {
    crate::scene::SceneView {
        subject: crate::scene::Subject::Bowed {
            instrument: bowed_of(fs),
            holder, // r42:arcus-upper
        },
        room: crate::scene::Room::Studio,
        look: Default::default(),
        dark: s.t.dark,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: super::face::accent_on(s, s.hue),
        ink: s.t.text,
    }
}

/// Paint the instrument in `d` (Play) and take its drag: the scene engine's picture of the
/// cello or the bass in its room (baked, cached: at rest a frame paints one textured
/// quad), and over it, live, the sounding string, the finger and the bow. `tele` is the
/// newest voice's telemetry (zeros with no engine).
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
    drag(app, ui, track, d.shrink(s.s(4.0)), "picture");
    let n = now(fs, tele);
    // What holds it and whether the finger shows: this machine's preferences. // r42:arcus-upper
    let (holder, stop) = (app.settings.arcus_holder, app.settings.arcus_stop); // r42:arcus-upper
    let view = scene_view(fs, s, holder); // r42:arcus-upper
    let painter = ui.painter_at(d);
    let time = ui.input(|i| i.time);
    let entry = app
        .inst
        .arcus_scenes
        .entry(track.0)
        .or_insert_with(|| (crate::scene::Scene::default(), 0.5, time));
    let moving = entry.0.paint(&painter, d, &view);
    let b = bowed_of(fs);
    // The bow's place along its hair, carried from frame to frame as it moves.
    if n.live {
        entry.1 = body::travel(b, entry.1, n.speed, (time - entry.2) as f32);
    }
    entry.2 = time;
    let travel = entry.1;
    let Some(built) = entry.0.built() else {
        return;
    };
    let shown = entry.0.rect().unwrap_or(d);
    let v = body::View {
        cam: built.cam,
        origin: shown.min,
        k: egui::vec2(
            shown.width() / built.size.x.max(1.0),
            shown.height() / built.size.y.max(1.0),
        ),
    };
    let rig = crate::scene::bowed::rig_for(b, holder); // r42:arcus-upper
                                                       // r42:arcus-upper — the figure's left hand: its position for this note, glided.
    let hand = {
        let st = body::stop(fs, &n);
        let h = app.inst.arcus_hands.entry(track.0).or_insert((1.0, time));
        let to = hand::target(h.0, hand::semis(st));
        h.0 = hand::glide(h.0, to, (time - h.1) as f32);
        h.1 = time;
        (h.0, (h.0 - to).abs() > 0.005)
    };
    let pl = body::Play {
        b,
        rig: &rig,
        fs,
        n: &n,
        travel,
        time,
        vibrato: tele[tel::VIBRATO].clamp(0.0, 1.0),
        lit: super::face::voice2(s, s.hue),
        holder,                                   // r42:arcus-upper
        stop,                                     // r42:arcus-upper
        hand: hand.0,                             // r42:arcus-upper
        accent: super::face::accent_on(s, s.hue), // r42:arcus-upper
    };
    crate::kit::with_brush(&painter, |br| body::live(br, &v, &pl, s));
    if moving || (n.live && n.level > 0.0) || hand.1 {
        crate::views::workspace_ticker::moving(
            ui.ctx(),
            crate::views::workspace_ticker::Reason::Live,
        );
    }
}

/// For the tests (r42:arcus-look): `track`'s Arcus picture alone in `rect`, as Play draws
/// it on its display — the scene and what it plays, live.
#[doc(hidden)]
pub fn picture_for_tests(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, rect: Rect) {
    let Some(fs) = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .map(cantus_engine::inst::arcus::settings)
    else {
        return;
    };
    let tele = app.inst_telemetry();
    let s = Style::current().function(crate::kit::Function::Source);
    let sd = super::face::on_display(&s);
    picture(app, ui, track, &fs, &tele, rect, &sd);
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
        Rect::from_min_size(
            pos2(r.left() - s.s(46.0), r.top()),
            vec2(s.s(43.0), s.s(11.0)),
        ),
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
    body::thumbnail(b, r, bowed_of(fs), s);
}
