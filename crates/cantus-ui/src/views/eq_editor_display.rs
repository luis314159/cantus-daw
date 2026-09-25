//! The EQ's display: its geometry, what it draws, and every gesture on it — the plot and
//! the frequency axis (or the piano) under it.

use super::*;
use egui::{Rect, Vec2};

/// Height of the frequency axis under the plot; the piano is taller.
const AXIS_H: f32 = space::GRID * 5.0;
const PIANO_H: f32 = space::GRID * 8.0;
const DOT_R: f32 = 8.0;

/// A band's curve across the display, and what it was computed from.
#[derive(Clone, Debug)]
pub(super) struct CachedCurve {
    band: BandSettings,
    mode: Mode,
    view: (f32, f32),
    sr: f32,
    db: Vec<f32>,
}

/// Geometry of the display, shared by drawing and hit-testing.
#[derive(Copy, Clone, Debug)]
pub(super) struct Geo {
    pub rect: Rect,
    pub range: f32,
    pub lo: f32,
    pub hi: f32,
}

impl Geo {
    pub fn new(rect: Rect, range: f32, view: (f32, f32)) -> Geo {
        Geo {
            rect,
            range,
            lo: view.0,
            hi: view.1,
        }
    }
    pub fn x(&self, hz: f32) -> f32 {
        let f = (hz.max(1.0) / self.lo).ln() / (self.hi / self.lo).ln();
        self.rect.left() + f * self.rect.width()
    }
    pub fn hz(&self, x: f32) -> f32 {
        let f = (x - self.rect.left()) / self.rect.width().max(1.0);
        self.lo * (self.hi / self.lo).powf(f)
    }
    /// The frequency of point `i` of `n` across the view.
    pub fn hz_at(&self, i: usize, n: usize) -> f32 {
        self.lo * (self.hi / self.lo).powf(i as f32 / (n - 1) as f32)
    }
    pub fn y(&self, db: f32) -> f32 {
        self.rect.center().y - (db / self.range).clamp(-1.1, 1.1) * self.rect.height() * 0.5
    }
    pub fn db(&self, y: f32) -> f32 {
        (self.rect.center().y - y) / (self.rect.height() * 0.5) * self.range
    }
    /// The analyser's scale: its own, from `-floor` dB to 0 dBFS over the full height.
    pub fn y_spec(&self, db: f32, floor: f32) -> f32 {
        let f = ((db + floor) / floor).clamp(0.0, 1.0);
        self.rect.bottom() - f * self.rect.height()
    }
    pub fn dot(&self, b: &BandSettings) -> Pos2 {
        let gain = if b.shape.has_gain() { b.gain_db } else { 0.0 };
        pos2(self.x(b.freq), self.y(gain))
    }
}

/// A drag in progress on the plot.
#[derive(Clone, Debug)]
pub(super) struct Drag {
    kind: DragKind,
    key: MergeKey,
    /// Pointer movement so far, already scaled for fine moves.
    acc: Vec2,
    /// `Some(true)` locks to frequency, `Some(false)` to gain (Alt).
    lock: Option<bool>,
}

#[derive(Clone, Debug)]
enum DragKind {
    /// The selected bands, where each started: (band, freq, gain, q).
    Bands {
        start: Vec<(usize, f32, f32, f32)>,
        /// Ctrl: vertical movement changes Q instead of gain.
        q_mode: bool,
    },
    /// A dynamic band's range handle.
    Range { band: usize, start: f32 },
}

/// A drag on the frequency axis: zoom (vertical) and pan (horizontal).
#[derive(Clone, Debug)]
pub(super) struct AxisDrag {
    anchor: f32,
    view: (f32, f32),
    acc: Vec2,
}

fn used(slot: &EffectSlot, band: usize) -> bool {
    slot.spec.get(&eq_key(band, "used")) >= 0.5
}

/// The shape a double-click (or a press on the curve) makes at a point of the plot.
fn shape_at(rect: Rect, pt: Pos2, on_curve: bool) -> EqShape {
    let fx = (pt.x - rect.left()) / rect.width();
    let fy = (pt.y - rect.top()) / rect.height();
    if on_curve {
        return if fx < 0.1 {
            EqShape::LowShelf
        } else if fx > 0.9 {
            EqShape::HighShelf
        } else {
            EqShape::Bell
        };
    }
    if fx < 0.05 {
        EqShape::LowCut
    } else if fx > 0.95 {
        EqShape::HighCut
    } else if fx < 0.12 {
        EqShape::LowShelf
    } else if fx > 0.88 {
        EqShape::HighShelf
    } else if fy > 0.9 {
        EqShape::Notch
    } else {
        EqShape::Bell
    }
}

/// The part of the spectrum an audition plays, Hz.
fn audible_region(b: &BandSettings, view: (f32, f32)) -> (f32, f32) {
    match b.shape {
        EqShape::LowShelf | EqShape::LowCut => (view.0, b.freq),
        EqShape::HighShelf | EqShape::HighCut => (b.freq, view.1),
        EqShape::TiltShelf => view,
        _ => {
            // A band-pass's −3 dB points: bandwidth in octaves from its Q.
            let bw = 2.0 / std::f32::consts::LN_2 * (1.0 / (2.0 * b.q.max(0.1))).asinh();
            let k = 2f32.powf(bw * 0.5);
            (b.freq / k, b.freq * k)
        }
    }
}

/// The spectra's peaks worth grabbing: local maxima that stand at least 6 dB over their
/// surroundings within a third of an octave, the eight loudest, left to right.
fn peaks(spec: &[f32], geo: &Geo, floor: f32) -> Vec<usize> {
    let n = spec.len();
    let per_oct = (n - 1) as f32 / (geo.hi / geo.lo).log2();
    let w = (per_oct / 3.0).ceil().max(2.0) as usize;
    let mut found: Vec<usize> = (1..n.saturating_sub(1))
        .filter(|&i| {
            let v = spec[i];
            if v <= -floor + 12.0 {
                return false;
            }
            let lo = i.saturating_sub(w);
            let hi = (i + w).min(n - 1);
            if spec[lo..=hi].iter().any(|x| *x > v) {
                return false;
            }
            let left = spec[lo..i].iter().copied().fold(f32::MAX, f32::min);
            let right = spec[i + 1..=hi].iter().copied().fold(f32::MAX, f32::min);
            v - left.max(right) >= 6.0
        })
        .collect();
    found.sort_by(|a, b| spec[*b].total_cmp(&spec[*a]));
    found.truncate(8);
    found.sort_unstable();
    found
}

/// A band's curve across the display: designed once, read at every point.
fn curve_of(b: &BandSettings, mode: Mode, sr: f32, geo: &Geo) -> Vec<f32> {
    let hz: Vec<f32> = (0..POINTS).map(|k| geo.hz_at(k, POINTS)).collect();
    let mut out = vec![0.0; POINTS];
    aequalis::band_curve_db(b, mode, sr, &hz, &mut out);
    out
}

/// A band's curve across the display, from the cache when nothing changed.
fn curve(
    ed: &mut EqEditor,
    slot_index: usize,
    b: &BandSettings,
    mode: Mode,
    geo: &Geo,
    sr: f32,
) -> Vec<f32> {
    let view = (geo.lo, geo.hi);
    if let Some(Some(c)) = ed.curves.get(slot_index) {
        if c.band == *b && c.mode == mode && c.view == view && c.sr == sr {
            return c.db.clone();
        }
    }
    let db = curve_of(b, mode, sr, geo);
    if slot_index < ed.curves.len() {
        ed.curves[slot_index] = Some(CachedCurve {
            band: *b,
            mode,
            view,
            sr,
            db: db.clone(),
        });
    }
    db
}

pub(super) fn fill_to(p: &egui::Painter, top: &[Pos2], base: impl Fn(f32) -> f32, color: Color32) {
    let mut mesh = egui::Mesh::default();
    for (k, pt) in top.iter().enumerate() {
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(*pt, color);
        mesh.colored_vertex(pos2(pt.x, base(pt.x)), color);
        if k > 0 {
            mesh.add_triangle(i - 2, i - 1, i);
            mesh.add_triangle(i - 1, i, i + 1);
        }
    }
    p.add(mesh);
}

/// An axis tick: `50`, `500`, `1k`, `2.5k`, `20k`.
fn axis_label(hz: f32) -> String {
    if hz >= 1_000.0 {
        let k = hz / 1_000.0;
        if (k - k.round()).abs() < 0.01 {
            format!("{:.0}k", k)
        } else {
            format!("{k:.1}k")
        }
    } else {
        format!("{hz:.0}")
    }
}

