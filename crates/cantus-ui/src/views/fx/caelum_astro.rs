//! Caelum's astrolabe (design sheet v2): the shimmer's two voices read on a celestial
//! instrument. The **limb** is engraved with the twelve semitones of an octave (the
//! unison at the top, clockwise); the **plate** carries the pitch spiral — −36 … +36
//! semitones winding outward, six turns, every octave on the meridian straight up (so an
//! octave shimmer points up, a fifth to the lower left); the **rete**'s filigree (its
//! ecliptic ring, its star pointers) turns over it; the **alidade** points at the first
//! voice, a slimmer hand at the second, each voice a lit jewel on the spiral. Hung from
//! its throne and shackle. Colours from the colourway's metal (`caelum_art::metal`).

use super::super::canvas::{self, Ink};
use super::super::frame::tr;
use super::super::host::FxHost;
use super::art::{self, c, hash, Signed};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::observatory as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::EffectSpec;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::{FRAC_PI_2, TAU};

/// How far an octave's name stands from the meridian, points at 100 %.
pub(crate) const OCTAVE_NAME_GAP: f32 = 14.0;

/// The angle of `st` semitones on the dial: the unison at the top, clockwise.
fn angle(st: f32) -> f32 {
    -FRAC_PI_2 + st / 12.0 * TAU
}

/// The instrument's geometry in `r`.
struct Dial {
    c: Pos2,
    rad: f32,
}

impl Dial {
    /// The spiral's radius at `st` semitones.
    fn rho(&self, st: f32) -> f32 {
        let (lo, hi) = (self.rad * 0.13, self.rad * 0.74);
        lo + (st.clamp(-36.0, 36.0) + 36.0) / 72.0 * (hi - lo)
    }

    fn at(&self, st: f32, rho: f32) -> Pos2 {
        self.c + Vec2::angled(angle(st)) * rho
    }

    fn on_spiral(&self, st: f32) -> Pos2 {
        self.at(st, self.rho(st))
    }
}

/// Where the dial goes in `r`, leaving room for the title and the legend.
fn dial(r: Rect, k: f32) -> Dial {
    let top = r.top() + 20.0 * k;
    let bottom = r.bottom() - 36.0 * k;
    let rad = ((r.width() - 16.0 * k) * 0.5).min((bottom - top) / 2.3);
    Dial {
        c: pos2(r.center().x, top + rad * 0.3 + rad),
        rad,
    }
}

