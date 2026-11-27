//! r42:arcus-look — the neck end of a bowed instrument as surfaces (see `bowed_mesh`): the
//! ebony fingerboard (its top an arc across, its sides, its end and its underside), the
//! nut, the neck with its heel down to the button, the open pegbox with its cheeks, the
//! **scroll as a volute** — a logarithmic spiral of two and a half turns, its back fluted
//! either side of the spine over the first turn, its sides stepping in turn by turn to the
//! eye — and the cello's ebony pegs or the bass's geared machines, the strings wound on
//! them in their silks.

use super::bowed::{board_r, board_w, board_z, nut_y, string_local, Dims, Pose};
use super::bowed_mesh::{Mesh, Patch, Rod, Role};
use super::geo::{v, V3};
use std::f32::consts::{PI, TAU};

/// A grid patch from a point function over `rows` × `cols`, its normals turned toward
/// `out`, grain coordinates `uv`.
pub fn grid(
    rows: usize,
    cols: usize,
    closed: bool,
    p: impl Fn(usize, usize) -> V3,
    out: impl Fn(V3) -> V3,
    uv: impl Fn(usize, usize) -> [f32; 2],
) -> Patch {
    let mut pts = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            pts.push(p(r, c));
        }
    }
    Patch::of(pts, rows, cols, closed, out, uv)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A flattened ellipsoid (a peg's thumb piece, a machine's key) round `c`: half-sizes `a`
/// along `e1`, `b` along `e2`, `t` along their normal.
#[allow(clippy::too_many_arguments)]
pub(super) fn paddle(m: &mut Mesh, role: Role, c: V3, e1: V3, e2: V3, a: f32, b: f32, t: f32) {
    // r42:arcus-upper (the chinrest)
    let e3 = e1.cross(e2).norm();
    let p = grid(
        9,
        18,
        true,
        |r, k| {
            let ph = -PI / 2.0 + PI * r as f32 / 8.0;
            let th = TAU * k as f32 / 18.0;
            c + e1 * (a * ph.cos() * th.cos())
                + e2 * (b * ph.cos() * th.sin())
                + e3 * (t * ph.sin())
        },
        move |q| q - c,
        |r, k| [k as f32, r as f32],
    );
    m.patches.push((role, p));
}

