//! Nitor's corona (design sheet v1): the dark glass at the top of the face, dark in every
//! theme. The one knob sits in its middle like a sun:
//!
//! - **the halo** behind it brightens with the amount;
//! - **the ring** around it is the character's four bands — lows (lower left), low mids,
//!   presence, air (lower right), in a three-quarter circle — each arc as long as its
//!   share and lit by its gain now: sun gold where the character lifts, dawn rose where it
//!   holds back, its dB beside it;
//! - **the rays** beyond the ring are the exciter, in three fans (body, presence, air), each
//!   ray's length the band's level in force (the character's and the knob's);
//! - **the horizon** under the sun is the low end: the sub's wave on the song's tonic (or
//!   the octave it follows), the note's name large, the tightening's dip, the lift's mark.
//!
//! Every number from the engine's telemetry (`cantus_engine::fx::nitor`'s `T_*` slots),
//! smoothed for the eye in [`View`].

use super::super::canvas::{self, Ink};
use crate::kit::color::{ensure_contrast, mix, with_alpha};
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_engine::fx::nitor as t;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};

/// The bands' words, lowest first.
pub const BAND_KEYS: [&str; 4] = [
    "nitor.d.lows",
    "nitor.d.mids",
    "nitor.d.presence",
    "nitor.d.air",
];
/// The exciter's fans' words.
pub const FAN_KEYS: [&str; 3] = ["fx.nitor.body", "fx.nitor.pres", "fx.nitor.air"];

/// What the engine reports, read once a frame.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub seen: bool,
    pub active: bool,
    pub bands_db: [f32; 4],
    /// The exciter's level in force per band, 0 … 1.
    pub excite: [f32; 3],
    pub sub_db: f32,
    pub harm_db: f32,
    pub tight_db: f32,
    pub sub_hz: f32,
    pub tonic: u8,
    pub song: Option<u8>,
    pub tonic_hz: f32,
    pub match_db: f32,
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        let mut r = Reading {
            seen: tel[t::T_BLOCKS] > 0.0,
            active: tel[t::T_ACTIVE] > 0.5,
            sub_db: tel[t::T_SUB],
            harm_db: tel[t::T_HARM],
            tight_db: tel[t::T_TIGHT],
            sub_hz: tel[t::T_SUB_HZ],
            tonic: tel[t::T_TONIC].round().clamp(0.0, 11.0) as u8,
            song: (tel[t::T_SONG] >= 0.0).then(|| tel[t::T_SONG].round() as u8),
            tonic_hz: tel[t::T_TONIC_HZ],
            match_db: tel[t::T_MATCH],
            ..Reading::default()
        };
        r.bands_db.copy_from_slice(&tel[t::T_BANDS..t::T_BANDS + 4]);
        r.excite.copy_from_slice(&tel[t::T_EXCITE..t::T_EXCITE + 3]);
        r
    }
}

/// What the eye sees: the readings eased (the rays fall back slowly, as a light dims).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct View {
    pub rays: [f32; 3],
    pub bands: [f32; 4],
    pub sub: f32,
    pub tight: f32,
}

impl View {
    pub fn update(&mut self, r: &Reading, dt: f32) {
        let ease = |v: &mut f32, to: f32, up: f32, down: f32| {
            let k = if to > *v { up } else { down };
            *v += (to - *v) * (1.0 - (-dt / k).exp());
        };
        for i in 0..3 {
            let to = r.excite[i].clamp(0.0, 1.0);
            ease(&mut self.rays[i], to, 0.03, 0.35);
        }
        for i in 0..4 {
            ease(&mut self.bands[i], r.bands_db[i], 0.05, 0.25);
        }
        ease(
            &mut self.sub,
            ((r.sub_db + 60.0) / 54.0).clamp(0.0, 1.0),
            0.03,
            0.3,
        );
        ease(&mut self.tight, r.tight_db.min(0.0), 0.02, 0.2);
    }
}

