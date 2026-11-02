//! Lottie's animated properties: a value, or keyframes with hold, linear and cubic-bezier
//! easing (per dimension), and spatial tangents on positions. Evaluated as a pure function
//! of the frame.

use super::super::limits::{Budget, Limits};
use super::super::ImportError;
use serde_json::Value;

/// One keyframe.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub t: f32,
    pub s: Vec<f32>,
    pub hold: bool,
    /// Out and in tangents of the easing, per dimension (x, y), in 0…1 time.
    pub o: Vec<(f32, f32)>,
    pub i: Vec<(f32, f32)>,
    /// Spatial tangents (positions): out of this key, into the next.
    pub to: Option<[f32; 2]>,
    pub ti: Option<[f32; 2]>,
}

/// An animated property of numbers (a scalar, a point, a colour, a gradient's stops).
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Static(Vec<f32>),
    Keys(Vec<Key>),
}

pub fn num(v: &Value) -> f32 {
    v.as_f64()
        .filter(|f| f.is_finite())
        .map(|f| f.clamp(-1e7, 1e7) as f32)
        .unwrap_or(0.0)
}

/// Numbers from a number or an array of numbers (anything else: none).
pub fn nums(v: &Value) -> Vec<f32> {
    match v {
        Value::Number(_) => vec![num(v)],
        Value::Array(a) => a.iter().filter(|x| x.is_number()).map(num).collect(),
        _ => Vec::new(),
    }
}

fn tangents(v: &Value) -> Vec<(f32, f32)> {
    let xs = nums(&v["x"]);
    let ys = nums(&v["y"]);
    let n = xs.len().max(ys.len());
    (0..n)
        .map(|k| {
            let x = xs.get(k).or(xs.first()).copied().unwrap_or(0.0);
            let y = ys.get(k).or(ys.first()).copied().unwrap_or(0.0);
            (x, y)
        })
        .collect()
}

fn pair(v: &Value) -> Option<[f32; 2]> {
    let n = nums(v);
    (n.len() >= 2).then(|| [n[0], n[1]])
}

/// Whether `v` (a property object) carries an expression — never evaluated.
pub fn has_expression(v: &Value) -> bool {
    v.get("x").is_some_and(|x| x.is_string())
}

impl Prop {
    /// A property from its JSON (`{"a": 0|1, "k": …}`), `default` when absent.
    pub fn parse(
        v: Option<&Value>,
        default: &[f32],
        budget: &mut Budget,
        limits: &Limits,
    ) -> Result<Prop, ImportError> {
        let Some(v) = v.filter(|v| v.is_object()) else {
            return Ok(Prop::Static(default.to_vec()));
        };
        let k = &v["k"];
        let animated = matches!(k, Value::Array(a) if a.first().is_some_and(Value::is_object));
        if !animated {
            let n = nums(k);
            return Ok(Prop::Static(if n.is_empty() {
                default.to_vec()
            } else {
                n
            }));
        }
        let list = k.as_array().map(Vec::as_slice).unwrap_or(&[]);
        budget.take("keyframes", list.len(), limits.keyframes)?;
        let mut keys: Vec<Key> = Vec::with_capacity(list.len());
        for (n, kf) in list.iter().enumerate() {
            let mut s = nums(&kf["s"]);
            if s.is_empty() && n > 0 {
                // The old form: a key's start is the previous key's end (`e`).
                s = nums(&list[n - 1]["e"]);
            }
            if s.is_empty() {
                s = keys
                    .last()
                    .map(|k| k.s.clone())
                    .unwrap_or_else(|| default.to_vec());
            }
            keys.push(Key {
                t: num(&kf["t"]),
                s,
                hold: kf["h"].as_i64() == Some(1) || kf["h"].as_bool() == Some(true),
                o: tangents(&kf["o"]),
                i: tangents(&kf["i"]),
                to: pair(&kf["to"]),
                ti: pair(&kf["ti"]),
            });
        }
        // Keys in time order (a file may not keep it).
        keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        if keys.is_empty() {
            return Ok(Prop::Static(default.to_vec()));
        }
        Ok(Prop::Keys(keys))
    }

