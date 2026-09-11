//! Arrangement view: track headers on the left, a timeline with clips on the right.

use super::{beats_to_ticks, ticks_to_beats, GRIDS};
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::icons::{self, Icon};
use crate::theme;
use cantus_core::{
    ClipId, Command, LoopRegion, MergeKey, Ticks, TrackId, MAX_TRACK_HEIGHT, MIN_TRACK_HEIGHT,
};
use egui::{pos2, vec2, Align2, CornerRadius, PointerButton, Rect, Sense, Stroke, StrokeKind};

/// The ruler is two lanes, and that is what makes both gestures direct.
///
/// The **loop lane** on top is where dragging sets the loop brace, which is what a bare
/// drag in the ruler used to do. The **selection lane** below it is where dragging makes a
/// time selection. Putting the new gesture on top of the old one would have cost the loop
/// its direct manipulation; putting the selection behind a modifier (FL's `Ctrl+drag`)
/// would have hidden the thing nearly everything else in the playlist plan hangs off.
/// Reaper splits the ruler the same way, for the same reason.
const RULER_H: f32 = 26.0;
const LOOP_LANE_H: f32 = 8.0;
/// How wide the grab zone on a header's bottom edge is.
const DIVIDER_GRIP: f32 = 5.0;

#[derive(Debug)]
pub struct ArrangementView {
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    /// The base row height. A track's own height is a multiple of this, so the interface
    /// scale and the per-track choice compose instead of fighting.
    pub track_height: f32,
    pub header_width: f32,
    /// A header divider being dragged: the track, and the height it started at.
    divider: Option<(TrackId, f32)>,
    /// A track header being dragged to reorder: the track, and where it would land.
    reorder: Option<(TrackId, usize)>,
    /// A time selection being dragged in the ruler: where it started.
    selecting: Option<Ticks>,
    /// A loop brace being dragged in the loop lane: where it started.
    looping: Option<Ticks>,
    /// What a clip drag snaps to, chosen from the toolbar. Ctrl bypasses it for one drag.
    pub grid: Ticks,
    /// Whether the grid is on at all. The toolbar toggle, mirroring the piano roll's.
    pub snap: bool,
    drag: Option<ClipDrag>,
    /// Rubber band over empty timeline: press origin and current pointer.
    marquee: Option<(egui::Pos2, egui::Pos2)>,
    /// The clip being renamed in place, and the text so far.
    rename_clip: Option<((TrackId, ClipId), String)>,
    /// Where the timeline was drawn last frame (`TimelineGeometry`).
    geometry: Option<TimelineGeometry>,
    /// Which clip the context menu is about, captured when the right button goes down.
    ///
    /// It cannot be re-read from the pointer while the menu is open: the menu appears
    /// next to the pointer, so moving towards it takes the pointer off the clip, and a
    /// menu that only exists while its own target is hovered closes before it can be used.
    menu_clip: Option<(TrackId, ClipId)>,
    rename: Option<(TrackId, String)>,
}

/// What a drag on a clip is doing. Decided at the press, from where in the clip the
/// pointer went down and which modifier is held, and then never re-decided — a gesture
/// that changes meaning halfway through is a gesture you cannot aim.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DragMode {
    /// Move the whole selection along the timeline and across tracks.
    Move,
    /// Trim the left edge: the window moves, the content stays put.
    TrimStart,
    /// Trim the right edge.
    TrimEnd,
    /// Slip: the window stays, the content moves under it.
    Slip,
}

/// A drag in progress. **Nothing is committed until the mouse comes up.**
///
/// The old code issued a merged `SetClipBounds` on every frame of the drag, which meant a
/// cross-track move could not be expressed at all (it is a different command) and undo
/// depended on gesture merging behaving. Previewing instead — drawing the clips where they
/// would land and committing one batch on release — makes move, copy, trim and slip one
/// code path, and gives exactly one undo step whatever the pointer did on the way.
#[derive(Debug)]
struct ClipDrag {
    mode: DragMode,
    /// The clip under the pointer. Trims and slips act on this one; a move acts on the
    /// whole selection.
    anchor: (TrackId, ClipId),
    anchor_start: Ticks,
    anchor_length: Ticks,
    press: egui::Pos2,
    /// Snapped offset along the timeline, updated as the pointer moves.
    delta: Ticks,
    /// How many tracks down (negative: up) the selection would land.
    rows: i32,
    /// Ctrl: leave the originals where they are.
    copy: bool,
}

impl Default for ArrangementView {
    fn default() -> Self {
        ArrangementView {
            px_per_beat: 26.0,
            scroll_beats: 0.0,
            // 40 px, not 64. The header used to hold a fader, a pan bar and a meter — a
            // mixer strip inside a track header — and it is why only four tracks fitted on
            // screen. Those live in the inspector now, which is what buys the density.
            track_height: 40.0,
            header_width: 220.0,
            divider: None,
            reorder: None,
            selecting: None,
            looping: None,
            // A bar, which is where clips land nine times out of ten. The toolbar's grid
            // picker is there for the tenth.
            grid: Ticks::beats(4),
            snap: true,
            drag: None,
            marquee: None,
            menu_clip: None,
            rename_clip: None,
            geometry: None,
            rename: None,
        }
    }
}

impl ArrangementView {
    /// Where the timeline was drawn last frame, or `None` before the first one.
    pub fn geometry(&self) -> Option<TimelineGeometry> {
        self.geometry.clone()
    }

    /// Start renaming a clip in place.
    pub fn begin_rename_clip(&mut self, clip: (TrackId, ClipId), name: String) {
        self.rename_clip = Some((clip, name));
    }

    /// Where a clip would be drawn while a drag is in progress: its start, its length and
    /// how many tracks down it has moved. `None` when this clip is not part of the drag.
    fn preview(
        &self,
        selection: &[(TrackId, ClipId)],
        r: (TrackId, ClipId),
    ) -> Option<(Ticks, Ticks, i32)> {
        let d = self.drag.as_ref()?;
        match d.mode {
            DragMode::Move => {
                if !selection.contains(&r) && r != d.anchor {
                    return None;
                }
                Some((d.delta, Ticks::ZERO, d.rows))
            }
            DragMode::TrimStart if r == d.anchor => Some((d.delta, -d.delta, 0)),
            DragMode::TrimEnd if r == d.anchor => Some((Ticks::ZERO, d.delta, 0)),
            DragMode::Slip if r == d.anchor => Some((Ticks::ZERO, Ticks::ZERO, 0)),
            _ => None,
        }
    }

    /// How far the content has been slipped under the window, for the preview.
    fn slip_preview(&self, r: (TrackId, ClipId)) -> Ticks {
        match &self.drag {
            Some(d) if d.mode == DragMode::Slip && d.anchor == r => -d.delta,
            _ => Ticks::ZERO,
        }
    }
}

/// Copy of the view's geometry so interaction code can call `&mut app` methods freely.
#[derive(Copy, Clone)]
struct Geo {
    left: f32,
    px_per_beat: f32,
    scroll_beats: f32,
}

