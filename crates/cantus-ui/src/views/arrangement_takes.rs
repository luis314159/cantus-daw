//! Take lanes under a track (ADR 0025 §5): each lane a row of its own with its take's
//! clips — drawn by the arrangement's own clip painter, so an audio take shows the same
//! waveform body lane 0 does — and the comp lit on them.
//!
//! - **Swipe comping**: drag across a take lane and that range of the playing lane becomes
//!   a window on the take (`comping::swipe`), one undo step per swipe; the range is shown
//!   while it is dragged. A click on a take clip makes the whole clip the comp; a click on
//!   a lane's name in the header, the whole lane.
//! - The lane's menu: use this take, delete unused takes, explode takes to new tracks,
//!   hide the takes.
//!
//! The lanes sit between their track's row and the next one; [`height`] is what the
//! arrangement's row layout adds for them.

use super::chrome;
use super::{beats_to_ticks, ticks_to_beats, TimelineGeometry, TrackRow};
use crate::app::CantusApp;
use crate::comping;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, led, Brush, Fill, Style, Tok};
use crate::record_pass::max_lane;
use crate::views::clips_chrome::Tint;
use cantus_core::{Ticks, Track, TrackId};
use egui::{pos2, vec2, Align2, Id, Rect, Sense};

/// A take lane's height at the base row height.
pub(super) fn lane_height(base: f32) -> f32 {
    (base * 0.75).round().max(18.0)
}

/// How many take lanes a track shows: up to its highest, while they are open.
pub(super) fn shown_lanes(t: &Track) -> u8 {
    if t.takes_open {
        max_lane(t)
    } else {
        0
    }
}

/// The height the take lanes add under a track's row.
pub(super) fn height(t: &Track, base: f32) -> f32 {
    shown_lanes(t) as f32 * lane_height(base)
}

/// Where take lane `lane` (1…) of a track's row is drawn, as a row of its own; `None`
/// while the lanes are hidden.
pub(super) fn lane_row(r: &TrackRow, lane: u8, base: f32) -> Option<TrackRow> {
    if lane == 0 || r.takes <= 0.0 {
        return None;
    }
    let h = lane_height(base);
    let top = r.top + r.height + (lane - 1) as f32 * h;
    (top + h <= r.top + r.height + r.takes + 0.5).then_some(TrackRow {
        top,
        height: h,
        takes: 0.0,
        auto: 0.0,
        pitch: 0.0,
    })
}

/// A swipe in progress: the track, the lane, where it was pressed and where it is now.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Swipe {
    track: TrackId,
    lane: u8,
    from: Ticks,
    to: Ticks,
}

fn swipe_id() -> Id {
    Id::new("arrangement-take-swipe")
}

/// The lanes' ground: a darker wash than the playing lane, a hairline between lanes.
pub(super) fn grounds(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let t = &s.t;
    let base = app.arrangement.track_height;
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(row) = geo.rows.get(i) else { continue };
        for lane in 1..=shown_lanes(track) {
            let Some(r) = lane_row(row, lane, base) else {
                break;
            };
            if r.top > rect.bottom() || r.top + r.height < rect.top() {
                continue;
            }
            let band = Rect::from_min_max(
                pos2(rect.left(), r.top),
                pos2(rect.right(), r.top + r.height),
            );
            b.register(band, "take_lane", &[Tok::Ink, Tok::Grid], &[]);
            b.rect(
                b.snap_rect(band),
                Fill::Solid(with_alpha(t.ink, if t.dark { 0.38 } else { 0.10 })),
            );
            b.hline(
                rect.left(),
                rect.right(),
                band.bottom() - 0.5 * b.px(),
                b.px(),
                t.grid,
            );
        }
    }
}

