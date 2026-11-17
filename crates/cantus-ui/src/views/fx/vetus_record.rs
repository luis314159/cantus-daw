//! Vetus's record (ADR 0130): the display's object, an LP on a turntable that
//! ages with the settings.
//!
//! - **The groove is the music**: the output of the last three turns at 33⅓ (5.4 s) wrapped
//!   round the record, the newest under the stylus, older turns outward and dimmer — one
//!   turn is one real revolution. The **bits** step its edges, the **rate** cuts it into
//!   facets.
//! - **Wow** is an eccentric pressing: the record sits off its spindle and swings round it;
//!   the platter's strobe marks sway with the speed. **Flutter** shakes the stylus.
//! - **Age** scratches the vinyl and yellows the label; **noise** is dust (a speck flashes
//!   at the stylus when a tick falls), a fine grain of hiss, and the hum's ripple round the
//!   label. **Tone** prints its medium on the label; **Character** its year.
//! - **Dragged**: turn the record back (anticlockwise) to age everything that is on — the
//!   Character macro; a double-click makes it new again.

use super::super::frame;
use super::super::host::FxHost;
use super::State;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::skin::records as paint;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::modfx_clock::hash64;
use cantus_dsp::vetus::{t, Medium, VetusSettings};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense, Vec2};
use std::f32::consts::{PI, TAU};

/// Samples a segment of the groove holds at 48 kHz, and a turn's segments (33⅓ a minute).
const SEG: usize = 128;
pub const PER_TURN: usize = 675;
const TURNS: usize = 3;

/// The output's envelope over the last turns: a peak per segment, newest last.
#[derive(Clone, Debug)]
pub struct Groove {
    peaks: Vec<f32>,
    acc: f32,
    count: usize,
    /// The last pair taken from the analyser tap (its bits), to find what is new.
    last: Option<(u32, u32)>,
}

impl Default for Groove {
    fn default() -> Self {
        Groove {
            peaks: Vec::with_capacity(PER_TURN * TURNS),
            acc: 0.0,
            count: 0,
            last: None,
        }
    }
}

impl Groove {
    fn push(&mut self, y: f32) {
        self.acc = self.acc.max(y.abs());
        self.count += 1;
        if self.count == SEG {
            if self.peaks.len() == PER_TURN * TURNS {
                self.peaks.remove(0);
            }
            self.peaks.push(self.acc);
            self.acc = 0.0;
            self.count = 0;
        }
    }

    /// What the analyser tap brought since the last frame (its output, the pairs' second).
    pub fn update(&mut self, pairs: &std::collections::VecDeque<(f32, f32)>) {
        let Some(&(a, b)) = pairs.back() else {
            return;
        };
        let sig = (a.to_bits(), b.to_bits());
        if self.last == Some(sig) {
            return;
        }
        let from = self
            .last
            .and_then(|l| {
                pairs
                    .iter()
                    .rev()
                    .position(|p| (p.0.to_bits(), p.1.to_bits()) == l)
                    .map(|back| pairs.len() - back)
            })
            .unwrap_or(0);
        for p in pairs.iter().skip(from) {
            self.push(p.1);
        }
        self.last = Some(sig);
    }

    /// The peaks, oldest first (a segment's each).
    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }

    /// The groove as a playing engine would have cut it (pictures with no engine).
    pub fn seed(&mut self, out: &[f32]) {
        *self = Groove::default();
        for y in out {
            self.push(*y);
        }
    }
}

/// What the record shows: the settings after Character and the telemetry.
pub struct Look {
    pub s: VetusSettings,
    /// The record's turn now, 0 … 1.
    pub turn: f32,
    pub flutter: f32,
    pub speed_ct: f32,
    pub clicks: f32,
    pub playing: bool,
}

impl Look {
    pub fn of(s: &VetusSettings, tel: &[f32; 64], live: bool) -> Look {
        Look {
            s: s.aged(),
            turn: if live { tel[t::TURN] } else { 0.0 },
            flutter: if live { tel[t::FLUT] } else { 0.0 },
            speed_ct: if live { tel[t::SPEED_CT] } else { 0.0 },
            clicks: if live { tel[t::CLICKS] } else { 0.0 },
            playing: live,
        }
    }
}