/// The astrolabe in `r`, its title and its legend.
pub fn show(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, spec: &EffectSpec) {
    let k = s.k;
    let m = art::metal(s);
    canvas::put(
        b,
        r.min,
        Align2::LEFT_TOP,
        &Text::key("caelum.d.voices", Role::Micro, ink.dim),
        k,
        None,
    );
    let d = dial(r, k);
    let (cx, rad) = (d.c, d.rad);
    // A pool of light behind the instrument, so it hangs in front of the deck.
    b.radial(
        r,
        cx,
        Vec2::splat(rad * 1.5),
        &[
            (0.0, with_alpha(ink.text, 0.06)),
            (1.0, with_alpha(ink.text, 0.0)),
        ],
        10,
    );
    b.radial(
        r,
        cx + vec2(0.0, 4.0 * k),
        Vec2::splat(rad * 1.06),
        &[
            (0.88, with_alpha(c(pal::SHADOW), 0.55)),
            (1.0, with_alpha(c(pal::SHADOW), 0.0)),
        ],
        16,
    );
    throne(b, s, cx, rad);
    // The mater and its limb, turned on a lathe.
    b.circle(cx, rad, Fill::V(c(m.edge), c(m.foot)));
    b.circle(
        cx,
        rad * 0.975,
        Fill::V(c(m.top), mix(c(m.face), c(m.foot), 0.4)),
    );
    for i in 0..10u32 {
        let rr = rad * (0.81 + 0.16 * hash(i + 300));
        b.circle_stroke(
            cx,
            rr,
            0.5 * k,
            with_alpha(c(m.grain), 0.05 + 0.1 * hash(i + 320)),
        );
    }
    let plate_r = rad * 0.8;
    b.circle(
        cx,
        plate_r + 1.2 * k,
        Fill::Solid(with_alpha(c(m.engrave), 0.6)),
    );
    // The plate sits a little below the limb: darker at the top, lit at the foot.
    b.circle(
        cx,
        plate_r,
        Fill::V(
            mix(c(m.face), c(m.foot), 0.35),
            mix(c(m.face), c(m.top), 0.25),
        ),
    );
    for i in 0..14u32 {
        let rr = plate_r * (0.1 + 0.88 * hash(i + 340));
        b.circle_stroke(
            cx,
            rr,
            0.5 * k,
            with_alpha(c(m.grain), 0.04 + 0.07 * hash(i + 360)),
        );
    }
    limb(b, s, &d, plate_r);
    plate(b, s, &d, plate_r);
    rete(b, s, &d);
    voices(b, s, ink, &d, spec);
    // The pin through the centre, its wedge.
    b.circle(
        cx + vec2(0.0, 1.0 * k),
        rad * 0.055,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    b.circle(cx, rad * 0.05, Fill::V(c(m.edge), c(m.foot)));
    b.segment(
        cx - vec2(rad * 0.03, 0.0),
        cx + vec2(rad * 0.03, 0.0),
        1.0 * k,
        with_alpha(c(m.engrave), 0.8),
        false,
    );
    legend(b, s, ink, r, &d, spec);
}

/// The throne over the dial and the shackle it hangs by.
fn throne(b: &mut Brush, s: &Style, cx: Pos2, rad: f32) {
    let k = s.k;
    let m = art::metal(s);
    let top = cx.y - rad;
    let h = rad * 0.2;
    let body = [
        pos2(cx.x - rad * 0.26, top + rad * 0.06),
        pos2(cx.x - rad * 0.1, top - h),
        pos2(cx.x + rad * 0.1, top - h),
        pos2(cx.x + rad * 0.26, top + rad * 0.06),
    ];
    b.polygon(
        &body.map(|p| p + vec2(0.0, 2.0 * k)),
        with_alpha(c(pal::SHADOW), 0.45),
    );
    b.polygon(&body, c(m.face));
    b.polyline(
        &[body[0], body[1], body[2], body[3]],
        0.8 * k,
        with_alpha(c(m.edge), 0.7),
    );
    // Its scrolls, one each side, and the boss at its top.
    for side in [-1.0f32, 1.0] {
        let p = pos2(cx.x + side * rad * 0.15, top - h * 0.35);
        b.circle_stroke(p, rad * 0.045, 1.3 * k, c(m.edge));
        b.circle_stroke(p, rad * 0.045, 0.6 * k, with_alpha(c(m.engrave), 0.6));
        b.dot(p, rad * 0.015, c(m.engrave));
    }
    let boss = pos2(cx.x, top - h);
    b.circle(boss, rad * 0.05, Fill::V(c(m.edge), c(m.foot)));
    // The shackle and the ring.
    let ring = pos2(cx.x, boss.y - rad * 0.1);
    b.circle_stroke(
        ring + vec2(0.0, 1.0 * k),
        rad * 0.075,
        2.6 * k,
        with_alpha(c(pal::SHADOW), 0.45),
    );
    b.circle_stroke(ring, rad * 0.075, 2.2 * k, c(m.face));
    b.arc(ring, rad * 0.075, -2.6, -0.6, 1.0 * k, c(m.edge), true);
}

/// The limb's scale: a tick each quarter of a semitone, the twelve semitones numbered.
fn limb(b: &mut Brush, s: &Style, d: &Dial, plate_r: f32) {
    let k = s.k;
    let m = art::metal(s);
    let ink = c(m.engrave);
    b.circle_stroke(d.c, plate_r + 0.5 * k, 0.8 * k, with_alpha(ink, 0.8));
    b.circle_stroke(d.c, d.rad * 0.865, 0.6 * k, with_alpha(ink, 0.6));
    for q in 0..48 {
        let st = q as f32 / 4.0;
        let major = q % 4 == 0;
        let r1 = if major { d.rad * 0.865 } else { d.rad * 0.835 };
        let (a, z) = (d.at(st, plate_r + 0.5 * k), d.at(st, r1));
        art::engrave(b, s, a, z, if major { 1.0 * k } else { 0.6 * k });
    }
    for st in 0..12 {
        let p = d.at(st as f32, d.rad * 0.92);
        canvas::putf(
            b,
            p,
            Align2::CENTER_CENTER,
            Role::Micro,
            ink,
            k,
            None,
            format_args!("{st}"),
        );
    }
    // A star on the octave and the fifth, the shimmer's two classic intervals.
    for st in [0.0f32, 7.0] {
        let p = d.at(st, d.rad * 0.85);
        b.dot(p, 1.4 * k, c(m.edge));
    }
}

/// The plate: the meridian and the horizon line, the pitch spiral with its semitone dots,
/// the octaves numbered along the meridian.
fn plate(b: &mut Brush, s: &Style, d: &Dial, plate_r: f32) {
    let k = s.k;
    let m = art::metal(s);
    let ink = c(m.engrave);
    art::engrave(b, s, d.c, d.at(0.0, plate_r), 0.7 * k);
    art::engrave(
        b,
        s,
        d.c + vec2(-plate_r, 0.0),
        d.c + vec2(plate_r, 0.0),
        0.5 * k,
    );
    // The fifth's ray, dotted.
    let (a, z) = (d.at(7.0, d.rad * 0.13), d.at(7.0, plate_r));
    canvas::dashed(b, a, z, 0.6 * k, with_alpha(ink, 0.6), 2.0 * k, 3.0 * k);
    let pts: [Pos2; 289] = std::array::from_fn(|i| {
        let st = -36.0 + i as f32 * 0.25;
        d.on_spiral(st)
    });
    b.polyline(
        &pts.map(|p| p + vec2(0.0, 0.7 * k)),
        0.6 * k,
        with_alpha(c(m.edge), 0.35),
    );
    b.polyline(&pts, 0.9 * k, with_alpha(ink, 0.85));
    for st in -36..=36 {
        let p = d.on_spiral(st as f32);
        let octave = st % 12 == 0;
        b.dot(
            p,
            if octave { 1.8 * k } else { 0.9 * k },
            with_alpha(ink, 0.9),
        );
    }
    // The octaves along the meridian, alternately left and right of it, each name led out
    // from its dot by an engraved line: the two columns stand well apart (v2 note: the
    // names crowded and touched), so the alidade passes between them.
    for (i, st) in (-36..=36).step_by(12).enumerate() {
        let p = d.on_spiral(st as f32);
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let reach = OCTAVE_NAME_GAP * k;
        art::engrave(
            b,
            s,
            p + vec2(side * 2.5 * k, 0.0),
            p + vec2(side * (reach - 2.0 * k), 0.0),
            0.5 * k,
        );
        let at = p + vec2(side * reach, 0.0);
        let align = if side < 0.0 {
            Align2::RIGHT_CENTER
        } else {
            Align2::LEFT_CENTER
        };
        canvas::putf(
            b,
            at,
            align,
            Role::Micro,
            ink,
            k,
            Some(mix(c(m.face), c(m.top), 0.3)),
            format_args!("{}", Signed(st as f32)),
        );
    }
}

/// The rete: its ecliptic ring and five star pointers, a filigree over the plate.
fn rete(b: &mut Brush, s: &Style, d: &Dial) {
    let k = s.k;
    let m = art::metal(s);
    let ring_c = d.c + vec2(0.0, -d.rad * 0.2);
    let ring_r = d.rad * 0.5;
    let shade = with_alpha(c(pal::SHADOW), 0.4);
    let drop = vec2(0.0, 1.6 * k);
    b.circle_stroke(ring_c + drop, ring_r, 3.0 * k, shade);
    b.circle_stroke(ring_c, ring_r, 2.6 * k, c(m.face));
    b.circle_stroke(
        ring_c,
        ring_r + 0.9 * k,
        0.6 * k,
        with_alpha(c(m.edge), 0.8),
    );
    b.circle_stroke(
        ring_c,
        ring_r - 0.9 * k,
        0.5 * k,
        with_alpha(c(m.engrave), 0.6),
    );
    // The pointers: a flame from the ring outward, a curl at its root.
    for (i, a) in [0.35f32, 1.4, 2.5, 3.7, 4.9].into_iter().enumerate() {
        let dir = Vec2::angled(a);
        let root = ring_c + dir * ring_r;
        let tip = ring_c + dir * (ring_r + d.rad * (0.12 + 0.08 * hash(i as u32 + 60)));
        let n = vec2(-dir.y, dir.x);
        let flame = [root + n * 2.4 * k, tip, root - n * 2.4 * k];
        b.polygon(&flame.map(|p| p + drop), shade);
        b.polygon(&flame, c(m.face));
        b.segment(
            root + n * 2.4 * k,
            tip,
            0.5 * k,
            with_alpha(c(m.edge), 0.8),
            false,
        );
        let curl = root - dir * 3.0 * k;
        b.circle_stroke(curl, 2.2 * k, 0.9 * k, c(m.face));
        b.dot(tip, 0.9 * k, c(m.edge));
    }
}

/// The alidade on the first voice, a slimmer hand on the second, each voice's jewel.
fn voices(b: &mut Brush, s: &Style, ink: &Ink, d: &Dial, spec: &EffectSpec) {
    let k = s.k;
    let m = art::metal(s);
    let shim = spec.get("shim").clamp(0.0, 1.0);
    let (pa, pb, lvl) = (spec.get("pa"), spec.get("pb"), spec.get("pblvl"));
    let glow = ink.voice(ink.hue, false);
    let spark = ink.voice(ink.hue, true);
    let shade = with_alpha(c(pal::SHADOW), 0.45);
    let drop = vec2(0.0, 2.0 * k);
    // The second voice's hand, under the alidade.
    let tip_b = d.at(pb, d.rho(pb) + d.rad * 0.06);
    b.segment(d.c + drop, tip_b + drop, 2.2 * k, shade, true);
    b.segment(d.c, tip_b, 1.6 * k, c(m.face), true);
    b.segment(d.c, tip_b, 0.5 * k, with_alpha(c(m.edge), 0.7), false);
    // The alidade: a rule across the plate, pointed at the first voice's end, its vanes.
    let dir = Vec2::angled(angle(pa));
    let n = vec2(-dir.y, dir.x);
    let reach = d.rad * 0.8;
    let hw = 3.2 * k;
    let rule = [
        d.c - dir * reach * 0.86 + n * hw,
        d.c + dir * (reach - 8.0 * k) + n * hw,
        d.c + dir * reach,
        d.c + dir * (reach - 8.0 * k) - n * hw,
        d.c - dir * reach * 0.86 - n * hw,
    ];
    b.polygon(&rule.map(|p| p + drop), shade);
    b.polygon(&rule, c(m.face));
    b.segment(
        rule[0],
        rule[1],
        0.7 * k,
        with_alpha(c(m.edge), 0.85),
        false,
    );
    b.segment(
        rule[1],
        rule[2],
        0.7 * k,
        with_alpha(c(m.edge), 0.85),
        false,
    );
    b.segment(
        rule[3],
        rule[4],
        0.6 * k,
        with_alpha(c(m.engrave), 0.7),
        false,
    );
    // The fiducial line down its middle and the two sight vanes.
    art::engrave(b, s, d.c, d.c + dir * (reach - 1.0 * k), 0.5 * k);
    for at in [0.52f32, -0.62] {
        let p = d.c + dir * reach * at;
        let vane = [p + n * 5.0 * k, p - n * 5.0 * k];
        b.segment(vane[0] + drop, vane[1] + drop, 3.0 * k, shade, false);
        b.segment(vane[0], vane[1], 2.6 * k, c(m.top), false);
        b.segment(
            vane[0],
            vane[1],
            0.6 * k,
            with_alpha(c(m.engrave), 0.7),
            false,
        );
    }
    // The jewels: the voices' places on the spiral, lit by the shimmer's share.
    for (st, level, col) in [(pa, 1.0, glow), (pb, lvl, spark)] {
        let p = d.on_spiral(st);
        let lit = if shim > 0.0 {
            0.25 + 0.75 * level
        } else {
            0.15
        };
        jewel(b, p, 4.2 * k, col, lit);
    }
}

/// A cut stone set at `p`, lit `lit` (0 … 1) in `col`, its halo on the metal round it.
fn jewel(b: &mut Brush, p: Pos2, rad: f32, col: Color32, lit: f32) {
    b.radial(
        Rect::from_center_size(p, Vec2::splat(rad * 8.0)),
        p,
        Vec2::splat(rad * 3.6),
        &[
            (0.0, with_alpha(col, 0.55 * lit)),
            (0.45, with_alpha(col, 0.16 * lit)),
            (1.0, with_alpha(col, 0.0)),
        ],
        10,
    );
    b.circle(p, rad * 1.3, Fill::Solid(c(pal::GASKET)));
    let dark = mix(col, c(pal::SHADOW), 0.65);
    b.circle(
        p,
        rad,
        Fill::V(
            mix(dark, mix(col, c(pal::STAR), 0.5), lit),
            mix(dark, col, lit * 0.8),
        ),
    );
    b.circle(
        p + vec2(-0.35, -0.4) * rad,
        rad * 0.28,
        Fill::Solid(with_alpha(c(pal::STAR), 0.85)),
    );
}

/// The voices in words under the instrument: "First voice +12 st", "Second voice +7 st ·
/// Second level 30 %".
fn legend(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, d: &Dial, spec: &EffectSpec) {
    let k = s.k;
    let y = (d.c.y + d.rad + 6.0 * k).min(r.bottom() - 30.0 * k);
    let glow = ink.voice(ink.hue, false);
    let spark = ink.voice(ink.hue, true);
    canvas::putf(
        b,
        pos2(r.center().x, y),
        Align2::CENTER_TOP,
        Role::Micro,
        glow,
        k,
        None,
        format_args!("{} {:+.0} st", tr("caelum.l.first"), spec.get("pa")),
    );
    canvas::putf(
        b,
        pos2(r.center().x, y + 14.0 * k),
        Align2::CENTER_TOP,
        Role::Micro,
        spark,
        k,
        None,
        format_args!(
            "{} {:+.0} st \u{b7} {} {:.0} %",
            tr("caelum.l.second"),
            spec.get("pb"),
            tr("caelum.l.secondlevel"),
            100.0 * spec.get("pblvl")
        ),
    );
}

/// Which hand a drag on the astrolabe holds.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Hand {
    /// The alidade: the first voice (`pa`).
    First,
    /// The slimmer hand: the second voice (`pb`).
    Second,
}

