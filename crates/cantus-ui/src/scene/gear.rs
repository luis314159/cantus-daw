//! The microphones and their stands (port of `scene/gear.py`): small-diaphragm pencils on a
//! stereo bar placed by Perspective (over the hammers 0 → at the player 0.35 → out where the
//! audience sits 1, Width spacing them), a far room pair of large-diaphragm condensers on
//! tall stands when Room is above 3 %, and the drummer's mics. Every cable runs down its
//! stand and across the floor, out of the frame.

use super::display::{Paint, Part};
use super::geo::{v, V3};
use super::kit::tripod;
use super::prims::{tube, Ctx, OWN, OWN_NS, OWN_NS_BUTT};
use crate::kit::skin::scene::{ink, mat};
use egui::Pos2;

/// A tripod, a mast, a boom to `end` with its counterweight behind the mast.
pub fn boom_stand(c: &mut Ctx, base: V3, mast_h: f32, end: V3, cable_to: Option<V3>) {
    tripod(c, base, 26.0, 26.0, base.x * 0.7, true);
    let top = v(base.x, mast_h, base.z);
    // Masts and booms a size lighter than a cymbal stand's (round 40 cp4: with the mics
    // shown, a kit in a room read as a forest of verticals).
    tube(c, v(base.x, 26.0, base.z), top, 0.75, &mat::CHROME, OWN);
    tube(
        c,
        top - v(0.0, 1.5, 0.0),
        top + v(0.0, 1.5, 0.0),
        1.4,
        &mat::BLACK,
        OWN,
    );
    let d = (end - top).norm();
    let tail = top - d * 22.0;
    tube(c, tail, end, 0.55, &mat::CHROME, OWN);
    tube(c, tail - d * 2.0, tail + d * 3.0, 2.0, &mat::BLACK, OWN);
    if let Some(to) = cable_to {
        cable(c, base, to);
    }
}

/// A cable from the stand's foot over the floor, out of the frame.
pub fn cable(c: &mut Ctx, base: V3, away: V3) {
    let cam = c.cam;
    let pts: Vec<Pos2> = (0..12)
        .map(|k| {
            let f = k as f32 / 11.0;
            let x = base.x + (away.x - base.x) * f + 14.0 * (f * 3.1).sin();
            let z = base.z + (away.z - base.z) * f;
            cam.p(v(x, 0.5, z))
        })
        .collect();
    c.p.at(cam.depth(base) + 30.0, c.piece);
    let w = (0.7 * cam.s / cam.depth(base)).max(1.0);
    c.p.polyline(pts, w, Paint::Solid(ink::CABLE), 0.95, false);
}

/// A small-diaphragm pencil, its head at `head` aimed along `aim`; returns its tail.
pub fn pencil(c: &mut Ctx, head: V3, aim: V3, length: f32, r: f32, scale: f32, tally: bool) -> V3 {
    let tail = head - aim * (length * scale);
    let grille = head - aim * (length * scale * 0.25);
    tube(c, tail, grille, r * scale, &mat::MIC_BODY, OWN_NS);
    tube(c, grille, head, r * scale * 1.04, &mat::NICKEL, OWN_NS);
    if tally {
        let at = head - aim * (length * scale * 0.3);
        tube(
            c,
            at - aim * 0.6,
            at + aim * 0.6,
            r * scale * 1.1,
            &mat::TALLY,
            OWN_NS,
        );
    }
    tail
}

/// Two pencils on a stereo bar, toed out a little, the bar's middle at `centre`.
pub fn pair_on_bar(
    c: &mut Ctx,
    centre: V3,
    aim: V3,
    spacing: f32,
    spread: V3,
    scale: f32,
    tally: bool,
) -> (V3, V3) {
    let a = centre - spread * (spacing / 2.0);
    let b = centre + spread * (spacing / 2.0);
    // One clean T: the black bar across, the pencils from its ends along the aim, toed out
    // a little.
    tube(c, a, b, 0.75, &mat::BLACK, OWN_NS);
    for (p, sgn) in [(a, -1.0f32), (b, 1.0)] {
        let toe = (aim + spread * (sgn * 0.12)).norm();
        tube(
            c,
            p - v(0.0, 1.2, 0.0),
            p + v(0.0, 1.2, 0.0),
            1.1,
            &mat::BLACK,
            OWN_NS,
        );
        pencil(c, p + toe * (15.0 * scale), toe, 15.0, 1.0, scale, tally);
    }
    (a, b)
}

