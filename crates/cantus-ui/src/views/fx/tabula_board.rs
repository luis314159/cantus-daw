//! Tabula's board (ADR 0131 §5): where the pedals stand and how they are drawn — a top-down
//! board of real enclosures (`pedal_draw`), each unit wearing its own finish, the cables
//! carrying the signal as bright as it is at that point, the A/B split's splitter and
//! mixer boxes, an empty place to add a pedal, and the pedal picker drawn as a shelf of
//! small pedals.
//!
//! The board is the window's display: dark in every theme (it is drawn in the dark
//! theme's tokens), the pedals' paint their own.

use super::canvas;
use super::fragor;
use super::pedal_draw::{self as pd, Enclosure, Knob};
use super::tabula_faces::{self as faces, Face};
use crate::kit::color::{ensure_contrast, mix, with_alpha, Hsl};
use crate::kit::skin::pedals::{self as pal, Finish, Shape};
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::fx::tabula::{Path, Pedal, PedalId, PedalShape};
use cantus_core::{EffectKind, EffectSpec};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};

/// The kit's shape of a pedal: the user's choice, else its unit's natural one.
pub fn shape_of(p: &Pedal) -> Shape {
    let s = p
        .shape
        .or_else(|| faces::of(p.spec.kind).map(|f| f.shape))
        .unwrap_or(PedalShape::Standard);
    kit_shape(s)
}

pub fn kit_shape(s: PedalShape) -> Shape {
    match s {
        PedalShape::Mini => Shape::Mini,
        PedalShape::Standard => Shape::Standard,
        PedalShape::Wide => Shape::Wide,
        PedalShape::Double => Shape::Double,
        PedalShape::Round => Shape::Round,
    }
}

/// The Fragor model a Fragor pedal shows on the board: its first slot's (the green
/// overdrive when its chain is empty).
fn fragor_model(spec: &EffectSpec) -> usize {
    fragor::model_of(spec.get(cantus_core::fx::fragor::SLOTS[0])).unwrap_or(0)
}

/// A unit's finish on the board: Fragor's pedals their own; every other unit painted from
/// its own face (ADR 0074) — its surface's hue as the enamel, its accent as the band —
/// each kind a shade of its own so two units of one family never look alike.
pub fn finish_of(kind: EffectKind, spec: &EffectSpec) -> Finish {
    if kind == EffectKind::Pedal {
        return pal::FRAGOR[fragor_model(spec)];
    }
    let sk = crate::kit::skin::of_effect(kind);
    let i = EffectKind::ALL.iter().position(|k| *k == kind).unwrap_or(0) as f32;
    let shade = ((i * 37.0) % 9.0 - 4.0) / 4.0; // −1 … 1, by kind
    let h = (sk.surface.hue + shade * 14.0).rem_euclid(360.0);
    let sat = (sk.surface.sat * 1.5 + 0.18).clamp(0.22, 0.62);
    let body = Hsl {
        h,
        s: sat,
        l: 0.27 + 0.05 * shade,
    };
    let accent = sk.accent.map(|a| a.dark).unwrap_or(Hsl {
        h: (h + 30.0) % 360.0,
        s: 0.6,
        l: 0.62,
    });
    let face = faces::of(kind);
    Finish {
        body,
        band: Hsl {
            l: accent.l.max(0.58),
            ..accent
        },
        ink: Hsl {
            h,
            s: 0.18,
            l: 0.94,
        },
        cap: Hsl {
            h,
            s: 0.08,
            l: 0.11,
        },
        led: Hsl {
            h: accent.h,
            s: 0.95,
            l: 0.58,
        },
        metallic: face.is_some_and(|f| f.texture == faces::Texture::Brushed),
    }
}

fn motif_of(f: &Face) -> pd::Motif {
    match f.motif {
        faces::Motif::None => pd::Motif::Plain,
        faces::Motif::Wave => pd::Motif::Sine,
        faces::Motif::Echoes => pd::Motif::Echoes,
        faces::Motif::Rings => pd::Motif::Rings,
        faces::Motif::Clip => pd::Motif::Square,
        faces::Motif::Needle => pd::Motif::Needle,
        faces::Motif::Curve => pd::Motif::Curve,
        faces::Motif::Steps => pd::Motif::Steps,
        faces::Motif::Spiral => pd::Motif::Spiral,
        faces::Motif::Leap => pd::Motif::Leap,   // fx3:saltus
        faces::Motif::Wheel => pd::Motif::Wheel, // fx3:rota
        faces::Motif::Chest => pd::Motif::Chest, // fx3:arca
    }
}

fn letter_of(f: &Face) -> pd::Letter {
    match f.motif {
        faces::Motif::Wave | faces::Motif::Spiral | faces::Motif::Wheel => pd::Letter::Script, // fx3:rota
        faces::Motif::Echoes => pd::Letter::Wide,
        faces::Motif::Rings | faces::Motif::Curve => pd::Letter::Deco,
        faces::Motif::Clip | faces::Motif::Chest => pd::Letter::Bold, // fx3:arca
        _ => pd::Letter::Technical,
    }
}

fn texture_of(f: &Face) -> pd::Texture {
    match f.texture {
        faces::Texture::Smooth => pd::Texture::Sparkle,
        faces::Texture::Hammered => pd::Texture::Hammer,
        faces::Texture::Brushed => pd::Texture::Brushed,
        faces::Texture::Wrinkle => pd::Texture::Crinkle,
    }
}

