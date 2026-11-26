//! r42:arcus-upper — the realistic figures' left hand on a violin's or a viola's neck, posed
//! from the table authored offline (`scene::figure::neck_hand`,
//! `tools/arcus-figures/left_hand.py`; the owner, 2026-11-24: "the violin's left hand is a
//! disaster"). What a violinist's hand keeps, for every string, finger and position the
//! table holds: the playing fingertip — the skinned hand's own — on its stop within a pixel;
//! the neck in the crook (the index finger's base against the E side, the thumb up the G
//! side about level with the fingerboard, a gap under the palm); each finger arched high
//! (MCP 45–70°, PIP 70–100°, DIP 20–50° on the playing one, its last phalanx nearly
//! upright); the wrist straight.
//!
//! `CANTUS_HAND_DUMP=<file> cargo test -p cantus-ui --test arcus_hand_table -- --ignored
//! dump` writes what the tool solves against (the strings, the fingerboard, the shoulder,
//! the camera, every row's targets).

use cantus_ui::scene::bowed::{board_w, board_z, nut_y, rig_for, Bowed};
use cantus_ui::scene::bowed_draw;
use cantus_ui::scene::bowed_hold::{left_arm, Hold, Holder};
use cantus_ui::scene::geo::V3;
use cantus_ui::scene::Room;
use cantus_ui::views::inst::arcus::hand;

/// The positions the table holds (semitones of the first finger over the open string).
const AT: [f32; 7] = [1.0, 3.0, 5.0, 7.0, 9.0, 12.0, 15.0];

fn j(p: V3) -> serde_json::Value {
    serde_json::json!([p.x, p.y, p.z])
}

#[test]
#[ignore = "writes the tool's inputs: CANTUS_HAND_DUMP=<file>"]
fn dump_left_hand_inputs() {
    let path = std::env::var("CANTUS_HAND_DUMP").expect("CANTUS_HAND_DUMP=<file>");
    let mut all = Vec::new();
    for (who, hold) in [("woman", Hold::Woman), ("man", Hold::Man)] {
        for (inst, b) in [("violin", Bowed::Violin), ("viola", Bowed::Viola)] {
            let h = Holder::of(hold);
            let rig = rig_for(b, h);
            let d = b.dims();
            let arm = left_arm(b, h).expect("a figure");
            let cam = bowed_draw::camera(b, h, Room::Studio);
            let span = hand::span(true);
            let gap = span / 3.0;
            let mut rows = Vec::new();
            for string in 0..4 {
                for p in 0..5usize {
                    for &at in &AT {
                        let semis = if p < 4 { at + p as f32 * gap } else { 0.0 };
                        let l = hand::Left {
                            rig: &rig,
                            d,
                            string,
                            semis,
                            at,
                            rock: 0.0,
                            side: rig.pose.ex,
                            span,
                        };
                        let la = hand::left_at(&arm, &l);
                        rows.push(serde_json::json!({
                            "string": string, "finger": p, "at": at, "semis": semis,
                            "a": j(la.a), "bw": j(la.bw), "kx": j(la.kx), "z": j(la.z),
                            "hw": la.hw, "top": la.top,
                            "playing": la.playing.map_or(-1, |x| x as i32),
                            "tips": la.tips.iter().map(|&t| j(t)).collect::<Vec<_>>(),
                        }));
                    }
                }
            }
            // r42:arcus-hands — the neck, the fingerboard and the body as drawn.
            let solid = solid_of(b);
            let ys: Vec<f32> = (0..=120)
                .map(|i| d.body - 4.0 + (nut_y(d) + 1.0 - d.body + 4.0) * i as f32 / 120.0)
                .collect();
            all.push(serde_json::json!({
                "who": who, "inst": inst,
                "o": j(rig.pose.o), "ex": j(rig.pose.ex), "ey": j(rig.pose.ey), "ez": j(rig.pose.ez),
                "strings": (0..4).map(|i| serde_json::json!({
                    "bridge": j(rig.strings[i][1]), "nut": j(rig.strings[i][2]),
                    "normal": j(rig.normal[i]), "gauge": rig.gauge[i]})).collect::<Vec<_>>(),
                "body": d.body, "nut_y": nut_y(d), "depth": hand::neck_depth(d),
                "board": ys.iter().map(|&y| serde_json::json!([y, board_w(d, y) * 0.5, board_z(d, y)])).collect::<Vec<_>>(),
                "shoulder": j(arm.shoulder), "upper": arm.upper, "fore": arm.fore, "pole": j(arm.pole),
                "eye": j(cam.eye()), "cam_f": j(cam.f), "cam_r": j(cam.r), "cam_u": j(cam.u),
                "solid": solid.json(),
                "rows": rows,
            }));
        }
    }
    std::fs::write(
        &path,
        serde_json::to_string(&serde_json::json!({ "at": AT, "figs": all })).unwrap(),
    )
    .expect("write the dump");
}

