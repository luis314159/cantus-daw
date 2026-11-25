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

/// How far from upright the playing finger's last phalanx may lean (degrees): nearly
/// upright on the strings near its knuckles, leaning more as it reaches across the
/// fingerboard to the G string.
const UPRIGHT: f32 = 45.0;

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
    let mut worst = [0.0f32; 4];
    for (who, hold) in [(hu::Who::Woman, Hold::Woman), (hu::Who::Man, Hold::Man)] {
        for b in [Bowed::Violin, Bowed::Viola] {
            let h = Holder::of(hold);
            let rig = rig_for(b, h);
            let d = b.dims();
            let arm = left_arm(b, h).expect("a figure");
            let fig = hu::figure(who);
            let base = hu::rest_body(b, who);
            let mut cam = bowed_draw::camera(b, h, Room::Studio);
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
                        || !(10.0..=60.0).contains(&dip)
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
                    if ix <= la.hw || tx >= -la.hw * 0.5 || !(top - 1.6..=top + 1.2).contains(&tz) {
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
        "worst: {:.0}° off upright, MCP ±{:.0}°, PIP ±{:.0}°, DIP ±{:.0}° from the ranges' middles",
        worst[0], worst[1], worst[2], worst[3]
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
