//! The grid of slots (ADR 0077): only the cells in view painted and sensed; a press on a
//! clip's launcher launches it (and its release releases it — a gate clip), on an empty
//! slot's square stops its track (its dot, armed, records into it); a click chooses the
//! slot (its clip in the piano roll and the clip panel), a double-click opens the clip or,
//! on an empty slot, makes a one-bar MIDI clip there; a drag moves the clip to another
//! slot (Alt or Ctrl: a copy) or, let go over the arrangement, puts it there.

use super::super::layout::{Geometry, Hit};
use super::super::paint::{self, Empty, Launcher, SlotLook};
use super::super::{Pressed, SlotDrag};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::{with_brush, Style};
use cantus_core::session::SlotRef;
use cantus_core::{ClipId, Ticks};
use cantus_engine::messages::SlotPhase;
use egui::{Id, Rect, Sense, Ui};

/// This moment's half of the blink (twice a second, as Live's queued clips).
pub(crate) fn blink_on(ui: &Ui) -> bool {
    (ui.input(|i| i.time) * 4.0) as i64 % 2 == 0
}

/// A slot's launcher as its phase shows it.
pub(crate) fn launcher_of(phase: SlotPhase, blink: bool) -> Launcher {
    match phase {
        SlotPhase::Stopped => Launcher::Idle,
        SlotPhase::Queued { .. } => Launcher::Queued { on: blink },
        SlotPhase::Playing { .. } => Launcher::Playing,
        SlotPhase::Stopping { .. } => Launcher::Stopping { on: blink },
        SlotPhase::RecordQueued { .. } => Launcher::RecordQueued { on: blink },
        SlotPhase::Recording { .. } => Launcher::Recording,
    }
}

/// Where a playing clip is, 0…1 of its loop (of its length without one).
pub(crate) fn progress(
    app: &CantusApp,
    slot: SlotRef,
    phase: SlotPhase,
    now: Ticks,
) -> Option<f32> {
    let c = app.project().slot_clip(slot)?;
    let at = phase.content_at(c, now)?;
    let pass = c.pass().0.max(1) as f32;
    Some(((at - c.clip.offset).0.max(0) as f32 / pass).clamp(0.0, 1.0))
}

/// The waveform of a session clip at `n` columns, from the peak store (kept per clip,
/// revision and width; `None` until the peaks are read).
pub(crate) fn wave_of<'a>(
    app: &'a mut CantusApp,
    ctx: &egui::Context,
    track: cantus_core::TrackId,
    clip: ClipId,
    n: usize,
) -> Option<&'a [(f32, f32)]> {
    let rev = app.bus.revision();
    let fresh = app
        .session_view
        .waves
        .get(&clip)
        .is_some_and(|(r, c, _)| *r == rev && *c == n);
    if !fresh {
        let (c, region) = crate::audio_clips::region(app.project(), (track, clip))?;
        let src = app.project().audio_source(region.source)?;
        let crate::waveform::Lookup::Ready(peaks) =
            app.peaks.lookup(src, app.audio_base(), Some(ctx))
        else {
            return None;
        };
        let (from, to) = crate::audio_clips::window_frames(app.project(), c, src.sample_rate);
        let per = ((to - from) / n.max(1) as f64).max(1e-9);
        let mut cols = Vec::new();
        peaks.columns(crate::waveform::Which::Mono, from, per, n, &mut cols);
        if region.reverse {
            cols.reverse();
        }
        app.session_view.waves.insert(clip, (rev, n, cols));
    }
    app.session_view
        .waves
        .get(&clip)
        .map(|(_, _, w)| w.as_slice())
}