    pub fn is_animated(&self) -> bool {
        matches!(self, Prop::Keys(k) if k.len() > 1)
    }

    /// The value at frame `f`.
    pub fn at(&self, f: f32) -> Vec<f32> {
        let keys = match self {
            Prop::Static(v) => return v.clone(),
            Prop::Keys(k) => k,
        };
        let first = &keys[0];
        if f <= first.t || keys.len() == 1 {
            return first.s.clone();
        }
        // The segment f is in: the last key at or before f.
        let n = keys.partition_point(|k| k.t <= f);
        if n >= keys.len() {
            return keys[keys.len() - 1].s.clone();
        }
        let (a, b) = (&keys[n - 1], &keys[n]);
        if a.hold || b.t <= a.t {
            return a.s.clone();
        }
        let x = ((f - a.t) / (b.t - a.t)).clamp(0.0, 1.0);
        let dims = a.s.len().min(b.s.len());
        let mut out: Vec<f32> = (0..dims)
            .map(|d| {
                let o = a.o.get(d).or(a.o.first()).copied().unwrap_or((0.0, 0.0));
                let i = a.i.get(d).or(a.i.first()).copied().unwrap_or((1.0, 1.0));
                let e = ease(x, o, i);
                a.s[d] + (b.s[d] - a.s[d]) * e
            })
            .collect();
        // A position moving along a curve (its spatial tangents).
        if let (Some(to), Some(ti)) = (a.to, a.ti) {
            if dims >= 2 && (to != [0.0, 0.0] || ti != [0.0, 0.0]) {
                let o = a.o.first().copied().unwrap_or((0.0, 0.0));
                let i = a.i.first().copied().unwrap_or((1.0, 1.0));
                let u = ease(x, o, i);
                let p0 = [a.s[0], a.s[1]];
                let p3 = [b.s[0], b.s[1]];
                let p1 = [p0[0] + to[0], p0[1] + to[1]];
                let p2 = [p3[0] + ti[0], p3[1] + ti[1]];
                let v = 1.0 - u;
                for k in 0..2 {
                    out[k] = v * v * v * p0[k]
                        + 3.0 * v * v * u * p1[k]
                        + 3.0 * v * u * u * p2[k]
                        + u * u * u * p3[k];
                }
            }
        }
        out
    }

    pub fn at1(&self, f: f32, default: f32) -> f32 {
        let v = self.at(f);
        v.first()
            .copied()
            .filter(|x| x.is_finite())
            .unwrap_or(default)
    }

    pub fn at2(&self, f: f32, default: [f32; 2]) -> [f32; 2] {
        let v = self.at(f);
        [
            v.first().copied().unwrap_or(default[0]),
            v.get(1).copied().unwrap_or(default[1]),
        ]
    }

    /// The frames its keys sit on.
    pub fn key_times(&self) -> Vec<f32> {
        match self {
            Prop::Static(_) => Vec::new(),
            Prop::Keys(k) => k.iter().map(|k| k.t).collect(),
        }
    }
}

/// CSS-style cubic-bezier easing: (0,0), o, i, (1,1); `x` is time, the result progress.
pub fn ease(x: f32, o: (f32, f32), i: (f32, f32)) -> f32 {
    let (x1, y1, x2, y2) = (o.0.clamp(0.0, 1.0), o.1, i.0.clamp(0.0, 1.0), i.1);
    if (x1 - y1).abs() < 1e-6 && (x2 - y2).abs() < 1e-6 {
        return x; // linear
    }
    let bez = |u: f32, p1: f32, p2: f32| {
        let v = 1.0 - u;
        3.0 * v * v * u * p1 + 3.0 * v * u * u * p2 + u * u * u
    };
    // Bisection on x(u) (monotonic for x1, x2 in 0…1): 24 steps, ~6e-8.
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if bez(mid, x1, x2) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(0.5 * (lo + hi), y1, y2)
}