/// Where the corona's parts stand.
#[derive(Copy, Clone, Debug)]
pub struct Parts {
    pub glass: Rect,
    /// The characters' facets, down the left, and the chosen one's words under them.
    pub facets: [Rect; 5],
    pub what: Rect,
    /// The sun: its centre, the knob's box, the ring's radius.
    pub sun: Pos2,
    pub knob: Rect,
    pub ring: f32,
    /// The low end's readout, down the right.
    pub horizon: Rect,
}

pub fn parts(glass: Rect, k: f32) -> Parts {
    let side = 250.0 * k;
    let pad = 16.0 * k;
    let left = Rect::from_min_max(
        pos2(glass.left() + pad, glass.top() + pad),
        pos2(glass.left() + pad + side, glass.bottom() - pad),
    );
    let fh = 44.0 * k;
    let gap = 7.0 * k;
    let mut facets = [Rect::NOTHING; 5];
    for (i, f) in facets.iter_mut().enumerate() {
        let y = left.top() + i as f32 * (fh + gap);
        *f = Rect::from_min_size(pos2(left.left(), y), vec2(side, fh));
    }
    let what = Rect::from_min_max(pos2(left.left(), facets[4].bottom() + 12.0 * k), left.max);
    let horizon = Rect::from_min_max(
        pos2(glass.right() - pad - side, glass.top() + pad),
        pos2(glass.right() - pad, glass.bottom() - pad),
    );
    let mid_l = left.right() + pad;
    let mid_r = horizon.left() - pad;
    let sun = pos2(0.5 * (mid_l + mid_r), glass.top() + 0.56 * glass.height());
    // Room for the rays (up to 100 points) and their words over the ring.
    let ring = ((0.5 * (mid_r - mid_l)) - 120.0 * k)
        .min(sun.y - glass.top() - 128.0 * k)
        .max(96.0 * k);
    let knob = Rect::from_center_size(sun + vec2(0.0, 4.0 * k), vec2(120.0 * k, 132.0 * k));
    Parts {
        glass,
        facets,
        what,
        sun,
        knob,
        ring,
        horizon,
    }
}

/// The note names' keys, by tonic (0 = C): short, in the interface's language.
const NOTES: [&str; 12] = [
    "nitor.note.0",
    "nitor.note.1",
    "nitor.note.2",
    "nitor.note.3",
    "nitor.note.4",
    "nitor.note.5",
    "nitor.note.6",
    "nitor.note.7",
    "nitor.note.8",
    "nitor.note.9",
    "nitor.note.10",
    "nitor.note.11",
];

/// The colours of the corona: its glass, the sun gold, the dawn rose. Its quiet words (the
/// bands' names, the readout's labels) are the display's dim ink held to the kit's text
/// floor, 4.5:1 on the glass (r41:no-elision: the phosphor's dim was 3.8:1 in every theme).
fn voices(s: &Style) -> (Ink, Color32, Color32) {
    let mut ink = Ink::of(s);
    ink.dim = ensure_contrast(ink.dim, ink.ground, 4.5);
    // The sheet's second voice is the sun gold, its lead the dawn rose (`side_r41.rs`).
    (ink, ink.voice(s.hue, true), ink.voice(s.hue, false))
}

/// The glass, the halo, the ring and the rays (the knob is the caller's, over it).
pub fn paint(b: &mut Brush, s: &Style, p: &Parts, v: &View, rd: &Reading, amount: f32) {
    let k = s.k;
    let (ink, gold, rose) = voices(s);
    canvas::ground_b(b, p.glass, &ink, None);
    let clip = p.glass.shrink(1.0);
    // The halo: the amount's light, a warm bloom fading to the glass.
    let a = amount.clamp(0.0, 1.0);
    let bloom = 0.10 + 0.45 * a;
    b.radial(
        clip,
        p.sun,
        Vec2::splat(p.ring * (1.9 + 0.6 * a)),
        &[
            (0.0, with_alpha(gold, bloom)),
            (0.45, with_alpha(mix(gold, rose, 0.5), bloom * 0.45)),
            (1.0, with_alpha(rose, 0.0)),
        ],
        24,
    );
    rays(b, p, v, gold, rose, &ink, k);
    ring(b, p, v, rd, gold, rose, &ink, k);
}

