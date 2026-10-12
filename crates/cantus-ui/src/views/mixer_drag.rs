//! Dragging on the desk (round 25, ADR 0041; design §3.3, the owner's answers 2, 11, 12):
//! a cable from a strip's port (or the selected strip's ▼ chip) to another strip — plain
//! = a send, Ctrl = the only output, Alt = a key — a cable's end moved or dropped on
//! nothing, a track or a channel-rack channel dropped on a strip
//! ([`crate::routing_actions::TrackDrag`]), and strips and groups dragged along the desk
//! to reorder them.
//!
//! The desk calls [`source`] from every control a drag can start on (the band's ports and
//! end marks, the send row's ▼ chip, the number cell, a group's separator), [`target`] /
//! [`target_group`] once per visible column, and [`finish`] once a frame after the three
//! docks; it paints what [`Target`] says (the hatching of a refused column).
//!
//! **How a drag goes.** A press on a source is only a press until the pointer travels
//! [`THRESHOLD`] points (so a click on a number cell still selects, and a click on a
//! separator still folds it); then the drag is on, and egui is told something is dragged
//! so nothing else on the desk lights under the pointer. While it is on, every column says
//! whether a drop there would work (`Project::route_check` per column, worked out once per
//! revision, source and modifier, never per frame) — refused columns are hatched before
//! the drop; the column under the pointer is outlined, and a chip beside the pointer says
//! what releasing does, or why it cannot ("Would make a loop: Bus already feeds Keys").
//! The modifiers are read live (plain = send, Ctrl = the only output, Alt = a key), and
//! the rubber band changes with them. The middle dock scrolls by itself within
//! [`EDGE`] points of its edges. **Esc cancels**; a drop does one `routing_actions` call —
//! one undo step — or nothing.
//!
//! Strips and groups: the drop indicator is a line in the gap the block would land in;
//! the lifted strips are washed; the drop calls `move_strips_to` / `move_group_to` with
//! the [`Spot`] under the pointer (its dock named by where it is: a strip dragged onto the
//! right dock is pinned there).

use super::desk::Item;
use super::menu::Menu;
use super::{in_selection, Geo, MixerView};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{safe_track, with_alpha, FillSize};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Function, Style};
use crate::routing_actions::{refusal_text, Gesture, Outcome, TrackDrag};
use cantus_core::routing::order::Spot;
use cantus_core::routing::{plan, End, Route, RouteKind, RouteRefusal};
use cantus_core::{Command, Dock, GroupId, StripId, TrackId};
use egui::{pos2, vec2, Id, Key, Modifiers, Pos2, Rect, Response, Ui};

/// How far a press travels before it is a drag, in points (egui's click tolerance).
pub(crate) const THRESHOLD: f32 = 6.0;
/// Within this many points of the middle dock's edges, the middle scrolls by itself.
pub(crate) const EDGE: f32 = 24.0;
/// The fastest auto-scroll, in points a second (at the very edge).
const SCROLL_SPEED: f32 = 900.0;
/// How long a cable hovers a folded group before it springs open, in seconds.
const SPRING: f64 = 0.6;
/// How long *Show the destination* flashes a strip's number, in seconds.
const FLASH: f64 = 1.4;

/// The drag in progress, kept in `MixerView` between frames.
#[derive(Debug, Default)]
pub(crate) struct Drag {
    /// A press on a source that has not travelled far enough to be a drag.
    press: Option<Press>,
    /// The drag that is on.
    on: Option<On>,
    /// Click mode (the owner's answer 2: "release **or click** on the destination"): a
    /// click on a port or the ▼ chip armed a route from that strip; the next click on a
    /// strip makes it.
    armed: Option<Armed>,
    /// A track (or channel) dragged from the arrangement or the channel rack, as last
    /// frame's `finish` saw it.
    track: Option<TrackId>,
    /// The column under the pointer this frame (set by [`target`] and [`target_group`],
    /// read and cleared by [`finish`]).
    over: Option<Over>,
    /// The lifted columns this frame (the strips a reorder carries), washed by `finish`.
    lifted: Vec<Rect>,
    /// What each column says for the drag on, by project strip index (the master last),
    /// and what that was worked out for.
    checks: Vec<Option<Target>>,
    checks_for: Option<CheckKey>,
    /// The words beside the pointer, and what they were written for.
    words: String,
    words_for: Option<(CheckKey, Option<Over>)>,
    /// Last frame's docks (left, middle, right): a middle column partly under a pinned
    /// dock is a target only where it shows.
    docks: Option<[Rect; 3]>,
    /// A folded group a cable hovers, and since when.
    spring: Option<(GroupId, f64)>,
    /// The keymap's *Route menu* (Shift+F10) asked for the selected strip's route menu.
    pub(super) route_menu: bool,
    /// *Show the destination*: the strip, when it was asked, and where its column was
    /// drawn this frame.
    reveal: Option<(StripId, f64)>,
    reveal_at: Option<Rect>,
    /// The *Route here* plate under the pointer this frame (its strip and well).
    pub(super) plate: Option<(StripId, Rect)>,
    /// The selection before a click on a plate (the meter under it selects its strip on
    /// the same click): put back after the docks, so the plate opens its menu for the
    /// selection the musician had — "Send Keys here" — rather than for the plate's strip.
    pub(super) restore: Option<(
        Option<StripId>,
        Vec<StripId>,
        crate::views::inspector::Subject,
    )>,
}

/// A route armed by a click (click mode), waiting for the click on its destination.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Armed {
    from: StripId,
    /// Where the rubber band starts, and the control that armed it (the port or the ▼
    /// chip): the anchor follows that control when the middle scrolls.
    anchor: Pos2,
    control: Id,
    /// Armed this frame: the arming click's release is not the completing click.
    fresh: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Press {
    src: Source,
    origin: Pos2,
    /// The control pressed (a port's ring, an end mark): where the rubber band starts.
    anchor: Rect,
}

/// What is being dragged.
#[derive(Copy, Clone, Debug, PartialEq)]
enum What {
    Source(Source),
    Track(TrackId),
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct On {
    what: What,
    /// Where the rubber band starts.
    anchor: Pos2,
}

/// What the column checks were worked out for: the project, the drag, the modifier and
/// the selection (a gesture from a multi-selection acts for every selected strip).
#[derive(Copy, Clone, Debug, PartialEq)]
struct CheckKey {
    revision: u64,
    what: What,
    gesture: Gesture,
    selected: Option<StripId>,
    selection: usize,
}

/// A column a drag is over this frame.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Over {
    /// A strip's column (`None`: the master's).
    Strip(Option<StripId>, Rect, Target),
    /// A group's separator.
    Group(GroupId, Rect, Target),
}

/// What a drag starts from.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Source {
    /// A strip's port in the cable band, or the selected strip's ▼ chip: a new route.
    #[allow(
        dead_code,
        reason = "the desk builder starts cables from ports and ▼ chips"
    )]
    Port(StripId),
    /// A lit cable's end mark: the route moves (or is removed, dropped on nothing).
    #[allow(dead_code, reason = "the desk builder starts end moves from end marks")]
    End { route: Route, end: End },
    /// A strip's number cell: the strip (or the selection it is in) moves along the desk.
    Strip(StripId),
    /// A group's separator: the whole group moves.
    Group(GroupId),
}

