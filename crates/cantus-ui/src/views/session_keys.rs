//! The session's keys and edits (ADR 0077, keymap 14). The session's own actions work
//! wherever the keys are pressed (the cursor's slot and scene); while the pane has the
//! focus, the editing keys act on the grid instead of the arrangement — the arrows move
//! the cursor (Shift: a page), Del deletes, Ctrl+D duplicates, Ctrl+C / X / V copy, cut
//! and paste clips, F2 renames, Ctrl+Shift+C recolours, Ctrl+Shift+L loops.

use super::SessionCommand as S;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::keymap::Action;
use crate::views::workspace::Pane;
use cantus_core::session::{edit, SceneId, SlotRef, SlotRegion};
use cantus_core::{Color, Command, Ticks};

/// The session's actions, and the editing keys while the pane has the focus; `true` when
/// `a` was one of them.
pub fn action(app: &mut CantusApp, a: Action) -> bool {
    // The grid's own actions act where it is: with the pane holding the focus, or from a
    // controller (a bare Enter elsewhere — a dialog's, the arrangement's — is not the
    // session's).
    let here = app.session_view.focused || app.session_view.controller_fire;
    let from_key = !app.session_view.controller_fire;
    match a {
        Action::ToggleSessionView => toggle_view(app),
        // Sync's action (ADR 0078).
        Action::ToggleLink => crate::views::transport::sync_service::toggle_link(app),
        Action::SessionArrangementRecord => {
            let on = !app.session_view.live.arrangement_record;
            super::set_arrangement_record(app, on);
        }
        Action::SessionBackToArrangement => super::back_to_arrangement(app, None),
        Action::SessionLaunch
        | Action::SessionLaunchScene
        | Action::SessionStopTrack
        | Action::SessionStopAll
        | Action::SessionRecordSlot
        | Action::SessionCaptureScene
        | Action::SessionInsertScene
        | Action::SessionScenePrev
        | Action::SessionSceneNext
            if !here =>
        {
            return false;
        }
        Action::SessionLaunch => with_cursor(app, |app, s| {
            super::press_slot(app, s, 1.0);
            // Held by its key: the key's release lets it go (a gate clip stops).
            if from_key {
                app.session_view.key_held = Some(super::Pressed::Slot(s));
            }
        }),
        Action::SessionLaunchScene => with_cursor(app, |app, s| {
            super::launch_scene(app, s.scene, 1.0);
            if from_key {
                app.session_view.key_held = Some(super::Pressed::Scene(s.scene));
            }
        }),
        Action::SessionStopTrack => with_cursor(app, |app, s| super::stop_track(app, s.track)),
        Action::SessionStopAll => super::stop_all(app),
        Action::SessionRecordSlot => with_cursor(app, super::record),
        Action::SessionCaptureScene => capture_scene(app),
        Action::SessionInsertScene => {
            let at = cursor_or_first(app)
                .and_then(|s| app.project().session.scene_index(s.scene))
                .map_or(app.project().session.scenes.len(), |i| i + 1);
            insert_scene_at(app, at);
        }
        Action::SessionScenePrev => step(app, 0, -1),
        Action::SessionSceneNext => step(app, 0, 1),
        _ if !app.session_view.focused => return false,
        Action::NudgeLeft => step(app, -1, 0),
        Action::NudgeRight => step(app, 1, 0),
        Action::TransposeUp => step(app, 0, -1),
        Action::TransposeDown => step(app, 0, 1),
        Action::NudgeLeftBar => step(app, -8, 0),
        Action::NudgeRightBar => step(app, 8, 0),
        Action::TransposeOctaveUp => step(app, 0, -8),
        Action::TransposeOctaveDown => step(app, 0, 8),
        Action::Delete => delete(app),
        Action::Duplicate => duplicate(app),
        Action::Copy => copy(app),
        // What is cut is what is copied: the chosen scene's row, or the slot's clip.
        Action::Cut => {
            copy(app);
            delete(app);
        }
        Action::Paste => paste(app),
        Action::RenameClip => begin_rename(app),
        Action::ClipColorNext => cycle_colour(app),
        Action::ToggleClipLoop => toggle_loop(app),
        _ => return false,
    }
    true
}