impl Hand {
    pub fn key(self) -> &'static str {
        match self {
            Hand::First => "pa",
            Hand::Second => "pb",
        }
    }
}

/// The dial in `r` as the pointer reads it: its centre and radius.
pub fn dial_of(r: Rect, k: f32) -> (Pos2, f32) {
    let d = dial(r, k);
    (d.c, d.rad)
}

/// Where `st` semitones lies on the spiral of a dial at `c` of radius `rad`.
pub fn point_of(c: Pos2, rad: f32, st: f32) -> Pos2 {
    Dial { c, rad }.on_spiral(st)
}

/// The spiral's semitone under `p` (−36 … +36): the angle names the semitone in the octave,
/// the distance from the centre the turn of the spiral nearest to it.
pub fn semitone_at(c: Pos2, rad: f32, p: Pos2) -> f32 {
    let d = Dial { c, rad };
    let v = p - c;
    let a = cantus_dsp::math::atan2(v.y, v.x) + FRAC_PI_2;
    let class = (a / TAU * 12.0).round().rem_euclid(12.0);
    let dist = v.length();
    let mut best = (f32::MAX, 0.0f32);
    for turn in -4..=3 {
        let st = class + 12.0 * turn as f32;
        if !(-36.0..=36.0).contains(&st) {
            continue;
        }
        let miss = (d.rho(st) - dist).abs();
        if miss < best.0 {
            best = (miss, st);
        }
    }
    best.1
}

