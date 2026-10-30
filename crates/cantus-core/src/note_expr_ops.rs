//! What the note operations and the expression lane do to a note's curves (ADR 0080):
//! pure functions over [`NoteExpr`], each returning the note's new expression, so a
//! command stores the result and its inverse is the old value, exactly.

use super::{raw_at, ExprKind, ExprPoint, NoteExpr};

/// One curve cut at `cut` ticks: the left part and the right part shifted to start at 0.
/// Each keeps the point just past its edge (past the left's end, before the right's start),
/// so both halves sound exactly what the whole did there, to the integer.
pub fn split_curve(c: &[ExprPoint], cut: i64) -> (Vec<ExprPoint>, Vec<ExprPoint>) {
    if c.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let i = c.partition_point(|p| p.0 < cut);
    let left = c[..(i + 1).min(c.len())].to_vec();
    let j = c.partition_point(|p| p.0 <= cut);
    let from = j.saturating_sub(1);
    let right = c[from..]
        .iter()
        .map(|p| ExprPoint(p.0 - cut, p.1))
        .collect();
    (left, right)
}

/// A note's expression cut at `cut` ticks from its start: what the left note keeps and
/// what the right note (starting at `cut`) gets. A note split by the knife, a note cut by a
/// clip split that is made unique, a slice.
pub fn split(e: Option<&NoteExpr>, cut: i64) -> (Option<Box<NoteExpr>>, Option<Box<NoteExpr>>) {
    let Some(e) = e else {
        return (None, None);
    };
    let (mut l, mut r) = (NoteExpr::default(), NoteExpr::default());
    for k in ExprKind::ALL {
        let (a, b) = split_curve(e.curve(k), cut);
        *l.curve_mut(k) = a;
        *r.curve_mut(k) = b;
    }
    (l.boxed(), r.boxed())
}

/// The note's start moved `d` ticks later (a left trim, keeping its end): the curves stay
/// where they were heard. `d < 0` (a left edge pulled out) holds the first value earlier.
pub fn trim_start(e: Option<&NoteExpr>, d: i64) -> Option<Box<NoteExpr>> {
    let e = e?;
    if d <= 0 {
        let mut out = e.clone();
        for k in ExprKind::ALL {
            for p in out.curve_mut(k).iter_mut() {
                p.0 -= d;
            }
        }
        return out.boxed();
    }
    split(Some(e), d).1
}

/// The note's length scaled by `num / den` (a time stretch of a selection, a note made
/// shorter by a legato or a "scale length"): the curves stretched with it.
pub fn stretch(e: Option<&NoteExpr>, num: i64, den: i64) -> Option<Box<NoteExpr>> {
    let e = e?;
    if den <= 0 || num <= 0 || num == den {
        return Some(Box::new(e.clone()));
    }
    let mut out = e.clone();
    for k in ExprKind::ALL {
        for p in out.curve_mut(k).iter_mut() {
            p.0 = (i128::from(p.0) * i128::from(num) / i128::from(den)) as i64;
        }
    }
    out.normalised().boxed()
}

/// The note played backwards over `len` ticks (a "reverse" of the selection): each curve
/// mirrored, so what the note did at its end it does at its start.
pub fn reverse(e: Option<&NoteExpr>, len: i64) -> Option<Box<NoteExpr>> {
    let e = e?;
    let mut out = NoteExpr::default();
    for k in ExprKind::ALL {
        let c = e.curve(k);
        let mut r: Vec<ExprPoint> = c
            .iter()
            .filter(|p| p.0 <= len)
            .map(|p| ExprPoint(len - p.0, p.1))
            .collect();
        if !c.is_empty() && c.iter().all(|p| p.0 > len) {
            r.push(ExprPoint(0, c[0].1));
        }
        r.reverse();
        *out.curve_mut(k) = r;
    }
    out.normalised().boxed()
}

/// Two notes joined into one (glue): `b`'s curves, which start `offset` ticks after `a`'s
/// start, appended after `a`'s up to there. A side that had no curve is at rest.
pub fn join(a: Option<&NoteExpr>, b: Option<&NoteExpr>, offset: i64) -> Option<Box<NoteExpr>> {
    if a.is_none() && b.is_none() {
        return None;
    }
    let mut out = NoteExpr::default();
    for k in ExprKind::ALL {
        let ca = a.map(|e| e.curve(k)).unwrap_or(&[]);
        let cb = b.map(|e| e.curve(k)).unwrap_or(&[]);
        if ca.is_empty() && cb.is_empty() {
            continue;
        }
        // The first note's part: its own points, held to the tick before the second starts
        // (at rest when it had none); the second's from there (at rest when it had none).
        let held = |c: &[ExprPoint], at: i64| raw_at(c, at).unwrap_or(k.rest());
        let mut c: Vec<ExprPoint> = ca.iter().copied().filter(|p| p.0 < offset).collect();
        if ca.is_empty() || cb.is_empty() {
            c.push(ExprPoint(offset - 1, held(ca, offset - 1)));
            c.push(ExprPoint(offset, held(cb, 0)));
            c.extend(
                cb.iter()
                    .filter(|p| p.0 > 0)
                    .map(|p| ExprPoint(p.0 + offset, p.1)),
            );
        } else {
            // Both had curves (the halves of a split): their edge points make it exact.
            if c.is_empty() {
                c.push(ExprPoint(0, ca[0].1));
            }
            c.extend(cb.iter().map(|p| ExprPoint(p.0 + offset, p.1)));
        }
        *out.curve_mut(k) = c;
    }
    out.normalised().boxed()
}