impl Geo {
    fn x(&self, beats: f32) -> f32 {
        self.left + (beats - self.scroll_beats) * self.px_per_beat
    }
    fn beats(&self, x: f32) -> f32 {
        (x - self.left) / self.px_per_beat + self.scroll_beats
    }
}

/// The clip toolbar: the same shape as the piano roll's, for the same reason.
///
/// Everything here was already reachable from a menu and a shortcut, and that is not the
/// same as being to hand. A row of icons above the thing they act on is how a musician
/// finds out an operation exists at all, and the tooltips carry whatever key is bound
/// *right now* (`Keymap::display_for`), so the toolbar teaches the shortcut rather than
/// competing with it.
fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui) {
    use crate::keymap::Action;
    let selection = app.clip_selection();
    let selected = selection.len();
    let (muted, looping, color) = selection
        .first()
        .and_then(|(t, c)| app.project().track(*t)?.clip(*c))
        .map(|c| (c.muted, c.content_length.is_some(), c.color))
        .unwrap_or((false, false, None));
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(t("arrangement.title"))
                .strong()
                .color(theme::p().gold),
        );
        ui.separator();
        // `active` is state, never a press: mute and repeat light up because the clip *is*
        // muted or repeating, and the rest never light up at all.
        for (action, icon, label, active) in [
            (Action::SplitClip, Icon::Slice, "clip.split", false),
            (Action::GlueClips, Icon::Glue, "clip.glue", false),
            (Action::Duplicate, Icon::Duplicate, "clip.duplicate", false),
            (Action::ToggleClipMute, Icon::Mute, "clip.mute", muted),
            (Action::ToggleClipLoop, Icon::Loop, "clip.loop_on", looping),
        ] {
            let keys = app.keymap.display_for(action);
            let tip = if keys.is_empty() {
                t(label)
            } else {
                format!("{} ({keys})", t(label))
            };
            if icons::button(ui, icon, active, tip).clicked() {
                app.run_action(action);
            }
        }
        // The colour button shows the colour rather than an icon of one: a monochrome
        // glyph could only say "colour", and this says *which*.
        color_button(app, ui, color);
        let keys = app.keymap.display_for(Action::Delete);
        if icons::button(
            ui,
            Icon::Delete,
            false,
            format!("{} ({keys})", t("clip.delete")),
        )
        .clicked()
        {
            app.focus_arrangement();
            app.delete_clip_selection();
        }

        ui.separator();
        ui.label(egui::RichText::new(t("arrangement.grid")).text_style(design::caption()));
        // The same list the piano roll offers, with the same names: two grid pickers that
        // disagree about what "1/8" is called would be a small cruelty.
        let current = GRIDS
            .iter()
            .find(|(_, g)| *g == app.arrangement.grid.0)
            .map(|(k, _)| t(k))
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("arrangement-grid")
            .selected_text(current)
            .width(70.0)
            .show_ui(ui, |ui| {
                for (key, g) in GRIDS {
                    if ui
                        .selectable_label(app.arrangement.grid.0 == g, t(key))
                        .clicked()
                    {
                        app.arrangement.grid = Ticks(g);
                    }
                }
            });
        ui.toggle_value(&mut app.arrangement.snap, t("pianoroll.snap"));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(tf("arrangement.selected", &[("n", &selected)]))
                    .text_style(design::caption())
                    .color(theme::p().text_dim),
            );
            ui.separator();
            // The time selection's length in bars, monospaced so it does not shift as it
            // is dragged. Shown even when empty, so the readout is a place the eye learns.
            let text = match app.time_selection {
                Some((from, to)) => {
                    let bar = app.project().time_signature.ticks_per_bar().0 as f32;
                    let bars = (to - from).0 as f32 / bar;
                    tf("arrangement.selection", &[("bars", &format!("{bars:.2}"))])
                }
                None => t("arrangement.no_selection"),
            };
            ui.label(
                egui::RichText::new(text)
                    .text_style(design::mono_small())
                    .color(theme::p().text_dim),
            );
            // Icons, like the rest of the row: the tooltip carries the sentence and the
            // key, the button carries the picture.
            let has_selection = app.time_selection.is_some();
            for (action, icon, key) in [
                (
                    Action::ZoomToSelection,
                    Icon::Zoom,
                    "action.zoom-to-selection",
                ),
                (
                    Action::LoopToSelection,
                    Icon::Loop,
                    "action.loop-to-selection",
                ),
            ] {
                let keys = app.keymap.display_for(action);
                let tip = if keys.is_empty() {
                    t(key)
                } else {
                    format!("{} ({keys})", t(key))
                };
                let clicked = ui
                    .add_enabled_ui(has_selection, |ui| icons::button(ui, icon, false, tip))
                    .inner
                    .clicked();
                if clicked {
                    app.run_action(action);
                }
            }
        });
    });
}

/// A swatch that opens the clip palette, and shows what the selection is set to.
fn color_button(app: &mut CantusApp, ui: &mut egui::Ui, color: Option<cantus_core::Color>) {
    let (rect, resp) = ui.allocate_exact_size(design::ICON_BUTTON, Sense::click());
    let pal = theme::p();
    let swatch = Rect::from_center_size(rect.center(), vec2(space::S4, space::S4));
    let fill = color.map(theme::track_color).unwrap_or(pal.grid_bar);
    ui.painter().rect(
        swatch,
        pal.corner_radius,
        fill,
        Stroke::new(
            1.0,
            if resp.hovered() {
                pal.text
            } else {
                pal.grid_strong
            },
        ),
        StrokeKind::Inside,
    );
    // No override: a diagonal bar says "the track's colour", not "no colour".
    if color.is_none() {
        ui.painter().line_segment(
            [swatch.left_bottom(), swatch.right_top()],
            Stroke::new(1.0, pal.text_dim),
        );
    }
    let keys = app.keymap.display_for(crate::keymap::Action::ClipColorNext);
    let resp = resp.on_hover_text(format!("{} ({keys})", t("clip.color")));
    egui::Popup::menu(&resp).show(|ui| {
        ui.horizontal(|ui| {
            for c in cantus_core::Color::PALETTE {
                let (r, resp) = ui.allocate_exact_size(vec2(space::S4, space::S4), Sense::click());
                ui.painter()
                    .rect_filled(r, pal.corner_radius, theme::track_color(c));
                if resp.hovered() {
                    ui.painter().rect_stroke(
                        r,
                        pal.corner_radius,
                        Stroke::new(1.0, pal.text),
                        StrokeKind::Inside,
                    );
                }
                if resp.clicked() {
                    app.set_clip_color(Some(c));
                    ui.close();
                }
            }
        });
        if ui.button(t("clip.color_reset")).clicked() {
            app.set_clip_color(None);
            ui.close();
        }
    });
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    toolbar(app, ui);
    let full = ui.available_rect_before_wrap();
    let header_w = app.arrangement.header_width;
    let header_rect = Rect::from_min_max(full.min, pos2(full.left() + header_w, full.bottom()));
    let timeline_rect =
        Rect::from_min_max(pos2(full.left() + header_w + 1.0, full.top()), full.max);

    ui.painter().rect_filled(full, 0.0, theme::p().ink);
    headers(app, ui, header_rect);
    timeline(app, ui, timeline_rect);
}

