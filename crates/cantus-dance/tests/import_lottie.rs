//! Lottie (ADR 0089 §2): our player reads our generated figure and its package, maps its
//! markers onto our moves, draws masks, mattes, repeaters, trims and gradients, ignores
//! expressions (and says so), and survives what made velato panic.

use cantus_dance::import::lottie::{self, Lottie};
use cantus_dance::import::{self, samples, Limits};
use cantus_dance::render::{self, Background, Stage};
use cantus_dance::{pose_at, DanceFrame, Licence, Move};
use serde_json::{json, Value};
use std::path::Path;
use tiny_skia::{Pixmap, Transform};

fn playing_at(beats: f64) -> DanceFrame {
    DanceFrame {
        beats,
        seconds: beats * 0.5,
        playing: true,
        bar: (beats / 4.0).floor() as i32,
        bar_start_beats: (beats / 4.0).floor() * 4.0,
        ..DanceFrame::default()
    }
}

fn read(v: &Value) -> Result<import::Found, import::ImportError> {
    lottie::read(v, Path::new("."), &Limits::DEFAULT)
}

fn frame_of(v: &Value, f: f32) -> Pixmap {
    let found = read(v).unwrap();
    let mut pm = Pixmap::new(256, 256).unwrap();
    // Drawn through the Source at half size.
    let doc = lottie::model::read(
        v,
        lottie::model::Files::Folder(Path::new(".")),
        &Limits::DEFAULT,
    )
    .unwrap();
    let l = Lottie::new(doc);
    l.draw_frame(f, Transform::from_scale(0.5, 0.5), 0.0, &mut pm);
    drop(found);
    pm
}

fn opaque(pm: &Pixmap) -> usize {
    pm.pixels().iter().filter(|p| p.alpha() > 0).count()
}

/// A one-layer document of `shapes` (128×128, 10 frames).
fn doc_of(shapes: Value) -> Value {
    json!({"v": "5.7.0", "fr": 10, "ip": 0, "op": 10, "w": 512, "h": 512, "layers": [
        {"ty": 4, "ind": 1, "ip": 0, "op": 10, "st": 0,
         "ks": {"p": {"a": 0, "k": [0, 0]}, "a": {"a": 0, "k": [0, 0]}, "s": {"a": 0, "k": [100, 100]}, "r": {"a": 0, "k": 0}, "o": {"a": 0, "k": 100}},
         "shapes": shapes}]})
}

fn st(v: Value) -> Value {
    json!({"a": 0, "k": v})
}

#[test]
fn our_figure_maps_its_markers_and_states_its_licence() {
    let v = samples::lottie_doc("figure");
    let found = read(&v).unwrap();
    assert_eq!(found.source.format(), "Lottie");
    assert_eq!(found.source.clips().len(), 16);
    assert_eq!(found.author, "Cantus");
    assert_eq!(
        found.licence,
        Licence::Stated {
            text: "CC0-1.0".into(),
            source: "meta".into()
        }
    );
    let s = import::guessed(&found);
    for m in Move::ALL {
        assert_eq!(s.mapping.moves[m.index()], Some(m.index()), "{m:?}");
        assert_eq!(s.mapping.beats_of(m), 1.0, "half a second is one beat");
    }
    assert!(found.notes.is_empty(), "{:?}", found.notes);
}

#[test]
fn it_dances_the_same_at_any_frame_rate_and_moves_on_the_grid() {
    let d = tempfile::tempdir().unwrap();
    samples::write(d.path(), &samples::lottie("figure")).unwrap();
    let ch = import::import(&d.path().join("figure.json")).unwrap();
    let stage = Stage::new(300, 300, Background::Transparent);
    let mut seen = Vec::new();
    for i in 0..16 {
        let f = playing_at(f64::from(i) * 0.25);
        let a = render::render(&stage, &[(ch.as_ref(), pose_at(&f, ch.as_ref(), 3.0))]).unwrap();
        let b = render::render(&stage, &[(ch.as_ref(), pose_at(&f, ch.as_ref(), 99.0))]).unwrap();
        assert_eq!(
            a.data(),
            b.data(),
            "the wall clock does not matter while playing"
        );
        assert!(opaque(&a) > 2000, "drawn");
        seen.push(a.data().to_vec());
    }
    seen.dedup();
    assert!(
        seen.len() > 8,
        "it moves: {} different frames of 16",
        seen.len()
    );
}

