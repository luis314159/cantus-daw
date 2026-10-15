//! A comp on the playing lane (ADR 0058), drawn and edited where it plays, for a track
//! whose take lanes are shown:
//!
//! - **Seams**: each seam a handle at the foot of the playing lane, its crossfade drawn as
//!   an X over the stretch it crosses. Drag the handle (or a window's edge there) and both
//!   windows move — one undo step; right-click for its crossfade (off, 5, 10, 20, 50 ms, or
//!   that one for every seam of the track).
//! - **Which take**: each comp window wears its take lane's number, and the take lanes
//!   light the ranges in the comp (`arrangement_takes`).
//! - **Comps**: the first take lane's header holds the comp's chip — *Comp A ▾* — and its
//!   menu: the comps (switch), *New comp* (a copy, played), *Rename…*, delete one, flatten,
//!   flatten and merge, quick swipe, grouped comping, every seam's crossfade.

use super::chrome;
use super::{beats_to_ticks, ticks_to_beats, TimelineGeometry, TrackRow};
use crate::app::CantusApp;
use crate::comping;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Policy, Role, Src, Text};
use crate::kit::{self, Brush, Fill, Style};
use cantus_core::{ClipId, Ticks, Track, TrackId};
use egui::{pos2, vec2, Align2, Id, Rect, Sense};

/// The crossfades a menu offers, in milliseconds (0 is off).
pub const XFADES_MS: [f32; 5] = [0.0, 5.0, 10.0, 20.0, 50.0];

/// A seam handle's rect: at the foot of the playing lane, centred on the seam.
fn handle(geo: &TimelineGeometry, row: &TrackRow, at: Ticks, k: f32) -> Rect {
    let x = geo.x(ticks_to_beats(at));
    Rect::from_center_size(
        pos2(x, row.top + row.height - 6.0 * k),
        vec2(12.0 * k, 11.0 * k),
    )
}

/// Ticks for `ms` at the project's tempo.
pub fn ticks_of_ms(app: &CantusApp, ms: f32) -> Ticks {
    let bpm = app.project().tempo_bpm;
    Ticks((f64::from(ms) / 1000.0 * bpm / 60.0 * cantus_core::PPQ as f64).round() as i64)
}

/// The comp drawn over the playing lane: the seams' crossfades and handles, and each comp
/// window's take number.
pub(super) fn paint(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, rect: Rect, s: &Style) {
    let t = &s.t;
    let k = s.k;
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(row) = geo.rows.get(i) else { continue };
        if super::takes::shown_lanes(track) == 0
            || row.top > rect.bottom()
            || row.top + row.height < rect.top()
        {
            continue;
        }
        // Each window's take, by its lane's number.
        for w in comping::comp_windows(track) {
            let Some(lane) = take_lane_of(track, w) else {
                continue;
            };
            let x0 = geo.x(ticks_to_beats(w.start)).max(rect.left());
            let x1 = geo.x(ticks_to_beats(w.end())).min(rect.right());
            if x1 - x0 < 22.0 * k {
                continue;
            }
            // At the body's foot, clear of the title, the clip's words and the seams'
            // handles.
            let chip = Rect::from_min_size(
                pos2(x0 + 11.0 * k, row.top + row.height - 16.0 * k),
                vec2(16.0 * k, 12.0 * k),
            );
            if chip.top() < row.top + 18.0 * k {
                continue;
            }
            b.rrect(chip, s.r(2.0), Fill::Solid(with_alpha(t.selection, 0.85)));
            let n = lane.to_string();
            let tx = Text::new(&n, Role::Micro, crate::kit::color::on_fill(t.selection))
                .align(Align2::CENTER_CENTER);
            let _ = text::paint(b, chip, &tx, s.tk());
        }
        for (_, _, at, x) in comping::seams(track) {
            let h = handle(geo, row, at, k);
            if h.right() < rect.left() || h.left() > rect.right() {
                continue;
            }
            if x > Ticks::ZERO {
                let half = Ticks(x.0 / 2);
                let x0 = geo.x(ticks_to_beats(at - half));
                let x1 = geo.x(ticks_to_beats(at + (x - half))).max(x0 + 2.0 * k);
                let y1 = row.top + row.height - 2.0 * k;
                let y0 = (y1 - (row.height * 0.45).min(20.0 * k)).max(row.top + 2.0 * k);
                let ink = with_alpha(t.text, 0.75);
                b.segment(pos2(x0, y1), pos2(x1, y0), 1.2 * k, ink, true);
                b.segment(pos2(x0, y0), pos2(x1, y1), 1.2 * k, ink, true);
            }
            let c = h.center();
            let d = 4.5 * k;
            b.polygon(
                &[
                    pos2(c.x, c.y - d),
                    pos2(c.x + d, c.y),
                    pos2(c.x, c.y + d),
                    pos2(c.x - d, c.y),
                ],
                t.selection,
            );
        }
    }
}

