//! A console fader: a recessed groove, a raised cap with a read line, and a dB scale.
//!
//! ## Why this is not an `egui::Slider`
//!
//! egui paints a slider's rail and its handle from the same `inactive.bg_fill`, so at
//! rest the two can differ only by the handle's outline. That reads as a rail with a lump
//! on it rather than as a control with a cap you grab. A fader also wants a *scale* — the
//! dB marks that tell you where unity is without reading a number — and a read line
//! across the cap, which is the thing your eye actually lines up with the scale. None of
//! that is reachable through `Slider`'s builder, so this is a widget of our own.
//!
//! ## The taper
//!
//! A fader is not linear in decibels, and it is not linear in amplitude either. Linear dB
//! spends half the travel on −60…−30 dB, a range nobody mixes in; linear amplitude
//! crushes the whole useful range into the top centimetre. Real faders compress the
//! bottom and keep the top even, which is what [`ANCHORS`] describes:
//!
//! **Through the useful range — everything above −24 dB — the fader is exactly 6 dB per
//! 10 % of travel, and unity sits at 80 %.** Below that the scale steepens, so the last
//! fifth of the travel covers −24 dB down to silence. The anchors are interpolated
//! linearly in dB, which keeps [`db_at`] and [`pos_of`] exact inverses of each other
//! (there is a test).
//!
//! Silence is a real position, not a clamp: the bottom of the travel is
//! [`MIN_GAIN_DB`], which [`db_to_linear`](cantus_core::db_to_linear) turns into a hard
//! zero. A fader pulled all the way down is off.

use crate::design::{self, space};
use crate::theme;
use cantus_core::{MAX_GAIN_DB, MIN_GAIN_DB};
use egui::{
    pos2, vec2, Align2, CornerRadius, EventFilter, Key, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Widget, WidgetInfo,
};

// ---- the taper ----------------------------------------------------------------------

/// Where 0 dB sits on the travel, measured from the bottom.
pub const UNITY_POS: f32 = 0.80;

/// `(position from the bottom, decibels)`, ascending. Interpolated linearly in dB.
/// One table for the mixer and for automation (ADR 0027): a curve drawn at 0 dB plays
/// 0 dB.
const ANCHORS: [(f32, f32); 6] = cantus_core::automation::range::FADER_ANCHORS;

/// The decibel value at a position on the travel (0 = bottom, 1 = top).
pub fn db_at(pos: f32) -> f32 {
    let pos = pos.clamp(0.0, 1.0);
    for w in ANCHORS.windows(2) {
        let ((p0, d0), (p1, d1)) = (w[0], w[1]);
        if pos <= p1 {
            let t = if p1 > p0 { (pos - p0) / (p1 - p0) } else { 0.0 };
            return d0 + t * (d1 - d0);
        }
    }
    MAX_GAIN_DB
}

/// Where a decibel value sits on the travel. The exact inverse of [`db_at`].
pub fn pos_of(db: f32) -> f32 {
    let db = if db.is_finite() {
        db.clamp(MIN_GAIN_DB, MAX_GAIN_DB)
    } else {
        MIN_GAIN_DB
    };
    for w in ANCHORS.windows(2) {
        let ((p0, d0), (p1, d1)) = (w[0], w[1]);
        if db <= d1 {
            let t = if d1 > d0 { (db - d0) / (d1 - d0) } else { 0.0 };
            return p0 + t * (p1 - p0);
        }
    }
    1.0
}

/// What a fader reads as. `-inf` at the bottom of the travel, because "−96.0 dB" is a
/// number pretending to be a state: that position is silence, and it should say so.
pub fn read_out(db: f32) -> String {
    if db <= MIN_GAIN_DB {
        "-\u{221e} dB".to_string()
    } else {
        format!("{db:+.1} dB")
    }
}

/// A mark on the scale. `label` is drawn when the fader is long enough for the labels not
/// to collide, and is `None` for the in-between marks that are only there for rhythm.
struct Mark {
    db: f32,
    label: Option<&'static str>,
}

const fn mark(db: f32, label: Option<&'static str>) -> Mark {
    Mark { db, label }
}