#[test]
fn a_package_brings_its_image_author_and_licence() {
    let d = tempfile::tempdir().unwrap();
    samples::write(d.path(), &samples::dotlottie("pkg")).unwrap();
    let found = import::inspect(&d.path().join("pkg.lottie")).unwrap();
    assert_eq!(found.source.format(), "Lottie");
    assert_eq!(found.author, "Cantus");
    assert_eq!(
        found.licence,
        Licence::Stated {
            text: "CC0-1.0".into(),
            source: "meta".into()
        }
    );
    assert!(
        found
            .notes
            .iter()
            .all(|n| n.key != "dance_import.note.image_unreadable"),
        "{:?}",
        found.notes
    );
    let ch = import::import(&d.path().join("pkg.lottie")).unwrap();
    let stage = Stage::new(300, 300, Background::Transparent);
    let img = render::render(
        &stage,
        &[(ch.as_ref(), pose_at(&playing_at(1.0), ch.as_ref(), 0.0))],
    )
    .unwrap();
    let sun = img
        .pixels()
        .iter()
        .filter(|p| p.alpha() == 255 && p.red() > 240 && p.green() > 190 && p.blue() < 90)
        .count();
    assert!(sun > 40, "the package's image is drawn ({sun} sun pixels)");
}

#[test]
fn expressions_are_never_run_and_are_said() {
    let mut v = samples::lottie_doc("figure");
    v["layers"][2]["ks"]["r"]["x"] = json!("while(true){}; wiggle(10, 50)");
    let found = read(&v).unwrap();
    let n = found
        .notes
        .iter()
        .find(|n| n.key == "dance_import.note.expressions")
        .unwrap();
    assert_eq!(n.arg, "1");
    let plain = samples::lottie_doc("figure");
    assert_eq!(
        frame_of(&v, 30.0).data(),
        frame_of(&plain, 30.0).data(),
        "the keyframes play"
    );
}