/// How a column should look while a drag hovers it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Target {
    /// No drag, or a drag this column is not part of.
    #[default]
    Idle,
    /// A drop here would do something: lit.
    Valid,
    /// A drop here is refused (a loop, itself, no room): hatched, the reason in the hint.
    Refused,
}

impl What {
    /// A route being made or moved (the columns answer), not strips being reordered.
    fn routes(self) -> bool {
        !matches!(
            self,
            What::Source(Source::Strip(_)) | What::Source(Source::Group(_))
        )
    }
}

impl Drag {
    fn what(&self) -> Option<What> {
        self.on
            .map(|o| o.what)
            .or(self.armed.map(|a| What::Source(Source::Port(a.from))))
            .or(self.track.map(What::Track))
    }
}

/// A control a drag can start on was sensed this frame (`resp` is its response).
pub(super) fn source(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    resp: &Response,
    src: Source,
) {
    let _ = app;
    let d = &mut v.drag;
    if let Some(arm) = d.armed.as_mut() {
        // The control that armed the route moves when the middle scrolls: follow it.
        if arm.control == resp.id {
            arm.anchor = resp.rect.center();
        }
        return;
    }
    if let (Source::Port(from), true, None) = (src, resp.clicked(), d.on) {
        // Click mode: a click (no travel) on a port or the ▼ chip arms a route from the
        // strip; the next click on a strip makes it (`finish`).
        d.press = None;
        d.armed = Some(Armed {
            from,
            anchor: resp.rect.center(),
            control: resp.id,
            fresh: true,
        });
        d.words_for = None;
        ui.ctx().request_repaint();
        return;
    }
    if d.on.is_none() && resp.hovered() {
        // A hand over anything that can be picked up: the gesture is there to find.
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if let Some(on) = d.on.as_mut() {
        // The control a cable starts from moves when the middle scrolls: follow it.
        if on.what == What::Source(src) && matches!(src, Source::Port(_)) {
            on.anchor = resp.rect.center();
        }
        return;
    }
    if d.press.is_some() || !(resp.is_pointer_button_down_on() || resp.drag_started()) {
        return;
    }
    let (down, origin) = ui.input(|i| (i.pointer.primary_down(), i.pointer.press_origin()));
    if let (true, Some(origin)) = (down, origin) {
        d.press = Some(Press {
            src,
            origin,
            anchor: resp.rect,
        });
    }
}

/// The pointer, when it is on the desk's window.
fn pointer(ui: &Ui) -> Option<Pos2> {
    ui.input(|i| i.pointer.latest_pos())
}

/// Whether `pos` is in `column` where the column shows (a middle strip partly under a
/// pinned dock counts only where it is visible).
fn shows_at(
    v: &MixerView,
    app: &CantusApp,
    strip: Option<StripId>,
    column: Rect,
    pos: Pos2,
) -> bool {
    if !column.contains(pos) {
        return false;
    }
    let dock = match strip {
        None => Dock::Left,
        Some(id) => app.project().strip(id).map_or(Dock::Middle, |s| s.dock),
    };
    let Some(docks) = v.drag.docks else {
        return true;
    };
    let r = match dock {
        Dock::Left => docks[0],
        Dock::Middle => docks[1],
        Dock::Right => docks[2],
    };
    r.contains(pos)
}

/// A strip's column (`None`: the master's), on screen this frame, as a drop target.
pub(super) fn target(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    strip: Option<StripId>,
    column: Rect,
) -> Target {
    if let (Some((r, _)), Some(id)) = (v.drag.reveal, strip) {
        if r == id {
            v.drag.reveal_at = Some(column);
        }
    }
    let Some(what) = v.drag.what() else {
        return Target::Idle;
    };
    let target = if what.routes() {
        check(v, app, ui, what, strip)
    } else {
        if let (What::Source(src), Some(id)) = (what, strip) {
            if lifts(app, src, id) {
                v.drag.lifted.push(column);
            }
        }
        Target::Idle
    };
    if let Some(pos) = pointer(ui) {
        if shows_at(v, app, strip, column, pos) {
            v.drag.over = Some(Over::Strip(strip, column, target));
        }
    }
    target
}

/// A group's separator as a drop target (a collapsed group refuses drops and springs
/// open after 600 ms of hovering with a cable).
pub(super) fn target_group(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    group: GroupId,
    column: Rect,
) -> Target {
    let Some(what) = v.drag.what() else {
        return Target::Idle;
    };
    let collapsed = app
        .project()
        .strip_group(group)
        .is_some_and(|g| g.collapsed);
    let target = if what.routes() && collapsed {
        Target::Refused
    } else {
        Target::Idle
    };
    let Some(pos) = pointer(ui).filter(|p| column.contains(*p)) else {
        return target;
    };
    v.drag.over = Some(Over::Group(group, column, target));
    if what.routes() && collapsed {
        let now = ui.input(|i| i.time);
        match v.drag.spring {
            Some((g, since)) if g == group => {
                if now - since >= SPRING {
                    v.drag.spring = None;
                    app.exec(Command::SetStripGroupCollapsed {
                        id: group,
                        collapsed: false,
                    });
                } else {
                    let left = SPRING - (now - since);
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(left));
                }
            }
            _ => {
                v.drag.spring = Some((group, now));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(SPRING));
            }
        }
        let _ = pos;
    }
    target
}

/// Whether a drag is in progress (the band lights every port; the hint shows the
/// modifiers).
pub(crate) fn active(v: &MixerView) -> bool {
    v.drag.on.is_some() || v.drag.track.is_some() || v.drag.armed.is_some()
}

/// The strip a click armed a route from (click mode), if any: its port and ▼ chip show
/// it armed until the destination's click, Esc, or a click elsewhere.
pub(crate) fn armed(v: &MixerView) -> Option<StripId> {
    v.drag.armed.map(|a| a.from)
}

// ---- what each column says ----------------------------------------------------------

/// The modifiers' gesture this frame.
fn gesture(ui: &Ui) -> Gesture {
    Gesture::from_modifiers(ui.input(|i| i.modifiers))
}

fn key_now(app: &CantusApp, ui: &Ui, what: What, v: &MixerView) -> CheckKey {
    CheckKey {
        revision: app.bus.revision(),
        what,
        gesture: gesture(ui),
        selected: app.selected_strip,
        selection: v.selection_len(app),
    }
}

/// Whether the reorder drag `src` carries the strip `id`.
fn lifts(app: &CantusApp, src: Source, id: StripId) -> bool {
    match src {
        Source::Strip(s) => {
            if in_selection(app, s) {
                in_selection(app, id)
            } else {
                s == id
            }
        }
        Source::Group(g) => app.project().strip(id).is_some_and(|x| x.group == Some(g)),
        Source::Port(_) | Source::End { .. } => false,
    }
}

/// What the column of `strip` (`None`: the master) says for the drag on: worked out once
/// per revision, drag, modifier and selection, then read.
fn check(
    v: &mut MixerView,
    app: &CantusApp,
    ui: &Ui,
    what: What,
    strip: Option<StripId>,
) -> Target {
    let key = key_now(app, ui, what, v);
    let p = app.project();
    let n = p.strips.len();
    let d = &mut v.drag;
    if d.checks_for != Some(key) {
        d.checks.clear();
        d.checks.resize(n + 1, None);
        d.checks_for = Some(key);
    }
    let i = match strip {
        Some(id) => match p.strip_index(id) {
            Some(i) => i,
            None => return Target::Idle,
        },
        None => n,
    };
    if let Some(Some(t)) = d.checks.get(i) {
        return *t;
    }
    let to = strip.unwrap_or(StripId::MASTER);
    let t = match refusal(app, what, key.gesture, to) {
        Ok(()) => Target::Valid,
        Err(None) => Target::Idle,
        Err(Some(_)) => Target::Refused,
    };
    if let Some(slot) = v.drag.checks.get_mut(i) {
        *slot = Some(t);
    }
    t
}