/// Tab (Live's): the session and the arrangement take turns in the centre — the Session
/// pane comes forward, or goes back behind the arrangement; the selection stays.
pub fn toggle_view(app: &mut CantusApp) {
    let shown = app.workspace.is_visible(Pane::Session);
    if shown {
        app.show_pane(Pane::Arrangement);
        app.session_view.focused = false;
        // Hidden: nothing may land in its slots (an arrangement drag let go where they were).
        app.session_view.geo = None;
        app.focus_arrangement();
    } else {
        app.show_pane(Pane::Session);
        app.session_view.focused = true;
        if app.session_view.cursor.is_none() {
            app.session_view.cursor = first_slot(app);
        }
    }
    // A bare Tab also moves egui's focus between widgets: here it only switches views.
    if let Some(ctx) = app.theme_ctx.clone() {
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
    }
}

fn first_slot(app: &CantusApp) -> Option<SlotRef> {
    let t = app.project().tracks.first()?.id;
    let s = app.project().session.scenes.first()?.id;
    Some(SlotRef::new(t, s))
}

/// The cursor, or the first slot.
pub(super) fn cursor_or_first(app: &CantusApp) -> Option<SlotRef> {
    app.session_view
        .cursor
        .filter(|c| {
            app.project().track(c.track).is_some() && app.project().session.scene(c.scene).is_some()
        })
        .or_else(|| first_slot(app))
}

fn with_cursor(app: &mut CantusApp, f: impl FnOnce(&mut CantusApp, SlotRef)) {
    match cursor_or_first(app) {
        Some(s) => {
            app.session_view.cursor = Some(s);
            f(app, s)
        }
        None => app.set_status(t("session.status.no_slot")),
    }
}

/// Move the cursor `dx` columns and `dy` rows, kept in the grid and in view.
pub fn step(app: &mut CantusApp, dx: i32, dy: i32) {
    let tracks = super::track_ids(app);
    let scenes = super::scene_ids(app);
    if tracks.is_empty() || scenes.is_empty() {
        return;
    }
    let (i, j) = match cursor_or_first(app) {
        Some(c) => (
            tracks.iter().position(|t| *t == c.track).unwrap_or(0) as i32,
            scenes.iter().position(|s| *s == c.scene).unwrap_or(0) as i32,
        ),
        None => (0, 0),
    };
    let i = (i + dx).clamp(0, tracks.len() as i32 - 1) as usize;
    let j = (j + dy).clamp(0, scenes.len() as i32 - 1) as usize;
    let slot = SlotRef::new(tracks[i], scenes[j]);
    super::input::grid_choose(app, slot);
    app.session_view.scene_chosen = false;
    if let Some(g) = app.session_view.geo.clone() {
        app.session_view.scroll = g.scroll_to_show(i, j) / g.k;
    }
}

/// A one-bar MIDI clip in an empty slot (a double-click on it), chosen.
pub fn new_clip(app: &mut CantusApp, slot: SlotRef) {
    if app.project().slot_clip(slot).is_some() {
        return;
    }
    let bar = crate::views::bars::add_bars(app.project(), Ticks::ZERO, 1).max(Ticks(1));
    let name = app
        .project()
        .track(slot.track)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let clip = super::with_ids(app, |_, ids| edit::new_note_clip(&name, bar, ids));
    if super::exec(
        app,
        S::InsertSlotClip {
            slot,
            clip: Box::new(clip),
        },
    ) {
        super::input::grid_choose(app, slot);
    }
}

/// A new, empty scene at the bottom.
pub fn insert_scene(app: &mut CantusApp) {
    let n = app.project().session.scenes.len();
    insert_scene_at(app, n);
}