#[test]
fn what_made_velato_panic_plays_here() {
    let base = samples::lottie_doc("figure");
    let mut cases = Vec::new();
    let mut v = base.clone();
    v["layers"][2]["ks"].as_object_mut().unwrap().remove("r");
    cases.push(("a transform without rotation", v));
    let mut v = base.clone();
    let ks = v["layers"][2]["ks"].as_object_mut().unwrap();
    ks.remove("r");
    ks.insert("rx".into(), st(json!(10)));
    ks.insert("ry".into(), st(json!(20)));
    ks.insert("rz".into(), st(json!(15)));
    v["layers"][2]["ddd"] = json!(1);
    cases.push(("a 3D layer", v));
    let mut v = base.clone();
    v["layers"][2]["bm"] = json!(16);
    cases.push(("blend Add", v));
    let mut v = base.clone();
    let tr = v["layers"][2]["shapes"][0]["it"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    tr["p"] = json!({"s": true, "x": st(json!(3)), "y": st(json!(4))});
    cases.push(("a group's split position", v));
    for (what, v) in cases {
        let found = read(&v).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert!(opaque(&frame_of(&v, 20.0)) > 1000, "{what} draws");
        if what == "a 3D layer" {
            assert!(found
                .notes
                .iter()
                .any(|n| n.key == "dance_import.note.three_d"));
        }
    }
}

#[test]
fn precomps_that_contain_themselves_or_run_away_are_refused() {
    let mut v = doc_of(json!([]));
    v["assets"] = json!([{"id": "a", "layers": [{"ty": 0, "refId": "a", "ind": 1, "ip": 0, "op": 10, "st": 0, "ks": {}}]}]);
    v["layers"] = json!([{"ty": 0, "refId": "a", "ind": 1, "ip": 0, "op": 10, "st": 0, "ks": {}, "w": 10, "h": 10}]);
    assert!(read(&v).is_err());
    // Twenty levels, each using the next: past the depth.
    let mut assets = Vec::new();
    for k in 0..20 {
        let next = if k < 19 {
            json!([{"ty": 0, "refId": format!("p{}", k + 1), "ip": 0, "op": 10, "st": 0, "ks": {}}])
        } else {
            json!([])
        };
        assets.push(json!({"id": format!("p{k}"), "layers": next}));
    }
    let mut v = doc_of(json!([]));
    v["assets"] = Value::Array(assets.clone());
    v["layers"] = json!([{"ty": 0, "refId": "p0", "ip": 0, "op": 10, "st": 0, "ks": {}}]);
    assert!(matches!(read(&v), Err(import::ImportError::TooLarge(_))));
    // Eight levels of ten uses each: 10^8 layers in a frame.
    let mut assets = Vec::new();
    for k in 0..8 {
        let uses: Vec<Value> = (0..10)
            .map(|_| if k < 7 { json!({"ty": 0, "refId": format!("q{}", k + 1), "ip": 0, "op": 10, "st": 0, "ks": {}}) } else { json!({"ty": 3, "ip": 0, "op": 10, "st": 0, "ks": {}}) })
            .collect();
        assets.push(json!({"id": format!("q{k}"), "layers": uses}));
    }
    let mut v = doc_of(json!([]));
    v["assets"] = Value::Array(assets);
    v["layers"] = json!([{"ty": 0, "refId": "q0", "ip": 0, "op": 10, "st": 0, "ks": {}}]);
    assert!(matches!(read(&v), Err(import::ImportError::TooLarge(_))));
}

#[test]
fn masks_and_mattes_cut_what_is_drawn() {
    let square = json!([{"ty": "gr", "it": [
        {"ty": "rc", "p": st(json!([256, 256])), "s": st(json!([400, 400])), "r": st(json!(0))},
        {"ty": "fl", "c": st(json!([1, 0, 0, 1])), "o": st(json!(100))},
        {"ty": "tr", "p": st(json!([0, 0])), "a": st(json!([0, 0])), "s": st(json!([100, 100])), "r": st(json!(0)), "o": st(json!(100))}]}]);
    let full = opaque(&frame_of(&doc_of(square.clone()), 0.0));
    let circle = |c: f32| {
        json!({"c": true, "v": [[256.0, 256.0 - c], [256.0 + c, 256.0], [256.0, 256.0 + c], [256.0 - c, 256.0]],
        "i": [[-55, 0], [0, -55], [55, 0], [0, 55]], "o": [[55, 0], [0, 55], [-55, 0], [0, -55]]})
    };
    let mut masked = doc_of(square.clone());
    masked["layers"][0]["masksProperties"] =
        json!([{"mode": "a", "inv": false, "pt": st(circle(100.0)), "o": st(json!(100))}]);
    let m = opaque(&frame_of(&masked, 0.0));
    assert!(
        m < full / 3 && m > full / 8,
        "a mask keeps the circle: {m} of {full}"
    );
    masked["layers"][0]["masksProperties"][0]["mode"] = json!("s");
    let s = opaque(&frame_of(&masked, 0.0));
    assert!(
        s > full * 2 / 3 && s < full,
        "a subtract mask cuts the hole: {s} of {full}"
    );
    // An alpha matte: the circle layer above, the square matted by it.
    let mut matted = doc_of(square);
    let matte_layer = json!({"ty": 4, "ind": 9, "td": 1, "ip": 0, "op": 10, "st": 0,
        "ks": {"o": st(json!(100))},
        "shapes": [{"ty": "gr", "it": [{"ty": "el", "p": st(json!([256, 256])), "s": st(json!([200, 200]))},
            {"ty": "fl", "c": st(json!([1, 1, 1, 1])), "o": st(json!(100))}, {"ty": "tr", "o": st(json!(100))}]}]});
    matted["layers"][0]["tt"] = json!(1);
    matted["layers"]
        .as_array_mut()
        .unwrap()
        .insert(0, matte_layer);
    let mt = opaque(&frame_of(&matted, 0.0));
    assert!(
        (mt as f32 - m as f32).abs() < m as f32 * 0.1,
        "the matte cuts like the mask: {mt} against {m}"
    );
}

#[test]
fn repeaters_trims_and_gradients_draw() {
    let dot = |extra: Vec<Value>| {
        let mut it = vec![
            json!({"ty": "el", "p": st(json!([60, 256])), "s": st(json!([40, 40]))}),
            json!({"ty": "fl", "c": st(json!([0, 0, 1, 1])), "o": st(json!(100))}),
        ];
        it.extend(extra);
        it.push(json!({"ty": "tr", "o": st(json!(100))}));
        json!([{"ty": "gr", "it": it}])
    };
    let one = opaque(&frame_of(&doc_of(dot(vec![])), 0.0));
    let rep = json!({"ty": "rp", "c": st(json!(5)), "o": st(json!(0)), "m": 1,
        "tr": {"p": st(json!([80, 0])), "a": st(json!([0, 0])), "s": st(json!([100, 100])), "r": st(json!(0)), "so": st(json!(100)), "eo": st(json!(100))}});
    let five = opaque(&frame_of(&doc_of(dot(vec![rep])), 0.0));
    assert!(
        (five as f32 / one as f32 - 5.0).abs() < 0.2,
        "five copies: {five} against {one}"
    );
    // A trimmed stroke: half the line, half the pixels.
    let line = |e: f32| {
        json!([{"ty": "gr", "it": [
        {"ty": "sh", "ks": st(json!({"c": false, "v": [[50, 256], [450, 256]], "i": [[0, 0], [0, 0]], "o": [[0, 0], [0, 0]]}))},
        {"ty": "tm", "s": st(json!(0)), "e": st(json!(e)), "o": st(json!(0)), "m": 1},
        {"ty": "st", "c": st(json!([0, 0, 0, 1])), "o": st(json!(100)), "w": st(json!(10)), "lc": 1, "lj": 1},
        {"ty": "tr", "o": st(json!(100))}]}])
    };
    let whole = opaque(&frame_of(&doc_of(line(100.0)), 0.0));
    let half = opaque(&frame_of(&doc_of(line(50.0)), 0.0));
    assert!(
        (half as f32 / whole as f32 - 0.5).abs() < 0.03,
        "{half} of {whole}"
    );
    // A vertical gradient: red at the top, blue at the bottom.
    let grad = json!([{"ty": "gr", "it": [
        {"ty": "rc", "p": st(json!([256, 256])), "s": st(json!([400, 400])), "r": st(json!(0))},
        {"ty": "gf", "o": st(json!(100)), "r": 1, "t": 1, "s": st(json!([0, 56])), "e": st(json!([0, 456])),
         "g": {"p": 2, "k": st(json!([0, 1, 0, 0, 1, 0, 0, 1]))}},
        {"ty": "tr", "o": st(json!(100))}]}]);
    let g = frame_of(&doc_of(grad), 0.0);
    let top = g.pixel(128, 40).unwrap();
    let bottom = g.pixel(128, 215).unwrap();
    assert!(top.red() > 200 && top.blue() < 50, "{top:?}");
    assert!(bottom.blue() > 200 && bottom.red() < 50, "{bottom:?}");
}

#[test]
fn a_heavy_file_is_cached_and_looks_the_same() {
    // Our figure with forty more filled groups: past the heavy line.
    let mut v = samples::lottie_doc("busy");
    let shapes = v["layers"][2]["shapes"].as_array_mut().unwrap();
    for k in 0..130 {
        shapes.push(json!({"ty": "gr", "it": [
            {"ty": "el", "p": st(json!([(k % 13) as f32 * 4.0 - 24.0, (k / 13) as f32 * 4.0 - 20.0])), "s": st(json!([3, 3]))},
            {"ty": "fl", "c": st(json!([0.2, 0.8, 0.4, 1])), "o": st(json!(100))},
            {"ty": "tr", "o": st(json!(100))}]}));
    }
    let found = read(&v).unwrap();
    assert!(found.source.heavy());
    let settings = import::guessed(&found);
    let heavy = import::character(found, "file:busy".into(), settings);
    let stage = Stage::new(400, 400, Background::Transparent);
    let f = playing_at(3.0);
    let a = render::render(&stage, &[(&heavy, pose_at(&f, &heavy, 0.0))]).unwrap();
    let b = render::render(&stage, &[(&heavy, pose_at(&f, &heavy, 0.0))]).unwrap();
    assert_eq!(a.data(), b.data(), "the cache gives the same pixels again");
    assert!(opaque(&a) > 2000);
    // Review 2, item 6: a hue sweep keeps one copy of the frame, turned as it is placed.
    let kept = heavy.cached_frames();
    let mut pose = pose_at(&f, &heavy, 0.0);
    let mut turned = Vec::new();
    for hue in [30.0f32, 60.0, 90.0, 120.0, 150.0, 180.0] {
        pose.hue = hue;
        turned.push(render::render(&stage, &[(&heavy, pose)]).unwrap());
    }
    assert_eq!(heavy.cached_frames(), kept, "no frame per hue");
    assert!(turned[3].data() != a.data(), "the hue turns it");
    let alpha = |p: &Pixmap| p.pixels().iter().map(|c| u64::from(c.alpha())).sum::<u64>();
    assert_eq!(alpha(&turned[3]), alpha(&a), "and keeps its shape");
}

#[test]
fn text_layers_draw_from_the_glyphs_the_file_carries() {
    let glyph = json!({"ch": "I", "size": 100, "style": "Regular", "w": 30, "fFamily": "Test",
        "data": {"shapes": [{"ty": "gr", "it": [
            {"ty": "sh", "ks": st(json!({"c": true, "v": [[0, 0], [20, 0], [20, -70], [0, -70]], "i": [[0,0],[0,0],[0,0],[0,0]], "o": [[0,0],[0,0],[0,0],[0,0]]}))},
            {"ty": "tr", "o": st(json!(100))}]}]}});
    let mut v = doc_of(json!([]));
    v["layers"][0] = json!({"ty": 5, "ind": 1, "ip": 0, "op": 10, "st": 0,
        "ks": {"p": st(json!([100, 300])), "o": st(json!(100))},
        "t": {"d": {"k": [{"t": 0, "s": {"t": "III", "s": 100, "f": "Test", "fc": [0, 0, 0], "j": 0, "tr": 0, "lh": 120}}]}}});
    let without = read(&v).unwrap();
    assert!(without
        .notes
        .iter()
        .any(|n| n.key == "dance_import.note.text_without_glyphs"));
    assert_eq!(opaque(&frame_of(&v, 0.0)), 0);
    v["chars"] = json!([glyph]);
    let drawn = opaque(&frame_of(&v, 0.0));
    assert!(drawn > 3 * 300, "three glyphs drawn ({drawn} pixels)");
}

/// The Lottie specification's examples (downloaded for measurement only, never committed):
/// `CANTUS_LOTTIE_CORPUS=<folder>` — each loads and draws every frame without a panic.
#[test]
#[ignore]
fn the_specification_examples_play() {
    let Ok(dir) = std::env::var("CANTUS_LOTTIE_CORPUS") else {
        return;
    };
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let found = import::inspect(&p).unwrap_or_else(|err| panic!("{}: {err}", p.display()));
        let c = import::character(
            found,
            "file:x".into(),
            import::Settings {
                mapping: import::Mapping::guess(&[]),
                filter: import::Filter::Bilinear,
                own_speed: false,
            },
        );
        let stage = Stage::new(256, 256, Background::Transparent);
        for i in 0..24 {
            let _ = render::render(
                &stage,
                &[(&c, pose_at(&playing_at(f64::from(i) * 0.2), &c, 0.0))],
            );
        }
        println!("{}: ok", p.display());
    }
}