/// The printed scale. Labelled marks are the ones a mixing engineer actually aims for;
/// the unlabelled ones keep the eye from having to measure between them.
const MARKS: [Mark; 11] = [
    mark(MAX_GAIN_DB, Some("+12")),
    mark(6.0, None),
    mark(0.0, Some("0")),
    mark(-6.0, None),
    mark(-12.0, Some("-12")),
    mark(-18.0, None),
    mark(-24.0, None),
    mark(-30.0, Some("-30")),
    mark(-40.0, None),
    mark(-60.0, Some("-60")),
    mark(MIN_GAIN_DB, Some("-\u{221e}")),
];

// ---- geometry -----------------------------------------------------------------------

/// The cap's size along the travel. Narrow, so the read line is a precise reading.
const CAP_ALONG: f32 = space::S3;
/// The cap's size across the travel — twice its length, the way a fader cap is shaped.
const CAP_ACROSS: f32 = space::S6;
/// The cap across the travel in a compact fader (a track header row is 20 px tall).
const CAP_ACROSS_COMPACT: f32 = space::S4;
/// Width of the slot the cap rides in.
const GROOVE: f32 = space::S1;
/// The scale column beside a vertical fader: labels, then the marks.
const SCALE_W: f32 = space::S8;
/// The scale row under a horizontal fader.
const SCALE_H: f32 = space::S4;
/// A labelled mark reaches this far from the groove; the others reach half as far.
const MARK_LONG: f32 = space::S2;
/// Fine mode: a drag or a wheel notch covers a fifth of the distance.
const FINE: f32 = 0.2;
/// One arrow-key press, in decibels (a tenth of that with Shift).
const KEY_STEP_DB: f32 = 1.0;
/// Below this length the marks are drawn without labels — they would collide.
const LABELS_NEED: f32 = 96.0;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Axis {
    Vertical,
    Horizontal,
}

/// A fader. `db` is the value in decibels, in `MIN_GAIN_DB..=MAX_GAIN_DB`.
///
/// ```ignore
/// let mut gain = track.gain_db;
/// let r = ui.add(Fader::new(&mut gain).vertical(160.0).wheel(true));
/// if r.changed() { /* issue the command */ }
/// if r.drag_stopped() { app.end_gesture(); }
/// ```
pub struct Fader<'a> {
    db: &'a mut f32,
    axis: Axis,
    len: f32,
    /// Draw the dB scale beside the groove.
    scale: bool,
    /// The cap's size across the travel.
    across: f32,
    /// Where a double-click puts it.
    default_db: f32,
    /// Whether the wheel moves it. Off by default: a fader inside a scroll area that
    /// scrolls the same axis would move *and* scroll, and the caller is the only one who
    /// knows which axis its scroll area owns.
    wheel: bool,
    /// Accessibility name; also what the tooltip-less screen reader announces.
    name: String,
}

impl<'a> Fader<'a> {
    pub fn new(db: &'a mut f32) -> Self {
        Fader {
            db,
            axis: Axis::Vertical,
            len: space::GRID * 40.0,
            scale: true,
            across: CAP_ACROSS,
            default_db: 0.0,
            wheel: false,
            name: String::new(),
        }
    }

    /// A vertical fader `len` points tall — a mixer strip.
    pub fn vertical(mut self, len: f32) -> Self {
        self.axis = Axis::Vertical;
        self.len = len;
        self
    }

    /// A horizontal fader `len` points wide — an inspector row.
    pub fn horizontal(mut self, len: f32) -> Self {
        self.axis = Axis::Horizontal;
        self.len = len;
        self
    }

    /// No scale column and a slimmer cap, for a dense row that has no space for either.
    /// The unity mark stays: without it the control would have no reference at all.
    pub fn compact(mut self) -> Self {
        self.scale = false;
        self.across = CAP_ACROSS_COMPACT;
        self
    }

    /// What a double-click resets to. Unity unless something else makes more sense.
    pub fn default_db(mut self, db: f32) -> Self {
        self.default_db = db;
        self
    }