/// A moving average over `±w` points, for curves read at a glance.
fn smooth(v: &[f32], w: usize) -> Vec<f32> {
    let n = v.len();
    (0..n)
        .map(|i| {
            let lo = i.saturating_sub(w);
            let hi = (i + w).min(n - 1);
            v[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32
        })
        .collect()
}

/// Diagonal hatching under a curve (points left to right) down to `bottom`.
fn hatch_under(p: &egui::Painter, pts: &[Pos2], bottom: f32, color: Color32) {
    let (Some(first), Some(last)) = (pts.first(), pts.last()) else {
        return;
    };
    let (x0, x1) = (first.x, last.x.max(first.x + 1.0));
    let curve_at = |x: f32| -> f32 {
        let i = pts
            .partition_point(|q| q.x < x)
            .clamp(1, pts.len().max(2) - 1);
        let (a, b) = (pts[i - 1], pts[i.min(pts.len() - 1)]);
        if (b.x - a.x).abs() < 1e-3 {
            a.y
        } else {
            a.y + (b.y - a.y) * ((x - a.x) / (b.x - a.x)).clamp(0.0, 1.0)
        }
    };
    let h = (bottom - pts.iter().map(|q| q.y).fold(bottom, f32::min)).max(1.0);
    let mut c = x0 - h;
    while c < x1 {
        let mut run: Option<(Pos2, Pos2)> = None;
        let mut t = 0.0;
        while t <= h {
            let at = pos2(c + t, bottom - t);
            let inside = at.x >= x0 && at.x <= x1 && at.y >= curve_at(at.x);
            match (&mut run, inside) {
                (Some((_, end)), true) => *end = at,
                (None, true) => run = Some((at, at)),
                (Some((a, b)), false) => {
                    p.line_segment([*a, *b], Stroke::new(1.0, color));
                    run = None;
                }
                (None, false) => {}
            }
            t += 2.0;
        }
        if let Some((a, b)) = run {
            p.line_segment([a, b], Stroke::new(1.0, color));
        }
        c += 6.0;
    }
}

/// The grid's frequency lines for a view: decades, and their 2 and 5 marks.
fn grid_lines(lo: f32, hi: f32) -> Vec<(f32, bool, bool)> {
    let mut out = Vec::new();
    let span_oct = (hi / lo).log2();
    for decade in [1.0f32, 10.0, 100.0, 1_000.0, 10_000.0] {
        for m in 1..10 {
            let hz = decade * m as f32;
            if hz < lo || hz > hi {
                continue;
            }
            let strong = m == 1;
            let labelled = strong || m == 5 || (m == 2 && span_oct < 6.0) || span_oct < 3.0;
            if strong || m == 2 || m == 5 || span_oct < 5.0 {
                out.push((hz, strong, labelled));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(super) fn display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    bands: &[BandSettings; cantus_dsp::aequalis::MAX_BANDS],
    tel: &[f32; 64],
    height: f32,
) {
    let pal = theme::p();
    let sr = app.eq_sample_rate();
    let mode = Mode::from_index(slot.spec.get("mode").round() as usize);
    let solo = {
        let s = slot.spec.get("solo").round() as usize;
        (s >= 1).then(|| s - 1)
    };
    let width = ui.available_width();
    let axis_h = if app.eq_editor.piano { PIANO_H } else { AXIS_H };
    let (rect, resp) =
        ui.allocate_exact_size(vec2(width, height - axis_h), Sense::click_and_drag());
    let (axis_rect, axis_resp) =
        ui.allocate_exact_size(vec2(width, axis_h), Sense::click_and_drag());
    let ed = &mut app.eq_editor;
    let view = (
        ed.view.0.max(VIEW_LIMITS.0),
        ed.view.1.min(VIEW_LIMITS.1.min(sr * 0.5)),
    );
    let geo = Geo::new(rect, ed.range_db, view);
    ed.plot = Some(rect);
    ed.plot_view = view;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let floor = ed.show.range_db;

    // ---- grid: frequency lines, the EQ's dB lines (right) and the analyser's (left) ----
    for (hz, strong, _) in grid_lines(view.0, view.1) {
        p.vline(
            geo.x(hz),
            rect.y_range(),
            Stroke::new(1.0, if strong { pal.grid_strong } else { pal.grid }),
        );
    }
    let step = if geo.range <= 3.0 {
        1.0
    } else if geo.range <= 6.0 {
        2.0
    } else if geo.range <= 12.0 {
        3.0
    } else {
        6.0
    };
    let mut db = -geo.range;
    while db <= geo.range + 1e-3 {
        let y = geo.y(db);
        p.hline(
            rect.x_range(),
            y,
            Stroke::new(1.0, if db == 0.0 { pal.grid_bar } else { pal.grid }),
        );
        p.text(
            pos2(rect.right() - 3.0, y - 1.0),
            Align2::RIGHT_BOTTOM,
            format!("{db:+.0}"),
            design::paint::ruler(),
            pal.gold,
        );
        db += step;
    }
    let mut a = -20.0;
    while a > -floor + 1.0 {
        p.text(
            pos2(rect.left() + 3.0, geo.y_spec(a, floor) - 1.0),
            Align2::LEFT_BOTTOM,
            format!("{a:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
        a -= 20.0;
    }

    // ---- spectra ------------------------------------------------------------------------
    let show = ed.show;
    let now = std::time::Instant::now();
    let dt = now.duration_since(ed.last).as_secs_f32().min(0.5);
    ed.last = now;
    let fall = AnalyserView::FALL[show.speed.min(4)] * dt;
    let tilt_at = |hz: f32| show.tilt * (hz / 1_000.0).log2();
    if ed.analyser.has_data() && !show.grab {
        ed.analyser.compute();
        for i in 0..POINTS {
            let hz = geo.hz_at(i, POINTS);
            let a = ed.analyser.level_db(0, sr, hz, 1.0 / 6.0) + tilt_at(hz);
            let b = ed.analyser.level_db(1, sr, hz, 1.0 / 6.0) + tilt_at(hz);
            if show.freeze {
                ed.pre[i] = ed.pre[i].max(a);
                ed.post[i] = ed.post[i].max(b);
            } else {
                ed.pre[i] = a.max(ed.pre[i] - fall);
                ed.post[i] = b.max(ed.post[i] - fall);
            }
        }
    }
    // The key strip's spectrum, from the unit's third octaves, read at every point.
    let key_on = tel[aequalis::telemetry::KEY_ON] >= 0.5;
    if key_on && !show.grab {
        let bands3: Vec<(f32, f32)> = (0..aequalis::KEY_BANDS)
            .map(|k| (aequalis::key_band_hz(k), tel[aequalis::telemetry::KEY + k]))
            .collect();
        for i in 0..POINTS {
            let hz = geo.hz_at(i, POINTS);
            let v = aequalis::fit::interpolate(&bands3, hz) + tilt_at(hz);
            ed.key[i] = if show.freeze {
                ed.key[i].max(v)
            } else {
                v.max(ed.key[i] - fall)
            };
        }
    } else if !key_on && !show.freeze {
        ed.key.fill(-200.0);
    }
    // EQ match listens while it is open: the input's and the key's long-term spectra.
    if let Some(m) = ed.matching.as_mut() {
        if m.listening && key_on && ed.analyser.has_data() {
            for k in 0..aequalis::KEY_BANDS {
                let hz = aequalis::key_band_hz(k);
                let input = ed.analyser.level_db(0, sr, hz, 1.0 / 3.0);
                let key = tel[aequalis::telemetry::KEY + k];
                m.input[k] += 10f64.powf(input as f64 / 10.0);
                m.reference[k] += 10f64.powf(key as f64 / 10.0);
            }
            m.frames += 1;
        }
    }
    let spec_pts = |v: &[f32]| -> Vec<Pos2> {
        (0..POINTS)
            .map(|i| pos2(geo.x(geo.hz_at(i, POINTS)), geo.y_spec(v[i], floor)))
            .collect()
    };
    let dark = pal.dark;
    if show.pre && ed.analyser.has_data() {
        let pts = spec_pts(&ed.pre);
        fill_to(
            &p,
            &pts,
            |_| rect.bottom(),
            pal.text_dim.gamma_multiply(if dark { 0.16 } else { 0.12 }),
        );
    }
    if show.post && ed.analyser.has_data() {
        p.add(egui::Shape::line(
            spec_pts(&ed.post),
            Stroke::new(
                1.25,
                pal.text_dim.gamma_multiply(if dark { 0.85 } else { 0.95 }),
            ),
        ));
    }
    // The key strip: dashed, in the text colour — a line no band colour can be confused
    // with — smoothed over a sixth of an octave, since it is measured in third octaves.
    let key_color = pal.text;
    let key_smooth = smooth(&ed.key, 3);
    if show.key && key_on {
        p.add(egui::Shape::dashed_line(
            &spec_pts(&key_smooth),
            Stroke::new(1.25, key_color),
            6.0,
            4.0,
        ));
    }
    // Collisions: where the output and the key strip are both loud, and near each other,
    // hatched in the danger colour — tinted and hatched, so it reads without colour.
    if show.collisions && key_on && ed.analyser.has_data() {
        // Both spectra smoothed over a third of an octave; a collision is where both are
        // within 8 dB of each other and loud (in the top half of the analyser's scale).
        // Gaps shorter than a sixth of an octave are bridged, so the hatching reads as
        // regions, not as noise.
        let mine = smooth(if show.post { &ed.post } else { &ed.pre }, 5);
        let theirs = smooth(&ed.key, 5);
        let thresh = -floor * 0.5;
        let hit: Vec<bool> = (0..POINTS)
            .map(|k| mine[k] > thresh && theirs[k] > thresh && (mine[k] - theirs[k]).abs() < 8.0)
            .collect();
        let mut i = 0;
        while i < POINTS {
            if !hit[i] {
                i += 1;
                continue;
            }
            let start = i;
            let mut end = i;
            let mut gap = 0;
            while i < POINTS && gap <= 5 {
                if hit[i] {
                    end = i;
                    gap = 0;
                } else {
                    gap += 1;
                }
                i += 1;
            }
            // The area under the quieter of the two, tinted and hatched, edged in the
            // danger colour.
            let pts: Vec<Pos2> = (start..=end)
                .map(|k| {
                    pos2(
                        geo.x(geo.hz_at(k, POINTS)),
                        geo.y_spec(mine[k].min(theirs[k]), floor),
                    )
                })
                .collect();
            fill_to(&p, &pts, |_| rect.bottom(), pal.danger.gamma_multiply(0.08));
            hatch_under(&p, &pts, rect.bottom(), pal.danger.gamma_multiply(0.4));
            if pts.len() > 1 {
                p.add(egui::Shape::line(pts, Stroke::new(1.5, pal.danger)));
            }
        }
    }

    // ---- bands --------------------------------------------------------------------------
    let deltas = if ed.injected.is_some() {
        ed.history.back().map(|h| h.0).unwrap_or([0.0; EQ_BANDS])
    } else {
        let d = app.engine.meters().eq_deltas;
        let mut out = [0.0; EQ_BANDS];
        out.copy_from_slice(&d[..EQ_BANDS]);
        out
    };
    let ed = &mut app.eq_editor;
    let curves: Vec<(usize, Vec<f32>)> = bands
        .iter()
        .enumerate()
        .filter(|(_, b)| b.active)
        .map(|(i, b)| (i, curve(ed, i, b, mode, &geo, sr)))
        .collect();
    let pts_of = |db: &[f32]| -> Vec<Pos2> {
        (0..POINTS)
            .map(|k| pos2(geo.x(geo.hz_at(k, POINTS)), geo.y(db[k])))
            .collect()
    };
    // The audition: everything outside the band's region is shaded.
    if let Some(s) = solo.filter(|s| used(slot, *s)) {
        let (a, b) = audible_region(&bands[s], view);
        let shade = theme::recess(&pal).gamma_multiply(0.75);
        let (xa, xb) = (geo.x(a).max(rect.left()), geo.x(b).min(rect.right()));
        if xa > rect.left() {
            p.rect_filled(
                Rect::from_x_y_ranges(rect.left()..=xa, rect.y_range()),
                0.0,
                shade,
            );
        }
        if xb < rect.right() {
            p.rect_filled(
                Rect::from_x_y_ranges(xb..=rect.right(), rect.y_range()),
                0.0,
                shade,
            );
        }
        for x in [xa, xb] {
            p.vline(x, rect.y_range(), Stroke::new(1.0, band_color(s)));
        }
    }
    for (i, db) in &curves {
        let b = &bands[*i];
        let color = band_color(*i);
        let pts = pts_of(db);
        if ed.selection.contains(i) {
            // A cut's area is most of the display: fill it faintly, or it drowns the rest.
            let fill = color.gamma_multiply(if b.shape.has_gain() { 0.22 } else { 0.08 });
            let zero = geo.y(0.0);
            fill_to(&p, &pts, |_| zero, fill);
        }
        p.add(egui::Shape::line(
            pts.clone(),
            Stroke::new(1.0, color.gamma_multiply(0.85)),
        ));
        // A dynamic band: the region its range can take it to, and where it is now.
        if b.dynamic && b.shape.has_gain() {
            let moved = BandSettings {
                gain_db: b.gain_db + b.range_db,
                ..*b
            };
            let reach = curve_of(&moved, mode, sr, &geo);
            let reach_pts = pts_of(&reach);
            let mut mesh = egui::Mesh::default();
            let tint = color.gamma_multiply(0.10);
            for k in 0..POINTS {
                let v = mesh.vertices.len() as u32;
                mesh.colored_vertex(pts[k], tint);
                mesh.colored_vertex(reach_pts[k], tint);
                if k > 0 {
                    mesh.add_triangle(v - 2, v - 1, v);
                    mesh.add_triangle(v - 1, v, v + 1);
                }
            }
            p.add(mesh);
            p.add(egui::Shape::dashed_line(
                &reach_pts,
                Stroke::new(1.0, color),
                4.0,
                3.0,
            ));
            if deltas[*i].abs() > 0.05 {
                let live = BandSettings {
                    gain_db: b.gain_db + deltas[*i],
                    ..*b
                };
                let live_pts = pts_of(&curve_of(&live, mode, sr, &geo));
                p.add(egui::Shape::line(live_pts, Stroke::new(2.0, color)));
            }
        }
    }
    // The result: one curve, or one per part of the image when bands are placed (L, R,
    // M, S), each named at its right end — colour is never the only cue.
    let placed: Vec<Placement> = [
        Placement::Left,
        Placement::Right,
        Placement::Mid,
        Placement::Side,
    ]
    .into_iter()
    .filter(|pl| curves.iter().any(|(i, _)| bands[*i].placement == *pl))
    .collect();
    let total_for = |pl: Option<Placement>| -> Vec<f32> {
        (0..POINTS)
            .map(|k| {
                curves
                    .iter()
                    .filter(|(i, _)| {
                        let bp = bands[*i].placement;
                        bp == Placement::Stereo || Some(bp) == pl
                    })
                    .map(|(_, c)| c[k])
                    .sum()
            })
            .collect()
    };
    // The bands on the whole image, thick; where a part of the image (L, R, M, S) differs
    // from that, its own curve, thick in its colour and named where it ends.
    let total = total_for(None);
    p.add(egui::Shape::line(
        pts_of(&total),
        Stroke::new(2.5, pal.gold),
    ));
    for pl in &placed {
        let (glyph, color) = placement_style(*pl, &pal);
        let db = total_for(Some(*pl));
        let pts = pts_of(&db);
        let mut run: Vec<Pos2> = Vec::new();
        let mut last = None;
        for k in 0..POINTS {
            if (db[k] - total[k]).abs() > 0.05 {
                run.push(pts[k]);
                last = Some(pts[k]);
            } else if !run.is_empty() {
                run.push(pts[k]);
                p.add(egui::Shape::line(
                    std::mem::take(&mut run),
                    Stroke::new(2.0, color),
                ));
            }
        }
        if run.len() > 1 {
            p.add(egui::Shape::line(run, Stroke::new(2.0, color)));
        }
        if let Some(end) = last {
            let at = pos2(end.x.min(rect.right() - space::S8 - space::S4), end.y);
            p.circle_filled(at, 7.0, color);
            p.text(
                at,
                Align2::CENTER_CENTER,
                glyph,
                design::paint::caption(),
                theme::on_color(&pal, color),
            );
        }
    }

    // EQ match: the difference it aims at, thick, and what the bands would make of it.
    if let Some(m) = ed.matching.as_ref() {
        if let Some(target) = m.target() {
            let tp: Vec<Pos2> = (0..POINTS)
                .map(|k| {
                    let hz = geo.hz_at(k, POINTS);
                    pos2(geo.x(hz), geo.y(aequalis::fit::interpolate(&target, hz)))
                })
                .collect();
            p.add(egui::Shape::line(tp, Stroke::new(3.0, pal.text)));
        }
        if let Some(fit) = &m.fit {
            let fp: Vec<Pos2> = (0..POINTS)
                .map(|k| {
                    let hz = geo.hz_at(k, POINTS);
                    let d: f32 = fit
                        .bands
                        .iter()
                        .map(|b| aequalis::analog_db(b, sr, hz))
                        .sum();
                    pos2(geo.x(hz), geo.y(d))
                })
                .collect();
            p.add(egui::Shape::dashed_line(
                &fp,
                Stroke::new(2.0, pal.gold),
                6.0,
                3.0,
            ));
        }
    }

    // ---- hit-testing for this frame ------------------------------------------------------
    let pointer = resp.hover_pos();
    let near_dot = |pt: Pos2| -> Option<usize> {
        (0..EQ_BANDS)
            .filter(|i| used(slot, *i))
            .map(|i| (i, geo.dot(&bands[i]).distance(pt)))
            .filter(|(_, d)| *d <= DOT_R + 4.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let range_handle = |i: usize| -> Option<Pos2> {
        let b = &bands[i];
        (b.dynamic && b.shape.has_gain() && used(slot, i))
            .then(|| pos2(geo.x(b.freq), geo.y(b.gain_db + b.range_db)))
    };
    let near_handle = |pt: Pos2| -> Option<usize> {
        (0..EQ_BANDS).find(|i| range_handle(*i).is_some_and(|h| h.distance(pt) <= 8.0))
    };
    let curve_y = |x: f32| -> f32 {
        let f = ((x - rect.left()) / rect.width() * (POINTS - 1) as f32)
            .clamp(0.0, (POINTS - 1) as f32);
        let k = f.floor() as usize;
        let t = f - k as f32;
        let a = total[k];
        let b = total[(k + 1).min(POINTS - 1)];
        geo.y(a + (b - a) * t)
    };
    let grab_spec: &[f32] = if show.post { &ed.post } else { &ed.pre };
    let grab_peaks = if show.grab && ed.analyser.has_data() {
        peaks(grab_spec, &geo, floor)
    } else {
        Vec::new()
    };
    let peak_at = |k: usize| pos2(geo.x(geo.hz_at(k, POINTS)), geo.y_spec(grab_spec[k], floor));
    let near_peak = |pt: Pos2| -> Option<usize> {
        grab_peaks
            .iter()
            .copied()
            .find(|k| peak_at(*k).distance(pt) <= 10.0)
    };
    let hovered_band = pointer.and_then(near_dot);
    let hovered_handle = pointer.and_then(near_handle);

    // The ghost: what a double-click here would make, dashed.
    if let Some(pt) = pointer {
        if hovered_band.is_none()
            && hovered_handle.is_none()
            && ed.drag.is_none()
            && ed.rubber.is_none()
            && !show.grab
        {
            let on_curve = (pt.y - curve_y(pt.x)).abs() < 6.0;
            let shape = shape_at(rect, pt, on_curve);
            let gain = if shape.has_gain() && !on_curve {
                geo.db(pt.y).clamp(-30.0, 30.0)
            } else {
                0.0
            };
            let ghost = BandSettings {
                active: true,
                shape,
                freq: geo.hz(pt.x),
                gain_db: gain,
                q: if matches!(shape, EqShape::Bell | EqShape::Notch) {
                    1.0
                } else {
                    0.71
                },
                slope: 24.0,
                ..BandSettings::default()
            };
            if shape.has_gain() && gain.abs() > 0.2 || !shape.has_gain() {
                let gp: Vec<Pos2> = curve_of(&ghost, mode, sr, &geo)
                    .iter()
                    .enumerate()
                    .map(|(k, db)| pos2(geo.x(geo.hz_at(k, POINTS)), geo.y(*db)))
                    .collect();
                p.add(egui::Shape::dashed_line(
                    &gp,
                    Stroke::new(1.0, pal.text_dim.gamma_multiply(0.7)),
                    3.0,
                    4.0,
                ));
            }
        }
    }

    // ---- dots, handles, the plate ---------------------------------------------------------
    for i in 0..EQ_BANDS {
        if !used(slot, i) {
            continue;
        }
        let b = &bands[i];
        let c = geo.dot(b);
        let color = band_color(i);
        let selected = ed.selection.contains(&i);
        let on = b.active;
        if let Some(h) = range_handle(i) {
            p.line_segment([c, h], Stroke::new(1.0, color.gamma_multiply(0.7)));
            let up = b.range_db > 0.0;
            let tri = if up {
                vec![
                    pos2(h.x, h.y - 6.0),
                    pos2(h.x - 5.0, h.y + 3.0),
                    pos2(h.x + 5.0, h.y + 3.0),
                ]
            } else {
                vec![
                    pos2(h.x, h.y + 6.0),
                    pos2(h.x - 5.0, h.y - 3.0),
                    pos2(h.x + 5.0, h.y - 3.0),
                ]
            };
            let hot = hovered_handle == Some(i);
            p.add(egui::Shape::convex_polygon(
                tri,
                if hot {
                    color
                } else {
                    color.gamma_multiply(0.8)
                },
                Stroke::new(1.0, pal.text),
            ));
            // Where the band is right now.
            if on && deltas[i].abs() > 0.05 {
                let y = geo.y(b.gain_db + deltas[i]);
                p.line_segment(
                    [pos2(c.x - DOT_R - 3.0, y), pos2(c.x + DOT_R + 3.0, y)],
                    Stroke::new(2.5, color),
                );
            }
        }
        if on {
            p.circle_filled(c, DOT_R, color);
        } else {
            p.circle_filled(c, DOT_R, theme::recess(&pal));
            p.circle_stroke(c, DOT_R, Stroke::new(1.5, color));
        }
        if selected || hovered_band == Some(i) {
            p.circle_stroke(
                c,
                DOT_R + 2.5,
                Stroke::new(if selected { 2.0 } else { 1.0 }, pal.text),
            );
        }
        p.text(
            c,
            Align2::CENTER_CENTER,
            (i + 1).to_string(),
            design::paint::ruler(),
            if on {
                theme::on_color(&pal, color)
            } else {
                pal.text
            },
        );
        if b.placement != Placement::Stereo {
            let (glyph, pc) = placement_style(b.placement, &pal);
            let at = c + vec2(DOT_R + 1.0, -DOT_R - 1.0);
            p.circle_filled(at, 6.0, pc);
            p.text(
                at,
                Align2::CENTER_CENTER,
                glyph,
                design::paint::caption(),
                theme::on_color(&pal, pc),
            );
        }
        if solo == Some(i) {
            p.text(
                c + vec2(0.0, DOT_R + 3.0),
                Align2::CENTER_TOP,
                t("eq.solo_short"),
                design::paint::caption(),
                color,
            );
        }
    }
    if let Some(i) = ed.primary.filter(|i| used(slot, *i)) {
        plate(&p, &geo, &bands[i], i, mode, &pal);
    }

    // Spectrum Grab: the peaks, labelled, ready to be pulled into a band.
    if show.grab {
        for &k in &grab_peaks {
            let at = peak_at(k);
            let hot = pointer.is_some_and(|pt| at.distance(pt) <= 10.0);
            p.circle_stroke(
                at,
                if hot { 7.0 } else { 5.0 },
                Stroke::new(if hot { 2.0 } else { 1.5 }, pal.gold),
            );
            let hz = geo.hz_at(k, POINTS);
            let level = grab_spec[k] - show.tilt * (hz / 1_000.0).log2();
            let name = if ed.piano {
                parse::note_label(hz)
            } else {
                hz_label(hz)
            };
            p.text(
                at - vec2(0.0, 9.0),
                Align2::CENTER_BOTTOM,
                format!("{name}  {level:.0} dB"),
                design::paint::ruler(),
                pal.text,
            );
        }
        p.text(
            pos2(rect.center().x, rect.top() + space::S2),
            Align2::CENTER_TOP,
            t("eq.grab_hint"),
            design::paint::caption(),
            pal.gold,
        );
    }

    // The rubber band.
    if let Some((a, b)) = ed.rubber {
        let r = Rect::from_two_pos(a, b);
        p.rect_filled(r, 0.0, pal.selection.gamma_multiply(0.25));
        p.rect_stroke(r, 0.0, Stroke::new(1.0, pal.gold), StrokeKind::Inside);
    }

    // The cursor readout: frequency, note, the curve's gain, and the spectra's levels
    // there — the numbers the benchmark's users ask for.
    if let Some(pt) = pointer {
        let hz = geo.hz(pt.x);
        let k = (((pt.x - rect.left()) / rect.width()) * (POINTS - 1) as f32)
            .round()
            .clamp(0.0, (POINTS - 1) as f32) as usize;
        let untilt = |v: f32| v - show.tilt * (hz / 1_000.0).log2();
        p.vline(
            pt.x,
            rect.y_range(),
            Stroke::new(1.0, pal.text_dim.gamma_multiply(0.5)),
        );
        let mut text = format!(
            "{}  \u{b7}  {}  \u{b7}  {} {:+.1} dB",
            hz_label(hz),
            parse::note_label(hz),
            t("eq.readout.eq"),
            total[k]
        );
        if ed.analyser.has_data() {
            text.push_str(&format!(
                "  \u{b7}  {} {:.0}  {} {:.0} dB",
                t("eq.an.pre"),
                untilt(ed.pre[k]),
                t("eq.an.post"),
                untilt(ed.post[k])
            ));
        }
        if key_on {
            text.push_str(&format!(
                "  \u{b7}  {} {:.0} dB",
                t("eq.an.key"),
                untilt(ed.key[k])
            ));
        }
        p.text(
            pos2(rect.left() + space::S2 + space::S6, rect.top() + space::S2),
            Align2::LEFT_TOP,
            text,
            design::paint::ruler(),
            pal.text,
        );
    } else if !bands.iter().any(|b| b.active) && (0..EQ_BANDS).all(|i| !used(slot, i)) {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            t("eq.empty"),
            design::paint::empty_state(),
            pal.text_dim,
        );
    }
    // The legend of what is drawn, top right, so the lines are never a guess.
    legend(&p, rect, &show, key_on, &pal, key_color);
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(
            1.0,
            if resp.has_focus() {
                pal.gold
            } else {
                pal.grid_strong
            },
        ),
        StrokeKind::Inside,
    );

    let peak_q = |k: usize| -> (f32, f32) {
        let hz = geo.hz_at(k, POINTS);
        // Q from the peak's width: where it falls 3 dB either side.
        let v = grab_spec[k];
        let mut lo = k;
        while lo > 0 && grab_spec[lo - 1] > v - 3.0 {
            lo -= 1;
        }
        let mut hi = k;
        while hi + 1 < POINTS && grab_spec[hi + 1] > v - 3.0 {
            hi += 1;
        }
        let bw = (geo.hz_at(hi + 1, POINTS) / geo.hz_at(lo, POINTS)).max(1.01);
        (hz, (bw.sqrt() / (bw - 1.0)).clamp(1.0, 12.0))
    };
    // What is under the pointer now, and what was under it where the button went down: a
    // drag is recognised a few pixels after the press, and must start from what was
    // pressed, not from where the pointer has got to.
    let hits = |pt: Option<Pos2>| Hits {
        pos: pt,
        band: pt.and_then(near_dot),
        handle: pt.and_then(near_handle),
        on_curve: pt.is_some_and(|pt| (pt.y - curve_y(pt.x)).abs() < 6.0),
        peak: pt.and_then(near_peak).map(peak_q),
    };
    let press = ui
        .input(|i| i.pointer.press_origin())
        .filter(|pt| rect.contains(*pt));
    let gctx = GestureCtx {
        hovered_band,
        press: hits(press),
    };
    axis(app, ui, &axis_rect, &axis_resp, &geo, slot, bands);
    gestures(app, ui, &resp, strip, effect, slot, bands, &geo, gctx);
    if app.eq_editor.typing.is_some() {
        typing_popup(app, ui, strip, effect, bands, &geo);
    }
}

/// L, R, M, S: a letter and a colour each.
fn placement_style(pl: Placement, pal: &theme::Palette) -> (&'static str, Color32) {
    let pc = |i: usize| theme::track_color(cantus_core::Color::PALETTE[i]);
    match pl {
        Placement::Stereo => ("", pal.gold),
        Placement::Left => ("L", pal.text),
        Placement::Right => ("R", pc(4)),
        Placement::Mid => ("M", pc(3)),
        Placement::Side => ("S", pc(6)),
    }
}

fn legend(
    p: &egui::Painter,
    rect: Rect,
    show: &AnalyserView,
    key_on: bool,
    pal: &theme::Palette,
    key_color: Color32,
) {
    let mut x = rect.right() - space::S8 - space::S4;
    let y = rect.top() + space::S2 + 6.0;
    let mut item = |label: String, draw: &dyn Fn(&egui::Painter, Pos2)| {
        let g = p.layout_no_wrap(label, design::paint::caption(), pal.text_dim);
        x -= g.size().x;
        p.galley(pos2(x, y - g.size().y * 0.5), g, pal.text_dim);
        x -= space::S4;
        draw(p, pos2(x, y));
        x -= space::S4;
    };
    if show.key && key_on {
        item(t("eq.an.key"), &|p, at| {
            p.add(egui::Shape::dashed_line(
                &[at - vec2(8.0, 0.0), at + vec2(8.0, 0.0)],
                Stroke::new(1.5, key_color),
                4.0,
                2.0,
            ));
        });
    }
    if show.post {
        item(t("eq.an.post"), &|p, at| {
            p.line_segment(
                [at - vec2(8.0, 0.0), at + vec2(8.0, 0.0)],
                Stroke::new(1.5, pal.text_dim),
            );
        });
    }
    if show.pre {
        item(t("eq.an.pre"), &|p, at| {
            p.rect_filled(
                Rect::from_center_size(at, vec2(16.0, 8.0)),
                1.0,
                pal.text_dim.gamma_multiply(0.3),
            );
        });
    }
    if show.freeze {
        item(t("eq.an.frozen"), &|p, at| {
            p.circle_filled(at, 3.0, pal.gold);
        });
    }
}

/// The selected band's numbers, on a plate beside its dot — never over it.
fn plate(
    p: &egui::Painter,
    geo: &Geo,
    b: &BandSettings,
    i: usize,
    mode: Mode,
    pal: &theme::Palette,
) {
    let c = geo.dot(b);
    let mut line2 = hz_label(b.freq);
    if b.shape.has_gain() {
        line2.push_str(&format!("  {:+.1} dB", b.gain_db));
    }
    if b.shape.has_slope() {
        line2.push_str(&format!("  {:.0} dB/oct", b.slope));
    } else {
        line2.push_str(&format!("  Q {:.2}", b.q));
    }
    let mut line1 = format!(
        "{}  {}",
        tf("eq.band_n", &[("n", &(i + 1))]),
        t(&format!("eq.shape.{}", b.shape.index()))
    );
    if b.placement != Placement::Stereo {
        line1.push_str(&format!(
            "  \u{b7} {}",
            t(&format!("eq.place.{}", b.placement.index()))
        ));
    }
    if b.dynamic && b.shape.has_gain() {
        line1.push_str(&format!("  \u{b7} {}", t("fx.eq.dyn")));
    }
    if mode == Mode::LinearPhase && b.shape == EqShape::AllPass {
        line1.push_str(&format!("  \u{b7} {}", t("eq.allpass_lp")));
    }
    let g1 = p.layout_no_wrap(line1, design::paint::caption(), pal.text_dim);
    let g2 = p.layout_no_wrap(line2, design::paint::ruler(), pal.text);
    let w = g1.size().x.max(g2.size().x) + space::S4;
    let h = g1.size().y + g2.size().y + space::S2;
    let mut at = c + vec2(DOT_R + space::S3, -h - space::S2);
    if at.x + w > geo.rect.right() - space::S8 {
        at.x = c.x - DOT_R - space::S3 - w;
    }
    if at.y < geo.rect.top() + space::S6 {
        at.y = c.y + DOT_R + space::S3;
    }
    let r = Rect::from_min_size(at, vec2(w, h));
    p.rect(
        r,
        pal.corner_radius,
        pal.panel_raised.gamma_multiply(0.94),
        Stroke::new(1.0, band_color(i)),
        StrokeKind::Inside,
    );
    let g1h = g1.size().y;
    p.galley(r.min + vec2(space::S2, space::S1), g1, pal.text_dim);
    p.galley(r.min + vec2(space::S2, space::S1 + g1h), g2, pal.text);
}

/// The frequency axis under the plot — a ruler, or a piano — and its gestures: drag up
/// or down to zoom around the pointer, sideways to pan, double-click to reset, the wheel
/// to zoom.
fn axis(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: &Rect,
    resp: &egui::Response,
    geo: &Geo,
    slot: &EffectSlot,
    bands: &[BandSettings],
) {
    let pal = theme::p();
    let p = ui.painter_at(*rect);
    let ed = &mut app.eq_editor;
    let axis_geo = Geo {
        rect: *rect,
        ..*geo
    };
    if ed.piano {
        // A0 to C8, then as far as the view goes.
        let lo_note = parse::note_of(geo.lo).0 - 1;
        let hi_note = parse::note_of(geo.hi).0 + 1;
        let key_x = |n: i32, cents: f32| axis_geo.x(parse::note_hz(n as f32, cents));
        for n in lo_note..=hi_note {
            let black = matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10);
            if black {
                continue;
            }
            let r = Rect::from_x_y_ranges(
                key_x(n, -50.0).max(rect.left())..=key_x(n, 50.0).min(rect.right()),
                rect.y_range(),
            );
            if r.width() <= 0.0 {
                continue;
            }
            p.rect_filled(r, 0.0, pal.white_key);
            p.vline(r.left(), rect.y_range(), Stroke::new(1.0, pal.grid_strong));
            if n.rem_euclid(12) == 0 && r.width() > 1.5 {
                p.text(
                    pos2(r.center().x, rect.bottom() - 2.0),
                    Align2::CENTER_BOTTOM,
                    cantus_core::pitch_name(n.clamp(0, 127) as u8),
                    design::paint::caption(),
                    theme::on_light_key(&pal),
                );
            }
        }
        for n in lo_note..=hi_note {
            if matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10) {
                let r = Rect::from_x_y_ranges(
                    key_x(n, -40.0)..=key_x(n, 40.0),
                    rect.top()..=rect.top() + rect.height() * 0.6,
                );
                p.rect_filled(r, 0.0, pal.black_key);
            }
        }
    } else {
        p.rect_filled(*rect, 0.0, pal.panel);
        for (hz, strong, labelled) in grid_lines(geo.lo, geo.hi) {
            let x = axis_geo.x(hz);
            p.vline(
                x,
                rect.top()..=rect.top() + if strong { 6.0 } else { 3.0 },
                Stroke::new(1.0, pal.grid_strong),
            );
            if labelled {
                p.text(
                    pos2(x, rect.bottom() - 2.0),
                    Align2::CENTER_BOTTOM,
                    axis_label(hz),
                    design::paint::ruler(),
                    pal.text_dim,
                );
            }
        }
    }
    // Where each band sits on the axis.
    for (i, b) in bands.iter().enumerate() {
        if !used(slot, i) {
            continue;
        }
        let x = axis_geo.x(b.freq);
        if x < rect.left() || x > rect.right() {
            continue;
        }
        let tri = vec![
            pos2(x, rect.top() + 7.0),
            pos2(x - 4.0, rect.top()),
            pos2(x + 4.0, rect.top()),
        ];
        p.add(egui::Shape::convex_polygon(
            tri,
            band_color(i),
            Stroke::NONE,
        ));
    }
    if let Some(pt) = resp.hover_pos() {
        let hz = axis_geo.hz(pt.x);
        let text = if ed.piano {
            format!("{}  {}", parse::note_label(hz), hz_label(hz))
        } else {
            format!("{}  {}", hz_label(hz), parse::note_label(hz))
        };
        let g = p.layout_no_wrap(text, design::paint::ruler(), pal.text);
        let r = Rect::from_center_size(
            pos2(
                pt.x.clamp(rect.left() + 60.0, rect.right() - 60.0),
                rect.center().y,
            ),
            g.size() + vec2(space::S2, 2.0),
        );
        p.rect_filled(r, pal.corner_radius, pal.panel_raised);
        p.rect_stroke(
            r,
            pal.corner_radius,
            Stroke::new(1.0, pal.gold),
            StrokeKind::Inside,
        );
        p.galley(r.center() - g.size() * 0.5, g, pal.text);
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let resp = resp.clone().on_hover_text(t("eq.axis_tip"));
    // Zoom and pan.
    let clamp_view = |lo: f32, hi: f32| -> (f32, f32) {
        let (min, max) = VIEW_LIMITS;
        let (mut lo, mut hi) = (lo, hi);
        if hi / lo < 2.0 {
            let mid = (lo * hi).sqrt();
            lo = mid / 2f32.sqrt();
            hi = mid * 2f32.sqrt();
        }
        if lo < min {
            hi *= min / lo;
            lo = min;
        }
        if hi > max {
            lo /= hi / max;
            hi = max;
        }
        (lo.max(min), hi.min(max))
    };
    if resp.drag_started() {
        if let Some(pt) = resp.interact_pointer_pos() {
            ed.axis = Some(AxisDrag {
                anchor: axis_geo.hz(pt.x),
                view: ed.view,
                acc: Vec2::ZERO,
            });
        }
    }
    if let Some(a) = ed.axis.as_mut() {
        if resp.dragged() {
            a.acc += resp.drag_delta();
            let zoom = 2f32.powf(a.acc.y / 120.0);
            let (lo, hi) = a.view;
            let span = (hi / lo).powf(zoom);
            let frac = (a.anchor / lo).ln() / (hi / lo).ln();
            let new_lo = a.anchor / span.powf(frac);
            let shift = span.powf(-a.acc.x / rect.width().max(1.0));
            ed.view = clamp_view(new_lo * shift, new_lo * span * shift);
        }
    }
    if resp.drag_stopped() {
        ed.axis = None;
    }
    if resp.double_clicked() {
        ed.view = VIEW;
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(pt) = resp.hover_pos() {
                let anchor = axis_geo.hz(pt.x);
                let (lo, hi) = ed.view;
                let zoom = 2f32.powf(-scroll / 240.0);
                let span = (hi / lo).powf(zoom);
                let frac = (anchor / lo).ln() / (hi / lo).ln();
                let new_lo = anchor / span.powf(frac);
                ed.view = clamp_view(new_lo, new_lo * span);
            }
        }
    }
}

/// What the gesture code needs from the drawing pass.
struct GestureCtx {
    hovered_band: Option<usize>,
    press: Hits,
}

/// What is at a point of the plot.
struct Hits {
    pos: Option<Pos2>,
    band: Option<usize>,
    handle: Option<usize>,
    on_curve: bool,
    /// A grabbable peak: its frequency and the Q its width suggests.
    peak: Option<(f32, f32)>,
}

/// Wheel notches this frame, with the modifiers held.
fn wheel(ui: &egui::Ui) -> (f32, egui::Modifiers) {
    ui.input(|i| {
        let mut notches = 0.0;
        let mut mods = i.modifiers;
        for e in &i.raw.events {
            if let egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } = e
            {
                let d = if delta.y != 0.0 { delta.y } else { delta.x };
                notches += match unit {
                    egui::MouseWheelUnit::Point => d / 40.0,
                    egui::MouseWheelUnit::Line => d,
                    egui::MouseWheelUnit::Page => d * 8.0,
                };
                mods = *modifiers;
            }
        }
        (notches, mods)
    })
}

impl CantusApp {
    /// A merge key for a burst of wheel turns or key presses: presses within half a second
    /// are one undo step.
    fn eq_burst(&mut self, effect: EffectId, what: &str) -> MergeKey {
        let now = std::time::Instant::now();
        let fresh = self
            .eq_editor
            .burst_at
            .is_none_or(|t| now.duration_since(t).as_millis() > 500);
        self.eq_editor.burst_at = Some(now);
        if fresh {
            self.end_gesture();
            self.eq_editor.gesture += 1;
        }
        MergeKey::of(&("eq-burst", effect.0, what, self.eq_editor.gesture))
    }
}

#[allow(clippy::too_many_arguments)]
fn gestures(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    bands: &[BandSettings],
    geo: &Geo,
    g: GestureCtx,
) {
    let mods = ui.input(|i| i.modifiers);
    let fine = mods.shift;
    let rect = geo.rect;
    let piano = app.eq_editor.piano;
    let spec = &slot.spec;

    if resp.drag_started() || resp.clicked() || resp.double_clicked() {
        resp.request_focus();
    }

    // ---- press and drag ------------------------------------------------------------------
    if resp.drag_started() {
        let origin = g
            .press
            .pos
            .or(resp.interact_pointer_pos())
            .unwrap_or_default();
        // The pixels moved before the drag was recognised count too.
        let lead = resp
            .interact_pointer_pos()
            .map_or(Vec2::ZERO, |pt| pt - origin - resp.drag_delta());
        let key = app.eq_editor.next_gesture(effect);
        let ed = &mut app.eq_editor;
        if let Some(band) = g.press.handle {
            ed.drag = Some(Drag {
                kind: DragKind::Range {
                    band,
                    start: spec.get(&eq_key(band, "range")),
                },
                key,
                acc: lead,
                lock: None,
            });
        } else if let Some(band) = g.press.band {
            if !ed.selection.contains(&band) {
                if !(mods.command || mods.shift) {
                    ed.selection.clear();
                }
                ed.selection.insert(band);
            }
            ed.primary = Some(band);
            let start = ed
                .selection
                .iter()
                .map(|b| {
                    (
                        *b,
                        spec.get(&eq_key(*b, "freq")),
                        spec.get(&eq_key(*b, "gain")),
                        spec.get(&eq_key(*b, "q")),
                    )
                })
                .collect();
            ed.drag = Some(Drag {
                kind: DragKind::Bands {
                    start,
                    q_mode: mods.command,
                },
                key,
                acc: lead,
                lock: None,
            });
        } else if let Some((hz, q)) = g.press.peak {
            // Spectrum Grab: a bell at the peak, pulled from 0 dB.
            let made = app.add_eq_band_with(
                strip,
                effect,
                EqShape::Bell,
                hz,
                0.0,
                &[("q", q)],
                Some(key),
            );
            if let Some(band) = made {
                app.eq_editor.drag = Some(Drag {
                    kind: DragKind::Bands {
                        start: vec![(band, hz, 0.0, q)],
                        q_mode: false,
                    },
                    key,
                    acc: lead,
                    lock: None,
                });
            }
        } else if g.press.on_curve {
            // Pull the curve: a band where it was pressed, gain following the pointer.
            let shape = shape_at(rect, origin, true);
            let hz = geo.hz(origin.x);
            let extra: &[(&'static str, f32)] = if mods.alt {
                &[("dyn", 1.0), ("range", -6.0)]
            } else {
                &[]
            };
            let made = app.add_eq_band_with(strip, effect, shape, hz, 0.0, extra, Some(key));
            if let Some(band) = made {
                let q = if shape == EqShape::Bell { 1.0 } else { 0.71 };
                app.eq_editor.drag = Some(Drag {
                    kind: DragKind::Bands {
                        start: vec![(band, hz, 0.0, q)],
                        q_mode: false,
                    },
                    key,
                    acc: lead,
                    lock: None,
                });
            }
        } else {
            ed.rubber = Some((origin, origin));
        }
    }
    if resp.dragged() {
        let delta = resp.drag_delta() * if fine { 0.2 } else { 1.0 };
        if let Some((a, _)) = app.eq_editor.rubber {
            if let Some(pt) = resp.interact_pointer_pos() {
                app.eq_editor.rubber = Some((a, pt));
            }
        }
        if let Some(mut d) = app.eq_editor.drag.clone() {
            d.acc += delta;
            if mods.alt && d.lock.is_none() && d.acc.length() > 4.0 {
                d.lock = Some(d.acc.x.abs() >= d.acc.y.abs());
            }
            let (dx, dy) = match d.lock {
                Some(true) => (d.acc.x, 0.0),
                Some(false) => (0.0, d.acc.y),
                None => (d.acc.x, d.acc.y),
            };
            let mut commands = Vec::new();
            let mut widest = 0.0f32;
            match &d.kind {
                DragKind::Bands { start, q_mode } => {
                    for &(band, f0, g0, q0) in start {
                        let shape = bands[band].shape;
                        let mut f = geo.hz(geo.x(f0) + dx).clamp(10.0, 30_000.0);
                        if piano && !fine {
                            let (n, _) = parse::note_of(f);
                            f = parse::note_hz(n as f32, 0.0);
                        }
                        let mut fields = vec![("freq", f)];
                        if *q_mode {
                            let q = (q0 * 2f32.powf(-dy / 60.0)).clamp(0.025, 40.0);
                            fields.push(("q", q));
                        } else if shape.has_gain() {
                            let gain =
                                (g0 - dy / (rect.height() * 0.5) * geo.range).clamp(-30.0, 30.0);
                            widest = widest.max(gain.abs());
                            fields.push(("gain", gain));
                        }
                        commands.extend(CantusApp::band_cmds(strip, effect, band, &fields));
                    }
                }
                DragKind::Range { band, start } => {
                    let range = (start - dy / (rect.height() * 0.5) * geo.range).clamp(-30.0, 30.0);
                    widest = widest.max((bands[*band].gain_db + range).abs());
                    commands.extend(CantusApp::band_cmds(
                        strip,
                        effect,
                        *band,
                        &[("range", range)],
                    ));
                }
            }
            // The dB range grows to keep a dragged band on the display.
            app.eq_editor.range_db = app.eq_editor.range_for(widest);
            let key = d.key;
            app.eq_editor.drag = Some(d);
            app.eq_exec("cmd.eq_band", commands, Some(key));
        }
    }
    if resp.drag_stopped() {
        if let Some((a, b)) = app.eq_editor.rubber.take() {
            let r = Rect::from_two_pos(a, b);
            let inside: Vec<usize> = (0..EQ_BANDS)
                .filter(|i| used(slot, *i) && r.contains(geo.dot(&bands[*i])))
                .collect();
            let ed = &mut app.eq_editor;
            if !(mods.command || mods.shift) {
                ed.selection.clear();
            }
            ed.selection.extend(inside.iter().copied());
            ed.primary = inside.first().copied().or(ed.primary);
        }
        if app.eq_editor.drag.take().is_some() {
            app.end_gesture();
        }
    }

    // ---- clicks ----------------------------------------------------------------------------
    if resp.clicked() {
        match g.hovered_band {
            Some(b) if mods.alt => {
                let on = spec.get(&eq_key(b, "on")) >= 0.5;
                app.set_band(
                    strip,
                    effect,
                    b,
                    &[("on", if on { 0.0 } else { 1.0 })],
                    false,
                );
            }
            Some(b) => {
                let ed = &mut app.eq_editor;
                if mods.command {
                    if !ed.selection.remove(&b) {
                        ed.selection.insert(b);
                    }
                } else if mods.shift {
                    // A consecutive range of band numbers, from the primary to this one.
                    let from = ed.primary.unwrap_or(b);
                    let (lo, hi) = (from.min(b), from.max(b));
                    ed.selection.extend((lo..=hi).filter(|i| used(slot, *i)));
                } else {
                    ed.selection.clear();
                    ed.selection.insert(b);
                }
                ed.primary = Some(b);
            }
            None => {
                app.eq_editor.selection.clear();
                app.eq_editor.primary = None;
            }
        }
    }
    if resp.double_clicked() {
        match g.hovered_band {
            Some(b) => open_typing(app, b, &bands[b]),
            None => {
                if let Some(pt) = resp.interact_pointer_pos() {
                    let shape = shape_at(rect, pt, false);
                    let gain = if shape.has_gain() {
                        geo.db(pt.y).clamp(-30.0, 30.0)
                    } else {
                        0.0
                    };
                    let extra: &[(&'static str, f32)] = if mods.alt {
                        &[("dyn", 1.0), ("range", -6.0)]
                    } else {
                        &[]
                    };
                    app.add_eq_band_with(strip, effect, shape, geo.hz(pt.x), gain, extra, None);
                }
            }
        }
    }

    // ---- the wheel: Q (slope on a cut), Ctrl for gain, Alt for the dynamic range ------------
    let target: Vec<usize> = match (&app.eq_editor.drag, g.hovered_band) {
        (
            Some(Drag {
                kind: DragKind::Bands { start, .. },
                ..
            }),
            _,
        ) => start.iter().map(|s| s.0).collect(),
        (_, Some(b)) => vec![b],
        _ => Vec::new(),
    };
    if !target.is_empty() && resp.hovered() {
        let (notches, wmods) = wheel(ui);
        if notches != 0.0 {
            let what = if wmods.command {
                "gain"
            } else if wmods.alt {
                "range"
            } else {
                "q"
            };
            let key = app.eq_burst(effect, what);
            let step = if wmods.shift { 0.2 } else { 1.0 };
            let mut commands = Vec::new();
            for b in target {
                let band = &bands[b];
                let fields: Vec<(&str, f32)> = match what {
                    "gain" if band.shape.has_gain() => vec![(
                        "gain",
                        (spec.get(&eq_key(b, "gain")) + 0.5 * notches * step).clamp(-30.0, 30.0),
                    )],
                    "range" if band.shape.has_gain() => vec![
                        ("dyn", 1.0),
                        (
                            "range",
                            (spec.get(&eq_key(b, "range")) + 0.5 * notches * step)
                                .clamp(-30.0, 30.0),
                        ),
                    ],
                    "q" if band.shape.has_slope() => {
                        let s = spec.get(&eq_key(b, "slope"));
                        vec![("slope", (s + 6.0 * notches.signum()).clamp(6.0, 96.0))]
                    }
                    "q" => vec![(
                        "q",
                        (spec.get(&eq_key(b, "q")) * 2f32.powf(notches * step / 6.0))
                            .clamp(0.025, 40.0),
                    )],
                    _ => Vec::new(),
                };
                commands.extend(CantusApp::band_cmds(strip, effect, b, &fields));
            }
            app.eq_exec("cmd.eq_band", commands, Some(key));
        }
    }

    // ---- right-click: the band's menu, or the display's ------------------------------------
    let menu_band = g.hovered_band.or(app.eq_editor.primary);
    resp.context_menu(|ui| match menu_band.filter(|b| used(slot, *b)) {
        Some(b) => band_menu(app, ui, strip, effect, slot, b, &bands[b]),
        None => background_menu(app, ui, strip, effect, slot),
    });

    // ---- keyboard ------------------------------------------------------------------------------
    if resp.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
        keyboard(app, ui, strip, effect, slot, bands);
    }
}

fn open_typing(app: &mut CantusApp, band: usize, b: &BandSettings) {
    app.eq_editor.typing = Some(Typing {
        band,
        fields: [
            format!("{:.1}", b.freq),
            format!("{:.1}", b.gain_db),
            format!("{:.2}", b.q),
        ],
        focus: 0,
        fresh: true,
    });
    app.eq_editor.select(&[band]);
}

/// Keys, while the display has focus. Numbers select, arrows nudge, brackets change Q,
/// letters switch; Space, undo and redo still reach the app.
fn keyboard(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    bands: &[BandSettings],
) {
    let presses: Vec<(egui::Key, egui::Modifiers)> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            })
            .collect()
    });
    let used_bands: Vec<usize> = (0..EQ_BANDS).filter(|b| used(slot, *b)).collect();
    use egui::Key;
    for (key, m) in presses {
        let fine = m.shift;
        let digit = match key {
            Key::Num1 => Some(0),
            Key::Num2 => Some(1),
            Key::Num3 => Some(2),
            Key::Num4 => Some(3),
            Key::Num5 => Some(4),
            Key::Num6 => Some(5),
            Key::Num7 => Some(6),
            Key::Num8 => Some(7),
            Key::Num9 => Some(8),
            Key::Num0 => Some(9),
            _ => None,
        };
        if let Some(n) = digit {
            if used(slot, n) {
                if m.command {
                    app.eq_editor.selection.insert(n);
                    app.eq_editor.primary = Some(n);
                } else {
                    app.eq_editor.select(&[n]);
                }
            }
            continue;
        }
        match key {
            Key::Tab if !used_bands.is_empty() => {
                let cur = app.eq_editor.primary;
                let pos = cur.and_then(|c| used_bands.iter().position(|b| *b == c));
                let next = match (pos, m.shift) {
                    (Some(p), false) => used_bands[(p + 1) % used_bands.len()],
                    (Some(p), true) => used_bands[(p + used_bands.len() - 1) % used_bands.len()],
                    (None, _) => used_bands[0],
                };
                app.eq_editor.select(&[next]);
            }
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
                if !app.eq_editor.selection.is_empty() =>
            {
                let (semis, db) = match key {
                    Key::ArrowLeft => (-1.0, 0.0),
                    Key::ArrowRight => (1.0, 0.0),
                    Key::ArrowUp => (0.0, 0.5),
                    _ => (0.0, -0.5),
                };
                let k = if fine { 0.1 } else { 1.0 };
                app.eq_nudge_selection(strip, effect, semis * k, db * if fine { 0.2 } else { 1.0 });
            }
            Key::OpenBracket | Key::CloseBracket if !app.eq_editor.selection.is_empty() => {
                let step = if fine { 1.0 / 24.0 } else { 1.0 / 6.0 };
                let f = 2f32.powf(if key == Key::CloseBracket {
                    step
                } else {
                    -step
                });
                app.eq_scale_selection_q(strip, effect, f);
            }
            Key::Delete | Key::Backspace if !app.eq_editor.selection.is_empty() => {
                let sel: Vec<usize> = app.eq_editor.selection.iter().copied().collect();
                app.remove_eq_bands(strip, effect, &sel);
            }
            Key::B if !app.eq_editor.selection.is_empty() => {
                app.eq_toggle_selection(strip, effect, "on");
            }
            Key::D if !app.eq_editor.selection.is_empty() => {
                app.eq_toggle_selection(strip, effect, "dyn");
            }
            Key::S if m.is_none() => {
                let soloed = slot.spec.get("solo").round() as usize;
                match app.eq_editor.primary {
                    Some(p) if soloed != p + 1 => app.set_eq_solo(strip, effect, Some(p)),
                    _ if soloed > 0 => app.set_eq_solo(strip, effect, None),
                    _ => {}
                }
            }
            Key::P if !app.eq_editor.selection.is_empty() => {
                let first = app.eq_editor.primary.unwrap_or(0);
                let next = (bands[first].placement.index() + 1) % Placement::ALL.len();
                let sel: Vec<usize> = app.eq_editor.selection.iter().copied().collect();
                let mut commands = Vec::new();
                for b in sel {
                    commands.extend(CantusApp::band_cmds(
                        strip,
                        effect,
                        b,
                        &[("place", next as f32)],
                    ));
                }
                app.eq_exec("cmd.eq_band", commands, None);
            }
            Key::A if m.command => {
                app.eq_editor.selection = used_bands.iter().copied().collect();
                app.eq_editor.primary = used_bands.first().copied();
            }
            Key::Enter => {
                if let Some(p) = app.eq_editor.primary {
                    open_typing(app, p, &bands[p]);
                }
            }
            Key::Escape => {
                if app.eq_editor.drag.take().is_some() {
                    // Esc during a drag puts everything back.
                    app.end_gesture();
                    app.undo();
                } else if slot.spec.get("solo") >= 1.0 {
                    app.set_eq_solo(strip, effect, None);
                } else {
                    app.eq_editor.selection.clear();
                    app.eq_editor.primary = None;
                }
            }
            _ => {}
        }
    }
    // What is not the EQ's still reaches the app: play, stop, undo, redo, save.
    use crate::keymap::Action;
    let actions = app.keymap.pressed(ui.ctx(), true, true);
    for a in actions {
        if matches!(
            a,
            Action::PlayPause | Action::Stop | Action::Undo | Action::Redo | Action::Save
        ) {
            app.run_action(a);
        }
    }
}

