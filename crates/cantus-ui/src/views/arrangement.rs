//! Arrangement view: track headers on the left, a timeline with clips on the right.

use super::{beats_to_ticks, ticks_to_beats, GRIDS};
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::{fader, Fader};
use cantus_core::{ClipId, Command, MergeKey, Ticks, TrackId};
use egui::{pos2, vec2, Align2, CornerRadius, PointerButton, Rect, Sense, Stroke, StrokeKind};

const RULER_H: f32 = 22.0;

#[derive(Debug)]
pub struct ArrangementView {
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    pub track_height: f32,
    pub header_width: f32,
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
            track_height: 64.0,
            header_width: 220.0,
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
        self.geometry
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

    let th = app.arrangement.track_height;
    let ids: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    let mut to_delete = None;
    for (i, id) in ids.iter().enumerate() {
        let (row, resp) = ui.allocate_exact_size(vec2(rect.width(), th), Sense::click());
        let selected = app.selected_track == Some(*id);
        let track = match app.project().track(*id) {
            Some(t) => t.clone(),
            None => continue,
        };
        let p = ui.painter();
        let bg = if selected {
            theme::p().panel_raised
        } else if i % 2 == 0 {
            theme::p().panel
        } else {
            theme::p().row_white
        };
        p.rect_filled(row, 0.0, bg);
        p.rect_filled(
            Rect::from_min_size(row.min, vec2(4.0, th)),
            0.0,
            theme::track_color(track.color),
        );
        p.hline(
            row.x_range(),
            row.bottom() - 0.5,
            Stroke::new(1.0, theme::p().grid),
        );
        if resp.clicked() {
            app.select_track(*id);
        }
        if resp.double_clicked() {
            app.arrangement.rename = Some((*id, track.name.clone()));
        }
        resp.context_menu(|ui| {
            if ui.button(t("track.delete")).clicked() {
                to_delete = Some(*id);
                ui.close();
            }
        });

        let inner = row.shrink2(vec2(10.0, 4.0));
        let mut r = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        r.horizontal(|ui| {
            if let Some((rid, name)) = app.arrangement.rename.as_mut().filter(|(rid, _)| rid == id)
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
                let color = if selected {
                    theme::p().gold_bright
                } else {
                    theme::p().text
                };
                ui.label(
                    egui::RichText::new(&track.name)
                        .text_style(design::strong())
                        .color(color),
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
                    app.exec(Command::SetTrackSolo {
                        id: *id,
                        soloed: solo,
                    });
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
                    app.exec(Command::SetTrackMute { id: *id, muted });
                }
            });
        });
        r.horizontal(|ui| {
            let mut gain = track.gain_db;
            // A track header row has no space for a scale, but it keeps the unity mark,
            // so "is this track above or below unity" is still readable at a glance.
            let g = ui.add(
                Fader::new(&mut gain)
                    .horizontal(90.0)
                    .compact()
                    .name(&track.name),
            );
            if g.changed() {
                app.exec_merging(
                    Command::SetTrackGain {
                        id: *id,
                        gain_db: gain,
                    },
                    MergeKey::of(&("gain", id.0)),
                );
            }
            if g.drag_stopped() {
                app.end_gesture();
            }
            ui.label(
                egui::RichText::new(fader::read_out(gain))
                    .text_style(design::mono_small())
                    .color(theme::p().text_dim),
            );
            let mut pan = track.pan;
            let pr = ui.add(
                crate::widgets::Bar::pan(&mut pan)
                    .size(56.0)
                    .name(t("track.pan")),
            );
            if pr.changed() {
                app.exec_merging(
                    Command::SetTrackPan { id: *id, pan },
                    MergeKey::of(&("pan", id.0)),
                );
            }
            if pr.drag_stopped() {
                app.end_gesture();
            }
        });
        // Peak meter strip.
        let meters = app.engine.meters();
        if let Some(idx) = app.project().track_index(*id) {
            if idx < meters.track_count {
                let peak = meters.tracks[idx];
                let frac = ((cantus_core::linear_to_db(peak) + 60.0) / 60.0).clamp(0.0, 1.0);
                let bar = Rect::from_min_size(
                    pos2(inner.left(), row.bottom() - 6.0),
                    vec2(inner.width() * frac, 3.0),
                );
                ui.painter().rect_filled(bar, 1.0, theme::meter_color(peak));
            }
        }
    }
    if let Some(id) = to_delete {
        app.delete_track(id);
    }
}

