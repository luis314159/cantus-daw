//! The automation clip editor (ADR 0027): an automation clip opened in the piano roll's
//! area — FL's automation clip, large. The curve over the clip's content with its window
//! braced, the value axis in the parameter's own units, and the tools:
//!
//! - **Edit** — click to add a point, drag points (a selection moves together), drag a
//!   segment's ring to bend it, drag on empty ground to select, Delete to remove,
//!   double-click a point to delete it, right-click a point for its shape;
//! - **Draw** — paint the curve with the pointer, one point per grid step: **Slide**
//!   draws ramps between them, **Step** holds each until the next;
//! - the shape of the selected points; FL's articulators: flip either way, scale,
//!   smooth, randomise a little, normalise, and an LFO over the selection or the clip.
//!
//! Every edit is one `SetClipAutomation`, merged over a drag: one undo step per gesture.

use super::auto_curve::{self as curve, Frame};
use super::clips_chrome::Toolbar;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::icons::Icon;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{self, Fill, Style, Tok};
use cantus_core::automation::edit::{self, Lfo};
use cantus_core::{AutoPoint, Automation, ClipId, Command, MergeKey, Shape, Ticks, TrackId};
use egui::{pos2, vec2, Align2, Id, Pos2, Rect, Sense};

/// The editor's tools.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Edit,
    Draw,
}

/// What a press in the canvas is doing.
#[derive(Clone, Debug)]
enum Gesture {
    /// Points `sel` moved together from where they were at the press.
    Move {
        at_press: Automation,
        anchor: usize,
    },
    Bend {
        at_press: Automation,
        seg: usize,
    },
    Marquee {
        from: Pos2,
        to: Pos2,
    },
    Draw {
        at_press: Automation,
        painted: Vec<(Ticks, f32)>,
    },
}

/// The editor's state, kept between frames on the app.
#[derive(Clone, Debug, Default)]
pub struct EditorState {
    pub tool: Tool,
    /// Draw holds (Step) instead of ramps (Slide).
    pub step: bool,
    /// Selected points, by index, sorted.
    pub selected: Vec<usize>,
    /// The clip the view was fitted to, so a newly opened clip is fitted once.
    fitted: Option<ClipId>,
    /// Points per content tick, and the content tick at the canvas's left edge.
    pub px_per_tick: f32,
    pub scroll: f64,
    gesture: Option<Gesture>,
    /// How many cycles an LFO draws (0 reads as 4).
    pub lfo_cycles: u32,
    /// Where the curve was drawn last frame (published for tests and for the input).
    pub frame: Option<Frame>,
    pub canvas: Option<Rect>,
}

/// The value axis's width, the ruler's height, the clip window brace's height.
const AXIS_W: f32 = 64.0;
const RULER_H: f32 = 22.0;

/// Show the editor for an automation clip (the piano roll calls this).
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId) {
    let s = Style::current();
    toolbar(app, ui, track, clip);
    let full = ui.available_rect_before_wrap();
    if full.height() < 80.0 || full.width() < 160.0 {
        return;
    }
    let Some((auto, window)) = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .and_then(|c| c.automation.clone().map(|a| (a, c.window())))
    else {
        return;
    };
    let axis = Rect::from_min_max(full.min, pos2(full.left() + s.s(AXIS_W), full.bottom()));
    let ruler = Rect::from_min_max(
        pos2(axis.right(), full.top()),
        pos2(full.right(), full.top() + s.s(RULER_H)),
    );
    let canvas = Rect::from_min_max(pos2(axis.right(), ruler.bottom()), full.max);
    ui.allocate_rect(full, Sense::hover());
    // Fit the clip's content to the width the first time it is opened.
    let st = &mut app.automation.editor;
    let content_len = window.period().unwrap_or(window.length).max(
        auto.points
            .last()
            .map_or(Ticks(1), |p| p.tick - window.offset + Ticks(1)),
    );
    if st.fitted != Some(clip) || st.px_per_tick <= 0.0 {
        st.fitted = Some(clip);
        st.selected.clear();
        st.gesture = None;
        let span = (content_len.0 as f32 * 1.08).max(cantus_core::PPQ as f32);
        st.px_per_tick = canvas.width() / span;
        st.scroll = window.offset.0 as f64 - content_len.0 as f64 * 0.02;
    }
    navigate(app, ui, canvas);
    let st = &app.automation.editor;
    let pad = s.s(10.0);
    let f = Frame {
        x0: canvas.left() - st.scroll as f32 * st.px_per_tick,
        px_per_tick: st.px_per_tick,
        top: canvas.top() + pad,
        bottom: canvas.bottom() - pad,
        clip_left: canvas.left(),
        clip_right: canvas.right(),
        origin: window.start - window.offset,
    };
    paint(app, ui, &auto, window, f, canvas, ruler, axis, &s);
    let st = &mut app.automation.editor;
    st.frame = Some(f);
    st.canvas = Some(canvas);
    input(app, ui, track, clip, &auto, f, canvas, &s);
}

