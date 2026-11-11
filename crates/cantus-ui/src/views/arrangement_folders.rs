//! r39:el-arrange — folders of tracks in the arrangement (ADR 0113): a folder's own row
//! above its members, open (its members under it, indented a step and a step back) or
//! closed (one 44-px row with its summary, its members hidden). Its lane shows the group's
//! span as one quiet body with each member's clips as bars in its colour. Its M and S are
//! its members' own (one batch, one undo step).
//!
//! The rows are laid out here, for the headers and the lanes alike: each track in the
//! song's order, a folder's row where its first member comes, every other member gathered
//! under it, a closed folder's members at no height, and a folder with no member at the end.

use super::head::{FOLDER_OPEN, ROW};
use super::TrackRow;
use crate::app::CantusApp;
use cantus_core::arrange::{ArrangeCommand, FolderId, TrackFolder};
use cantus_core::{Command, Project, TrackId};

/// Where a folder's own row was drawn.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FolderRow {
    pub id: FolderId,
    pub top: f32,
    pub height: f32,
}

/// The rows of `project` from `top` at base row `base`: each track's (by index; a closed
/// folder's members at no height), each folder's, and where the add-track row goes.
pub fn rows_into(
    rows: &mut Vec<TrackRow>,
    folders: &mut Vec<FolderRow>,
    project: &Project,
    top: f32,
    base: f32,
) -> f32 {
    let n = project.tracks.len();
    // Not yet placed: a height below zero (a hidden member has 0).
    let unplaced = TrackRow {
        top,
        height: -1.0,
        takes: 0.0,
        auto: 0.0,
        pitch: 0.0,
    };
    rows.clear();
    rows.resize(n, unplaced);
    folders.clear();
    let k = base / ROW;
    let mut y = top;
    let place = |rows: &mut Vec<TrackRow>, i: usize, y: &mut f32| {
        let t = &project.tracks[i];
        let height = (base * t.height()).round();
        let takes = super::takes::height(t, base);
        let auto = super::autolanes::height(t, base);
        let pitch = super::super::pitch_lane::height(t, base);
        rows[i] = TrackRow {
            top: *y,
            height,
            takes,
            auto,
            pitch,
        };
        *y += height + takes + auto + pitch;
    };
    for i in 0..n {
        if rows[i].height >= 0.0 {
            continue;
        }
        let folder = project.tracks[i]
            .folder
            .and_then(|f| project.folder(f))
            .filter(|f| !folders.iter().any(|r| r.id == f.id));
        let Some(f) = folder else {
            place(rows, i, &mut y);
            continue;
        };
        let h = if f.open {
            (FOLDER_OPEN * k).round()
        } else {
            base
        };
        folders.push(FolderRow {
            id: f.id,
            top: y,
            height: h,
        });
        y += h;
        for j in i..n {
            if project.tracks[j].folder != Some(f.id) || rows[j].height >= 0.0 {
                continue;
            }
            if f.open {
                place(rows, j, &mut y);
            } else {
                rows[j] = TrackRow {
                    top: y,
                    height: 0.0,
                    takes: 0.0,
                    auto: 0.0,
                    pitch: 0.0,
                };
            }
        }
    }
    // Folders with no member, at the end.
    for f in &project.folders {
        if !folders.iter().any(|r| r.id == f.id) {
            let h = if f.open {
                (FOLDER_OPEN * k).round()
            } else {
                base
            };
            folders.push(FolderRow {
                id: f.id,
                top: y,
                height: h,
            });
            y += h;
        }
    }
    y
}

/// The folder a track is drawn in: its depth and the folder's colour.
pub fn of_track(p: &Project, i: usize) -> Option<&TrackFolder> {
    p.tracks.get(i)?.folder.and_then(|f| p.folder(f))
}

/// Whether every member of `f` is muted (soloed): its M (S) is lit.
pub fn all_members(p: &Project, f: FolderId, what: fn(&cantus_core::Track) -> bool) -> bool {
    let mut any = false;
    for t in p.folder_members(f) {
        any = true;
        if !what(t) {
            return false;
        }
    }
    any
}