/// The exciter's fans: 3 × 15 rays from just outside the ring.
fn rays(b: &mut Brush, p: &Parts, v: &View, gold: Color32, rose: Color32, ink: &Ink, k: f32) {
    use std::f32::consts::PI;
    // The fans over the top half: body on the left, presence above, air on the right.
    let fans = [
        (-PI * 0.95, -PI * 0.68),
        (-PI * 0.63, -PI * 0.37),
        (-PI * 0.32, -PI * 0.05),
    ];
    for (i, (a0, a1)) in fans.iter().enumerate() {
        let n = 15;
        let lit = v.rays[i];
        for j in 0..n {
            let u = j as f32 / (n - 1) as f32;
            let ang = a0 + (a1 - a0) * u;
            // A gentle crown: the middle rays a little longer.
            let crown = 1.0 - 0.35 * (2.0 * u - 1.0).powi(2);
            let r0 = p.ring + 16.0 * k;
            let len = (10.0 + 88.0 * lit * crown) * k;
            let dir = Vec2::angled(ang);
            let c = mix(rose, gold, 0.35 + 0.65 * lit);
            b.segment(
                p.sun + dir * r0,
                p.sun + dir * (r0 + 8.0 * k),
                1.4 * k,
                with_alpha(ink.dim, 0.5),
                true,
            );
            if lit > 0.002 {
                b.segment(
                    p.sun + dir * r0,
                    p.sun + dir * (r0 + len),
                    (1.2 + 1.2 * lit) * k,
                    with_alpha(c, 0.35 + 0.6 * lit),
                    true,
                );
            }
        }
        let mid = Vec2::angled(0.5 * (a0 + a1));
        canvas::put(
            b,
            p.sun + mid * (p.ring + 118.0 * k),
            Align2::CENTER_CENTER,
            &Text::key(FAN_KEYS[i], Role::Micro, with_alpha(gold, 0.9)),
            k,
            None,
        );
    }
}

/// The four bands: a three-quarter ring, each arc lit by its gain now.
#[allow(clippy::too_many_arguments)]
fn ring(
    b: &mut Brush,
    p: &Parts,
    v: &View,
    rd: &Reading,
    gold: Color32,
    rose: Color32,
    ink: &Ink,
    k: f32,
) {
    use std::f32::consts::PI;
    // From the lower left (135°) round through the top to the lower right (45°).
    let start = PI * 0.75;
    let span = PI * 1.5;
    let gap = 0.05;
    for (i, &band_key) in BAND_KEYS.iter().enumerate() {
        let a0 = start + span * i as f32 / 4.0 + gap;
        let a1 = start + span * (i + 1) as f32 / 4.0 - gap;
        b.arc(
            p.sun,
            p.ring,
            a0,
            a1,
            6.0 * k,
            with_alpha(ink.grid_strong, 0.9),
            true,
        );
        let db = v.bands[i];
        let share = (db.abs() / 8.0).clamp(0.0, 1.0);
        if share > 0.01 {
            let c = if db > 0.0 { gold } else { rose };
            let mid = 0.5 * (a0 + a1);
            let half = 0.5 * (a1 - a0) * share;
            b.arc(p.sun, p.ring, mid - half, mid + half, 6.0 * k, c, true);
        }
        let mid = Vec2::angled(0.5 * (a0 + a1));
        let at = p.sun + mid * (p.ring - 30.0 * k);
        canvas::put(
            b,
            at - vec2(0.0, 7.0 * k),
            Align2::CENTER_CENTER,
            &Text::key(band_key, Role::Micro, ink.dim),
            k,
            None,
        );
        if rd.seen && rd.active {
            canvas::putf(
                b,
                at + vec2(0.0, 7.0 * k),
                Align2::CENTER_CENTER,
                Role::Micro,
                ink.text,
                k,
                None,
                format_args!("{db:+.1} dB"),
            );
        }
    }
}

