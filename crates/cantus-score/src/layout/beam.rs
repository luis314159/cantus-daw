//! Stems, flags and beams (Gould, *Behind Bars*, "Beams"): a beam slopes with its outer
//! notes by their interval — a quarter space for a second, up to a space — but never more
//! than its length allows, and lies flat over a concave line or a repeated note; its
//! shortest stem is never under three spaces (more for each beam it carries); notes off
//! the staff stretch their beam to the middle line; inside the staff each end sits on,
//! straddles or hangs from a line (no thin wedge of white between beam and line).

use super::chord::y_of;
use crate::display::{Item, Owner};
use crate::glyphs::{defaults, Glyph};
use crate::model::{BeamMark, Dur, Stem};

/// A stemmed event as the beam and stem drawing need it.
#[derive(Clone, Debug)]
pub struct Stemmed {
    /// The stem's centre x (absolute).
    pub x: f32,
    /// Its heads' positions.
    pub lo: i32,
    pub hi: i32,
    pub stem: Stem,
    pub dur: Dur,
    pub beams: Vec<BeamMark>,
    /// The notehead glyph (for its stem anchors).
    pub head: Glyph,
    pub owner: Owner,
}

/// A stem's length to its flag (or a lone stem's), by value.
fn stem_len(d: Dur) -> f32 {
    match d.log {
        i8::MIN..=4 => 3.5,
        5 => 4.25,
        _ => 5.0,
    }
}

/// Where a stem starts (at its far notehead) and which way it runs.
fn stem_root(s: &Stemmed) -> f32 {
    match s.stem {
        Stem::Up => y_of(s.lo) + s.head.stem_up_se()[1],
        Stem::Down => y_of(s.hi) + s.head.stem_down_nw()[1],
    }
}

/// The y of the note nearest the beam (or flag).
fn near_y(s: &Stemmed) -> f32 {
    match s.stem {
        Stem::Up => y_of(s.hi),
        Stem::Down => y_of(s.lo),
    }
}

/// A lone stem with its flag. Returns the stem's tip y.
pub fn lone(s: &Stemmed, out: &mut Vec<Item>) -> f32 {
    let th = defaults::STEM_THICKNESS;
    let len = stem_len(s.dur);
    let near = near_y(s);
    let tip = match s.stem {
        Stem::Up => (near - len).min(2.0),
        Stem::Down => (near + len).max(2.0),
    };
    out.push(Item::Line {
        x0: s.x,
        y0: stem_root(s),
        x1: s.x,
        y1: tip,
        w: th,
        owner: s.owner,
    });
    if let Some(g) = super::chord::flag(s.dur, s.stem) {
        let a = match s.stem {
            Stem::Up => g.stem_up_nw(),
            Stem::Down => g.stem_down_sw(),
        };
        out.push(Item::Glyph {
            glyph: g,
            x: s.x - th / 2.0 - a[0],
            y: tip - a[1],
            scale: 1.0,
            owner: s.owner,
        });
    }
    tip
}

/// The slope a beam takes over `span` spaces between outer notes `d` steps apart.
fn slope(d: i32, span: f32) -> f32 {
    let by_interval: f32 = match d.abs() {
        0 => 0.0,
        1 => 0.25,
        2 => 0.5,
        3 => 0.75,
        _ => 1.0,
    };
    by_interval.min(0.15 * span) * d.signum() as f32
}

/// Round a beam end to the staff's lines: inside the staff, never a quarter-space wedge.
fn quantise(y_outer: f32, up: bool) -> f32 {
    let q = (y_outer * 4.0).round() / 4.0;
    if !(-0.6..=4.6).contains(&q) {
        return q;
    }
    let frac = q.rem_euclid(1.0);
    if up {
        // The top edge: on a line (hang), a quarter above (straddle), a half above (sit).
        if (frac - 0.25).abs() < 1e-3 {
            q - 0.25
        } else {
            q
        }
    } else {
        // The bottom edge, mirrored.
        if (frac - 0.75).abs() < 1e-3 {
            q + 0.25
        } else {
            q
        }
    }
}