/// A shape's path: vertices with their in and out tangents (relative), open or closed.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Bezier {
    pub closed: bool,
    pub v: Vec<[f32; 2]>,
    pub i: Vec<[f32; 2]>,
    pub o: Vec<[f32; 2]>,
}

fn points(v: &Value) -> Vec<[f32; 2]> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let n = nums(p);
            [
                n.first().copied().unwrap_or(0.0),
                n.get(1).copied().unwrap_or(0.0),
            ]
        })
        .collect()
}

impl Bezier {
    pub fn parse(v: &Value, budget: &mut Budget, limits: &Limits) -> Result<Bezier, ImportError> {
        // A shape keyframe's `s` is an array of one bezier.
        let v = if v.is_array() { &v[0] } else { v };
        let mut b = Bezier {
            closed: v["c"].as_bool().unwrap_or(false),
            v: points(&v["v"]),
            i: points(&v["i"]),
            o: points(&v["o"]),
        };
        budget.take("path vertices", b.v.len(), limits.path_vertices)?;
        let n = b.v.len();
        b.i.resize(n, [0.0, 0.0]);
        b.o.resize(n, [0.0, 0.0]);
        Ok(b)
    }

    fn lerp(&self, other: &Bezier, e: f32) -> Bezier {
        if other.v.len() != self.v.len() {
            return self.clone();
        }
        let mix = |a: &[[f32; 2]], b: &[[f32; 2]]| {
            a.iter()
                .zip(b)
                .map(|(p, q)| [p[0] + (q[0] - p[0]) * e, p[1] + (q[1] - p[1]) * e])
                .collect()
        };
        Bezier {
            closed: self.closed,
            v: mix(&self.v, &other.v),
            i: mix(&self.i, &other.i),
            o: mix(&self.o, &other.o),
        }
    }
}

/// A path's keyframe: its frame, its shape, held, and its easing out and in.
pub type PathKey = (f32, Bezier, bool, (f32, f32), (f32, f32));

/// An animated path.
#[derive(Clone, Debug, PartialEq)]
pub enum PathProp {
    Static(Bezier),
    Keys(Vec<PathKey>),
}

impl PathProp {
    pub fn parse(v: &Value, budget: &mut Budget, limits: &Limits) -> Result<PathProp, ImportError> {
        let k = &v["k"];
        let animated =
            matches!(k, Value::Array(a) if a.first().is_some_and(|x| x.get("t").is_some()));
        if !animated {
            return Ok(PathProp::Static(Bezier::parse(k, budget, limits)?));
        }
        let list = k.as_array().map(Vec::as_slice).unwrap_or(&[]);
        budget.take("keyframes", list.len(), limits.keyframes)?;
        let mut keys: Vec<PathKey> = Vec::new();
        for (n, kf) in list.iter().enumerate() {
            let s = if kf.get("s").is_some() {
                Bezier::parse(&kf["s"], budget, limits)?
            } else if n > 0 {
                Bezier::parse(&list[n - 1]["e"], budget, limits)?
            } else {
                Bezier::default()
            };
            let first = |v: &Value| tangents(v).first().copied();
            keys.push((
                num(&kf["t"]),
                s,
                kf["h"].as_i64() == Some(1),
                first(&kf["o"]).unwrap_or((0.0, 0.0)),
                first(&kf["i"]).unwrap_or((1.0, 1.0)),
            ));
        }
        keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if keys.is_empty() {
            return Ok(PathProp::Static(Bezier::default()));
        }
        Ok(PathProp::Keys(keys))
    }