/// The editor's toolbar: the clip, the tools, Slide/Step, the selection's shape, the
/// articulators.
fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId) {
    let id = ui.id().with("auto-editor-toolbar");
    let mut bar = Toolbar::new(ui);
    let r = (track, clip);
    let Some(auto) = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .and_then(|c| c.automation.clone())
    else {
        return;
    };
    {
        let a: &CantusApp = app;
        let name = crate::automation::target_name(a.project(), &auto.target);
        let clip_name = a
            .project()
            .track(track)
            .and_then(|t| t.clip(clip))
            .map(|c| c.name.clone());
        bar.title(ui, Src::Str(&name), clip_name.as_deref());
    }
    bar.divider(ui);
    for (tool, icon, key) in [
        (Tool::Edit, Icon::Select, "auto.tool.edit"),
        (Tool::Draw, Icon::Draw, "auto.tool.draw"),
    ] {
        let lit = app.automation.editor.tool == tool;
        if bar
            .key(ui, id.with(key), icon, lit, true, || t(key))
            .is_some_and(|r| r.clicked())
        {
            app.automation.editor.tool = tool;
        }
    }
    let step = app.automation.editor.step;
    if bar
        .led(ui, id.with("step"), step, "auto.step", 58.0, || {
            t("auto.step_tip")
        })
        .is_some_and(|r| r.clicked())
    {
        app.automation.editor.step = !step;
    }
    bar.divider(ui);
    // The shape of the selected points (of all, with none selected).
    let sel = app.automation.editor.selected.clone();
    let shown = sel
        .first()
        .and_then(|i| auto.points.get(*i))
        .or(auto.points.first())
        .map_or(Shape::Curve, |p| p.shape);
    let caption = t("auto.shape");
    if let Some(resp) = bar.field(
        ui,
        id.with("shape"),
        Some(&caption),
        &t(shown.label_key()),
        84.0,
        || t("auto.shape_tip"),
    ) {
        egui::Popup::menu(&resp)
            .frame(crate::views::clips_chrome::menu_frame())
            .show(|ui| {
                let items: Vec<kit::menu::Item<'_>> = Shape::ALL
                    .iter()
                    .map(|sh| kit::menu::Item::Action {
                        label: Src::Key(sh.label_key()),
                        check: kit::menu::Check::Radio(*sh == shown),
                        shortcut: None,
                        submenu: false,
                        enabled: true,
                    })
                    .collect();
                if let Some(i) = kit::widgets::menu_list(ui, &items, 180.0).1.clicked {
                    let mut a = auto.clone();
                    for (k, p) in a.points.iter_mut().enumerate() {
                        if sel.is_empty() || sel.contains(&k) {
                            p.shape = Shape::ALL[i];
                        }
                    }
                    commit(app, r, a, false);
                    ui.close();
                }
            });
    }
    bar.divider(ui);
    let len = clip_len(app, r);
    for (key, w) in [
        ("auto.art.flip_v_short", 58.0),
        ("auto.art.flip_h_short", 58.0),
        ("auto.art.smooth_short", 64.0),
        ("auto.art.randomize_short", 64.0),
        ("auto.art.normalize_short", 54.0),
    ] {
        let word = t(key);
        let tip_key = key.trim_end_matches("_short");
        let Some(resp) = bar.word(ui, id.with(key), &word, w, || t(tip_key)) else {
            continue;
        };
        if !resp.clicked() {
            continue;
        }
        let mut a = auto.clone();
        match key {
            "auto.art.flip_v_short" => edit::flip_vertical(&mut a.points, &sel),
            "auto.art.flip_h_short" => edit::flip_horizontal(&mut a.points, Ticks::ZERO, len),
            "auto.art.smooth_short" => edit::smooth(&mut a.points, 0.5),
            "auto.art.randomize_short" => {
                edit::randomize(&mut a.points, &sel, 0.08, app.bus.revision())
            }
            _ => edit::normalize(&mut a.points),
        }
        app.automation.editor.selected.clear();
        commit(app, r, a, false);
    }
    let scale = t("auto.art.scale_short");
    if let Some(resp) = bar.word(ui, id.with("scale"), &scale, 58.0, || t("auto.art.scale")) {
        egui::Popup::menu(&resp)
            .frame(crate::views::clips_chrome::menu_frame())
            .show(|ui| {
                scale_menu(app, ui, r, &auto, &sel);
            });
    }
    let lfo = t("auto.art.lfo_short");
    if let Some(resp) = bar.word(ui, id.with("lfo"), &lfo, 54.0, || t("auto.art.lfo")) {
        egui::Popup::menu(&resp)
            .frame(crate::views::clips_chrome::menu_frame())
            .show(|ui| {
                lfo_menu(app, ui, r, &auto, len);
            });
    }
}