/// The fingerboard, the nut, the neck, the pegbox, the scroll, the pegs or machines and
/// the strings' ends wound on them.
pub fn neck(m: &mut Mesh, d: &Dims, pose: &Pose) {
    let yn = nut_y(d);
    let ye = yn - d.board;
    let rb = board_r(d);
    // r42:arcus-polish2 — a violin's or a viola's as a luthier makes it (the owner: "hazlo
    // más delgado, compáralo con violines reales"): the fingerboard about 5 mm thick at its
    // edges, the neck 19 mm deep at the nut and 21.5 at the heel with the fingerboard (a
    // viola's in proportion, a little deeper) — ours had been 39 and 51 mm.
    let k = d.body / 35.6;
    let thick = if d.held { 0.5 * k } else { d.board_w.0 * 0.36 };
    let at = |x: f32, y: f32, z: f32| pose.at(x, y, z);
    let top = move |x: f32, y: f32| board_z(d, y) - x * x / (2.0 * rb);
    let (ez, ex, ey) = (pose.ez, pose.ex, pose.ey);
    // ---- the fingerboard: top, sides, end, underside ----
    let rows = 34;
    let yy = move |r: usize| ye + (yn - ye) * r as f32 / (rows - 1) as f32;
    let xx = move |c: usize, y: f32| (c as f32 / 8.0 - 0.5) * board_w(d, y);
    m.patches.push((
        Role::Ebony,
        grid(
            rows,
            9,
            false,
            |r, c| {
                let y = yy(r);
                let x = xx(c, y);
                at(x, y, top(x, y))
            },
            move |_| ez,
            |r, c| [yy(r), c as f32],
        ),
    ));
    for s in [-1.0f32, 1.0] {
        m.patches.push((
            Role::Ebony,
            grid(
                rows,
                2,
                false,
                |r, c| {
                    let y = yy(r);
                    let x = s * board_w(d, y) * 0.5;
                    at(x, y, top(x, y) - thick * c as f32)
                },
                move |_| ex * s,
                |r, c| [yy(r), c as f32],
            ),
        ));
    }
    m.patches.push((
        Role::Ebony,
        grid(
            2,
            9,
            false,
            |r, c| {
                let x = xx(c, ye);
                at(x, ye, top(x, ye) - thick * r as f32)
            },
            move |_| -ey,
            |r, c| [r as f32, c as f32],
        ),
    ));
    m.patches.push((
        Role::Ebony,
        grid(
            rows,
            2,
            false,
            |r, c| {
                let y = yy(r);
                let x = (c as f32 - 0.5) * board_w(d, y);
                at(x, y, top(x, y) - thick)
            },
            move |_| -ez,
            |r, c| [yy(r), c as f32],
        ),
    ));
    // ---- the nut ----
    let wn = board_w(d, yn);
    let nz = board_z(d, yn);
    let nut_h = d.gauge[0] * 2.0 + 0.15;
    m.patches.push((
        Role::Ebony,
        grid(
            3,
            2,
            false,
            |r, c| {
                let x = (c as f32 - 0.5) * wn;
                let (y, z) = [(yn - 0.7, nz), (yn - 0.5, nz + nut_h), (yn, nz + nut_h)][r];
                at(x, y, z)
            },
            move |_| ez,
            |r, c| [r as f32, c as f32],
        ),
    ));
    // ---- the neck under the board, its heel down to the back's button ----
    // (r42:arcus-polish3: a violin's or a viola's heel as a luthier cuts it — the neck keeps
    // its depth to within about 1.8 cm of the body, then curves down to the button; over
    // the board's width × 1.8, some 7.6 cm, it was already 2.4–3.5 cm deep 2–5 cm out.)
    let heel = if d.held {
        1.8 * d.body / 35.6
    } else {
        d.board_w.1 * 1.8
    };
    let base = if d.held {
        (NECK_DEEP.0 * k - thick, NECK_DEEP.1 * k - thick)
    } else {
        (d.board_w.0 * 1.3, d.board_w.1 * 1.0)
    };
    let nrows = 28;
    let ny = move |r: usize| d.body - 0.8 + (yn - d.body + 0.8) * r as f32 / (nrows - 1) as f32;
    let mid = if d.held { base.0 * 0.5 } else { 1.0 };
    let neck_c = move |y: f32| v(0.0, y, board_z(d, y) - thick - mid);
    m.patches.push((
        Role::Maple,
        grid(
            nrows,
            13,
            false,
            |r, c| {
                let y = ny(r);
                let t = ((y - d.body) / (yn - d.body)).clamp(0.0, 1.0);
                let a = board_w(d, y) * 0.47;
                let mut dep = base.1 + (base.0 - base.1) * t;
                let k = 1.0 - smoothstep(0.0, heel, y - d.body);
                let zt = board_z(d, y) - thick;
                dep += k * (zt + super::bowed::rib(d, d.body) - dep - 0.3).max(0.0);
                let th = PI * c as f32 / 12.0;
                at(a * th.cos(), y, zt - dep * th.sin())
            },
            move |q| {
                let l = q - pose.o;
                let y = l.dot(ey);
                let cc = neck_c(y);
                q - pose.at(cc.x, cc.y, cc.z)
            },
            |r, c| [ny(r), c as f32 * 0.4],
        ),
    ));
    head(m, d, pose, yn, nz);
}

/// r42:arcus-polish2 — a violin's neck's depth with its fingerboard, from the fingerboard's
/// top at its middle to the neck's back, at the nut and at the heel (cm; a viola's scaled
/// by its body).
pub const NECK_DEEP: (f32, f32) = (1.9, 2.15);