/// A large-diaphragm condenser in its shock mount.
pub fn ldc(c: &mut Ctx, pos: V3, scale: f32) {
    let up = v(0.0, 1.0, 0.0);
    let bot = pos - up * (9.0 * scale);
    tube(c, bot, pos, 2.6 * scale, &mat::NICKEL, OWN_NS);
    tube(
        c,
        pos,
        pos + up * (7.0 * scale),
        2.8 * scale,
        &mat::CHROME,
        OWN_NS,
    );
    for y in [-6.0, 0.0] {
        let p = pos + v(0.0, y * scale, 0.0);
        tube(
            c,
            p + v(-4.5 * scale, 0.0, 0.0),
            p + v(4.5 * scale, 0.0, 0.0),
            0.5 * scale,
            &mat::BLACK,
            OWN_NS,
        );
    }
}

/// Where Perspective puts the pair: (centre, aim, spacing, spread, the stand's foot).
pub fn piano_pair_place(persp: f32, width: f32, upright: bool) -> (V3, V3, f32, V3, V3) {
    type K = (f32, V3, V3, f32, V3, V3);
    let mut keys: [K; 3] = [
        (
            0.0,
            v(-4.0, 118.0, -34.0),
            v(0.0, -1.0, -0.15),
            70.0,
            v(1.0, 0.0, 0.0),
            v(-150.0, 0.0, -40.0),
        ),
        (
            0.35,
            v(0.0, 132.0, 92.0),
            v(0.0, -0.35, -1.0),
            22.0,
            v(1.0, 0.0, 0.0),
            v(-60.0, 0.0, 120.0),
        ),
        (
            1.0,
            v(170.0, 150.0, -170.0),
            v(-1.0, -0.22, 0.1),
            70.0,
            v(0.0, 0.0, 1.0),
            v(196.0, 0.0, -170.0),
        ),
    ];
    if upright {
        keys[0] = (
            0.0,
            v(0.0, 150.0, -10.0),
            v(0.0, -1.0, -0.4),
            70.0,
            v(1.0, 0.0, 0.0),
            v(-150.0, 0.0, -20.0),
        );
    }
    let persp = persp.clamp(0.0, 1.0);
    for w in keys.windows(2) {
        let (p0, c0, a0, s0, d0, b0) = w[0];
        let (p1, c1, a1, s1, d1, b1) = w[1];
        if p0 <= persp && persp <= p1 {
            let f = (persp - p0) / (p1 - p0);
            return (
                c0.lerp(c1, f),
                a0.lerp(a1, f).norm(),
                (s0 + (s1 - s0) * f) * (0.4 + 0.86 * width),
                d0.lerp(d1, f).norm(),
                b0.lerp(b1, f),
            );
        }
    }
    let k = keys[0];
    (k.1, k.2.norm(), k.3, k.4, k.5)
}

