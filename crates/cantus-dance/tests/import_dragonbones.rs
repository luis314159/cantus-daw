//! DragonBones (ADR 0089 §6): our generated skeleton loads with its atlas, maps every move,
//! deforms its mesh, switches displays, reorders slots, and IK reaches its target.

use cantus_dance::import::{self, dragonbones, samples, Limits};
use cantus_dance::render::{self, Background, Stage};
use cantus_dance::{pose_at, DanceFrame, Move};
use serde_json::{json, Value};

fn written() -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    samples::write(d.path(), &samples::dragonbones("hero")).unwrap();
    let p = d.path().join("hero_ske.json");
    (d, p)
}

fn with_move(m: Move, beats: f64) -> DanceFrame {
    let mut f = DanceFrame {
        beats,
        seconds: beats * 0.5,
        playing: true,
        bar: (beats / 4.0).floor() as i32,
        bar_start_beats: (beats / 4.0).floor() * 4.0,
        ..DanceFrame::default()
    };
    let i = cantus_core::inst::dancer::KEYS
        .iter()
        .position(|k| *k == "move")
        .unwrap();
    f.values[i] = (m.index() + 1) as f32;
    f
}

#[test]
fn the_skeleton_loads_with_its_atlas_and_maps_every_move() {
    let (_d, p) = written();
    let found = import::inspect(&p).unwrap();
    assert_eq!(found.source.format(), "DragonBones");
    assert_eq!(found.name, "hero");
    assert_eq!(
        found.files,
        vec!["hero_tex.json".to_string(), "hero_tex.png".to_string()]
    );
    assert!(found.notes.is_empty(), "{:?}", found.notes);
    let s = import::guessed(&found);
    for m in Move::ALL {
        assert_eq!(s.mapping.moves[m.index()], Some(m.index()), "{m:?}");
    }
}

#[test]
fn it_draws_each_move_and_changes_on_the_beat() {
    let (_d, p) = written();
    let ch = import::import(&p).unwrap();
    let stage = Stage::new(300, 300, Background::Transparent);
    let draw = |f: &DanceFrame| {
        render::render(&stage, &[(ch.as_ref(), pose_at(f, ch.as_ref(), 0.0))]).unwrap()
    };
    for m in Move::ALL {
        let a = draw(&with_move(m, 4.0));
        let b = draw(&with_move(m, 4.3));
        let drawn = a.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(drawn > 3000, "{m:?} drawn ({drawn})");
        if !matches!(m, Move::Idle | Move::Freeze | Move::Point) {
            assert!(a.data() != b.data(), "{m:?} moves within its beat");
        }
    }
}

#[test]
fn the_wink_the_scarf_and_the_z_order_play() {
    let (_d, p) = written();
    let ch = import::import(&p).unwrap();
    let stage = Stage::new(300, 300, Background::Transparent);
    let draw = |f: &DanceFrame| {
        render::render(&stage, &[(ch.as_ref(), pose_at(f, ch.as_ref(), 0.0))]).unwrap()
    };
    // Freeze: the head's second display (a wink) in the second half.
    assert!(
        draw(&with_move(Move::Freeze, 8.1)).data() != draw(&with_move(Move::Freeze, 8.6)).data()
    );
    // Wave: the scarf's mesh is deformed (red pixels move).
    let red = |pm: &tiny_skia::Pixmap| -> Vec<usize> {
        pm.pixels()
            .iter()
            .enumerate()
            .filter(|(_, c)| c.alpha() > 200 && c.red() > 200 && c.green() < 110)
            .map(|(i, _)| i)
            .collect()
    };
    let w1 = red(&draw(&with_move(Move::Wave, 12.0)));
    let w2 = red(&draw(&with_move(Move::Wave, 12.45)));
    assert!(!w1.is_empty() && w1 != w2, "the scarf wobbles");
}