/// The record's geometry in a rect.
#[derive(Copy, Clone, Debug)]
pub struct Geo {
    /// The spindle and the record's radius.
    pub c: Pos2,
    pub r: f32,
    /// Where the stylus sits.
    pub stylus: Pos2,
}

/// The stylus's angle and radius share.
const STYLUS_A: f32 = -0.30;
const STYLUS_R: f32 = 0.68;
/// The groove's pitch between turns, of the radius.
const PITCH: f32 = 0.085;

pub fn geo(rect: Rect) -> Geo {
    let r = ((rect.height() * 0.5 - 10.0) / 1.08)
        .min(rect.width() * 0.40)
        .max(20.0);
    let c = pos2(rect.left() + r * 1.08 + 12.0, rect.center().y + 4.0);
    let stylus = c + vec2(STYLUS_A.cos(), STYLUS_A.sin()) * (r * STYLUS_R);
    Geo { c, r, stylus }
}

fn rgb(h: crate::kit::color::Hsl) -> Color32 {
    h.rgb()
}

/// A point at angle `a` (screen radians, clockwise from +x) and radius `r` round `c`.
#[inline]
fn polar(c: Pos2, r: f32, a: f32) -> Pos2 {
    c + vec2(a.cos(), a.sin()) * r
}

/// A uniform 0 … 1 from a hash of `i` and `salt`.
fn u(i: u64, salt: u64) -> f32 {
    (hash64(i.wrapping_mul(0x9E37_79B9) ^ salt) >> 40) as f32 / (1u64 << 24) as f32
}

/// The record, the turntable and the arm.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, look: &Look, groove: &Groove, k: f32) {
    let g = geo(rect);
    let (c, r) = (g.c, g.r);
    let v = &look.s;
    let accent = s.hue;
    let second = s.skin.accent2.map_or(accent, |p| p.get(true).rgb());
    let spin = look.turn * TAU;
    // The eccentric pressing: wow pushes the record off its spindle, swinging round it.
    let ecc = if v.wob_on {
        (v.wow_ct / 100.0).min(1.0) * 0.045 * r
    } else {
        0.0
    };
    let rc = c + vec2(spin.cos(), spin.sin()) * ecc;
    // The well, darker to its edges, and the platter.
    b.rect(rect, Fill::Solid(rgb(paint::PLINTH)));
    b.radial(
        rect,
        rect.center(),
        rect.size() * 0.75,
        &[
            (0.0, with_alpha(rgb(paint::PLINTH_EDGE), 0.0)),
            (0.6, with_alpha(rgb(paint::PLINTH_EDGE), 0.0)),
            (1.0, with_alpha(rgb(paint::PLINTH_EDGE), 0.9)),
        ],
        16,
    );
    b.circle(c, r * 1.065, Fill::Solid(rgb(paint::PLATTER)));
    b.circle_stroke(
        c,
        r * 1.065,
        1.0 * k,
        with_alpha(rgb(paint::PLATTER_LIGHT), 0.55),
    );
    // The strobe marks: they stand still at the right speed and sway with the wow.
    let sway = look.speed_ct * 0.004;
    for i in 0..120 {
        let a = i as f32 / 120.0 * TAU + spin + sway;
        let (p0, p1) = (polar(c, r * 1.018, a), polar(c, r * 1.05, a));
        b.segment(p0, p1, 0.8 * k, with_alpha(rgb(paint::STROBE), 0.42), false);
    }
    // The vinyl, its edge, its grooves and the gaps between the tracks.
    b.circle(rc, r, Fill::Solid(rgb(paint::VINYL)));
    b.circle_stroke(rc, r - 0.6 * k, 1.2 * k, rgb(paint::VINYL_EDGE));
    let step = (1.9 * k).max(1.2);
    let mut rr = r * 0.36;
    let mut i = 0;
    while rr < r * 0.985 {
        let a = if i % 2 == 0 { 0.055 } else { 0.095 };
        b.circle_stroke(rc, rr, 0.5 * k, with_alpha(rgb(paint::GROOVE), a));
        rr += step;
        i += 1;
    }
    for gap in [0.53f32, 0.80] {
        b.circle_stroke(rc, r * gap, 1.6 * k, with_alpha(rgb(paint::VINYL), 1.0));
        b.circle_stroke(rc, r * gap, 0.7 * k, with_alpha(rgb(paint::GROOVE), 0.22));
    }
    sheen(b, rc, r, k, v);
    marks(b, rc, r, spin, v, k);
    music(b, &g, rc, groove, look, accent, k);
    label(b, s, c, rc, r, v, second, k);
    arm(b, &g, look, k);
    if look.clicks > 0.0 && v.noise_on {
        let a = (look.clicks / 6.0).min(1.0);
        b.circle_stroke(
            g.stylus,
            5.0 * k,
            0.8 * k,
            with_alpha(rgb(paint::FLASH), 0.6 * a),
        );
        b.dot(g.stylus, 1.6 * k, with_alpha(rgb(paint::FLASH), 0.9 * a));
    }
}