/// Where the timeline was drawn, in screen coordinates.
///
/// Recorded every frame so that anything outside `show` — a test aiming at a clip, and
/// later anything that wants to scroll the view to a tick — can map between ticks and
/// pixels by *asking* rather than by mirroring the layout constants. A test that
/// recomputes the layout is a test that breaks the day a toolbar is added, which is
/// exactly what happened.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TimelineGeometry {
    pub left: f32,
    pub body_top: f32,
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    pub track_height: f32,
}

impl TimelineGeometry {
    /// Screen x of a position in beats.
    pub fn x(&self, beats: f32) -> f32 {
        self.left + (beats - self.scroll_beats) * self.px_per_beat
    }

    /// Screen y of the middle of a track's row.
    pub fn row_y(&self, row: usize) -> f32 {
        self.body_top + (row as f32 + 0.5) * self.track_height
    }
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

    // Rows.
    let th = app.arrangement.track_height;
    let body_top = rect.top() + RULER_H;
    app.arrangement.geometry = Some(TimelineGeometry {
        left,
        body_top,
        px_per_beat: view.px_per_beat,
        scroll_beats: view.scroll_beats,
        track_height: th,
    });
    let n_tracks = app.project().tracks.len();
    for i in 0..n_tracks {
        let y = body_top + i as f32 * th;
        let row = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + th));
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
    // Loop region.
    if let Some(l) = &app.project().loop_region {
        let x0 = view.x(ticks_to_beats(l.start));
        let x1 = view.x(ticks_to_beats(l.end));
        let alpha = if app.project().loop_enabled { 0.8 } else { 0.3 };
        p.rect_filled(
            Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.top() + 4.0)),
            0.0,
            theme::p().gold.gamma_multiply(alpha),
        );
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
        let y = body_top + i as f32 * th;
        let track_color = theme::track_color(track.color);
        for clip in &track.clips {
            let r = (track.id, clip.id);
            let (dstart, dlen, drows) =
                app.arrangement
                    .preview(&selection, r)
                    .unwrap_or((Ticks::ZERO, Ticks::ZERO, 0));
            let start = clip.start + dstart;
            let length = (clip.length + dlen).max(Ticks::SIXTEENTH);
            let row_y = y + drows as f32 * th;
            let x0 = view.x(ticks_to_beats(start));
            let x1 = view.x(ticks_to_beats(start + length));
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let color = clip.color.map(theme::track_color).unwrap_or(track_color);
            let body = Rect::from_min_max(
                pos2(x0, row_y + 4.0),
                pos2(x1.max(x0 + 2.0), row_y + th - 4.0),
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
                let inner = body.shrink2(vec2(2.0, 14.0));
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
    let track_ids: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    let row_of = move |y: f32| -> Option<usize> {
        let i = ((y - body_top) / th).floor();
        (i >= 0.0)
            .then_some(i as usize)
            .filter(|i| *i < track_ids.len())
    };
    let track_at = {
        let ids = app
            .project()
            .tracks
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>();
        move |y: f32| -> Option<TrackId> {
            let i = ((y - body_top) / th).floor();
            if i < 0.0 {
                return None;
            }
            ids.get(i as usize).copied()
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
            } else if let Some(origin) = press {
                if origin.y < body_top {
                    let a = snap_to(beats_to_ticks(view.beats(origin.x).max(0.0)), mods.command);
                    let b = snap_to(beats_to_ticks(view.beats(pos.x).max(0.0)), mods.command);
                    let (s, e) = if a <= b { (a, b) } else { (b, a) };
                    if e > s {
                        app.exec_merging(
                            Command::SetLoopRegion {
                                region: Some(cantus_core::LoopRegion { start: s, end: e }),
                            },
                            MergeKey::of(&"loop"),
                        );
                    }
                }
            }
        }
    }
    if resp.drag_stopped() {
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
                let y = body_top + i as f32 * th;
                let row = Rect::from_min_max(
                    pos2(rect.left(), y + 4.0),
                    pos2(rect.right(), y + th - 4.0),
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
