//! The songbird's face: the family's eyes, cheeks and sunglasses (`rig::face`) in its
//! colours, and its own two-part beak that opens to sing.

use super::pal;
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Body;
use crate::rig::draw::{self as d, ellipse_into, Sink};
use crate::rig::face::{self, Face, FaceStyle};
use crate::rig::{self, Ball, OnBall};

const STYLE: FaceStyle = FaceStyle {
    eye_lon: 25.0,
    eye_lat: 14.0,
    eye_rx: 0.082,
    eye_ry: 0.1,
    white: pal::EYE_WHITE,
    pupil: pal::PUPIL,
    iris: pal::IRIS,
    lid: pal::BODY.base,
    ink: pal::OUTLINE,
    cheek_lon: 46.0,
    cheek_lat: -8.0,
    cheek_rx: 0.07,
    cheek: pal::CHEEK,
    skin: pal::BODY.base,
};

/// The beak at `on`, open by `open` (0…1), turned with the ball's yaw.
fn beak(c: &mut Canvas, on: &OnBall, yaw: f32, open: f32, smirk: f32) {
    let o = open.clamp(0.0, 1.0);
    let sq = 0.55 + 0.45 * on.squeeze;
    let w = 0.078 * sq;
    let tip = rig::sin_deg(yaw) * 0.075;
    let at = on.at;
    let saved = c.transform;
    c.transform = saved.pre_concat(crate::rig::rotation_at(-9.0 * smirk, at.0, at.1));
    let ink = pal::OUTLINE.thin(0.8);
    if o > 0.04 {
        let mc = (at.0 + tip * 0.4, at.1 + 0.045 + 0.028 * o);
        let (mrx, mry) = (w * 0.78, 0.016 + 0.05 * o);
        let mouth = |s: &mut dyn Sink, g: f32| ellipse_into(s, mc, mrx + g, mry + g, 0.0);
        d::outline(c, &mouth, &pal::OUTLINE.thin(0.7));
        d::fixed(c, &mouth, pal::MOUTH);
        let tc = (at.0 + tip * 0.4, at.1 + 0.06 + 0.06 * o);
        d::fixed(
            c,
            &|s, _| ellipse_into(s, tc, w * 0.42, 0.012 + 0.022 * o, 0.0),
            pal::TONGUE,
        );
        let lower = [
            (at.0 - w * 0.66, at.1 + 0.04 + 0.03 * o),
            (at.0 + w * 0.66, at.1 + 0.04 + 0.03 * o),
            (at.0 + tip * 0.75, at.1 + 0.07 + 0.075 * o),
        ];
        d::part(
            c,
            &|s, g| d::blob_grown_into(s, &lower, g),
            &pal::BEAK,
            &ink,
            2,
        );
    }
    let upper = [
        (at.0 - w, at.1 + 0.004),
        (at.0, at.1 - 0.034),
        (at.0 + w, at.1 + 0.004),
        (at.0 + tip, at.1 + 0.068 - 0.022 * o),
    ];
    d::part(
        c,
        &|s, g| d::blob_grown_into(s, &upper, g),
        &pal::BEAK,
        &ink,
        2,
    );
    let gl = (at.0 - w * 0.35, at.1 - 0.004);
    d::flat(
        c,
        &|s, _| ellipse_into(s, gl, w * 0.3, 0.01, -8.0),
        d::mix(pal::BEAK.light, [255, 255, 255, 255], 0.6),
    );
    c.transform = saved;
}

/// The whole face on the ball (the canvas already turned by the body's tilt).
pub(super) fn draw(c: &mut Canvas, ball: &Ball, b: &Body, pose: &Pose) {
    let f: Face = face::face_of(pose.mood);
    face::cheeks(c, ball, &STYLE, &f, b, pose);
    face::eyes(c, ball, &STYLE, &f, b, pose);
    let bk = ball.at(0.0, -1.0 + b.nod * 0.5, 1.0);
    if bk.z > 0.05 {
        beak(c, &bk, ball.yaw, f.mouth.max(b.mouth), f.smirk);
    }
}