pub(crate) fn grid(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry, s: &Style) {
    let k = s.k;
    let resp = ui.interact(
        geo.grid,
        Id::new("cantus-session-grid"),
        Sense::click_and_drag(),
    );
    // The screen reader hears the grid as the slot the cursor is on.
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, spoken_slot(app)));
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let hover = pointer
        .filter(|p| geo.grid.contains(*p))
        .and_then(|p| geo.hit(p));
    let now = super::super::now(app);
    let blink = blink_on(ui);
    let (cols, rows) = geo.visible();
    // The waveforms of the audio clips in view.
    let n_cols = ((geo.col_w * 0.5) / (2.0 * k)).round().max(8.0) as usize;
    let ctx = ui.ctx().clone();
    for i in cols.clone() {
        for j in rows.clone() {
            let Some(scene) = geo.scenes.get(j) else {
                continue;
            };
            let slot = SlotRef::new(geo.tracks[i], *scene);
            let audio = app
                .project()
                .slot_clip(slot)
                .filter(|c| c.clip.audio.is_some())
                .map(|c| c.id());
            if let Some(id) = audio {
                let _ = wave_of(app, &ctx, slot.track, id, n_cols);
            }
        }
    }
    // Where a dragged clip, an arrangement clip or a sample would land.
    let dropping = app.session_view.drag.is_some_and(|d| d.moving)
        || app.arrangement.moving_clips()
        || app.samples.drag.is_some();
    let drop_at = if dropping {
        hover.and_then(|h| match h {
            Hit::Slot(s) if app.project().slot_clip(s).is_none() => geo.index_of(s),
            _ => None,
        })
    } else {
        None
    };
    let cursor = app.session_view.cursor;
    let focused = app.session_view.focused;
    let renaming = match &app.session_view.rename {
        Some(super::super::Rename::Clip(s, _)) => Some(*s),
        _ => None,
    };
    let dragged = app.session_view.drag.filter(|d| d.moving).map(|d| d.from);
    let clip_rect = geo.grid.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip_rect);
    let mut tints = std::mem::take(&mut app.session_view.tints);
    with_brush(&painter, |b| {
        for i in cols.clone() {
            let track = &app.project().tracks[i];
            for j in rows.clone() {
                let cell = geo.cell(i, j);
                let Some(scene) = geo.scenes.get(j) else {
                    continue;
                };
                let slot = SlotRef::new(track.id, *scene);
                let phase = app.session_view.live.phase(slot);
                let hovered = hover == Some(Hit::Slot(slot)) && !dropping;
                let is_cursor = cursor == Some(slot) && !app.session_view.scene_chosen;
                match track.slot_clip(*scene) {
                    Some(c) => {
                        let colour = crate::theme::track_color(c.clip.color.unwrap_or(track.color));
                        let look = SlotLook {
                            tint: tints.of(colour, s),
                            launcher: launcher_of(phase, blink),
                            progress: progress(app, slot, phase, now),
                            selected: is_cursor,
                            cursor: is_cursor && focused,
                            hovered,
                            renaming: renaming == Some(slot),
                            ghost: dragged == Some(slot),
                        };
                        let wave = app
                            .session_view
                            .waves
                            .get(&c.id())
                            .map(|(_, _, w)| w.as_slice());
                        paint::slot_clip(b, cell, s, &c.clip, wave, look);
                    }
                    None => {
                        let stop = track.slot(*scene).is_none_or(|x| x.stop);
                        let what = match phase {
                            SlotPhase::RecordQueued { .. } => Empty::RecordQueued { on: blink },
                            SlotPhase::Recording { .. } => Empty::Recording,
                            _ if track.armed => Empty::Record,
                            _ if stop => Empty::Stop,
                            _ => Empty::NoStop,
                        };
                        let drop = drop_at == Some((i, j));
                        paint::slot_empty(
                            b,
                            cell,
                            s,
                            what,
                            is_cursor,
                            is_cursor && focused,
                            hovered,
                            drop,
                        );
                    }
                }
            }
        }
        // The grids' session rings.
        for &(t0, s0, w, h) in &app.session_view.rings {
            if t0 >= geo.tracks.len() {
                continue;
            }
            let a = geo.cell(t0, s0);
            let z = geo.cell(
                (t0 + w).min(geo.tracks.len()).saturating_sub(1),
                (s0 + h).saturating_sub(1).min(geo.scenes.len().max(1) - 1),
            );
            paint::ring(b, a.union(z), s);
        }
    });
    app.session_view.tints = tints;
    if dropping {
        ctx.request_repaint();
    }
    // The learn mode: every slot in view is learnable (a pad launches it).
    if app.controllers.learn_mode {
        for i in cols.clone() {
            for j in rows.clone() {
                let Some(scene) = geo.scenes.get(j) else {
                    continue;
                };
                let target = cantus_core::controller::MappingTarget::SessionSlot {
                    track: geo.tracks[i],
                    scene: *scene,
                };
                crate::controllers::learn_spot_in(app, ui, geo.cell(i, j), clip_rect, target);
            }
        }
        return;
    }
    // Outside it, a mapped slot wears the mapped mark.
    for i in cols.clone() {
        for j in rows.clone() {
            let Some(scene) = geo.scenes.get(j) else {
                continue;
            };
            let target = cantus_core::controller::MappingTarget::SessionSlot {
                track: geo.tracks[i],
                scene: *scene,
            };
            if app.is_mapped(&target) {
                crate::controllers::learn_spot_in(app, ui, geo.cell(i, j), clip_rect, target);
            }
        }
    }
    if let Some(Hit::Slot(slot)) = hover {
        let tip = match app.project().slot_clip(slot) {
            Some(_) => "session.slot.clip_tip",
            None if app.project().track(slot.track).is_some_and(|t| t.armed) => {
                "session.slot.record_tip"
            }
            None => "session.slot.empty_tip",
        };
        let name = app
            .project()
            .slot_clip(slot)
            .map(|c| c.clip.name.clone())
            .unwrap_or_default();
        hint::set_text(
            ui.ctx(),
            &name,
            &crate::i18n::t(tip),
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    input(app, ui, &resp, geo, hover);
}

/// The grid's clicks, presses, drags and its menu.
fn input(
    app: &mut CantusApp,
    ui: &mut Ui,
    resp: &egui::Response,
    geo: &Geometry,
    hover: Option<Hit>,
) {
    let k = geo.k;
    let (pressed, released, mods) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_released(),
            i.modifiers,
        )
    });
    let press_pos = ui.input(|i| i.pointer.press_origin());
    if pressed && resp.contains_pointer() {
        if let (Some(Hit::Slot(slot)), Some(p)) = (hover, press_pos) {
            let (i, j) = geo.index_of(slot).expect("hit");
            let cell = geo.cell(i, j);
            let on_launcher = paint::launcher_rect(cell, k).contains(p);
            if on_launcher {
                // The one press everything shares (the mouse, keys, pads, learnt controls).
                super::super::press_slot(app, slot, 1.0);
                app.session_view.pressed = Some(Pressed::Slot(slot));
            }
            choose(app, slot);
        }
    }
    if released {
        // Only a slot's: a scene's launcher is the master column's to let go.
        if let Some(Pressed::Slot(slot)) = app.session_view.pressed {
            app.session_view.pressed = None;
            super::super::release(app, slot);
        }
    }
    if resp.double_clicked() {
        match hover {
            Some(Hit::Slot(slot)) => match app
                .project()
                .slot_clip(slot)
                .map(|c| (c.id(), c.clip.audio.is_some()))
            {
                Some((id, audio)) => {
                    app.select_clip(slot.track, id);
                    if !audio {
                        app.show_pane(crate::views::workspace::Pane::PianoRoll);
                    }
                }
                None => super::super::keys::new_clip(app, slot),
            },
            Some(Hit::NewScene) => super::super::keys::insert_scene(app),
            _ => {}
        }
    }
    // A drag of a clip: decided at the press, moving past a few points.
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let (Some(p), Some(Hit::Slot(slot))) = (press_pos, press_pos.and_then(|p| geo.hit(p))) {
            let (i, j) = geo.index_of(slot).expect("hit");
            let on_launcher = paint::launcher_rect(geo.cell(i, j), k).contains(p);
            if app.project().slot_clip(slot).is_some() && !on_launcher {
                app.session_view.drag = Some(SlotDrag {
                    from: slot,
                    press: p,
                    moving: false,
                });
            }
        }
    }
    if let Some(d) = app.session_view.drag.as_mut() {
        if let Some(p) = ui.input(|i| i.pointer.latest_pos()) {
            if (p - d.press).length() > 4.0 * k {
                d.moving = true;
            }
        }
    }
    // Let go anywhere (over the grid, the arrangement, or nowhere).
    let up = !ui.input(|i| i.pointer.primary_down());
    if up {
        if let Some(d) = app.session_view.drag.take() {
            if d.moving {
                let at = ui.input(|i| i.pointer.latest_pos());
                super::super::drop::finish_slot_drag(app, d.from, at, mods.alt || mods.command);
            }
        }
    }
    if let Some(d) = app.session_view.drag.filter(|d| d.moving) {
        super::super::drop::drag_feedback(app, ui.ctx(), d.from);
    }
    if resp.secondary_clicked() {
        if let Some(Hit::Slot(slot)) = hover {
            choose(app, slot);
        }
        app.session_view.menu_on = hover;
    }
    let subject = app.session_view.menu_on;
    egui::Popup::context_menu(resp)
        .frame(crate::views::clips_chrome::menu_frame())
        .show(|ui| {
            if let Some(h) = subject {
                if super::super::menu::grid_menu(app, ui, h) {
                    ui.close();
                }
            }
        });
    let _ = Rect::NOTHING;
}

