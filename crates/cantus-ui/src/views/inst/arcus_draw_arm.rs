//! r42:arcus-upper — the bow arm of the figure that holds an Arcus instrument (a musician's
//! sleeve and hand, the wooden mannequin's), drawn live with the bow: the shoulder is the
//! baked figure's (`scene::bowed_hold::arm`), the hand closes on the frog wherever the bow
//! has travelled, the elbow falls where a two-bone arm puts it.

use super::body::View;
use crate::kit::appearance::rgb;
use crate::kit::color::{mix, with_alpha};
use crate::kit::Brush;
use crate::scene::bowed_hold::{elbow, Arm};
use crate::scene::geo::V3;
use egui::{Pos2, Vec2};

/// The arm from its shoulder to the bow's `frog` (the bow's way `dir`, frog to tip; `n`
/// the string's normal, the side the stick is on).
pub fn right_arm(b: &mut Brush, v: &View, arm: &Arm, frog: V3, dir: V3, n: V3) {
    let hand = frog + n * 2.4 - dir * 1.2;
    let wrist = hand - dir * 4.8 + n * 2.6;
    let el = elbow(arm.shoulder, wrist, arm.upper, arm.fore, arm.pole);
    let (ps, pe, pw, ph) = (v.p(arm.shoulder), v.p(el), v.p(wrist), v.p(hand));
    let sleeve = rgb(arm.sleeve);
    let shade = rgb(arm.sleeve_shade);
    let lit = mix(sleeve, rgb(crate::kit::skin::scene::hold::SHEEN), 0.18);
    let wu = 2.0 * arm.r.0 * v.scale(el);
    let wf = 2.0 * arm.r.1 * v.scale(wrist);
    limb(b, ps, pe, wu, shade, sleeve, lit);
    limb(b, pe, pw, wf, shade, sleeve, lit);
    // The cuff at the wrist, a shade deeper.
    let cuff = el.lerp(wrist, 0.92);
    b.segment(v.p(cuff), pw, wf * 1.08, shade, true);
    // The hand: the palm over the frog, three fingers round the stick, the thumb under it.
    let skin = rgb(arm.hand);
    let skin_shade = rgb(arm.hand_shade);
    let wh = 2.0 * 3.8 * v.scale(hand);
    b.segment(pw, ph, wh * 1.06, skin_shade, true);
    b.segment(pw, ph, wh * 0.86, skin, true);
    let wfi = 1.7 * v.scale(hand);
    for k in 0..4 {
        let a = hand + dir * ((k as f32 - 1.5) * 1.6);
        let tip = a - n * 3.0 + dir * 0.4;
        b.segment(v.p(a), v.p(tip), wfi * 1.15, skin_shade, true);
        b.segment(v.p(a), v.p(tip), wfi * 0.85, skin, true);
    }
    let t0 = hand - dir * 2.0 - n * 0.8;
    let t1 = frog + dir * 0.6 - n * 0.9;
    b.segment(v.p(t0), v.p(t1), wfi * 1.2, skin_shade, true);
    b.segment(v.p(t0), v.p(t1), wfi * 0.9, skin, true);
    // A soft shadow the hand throws on the stick.
    b.segment(
        v.p(frog),
        v.p(frog + dir * 3.0),
        wfi,
        with_alpha(skin_shade, 0.25),
        true,
    );
}

/// A sleeve or a wooden limb: its shade, the colour over it, a highlight along its lit
/// side.
fn limb(
    b: &mut Brush,
    a: Pos2,
    e: Pos2,
    w: f32,
    shade: egui::Color32,
    col: egui::Color32,
    lit: egui::Color32,
) {
    b.segment(a, e, w, shade, true);
    let side = (e - a).normalized();
    let off = Vec2::new(side.y, -side.x) * (w * 0.08);
    let off = if off.y > 0.0 { -off } else { off };
    b.segment(a + off, e + off, w * 0.8, col, true);
    b.segment(
        a + off * 3.5,
        e + off * 3.5,
        w * 0.14,
        with_alpha(lit, 0.3),
        true,
    );
}