#[test]
fn two_bone_ik_reaches_its_target() {
    // A shoulder, an elbow (length 40) and a target at a reachable point.
    let ske = json!({"name": "ik", "version": "5.5", "armature": [{"name": "a", "frameRate": 24,
        "bone": [
            {"name": "root"},
            {"name": "upper", "parent": "root", "length": 40, "transform": {"x": 0, "y": 0}},
            {"name": "lower", "parent": "upper", "length": 40, "transform": {"x": 40, "y": 0}},
            {"name": "target", "parent": "root", "transform": {"x": 30, "y": 50}}],
        "slot": [{"name": "s", "parent": "lower"}],
        "skin": [{"name": "", "slot": [{"name": "s", "display": [{"name": "t", "transform": {"x": 40}}]}]}],
        "ik": [{"name": "k", "bone": "lower", "target": "target", "chain": 1, "bendPositive": true, "weight": 1}],
        "animation": [{"name": "idle", "duration": 1}]}]});
    let tex = json!({"imagePath": "ik_tex.png", "SubTexture": [{"name": "t", "x": 0, "y": 0, "width": 4, "height": 4}]});
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("ik_ske.json"),
        serde_json::to_vec(&ske).unwrap(),
    )
    .unwrap();
    std::fs::write(
        d.path().join("ik_tex.json"),
        serde_json::to_vec(&tex).unwrap(),
    )
    .unwrap();
    let mut img = tiny_skia::Pixmap::new(4, 4).unwrap();
    img.fill(tiny_skia::Color::BLACK);
    std::fs::write(d.path().join("ik_tex.png"), img.encode_png().unwrap()).unwrap();
    let found = import::inspect(&d.path().join("ik_ske.json")).unwrap();
    // The image at the lower bone's tip sits on the target (30, 50).
    let b = found.source.bounds();
    let (cx, cy) = ((b.left + b.right) * 0.5, (b.top + b.bottom) * 0.5);
    assert!(
        (cx - 30.0).abs() < 1.0 && (cy - 50.0).abs() < 1.0,
        "the tip at ({cx}, {cy})"
    );
}

#[test]
fn hostile_skeletons_are_refused_in_words() {
    let (_d, p) = written();
    let folder = p.parent().unwrap();
    let base: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let read = |v: &Value| dragonbones::read(v, folder, &Limits::DEFAULT);
    // A loop of parents.
    let mut v = base.clone();
    v["armature"][0]["bone"][0]["parent"] = json!("leg_l");
    assert!(read(&v).is_err());
    // Too many bones.
    let mut v = base.clone();
    let bones: Vec<Value> = (0..1001)
        .map(|i| json!({"name": format!("b{i}")}))
        .collect();
    v["armature"][0]["bone"] = Value::Array(bones);
    assert!(matches!(read(&v), Err(import::ImportError::TooLarge(_))));
    // An armature that holds itself.
    let mut v = base.clone();
    v["armature"][0]["skin"][0]["slot"][0]["display"] =
        json!([{"type": "armature", "name": "figure"}]);
    assert!(read(&v).is_err());
    // An old version.
    let mut v = base.clone();
    v["version"] = json!("4.5");
    assert!(read(&v).is_err());
    // A texture outside its image.
    let d2 = tempfile::tempdir().unwrap();
    let mut files = samples::dragonbones("hero");
    let mut tex: Value = serde_json::from_slice(&files[1].1).unwrap();
    tex["SubTexture"][0]["x"] = json!(250);
    files[1].1 = serde_json::to_vec(&tex).unwrap();
    samples::write(d2.path(), &files).unwrap();
    assert!(import::inspect(&d2.path().join("hero_ske.json")).is_err());
}

