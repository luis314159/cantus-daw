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
const WRIST: f32 = 8.0;
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
    let mut worst = [0.0f32; 7];
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
                    worst[3] = worst[3].max((dip - 35.0).abs());
                    if !(35.0..=80.0).contains(&mcp)
                        || !(60.0..=110.0).contains(&pip)
                        // (The little finger, the shortest, may land with its last joint
                        // near straight, as a violist's often does on a long reach.)
                        || !((if j == 3 { 0.0 } else { 10.0 })..=60.0).contains(&dip)
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
                    if !(INDEX_OUT.0..=INDEX_OUT.1).contains(&(ix - ihw)) {
                        bad.push(format!("{tag}: the index's base {:.1} cm out from the E edge", ix - ihw));
                    }
                    for (f, fj) in lp.fingers.iter().enumerate() {
                        let (kx, ky, kz) = loc(fj[0]);
                        let (khw, ktop) = (board_w(d, ky) * 0.5, board_z(d, ky));
                        if kx <= khw + INDEX_OUT.0 || !(KNUCKLES_UP.0..=KNUCKLES_UP.1).contains(&(kz - ktop)) {
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
                        bad.push(format!("{tag}: the back of the hand faces the camera at {facing:.2}"));
                    }
                    // The back at a slant in the picture (BACK_WL).
                    let wl = (cam.p(lp.fingers[0][0]) - cam.p(lp.fingers[3][0])).length()
                        / (cam.p(mid) - cam.p(lp.wrist)).length().max(1e-3);
                    let hi = if at > 8.0 { BACK_WL_HIGH } else { BACK_WL.1 };
                    if !(BACK_WL.0..=hi).contains(&wl) {
                        bad.push(format!("{tag}: the back of the hand {wl:.2} wide over long"));
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
                        let (d0, d1, d2) =
                            ((fj[1] - fj[0]).norm(), (fj[2] - fj[1]).norm(), (fj[3] - fj[2]).norm());
                        let n = d0.cross(d1);
                        if n.len() > 0.15 {
                            let side = d2.dot(n.norm()).abs().asin().to_degrees();
                            if side > SIDEWAYS {
                                bad.push(format!("{tag}: finger {fi} bent {side:.0}° sideways"));
                            }
                        }
                        let (p, q) = (deg(d0, d1), deg(d1, d2));
                        if q > p + 5.0 {
                            bad.push(format!("{tag}: finger {fi}'s last joint {q:.0}° over its middle {p:.0}°"));
                        }
                    }
                    if ix <= la.hw + INDEX_OUT.0 || tx >= -la.hw * 0.5 || !(top - 1.6..=top + 1.2).contains(&tz) {
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
        "worst: {:.0}° off upright, MCP ±{:.0}°, PIP ±{:.0}°, DIP ±{:.0}° from the ranges' middles; the back's least facing {:.2}, the wrist {:.0}°",
        worst[0], worst[1], worst[2], worst[3], -worst[4], worst[5]
    );
    assert!(
        bad.is_empty(),
        "{} of the hand's rules broken: {bad:#?}",
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
    assert!(!serde_json::to_string(&s.arcus_holder).unwrap().contains("eyes"));
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
            assert!(lowest < -0.3, "{who:?} {side}: the lid comes down {lowest:.2} cm");
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
    assert!(moved > 1.0, "the closed and the open pictures differ by {moved:.2} cm");
}