/// Mute or solo every member of `f` (one step).
pub fn set_members(app: &mut CantusApp, f: FolderId, mute: bool, on: bool) {
    let commands: Vec<Command> = app
        .project()
        .folder_members(f)
        .filter(|t| if mute { t.muted != on } else { t.soloed != on })
        .map(|t| {
            if mute {
                Command::SetTrackMute {
                    id: t.id,
                    muted: on,
                }
            } else {
                Command::SetTrackSolo {
                    id: t.id,
                    soloed: on,
                }
            }
        })
        .collect();
    if commands.is_empty() {
        return;
    }
    app.exec(Command::Batch {
        label_key: if mute {
            "cmd.set_track_mute".into()
        } else {
            "cmd.set_track_solo".into()
        },
        commands,
    });
}

/// Open or close a folder.
pub fn set_open(app: &mut CantusApp, f: FolderId, open: bool) {
    if let Some(mut folder) = app.project().folder(f).cloned() {
        if folder.open != open {
            folder.open = open;
            app.exec(Command::Arrange(ArrangeCommand::SetFolder { folder }));
        }
    }
}

/// Put `tracks` in a new folder named `name`, coloured as the first one, gathered where the
/// first one is (one step). Returns the folder.
pub fn new_folder(app: &mut CantusApp, tracks: &[TrackId], name: &str) -> Option<FolderId> {
    let p = app.project();
    let first = tracks.first().and_then(|t| p.track(*t));
    let color = first.map_or(cantus_core::Color::PALETTE[0], |t| t.color);
    let at = first.and_then(|t| p.track_index(t.id));
    let id = FolderId(app.bus.alloc_id());
    let folder = TrackFolder {
        id,
        name: name.into(),
        color,
        open: true,
    };
    let mut commands = vec![Command::Arrange(ArrangeCommand::InsertFolder {
        index: app.project().folders.len(),
        folder,
    })];
    // The members gathered under the first, in their order.
    if let Some(mut to) = at {
        for t in tracks.iter().skip(1) {
            let Some(from) = app.project().track_index(*t) else {
                continue;
            };
            let index = if from > to { to + 1 } else { to };
            commands.push(Command::MoveTrack { id: *t, index });
            to = index;
        }
    }
    for t in tracks {
        commands.push(Command::Arrange(ArrangeCommand::SetTrackFolder {
            track: *t,
            folder: Some(id),
        }));
    }
    app.exec(Command::Batch {
        label_key: "cmd.arrange.add_folder".into(),
        commands,
    });
    app.project().folder(id).map(|f| f.id)
}

/// Take every member out of `f` and remove it (one step).
pub fn ungroup(app: &mut CantusApp, f: FolderId) {
    let mut commands: Vec<Command> = app
        .project()
        .folder_members(f)
        .map(|t| {
            Command::Arrange(ArrangeCommand::SetTrackFolder {
                track: t.id,
                folder: None,
            })
        })
        .collect();
    commands.push(Command::Arrange(ArrangeCommand::RemoveFolder { id: f }));
    app.exec(Command::Batch {
        label_key: "cmd.arrange.remove_folder".into(),
        commands,
    });
}

/// The rows' whole height at base row `base`, as [`rows_into`] lays them out (without
/// laying them out: nothing allocated).
pub fn content_height(project: &Project, base: f32) -> f32 {
    let k = base / ROW;
    let folder_h = |f: &TrackFolder| {
        if f.open {
            (FOLDER_OPEN * k).round()
        } else {
            base
        }
    };
    let tracks: f32 = project
        .tracks
        .iter()
        .filter(|t| {
            t.folder
                .and_then(|f| project.folder(f))
                .is_none_or(|f| f.open)
        })
        .map(|t| {
            (base * t.height()).round()
                + super::takes::height(t, base)
                + super::autolanes::height(t, base)
                + super::super::pitch_lane::height(t, base)
        })
        .sum();
    tracks + project.folders.iter().map(folder_h).sum::<f32>()
}
