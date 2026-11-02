//! Prototype: a minimal Lottie player over tiny-skia (measurement only, ADR 0089).
//! Shape layers, null parents, groups, ellipse/rect/path, fill/stroke, transforms with
//! keyframes (hold, linear, cubic-bezier easing), split positions, markers.
use serde_json::Value;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Clone, Debug)]
pub enum Prop {
    Static(Vec<f32>),
    Keys(Vec<Key>),
}

#[derive(Clone, Debug)]
pub struct Key {
    t: f32,
    s: Vec<f32>,
    hold: bool,
    o: (f32, f32),
    i: (f32, f32),
}

fn num_vec(v: &Value) -> Vec<f32> {
    match v {
        Value::Number(n) => vec![n.as_f64().unwrap_or(0.0) as f32],
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect(),
        _ => vec![],
    }
}

fn first(v: &Value) -> f32 {
    num_vec(v).first().copied().unwrap_or(0.0)
}

pub fn prop(v: Option<&Value>, default: &[f32]) -> Prop {
    let Some(v) = v else {
        return Prop::Static(default.to_vec());
    };
    let k = &v["k"];
    let animated = v["a"].as_i64() == Some(1)
        || matches!(k, Value::Array(a) if a.first().map(|x| x.is_object()).unwrap_or(false));
    if !animated {
        return Prop::Static(num_vec(k));
    }
    let keys = k.as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for (n, kf) in keys.iter().enumerate() {
        let s = if kf.get("s").is_some() {
            num_vec(&kf["s"])
        } else if n > 0 {
            num_vec(&keys[n - 1]["e"])
        } else {
            vec![]
        };
        out.push(Key {
            t: kf["t"].as_f64().unwrap_or(0.0) as f32,
            s,
            hold: kf["h"].as_i64() == Some(1),
            o: (first(&kf["o"]["x"]), first(&kf["o"]["y"])),
            i: (
                kf["i"]["x"].as_f64().map(|x| x as f32).unwrap_or(first(&kf["i"]["x"])),
                first(&kf["i"]["y"]),
            ),
        });
    }
    Prop::Keys(out)
}

/// Solve a CSS-style cubic bezier: x(u) = t, return y(u).
pub fn ease(t: f32, o: (f32, f32), i: (f32, f32)) -> f32 {
    let (x1, y1, x2, y2) = (o.0, o.1, i.0, i.1);
    let bez = |u: f32, a: f32, b: f32| {
        let v = 1.0 - u;
        3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
    };
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut u = t;
    for _ in 0..24 {
        let x = bez(u, x1, x2);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            lo = u;
        } else {
            hi = u;
        }
        u = 0.5 * (lo + hi);
    }
    bez(u, y1, y2)
}

impl Prop {
    pub fn at(&self, f: f32) -> Vec<f32> {
        match self {
            Prop::Static(v) => v.clone(),
            Prop::Keys(k) => {
                if k.is_empty() {
                    return vec![];
                }
                if f <= k[0].t {
                    return k[0].s.clone();
                }
                for w in k.windows(2) {
                    let (a, b) = (&w[0], &w[1]);
                    if f < b.t {
                        if a.hold || b.t <= a.t {
                            return a.s.clone();
                        }
                        let e = ease((f - a.t) / (b.t - a.t), a.o, a.i);
                        return a.s.iter().zip(&b.s).map(|(x, y)| x + (y - x) * e).collect();
                    }
                }
                k[k.len() - 1].s.clone()
            }
        }
    }
    pub fn at1(&self, f: f32, d: f32) -> f32 {
        self.at(f).first().copied().unwrap_or(d)
    }
}

pub struct Xform {
    a: Prop,
    p: Option<Prop>,
    px: Option<Prop>,
    py: Option<Prop>,
    s: Prop,
    r: Prop,
    pub o: Prop,
}

pub fn xform(v: &Value) -> Xform {
    let split = v["p"]["s"].as_bool() == Some(true);
    Xform {
        a: prop(v.get("a"), &[0.0, 0.0]),
        p: (!split).then(|| prop(v.get("p"), &[0.0, 0.0])),
        px: split.then(|| prop(v["p"].get("x"), &[0.0])),
        py: split.then(|| prop(v["p"].get("y"), &[0.0])),
        s: prop(v.get("s"), &[100.0, 100.0]),
        r: prop(v.get("r"), &[0.0]),
        o: prop(v.get("o"), &[100.0]),
    }
}

impl Xform {
    pub fn at(&self, f: f32) -> Transform {
        let a = self.a.at(f);
        let (px, py) = match (&self.p, &self.px, &self.py) {
            (Some(p), _, _) => {
                let p = p.at(f);
                (p.first().copied().unwrap_or(0.0), p.get(1).copied().unwrap_or(0.0))
            }
            (None, Some(x), Some(y)) => (x.at1(f, 0.0), y.at1(f, 0.0)),
            _ => (0.0, 0.0),
        };
        let s = self.s.at(f);
        let (sx, sy) = (
            s.first().copied().unwrap_or(100.0) / 100.0,
            s.get(1).copied().unwrap_or(100.0) / 100.0,
        );
        Transform::from_translate(px, py)
            .pre_rotate(self.r.at1(f, 0.0))
            .pre_scale(sx, sy)
            .pre_translate(-a.first().copied().unwrap_or(0.0), -a.get(1).copied().unwrap_or(0.0))
    }
}

