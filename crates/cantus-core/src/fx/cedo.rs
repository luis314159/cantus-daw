//! Cedo, the ducker (ADR 0144, provisional): its parameter model and its drawn shape.
//! Owned by the wave-B builder; the DSP is `cantus_dsp::cedo`, whose report
//! (`docs/round-fx/cedo.md`) says how each trigger and band split works.
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the bit**:
//! a depth of 0 ducks nothing (the DSP skips a gain of exactly 1); `out` at 0 dB is not
//! applied. The shape is content ([`crate::EffectSpec::cedo`]): `None` is the classic pump.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `trig` | Sidechain (another strip's audio), Tempo (the shape on every division), MIDI (the shape on another track's notes) | 0 … 2 |
//! | `depth` | how far it ducks (80 dB is silence, shown ∞) | 0 … 80 dB |
//! | `thr` | the filtered key's level that fires it | −90 … +6 dB |
//! | `att`, `hold`, `rel` | to the full depth along an S, held after the key's last sample over the threshold, back to nothing | 0 … 500 ms, 0 … 2 000 ms, 0 … 5 000 ms |
//! | `rshape` | the release: Linear, Exponential, S-curve | 0 … 2 |
//! | `kfilt`, `khz`, `kq` | the key's filter: Off, High-pass (24 dB/oct), Band-pass; its frequency and Q | 0 … 2, 20 … 15 000 Hz, 0.3 … 12 |
//! | `det`, `rms` | the key's level: sample Peak or RMS over a window | 0 … 1, 1 … 300 ms |
//! | `look` | the lookahead (the latency) | 0 … 10 ms |
//! | `div`, `swing`, `edge` | the shape's cycle (`modfx.div`), the swing, the edges' smoothing | 0 … 16, 0 … 100 %, 0 … 10 ms |
//! | `band`, `xover`, `xhz` | where it acts (Whole, Lows, Highs), the split (Minimum phase, Linear phase from 60 Hz), the crossover | 0 … 2, 0 … 1, 40 … 2 000 Hz |
//! | `link` | each channel by its own key (0) … both by the louder (100 %) | 0 … 100 % |
//! | `mix` | how much of the duck is applied | 0 … 100 % |
//! | `out` | the output level | −60 … +24 dB |
//! | `listen` | Output, Delta (what the duck takes away), Key (the filtered key) | 0 … 2 |
//! | `vel` | MIDI: a note's velocity scales the depth | 0 … 1 |
//!
//! The MIDI trigger plays the notes the slot hears (`EffectSlot::midi_from`, another track,
//! ADR 0022); the sidechain is the slot's key (`EffectSlot::sidechain`).

use crate::effect::{p, EffectParamInfo};
use crate::fx::lapsus::Point;
use serde::{Deserialize, Serialize};

pub const NAME_KEY: &str = "effect.cedo";
pub const SPARSE: bool = false;
/// The sidechain trigger listens to another strip.
pub const TAKES_SIDECHAIN: bool = true;
/// The MIDI trigger plays another track's notes.
pub const TAKES_NOTES: bool = true;

/// Points a shape may hold (the DSP's curve).
pub const MAX_POINTS: usize = 64;
/// A depth at or over this is silence (shown ∞).
pub const DEPTH_INF: f32 = 80.0;
/// The quarter note's index in the family's divisions (`modfx.div`).
pub const DIV_QUARTER: f32 = 8.0;

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("trig", "fx.cedo.trig"),
    ("rshape", "fx.cedo.rshape"),
    ("kfilt", "fx.cedo.kfilt"),
    ("det", "fx.cedo.det"),
    ("div", "modfx.div"),
    ("band", "fx.cedo.band"),
    ("xover", "fx.cedo.xover"),
    ("listen", "fx.cedo.listen"),
    ("vel", "fx.cedo.vel"),
];

const fn step(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn ms(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "ms", false, false)
}