/// The short word a pedal prints under a knob (the parameter's own name is long: it
/// carries its unit's, for lanes and menus).
pub fn knob_word(key: &str) -> Option<&'static str> {
    Some(match key {
        "time_ms" => "tabula.k.time",
        "feedback" => "tabula.k.repeats",
        "fb" => "tabula.k.intensity",
        "mix" => "tabula.k.mix",
        "rate" => "tabula.k.rate",
        "depth" => "tabula.k.depth",
        "drive" => "tabula.k.drive",
        "tone" => "tabula.k.tone",
        "space" => "tabula.k.size",
        "decay" => "tabula.k.decay",
        "thr" => "tabula.k.threshold",
        "ratio" => "tabula.k.ratio",
        "att" | "attack" => "tabula.k.attack",
        "rel" => "tabula.k.release",
        "gain" => "tabula.k.gain",
        "out" => "tabula.k.level",
        "width" => "tabula.k.width",
        "range" => "tabula.k.range",
        "div" => "tabula.k.division",
        "knee" => "tabula.k.knee",
        "sustain" => "tabula.k.sustain",
        "key" => "tabula.k.key",
        "scale" => "tabula.k.scale",
        "a.cut" => "tabula.k.cutoff",
        "a.res" => "tabula.k.resonance",
        "tslot" => "tabula.k.time_pattern",
        "vslot" => "tabula.k.volume_pattern",
        "wow" => "tabula.k.wow",
        "flut" => "tabula.k.flutter",
        "hiss" => "tabula.k.hiss",
        "pan" => "tabula.k.pan",
        "t.gain" => "tabula.k.attack",
        "b.gain" => "tabula.k.body",
        "b0.gain" => "tabula.k.low",
        "b1.gain" => "tabula.k.middle",
        "b2.gain" => "tabula.k.high",
        // fx3:saltus — Saltus's knobs.
        "st1" => "saltus.l.interval",
        "lvl1" => "saltus.l.level",
        "dry" => "saltus.l.dry",
        "pedal" => "saltus.l.pedal",
        // fx3:arca — Arca's knobs.
        "bass" => "arca.l.bass",
        "treble" => "arca.l.treble",
        "master" => "arca.l.master",
        // fx3:rota — Rota's knobs.
        "speed" => "rota.d.speed",
        "balance" => "rota.l.balance",
        _ => return None,
    })
}

/// A parameter's position on its knob, 0 … 1 (along its taper).
pub fn norm(spec: &EffectSpec, key: &str) -> f32 {
    let Some(i) = spec.kind.param(key) else {
        return 0.0;
    };
    let v = spec.get(key);
    let x = if i.logarithmic && i.min > 0.0 && i.max > i.min {
        (v / i.min).max(1e-9).ln() / (i.max / i.min).ln()
    } else if i.max > i.min {
        (v - i.min) / (i.max - i.min)
    } else {
        0.0
    };
    x.clamp(0.0, 1.0)
}

/// The knobs a pedal shows (its unit's face; a Fragor its model's three), with the words
/// under them, into `out`.
pub fn knobs_of<'a>(spec: &EffectSpec, hot: Option<usize>, out: &mut Vec<Knob<'a>>) {
    out.clear();
    if spec.kind == EffectKind::Pedal {
        let m = fragor_model(spec);
        let labels = fragor::knob_labels(m);
        for (j, key) in cantus_core::fx::fragor::KNOBS[m].iter().enumerate() {
            out.push(Knob {
                label: Src::Key(labels[j]),
                value: spec.get(key),
                hot: hot == Some(j),
            });
        }
        return;
    }
    let Some(face) = faces::of(spec.kind) else {
        return;
    };
    for (j, key) in face.knobs.iter().enumerate() {
        let label = match knob_word(key) {
            Some(w) => Src::Key(w),
            None => Src::Key(spec.kind.param(key).map_or("", |i| i.label_key)),
        };
        out.push(Knob {
            label,
            value: norm(spec, key),
            hot: hot == Some(j),
        });
    }
}

/// The keys of the knobs a pedal shows, in their order on its face.
pub fn knob_keys(spec: &EffectSpec) -> Vec<&'static str> {
    if spec.kind == EffectKind::Pedal {
        return cantus_core::fx::fragor::KNOBS[fragor_model(spec)].to_vec();
    }
    faces::of(spec.kind).map_or(Vec::new(), |f| f.knobs.to_vec())
}

/// A knob's position (0 … 1, along the taper) as the parameter's value.
pub fn denorm(i: &cantus_core::EffectParamInfo, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let v = if i.logarithmic && i.min > 0.0 && i.max > i.min {
        i.min * (i.max / i.min).powf(x)
    } else {
        i.min + (i.max - i.min) * x
    };
    if i.stepped {
        v.round()
    } else {
        v
    }
}

/// The name a pedal letters and the line under it.
pub fn names_of(spec: &EffectSpec) -> (Src<'static>, Src<'static>) {
    if spec.kind == EffectKind::Pedal {
        let m = fragor_model(spec);
        return (
            Src::Key(fragor::NAMES[m]),
            Src::Key("picker.dev.fragor.name"),
        );
    }
    let slug = crate::picker_catalog::slug_of(&crate::picker::PickChoice::Effect(spec.kind));
    let name = slug.map_or(spec.kind.name_key(), |s| {
        leak_key("picker.dev.", s, ".name")
    });
    let what = slug.map_or("", |s| leak_key("picker.dev.", s, ".what"));
    (Src::Key(name), Src::Key(what))
}

/// A locale key built once per unit and kept (the board asks for the same few each frame).
fn leak_key(a: &str, slug: &'static str, b: &str) -> &'static str {
    use std::sync::Mutex;
    static KEYS: Mutex<Vec<(String, &'static str)>> = Mutex::new(Vec::new());
    let want = format!("{a}{slug}{b}");
    let mut keys = KEYS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, k)) = keys.iter().find(|(w, _)| *w == want) {
        return k;
    }
    let k: &'static str = Box::leak(want.clone().into_boxed_str());
    keys.push((want, k));
    k
}