/// Whether a drop of `what` on `to` would do something: `Ok`, or nothing to do
/// (`Err(None)`: itself, already there — not hatched), or refused and why.
fn refusal(
    app: &CantusApp,
    what: What,
    gesture: Gesture,
    to: StripId,
) -> Result<(), Option<RouteRefusal>> {
    let p = app.project();
    match what {
        What::Source(Source::Port(from)) => {
            if to == from {
                return Err(None);
            }
            if gesture == Gesture::Key && !to.is_master() {
                return quiet(p.route_check(from, to, RouteKind::Key(None)));
            }
            let kind = if to.is_master() {
                RouteKind::Output
            } else {
                gesture.kind()
            };
            let mut first = Err(None);
            for (k, s) in app.gesture_sources(from).into_iter().enumerate() {
                if s == to {
                    continue;
                }
                match quiet(p.route_check(s, to, kind)) {
                    Ok(()) => return Ok(()),
                    // The dragged strip's own reason is the one worth saying.
                    Err(r) if k == 0 || s == from => first = Err(r),
                    Err(_) => {}
                }
            }
            first
        }
        What::Source(Source::End { route, end }) => {
            let same = match end {
                End::Destination => route.to == to,
                End::Source => route.from == to,
            };
            if same {
                return Err(None);
            }
            match plan::move_route(p, route, end, to) {
                Some(_) => Ok(()),
                None => {
                    let r = match end {
                        End::Destination => p.route_check(route.from, to, route.kind),
                        End::Source if to.is_master() => Err(RouteRefusal::FromMaster),
                        End::Source => p.route_check(to, route.to, route.kind),
                    };
                    Err(Some(r.err().unwrap_or(RouteRefusal::AlreadyThere)))
                }
            }
        }
        What::Track(track) => {
            let dest = (!to.is_master()).then_some(to);
            match plan::route_tracks(p, &[track], dest) {
                Some(_) => Ok(()),
                None => Err(None),
            }
        }
        What::Source(Source::Strip(_)) | What::Source(Source::Group(_)) => Err(None),
    }
}

/// A route check with "already there" said quietly (nothing to do, nothing wrong).
fn quiet(r: Result<(), RouteRefusal>) -> Result<(), Option<RouteRefusal>> {
    match r {
        Ok(()) => Ok(()),
        Err(RouteRefusal::AlreadyThere | RouteRefusal::ToItself) => Err(None),
        Err(e) => Err(Some(e)),
    }
}

/// The source strip a drag's words and colour come from.
fn source_strip(what: What) -> Option<StripId> {
    match what {
        What::Source(Source::Port(s)) | What::Source(Source::Strip(s)) => Some(s),
        What::Source(Source::End { route, .. }) => Some(route.from),
        What::Source(Source::Group(_)) | What::Track(_) => None,
    }
}

/// The words beside the pointer: what releasing here does, or why it cannot.
fn words(v: &MixerView, app: &CantusApp, what: What, g: Gesture, over: Option<Over>) -> String {
    let p = app.project();
    let name = |id: StripId| crate::views::route_menu::strip_label(app, id);
    let over_strip = match over {
        Some(Over::Strip(s, _, _)) => Some(s.unwrap_or(StripId::MASTER)),
        _ => None,
    };
    let folded = matches!(over, Some(Over::Group(_, _, Target::Refused)));
    match what {
        What::Source(Source::Strip(id)) => {
            let n = if in_selection(app, id) {
                v.selection_len(app)
            } else {
                1
            };
            if n > 1 {
                tf("routing.drag.strips", &[("n", &n)])
            } else {
                tf("routing.drag.strip", &[("strip", &name(id))])
            }
        }
        What::Source(Source::Group(gid)) => {
            let g = p
                .strip_group(gid)
                .map(|g| g.name.clone())
                .unwrap_or_default();
            tf("routing.drag.group", &[("group", &g)])
        }
        _ if folded => t("routing.drag.folded"),
        What::Source(Source::End { route, end }) => match over_strip {
            None => t("routing.drag.remove"),
            Some(to) => match refusal(app, what, g, to) {
                Ok(()) => tf("routing.drag.move", &[("to", &name(to))]),
                Err(None) => t("routing.drag.same"),
                Err(Some(r)) => {
                    let from = if end == End::Source { to } else { route.from };
                    refusal_text(p, from, r)
                }
            },
        },
        What::Track(track) => {
            let tn = p.track(track).map(|t| t.name.clone()).unwrap_or_default();
            match over_strip {
                None => tf("routing.drag.track_hint", &[("track", &tn)]),
                Some(to) => match refusal(app, what, g, to) {
                    Ok(()) => tf("routing.drag.track", &[("track", &tn), ("to", &name(to))]),
                    _ => t("routing.drag.same"),
                },
            }
        }
        What::Source(Source::Port(from)) => {
            let Some(to) = over_strip else {
                return t(if v.drag.armed.is_some() {
                    "routing.arm.port_hint"
                } else {
                    "routing.drag.port_hint"
                });
            };
            match refusal(app, what, g, to) {
                Err(Some(r)) => refusal_text(p, from, r),
                Err(None) if to == from => t("routing.refusal.itself"),
                Err(None) => t("routing.refusal.already_there"),
                Ok(()) => {
                    let n = app.gesture_sources(from).len();
                    let to_name = name(to);
                    let args: [(&str, &dyn std::fmt::Display); 2] = [("to", &to_name), ("n", &n)];
                    let key = match (to.is_master() || g == Gesture::Output, g, n > 1) {
                        (true, _, false) => "routing.drag.output",
                        (true, _, true) => "routing.drag.output_many",
                        (false, Gesture::Key, _) => "routing.drag.key",
                        (false, _, false) => "routing.drag.send",
                        (false, _, true) => "routing.drag.send_many",
                    };
                    let mut s = tf(key, &args);
                    if g == Gesture::Send && !to.is_master() {
                        if let Some(d) = crate::routing_actions::doubled_text(p, from, to) {
                            s.push_str(" \u{b7} ");
                            s.push_str(&d);
                        }
                    }
                    s
                }
            }
        }
    }
}

// ---- once a frame, after the docks ----------------------------------------------------

/// The id egui is told is dragged while a desk drag is on (so nothing else lights).
fn drag_id() -> Id {
    Id::new("mixer-drag")
}