    /// Let the wheel move this fader. Only safe when the enclosing scroll area does not
    /// scroll along the fader's own axis.
    pub fn wheel(mut self, on: bool) -> Self {
        self.wheel = on;
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// The space a fader asks for: the lane, plus the scale column or row when it has one.
fn size(axis: Axis, len: f32, scale: bool, across: f32) -> egui::Vec2 {
    match axis {
        Axis::Vertical => vec2(if scale { SCALE_W } else { 0.0 } + across, len),
        Axis::Horizontal => vec2(len, across + if scale { SCALE_H } else { 0.0 }),
    }
}

/// Everything the painting needs, resolved once from the allocated rectangle.
struct Geo {
    axis: Axis,
    /// The strip the cap rides in: the groove and the cap, without the scale.
    lane: Rect,
    /// Coordinate of the bottom (or left) end of the travel.
    from: f32,
    /// Coordinate of the top (or right) end of the travel.
    to: f32,
    across: f32,
}

impl Geo {
    fn new(axis: Axis, rect: Rect, scale: bool, across: f32) -> Geo {
        let lane = match axis {
            Axis::Vertical => Rect::from_min_max(
                pos2(rect.right() - across, rect.top()),
                pos2(rect.right(), rect.bottom()),
            ),
            Axis::Horizontal => Rect::from_min_max(
                rect.min,
                pos2(
                    rect.right(),
                    rect.bottom() - if scale { SCALE_H } else { 0.0 },
                ),
            ),
        };
        let (from, to) = match axis {
            Axis::Vertical => (
                lane.bottom() - CAP_ALONG / 2.0,
                lane.top() + CAP_ALONG / 2.0,
            ),
            Axis::Horizontal => (
                lane.left() + CAP_ALONG / 2.0,
                lane.right() - CAP_ALONG / 2.0,
            ),
        };
        Geo {
            axis,
            lane,
            from,
            to,
            across,
        }
    }

    /// Length of the travel in points. Never zero, so the drag maths cannot divide by it.
    fn travel(&self) -> f32 {
        (self.to - self.from).abs().max(1.0)
    }

    fn coord(&self, pos: f32) -> f32 {
        self.from + (self.to - self.from) * pos.clamp(0.0, 1.0)
    }

    fn groove(&self) -> Rect {
        match self.axis {
            Axis::Vertical => Rect::from_min_max(
                pos2(self.lane.center().x - GROOVE / 2.0, self.lane.top()),
                pos2(self.lane.center().x + GROOVE / 2.0, self.lane.bottom()),
            ),
            Axis::Horizontal => Rect::from_min_max(
                pos2(self.lane.left(), self.lane.center().y - GROOVE / 2.0),
                pos2(self.lane.right(), self.lane.center().y + GROOVE / 2.0),
            ),
        }
    }

    fn cap(&self, pos: f32) -> Rect {
        let c = self.coord(pos);
        match self.axis {
            Axis::Vertical => {
                Rect::from_center_size(pos2(self.lane.center().x, c), vec2(self.across, CAP_ALONG))
            }
            Axis::Horizontal => {
                Rect::from_center_size(pos2(c, self.lane.center().y), vec2(CAP_ALONG, self.across))
            }
        }
    }

    /// A mark reaching `reach` points out of the lane, on the scale's side of it.
    fn mark_line(&self, pos: f32, reach: f32) -> (egui::Pos2, egui::Pos2) {
        let c = self.coord(pos);
        match self.axis {
            Axis::Vertical => (pos2(self.lane.left() - reach, c), pos2(self.lane.left(), c)),
            Axis::Horizontal => (
                pos2(c, self.lane.bottom()),
                pos2(c, self.lane.bottom() + reach),
            ),
        }
    }

    /// The unity mark spans the whole lane, so it is the one reference that survives even
    /// when there is no room for a scale.
    fn unity_line(&self) -> (egui::Pos2, egui::Pos2) {
        let c = self.coord(UNITY_POS);
        match self.axis {
            Axis::Vertical => (pos2(self.lane.left(), c), pos2(self.lane.right(), c)),
            Axis::Horizontal => (pos2(c, self.lane.top()), pos2(c, self.lane.bottom())),
        }
    }
}

impl Widget for Fader<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Fader {
            db,
            axis,
            len,
            scale,
            across,
            default_db,
            wheel,
            name,
        } = self;
        let (rect, mut response) =
            ui.allocate_exact_size(size(axis, len, scale, across), Sense::click_and_drag());
        let geo = Geo::new(axis, rect, scale, across);
        let horizontal = axis == Axis::Horizontal;

        let start = (*db).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        let mut pos = pos_of(start);
        let fine = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };

        if response.dragged() {
            let d = response.drag_delta();
            let along = if horizontal { d.x } else { -d.y };
            if along != 0.0 {
                pos += along * fine / geo.travel();
            }
        }
        if wheel && response.hovered() {
            let d = ui.input(|i| i.smooth_scroll_delta);
            let along = if horizontal { d.x } else { d.y };
            if along != 0.0 {
                pos += along * fine / geo.travel();
            }
        }
        if response.has_focus() {
            // Stop Tab-less arrow navigation from moving focus off the fader...
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    EventFilter {
                        horizontal_arrows: horizontal,
                        vertical_arrows: !horizontal,
                        ..Default::default()
                    },
                );
            });
            // ...and stop the application's own dispatch from nudging the note selection
            // with the same key press (see the module docs).
            super::claim_arrow_keys(ui.ctx(), response.id);
            let (dec, inc) = if horizontal {
                (Key::ArrowLeft, Key::ArrowRight)
            } else {
                (Key::ArrowDown, Key::ArrowUp)
            };
            let steps = ui.input(|i| i.num_presses(inc) as i32 - i.num_presses(dec) as i32);
            if steps != 0 {
                // In decibels, not in pixels: a fader nudged by the keyboard should move
                // by an amount you can name.
                let step = KEY_STEP_DB * if fine < 1.0 { 0.1 } else { 1.0 };
                pos = pos_of(db_at(pos) + steps as f32 * step);
            }
        }
        if response.double_clicked() {
            pos = pos_of(default_db);
        }

        let value = db_at(pos.clamp(0.0, 1.0));
        if value != start {
            *db = value;
            response.mark_changed();
        }
        let pos = pos_of(value);

        if ui.is_rect_visible(rect) {
            paint(
                ui,
                &geo,
                &response,
                pos,
                scale,
                scale && len >= labels_need(axis),
            );
        }
        response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), value as f64, &name));
        response
    }
}

fn labels_need(axis: Axis) -> f32 {
    match axis {
        Axis::Vertical => LABELS_NEED,
        // Horizontal labels sit side by side, so they need more room than stacked ones.
        Axis::Horizontal => LABELS_NEED * 2.0,
    }
}

// ---- painting -----------------------------------------------------------------------