pub fn insert_scene_at(app: &mut CantusApp, index: usize) {
    let index = index.min(app.project().session.scenes.len());
    let cmd = super::with_ids(app, |_, ids| edit::insert_scene(index, "", ids));
    app.exec(cmd);
}

/// *Capture and insert scene*: what plays now, copied into a new scene under the cursor's.
pub fn capture_scene(app: &mut CantusApp) {
    let playing = app.session_view.live.playing();
    if playing.is_empty() {
        app.set_status(t("session.status.nothing_playing"));
        return;
    }
    let at = cursor_or_first(app)
        .and_then(|s| app.project().session.scene_index(s.scene))
        .map_or(app.project().session.scenes.len(), |i| i + 1);
    let cmd = super::with_ids(app, |p, ids| edit::capture_scene(p, &playing, at, "", ids));
    if let Some(c) = cmd {
        app.exec(c);
    }
}

/// Delete what is chosen: the scene (its row) or the slot's clip.
pub fn delete(app: &mut CantusApp) {
    let Some(c) = cursor_or_first(app) else {
        return;
    };
    if app.session_view.scene_chosen {
        delete_scene(app, c.scene);
    } else if app.project().slot_clip(c).is_some() {
        super::exec(app, S::RemoveSlotClip { slot: c });
    }
}

pub fn delete_scene(app: &mut CantusApp, scene: SceneId) {
    super::exec(app, S::RemoveScene { id: scene });
}

/// Duplicate what is chosen: a scene under itself, a clip into the next free slot below
/// (a new scene when there is none).
pub fn duplicate(app: &mut CantusApp) {
    let Some(c) = cursor_or_first(app) else {
        return;
    };
    if app.session_view.scene_chosen {
        duplicate_scene(app, c.scene);
        return;
    }
    if app.project().slot_clip(c).is_none() {
        return;
    }
    let scenes = super::scene_ids(app);
    let j = scenes.iter().position(|s| *s == c.scene).unwrap_or(0);
    let track = app.project().track(c.track).cloned();
    let free = scenes[j + 1..]
        .iter()
        .find(|s| track.as_ref().is_some_and(|t| t.slot_clip(**s).is_none()))
        .copied();
    let to = match free {
        Some(s) => SlotRef::new(c.track, s),
        // No free slot below: a new scene at the bottom holding the copy — one step.
        None => {
            let n = scenes.len();
            let cmd = super::with_ids(app, |p, ids| {
                let src = p.slot_clip(c)?;
                let scene = cantus_core::session::Scene::new(SceneId(ids()), "");
                let id = scene.id;
                let slot = cantus_core::session::Slot {
                    scene: id,
                    clip: Some(edit::fresh(src, ids)),
                    stop: true,
                };
                Some((
                    id,
                    Command::Batch {
                        label_key: "cmd.session.duplicate_clip".into(),
                        commands: vec![Command::Session(S::InsertScene {
                            index: n,
                            scene: Box::new(scene),
                            slots: vec![(c.track, slot)],
                        })],
                    },
                ))
            });
            if let Some((id, cmd)) = cmd {
                if app.exec(cmd) {
                    super::input::grid_choose(app, SlotRef::new(c.track, id));
                }
            }
            return;
        }
    };
    let cmd = super::with_ids(app, |p, ids| edit::duplicate_slot(p, c, to, ids));
    if let Some(cmd) = cmd {
        if app.exec(cmd) {
            super::input::grid_choose(app, to);
        }
    }
}

pub fn duplicate_scene(app: &mut CantusApp, scene: SceneId) {
    let cmd = super::with_ids(app, |p, ids| edit::duplicate_scene(p, scene, ids));
    if let Some(cmd) = cmd {
        app.exec(cmd);
    }
}