/// Review 1, item 1: two assets of one id once mapped past the precomps (a panic). The
/// first is kept and the card says so.
#[test]
fn duplicate_asset_ids_keep_the_first_and_say_so() {
    let mut v = doc_of(json!([]));
    let inner = |c: f64| {
        json!([{"ty": 4, "ip": 0, "op": 10, "st": 0, "ks": {"o": st(json!(100))},
        "shapes": [{"ty": "gr", "it": [{"ty": "el", "p": st(json!([256, 256])), "s": st(json!([200, 200]))},
            {"ty": "fl", "c": st(json!([c, 0, 0, 1])), "o": st(json!(100))}, {"ty": "tr", "o": st(json!(100))}]}]}])
    };
    v["assets"] = json!([
        {"id": "p", "layers": inner(1.0)},
        {"id": "p", "layers": inner(0.0)},
        {"id": "i", "p": "data:image/png;base64,AAAA"},
        {"id": "i", "p": "data:image/png;base64,AAAA"}
    ]);
    v["layers"] =
        json!([{"ty": 0, "refId": "p", "ip": 0, "op": 10, "st": 0, "ks": {}, "w": 512, "h": 512}]);
    let found = read(&v).expect("read, not a panic");
    assert!(found
        .notes
        .iter()
        .any(|n| n.key == "dance_import.note.duplicate_ids"));
    let pm = frame_of(&v, 0.0);
    let p = pm.pixel(128, 128).unwrap();
    assert!(p.red() > 200, "the first asset of the id is drawn: {p:?}");
}