/// The clip's content length: its loop's period, or its window.
fn clip_len(app: &CantusApp, r: (TrackId, ClipId)) -> Ticks {
    app.project()
        .track(r.0)
        .and_then(|t| t.clip(r.1))
        .map_or(Ticks::ZERO, |c| c.period().unwrap_or(c.length) + c.offset)
}

/// Write the curve: one step, or merged into the gesture's.
fn commit(app: &mut CantusApp, r: (TrackId, ClipId), a: Automation, merge: bool) {
    let cmd = Command::SetClipAutomation {
        track: r.0,
        clip: r.1,
        automation: Some(a),
    };
    if merge {
        app.exec_merging(cmd, MergeKey::of(&("auto-editor", r.1 .0)));
    } else {
        app.exec(cmd);
    }
}

/// Scale: values about the middle (or the selection's), and time about the start.
fn scale_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    r: (TrackId, ClipId),
    auto: &Automation,
    sel: &[usize],
) {
    use kit::menu::Item;
    let items = [
        Item::Band(Src::Key("auto.scale.values")),
        Item::action(Src::Key("auto.scale.half")),
        Item::action(Src::Key("auto.scale.more")),
        Item::Band(Src::Key("auto.scale.time")),
        Item::action(Src::Key("auto.scale.time_half")),
        Item::action(Src::Key("auto.scale.time_double")),
    ];
    let Some(i) = kit::widgets::menu_list(ui, &items, 200.0).1.clicked else {
        return;
    };
    let mut a = auto.clone();
    let picked: Vec<&AutoPoint> = a
        .points
        .iter()
        .enumerate()
        .filter(|(k, _)| sel.is_empty() || sel.contains(k))
        .map(|(_, p)| p)
        .collect();
    let (lo, hi) = picked.iter().fold((1f32, 0f32), |(lo, hi), p| {
        (lo.min(p.value), hi.max(p.value))
    });
    let pivot = if hi >= lo { (lo + hi) * 0.5 } else { 0.5 };
    match i {
        1 => edit::scale_values(&mut a.points, sel, pivot, 0.5),
        2 => edit::scale_values(&mut a.points, sel, pivot, 1.5),
        4 => edit::scale_time(&mut a.points, Ticks::ZERO, 0.5),
        5 => edit::scale_time(&mut a.points, Ticks::ZERO, 2.0),
        _ => return,
    }
    commit(app, r, a, false);
    ui.close();
}