/// One curve's values scaled by `factor` about the kind's rest (the bend about the note's
/// pitch, pressure about none, timbre about its middle), clamped.
pub fn scale(c: &[ExprPoint], kind: ExprKind, factor: f64) -> Vec<ExprPoint> {
    let rest = kind.rest() as f64;
    c.iter()
        .map(|p| {
            let v = rest + (p.1 as f64 - rest) * factor;
            ExprPoint(p.0, kind.clamp(v.round() as i64))
        })
        .collect()
}

/// One curve smoothed: `passes` rounds of a three-point binomial filter (¼ ½ ¼) over the
/// values, the ends kept — what takes a controller's jitter out without moving the shape.
pub fn smooth(c: &[ExprPoint], kind: ExprKind, passes: usize) -> Vec<ExprPoint> {
    let mut v: Vec<i64> = c.iter().map(|p| p.1).collect();
    if v.len() < 3 {
        return c.to_vec();
    }
    for _ in 0..passes.min(64) {
        let prev = v.clone();
        for i in 1..v.len() - 1 {
            let s = i128::from(prev[i - 1]) + 2 * i128::from(prev[i]) + i128::from(prev[i + 1]);
            v[i] = ((s + 2).div_euclid(4)) as i64;
        }
    }
    c.iter()
        .zip(v)
        .map(|(p, x)| ExprPoint(p.0, kind.clamp(x)))
        .collect()
}

/// One curve thinned: the fewest points that stay within `tolerance` (in the kind's
/// integer unit) of every dropped one (Ramer–Douglas–Peucker, ends kept). What a recorded
/// curve goes through before it is stored, and the lane's *thin*.
pub fn thin(c: &[ExprPoint], tolerance: i64) -> Vec<ExprPoint> {
    if c.len() < 3 {
        return c.to_vec();
    }
    // Runs of one value collapse first (a held key sends the same pressure again).
    let mut keep = vec![false; c.len()];
    keep[0] = true;
    keep[c.len() - 1] = true;
    let mut stack = vec![(0usize, c.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (c[a], c[b]);
        let mut worst = (0i64, a);
        for (i, p) in c.iter().enumerate().take(b).skip(a + 1) {
            let line = raw_at(&[pa, pb], p.0).unwrap_or(p.1);
            let d = (p.1 - line).abs();
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > tolerance {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    c.iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// The tolerance [`thin`] uses for a kind by default: a cent of bend, a thousandth of
/// pressure or timbre — below what an ear or a 7-bit controller tells apart.
pub fn default_tolerance(kind: ExprKind) -> i64 {
    match kind {
        ExprKind::Bend => super::BEND_UNIT / 100,
        _ => super::UNIT_MAX / 1000,
    }
}

/// A curve with `drawn` (sorted, from the pen or the line tool) in place of whatever it
/// had between the drawn stretch's first and last ticks.
pub fn draw(c: &[ExprPoint], drawn: &[ExprPoint]) -> Vec<ExprPoint> {
    let (Some(a), Some(b)) = (drawn.first(), drawn.last()) else {
        return c.to_vec();
    };
    let mut out: Vec<ExprPoint> = c.iter().copied().filter(|p| p.0 < a.0).collect();
    out.extend_from_slice(drawn);
    out.extend(c.iter().copied().filter(|p| p.0 > b.0));
    out
}

/// A curve with no movement: the bend back to the note's pitch, pressure and timbre held
/// at their average over the note's `len` ticks.
pub fn flatten(c: &[ExprPoint], kind: ExprKind, len: i64) -> Vec<ExprPoint> {
    if c.is_empty() {
        return Vec::new();
    }
    let v = match kind {
        ExprKind::Bend => 0,
        _ => mean(c, len.max(1)),
    };
    vec![ExprPoint(0, v)]
}

/// A curve's time-weighted mean over `[0, len)`.
pub fn mean(c: &[ExprPoint], len: i64) -> i64 {
    let step = (len / 256).max(1);
    let (mut sum, mut n) = (0i128, 0i128);
    let mut t = 0;
    while t < len {
        sum += i128::from(raw_at(c, t).unwrap_or(0));
        n += 1;
        t += step;
    }
    if n == 0 {
        return raw_at(c, 0).unwrap_or(0);
    }
    (sum / n) as i64
}

/// The bend snapped to whole semitones (the lane's draw with its modifier held).
pub fn snap_semitone(raw: i64) -> i64 {
    let u = super::BEND_UNIT;
    let q = if raw >= 0 {
        (raw + u / 2) / u
    } else {
        (raw - u / 2) / u
    };
    q * u
}
