//! r42:arcus-look — what stands round Arcus's instrument in its room (sheet v4: *the room
//! furnished like Chorda's*): the cellist's chair behind the cello (or the bassist's stool
//! beside the bass), and the two microphones an engineer would put up — a pencil low on
//! the f-holes, a large-diaphragm condenser higher, out at the instrument's sides so nothing
//! stands in front of it — drawn by the engine's own microphone gear (`gear::KitMic`).

use super::bowed::{Bowed, Pose};
use super::bowed_head::grid;
use super::bowed_mesh::{Mesh, Rod, Role};
use super::gear::{KitMic, MicKind};
use super::geo::{v, V3};
use std::f32::consts::TAU;

/// A box round `c` (half sizes `h` along its own x, y, z), turned `yaw` radians about the
/// vertical: its six faces, and its floor shadow.
fn block(m: &mut Mesh, role: Role, c: V3, h: V3, yaw: f32) {
    let (s, co) = yaw.sin_cos();
    let ax = v(co, 0.0, -s);
    let az = v(s, 0.0, co);
    let ay = v(0.0, 1.0, 0.0);
    let faces: [(V3, V3, V3, f32, f32, f32); 6] = [
        (ay, ax, az, h.y, h.x, h.z),
        (-ay, az, ax, h.y, h.z, h.x),
        (ax, az, ay, h.x, h.z, h.y),
        (-ax, ay, az, h.x, h.y, h.z),
        (az, ay, ax, h.z, h.y, h.x),
        (-az, ax, ay, h.z, h.x, h.y),
    ];
    for (n, u, w, dn, du, dw) in faces {
        let o = c + n * dn;
        m.patches.push((
            role,
            grid(
                2,
                2,
                false,
                |r, k| o + u * (du * (2.0 * k as f32 - 1.0)) + w * (dw * (2.0 * r as f32 - 1.0)),
                move |_| n,
                |r, k| [k as f32 * du * 2.0, r as f32 * dw * 2.0],
            ),
        ));
    }
    let mut corners = Vec::with_capacity(8);
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                corners.push(c + ax * (sx * h.x) + ay * (sy * h.y) + az * (sz * h.z));
            }
        }
    }
    m.shadows.push(corners);
}

fn rod(m: &mut Mesh, role: Role, a: V3, b: V3, r: f32) {
    m.rods.push((role, Rod { a, b, r }));
}

/// The cellist's chair: a wooden chair without arms, its seat at 47 cm, behind the cello
/// where the player sits, facing the audience.
fn chair(m: &mut Mesh, at: V3, yaw: f32) {
    let (s, co) = yaw.sin_cos();
    let local = |x: f32, y: f32, z: f32| at + v(x * co + z * s, y, -x * s + z * co);
    block(
        m,
        Role::Seat,
        local(0.0, 46.0, 0.0),
        v(22.0, 2.2, 20.0),
        yaw,
    );
    for (x, z) in [(-19.0, 17.0), (19.0, 17.0), (-19.0, -17.0), (19.0, -17.0)] {
        rod(m, Role::Chair, local(x, 0.0, z), local(x, 44.0, z), 1.6);
    }
    // The back's posts and its curved rail; a stretcher low between the front legs.
    for x in [-18.5f32, 18.5] {
        rod(
            m,
            Role::Chair,
            local(x, 44.0, -18.0),
            local(x * 1.04, 88.0, -22.0),
            1.4,
        );
    }
    block(
        m,
        Role::Chair,
        local(0.0, 79.0, -21.5),
        v(19.5, 6.5, 1.1),
        yaw,
    );
    rod(
        m,
        Role::Chair,
        local(-19.0, 14.0, 17.0),
        local(19.0, 14.0, 17.0),
        0.9,
    );
}