fn band_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    b: usize,
    band: &BandSettings,
) {
    ui.label(
        RichText::new(tf("eq.band_n", &[("n", &(b + 1))]))
            .text_style(design::strong())
            .color(band_color(b)),
    );
    ui.menu_button(t("fx.eq.shape"), |ui| {
        for s in EqShape::ALL {
            if ui
                .selectable_label(band.shape == s, t(&format!("eq.shape.{}", s.index())))
                .clicked()
            {
                app.set_band(strip, effect, b, &[("shape", s.index() as f32)], false);
                ui.close();
            }
        }
    });
    ui.menu_button(t("fx.eq.place"), |ui| {
        for (i, pl) in Placement::ALL.iter().enumerate() {
            if ui
                .selectable_label(band.placement == *pl, t(&format!("eq.place.{i}")))
                .clicked()
            {
                app.set_band(strip, effect, b, &[("place", i as f32)], false);
                ui.close();
            }
        }
    });
    if ui.button(t("eq.split_lr")).clicked() {
        app.split_eq_band(strip, effect, b, false);
        ui.close();
    }
    if ui.button(t("eq.split_ms")).clicked() {
        app.split_eq_band(strip, effect, b, true);
        ui.close();
    }
    ui.separator();
    if band.shape.has_gain() && ui.selectable_label(band.dynamic, t("fx.eq.dyn")).clicked() {
        app.set_band(
            strip,
            effect,
            b,
            &[("dyn", if band.dynamic { 0.0 } else { 1.0 })],
            false,
        );
        ui.close();
    }
    let soloed = slot.spec.get("solo").round() as usize == b + 1;
    if ui.selectable_label(soloed, t("eq.solo")).clicked() {
        app.set_eq_solo(strip, effect, if soloed { None } else { Some(b) });
        ui.close();
    }
    let on = slot.spec.get(&eq_key(b, "on")) >= 0.5;
    if ui.selectable_label(!on, t("eq.bypass_band")).clicked() {
        app.set_band(
            strip,
            effect,
            b,
            &[("on", if on { 0.0 } else { 1.0 })],
            false,
        );
        ui.close();
    }
    if ui.button(t("eq.type_values")).clicked() {
        open_typing(app, b, band);
        ui.close();
    }
    ui.separator();
    if ui.button(t("eq.remove_band")).clicked() {
        app.remove_eq_band(strip, effect, b);
        ui.close();
    }
}