/// After the three docks: the rubber band, auto-scroll at the middle's edges, and the
/// drop or the cancel.
pub(super) fn finish(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
) {
    v.drag.docks = Some([geo.left, geo.middle, geo.right]);
    let ctx = ui.ctx().clone();
    let (pos, down, released, escape, dt, now) = ui.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.key_pressed(Key::Escape),
            i.stable_dt.min(0.05),
            i.time,
        )
    });
    // A track or channel dragged in from the arrangement or the channel rack.
    let track = egui::DragAndDrop::payload::<TrackDrag>(&ctx).map(|p| p.0);
    if track.is_some() && v.drag.track.is_none() {
        // Its first frame: the columns answer from the next one.
        ctx.request_repaint();
    }
    v.drag.track = track;

    // A press becomes a drag once it has travelled.
    if let Some(press) = v.drag.press {
        if !down || v.drag.on.is_some() {
            v.drag.press = None;
        } else if pos.is_some_and(|p| p.distance(press.origin) > THRESHOLD) {
            v.drag.press = None;
            start(v, app, &ctx, press);
        }
    }

    if v.drag.armed.is_some() {
        click_mode(v, app, ui, b, s, geo, escape);
    }

    if let Some(on) = v.drag.on {
        if escape {
            ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            cancel(v, &ctx);
        } else if released || !down {
            if let Some(p) = pos {
                drop(v, app, &ctx, geo, on.what, p);
            }
            cancel(v, &ctx);
        } else if let Some(p) = pos {
            auto_scroll(v, geo, p, dt, &ctx);
            paint_drag(v, app, ui, b, s, geo, on, p);
            let refused = matches!(
                v.drag.over,
                Some(Over::Strip(_, _, Target::Refused) | Over::Group(_, _, Target::Refused))
            );
            ctx.set_cursor_icon(if refused {
                egui::CursorIcon::NotAllowed
            } else {
                egui::CursorIcon::Grabbing
            });
        }
    } else if let (Some(tr), Some(p)) = (track, pos) {
        if released {
            if let Some(Over::Strip(strip, _, Target::Valid)) = v.drag.over {
                egui::DragAndDrop::clear_payload(&ctx);
                app.route_tracks_here(&[tr], strip);
            }
        } else if geo.rows.contains(p) {
            auto_scroll(v, geo, p, dt, &ctx);
            let on = On {
                what: What::Track(tr),
                anchor: p,
            };
            paint_drag(v, app, ui, b, s, geo, on, p);
        }
    }

    plate_hint(v, ui);
    if let Some((lead, list, subject)) = v.drag.restore.take() {
        app.selected_strip = lead;
        app.selected_strips = list;
        app.inspector.follow(subject);
    }
    route_key_pressed(v, app, ui, geo);
    reveal(v, b, s, geo, &ctx, now);
    v.drag.over = None;
    v.drag.lifted.clear();
    v.drag.plate = None;
}

fn start(v: &mut MixerView, app: &mut CantusApp, ctx: &egui::Context, press: Press) {
    if let Source::Strip(id) = press.src {
        // A strip outside the selection is picked up alone, and shown picked.
        if !in_selection(app, id) {
            app.select_strip(id);
        }
    }
    v.drag.on = Some(On {
        what: What::Source(press.src),
        anchor: press.anchor.center(),
    });
    v.drag.spring = None;
    ctx.set_dragged_id(drag_id());
}

/// Click mode, once a frame while a route is armed: the cover that keeps the completing
/// click from every control under it, the rubber band to the pointer, the words, and the
/// second click — on a strip that takes the route it makes it (the modifiers read at that
/// click: plain = send, Ctrl = only output, Alt = key); on a refused strip it stays armed
/// (the chip says why); anywhere else, on its own strip, or with Esc, it is let go.
#[allow(clippy::too_many_arguments)]
fn click_mode(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
    escape: bool,
) {
    let ctx = ui.ctx().clone();
    let Some(arm) = v.drag.armed else {
        return;
    };
    if app.project().strip(arm.from).is_none() {
        disarm(v, &ctx);
        return;
    }
    if escape {
        ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        disarm(v, &ctx);
        return;
    }
    // On top of every control of the desk (registered last), so the completing click
    // reaches no fader, mute or name under it.
    let cover = ui.interact(geo.full, Id::new("mixer-armed-cover"), egui::Sense::click());
    let elsewhere = ui.input(|i| {
        let any = i.pointer.button_clicked(egui::PointerButton::Primary)
            || i.pointer.button_clicked(egui::PointerButton::Secondary);
        any && i
            .pointer
            .interact_pos()
            .is_none_or(|p| !geo.full.contains(p))
    });
    if arm.fresh {
        if let Some(a) = v.drag.armed.as_mut() {
            a.fresh = false;
        }
    } else if cover.clicked() {
        let p = cover.interact_pointer_pos().or(pointer(ui));
        match (v.drag.over, p) {
            (Some(Over::Strip(_, _, Target::Valid)), Some(p)) => {
                let what = What::Source(Source::Port(arm.from));
                disarm(v, &ctx);
                // `drop` reads the column under the pointer, which this frame set.
                drop(v, app, &ctx, geo, what, p);
                return;
            }
            // A refused strip: still armed; the chip already says why.
            (Some(Over::Strip(_, _, Target::Refused) | Over::Group(_, _, Target::Refused)), _) => {}
            _ => {
                disarm(v, &ctx);
                return;
            }
        }
    } else if cover.secondary_clicked() || elsewhere {
        disarm(v, &ctx);
        return;
    }
    let Some(p) = ctx.pointer_hover_pos().filter(|p| geo.full.contains(*p)) else {
        return;
    };
    let on = On {
        what: What::Source(Source::Port(arm.from)),
        anchor: arm.anchor,
    };
    paint_drag(v, app, ui, b, s, geo, on, p);
    let refused = matches!(
        v.drag.over,
        Some(Over::Strip(_, _, Target::Refused) | Over::Group(_, _, Target::Refused))
    );
    ctx.set_cursor_icon(if refused {
        egui::CursorIcon::NotAllowed
    } else {
        egui::CursorIcon::Crosshair
    });
}

/// Let an armed route go (click mode).
fn disarm(v: &mut MixerView, ctx: &egui::Context) {
    v.drag.armed = None;
    v.drag.spring = None;
    v.drag.words_for = None;
    ctx.request_repaint();
}

fn cancel(v: &mut MixerView, ctx: &egui::Context) {
    v.drag.on = None;
    v.drag.spring = None;
    v.drag.words_for = None;
    if ctx.dragged_id() == Some(drag_id()) {
        ctx.stop_dragging();
    }
    ctx.request_repaint();
}

/// Release: one routing action, or nothing.
fn drop(
    v: &mut MixerView,
    app: &mut CantusApp,
    ctx: &egui::Context,
    geo: &Geo,
    what: What,
    p: Pos2,
) {
    let over = v.drag.over;
    let owner = super::route_owner();
    match what {
        What::Source(Source::Port(from)) => {
            let Some(Over::Strip(strip, _, Target::Valid)) = over else {
                return;
            };
            let to = strip.unwrap_or(StripId::MASTER);
            let g = Gesture::from_modifiers(ctx.input(|i| i.modifiers));
            match app.route_gesture(from, to, g) {
                Outcome::ChooseKey(choices) => {
                    crate::views::route_menu::open_key_choice(ctx, owner, from, to, choices, p);
                }
                Outcome::NothingToKey => {
                    crate::views::route_menu::open_nothing_to_key(ctx, owner, from, to, p);
                }
                Outcome::Done | Outcome::Refused(_) => {}
            }
        }
        What::Source(Source::End { route, end }) => match over {
            Some(Over::Strip(strip, _, Target::Valid)) => {
                app.move_route(route, end, strip.unwrap_or(StripId::MASTER));
            }
            // Released over nothing: the route goes (undo brings it back).
            None => {
                app.remove_route(route);
            }
            _ => {}
        },
        What::Source(src @ (Source::Strip(_) | Source::Group(_))) => {
            let Some((spot, _)) = spot_at(v, app, geo, src, p) else {
                return;
            };
            match src {
                Source::Strip(id) => {
                    let strips = app.gesture_sources(id);
                    app.move_strips_to(&strips, spot);
                }
                Source::Group(g) => {
                    app.move_group_to(g, spot);
                }
                Source::Port(_) | Source::End { .. } => {}
            }
        }
        What::Track(_) => {}
    }
}

