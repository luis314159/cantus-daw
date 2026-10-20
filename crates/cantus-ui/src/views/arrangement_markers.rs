//! The markers on the ruler (round 29, ADR 0060): FL's playlist markers and Cubase's
//! marker track. A lane under the loop lane, there while the song has markers: each marker
//! a tab with its name in its colour (the track palette's, or the accent) and a line down
//! from it. Click a marker to put the playhead there (anywhere on its tab, as in FL —
//! review 2); double-click it to rename it; drag it along the grid; its menu renames,
//! recolours, loops, selects or exports to the next marker, and removes it. Markers on one
//! tick sit side by side. Double-click the lane to
//! add a marker there (Alt+M adds one at the playhead); the lane's menu adds a marker or a
//! metre change, jumps to a marker by name, loops and selects between markers, and shows
//! or hides the tempo track.

use super::{beats_to_ticks, chrome, ticks_to_beats, TimelineGeometry};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, Brush, Fill, Style, Tok};
use cantus_core::tempo::{Marker, MarkerId};
use cantus_core::{Color, Command, MergeKey, Ticks};
use egui::{pos2, vec2, Align2, Color32, Id, Rect, Sense};

/// The marker lane's height at full size.
pub const MARKER_H: f32 = 16.0;

/// What the marker lane remembers between frames.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The marker being renamed, and the words so far.
    rename: Option<(MarkerId, String)>,
    /// What the lane's menu was opened on.
    menu: Option<MenuOn>,
    /// The marker whose colours are offered, and the pass they were offered in (the click
    /// that offered them is not a click away from them).
    colour: Option<(MarkerId, u64)>,
    /// Each visible marker's tab and its name's letters, last frame (published geometry:
    /// tests aim with it).
    drawn: Vec<(MarkerId, Rect, Rect)>,
}

impl State {
    /// Each visible marker's tab and its letters as drawn last frame.
    pub fn tabs(&self) -> &[(MarkerId, Rect, Rect)] {
        &self.drawn
    }

    /// The marker being renamed.
    pub fn renaming(&self) -> Option<MarkerId> {
        self.rename.as_ref().map(|(id, _)| *id)
    }

    /// The marker whose colours are offered.
    pub fn colours_open(&self) -> Option<MarkerId> {
        self.colour.map(|(id, _)| id)
    }
}