/// Review 1, item 3: markers are bounded like tags and animations.
#[test]
fn markers_are_bounded() {
    let mut v = samples::lottie_doc("m");
    let many: Vec<Value> = (0..300)
        .map(|i| json!({"cm": format!("m{i}"), "tm": f64::from(i) * 0.5, "dr": 0.5}))
        .collect();
    v["markers"] = Value::Array(many);
    assert!(matches!(read(&v), Err(import::ImportError::TooLarge(_))));
}

/// Review 1, item 5: an external image that cannot be read is said on the card, and *Use*
/// copies the rest (the image is left out, as said).
#[test]
fn a_missing_external_image_is_said_and_the_rest_is_copied() {
    let d = tempfile::tempdir().unwrap();
    let mut v = samples::lottie_doc("img");
    v["assets"] = json!([{"id": "im", "w": 8, "h": 8, "u": "images/", "p": "gone.png", "e": 0}]);
    v["layers"].as_array_mut().unwrap().insert(
        0,
        json!({"ty": 2, "refId": "im", "ip": 0, "op": 192, "st": 0, "ks": {}}),
    );
    let path = d.path().join("img.json");
    std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    let found = import::inspect(&path).unwrap();
    assert!(found
        .notes
        .iter()
        .any(|n| n.key == "dance_import.note.image_unreadable"));
    let project = tempfile::tempdir().unwrap();
    let s = import::guessed(&found);
    let l = found.licence.clone();
    let rel = import::package::copy_into(project.path(), &path, &found, &s, &l).expect("Use works");
    assert!(import::load(project.path(), &rel).is_ok());
}