/// r42:arcus-hands — the violin's or the viola's neck, fingerboard and body as the picture
/// draws them, in the instrument's own frame (x the E side, y toward the scroll, z the
/// fingerboard's normal): a signed distance (cm, negative inside) the hands are kept out
/// of. The fingerboard's and the neck's sections are `bowed_head::neck`'s own patches'
/// rows (the fingerboard's top arc and its underside; the neck's rounded back closed along
/// its top), the body the outline with the plates' arching over and under the ribs.
struct Solid {
    board: Vec<(f32, Vec<(f32, f32)>)>,
    shaft: Vec<(f32, Vec<(f32, f32)>)>,
    outline: Vec<(f32, f32)>,
    d: &'static cantus_ui::scene::bowed::Dims,
}

/// The signed distance from `p` to the closed polygon `poly` (negative inside).
fn sd_poly(poly: &[(f32, f32)], p: (f32, f32)) -> f32 {
    let n = poly.len();
    let mut best = f32::MAX;
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = (dx * dx + dy * dy).max(1e-12);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
        let (ex, ey) = (a.0 + dx * t - p.0, a.1 + dy * t - p.1);
        best = best.min(ex * ex + ey * ey);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * dx {
            inside = !inside;
        }
    }
    if inside {
        -best.sqrt()
    } else {
        best.sqrt()
    }
}

fn solid_of(b: Bowed) -> Solid {
    use cantus_ui::scene::bowed::{outline, Pose};
    use cantus_ui::scene::bowed_head;
    use cantus_ui::scene::bowed_mesh::{Mesh, Role};
    let d = b.dims();
    let mut m = Mesh::default();
    let id = Pose {
        o: V3::default(),
        ex: V3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        ey: V3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        ez: V3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
    };
    bowed_head::neck(&mut m, d, &id);
    // (The order `neck` pushes them in: the fingerboard's top, its two sides, its end, its
    // underside, the nut, then the neck's back.)
    let p = &m.patches;
    let (top, under) = (&p[0].1, &p[4].1);
    let shaft = &p[6].1;
    assert!(
        p[0].0 == Role::Ebony && p[4].0 == Role::Ebony && p[6].0 == Role::Maple,
        "bowed_head::neck's patches as the solid reads them"
    );
    assert!(top.cols >= 3 && under.cols == 2 && under.rows == top.rows && shaft.cols >= 7);
    let board = (0..top.rows)
        .map(|r| {
            let mut poly: Vec<(f32, f32)> = (0..top.cols)
                .map(|c| {
                    let q = top.pts[r * top.cols + c];
                    (q.x, q.z)
                })
                .collect();
            for c in [1, 0] {
                let q = under.pts[r * 2 + c];
                poly.push((q.x, q.z));
            }
            (top.pts[r * top.cols].y, poly)
        })
        .collect();
    let shaft = (0..shaft.rows)
        .map(|r| {
            let poly: Vec<(f32, f32)> = (0..shaft.cols)
                .map(|c| {
                    let q = shaft.pts[r * shaft.cols + c];
                    (q.x, q.z)
                })
                .collect();
            (shaft.pts[r * shaft.cols].y, poly)
        })
        .collect();
    Solid {
        board,
        shaft,
        outline: outline(b, 8),
        d,
    }
}

impl Solid {
    /// The signed distance (cm) of a section stack at local `q`: the two sections either
    /// side of its y blended, past the stack's ends the distance to its end face.
    fn stack(s: &[(f32, Vec<(f32, f32)>)], q: V3) -> f32 {
        let (y0, y1) = (s[0].0, s[s.len() - 1].0);
        let y = q.y.clamp(y0, y1);
        let i = s
            .iter()
            .position(|r| r.0 >= y)
            .unwrap_or(s.len() - 1)
            .max(1);
        let (a, b) = (&s[i - 1], &s[i]);
        let t = ((y - a.0) / (b.0 - a.0).max(1e-6)).clamp(0.0, 1.0);
        let poly: Vec<(f32, f32)> =
            a.1.iter()
                .zip(b.1.iter())
                .map(|(p, r)| (p.0 + (r.0 - p.0) * t, p.1 + (r.1 - p.1) * t))
                .collect();
        let d2 = sd_poly(&poly, (q.x, q.z));
        let e = (y0 - q.y).max(q.y - y1);
        if e <= 0.0 {
            d2.max(e)
        } else if d2 > 0.0 {
            (d2 * d2 + e * e).sqrt()
        } else {
            e
        }
    }

