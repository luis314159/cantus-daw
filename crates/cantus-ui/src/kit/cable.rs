//! A routing cable, drawn by hand (round 25, ADR 0041; design §3.2, §6): a soft U hanging
//! below two ports, styled by kind — an output solid, a send thinner with its level on a
//! pill, a pre-fader send with a gap at its source end (its pill says *PRE*), a key dashed
//! in the dynamics hue — and the marks the desk's band and the Routing tab's matrix share
//! (the key glyph, the doubled-signal warning).
//!
//! **The shape.** A cable leaves its source port straight down, turns through a quarter
//! curve, runs flat at its depth and turns back up into the destination's port. The depth
//! grows with the span ([`depth_for`]) and the corners are as wide as the cable is deep,
//! so two cables from one port nest — the longer one runs lower and never crosses the
//! shorter (the corners of a capped pair coincide; they do not cross). Kinds differ by
//! stroke and end mark, never by colour alone: the band reads in grey scale.
//!
//! **Cost.** A cable is fourteen points in a stack array, stroked into the caller's brush:
//! no allocation, no draw call of its own (ADR 0024 §13).

use super::color::with_alpha;
use super::glyph::{self, Glyph};
use super::{Brush, Style};
use egui::{pos2, vec2, Color32, Pos2, Rect};

/// What a cable carries, for its stroke and its end mark.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CableKind {
    /// The strip's output to another strip: 2 pt solid, a filled dot at the end.
    Output,
    /// The selected strip's output to the master (the master route is implied for the
    /// others).
    Master,
    /// A send: 1.5 pt, a chevron at the end; dashed and faint when switched off; a gap at
    /// the source end when pre-fader.
    Send { on: bool, pre: bool },
    /// A sidechain key: 1.25 pt dashed in the dynamics hue, the key glyph at the end.
    Key,
}

/// How one cable looks this frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CableLook {
    pub kind: CableKind,
    /// The source strip's colour (a key uses the dynamics hue instead).
    pub color: Color32,
    /// Lit (the selection's, hovered, dragged) or faint (*All* mode's others).
    pub lit: bool,
    /// Selected: a 2-pt halo.
    pub selected: bool,
    /// A doubled signal (the owner's answer 5): the warning mark rides on it.
    pub doubled: bool,
    /// Where its end mark goes (the band slides it down the corner, or leaves it out where
    /// it would meet another's glyph); `None`: no mark.
    pub mark: Option<Pos2>,
    /// Lit by the hover but with no room of its own in the band: in its colour, thin, no
    /// mark — still reads as the hovered strip's.
    pub related: bool,
}

/// The tokens a cable paints with besides its strip's colour: the doubled mark's ground
/// and warning, the selection's halo, a key's hue.
const TOKENS: &[super::Tok] = &[
    super::Tok::Panel,
    super::Tok::Danger,
    super::Tok::Selection,
    super::Tok::HueDynamics,
];

/// The shallowest a cable hangs, in points at 100 %: a hop to the next strip is a hook.
pub const MIN_DEPTH: f32 = 5.0;
/// Depth per point of span, below the cap.
pub const SLOPE: f32 = 0.35;
/// Points per corner (every 15°).
const CORNER: usize = 7;
/// Points in a cable's path.
pub const POINTS: usize = 2 * CORNER;
/// cos of 0°, 15°, … 90° (read backwards, the sines): the corners are sampled without
/// trigonometry a frame.
const COS: [f32; CORNER] = [
    1.0,
    0.965_925_8,
    0.866_025_4,
    std::f32::consts::FRAC_1_SQRT_2,
    0.5,
    0.258_819,
    0.0,
];

/// How deep the arc between two ports hangs: proportional to their distance, capped by
/// the band's height, so cables from one strip nest instead of crossing.
pub fn depth_for(from: Pos2, to: Pos2, band_height: f32) -> f32 {
    let cap = band_height.max(0.0);
    let floor = MIN_DEPTH.min(cap);
    ((to.x - from.x).abs() * SLOPE).clamp(floor, cap.max(floor))
}