/// Review 1, item 8: canvas-sized buffers (masks, mattes, clips) are bounded per frame.
#[test]
fn masks_and_mattes_are_bounded_per_frame() {
    let mut v = doc_of(json!([]));
    let one = json!({"ty": 4, "ip": 0, "op": 10, "st": 0, "ks": {"o": st(json!(100))},
        "masksProperties": [{"mode": "a", "inv": false, "o": st(json!(100)),
            "pt": st(json!({"c": true, "v": [[0, 0], [100, 0], [100, 100]], "i": [[0,0],[0,0],[0,0]], "o": [[0,0],[0,0],[0,0]]}))}],
        "shapes": [{"ty": "gr", "it": [{"ty": "el", "p": st(json!([50, 50])), "s": st(json!([40, 40]))},
            {"ty": "fl", "c": st(json!([1, 0, 0, 1])), "o": st(json!(100))}, {"ty": "tr", "o": st(json!(100))}]}]});
    v["layers"] = Value::Array(vec![one; 200]);
    let doc = lottie::model::read(
        &v,
        lottie::model::Files::Folder(Path::new(".")),
        &Limits::DEFAULT,
    )
    .unwrap();
    let mut pm = Pixmap::new(256, 256).unwrap();
    let mut frame = lottie::draw::Frame::new(&doc, 0.0, import::Filter::Bilinear);
    let mut target = lottie::draw::Target::Pixels(&mut pm);
    frame.layers(
        &doc.layers,
        0.0,
        Transform::identity(),
        1.0,
        None,
        &mut target,
        0,
    );
    assert!(frame.offscreen_used() <= lottie::draw::MAX_OFFSCREEN);
    assert!(frame.offscreen_used() > 0);
}

