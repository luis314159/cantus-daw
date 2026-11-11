//! r39:el-arrange — the header column: every row's header painted (tracks, folders, their
//! lanes, the add-track row), then its input — select, reorder, resize, rename, the keys,
//! the menus, the track's own icon. The layout is `arrangement_head.rs`'s.

use super::head::{self, Look, Width, KEYS};
use super::{
    autolanes, folders, takes, Frame, HeaderHit, TimelineGeometry, TrackRow, DIVIDER_GRIP,
};
use crate::app::CantusApp;
use crate::kit::{self, Style};
use egui::{pos2, vec2, Rect};

/// The whole extent of a row: the track and its lanes under it.
fn extent(r: &TrackRow) -> f32 {
    r.height + r.takes + r.auto + r.pitch
}

/// Whether row `r` shows in `clip`.
pub(super) fn shows(r: &TrackRow, clip: Rect) -> bool {
    r.height > 0.0 && r.top + extent(r) > clip.top() && r.top < clip.bottom()
}

/// The header of a track row.
pub(super) fn row_rect(clip: Rect, r: &TrackRow) -> Rect {
    Rect::from_min_size(pos2(clip.left(), r.top), vec2(clip.width(), r.height))
}

/// The keys a header carries at this width (a folder's: M S).
fn key_count(w: Width) -> usize {
    head::keys_of(w)
}

