//! r42:arcus-look — the fittings of a bowed instrument as surfaces (see `bowed_mesh`): the
//! carved f-holes (their eyes, the S of the stem, the wings and the nicks at the bridge's
//! feet), the bridge (its feet, legs, kidneys, heart and the arc its strings sit on), the
//! tailpiece with its fret and fine tuners, the saddle and tailgut, the endpin and its
//! collar, and the four strings in their steel or tungsten.

use super::bowed::{arch, bridge_y, dist, reach, string_local, tail_z, Dims, Pose};
use super::bowed_head::grid;
use super::bowed_mesh::{Mesh, Poly, Rod, Role};
use super::geo::{v, V3};
use std::f32::consts::{PI, TAU};

fn bump(x: f32, c: f32, w: f32) -> f32 {
    (-((x - c) / w).powi(2)).exp()
}

/// One f-hole's outline in the plate's (x, y), the viewer's right one (mirror for the
/// other): from its lower eye (outer) to its upper eye (inner), the nicks on the bridge's
/// line.
fn f_outline(d: &Dims) -> Vec<(f32, f32)> {
    let yb = bridge_y(d);
    let len = d.f_len;
    let lo = (d.f_lower, yb - 0.47 * len);
    let up = (d.f_upper, yb + 0.47 * len);
    let k = len / 12.8;
    let (r_lo, r_up, stem) = (0.72 * k, 0.5 * k, 0.24 * k);
    let ax = (up.0 - lo.0, up.1 - lo.1);
    let al = ax.0.hypot(ax.1);
    let (tx, ty) = (ax.0 / al, ax.1 / al);
    // Outward: away from the centre line.
    let (px, py) = (ty, -tx);
    let (px, py) = if px < 0.0 { (-px, -py) } else { (px, py) };
    let c = |s: f32| {
        let sway = 0.055 * al * (TAU * s).sin();
        (lo.0 + ax.0 * s + px * sway, lo.1 + ax.1 * s + py * sway)
    };
    // The half-width: the eyes' circles at the ends, the stem between.
    let w = |s: f32| {
        let dl = s * al;
        let du = (1.0 - s) * al;
        let mut w = stem;
        if dl < r_lo {
            w = w.max((r_lo * r_lo - dl * dl).max(0.0).sqrt());
        }
        if du < r_up {
            w = w.max((r_up * r_up - du * du).max(0.0).sqrt());
        }
        w
    };
    // The lower wing toward the centre (pointed up at its tip), the upper wing outward
    // (pointed down), each flaring from the stem to its point; the nicks either side at
    // the middle.
    let pointed = |t: f32| {
        if t <= 0.0 || t >= 1.0 {
            0.0
        } else {
            let e = ((t - 0.82) / 0.18).clamp(0.0, 1.0);
            t.powf(1.3) * (1.0 - e * e * (3.0 - 2.0 * e))
        }
    };
    let nick = |s: f32| 0.17 * k * bump(s, 0.5, 0.012);
    let wing_in = |s: f32| 1.05 * k * pointed((s - 0.06) / 0.27) + nick(s);
    let wing_out = |s: f32| 0.72 * k * pointed((0.9 - s) / 0.24) + nick(s);
    let n = 64;
    let mut out = Vec::with_capacity(2 * n + 24);
    for i in 0..=n {
        let s = i as f32 / n as f32;
        let (cx, cy) = c(s);
        let r = w(s) + wing_out(s);
        out.push((cx + px * r, cy + py * r));
    }
    // Over the upper eye.
    for j in 1..12 {
        let a = PI * j as f32 / 12.0;
        let (dx, dy) = (px * a.cos() + tx * a.sin(), py * a.cos() + ty * a.sin());
        out.push((up.0 + dx * r_up, up.1 + dy * r_up));
    }
    for i in (0..=n).rev() {
        let s = i as f32 / n as f32;
        let (cx, cy) = c(s);
        let r = w(s) + wing_in(s);
        out.push((cx - px * r, cy - py * r));
    }
    // Under the lower eye.
    for j in 1..12 {
        let a = PI * j as f32 / 12.0;
        let (dx, dy) = (-px * a.cos() - tx * a.sin(), -py * a.cos() - ty * a.sin());
        out.push((lo.0 + dx * r_lo, lo.1 + dy * r_lo));
    }
    out
}

