//! r39:el-arrange — the header column's input: each track's header (select, reorder,
//! resize, rename, the keys, "more", its menu, its own icon), each folder's row, the
//! add-track row.

use super::super::head::{self, Key, KEYS};
use super::super::{
    autolanes, chrome, folders, menus, takes, Frame, TimelineGeometry, DIVIDER_GRIP,
};
use super::{add_rect, row_rect, shows};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::Style;
use cantus_core::{Command, MergeKey, TrackId, MAX_TRACK_HEIGHT, MIN_TRACK_HEIGHT};
use egui::{pos2, PointerButton, Rect, Sense};

pub(super) fn input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    f: &Frame,
    geo: &TimelineGeometry,
    s: &Style,
    n: usize,
) {
    let clip = f.headers;
    let base = app.arrangement.track_height;
    let mut to_delete = None;
    let mut open_name = None;
    folder_rows(app, ui, geo, clip, s);
    for (i, row) in geo.rows.iter().enumerate() {
        if !shows(row, clip) {
            continue;
        }
        let Some((id, muted, soloed, height, has_glyph, in_folder)) =
            app.project().tracks.get(i).map(|t| {
                (
                    t.id,
                    t.muted,
                    t.soloed,
                    t.height(),
                    t.glyph.is_some(),
                    t.folder.is_some(),
                )
            })
        else {
            continue;
        };
        let rr = row_rect(clip, row);
        let r = head::rects(rr, usize::from(in_folder), n, has_glyph, s);
        // A row owns the divider below it, not the one above (a press on the line resizes
        // the row whose height it changes).
        let hit = Rect::from_min_max(
            pos2(rr.left(), row.top + DIVIDER_GRIP),
            pos2(rr.right(), row.top + row.height + DIVIDER_GRIP),
        )
        .intersect(clip);
        let resp = ui.interact(
            hit,
            ui.id().with(("arr-header", id.0)),
            Sense::click_and_drag(),
        );
        // The keys, "more", the icon slot: widgets of their own over the header.
        let mut keys = [None, None, None, None];
        for (j, key) in KEYS.iter().enumerate().take(r.n) {
            let name = match key {
                Key::Mute => "arr-mute",
                Key::Solo => "arr-solo",
                Key::Arm => "arr-arm",
                Key::Auto => "arr-auto",
            };
            keys[j] = Some(ui.interact(
                r.keys[j].intersect(clip),
                ui.id().with((name, id.0)),
                Sense::click(),
            ));
        }
        let more = resp
            .hovered()
            .then(|| ui.interact(r.more, ui.id().with(("arr-more", id.0)), Sense::click()));
        // The track's own icon, when it has one: a click changes it (the menu's *Icon…*
        // gives one to a track that has none).
        let slot = has_glyph.then(|| {
            ui.interact(
                r.glyph.expand(s.s(2.0)).intersect(clip),
                ui.id().with(("arr-glyph", id.0)),
                Sense::click(),
            )
        });
        // The name's letters rename on a click (round 27, ADR 0051).
        let letters = app
            .arrangement
            .hits
            .iter()
            .find(|h| h.track == id)
            .and_then(|h| h.letters);
        let name = letters.map(|l| {
            ui.interact(
                l.intersect(clip),
                ui.id().with(("arr-name", id.0)),
                Sense::click_and_drag(),
            )
        });
        let on_name = name.as_ref().is_some_and(|n| n.hovered());
        if (resp.hovered() || on_name)
            && app.arrangement.drag.is_none()
            && app.arrangement.reorder.is_none()
        {
            let tip = if on_name {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                "track.header.name_tip"
            } else {
                "track.header.tip"
            };
            if let Some(t) = app.project().track(id) {
                let caps = Caps {
                    menu: true,
                    typed: on_name,
                    ..Caps::NONE
                };
                hint::set_text(ui.ctx(), &t.name, chrome::word(tip), caps);
            }
        }
        // The keys.
        let mut open_auto = false;
        for (j, key) in KEYS.iter().enumerate().take(r.n) {
            let Some(k) = keys[j].as_ref() else {
                continue;
            };
            let tip = match key {
                Key::Mute => "track.mute_tip",
                Key::Solo => "track.solo_tip",
                Key::Arm => "rec.arm_tip",
                Key::Auto => "arrange.key.auto_tip",
            };
            if k.hovered() {
                hint::set_text(ui.ctx(), chrome::word(tip), "", Caps::NONE);
            }
            let target = match key {
                Key::Mute => Some(cantus_core::controller::MappingTarget::TrackMute(id)),
                Key::Solo => Some(cantus_core::controller::MappingTarget::TrackSolo(id)),
                Key::Arm => Some(cantus_core::controller::MappingTarget::TrackArm(id)),
                Key::Auto => None,
            };
            if let Some(t) = target {
                crate::controllers::learn_spot_in(app, ui, k.rect, clip, t);
            }
            if k.clicked() {
                match key {
                    Key::Mute => {
                        app.exec(Command::SetTrackMute { id, muted: !muted });
                    }
                    Key::Solo => {
                        app.exec(Command::SetTrackSolo {
                            id,
                            soloed: !soloed,
                        });
                    }
                    Key::Arm => {
                        let only = ui.input(|i| i.modifiers.shift);
                        app.toggle_arm(id, only);
                    }
                    Key::Auto => {
                        let open = app.project().track(id).is_some_and(|t| t.automation_open);
                        let lanes = app.project().track(id).map_or(0, autolanes::lane_count);
                        if lanes > 0 {
                            app.exec(Command::SetTrackAutomationOpen { id, open: !open });
                        } else {
                            open_auto = true;
                        }
                    }
                }
            }
            if *key == Key::Auto {
                // With no lane yet, a click offers what to automate (its right-click too).
                let popup = egui::Popup::context_menu(k);
                let popup = if open_auto {
                    popup.open_memory(Some(egui::containers::SetOpenCommand::Bool(true)))
                } else {
                    popup
                };
                popup.frame(chrome::menu_frame()).show(|ui| {
                    if autolanes::track_menu(app, ui, id) {
                        ui.close();
                    }
                });
            }
        }
        if slot.as_ref().is_some_and(|r| r.hovered()) {
            hint::set_text(ui.ctx(), chrome::word("arrange.glyph.tip"), "", Caps::NONE);
        }
        if slot.as_ref().is_some_and(|r| r.clicked()) {
            app.select_track(id);
            app.arrangement
                .glyphs
                .open_for(id, r.glyph.left_bottom(), ui.ctx());
        }
        rename_in_place(app, ui, id, r.name);
        // The type icon is the instrument chip: a click swaps the instrument (round 20).
        if r.line2.is_some() {
            super::super::super::device_chrome::chip_input(app, ui, r.kind, id, s.k);
        }
        // The monitor, the input (while armed), the takes' disclosure (ADR 0025).
        if let Some(t) = app.project().track(id) {
            let rr = super::super::rec::layout(&r, t, s);
            super::super::rec::header_input(app, ui, id, &rr, s);
        }
        // A device dragged from the browser swaps the instrument; a track template lands
        // after the track (round 31, ADR 0072).
        let template = |p: Option<std::sync::Arc<crate::picker::DevicePayload>>| {
            p.is_some_and(|p| matches!(p.0, crate::picker::PickChoice::TrackTemplate(_)))
        };
        let swap = if template(resp.dnd_hover_payload()) || template(resp.dnd_release_payload()) {
            crate::picker::PickPurpose::AddInstrument { index: Some(i + 1) }
        } else {
            crate::picker::PickPurpose::ReplaceInstrument { track: id }
        };
        super::super::super::device_chrome::drop_target(app, ui, &resp, swap);
        takes::header_input(app, ui, id, row, clip, s);
        autolanes::header_input(app, ui, i, id, row, clip);

        let grip = Rect::from_min_max(
            pos2(rr.left(), row.top + row.height - DIVIDER_GRIP),
            pos2(rr.right(), row.top + row.height + DIVIDER_GRIP),
        );
        let resizing = app.arrangement.divider.map(|(d, _)| d) == Some(id);
        if resp.hover_pos().is_some_and(|p| grip.contains(p)) || resizing {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            let px = (row.height / s.k).round();
            let tip = crate::i18n::tf("arrange.height.tip", &[("h", &px)]);
            hint::set_text(
                ui.ctx(),
                &tip,
                chrome::word("arrange.height.alt"),
                Caps::NONE,
            );
        }
        if resp.clicked() {
            app.select_track(id);
        }
        if name.as_ref().is_some_and(|n| n.clicked()) {
            app.select_track(id);
            open_name = Some(id);
        }
        if resp.double_clicked() {
            app.open_track_instrument(id);
        }
        let from_name = name.as_ref().filter(|n| {
            n.drag_started_by(PointerButton::Primary)
                || n.dragged_by(PointerButton::Primary)
                || n.drag_stopped()
        });
        let d = from_name.unwrap_or(&resp);
        if d.drag_started_by(PointerButton::Primary) {
            let origin = ui.input(|i| i.pointer.press_origin());
            if from_name.is_none() && origin.is_some_and(|p| grip.contains(p)) {
                app.arrangement.divider = Some((id, height));
            } else {
                app.select_track(id);
                app.arrangement.reorder = Some((id, i));
                egui::DragAndDrop::set_payload(ui.ctx(), crate::routing_actions::TrackDrag(id));
            }
        }
        if d.dragged_by(PointerButton::Primary) {
            if let Some(pos) = d.interact_pointer_pos() {
                if let Some((did, start_h)) = app.arrangement.divider {
                    if did == id {
                        let origin = ui
                            .input(|i| i.pointer.press_origin())
                            .map(|p| p.y)
                            .unwrap_or(pos.y);
                        let height = (start_h + (pos.y - origin) / base)
                            .clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT);
                        // Alt: every track at once (one merged step).
                        if ui.input(|i| i.modifiers.alt) {
                            app.set_all_track_heights(height);
                        } else {
                            app.exec_merging(
                                Command::SetTrackHeight { id, height },
                                MergeKey::of(&("height", id.0)),
                            );
                        }
                    }
                } else if app.arrangement.reorder.map(|(d, _)| d) == Some(id) {
                    let home = f.headers.union(f.lanes);
                    let to = if home.contains(pos) {
                        drop_index(app, geo, pos.y)
                    } else {
                        i
                    };
                    app.arrangement.reorder = Some((id, to));
                }
            }
        }
        if d.drag_stopped() {
            app.arrangement.divider = None;
            if let Some((did, to)) = app.arrangement.reorder.take() {
                let home = f.headers.union(f.lanes);
                let inside = ui
                    .input(|i| i.pointer.latest_pos())
                    .is_none_or(|p| home.contains(p));
                if did == id && to != i && inside {
                    move_track(app, did, to);
                }
            }
            app.end_gesture();
        }
        // r39:ux-b — Shift+F10 on the selected track opens this menu (ADR 0110).
        let by_key = crate::menu_keys::take_track(ui.ctx(), id);
        let by_more = more.as_ref().is_some_and(|m| m.clicked());
        if more.as_ref().is_some_and(|m| m.hovered()) {
            hint::set_text(ui.ctx(), chrome::word("arrange.more_tip"), "", Caps::NONE);
        }
        if by_more {
            app.arrangement.menu_page = Some((id, menus::Page::Main));
        }
        for (k, r) in std::iter::once(&resp).chain(name.as_ref()).enumerate() {
            let popup = egui::Popup::context_menu(r);
            let popup = if (by_key || by_more) && k == 0 {
                popup.open_memory(Some(egui::containers::SetOpenCommand::Bool(true)))
            } else {
                popup
            };
            popup.frame(chrome::menu_frame()).show(|ui| {
                // r39:el-controls — inside the window: a long menu scrolls.
                crate::kit::menu::fit(ui, |ui| menus::track(app, ui, id, &mut to_delete));
            });
        }
    }
    if let Some(id) = open_name {
        match app.arrangement.rename.take() {
            Some((rid, text)) if rid != id => super::super::commit_track_name(app, rid, &text),
            same => app.arrangement.rename = same,
        }
        if app.arrangement.rename.is_none() {
            if let Some(name) = app.project().track(id).map(|t| t.name.clone()) {
                app.arrangement.rename = Some((id, name));
            }
        }
    }
    if let Some(id) = to_delete {
        app.delete_track(id);
    }
    add_row(app, ui, geo, clip, s);
}