pub enum Item {
    Group(Vec<Item>, Xform),
    Ellipse { p: Prop, s: Prop },
    Rect { p: Prop, s: Prop, r: Prop },
    Path(Value),
    Fill { c: Prop, o: Prop },
    Stroke { c: Prop, o: Prop, w: Prop },
}

pub fn items(v: &Value) -> Vec<Item> {
    let mut out = Vec::new();
    for it in v.as_array().into_iter().flatten() {
        match it["ty"].as_str().unwrap_or("") {
            "gr" => {
                let list = it["it"].as_array().cloned().unwrap_or_default();
                let tr = list.iter().find(|x| x["ty"] == "tr").map(xform);
                let tr = tr.unwrap_or_else(|| xform(&Value::Null));
                out.push(Item::Group(items(&it["it"]), tr));
            }
            "el" => out.push(Item::Ellipse { p: prop(it.get("p"), &[0.0, 0.0]), s: prop(it.get("s"), &[0.0, 0.0]) }),
            "rc" => out.push(Item::Rect {
                p: prop(it.get("p"), &[0.0, 0.0]),
                s: prop(it.get("s"), &[0.0, 0.0]),
                r: prop(it.get("r"), &[0.0]),
            }),
            "sh" => out.push(Item::Path(it["ks"]["k"].clone())),
            "fl" => out.push(Item::Fill { c: prop(it.get("c"), &[0.0, 0.0, 0.0, 1.0]), o: prop(it.get("o"), &[100.0]) }),
            "st" => out.push(Item::Stroke {
                c: prop(it.get("c"), &[0.0, 0.0, 0.0, 1.0]),
                o: prop(it.get("o"), &[100.0]),
                w: prop(it.get("w"), &[1.0]),
            }),
            _ => {}
        }
    }
    out
}

fn path_from(v: &Value, pb: &mut PathBuilder) {
    // a static bezier {c, v, i, o}, or the first keyframe's
    let v = if v.is_array() { &v[0]["s"][0] } else { v };
    let pts = |k: &str| -> Vec<(f32, f32)> {
        v[k].as_array().into_iter().flatten().map(|p| (first(&p[0]), first(&p[1]))).collect()
    };
    let (vs, is, os) = (pts("v"), pts("i"), pts("o"));
    if vs.is_empty() || is.len() != vs.len() || os.len() != vs.len() {
        return;
    }
    pb.move_to(vs[0].0, vs[0].1);
    let n = vs.len();
    let closed = v["c"].as_bool() == Some(true);
    let segs = if closed { n } else { n - 1 };
    for k in 0..segs {
        let (a, b) = (k, (k + 1) % n);
        pb.cubic_to(
            vs[a].0 + os[a].0, vs[a].1 + os[a].1,
            vs[b].0 + is[b].0, vs[b].1 + is[b].1,
            vs[b].0, vs[b].1,
        );
    }
    if closed {
        pb.close();
    }
}

/// Geometry collected so far in a group: (path, transform).
type Geo = Vec<(tiny_skia::Path, Transform)>;

fn color(c: &[f32], o: f32, alpha: f32) -> tiny_skia::Color {
    let g = |i: usize| c.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
    tiny_skia::Color::from_rgba(g(0), g(1), g(2), (o / 100.0 * alpha).clamp(0.0, 1.0))
        .unwrap_or(tiny_skia::Color::BLACK)
}