/// Which hand a press at `p` takes: the hand whose jewel it is on (a jewel lies under the
/// other hand's line when the two voices share a note in the octave), else the one whose
/// line (centre to its jewel) passes nearer.
pub fn hand_at(c: Pos2, rad: f32, spec: &EffectSpec, p: Pos2) -> Hand {
    let (pa, pb) = (spec.get("pa"), spec.get("pb"));
    let jewel = |st: f32| (point_of(c, rad, st) - p).length();
    let on = rad * 0.07;
    let (ja, jb) = (jewel(pa), jewel(pb));
    if ja.min(jb) <= on {
        return if jb < ja { Hand::Second } else { Hand::First };
    }
    let to = |st: f32| {
        let tip = point_of(c, rad, st);
        let seg = tip - c;
        let t = ((p - c).dot(seg) / seg.length_sq().max(1e-6)).clamp(0.0, 1.0);
        (c + seg * t - p).length()
    };
    if to(pb) < to(pa) {
        Hand::Second
    } else {
        Hand::First
    }
}

/// The voices answer the pointer: a press on the plate takes the nearer hand (the alidade
/// the first voice, the slimmer hand the second, heard or not), and
/// dragging turns it to the semitone under the pointer (`semitone_at`); a click puts the
/// nearer hand where it falls. One drag, one step of undo (the cards' pills keep each
/// voice's automation and MIDI learn).
pub fn edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let (c, rad) = dial_of(r, s.k);
    let plate = Rect::from_center_size(c, Vec2::splat(rad * 1.6));
    let id = ui.id().with("caelum-astrolabe");
    let resp = ui.interact(plate, id, egui::Sense::click_and_drag());
    let inside = |p: Pos2| (p - c).length() <= rad * 0.8;
    if resp.drag_started() || resp.clicked() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin
            .or(resp.interact_pointer_pos())
            .filter(|p| inside(*p))
        {
            let hand = hand_at(c, rad, host.spec(), p);
            ui.data_mut(|d| d.insert_temp(id, Some(hand)));
        }
    }
    let held: Option<Hand> = ui.data(|d| d.get_temp(id)).flatten();
    if let (Some(hand), Some(p)) = (
        held,
        resp.interact_pointer_pos()
            .filter(|_| resp.is_pointer_button_down_on() || resp.clicked()),
    ) {
        let st = semitone_at(c, rad, p);
        if host.spec().get(hand.key()) != st {
            host.set(hand.key(), st, Some("caelum-voice"));
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        if held.is_some() {
            host.end_gesture();
        }
        ui.data_mut(|d| d.insert_temp::<Option<Hand>>(id, None));
    }
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            tr("caelum.hint.voices"),
            tr("caelum.hint.voices_tip"),
            crate::kit::hint::Caps::NONE,
        );
    }
}