/// Offer `id`'s colours (the menu's *Colour…*), from this pass on.
pub fn offer_colours(app: &mut CantusApp, id: MarkerId, ctx: &egui::Context) {
    app.arrangement.markers.colour = Some((id, ctx.cumulative_pass_nr()));
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum MenuOn {
    Marker(MarkerId),
    At(Ticks),
}

/// The lane's height: there while the song has markers.
pub(super) fn height(app: &CantusApp, s: &Style) -> f32 {
    if app.project().markers.is_empty() {
        0.0
    } else {
        s.s(MARKER_H)
    }
}

/// A marker's colour on screen.
fn ink(m: &Marker, s: &Style) -> Color32 {
    m.color.map_or(s.t.highlight, crate::theme::track_color)
}

/// Where each visible marker's tab is, and where its name's letters are.
fn tabs(
    app: &CantusApp,
    ctx: &egui::Context,
    geo: &TimelineGeometry,
    lane: Rect,
    s: &Style,
) -> Vec<(MarkerId, Rect, Rect)> {
    let k = s.k;
    let markers = &app.project().markers;
    let mut out: Vec<(MarkerId, Rect, Rect)> = Vec::with_capacity(markers.len());
    // The right edge of the tab before, when it is on the same tick: the next starts there
    // (review 2: a later tab on one tick covered the earlier one whole).
    let mut beside: Option<(Ticks, f32)> = None;
    for (i, m) in markers.iter().enumerate() {
        let at = geo.x(ticks_to_beats(m.tick));
        let x = match beside {
            Some((t, right)) if t == m.tick => right + 1.0 * k,
            _ => at,
        };
        // The next marker on another tick bounds the tab.
        let next = markers[i + 1..]
            .iter()
            .find(|n| n.tick > m.tick)
            .map_or(f32::INFINITY, |n| geo.x(ticks_to_beats(n.tick)));
        if x > lane.right() || next < lane.left() {
            beside = None;
            continue;
        }
        let w = text::measure(
            ctx,
            &Text::new(&m.name, Role::Micro, s.t.text).caps(false),
            s.tk(),
        )
        .x;
        // On one tick the tabs keep their whole names; otherwise the next marker cuts.
        let same = markers.get(i + 1).is_some_and(|n| n.tick == m.tick);
        let full = x + w + 10.0 * k;
        let right = if same {
            full
        } else {
            full.min(next - 2.0 * k).max(x + 6.0 * k)
        };
        let tab = Rect::from_min_max(pos2(x, lane.top() + 1.0), pos2(right, lane.bottom() - 1.0));
        let letters = Rect::from_min_max(
            pos2(x + 4.0 * k, tab.top()),
            pos2((x + 4.0 * k + w).min(tab.right()), tab.bottom()),
        );
        out.push((m.id, tab, letters));
        beside = Some((m.tick, right));
    }
    out
}

/// The lane and its markers.
pub(super) fn paint(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, lane: Rect, s: &Style) {
    if lane.height() <= 0.0 {
        return;
    }
    let t = &s.t;
    let k = s.k;
    b.register(lane, "marker_lane", &[Tok::Ink, Tok::Highlight], &[]);
    b.rect(b.snap_rect(lane), Fill::Solid(mix(t.ink, t.row_black, 0.2)));
    let renaming = app.arrangement.markers.rename.as_ref().map(|(id, _)| *id);
    for (id, tab, _) in tabs(app, b.ctx(), geo, lane, s) {
        let Some(m) = app.project().marker(id) else {
            continue;
        };
        let c = ink(m, s);
        b.rect(b.snap_rect(tab), Fill::Solid(with_alpha(c, 0.35)));
        let at = geo.x(ticks_to_beats(m.tick));
        b.vline(at, lane.top(), lane.bottom(), 2.0 * b.px(), c);
        if renaming == Some(id) {
            continue;
        }
        let _ = text::paint(
            b,
            tab.shrink2(vec2(4.0 * k, 0.0)),
            &Text::new(&m.name, Role::Micro, t.text)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    }
}

/// The markers' lines down the lanes, faint, so a section reads under its name.
pub(super) fn paint_lines(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    lanes: Rect,
    s: &Style,
) {
    for m in &app.project().markers {
        let x = geo.x(ticks_to_beats(m.tick));
        if x >= lanes.left() && x <= lanes.right() {
            b.vline(
                x,
                lanes.top(),
                lanes.bottom(),
                b.px(),
                with_alpha(ink(m, s), 0.45),
            );
        }
    }
}

/// The tick under `x`, on the grid unless `free` or snap is off.
fn tick_at(app: &CantusApp, geo: &TimelineGeometry, x: f32, free: bool) -> Ticks {
    let t = beats_to_ticks(geo.beats(x).max(0.0));
    if free || !app.arrangement.snap {
        t
    } else {
        crate::views::bars::snap(app.project(), t, app.arrangement.grid)
    }
}

/// The lane's gestures and each marker's (registered after the timeline's response). The
/// lane's own response first, the markers' over it.
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
    let mods = ui.input(|i| i.modifiers);
    let resp = ui.interact(lane, Id::new("arrangement-marker-lane"), Sense::click());
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            &t("tempo.marker_lane"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if resp.double_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let at = tick_at(app, geo, p.x, mods.command);
            if let Some(id) = app.add_marker(at) {
                let name = app.project().marker(id).map(|m| m.name.clone());
                app.arrangement.markers.rename = name.map(|n| (id, n));
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            app.arrangement.markers.menu = Some(MenuOn::At(tick_at(app, geo, p.x, mods.command)));
        }
    }
    egui::Popup::context_menu(&resp)
        .frame(chrome::menu_frame())
        .show(|ui| lane_menu(app, ui));

    let drawn = tabs(app, ui.ctx(), geo, lane, s);
    app.arrangement.markers.drawn.clone_from(&drawn);
    for (id, tab, _letters) in drawn {
        let r = ui.interact(tab, Id::new(("marker", id.0)), Sense::click_and_drag());
        if r.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            let name = app
                .project()
                .marker(id)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            hint::set_text(
                ui.ctx(),
                &t("tempo.marker"),
                &name,
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        // A click goes there; a double-click (the click went there first) renames.
        if r.double_clicked() {
            if let Some(m) = app.project().marker(id).cloned() {
                app.arrangement.markers.rename = Some((id, m.name));
            }
        } else if r.clicked() {
            if let Some(m) = app.project().marker(id).cloned() {
                app.seek(m.tick);
            }
        }
        if r.dragged_by(egui::PointerButton::Primary) {
            if let (Some(p), Some(m)) =
                (r.interact_pointer_pos(), app.project().marker(id).cloned())
            {
                let at = tick_at(app, geo, p.x, mods.command);
                if at != m.tick {
                    app.exec_merging(
                        Command::SetMarker {
                            marker: Marker { tick: at, ..m },
                        },
                        MergeKey::of(&("marker-move", id.0)),
                    );
                }
            }
        }
        if r.drag_stopped() {
            app.end_gesture();
        }
        if r.secondary_clicked() {
            app.arrangement.markers.menu = Some(MenuOn::Marker(id));
        }
        egui::Popup::context_menu(&r)
            .frame(chrome::menu_frame())
            .show(|ui| marker_menu(app, ui, id));
        if app.arrangement.markers.colours_open() == Some(id) {
            colours(app, ui, id, tab, s);
        }
    }
    rename(app, ui, geo, lane, s);
}

/// A marker's menu.
fn marker_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: MarkerId) {
    let Some(m) = app.project().marker(id).cloned() else {
        return;
    };
    let next = app
        .project()
        .markers
        .iter()
        .find(|n| n.tick > m.tick)
        .map(|n| n.id);
    let to_next = |label: &'static str| Item::Action {
        label: Src::Key(label),
        check: Check::None,
        shortcut: None,
        submenu: false,
        enabled: next.is_some(),
    };
    let items = [
        Item::action(Src::Key("tempo.menu.rename_marker")),
        Item::action(Src::Key("tempo.menu.colour")),
        Item::Separator,
        to_next("tempo.menu.loop_next"),
        to_next("tempo.menu.select_next"),
        to_next("tempo.menu.export_next"),
        Item::Separator,
        Item::action(Src::Key("tempo.menu.remove_marker")),
    ];
    match kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
        Some(0) => {
            app.arrangement.markers.rename = Some((id, m.name));
            ui.close();
        }
        Some(1) => {
            offer_colours(app, id, ui.ctx());
            ui.close();
        }
        Some(3) => {
            if let Some((a, b)) = next.and_then(|n| app.span_between(id, n)) {
                app.loop_span(a, b);
            }
            ui.close();
        }
        Some(4) => {
            if let Some((a, b)) = next.and_then(|n| app.span_between(id, n)) {
                app.set_time_selection(a, b);
            }
            ui.close();
        }
        Some(5) => {
            if let Some(n) = next {
                app.export_between_markers(id, n);
            }
            ui.close();
        }
        Some(7) => {
            app.exec(Command::RemoveMarker { id });
            ui.close();
        }
        Some(_) => ui.close(),
        None => {}
    }
}