/// The track headers.
///
/// **What is *not* here any more is the point.** Until this round each header carried a
/// fader, a pan bar and a meter — a mixer strip inside a track header, in 64 px, which is
/// why only four tracks fitted on a screen. Those moved to the inspector, where there is
/// room to show them properly and where somebody mixing is already looking. What is left
/// is what a header is *for*: which track this is (colour, icon, name) and whether it is
/// sounding (mute, solo, a hairline meter). That fits in 40 px, and 40 px is what makes a
/// twelve-track song legible without scrolling.
fn headers(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let ui = &mut child;
    ui.painter().rect_filled(rect, 0.0, theme::p().panel);
    // Ruler-height spacer with the add button.
    let (top, _) = ui.allocate_exact_size(vec2(rect.width(), RULER_H), Sense::hover());
    let mut top_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(top.shrink2(vec2(6.0, 2.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    if top_ui
        .small_button(format!("+ {}", t("track.add")))
        .clicked()
    {
        app.add_track();
    }

    let base = app.arrangement.track_height;
    let rows = rows_of(app.project(), top.bottom(), base);
    let ids: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    let mut to_delete = None;
    let pal = theme::p();

    for (i, id) in ids.iter().enumerate() {
        let Some(geo_row) = rows.get(i).copied() else {
            continue;
        };
        let row = Rect::from_min_size(
            pos2(rect.left(), geo_row.top),
            vec2(rect.width(), geo_row.height),
        );
        // The interactive area is shifted down by the grip: a row owns the divider *below*
        // it and not the one above, so a press exactly on the line goes to the row whose
        // height it changes. With the plain rect, both rows contained the line and the
        // later one won, which turned every divider drag into a reorder of the row below.
        let hit = Rect::from_min_max(
            pos2(row.left(), row.top() + DIVIDER_GRIP),
            pos2(row.right(), row.bottom() + DIVIDER_GRIP),
        );
        let resp = ui.allocate_rect(hit, Sense::click_and_drag());
        let selected = app.selected_track == Some(*id);
        let track = match app.project().track(*id) {
            Some(t) => t.clone(),
            None => continue,
        };
        let p = ui.painter();
        let bg = if selected {
            pal.panel_raised
        } else if i % 2 == 0 {
            pal.panel
        } else {
            pal.row_white
        };
        p.rect_filled(row, 0.0, bg);
        // The colour stripe is wider than it was and it is the grab handle for reordering,
        // which is why it is worth the pixels: a header you can drag anywhere would fight
        // with the name editor and the buttons.
        p.rect_filled(
            Rect::from_min_size(row.min, vec2(6.0, row.height())),
            0.0,
            theme::track_color(track.color),
        );
        p.hline(
            row.x_range(),
            row.bottom() - 0.5,
            Stroke::new(1.0, pal.grid),
        );

        // Where the row is being dragged to, drawn as a line rather than by moving the
        // headers: a list that reshuffles under the pointer is a list you cannot aim at.
        if let Some((dragged, to)) = app.arrangement.reorder {
            if dragged != *id && to == i {
                p.hline(row.x_range(), row.top(), Stroke::new(2.0, pal.gold_bright));
            }
        }

        let inner = row.shrink2(vec2(space::S3, 2.0));
        let icon_rect =
            Rect::from_min_size(pos2(inner.left(), inner.center().y - 8.0), vec2(16.0, 16.0));
        icons::paint_track_icon(
            p,
            icon_rect,
            track.icon(),
            if selected {
                pal.gold_bright
            } else {
                pal.text_dim
            },
        );

        let body = Rect::from_min_max(pos2(icon_rect.right() + space::S2, inner.top()), inner.max);
        let mut r = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        header_row(app, &mut r, *id, &track, selected);

        // The meter is a hairline along the bottom of the header now, not a bar with a
        // lane of its own. It answers "is this track making a sound", which is the only
        // metering question a *header* has to answer; the mixer answers the rest.
        let meters = app.engine.meters();
        if let Some(idx) = app.project().track_index(*id) {
            if idx < meters.track_count {
                let peak = meters.tracks[idx];
                let frac = ((cantus_core::linear_to_db(peak) + 60.0) / 60.0).clamp(0.0, 1.0);
                if frac > 0.0 {
                    let bar = Rect::from_min_size(
                        pos2(row.left() + 6.0, row.bottom() - 3.0),
                        vec2((row.width() - 6.0) * frac, 2.0),
                    );
                    ui.painter().rect_filled(bar, 0.0, theme::meter_color(peak));
                }
            }
        }

        // ---- the divider ------------------------------------------------------------
        let grip = Rect::from_min_max(
            pos2(row.left(), row.bottom() - DIVIDER_GRIP),
            pos2(row.right(), row.bottom() + DIVIDER_GRIP),
        );
        let over_grip = resp.hover_pos().is_some_and(|p| grip.contains(p));
        if over_grip || app.arrangement.divider.map(|(d, _)| d) == Some(*id) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            ui.painter().hline(
                row.x_range(),
                row.bottom() - 0.5,
                Stroke::new(2.0, pal.gold),
            );
        }

        if resp.clicked() {
            app.select_track(*id);
        }
        if resp.double_clicked() {
            app.arrangement.rename = Some((*id, track.name.clone()));
        }
        // A drag that starts on the divider resizes; anywhere else it reorders. Decided at
        // the **press origin**, never at the pointer: egui reports a drag only once the
        // pointer has travelled, by which time it has left the five-pixel grip. That is the
        // round-3 lesson the arrangement had to learn twice.
        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin());
            if origin.is_some_and(|p| grip.contains(p)) {
                app.arrangement.divider = Some((*id, track.height()));
            } else {
                app.select_track(*id);
                app.arrangement.reorder = Some((*id, i));
            }
        }
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some((did, start_h)) = app.arrangement.divider {
                    if did == *id {
                        let origin = ui
                            .input(|i| i.pointer.press_origin())
                            .map(|p| p.y)
                            .unwrap_or(pos.y);
                        let height = start_h + (pos.y - origin) / base;
                        app.exec_merging(
                            Command::SetTrackHeight {
                                id: *id,
                                height: height.clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT),
                            },
                            MergeKey::of(&("height", id.0)),
                        );
                    }
                } else if app.arrangement.reorder.map(|(d, _)| d) == Some(*id) {
                    let to = rows
                        .iter()
                        .position(|r| pos.y < r.top + r.height * 0.5)
                        .unwrap_or(rows.len().saturating_sub(1));
                    app.arrangement.reorder = Some((*id, to));
                }
            }
        }
        if resp.drag_stopped() {
            app.arrangement.divider = None;
            if let Some((did, to)) = app.arrangement.reorder.take() {
                if did == *id && to != i {
                    app.exec(Command::MoveTrack { id: did, index: to });
                }
            }
            app.end_gesture();
        }

        resp.context_menu(|ui| {
            track_menu(app, ui, *id, &mut to_delete);
        });
    }
    if let Some(id) = to_delete {
        app.delete_track(id);
    }
}