/// The bassist's stool: a round seat at 74 cm on four splayed legs and a foot ring.
fn stool(m: &mut Mesh, at: V3) {
    let n = 28;
    let ring = move |r: f32, y: f32, k: usize| {
        let a = TAU * k as f32 / n as f32;
        at + v(r * a.cos(), y, r * a.sin())
    };
    let (r, top, under) = (18.0, 74.0, 70.0);
    m.patches.push((
        Role::Seat,
        grid(
            2,
            n,
            true,
            |row, k| ring(r * row as f32, top, k),
            |_| v(0.0, 1.0, 0.0),
            |row, k| [row as f32, k as f32],
        ),
    ));
    m.patches.push((
        Role::Seat,
        grid(
            2,
            n,
            true,
            |row, k| ring(r, if row == 0 { top } else { under }, k),
            move |q| v(q.x - at.x, 0.0, q.z - at.z),
            |row, k| [row as f32, k as f32],
        ),
    ));
    let at_angle = |a: f32, rr: f32, y: f32| at + v(a.cos() * rr, y, a.sin() * rr);
    for k in 0..4 {
        let a = TAU * (k as f32 + 0.5) / 4.0;
        rod(
            m,
            Role::Chair,
            at_angle(a, 11.0, under),
            at_angle(a, 21.0, 0.0),
            1.3,
        );
    }
    for k in 0..8 {
        let (a0, a1) = (TAU * k as f32 / 8.0, TAU * (k + 1) as f32 / 8.0);
        rod(
            m,
            Role::Nickel,
            at_angle(a0, 16.5, 30.0),
            at_angle(a1, 16.5, 30.0),
            0.6,
        );
    }
    let mut pts: Vec<V3> = (0..8).map(|k| ring(r, top, k * 3)).collect();
    pts.extend((0..4).map(|k| at_angle(TAU * (k as f32 + 0.5) / 4.0, 21.0, 0.0)));
    m.shadows.push(pts);
}

/// The chair or the stool, placed for `b` standing in `pose`.
pub fn furniture(m: &mut Mesh, b: Bowed, pose: &Pose) {
    let d = b.dims();
    let foot = pose.at(0.0, -d.endpin, -d.rib_lo * 0.5);
    match b {
        Bowed::Cello => chair(m, v(foot.x + 8.0, 0.0, foot.z - 80.0), 0.12),
        Bowed::Bass => stool(m, v(foot.x - 46.0, 0.0, foot.z - 50.0)),
        // Held in the air, framed close: nothing beside it to cross the picture.
        Bowed::Violin | Bowed::Viola => {} // r42:arcus-upper
    }
}

/// The two microphones an engineer puts up for `b`: a small-diaphragm pencil out to the
/// left at the bridge's height, angled a little down onto the bridge and the f-holes; a
/// large-diaphragm condenser out to the right, higher, on the whole instrument — both on
/// boom stands, both beside the instrument.
pub fn mics(b: Bowed) -> Vec<KitMic> {
    let d = b.dims();
    if d.held {
        // A violin or a viola, framed close: any stand would cross it. // r42:arcus-upper
        return Vec::new(); // r42:arcus-upper
    } // r42:arcus-upper
    let pose = Pose::of(b);
    let k = d.body / 75.7;
    // The pencil's target: the bridge, between the f-holes, where an engineer aims a
    // close mic on a cello or a bass.
    let low = pose.at(0.0, super::bowed::bridge_y(d), d.arch + d.bridge_h * 0.5);
    let high = pose.at(0.0, d.body * 0.62, d.arch);
    let pencil = low + v(-64.0 * k, 16.0 * k, 24.0 * k);
    let ldc = high + v(66.0 * k, 22.0 * k, 22.0 * k);
    vec![
        KitMic {
            p: pencil,
            aim: (low - pencil).norm(),
            kind: MicKind::Pencil,
            stand: Some(v(pencil.x - 14.0, 0.0, pencil.z - 48.0)),
            clip: false,
            spread: 0.0,
        },
        KitMic {
            p: ldc,
            aim: (high - ldc).norm(),
            kind: MicKind::Ldc,
            stand: Some(v(ldc.x + 24.0, 0.0, ldc.z - 20.0)),
            clip: false,
            spread: 0.0,
        },
    ]
}
