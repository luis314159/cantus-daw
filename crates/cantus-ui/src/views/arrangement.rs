//! Arrangement view: track headers on the left, a timeline with clips on the right.

use super::{beats_to_ticks, ticks_to_beats};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::theme;
use cantus_core::{ClipId, Command, MergeKey, Ticks, TrackId};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, PointerButton, Rect, Sense, Stroke,
    StrokeKind,
};

const RULER_H: f32 = 22.0;

#[derive(Debug)]
pub struct ArrangementView {
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    pub track_height: f32,
    pub header_width: f32,
    drag: Option<ClipDrag>,
    rename: Option<(TrackId, String)>,
}

#[derive(Debug)]
struct ClipDrag {
    track: TrackId,
    clip: ClipId,
    orig_start: Ticks,
    orig_length: Ticks,
    press_x: f32,
    resize: bool,
}

impl Default for ArrangementView {
    fn default() -> Self {
        ArrangementView {
            px_per_beat: 26.0,
            scroll_beats: 0.0,
            track_height: 64.0,
            header_width: 220.0,
            drag: None,
            rename: None,
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

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
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
                ui.label(egui::RichText::new(&track.name).color(color).strong());
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
            ui.spacing_mut().slider_width = 90.0;
            let mut gain = track.gain_db;
            let g = ui.add(
                egui::Slider::new(&mut gain, -60.0..=12.0)
                    .show_value(false)
                    .suffix(" dB"),
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
                egui::RichText::new(format!("{gain:+.1}"))
                    .monospace()
                    .small()
                    .color(theme::p().text_dim),
            );
            ui.spacing_mut().slider_width = 50.0;
            let mut pan = track.pan;
            let pr = ui.add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false));
            if pr.changed() {
                app.exec_merging(
                    Command::SetTrackPan { id: *id, pan },
                    MergeKey::of(&("pan", id.0)),
                );
            }
            if pr.drag_stopped() {
                app.end_gesture();
            }
            if pr.double_clicked() {
                app.exec(Command::SetTrackPan { id: *id, pan: 0.0 });
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
                FontId::proportional(10.0),
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

    // Clips.
    let mut hit: Option<(TrackId, ClipId, bool)> = None;
    let pointer = resp.hover_pos();
    let selected_clip = app.selected_clip;
    for (i, track) in app.project().tracks.iter().enumerate() {
        let y = body_top + i as f32 * th;
        let color = theme::track_color(track.color);
        for clip in &track.clips {
            let x0 = view.x(ticks_to_beats(clip.start));
            let x1 = view.x(ticks_to_beats(clip.end()));
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let r = Rect::from_min_max(pos2(x0, y + 4.0), pos2(x1.max(x0 + 2.0), y + th - 4.0));
            let sel = selected_clip == Some(clip.id);
            p.rect_filled(
                r,
                CornerRadius::same(3),
                color.gamma_multiply(if sel { 0.55 } else { 0.35 }),
            );
            p.rect_stroke(
                r,
                CornerRadius::same(3),
                Stroke::new(
                    if sel { 2.0 } else { 1.0 },
                    if sel { theme::p().gold_bright } else { color },
                ),
                StrokeKind::Inside,
            );
            // Note miniature.
            if !clip.notes.is_empty() {
                let (lo, hi) = clip.notes.iter().fold((127u8, 0u8), |(lo, hi), n| {
                    (lo.min(n.pitch), hi.max(n.pitch))
                });
                let span = (hi - lo).max(12) as f32;
                let inner = r.shrink2(vec2(2.0, 14.0));
                for n in &clip.notes {
                    let nx0 = view.x(ticks_to_beats(clip.start + n.start));
                    let nx1 = view.x(ticks_to_beats(clip.start + n.end()));
                    if nx1 < rect.left() || nx0 > rect.right() {
                        continue;
                    }
                    let ny = inner.bottom() - (n.pitch - lo) as f32 / span * inner.height();
                    p.rect_filled(
                        Rect::from_min_max(
                            pos2(nx0.max(r.left()), ny - 1.0),
                            pos2(nx1.min(r.right()).max(nx0 + 1.0), ny + 1.0),
                        ),
                        0.0,
                        color.gamma_multiply(1.4),
                    );
                }
            }
            let clip_p = p.with_clip_rect(r);
            clip_p.text(
                pos2(r.left() + 5.0, r.top() + 3.0),
                Align2::LEFT_TOP,
                &clip.name,
                FontId::proportional(11.0),
                theme::p().text,
            );
            if let Some(pos) = pointer {
                if r.contains(pos) {
                    hit = Some((track.id, clip.id, pos.x > r.right() - 8.0));
                }
            }
        }
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

    // Interaction.
    let press = ui.input(|i| i.pointer.press_origin());
    let track_ids: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    let track_at = move |y: f32| -> Option<TrackId> {
        let i = ((y - body_top) / th).floor();
        if i < 0.0 {
            return None;
        }
        track_ids.get(i as usize).copied()
    };
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.y < body_top {
                let beats = view.beats(pos.x).max(0.0);
                let tick = beats_to_ticks(beats).snap_floor(Ticks::QUARTER);
                app.seek(tick);
            } else if let Some((tid, cid, _)) = hit {
                app.select_clip(tid, cid);
            } else if let Some(tid) = track_at(pos.y) {
                app.select_track(tid);
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
    if resp.secondary_clicked() {
        if let Some((tid, cid, _)) = hit {
            app.delete_clip(tid, cid);
        }
    }
    if resp.drag_started_by(PointerButton::Primary) {
        if let Some(pos) = press {
            if pos.y < body_top {
                // Drag in ruler: set loop region.
                app.arrangement.drag = None;
            } else if let Some((tid, cid, resize)) = hit {
                if let Some(clip) = app.project().track(tid).and_then(|t| t.clip(cid)) {
                    app.arrangement.drag = Some(ClipDrag {
                        track: tid,
                        clip: cid,
                        orig_start: clip.start,
                        orig_length: clip.length,
                        press_x: pos.x,
                        resize,
                    });
                    app.select_clip(tid, cid);
                }
            }
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let (Some(pos), Some(d)) = (resp.interact_pointer_pos(), app.arrangement.drag.as_ref()) {
            let delta_beats = (pos.x - d.press_x) / view.px_per_beat;
            let delta = beats_to_ticks(delta_beats).snap(Ticks::QUARTER);
            let (track, clip, resize, os, ol) =
                (d.track, d.clip, d.resize, d.orig_start, d.orig_length);
            let (start, length) = if resize {
                (os, (ol + delta).max(Ticks::QUARTER))
            } else {
                ((os + delta).max(Ticks::ZERO), ol)
            };
            app.exec_merging(
                Command::SetClipBounds {
                    track,
                    clip,
                    start,
                    length,
                },
                MergeKey::of(&("clip", clip.0)),
            );
        } else if let Some(origin) = press {
            if origin.y < body_top {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let a = beats_to_ticks(view.beats(origin.x).max(0.0)).snap(Ticks::QUARTER);
                    let b = beats_to_ticks(view.beats(pos.x).max(0.0)).snap(Ticks::QUARTER);
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
        app.arrangement.drag = None;
        app.end_gesture();
    }

    if n_tracks == 0 {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            t("track.add"),
            FontId::proportional(13.0),
            theme::p().text_dim,
        );
    } else {
        p.text(
            pos2(rect.left() + 8.0, rect.bottom() - 6.0),
            Align2::LEFT_BOTTOM,
            t("clip.new_tip"),
            FontId::proportional(10.0),
            theme::p().text_dim.gamma_multiply(0.7),
        );
    }
    let _ = Color32::WHITE;
}