/// Review 1, item 2: nested armatures are counted as instances before anything is walked —
/// five armatures of 200 slots each showing the next would be 200^5 drawn.
#[test]
fn nested_armatures_cannot_multiply_without_limit() {
    let arm = |k: usize| {
        let slots: Vec<Value> = (0..200)
            .map(|i| json!({"name": format!("s{i}"), "parent": "root"}))
            .collect();
        let display = if k < 4 {
            json!([{"type": "armature", "name": format!("a{}", k + 1)}])
        } else {
            json!([{"name": "t"}])
        };
        let skin: Vec<Value> = (0..200)
            .map(|i| json!({"name": format!("s{i}"), "display": display.clone()}))
            .collect();
        json!({"name": format!("a{k}"), "frameRate": 24, "bone": [{"name": "root"}], "slot": slots,
               "skin": [{"name": "", "slot": skin}], "animation": [{"name": "idle", "duration": 1}]})
    };
    let ske =
        json!({"name": "bomb", "version": "5.5", "armature": (0..5).map(arm).collect::<Vec<_>>()});
    let tex = json!({"imagePath": "bomb_tex.png", "SubTexture": [{"name": "t", "x": 0, "y": 0, "width": 4, "height": 4}]});
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("bomb_ske.json"),
        serde_json::to_vec(&ske).unwrap(),
    )
    .unwrap();
    std::fs::write(
        d.path().join("bomb_tex.json"),
        serde_json::to_vec(&tex).unwrap(),
    )
    .unwrap();
    let img = tiny_skia::Pixmap::new(4, 4).unwrap();
    std::fs::write(d.path().join("bomb_tex.png"), img.encode_png().unwrap()).unwrap();
    let t0 = std::time::Instant::now();
    let r = import::inspect(&d.path().join("bomb_ske.json"));
    assert!(matches!(r, Err(import::ImportError::TooLarge(_))), "{r:?}");
    assert!(t0.elapsed().as_secs_f32() < 2.0, "refused before walking");
}

/// A skeleton of `arms` armatures (the first is the main one) beside a 4×4 atlas, read
/// with `limits`.
fn skeleton_with(arms: Vec<Value>, limits: &Limits) -> Result<import::Found, import::ImportError> {
    let ske = json!({"name": "w", "version": "5.5", "armature": arms});
    let tex = json!({"imagePath": "w_tex.png", "SubTexture": [{"name": "t", "x": 0, "y": 0, "width": 4, "height": 4}]});
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("w_ske.json"),
        serde_json::to_vec(&ske).unwrap(),
    )
    .unwrap();
    std::fs::write(
        d.path().join("w_tex.json"),
        serde_json::to_vec(&tex).unwrap(),
    )
    .unwrap();
    let mut img = tiny_skia::Pixmap::new(4, 4).unwrap();
    img.fill(tiny_skia::Color::BLACK);
    std::fs::write(d.path().join("w_tex.png"), img.encode_png().unwrap()).unwrap();
    import::inspect_with(&d.path().join("w_ske.json"), limits)
}

/// An armature named `name` whose slots each show `display`.
fn arm_of(name: &str, slots: usize, display: Value) -> Value {
    let slot_list: Vec<Value> = (0..slots)
        .map(|i| json!({"name": format!("s{i}"), "parent": "root"}))
        .collect();
    let skin: Vec<Value> = (0..slots)
        .map(|i| json!({"name": format!("s{i}"), "display": [display.clone()]}))
        .collect();
    json!({"name": name, "frameRate": 24, "bone": [{"name": "root"}], "slot": slot_list,
           "skin": [{"name": "", "slot": skin}], "animation": [{"name": "idle", "duration": 1}]})
}

/// A mesh of an n×n grid of the texture.
fn mesh(n: usize) -> Value {
    let (mut v, mut uv, mut tri) = (Vec::new(), Vec::new(), Vec::new());
    for j in 0..=n {
        for i in 0..=n {
            v.extend([json!(i as f32 * 4.0), json!(j as f32 * 4.0)]);
            uv.extend([json!(i as f32 / n as f32), json!(j as f32 / n as f32)]);
        }
    }
    for j in 0..n {
        for i in 0..n {
            let k = j * (n + 1) + i;
            tri.extend([k, k + 1, k + n + 2, k, k + n + 2, k + n + 1].map(|x| json!(x)));
        }
    }
    json!({"type": "mesh", "name": "t", "vertices": v, "uvs": uv, "triangles": tri})
}