/// The two f-holes, carved through the top: the pale cut, the dark of the box inside.
pub fn f_holes(m: &mut Mesh, d: &Dims, pose: &Pose, o: &[(f32, f32)]) {
    let f = f_outline(d);
    let rc = reach(d);
    let n = pose.ez;
    for s in [1.0f32, -1.0] {
        let on = |x: f32, y: f32, lift: f32| pose.at(x, y, arch(d.arch, dist(o, x, y), rc) + lift);
        let cut: Vec<V3> = f.iter().map(|&(x, y)| on(s * x, y, 0.02)).collect();
        // The hole a little in from the cut (the end grain's pale wall shows round it).
        let cx = f.iter().map(|p| p.0).sum::<f32>() / f.len() as f32;
        let cy = f.iter().map(|p| p.1).sum::<f32>() / f.len() as f32;
        let hole: Vec<V3> = f
            .iter()
            .map(|&(x, y)| {
                let (dx, dy) = (x - cx, y - cy);
                let l = dx.hypot(dy).max(1e-3);
                let sh = (0.05 * d.f_len / 12.8).min(l * 0.2);
                on(s * (x - dx / l * sh), y - dy / l * sh + 0.03, 0.03)
            })
            .collect();
        let (cut, hole) = if s < 0.0 {
            (
                cut.into_iter().rev().collect(),
                hole.into_iter().rev().collect(),
            )
        } else {
            (cut, hole)
        };
        m.polys.push((Role::Cut, Poly { pts: cut, n }));
        m.polys.push((Role::Hole, Poly { pts: hole, n }));
    }
}

/// The bridge's outline (x, h) and its cut-outs, cm, for the instrument's size.
/// A plane outline (x, h), cm.
type Outline = Vec<(f32, f32)>;

fn bridge_shape(d: &Dims) -> (Outline, Vec<Outline>) {
    let (kx, kh) = (d.bridge_w / 9.0, d.bridge_h / 8.8);
    let half: [(f32, f32); 17] = [
        (0.0, 1.9),
        (0.9, 1.75),
        (1.6, 1.3),
        (2.05, 0.6),
        (2.2, 0.0),
        (4.45, 0.0),
        (4.5, 0.35),
        (4.0, 0.9),
        (3.55, 1.7),
        (3.35, 2.6),
        (2.95, 3.3),
        (3.55, 3.75),
        (4.15, 4.35),
        (4.55, 5.6),
        (4.75, 7.2),
        (4.6, 7.75),
        (4.0, 8.15),
    ];
    let mut right: Vec<(f32, f32)> = half.iter().map(|&(x, h)| (x * kx, h * kh)).collect();
    // The top's arc to the middle.
    for j in 1..8 {
        let t = j as f32 / 8.0;
        right.push((
            4.0 * kx * (1.0 - t),
            (8.15 + 0.65 * (t * PI / 2.0).sin()) * kh,
        ));
    }
    // Up the right side from the middle of the legs' arch, over the top, down the left.
    let mut out = right.clone();
    out.push((0.0, 8.8 * kh));
    out.extend(right.iter().skip(1).rev().map(|&(x, h)| (-x, h)));
    // The heart, and the kidneys either side.
    let heart: Vec<(f32, f32)> = (0..24)
        .map(|k| {
            let t = TAU * k as f32 / 24.0;
            let x = 16.0 * t.sin().powi(3);
            let y =
                13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
            (x * 0.05 * kx, (4.75 - y * 0.05) * kh)
        })
        .collect();
    let kidney = |s: f32| -> Vec<(f32, f32)> {
        (0..16)
            .map(|k| {
                let t = TAU * k as f32 / 16.0;
                let (a, b) = (0.75, 0.24 + 0.08 * t.sin());
                let (lx, ly) = (a * t.cos(), b * t.sin());
                let ang = s * 1.05;
                let (x, y) = (
                    lx * ang.cos() - ly * ang.sin(),
                    lx * ang.sin() + ly * ang.cos(),
                );
                ((s * 2.35 + x) * kx, (3.15 + y) * kh)
            })
            .collect()
    };
    (out, vec![heart, kidney(1.0), kidney(-1.0)])
}