/// The lane's menu: a marker or a metre change added where it was opened, a jump to any
/// marker by name, looping and selecting between markers, the tempo track shown.
fn lane_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    let at = match app.arrangement.markers.menu {
        Some(MenuOn::At(t)) => t,
        _ => app.engine.position(),
    };
    let names: Vec<(MarkerId, String)> = app
        .project()
        .markers
        .iter()
        .map(|m| (m.id, m.name.clone()))
        .collect();
    let shown = app.tempo_lane_shown();
    let mut items = vec![
        Item::action(Src::Key("tempo.menu.add_marker")),
        Item::action(Src::Key("tempo.menu.add_metre")),
        Item::Separator,
        Item::action(Src::Key("tempo.menu.loop_markers")),
        Item::action(Src::Key("tempo.menu.select_markers")),
        Item::Action {
            label: Src::Key("tempo.view_menu"),
            check: if shown { Check::On } else { Check::Off },
            shortcut: None,
            submenu: false,
            enabled: true,
        },
    ];
    let first_name = items.len() + 2;
    if !names.is_empty() {
        items.push(Item::Separator);
        items.push(Item::Band(Src::Key("tempo.menu.go_to")));
        for (_, n) in &names {
            items.push(Item::action(Src::Str(n)));
        }
    }
    match kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
        Some(0) => {
            if let Some(id) = app.add_marker(at) {
                let name = app.project().marker(id).map(|m| m.name.clone());
                app.arrangement.markers.rename = name.map(|n| (id, n));
            }
            ui.close();
        }
        Some(1) => {
            super::tempo::add_metre_change(app, at);
            ui.close();
        }
        Some(3) => {
            app.loop_between_markers(at);
            ui.close();
        }
        Some(4) => {
            app.select_between_markers(at);
            ui.close();
        }
        Some(5) => {
            app.toggle_tempo_lane();
            ui.close();
        }
        Some(i) if i >= first_name && i - first_name < names.len() => {
            let id = names[i - first_name].0;
            if let Some(m) = app.project().marker(id) {
                let tick = m.tick;
                app.seek(tick);
            }
            ui.close();
        }
        Some(_) => ui.close(),
        None => {}
    }
}