/// The one row of controls a header keeps: the name, then mute and solo on the right.
fn header_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    track: &cantus_core::Track,
    selected: bool,
) {
    if let Some((rid, name)) = app
        .arrangement
        .rename
        .as_mut()
        .filter(|(rid, _)| *rid == id)
    {
        let resp = ui.text_edit_singleline(name);
        resp.request_focus();
        if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let new_name = name.clone();
            let rid = *rid;
            app.arrangement.rename = None;
            if new_name != track.name {
                app.exec(Command::SetTrackName {
                    id: rid,
                    name: new_name,
                });
            }
        }
    } else {
        ui.add(
            egui::Label::new(
                egui::RichText::new(&track.name)
                    .text_style(design::strong())
                    .color(if selected {
                        theme::p().gold_bright
                    } else {
                        theme::p().text
                    }),
            )
            .truncate()
            .selectable(false),
        );
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let mut solo = track.soloed;
        let s = ui.selectable_label(
            solo,
            egui::RichText::new(t("track.solo")).color(if solo {
                theme::p().gold_bright
            } else {
                theme::p().text_dim
            }),
        );
        if s.on_hover_text(t("track.solo_tip")).clicked() {
            solo = !solo;
            app.exec(Command::SetTrackSolo { id, soloed: solo });
        }
        let mut muted = track.muted;
        let m = ui.selectable_label(
            muted,
            egui::RichText::new(t("track.mute")).color(if muted {
                theme::p().danger
            } else {
                theme::p().text_dim
            }),
        );
        if m.on_hover_text(t("track.mute_tip")).clicked() {
            muted = !muted;
            app.exec(Command::SetTrackMute { id, muted });
        }
    });
}

/// The track's own menu: height presets, an icon, and delete.
///
/// Presets as well as the divider because a divider is for *aiming* and a preset is for
/// *agreeing*: getting six tracks to exactly the same height by hand is a job nobody
/// should be given.
fn track_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    to_delete: &mut Option<TrackId>,
) {
    let current = app.project().track(id).map(|t| t.height()).unwrap_or(1.0);
    ui.label(
        egui::RichText::new(t("track.height"))
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
    for (key, h) in TRACK_HEIGHTS {
        if ui
            .selectable_label((current - h).abs() < 0.01, t(key))
            .clicked()
        {
            app.exec(Command::SetTrackHeight { id, height: h });
            ui.close();
        }
    }
    if ui.button(t("track.height_all")).clicked() {
        app.set_all_track_heights(current);
        ui.close();
    }
    ui.separator();
    ui.label(
        egui::RichText::new(t("track.icon"))
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
    let chosen = app.project().track(id).and_then(|t| t.icon);
    ui.horizontal(|ui| {
        for icon in cantus_core::TrackIcon::ALL {
            let (rect, resp) = ui.allocate_exact_size(design::ICON_BUTTON, Sense::click());
            let on = chosen == Some(icon);
            let pal = theme::p();
            if on {
                ui.painter()
                    .rect_filled(rect, pal.corner_radius, pal.gold.gamma_multiply(0.35));
            }
            icons::paint_track_icon(
                ui.painter(),
                rect.shrink(space::S1),
                icon,
                if on || resp.hovered() {
                    pal.text
                } else {
                    pal.text_dim
                },
            );
            if resp.on_hover_text(t(icon.label_key())).clicked() {
                app.exec(Command::SetTrackIcon {
                    id,
                    icon: Some(icon),
                });
                ui.close();
            }
        }
    });
    if ui.button(t("track.icon_auto")).clicked() {
        app.exec(Command::SetTrackIcon { id, icon: None });
        ui.close();
    }
    ui.separator();
    if ui.button(t("track.delete")).clicked() {
        *to_delete = Some(id);
        ui.close();
    }
}

/// The height presets, as multiples of the base row. Small enough to scan a long song,
/// tall enough to read a clip's contents, and two steps in between.
pub const TRACK_HEIGHTS: [(&str, f32); 4] = [
    ("track.height_small", 0.6),
    ("track.height_medium", 1.0),
    ("track.height_large", 1.8),
    ("track.height_huge", 3.0),
];

/// Where the timeline was drawn, in screen coordinates.
///
/// Recorded every frame so that anything outside `show` — a test aiming at a clip, and
/// later anything that wants to scroll the view to a tick — can map between ticks and
/// pixels by *asking* rather than by mirroring the layout constants. A test that
/// recomputes the layout is a test that breaks the day a toolbar is added, which is
/// exactly what happened.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineGeometry {
    pub left: f32,
    /// How wide the timeline was drawn. Zoom-to-selection needs it and cannot recompute
    /// it: the panel's width is whatever the layout gave it this frame.
    pub width: f32,
    /// The top of the ruler; the loop lane runs from here to `loop_lane_bottom`, the
    /// selection lane from there to `body_top`.
    pub ruler_top: f32,
    pub loop_lane_bottom: f32,
    pub body_top: f32,
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    /// Where each track's row was drawn: its top and its height. Rows are no longer all
    /// the same size, so a single `track_height` cannot answer "where is row 3" any more —
    /// and every caller that used to multiply is exactly the caller that would have got it
    /// wrong.
    pub rows: Vec<TrackRow>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TrackRow {
    pub top: f32,
    pub height: f32,
}

impl TimelineGeometry {
    /// Screen x of a position in beats.
    pub fn x(&self, beats: f32) -> f32 {
        self.left + (beats - self.scroll_beats) * self.px_per_beat
    }

    /// Position in beats of a screen x.
    pub fn beats(&self, x: f32) -> f32 {
        (x - self.left) / self.px_per_beat + self.scroll_beats
    }

    /// Screen y of the middle of a track's row.
    pub fn row_y(&self, row: usize) -> f32 {
        match self.rows.get(row) {
            Some(r) => r.top + r.height * 0.5,
            None => self.body_top,
        }
    }

    /// Which row a screen y falls in.
    pub fn row_at(&self, y: f32) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| y >= r.top && y < r.top + r.height)
    }

    /// One past the bottom of the last row.
    pub fn bottom(&self) -> f32 {
        self.rows
            .last()
            .map(|r| r.top + r.height)
            .unwrap_or(self.body_top)
    }
}

/// Where each track's row sits, given the base row height. One function, so the headers
/// and the timeline can never disagree about which stripe belongs to which track — which
/// they would the first time one of them forgot that heights vary.
pub fn rows_of(project: &cantus_core::Project, top: f32, base: f32) -> Vec<TrackRow> {
    let mut y = top;
    project
        .tracks
        .iter()
        .map(|t| {
            let height = (base * t.height()).round();
            let row = TrackRow { top: y, height };
            y += height;
            row
        })
        .collect()
}