/// The light the vinyl gives back: two soft wedges across from each other, duller as it
/// ages.
fn sheen(b: &mut Brush, c: Pos2, r: f32, k: f32, v: &VetusSettings) {
    let worn = 1.0 - 0.5 * (v.character + if v.age_on { v.age.wear } else { 0.0 }).min(1.0);
    let (r0, r1) = (r * 0.37, r * 0.975);
    for centre in [-PI * 0.30, PI * 0.70] {
        for (span, a) in [
            (0.40f32, 0.020),
            (0.28, 0.024),
            (0.18, 0.028),
            (0.10, 0.034),
            (0.045, 0.04),
        ] {
            b.arc(
                c,
                (r0 + r1) * 0.5,
                centre - span,
                centre + span,
                r1 - r0,
                with_alpha(rgb(paint::SHEEN), a * worn),
                false,
            );
        }
    }
    let _ = k;
}

/// What the medium wears on its surface: dust, the grain of hiss, scratches, the hum's
/// ripple — every mark fixed to the record, so it turns with it.
fn marks(b: &mut Brush, c: Pos2, r: f32, spin: f32, v: &VetusSettings, k: f32) {
    if v.noise_on {
        let grains = (v.noise.hiss * 260.0) as u64;
        for i in 0..grains {
            let rr = r * (0.38 + 0.6 * u(i, 1));
            let a = u(i, 2) * TAU + spin;
            b.dot(
                polar(c, rr, a),
                0.45 * k,
                with_alpha(rgb(paint::GROOVE), 0.16),
            );
        }
        let specks = (v.noise.crackle * 48.0) as u64;
        for i in 0..specks {
            let rr = r * (0.39 + 0.58 * u(i, 3));
            let a = u(i, 4) * TAU + spin;
            let size = (0.55 + 0.8 * u(i, 5)) * k;
            b.dot(
                polar(c, rr, a),
                size,
                with_alpha(rgb(paint::DUST), 0.45 + 0.35 * u(i, 6)),
            );
        }
        if v.noise.hum > 0.0 {
            let n = if v.noise.hum_60 { 60.0 } else { 50.0 };
            let amp = v.noise.hum * 0.012 * r;
            let pts: Vec<Pos2> = (0..=360)
                .map(|j| {
                    let a = j as f32 / 360.0 * TAU;
                    polar(c, r * 0.375 + amp * (a * n).sin(), a + spin)
                })
                .collect();
            b.polyline(&pts, 0.8 * k, with_alpha(rgb(paint::GROOVE), 0.55));
        }
    }
    if v.age_on {
        let n = (v.age.drop * 10.0 + v.age.wear * 5.0) as u64;
        for i in 0..n {
            let rr = r * (0.42 + 0.54 * u(i, 7));
            let a0 = u(i, 8) * TAU + spin;
            let span = 0.12 + 0.5 * u(i, 9);
            b.arc(
                c,
                rr,
                a0,
                a0 + span,
                0.7 * k,
                with_alpha(rgb(paint::SCRATCH), 0.16 + 0.2 * u(i, 10)),
                true,
            );
        }
    }
}