/// The piano's microphones: the pair Perspective places, and the room pair when `room`
/// is above 3 %. Returns where each lands (the pair's bar, the room pair's middle).
#[allow(clippy::too_many_arguments)]
pub fn piano_mics(
    c: &mut Ctx,
    persp: f32,
    width: f32,
    room: f32,
    upright: bool,
    back_z: f32,
) -> Vec<(Part, V3)> {
    let scale = 1.6;
    c.piece = Part::Pair;
    let (centre, aim, spacing, sd, base) = piano_pair_place(persp, width, upright);
    let mast = centre.y + 18.0;
    boom_stand(
        c,
        base,
        mast,
        centre + v(0.0, 6.0, 0.0),
        Some(v(base.x - 200.0, 0.0, base.z + 300.0)),
    );
    // Round 40 (visual): a short black clip from the boom's end down to the bar (a chrome
    // drop rod and splayed pencils read as a zig-zag).
    tube(
        c,
        centre + v(0.0, 6.0, 0.0),
        centre + v(0.0, 1.2, 0.0),
        1.0,
        &mat::BLACK,
        OWN_NS,
    );
    pair_on_bar(c, centre, aim, spacing, sd, scale, true);
    let mut anchors = vec![(Part::Pair, centre)];
    if room > 0.03 {
        c.piece = Part::RoomPair;
        for sx in [-1.0f32, 1.0] {
            let b = v(-40.0 + sx * 150.0, 0.0, back_z + 50.0);
            let pos = v(b.x, 210.0, b.z + 4.0);
            tripod(c, b, 30.0, 32.0, 10.0, true);
            tube(
                c,
                v(b.x, 30.0, b.z),
                v(b.x, 196.0, b.z),
                0.75,
                &mat::CHROME,
                OWN,
            );
            ldc(c, pos, scale * 0.9);
            cable(c, b, v(b.x + sx * 300.0, 0.0, b.z - 200.0));
        }
        anchors.push((Part::RoomPair, v(-40.0, 210.0, back_z + 54.0)));
    }
    anchors
}

/// How a drummer's mic is built and held.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MicKind {
    Dynamic,
    Pencil,
    Ribbon,
    Ldc,
    Sub,
    /// A spaced pair of pencils over the kit.
    Pair,
    /// A pair of condensers out in the room.
    LdcPair,
}

/// A drummer's mic: where its head is, its aim, how it is built, its stand's foot (or a
/// clip on the rim), a pair's spread.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct KitMic {
    pub p: V3,
    pub aim: V3,
    pub kind: MicKind,
    pub stand: Option<V3>,
    pub clip: bool,
    pub spread: f32,
}

const fn km(p: V3, aim: V3, kind: MicKind, stand: Option<V3>, clip: bool, spread: f32) -> KitMic {
    KitMic {
        p,
        aim,
        kind,
        stand,
        clip,
        spread,
    }
}

/// The designer's studio kit's microphones, in order: kick out, kick sub, snare top,
/// hi-hat, tom 1, tom 2, floor 1, floor 2, ride, overheads, room near, mono room.
pub fn studio_mics() -> Vec<KitMic> {
    use MicKind::*;
    vec![
        km(
            v(9.0, 26.0, 62.0),
            v(0.0, 0.0, -1.0),
            Ldc,
            Some(v(9.0, 0.0, 80.0)),
            false,
            0.0,
        ),
        km(
            v(-14.0, 28.0, 58.0),
            v(0.0, 0.0, -1.0),
            Sub,
            Some(v(-14.0, 0.0, 64.0)),
            false,
            0.0,
        ),
        km(
            v(26.0, 72.0, 10.0),
            v(0.3, -0.7, -0.6),
            Dynamic,
            None,
            true,
            0.0,
        ),
        km(
            v(70.0, 112.0, 8.0),
            v(0.0, -1.0, -0.2),
            Pencil,
            Some(v(110.0, 0.0, 40.0)),
            false,
            0.0,
        ),
        km(
            v(20.0, 92.0, 26.0),
            v(-0.3, -0.6, -0.6),
            Dynamic,
            None,
            true,
            0.0,
        ),
        km(
            v(-22.0, 94.0, 26.0),
            v(0.3, -0.6, -0.6),
            Dynamic,
            None,
            true,
            0.0,
        ),
        km(
            v(-44.0, 62.0, 16.0),
            v(-0.3, -0.5, -0.6),
            Dynamic,
            None,
            true,
            0.0,
        ),
        km(
            v(-74.0, 60.0, -6.0),
            v(-0.3, -0.5, -0.6),
            Dynamic,
            None,
            true,
            0.0,
        ),
        km(
            v(-64.0, 124.0, 30.0),
            v(-0.3, -1.0, -0.3),
            Pencil,
            Some(v(-100.0, 0.0, 70.0)),
            false,
            0.0,
        ),
        km(
            v(0.0, 196.0, -4.0),
            v(0.0, -1.0, 0.0),
            Pair,
            None,
            false,
            56.0,
        ),
        km(
            v(0.0, 150.0, 190.0),
            v(0.0, -0.2, -1.0),
            LdcPair,
            None,
            false,
            230.0,
        ),
        km(
            v(0.0, 92.0, 150.0),
            v(0.0, -0.1, -1.0),
            Ribbon,
            Some(v(0.0, 0.0, 150.0)),
            false,
            0.0,
        ),
    ]
}