/// Where a reorder drop at `p` lands, and the x of the gap that shows it: the dock under
/// the pointer, before the first column (not carried) whose middle is right of it.
fn spot_at(v: &MixerView, app: &CantusApp, geo: &Geo, src: Source, p: Pos2) -> Option<(Spot, f32)> {
    let (dock, items, x0, edge) = if p.x < geo.left.right() && p.x >= geo.left.left() {
        (
            Dock::Left,
            &v.desk.left,
            geo.left.left() + geo.master_w,
            geo.left.right(),
        )
    } else if p.x >= geo.middle.left() && p.x <= geo.middle.right() {
        let x0 = geo.middle.left() - v.scroll_x;
        (Dock::Middle, &v.desk.middle, x0, geo.middle.right())
    } else if p.x >= geo.right.left() && p.x <= geo.right.right() {
        (
            Dock::Right,
            &v.desk.right,
            geo.right.left(),
            geo.right.right(),
        )
    } else {
        return None;
    };
    let carried = |id: StripId| lifts(app, src, id);
    let mut x = x0;
    let mut last = x0;
    let mut sep = 0;
    for item in items {
        let w = super::desk::item_width(item, geo.pitch);
        let first = match *item {
            Item::Strip(id, _) => Some(id),
            Item::Separator(g) => {
                sep += 1;
                if matches!(src, Source::Group(own) if own == g) {
                    None
                } else {
                    run_first(app, dock, sep - 1, &carried)
                }
            }
        };
        if let Some(id) = first.filter(|id| !carried(*id)) {
            if x + w * 0.5 > p.x {
                return Some((Spot::before(dock, id), x));
            }
            last = x + w;
        }
        x += w;
    }
    let _ = edge;
    Some((Spot::end(dock), last.max(x0)))
}

/// What the `n`-th separator of `dock` stands for in a drop: the first member of its run
/// in that dock not carried by the drag (a separator opens one run of a group in one dock,
/// as the desk lays them out — a group split across docks or runs has one per run; a
/// folded run's members are under it). Walks the dock's strips; no allocation.
fn run_first(
    app: &CantusApp,
    dock: Dock,
    n: usize,
    carried: &dyn Fn(StripId) -> bool,
) -> Option<StripId> {
    let p = app.project();
    let mut run = None;
    let mut seen = 0;
    let mut inside = None;
    for s in p.strips.iter().filter(|s| s.dock == dock) {
        let group = s.group.filter(|g| p.strip_group(*g).is_some());
        if group != run {
            run = group;
            if inside.is_some() {
                // The run ended with every member carried.
                return None;
            }
            if group.is_some() {
                if seen == n {
                    inside = group;
                }
                seen += 1;
            }
        }
        if inside.is_some() && group == inside && !carried(s.id) {
            return Some(s.id);
        }
    }
    None
}

/// Scroll the middle while the pointer is near its edges.
fn auto_scroll(v: &mut MixerView, geo: &Geo, p: Pos2, dt: f32, ctx: &egui::Context) {
    let m = geo.middle;
    if m.width() <= 0.0 || p.y < geo.rows.top() || p.y > geo.rows.bottom() {
        return;
    }
    let max = (geo.content_w - m.width()).max(0.0);
    let into_left = (m.left() + EDGE - p.x) / EDGE;
    let into_right = (p.x - (m.right() - EDGE)) / EDGE;
    let step = if (0.0..=1.5).contains(&into_left) && v.scroll_x > 0.0 {
        -into_left.min(1.0)
    } else if (0.0..=1.5).contains(&into_right) && v.scroll_x < max {
        into_right.min(1.0)
    } else {
        return;
    };
    v.scroll_x = (v.scroll_x + step * SCROLL_SPEED * dt.max(1.0 / 120.0)).clamp(0.0, max);
    ctx.request_repaint();
}

// ---- what a drag looks like -----------------------------------------------------------

/// The colour a drag from `strip` is drawn in: the strip's own, else the selection's.
fn strip_ink(v: &MixerView, app: &CantusApp, s: &Style, strip: Option<StripId>) -> egui::Color32 {
    strip
        .and_then(|id| app.project().strip_index(id))
        .and_then(|i| v.desk.info(i))
        .and_then(|i| i.color)
        .map_or(s.t.selection, |c| {
            safe_track(crate::theme::track_color(c), s.t.dark, FillSize::Small)
        })
}

#[allow(clippy::too_many_arguments)]
fn paint_drag(
    v: &mut MixerView,
    app: &CantusApp,
    ui: &Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
    on: On,
    p: Pos2,
) {
    let t = &s.t;
    let g = gesture(ui);
    let over = v.drag.over;
    // The column under the pointer: outlined, in the selection's colour when a drop
    // works there, the danger colour (and a wash) when it is refused.
    let refused = match over {
        Some(Over::Strip(_, col, tg)) | Some(Over::Group(_, col, tg)) if on.what.routes() => {
            let col = col.intersect(geo.rows);
            match tg {
                Target::Valid => {
                    b.rect(col, Fill::Solid(with_alpha(t.selection, 0.10)));
                    b.rrect_stroke(col.shrink(1.0), 0.0, 2.0, t.selection);
                    false
                }
                Target::Refused => {
                    b.rect(col, Fill::Solid(with_alpha(t.danger, 0.12)));
                    b.rrect_stroke(col.shrink(1.0), 0.0, 1.5, with_alpha(t.danger, 0.9));
                    true
                }
                Target::Idle => false,
            }
        }
        _ => false,
    };
    match on.what {
        What::Source(src @ (Source::Strip(_) | Source::Group(_))) => {
            for r in &v.drag.lifted {
                b.rect(
                    r.intersect(geo.rows),
                    Fill::Solid(with_alpha(t.selection, 0.16)),
                );
            }
            if let Some((_, x)) = spot_at(v, app, geo, src, p) {
                let top = geo.rows.top() + 2.0;
                let bottom = geo.rows.bottom() - 2.0;
                b.vline(x, top, bottom, 3.0, t.selection);
                b.segment(
                    pos2(x - 5.0, top),
                    pos2(x + 5.0, top),
                    2.0,
                    t.selection,
                    true,
                );
                b.segment(
                    pos2(x - 5.0, bottom),
                    pos2(x + 5.0, bottom),
                    2.0,
                    t.selection,
                    true,
                );
            }
        }
        What::Source(Source::Port(from)) => {
            let ink = if g == Gesture::Key {
                t.hue(Function::Dynamics)
            } else {
                strip_ink(v, app, s, Some(from))
            };
            rubber_band(b, on.anchor, p, g, ink, refused);
        }
        What::Source(Source::End { route, .. }) => {
            let ink = strip_ink(v, app, s, Some(route.from));
            let g = match route.kind {
                RouteKind::Output => Gesture::Output,
                RouteKind::Send => Gesture::Send,
                RouteKind::Key(_) => Gesture::Key,
            };
            let gone = over.is_none();
            rubber_band(b, on.anchor, p, g, ink, refused || gone);
        }
        What::Track(_) => {}
    }
    // The words beside the pointer (and in the hint bar).
    let key = key_now(app, ui, on.what, v);
    let over_key = over.map(|o| match o {
        Over::Strip(s, _, tg) => Over::Strip(s, Rect::ZERO, tg),
        Over::Group(g, _, tg) => Over::Group(g, Rect::ZERO, tg),
    });
    if v.drag.words_for != Some((key, over_key)) {
        v.drag.words = words(v, app, on.what, g, over);
        v.drag.words_for = Some((key, over_key));
    }
    let warn = refused || (matches!(on.what, What::Source(Source::End { .. })) && over.is_none());
    chip(b, s, geo, p, &v.drag.words, warn);
    let name = match on.what {
        What::Source(Source::Strip(_) | Source::Group(_)) => {
            crate::i18n::t("routing.drag.reorder_hint")
        }
        What::Track(_) => crate::i18n::t("routing.drag.track_bar"),
        _ if v.drag.armed.is_some() => crate::i18n::t("routing.arm.hint"),
        _ => crate::i18n::t("routing.drag.hint"),
    };
    // Not "live": the words already ride beside the pointer on the chip, so the house's
    // value pill would only say them twice.
    hint::set(ui.ctx(), &name, Caps::NONE, false, |w| {
        w.push_str(&v.drag.words);
        Ok(())
    });
}

