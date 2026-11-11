//! The piano roll's keyboard as an instrument (round 39, `des-views`; the owner's open item
//! since round 37): ivory and ebony with a real piano's widths. An octave's twelve rows are
//! shared by seven white keys — C, D, E over five rows, F, G, A, B over seven — so only E|F
//! and B|C fall on row lines ([`super::paint::white_rows`]). Each white key is a piece of its
//! own with a 1-px gap under it where the bed shows, its front corners rounded, a lit top
//! and a shade at its foot; the black keys, one row high and 0.60 of the depth, lift off
//! them with a lit upper side and front end and a soft shadow onto the whites. Every C is
//! named in its key, middle C stronger; from 16-px rows every white key is named.
//!
//! The key under the pointer, the key heard by a click and the keys sounding while the
//! song plays each have their own tint (`des-views/README.md`, *Key states*).

use super::{is_black, Geo};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::Weight;
use crate::kit::text::{self, Elide, Face, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::room::{Piano, Room};
use egui::{pos2, Align2, Color32, Rect};

const TOKENS: &[Tok] = &[Tok::KeyWhite, Tok::KeyBlack, Tok::Selection, Tok::Panel];

/// Which keys are lit, by pitch (bit `p` of a 128-bit set).
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct KeyLight {
    /// The row under the pointer.
    pub hover: Option<usize>,
    /// Keys heard by a click on the keyboard, a moment ago.
    pub heard: u128,
    /// Keys sounding under the playhead while the song plays.
    pub sounding: u128,
    /// The track's colour, for the sounding keys.
    pub hue: Color32,
}

impl KeyLight {
    fn has(set: u128, pitch: u8) -> bool {
        pitch < 128 && set & (1u128 << pitch) != 0
    }
}

/// The keyboard's width for a row height in points: 56 at 8-px rows, 72 at 12, 88 at 18
/// (the design's three measures), straight lines between them, held at the ends.
pub(crate) fn width_for(row: f32) -> f32 {
    if row <= 12.0 {
        (72.0 - (12.0 - row) * 4.0).max(48.0)
    } else {
        (72.0 + (row - 12.0) * 16.0 / 6.0).min(104.0)
    }
}

/// A key's face lit by its state: the pointer, a click, the song (`black` picks the
/// stronger mixes the design gives the ebony).
fn lit(
    face: Color32,
    pitch: u8,
    lane: usize,
    light: &KeyLight,
    room: &Room,
    black: bool,
) -> Color32 {
    let (hover, heard, sound) = if black {
        (0.30, 0.70, 0.75)
    } else {
        (0.16, 0.55, 0.55)
    };
    if KeyLight::has(light.heard, pitch) {
        mix(face, room.accent, heard)
    } else if KeyLight::has(light.sounding, pitch) {
        mix(face, light.hue, sound)
    } else if light.hover == Some(lane) {
        mix(face, room.accent, hover)
    } else {
        face
    }
}

/// The keyboard: the bed, the white keys, their depth, the black keys and their shadows,
/// the C names. Drawn on the visible rows only.
pub(crate) fn piano(b: &mut Brush, geo: &Geo<'_>, rect: Rect, light: &KeyLight, s: &Style) {
    let t = &s.t;
    let room = Room::of(t);
    let k = Piano::of(t);
    let rect = b.snap_rect(rect);
    b.register(rect, "keyboard", TOKENS, &[]);
    b.rect(rect, Fill::Solid(k.bed));
    let n = geo.axis.len() as i32;
    let lo = (geo.lane(rect.bottom()).floor() as i32 - 2).max(0);
    let hi = (geo.lane(rect.top()).ceil() as i32 + 2).min(n);
    let px = b.px();
    let row = geo.key_h;
    let rad = if row < 10.0 { 1.5 } else { 2.0 } * s.k;
    // The white keys: each its own piece, the bed showing in a 1-px gap under it; its
    // left end runs under the case (past the clip), so only its front corners are round.
    for lane in lo..hi {
        let pitch = geo.axis.pitch(lane as usize);
        let Some((w0, w1)) = super::paint::white_rows(pitch) else {
            continue;
        };
        let at = pitch as f32 - (pitch % 12) as f32;
        let top = b.snap(geo.y(lane as f32 + (at + w1 - pitch as f32)));
        let bottom = b.snap(geo.y(lane as f32 + (at + w0 - pitch as f32))) - px;
        if bottom <= top {
            continue;
        }
        let key = Rect::from_min_max(
            pos2(rect.left() - 4.0 * s.k, top),
            pos2(rect.right(), bottom),
        );
        let face = lit(k.ivory, pitch, lane as usize, light, &room, false);
        b.rrect(key, rad, Fill::Solid(face));
        let hi_ink = if face == k.ivory {
            k.ivory_hi
        } else {
            mix(face, t.light, 0.25)
        };
        b.hline(rect.left(), rect.right() - rad, top + 0.5 * px, px, hi_ink);
        if bottom - top > 3.0 * px {
            b.hline(
                rect.left(),
                rect.right() - rad,
                bottom - 0.5 * px,
                px,
                mix(face, k.bed, 0.28),
            );
        }
    }
    // The depth over every key: the back in the case's shadow, the front lit (Night) and
    // its very edge falling away.
    let black = t.dark_shade;
    let mut depth = vec![
        (0.0, with_alpha(black, 0.20)),
        (0.10, with_alpha(black, 0.07)),
        (0.55, with_alpha(black, 0.0)),
    ];
    if t.dark {
        depth.push((0.92, with_alpha(t.light, 0.10)));
    } else {
        depth.push((0.92, with_alpha(black, 0.0)));
    }
    depth.push((1.0, with_alpha(black, 0.06)));
    b.hrect(rect, &depth);
    // The black keys over them: a shadow onto the whites, the body from the back's ebony
    // to the lit front, a 1-px lit edge along the upper side and the front end.
    let bw = (rect.width() * 0.60).round();
    let shadow = if t.dark { 0.26 } else { 0.18 };
    for lane in lo..hi {
        let pitch = geo.axis.pitch(lane as usize);
        if !is_black(pitch) {
            continue;
        }
        let y1 = b.snap(geo.y(lane as f32));
        let y0 = b.snap(y1 - row);
        let r = Rect::from_min_max(
            pos2(rect.left() - 2.0 * s.k, y0),
            pos2(rect.left() + bw, y1),
        );
        let soft = |dx: f32, dy: f32| {
            Rect::from_min_max(
                pos2(r.left(), r.top() + dy * s.k),
                pos2(r.right() + dx * s.k, r.bottom() + dy * s.k),
            )
        };
        b.rrect(
            soft(2.0, 1.0),
            1.5 * s.k,
            Fill::Solid(with_alpha(black, shadow * 0.45)),
        );
        b.rrect(
            soft(1.0, 0.5),
            1.5 * s.k,
            Fill::Solid(with_alpha(black, shadow)),
        );
        b.rrect(
            r,
            1.5 * s.k,
            Fill::Solid(lit(k.ebony, pitch, lane as usize, light, &room, true)),
        );
        let body = r.shrink(px);
        if body.height() > 0.0 {
            let (e, top, front) = (k.ebony, k.ebony_top, k.ebony_front);
            let tint = |c| lit(c, pitch, lane as usize, light, &room, true);
            b.hrect(
                body,
                &[(0.0, tint(e)), (0.62, tint(top)), (1.0, tint(front))],
            );
            let edge = tint(k.ebony_lit);
            b.hline(body.left(), body.right(), body.top() + 0.5 * px, px, edge);
            b.vline(body.right() - 0.5 * px, body.top(), body.bottom(), px, edge);
        }
    }
    // The bed's edge toward the grid.
    b.vline(
        rect.right() - 0.5 * px,
        rect.top(),
        rect.bottom(),
        px,
        room.edge,
    );
    names(b, geo, rect, lo, hi, &k, s);
}

/// Every C in its key, right-aligned 5 from the front: middle C SemiBold 10, the others
/// Medium 10; from 16-px rows, every white key in Regular 11, quieter.
fn names(b: &mut Brush, geo: &Geo<'_>, rect: Rect, lo: i32, hi: i32, k: &Piano, s: &Style) {
    let row = geo.key_h;
    if row < 7.0 {
        return;
    }
    let every = row >= 16.0;
    for lane in lo..hi {
        let pitch = geo.axis.pitch(lane as usize);
        let Some((w0, w1)) = super::paint::white_rows(pitch) else {
            continue;
        };
        let c = pitch.is_multiple_of(12);
        if !c && !every {
            continue;
        }
        let at = pitch as f32 - (pitch % 12) as f32;
        let top = geo.y(lane as f32 + (at + w1 - pitch as f32));
        let bottom = geo.y(lane as f32 + (at + w0 - pitch as f32));
        let r = Rect::from_min_max(
            pos2(rect.left() + rect.width() * 0.62, top),
            pos2(rect.right() - 5.0 * s.k, bottom),
        );
        let label = super::pitch_label(pitch);
        let (role, ink, weight, size) = if pitch == 60 {
            (Role::Micro, k.c4, Weight::SemiBold, 10.0 / 9.0)
        } else if c {
            (Role::Label, k.c, Weight::Medium, 10.0 / 11.0)
        } else {
            (Role::Label, k.name, Weight::Regular, 1.0)
        };
        let tx = Text::new(label, role, ink)
            .caps(false)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let face = Face {
            letters: Some((weight, true)),
            ..Face::ROLE
        };
        let _ = text::paint_face(b, r, &tx, s.tk() * size, face);
    }
}