/// The bridge, the tailpiece, the saddle and tailgut, the endpin, the strings.
pub fn fittings(m: &mut Mesh, d: &Dims, pose: &Pose, o: &[(f32, f32)]) {
    let yb = bridge_y(d);
    let rc = reach(d);
    let (ex, ey, ez) = (pose.ex, pose.ey, pose.ez);
    // ---- the bridge ----
    let (shape, cuts) = bridge_shape(d);
    let foot_x = d.bridge_w * 0.36;
    let zf = arch(d.arch, dist(o, foot_x, yb), rc) - 0.05;
    let thick = |h: f32| (0.48 - 0.32 * (h / d.bridge_h).clamp(0.0, 1.0)) * d.bridge_w / 9.0;
    for (s, n) in [(1.0f32, ey), (-1.0, -ey)] {
        let face: Vec<V3> = shape
            .iter()
            .map(|&(x, h)| pose.at(x, yb + s * thick(h) * 0.5, zf + h))
            .collect();
        let face = if s < 0.0 {
            face.into_iter().rev().collect()
        } else {
            face
        };
        m.polys.push((Role::Bridge, Poly { pts: face, n }));
        for c in &cuts {
            let p: Vec<V3> = c
                .iter()
                .map(|&(x, h)| pose.at(x, yb + s * (thick(h) * 0.5 + 0.01), zf + h))
                .collect();
            let p = if s < 0.0 {
                p.into_iter().rev().collect()
            } else {
                p
            };
            m.polys.push((Role::BridgeCut, Poly { pts: p, n }));
        }
    }
    let ns = shape.len();
    let bridge_mid = pose.at(0.0, yb, zf + d.bridge_h * 0.5);
    m.patches.push((
        Role::Bridge,
        grid(
            2,
            ns,
            true,
            |r, c| {
                let (x, h) = shape[c];
                let s = if r == 0 { 1.0 } else { -1.0 };
                pose.at(x, yb + s * thick(h) * 0.5, zf + h)
            },
            move |q| {
                let l = q - bridge_mid;
                l - ey * l.dot(ey)
            },
            |r, c| [c as f32 * 0.3, r as f32],
        ),
    ));
    // ---- the tailpiece: cambered top, sides, its fret, the fine tuners ----
    let y1 = yb - d.after;
    let y0 = y1 - d.tail_len;
    let tw = move |y: f32| {
        let t = ((y - y0) / (y1 - y0)).clamp(0.0, 1.0);
        let w = d.tail_w.0 + (d.tail_w.1 - d.tail_w.0) * t.powf(0.8);
        // The fret end rounded.
        let e = ((y1 - y) / (d.tail_w.1 * 0.5)).clamp(0.0, 1.0);
        w * (1.0 - (1.0 - e).powi(2)).sqrt().max(0.15)
    };
    let th = d.tail_w.1 * 0.2;
    let rows = 16;
    let ty = move |r: usize| y0 + (y1 - y0) * r as f32 / (rows - 1) as f32;
    let tz = move |x: f32, y: f32| {
        let w = tw(y);
        tail_z(d, y) + th + 0.08 * d.tail_w.1 * (1.0 - (2.0 * x / w).powi(2)).max(0.0)
    };
    m.patches.push((
        Role::Ebony,
        grid(
            rows,
            7,
            false,
            |r, c| {
                let y = ty(r);
                let x = (c as f32 / 6.0 - 0.5) * tw(y);
                pose.at(x, y, tz(x, y))
            },
            move |_| ez,
            |r, c| [ty(r), c as f32],
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
                    let y = ty(r);
                    let x = s * tw(y) * 0.5;
                    pose.at(x, y, if c == 0 { tz(x, y) } else { tail_z(d, y) })
                },
                move |_| ex * s,
                |r, c| [ty(r), c as f32],
            ),
        ));
    }
    let fret_z = tz(0.0, y1 - 0.4) + 0.15;
    m.rods.push((
        Role::Ebony,
        Rod {
            a: pose.at(-tw(y1 - 0.4) * 0.42, y1 - 0.4, fret_z),
            b: pose.at(tw(y1 - 0.4) * 0.42, y1 - 0.4, fret_z),
            r: 0.22 * d.tail_w.1 / 5.4,
        },
    ));
    if !d.machines {
        for i in 0..4 {
            let [t, _, _] = string_local(d, i);
            let base = pose.at(t.x, y1 - 2.2, tz(t.x, y1 - 2.2));
            m.rods.push((
                Role::Nickel,
                Rod {
                    a: base,
                    b: base + ez * 0.9,
                    r: 0.22,
                },
            ));
            m.rods.push((
                Role::Nickel,
                Rod {
                    a: base + ez * 0.5,
                    b: base + ez * 0.25 - ey * 2.3,
                    r: 0.14,
                },
            ));
        }
    }
    // ---- the saddle, the tailgut over it to the endpin's collar ----
    let sw = d.tail_w.0 * 1.15;
    m.patches.push((
        Role::Ebony,
        grid(
            3,
            2,
            false,
            |r, c| {
                let x = (c as f32 - 0.5) * sw;
                let (y, z) = [(1.4, 0.42), (0.2, 0.95), (-0.25, 0.4)][r];
                pose.at(x, y, z)
            },
            move |_| ez,
            |r, c| [r as f32, c as f32],
        ),
    ));
    for s in [-1.0f32, 1.0] {
        let a = pose.at(s * d.tail_w.0 * 0.22, y0 + 0.4, tail_z(d, y0) + th * 0.5);
        let b = pose.at(s * d.tail_w.0 * 0.25, 0.3, 1.05);
        let c = pose.at(s * 0.4, -0.4, -1.2);
        m.rods.push((Role::Rubber, Rod { a, b, r: 0.16 }));
        m.rods.push((
            Role::Rubber,
            Rod {
                a: b,
                b: c,
                r: 0.16,
            },
        ));
    }
    // ---- the endpin: its ebony collar, the steel pin to the floor ----
    let zc = -d.rib_lo * 0.5;
    let k = d.rib_lo / 12.2;
    m.rods.push((
        Role::Ebony,
        Rod {
            a: pose.at(0.0, 0.2, zc),
            b: pose.at(0.0, -1.3 * k, zc),
            r: 1.5 * k,
        },
    ));
    if d.held {
        // A violin's or a viola's end button (the collar above) and no pin: it is held; its
        // foot (for the floor's shadow) under its middle. // r42:arcus-upper
        m.foot = pose.at(0.0, 0.5 * d.body, -d.rib_lo); // r42:arcus-upper
        draw_strings(m, d, pose); // r42:arcus-upper
        return; // r42:arcus-upper
    }
    m.rods.push((
        Role::Nickel,
        Rod {
            a: pose.at(0.0, -1.3 * k, zc),
            b: pose.at(0.0, -d.endpin + 2.0, zc),
            r: 0.45 * k,
        },
    ));
    m.rods.push((
        Role::Nickel,
        Rod {
            a: pose.at(0.0, -d.endpin + 2.0, zc),
            b: pose.at(0.0, -d.endpin, zc),
            r: 0.18 * k,
        },
    ));
    m.foot = pose.at(0.0, -d.endpin, zc);
    m.shadow.push(m.foot);
    // The endpin's rest: a black rubber disc on the floor the point sits in, as a player
    // puts down so it never slides — its top and its rim.
    let rest = d.rib_lo * 0.32;
    let (fx, fz) = (m.foot.x, m.foot.z);
    let ring = move |r: f32, y: f32, k: usize| {
        let a = TAU * k as f32 / 24.0;
        v(fx + r * a.cos(), y, fz + r * a.sin())
    };
    let up = v(0.0, 1.0, 0.0);
    m.patches.push((
        Role::Rubber,
        grid(
            2,
            24,
            true,
            |r, k| ring(rest * r as f32, 1.1, k),
            move |_| up,
            |r, k| [r as f32, k as f32],
        ),
    ));
    m.patches.push((
        Role::Rubber,
        grid(
            2,
            24,
            true,
            |r, k| ring(rest, 1.1 * (1 - r) as f32, k),
            move |q| v(q.x - fx, 0.0, q.z - fz),
            |r, k| [r as f32, k as f32],
        ),
    ));
    draw_strings(m, d, pose); // r42:arcus-upper
}

/// The strings: steel (the cello's A and D, the violin's E) or wound. // r42:arcus-upper
fn draw_strings(m: &mut Mesh, d: &Dims, pose: &Pose) {
    for i in 0..4 {
        let [t, b, n] = string_local(d, i);
        let steel = if d.held { i == 3 } else { i >= 2 }; // r42:arcus-upper
        let role = if !d.machines && steel {
            Role::Steel
        } else {
            Role::Wound
        };
        let r = d.gauge[i];
        let (tw_, bw, nw) = (
            pose.at(t.x, t.y, t.z),
            pose.at(b.x, b.y, b.z),
            pose.at(n.x, n.y, n.z),
        );
        // Its silk where it leaves the tailpiece.
        let silk_end = tw_ + (bw - tw_).norm() * (0.9 * d.tail_w.1 / 5.4);
        m.rods.push((
            Role::Silk(i as u8),
            Rod {
                a: tw_,
                b: silk_end,
                r: r * 1.2,
            },
        ));
        m.rods.push((
            role,
            Rod {
                a: silk_end,
                b: bw,
                r,
            },
        ));
        m.rods.push((role, Rod { a: bw, b: nw, r }));
    }
}