/// The low end's readout: the tonic large, where it comes from, the sub's wave and level,
/// the tightening's dip, the level match.
pub fn horizon(b: &mut Brush, s: &Style, p: &Parts, v: &View, rd: &Reading, words: &Words<'_>) {
    let k = s.k;
    let (ink, gold, rose) = voices(s);
    let r = p.horizon;
    b.rrect(r, 6.0 * k, Fill::Solid(with_alpha(ink.plate, 0.55)));
    let mut y = r.top() + 12.0 * k;
    canvas::put(
        b,
        pos2(r.left() + 14.0 * k, y),
        Align2::LEFT_TOP,
        &Text::key("nitor.d.tuned", Role::Micro, ink.dim),
        k,
        None,
    );
    y += 22.0 * k;
    // The tonic, large.
    canvas::put(
        b,
        pos2(r.left() + 14.0 * k, y),
        Align2::LEFT_TOP,
        &Text::key(NOTES[rd.tonic as usize % 12], Role::Hero, gold).caps(false),
        k,
        None,
    );
    canvas::put(
        b,
        pos2(r.right() - 14.0 * k, y + 8.0 * k),
        Align2::RIGHT_TOP,
        &Text::key(words.source, Role::Label, ink.text).caps(false),
        k,
        None,
    );
    canvas::putf(
        b,
        pos2(r.right() - 14.0 * k, y + 30.0 * k),
        Align2::RIGHT_TOP,
        Role::Micro,
        ink.dim,
        k,
        None,
        format_args!("{:.1} Hz", rd.tonic_hz),
    );
    y += 64.0 * k;
    // The sub's wave: a sine whose height is the sub's level now.
    let wave = Rect::from_min_size(
        pos2(r.left() + 14.0 * k, y),
        vec2(r.width() - 28.0 * k, 54.0 * k),
    );
    b.hline(
        wave.left(),
        wave.right(),
        wave.center().y,
        1.0 * k,
        with_alpha(ink.grid, 0.9),
    );
    let mut pts = [Pos2::ZERO; 64];
    let amp = 0.45 * wave.height() * (0.08 + 0.92 * v.sub);
    for (i, q) in pts.iter_mut().enumerate() {
        let u = i as f32 / 63.0;
        let ph = u * std::f32::consts::TAU * 2.0;
        *q = pos2(
            wave.left() + u * wave.width(),
            wave.center().y - amp * ph.sin(),
        );
    }
    canvas::trace(b, &pts, 2.0 * k, mix(gold, rose, 0.25), &ink);
    canvas::put(
        b,
        pos2(wave.left(), wave.bottom() + 4.0 * k),
        Align2::LEFT_TOP,
        &Text::key(words.sub_follows, Role::Micro, ink.dim).caps(false),
        k,
        None,
    );
    if rd.sub_hz > 0.0 {
        canvas::putf(
            b,
            pos2(wave.right(), wave.bottom() + 4.0 * k),
            Align2::RIGHT_TOP,
            Role::Micro,
            ink.text,
            k,
            None,
            format_args!("{:.1} Hz", rd.sub_hz),
        );
    }
    y = wave.bottom() + 30.0 * k;
    // The tightening's dip and the level match, as two lines of label and value.
    let line = |b: &mut Brush, y: f32, key: &str, value: f32, unit: &str, c: Color32| {
        canvas::put(
            b,
            pos2(r.left() + 14.0 * k, y),
            Align2::LEFT_TOP,
            &Text::key(key, Role::Label, ink.dim).caps(false),
            k,
            None,
        );
        canvas::putf(
            b,
            pos2(r.right() - 14.0 * k, y),
            Align2::RIGHT_TOP,
            Role::Value,
            c,
            k,
            None,
            format_args!("{value:+.1} {unit}"),
        );
    };
    line(b, y, "nitor.d.tight", v.tight, "dB", rose);
    line(
        b,
        y + 24.0 * k,
        "nitor.d.match",
        rd.match_db,
        "dB",
        ink.text,
    );
    // The parts' light: what the sub, the bass's harmonics and the exciter add now.
    let mut y = y + 58.0 * k;
    let rays = (v.rays[0] + v.rays[1] + v.rays[2]) / 3.0;
    for (key, share, c) in [
        ("fx.nitor.sub", v.sub, gold),
        (
            "fx.nitor.harm",
            ((rd.harm_db + 60.0) / 54.0).clamp(0.0, 1.0),
            gold,
        ),
        ("nitor.card.exciter", rays, rose),
    ] {
        canvas::put(
            b,
            pos2(r.left() + 14.0 * k, y),
            Align2::LEFT_TOP,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            None,
        );
        let bar = Rect::from_min_size(
            pos2(r.left() + 14.0 * k, y + 15.0 * k),
            vec2(r.width() - 28.0 * k, 5.0 * k),
        );
        b.rrect(bar, 2.5 * k, Fill::Solid(with_alpha(ink.grid_strong, 0.9)));
        if share > 0.005 {
            let lit = Rect::from_min_size(bar.min, vec2(bar.width() * share, bar.height()));
            b.rrect(lit, 2.5 * k, Fill::Solid(c));
        }
        y += 32.0 * k;
    }
}

