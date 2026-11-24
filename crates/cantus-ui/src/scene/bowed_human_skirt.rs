//! r42:arcus-upper — the long skirt of the realistic woman's concert dress (`bowed_human`):
//! the velvet draped over her seated legs, from the waist over the thighs' tops and the
//! knees, falling from the knees to the floor in soft folds, a curtain between the legs (a
//! cello stands there) and round each leg's outer side down to the seat. Built from the
//! posed legs, so it follows every instrument's seat.

use super::bowed_figure::Skel;
use super::bowed_human::{side, Figure, Posed, FOOT, HIPS, SHIN, THIGH};
use super::bowed_mesh::{Mesh, Patch, Role};
use super::geo::{v, V3};

const UP: V3 = V3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// Rows down the skirt, columns across it.
const ROWS: usize = 26;
const PER_SEG: usize = 6;

fn catmull(p: &[V3], per: usize) -> Vec<V3> {
    let n = p.len();
    let at = |i: isize| p[i.clamp(0, n as isize - 1) as usize];
    let mut out = Vec::with_capacity((n - 1) * per + 1);
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (
            at(i as isize - 1),
            at(i as isize),
            at(i as isize + 1),
            at(i as isize + 2),
        );
        for k in 0..per {
            let t = k as f32 / per as f32;
            let (t2, t3) = (t * t, t * t * t);
            out.push(
                (p1 * 2.0
                    + (p2 - p0) * t
                    + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
                    + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3)
                    * 0.5,
            );
        }
    }
    out.push(p[n - 1]);
    out
}

/// The skirt over the posed legs.
pub fn skirt(m: &mut Mesh, fig: &Figure, p: &Posed, s: &Skel) {
    let b = &fig.bones;
    let mut hip = [V3::default(); 2];
    let mut knee = [V3::default(); 2];
    let mut ankle = [V3::default(); 2];
    for i in 0..2 {
        let o = side(i);
        hip[i] = p.at(HIPS, b[o + THIGH].head);
        knee[i] = p.at(o + THIGH, b[o + SHIN].head);
        ankle[i] = p.at(o + SHIN, b[o + FOOT].head);
    }
    let waist = (hip[0] + hip[1]) * 0.5 + UP * 9.0;
    let f = v(s.f.x, 0.0, s.f.z).norm();
    // Where the thighs are measured to: a seated woman's thigh about 8 cm round its axis
    // at the hip, 6.5 at the knee.
    let (r_hip, r_knee, r_hem) = (9.2f32, 8.0f32, 17.0f32);
    let k_row = ROWS * 2 / 5;
    let mut rows: Vec<Vec<V3>> = Vec::with_capacity(ROWS);
    for row in 0..ROWS {
        let mut ctl = [V3::default(); 5];
        let mut tops = [V3::default(); 2];
        let mut outs = [V3::default(); 2];
        for i in 0..2 {
            let t_dir = (knee[i] - hip[i]).norm();
            let up_n = (UP - t_dir * UP.dot(t_dir)).norm();
            let out = {
                let o = t_dir.cross(UP).norm();
                if o.dot(knee[i] - (knee[0] + knee[1]) * 0.5) < 0.0 {
                    -o
                } else {
                    o
                }
            };
            if row <= k_row {
                let t = row as f32 / k_row as f32;
                let from = waist.lerp(hip[i], 0.75);
                let axis = if t < 0.25 {
                    from.lerp(hip[i], t / 0.25)
                } else {
                    hip[i].lerp(knee[i], (t - 0.25) / 0.75)
                };
                let r = r_hip + (r_knee - r_hip) * t;
                tops[i] = axis + up_n * (r + 0.6) + f * (r * 0.2 * t);
                outs[i] = axis + out * (r + 1.0) - UP * (r * 0.8);
            } else {
                let t = (row - k_row) as f32 / (ROWS - 1 - k_row) as f32;
                // Over the kneecap, then straight down, a little forward of the shin, to
                // the floor where the hem spreads.
                let knee_front = knee[i] + f * (r_knee + 0.8) + UP * (r_knee * 0.5);
                // The hem down by the feet, over the insteps (the toes show).
                let ahead = (ankle[i] - knee[i]).dot(f).max(0.0) + 4.0;
                let floor = v(knee[i].x, 0.6, knee[i].z) + f * (r_knee + 3.0).max(ahead);
                let e = t * t * (3.0 - 2.0 * t);
                let fall = knee_front.lerp(floor, t);
                let fall = v(
                    knee_front.x + (fall.x - knee_front.x) * e.sqrt(),
                    fall.y,
                    knee_front.z + (fall.z - knee_front.z) * e.sqrt(),
                );
                let r = r_knee + (r_hem - r_knee) * t * t;
                tops[i] = fall;
                outs[i] = fall + out * r - f * (r * 0.45);
            }
        }
        let t_all = row as f32 / (ROWS - 1) as f32;
        let mid = (tops[0] + tops[1]) * 0.5;
        // Between the legs: sagging over the lap, then hanging back as a curtain.
        let sag = if row <= k_row {
            -UP * (3.5 + 3.0 * t_all)
        } else {
            -f * 2.5 - UP * 1.0
        };
        ctl[0] = outs[0];
        ctl[1] = tops[0];
        ctl[2] = mid + sag;
        ctl[3] = tops[1];
        ctl[4] = outs[1];
        let mut line = catmull(&ctl, PER_SEG);
        // Soft folds below the knees: deeper toward the hem.
        if row > k_row {
            let t = (row - k_row) as f32 / (ROWS - 1 - k_row) as f32;
            let n = line.len();
            for (c, q) in line.iter_mut().enumerate() {
                let u = c as f32 / (n - 1) as f32;
                let fold = (u * std::f32::consts::TAU * 7.0).sin() * 1.3 * t;
                *q = *q + f * fold;
            }
        }
        rows.push(line);
    }
    let cols = rows[0].len();
    let mut pts = Vec::with_capacity(ROWS * cols);
    for r in &rows {
        pts.extend_from_slice(r);
    }
    let mid = (knee[0] + knee[1]) * 0.5 - f * 20.0 + UP * 10.0;
    m.patches.push((
        Role::Velvet,
        Patch::of(
            pts,
            ROWS,
            cols,
            false,
            move |q| q - mid,
            |r, c| [c as f32, r as f32],
        ),
    ));
}
