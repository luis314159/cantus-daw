//! The microphones (ADR 0104): the pair the perspective places — over the hammers on a
//! boom (0), at the player's ear behind the bench (0.35), out where the audience sits (1) —
//! two small-diaphragm pencils on a stereo bar, spaced by the width control, on a proper
//! stand with its cables; and, when the room is in the mix, a room pair: large-diaphragm
//! condensers in shock mounts on tall stands far back in the room. The cables run across
//! the floor and out of the frame toward the control room.

use super::geo::{Geo, M};
use super::mesh::{v3, V3};
use super::piano::Piano;
use super::room::Room;
use cantus_core::inst::chorda_look::Body;

/// A pair on a stand: where the bar's centre is, the mics' spacing, what they aim at, the
/// stand's foot on the floor and its mast's top (a boom reaches from there to the bar).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pair {
    pub centre: V3,
    pub spacing: f32,
    pub aim: V3,
    /// How far the two capsules turn apart (radians, each side).
    pub splay: f32,
    pub base: V3,
    pub mast: f32,
    /// Two stands, one large-diaphragm mic on each (a room pair), instead of a bar.
    pub split: bool,
}

impl Pair {
    /// The two capsules: each one's position and the direction it points.
    pub fn mics(&self) -> [(V3, V3); 2] {
        let dir = (self.aim - self.centre).norm();
        let flat = v3(dir.x, 0.0, dir.z);
        let axis = if flat.len() > 0.2 {
            flat.norm().cross(v3(0.0, 1.0, 0.0)).norm()
        } else {
            v3(1.0, 0.0, 0.0)
        };
        let turn = |d: V3, a: f32| {
            let (s, c) = a.sin_cos();
            v3(d.x * c - d.z * s, d.y, d.x * s + d.z * c).norm()
        };
        let half = self.spacing * 0.5;
        [
            (self.centre - axis * half, turn(dir, -self.splay)),
            (self.centre + axis * half, turn(dir, self.splay)),
        ]
    }
}

