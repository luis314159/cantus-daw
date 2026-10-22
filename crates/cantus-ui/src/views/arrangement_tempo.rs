//! The tempo track and the metre changes in the arrangement (round 29, ADR 0060).
//!
//! **The tempo track** is a global lane under the ruler (FL's tempo automation clip for the
//! look, Cubase's tempo track for the editing): each point on a bpm axis scaled to the
//! song's tempos, a step held flat to the next point, a ramp drawn straight to it (the
//! tempo moves linearly in beats, and the timeline is linear in beats). Double-click adds
//! a point (time on the grid, tempo where you clicked); drag a point (Shift for fine
//! steps of the tempo, Ctrl off the grid); Alt+click makes a point ramp to the next or
//! hold; double-click a point to type its tempo; the menu does all of it too, and removes
//! points or the whole track. The tempo under the pointer reads in the hint, label and
//! value. Shown and hidden from the View menu, the ruler's menu or its key.
//!
//! **A metre change** is a chip on the ruler at its bar ("7/8"), drawn before the bar's
//! number: drag it along the bar lines of the metre before it, click it to edit the
//! signature, remove it from its menu. The bars after it keep their bar numbers.

use super::{beats_to_ticks, chrome, ticks_to_beats, TimelineGeometry};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, Brush, Fill, Style, Tok};
use cantus_core::tempo::{edit, TempoPoint};
use cantus_core::time::{MAX_BPM, MIN_BPM};
use cantus_core::{Command, MergeKey, Project, Ticks, TimeSignature};
use egui::{pos2, vec2, Align2, Id, Pos2, Rect, Sense};
use std::fmt::Write as _;

/// The tempo lane's height at full size.
pub const TEMPO_H: f32 = 54.0;

/// What the tempo lane remembers between frames.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The user's choice to show it (`None`: shown while the project has a tempo track).
    pub shown: Option<bool>,
    /// The project (`CantusApp::epoch`) this state is about: another project starts afresh
    /// (review 2: a lane hidden in one stayed hidden in the next).
    pub(crate) epoch: u64,
    /// The point being dragged: its index and its tempo, and where the press was.
    drag: Option<Drag>,
    /// The point whose tempo is being typed, and the words so far.
    typing: Option<(usize, String)>,
    /// The metre change being dragged: its tick at the press, and the pointer's distance
    /// from its line there (a drag moves it by the pointer's travel: review 3).
    metre_drag: Option<(Ticks, Ticks)>,
    /// A metre editor opened at a ruler point (*Add a metre change*): the change's tick,
    /// where it is drawn, and the pass it was asked in (review 3: an editor asked for is
    /// one the owner sees, here and now).
    metre_at: Option<(Ticks, Pos2, u64)>,
    /// What the lane's menu was opened on: a point, or a tick.
    menu: Option<MenuOn>,
    /// The metre change whose editor opens next frame (one just added, or *Edit*).
    open_metre: Option<Ticks>,
    /// Where each point was drawn last frame (published geometry: tests aim with it).
    drawn: Vec<Pos2>,
    /// Each metre chip drawn last frame: its change's tick and its rect (published).
    chips_drawn: Vec<(Ticks, Rect)>,
}

impl State {
    /// Where each tempo point was drawn last frame, in order.
    pub fn points_drawn(&self) -> &[Pos2] {
        &self.drawn
    }