/// FL's *create sequence*: an LFO over the selection's span, or the whole clip.
fn lfo_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    r: (TrackId, ClipId),
    auto: &Automation,
    len: Ticks,
) {
    use kit::menu::Item;
    let mut items: Vec<Item<'_>> = vec![Item::Band(Src::Key("auto.art.lfo"))];
    items.extend(
        Lfo::ALL
            .iter()
            .map(|l| Item::action(Src::Key(l.label_key()))),
    );
    items.push(Item::Band(Src::Key("auto.lfo.cycles")));
    let cycles = match app.automation.editor.lfo_cycles {
        0 => 4,
        n => n,
    };
    for (n, word) in [(1u32, "1"), (2, "2"), (4, "4"), (8, "8"), (16, "16")] {
        items.push(Item::Action {
            label: Src::Str(word),
            check: kit::menu::Check::Radio(cycles == n),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    let Some(i) = kit::widgets::menu_list(ui, &items, 180.0).1.clicked else {
        return;
    };
    let shapes = Lfo::ALL.len();
    if (1..=shapes).contains(&i) {
        let sel = &app.automation.editor.selected;
        let (from, to) = if sel.len() >= 2 {
            let a = auto.points[sel[0]].tick;
            let b = auto.points[*sel.last().unwrap_or(&sel[0])].tick;
            (a, b)
        } else {
            (Ticks::ZERO, len)
        };
        if to > from {
            let (lo, hi) = (0.2, 0.8);
            let new = edit::lfo(
                Lfo::ALL[i - 1],
                from,
                to,
                cycles,
                lo,
                hi,
                app.bus.revision(),
            );
            let mut a = auto.clone();
            edit::splice(&mut a.points, from, to + Ticks(1), new);
            app.automation.editor.selected.clear();
            commit(app, r, a, false);
        }
        ui.close();
    } else if i > shapes + 1 {
        let n = [1u32, 2, 4, 8, 16][i - shapes - 2];
        app.automation.editor.lfo_cycles = n;
    }
}

/// The wheel: Ctrl zooms time about the pointer, otherwise it scrolls; the middle button
/// pans.
fn navigate(app: &mut CantusApp, ui: &mut egui::Ui, canvas: Rect) {
    let Some(p) = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| canvas.contains(*p))
    else {
        return;
    };
    let (dx, dy, ctrl, zoom) = ui.input(|i| {
        (
            i.smooth_scroll_delta.x,
            i.smooth_scroll_delta.y,
            i.modifiers.command,
            i.zoom_delta(),
        )
    });
    let st = &mut app.automation.editor;
    let at = st.scroll + ((p.x - canvas.left()) / st.px_per_tick) as f64;
    if ctrl || zoom != 1.0 {
        let k = if zoom != 1.0 {
            zoom
        } else {
            (dy * 0.003).exp()
        };
        st.px_per_tick = (st.px_per_tick * k).clamp(0.002, 2.0);
        st.scroll = at - ((p.x - canvas.left()) / st.px_per_tick) as f64;
    } else if dx != 0.0 || dy != 0.0 {
        st.scroll -= ((dx + dy) / st.px_per_tick) as f64;
    }
    if ui.input(|i| i.pointer.middle_down()) {
        let d = ui.input(|i| i.pointer.delta());
        st.scroll -= (d.x / st.px_per_tick) as f64;
    }
}

/// Paint the ruler, the value axis, the grid, the clip's window, the curve, the points
/// and rings, the playhead and a marquee.
#[allow(clippy::too_many_arguments)]
fn paint(
    app: &CantusApp,
    ui: &egui::Ui,
    auto: &Automation,
    window: cantus_core::ClipWindow,
    f: Frame,
    canvas: Rect,
    ruler: Rect,
    axis: Rect,
    s: &Style,
) {
    let t = &s.t;
    let k = s.k;
    let project = app.project();
    let range = cantus_core::automation::range::range_of(project, &auto.target);
    let st = &app.automation.editor;
    let painter = ui.painter().with_clip_rect(canvas.union(ruler).union(axis));
    // The bars and beats through the metre changes (ADR 0060); the step chosen by the
    // metre at the window's left.
    // Content ticks here; the bars are the timeline's where the clip lies (review 2).
    let map = crate::views::bars::map(project);
    // With one metre the lines are the content's own, as the snapping is (review 3).
    let shift = if map.is_one_metre() {
        Ticks::ZERO
    } else {
        f.origin
    };
    let lo = f.tick(canvas.left()).floor() as i64;
    let bar = map.bar_len_at(Ticks(lo) + shift).0.max(1);
    let beat = map.beat_len_at(Ticks(lo) + shift).0.max(1);
    let hi = f.tick(canvas.right()).ceil() as i64;
    let pointer = ui.input(|i| i.pointer.hover_pos());
    kit::with_brush(&painter, |b| {
        b.register(canvas, "auto_editor", &[Tok::Ink, Tok::Grid], &[]);
        b.rect(b.snap_rect(canvas), Fill::Solid(t.row_black));
        // Outside the clip's window, the ground is dimmed: what is there does not play.
        let (w0, w1) = (
            window.offset,
            window.offset + window.period().unwrap_or(window.length),
        );
        let (x0, x1) = (f.x(w0.0 as f64), f.x(w1.0 as f64));
        for side in [
            Rect::from_min_max(
                canvas.min,
                pos2(x0.clamp(canvas.left(), canvas.right()), canvas.bottom()),
            ),
            Rect::from_min_max(
                pos2(x1.clamp(canvas.left(), canvas.right()), canvas.top()),
                canvas.max,
            ),
        ] {
            if side.width() > 0.0 {
                b.rect(
                    side,
                    Fill::Solid(with_alpha(t.ink, if t.dark { 0.45 } else { 0.12 })),
                );
            }
        }
        // Beats and bars, as the arrangement draws them.
        let step = if (beat as f32 * f.px_per_tick) >= 10.0 {
            beat
        } else {
            bar
        };
        // Through the metre changes (ADR 0060).
        crate::views::bars::lines(
            &map,
            Ticks(lo) + shift,
            Ticks(hi + 1) + shift,
            step == beat,
            |tick, is_bar| {
                let x = f.x((tick - shift).0 as f64);
                b.vline(
                    x,
                    canvas.top(),
                    canvas.bottom(),
                    b.px() * if is_bar { 1.5 } else { 1.0 },
                    if is_bar { t.grid_bar } else { t.grid_strong },
                );
            },
        );
        // Value lines at quarters, the middle stronger.
        for q in 1..4 {
            let y = f.y(q as f32 * 0.25);
            b.hline(
                canvas.left(),
                canvas.right(),
                y,
                b.px(),
                if q == 2 { t.grid_strong } else { t.grid },
            );
        }
        // The window's brace on the ruler, and the ruler's bar numbers.
        b.rect(b.snap_rect(ruler), Fill::Solid(t.panel));
        let brace = Rect::from_min_max(
            pos2(x0.max(ruler.left()), ruler.bottom() - 5.0 * k),
            pos2(x1.min(ruler.right()), ruler.bottom() - 1.0 * k),
        );
        if brace.width() > 0.0 {
            b.rrect(brace, 1.5 * k, Fill::Solid(with_alpha(t.selection, 0.8)));
        }
        let from = map.bar_start(Ticks(lo) + shift);
        for line in map.bar_lines(from, Ticks(hi + 1) + shift) {
            let barn = map.bbt(line).0 - 1;
            let x = f.x((line - shift).0 as f64);
            if x >= ruler.left() - 1.0 && x <= ruler.right() {
                b.vline(x, ruler.top() + 6.0 * k, ruler.bottom(), b.px(), t.text_dim);
                let label = Rect::from_min_size(
                    pos2(x + 3.0 * k, ruler.top()),
                    vec2(40.0 * k, ruler.height() - 6.0 * k),
                );
                let n = (barn + 1).to_string();
                let _ = text::paint(
                    b,
                    label,
                    &Text::new(&n, Role::Value, t.text_dim).align(Align2::LEFT_CENTER),
                    s.tk(),
                );
            }
        }
        // The value axis: the top, the middle and the bottom of the range, in its units.
        b.rect(b.snap_rect(axis), Fill::Solid(t.panel));
        if let Some(range) = range {
            let mut v = String::new();
            for q in [1.0f32, 0.75, 0.5, 0.25, 0.0] {
                v.clear();
                crate::automation::write_value(project, &auto.target, range.value(q), &mut v);
                let y = f.y(q);
                let r = Rect::from_min_max(
                    pos2(axis.left() + 4.0 * k, y - 8.0 * k),
                    pos2(axis.right() - 6.0 * k, y + 8.0 * k),
                );
                let _ = text::paint(
                    b,
                    r,
                    &Text::new(&v, Role::Value, t.text_dim).align(Align2::RIGHT_CENTER),
                    s.tk(),
                );
                b.hline(axis.right() - 4.0 * k, axis.right(), y, b.px(), t.text_dim);
            }
        }
        // The curve, in the clip's colour.
        let ink = ink_of(app, auto);
        let mut pts = Vec::new();
        curve::trace(
            auto,
            Ticks(lo.max(0) - 1).min(Ticks::ZERO),
            Ticks(hi.max(1) + 1),
            &f,
            2.0,
            &mut pts,
        );
        curve::paint(b, &pts, f.bottom, curve::Ink::of(ink, 2.0 * k, s));
        // Rings, then points.
        for i in 0..auto.points.len().saturating_sub(1) {
            if let Some(c) = curve::handle_at(auto, i, &f) {
                if canvas.contains(c) {
                    let hot = pointer.is_some_and(|p| p.distance(c) <= 7.0 * k);
                    curve::handle(b, c, 3.5 * k, ink, hot);
                }
            }
        }
        for (i, p) in auto.points.iter().enumerate() {
            let c = f.point(p.tick, p.value);
            if !canvas.expand(6.0).contains(c) {
                continue;
            }
            let lit = st.selected.contains(&i);
            let hot = pointer.is_some_and(|q| q.distance(c) <= 7.0 * k);
            if lit {
                b.circle(c, 6.5 * k, Fill::Solid(with_alpha(t.selection, 0.35)));
            }
            curve::point(
                b,
                c,
                if hot { 5.0 * k } else { 4.0 * k },
                if lit { t.selection } else { ink },
                t.row_black,
                lit || hot,
            );
        }
        // The playhead, where it is inside the clip.
        let at = app.engine.position();
        if let Some(c) = clip_content_at(app, window, at) {
            let x = f.x(c.0 as f64);
            if x >= canvas.left() && x <= canvas.right() {
                b.vline(x, ruler.top(), canvas.bottom(), 1.5 * k, t.playhead);
            }
        }
        if let Some(Gesture::Marquee { from, to }) = st.gesture.as_ref() {
            let r = Rect::from_two_pos(*from, *to);
            b.rect(r, Fill::Solid(with_alpha(t.selection, 0.12)));
            b.rrect_stroke(r, 0.0, b.px(), t.selection);
        }
    });
}

/// The content tick shown at timeline tick `at`, if the clip covers it.
fn clip_content_at(_app: &CantusApp, w: cantus_core::ClipWindow, at: Ticks) -> Option<Ticks> {
    if at < w.start || at >= w.start + w.length {
        return None;
    }
    let d = (at - w.start).0;
    Some(match w.period() {
        Some(p) => w.offset + Ticks(d.rem_euclid(p.0)),
        None => w.offset + Ticks(d),
    })
}

/// The curve's colour: its clip's (or its track's), made safe for a line on the ground.
fn ink_of(app: &CantusApp, auto: &Automation) -> egui::Color32 {
    let s = Style::current();
    let color = app
        .project()
        .tracks
        .iter()
        .find_map(|t| {
            t.clips
                .iter()
                .find(|c| c.automation.as_ref() == Some(auto))
                .map(|c| c.color.unwrap_or(t.color))
        })
        .unwrap_or(cantus_core::Color::PALETTE[0]);
    crate::kit::color::safe_track(
        crate::theme::track_color(color),
        s.t.dark,
        crate::kit::color::FillSize::Small,
    )
}

/// The pointer in the canvas: points, rings, the marquee, drawing; Delete and Ctrl+A.
#[allow(clippy::too_many_arguments)]
fn input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    clip: ClipId,
    auto: &Automation,
    f: Frame,
    canvas: Rect,
    s: &Style,
) {
    let k = s.k;
    let r = (track, clip);
    let resp = ui.interact(
        canvas,
        Id::new("auto-editor-canvas"),
        Sense::click_and_drag(),
    );
    let pointer = resp.hover_pos();
    let tool = app.automation.editor.tool;
    let near_point = |p: Pos2| curve::point_at(auto, &f, p, 7.0 * k);
    let near_ring = |p: Pos2| curve::handle_under(auto, &f, p, 7.0 * k);
    if let Some(p) = pointer {
        let icon = match (tool, near_point(p), near_ring(p)) {
            (Tool::Draw, ..) => egui::CursorIcon::Crosshair,
            (_, Some(_), _) => egui::CursorIcon::Grab,
            (_, None, Some(_)) => egui::CursorIcon::ResizeVertical,
            _ => egui::CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(icon);
        let project = app.project();
        if let Some(range) = cantus_core::automation::range::range_of(project, &auto.target) {
            let mut v = String::new();
            let pos = near_point(p)
                .and_then(|i| auto.points.get(i))
                .map_or(f.value(p.y), |q| q.value);
            crate::automation::write_value(project, &auto.target, range.value(pos), &mut v);
            hint::set_text(ui.ctx(), &v, &t("auto.hint.editor"), Caps::PARAM);
        }
    }
    let snap = |app: &CantusApp, tick: f64, free: bool| -> Ticks {
        let t = Ticks(tick.max(0.0).round() as i64);
        if app.arrangement.snap && !free {
            curve::snap_point(
                app.project(),
                f.origin,
                t,
                app.arrangement.grid.min(Ticks::beats(1)).max(Ticks(1)),
            )
        } else {
            t
        }
    };
    let free = ui.input(|i| i.modifiers.alt);
    let shift = ui.input(|i| i.modifiers.shift);

    if resp.drag_started_by(egui::PointerButton::Primary) {
        let press = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
        let st = &mut app.automation.editor;
        st.gesture = Some(match (tool, near_point(press), near_ring(press)) {
            (Tool::Draw, ..) => Gesture::Draw {
                at_press: auto.clone(),
                painted: Vec::new(),
            },
            (_, Some(i), _) => {
                if !st.selected.contains(&i) {
                    if !shift {
                        st.selected.clear();
                    }
                    st.selected.push(i);
                    st.selected.sort_unstable();
                }
                Gesture::Move {
                    at_press: auto.clone(),
                    anchor: i,
                }
            }
            (_, None, Some(seg)) => Gesture::Bend {
                at_press: auto.clone(),
                seg,
            },
            _ => Gesture::Marquee {
                from: press,
                to: press,
            },
        });
    }
    if resp.dragged_by(egui::PointerButton::Primary) {
        if let Some(pos) = resp.interact_pointer_pos() {
            let g = app.automation.editor.gesture.clone();
            match g {
                Some(Gesture::Move { at_press, anchor }) => {
                    let press = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
                    let a = moved(app, &at_press, anchor, &f, press, pos, free);
                    commit(app, r, a, true);
                }
                Some(Gesture::Bend { at_press, seg }) => {
                    let mut a = at_press.clone();
                    let (p, q) = (a.points[seg], a.points[seg + 1]);
                    let v = f.value(pos.y);
                    let mid = if (q.value - p.value).abs() > 1e-6 {
                        (v - p.value) / (q.value - p.value)
                    } else {
                        0.5
                    };
                    a.points[seg].tension = edit::tension_for_mid(p.shape, mid);
                    commit(app, r, a, true);
                }
                Some(Gesture::Marquee { from, .. }) => {
                    app.automation.editor.gesture = Some(Gesture::Marquee { from, to: pos });
                }
                Some(Gesture::Draw {
                    at_press,
                    mut painted,
                }) => {
                    let grid = draw_grid(app);
                    let at = curve::snap_point(
                        app.project(),
                        f.origin,
                        Ticks(f.tick(pos.x).max(0.0).round() as i64),
                        grid,
                    );
                    paint_at(&mut painted, at, f.value(pos.y), grid);
                    let a = drawn(&at_press, &painted, app.automation.editor.step);
                    app.automation.editor.gesture = Some(Gesture::Draw { at_press, painted });
                    commit(app, r, a, true);
                }
                None => {}
            }
        }
    }
    if resp.drag_stopped() {
        if let Some(Gesture::Marquee { from, to }) = app.automation.editor.gesture.take() {
            let band = Rect::from_two_pos(from, to);
            let st = &mut app.automation.editor;
            if !shift {
                st.selected.clear();
            }
            for (i, p) in auto.points.iter().enumerate() {
                if band.contains(f.point(p.tick, p.value)) && !st.selected.contains(&i) {
                    st.selected.push(i);
                }
            }
            st.selected.sort_unstable();
        }
        app.automation.editor.gesture = None;
        app.end_gesture();
    }
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            match near_point(p) {
                Some(i) => {
                    let st = &mut app.automation.editor;
                    if !shift {
                        st.selected.clear();
                    }
                    st.selected.push(i);
                    st.selected.dedup();
                }
                None if near_ring(p).is_none() => {
                    // A new point where the click was, of the drawing mode's shape.
                    let mut a = auto.clone();
                    let mut pt = AutoPoint::new(snap(app, f.tick(p.x), free), f.value(p.y));
                    if app.automation.editor.step {
                        pt.shape = Shape::Hold;
                    }
                    let i = edit::insert(&mut a.points, pt);
                    app.automation.editor.selected = vec![i];
                    commit(app, r, a, false);
                }
                None => {}
            }
        }
    }
    if resp.double_clicked() {
        if let Some(i) = pointer.and_then(near_point) {
            let mut a = auto.clone();
            if a.points.len() > 1 {
                a.points.remove(i);
                app.automation.editor.selected.clear();
                commit(app, r, a, false);
            }
        }
    }
    if let Some(i) = pointer.and_then(near_point) {
        egui::Popup::context_menu(&resp)
            .frame(crate::views::clips_chrome::menu_frame())
            .show(|ui| {
                let items: Vec<kit::menu::Item<'_>> = Shape::ALL
                    .iter()
                    .map(|sh| kit::menu::Item::Action {
                        label: Src::Key(sh.label_key()),
                        check: kit::menu::Check::Radio(
                            auto.points.get(i).is_some_and(|p| p.shape == *sh),
                        ),
                        shortcut: None,
                        submenu: false,
                        enabled: true,
                    })
                    .chain([
                        kit::menu::Item::Separator,
                        kit::menu::Item::action(Src::Key("auto.point.delete")),
                    ])
                    .collect();
                if let Some(c) = kit::widgets::menu_list(ui, &items, 200.0).1.clicked {
                    let mut a = auto.clone();
                    if c < Shape::ALL.len() {
                        a.points[i].shape = Shape::ALL[c];
                    } else if a.points.len() > 1 {
                        a.points.remove(i);
                        app.automation.editor.selected.clear();
                    }
                    commit(app, r, a, false);
                    ui.close();
                }
            });
    }
    // Keys, while the pointer is over the editor: Delete, Ctrl+A.
    if resp.hovered() || resp.has_focus() {
        let (del, all) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.modifiers.command && i.key_pressed(egui::Key::A),
            )
        });
        if all {
            app.automation.editor.selected = (0..auto.points.len()).collect();
        }
        let sel = app.automation.editor.selected.clone();
        if del && !sel.is_empty() && sel.len() < auto.points.len() {
            let mut a = auto.clone();
            let mut k = 0;
            a.points.retain(|_| {
                let keep = !sel.contains(&k);
                k += 1;
                keep
            });
            app.automation.editor.selected.clear();
            commit(app, r, a, false);
        }
    }
}