/// A beamed group: stems and beams. Returns each stem's tip y.
pub fn group(g: &[Stemmed], out: &mut Vec<Item>) -> Vec<f32> {
    let th = defaults::STEM_THICKNESS;
    let h = defaults::BEAM_THICKNESS;
    let step = defaults::BEAM_THICKNESS + defaults::BEAM_SPACING;
    let up = g[0].stem == Stem::Up;
    let levels = g.iter().map(|s| s.beams.len()).max().unwrap_or(1).max(1);
    let (x0, x1) = (g[0].x, g[g.len() - 1].x);
    let span = (x1 - x0).max(0.01);
    // The outer notes' interval, toward the beam (positions rise upward, y falls).
    let near_pos = |s: &Stemmed| if up { s.hi } else { s.lo };
    let (pa, pb) = (near_pos(&g[0]), near_pos(&g[g.len() - 1]));
    let inner_extreme = g[1..g.len() - 1].iter().any(|s| {
        let p = near_pos(s);
        if up {
            p > pa.max(pb)
        } else {
            p < pa.min(pb)
        }
    });
    let rise = if inner_extreme {
        0.0
    } else {
        slope(pb - pa, span)
    };
    // y of the beam's outer edge: y0 at x0, falling by `rise` (rise is upward).
    let m = -rise / span;
    let shortest = 3.0 + 0.75 * (levels as f32 - 1.0);
    let ideal_end = 3.5 + 0.75 * (levels as f32 - 1.0);
    let mut y0 = if up { f32::MAX } else { f32::MIN };
    for (k, s) in g.iter().enumerate() {
        let len = if k == 0 || k == g.len() - 1 {
            ideal_end
        } else {
            shortest
        };
        let at = near_y(s);
        let dx = s.x - x0;
        if up {
            y0 = y0.min(at - len - dx * m);
        } else {
            y0 = y0.max(at + len - dx * m);
        }
    }
    // Off the staff: the beam reaches the middle line.
    if up {
        y0 = y0.min(2.0).min(2.0 - span * m);
    } else {
        y0 = y0.max(2.0).max(2.0 - span * m);
    }
    let mut ya = quantise(y0, up);
    let mut yb = quantise(y0 + span * m, up);
    // Quantising never reverses or invents a slope.
    if rise == 0.0 {
        let y = if up { ya.min(yb) } else { ya.max(yb) };
        ya = y;
        yb = y;
    } else if (yb - ya).signum() != (m).signum() {
        yb = ya;
    }
    let outer = |x: f32| ya + (x - x0) * (yb - ya) / span;
    // Stems to the outer edge.
    let mut tips = Vec::with_capacity(g.len());
    for s in g {
        let tip = outer(s.x);
        out.push(Item::Line {
            x0: s.x,
            y0: stem_root(s),
            x1: s.x,
            y1: tip,
            w: th,
            owner: s.owner,
        });
        tips.push(tip);
    }
    // Beams: the primary outermost, each further level a step toward the notes.
    let top_of = |x: f32, level: usize| {
        let o = outer(x);
        if up {
            o + level as f32 * step
        } else {
            o - h - level as f32 * step
        }
    };
    let hook = 1.1;
    for level in 0..levels {
        let mut k = 0;
        while k < g.len() {
            let Some(mark) = g[k].beams.get(level).copied() else {
                k += 1;
                continue;
            };
            let (a, b) = match mark {
                BeamMark::Begin => {
                    let mut e = k + 1;
                    while e < g.len() && g[e].beams.get(level) != Some(&BeamMark::End) {
                        e += 1;
                    }
                    let e = e.min(g.len() - 1);
                    let r = (g[k].x - th / 2.0, g[e].x + th / 2.0);
                    k = e;
                    r
                }
                BeamMark::HookForward => (g[k].x - th / 2.0, g[k].x + hook),
                BeamMark::HookBackward => (g[k].x - hook, g[k].x + th / 2.0),
                _ => {
                    k += 1;
                    continue;
                }
            };
            out.push(Item::Beam {
                x0: a,
                y0: top_of(a, level),
                x1: b,
                y1: top_of(b, level),
                h,
                owner: None,
            });
            k += 1;
        }
    }
    tips
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: f32, pos: i32, stem: Stem) -> Stemmed {
        Stemmed {
            x,
            lo: pos,
            hi: pos,
            stem,
            dur: Dur::new(3, 0),
            beams: vec![BeamMark::Continue],
            head: Glyph::NoteheadBlack,
            owner: None,
        }
    }

    #[test]
    fn slopes_follow_the_outer_interval_and_stay_gentle() {
        assert_eq!(slope(1, 10.0), 0.25);
        assert_eq!(slope(-3, 10.0), -0.75);
        assert_eq!(slope(7, 10.0), 1.0);
        assert!((slope(4, 2.0) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_concave_group_lies_flat_and_stems_are_long_enough() {
        let mut out = Vec::new();
        let mut g = vec![
            s(0.0, -2, Stem::Up),
            s(2.0, 2, Stem::Up),
            s(4.0, -2, Stem::Up),
        ];
        g[0].beams = vec![BeamMark::Begin];
        g[2].beams = vec![BeamMark::End];
        let tips = group(&g, &mut out);
        assert!((tips[0] - tips[2]).abs() < 1e-6, "{tips:?}");
        for (k, t) in tips.iter().enumerate() {
            assert!(y_of(g[k].hi) - t >= 3.0 - 1e-4, "stem {k} too short: {t}");
        }
        assert!(out.iter().any(|i| matches!(i, Item::Beam { .. })));
    }

    #[test]
    fn beam_ends_inside_the_staff_avoid_wedges() {
        for y in [0.25f32, 1.25, 3.25] {
            let q = quantise(y, true);
            assert!((q.rem_euclid(1.0) - 0.25).abs() > 1e-3, "{y} → {q}");
        }
        assert_eq!(quantise(-3.25, true), -3.25);
    }
}