/// Where a header dropped at `y` lands: the index of the row it falls before.
fn drop_index(app: &CantusApp, geo: &TimelineGeometry, y: f32) -> usize {
    let n = app.project().tracks.len();
    geo.rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.height > 0.0 && y < r.top + r.height * 0.5)
        .min_by(|a, b| a.1.top.total_cmp(&b.1.top))
        .map_or(n.saturating_sub(1), |(i, _)| i)
}

/// Move a track to `to`: landing between two members of a folder puts it in that folder,
/// anywhere else takes it out of any (one step).
fn move_track(app: &mut CantusApp, id: TrackId, to: usize) {
    let p = app.project();
    let rest: Vec<Option<cantus_core::FolderId>> = p
        .tracks
        .iter()
        .filter(|t| t.id != id)
        .map(|t| t.folder)
        .collect();
    let above = to
        .checked_sub(1)
        .and_then(|k| rest.get(k).copied())
        .flatten();
    let below = rest.get(to).copied().flatten();
    let folder = above.filter(|_| above == below);
    let now = p.track(id).and_then(|t| t.folder);
    let mut commands = vec![Command::MoveTrack { id, index: to }];
    if folder != now {
        commands.push(Command::Arrange(
            cantus_core::ArrangeCommand::SetTrackFolder { track: id, folder },
        ));
    }
    if commands.len() == 1 {
        app.exec(commands.remove(0));
    } else {
        app.exec(Command::Batch {
            label_key: "cmd.move_track".into(),
            commands,
        });
    }
}

