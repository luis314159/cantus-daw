//! Test files made by code (ours): a little Lottie character with every move as a marker,
//! and hostile variants.
use serde_json::{json, Value};

pub const MOVES: [&str; 16] = [
    "idle", "bounce", "step_left", "step_right", "sway", "clap", "wave", "jump", "spin", "kick",
    "shuffle", "point", "arms_up", "twist", "bow", "freeze",
];
const FPM: f64 = 30.0; // frames per move in the file

fn st(v: Value) -> Value {
    json!({"a": 0, "k": v})
}

/// Rotation keyframes: for each move m, swing to `amp(m)` and back with an ease.
fn rot_keys(amp: impl Fn(usize) -> f64) -> Value {
    let mut k = Vec::new();
    for m in 0..MOVES.len() {
        let t0 = m as f64 * FPM;
        let ease = json!({"o": {"x": [0.33], "y": [0.0]}, "i": {"x": [0.67], "y": [1.0]}});
        for (dt, v) in [(0.0, 0.0), (FPM * 0.5, amp(m)), (FPM - 1.0, 0.0)] {
            let mut kf = json!({"t": t0 + dt, "s": [v]});
            kf["o"] = ease["o"].clone();
            kf["i"] = ease["i"].clone();
            k.push(kf);
        }
    }
    json!({"a": 1, "k": k})
}

fn ks(p: [f64; 2], a: [f64; 2], r: Value) -> Value {
    json!({"a": st(json!(a)), "p": st(json!(p)), "s": st(json!([100, 100])), "r": r, "o": st(json!(100))})
}

fn fill(c: [f64; 3]) -> Value {
    json!({"ty": "fl", "c": st(json!([c[0], c[1], c[2], 1])), "o": st(json!(100)), "r": 1})
}

fn stroke(w: f64) -> Value {
    json!({"ty": "st", "c": st(json!([0.12, 0.1, 0.16, 1])), "o": st(json!(100)), "w": st(json!(w)), "lc": 2, "lj": 2})
}

fn group(items: Vec<Value>) -> Value {
    let mut it = items;
    it.push(json!({"ty": "tr", "p": st(json!([0, 0])), "a": st(json!([0, 0])), "s": st(json!([100, 100])), "r": st(json!(0)), "o": st(json!(100))}));
    json!({"ty": "gr", "it": it})
}

fn ellipse(p: [f64; 2], s: [f64; 2]) -> Value {
    json!({"ty": "el", "p": st(json!(p)), "s": st(json!(s))})
}

fn rect(p: [f64; 2], s: [f64; 2], r: f64) -> Value {
    json!({"ty": "rc", "p": st(json!(p)), "s": st(json!(s)), "r": st(json!(r))})
}

fn mouth() -> Value {
    json!({"ty": "sh", "ks": st(json!({"c": false, "v": [[-14, 0], [14, 0]], "i": [[0, 0], [-8, 12]], "o": [[8, 12], [0, 0]]}))})
}

fn layer(ind: i64, parent: Option<i64>, nm: &str, ks: Value, shapes: Vec<Value>) -> Value {
    let mut l = json!({"ddd": 0, "ty": 4, "ind": ind, "nm": nm, "ip": 0, "op": FPM * 16.0, "st": 0, "ks": ks, "shapes": shapes});
    if let Some(p) = parent {
        l["parent"] = json!(p);
    }
    l
}