/// The music in the groove: a turn per 1.8 s, the newest at the stylus.
fn music(b: &mut Brush, g: &Geo, c: Pos2, groove: &Groove, look: &Look, accent: Color32, k: f32) {
    let v = &look.s;
    let n = groove.peaks.len();
    if n < 2 {
        return;
    }
    let r = g.r;
    let a_stylus = STYLUS_A;
    let rate_hold = if v.rate_on {
        let facets = (PER_TURN as f32 * v.rate_hz / 48_000.0).clamp(24.0, PER_TURN as f32);
        (PER_TURN as f32 / facets).round().max(1.0) as usize
    } else {
        1
    };
    let levels = v
        .bits_on
        .then(|| cantus_dsp::math::exp2(v.bits * 0.5).clamp(2.0, 18.0));
    let width = 0.03 * r;
    for turn in 0..TURNS {
        let mut top = Vec::with_capacity(PER_TURN + 1);
        let mut bot = Vec::with_capacity(PER_TURN + 1);
        for j in 0..=PER_TURN {
            let age = turn * PER_TURN + j;
            if age >= n {
                break;
            }
            let held = age - age % rate_hold;
            let peak = groove.peaks[n - 1 - held.min(n - 1)];
            let mut amp = peak.min(1.0).sqrt();
            if let Some(l) = levels {
                amp = (amp * l).round() / l;
            }
            let along = age as f32 / PER_TURN as f32;
            let a = a_stylus + along * TAU;
            let rr = r * (STYLUS_R + PITCH * along);
            top.push(polar(c, rr + amp * width, a));
            bot.push(polar(c, rr - amp * width, a));
        }
        if top.len() < 2 {
            break;
        }
        let fade = [0.85f32, 0.5, 0.28][turn];
        b.between(&top, &bot, with_alpha(accent, 0.38 * fade));
        b.polyline(
            &top,
            (if turn == 0 { 1.0 } else { 0.8 }) * k,
            with_alpha(accent, fade),
        );
        b.polyline(&bot, 0.6 * k, with_alpha(accent, 0.6 * fade));
    }
}

/// The label: its paper yellowed by wear and Character, the name, the medium, the year;
/// the spindle through it (off its centre when the record is eccentric).
#[allow(clippy::too_many_arguments)]
fn label(
    b: &mut Brush,
    s: &Style,
    spindle: Pos2,
    c: Pos2,
    r: f32,
    v: &VetusSettings,
    second: Color32,
    k: f32,
) {
    let age = (0.8 * if v.age_on { v.age.wear } else { 0.0 } + 0.6 * v.character).min(1.0);
    let paper = rgb(paint::LABEL_NEW).lerp_to_gamma(rgb(paint::LABEL_OLD), age);
    let lr = r * 0.33;
    b.circle(c, lr, Fill::Solid(paper));
    b.circle_stroke(
        c,
        lr * 0.92,
        0.8 * k,
        with_alpha(rgb(paint::LABEL_ACCENT), 0.8),
    );
    b.circle_stroke(
        c,
        lr * 0.89,
        0.5 * k,
        with_alpha(rgb(paint::LABEL_ACCENT), 0.6),
    );
    let ink = rgb(paint::LABEL_INK);
    let name = Text::new("VETUS", Role::Section, ink).caps(false);
    super::super::canvas::put(
        b,
        c + vec2(0.0, -lr * 0.42),
        Align2::CENTER_CENTER,
        &name,
        k,
        None,
    );
    let medium = if v.tone_on {
        frame::tr(super::MEDIUM_KEYS[v.medium as usize])
    } else {
        frame::tr("vetus.label.record")
    };
    let m = Text::new(medium, Role::Micro, ink).caps(false);
    super::super::canvas::put(
        b,
        c + vec2(0.0, lr * 0.30),
        Align2::CENTER_CENTER,
        &m,
        k,
        None,
    );
    let year = super::year_text(v.character);
    let y = Text::new(
        &year,
        Role::Micro,
        with_alpha(rgb(paint::LABEL_ACCENT), 1.0),
    )
    .caps(false);
    super::super::canvas::put(
        b,
        c + vec2(0.0, lr * 0.58),
        Align2::CENTER_CENTER,
        &y,
        k,
        None,
    );
    let _ = (s, second, Medium::Tape);
    // The spindle and the hole round it.
    b.dot(spindle, r * 0.032, with_alpha(rgb(paint::VINYL), 0.9));
    b.dot(spindle, r * 0.020, rgb(paint::SPINDLE));
}