/// A slot chosen (the cursor, its clip selected for the piano roll and the panel).
pub(crate) fn choose(app: &mut CantusApp, slot: SlotRef) {
    app.session_view.cursor = Some(slot);
    app.session_view.scene_chosen = false;
    match app.project().slot_clip(slot).map(|c| c.id()) {
        Some(id) => app.select_clip(slot.track, id),
        None => app.select_track(slot.track),
    }
}

/// The cursor's slot in words, for the screen reader: its track, its scene, its clip.
pub(crate) fn spoken_slot(app: &CantusApp) -> String {
    let Some(c) = app.session_view.cursor else {
        return crate::i18n::t("session.pane");
    };
    let p = app.project();
    let track = p.track(c.track).map(|t| t.name.clone()).unwrap_or_default();
    let scene = match (p.session.scene_index(c.scene), p.session.scene(c.scene)) {
        (_, Some(s)) if !s.name.is_empty() => s.name.clone(),
        (Some(j), _) => (j + 1).to_string(),
        _ => String::new(),
    };
    let clip = p.slot_clip(c).map_or_else(
        || crate::i18n::t("session.a11y.empty"),
        |k| k.clip.name.clone(),
    );
    crate::i18n::tf(
        "session.a11y.slot",
        &[("track", &track), ("scene", &scene), ("clip", &clip)],
    )
}