fn paint(ui: &Ui, geo: &Geo, response: &Response, pos: f32, scale: bool, labels: bool) {
    let pal = theme::p();
    let p = ui.painter();
    let radius = CornerRadius::same(2);

    // The groove is a slot cut into the surface, so it is *darker* than the surface in
    // every theme — see `theme::recess`. This is the whole reason the widget exists:
    // egui paints the rail and the handle from one colour, and a slot you cannot see is
    // not a slot.
    let groove = geo.groove();
    p.rect_filled(groove, radius, theme::recess(&pal));
    p.rect_stroke(
        groove,
        radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    // Scale. A compact fader has no column to draw it in, so it draws none: the marks
    // would land outside the widget, on top of whatever is next to it.
    if scale {
        for m in &MARKS {
            let mp = pos_of(m.db);
            let long = m.label.is_some();
            let (a, b) = geo.mark_line(mp, if long { MARK_LONG } else { MARK_LONG * 0.5 });
            p.line_segment(
                [a, b],
                Stroke::new(1.0, if long { pal.text_dim } else { pal.grid_bar }),
            );
            if labels {
                if let Some(text) = m.label {
                    let (anchor, at) = match geo.axis {
                        Axis::Vertical => (
                            Align2::RIGHT_CENTER,
                            pos2(geo.lane.left() - MARK_LONG - 2.0, a.y),
                        ),
                        Axis::Horizontal => (
                            Align2::CENTER_TOP,
                            pos2(a.x, geo.lane.bottom() + MARK_LONG + 1.0),
                        ),
                    };
                    p.text(at, anchor, text, design::paint::ruler(), pal.text_dim);
                }
            }
        }
    }
    // Unity: the one mark that crosses the whole lane, so it reads as a datum rather than
    // as another tick — and so a compact fader with no scale still has a reference.
    let (ua, ub) = geo.unity_line();
    p.line_segment([ua, ub], Stroke::new(1.0, pal.text_dim));

    // The cap.
    let cap = geo.cap(pos);
    p.rect_filled(cap, radius, pal.panel_raised);
    // Rest, hover and grabbed are three different edges — never three different fills,
    // which is what makes a light theme's "pressed" look like a disabled control.
    let edge = if response.dragged() || response.has_focus() {
        Stroke::new(2.0, pal.gold)
    } else if response.hovered() {
        Stroke::new(1.0, pal.text_dim)
    } else {
        Stroke::new(1.0, pal.grid_bar)
    };
    p.rect_stroke(cap, radius, edge, StrokeKind::Inside);
    // The read line: the thing the eye actually lines up with the scale.
    let inset = 3.0;
    let (la, lb) = match geo.axis {
        Axis::Vertical => (
            pos2(cap.left() + inset, cap.center().y),
            pos2(cap.right() - inset, cap.center().y),
        ),
        Axis::Horizontal => (
            pos2(cap.center().x, cap.top() + inset),
            pos2(cap.center().x, cap.bottom() - inset),
        ),
    };
    p.line_segment([la, lb], Stroke::new(1.5, pal.text));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        design::install_style(&ctx);
        ctx
    }

    fn frame(ctx: &egui::Context, mut add: impl FnMut(&mut Ui)) -> Vec<egui::epaint::ClippedShape> {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 400.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| add(ui));
        out.textures_delta.clear();
        out.shapes
    }

    /// Every filled rectangle in the frame, with its colour.
    fn rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, egui::Color32)> {
        fn walk(s: &egui::Shape, out: &mut Vec<(Rect, egui::Color32)>) {
            match s {
                egui::Shape::Rect(r) if r.fill != egui::Color32::TRANSPARENT => {
                    out.push((r.rect, r.fill))
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for s in shapes {
            walk(&s.shape, &mut out);
        }
        out
    }

    #[test]
    fn the_taper_is_monotonic_and_round_trips() {
        let mut last = f32::NEG_INFINITY;
        for i in 0..=1000 {
            let pos = i as f32 / 1000.0;
            let db = db_at(pos);
            assert!(db >= last, "the taper went backwards at pos {pos}");
            last = db;
            // pos → dB → pos must land where it started, or the cap would jump under the
            // pointer on the frame after a drag.
            assert!(
                (pos_of(db) - pos).abs() < 1e-3,
                "pos {pos} → {db} dB → {}",
                pos_of(db)
            );
        }
        assert_eq!(
            db_at(0.0),
            MIN_GAIN_DB,
            "the bottom of the travel is silence"
        );
        assert_eq!(db_at(1.0), MAX_GAIN_DB);
        assert!(db_at(UNITY_POS).abs() < 1e-4, "unity is at {UNITY_POS}");
    }

    #[test]
    fn the_useful_range_is_six_db_per_tenth_of_travel() {
        // The promise the scale makes: above −24 dB the fader is even, so "down 3 dB" is
        // the same distance wherever you are, and the marks are evenly spaced.
        for (a, b) in [(0.4, 0.5), (0.5, 0.6), (0.7, 0.8), (0.9, 1.0)] {
            let span = db_at(b) - db_at(a);
            assert!(
                (span - 6.0).abs() < 1e-3,
                "{a}..{b} covers {span} dB, not 6"
            );
        }
        // And the bottom compresses instead: silence is reachable without spending the
        // whole fader on it.
        assert!(db_at(0.1) < -60.0);
    }

    #[test]
    fn out_of_range_and_nan_land_somewhere_sane() {
        assert_eq!(pos_of(f32::NAN), 0.0);
        assert_eq!(pos_of(-1000.0), 0.0);
        assert_eq!(pos_of(1000.0), 1.0);
        assert_eq!(db_at(-1.0), MIN_GAIN_DB);
        assert_eq!(db_at(2.0), MAX_GAIN_DB);
        assert_eq!(read_out(MIN_GAIN_DB), "-\u{221e} dB");
        assert_eq!(read_out(0.0), "+0.0 dB");
    }

    /// The defect this widget exists to fix: egui paints a slider's rail and its handle
    /// from the same `inactive.bg_fill`, so at rest the two are the same colour.
    ///
    /// The palette is process-wide and tests run in parallel, so this asserts the colour
    /// rule against every shipped palette *without* installing one, and asserts the
    /// painting against whichever palette happens to be current.
    #[test]
    fn the_groove_and_the_cap_are_different_colours() {
        for (_, preset) in crate::theme::PRESETS {
            let theme = preset();
            let pal = theme.palette();
            assert_ne!(
                theme::recess(&pal),
                pal.panel_raised,
                "{}: the cap and the groove would be the same colour — the very bug this                  widget replaces",
                theme.name
            );
        }
        let ctx = ctx();
        let pal = theme::p();
        let mut db = 0.0f32;
        let shapes = frame(&ctx, |ui| {
            ui.add(Fader::new(&mut db).vertical(160.0));
        });
        let found = rects(&shapes);
        assert!(
            found.iter().any(|(_, c)| *c == theme::recess(&pal)),
            "no groove painted in the recess colour"
        );
        assert!(
            found.iter().any(|(_, c)| *c == pal.panel_raised),
            "no cap painted"
        );
    }

    #[test]
    fn the_cap_sits_where_the_value_says_and_the_scale_agrees() {
        let ctx = ctx();
        let pal = theme::p();
        // Unity: the cap's read line must land on the unity datum, or the scale is
        // decorative rather than a scale.
        let mut db = 0.0f32;
        let shapes = frame(&ctx, |ui| {
            ui.add(Fader::new(&mut db).vertical(200.0));
        });
        let cap = rects(&shapes)
            .into_iter()
            .find(|(_, c)| *c == pal.panel_raised)
            .expect("cap")
            .0;
        let mut lines = Vec::new();
        fn walk(s: &egui::Shape, out: &mut Vec<(egui::Pos2, egui::Pos2)>) {
            match s {
                egui::Shape::LineSegment { points, .. } => out.push((points[0], points[1])),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        for s in &shapes {
            walk(&s.shape, &mut lines);
        }
        // The unity datum is the horizontal line that spans the whole lane.
        let unity = lines
            .iter()
            .filter(|(a, b)| (a.y - b.y).abs() < 0.01 && (b.x - a.x) >= CAP_ACROSS - 0.01)
            .map(|(a, _)| a.y)
            .fold(f32::INFINITY, f32::min);
        assert!(
            (unity - cap.center().y).abs() < 0.51,
            "at 0 dB the cap sits at y={} but unity is drawn at y={unity}",
            cap.center().y
        );

        // Silence: the cap sits at the very bottom of the travel.
        let mut db = MIN_GAIN_DB;
        let shapes = frame(&ctx, |ui| {
            ui.add(Fader::new(&mut db).vertical(200.0));
        });
        let low = rects(&shapes)
            .into_iter()
            .find(|(_, c)| *c == pal.panel_raised)
            .expect("cap")
            .0;
        assert!(low.center().y > cap.center().y, "silence is below unity");
    }

    #[test]
    fn a_compact_fader_is_shorter_and_still_shows_unity() {
        assert!(
            size(Axis::Horizontal, 90.0, false, CAP_ACROSS_COMPACT).y
                < size(Axis::Horizontal, 90.0, true, CAP_ACROSS).y
        );
        let ctx = ctx();
        let mut db = -6.0f32;
        let shapes = frame(&ctx, |ui| {
            ui.add(Fader::new(&mut db).horizontal(90.0).compact());
        });
        let mut lines = 0;
        fn walk(s: &egui::Shape, n: &mut usize) {
            match s {
                egui::Shape::LineSegment { .. } => *n += 1,
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, n)),
                _ => {}
            }
        }
        for s in &shapes {
            walk(&s.shape, &mut lines);
        }
        // The unity datum and the read line: a compact fader keeps both and nothing else.
        assert_eq!(lines, 2, "a compact fader drew {lines} lines, expected 2");
    }
}