/// The name editor over a header's name, while that track is renamed.
fn rename_in_place(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, at: Rect) {
    let Some((rid, name)) = app.arrangement.rename.as_mut().filter(|(r, _)| *r == id) else {
        return;
    };
    let rid = *rid;
    // Its words as large as the line allows (a short row's field stays inside its row).
    let s = Style::current();
    let font = crate::kit::text::Role::Body.font(s.tk());
    let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font)).max(1.0);
    let fit = (at.height() / row_h).min(1.0);
    let out = crate::kit::field::Field::new(egui::Id::new(("track-rename", rid.0)))
        .scale(fit)
        .opens_when_shown()
        .show(ui, at, name);
    if out.event == Some(crate::kit::field::Event::Cancel) {
        app.arrangement.rename = None;
        return;
    }
    if out.event == Some(crate::kit::field::Event::Commit) {
        if let Some((_, text)) = app.arrangement.rename.take() {
            super::super::commit_track_name(app, rid, &text);
        }
    }
}

/// The add-track row: a click opens its menu.
fn add_row(app: &mut CantusApp, ui: &mut egui::Ui, geo: &TimelineGeometry, clip: Rect, s: &Style) {
    let r = add_rect(geo, clip, s).intersect(clip);
    if r.height() <= 0.0 {
        return;
    }
    let resp = ui.interact(r, ui.id().with("arr-add-row"), Sense::click());
    super::super::super::device_chrome::publish(
        ui.ctx(),
        super::super::super::device_chrome::Spot::AddTrack,
        r,
    );
    if resp.hovered() {
        hint::set_text(ui.ctx(), chrome::word("swap.add_track_tip"), "", Caps::NONE);
    }
    egui::Popup::menu(&resp)
        .frame(chrome::menu_frame())
        .show(|ui| menus::add_track(app, ui, None));
}