    /// Each metre chip drawn last frame: its change's tick and its rect.
    pub fn chips_drawn(&self) -> &[(Ticks, Rect)] {
        &self.chips_drawn
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Drag {
    index: usize,
    bpm: f64,
    y: f32,
    /// The point's tick at the press, and where in the song the pointer held it (ticks
    /// from the point): the point moves along the lane by the pointer's travel, never to
    /// the grid line under it (review 4), and stays under the pointer through a zoom or a
    /// scroll mid-drag (review 5: a screen x did not).
    tick: Ticks,
    grab: Ticks,
    /// The lane's tempo range at the press: the scale holds for the whole gesture (review
    /// 2: rebuilt from the point being dragged, it ran away from the pointer).
    lo: f64,
    hi: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum MenuOn {
    Point(usize),
    At(Ticks, f64),
}

/// The lane's height: zero while it is hidden.
pub(super) fn height(app: &CantusApp, s: &Style) -> f32 {
    if app.tempo_lane_shown() {
        s.s(TEMPO_H)
    } else {
        0.0
    }
}

/// The points drawn: the tempo track's, or the song's one tempo as a point at tick 0.
fn points(p: &Project) -> Vec<TempoPoint> {
    if p.tempo_points.is_empty() {
        vec![TempoPoint {
            tick: Ticks::ZERO,
            bpm: p.tempo_bpm,
            ramp: false,
        }]
    } else {
        p.tempo_points.clone()
    }
}

/// The lane's tempo axis: the song's tempos with room above and below, at least 20 BPM.
#[derive(Copy, Clone, Debug)]
struct Axis {
    lo: f64,
    hi: f64,
    top: f32,
    bottom: f32,
}

impl Axis {
    fn of(pts: &[TempoPoint], rect: Rect, s: &Style) -> Axis {
        let lo = pts.iter().map(|p| p.bpm).fold(f64::MAX, f64::min);
        let hi = pts.iter().map(|p| p.bpm).fold(f64::MIN, f64::max);
        let mid = (lo + hi) * 0.5;
        let half = ((hi - lo) * 0.65).max(10.0);
        Axis {
            lo: (mid - half).max(MIN_BPM),
            hi: (mid + half).min(MAX_BPM),
            top: rect.top() + 6.0 * s.k,
            bottom: rect.bottom() - 6.0 * s.k,
        }
    }

    fn y(&self, bpm: f64) -> f32 {
        let u = ((bpm - self.lo) / (self.hi - self.lo).max(1e-9)) as f32;
        self.bottom + (self.top - self.bottom) * u.clamp(0.0, 1.0)
    }

    fn bpm(&self, y: f32) -> f64 {
        let u = f64::from((y - self.bottom) / (self.top - self.bottom).min(-1.0));
        (self.lo + (self.hi - self.lo) * u).clamp(MIN_BPM, MAX_BPM)
    }

    /// BPM per point of height.
    fn per_px(&self) -> f64 {
        (self.hi - self.lo) / f64::from((self.bottom - self.top).max(1.0))
    }

    /// The lane's axis: the one at a drag's press while a point is dragged — its scale
    /// held, its range slid to keep the dragged value inside (review 3: past the range the
    /// dot stuck to the edge while the value ran on).
    fn of_lane(st: &State, pts: &[TempoPoint], rect: Rect, s: &Style) -> Axis {
        let a = Axis::of(pts, rect, s);
        match st.drag {
            Some(d) => {
                let v = pts.get(d.index).map_or(d.bpm, |p| p.bpm);
                let span = d.hi - d.lo;
                let lo = if v > d.hi {
                    v - span
                } else if v < d.lo {
                    v
                } else {
                    d.lo
                };
                Axis {
                    lo,
                    hi: lo + span,
                    ..a
                }
            }
            None => a,
        }
    }
}

/// The point under `pos`, if one is within reach.
fn point_at(
    pts: &[TempoPoint],
    geo: &TimelineGeometry,
    axis: &Axis,
    pos: Pos2,
    s: &Style,
) -> Option<usize> {
    let reach = 7.0 * s.k;
    pts.iter()
        .enumerate()
        .map(|(i, p)| {
            let x = geo.x(ticks_to_beats(p.tick));
            (i, (x - pos.x).hypot(axis.y(p.bpm) - pos.y))
        })
        .filter(|(_, d)| *d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The lane: its ground, the grid's bar lines, the tempo curve, the points and their
/// values; and in the header column beside it, its name and the tempo at the playhead.
pub(super) fn paint(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    lane: Rect,
    head: Rect,
    s: &Style,
) {
    if lane.height() <= 0.0 {
        return;
    }
    let t = &s.t;
    let k = s.k;
    b.register(
        lane,
        "tempo_lane",
        &[Tok::Ink, Tok::Highlight, Tok::Grid],
        &[],
    );
    let ground = mix(t.ink, t.row_black, 0.35);
    b.rect(b.snap_rect(lane), Fill::Solid(ground));
    b.hline(
        lane.left(),
        lane.right(),
        lane.bottom() - 0.5 * b.px(),
        b.px(),
        t.grid_bar,
    );
    // The bar lines, faint, through the metre changes.
    let map = crate::views::bars::map(app.project());
    let first = geo.scroll_beats.max(0.0);
    let last = geo.beats(lane.right());
    crate::views::bars::ladder(&map, first, last, geo.px_per_beat, 24.0, |l| {
        if l.bar {
            b.vline(
                geo.x(l.beat),
                lane.top(),
                lane.bottom(),
                b.px(),
                with_alpha(t.grid, 0.6),
            );
        }
    });
    let pts = points(app.project());
    let axis = Axis::of_lane(&app.arrangement.tempo, &pts, lane, s);
    let ink = t.highlight;
    // The curve: held flat to the next point, then the step; a ramp straight to it.
    for (i, p) in pts.iter().enumerate() {
        let x0 = geo.x(ticks_to_beats(p.tick));
        let y0 = axis.y(p.bpm);
        let (x1, y1, step) = match pts.get(i + 1) {
            Some(n) if p.ramp => (geo.x(ticks_to_beats(n.tick)), axis.y(n.bpm), None),
            Some(n) => (geo.x(ticks_to_beats(n.tick)), y0, Some(axis.y(n.bpm))),
            None => (lane.right().max(x0), y0, None),
        };
        if x1 < lane.left() || x0 > lane.right() {
            continue;
        }
        // Under the curve, a wash of the line's colour.
        let fill = with_alpha(ink, 0.12);
        b.polygon(
            &[
                pos2(x0, lane.bottom()),
                pos2(x0, y0),
                pos2(x1, y1),
                pos2(x1, lane.bottom()),
            ],
            fill,
        );
        b.segment(pos2(x0, y0), pos2(x1, y1), 1.5 * k, ink, true);
        if let Some(y2) = step {
            b.segment(pos2(x1, y1), pos2(x1, y2), 1.5 * k, ink, true);
        }
    }
    let st = &app.arrangement.tempo;
    let dragged = st.drag.map(|d| d.index);
    let mut words = String::new();
    for (i, p) in pts.iter().enumerate() {
        let x = geo.x(ticks_to_beats(p.tick));
        if x < lane.left() - 8.0 || x > lane.right() + 8.0 {
            continue;
        }
        let y = axis.y(p.bpm);
        let r = if dragged == Some(i) { 5.0 } else { 4.0 } * k;
        b.circle(pos2(x, y), r, Fill::Solid(ink));
        b.circle(
            pos2(x, y),
            r - 1.5 * k,
            Fill::Solid(if p.ramp { ink } else { ground }),
        );
        // The value beside the point, where there is room before the next.
        let room = pts
            .get(i + 1)
            .map_or(lane.right(), |n| geo.x(ticks_to_beats(n.tick)))
            - x;
        if room > 44.0 * k {
            words.clear();
            let _ = write!(words, "{:.1}", p.bpm);
            let above = y - lane.top() > 14.0 * k;
            let lr = Rect::from_min_size(
                pos2(x + 6.0 * k, if above { y - 15.0 * k } else { y + 2.0 * k }),
                vec2((room - 8.0 * k).min(60.0 * k), 13.0 * k),
            );
            let _ = text::paint(
                b,
                lr,
                &Text::new(&words, Role::Micro, t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
    }
    head_paint(b, app, head, s);
}

/// The lane's header: its name and the tempo at the playhead, label and value.
fn head_paint(b: &mut Brush, app: &CantusApp, head: Rect, s: &Style) {
    if head.width() < 20.0 || head.height() <= 0.0 {
        return;
    }
    let t = &s.t;
    let k = s.k;
    b.rect(
        b.snap_rect(head),
        Fill::Solid(mix(t.ink, t.row_black, 0.25)),
    );
    b.hline(
        head.left(),
        head.right(),
        head.bottom() - 0.5 * b.px(),
        b.px(),
        t.grid_bar,
    );
    let inner = head.shrink2(vec2(8.0 * k, 4.0 * k));
    let name = Rect::from_min_max(inner.min, pos2(inner.right(), inner.center().y));
    let _ = text::paint(
        b,
        name,
        &Text::key("tempo.lane", Role::Label, t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
    let mut words = String::new();
    let _ = write!(words, "{:.1} {}", app.tempo_now(), t_("transport.bpm"));
    let value = Rect::from_min_max(pos2(inner.left(), inner.center().y), inner.max);
    let _ = text::paint(
        b,
        value,
        &Text::new(&words, Role::Value, t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
}

fn t_(key: &str) -> String {
    t(key)
}

/// The tick under `x`, on the grid unless `free` (Ctrl) or snap is off.
fn tick_at(app: &CantusApp, geo: &TimelineGeometry, x: f32, free: bool) -> Ticks {
    let t = beats_to_ticks(geo.beats(x).max(0.0));
    if free || !app.arrangement.snap {
        t
    } else {
        crate::views::bars::snap(app.project(), t, app.arrangement.grid)
    }
}

/// The command that moves point `i` to `tick` at `bpm` (kept between its neighbours; the
/// first stays at tick 0). Without a tempo track the point is the song's tempo.
fn move_point(p: &Project, i: usize, tick: Ticks, bpm: f64) -> Command {
    let bpm = cantus_core::time::clamp_bpm(bpm);
    if p.tempo_points.is_empty() {
        return Command::SetTempo { bpm };
    }
    let pts = &p.tempo_points;
    let tick = if i == 0 {
        Ticks::ZERO
    } else {
        let lo = pts[i - 1].tick + Ticks(1);
        let hi = pts
            .get(i + 1)
            .map_or(Ticks(i64::MAX / 4), |n| n.tick - Ticks(1));
        tick.clamp(lo, hi.max(lo))
    };
    Command::SetTempoPoint {
        index: i,
        point: TempoPoint {
            tick,
            bpm,
            ..pts[i]
        },
    }
}

/// The lane's gestures (registered after the timeline's response, so on top of it).
pub(super) fn input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    geo: &TimelineGeometry,
    lane: Rect,
    s: &Style,
) {
    if lane.height() <= 0.0 {
        return;
    }
    let id = Id::new("arrangement-tempo-lane");
    let resp = ui.interact(lane, id, Sense::click_and_drag());
    let pts = points(app.project());
    let axis = Axis::of_lane(&app.arrangement.tempo, &pts, lane, s);
    app.arrangement.tempo.drawn = pts
        .iter()
        .map(|p| pos2(geo.x(ticks_to_beats(p.tick)), axis.y(p.bpm)))
        .collect();
    let mods = ui.input(|i| i.modifiers);
    let hover = resp.hover_pos();
    let under = hover.and_then(|p| point_at(&pts, geo, &axis, p, s));
    if let Some(pos) = hover {
        if app.arrangement.tempo.drag.is_none() {
            ui.ctx().set_cursor_icon(if under.is_some() {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Crosshair
            });
        }
        let tick = beats_to_ticks(geo.beats(pos.x).max(0.0));
        let bpm = match (app.arrangement.tempo.drag, under) {
            (Some(d), _) => app
                .project()
                .tempo_points
                .get(d.index)
                .map_or(app.project().tempo_bpm, |p| p.bpm),
            (None, Some(i)) => pts[i].bpm,
            (None, None) => crate::views::bars::bpm_at(app.project(), tick),
        };
        // Tempo is no automation target (the tempo track is how it changes), and only a
        // point can be typed (review 2).
        let caps = Caps {
            menu: true,
            typed: under.is_some(),
            ..Caps::NONE
        };
        hint::set(ui.ctx(), &t("tempo.lane"), caps, resp.dragged(), |b| {
            write!(b, "{bpm:.1} {}", t("transport.bpm"))
        });
    }
    // Alt+click: a point ramps to the next, or holds.
    if resp.clicked() && mods.alt {
        if let Some(i) = under.filter(|_| !app.project().tempo_points.is_empty()) {
            let p = app.project().tempo_points[i];
            app.exec(Command::SetTempoPoint {
                index: i,
                point: TempoPoint { ramp: !p.ramp, ..p },
            });
        }
    }
    if resp.double_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            match under {
                Some(i) => app.arrangement.tempo.typing = Some((i, format!("{:.1}", pts[i].bpm))),
                None => {
                    let tick = tick_at(app, geo, pos.x, mods.command);
                    let cmd = edit::add_tempo_point(app.project(), tick, axis.bpm(pos.y), false);
                    app.exec(cmd);
                }
            }
        }
    }
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
            if let Some(i) = point_at(&pts, geo, &axis, p, s) {
                app.arrangement.tempo.drag = Some(Drag {
                    index: i,
                    bpm: pts[i].bpm,
                    y: p.y,
                    tick: pts[i].tick,
                    grab: beats_to_ticks(geo.beats(p.x)) - pts[i].tick,
                    lo: axis.lo,
                    hi: axis.hi,
                });
            }
        }
    }
    if resp.dragged_by(egui::PointerButton::Primary) {
        if let (Some(d), Some(p)) = (app.arrangement.tempo.drag, resp.interact_pointer_pos()) {
            // Shift: a tenth of the speed, for fine tempos.
            let fine = if mods.shift { 0.1 } else { 1.0 };
            let bpm = d.bpm - f64::from(p.y - d.y) * axis.per_px() * fine;
            let bpm = (bpm * 10.0).round() / 10.0;
            // Along the lane by the travel, once it is more than a few points (a drag up
            // or down keeps the point's place, on the grid or off it), snapped as the lane
            // snaps.
            let held = geo.x(ticks_to_beats(d.tick + d.grab));
            let tick = if (p.x - held).abs() < 4.0 * s.k {
                d.tick
            } else {
                let t = (beats_to_ticks(geo.beats(p.x)) - d.grab).max(Ticks::ZERO);
                if mods.command || !app.arrangement.snap {
                    t
                } else {
                    crate::views::bars::snap(app.project(), t, app.arrangement.grid)
                }
            };
            let cmd = move_point(app.project(), d.index, tick, bpm);
            app.exec_merging(cmd, MergeKey::of(&("tempo-point", d.index)));
        }
    }
    if resp.drag_stopped() {
        app.arrangement.tempo.drag = None;
        app.end_gesture();
    }
    if resp.secondary_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            app.arrangement.tempo.menu = Some(match under {
                Some(i) => MenuOn::Point(i),
                None => MenuOn::At(tick_at(app, geo, p.x, mods.command), axis.bpm(p.y)),
            });
        }
    }
    egui::Popup::context_menu(&resp)
        .frame(chrome::menu_frame())
        .show(|ui| lane_menu(app, ui));
    typing(app, ui, geo, &axis, s);
}

/// The lane's menu: on a point, its ramp, its tempo typed, its removal; elsewhere, a point
/// added there, the whole track removed; always, the lane hidden.
fn lane_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    let on = app.arrangement.tempo.menu;
    let has = app.project().has_tempo_track();
    let n = app.project().tempo_points.len();
    match on {
        Some(MenuOn::Point(i)) => {
            let ramp = app.project().tempo_points.get(i).is_some_and(|p| p.ramp);
            let items = [
                Item::Action {
                    label: Src::Key("tempo.menu.ramp"),
                    check: if ramp { Check::On } else { Check::Off },
                    shortcut: None,
                    submenu: false,
                    enabled: has && i + 1 < n,
                },
                Item::action(Src::Key("tempo.menu.type")),
                Item::Action {
                    label: Src::Key("tempo.menu.remove_point"),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: has && i > 0,
                },
                Item::Separator,
                Item::action(Src::Key("tempo.menu.hide")),
            ];
            match kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
                Some(0) => {
                    if let Some(p) = app.project().tempo_points.get(i).copied() {
                        app.exec(Command::SetTempoPoint {
                            index: i,
                            point: TempoPoint { ramp: !p.ramp, ..p },
                        });
                    }
                    ui.close();
                }
                Some(1) => {
                    let bpm = points(app.project()).get(i).map_or(120.0, |p| p.bpm);
                    app.arrangement.tempo.typing = Some((i, format!("{bpm:.1}")));
                    ui.close();
                }
                Some(2) => {
                    app.exec(Command::RemoveTempoPoint { index: i });
                    ui.close();
                }
                Some(4) => {
                    app.toggle_tempo_lane();
                    ui.close();
                }
                Some(_) => ui.close(),
                None => {}
            }
        }
        _ => {
            let at = match on {
                Some(MenuOn::At(t, b)) => Some((t, b)),
                _ => None,
            };
            let items = [
                Item::Action {
                    label: Src::Key("tempo.menu.add_point"),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: at.is_some(),
                },
                Item::Action {
                    label: Src::Key("tempo.menu.remove_track"),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: has,
                },
                Item::Separator,
                Item::action(Src::Key("tempo.menu.hide")),
            ];
            match kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
                Some(0) => {
                    if let Some((tick, bpm)) = at {
                        let cmd = edit::add_tempo_point(app.project(), tick, bpm, false);
                        app.exec(cmd);
                    }
                    ui.close();
                }
                Some(1) => {
                    let bpm = app.project().tempo_bpm;
                    app.exec(Command::SetTempoTrack {
                        bpm,
                        points: Vec::new(),
                    });
                    ui.close();
                }
                Some(3) => {
                    app.toggle_tempo_lane();
                    ui.close();
                }
                Some(_) => ui.close(),
                None => {}
            }
        }
    }
}

/// A point's tempo being typed: the kit's field over the point; Enter sets it, Esc
/// leaves it.
fn typing(app: &mut CantusApp, ui: &mut egui::Ui, geo: &TimelineGeometry, axis: &Axis, s: &Style) {
    let Some((i, mut words)) = app.arrangement.tempo.typing.take() else {
        return;
    };
    let pts = points(app.project());
    let Some(p) = pts.get(i).copied() else {
        return;
    };
    let k = s.k;
    let at = pos2(
        geo.x(ticks_to_beats(p.tick)) + 6.0 * k,
        axis.y(p.bpm) - 10.0 * k,
    );
    let mut event = None;
    let fid = Id::new(("tempo-type", i));
    egui::Area::new(fid.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            let (line, _) = ui.allocate_exact_size(vec2(72.0 * k, 20.0 * k), Sense::hover());
            let out = crate::kit::field::Field::new(fid)
                .scale(k)
                .opens_when_shown()
                .show(ui, line, &mut words);
            event = out.event;
        });
    match event {
        Some(crate::kit::field::Event::Commit) => {
            if let Ok(bpm) = words.trim().replace(',', ".").parse::<f64>() {
                if bpm.is_finite() {
                    let cmd = move_point(app.project(), i, p.tick, bpm);
                    app.exec(cmd);
                }
            }
        }
        Some(crate::kit::field::Event::Cancel) => {}
        None => app.arrangement.tempo.typing = Some((i, words)),
    }
}

// ---- the metre changes: chips on the ruler ----------------------------------------------

/// A metre chip's width for `sig` (its text and a margin).
fn chip_w(ctx: &egui::Context, sig: TimeSignature, s: &Style) -> f32 {
    let mut w = String::new();
    let _ = write!(w, "{sig}");
    text::measure(ctx, &Text::new(&w, Role::Micro, s.t.text), s.tk()).x + 8.0 * s.k
}

/// The chip of a metre change whose bar line is at `x`, in the bar-number band
/// `top..bottom`.
pub(super) fn chip_rect(
    ctx: &egui::Context,
    x: f32,
    top: f32,
    bottom: f32,
    sig: TimeSignature,
    s: &Style,
) -> Rect {
    let k = s.k;
    Rect::from_min_max(
        pos2(x + 2.0 * k, top + 2.0 * k),
        pos2(x + 2.0 * k + chip_w(ctx, sig, s), bottom - 2.0 * k),
    )
}

/// The metre changes shown as chips: every change when the song has more than one metre.
pub(super) fn chips(app: &CantusApp) -> Vec<(Ticks, TimeSignature)> {
    let p = app.project();
    if p.metre_changes.len() < 2 {
        return Vec::new();
    }
    p.metre_changes
        .iter()
        .map(|m| (m.tick, m.signature))
        .collect()
}

/// Paint a chip; returns its width (the bar's number is drawn after it).
pub(super) fn paint_chip(b: &mut Brush, r: Rect, sig: TimeSignature, hot: bool, s: &Style) -> f32 {
    let t = &s.t;
    let fill = if hot {
        mix(t.highlight, t.ink, 0.25)
    } else {
        mix(t.highlight, t.ink, 0.55)
    };
    b.rrect(r, 2.0 * s.k, Fill::Solid(fill));
    let mut w = String::new();
    let _ = write!(w, "{sig}");
    let _ = text::paint(
        b,
        r,
        &Text::new(&w, Role::Micro, t.text).align(Align2::CENTER_CENTER),
        s.tk(),
    );
    r.width() + 4.0 * s.k
}

/// The metre chips' gestures: drag a chip along the bar lines of the metre before it, click
/// it to edit its signature, its menu removes it. `numbers` is the ruler's bar-number band.
pub(super) fn metre_input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    geo: &TimelineGeometry,
    numbers: Rect,
    s: &Style,
) {
    // A chip released out of view is never told it stopped: its drag ends here (review 5).
    if app.arrangement.tempo.metre_drag.is_some() && !ui.input(|i| i.pointer.primary_down()) {
        app.arrangement.tempo.metre_drag = None;
        app.end_gesture();
    }
    let chips = chips(app);
    let mut drawn = Vec::with_capacity(chips.len());
    let mut chip_rects = Vec::with_capacity(chips.len());
    for (i, (tick, sig)) in chips.iter().copied().enumerate() {
        let x = geo.x(ticks_to_beats(tick));
        if x < numbers.left() - 40.0 || x > numbers.right() {
            continue;
        }
        drawn.push(tick);
        let r = chip_rect(ui.ctx(), x, numbers.top(), numbers.bottom(), sig, s);
        chip_rects.push((tick, r));
        let id = Id::new(("metre-chip", i));
        let resp = ui.interact(r, id, Sense::click_and_drag());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(if i > 0 {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::PointingHand
            });
            let mut w = String::new();
            let _ = write!(w, "{sig}");
            hint::set_text(
                ui.ctx(),
                &t("tempo.metre_change"),
                &w,
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        if i > 0 && resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                let at = beats_to_ticks(geo.beats(p.x).max(0.0));
                if app.arrangement.tempo.metre_drag.is_none() {
                    // Where the press was against the line: kept through the drag.
                    let press = ui
                        .input(|i| i.pointer.press_origin())
                        .map_or(at, |o| beats_to_ticks(geo.beats(o.x).max(0.0)));
                    app.arrangement.tempo.metre_drag = Some((tick, press - tick));
                }
                let grab = app
                    .arrangement
                    .tempo
                    .metre_drag
                    .map_or(Ticks::ZERO, |d| d.1);
                let p0 = app.project();
                let mut without = p0.metre_changes.clone();
                without.remove(i);
                let other = cantus_core::TempoMap::new(120.0, p0.time_signature, &[], &without);
                let to = other.snap_bar((at - grab).max(Ticks::ZERO));
                // Held between its neighbours (review 2): no refusal of a move out of
                // range; a frozen sound's refusal is said, as for every edit (review 3).
                if let Some(to) = clamp_move(p0, i, to) {
                    let key = MergeKey::of(&(
                        "metre-move",
                        app.arrangement.tempo.metre_drag.map(|d| d.0),
                    ));
                    app.exec_merging(Command::MoveMetre { from: tick, to }, key);
                }
            }
        }
        if resp.drag_stopped() {
            app.arrangement.tempo.metre_drag = None;
            app.end_gesture();
        }
        let popup = Id::new(("metre-chip-editor", i));
        if resp.clicked() || app.arrangement.tempo.open_metre == Some(tick) {
            app.arrangement.tempo.open_metre = None;
            egui::Popup::open_id(ui.ctx(), popup);
        }
        if egui::Popup::is_id_open(ui.ctx(), popup) {
            egui::Popup::from_response(&resp)
                .id(popup)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .frame(crate::views::record_options::frame_for(s))
                .gap(4.0)
                .show(|ui| crate::views::transport::tempo::metre_editor(app, ui, tick, true));
        }
        egui::Popup::context_menu(&resp)
            .frame(chrome::menu_frame())
            .show(|ui| {
                let items = [
                    Item::action(Src::Key("tempo.menu.edit_metre")),
                    Item::Action {
                        label: Src::Key("tempo.menu.remove_metre"),
                        check: Check::None,
                        shortcut: None,
                        submenu: false,
                        enabled: i > 0,
                    },
                ];
                match kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
                    Some(0) => {
                        app.arrangement.tempo.open_metre = Some(tick);
                        ui.close();
                    }
                    Some(1) => {
                        app.exec(Command::RemoveMetreAt { tick });
                        ui.close();
                    }
                    Some(_) => ui.close(),
                    None => {}
                }
            });
    }
    // An editor asked for on a chip not drawn this frame (gone, or out of view): forgotten,
    // so it never opens by itself later (reviews 2 and 3).
    if let Some(t) = app.arrangement.tempo.open_metre {
        if !drawn.contains(&t) {
            app.arrangement.tempo.open_metre = None;
        }
    }
    app.arrangement.tempo.chips_drawn = chip_rects;
    metre_popup(app, ui, s);
}

