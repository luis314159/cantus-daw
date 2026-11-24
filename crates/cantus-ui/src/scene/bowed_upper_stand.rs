//! r42:arcus-upper — the stand a violin or a viola rests on in Arcus's picture: a chromed
//! instrument stand of the usual make — three legs on the floor, a column, a padded cradle
//! the lower bout sits in, and a padded fork behind the neck's root — the instrument leant
//! back a little in it, its end button just above the cradle.

use super::bowed::{Dims, Pose};
use super::bowed_mesh::{Mesh, Rod, Role};
use super::geo::{v, V3};
use std::f32::consts::TAU;

fn rod(m: &mut Mesh, role: Role, a: V3, b: V3, r: f32) {
    m.rods.push((role, Rod { a, b, r }));
}

/// The stand under `d` in `pose`: added to `m`, its floor contact as the mesh's foot.
pub fn stand(m: &mut Mesh, d: &Dims, pose: &Pose) {
    let back = -(d.rib_lo + d.arch_back) - 0.9;
    // The cradle: a padded bar under the lower edge, two padded arms rising round the
    // lower bout's sides (a third of its width out).
    let width = d.body * 0.29;
    let c = pose.at(0.0, -0.9, -d.rib_lo * 0.5);
    let side = pose.ex * (width * 0.72);
    let up = pose.ey * (d.body * 0.13);
    rod(m, Role::Rubber, c - side, c + side, 0.75);
    for s in [-1.0f32, 1.0] {
        let a = c + side * s;
        let b = a + up - pose.ez * 0.3;
        rod(m, Role::Rubber, a, b, 0.7);
    }
    // The column: straight down from under the cradle to the hub of the legs.
    let col_top = pose.at(0.0, -2.4, back * 0.55);
    let hub = v(col_top.x, 14.0, col_top.z);
    rod(m, Role::Nickel, c, col_top, 0.55);
    rod(m, Role::Nickel, col_top, hub, 0.85);
    // The legs, splayed, each ending in a rubber foot.
    let mut floor = Vec::new();
    for k in 0..3 {
        let a = TAU * (k as f32 + 0.25) / 3.0;
        let f = v(hub.x + 27.0 * a.cos(), 1.0, hub.z + 27.0 * a.sin());
        rod(m, Role::Nickel, hub, f, 0.6);
        rod(m, Role::Rubber, f, f - v(0.0, 1.0, 0.0), 1.0);
        floor.push(v(f.x, 0.0, f.z));
    }
    // The neck's support: a rod up behind the back to the neck's root, a padded fork round
    // the neck's heel.
    let y_fork = d.body + d.neck * 0.18;
    let fork = pose.at(0.0, y_fork, back);
    let low = pose.at(0.0, d.body * 0.2, back);
    rod(m, Role::Nickel, col_top, low, 0.45);
    rod(m, Role::Nickel, low, fork, 0.45);
    let neck_w = d.board_w.1 * 0.62;
    for s in [-1.0f32, 1.0] {
        let tip = pose.at(s * neck_w, y_fork, d.overstand * 0.4);
        let elbow = pose.at(s * neck_w, y_fork, back);
        rod(m, Role::Nickel, fork, elbow, 0.4);
        rod(m, Role::Rubber, elbow, tip, 0.55);
    }
    m.foot = v(hub.x, 0.0, hub.z);
    floor.push(m.foot);
    m.shadows.push(floor);
}