pub const PARAMS: &[EffectParamInfo] = &[
    // A host's list: what fires it and how deep, then the envelope, the key, the pump, the
    // band, the output.
    step("trig", "fx.cedo.trig", 2.0, 0.0),
    p(
        "depth",
        "fx.cedo.depth",
        0.0,
        DEPTH_INF,
        0.0,
        "dB",
        false,
        false,
    ),
    p("thr", "fx.cedo.thr", -90.0, 6.0, -24.0, "dB", false, false),
    ms("att", "fx.cedo.att", 500.0, 1.0),
    ms("hold", "fx.cedo.hold", 2_000.0, 40.0),
    ms("rel", "fx.cedo.rel", 5_000.0, 150.0),
    step("rshape", "fx.cedo.rshape", 2.0, 1.0),
    step("kfilt", "fx.cedo.kfilt", 2.0, 0.0),
    p(
        "khz",
        "fx.cedo.khz",
        20.0,
        15_000.0,
        100.0,
        "Hz",
        true,
        false,
    ),
    p("kq", "fx.cedo.kq", 0.3, 12.0, 1.0, "", true, false),
    step("det", "fx.cedo.det", 1.0, 0.0),
    p("rms", "fx.cedo.rms", 1.0, 300.0, 10.0, "ms", true, false),
    ms("look", "fx.cedo.look", 10.0, 0.0),
    step("div", "fx.cedo.div", 16.0, DIV_QUARTER),
    p("swing", "fx.cedo.swing", 0.0, 1.0, 0.0, "%", false, false),
    ms("edge", "fx.cedo.edge", 10.0, 1.0),
    step("band", "fx.cedo.band", 2.0, 0.0),
    step("xover", "fx.cedo.xover", 1.0, 0.0),
    p(
        "xhz",
        "fx.cedo.xhz",
        40.0,
        2_000.0,
        120.0,
        "Hz",
        true,
        false,
    ),
    p("link", "fx.cedo.link", 0.0, 1.0, 1.0, "%", false, false),
    p("mix", "fx.cedo.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("out", "fx.cedo.out", -60.0, 24.0, 0.0, "dB", false, false),
    step("listen", "fx.cedo.listen", 2.0, 0.0),
    step("vel", "fx.cedo.vel", 1.0, 0.0),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// The drawn shape: across one cycle of the division, `y` the amount ducked (0 … 1), each
/// segment shaped as the automation editor's (Fluctus's model, ADR 0055; Lapsus's points).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point>,
}

/// The classic pump (`cantus_dsp::cedo::CLASSIC_PUMP`): the full depth on the beat,
/// easing back to nothing by 65 % of the cycle, then nothing.
pub fn classic_pump() -> Shape {
    Shape {
        points: vec![
            Point::new(0.0, 1.0).bend(-0.2),
            Point::new(0.65, 0.0).shape(2),
        ],
    }
}

impl Shape {
    /// Clamped into the curve's ranges, at most [`MAX_POINTS`] points, sorted by place:
    /// what a shape from a file or a paste is made into before anything plays it.
    pub fn sanitized(&self) -> Shape {
        let fin = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let mut points: Vec<Point> = self
            .points
            .iter()
            .take(MAX_POINTS)
            .map(|q| Point {
                x: fin(q.x, 0.0).clamp(0.0, 1.0),
                y: fin(q.y, 0.0).clamp(0.0, 1.0),
                t: fin(q.t, 0.0).clamp(-1.0, 1.0),
                s: q.s.min(5),
            })
            .collect();
        points.sort_by(|a, b| a.x.total_cmp(&b.x));
        Shape { points }
    }
}

/// Shapes a command would refuse (a hand edit), repaired on load: a shape on a slot that is
/// not a Cedo taken off, points out of range or order sanitized. Returns how many changed.
pub fn repair_project(project: &mut crate::model::Project) -> usize {
    let mut changed = 0;
    for e in project.all_effects_mut() {
        if e.spec.kind != crate::EffectKind::Ducker {
            if e.spec.cedo.take().is_some() {
                changed += 1;
            }
            continue;
        }
        if let Some(s) = e.spec.cedo.as_deref() {
            let clean = s.sanitized();
            if clean != *s {
                changed += 1;
                e.spec.cedo = Some(Box::new(clean));
            }
        }
    }
    changed
}

/// fxb:cedo — a preset's shape rides in its flat map of numbers (ADR 0101, as Fractus's
/// lanes and Tabula's boards do), so it travels every path a preset does — the factory's
/// files, the user's presets, A/B, the drift mark, the audition, the agents: `shape.n` the
/// number of points (its presence says the preset draws a shape of its own; without it, the
/// classic pump), and point `i` as `shape.{i}.x`, `shape.{i}.y`, `shape.{i}.bend` (the
/// tension, written when not 0) and `shape.{i}.seg` (the segment's shape, written when not
/// 0). No parameter's key starts with `shape.`.
pub const FLAT_PREFIX: &str = "shape.";

/// The field a flat key names: `None` for a parameter's key; `Some((None, "n"))` for the
/// count; `Some((Some(i), field))` for a point's field.
pub fn flat_key(key: &str) -> Option<(Option<usize>, &str)> {
    let rest = key.strip_prefix(FLAT_PREFIX)?;
    if rest == "n" {
        return Some((None, "n"));
    }
    let (i, field) = rest.split_once('.')?;
    let i: usize = i.parse().ok()?;
    if i >= MAX_POINTS || !matches!(field, "x" | "y" | "bend" | "seg") {
        return None;
    }
    Some((Some(i), field))
}

/// Whether a flat key's value is in its range (a factory file's check).
pub fn flat_value_ok(key: &str, v: f32) -> bool {
    match flat_key(key) {
        Some((None, _)) => v.fract() == 0.0 && (0.0..=MAX_POINTS as f32).contains(&v),
        Some((Some(_), "x" | "y")) => (0.0..=1.0).contains(&v),
        Some((Some(_), "bend")) => (-1.0..=1.0).contains(&v),
        Some((Some(_), "seg")) => v.fract() == 0.0 && (0.0..=5.0).contains(&v),
        _ => false,
    }
}

/// The shape a flat map draws (sanitized), `None` when it draws none (the classic pump).
pub fn shape_of_flat(map: &std::collections::BTreeMap<String, f32>) -> Option<Shape> {
    let n = (*map.get("shape.n")?).max(0.0) as usize;
    let n = n.min(MAX_POINTS);
    let get = |i: usize, f: &str| map.get(&format!("shape.{i}.{f}")).copied();
    let points = (0..n)
        .map(|i| Point {
            x: get(i, "x").unwrap_or(0.0),
            y: get(i, "y").unwrap_or(0.0),
            t: get(i, "bend").unwrap_or(0.0),
            s: get(i, "seg").unwrap_or(0.0).max(0.0) as u8,
        })
        .collect();
    Some(Shape { points }.sanitized())
}

/// Whether the flat map draws `shape` (`None`: it draws none), without allocating: what an
/// editor asks every frame (has the preset been moved?).
pub fn flat_is(map: &std::collections::BTreeMap<String, f32>, shape: Option<&Shape>) -> bool {
    use std::ops::Bound;
    let Some(n) = map.get("shape.n") else {
        return shape.is_none();
    };
    let Some(s) = shape else {
        return false;
    };
    if (*n).max(0.0) as usize != s.points.len() {
        return false;
    }
    let (mut bends, mut segs) = (0usize, 0usize);
    let from: (Bound<&str>, Bound<&str>) = (Bound::Included(FLAT_PREFIX), Bound::Unbounded);
    for (k, v) in map.range::<str, _>(from) {
        if !k.starts_with(FLAT_PREFIX) {
            break;
        }
        let Some((Some(i), field)) = flat_key(k) else {
            continue;
        };
        let Some(q) = s.points.get(i) else {
            return false;
        };
        let want = match field {
            "x" => q.x,
            "y" => q.y,
            "bend" => {
                bends += usize::from(*v != 0.0);
                q.t
            }
            _ => {
                segs += usize::from(*v != 0.0);
                f32::from(q.s)
            }
        };
        if (want - v).abs() > 1e-6 {
            return false;
        }
    }
    bends == s.points.iter().filter(|q| q.t != 0.0).count()
        && segs == s.points.iter().filter(|q| q.s != 0).count()
}

/// `params` with `shape` written in (the module's flat keys; `None` writes none). Any flat
/// key `params` held is dropped first.
pub fn to_flat(
    params: &std::collections::BTreeMap<String, f32>,
    shape: Option<&Shape>,
) -> std::collections::BTreeMap<String, f32> {
    let mut out: std::collections::BTreeMap<String, f32> = params
        .iter()
        .filter(|(k, _)| flat_key(k).is_none())
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    if let Some(s) = shape {
        out.insert("shape.n".into(), s.points.len().min(MAX_POINTS) as f32);
        for (i, q) in s.points.iter().take(MAX_POINTS).enumerate() {
            out.insert(format!("shape.{i}.x"), q.x);
            out.insert(format!("shape.{i}.y"), q.y);
            if q.t != 0.0 {
                out.insert(format!("shape.{i}.bend"), q.t);
            }
            if q.s != 0 {
                out.insert(format!("shape.{i}.seg"), f32::from(q.s));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_defaults_in_range_and_choices_name_stepped_keys() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        for (k, _) in CHOICES {
            let info = PARAMS.iter().find(|p| p.key == *k).expect(k);
            assert!(info.stepped, "{k}");
        }
        let depth = PARAMS.iter().find(|p| p.key == "depth").unwrap();
        assert_eq!(depth.default, 0.0, "the null: depth 0");
    }

    #[test]
    fn a_shape_is_sanitized_into_its_ranges() {
        let s = Shape {
            points: vec![
                Point::new(0.9, 2.0),
                Point::new(f32::NAN, -1.0).bend(4.0),
                Point::new(0.2, 0.5).shape(9),
            ],
        }
        .sanitized();
        assert_eq!(s.points[0], Point::new(0.0, 0.0).bend(1.0));
        assert_eq!(s.points[1], Point::new(0.2, 0.5).shape(5));
        assert_eq!(s.points[2], Point::new(0.9, 1.0));
        assert_eq!(classic_pump().sanitized(), classic_pump());
    }

    #[test]
    fn a_shape_rides_a_flat_map_and_comes_back() {
        use std::collections::BTreeMap;
        let mut params = BTreeMap::new();
        params.insert("depth".to_string(), 9.0);
        params.insert("shape.7.x".to_string(), 0.5); // a stale point, dropped
        for s in [classic_pump(), Shape::default()] {
            let flat = to_flat(&params, Some(&s));
            assert_eq!(flat.get("depth"), Some(&9.0));
            assert_eq!(shape_of_flat(&flat), Some(s.clone()));
            assert!(flat_is(&flat, Some(&s)) && !flat_is(&flat, None));
            let mut moved = s.clone();
            moved.points.push(Point::new(0.9, 0.1));
            assert!(!flat_is(&flat, Some(&moved)));
            assert!(flat
                .iter()
                .all(|(k, v)| flat_key(k).is_none() || flat_value_ok(k, *v)));
        }
        let plain = to_flat(&params, None);
        assert_eq!(shape_of_flat(&plain), None, "no shape: the classic pump");
        assert!(flat_is(&plain, None) && !flat_is(&plain, Some(&classic_pump())));
        let mut bent = classic_pump();
        let flat = to_flat(&params, Some(&bent));
        bent.points[1].t = 0.5;
        assert!(!flat_is(&flat, Some(&bent)), "a bend the map does not write");
        assert!(!plain.contains_key("shape.7.x"));
        for p in PARAMS {
            assert!(flat_key(p.key).is_none(), "{}", p.key);
        }
        assert_eq!(flat_key("shape.3.bend"), Some((Some(3), "bend")));
        assert_eq!(flat_key("shape.64.x"), None);
        assert!(!flat_value_ok("shape.0.seg", 6.0) && flat_value_ok("shape.n", 3.0));
    }
}