/// Hit test result: which clip the pointer is over, and where in it.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Hit {
    track: TrackId,
    clip: ClipId,
    mode: DragMode,
}

/// How wide the trim zone at each end of a clip is. Wide enough to aim at, never more
/// than a third of the clip, so a very short clip can still be moved.
fn edge_zone(width: f32) -> f32 {
    (width / 3.0).min(8.0)
}

fn timeline(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect) {
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let p = ui.painter().with_clip_rect(rect);
    let left = rect.left();
    let ts = app.project().time_signature;
    let beats_per_bar = ts.numerator as f32 * 4.0 / ts.denominator as f32;

    // Zoom / scroll.
    if resp.hovered() {
        let (zoom, scroll, shift) =
            ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.modifiers.shift));
        if zoom != 1.0 {
            let v = &mut app.arrangement;
            let pointer_beats = resp
                .hover_pos()
                .map(|p| (p.x - left) / v.px_per_beat + v.scroll_beats)
                .unwrap_or(v.scroll_beats);
            v.px_per_beat = (v.px_per_beat * zoom).clamp(2.0, 400.0);
            v.scroll_beats = (pointer_beats
                - (resp.hover_pos().map(|p| p.x).unwrap_or(left) - left) / v.px_per_beat)
                .max(0.0);
        } else if scroll != egui::Vec2::ZERO {
            let dx = if shift { scroll.y } else { scroll.x };
            let v = &mut app.arrangement;
            v.scroll_beats = (v.scroll_beats - dx / v.px_per_beat).max(0.0);
        }
    }
    let view = Geo {
        left,
        px_per_beat: app.arrangement.px_per_beat,
        scroll_beats: app.arrangement.scroll_beats,
    };

    // Rows. Heights are per track now, and both halves of the view get them from
    // `rows_of` rather than each multiplying by a base — which is how a header and its
    // lane would end up one pixel apart at the bottom of a long song.
    let base = app.arrangement.track_height;
    let body_top = rect.top() + RULER_H;
    let rows = rows_of(app.project(), body_top, base);
    app.arrangement.geometry = Some(TimelineGeometry {
        left,
        width: rect.width(),
        ruler_top: rect.top(),
        loop_lane_bottom: rect.top() + LOOP_LANE_H,
        body_top,
        px_per_beat: view.px_per_beat,
        scroll_beats: view.scroll_beats,
        rows: rows.clone(),
    });
    let n_tracks = rows.len();
    for (i, r) in rows.iter().enumerate() {
        let row = Rect::from_min_max(
            pos2(rect.left(), r.top),
            pos2(rect.right(), r.top + r.height),
        );
        p.rect_filled(
            row,
            0.0,
            if i % 2 == 0 {
                theme::p().row_black
            } else {
                theme::p().row_white
            },
        );
        p.hline(
            row.x_range(),
            row.bottom() - 0.5,
            Stroke::new(1.0, theme::p().grid),
        );
    }

    // Ruler background (painted before the bar numbers so they stay readable).
    let ruler = Rect::from_min_max(rect.min, pos2(rect.right(), body_top));
    p.rect_filled(ruler, 0.0, theme::p().panel.gamma_multiply(0.9));

    // Grid lines: bars strong, beats faint.
    let first_beat = view.scroll_beats.floor() as i64;
    let last_beat = (view.scroll_beats + rect.width() / view.px_per_beat).ceil() as i64 + 1;
    let beat_step = if view.px_per_beat < 6.0 {
        beats_per_bar as i64
    } else {
        1
    };
    let mut b = first_beat - first_beat.rem_euclid(beat_step);
    while b <= last_beat {
        let x = view.x(b as f32);
        let is_bar = (b as f32 % beats_per_bar).abs() < 1e-3;
        let color = if is_bar {
            theme::p().grid_bar
        } else {
            theme::p().grid
        };
        p.vline(x, body_top..=rect.bottom(), Stroke::new(1.0, color));
        if is_bar {
            let bar_no = (b as f32 / beats_per_bar) as i64 + 1;
            p.text(
                pos2(x + 3.0, rect.top() + 3.0),
                Align2::LEFT_TOP,
                bar_no.to_string(),
                design::paint::ruler(),
                theme::p().text_dim,
            );
        }
        b += beat_step;
    }
    // The loop brace lives in the top lane, which is also where it is dragged.
    let loop_lane = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + LOOP_LANE_H));
    p.rect_filled(loop_lane, 0.0, theme::p().ink.gamma_multiply(0.25));
    if let Some(l) = &app.project().loop_region {
        let x0 = view.x(ticks_to_beats(l.start));
        let x1 = view.x(ticks_to_beats(l.end));
        let alpha = if app.project().loop_enabled {
            0.85
        } else {
            0.3
        };
        p.rect_filled(
            Rect::from_min_max(
                pos2(x0, loop_lane.top() + 1.0),
                pos2(x1, loop_lane.bottom() - 1.0),
            ),
            0.0,
            theme::p().gold.gamma_multiply(alpha),
        );
    }
    // The time selection: a translucent column across **every** track, which is what makes
    // "this stretch of the song" a thing you can see rather than remember. It is drawn
    // before the clips so it tints them rather than covering them.
    if let Some((from, to)) = app.time_selection {
        let x0 = view.x(ticks_to_beats(from));
        let x1 = view.x(ticks_to_beats(to));
        let column = Rect::from_min_max(pos2(x0, body_top), pos2(x1, rect.bottom()));
        p.rect_filled(column, 0.0, theme::p().selection.gamma_multiply(0.16));
        // Brighter in the ruler, where the edges are: a wash that is legible over a clip
        // is too faint to aim at, and a band you can aim at is too heavy over the music.
        p.rect_filled(
            Rect::from_min_max(pos2(x0, loop_lane.bottom()), pos2(x1, body_top)),
            0.0,
            theme::p().selection.gamma_multiply(0.55),
        );
        for x in [x0, x1] {
            p.vline(
                x,
                rect.top()..=rect.bottom(),
                Stroke::new(1.0, theme::p().selection),
            );
        }
    }
    p.hline(
        ruler.x_range(),
        body_top - 0.5,
        Stroke::new(1.0, theme::p().grid_strong),
    );

    // ---- clips ----------------------------------------------------------------------
    let selection = app.clip_selection();
    // Two hit tests, and the difference between them is the whole reason resizing works.
    //
    // `hover` is what is under the pointer *now*: the cursor shape, the context menu.
    // `at_press` is what was under the pointer when the button went down, and that is what
    // decides what a drag is doing. egui only reports a drag once the pointer has
    // travelled a few pixels — by which time it has left the 8-px trim zone at the clip's
    // edge, so asking "what is under the pointer now" turns every resize into a move. The
    // piano roll learned this in round 3; the arrangement had to learn it too.
    let pointer = resp.hover_pos();
    let press_origin = ui.input(|i| i.pointer.press_origin());
    let mut hit: Option<Hit> = None;
    let mut hit_at_press: Option<Hit> = None;
    // Every note miniature in the whole arrangement goes into one mesh. A clip can hold
    // twenty thousand notes and there can be a hundred clips on screen; the convenience
    // painters cost about ten times as much per item (ADR 0011), so this is a rule and
    // not an optimisation to remember.
    let mut minis = egui::Mesh::default();
    let mut at = Vec::new();
    // Where to put the in-place name editor, once the clip being renamed is drawn.
    let mut renaming: Option<Rect> = None;
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(here) = rows.get(i).copied() else {
            continue;
        };
        let track_color = theme::track_color(track.color);
        for clip in &track.clips {
            let r = (track.id, clip.id);
            let (dstart, dlen, drows) =
                app.arrangement
                    .preview(&selection, r)
                    .unwrap_or((Ticks::ZERO, Ticks::ZERO, 0));
            let start = clip.start + dstart;
            let length = (clip.length + dlen).max(Ticks::SIXTEENTH);
            // A clip being dragged across tracks lands in the row it lands in, and rows
            // are different heights now — so it takes the *target* row's shape, not its
            // own. A preview that kept its old height would be a lie about where it goes.
            let target = rows
                .get((i as i32 + drows).clamp(0, rows.len() as i32 - 1) as usize)
                .copied()
                .unwrap_or(here);
            let (row_y, row_h) = (target.top, target.height);
            let x0 = view.x(ticks_to_beats(start));
            let x1 = view.x(ticks_to_beats(start + length));
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let color = clip.color.map(theme::track_color).unwrap_or(track_color);
            // The chrome scales with the row: at 24 px there is no room for a four-pixel
            // margin top and bottom, and a clip that is all margin is a clip you cannot see.
            let inset = (row_h * 0.08).clamp(1.0, 4.0);
            let body = Rect::from_min_max(
                pos2(x0, row_y + inset),
                pos2(x1.max(x0 + 2.0), row_y + row_h - inset),
            );
            let sel = selection.contains(&r);
            // A muted clip keeps its colour but loses its weight: it is still that
            // track's clip, it just is not sounding.
            let fill = if clip.muted {
                0.15
            } else if sel {
                0.55
            } else {
                0.35
            };
            p.rect_filled(body, CornerRadius::same(3), color.gamma_multiply(fill));
            p.rect_stroke(
                body,
                CornerRadius::same(3),
                Stroke::new(
                    if sel { 2.0 } else { 1.0 },
                    if sel { theme::p().gold_bright } else { color },
                ),
                StrokeKind::Inside,
            );
            // Note miniature, in the clip's own content frame so a slipped, trimmed or
            // repeating clip shows what it actually plays.
            // Geometry only: cloning the clip here would copy every note in it, once per
            // clip, every frame (`ClipWindow`).
            let shown = cantus_core::ClipWindow {
                start,
                length,
                offset: clip.offset + app.arrangement.slip_preview(r),
                content_length: clip.content_length,
            };
            if !clip.notes.is_empty() && body.width() > 6.0 {
                let (lo, hi) = clip.notes.iter().fold((127u8, 0u8), |(lo, hi), n| {
                    (lo.min(n.pitch), hi.max(n.pitch))
                });
                let span = (hi - lo).max(12) as f32;
                // Leave room for the name at the top, but never more than half the clip:
                // on a short track the miniature would otherwise vanish entirely.
                let name_h = (row_h * 0.35).clamp(4.0, 14.0);
                let inner = body.shrink2(vec2(2.0, name_h));
                if inner.height() > 2.0 {
                    for n in &clip.notes {
                        shown.timeline_positions(n.start, &mut at);
                        for on in at.iter().copied() {
                            let nx0 = view.x(ticks_to_beats(on));
                            let nx1 = view.x(ticks_to_beats(on + n.length));
                            if nx1 < body.left() || nx0 > body.right() {
                                continue;
                            }
                            let ny = inner.bottom() - (n.pitch - lo) as f32 / span * inner.height();
                            minis.add_colored_rect(
                                Rect::from_min_max(
                                    pos2(nx0.max(body.left()), ny - 1.0),
                                    pos2(nx1.min(body.right()).max(nx0 + 1.0), ny + 1.0),
                                ),
                                color.gamma_multiply(if clip.muted { 0.5 } else { 1.4 }),
                            );
                        }
                    }
                }
            }
            // Repeat marks: a hairline where the content starts again, so a looping clip
            // is recognisable without opening it.
            if let Some(period) = shown.period() {
                let mut d = period;
                while d < length {
                    let x = view.x(ticks_to_beats(start + d));
                    p.vline(
                        x,
                        body.y_range(),
                        Stroke::new(1.0, theme::p().ink.gamma_multiply(0.5)),
                    );
                    d += period;
                }
            }
            if app
                .arrangement
                .rename_clip
                .as_ref()
                .is_some_and(|(target, _)| *target == r)
            {
                renaming = Some(body);
            } else {
                let clip_p = p.with_clip_rect(body);
                let label = if clip.muted {
                    format!("{} ({})", clip.name, t("clip.muted_short"))
                } else {
                    clip.name.clone()
                };
                clip_p.text(
                    pos2(body.left() + 5.0, body.top() + 3.0),
                    Align2::LEFT_TOP,
                    label,
                    design::paint::label(),
                    theme::p().text,
                );
            }
            let test = |pos: egui::Pos2| -> Option<Hit> {
                if !body.contains(pos) {
                    return None;
                }
                let zone = edge_zone(body.width());
                let mode = if pos.x < body.left() + zone {
                    DragMode::TrimStart
                } else if pos.x > body.right() - zone {
                    DragMode::TrimEnd
                } else {
                    DragMode::Move
                };
                Some(Hit {
                    track: track.id,
                    clip: clip.id,
                    mode,
                })
            };
            if let Some(h) = pointer.and_then(&test) {
                hit = Some(h);
            }
            if let Some(h) = press_origin.and_then(&test) {
                hit_at_press = Some(h);
            }
        }
    }
    if !minis.is_empty() {
        p.add(egui::Shape::Mesh(std::sync::Arc::new(minis)));
    }

    // Renaming happens on the clip itself rather than in a dialog: the name is a label on
    // a rectangle, and typing it anywhere else means looking away from the thing you are
    // naming. `F2` starts it; Enter and clicking away both commit.
    if let Some(body) = renaming {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body.shrink2(vec2(4.0, (body.height() - space::ROW_DENSE) * 0.5)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let mut done = None;
        if let Some((target, name)) = app.arrangement.rename_clip.as_mut() {
            let target = *target;
            let edit = child.add(
                egui::TextEdit::singleline(name)
                    .desired_width(f32::INFINITY)
                    .font(design::strong()),
            );
            edit.request_focus();
            if edit.lost_focus() || child.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some((target, name.clone()));
            }
            if child.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some((target, String::new()));
            }
        }
        if let Some((target, name)) = done {
            app.arrangement.rename_clip = None;
            app.rename_clip(target, name);
        }
    }

    // Marquee.
    if let Some((a, b)) = app.arrangement.marquee {
        let r = Rect::from_two_pos(a, b);
        p.rect_filled(r, 0.0, theme::p().selection.gamma_multiply(0.18));
        p.rect_stroke(
            r,
            CornerRadius::ZERO,
            Stroke::new(1.0, theme::p().selection),
            StrokeKind::Inside,
        );
    }

    // Playhead.
    let ph_x = view.x(ticks_to_beats(app.engine.position()));
    if ph_x >= rect.left() && ph_x <= rect.right() {
        p.vline(
            ph_x,
            rect.y_range(),
            Stroke::new(1.5, theme::p().gold_bright),
        );
    }
    if let Some(pos) = pointer {
        if pos.y < body_top && app.arrangement.drag.is_none() {
            ui.ctx()
                .set_cursor_icon(if pos.y < rect.top() + LOOP_LANE_H {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::Text
                });
        }
    }
    if let Some(h) = hit {
        if app.arrangement.drag.is_none() {
            ui.ctx().set_cursor_icon(match h.mode {
                DragMode::TrimStart | DragMode::TrimEnd => egui::CursorIcon::ResizeHorizontal,
                _ => egui::CursorIcon::Grab,
            });
        }
    }

    // ---- interaction ----------------------------------------------------------------
    let press = ui.input(|i| i.pointer.press_origin());
    let mods = ui.input(|i| i.modifiers);
    let ids: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    let row_rects = rows.clone();
    let row_of = move |y: f32| -> Option<usize> {
        row_rects
            .iter()
            .position(|r| y >= r.top && y < r.top + r.height)
    };
    let track_at = {
        let rows = rows.clone();
        let ids = ids.clone();
        move |y: f32| -> Option<TrackId> {
            rows.iter()
                .position(|r| y >= r.top && y < r.top + r.height)
                .and_then(|i| ids.get(i).copied())
        }
    };
    // Copied out of the view: the closure must not hold a borrow of `app`, since almost
    // everything below wants `&mut app`.
    //
    // The arrangement's modifiers are not the piano roll's, and deliberately so. There is
    // no pitch axis here, so Shift and Alt are free for the two things a clip drag needs
    // that a note drag does not — slipping the content, and copying instead of moving —
    // and Alt+drag to copy is what a musician arriving from any of the big four expects.
    // That leaves the grid bypass on Ctrl. Ctrl+*click* still extends the selection:
    // egui only reports a click when the pointer did not travel, so the two never meet.
    let grid = app.arrangement.grid;
    let snap_on = app.arrangement.snap;
    let snap_to = move |t: Ticks, off_grid: bool| {
        if off_grid || !snap_on {
            t
        } else {
            t.snap(grid)
        }
    };

    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.y < body_top {
                let beats = view.beats(pos.x).max(0.0);
                app.seek(snap_to(beats_to_ticks(beats), mods.command));
                if pos.y >= rect.top() + LOOP_LANE_H {
                    app.focus_arrangement();
                    app.clear_time_selection();
                }
            } else if let Some(h) = hit {
                app.focus_arrangement();
                if mods.command {
                    app.toggle_clip_selection(h.track, h.clip);
                } else {
                    app.select_clip(h.track, h.clip);
                }
            } else {
                app.focus_arrangement();
                if !mods.command {
                    app.clear_clip_selection();
                }
                if let Some(tid) = track_at(pos.y) {
                    app.select_track(tid);
                }
            }
        }
    }
    if resp.double_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.y >= body_top && hit.is_none() {
                if let Some(tid) = track_at(pos.y) {
                    let beats = view.beats(pos.x).max(0.0);
                    let start = beats_to_ticks(beats).snap_floor(beats_to_ticks(beats_per_bar));
                    app.create_clip(tid, start, beats_to_ticks(beats_per_bar * 4.0));
                }
            }
        }
    }
    // A context menu, not an immediate delete: right-clicking a clip away with no warning
    // is a hard thing to undo your way out of when it was the wrong clip. It is opened on
    // the clip that was under the pointer when the button went down, and drawn every frame
    // after that whatever the pointer is doing (see `menu_clip`).
    if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)) {
        app.arrangement.menu_clip = hit.map(|h| (h.track, h.clip));
    }
    let menu_target = app
        .arrangement
        .menu_clip
        .filter(|(t, c)| app.project().track(*t).and_then(|t| t.clip(*c)).is_some());
    resp.context_menu(|ui| match menu_target {
        Some((track, clip)) => clip_menu(app, ui, track, clip),
        None => timeline_menu(app, ui),
    });

    if resp.drag_started_by(PointerButton::Primary) {
        if let Some(pos) = press {
            if pos.y < body_top {
                app.arrangement.drag = None;
                let at = snap_to(beats_to_ticks(view.beats(pos.x).max(0.0)), mods.command);
                if pos.y < rect.top() + LOOP_LANE_H {
                    app.arrangement.looping = Some(at);
                } else {
                    app.focus_arrangement();
                    app.arrangement.selecting = Some(at);
                }
            } else if let Some(h) = hit_at_press {
                app.focus_arrangement();
                if !app.clip_selection().contains(&(h.track, h.clip)) {
                    app.select_clip(h.track, h.clip);
                }
                let clip = app
                    .project()
                    .track(h.track)
                    .and_then(|t| t.clip(h.clip))
                    .cloned();
                if let Some(clip) = clip {
                    // Shift turns a move into a slip: the window stays, the music inside
                    // it slides. Alt copies instead of moving; Ctrl ignores the grid.
                    let mode = if mods.shift && h.mode == DragMode::Move {
                        DragMode::Slip
                    } else {
                        h.mode
                    };
                    app.arrangement.drag = Some(ClipDrag {
                        mode,
                        anchor: (h.track, h.clip),
                        anchor_start: clip.start,
                        anchor_length: clip.length,
                        press: pos,
                        delta: Ticks::ZERO,
                        rows: 0,
                        copy: mods.alt && mode == DragMode::Move,
                    });
                }
            } else {
                app.arrangement.marquee = Some((pos, pos));
            }
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(d) = app.arrangement.drag.as_ref() {
                let raw = beats_to_ticks((pos.x - d.press.x) / view.px_per_beat);
                let (mode, anchor_start, anchor_length, press_y) =
                    (d.mode, d.anchor_start, d.anchor_length, d.press.y);
                // Snap the *result*, not the movement: the user is aiming the edge at a
                // bar line, not measuring how far they dragged.
                let delta = match mode {
                    DragMode::TrimEnd => {
                        snap_to(anchor_start + anchor_length + raw, mods.command)
                            - (anchor_start + anchor_length)
                    }
                    _ => snap_to(anchor_start + raw, mods.command) - anchor_start,
                };
                let rows = match mode {
                    DragMode::Move => {
                        let from = row_of(press_y).unwrap_or(0) as i32;
                        let to = row_of(pos.y).unwrap_or(from as usize) as i32;
                        to - from
                    }
                    _ => 0,
                };
                if let Some(d) = app.arrangement.drag.as_mut() {
                    d.delta = delta;
                    d.rows = rows;
                    d.copy = mods.alt && mode == DragMode::Move;
                }
            } else if let Some(m) = app.arrangement.marquee.as_mut() {
                m.1 = pos;
            } else if let Some(a) = app.arrangement.looping {
                let b = snap_to(beats_to_ticks(view.beats(pos.x).max(0.0)), mods.command);
                let (s, e) = if a <= b { (a, b) } else { (b, a) };
                if e > s {
                    app.exec_merging(
                        Command::SetLoopRegion {
                            region: Some(LoopRegion { start: s, end: e }),
                        },
                        MergeKey::of(&"loop"),
                    );
                }
            } else if let Some(a) = app.arrangement.selecting {
                // A selection is not a command: nothing about the piece changed, so there
                // is nothing to undo. It is the same kind of state as which clips are
                // highlighted.
                let b = snap_to(beats_to_ticks(view.beats(pos.x).max(0.0)), mods.command);
                app.set_time_selection(a, b);
            }
        }
    }
    if resp.drag_stopped() {
        app.arrangement.looping = None;
        app.arrangement.selecting = None;
        if let Some(d) = app.arrangement.drag.take() {
            app.commit_clip_drag(
                d.anchor,
                match d.mode {
                    DragMode::Move => ClipDragResult::Move {
                        delta: d.delta,
                        rows: d.rows,
                        copy: d.copy,
                    },
                    DragMode::TrimStart => ClipDragResult::TrimStart { delta: d.delta },
                    DragMode::TrimEnd => ClipDragResult::TrimEnd { delta: d.delta },
                    DragMode::Slip => ClipDragResult::Slip { delta: d.delta },
                },
            );
        } else if let Some((a, b)) = app.arrangement.marquee.take() {
            let band = Rect::from_two_pos(a, b);
            let mut picked = Vec::new();
            for (i, track) in app.project().tracks.iter().enumerate() {
                let Some(r) = rows.get(i).copied() else {
                    continue;
                };
                let row = Rect::from_min_max(
                    pos2(rect.left(), r.top + 2.0),
                    pos2(rect.right(), r.top + r.height - 2.0),
                );
                if !band.intersects(row) {
                    continue;
                }
                for clip in &track.clips {
                    let x0 = view.x(ticks_to_beats(clip.start));
                    let x1 = view.x(ticks_to_beats(clip.end()));
                    if band.intersects(Rect::from_min_max(
                        pos2(x0, row.top()),
                        pos2(x1, row.bottom()),
                    )) {
                        picked.push((track.id, clip.id));
                    }
                }
            }
            app.focus_arrangement();
            app.set_clip_selection(picked);
        }
        app.end_gesture();
    }

    if n_tracks == 0 {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            t("track.add"),
            design::paint::empty_state(),
            theme::p().text_dim,
        );
    } else {
        p.text(
            pos2(rect.left() + 8.0, rect.bottom() - 6.0),
            Align2::LEFT_BOTTOM,
            t("clip.new_tip"),
            design::paint::caption(),
            theme::p().text_dim.gamma_multiply(0.7),
        );
    }
}