/// The take lane whose take a comp window shows.
fn take_lane_of(track: &Track, w: &cantus_core::Clip) -> Option<u8> {
    let key = comping::content_key(w)?;
    track
        .clips
        .iter()
        .filter(|c| c.lane > 0)
        .find(|c| comping::content_key(c) == Some(key))
        .map(|c| c.lane)
}

/// A seam being dragged: the track, the two windows.
#[derive(Copy, Clone, Debug, PartialEq)]
struct SeamDrag {
    track: TrackId,
    left: ClipId,
    right: ClipId,
}

/// The seams' handles: drag to move a seam (both sides), right-click for its crossfade.
pub(super) fn input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let k = s.k;
    let mut handles: Vec<(TrackId, ClipId, ClipId, Rect)> = Vec::new();
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(row) = geo.rows.get(i) else { continue };
        if super::takes::shown_lanes(track) == 0 {
            continue;
        }
        for (l, r, at, _) in comping::seams(track) {
            let h = handle(geo, row, at, k).expand(2.0 * k);
            if h.intersects(rect) {
                handles.push((track.id, l, r, h.intersect(rect)));
            }
        }
    }
    let snap = app.arrangement.snap;
    let grid = app.arrangement.grid;
    for (tid, l, r, h) in handles {
        let resp = ui.interact(
            h,
            ui.id().with(("comp-seam", tid.0, l.0, r.0)),
            Sense::click_and_drag(),
        );
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            hint::set_text(
                ui.ctx(),
                chrome::word("comping.seam"),
                chrome::word("comping.seam_tip"),
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let free = ui.input(|i| i.modifiers.command);
                let mut at = beats_to_ticks(geo.beats(p.x).max(0.0));
                if snap && !free {
                    at = at.snap(grid);
                }
                ui.ctx().data_mut(|d| {
                    d.insert_temp(
                        Id::new("comp-seam-drag"),
                        SeamDrag {
                            track: tid,
                            left: l,
                            right: r,
                        },
                    )
                });
                app.move_comp_seam(tid, l, r, at, true);
            }
        }
        if resp.drag_stopped() {
            ui.ctx()
                .data_mut(|d| d.remove::<SeamDrag>(Id::new("comp-seam-drag")));
            app.end_gesture();
        }
        egui::Popup::context_menu(&resp)
            .frame(chrome::menu_frame())
            .show(|ui| seam_menu(app, ui, tid, r));
    }
}