/// What to draw of one pedal this frame.
pub struct Look<'a> {
    pub spec: &'a EffectSpec,
    pub shape: Shape,
    pub on: bool,
    pub glow: f32,
    pub lifted: bool,
    pub hot: Option<usize>,
    pub selected: bool,
    /// Knobs without their words (a pedal drawn small: the picker's shelf).
    pub bare: bool,
}

/// One pedal at `r`, as its unit's face: what the editor hits.
pub fn pedal(b: &mut Brush, s: &Style, r: Rect, l: &Look<'_>) -> pd::Hits {
    let kind = l.spec.kind;
    let finish = finish_of(kind, l.spec);
    let mut knobs = Vec::with_capacity(4);
    knobs_of(l.spec, l.hot, &mut knobs);
    if l.bare {
        for kn in &mut knobs {
            kn.label = Src::Str("");
        }
    }
    let (name, sub) = if l.bare {
        (Src::Str(""), Src::Str(""))
    } else {
        names_of(l.spec)
    };
    let (motif, letter, texture) = if kind == EffectKind::Pedal {
        let m = fragor_model(l.spec);
        (fragor::MOTIFS[m], fragor::LETTERS[m], fragor::TEXTURES[m])
    } else {
        match faces::of(kind) {
            Some(f) => (motif_of(f), letter_of(f), texture_of(f)),
            None => (
                pd::Motif::Plain,
                pd::Letter::Technical,
                pd::Texture::Sparkle,
            ),
        }
    };
    let e = Enclosure {
        finish: &finish,
        name,
        sub,
        motif,
        letter,
        texture,
        shape: l.shape,
        knobs: &knobs,
        on: l.on,
        glow: if l.on { 0.25 + 0.75 * l.glow } else { 0.0 },
        lifted: l.lifted,
        switch_hot: l.hot == Some(9),
    };
    let hits = pd::enclosure(b, s, r, &e);
    if l.selected {
        let rr = if l.shape == Shape::Round {
            r.width() * 0.5 + 5.0 * s.k
        } else {
            16.0 * s.k
        };
        b.rrect_stroke(
            r.expand(5.0 * s.k),
            rr,
            1.5 * s.k,
            with_alpha(s.t.highlight, 0.9),
        );
    }
    hits
}

// ---- the layout --------------------------------------------------------------------------

/// One pedal to place: its id, its lane and its natural size (100 %).
#[derive(Copy, Clone, Debug)]
pub struct Item {
    pub id: PedalId,
    pub path: Path,
    pub size: Vec2,
}

/// Where everything stands on the board.
#[derive(Clone, Debug)]
pub struct Geo {
    pub pedals: Vec<(PedalId, Rect)>,
    /// The split's boxes: the splitter and the mixer.
    pub splitter: Option<Rect>,
    pub mixer: Option<Rect>,
    /// The lanes' rows (an empty lane shows a place to drop a pedal).
    pub lane_a: Option<Rect>,
    pub lane_b: Option<Rect>,
    /// The place to add a pedal, at the chain's end.
    pub add: Rect,
    pub jack_in: Pos2,
    pub jack_out: Pos2,
    /// The pedals' scale (the window's `k` at most; a floor of 60 % of it).
    pub scale: f32,
    /// The content's width when it is wider than the board (then it scrolls).
    pub overflow: Option<f32>,
    /// The rows' bands, top to bottom (one unless the chain wraps).
    pub rows: Vec<Rect>,
}

impl Default for Geo {
    fn default() -> Self {
        Geo {
            pedals: Vec::new(),
            splitter: None,
            mixer: None,
            lane_a: None,
            lane_b: None,
            add: Rect::NOTHING,
            jack_in: Pos2::ZERO,
            jack_out: Pos2::ZERO,
            scale: 1.0,
            overflow: None,
            rows: Vec::new(),
        }
    }
}

/// Room between two things on the board, at 100 %.
pub const GAP: f32 = 34.0;
/// A split box's width, and the room between the lanes, at 100 %.
pub const BOX_W: f32 = 78.0;
pub const LANE_GAP: f32 = 58.0;
/// An empty lane's place, and the add place, at 100 %.
pub const EMPTY_LANE: Vec2 = Vec2::new(150.0, 150.0);
pub const ADD: Vec2 = Vec2::new(136.0, 220.0);
/// The legible floor: pedals never drawn under this share of the window's scale.
pub const FLOOR: f32 = 0.6;
/// How large pedals may stand when the board has room: they fill its height.
pub const MAX_SCALE: f32 = 1.45;
/// Room between two rows of a wrapped chain, at 100 % (the cable runs through it).
pub const ROW_GAP: f32 = 70.0;