/// Draws `list` (Lottie order: the first item is on top, so styles are applied bottom-up).
fn draw_items(list: &[Item], f: f32, t: Transform, alpha: f32, pm: &mut Pixmap, geo_out: &mut Geo, stats: &mut u32) {
    let mut geo: Geo = Vec::new();
    // Collect geometry per index, so a style at j paints all geometry before j.
    let mut draws: Vec<(usize, &Item)> = Vec::new();
    for (j, it) in list.iter().enumerate() {
        match it {
            Item::Group(children, x) => {
                let gt = t.pre_concat(x.at(f));
                let ga = alpha * x.o.at1(f, 100.0) / 100.0;
                let mut sub = Vec::new();
                draw_items(children, f, gt, ga, pm, &mut sub, stats);
                geo.extend(sub);
            }
            Item::Ellipse { p, s } => {
                let (p, s) = (p.at(f), s.at(f));
                let (cx, cy) = (p.first().copied().unwrap_or(0.0), p.get(1).copied().unwrap_or(0.0));
                let (w, h) = (s.first().copied().unwrap_or(0.0), s.get(1).copied().unwrap_or(0.0));
                if let Some(r) = tiny_skia::Rect::from_xywh(cx - w / 2.0, cy - h / 2.0, w, h) {
                    if let Some(pth) = PathBuilder::from_oval(r) {
                        geo.push((pth, t));
                    }
                }
            }
            Item::Rect { p, s, .. } => {
                let (p, s) = (p.at(f), s.at(f));
                let (cx, cy) = (p.first().copied().unwrap_or(0.0), p.get(1).copied().unwrap_or(0.0));
                let (w, h) = (s.first().copied().unwrap_or(0.0), s.get(1).copied().unwrap_or(0.0));
                if let Some(r) = tiny_skia::Rect::from_xywh(cx - w / 2.0, cy - h / 2.0, w, h) {
                    geo.push((PathBuilder::from_rect(r), t));
                }
            }
            Item::Path(v) => {
                let mut pb = PathBuilder::new();
                path_from(v, &mut pb);
                if let Some(pth) = pb.finish() {
                    geo.push((pth, t));
                }
            }
            Item::Fill { .. } | Item::Stroke { .. } => draws.push((j, it)),
        }
        let _ = j;
    }
    // Styles: paint the geometry gathered (in this prototype: all geometry of the group).
    for (_, it) in draws.iter().rev() {
        let mut paint = Paint::default();
        paint.anti_alias = true;
        match it {
            Item::Fill { c, o } => {
                paint.set_color(color(&c.at(f), o.at1(f, 100.0), alpha));
                for (pth, tr) in geo.iter().rev() {
                    pm.fill_path(pth, &paint, FillRule::Winding, *tr, None);
                    *stats += 1;
                }
            }
            Item::Stroke { c, o, w } => {
                paint.set_color(color(&c.at(f), o.at1(f, 100.0), alpha));
                let st = Stroke { width: w.at1(f, 1.0), ..Stroke::default() };
                for (pth, tr) in geo.iter().rev() {
                    pm.stroke_path(pth, &paint, &st, *tr, None);
                    *stats += 1;
                }
            }
            _ => {}
        }
    }
    geo_out.extend(geo);
}

pub struct Layer {
    ind: i64,
    parent: Option<i64>,
    ip: f32,
    op: f32,
    ks: Xform,
    shapes: Vec<Item>,
}

pub struct Anim {
    pub w: f32,
    pub h: f32,
    pub fr: f32,
    pub ip: f32,
    pub op: f32,
    pub layers: Vec<Layer>,
    pub markers: Vec<(String, f32, f32)>,
}

pub fn parse(bytes: &[u8]) -> Result<Anim, String> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let layers = v["layers"].as_array().ok_or("no layers")?;
    let layers = layers
        .iter()
        .map(|l| Layer {
            ind: l["ind"].as_i64().unwrap_or(-1),
            parent: l["parent"].as_i64(),
            ip: l["ip"].as_f64().unwrap_or(0.0) as f32,
            op: l["op"].as_f64().unwrap_or(1e9) as f32,
            ks: xform(&l["ks"]),
            shapes: if l["ty"].as_i64() == Some(4) { items(&l["shapes"]) } else { vec![] },
        })
        .collect();
    let markers = v["markers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| (m["cm"].as_str().unwrap_or("").to_string(), m["tm"].as_f64().unwrap_or(0.0) as f32, m["dr"].as_f64().unwrap_or(0.0) as f32))
        .collect();
    Ok(Anim {
        w: v["w"].as_f64().unwrap_or(512.0) as f32,
        h: v["h"].as_f64().unwrap_or(512.0) as f32,
        fr: v["fr"].as_f64().unwrap_or(30.0) as f32,
        ip: v["ip"].as_f64().unwrap_or(0.0) as f32,
        op: v["op"].as_f64().unwrap_or(60.0) as f32,
        layers,
        markers,
    })
}

impl Anim {
    fn layer_t(&self, i: usize, f: f32, depth: u32) -> Transform {
        let l = &self.layers[i];
        let own = l.ks.at(f);
        match l.parent.and_then(|p| self.layers.iter().position(|x| x.ind == p)) {
            Some(pi) if depth < 32 && pi != i => self.layer_t(pi, f, depth + 1).pre_concat(own),
            _ => own,
        }
    }

    /// Draw frame `f` with `base` mapping comp space to pixels. Returns paint calls.
    pub fn draw(&self, f: f32, base: Transform, pm: &mut Pixmap) -> u32 {
        let mut stats = 0;
        for i in (0..self.layers.len()).rev() {
            let l = &self.layers[i];
            if f < l.ip || f >= l.op || l.shapes.is_empty() {
                continue;
            }
            let t = base.pre_concat(self.layer_t(i, f, 0));
            let a = l.ks.o.at1(f, 100.0) / 100.0;
            let mut g = Vec::new();
            draw_items(&l.shapes, f, t, a, pm, &mut g, &mut stats);
        }
        stats
    }
}