    /// The body's signed distance at local `q` (the outline across, the top's arch over
    /// the edge plane, the back's under the ribs).
    fn body(&self, q: V3) -> f32 {
        use cantus_ui::scene::bowed::{arch, dist, reach, rib};
        let d = self.d;
        let dd = dist(&self.outline, q.x, q.y);
        let inside = sd_poly(&self.outline, (q.x, q.y)) < 0.0;
        let (zt, zb) = if inside {
            (
                arch(d.arch, dd, reach(d)),
                -rib(d, q.y.clamp(0.0, d.body)) - arch(d.arch_back, dd, reach(d)),
            )
        } else {
            (0.42, -rib(d, q.y.clamp(0.0, d.body)) - 0.42)
        };
        let dz = (q.z - zt).max(zb - q.z);
        let dxy = if inside { -dd } else { dd };
        if dxy > 0.0 && dz > 0.0 {
            (dxy * dxy + dz * dz).sqrt()
        } else {
            dxy.max(dz)
        }
    }

    /// The signed distance at local `q`: the fingerboard, the neck, the body.
    fn at(&self, q: V3) -> f32 {
        Self::stack(&self.board, q)
            .min(Self::stack(&self.shaft, q))
            .min(self.body(q))
    }

    /// The dump's form: each section stack as [y, [[x, z], ...]] rows, the outline, the
    /// plates' arching and the ribs.
    fn json(&self) -> serde_json::Value {
        use cantus_ui::scene::bowed::{reach, rib};
        let st = |s: &[(f32, Vec<(f32, f32)>)]| {
            s.iter()
                .map(|(y, p)| {
                    serde_json::json!([y, p.iter().map(|q| [q.0, q.1]).collect::<Vec<_>>()])
                })
                .collect::<Vec<_>>()
        };
        let d = self.d;
        serde_json::json!({
            "board": st(&self.board), "shaft": st(&self.shaft),
            "outline": self.outline.iter().map(|q| [q.0, q.1]).collect::<Vec<_>>(),
            "arch": d.arch, "arch_back": d.arch_back, "reach": reach(d), "flat_back": d.flat_back,
            "ribs": (0..=40).map(|i| { let y = d.body * i as f32 / 40.0; [y, rib(d, y)] }).collect::<Vec<_>>(),
        })
    }
}

/// How far the playing finger's last phalanx may lean from the fingerboard's normal
/// (degrees; r42:arcus-hand — the tip comes down onto its string from above, "upright"
/// meaning square to the fingerboard, not up in the room; was 45. The photographs' pads
/// meet the lower strings a little flatter than the upper ones).
const UPRIGHT: f32 = 30.0;

/// r42:arcus-hand — the photographs' gates (`tools/arcus-figures/left_hand_refs.md`): how
/// far the forearm may turn from the hand's line at the wrist (degrees), the least the
/// back of the hand faces the picture's camera (the cosine: it shows its back at a slant,
/// never its palm nor its edge), and where the index finger's base knuckle sits against the
/// neck's E side (cm out from the fingerboard's edge) and the knuckles' height against the
/// fingerboard's top (cm).
/// (r42:arcus-hands: 16 — the photographs' 3–23°, median ~10; was 8, which kept the hand
/// leaning along the neck.)
const WRIST: f32 = 16.0;
/// How much wider the hand's outline may be at its middle than at the wrist's crease where
/// the index or the middle finger plays low on the neck (the photographs: 1.1–1.3; ours
/// 1.05–1.3 but one viola case at 1.42).
const PALM_WIDE: f32 = 1.45;
/// The same while the little or the ring finger reaches its note or the hand is high up the
/// neck (it turns broader to the camera there; measured 1.4–1.63 — wider than the
/// photographs' 1.1–1.3, open in the handoff).
const PALM_WIDE_REACH: f32 = 1.7;
/// How far a finger's last phalanx may leave the plane of its first two (degrees).
const SIDEWAYS: f32 = 5.0;
/// The back of the hand's width at the knuckles over its length (wrist to knuckles) as the
/// picture shows it: the photographs' front views 0.5–0.9 (the hand at a slant, never
/// broadside); a high position, the forearm straight up under the neck, a little broader.
const BACK_WL: (f32, f32) = (0.45, 0.95);
const BACK_WL_HIGH: f32 = 1.2;
/// The middle finger (its base joint to its tip, along its bones) over the back of the hand
/// (the wrist to that joint): a real hand's about 1 (the knuckle fix; MakeHuman's own rig
/// read 0.87, its fingers bending a centimetre and a half past the knuckles).
const FINGER_PALM: (f32, f32) = (0.95, 1.35);
const BACK_TO_CAMERA: f32 = 0.2;
const INDEX_OUT: (f32, f32) = (-0.5, 2.2);
/// r42:arcus-hands — the hand's line to the fingerboard's edge in the picture (degrees): the
/// photographs' front views 70-89 (`left_hand_refs.md`), a few degrees' give.
const HAND_NECK: (f32, f32) = (68.0, 92.0);
const KNUCKLES_UP: (f32, f32) = (-1.6, 2.6);