/// Add a metre change at the bar line nearest `at` (the metre in force there, its editor
/// opened on it). At bar 1 — or on a change already there — nothing is added: that
/// change's editor opens, the top bar's for the song's own metre (review 2: a change
/// that changed nothing left an empty undo step and an editor that never opened).
pub fn add_metre_change(app: &mut CantusApp, at: Ticks) {
    let map = crate::views::bars::map(app.project());
    let bar = map.snap_bar(at.max(Ticks::ZERO));
    let sig = map.signature_at(bar);
    let there = app.project().metre_changes.iter().any(|m| m.tick == bar);
    if bar > Ticks::ZERO
        && !there
        && !app.exec(Command::SetMetreAt {
            tick: bar,
            signature: sig,
        })
    {
        return;
    }
    // The editor opens on the ruler at that bar — in view, or at the ruler's nearer end
    // (review 3: the top bar's metre may be hidden, a chip out of view).
    // Never `clamp` (review 4: an arrangement narrower than the editor made its bounds
    // cross, and that panics); the area keeps itself on the screen too.
    let pos = app.arrangement.geometry().map_or(pos2(0.0, 0.0), |g| {
        let x = g
            .x(ticks_to_beats(bar))
            .min(g.left + g.width - 160.0)
            .max(g.left);
        // Under the ruler and the tempo lane: the chip it edits stays in sight (review 5).
        pos2(x, g.body_top)
    });
    app.arrangement.tempo.metre_at = Some((bar, pos, u64::MAX));
}