/// Lay the board out in `r`: Pre, then the split (a splitter, lane A over lane B, a
/// mixer), then Post and the place to add one — in one row, or wrapped into two or three
/// when that lets the pedals stand larger (up to [`MAX_SCALE`] of the window's scale, so
/// they fill the board's height); scaled down as far as the floor, then scrolled sideways.
/// Never clipped.
pub fn layout(r: Rect, k: f32, items: &[Item], scroll: f32) -> Geo {
    #[derive(Copy, Clone)]
    enum Unit {
        Pedal(usize),
        Split,
        Add,
    }
    let on = |p: Path| items.iter().enumerate().filter(move |(_, i)| i.path == p);
    let lane = |p: Path| {
        let n = on(p).count();
        if n == 0 {
            (EMPTY_LANE.x, EMPTY_LANE.y)
        } else {
            (
                on(p).map(|(_, i)| i.size.x).sum::<f32>() + GAP * (n - 1) as f32,
                on(p).map(|(_, i)| i.size.y).fold(0.0, f32::max),
            )
        }
    };
    let split = on(Path::A).next().is_some() || on(Path::B).next().is_some();
    let (wa, ha) = lane(Path::A);
    let (wb, hb) = lane(Path::B);
    let lanes_w = wa.max(wb);
    let lanes_h = ha + LANE_GAP + hb;
    let mut units: Vec<Unit> = on(Path::Pre).map(|(j, _)| Unit::Pedal(j)).collect();
    if split {
        units.push(Unit::Split);
    }
    units.extend(on(Path::Post).map(|(j, _)| Unit::Pedal(j)));
    units.push(Unit::Add);
    let size = |u: Unit| match u {
        Unit::Pedal(j) => items[j].size,
        Unit::Split => vec2(BOX_W + GAP + lanes_w + GAP + BOX_W, lanes_h),
        Unit::Add => ADD,
    };
    let inner = Rect::from_min_max(
        pos2(r.left() + 58.0 * k, r.top() + 46.0 * k),
        pos2(r.right() - 58.0 * k, r.bottom() - 40.0 * k),
    );
    // The rows for `n`: units in order, each row about the total width over `n`.
    let rows_for = |n: usize| -> Vec<Vec<Unit>> {
        let total: f32 = units.iter().map(|u| size(*u).x + GAP).sum::<f32>() - GAP;
        let target = total / n as f32;
        let mut rows: Vec<Vec<Unit>> = vec![Vec::new()];
        let mut w = 0.0;
        for u in &units {
            let uw = size(*u).x;
            let count = rows.len();
            let last = rows.last_mut().expect("a row");
            if !last.is_empty() && w + GAP + uw * 0.5 > target && count < n {
                rows.push(vec![*u]);
                w = uw;
            } else {
                if !last.is_empty() {
                    w += GAP;
                }
                w += uw;
                rows.last_mut().expect("a row").push(*u);
            }
        }
        rows
    };
    let metrics = |rows: &[Vec<Unit>]| {
        let widths: Vec<f32> = rows
            .iter()
            .map(|r| r.iter().map(|u| size(*u).x).sum::<f32>() + GAP * (r.len() - 1) as f32)
            .collect();
        let heights: Vec<f32> = rows
            .iter()
            .map(|r| r.iter().map(|u| size(*u).y).fold(0.0, f32::max))
            .collect();
        let w = widths.iter().copied().fold(0.0, f32::max);
        let h = heights.iter().sum::<f32>() + ROW_GAP * (rows.len() - 1) as f32;
        (widths, heights, w, h)
    };
    let fit_of = |w: f32, h: f32| {
        (inner.width() / w)
            .min(inner.height() / h)
            .min(MAX_SCALE * k)
    };
    let mut best: Option<(f32, Vec<Vec<Unit>>)> = None;
    for n in 1..=3usize.min(units.len()) {
        let rows = rows_for(n);
        let (_, _, w, h) = metrics(&rows);
        let f = fit_of(w, h);
        // A wrap is taken only for a clear gain: one row reads the chain best.
        if best.as_ref().is_none_or(|(bf, _)| f > *bf * 1.12) {
            best = Some((f, rows));
        }
    }
    let (fit, rows) = best.expect("one row at least");
    let sc = fit.max(FLOOR * k);
    let (widths, heights, w, h) = metrics(&rows);
    let content = w * sc;
    let overflow = (content > inner.width() + 0.5).then_some(content);
    let shift = overflow.map_or(0.0, |c| (c - inner.width()) * scroll.clamp(0.0, 1.0));
    let mut g = Geo {
        scale: sc,
        overflow,
        ..Default::default()
    };
    let mut y = inner.center().y - h * sc * 0.5;
    for (ri, row) in rows.iter().enumerate() {
        let rh = heights[ri] * sc;
        let band = Rect::from_min_max(pos2(r.left(), y), pos2(r.right(), y + rh));
        g.rows.push(band);
        let mut x = match overflow {
            Some(_) => inner.left() - shift,
            None => inner.center().x - widths[ri] * sc * 0.5,
        };
        let has_split = row.iter().any(|u| matches!(u, Unit::Split));
        let yc = band.center().y;
        let place = |x: f32, size: Vec2| {
            let sz = size * sc;
            if has_split {
                Rect::from_min_size(pos2(x, yc - sz.y * 0.5), sz)
            } else {
                Rect::from_min_size(pos2(x, band.bottom() - sz.y), sz)
            }
        };
        for u in row {
            match *u {
                Unit::Pedal(j) => {
                    g.pedals.push((items[j].id, place(x, items[j].size)));
                }
                Unit::Add => g.add = place(x, ADD),
                Unit::Split => {
                    let top = yc - lanes_h * sc * 0.5;
                    let a_row = Rect::from_min_size(pos2(x, top), vec2(0.0, ha * sc));
                    let b_row = Rect::from_min_size(
                        pos2(x, top + (ha + LANE_GAP) * sc),
                        vec2(0.0, hb * sc),
                    );
                    let box_r = Rect::from_min_max(
                        pos2(x, a_row.center().y - 26.0 * sc),
                        pos2(x + BOX_W * sc, b_row.center().y + 26.0 * sc),
                    );
                    g.splitter = Some(box_r);
                    let lx0 = x + (BOX_W + GAP) * sc;
                    for (p, lr, lw) in [(Path::A, a_row, wa), (Path::B, b_row, wb)] {
                        let mut lx = lx0 + (lanes_w - lw) * sc * 0.5;
                        let mut any = false;
                        for (_, i) in on(p) {
                            let sz = i.size * sc;
                            g.pedals.push((
                                i.id,
                                Rect::from_min_size(pos2(lx, lr.bottom() - sz.y), sz),
                            ));
                            lx += (i.size.x + GAP) * sc;
                            any = true;
                        }
                        let area = Rect::from_min_max(
                            pos2(lx0, lr.top()),
                            pos2(lx0 + lanes_w * sc, lr.bottom()),
                        );
                        let lane_r = if any {
                            area
                        } else {
                            Rect::from_center_size(area.center(), EMPTY_LANE * sc)
                        };
                        if p == Path::A {
                            g.lane_a = Some(lane_r);
                        } else {
                            g.lane_b = Some(lane_r);
                        }
                    }
                    let mx = lx0 + (lanes_w + GAP) * sc;
                    g.mixer = Some(Rect::from_min_size(pos2(mx, box_r.top()), box_r.size()));
                }
            }
            x += (size(*u).x + GAP) * sc;
        }
        y += rh + ROW_GAP * sc;
    }
    let first = g.rows.first().copied().unwrap_or(inner);
    let last = g.rows.last().copied().unwrap_or(inner);
    let jack_y = |band: Rect, tallest: f32| {
        if rows.len() == 1 && split {
            band.center().y - tallest * sc * 0.5
        } else {
            band.top()
        }
    };
    g.jack_in = pos2(r.left() + 24.0 * k, jack_y(first, heights[0]));
    g.jack_out = pos2(
        r.right() - 24.0 * k,
        jack_y(last, heights[heights.len() - 1]),
    );
    if rows.len() == 1 && split {
        // Level with the main row's pedals' tops.
        let main_h = items
            .iter()
            .filter(|i| matches!(i.path, Path::Pre | Path::Post))
            .map(|i| i.size.y)
            .fold(ADD.y, f32::max);
        let y = first.center().y - main_h * sc * 0.5;
        g.jack_in.y = y;
        g.jack_out.y = y;
    }
    g
}