fn smooth(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The main pair for the perspective `persp` (0 … 1), `width` (0 … 1) and the lid (0 … 1).
pub fn main_pair(p: &Piano, persp: f32, width: f32, lid: f32) -> Pair {
    let persp = persp.clamp(0.0, 1.0);
    let width = width.clamp(0.0, 1.0);
    let (w, l) = (p.w, p.l);
    let upright = p.body == Body::Upright;
    let (close, close_base, close_mast) = if upright {
        (
            v3(w * 0.5, p.y1 + 0.34, -0.3),
            v3(w + 0.55, 0.0, -0.1),
            1.62,
        )
    } else {
        let y = p.y1 + 0.18 + 0.06 * (1.0 - lid);
        (v3(w * 0.5, y, -0.32), v3(w + 0.6, 0.0, -0.55), 1.38)
    };
    let (player, player_base, player_mast) = if upright {
        (v3(w * 0.5 + 0.1, 1.62, 0.95), v3(w + 0.45, 0.0, 0.9), 1.95)
    } else {
        (
            v3(w * 0.5 + 0.1, 1.72, 0.9),
            v3(w * 0.5 + 0.8, 0.0, 1.4),
            2.05,
        )
    };
    let far = if upright {
        v3(w * 0.5 + 1.25, 1.78, 2.5)
    } else {
        v3(w + 2.0, 1.95, -l * 0.5 + 0.2)
    };
    let target = if upright {
        v3(w * 0.5, 1.0, -0.25)
    } else {
        v3(w * 0.55, 0.92, -l * 0.42)
    };
    if persp <= 0.35 {
        let t = smooth(persp / 0.35);
        let lift = v3(w * 0.5, close.y.max(player.y) + 0.25, 0.25);
        let centre = close.lerp(lift, t).lerp(lift.lerp(player, t), t);
        let down = v3(centre.x, centre.y - 1.0, centre.z - 0.05);
        Pair {
            centre,
            spacing: (0.25 + 0.75 * width) * (1.0 - t) + (0.12 + 0.3 * width) * t,
            aim: down.lerp(target, t),
            splay: (0.15 + 0.55 * width) * t,
            base: close_base.lerp(player_base, t),
            mast: close_mast + (player_mast - close_mast) * t,
            split: false,
        }
    } else {
        let t = smooth((persp - 0.35) / 0.65);
        let centre = player.lerp(far, t);
        Pair {
            centre,
            spacing: 0.12 + 0.3 * width + 0.15 * width * t,
            aim: target,
            splay: 0.15 + 0.55 * width,
            base: player_base.lerp(v3(far.x, 0.0, far.z), t),
            mast: player_mast + (far.y + 0.02 - player_mast) * t,
            split: false,
        }
    }
}

/// The room pair, when the room is in the mix (its level above 3 %).
pub fn room_pair(p: &Piano, room: &Room) -> Option<Pair> {
    if room.live < 0.03 {
        return None;
    }
    let s = room.size;
    let back = -p.l;
    let z = back + (room.z0 - back) * 0.55;
    let x = (p.w * 0.35 + room.x0 * 0.25).max(room.x0 + 0.6);
    let y = 1.65 + 0.7 * s;
    Some(Pair {
        centre: v3(x, y, z),
        spacing: 1.1 + 2.4 * s,
        aim: p.centre(),
        splay: 0.0,
        base: v3(x, 0.0, z),
        mast: y,
        split: true,
    })
}

/// A stand: its tripod, the mast to `top`, a boom from there to `to` (when it reaches).
fn stand(g: &mut Geo, base: V3, top: f32, to: Option<V3>) {
    let hub = base + v3(0.0, 0.32, 0.0);
    for i in 0..3 {
        let a = 0.6 + i as f32 * 2.094;
        let foot = base + v3(a.cos() * 0.36, 0.012, a.sin() * 0.36);
        g.rod(hub, foot, 0.009, M::Stand, 0);
        g.sphere(foot, 0.014, 4, M::Rubber, 0);
    }
    g.tube(
        hub - v3(0.0, 0.05, 0.0),
        hub + v3(0.0, 0.05, 0.0),
        0.02,
        0.02,
        10,
        M::Stand,
        0,
    );
    let mtop = v3(base.x, top, base.z);
    let mid = base.lerp(mtop, 0.55);
    g.rod(hub, mid, 0.014, M::Stand, 0);
    g.rod(mid, mtop, 0.009, M::Chrome, 0);
    g.tube(
        mid - v3(0.0, 0.02, 0.0),
        mid + v3(0.0, 0.02, 0.0),
        0.02,
        0.02,
        10,
        M::Stand,
        0,
    );
    if let Some(to) = to {
        let d = to - mtop;
        if d.len() > 0.05 {
            let back = mtop - d.norm() * 0.32;
            g.rod(back, to, 0.009, M::Stand, 0);
            g.tube(back, back + d.norm() * 0.09, 0.03, 0.03, 12, M::Stand, 0);
            g.sphere(mtop, 0.022, 6, M::Stand, 0);
        }
    }
}

/// A small-diaphragm pencil condenser at `at`, pointing along `dir`; `tally`: the red ring
/// of the pair the mix listens through.
fn pencil(g: &mut Geo, at: V3, dir: V3, tally: bool) {
    let len = 0.16;
    let r = 0.0105;
    let back = at - dir * (len * 0.5);
    let tip = at + dir * (len * 0.5);
    g.tube(back, tip - dir * 0.03, r, r, 14, M::MicBody, 0);
    g.tube(tip - dir * 0.03, tip, r * 1.02, r * 1.02, 14, M::Grille, 0);
    g.tube(
        back - dir * 0.012,
        back,
        r * 0.8,
        r * 0.95,
        12,
        M::MicDark,
        0,
    );
    if tally {
        g.tube(
            back + dir * 0.01,
            back + dir * 0.016,
            r * 1.03,
            r * 1.03,
            14,
            M::Glow,
            7,
        );
    }
    // Its clip.
    g.tube(
        at - dir * 0.03,
        at - dir * 0.03 + v3(0.0, 0.03, 0.0),
        0.006,
        0.006,
        8,
        M::MicDark,
        0,
    );
}

/// A large-diaphragm condenser in its shock mount, at `at`, facing `dir`.
fn ldc(g: &mut Geo, at: V3, dir: V3) {
    let up = v3(0.0, 1.0, 0.0);
    let r = 0.026;
    let body0 = at - up * 0.09;
    g.tube(body0, at, r, r, 16, M::MicBody, 0);
    g.tube(at, at + up * 0.05, r * 1.01, r * 1.01, 16, M::Grille, 0);
    g.sphere(at + up * 0.05, r * 1.01, 8, M::Grille, 0);
    g.tube(body0 - up * 0.02, body0, r * 0.7, r, 14, M::MicDark, 0);
    // The shock mount: two rings round the body on elastic, a yoke to the stand.
    let side = dir.cross(up).norm();
    let fwd = side.cross(up).norm();
    for y in [-0.075f32, -0.015] {
        let mut ring = [V3::default(); 17];
        for (k, q) in ring.iter_mut().enumerate() {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            *q = at + up * y + (side * a.cos() + fwd * a.sin()) * 0.05;
        }
        g.hose(&ring, 0.004, M::Stand, 0);
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.4;
            let o = side * a.cos() + fwd * a.sin();
            g.line(
                at + up * y + o * 0.05,
                at + up * y + o * r,
                0.0012,
                M::Rubber,
                1.0,
            );
        }
    }
    g.rod(
        at - up * 0.12 - fwd * 0.05,
        at - up * 0.075 - fwd * 0.05,
        0.006,
        M::Stand,
        0,
    );
}

