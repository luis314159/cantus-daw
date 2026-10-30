//! The grid's right-click menus (ADR 0077): a clip's, an empty slot's, a scene's — every
//! row with its key, every edit one undo step (the same functions the keys run, `keys`).

use super::keys;
use super::layout::Hit;
use crate::app::CantusApp;
use crate::keymap::Action;
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use cantus_core::controller::MappingTarget;
use cantus_core::session::{SceneId, SlotRef};

/// A row with its label key and the shortcut of `action`.
fn row<'a>(
    app: &CantusApp,
    key: &'a str,
    action: Option<Action>,
    shortcuts: &mut Vec<String>,
    check: Check,
    enabled: bool,
) -> (Item<'a>, Option<usize>) {
    let sc = action.and_then(|a| app.keymap.bindings_for(a).first().map(|b| b.display()));
    let idx = sc.map(|s| {
        shortcuts.push(s);
        shortcuts.len() - 1
    });
    (
        Item::Action {
            label: Src::Key(key),
            check,
            shortcut: None,
            submenu: false,
            enabled,
        },
        idx,
    )
}

/// Build the items with their shortcuts filled in (the strings outlive the items).
fn items<'a>(rows: &'a [(Item<'a>, Option<usize>)], shortcuts: &'a [String]) -> Vec<Item<'a>> {
    rows.iter()
        .map(|(it, sc)| match (*it, sc) {
            (
                Item::Action {
                    label,
                    check,
                    submenu,
                    enabled,
                    ..
                },
                Some(i),
            ) => Item::Action {
                label,
                check,
                shortcut: Some(shortcuts[*i].as_str()),
                submenu,
                enabled,
            },
            (it, _) => it,
        })
        .collect()
}

/// The menu of what was right-clicked; `true` when a row was chosen.
pub(super) fn grid_menu(app: &mut CantusApp, ui: &mut egui::Ui, hit: Hit) -> bool {
    match hit {
        Hit::Slot(slot) if app.project().slot_clip(slot).is_some() => clip_menu(app, ui, slot),
        Hit::Slot(slot) => empty_menu(app, ui, slot),
        Hit::Scene(scene) => scene_menu(app, ui, scene),
        Hit::NewScene => {
            let it = [Item::action(Src::Key("session.menu.new_scene"))];
            if crate::kit::widgets::menu_list(ui, &it, 220.0)
                .1
                .clicked
                .is_some()
            {
                keys::insert_scene(app);
                return true;
            }
            false
        }
        _ => false,
    }
}

fn clip_menu(app: &mut CantusApp, ui: &mut egui::Ui, slot: SlotRef) -> bool {
    let looping = app.project().slot_clip(slot).is_some_and(|c| c.looping());
    let mut sc = Vec::new();
    let rows = [
        row(
            app,
            "session.menu.launch",
            Some(Action::SessionLaunch),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.stop_track",
            Some(Action::SessionStopTrack),
            &mut sc,
            Check::None,
            true,
        ),
        (Item::Separator, None),
        row(
            app,
            "session.menu.rename",
            Some(Action::RenameClip),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.colour",
            Some(Action::ClipColorNext),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.loop",
            Some(Action::ToggleClipLoop),
            &mut sc,
            if looping { Check::On } else { Check::Off },
            true,
        ),
        (Item::Separator, None),
        row(
            app,
            "session.menu.duplicate",
            Some(Action::Duplicate),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.copy",
            Some(Action::Copy),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.delete",
            Some(Action::Delete),
            &mut sc,
            Check::None,
            true,
        ),
        (Item::Separator, None),
        row(
            app,
            "session.menu.to_arrangement",
            None,
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.consolidate",
            None,
            &mut sc,
            Check::None,
            keys::can_consolidate(app, slot.track),
        ),
        row(app, "session.menu.learn", None, &mut sc, Check::None, true),
    ];
    let it = items(&rows, &sc);
    let Some(i) = crate::kit::widgets::menu_list(ui, &it, 240.0).1.clicked else {
        return false;
    };
    match i {
        0 => super::launch(app, slot, 1.0),
        1 => super::stop_track(app, slot.track),
        3 => keys::begin_rename(app),
        4 => keys::cycle_colour(app),
        5 => keys::toggle_loop(app),
        7 => keys::duplicate(app),
        8 => keys::copy(app),
        9 => keys::delete(app),
        11 => keys::to_arrangement(app, slot, None),
        12 => keys::consolidate(app, slot.track),
        13 => app.learn_target(MappingTarget::SessionSlot {
            track: slot.track,
            scene: slot.scene,
        }),
        _ => return false,
    }
    true
}