/// The row a point on the board stands in (a jack on a pedal's top edge is its row's).
pub fn row_of(g: &Geo, p: Pos2) -> usize {
    g.rows
        .iter()
        .position(|b| p.y >= b.top() - 2.0 && p.y <= b.bottom() + 2.0)
        .unwrap_or(0)
}

// ---- cables ----------------------------------------------------------------------------

/// A cable's end: the jack, and the way its plug leaves it.
#[derive(Copy, Clone, Debug)]
pub enum End {
    /// A jack on a top edge: the plug stands up out of it.
    Top(Pos2),
    /// A jack on a left side: the plug points left.
    Left(Pos2),
    /// A jack on a right side: the plug points right.
    Right(Pos2),
}

impl End {
    fn jack(self) -> Pos2 {
        match self {
            End::Top(p) | End::Left(p) | End::Right(p) => p,
        }
    }

    fn out(self) -> Vec2 {
        match self {
            End::Top(_) => vec2(0.0, -1.0),
            End::Left(_) => vec2(-1.0, 0.0),
            End::Right(_) => vec2(1.0, 0.0),
        }
    }
}

/// A patch cable from `a` to `z`, its signal flowing from `a` with `energy` (the level at
/// that point, 0 … 1): `pedal_draw`'s cable, its plugs in any of the three directions.
pub fn cable(b: &mut Brush, s: &Style, a: End, z: End, lift: f32, energy: Option<pd::Energy>) {
    if let (End::Top(p), End::Top(q)) = (a, z) {
        pd::cable_with(b, s, p, q, lift, energy);
        return;
    }
    let k = s.k;
    let plug = 12.0 * k;
    let (pa, pz) = (a.jack() + a.out() * plug, z.jack() + z.out() * plug);
    let reach = ((pz - pa).length() * 0.45).max(36.0 * k);
    let up = |e: End, p: Pos2, other: Pos2| match e {
        End::Top(_) => pos2(p.x, p.y.min(other.y) - lift),
        _ => p + e.out() * reach,
    };
    let (c1, c2) = (up(a, pa, pz), up(z, pz, pa));
    let mut pts = [Pos2::ZERO; 33];
    for (i, q) in pts.iter_mut().enumerate() {
        let t = i as f32 / 32.0;
        let u = 1.0 - t;
        *q = pos2(
            u * u * u * pa.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * pz.x,
            u * u * u * pa.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * pz.y,
        );
    }
    stroke(b, s, &pts, [a, z], energy);
}

/// A cable from a jack on one row to a jack on the next (a wrapped chain): up out of the
/// first, to the board's right side, down into the gap between the rows, along it and down
/// into the second — corners rounded, the signal flowing along it.
pub fn cable_wrapped(
    b: &mut Brush,
    s: &Style,
    g: &Geo,
    a: Pos2,
    z: Pos2,
    lift: f32,
    energy: Option<pd::Energy>,
) {
    let k = s.k;
    let ra = row_of(g, a);
    let gap_y = match (g.rows.get(ra), g.rows.get(ra + 1)) {
        (Some(x), Some(y)) => (x.bottom() + y.top()) * 0.5,
        _ => (a.y.max(z.y) + a.y.min(z.y)) * 0.5,
    };
    let a0 = a - vec2(0.0, 12.0 * k);
    let z0 = z - vec2(0.0, 12.0 * k);
    let xr = a.x + 46.0 * k;
    let mut pts: Vec<Pos2> = vec![
        a0,
        pos2(a0.x, a0.y - lift * 0.6),
        pos2(xr, a0.y - lift * 0.6),
        pos2(xr, gap_y),
        pos2(z0.x, gap_y),
        z0,
    ];
    // Corners cut three times (Chaikin): a cable's easy bends.
    for _ in 0..3 {
        let mut next = Vec::with_capacity(pts.len() * 2);
        next.push(pts[0]);
        for w in pts.windows(2) {
            let (p, q) = (w[0], w[1]);
            next.push(p + (q - p) * 0.25);
            next.push(p + (q - p) * 0.75);
        }
        next.push(*pts.last().expect("points"));
        pts = next;
    }
    stroke(b, s, &pts, [End::Top(a), End::Top(z)], energy);
}