/// A cable's geometry: the sampled path and the places its marks go.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Path {
    /// From the source port to the destination port.
    pub pts: [Pos2; POINTS],
    /// The middle of the flat run: where a level pill sits.
    pub low: Pos2,
    /// Where the flat run starts (after the source's corner): the doubled-signal mark.
    pub run_start: Pos2,
    /// 30° up the destination's corner from its port: the end mark.
    pub mark: Pos2,
    /// 30° down the source's corner from its port: where the source end is grabbed.
    pub tail: Pos2,
}

/// The path of a cable from `from` to `to` hanging `depth` below the lower port.
pub fn path(from: Pos2, to: Pos2, depth: f32) -> Path {
    let dir = if to.x >= from.x { 1.0 } else { -1.0 };
    let span = (to.x - from.x).abs();
    let depth = depth.max(0.0);
    let bottom = from.y.max(to.y) + depth;
    let r = depth.min(span * 0.5);
    let (v0, v1) = (bottom - from.y, bottom - to.y);
    let mut pts = [Pos2::ZERO; POINTS];
    // The source's corner: centre (from.x + r, from.y), from straight down to flat.
    let c0 = from.x + dir * r;
    for i in 0..CORNER {
        let (cos, sin) = (COS[i], COS[CORNER - 1 - i]);
        pts[i] = pos2(c0 - dir * r * cos, from.y + v0 * sin);
    }
    // The destination's corner, from flat back up to the port.
    let c1 = to.x - dir * r;
    for i in 0..CORNER {
        let (cos, sin) = (COS[CORNER - 1 - i], COS[i]);
        pts[CORNER + i] = pos2(c1 + dir * r * cos, to.y + v1 * sin);
    }
    Path {
        pts,
        low: pos2((from.x + to.x) * 0.5, bottom),
        run_start: pos2(c0, bottom),
        mark: pos2(c1 + dir * r * COS[2], to.y + v1 * COS[CORNER - 3]),
        tail: pos2(c0 - dir * r * COS[2], from.y + v0 * COS[CORNER - 3]),
    }
}