/// The curve with the selection moved by the pointer's travel: in time (snapped, as the
/// anchor lands) and in value, each point kept between its unselected neighbours.
fn moved(
    app: &CantusApp,
    at_press: &Automation,
    anchor: usize,
    f: &Frame,
    press: Pos2,
    pos: Pos2,
    free: bool,
) -> Automation {
    let mut a = at_press.clone();
    let sel = &app.automation.editor.selected;
    let Some(p0) = at_press.points.get(anchor) else {
        return a;
    };
    let raw = p0.tick.0 as f64 + ((pos.x - press.x) / f.px_per_tick) as f64;
    let mut land = Ticks(raw.max(0.0).round() as i64);
    if app.arrangement.snap && !free {
        land = curve::snap_point(
            app.project(),
            f.origin,
            land,
            app.arrangement.grid.min(Ticks::beats(1)).max(Ticks(1)),
        );
    }
    let dt = land - p0.tick;
    let dv = f.value(pos.y) - f.value(press.y);
    for (i, p) in a.points.iter_mut().enumerate() {
        if sel.contains(&i) {
            p.tick = (p.tick + dt).max(Ticks::ZERO);
            p.value = (p.value + dv).clamp(0.0, 1.0);
        }
    }
    // Keep the order: a selection cannot jump over a point it did not take with it.
    for i in 0..a.points.len() {
        if !sel.contains(&i) {
            continue;
        }
        let lo = (0..i)
            .rev()
            .find(|j| !sel.contains(j))
            .map(|j| a.points[j].tick);
        let hi = (i + 1..a.points.len())
            .find(|j| !sel.contains(j))
            .map(|j| a.points[j].tick);
        if let Some(lo) = lo {
            a.points[i].tick = a.points[i].tick.max(lo);
        }
        if let Some(hi) = hi {
            a.points[i].tick = a.points[i].tick.min(hi);
        }
    }
    a
}