/// Review 2, item 2: a canvas buffer is taken only for a layer that has a matte (or masks):
/// 600 plain layers leave the budget whole, every one of them is drawn, and the matte at
/// the end still cuts.
#[test]
fn plain_layers_take_no_buffer_and_a_late_matte_still_cuts() {
    let dot = |x: f64, y: f64| {
        json!({"ty": 4, "ip": 0, "op": 10, "st": 0, "ks": {"o": st(json!(100))},
        "shapes": [{"ty": "gr", "it": [{"ty": "rc", "p": st(json!([x, y])), "s": st(json!([4, 4])), "r": st(json!(0))},
            {"ty": "fl", "c": st(json!([0, 0, 1, 1])), "o": st(json!(100))}, {"ty": "tr", "o": st(json!(100))}]}]})
    };
    let mut layers = Vec::new();
    // Drawn last (first in the list): a big red square matted by a small circle.
    layers.push(json!({"ty": 4, "td": 1, "ip": 0, "op": 10, "st": 0, "ks": {"o": st(json!(100))},
        "shapes": [{"ty": "gr", "it": [{"ty": "el", "p": st(json!([256, 256])), "s": st(json!([60, 60]))},
            {"ty": "fl", "c": st(json!([1, 1, 1, 1])), "o": st(json!(100))}, {"ty": "tr", "o": st(json!(100))}]}]}));
    layers.push(json!({"ty": 4, "tt": 1, "ip": 0, "op": 10, "st": 0, "ks": {"o": st(json!(100))},
        "shapes": [{"ty": "gr", "it": [{"ty": "rc", "p": st(json!([256, 256])), "s": st(json!([300, 300])), "r": st(json!(0))},
            {"ty": "fl", "c": st(json!([1, 0, 0, 1])), "o": st(json!(100))}, {"ty": "tr", "o": st(json!(100))}]}]}));
    // A dot drawn just before them, then 600 more.
    layers.push(dot(20.0, 20.0));
    for i in 0..600 {
        layers.push(dot(
            8.0 + f64::from(i % 60) * 8.0,
            480.0 + f64::from(i / 60) * 2.0,
        ));
    }
    let mut v = doc_of(json!([]));
    v["layers"] = Value::Array(layers);
    let pm = frame_of(&v, 0.0);
    // Half size: the square spans 53…203, the circle 113…143.
    let inside = pm.pixel(128, 128).unwrap();
    let outside = pm.pixel(70, 70).unwrap();
    assert!(
        inside.red() > 200,
        "the matted square shows inside the circle: {inside:?}"
    );
    assert_eq!(outside.alpha(), 0, "and nowhere else: {outside:?}");
    assert!(
        pm.pixel(10, 10).unwrap().blue() > 200,
        "the late dot is drawn"
    );
}