/// Paint the column, then handle its input.
pub(super) fn headers(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    f: &Frame,
    geo: &TimelineGeometry,
    s: &Style,
) {
    let clip = f.headers;
    let p = ui.painter().with_clip_rect(clip);
    let pointer = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|_| ui.rect_contains_pointer(clip));
    let width = Width::of(clip.width(), s.k);
    let n = key_count(width);
    let mut words = std::mem::take(&mut app.arrangement.words);
    words.refresh(app);
    let mut hits = std::mem::take(&mut app.arrangement.hits);
    hits.clear();
    let mut scratch = std::mem::take(&mut app.arrangement.scratch);
    {
        let a: &CantusApp = app;
        let v = &a.arrangement;
        let project = a.project();
        let any_solo = project.tracks.iter().any(|t| t.soloed);
        let meters = a.engine.meters();
        kit::with_brush(&p, |b| {
            b.register(clip, "headers", head::TOKENS, &[]);
            let ink = super::slate::Ink::of(s);
            b.rect(b.snap_rect(clip), kit::Fill::Solid(ink.base));
            // Folders' rows.
            for fr in &geo.folders {
                let row =
                    Rect::from_min_size(pos2(clip.left(), fr.top), vec2(clip.width(), fr.height));
                if row.bottom() < clip.top() || row.top() > clip.bottom() {
                    continue;
                }
                let Some(fo) = project.folder(fr.id) else {
                    continue;
                };
                let fi = project
                    .folders
                    .iter()
                    .position(|x| x.id == fr.id)
                    .unwrap_or(0);
                let hovered = pointer.is_some_and(|q| row.contains(q));
                let look = head::FolderLook {
                    open: fo.open,
                    selected: v.folder_selected == Some(fr.id),
                    hovered,
                    hot: None,
                    muted: folders::all_members(project, fr.id, |t| t.muted),
                    soloed: folders::all_members(project, fr.id, |t| t.soloed),
                    renaming: v.folder_rename.as_ref().is_some_and(|(id, _)| *id == fr.id),
                };
                let line2 = words.folder.get(fi).map_or("", String::as_str);
                let hue = crate::theme::track_color(fo.color);
                head::folder(b, row, &fo.name, hue, line2, look, s);
            }
            // Tracks.
            for (i, row) in geo.rows.iter().enumerate() {
                let Some(track) = project.tracks.get(i).filter(|_| shows(row, clip)) else {
                    continue;
                };
                let rr = row_rect(clip, row);
                let r = head::rects(
                    rr,
                    usize::from(track.folder.is_some()),
                    n,
                    track.glyph.is_some(),
                    s,
                );
                let hovered = pointer.is_some_and(|q| rr.contains(q)) && v.drag.is_none();
                let hot = pointer.and_then(|q| {
                    KEYS.iter()
                        .take(r.n)
                        .enumerate()
                        .find(|(j, _)| r.keys[*j].contains(q))
                        .map(|(_, k)| *k)
                });
                let level = if i < meters.track_count {
                    ((cantus_core::linear_to_db(meters.tracks[i]) + 60.0) / 60.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let grip = Rect::from_min_max(
                    pos2(rr.left(), rr.bottom() - DIVIDER_GRIP),
                    pos2(rr.right(), rr.bottom() + DIVIDER_GRIP),
                );
                let folder =
                    folders::of_track(project, i).map(|fo| crate::theme::track_color(fo.color));
                let look = Look {
                    selected: a.selected_track == Some(track.id),
                    hovered,
                    hot,
                    silenced: any_solo && !track.soloed,
                    renaming: v.rename.as_ref().is_some_and(|(r, _)| *r == track.id),
                    drop_above: v.reorder.is_some_and(|(d, to)| d != track.id && to == i),
                    divider: v.divider.is_some_and(|(d, _)| d == track.id)
                        || (v.drag.is_none() && pointer.is_some_and(|q| grip.contains(q))),
                    level,
                    depth: usize::from(folder.is_some()),
                    folder,
                    lanes: autolanes::lane_count(track) > 0,
                    lanes_open: track.automation_open,
                    input: if track.armed { level } else { 0.0 },
                };
                let number = scratch.digits.fmt(i as i64 + 1);
                let letters = head::track(
                    b,
                    &r,
                    track,
                    number,
                    words.line2.get(i).map_or("", String::as_str),
                    words.level.get(i).map_or("", String::as_str),
                    words.inserts.get(i).map_or("", String::as_str),
                    &words.frozen,
                    look,
                    s,
                );
                hits.push(HeaderHit {
                    track: track.id,
                    row: rr,
                    letters,
                    keys: r.keys,
                    glyph: r.glyph,
                });
                let rr = super::rec::layout(&r, track, s);
                super::rec::paint_header(b, track, &rr, pointer, s);
                let tint = scratch.tints.of(crate::theme::track_color(track.color), s);
                takes::paint_headers(b, a, track, row, clip, tint, s);
                autolanes::paint_headers(b, a, i, track, row, clip, tint, s);
                super::super::pitch_lane::paint_header(b, row, clip, tint.large, s);
            }
            // The add-track row under the last row.
            let add = add_rect(geo, clip, s);
            if add.top() < clip.bottom() {
                let hover = pointer.is_some_and(|q| add.contains(q));
                head::add_row(b, add, &words.add, "", hover, s);
            }
            // A new song: the column says so under the add-track row.
            if project.tracks.is_empty() && project.folders.is_empty() {
                let r = Rect::from_min_size(
                    pos2(clip.left() + s.s(32.0), add.bottom() + s.s(12.0)),
                    vec2(clip.width() - s.s(40.0), s.s(18.0)),
                );
                let ink = super::slate::Ink::of(s);
                let _ = super::slate::say(
                    b,
                    r,
                    &words.no_tracks,
                    super::slate::W::Regular,
                    11.0,
                    ink.ink3,
                    egui::Align2::LEFT_CENTER,
                    s,
                );
            }
        });
        // The icons over the mesh: the type icon on line 2, the user's own before the name.
        for (i, row) in geo.rows.iter().enumerate() {
            let Some(track) = project.tracks.get(i).filter(|_| shows(row, clip)) else {
                continue;
            };
            let Some(h) = hits.iter().find(|h| h.track == track.id) else {
                continue;
            };
            let r = head::rects(
                h.row,
                usize::from(track.folder.is_some()),
                n,
                track.glyph.is_some(),
                s,
            );
            let ink = super::slate::Ink::of(s);
            let selected = a.selected_track == Some(track.id);
            if r.line2.is_some() && track.frozen.is_none() {
                let c = if selected { ink.ink2 } else { ink.ink3 };
                crate::icons::paint_track_icon(&p, r.kind, track.icon(), c);
            }
            if let Some(g) = track
                .glyph
                .as_deref()
                .and_then(crate::views::track_glyphs::by_id)
            {
                // The picker sheet: the icon in the track's colour (full when selected,
                // toward ink 2 at rest), its second ink half-way to the header's ground.
                let own = crate::theme::track_color(track.color);
                let hue = ink.hue(own);
                let ground = if selected {
                    ink.head_ground(ink.surface, own)
                } else {
                    ink.surface
                };
                let main = if track.muted {
                    ink.ink4
                } else if selected {
                    hue
                } else {
                    super::slate::Ink::to(hue, ink.ink2, 0.35)
                };
                let mut inks = crate::views::track_glyphs::inks(s, main, ground);
                inks.second = super::slate::Ink::to(hue, ground, 0.5);
                crate::views::track_glyphs::paint(&p, r.glyph, g, &inks);
            }
            super::super::plugin_notice::paint_header_stopped(&p, r.kind, a, track.id, s);
        }
    }
    app.arrangement.scratch = scratch;
    app.arrangement.hits = hits;
    app.arrangement.words = words;
    input(app, ui, f, geo, s, n);
}

/// Where the add-track row is: under the last row.
pub(super) fn add_rect(geo: &TimelineGeometry, clip: Rect, s: &Style) -> Rect {
    Rect::from_min_size(
        pos2(clip.left(), geo.bottom()),
        vec2(clip.width(), s.s(head::ADD_ROW)),
    )
}

#[path = "arrangement_headers_input.rs"]
mod input_mod;
use input_mod::input;