/// The angle between two ways, degrees.
fn deg(a: V3, b: V3) -> f32 {
    a.norm().dot(b.norm()).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Every figure, instrument, string and a spread of notes: the table's hand keeps what a
/// violinist's hand keeps (the module's rules), the skinned hand's own fingertip on its stop
/// within a pixel at 100 %.
#[test]
fn the_tables_hand_plays_as_a_violinists_does() {
    use cantus_ui::scene::bowed_human::{self as hu, finger, side};
    use cantus_ui::scene::figure::neck_hand;
    let size = egui::vec2(590.0, 430.0);
    let mut bad: Vec<String> = Vec::new();
    let mut worst = [0.0f32; 8];
    for (who, hold) in [(hu::Who::Woman, Hold::Woman), (hu::Who::Man, Hold::Man)] {
        for b in [Bowed::Violin, Bowed::Viola] {
            let h = Holder::of(hold);
            let rig = rig_for(b, h);
            let d = b.dims();
            let arm = left_arm(b, h).expect("a figure");
            let fig = hu::figure(who);
            {
                let o = side(0);
                let bn = |k: usize| fig.bones[o + finger(2, k)];
                let finger_len = (bn(0).tail - bn(0).head).len()
                    + (bn(1).tail - bn(1).head).len()
                    + (bn(2).tail - bn(2).head).len();
                let palm = (bn(0).head - fig.bones[o + hu::HAND].head).len();
                let r = finger_len / palm;
                assert!(
                    (FINGER_PALM.0..=FINGER_PALM.1).contains(&r),
                    "{who:?}: the middle finger {finger_len:.1} cm over the back of the hand {palm:.1} cm = {r:.2}"
                );
            }
            let base = hu::rest_body(b, who);
            let mut cam = bowed_draw::camera(b, h, Room::Studio);
            // The palm's normal at rest (out of the palm), as the tool takes it.
            let palm_rest = {
                let o = side(0);
                let kn = |f: usize| fig.bones[o + finger(f, 0)].head;
                let wr = fig.bones[o + hu::HAND].head;
                let mut nb = (kn(1) - kn(4)).cross((kn(2) + kn(3)) * 0.5 - wr).norm();
                if nb.x < 0.0 {
                    nb = nb * -1.0;
                }
                nb * -1.0
            };
            let fr = bowed_draw::frame_for(size);
            let [l0, t0, r0, b0] = fr.inset;
            cam.fit(
                &bowed_draw::points(b, h),
                (l0, t0, size.x - l0 - r0, size.y - t0 - b0),
                fr.margin,
                fr.anchor,
            );
            let span = hand::span(true);
            for string in 0..4 {
                for semis in [1.0f32, 2.0, 3.0, 5.0, 7.0, 9.0, 12.0, 14.0] {
                    let at = hand::target(1.0, semis, span);
                    let l = hand::Left {
                        rig: &rig,
                        d,
                        string,
                        semis,
                        at,
                        rock: 0.0,
                        side: rig.pose.ex,
                        span,
                    };
                    assert!(
                        neck_hand::row(who, b == Bowed::Viola, string, Some(0), at).is_some(),
                        "the table is there"
                    );
                    let lp = hand::left_pose(&arm, &l);
                    let la = hand::left_at(&arm, &l);
                    let j = lp.playing.expect("a stopped note");
                    let tag = format!("{who:?} {b:?} string {string} +{semis}");
                    // The skinned hand's own fingertip (its last bone's end) on the stop.
                    let aim = hu::ArmAim {
                        elbow: lp.elbow,
                        wrist: lp.wrist,
                        hand: Some(hu::HandAim {
                            fingers: lp.fingers,
                            thumb: lp.thumb,
                            exact: lp.playing,
                            rots: lp.rots,
                        }),
                    };
                    let posed = hu::repose_arm(fig, &base.0, 0, &aim);
                    let bone = side(0) + finger(j + 1, 2);
                    let tip = posed.at(bone, fig.bones[bone].tail);
                    // r42:arcus-hand — no mitten (the owner: "la parte abultada gorda"): the
                    // hand's outline from the camera at its middle no wider than PALM_WIDE
                    // times at the wrist.
                    {
                        // The hand's and the forearm's own vertices: each in a triangle whose
                        // corners are all at least 0.6 theirs (as hand_study.py measures).
                        let o = side(0);
                        let share = |i: usize| {
                            fig.bw[i]
                                .iter()
                                .filter(|(b, _)| {
                                    (*b as usize) >= o + hu::FORE1
                                        && (*b as usize) <= o + finger(4, 2)
                                })
                                .map(|x| x.1)
                                .sum::<f32>()
                        };
                        let mut used = vec![false; fig.pos.len()];
                        for t in &fig.tris {
                            if t.iter().all(|&i| share(i as usize) > 0.6) {
                                for &i in t {
                                    used[i as usize] = true;
                                }
                            }
                        }
                        let pts: Vec<V3> = (0..fig.pos.len())
                            .filter(|&i| used[i])
                            .map(|i| posed.vertex(fig, i))
                            .collect();
                        let mid = (lp.fingers[1][0] + lp.fingers[2][0]) * 0.5;
                        let ax = (mid - lp.wrist).norm();
                        let len = (mid - lp.wrist).len();
                        let width = |t: f32| {
                            let c = lp.wrist + ax * (len * t);
                            let across = (cam.eye() - c).norm().cross(ax).norm();
                            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                            for &p in &pts {
                                if ((p - c).dot(ax)).abs() < 0.35 {
                                    let w = (p - c).dot(across);
                                    lo = lo.min(w);
                                    hi = hi.max(w);
                                }
                            }
                            hi - lo
                        };
                        // (The wrist's crease, a centimetre into the hand from the wrist's
                        // joint, as the photographs' wrist is measured.)
                        let (w0, w1) = (width(0.12), width(0.5));
                        let wide = if semis <= 3.0 {
                            PALM_WIDE
                        } else {
                            PALM_WIDE_REACH
                        };
                        if w0 > 0.0 && w1 > w0 * wide {
                            bad.push(format!(
                                "{tag}: the palm {w1:.1} cm wide at its middle, the wrist {w0:.1}"
                            ));
                        }
                    }
                    let stop = lp.tip.expect("its stop");
                    let off = cam.p(tip) - cam.p(stop);
                    assert!(off.length() < 1.0, "{tag}: the fingertip {off:?} px off");
                    // The playing finger arched: MCP, PIP, DIP within the photographs'
                    // ranges (a few degrees' give), its last phalanx nearly upright.
                    let f = lp.fingers[j];
                    let meta = f[0] - lp.wrist;
                    let (mcp, pip, dip) = (
                        deg(meta, f[1] - f[0]),
                        deg(f[1] - f[0], f[2] - f[1]),
                        deg(f[2] - f[1], f[3] - f[2]),
                    );
                    let upright = deg(f[3] - f[2], -la.z);
                    worst[0] = worst[0].max(upright);
                    worst[1] = worst[1].max((mcp - 57.5).abs());
                    worst[2] = worst[2].max((pip - 85.0).abs());
                    worst[3] = worst[3].max((dip - 15.0).abs());
                    // (r42:arcus-hands: open arches — the last joint nearly straight onto the
                    // string, 5-20° in crop-08 and crop-11, the middle joint carrying the bend.)
                    if !(15.0..=80.0).contains(&mcp)
                        || !(50.0..=110.0).contains(&pip)
                        || !(0.0..=45.0).contains(&dip)
                        || upright > UPRIGHT
                    {
                        bad.push(format!(
                            "{tag}: MCP {mcp:.0}° PIP {pip:.0}° DIP {dip:.0}°, \
                             {upright:.0}° off upright"
                        ));
                    }
                    // The neck in the crook: the index finger's base on the E side, the
                    // thumb's tip up the G side about level with the fingerboard.
                    let loc = |w: V3| {
                        let q = w - rig.pose.o;
                        (q.dot(rig.pose.ex), q.dot(rig.pose.ey), q.dot(rig.pose.ez))
                    };
                    let (ix, _, _) = loc(lp.fingers[0][0]);
                    let (tx, ty, tz) = loc(lp.thumb[3]);
                    let top = board_z(d, ty);
                    // r42:arcus-hand — the knuckle line on the near (E) side: the index's
                    // base against it, every base knuckle out past the fingerboard's edge
                    // and about level with its top.
                    let (_, iy, iz) = loc(lp.fingers[0][0]);
                    let ihw = board_w(d, iy) * 0.5;
                    // (Up the G and D strings the hand comes round over the fingerboard
                    // and the thumb under the neck, as players' do: the index's base may sit
                    // a centimetre over the edge there.)
                    let index_in = if semis > 5.0 { -1.0 } else { INDEX_OUT.0 };
                    if !(index_in..=INDEX_OUT.1).contains(&(ix - ihw)) {
                        bad.push(format!(
                            "{tag}: the index's base {:.1} cm out from the E edge",
                            ix - ihw
                        ));
                    }
                    for (f, fj) in lp.fingers.iter().enumerate() {
                        let (kx, ky, kz) = loc(fj[0]);
                        let (khw, ktop) = (board_w(d, ky) * 0.5, board_z(d, ky));
                        if kx <= khw + index_in
                            || !(KNUCKLES_UP.0..=KNUCKLES_UP.1).contains(&(kz - ktop))
                        {
                            bad.push(format!(
                                "{tag}: knuckle {f} at {:.1} cm out, {:.1} cm over the top",
                                kx - khw,
                                kz - ktop
                            ));
                        }
                    }
                    let _ = iz;
                    // The back of the hand toward the audience (the picture's camera), at
                    // a slant; the wrist straight within WRIST.
                    let rots = lp.rots.expect("the table's turns");
                    let back = rots.hand.apply(palm_rest) * -1.0;
                    let mid = (lp.fingers[1][0] + lp.fingers[2][0]) * 0.5;
                    let facing = back.dot((cam.eye() - mid).norm());
                    worst[4] = worst[4].max(-facing);
                    if facing < BACK_TO_CAMERA {
                        bad.push(format!(
                            "{tag}: the back of the hand faces the camera at {facing:.2}"
                        ));
                    }
                    // The back at a slant in the picture (BACK_WL).
                    let wl = (cam.p(lp.fingers[0][0]) - cam.p(lp.fingers[3][0])).length()
                        / (cam.p(mid) - cam.p(lp.wrist)).length().max(1e-3);
                    let hi = if semis > 8.0 { BACK_WL_HIGH } else { BACK_WL.1 };
                    if !(BACK_WL.0..=hi).contains(&wl) {
                        bad.push(format!(
                            "{tag}: the back of the hand {wl:.2} wide over long"
                        ));
                    }
                    // r42:arcus-hands — the hand standing up from the neck in the picture as
                    // the photographs' front views show it (left_hand_refs.md: the hand's
                    // line, wrist to the base knuckles, 70-89° to the fingerboard's edge; the
                    // table before this pass stood it at ~65°): up to the octave.
                    if semis <= 9.0 {
                        let line = cam.p(mid) - cam.p(lp.wrist);
                        let edge = cam.p(la.a) - cam.p(la.a + la.bw * 5.0);
                        let a = line.angle() - edge.angle();
                        let a = a.sin().atan2(a.cos()).to_degrees().abs();
                        let a = if a > 90.0 { 180.0 - a } else { a };
                        worst[7] = worst[7].max(HAND_NECK.0 - a).max(a - HAND_NECK.1);
                        if !(HAND_NECK.0..=HAND_NECK.1).contains(&a) {
                            bad.push(format!(
                                "{tag}: the hand {a:.0}° to the neck in the picture"
                            ));
                        }
                    }
                    let wrist = deg(lp.wrist - lp.elbow, mid - lp.wrist);
                    worst[5] = worst[5].max(wrist);
                    if wrist > WRIST {
                        bad.push(format!("{tag}: the wrist bent {wrist:.0}°"));
                    }
                    worst[6] = worst[6].max(upright);
                    // No finger bent sideways at its middle or last joint (the owner, sheet
                    // k6: "como con artritis"), its last joint no more bent than its middle.
                    for (fi, fj) in lp.fingers.iter().enumerate() {
                        let (d0, d1, d2) = (
                            (fj[1] - fj[0]).norm(),
                            (fj[2] - fj[1]).norm(),
                            (fj[3] - fj[2]).norm(),
                        );
                        let n = d0.cross(d1);
                        if n.len() > 0.15 {
                            let side = d2.dot(n.norm()).abs().asin().to_degrees();
                            if side > SIDEWAYS {
                                bad.push(format!("{tag}: finger {fi} bent {side:.0}° sideways"));
                            }
                        }
                        let (p, q) = (deg(d0, d1), deg(d1, d2));
                        if q > p + 5.0 {
                            bad.push(format!(
                                "{tag}: finger {fi}'s last joint {q:.0}° over its middle {p:.0}°"
                            ));
                        }
                    }
                    if ix <= la.hw + index_in
                        || tx >= -la.hw * 0.5
                        || !(top - 1.6..=top + 1.2).contains(&tz)
                    {
                        bad.push(format!(
                            "{tag}: the index's base at x {ix:.1}, the thumb's tip at x {tx:.1}, \
                             {:.1} cm from the top",
                            tz - top
                        ));
                    }
                }
            }
        }
    }
    println!(
        "worst: {:.0}° off upright, MCP ±{:.0}°, PIP ±{:.0}°, DIP ±{:.0}° from the ranges' middles; the back's least facing {:.2}, the wrist {:.0}°, the hand to the neck {:.0}° outside its range",
        worst[0], worst[1], worst[2], worst[3], -worst[4], worst[5], worst[7]
    );
    assert!(
        bad.is_empty(),
        "{} of the hand's rules broken: {bad:#?}",
        bad.len()
    );
}

/// r42:arcus-hands — how far the playing fingertip's pad may press into its string and the
/// fingerboard (cm), and anything else of the hand or the forearm into the neck, the
/// fingerboard or the body (the skin's own sampling slack).
const PAD_PRESS: f32 = 0.1;
const SKIN_SLACK: f32 = 0.03;

/// r42:arcus-hands — the owner, 2026-11-25: "a veces choca con el mástil" and, high up, "se
/// solapa la mano y antebrazo con el cuerpo del violín". For every figure, instrument,
/// string and a spread of positions up to the body, the skinned left hand and forearm are
/// kept out of the neck, the fingerboard and the body as the picture draws them
/// (`Solid`): only the playing fingertip's pad presses its string, by at most a millimetre.
#[test]
fn the_left_hand_keeps_out_of_the_neck_and_the_body() {
    use cantus_ui::scene::bowed_human::{self as hu, finger, side};
    let mut bad: Vec<String> = Vec::new();
    let mut worst = (0.0f32, 0.0f32);
    for (who, hold) in [(hu::Who::Woman, Hold::Woman), (hu::Who::Man, Hold::Man)] {
        for b in [Bowed::Violin, Bowed::Viola] {
            let h = Holder::of(hold);
            let rig = rig_for(b, h);
            let d = b.dims();
            let arm = left_arm(b, h).expect("a figure");
            let fig = hu::figure(who);
            let base = hu::rest_body(b, who);
            let solid = solid_of(b);
            let o = side(0);
            // The hand's and the forearm's own vertices (as the palm's gate takes them), and
            // the bone each one follows most.
            let mut used: Vec<(usize, u16)> = Vec::new();
            for i in 0..fig.pos.len() {
                let mine = fig.bw[i].iter().filter(|(b, _)| {
                    (*b as usize) >= o + hu::FORE1 && (*b as usize) <= o + finger(4, 2)
                });
                let share: f32 = mine.clone().map(|x| x.1).sum();
                if share > 0.6 {
                    let top = mine.fold((0u16, 0.0f32), |a, x| {
                        if x.1 > a.1 {
                            (x.0 as u16, x.1)
                        } else {
                            a
                        }
                    });
                    used.push((i, top.0));
                }
            }
            let span = hand::span(true);
            for string in 0..4 {
                for semis in [1.0f32, 3.0, 5.0, 7.0, 9.0, 12.0, 14.0, 17.0] {
                    let at = hand::target(1.0, semis, span);
                    let l = hand::Left {
                        rig: &rig,
                        d,
                        string,
                        semis,
                        at,
                        rock: 0.0,
                        side: rig.pose.ex,
                        span,
                    };
                    let lp = hand::left_pose(&arm, &l);
                    let j = lp.playing.expect("a stopped note");
                    let aim = hu::ArmAim {
                        elbow: lp.elbow,
                        wrist: lp.wrist,
                        hand: Some(hu::HandAim {
                            fingers: lp.fingers,
                            thumb: lp.thumb,
                            exact: lp.playing,
                            rots: lp.rots,
                        }),
                    };
                    let posed = hu::repose_arm(fig, &base.0, 0, &aim);
                    let pad = (o + finger(j + 1, 2)) as u16;
                    let mut deep = [(0.0f32, 0u16); 2];
                    for &(i, bone) in &used {
                        let p = posed.vertex(fig, i) - rig.pose.o;
                        let q = V3 {
                            x: p.dot(rig.pose.ex),
                            y: p.dot(rig.pose.ey),
                            z: p.dot(rig.pose.ez),
                        };
                        let s = solid.at(q);
                        let k = (bone != pad) as usize;
                        if -s > deep[k].0 {
                            deep[k] = (-s, bone);
                        }
                    }
                    worst.0 = worst.0.max(deep[0].0);
                    worst.1 = worst.1.max(deep[1].0);
                    let tag = format!("{who:?} {b:?} string {string} +{semis}");
                    if deep[0].0 > PAD_PRESS {
                        bad.push(format!(
                            "{tag}: the playing pad {:.1} mm in",
                            deep[0].0 * 10.0
                        ));
                    }
                    if deep[1].0 > SKIN_SLACK {
                        let name = match deep[1].1 as usize - o {
                            x if x == hu::FORE1 || x == hu::FORE2 => "the forearm".to_string(),
                            x if x == hu::HAND => "the palm".to_string(),
                            x => format!(
                                "finger {} bone {}",
                                (x - finger(0, 0)) / 3,
                                (x - finger(0, 0)) % 3
                            ),
                        };
                        bad.push(format!(
                            "{tag}: {name} {:.1} mm into the instrument",
                            deep[1].0 * 10.0
                        ));
                    }
                }
            }
        }
    }
    println!(
        "worst: the playing pad {:.1} mm in, anything else {:.1} mm",
        worst.0 * 10.0,
        worst.1 * 10.0
    );
    assert!(
        bad.is_empty(),
        "{} cases into the instrument: {bad:#?}",
        bad.len()
    );
}

/// Figure detail (the owner, 2026-11-24): a machine preference, High unless chosen, kept
/// through the file the app reads at start; Auto steps down while the frames run long and
/// back once they are short again (12 ms down, 7 ms back).
#[test]
fn figure_detail_is_a_machine_preference_with_high_by_default() {
    use cantus_ui::scene::figure::{frames_long, Quality};
    use cantus_ui::settings::Settings;
    let read = |j: &str| serde_json::from_str::<Settings>(j).unwrap().migrated();
    assert_eq!(read("{}").figure_quality, Quality::High);
    assert_eq!(
        read(r#"{"figure_quality": "light"}"#).figure_quality,
        Quality::Light
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{ "figure_quality": "auto" }"#).unwrap();
    let s = Settings::load(&path);
    assert_eq!(s.figure_quality, Quality::Auto);
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""figure_quality":"auto""#), "{json}");
    // Auto: High until the frames run long, Light until they are short again.
    assert!(!Quality::High.light(true) && Quality::Light.light(false));
    let mut long = false;
    for (avg, want) in [
        (5.0, false),
        (11.0, false),
        (13.0, true),
        (9.0, true),
        (6.5, false),
    ] {
        long = frames_long(avg, long);
        assert_eq!(Quality::Auto.light(long), want, "{avg} ms");
    }
}

/// r42:arcus-hand — the realistic figures' eyes (the owner, 2026-11-25: he liked them
/// closed, "concentrated or enjoying the music"): a machine preference, Closed unless
/// chosen, kept through the file the app reads. Closing them turns each upper lid down
/// over its eye (its edge at least 3 mm lower) and moves nothing into the eyeball (the
/// lids slide over it), and the picture is baked again when the choice changes (the eyes
/// are part of the holder it is cached by).
#[test]
fn the_figures_eyes_are_closed_unless_chosen_open() {
    use cantus_ui::scene::bowed_human::{self as hu, Who};
    use cantus_ui::scene::bowed_mesh::mesh_for;
    use cantus_ui::scene::figure::Eyes;
    use cantus_ui::settings::Settings;
    let read = |j: &str| serde_json::from_str::<Settings>(j).unwrap().migrated();
    assert_eq!(read("{}").figure_eyes, Eyes::Closed);
    assert_eq!(read(r#"{"figure_eyes": "open"}"#).figure_eyes, Eyes::Open);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{ "figure_eyes": "open" }"#).unwrap();
    let s = Settings::load(&path);
    assert_eq!(s.figure_eyes, Eyes::Open);
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""figure_eyes":"open""#), "{json}");
    // The holder kept in the file never carries the eyes (they are their own row).
    assert!(!serde_json::to_string(&s.arcus_holder)
        .unwrap()
        .contains("eyes"));
    for who in [Who::Woman, Who::Man] {
        let fig = hu::figure(who);
        let lids = hu::lids(who);
        assert!(lids.len() > 200, "{who:?}: {} lid vertices", lids.len());
        for side in ["eye.L", "eye.R"] {
            let c = fig.mark(side);
            let near: Vec<_> = lids
                .iter()
                .filter(|(i, _)| (fig.pos[*i as usize] - c).len() < 2.6)
                .collect();
            assert!(!near.is_empty(), "{who:?} {side}: no lid moves");
            let lowest = near.iter().map(|(_, d)| d.y).fold(0.0f32, f32::min);
            assert!(
                lowest < -0.3,
                "{who:?} {side}: the lid comes down {lowest:.2} cm"
            );
            for (i, d) in &near {
                let p = fig.pos[*i as usize];
                let (was, now) = ((p - c).len(), (p + *d - c).len());
                assert!(
                    now > was - 0.03,
                    "{who:?} {side}: vertex {i} moved {:.3} cm into the eye",
                    was - now
                );
            }
        }
    }
    // A change bakes the picture again: the face differs, nothing else.
    let h = |eyes| Holder {
        eyes,
        ..Holder::of(Hold::Woman)
    };
    let (closed, open) = (
        mesh_for(Bowed::Violin, h(Eyes::Closed)),
        mesh_for(Bowed::Violin, h(Eyes::Open)),
    );
    let moved: f32 = closed
        .tris
        .iter()
        .zip(open.tris.iter())
        .flat_map(|((_, a), (_, b))| a.pts.iter().zip(b.pts.iter()).map(|(p, q)| (*p - *q).len()))
        .sum();
    assert!(
        moved > 1.0,
        "the closed and the open pictures differ by {moved:.2} cm"
    );
}