/// The distance from `p` to the cable, in points (`f32::INFINITY` far away): the hit test.
pub fn hit(from: Pos2, to: Pos2, depth: f32, p: Pos2) -> f32 {
    let bottom = from.y.max(to.y) + depth.max(0.0);
    let bounds = Rect::from_min_max(
        pos2(from.x.min(to.x), from.y.min(to.y)),
        pos2(from.x.max(to.x), bottom),
    )
    .expand(12.0);
    if !bounds.contains(p) {
        return f32::INFINITY;
    }
    let path = path(from, to, depth);
    path.pts
        .windows(2)
        .map(|w| segment_distance(p, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The stroke width of a kind, lit, at 100 %.
pub fn width(kind: CableKind) -> f32 {
    match kind {
        CableKind::Output | CableKind::Master => 2.0,
        CableKind::Send { .. } => 1.5,
        CableKind::Key => 1.25,
    }
}

/// The colour a cable is stroked in: its source's, a key's in the dynamics hue; faint
/// cables and switched-off sends lose most of it.
pub fn ink(s: &Style, look: &CableLook) -> Color32 {
    let base = match look.kind {
        CableKind::Key => s.t.hue_dynamics,
        _ => look.color,
    };
    let mut a = if look.lit {
        1.0
    } else if look.related {
        0.8
    } else {
        0.35
    };
    if matches!(look.kind, CableKind::Send { on: false, .. }) {
        a *= 0.55;
    }
    with_alpha(base, a)
}

/// Stroke the cable from `from` to `to`, its arc `depth` deep, into the brush (no
/// allocation: the curve is sampled into a stack array).
pub fn paint(b: &mut Brush, s: &Style, from: Pos2, to: Pos2, depth: f32, look: &CableLook) {
    if (to.x - from.x).abs() < 0.5 && (to.y - from.y).abs() < 0.5 {
        return;
    }
    let path = path(from, to, depth);
    let bounds = Rect::from_min_max(
        pos2(from.x.min(to.x), from.y.min(to.y)),
        pos2(from.x.max(to.x), path.low.y),
    );
    b.register(bounds.expand(4.0), "cable", TOKENS, &[]);
    let color = ink(s, look);
    let w = if look.lit {
        s.s(width(look.kind))
    } else if look.related {
        (b.px() * 1.5).max(s.s(0.9))
    } else {
        b.px()
    };
    let dashed = matches!(
        look.kind,
        CableKind::Key | CableKind::Send { on: false, .. }
    );
    // A pre-fader send leaves a gap at its source end: the tap is before the fader.
    let mut pts = path.pts;
    if matches!(look.kind, CableKind::Send { pre: true, .. }) {
        let gap = s.s(3.0);
        let d = pts[1] - pts[0];
        let len = d.length();
        if len > gap {
            pts[0] += d * (gap / len);
        }
    }
    if look.selected {
        b.polyline(&pts, w + s.s(4.0), with_alpha(s.t.selection, 0.35));
    }
    if dashed {
        b.dashed(&pts, w, color, s.s(4.0), s.s(3.0));
    } else {
        b.polyline(&pts, w, color);
    }
    if let Some(at) = look.mark {
        let path = Path { mark: at, ..path };
        end_mark(b, s, &path, to, look, color);
    }
    if look.doubled {
        doubled_mark(b, s, path.run_start, s.s(10.0));
    }
}

/// The mark at a cable's destination end: a dot (an output), a chevron pointing into the
/// port (a send; hollow when it is off), the key glyph (a key).
fn end_mark(b: &mut Brush, s: &Style, path: &Path, to: Pos2, look: &CableLook, color: Color32) {
    let at = path.mark;
    let size = if look.lit { 1.0 } else { 0.8 };
    match look.kind {
        CableKind::Output | CableKind::Master => b.dot(at, s.s(2.6 * size), color),
        CableKind::Send { on, .. } => {
            let d = to - at;
            let f = d / d.length().max(1e-3);
            let n = vec2(-f.y, f.x);
            let k = s.s(3.6 * size);
            let tip = at + f * k;
            let l = at - f * (k * 0.6) + n * k;
            let r = at - f * (k * 0.6) - n * k;
            if on {
                b.polygon(&[tip, l, r], color);
            } else {
                let w = b.px() * 1.2;
                b.segment(tip, l, w, color, false);
                b.segment(l, r, w, color, false);
                b.segment(r, tip, w, color, false);
            }
        }
        CableKind::Key => key_mark(b, s, at, s.s(12.0 * size), color),
    }
}

/// The sidechain key glyph, `size` points tall, centred on `center`: a key cable's end
/// mark and the matrix's key cell.
pub fn key_mark(b: &mut Brush, s: &Style, center: Pos2, size: f32, color: Color32) {
    let _ = s;
    glyph::paint(
        b,
        Rect::from_center_size(center, vec2(size, size)),
        Glyph::Key,
        color,
    );
}

/// The doubled-signal warning mark (answer 5), centred on `center`: the warning
/// triangle in the danger colour on a disc of the ground, so it reads over a cable.
pub fn doubled_mark(b: &mut Brush, s: &Style, center: Pos2, size: f32) {
    b.dot(center, size * 0.62, with_alpha(s.t.panel, 0.92));
    glyph::paint(
        b,
        Rect::from_center_size(center, vec2(size, size)),
        Glyph::Warning,
        s.t.danger,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const Y: f32 = 100.0;

    /// The cable's depth at `x`, read off its sampled path (the flat run included).
    fn y_at(p: &Path, x: f32) -> Option<f32> {
        let mut pts: Vec<Pos2> = p.pts.to_vec();
        pts.sort_by(|a, b| a.x.total_cmp(&b.x));
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if x >= a.x && x <= b.x {
                if (b.x - a.x).abs() < 1e-4 {
                    return Some(a.y.max(b.y));
                }
                let t = (x - a.x) / (b.x - a.x);
                return Some(a.y + (b.y - a.y) * t);
            }
        }
        None
    }

    #[test]
    fn depth_grows_with_the_span_and_is_capped() {
        let a = pos2(0.0, Y);
        let d = |x: f32| depth_for(a, pos2(x, Y), 20.0);
        assert!(d(24.0) < d(48.0) && d(48.0) <= d(72.0));
        assert_eq!(d(10_000.0), 20.0, "capped by the band");
        assert!(d(1.0) >= MIN_DEPTH, "a hop is still a hook");
        // Symmetric: a cable to the left hangs as deep as one to the right.
        assert_eq!(depth_for(a, pos2(-60.0, Y), 20.0), d(60.0));
        // A band too short for the minimum hangs what it can.
        assert_eq!(depth_for(a, pos2(200.0, Y), 3.0), 3.0);
    }

    #[test]
    fn the_path_runs_port_to_port_below_them() {
        let (a, z) = (pos2(10.0, Y), pos2(200.0, Y));
        let depth = depth_for(a, z, 24.0);
        let p = path(a, z, depth);
        assert_eq!(p.pts[0], a);
        assert!(p.pts[POINTS - 1].distance(z) < 1e-3);
        for q in p.pts {
            assert!(q.y >= Y - 1e-3 && q.y <= Y + depth + 1e-3, "{q:?}");
        }
        assert!((p.low.y - (Y + depth)).abs() < 1e-3);
        assert!((p.low.x - 105.0).abs() < 1e-3);
        // The end mark sits on the destination's corner, below the port.
        assert!(p.mark.y > Y && p.mark.x < z.x && p.mark.x > z.x - depth);
        // Leftwards, mirrored.
        let back = path(z, a, depth);
        assert!(back.mark.x > a.x && back.mark.x < a.x + depth);
    }

    #[test]
    fn nested_cables_from_one_port_never_cross() {
        let a = pos2(0.0, Y);
        for band in [20.0f32, 32.0, 64.0] {
            for short in [24.0f32, 48.0, 72.0, 150.0, 300.0] {
                for long in [48.0f32, 96.0, 200.0, 480.0, 1200.0] {
                    if long <= short {
                        continue;
                    }
                    for dir in [1.0f32, -1.0] {
                        let (zs, zl) = (pos2(dir * short, Y), pos2(dir * long, Y));
                        let ps = path(a, zs, depth_for(a, zs, band));
                        let pl = path(a, zl, depth_for(a, zl, band));
                        for i in 0..=200 {
                            let x = dir * short * i as f32 / 200.0;
                            let (Some(ys), Some(yl)) = (y_at(&ps, x), y_at(&pl, x)) else {
                                continue;
                            };
                            assert!(
                                yl >= ys - 0.05,
                                "band {band}: the {long}-pt cable crosses above the \
                                 {short}-pt one at x = {x} ({yl} < {ys})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_hit_test_finds_the_cable_and_nothing_far_away() {
        let (a, z) = (pos2(0.0, Y), pos2(240.0, Y));
        let depth = depth_for(a, z, 24.0);
        let p = path(a, z, depth);
        assert!(hit(a, z, depth, p.low) < 0.01, "the flat run");
        assert!(hit(a, z, depth, p.pts[3]) < 0.01, "a corner");
        assert!((hit(a, z, depth, p.low + vec2(0.0, 3.0)) - 3.0).abs() < 0.01);
        assert_eq!(hit(a, z, depth, pos2(120.0, Y - 80.0)), f32::INFINITY);
        assert_eq!(hit(a, z, depth, pos2(600.0, Y + 10.0)), f32::INFINITY);
        // Inside the U, well away from the stroke.
        assert!(hit(a, z, depth, pos2(120.0, Y + 1.0)) > 10.0);
    }
}