/// The cable being drawn: from `a` to the pointer, hanging below both, styled as the
/// route it would make — an output solid, a send thinner, a key dashed.
fn rubber_band(b: &mut Brush, a: Pos2, z: Pos2, g: Gesture, ink: egui::Color32, refused: bool) {
    const N: usize = 24;
    let dx = (z.x - a.x).abs();
    let depth = (dx * 0.3).clamp(14.0, 60.0);
    let (c1, c2) = (pos2(a.x, a.y + depth), pos2(z.x, z.y + depth));
    let mut pts = [Pos2::ZERO; N + 1];
    for (k, pt) in pts.iter_mut().enumerate() {
        let u = k as f32 / N as f32;
        let w = 1.0 - u;
        let x = w * w * w * a.x + 3.0 * w * w * u * c1.x + 3.0 * w * u * u * c2.x + u * u * u * z.x;
        let y = w * w * w * a.y + 3.0 * w * w * u * c1.y + 3.0 * w * u * u * c2.y + u * u * u * z.y;
        *pt = pos2(x, y);
    }
    let ink = if refused { with_alpha(ink, 0.45) } else { ink };
    let width = match g {
        Gesture::Output => 2.5,
        Gesture::Send => 1.75,
        Gesture::Key => 1.5,
    };
    if g == Gesture::Key {
        for k in (0..N).step_by(2) {
            b.segment(pts[k], pts[k + 1], width, ink, false);
        }
    } else {
        b.polyline(&pts, width, ink);
    }
    b.dot(a, 3.0, ink);
    match g {
        Gesture::Output => b.dot(z, 4.0, ink),
        Gesture::Send => {
            b.segment(pos2(z.x - 4.5, z.y - 4.0), z, 1.75, ink, true);
            b.segment(pos2(z.x + 4.5, z.y - 4.0), z, 1.75, ink, true);
        }
        Gesture::Key => b.circle_stroke(z, 3.5, 1.5, ink),
    }
}

/// The words beside the pointer, on a raised chip kept inside the desk.
fn chip(b: &mut Brush, s: &Style, geo: &Geo, p: Pos2, words: &str, warn: bool) {
    if words.is_empty() {
        return;
    }
    let t = &s.t;
    let tx = Text::new(words, Role::Label, t.text).policy(Policy::Elide(Elide::End));
    let size = text::measure(b.ctx(), &tx, s.tk());
    let w = (size.x + 24.0).min(420.0);
    let h = 22.0;
    let mut r = Rect::from_min_size(pos2(p.x + 14.0, p.y + 16.0), vec2(w, h));
    if r.right() > geo.full.right() - 4.0 {
        r = r.translate(vec2(-(w + 28.0), 0.0));
    }
    if r.bottom() > geo.rows.bottom() - 2.0 {
        r = r.translate(vec2(0.0, -(h + 30.0)));
    }
    let r = b.snap_rect(r);
    b.register(r, "drag_chip", &[crate::kit::Tok::PanelRaised], &[]);
    b.rrect(r, s.r(4.0), Fill::Solid(t.panel_raised));
    b.rrect_stroke(r.shrink(b.px() * 0.5), s.r(4.0), b.px(), t.edge);
    if warn {
        let bar = Rect::from_min_size(r.min + vec2(3.0, 4.0), vec2(3.0, h - 8.0));
        b.rrect(bar, 1.5, Fill::Solid(t.danger));
    }
    let _ = text::paint(b, r.shrink2(vec2(9.0, 0.0)), &tx, s.tk());
}

/// The *Route here* plate under the pointer: its words in the hint bar (the meter, sensed
/// after it, would otherwise speak for it).
fn plate_hint(v: &MixerView, ui: &Ui) {
    if v.drag.on.is_some() || v.drag.armed.is_some() || v.drag.plate.is_none() {
        return;
    }
    let name = t("routing.here.plate");
    let tip = t("routing.here.tip");
    hint::set_text(
        ui.ctx(),
        &name,
        &tip,
        Caps {
            menu: true,
            ..Caps::NONE
        },
    );
}

/// The route menu's key as the keymap binds it (Shift+F10 by default; rebindable — the
/// keymap's *Route menu* action, round 25's integration), for the menus that show it.
pub(crate) fn route_key(app: &CantusApp) -> Option<crate::keymap::Binding> {
    app.keymap
        .bindings_for(crate::keymap::Action::RouteMenu)
        .first()
        .copied()
}

/// The keymap's *Route menu* asked: the selected strip's routing menu, where its ▼ chip
/// or its number is.
fn route_key_pressed(v: &mut MixerView, app: &CantusApp, ui: &mut Ui, geo: &Geo) {
    let ctx = ui.ctx().clone();
    if !std::mem::take(&mut v.drag.route_menu) {
        return;
    }
    let Some(sel) = app.selected_strip.filter(|s| !s.is_master()) else {
        return;
    };
    let at = [super::Control::Send, super::Control::Head]
        .into_iter()
        .find_map(|c| super::control_rect(&ctx, Some(sel), c))
        .map_or(geo.middle.center(), |r| r.left_bottom());
    v.popups.open(Menu::Route(sel), at);
}