fn mic_body(c: &mut Ctx, p: V3, aim: V3, kind: MicKind, scale: f32) {
    match kind {
        MicKind::Dynamic => {
            let tail = p - aim * (13.0 * scale * 0.7);
            tube(
                c,
                tail,
                p - aim * (3.0 * scale),
                1.4 * scale * 0.7,
                &mat::MIC_DARK,
                OWN_NS,
            );
            tube(
                c,
                p - aim * (3.0 * scale),
                p,
                1.9 * scale * 0.7,
                &mat::NICKEL,
                OWN_NS,
            );
        }
        MicKind::Pencil => {
            pencil(c, p, aim, 15.0, 1.0, scale * 0.8, false);
        }
        MicKind::Ribbon => {
            pencil(c, p, aim, 12.0, 2.2, scale * 0.8, false);
        }
        MicKind::Ldc => ldc(c, p, scale * 0.6),
        MicKind::Sub => {
            tube(c, p - aim * 10.0, p, 8.0, &mat::BLACK, OWN_NS_BUTT);
            tube(c, p - aim * 0.6, p, 7.4, &mat::NICKEL, OWN_NS_BUTT);
        }
        MicKind::Pair | MicKind::LdcPair => {}
    }
}

/// The drummer's mics; returns each one's head (its pick).
pub fn kit_mics(c: &mut Ctx, mics: &[KitMic]) -> Vec<(Part, V3)> {
    let scale = 1.5;
    let mut anchors = Vec::with_capacity(mics.len());
    for (i, m) in mics.iter().enumerate() {
        let part = Part::Mic(i as u16);
        c.piece = part;
        let (p, aim) = (m.p, m.aim.norm());
        match m.kind {
            MicKind::Pair => {
                for sx in [-1.0f32, 1.0] {
                    let base = v(sx * 70.0, 0.0, -120.0);
                    let q = p + v(sx * m.spread / 2.0, 0.0, 0.0);
                    boom_stand(c, base, 176.0, q + v(0.0, 6.0, 0.0), None);
                    pencil(c, q, aim, 15.0, 1.0, scale * 0.8, false);
                }
            }
            MicKind::LdcPair => {
                for sx in [-1.0f32, 1.0] {
                    let q = p + v(sx * m.spread / 2.0, 0.0, 0.0);
                    let base = v(q.x, 0.0, q.z + 10.0);
                    tripod(c, base, 30.0, 30.0, 0.0, true);
                    tube(
                        c,
                        v(base.x, 30.0, base.z),
                        v(q.x, q.y - 10.0, q.z),
                        0.75,
                        &mat::CHROME,
                        OWN,
                    );
                    ldc(c, q, scale * 0.7);
                    cable(c, base, v(base.x + sx * 200.0, 0.0, base.z + 200.0));
                }
            }
            kind => {
                if let Some(st) = m.stand {
                    boom_stand(
                        c,
                        st,
                        p.y + 14.0,
                        p + v(0.0, 4.0, 0.0),
                        Some(v(st.x + 120.0, 0.0, st.z + 260.0)),
                    );
                }
                if m.clip {
                    let a = p - aim * 6.0;
                    tube(c, a, a + v(0.0, -7.0, 0.0), 0.5, &mat::BLACK, OWN_NS);
                }
                mic_body(c, p, aim, kind, scale);
            }
        }
        anchors.push((part, p));
    }
    anchors
}