/// The arm: its base and counterweight, the tube with its shadow, the cartridge on the
/// groove (shaken by the flutter).
fn arm(b: &mut Brush, g: &Geo, look: &Look, k: f32) {
    let r = g.r;
    let pivot = g.c + vec2(r * 0.98, -r * 0.80);
    let shake = if look.s.wob_on {
        look.flutter * 0.7 * k
    } else {
        0.0
    };
    let tip = g.stylus + vec2(shake, -shake * 0.5);
    let dir = (tip - pivot).normalized();
    let head = tip - dir * (r * 0.16);
    let bend = vec2(-dir.y, dir.x);
    let elbow = head + bend * (r * 0.035);
    let counter = pivot - dir * (r * 0.20);
    let shadow = with_alpha(rgb(paint::ARM_SHADOW), 0.45);
    let off = vec2(2.5 * k, 3.5 * k);
    b.segment(counter + off, elbow + off, 5.0 * k, shadow, true);
    b.circle(pivot, r * 0.115, Fill::Solid(rgb(paint::ARM_DARK)));
    b.circle_stroke(pivot, r * 0.115, 1.0 * k, with_alpha(rgb(paint::ARM), 0.7));
    b.circle(counter, r * 0.07, Fill::Solid(rgb(paint::ARM_DARK)));
    b.circle_stroke(counter, r * 0.07, 0.8 * k, with_alpha(rgb(paint::ARM), 0.8));
    b.segment(counter, elbow, 3.2 * k, rgb(paint::ARM_DARK), true);
    b.segment(counter, elbow, 1.8 * k, rgb(paint::ARM), true);
    b.circle(pivot, r * 0.045, Fill::Solid(rgb(paint::ARM)));
    // The headshell and the cartridge, pointing down the groove.
    let across = bend * (r * 0.032);
    let shell = [
        elbow + across,
        tip + across * 0.7,
        tip - across * 0.7,
        elbow - across,
    ];
    b.polygon(&shell.map(|p| p + off * 0.6), shadow);
    b.polygon(&shell, rgb(paint::CARTRIDGE));
    b.segment(elbow, tip, 0.8 * k, with_alpha(rgb(paint::ARM), 0.8), true);
    b.dot(tip, 1.4 * k, rgb(paint::ARM));
}

/// The drag on the record: turning it back (anticlockwise) ages everything that is on.
pub fn edit(host: &mut dyn FxHost, st: &mut State, ui: &egui::Ui, rect: Rect) {
    let g = geo(rect);
    edit_at(host, st, ui, g.c, g.r * 1.05);
}

/// The turn-back drag round `c` within `reach` (the record's, or the cassette's window).
pub fn edit_at(host: &mut dyn FxHost, st: &mut State, ui: &egui::Ui, c: Pos2, reach: f32) {
    let g = Geo {
        c,
        r: reach / 1.05,
        stylus: c,
    };
    let disc = Rect::from_center_size(g.c, Vec2::splat(reach * 2.0));
    let resp = ui.interact(disc, ui.id().with("vetus-record"), Sense::click_and_drag());
    let inside = |p: Pos2| (p - g.c).length() <= reach;
    if resp.double_clicked() && resp.interact_pointer_pos().is_some_and(inside) {
        host.set("char", 0.0, None);
        st.turning = None;
        return;
    }
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos().filter(|p| inside(*p)) {
            let a = (p - g.c).angle();
            st.turning = Some((a, host.spec().get("char")));
        }
    }
    if let (Some((a0, c0)), Some(p)) = (st.turning, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let a = (p - g.c).angle();
            let mut d = a - a0;
            while d > PI {
                d -= TAU;
            }
            while d < -PI {
                d += TAU;
            }
            // A whole turn back is the whole of Character; the start moves with the hand so
            // turning on round the record keeps going.
            let v = (c0 - d / TAU).clamp(0.0, 1.0);
            st.turning = Some((a, v));
            host.set("char", v, Some("char"));
        }
    }
    if resp.drag_stopped() && st.turning.take().is_some() {
        host.end_gesture();
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("vetus.record_tip"),
            frame::tr("vetus.record_how"),
            Caps::NONE,
        );
    }
}