/// The grid a drawing lays points on: the arrangement's, at most a beat, at least a
/// sixteenth.
fn draw_grid(app: &CantusApp) -> Ticks {
    app.arrangement
        .grid
        .min(Ticks::beats(1))
        .max(Ticks(cantus_core::PPQ / 4))
}

/// Paint `v` at grid tick `at`, filling the grid steps since the last painted one in a
/// line, so a fast stroke leaves no gaps. `painted` stays sorted by tick.
fn paint_at(painted: &mut Vec<(Ticks, f32)>, at: Ticks, v: f32, grid: Ticks) {
    let last = painted.last().copied();
    let mut set = |t: Ticks, v: f32| match painted.binary_search_by_key(&t, |p| p.0) {
        Ok(i) => painted[i].1 = v,
        Err(i) => painted.insert(i, (t, v)),
    };
    if let Some((t0, v0)) = last {
        let steps = ((at - t0).0 / grid.0.max(1)).abs();
        for k in 1..steps {
            let d = (at - t0).0.signum() * k * grid.0;
            let u = k as f32 / steps as f32;
            set(t0 + Ticks(d), v0 + (v - v0) * u);
        }
    }
    set(at, v);
}

/// The curve with what was painted laid in: the points the stroke covered replaced by
/// one per grid step (each holds in Step, ramps in Slide).
fn drawn(at_press: &Automation, painted: &[(Ticks, f32)], step: bool) -> Automation {
    let mut a = at_press.clone();
    let (Some(first), Some(last)) = (painted.first(), painted.last()) else {
        return a;
    };
    let new = painted
        .iter()
        .map(|(t, v)| {
            let mut p = AutoPoint::new(*t, *v);
            if step {
                p.shape = Shape::Hold;
            }
            p
        })
        .collect();
    edit::splice(&mut a.points, first.0, last.0 + Ticks(1), new);
    a
}