/// Review 2, item 1: what each nested armature draws is weighed — slots, corners, a mesh's
/// vertices and triangles, every instance its own — against the frame's work, before
/// anything is walked.
#[test]
fn nested_armatures_are_weighed_by_what_they_draw() {
    let arms = vec![
        arm_of("main", 50, json!({"type": "armature", "name": "child"})),
        arm_of("child", 1, mesh(10)),
    ];
    let small = Limits {
        frame_work: 10_000,
        ..Limits::DEFAULT
    };
    let t0 = std::time::Instant::now();
    let r = skeleton_with(arms.clone(), &small);
    assert!(matches!(r, Err(import::ImportError::TooLarge(_))), "{r:?}");
    assert!(t0.elapsed().as_secs_f32() < 2.0);
    // Within the frame's work it loads.
    assert!(skeleton_with(arms, &Limits::DEFAULT).is_ok());
}

/// Review 2, item 1: one depth limit, wherever an armature is reached from — an armature
/// met first near the top and then again five deep is refused.
#[test]
fn the_depth_limit_holds_through_a_shared_armature() {
    let show = |n: &str| json!({"type": "armature", "name": n});
    let mut main = arm_of("main", 2, show("a5"));
    main["skin"][0]["slot"][1]["display"] = json!([show("a1")]);
    let arms = vec![
        main,
        arm_of("a1", 1, show("a2")),
        arm_of("a2", 1, show("a3")),
        arm_of("a3", 1, show("a4")),
        arm_of("a4", 1, show("a5")),
        arm_of("a5", 1, json!({"name": "t"})),
    ];
    let r = skeleton_with(arms, &Limits::DEFAULT);
    assert!(matches!(r, Err(import::ImportError::TooLarge(_))), "{r:?}");
    // Four deep is drawn whole: the deepest armature's image is in the box.
    let show4 = vec![
        arm_of("main", 1, show("a1")),
        arm_of("a1", 1, show("a2")),
        arm_of("a2", 1, show("a3")),
        arm_of("a3", 1, show("a4")),
        arm_of("a4", 1, json!({"name": "t", "transform": {"x": 50}})),
    ];
    let found = skeleton_with(show4, &Limits::DEFAULT).unwrap();
    assert!(
        found.source.bounds().right > 40.0,
        "{:?}",
        found.source.bounds()
    );
}

/// Review 3, item 4: posing is weighed too — every bone of every instance, solved again for
/// each IK constraint.
#[test]
fn bones_and_ik_are_weighed_per_instance() {
    let mut child = arm_of("child", 1, json!({"name": "t"}));
    let bones: Vec<Value> = std::iter::once(json!({"name": "root"}))
        .chain((1..200).map(|i| json!({"name": format!("b{i}"), "parent": "root", "length": 10})))
        .collect();
    child["bone"] = Value::Array(bones);
    child["ik"] = json!([{"name": "k", "bone": "b2", "target": "b1", "chain": 0}]);
    let arms = vec![
        arm_of("main", 40, json!({"type": "armature", "name": "child"})),
        child,
    ];
    let small = Limits {
        frame_work: 5_000,
        ..Limits::DEFAULT
    };
    let r = skeleton_with(arms.clone(), &small);
    assert!(matches!(r, Err(import::ImportError::TooLarge(_))), "{r:?}");
    assert!(skeleton_with(arms, &Limits::DEFAULT).is_ok());
}

/// Review 3, item 4: the box's sampling never does more than its bound in all.
#[test]
fn the_box_is_sampled_within_its_bound() {
    use cantus_dance::import::dragonbones::{measure_plan, MEASURE_WORK};
    for (work, anims) in [
        (1u64, 1usize),
        (100, 256),
        (2_000_000, 256),
        (500_000, 3),
        (40_000_000, 10),
    ] {
        let (sampled, per) = measure_plan(work, anims);
        assert!(sampled >= 1 && sampled <= anims && (1..=17).contains(&per));
        let total = sampled as u64 * per as u64 * work;
        assert!(total <= MEASURE_WORK.max(work), "{work} × {anims}: {total}");
    }
    assert_eq!(measure_plan(1, 4), (4, 17), "a light file is sampled whole");
}