/// The character, `copies` times side by side (to measure a heavier file).
pub fn lottie_character(copies: u32) -> Vec<u8> {
    let mut layers = Vec::new();
    for c in 0..copies {
        let b = i64::from(c) * 100;
        let dx = 256.0 + (f64::from(c) - (f64::from(copies) - 1.0) / 2.0) * 60.0;
        let null = json!({"ddd": 0, "ty": 3, "ind": b + 1, "nm": "root", "ip": 0, "op": FPM * 16.0, "st": 0,
            "ks": ks([dx, 470.0], [0.0, 0.0], rot_keys(|m| if m == 4 { 8.0 } else { 0.0 }))});
        layers.push(null);
        let skin = [0.98, 0.72, 0.36];
        let shirt = [0.25, 0.55, 0.95];
        layers.push(layer(b + 2, Some(b + 1), "head", ks([0.0, -250.0], [0.0, 0.0], rot_keys(|m| (m as f64 % 3.0 - 1.0) * 10.0)), vec![
            group(vec![ellipse([-20.0, -10.0], [16.0, 22.0]), fill([1.0, 1.0, 1.0]), stroke(3.0)]),
            group(vec![ellipse([20.0, -10.0], [16.0, 22.0]), fill([1.0, 1.0, 1.0]), stroke(3.0)]),
            group(vec![mouth(), stroke(4.0)]),
            group(vec![ellipse([0.0, 0.0], [110.0, 100.0]), fill(skin), stroke(5.0)]),
        ]));
        layers.push(layer(b + 3, Some(b + 1), "body", ks([0.0, -130.0], [0.0, 0.0], st(json!(0))), vec![
            group(vec![rect([0.0, 0.0], [90.0, 130.0], 30.0), fill(shirt), stroke(5.0)]),
        ]));
        for (side, sx) in [("l", -1.0), ("r", 1.0)] {
            layers.push(layer(b + if sx < 0.0 { 4 } else { 5 }, Some(b + 3), &format!("arm_{side}"), ks([sx * 50.0, -50.0], [0.0, 0.0], rot_keys(move |m| sx * (20.0 + m as f64 * 7.0))), vec![
                group(vec![ellipse([sx * 0.0, 70.0], [26.0, 26.0]), fill(skin), stroke(4.0)]),
                group(vec![rect([0.0, 32.0], [22.0, 70.0], 11.0), fill(shirt), stroke(4.0)]),
            ]));
            layers.push(layer(b + if sx < 0.0 { 6 } else { 7 }, Some(b + 1), &format!("leg_{side}"), ks([sx * 22.0, -70.0], [0.0, 0.0], rot_keys(move |m| sx * (m as f64 % 4.0) * 6.0)), vec![
                group(vec![rect([sx * 6.0, 66.0], [40.0, 18.0], 9.0), fill([0.2, 0.2, 0.25]), stroke(4.0)]),
                group(vec![rect([0.0, 30.0], [26.0, 66.0], 12.0), fill([0.3, 0.3, 0.4]), stroke(4.0)]),
            ]));
        }
    }
    let markers: Vec<Value> = MOVES.iter().enumerate().map(|(i, m)| json!({"cm": m, "tm": i as f64 * FPM, "dr": FPM})).collect();
    let doc = json!({"v": "5.7.1", "fr": 30, "ip": 0, "op": FPM * 16.0, "w": 512, "h": 512, "nm": "proto", "ddd": 0,
        "assets": [], "layers": layers, "markers": markers,
        "meta": {"a": "Cantus tests", "d": "CC0-1.0"}});
    serde_json::to_vec(&doc).unwrap()
}

/// Valid or plausible files that a player must survive.
pub fn lottie_hostile() -> Vec<(String, Vec<u8>)> {
    let base: Value = serde_json::from_slice(&lottie_character(1)).unwrap();
    let mut out = Vec::new();
    // 1. A layer transform with no rotation (valid: every transform property is optional).
    let mut v = base.clone();
    v["layers"][2]["ks"].as_object_mut().unwrap().remove("r");
    out.push(("valid:no-rotation".into(), v));
    // 2. A 3D layer with split rotations (rx, ry, rz) — After Effects writes these.
    let mut v = base.clone();
    let ks = v["layers"][2]["ks"].as_object_mut().unwrap();
    ks.remove("r");
    ks.insert("rx".into(), st(json!(0)));
    ks.insert("ry".into(), st(json!(0)));
    ks.insert("rz".into(), st(json!(10)));
    v["layers"][2]["ddd"] = json!(1);
    out.push(("valid:3d-split-rotation".into(), v));
    // 3. Blend mode Add (bm 16).
    let mut v = base.clone();
    v["layers"][2]["bm"] = json!(16);
    out.push(("valid:blend-add".into(), v));
    // 4. A group transform with a split position.
    let mut v = base.clone();
    let tr = v["layers"][2]["shapes"][0]["it"].as_array_mut().unwrap().last_mut().unwrap();
    tr["p"] = json!({"s": true, "x": st(json!(1)), "y": st(json!(2))});
    out.push(("valid:group-split-position".into(), v));
    // 5. Nesting 100 000 deep (a stack bomb for a recursive parser).
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    out.push(("hostile:deep-nesting".into(), Value::Null));
    let mut o: Vec<(String, Vec<u8>)> = out.into_iter().map(|(n, v)| (n, serde_json::to_vec(&v).unwrap())).collect();
    o.last_mut().unwrap().1 = format!("{{\"v\":\"5\",\"layers\":{deep}}}").into_bytes();
    // 6. A precomp that contains itself.
    let mut v = base;
    v["assets"] = json!([{"id": "c", "layers": [{"ty": 0, "refId": "c", "ind": 1, "ip": 0, "op": 10, "st": 0, "ks": {}}]}]);
    v["layers"] = json!([{"ty": 0, "refId": "c", "ind": 1, "ip": 0, "op": 10, "st": 0, "ks": {}, "w": 10, "h": 10}]);
    o.push(("hostile:self-precomp".into(), serde_json::to_vec(&v).unwrap()));
    o
}

pub fn write_all(dir: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/character.json"), lottie_character(1)).unwrap();
}