/// The words the horizon reads from the spec.
pub struct Words<'a> {
    /// Where the tonic comes from: the song's key, or the unit's own.
    pub source: &'a str,
    pub sub_follows: &'a str,
}

/// One character's facet: a cut-cornered plate, its name and (when chosen) its line.
pub fn facet(b: &mut Brush, s: &Style, r: Rect, i: usize, chosen: bool, hot: bool) {
    let k = s.k;
    let (ink, gold, rose) = voices(s);
    let cut = 10.0 * k;
    let pts = [
        pos2(r.left() + cut, r.top()),
        pos2(r.right() - cut, r.top()),
        pos2(r.right(), r.center().y),
        pos2(r.right() - cut, r.bottom()),
        pos2(r.left() + cut, r.bottom()),
        pos2(r.left(), r.center().y),
    ];
    let ground = if chosen {
        with_alpha(mix(gold, rose, 0.3), 0.22)
    } else if hot {
        with_alpha(ink.grid_strong, 0.7)
    } else {
        with_alpha(ink.plate, 0.55)
    };
    b.polygon(&pts, ground);
    let edge = if chosen {
        gold
    } else {
        with_alpha(ink.edge, 0.9)
    };
    let mut loop_pts = pts.to_vec();
    loop_pts.push(pts[0]);
    b.polyline(&loop_pts, if chosen { 1.6 * k } else { 1.0 * k }, edge);
    let name = format!("fx.nitor.char.{i}");
    // A jewel: lit on the chosen character.
    let jewel = pos2(r.left() + 26.0 * k, r.center().y);
    b.circle_stroke(jewel, 5.0 * k, 1.2 * k, edge);
    if chosen {
        b.circle(jewel, 3.6 * k, Fill::Solid(gold));
    }
    canvas::put(
        b,
        pos2(r.left() + 42.0 * k, r.center().y),
        Align2::LEFT_CENTER,
        &Text::key(&name, Role::Label, if chosen { gold } else { ink.text }),
        k,
        None,
    );
}

/// The chosen character's line, wrapped under the facets.
pub fn what(b: &mut Brush, s: &Style, r: Rect, chosen: usize) {
    let k = s.k;
    let (ink, _, _) = voices(s);
    let key = format!("fx.nitor.char.{chosen}.what");
    let words = crate::i18n::t(&key);
    let lines = crate::kit::prose::wrap(b.ctx(), &words, Role::Body, k, r.width() - 8.0 * k);
    let _ = crate::kit::prose::paint(
        b,
        r.shrink2(vec2(4.0 * k, 0.0)),
        &lines,
        Role::Body,
        k,
        ink.text,
        Align2::LEFT_TOP,
    );
}