/// The pegbox, the scroll and the pegs or machines, from the nut's top (`yn`, `nz`).
fn head(m: &mut Mesh, d: &Dims, pose: &Pose, yn: f32, nz: f32) {
    let slope = (board_z(d, yn) - board_z(d, yn - 10.0)) / 10.0;
    let tau = 13f32.to_radians();
    let phi = slope.atan() - tau;
    // The head's axis (up the pegbox), its front, across: in local cm.
    let u = v(0.0, phi.cos(), phi.sin());
    let f = v(0.0, -u.z, u.y);
    let n0 = v(0.0, yn, nz);
    let len = d.pegbox;
    let hw = d.head_w;
    let wid = move |a: f32| hw * (0.80 + 0.12 * (a / len).clamp(0.0, 1.0));
    let dep = move |a: f32| hw * (0.58 + 0.22 * (a / len).clamp(0.0, 1.0));
    let front = 0.1;
    let cheek = hw * 0.1;
    let hp = move |a: f32, bb: f32, x: f32| {
        let l = n0 + u * a + f * bb + v(x, 0.0, 0.0);
        pose.at(l.x, l.y, l.z)
    };
    let (wu, wf, wx) = (pose.dir(u), pose.dir(f), pose.ex);
    let rows = 10;
    let aa = move |r: usize| len * r as f32 / (rows - 1) as f32;
    // Cheeks outside, inside, their front rims; the back outside; the floor inside.
    for s in [-1.0f32, 1.0] {
        m.patches.push((
            Role::Maple,
            grid(
                rows,
                3,
                false,
                |r, c| {
                    let a = aa(r);
                    let bb = [-dep(a), -dep(a) * 0.5, front][c];
                    hp(a, bb, s * wid(a) * 0.5)
                },
                move |_| wx * s,
                |r, c| [aa(r), c as f32],
            ),
        ));
        m.patches.push((
            Role::Inside,
            grid(
                rows,
                2,
                false,
                |r, c| {
                    let a = aa(r);
                    let bb = [-dep(a) + cheek, front][c];
                    hp(a, bb, s * (wid(a) * 0.5 - cheek))
                },
                move |_| wx * -s,
                |r, c| [aa(r), c as f32],
            ),
        ));
        m.patches.push((
            Role::Maple,
            grid(
                rows,
                2,
                false,
                |r, c| {
                    let a = aa(r);
                    hp(a, front, s * (wid(a) * 0.5 - cheek * c as f32))
                },
                move |_| wf,
                |r, c| [aa(r), c as f32],
            ),
        ));
    }
    m.patches.push((
        Role::Maple,
        grid(
            rows,
            2,
            false,
            |r, c| {
                let a = aa(r);
                hp(a, -dep(a), (c as f32 - 0.5) * wid(a))
            },
            move |_| -wf,
            |r, c| [aa(r), c as f32],
        ),
    ));
    m.patches.push((
        Role::Inside,
        grid(
            rows,
            2,
            false,
            |r, c| {
                let a = aa(r);
                hp(
                    a,
                    -dep(a) + cheek,
                    (c as f32 - 0.5) * (wid(a) - 2.0 * cheek),
                )
            },
            move |_| wf,
            |r, c| [aa(r), c as f32],
        ),
    ));
    // The solid foot of the pegbox behind the nut.
    m.patches.push((
        Role::Maple,
        grid(
            2,
            2,
            false,
            |r, c| hp(0.9 * r as f32, front, (c as f32 - 0.5) * wid(0.0)),
            move |_| wf,
            |r, c| [r as f32, c as f32],
        ),
    ));
    scroll(m, d, &hp, len, wid(len), dep(len), wx);
    pegs(m, d, pose, &hp, len, &wid, &dep, wu, wf, wx);
}

/// The volute: r(θ) = R₀·e^(−kθ) from just behind the pegbox's top, over the top, forward
/// and in to the eye in two and a half turns.
#[allow(clippy::too_many_arguments)]
fn scroll(
    m: &mut Mesh,
    d: &Dims,
    hp: &dyn Fn(f32, f32, f32) -> V3,
    len: f32,
    w: f32,
    dep: f32,
    wx: V3,
) {
    let r0 = d.scroll_r;
    let t_end = 2.5 * TAU;
    let r_end = r0 * 0.11;
    let k = (r0 / r_end).ln() / t_end;
    let t0 = -0.55;
    let e = (len + r0 * 0.32, -dep + r0);
    let r = move |t: f32| r0 * (-k * t).exp();
    let hw = move |t: f32| w * 0.5 * (1.0 - 0.5 * (t.max(0.0) / t_end));
    // dir(θ): back (−b) at 0, up (+a) at π/2, front (+b) at π, down at 3π/2.
    let pt = move |t: f32, rr: f32, x: f32| {
        let (s, c) = t.sin_cos();
        hp(e.0 + rr * s, e.1 - rr * c, x)
    };
    let centre = hp(e.0, e.1, 0.0);
    let steps = 150;
    let tt = move |i: usize| t0 + (t_end - t0) * i as f32 / (steps - 1) as f32;
    let flute = move |t: f32| 0.045 * r0 * (1.0 - smoothstep(PI, 1.7 * PI, t));
    m.patches.push((
        Role::Maple,
        grid(
            steps,
            9,
            false,
            |i, c| {
                let t = tt(i);
                let u = c as f32 / 8.0 * 2.0 - 1.0;
                let fl = flute(t) * (PI * u.abs()).sin().powi(2);
                pt(t, r(t) - fl, u * hw(t))
            },
            move |q| {
                let l = q - centre;
                l - wx * l.dot(wx)
            },
            |i, c| [tt(i) * 2.0, c as f32 * 0.6],
        ),
    ));
    // The sides: from each turn's edge in to the next turn's (a step in at every turn),
    // the last turn in to the eye.
    let sides = 120;
    let ts = move |i: usize| t0 + (t_end - t0) * i as f32 / (sides - 1) as f32;
    for s in [-1.0f32, 1.0] {
        m.patches.push((
            Role::Maple,
            grid(
                sides,
                3,
                false,
                |i, c| {
                    let t = ts(i);
                    let (ri, xi) = if t + TAU <= t_end {
                        (r(t + TAU), hw(t + TAU))
                    } else {
                        (r_end * 0.5, hw(t_end) * 0.94)
                    };
                    let (ro, xo) = (r(t), hw(t));
                    let k2 = c as f32 / 2.0;
                    pt(t, ro + (ri - ro) * k2, s * (xo + (xi - xo) * k2))
                },
                move |_| wx * s,
                |i, c| [ts(i), c as f32],
            ),
        ));
        // The eye's boss.
        m.rods.push((
            Role::Maple,
            Rod {
                a: centre + wx * (s * hw(t_end) * 0.8),
                b: centre + wx * (s * hw(t_end) * 1.02),
                r: r_end * 1.25,
            },
        ));
    }
}