/// A seam's menu: its crossfade, or that one for every seam of the track.
fn seam_menu(app: &mut CantusApp, ui: &mut egui::Ui, tid: TrackId, right: ClipId) {
    let now = app
        .project()
        .track(tid)
        .and_then(|t| t.clip(right))
        .and_then(|c| c.audio.as_ref())
        .map_or(Ticks::ZERO, |a| a.xfade);
    let labels: Vec<String> = XFADES_MS.iter().map(|ms| xfade_word(*ms)).collect();
    let mut items = vec![Item::Band(Src::Key("comping.menu.xfade"))];
    for (i, ms) in XFADES_MS.iter().enumerate() {
        let on = ticks_of_ms(app, *ms) == now;
        items.push(Item::Action {
            label: Src::Str(&labels[i]),
            check: Check::Radio(on),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    items.push(Item::Separator);
    items.push(Item::action(Src::Key("comping.xfade_all")));
    match kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
        Some(i) if (1..=XFADES_MS.len()).contains(&i) => {
            let x = ticks_of_ms(app, XFADES_MS[i - 1]);
            app.set_seam_xfade(tid, Some(right), x);
            ui.close();
        }
        Some(i) if i == XFADES_MS.len() + 2 => {
            app.set_seam_xfade(tid, None, now);
            ui.close();
        }
        _ => {}
    }
}

/// "Off", "10 ms".
pub fn xfade_word(ms: f32) -> String {
    if ms <= 0.0 {
        crate::i18n::t("comping.xfade_off")
    } else {
        format!("{ms:.0} ms")
    }
}

/// The comp's chip in the first take lane's header: *Comp A ▾*.
pub(super) fn chip_rect(header: Rect, k: f32) -> Rect {
    let w = (64.0 * k).min(header.width() * 0.45);
    Rect::from_min_max(
        pos2(header.right() - w - 4.0 * k, header.top() + 2.0 * k),
        pos2(header.right() - 4.0 * k, header.bottom() - 2.0 * k),
    )
}

/// Paint the chip: the playing comp's name.
pub(super) fn paint_chip(b: &mut Brush, track: &Track, r: Rect, s: &Style) {
    let t = &s.t;
    let name = track
        .comps
        .get(track.active_comp as usize)
        .map_or_else(|| cantus_core::comp::letter(0), |c| c.name.clone());
    let word = crate::i18n::tf("comping.chip", &[("name", &name)]);
    b.rrect(r, s.r(2.0), Fill::Solid(with_alpha(t.selection, 0.18)));
    b.rrect_stroke(
        r.shrink(b.px() * 0.5),
        s.r(2.0),
        b.px(),
        with_alpha(t.selection, 0.7),
    );
    let tx = Text::new(&word, Role::Label, t.text)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Abbreviate);
    let _ = text::paint(b, r.shrink2(vec2(4.0 * s.k, 0.0)), &tx, s.tk());
}

/// The chip's click opens the comp menu.
pub(super) fn chip_input(app: &mut CantusApp, ui: &mut egui::Ui, tid: TrackId, r: Rect) {
    let resp = ui.interact(r, ui.id().with(("comp-chip", tid.0)), Sense::click());
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            chrome::word("comping.chip_tip"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    egui::Popup::menu(&resp)
        .frame(chrome::menu_frame())
        .show(|ui| comp_menu(app, ui, tid));
    rename_field(app, ui, tid, r);
}

/// What a row of the comp menu does.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    Switch(u8),
    New,
    Rename,
    Delete(u8),
    Flatten,
    Merge,
    QuickSwipe,
    Grouped,
    Xfade(f32),
    Nothing,
}

/// The comp menu (the chip's, and the take lanes' menus' *Comp* part).
pub fn comp_menu(app: &mut CantusApp, ui: &mut egui::Ui, tid: TrackId) {
    let Some(track) = app.project().track(tid) else {
        return;
    };
    let rs = app.project().recording.clone();
    let names: Vec<String> = if track.comps.is_empty() {
        vec![cantus_core::comp::letter(0)]
    } else {
        track.comps.iter().map(|c| c.name.clone()).collect()
    };
    let active = track.active_comp;
    let deletes: Vec<(u8, String)> = names
        .iter()
        .enumerate()
        .filter(|(i, _)| *i as u8 != active)
        .map(|(i, n)| (i as u8, crate::i18n::tf("comping.delete", &[("name", n)])))
        .collect();
    let xf: Vec<String> = XFADES_MS.iter().map(|ms| xfade_word(*ms)).collect();
    let mut rows: Vec<(Row, Item<'_>)> =
        vec![(Row::Nothing, Item::Band(Src::Key("comping.menu.comps")))];
    for (i, n) in names.iter().enumerate() {
        rows.push((
            Row::Switch(i as u8),
            Item::Action {
                label: Src::Str(n),
                check: Check::Radio(i as u8 == active),
                shortcut: None,
                submenu: false,
                enabled: true,
            },
        ));
    }
    rows.push((Row::Nothing, Item::Separator));
    rows.push((
        Row::New,
        Item::Action {
            label: Src::Key("comping.new"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            // At the most a track holds, nothing to make (review 1).
            enabled: names.len() < cantus_core::comp::MAX_COMPS,
        },
    ));
    rows.push((Row::Rename, Item::action(Src::Key("comping.rename"))));
    for (i, w) in &deletes {
        rows.push((Row::Delete(*i), Item::action(Src::Str(w))));
    }
    rows.push((Row::Nothing, Item::Separator));
    rows.push((Row::Flatten, Item::action(Src::Key("comping.flatten"))));
    rows.push((
        Row::Merge,
        Item::Action {
            label: Src::Key("comping.merge"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: !app.merging(),
        },
    ));
    rows.push((Row::Nothing, Item::Separator));
    for (row, key, on) in [
        (Row::QuickSwipe, "comping.quick_swipe", rs.quick_swipe),
        (Row::Grouped, "comping.grouped", rs.comp_grouped),
    ] {
        rows.push((
            row,
            Item::Action {
                label: Src::Key(key),
                check: if on { Check::On } else { Check::Off },
                shortcut: None,
                submenu: false,
                enabled: true,
            },
        ));
    }
    rows.push((Row::Nothing, Item::Band(Src::Key("comping.menu.xfade_all"))));
    for (i, ms) in XFADES_MS.iter().enumerate() {
        rows.push((
            Row::Xfade(*ms),
            Item::Action {
                label: Src::Str(&xf[i]),
                check: Check::Radio(rs.comp_xfade_ms == *ms),
                shortcut: None,
                submenu: false,
                enabled: true,
            },
        ));
    }
    let items: Vec<Item<'_>> = rows.iter().map(|r| r.1).collect();
    // Many comps make a long menu: it scrolls rather than leave the screen (review 1).
    let room = (ui.ctx().content_rect().height() - ui.spacing().interact_size.y * 4.0).max(160.0);
    let clicks = egui::ScrollArea::vertical()
        .id_salt(("comp-menu", tid.0))
        .max_height(room)
        .show(ui, |ui| kit::widgets::menu_list(ui, &items, 240.0).1)
        .inner;
    let Some(row) = clicks
        .clicked
        .or(clicks.toggled)
        .and_then(|i| rows.get(i))
        .map(|r| r.0.clone())
    else {
        return;
    };
    match row {
        Row::Switch(i) => app.switch_comp(tid, i),
        Row::New => app.duplicate_comp(tid),
        Row::Rename => {
            let name = names.get(active as usize).cloned().unwrap_or_default();
            ui.ctx()
                .data_mut(|d| d.insert_temp(rename_id(tid), (active, name)));
        }
        Row::Delete(i) => app.delete_comp(tid, i),
        Row::Flatten => app.flatten_comp(tid),
        Row::Merge => {
            app.merge_comp(tid);
        }
        Row::QuickSwipe => app.toggle_quick_swipe(),
        Row::Grouped => app.toggle_comp_grouped(),
        Row::Xfade(ms) => app.set_comp_xfade_default(tid, ms),
        Row::Nothing => return,
    }
    ui.close();
}

fn rename_id(tid: TrackId) -> Id {
    Id::new(("comp-rename", tid.0))
}

/// While a comp is being renamed: the kit's field over the chip; Enter renames, Esc leaves.
fn rename_field(app: &mut CantusApp, ui: &mut egui::Ui, tid: TrackId, r: Rect) {
    let Some((which, mut words)) = ui
        .ctx()
        .data(|d| d.get_temp::<(u8, String)>(rename_id(tid)))
    else {
        return;
    };
    let k = Style::current().k;
    let mut event = None;
    egui::Area::new(rename_id(tid).with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            let (line, _) =
                ui.allocate_exact_size(vec2(r.width().max(96.0 * k), r.height()), Sense::hover());
            let out = crate::kit::field::Field::new(rename_id(tid).with("field"))
                .scale(k)
                .opens_when_shown()
                .show(ui, line, &mut words);
            event = out.event;
        });
    match event {
        Some(crate::kit::field::Event::Commit) => {
            ui.ctx()
                .data_mut(|d| d.remove::<(u8, String)>(rename_id(tid)));
            app.rename_comp(tid, which, &words);
        }
        Some(crate::kit::field::Event::Cancel) => {
            ui.ctx()
                .data_mut(|d| d.remove::<(u8, String)>(rename_id(tid)));
        }
        None => {
            ui.ctx()
                .data_mut(|d| d.insert_temp(rename_id(tid), (which, words)));
        }
    }
}