/// Copy what is chosen: the cursor's clip, or — the scene chosen — its row's clips.
pub fn copy(app: &mut CantusApp) {
    let Some(c) = cursor_or_first(app) else {
        return;
    };
    let tracks = super::track_ids(app);
    let i0 = tracks.iter().position(|t| *t == c.track).unwrap_or(0) as i32;
    let board: Vec<super::Copied> = if app.session_view.scene_chosen {
        tracks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let clip = app.project().track(*t)?.slot_clip(c.scene)?.clone();
                Some(super::Copied {
                    dx: i as i32 - i0,
                    dy: 0,
                    clip,
                    from: *t,
                })
            })
            .collect()
    } else {
        app.project()
            .slot_clip(c)
            .cloned()
            .map(|clip| super::Copied {
                dx: 0,
                dy: 0,
                clip,
                from: c.track,
            })
            .into_iter()
            .collect()
    };
    if board.is_empty() {
        app.set_status(t("clipboard.status.empty"));
        return;
    }
    app.session_view.clipboard = board;
    app.session_view.clipboard_scene = if app.session_view.scene_chosen {
        app.project().session.scene(c.scene).cloned()
    } else {
        None
    };
    app.set_status(t("session.status.copied"));
}

/// Paste what was copied at the cursor's slot (each copy in its own slot, empty ones only;
/// its envelopes following it to its new track).
pub fn paste(app: &mut CantusApp) {
    let Some(c) = cursor_or_first(app) else {
        return;
    };
    if app.session_view.clipboard.is_empty() {
        app.set_status(t("clipboard.status.empty"));
        return;
    }
    let tracks = super::track_ids(app);
    let scenes = super::scene_ids(app);
    let (Some(i), Some(j)) = (
        tracks.iter().position(|t| *t == c.track),
        scenes.iter().position(|s| *s == c.scene),
    ) else {
        return;
    };
    let board = app.session_view.clipboard.clone();
    // A scene's row pasted with a scene chosen: the scene comes back whole — its name,
    // colour, tempo, metre and follow actions, its row — under the cursor's, one step.
    if let (true, Some(scene)) = (
        app.session_view.scene_chosen,
        app.session_view.clipboard_scene.clone(),
    ) {
        let cmd = super::with_ids(app, |p, ids| {
            let mut scene = scene.clone();
            scene.id = SceneId(ids());
            let mut slots = Vec::new();
            for item in &board {
                // Back on the track it came from; a track since deleted: by offset.
                let x = i as i32 + item.dx;
                let Some(tr) = tracks
                    .iter()
                    .find(|t| **t == item.from)
                    .or_else(|| tracks.get(x.max(0) as usize).filter(|_| x >= 0))
                else {
                    continue;
                };
                if slots.iter().any(|(t, _)| t == tr) {
                    continue;
                }
                let mut fresh = edit::fresh(&item.clip, ids);
                cantus_core::session::envelopes_for(p, &mut fresh, item.from, *tr);
                slots.push((
                    *tr,
                    cantus_core::session::Slot {
                        scene: scene.id,
                        clip: Some(fresh),
                        stop: true,
                    },
                ));
            }
            Command::Batch {
                label_key: "cmd.session.paste".into(),
                commands: vec![Command::Session(S::InsertScene {
                    index: j + 1,
                    scene: Box::new(scene),
                    slots,
                })],
            }
        });
        app.exec(cmd);
        return;
    }
    let mut commands = Vec::new();
    for item in board {
        let (x, y) = (i as i32 + item.dx, j as i32 + item.dy);
        let (Some(tr), Some(sc)) = (
            tracks.get(x.max(0) as usize).filter(|_| x >= 0),
            scenes.get(y.max(0) as usize).filter(|_| y >= 0),
        ) else {
            continue;
        };
        let slot = SlotRef::new(*tr, *sc);
        if app.project().slot_clip(slot).is_some() {
            app.set_status(t("session.status.slot_taken"));
            continue;
        }
        let mut fresh = super::with_ids(app, |_, ids| edit::fresh(&item.clip, ids));
        // The session's one rule for a clip copied to another track (`envelopes_for`).
        cantus_core::session::envelopes_for(app.project(), &mut fresh, item.from, *tr);
        commands.push(Command::Session(S::InsertSlotClip {
            slot,
            clip: Box::new(fresh),
        }));
    }
    if !commands.is_empty() {
        app.exec(Command::Batch {
            label_key: "cmd.session.paste".into(),
            commands,
        });
    }
}