/// Over the take clips: the comp lit (a wash and a bar along the lane's foot in the
/// selection colour), and the swipe being dragged.
pub(super) fn overlay(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let t = &s.t;
    let base = app.arrangement.track_height;
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(row) = geo.rows.get(i) else { continue };
        if shown_lanes(track) == 0 {
            continue;
        }
        for take in track.clips.iter().filter(|c| c.lane > 0) {
            let Some(r) = lane_row(row, take.lane, base) else {
                continue;
            };
            if r.top > rect.bottom() || r.top + r.height < rect.top() {
                continue;
            }
            // What is not in the comp steps back (Logic's unused takes), so the comp reads
            // at a glance down the lanes. The comped ranges come in order.
            let shade_range = |b: &mut Brush, a: Ticks, z: Ticks| {
                let x0 = geo.x(ticks_to_beats(a)).max(rect.left());
                let x1 = geo.x(ticks_to_beats(z)).min(rect.right());
                if x1 > x0 {
                    b.rect(
                        Rect::from_min_max(pos2(x0, r.top + 1.0), pos2(x1, r.top + r.height - 1.0)),
                        Fill::Solid(with_alpha(if t.dark { t.ink } else { t.panel }, 0.38)),
                    );
                }
            };
            let mut cursor = take.start;
            comping::for_each_comped(track, take, |a, z| {
                if a > cursor {
                    shade_range(b, cursor, a);
                }
                cursor = cursor.max(z);
                let x0 = geo.x(ticks_to_beats(a)).max(rect.left());
                let x1 = geo.x(ticks_to_beats(z)).min(rect.right());
                if x1 <= x0 {
                    return;
                }
                let lit =
                    Rect::from_min_max(pos2(x0, r.top + 1.0), pos2(x1, r.top + r.height - 1.0));
                b.rect(lit, Fill::Solid(with_alpha(t.selection, 0.10)));
                b.rect(
                    Rect::from_min_max(pos2(x0, lit.bottom() - 3.0 * s.k), lit.max),
                    Fill::Solid(t.selection),
                );
            });
            if take.end() > cursor {
                shade_range(b, cursor, take.end());
            }
        }
    }
    let swipe: Option<Swipe> = b.ctx().data(|d| d.get_temp(swipe_id()));
    if let Some(sw) = swipe {
        let Some(i) = app.project().tracks.iter().position(|t| t.id == sw.track) else {
            return;
        };
        let Some(r) = geo.rows.get(i).and_then(|row| lane_row(row, sw.lane, base)) else {
            return;
        };
        let (a, z) = (sw.from.min(sw.to), sw.from.max(sw.to));
        let x0 = geo.x(ticks_to_beats(a));
        let x1 = geo.x(ticks_to_beats(z));
        let band = Rect::from_min_max(pos2(x0, r.top), pos2(x1.max(x0 + 1.0), r.top + r.height));
        b.rect(band, Fill::Solid(with_alpha(t.selection, 0.28)));
        b.rrect_stroke(band, 0.0, 1.5 * s.k, t.selection);
    }
}

/// The tick under a screen x, snapped to the arrangement's grid unless Ctrl is held.
fn tick_at(app: &CantusApp, geo: &TimelineGeometry, x: f32, free: bool) -> Ticks {
    let t = beats_to_ticks(geo.beats(x).max(0.0));
    if free || !app.arrangement.snap {
        t
    } else {
        t.snap(app.arrangement.grid)
    }
}

/// The take lanes' gestures: swipe to comp, click a take to use all of it, the menu.
pub(super) fn input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let base = app.arrangement.track_height;
    let mods = ui.input(|i| i.modifiers);
    let mut lanes: Vec<(TrackId, u8, Rect)> = Vec::new();
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(row) = geo.rows.get(i) else { continue };
        for lane in 1..=shown_lanes(track) {
            let Some(r) = lane_row(row, lane, base) else {
                break;
            };
            let band = Rect::from_min_max(
                pos2(rect.left(), r.top),
                pos2(rect.right(), r.top + r.height),
            )
            .intersect(rect);
            if band.height() > 1.0 {
                lanes.push((track.id, lane, band));
            }
        }
    }
    let _ = s;
    for (tid, lane, band) in lanes {
        let resp = ui.interact(
            band,
            ui.id().with(("take-lane", tid.0, lane)),
            Sense::click_and_drag(),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            hint::set_text(
                ui.ctx(),
                chrome::word("rec.lane_tip"),
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        if resp.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                let at = tick_at(app, geo, p.x, mods.command);
                ui.ctx().data_mut(|d| {
                    d.insert_temp(
                        swipe_id(),
                        Swipe {
                            track: tid,
                            lane,
                            from: at,
                            to: at,
                        },
                    )
                });
            }
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                let at = tick_at(app, geo, p.x, mods.command);
                ui.ctx().data_mut(|d| {
                    if let Some(mut sw) = d.get_temp::<Swipe>(swipe_id()) {
                        sw.to = at;
                        d.insert_temp(swipe_id(), sw);
                    }
                });
            }
        }
        if resp.drag_stopped() {
            let sw: Option<Swipe> = ui.ctx().data_mut(|d| {
                let v = d.get_temp::<Swipe>(swipe_id());
                d.remove::<Swipe>(swipe_id());
                v
            });
            if let Some(sw) = sw.filter(|sw| sw.track == tid && sw.lane == lane) {
                let (a, z) = (sw.from.min(sw.to), sw.from.max(sw.to));
                if z > a {
                    app.comp_swipe(tid, lane, a, z);
                }
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let at = beats_to_ticks(geo.beats(p.x).max(0.0));
                let hit = app
                    .project()
                    .track(tid)
                    .and_then(|t| {
                        t.clips
                            .iter()
                            .find(|c| c.lane == lane && c.start <= at && c.end() > at)
                    })
                    .map(|c| c.id);
                if let Some(clip) = hit {
                    app.promote_take_clip(tid, clip);
                }
            }
        }
        egui::Popup::context_menu(&resp)
            .frame(chrome::menu_frame())
            .show(|ui| lane_menu(app, ui, tid, lane));
    }
}