/// The editor *Add a metre change* opened on the ruler (see [`State::metre_at`]): closed by
/// a click away (after the pass that opened it) or Esc.
fn metre_popup(app: &mut CantusApp, ui: &mut egui::Ui, s: &Style) {
    let Some((tick, pos, pass)) = app.arrangement.tempo.metre_at else {
        return;
    };
    let now = ui.ctx().cumulative_pass_nr();
    // Stamped with the first pass that draws it.
    let pass = if pass == u64::MAX { now } else { pass };
    app.arrangement.tempo.metre_at = Some((tick, pos, pass));
    let area = egui::Area::new(Id::new("metre-at-editor"))
        .order(egui::Order::Foreground)
        .constrain(true)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            crate::views::record_options::frame_for(s).show(ui, |ui| {
                crate::views::transport::tempo::metre_editor(app, ui, tick, true)
            });
        });
    let away = area.response.clicked_elsewhere() && now > pass;
    if away || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.arrangement.tempo.metre_at = None;
    }
}

impl State {
    /// The tick whose metre editor is open on the ruler (*Add a metre change*).
    pub fn metre_editor_at(&self) -> Option<Ticks> {
        self.metre_at.map(|m| m.0)
    }
}

/// Where metre change `i` may be dragged to, `to` asked (review 2): on a bar line of the
/// metre before it, a bar after that change at the earliest and the last bar line before
/// the next change at the latest; `None` when it would not move.
pub fn clamp_move(p: &Project, i: usize, to: Ticks) -> Option<Ticks> {
    let tick = p.metre_changes.get(i)?.tick;
    if i == 0 {
        return None;
    }
    let mut without = p.metre_changes.clone();
    without.remove(i);
    let other = cantus_core::TempoMap::new(120.0, p.time_signature, &[], &without);
    let prev = p.metre_changes[i - 1];
    let lo = prev.tick + prev.signature.ticks_per_bar();
    let hi = match p.metre_changes.get(i + 1) {
        Some(next) => other.bar_start(next.tick - Ticks(1)),
        None => Ticks(i64::MAX / 4),
    };
    if hi < lo {
        return None;
    }
    let to = other.snap_bar(to).clamp(lo, hi);
    (to != tick).then_some(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review 3 (finding 32): a point dragged past the press's range slides the range with
    /// it, its scale held: the dot drawn is the tempo set, inside the lane.
    #[test]
    fn a_point_dragged_past_the_range_slides_it() {
        let s = Style::new(crate::kit::Tokens::slate());
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 54.0));
        let st = State {
            drag: Some(Drag {
                index: 0,
                bpm: 120.0,
                y: 30.0,
                tick: Ticks::ZERO,
                grab: Ticks::ZERO,
                lo: 110.0,
                hi: 130.0,
            }),
            ..State::default()
        };
        for (bpm, lo) in [(160.0, 140.0), (95.0, 95.0), (125.0, 110.0)] {
            let pts = [TempoPoint {
                tick: Ticks::ZERO,
                bpm,
                ramp: false,
            }];
            let axis = Axis::of_lane(&st, &pts, rect, &s);
            assert_eq!((axis.lo, axis.hi), (lo, lo + 20.0), "{bpm}");
            assert!(
                (axis.bpm(axis.y(bpm)) - bpm).abs() < 1e-6,
                "{bpm} drawn where it is"
            );
        }
    }
}