fn empty_menu(app: &mut CantusApp, ui: &mut egui::Ui, slot: SlotRef) -> bool {
    let armed = app.project().track(slot.track).is_some_and(|t| t.armed);
    let stop = app
        .project()
        .track(slot.track)
        .and_then(|t| t.slot(slot.scene))
        .is_none_or(|s| s.stop);
    let paste = !app.session_view.clipboard.is_empty();
    let mut sc = Vec::new();
    let rows = [
        row(
            app,
            "session.menu.new_clip",
            None,
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.record",
            Some(Action::SessionRecordSlot),
            &mut sc,
            Check::None,
            armed,
        ),
        row(
            app,
            "session.menu.paste",
            Some(Action::Paste),
            &mut sc,
            Check::None,
            paste,
        ),
        (Item::Separator, None),
        row(
            app,
            "session.menu.stop_button",
            None,
            &mut sc,
            if stop { Check::On } else { Check::Off },
            true,
        ),
        row(app, "session.menu.learn", None, &mut sc, Check::None, true),
    ];
    let it = items(&rows, &sc);
    let Some(i) = crate::kit::widgets::menu_list(ui, &it, 240.0).1.clicked else {
        return false;
    };
    match i {
        0 => keys::new_clip(app, slot),
        1 => super::record(app, slot),
        2 => keys::paste(app),
        4 => {
            super::exec(
                app,
                cantus_core::session::SessionCommand::SetSlotStop { slot, stop: !stop },
            );
        }
        5 => app.learn_target(MappingTarget::SessionSlot {
            track: slot.track,
            scene: slot.scene,
        }),
        _ => return false,
    }
    true
}

fn scene_menu(app: &mut CantusApp, ui: &mut egui::Ui, scene: SceneId) -> bool {
    let Some(sc_now) = app.project().session.scene(scene).cloned() else {
        return false;
    };
    let index = app.project().session.scene_index(scene).unwrap_or(0);
    let last = app.project().session.scenes.len().saturating_sub(1);
    let mut sc = Vec::new();
    let rows = [
        row(
            app,
            "session.menu.launch_scene",
            Some(Action::SessionLaunchScene),
            &mut sc,
            Check::None,
            true,
        ),
        (Item::Separator, None),
        row(
            app,
            "session.menu.rename",
            Some(Action::RenameClip),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.colour",
            Some(Action::ClipColorNext),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.scene_tempo",
            None,
            &mut sc,
            if sc_now.tempo.is_some() {
                Check::On
            } else {
                Check::Off
            },
            true,
        ),
        row(
            app,
            "session.menu.scene_metre",
            None,
            &mut sc,
            if sc_now.metre.is_some() {
                Check::On
            } else {
                Check::Off
            },
            true,
        ),
        (Item::Separator, None),
        row(
            app,
            "session.menu.insert_scene",
            Some(Action::SessionInsertScene),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.capture",
            Some(Action::SessionCaptureScene),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.duplicate",
            Some(Action::Duplicate),
            &mut sc,
            Check::None,
            true,
        ),
        row(
            app,
            "session.menu.move_up",
            None,
            &mut sc,
            Check::None,
            index > 0,
        ),
        row(
            app,
            "session.menu.move_down",
            None,
            &mut sc,
            Check::None,
            index < last,
        ),
        row(
            app,
            "session.menu.delete",
            Some(Action::Delete),
            &mut sc,
            Check::None,
            true,
        ),
        (Item::Separator, None),
        row(app, "session.menu.learn", None, &mut sc, Check::None, true),
    ];
    let it = items(&rows, &sc);
    let Some(i) = crate::kit::widgets::menu_list(ui, &it, 260.0).1.clicked else {
        return false;
    };
    use cantus_core::session::SessionCommand as S;
    match i {
        0 => super::launch_scene(app, scene, 1.0),
        2 => app.session_view.rename = Some(super::Rename::Scene(scene, sc_now.name.clone())),
        3 => keys::cycle_scene_colour(app, scene),
        4 => {
            let tempo = match sc_now.tempo {
                Some(_) => None,
                None => {
                    Some(crate::views::bars::map(app.project()).bpm_at(super::now(app).0 as f64))
                }
            };
            super::exec(app, S::SetSceneTempo { id: scene, tempo });
        }
        5 => {
            let metre = match sc_now.metre {
                Some(_) => None,
                None => Some(crate::views::bars::map(app.project()).signature_at(super::now(app))),
            };
            super::exec(app, S::SetSceneMetre { id: scene, metre });
        }
        7 => keys::insert_scene_at(app, index + 1),
        8 => keys::capture_scene(app),
        9 => keys::duplicate_scene(app, scene),
        10 => {
            super::exec(
                app,
                S::MoveScene {
                    id: scene,
                    to: index - 1,
                },
            );
        }
        11 => {
            super::exec(
                app,
                S::MoveScene {
                    id: scene,
                    to: index + 1,
                },
            );
        }
        12 => keys::delete_scene(app, scene),
        14 => app.learn_target(MappingTarget::SessionScene(scene)),
        _ => return false,
    }
    true
}