/// A take lane's menu (the lane on the timeline and its name in the header share it).
pub(super) fn lane_menu(app: &mut CantusApp, ui: &mut egui::Ui, tid: TrackId, lane: u8) {
    let items = [
        Item::action(Src::Key("rec.use_take")),
        Item::Separator,
        Item::action(Src::Key("rec.delete_unused")),
        Item::action(Src::Key("rec.explode")),
        Item::Separator,
        Item::action(Src::Key("rec.hide_takes")),
    ];
    match kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
        Some(0) => {
            app.promote_take_lane(tid, lane);
            ui.close();
        }
        Some(2) => {
            app.delete_unused_takes(tid);
            ui.close();
        }
        Some(3) => {
            app.explode_takes(tid);
            ui.close();
        }
        Some(5) => {
            app.toggle_takes(tid);
            ui.close();
        }
        _ => {}
    }
}

/// The lane's name: its first clip's, or "Take n" for a lane with none (rare: a gap a
/// deletion left before the lanes were closed up).
fn lane_name(track: &Track, lane: u8) -> Option<&str> {
    track
        .clips
        .iter()
        .find(|c| c.lane == lane)
        .map(|c| c.name.as_str())
}

/// The take lanes' headers under a track's: a flat face a step darker than the track's,
/// indented under its colour cell, the take's name, and a lamp lit while any of it is in
/// the comp.
pub(super) fn paint_headers(
    b: &mut Brush,
    app: &CantusApp,
    track: &Track,
    row: &TrackRow,
    clip: Rect,
    tint: Tint,
    s: &Style,
) {
    let base = app.arrangement.track_height;
    let t = &s.t;
    let k = s.k;
    for lane in 1..=shown_lanes(track) {
        let Some(r) = lane_row(row, lane, base) else {
            break;
        };
        if r.top > clip.bottom() || r.top + r.height < clip.top() {
            continue;
        }
        let rect = Rect::from_min_max(
            pos2(clip.left() + 1.0, r.top),
            pos2(clip.right() - 1.0, r.top + r.height - 1.0),
        );
        b.register(rect, "take_header", &[Tok::Panel, Tok::Selection], &[]);
        let face = mix(tint.large, t.panel, 0.55);
        b.rrect(
            b.snap_rect(rect),
            s.r(2.0),
            Fill::Solid(shade(face, if t.dark { -0.06 } else { -0.03 })),
        );
        // The track's colour as a rail down the indent, so the lanes read as the track's.
        let rail = Rect::from_min_size(
            pos2(rect.left() + 8.0 * k, rect.top()),
            vec2(3.0 * k, rect.height()),
        );
        b.rect(b.snap_rect(rail), Fill::Solid(with_alpha(tint.small, 0.8)));
        let comped = track.clips.iter().filter(|c| c.lane == lane).any(|c| {
            let mut any = false;
            comping::for_each_comped(track, c, |_, _| any = true);
            any
        });
        let lamp = pos2(rect.left() + 22.0 * k, rect.center().y);
        led::paint(b, lamp, 8.0 * k, comped, s, Tok::Selection, Tok::Panel, 1.0);
        let name_box = Rect::from_min_max(
            pos2(lamp.x + 9.0 * k, rect.top()),
            pos2(rect.right() - 6.0 * k, rect.bottom()),
        );
        let ink = with_alpha(tint.on_large, if comped { 1.0 } else { 0.72 });
        let tx = match lane_name(track, lane) {
            Some(name) => Text::new(name, Role::Label, ink),
            None => Text::key("rec.take", Role::Label, ink),
        }
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::Middle));
        let _ = text::paint(b, name_box, &tx, s.tk());
    }
}

/// A click on a lane's name in the header makes the whole lane the comp; right-click has
/// the lane's menu.
pub(super) fn header_input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    tid: TrackId,
    row: &TrackRow,
    clip: Rect,
    s: &Style,
) {
    let base = app.arrangement.track_height;
    let n = app.project().track(tid).map(shown_lanes).unwrap_or(0);
    let _ = s;
    for lane in 1..=n {
        let Some(r) = lane_row(row, lane, base) else {
            break;
        };
        let rect = Rect::from_min_max(
            pos2(clip.left(), r.top),
            pos2(clip.right(), r.top + r.height),
        )
        .intersect(clip);
        if rect.height() < 1.0 {
            continue;
        }
        let resp = ui.interact(
            rect,
            ui.id().with(("take-head", tid.0, lane)),
            Sense::click(),
        );
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                chrome::word("rec.lane_name_tip"),
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        if resp.clicked() {
            app.promote_take_lane(tid, lane);
        }
        egui::Popup::context_menu(&resp)
            .frame(chrome::menu_frame())
            .show(|ui| lane_menu(app, ui, tid, lane));
    }
}