/// What a finished clip drag asks the application to do.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ClipDragResult {
    Move { delta: Ticks, rows: i32, copy: bool },
    TrimStart { delta: Ticks },
    TrimEnd { delta: Ticks },
    Slip { delta: Ticks },
}

/// Right-clicking empty timeline. Small on purpose: the one thing worth reaching for
/// there is the clipboard.
fn timeline_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    app.focus_arrangement();
    if action_item(app, ui, crate::keymap::Action::Paste, "menu.paste") {
        ui.close();
    }
    if action_item(app, ui, crate::keymap::Action::SelectAll, "menu.select_all") {
        ui.close();
    }
}

/// A menu entry that runs an action, with whatever key is bound to it shown on the right.
///
/// Going through `run_action` is ADR 0010's rule — a menu and a shortcut must reach the
/// same code — and showing the binding is how the shortcut gets discovered at all.
pub fn action_item(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    action: crate::keymap::Action,
    label_key: &str,
) -> bool {
    let shortcut = app.keymap.bindings_for(action).first().map(|b| b.display());
    let clicked = ui
        .add(
            egui::Button::new(t(label_key))
                .shortcut_text(shortcut.unwrap_or_default())
                .wrap_mode(egui::TextWrapMode::Extend),
        )
        .clicked();
    if clicked {
        app.run_action(action);
    }
    clicked
}