/// The cello's four ebony pegs (alternating sides up the pegbox, A nearest the nut) or the
/// bass's machines (brass plates and keys), the strings from the nut wound on them.
#[allow(clippy::too_many_arguments)]
fn pegs(
    m: &mut Mesh,
    d: &Dims,
    pose: &Pose,
    hp: &dyn Fn(f32, f32, f32) -> V3,
    len: f32,
    wid: &dyn Fn(f32) -> f32,
    dep: &dyn Fn(f32) -> f32,
    wu: V3,
    wf: V3,
    wx: V3,
) {
    let order = [3usize, 2, 1, 0];
    for (k, &i) in order.iter().enumerate() {
        let a = len * (0.2 + 0.2 * k as f32);
        let s = if k % 2 == 0 { -1.0f32 } else { 1.0 };
        let mid = -dep(a) * 0.5;
        let half = wid(a) * 0.5;
        let shaft_r = if d.machines { 0.42 } else { 0.38 } * d.head_w / 5.4;
        m.rods.push((
            if d.machines {
                Role::Nickel
            } else {
                Role::Ebony
            },
            Rod {
                a: hp(a, mid, -s * (half + 0.15)),
                b: hp(a, mid, s * (half + 0.2)),
                r: shaft_r,
            },
        ));
        let out = |x: f32| hp(a, mid, s * (half + x));
        let sc = d.head_w / 5.4;
        if d.machines {
            m.rods.push((
                Role::Brass,
                Rod {
                    a: out(0.0),
                    b: out(0.5 * sc) + wf * (-0.6 * sc),
                    r: 0.32 * sc,
                },
            ));
            let shaft_end = out(2.4 * sc) + wf * (-1.8 * sc);
            m.rods.push((
                Role::Nickel,
                Rod {
                    a: out(0.5 * sc) + wf * (-0.6 * sc),
                    b: shaft_end,
                    r: 0.2 * sc,
                },
            ));
            let dir = (shaft_end - out(0.5 * sc)).norm();
            paddle(
                m,
                Role::Brass,
                shaft_end + dir * 1.1 * sc,
                dir,
                wu,
                1.3 * sc,
                0.95 * sc,
                0.32 * sc,
            );
        } else {
            m.rods.push((
                Role::Ebony,
                Rod {
                    a: out(0.0),
                    b: out(1.0 * sc),
                    r: 0.62 * sc,
                },
            ));
            m.rods.push((
                Role::Bridge,
                Rod {
                    a: out(1.0 * sc),
                    b: out(1.25 * sc),
                    r: 0.66 * sc,
                },
            ));
            paddle(
                m,
                Role::Ebony,
                out(2.6 * sc),
                wx * s,
                wf,
                1.5 * sc,
                1.15 * sc,
                0.38 * sc,
            );
        }
        // The string from the nut to its peg, in its silk.
        let [_, _, nut] = string_local(d, i);
        let nutw = pose.at(nut.x, nut.y, nut.z);
        let wind = hp(
            a,
            mid + shaft_r,
            (nut.x * 0.5).clamp(-half * 0.6, half * 0.6),
        );
        m.rods.push((
            Role::Silk(i as u8),
            Rod {
                a: nutw,
                b: wind,
                r: d.gauge[i] * 1.15,
            },
        ));
    }
    if d.machines {
        for s in [-1.0f32, 1.0] {
            let plate = |a: f32, c: f32| hp(a, -dep(a) * 0.5 + c, s * (wid(a) * 0.5 + 0.08));
            m.patches.push((
                Role::Brass,
                grid(
                    2,
                    2,
                    false,
                    |r, c| {
                        let a = len * (0.1 + 0.75 * r as f32);
                        plate(a, (c as f32 - 0.5) * dep(a) * 0.7)
                    },
                    move |_| wx * s,
                    |r, c| [r as f32, c as f32],
                ),
            ));
        }
    }
}