/// *Show the destination*: bring the strip into view, and flash its column.
fn reveal(v: &mut MixerView, b: &mut Brush, s: &Style, geo: &Geo, ctx: &egui::Context, now: f64) {
    let asked = ctx.data(|d| d.get_temp::<(StripId, f64)>(crate::views::route_menu::reveal_id()));
    // A request older than the flash (the desk was off screen when it was made, and
    // stayed so) is dropped: scrolling to it now would surprise.
    if asked.is_some() {
        ctx.data_mut(|d| d.remove::<(StripId, f64)>(crate::views::route_menu::reveal_id()));
    }
    if let Some((id, at)) = asked.filter(|(_, at)| now - at <= FLASH) {
        v.drag.reveal = Some((id, at));
        v.drag.reveal_at = None;
        // In the middle and out of view: scroll it in.
        let mut x = 0.0;
        for item in &v.desk.middle {
            let w = super::desk::item_width(item, geo.pitch);
            if matches!(item, Item::Strip(s, _) if *s == id) {
                if x < v.scroll_x || x + w > v.scroll_x + geo.middle.width() {
                    v.scroll_x = (x + w * 0.5 - geo.middle.width() * 0.5).max(0.0);
                }
                break;
            }
            x += w;
        }
        ctx.request_repaint();
    }
    let Some((_, at)) = v.drag.reveal else {
        return;
    };
    let age = now - at;
    if age > FLASH {
        v.drag.reveal = None;
        v.drag.reveal_at = None;
        return;
    }
    if let Some(col) = v.drag.reveal_at.take() {
        let a = (1.0 - (age / FLASH) as f32).clamp(0.0, 1.0);
        let col = col.intersect(geo.rows);
        b.rect(col, Fill::Solid(with_alpha(s.t.highlight, 0.16 * a)));
        b.rrect_stroke(col.shrink(1.0), 0.0, 2.0, with_alpha(s.t.highlight, a));
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
}

/// Unused names kept for the desk builder's band (the strip a drag starts from).
#[allow(
    dead_code,
    reason = "the desk builder's band may light the dragged strip's port"
)]
pub(crate) fn dragged_from(v: &MixerView) -> Option<StripId> {
    v.drag.on.and_then(|o| source_strip(o.what))
}

#[cfg(test)]
mod tests {
    //! The cable gestures (design §8 tests 9 and 10), driven through real frames of the
    //! desk. The ports and end marks are the desk builder's controls, so a press on one is
    //! seeded as [`source`] records it; everything after — travel, the columns' answers,
    //! the words, the drop, Esc — runs as it does for a musician.
    use super::*;
    use crate::views::mixer::{self, Control};
    use cantus_core::{Project, Send};
    use cantus_engine::EngineHandle;
    use egui::Event;

    struct Rig {
        ctx: egui::Context,
        app: CantusApp,
        frame: u64,
        pointer: Option<Pos2>,
        mods: Modifiers,
    }