/// A cable's body along `pts` (from plug to plug), with energy, and its two plugs.
fn stroke(b: &mut Brush, s: &Style, pts: &[Pos2], ends: [End; 2], energy: Option<pd::Energy>) {
    let k = s.k;
    let plug = 12.0 * k;
    let c = |h: Hsl| h.rgb();
    let shade: Vec<Pos2> = pts.iter().map(|q| *q + vec2(0.0, 5.0 * k)).collect();
    b.polyline(&shade, 6.0 * k, with_alpha(c(pal::SHADOW), 0.30));
    b.polyline(pts, 6.0 * k, c(pal::CABLE));
    let sheen: Vec<Pos2> = pts.iter().map(|q| *q + vec2(-1.0, -1.2) * k).collect();
    b.polyline(&sheen, 1.4 * k, with_alpha(c(pal::CABLE_SHEEN), 0.8));
    if let Some(e) = energy.filter(|e| e.level > 0.01) {
        use crate::views::mixer::wires;
        let l = e.level.clamp(0.0, 1.0);
        let amber = c(pal::JEWEL);
        let light = mix(amber, c(pal::CHROME_LIGHT), 0.55);
        b.polyline(pts, 2.4 * k, with_alpha(amber, 0.30 + 0.30 * l));
        let a_g = wires::gleam_alpha(l) * (0.6 + 0.4 * l);
        let o = wires::gleam_offset(e.time, l) * k;
        wires::gleams_of(pts, o, wires::GLEAM * 2.4 * k, |p, q| {
            b.segment(p, q, 5.0 * k, with_alpha(amber, 0.35 * a_g), true);
            b.segment(p, q, 2.4 * k, with_alpha(light, a_g), true);
        });
    }
    for e in ends {
        let j = e.jack();
        let d = e.out();
        let tip = j + d * plug;
        let n = vec2(-d.y, d.x) * 5.0 * k;
        let plug_r = Rect::from_two_pos(j + d * 3.0 * k - n, tip + n);
        b.rrect(
            plug_r,
            2.0 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
    }
}

/// A jack on a box's side or a board's edge: a nickel socket.
pub fn side_jack(b: &mut Brush, s: &Style, p: Pos2, vertical: bool) {
    let k = s.k;
    let c = |h: Hsl| h.rgb();
    let sz = if vertical {
        vec2(9.0, 18.0)
    } else {
        vec2(18.0, 9.0)
    } * k;
    let r = Rect::from_center_size(p, sz);
    b.rrect(r, 2.5 * k, Fill::V(c(pal::CHROME_LIGHT), c(pal::NICKEL)));
    b.rrect_stroke(r, 2.5 * k, b.px(), c(pal::CHROME_DARK));
}

/// A signal's level on a cable, 0 … 1, from a peak (linear): −60 dBFS to 0.
pub fn level(peak: f32) -> f32 {
    if peak <= 1e-6 {
        return 0.0;
    }
    ((20.0 * peak.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

// ---- the split's boxes -----------------------------------------------------------------

/// Where a split box's jacks are: its input(s) and output(s).
#[derive(Copy, Clone, Debug)]
pub struct BoxJacks {
    /// The splitter's input, or the mixer's output: the middle of a side.
    pub main: Pos2,
    /// Lane A's and lane B's jacks on the other side.
    pub a: Pos2,
    pub b: Pos2,
    /// The mixer's knob (the A/B balance), for its hits.
    pub knob: Option<Rect>,
}

fn words(src: Src<'_>, role: Role, caps: bool, color: Color32, align: Align2) -> Text<'_> {
    Text {
        src,
        role,
        caps,
        align,
        policy: Policy::Elide(Elide::End),
        color,
    }
}

/// A split box: a die-cast case with its signal paths printed on it. The splitter takes
/// one signal in on its left and gives it to lanes A and B on its right; the mixer takes
/// the lanes on its left and gives one out on its right, its knob the A/B balance (`ab`:
/// `None` for the splitter). `hot`: the knob under the pointer.
pub fn split_box(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    title: Src<'_>,
    ab: Option<f32>,
    hot: bool,
) -> BoxJacks {
    let k = s.k;
    let u = r.width() / BOX_W;
    let c = |h: Hsl| h.rgb();
    let f = &pal::GRAPHITE;
    b.rrect_soft(
        r.translate(vec2(0.0, 6.0 * u)),
        10.0 * u,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
        10.0 * u,
    );
    let hi = Hsl {
        l: f.body.l + 0.10,
        ..f.body
    };
    let lo = Hsl {
        l: f.body.l - 0.12,
        ..f.body
    };
    b.rrect(r, 10.0 * u, Fill::V(c(hi), c(lo)));
    super::pedal_art::texture(b, u, r, pd::Texture::Brushed, c(hi), c(lo), 4243);
    b.rrect_stroke(
        r.shrink(0.5 * b.px()),
        10.0 * u,
        b.px(),
        with_alpha(c(pal::CHROME_LIGHT), 0.7),
    );
    let ink = ensure_contrast(c(f.ink), c(f.body), 4.5);
    let mixer = ab.is_some();
    let (left, right) = (r.left(), r.right());
    let ya = r.top() + 28.0 * u;
    let yb = r.bottom() - 28.0 * u;
    let ym = r.center().y;
    // The single signal on the top edge (its cable hangs over the board like a pedal's),
    // the lanes on the side facing them.
    let (main, a, bj) = if mixer {
        (
            pos2(right - 16.0 * u, r.top()),
            pos2(left, ya),
            pos2(left, yb),
        )
    } else {
        (
            pos2(left + 16.0 * u, r.top()),
            pos2(right, ya),
            pos2(right, yb),
        )
    };
    // The paths printed on the case: one line forking into two (or two joining one).
    let node = pos2(r.center().x + if mixer { 8.0 } else { -8.0 } * u, ym);
    let fine = 1.1 * u;
    let path_c = with_alpha(ink, 0.75);
    let drop = pos2(main.x, r.top() + 14.0 * u);
    b.segment(main, drop, fine, path_c, false);
    b.segment(drop, pos2(main.x, ym), fine, path_c, false);
    b.segment(pos2(main.x, ym), node, fine, path_c, false);
    for (j, y) in [(a, ya), (bj, yb)] {
        let bend = pos2(node.x + if mixer { -14.0 } else { 14.0 } * u, y);
        b.segment(node, bend, fine, path_c, false);
        b.segment(bend, j, fine, path_c, false);
    }
    b.circle(node, 2.6 * u, Fill::Solid(c(f.band)));
    // The lanes' letters by their jacks; the title across the middle.
    let lx = if mixer {
        left + 14.0 * u
    } else {
        right - 14.0 * u
    };
    for (y, key) in [(ya, "tabula.lane.a"), (yb, "tabula.lane.b")] {
        canvas::put(
            b,
            pos2(lx, y - 13.0 * u),
            Align2::CENTER_CENTER,
            &words(Src::Key(key), Role::Label, true, ink, Align2::CENTER_CENTER),
            k,
            None,
        );
    }
    let mut knob = None;
    if let Some(v) = ab {
        let kp = pos2(r.center().x, ym + 34.0 * u);
        let kr = 15.0 * u;
        pd::knob(
            b,
            s,
            kp,
            kr,
            u,
            &Knob {
                label: Src::Key("tabula.mixer.knob"),
                value: v,
                hot,
            },
            f,
        );
        knob = Some(Rect::from_center_size(kp, vec2(kr * 2.6, kr * 2.6)));
        canvas::put(
            b,
            pos2(kp.x, kp.y + kr + 11.0 * u),
            Align2::CENTER_CENTER,
            &words(
                Src::Key("tabula.mixer.knob"),
                Role::Micro,
                true,
                ink,
                Align2::CENTER_CENTER,
            ),
            k,
            None,
        );
    }
    let tp = pos2(r.center().x, ym - 30.0 * u);
    ktext::paint(
        b,
        Rect::from_center_size(tp, vec2(r.width() - 8.0 * u, 13.0 * k)),
        &words(title, Role::Micro, true, ink, Align2::CENTER_CENTER),
        k,
    );
    let top_jack = Rect::from_center_size(main + vec2(0.0, -2.5 * u), vec2(20.0 * u, 8.0 * u));
    b.rrect(
        top_jack,
        2.0 * u,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::NICKEL)),
    );
    b.rrect_stroke(top_jack, 2.0 * u, b.px(), c(pal::CHROME_DARK));
    for p in [a, bj] {
        side_jack(b, s, p, false);
    }
    BoxJacks {
        main,
        a,
        b: bj,
        knob,
    }
}

// ---- the picker ------------------------------------------------------------------------

/// What the picker hit: a shelf, or a pedal on the shown shelf.
#[derive(Clone, Debug)]
pub struct PickerHits {
    pub panel: Rect,
    pub shelves: Vec<Rect>,
    pub pedals: Vec<(EffectKind, Rect)>,
}

/// The pedal picker over the board: the shelves down its left side (`shelf` lit), the
/// shelf's units as small pedals on the right — each in its own finish, its name and what
/// it is under it (`hover`: the pedal under the pointer).
#[allow(clippy::too_many_arguments)]
pub fn picker(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    title: Src<'_>,
    shelves: &[(Src<'_>, usize)],
    shelf: usize,
    kinds: &[EffectKind],
    hover: Option<usize>,
) -> PickerHits {
    let k = s.k;
    let t = &s.t;
    let mut hits = PickerHits {
        panel: r,
        shelves: Vec::new(),
        pedals: Vec::new(),
    };
    b.rrect_soft(
        r.translate(vec2(0.0, 8.0 * k)),
        8.0 * k,
        Fill::Solid(with_alpha(pal::SHADOW.rgb(), 0.45)),
        18.0 * k,
    );
    b.rrect(r, 8.0 * k, Fill::Solid(t.panel));
    b.rrect_stroke(r, 8.0 * k, b.px(), t.edge);
    let head = Rect::from_min_size(r.min, vec2(r.width(), 34.0 * k));
    canvas::put(
        b,
        pos2(head.left() + 14.0 * k, head.center().y),
        Align2::LEFT_CENTER,
        &words(title, Role::Title, false, t.text, Align2::LEFT_CENTER),
        k,
        None,
    );
    b.hline(r.left(), r.right(), head.bottom(), b.px(), t.edge);
    // The shelves.
    let col = Rect::from_min_max(
        pos2(r.left(), head.bottom()),
        pos2(r.left() + 168.0 * k, r.bottom()),
    );
    b.rect(col.shrink2(vec2(0.0, 0.0)), Fill::Solid(t.panel_raised));
    b.vline(col.right(), col.top(), col.bottom(), b.px(), t.edge);
    let row_h = 26.0 * k;
    for (i, (name, n)) in shelves.iter().enumerate() {
        let rr = Rect::from_min_size(
            pos2(col.left() + 6.0 * k, col.top() + 6.0 * k + i as f32 * row_h),
            vec2(col.width() - 12.0 * k, row_h - 2.0 * k),
        );
        let lit = i == shelf;
        if lit {
            b.rrect(rr, 4.0 * k, Fill::Solid(with_alpha(t.highlight, 0.16)));
            b.rect(
                Rect::from_min_size(rr.min, vec2(3.0 * k, rr.height())),
                Fill::Solid(t.highlight),
            );
        }
        let ink = if lit { t.text } else { t.text_dim };
        ktext::paint(
            b,
            Rect::from_min_max(
                pos2(rr.left() + 10.0 * k, rr.top()),
                pos2(rr.right() - 30.0 * k, rr.bottom()),
            ),
            &words(*name, Role::Label, false, ink, Align2::LEFT_CENTER),
            k,
        );
        let count = n.to_string();
        canvas::put(
            b,
            pos2(rr.right() - 8.0 * k, rr.center().y),
            Align2::RIGHT_CENTER,
            &words(
                Src::Str(&count),
                Role::Micro,
                false,
                t.text_dim,
                Align2::RIGHT_CENTER,
            ),
            k,
            None,
        );
        hits.shelves.push(rr);
    }
    // The shelf's pedals, small, four to a row.
    let area = Rect::from_min_max(
        pos2(col.right() + 14.0 * k, head.bottom() + 12.0 * k),
        pos2(r.right() - 14.0 * k, r.bottom() - 12.0 * k),
    );
    let per = 4usize;
    let cell_w = area.width() / per as f32;
    let cell_h = 168.0 * k;
    for (i, kind) in kinds.iter().enumerate() {
        let (cx, cy) = (i % per, i / per);
        let cell = Rect::from_min_size(
            pos2(
                area.left() + cx as f32 * cell_w,
                area.top() + cy as f32 * cell_h,
            ),
            vec2(cell_w, cell_h),
        );
        if cell.bottom() > r.bottom() + 1.0 {
            break;
        }
        let spec = EffectSpec::new(*kind);
        let shape = faces::of(*kind).map_or(Shape::Standard, |f| kit_shape(f.shape));
        let nat = pd::size_of(shape);
        let fit = ((cell.width() - 18.0 * k) / nat.x)
            .min((cell.height() - 46.0 * k) / nat.y)
            .min(0.42 * k);
        let pr = Rect::from_min_size(
            pos2(cell.center().x - nat.x * fit * 0.5, cell.top() + 4.0 * k),
            nat * fit,
        );
        let hot = hover == Some(i);
        if hot {
            b.rrect(cell.shrink(3.0 * k), 6.0 * k, Fill::Solid(t.panel_raised));
            b.rrect_stroke(cell.shrink(3.0 * k), 6.0 * k, b.px(), t.highlight);
        }
        pedal(
            b,
            s,
            pr,
            &Look {
                spec: &spec,
                shape,
                on: true,
                glow: 0.2,
                lifted: hot,
                hot: None,
                selected: false,
                bare: true,
            },
        );
        let (name, what) = names_of(&spec);
        let ny = pr.bottom() + 12.0 * k;
        ktext::paint(
            b,
            Rect::from_center_size(pos2(cell.center().x, ny), vec2(cell_w - 8.0 * k, 15.0 * k)),
            &words(name, Role::Label, false, t.text, Align2::CENTER_CENTER),
            k,
        );
        ktext::paint(
            b,
            Rect::from_center_size(
                pos2(cell.center().x, ny + 15.0 * k),
                vec2(cell_w - 8.0 * k, 14.0 * k),
            ),
            &words(what, Role::Micro, false, t.text_dim, Align2::CENTER_CENTER),
            k,
        );
        hits.pedals.push((*kind, cell));
    }
    hits
}

/// The board's own nameplate, engraved brass screwed to its lower right corner.
pub fn nameplate(b: &mut Brush, s: &Style, board: Rect) {
    let k = s.k;
    let c = |h: Hsl| h.rgb();
    let r = Rect::from_min_size(
        pos2(board.right() - 132.0 * k, board.bottom() - 34.0 * k),
        vec2(114.0 * k, 22.0 * k),
    );
    b.rrect_soft(
        r.translate(vec2(0.0, 2.0 * k)),
        3.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
        3.0 * k,
    );
    b.rrect(r, 3.0 * k, Fill::V(c(pal::BRASS_LIGHT), c(pal::BRASS)));
    b.rrect_stroke(r, 3.0 * k, b.px(), c(pal::BRASS_DARK));
    for x in [r.left() + 7.0 * k, r.right() - 7.0 * k] {
        b.circle(
            pos2(x, r.center().y),
            2.2 * k,
            Fill::V(c(pal::BRASS_LIGHT), c(pal::BRASS_DARK)),
        );
    }
    canvas::put(
        b,
        r.center(),
        Align2::CENTER_CENTER,
        &words(
            Src::Key("picker.dev.tabula.name"),
            Role::Label,
            true,
            c(pal::BRASS_DARK),
            Align2::CENTER_CENTER,
        ),
        k,
        None,
    );
}