/// A marker's name being typed: the kit's field over its tab; Enter or a click away keeps
/// it, Esc leaves the name as it was.
fn rename(app: &mut CantusApp, ui: &mut egui::Ui, geo: &TimelineGeometry, lane: Rect, s: &Style) {
    let Some((id, mut words)) = app.arrangement.markers.rename.take() else {
        return;
    };
    let Some(m) = app.project().marker(id) else {
        return;
    };
    let k = s.k;
    let x = geo.x(ticks_to_beats(m.tick));
    let fid = Id::new(("marker-rename", id.0));
    let mut event = None;
    egui::Area::new(fid.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(x + 2.0 * k, lane.top()))
        .show(ui.ctx(), |ui| {
            let (line, _) = ui
                .allocate_exact_size(vec2(140.0 * k, lane.height().max(18.0 * k)), Sense::hover());
            let out = crate::kit::field::Field::new(fid)
                .scale(k)
                .opens_when_shown()
                .show(ui, line, &mut words);
            event = out.event;
        });
    match event {
        Some(crate::kit::field::Event::Commit) => app.rename_marker(id, &words),
        Some(crate::kit::field::Event::Cancel) => {}
        None => app.arrangement.markers.rename = Some((id, words)),
    }
}

/// The track palette offered for a marker (and the accent), as swatches under its tab.
fn colours(app: &mut CantusApp, ui: &mut egui::Ui, id: MarkerId, tab: Rect, s: &Style) {
    let k = s.k;
    let side = 16.0 * k;
    let choices: Vec<Option<Color>> = std::iter::once(None)
        .chain(Color::PALETTE.iter().copied().map(Some))
        .collect();
    let mut picked = None;
    let mut away = false;
    let area = egui::Area::new(Id::new(("marker-colours", id.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(tab.left(), tab.bottom() + 2.0 * k))
        .show(ui.ctx(), |ui| {
            let w = side * choices.len() as f32 + 4.0 * k * (choices.len() + 1) as f32;
            let (rect, _) = ui.allocate_exact_size(vec2(w, side + 8.0 * k), Sense::hover());
            let mut rects = Vec::with_capacity(choices.len());
            for (i, _) in choices.iter().enumerate() {
                let x = rect.left() + 4.0 * k + i as f32 * (side + 4.0 * k);
                let r = Rect::from_min_size(pos2(x, rect.top() + 4.0 * k), vec2(side, side));
                if ui
                    .interact(r, Id::new(("marker-colour", id.0, i)), Sense::click())
                    .clicked()
                {
                    picked = Some(i);
                }
                rects.push(r);
            }
            let current = app.project().marker(id).and_then(|m| m.color);
            kit::with_brush(ui.painter(), |b| {
                b.rrect(rect, 4.0 * k, Fill::Solid(s.t.panel));
                for (r, c) in rects.iter().zip(&choices) {
                    let fill = c.map_or(s.t.highlight, crate::theme::track_color);
                    b.rrect(*r, 3.0 * k, Fill::Solid(fill));
                    if *c == current {
                        b.rrect_stroke(r.expand(1.5 * k), 4.0 * k, 1.5 * k, s.t.text);
                    }
                }
            });
        });
    // A click away closes them — not the click that offered them, in this same pass
    // (review 2) — and so does Esc.
    let offered = app.arrangement.markers.colour.map_or(0, |(_, pass)| pass);
    if area.response.clicked_elsewhere() && ui.ctx().cumulative_pass_nr() > offered {
        away = true;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        away = true;
    }
    if let Some(i) = picked {
        if let Some(m) = app.project().marker(id).cloned() {
            app.exec(Command::SetMarker {
                marker: Marker {
                    color: choices[i],
                    ..m
                },
            });
        }
        app.arrangement.markers.colour = None;
    } else if away {
        app.arrangement.markers.colour = None;
    }
}

/// Start renaming marker `id` (one just added), its name so far `name`.
pub(super) fn rename_now(app: &mut CantusApp, id: MarkerId, name: Option<String>) {
    app.arrangement.markers.rename = name.map(|n| (id, n));
}