    impl Rig {
        fn new() -> Rig {
            crate::i18n::default_thread_locale(crate::i18n::Locale::En);
            let ctx = egui::Context::default();
            crate::design::install_fonts(&ctx);
            crate::design::install_style(&ctx);
            let app =
                CantusApp::from_parts(Some(crate::starter_project()), EngineHandle::dummy(), false);
            let mut r = Rig {
                ctx,
                app,
                frame: 0,
                pointer: None,
                mods: Modifiers::NONE,
            };
            r.idle(3);
            r
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.frame += 1;
            let mut ev = vec![Event::ModifiersChanged(self.mods)];
            if let Some(p) = self.pointer {
                ev.push(Event::PointerMoved(p));
            }
            ev.extend(events);
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
                time: Some(self.frame as f64 / 60.0),
                predicted_dt: 1.0 / 60.0,
                events: ev,
                ..Default::default()
            };
            let app = &mut self.app;
            let mut out = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| mixer::show(app, ui));
            });
            out.textures_delta.clear();
        }

        fn idle(&mut self, n: usize) {
            for _ in 0..n {
                self.frame(Vec::new());
            }
        }

        fn ids(&self) -> Vec<StripId> {
            self.app.project().strips.iter().map(|s| s.id).collect()
        }

        /// The middle of a strip's fader: inside its column.
        fn column(&self, strip: Option<StripId>) -> Pos2 {
            self.at(strip, Control::Fader)
        }

        /// Where a press stands in for the desk builder's port or end mark: the strip's
        /// meter (it senses clicks only, so a press there moves nothing as it travels).
        fn port(&self, strip: Option<StripId>) -> Pos2 {
            self.at(strip, Control::Meter)
        }

        fn at(&self, strip: Option<StripId>, c: Control) -> Pos2 {
            mixer::control_rect(&self.ctx, strip, c)
                .map(|r| r.center())
                .unwrap_or_else(|| panic!("{c:?} of {strip:?} is not on the desk"))
        }

        fn button(&mut self, at: Pos2, pressed: bool) {
            self.pointer = Some(at);
            let modifiers = self.mods;
            self.frame(vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
            }]);
        }

        /// A press on `src` at `a` (as `source` records it), carried to `z`, held there.
        fn carry(&mut self, src: Source, a: Pos2, z: Pos2) {
            self.pointer = Some(a);
            self.frame(Vec::new());
            self.app.mixer.drag.press = Some(Press {
                src,
                origin: a,
                anchor: Rect::from_center_size(a, vec2(8.0, 8.0)),
            });
            self.button(a, true);
            for k in 1..=6 {
                self.pointer = Some(a + (z - a) * (k as f32 / 6.0));
                self.frame(Vec::new());
            }
            self.frame(Vec::new());
        }

        fn release(&mut self) {
            let at = self.pointer.unwrap_or_default();
            self.button(at, false);
            self.idle(2);
        }

        fn words(&self) -> String {
            self.app.mixer.drag.words.clone()
        }
    }

    fn restores(app: &mut CantusApp, before: &Project) {
        app.undo();
        let mut now = app.project().clone();
        now.next_id = before.next_id;
        if &now != before {
            for (a, b) in now.strips.iter().zip(&before.strips) {
                if a != b {
                    panic!(
                        "strip differs:
{a:?}
{b:?}"
                    );
                }
            }
            panic!("one undo step restores the project");
        }
    }

    #[test]
    fn a_plain_cable_sends_ctrl_routes_alt_keys() {
        let mut r = Rig::new();
        let s = r.ids();
        let before = r.app.project().clone();
        // Plain: a unity post-fader send; the output untouched.
        let (a, z) = (r.port(Some(s[1])), r.column(Some(s[4])));
        r.carry(Source::Port(s[1]), a, z);
        assert!(r.words().starts_with("Send to 5"), "{}", r.words());
        assert!(r.app.mixer.drag.on.is_some(), "the drag is on");
        assert_eq!(r.ctx.dragged_id(), Some(drag_id()), "nothing else lights");
        r.release();
        let st = r.app.project().strip(s[1]).unwrap();
        assert_eq!(st.send_to(s[4]), Some(&Send::new(s[4])));
        assert_eq!(st.output, None);
        assert_eq!(r.app.bus.undo_label_key(), Some("cmd.add_send"));
        restores(&mut r.app, &before);
        // Ctrl: the only output; the modifier read live while carrying.
        r.mods = Modifiers::COMMAND;
        r.carry(Source::Port(s[1]), a, z);
        assert!(r.words().contains("only output"), "{}", r.words());
        r.release();
        r.mods = Modifiers::NONE;
        assert_eq!(r.app.project().strip(s[1]).unwrap().output, Some(s[4]));
        assert_eq!(r.app.bus.undo_label_key(), Some("cmd.replace_output"));
        // Alt onto a strip with one compressor: keyed at once.
        let comp = r.app.duck_strip_from(s[6], s[7]).expect("a compressor");
        r.idle(1);
        r.mods = Modifiers::ALT;
        let k = r.column(Some(s[6]));
        let a2 = r.port(Some(s[2]));
        r.carry(Source::Port(s[2]), a2, k);
        r.release();
        r.mods = Modifiers::NONE;
        let fx = r.app.project().strip(s[6]).unwrap().effect(comp).unwrap();
        assert_eq!(fx.sidechain, Some(s[2]));
    }

    #[test]
    fn alt_onto_nothing_to_key_offers_a_compressor_or_a_gate() {
        let mut r = Rig::new();
        let s = r.ids();
        r.mods = Modifiers::ALT;
        let (a, z) = (r.port(Some(s[2])), r.column(Some(s[5])));
        r.carry(Source::Port(s[2]), a, z);
        r.release();
        r.mods = Modifiers::NONE;
        let owner = mixer::route_owner();
        assert!(crate::views::route_menu::is_open(&r.ctx, owner));
        let rows = crate::views::route_menu::row_labels(&r.ctx, owner);
        assert!(
            rows.iter()
                .any(|l| l.starts_with("Add a gate keyed from 3")),
            "{rows:?}"
        );
    }

    #[test]
    fn a_loop_target_refuses_before_the_drop_and_issues_nothing() {
        let mut r = Rig::new();
        let s = r.ids();
        r.app.exec(Command::SetStripOutput {
            id: s[0],
            output: Some(s[1]),
        });
        r.idle(1);
        let before = r.app.project().clone();
        let label = r.app.bus.undo_label_key().map(str::to_string);
        // Strip 2 into strip 1, which already feeds it.
        let (a, z) = (r.port(Some(s[1])), r.column(Some(s[0])));
        r.carry(Source::Port(s[1]), a, z);
        assert!(
            matches!(r.app.mixer.drag.checks.first(), Some(Some(Target::Refused))),
            "the column says refused while carrying"
        );
        assert!(r.words().contains("Would make a loop"), "{}", r.words());
        r.release();
        assert!(r.app.project() == &before, "nothing was issued");
        assert_eq!(r.app.bus.undo_label_key().map(str::to_string), label);
    }

    #[test]
    fn a_cable_from_a_selection_sends_from_every_selected_strip_in_one_step() {
        let mut r = Rig::new();
        let s = r.ids();
        r.app.select_strip(s[1]);
        r.app.toggle_strip_selected(s[2]);
        r.idle(1);
        let (a, z) = (r.port(Some(s[1])), r.column(Some(s[6])));
        r.carry(Source::Port(s[1]), a, z);
        assert!(r.words().starts_with("Send 2 strips"), "{}", r.words());
        r.release();
        for from in [s[1], s[2]] {
            assert!(r.app.project().strip(from).unwrap().send_to(s[6]).is_some());
        }
        r.app.undo();
        assert!(r.app.project().strip(s[1]).unwrap().sends.is_empty());
        assert!(r.app.project().strip(s[2]).unwrap().sends.is_empty());
    }

    #[test]
    fn an_end_moves_the_route_and_dropped_on_nothing_removes_it() {
        let mut r = Rig::new();
        let s = r.ids();
        let mut send = Send::new(s[3]);
        send.level = 0.5;
        r.app.set_strip_send(s[1], send);
        r.app.end_gesture();
        r.idle(1);
        let before = r.app.project().clone();
        let route = Route::send(s[1], s[3]);
        let end = Source::End {
            route,
            end: End::Destination,
        };
        let (a, z) = (r.port(Some(s[3])), r.column(Some(s[5])));
        r.carry(end, a, z);
        assert!(r.words().starts_with("Move this route"), "{}", r.words());
        r.release();
        let st = r.app.project().strip(s[1]).unwrap();
        assert_eq!(st.send_to(s[5]).map(|x| x.level), Some(0.5), "level kept");
        assert!(st.send_to(s[3]).is_none());
        assert_eq!(r.app.bus.undo_label_key(), Some("cmd.move_route"));
        restores(&mut r.app, &before);
        // Released over the title row (no column): the route goes.
        r.idle(1);
        let title = pos2(a.x, 10.0);
        r.carry(end, a, title);
        assert!(r.words().contains("Release to remove"), "{}", r.words());
        r.release();
        assert!(r.app.project().strip(s[1]).unwrap().sends.is_empty());
        restores(&mut r.app, &before);
    }

    #[test]
    fn escape_cancels_a_cable() {
        let mut r = Rig::new();
        let s = r.ids();
        let before = r.app.project().clone();
        let (a, z) = (r.port(Some(s[1])), r.column(Some(s[4])));
        r.carry(Source::Port(s[1]), a, z);
        r.frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        assert!(r.app.mixer.drag.on.is_none());
        r.release();
        assert!(r.app.project() == &before);
    }

    #[test]
    fn a_cable_near_the_middles_edge_scrolls_it() {
        let mut r = Rig::new();
        while r.app.project().strips.len() < 64 {
            r.app.add_strip();
        }
        let s = r.ids();
        r.app.select_strip(s[1]);
        r.idle(2);
        let right = r.app.mixer.drag.docks.expect("docks seen")[1].right();
        let a = r.port(Some(s[1]));
        r.carry(Source::Port(s[1]), a, pos2(right - 4.0, a.y));
        r.idle(10);
        assert!(r.app.mixer.scroll().0 > 0.0, "the middle scrolled");
        r.release();
    }

    #[test]
    fn the_columns_are_worked_out_once_per_drag_not_per_frame() {
        let mut r = Rig::new();
        let s = r.ids();
        let (a, z) = (r.port(Some(s[1])), r.column(Some(s[4])));
        r.carry(Source::Port(s[1]), a, z);
        let key = r.app.mixer.drag.checks_for;
        r.idle(3);
        assert_eq!(r.app.mixer.drag.checks_for, key);
        assert!(key.is_some());
        r.release();
    }

    /// *Show the destination* asked while the desk was off screen, longer ago than the
    /// flash: dropped, not acted on late (no scroll, no flash).
    #[test]
    fn a_stale_reveal_request_is_dropped() {
        let mut r = Rig::new();
        while r.app.project().strips.len() < 64 {
            r.app.add_strip();
        }
        r.app.select_strip(r.ids()[0]);
        r.idle(2);
        let start = r.app.mixer.scroll().0;
        let far = r.ids()[60];
        let old = r.frame as f64 / 60.0 - FLASH - 1.0;
        r.ctx.data_mut(|d| {
            d.insert_temp(crate::views::route_menu::reveal_id(), (far, old));
        });
        r.idle(2);
        assert_eq!(r.app.mixer.scroll().0, start, "no late scroll");
        assert!(r.app.mixer.drag.reveal.is_none());
        // A fresh one scrolls.
        let now = r.frame as f64 / 60.0;
        r.ctx.data_mut(|d| {
            d.insert_temp(crate::views::route_menu::reveal_id(), (far, now));
        });
        r.idle(2);
        assert!(r.app.mixer.scroll().0 > start + 100.0);
    }

    #[test]
    fn a_folded_group_refuses_a_cable_and_springs_open_under_it() {
        let mut r = Rig::new();
        let s = r.ids();
        let g = r.app.create_strip_group(&[s[3], s[4]]).expect("a group");
        r.app.exec(Command::SetStripGroupCollapsed {
            id: g,
            collapsed: true,
        });
        r.idle(2);
        let sep = r
            .ctx
            .read_response(Id::new(("mixer-group", g.0)))
            .expect("the separator is drawn")
            .rect
            .center();
        let a = r.port(Some(s[1]));
        r.carry(Source::Port(s[1]), a, sep);
        assert!(
            matches!(
                r.app.mixer.drag.over,
                None | Some(Over::Group(_, _, Target::Refused))
            ),
            "a folded group is not a target"
        );
        assert!(r.words().contains("folded"), "{}", r.words());
        // Held over it past the spring time (frames are 1/60 s): it opens.
        r.idle(45);
        assert!(!r.app.project().strip_group(g).unwrap().collapsed);
        r.release();
    }
}

#[cfg(test)]
#[path = "mixer_drag_shots.rs"]
mod shots;