    pub fn at(&self, f: f32) -> Bezier {
        let keys = match self {
            PathProp::Static(b) => return b.clone(),
            PathProp::Keys(k) => k,
        };
        if f <= keys[0].0 || keys.len() == 1 {
            return keys[0].1.clone();
        }
        let n = keys.partition_point(|k| k.0 <= f);
        if n >= keys.len() {
            return keys[keys.len() - 1].1.clone();
        }
        let (a, b) = (&keys[n - 1], &keys[n]);
        if a.2 || b.0 <= a.0 {
            return a.1.clone();
        }
        let x = ((f - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        a.1.lerp(&b.1, ease(x, a.3, a.4))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn p(v: Value) -> Prop {
        Prop::parse(Some(&v), &[0.0], &mut Budget::new(), &Limits::DEFAULT).unwrap()
    }

    #[test]
    fn statics_holds_linears_and_eases() {
        assert_eq!(p(json!({"a": 0, "k": [1, 2]})).at(5.0), vec![1.0, 2.0]);
        let lin = p(json!({"a": 1, "k": [
            {"t": 0, "s": [0], "o": {"x": [0], "y": [0]}, "i": {"x": [1], "y": [1]}},
            {"t": 10, "s": [100]}]}));
        assert_eq!(lin.at1(5.0, 0.0), 50.0);
        assert_eq!(lin.at1(-3.0, 0.0), 0.0);
        assert_eq!(lin.at1(30.0, 0.0), 100.0);
        let hold = p(json!({"a": 1, "k": [{"t": 0, "s": [0], "h": 1}, {"t": 10, "s": [100]}]}));
        assert_eq!(hold.at1(9.9, 0.0), 0.0);
        assert_eq!(hold.at1(10.0, 0.0), 100.0);
        let eased = p(json!({"a": 1, "k": [
            {"t": 0, "s": [0], "o": {"x": 0.42, "y": 0}, "i": {"x": 0.58, "y": 1}},
            {"t": 10, "s": [100]}]}));
        let mid = eased.at1(5.0, 0.0);
        assert!((mid - 50.0).abs() < 0.01, "ease-in-out is symmetric: {mid}");
        assert!(eased.at1(2.0, 0.0) < 20.0, "slow at the start");
    }

    #[test]
    fn the_old_form_takes_its_start_from_the_previous_end() {
        let old = p(json!({"a": 1, "k": [
            {"t": 0, "s": [10], "e": [20], "o": {"x": [0], "y": [0]}, "i": {"x": [1], "y": [1]}},
            {"t": 10}]}));
        assert_eq!(old.at1(10.0, 0.0), 20.0);
    }

    #[test]
    fn a_position_follows_its_spatial_tangents() {
        let pos = p(json!({"a": 1, "k": [
            {"t": 0, "s": [0, 0], "to": [0, -30], "ti": [0, -30], "o": {"x": 0, "y": 0}, "i": {"x": 1, "y": 1}},
            {"t": 10, "s": [100, 0]}]}));
        let [x, y] = pos.at2(5.0, [0.0, 0.0]);
        assert!(
            (x - 50.0).abs() < 1e-3 && y < -20.0,
            "an arc through ({x}, {y})"
        );
    }

    #[test]
    fn nonsense_numbers_are_tamed() {
        let v = p(json!({"a": 0, "k": [1e300, "x", null]}));
        assert_eq!(v.at(0.0), vec![1e7]);
        let empty = p(json!({"a": 1, "k": []}));
        assert_eq!(empty.at(1.0), vec![0.0]);
    }

    #[test]
    fn paths_morph_between_keys() {
        let b = |x: f32| json!([{"c": true, "v": [[0, 0], [x, 0], [x, x]], "i": [[0,0],[0,0],[0,0]], "o": [[0,0],[0,0],[0,0]]}]);
        let v = json!({"a": 1, "k": [{"t": 0, "s": b(10.0), "o": {"x": 0, "y": 0}, "i": {"x": 1, "y": 1}}, {"t": 10, "s": b(20.0)}]});
        let pp = PathProp::parse(&v, &mut Budget::new(), &Limits::DEFAULT).unwrap();
        assert_eq!(pp.at(5.0).v[1], [15.0, 0.0]);
    }
}