/// F2: the chosen scene's or clip's name typed in place.
pub fn begin_rename(app: &mut CantusApp) {
    let Some(c) = cursor_or_first(app) else {
        return;
    };
    if app.session_view.scene_chosen {
        let name = app
            .project()
            .session
            .scene(c.scene)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        app.session_view.rename = Some(super::Rename::Scene(c.scene, name));
    } else if let Some(clip) = app.project().slot_clip(c) {
        let name = clip.clip.name.clone();
        app.session_view.rename = Some(super::Rename::Clip(c, name));
    }
}

fn next_colour(current: Option<Color>) -> Option<Color> {
    match current {
        None => Some(Color::PALETTE[0]),
        Some(c) => Color::PALETTE
            .iter()
            .position(|p| *p == c)
            .and_then(|i| Color::PALETTE.get(i + 1).copied()),
    }
}

/// The chosen clip's (or scene's) colour, the next of the palette (then its track's).
pub fn cycle_colour(app: &mut CantusApp) {
    let Some(c) = cursor_or_first(app) else {
        return;
    };
    if app.session_view.scene_chosen {
        cycle_scene_colour(app, c.scene);
        return;
    }
    let Some(clip) = app.project().slot_clip(c) else {
        return;
    };
    let (id, color) = (clip.id(), next_colour(clip.clip.color));
    app.exec(Command::SetClipColor {
        track: c.track,
        clip: id,
        color,
    });
}

pub fn cycle_scene_colour(app: &mut CantusApp, scene: SceneId) {
    let current = app.project().session.scene(scene).and_then(|s| s.color);
    super::exec(
        app,
        S::SetSceneColor {
            id: scene,
            color: next_colour(current),
        },
    );
}

/// The chosen clip's loop on or off (its end marker kept as the loop's end).
pub fn toggle_loop(app: &mut CantusApp) {
    let Some(c) = cursor_or_first(app) else {
        return;
    };
    let Some(clip) = app.project().slot_clip(c) else {
        return;
    };
    let mut region = SlotRegion::of(clip);
    region.loop_length = match region.loop_length {
        Some(_) => None,
        None => Some(region.length),
    };
    if region
        .start
        .is_some_and(|s| s >= region.offset + region.loop_length.unwrap_or(region.length))
    {
        region.start = None;
    }
    super::exec(app, S::SetSlotRegion { slot: c, region });
}

/// A slot's clip into the arrangement at `at` (the bar at the playhead when `None`).
pub fn to_arrangement(app: &mut CantusApp, slot: SlotRef, at: Option<Ticks>) {
    let at = at.unwrap_or_else(|| {
        let now = super::now(app);
        crate::views::bars::map(app.project()).bar_start(now)
    });
    let cmd = super::with_ids(app, |p, ids| {
        edit::slot_to_arrangement(p, slot, at, None, ids)
    });
    if let Some(cmd) = cmd {
        app.exec(cmd);
    }
}

/// Whether a track's column has two note clips to consolidate.
pub fn can_consolidate(app: &CantusApp, track: cantus_core::TrackId) -> bool {
    app.project().track(track).is_some_and(|t| {
        t.session_clips().filter(|c| c.clip.audio.is_none()).count() >= 2
            && t.session_clips().all(|c| c.clip.audio.is_none())
    })
}

/// A column's clips joined into one looping clip in its first slot.
pub fn consolidate(app: &mut CantusApp, track: cantus_core::TrackId) {
    let scenes: Vec<SceneId> = super::scene_ids(app)
        .into_iter()
        .filter(|s| {
            app.project()
                .track(track)
                .is_some_and(|t| t.slot_clip(*s).is_some())
        })
        .collect();
    let cmd = super::with_ids(app, |p, ids| {
        edit::consolidate_column(p, track, &scenes, ids)
    });
    if let Some(cmd) = cmd {
        app.exec(cmd);
    }
}