fn background_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    _strip: StripId,
    _effect: EffectId,
    slot: &EffectSlot,
) {
    let used_bands: Vec<usize> = (0..EQ_BANDS).filter(|b| used(slot, *b)).collect();
    if used_bands.is_empty() {
        ui.label(t("eq.empty"));
    }
    if !used_bands.is_empty() && ui.button(t("eq.select_all")).clicked() {
        app.eq_editor.selection = used_bands.iter().copied().collect();
        app.eq_editor.primary = used_bands.first().copied();
        ui.close();
    }
    if ui.button(t("eq.reset_view")).clicked() {
        app.eq_editor.view = VIEW;
        ui.close();
    }
}

/// The exact-entry plate for a band: Freq, Gain, Q boxes. Tab moves between them, Enter
/// sets all three as one step, Esc leaves without changing anything.
fn typing_popup(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    bands: &[BandSettings],
    geo: &Geo,
) {
    let Some(mut typing) = app.eq_editor.typing.clone() else {
        return;
    };
    let b = typing.band;
    let band = bands[b];
    let pal = theme::p();
    let at = geo.dot(&band) + vec2(DOT_R + space::S3, DOT_R + space::S2);
    let mut commit = false;
    let mut cancel = false;
    egui::Area::new(egui::Id::new(("eq-typing", effect.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            design::Elevation::Card.frame(&pal).show(ui, |ui| {
                ui.label(
                    RichText::new(tf("eq.band_n", &[("n", &(b + 1))]))
                        .text_style(design::strong())
                        .color(band_color(b)),
                );
                let labels = ["fx.eq.freq", "fx.eq.gain", "fx.eq.q"];
                egui::Grid::new(("eq-typing-grid", effect.0))
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (k, label) in labels.iter().enumerate() {
                            if k == 1 && !band.shape.has_gain() {
                                continue;
                            }
                            ui.label(t(label));
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut typing.fields[k])
                                    .desired_width(space::GRID * 22.0)
                                    .font(egui::TextStyle::Monospace),
                            );
                            if typing.fresh && k == 0 {
                                r.request_focus();
                                typing.fresh = false;
                            }
                            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                commit = true;
                            }
                            ui.end_row();
                        }
                    });
                ui.label(
                    RichText::new(t("eq.type_hint"))
                        .text_style(design::caption())
                        .color(pal.text_dim),
                );
                ui.horizontal(|ui| {
                    if ui.button(t("eq.type_set")).clicked() {
                        commit = true;
                    }
                    if ui.button(t("layout.cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        });
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if commit {
        let mut fields = Vec::new();
        if let Some(f) = parse::parse_freq(&typing.fields[0]) {
            fields.push(("freq", f.clamp(10.0, 30_000.0)));
        }
        if band.shape.has_gain() {
            if let Some(g) = parse::parse_gain(&typing.fields[1]) {
                fields.push(("gain", g.clamp(-30.0, 30.0)));
                app.eq_editor.range_db = app.eq_editor.range_for(g.clamp(-30.0, 30.0));
            }
        }
        if let Some(q) = parse::parse_q(&typing.fields[2]) {
            fields.push(("q", q.clamp(0.025, 40.0)));
        }
        if fields.is_empty() {
            app.set_error(t("eq.type_error"));
            app.eq_editor.typing = Some(typing);
        } else {
            app.set_band(strip, effect, b, &fields, false);
            app.eq_editor.typing = None;
        }
    } else if cancel {
        app.eq_editor.typing = None;
    } else {
        app.eq_editor.typing = Some(typing);
    }
}