/// A cable from `from` down the mast at `base`, across the floor and out of the frame (to
/// the floor two metres in front of the lens: below the frame's foot, always in view of
/// the lens's front).
fn cable(g: &mut Geo, from: V3, mast_top: V3, base: V3, seed: f32) {
    let cam = g.cam;
    let f = v3(cam.fwd.x, 0.0, cam.fwd.z).norm();
    let r = v3(cam.right.x, 0.0, cam.right.z).norm();
    let side = v3(0.02, 0.0, 0.02);
    let foot = v3(base.x, 0.006, base.z);
    let end = v3(cam.eye.x, 0.006, cam.eye.z) + f * 2.0 - r * (0.6 + seed * 1.5);
    let ctrl = foot.lerp(end, 0.35) + r * (0.5 - seed);
    let mut pts = [V3::default(); 22];
    pts[0] = from;
    pts[1] = mast_top + side + v3(0.0, -0.05, 0.0);
    pts[2] = v3(base.x, 0.45, base.z) + side;
    pts[3] = v3(base.x, 0.12, base.z) + side * 1.5;
    pts[4] = foot + side * 2.0;
    pts[5] = foot + side * 3.0 + f * -0.05;
    for i in 0..16 {
        let t = (i + 1) as f32 / 16.0;
        let a = foot.lerp(ctrl, t);
        let b = ctrl.lerp(end, t);
        pts[6 + i] = a.lerp(b, t);
    }
    g.hose(&pts, 0.0045, M::Rubber, 0);
}

/// A pair, whole: stands, cables and the capsules.
pub fn build(g: &mut Geo, pair: &Pair, tally: bool) {
    let mics = pair.mics();
    if pair.split {
        for (i, (at, dir)) in mics.iter().enumerate() {
            let base = v3(at.x, 0.0, at.z);
            let top = v3(at.x, at.y - 0.13, at.z);
            cable(g, *at - v3(0.0, 0.1, 0.0), top, base, 0.3 * i as f32);
            stand(g, base, at.y - 0.13, None);
            ldc(g, *at, *dir);
        }
        return;
    }
    let top = v3(pair.base.x, pair.mast, pair.base.z);
    for (i, (at, dir)) in mics.iter().enumerate() {
        cable(g, *at - *dir * 0.09, top, pair.base, 0.25 * i as f32);
    }
    stand(g, pair.base, pair.mast, Some(pair.centre));
    let (a, b) = (mics[0].0, mics[1].0);
    g.rod(
        a + v3(0.0, 0.03, 0.0),
        b + v3(0.0, 0.03, 0.0),
        0.006,
        M::Stand,
        0,
    );
    for (at, dir) in mics {
        pencil(g, at, dir, tally);
    }
}