/// The clip's own menu. Every entry is something `run_action` can also do, so a shortcut
/// can never reach less than the menu (ADR 0010).
fn clip_menu(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId) {
    if !app.clip_selection().contains(&(track, clip)) {
        app.select_clip(track, clip);
    }
    app.focus_arrangement();
    let (muted, looping) = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .map(|c| (c.muted, c.content_length.is_some()))
        .unwrap_or((false, false));
    use crate::keymap::Action;
    for (action, key) in [
        (Action::SplitClip, "clip.split"),
        (Action::GlueClips, "clip.glue"),
        (Action::Duplicate, "clip.duplicate"),
        (Action::Cut, "menu.cut"),
        (Action::Copy, "menu.copy"),
    ] {
        if action_item(app, ui, action, key) {
            ui.close();
        }
    }
    ui.separator();
    if action_item(
        app,
        ui,
        Action::ToggleClipMute,
        if muted { "clip.unmute" } else { "clip.mute" },
    ) {
        ui.close();
    }
    if action_item(
        app,
        ui,
        Action::ToggleClipLoop,
        if looping {
            "clip.loop_off"
        } else {
            "clip.loop_on"
        },
    ) {
        ui.close();
    }
    ui.separator();
    ui.label(
        egui::RichText::new(t("clip.color"))
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
    ui.horizontal(|ui| {
        for c in cantus_core::Color::PALETTE {
            let (r, resp) = ui.allocate_exact_size(vec2(space::S4, space::S4), Sense::click());
            ui.painter()
                .rect_filled(r, theme::p().corner_radius, theme::track_color(c));
            if resp.hovered() {
                ui.painter().rect_stroke(
                    r,
                    theme::p().corner_radius,
                    Stroke::new(1.0, theme::p().text),
                    StrokeKind::Inside,
                );
            }
            if resp.clicked() {
                app.set_clip_color(Some(c));
                ui.close();
            }
        }
    });
    if ui.button(t("clip.color_reset")).clicked() {
        app.set_clip_color(None);
        ui.close();
    }
    ui.separator();
    if ui.button(t("clip.delete")).clicked() {
        app.delete_clip_selection();
        ui.close();
    }
}