/// Each folder's row: the chevron opens and closes, M S act on its members, a click selects
/// it, a double-click on its name renames, its menu renames, recolours and ungroups.
fn folder_rows(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    geo: &TimelineGeometry,
    clip: Rect,
    s: &Style,
) {
    for fr in &geo.folders {
        let row = Rect::from_min_size(
            pos2(clip.left(), fr.top),
            egui::vec2(clip.width(), fr.height),
        );
        let hit = row.intersect(clip);
        if hit.height() <= 0.0 {
            continue;
        }
        let r = head::rects(row, 0, 2, false, s);
        let resp = ui.interact(hit, ui.id().with(("arr-folder", fr.id.0)), Sense::click());
        let chevron = ui.interact(
            r.number.expand(s.s(4.0)).intersect(clip),
            ui.id().with(("arr-folder-open", fr.id.0)),
            Sense::click(),
        );
        let m = ui.interact(
            r.keys[0],
            ui.id().with(("arr-folder-m", fr.id.0)),
            Sense::click(),
        );
        let so = ui.interact(
            r.keys[1],
            ui.id().with(("arr-folder-s", fr.id.0)),
            Sense::click(),
        );
        let Some(folder) = app.project().folder(fr.id).cloned() else {
            continue;
        };
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                &folder.name,
                chrome::word("arrange.folder.tip"),
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        for (r, key) in [
            (&chevron, "arrange.folder.chevron_tip"),
            (&m, "arrange.folder.mute_tip"),
            (&so, "arrange.folder.solo_tip"),
        ] {
            if r.hovered() {
                hint::set_text(ui.ctx(), chrome::word(key), "", Caps::NONE);
            }
        }
        if chevron.clicked() {
            folders::set_open(app, fr.id, !folder.open);
        } else if resp.double_clicked() {
            app.arrangement.folder_rename = Some((fr.id, folder.name.clone()));
        } else if resp.clicked() {
            app.arrangement.folder_selected = Some(fr.id);
        }
        if m.clicked() {
            let on = !folders::all_members(app.project(), fr.id, |t| t.muted);
            folders::set_members(app, fr.id, true, on);
        }
        if so.clicked() {
            let on = !folders::all_members(app.project(), fr.id, |t| t.soloed);
            folders::set_members(app, fr.id, false, on);
        }
        // Rename in place.
        if let Some((rid, text)) = app
            .arrangement
            .folder_rename
            .as_mut()
            .filter(|(i, _)| *i == fr.id)
        {
            let rid = *rid;
            let out = crate::kit::field::Field::new(egui::Id::new(("folder-rename", rid.0)))
                .opens_when_shown()
                .show(ui, r.name, text);
            match out.event {
                Some(crate::kit::field::Event::Commit) => {
                    if let Some((_, text)) = app.arrangement.folder_rename.take() {
                        let name = text.trim();
                        if !name.is_empty() && name != folder.name {
                            let mut f2 = folder.clone();
                            f2.name = name.to_string();
                            app.exec(Command::Arrange(cantus_core::ArrangeCommand::SetFolder {
                                folder: f2,
                            }));
                        }
                    }
                }
                Some(crate::kit::field::Event::Cancel) => app.arrangement.folder_rename = None,
                None => {}
            }
        }
        egui::Popup::context_menu(&resp)
            .frame(chrome::menu_frame())
            .show(|ui| {
                use crate::kit::menu::Item;
                use crate::kit::text::Src;
                let items = [
                    Item::action(Src::Key("arrange.menu.rename")),
                    Item::action(Src::Key(if folder.open {
                        "arrange.folder.close"
                    } else {
                        "arrange.folder.open"
                    })),
                    Item::Separator,
                    Item::action(Src::Key("arrange.folder.ungroup")),
                ];
                match crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
                    Some(0) => {
                        app.arrangement.folder_rename = Some((fr.id, folder.name.clone()));
                        ui.close();
                    }
                    Some(1) => {
                        folders::set_open(app, fr.id, !folder.open);
                        ui.close();
                    }
                    Some(3) => {
                        folders::ungroup(app, fr.id);
                        ui.close();
                    }
                    _ => {}
                }
            });
    }
}
