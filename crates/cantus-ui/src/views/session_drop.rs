//! What lands in the grid and what leaves it (ADR 0077): a slot's clip dragged to another
//! slot (moved; Alt or Ctrl: copied) or onto the arrangement (put there, on its track, at
//! the time under the pointer); arrangement clips dragged into a slot (each track's into
//! its own column, from the slot under the pointer down); a sample from the browser into
//! an empty slot (an audio clip looping its bars). Each one undo step.

use super::layout::Hit;
use crate::app::CantusApp;
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Fill, Style};
use cantus_core::session::{edit, SessionClip, SessionCommand, SlotRef};
use cantus_core::{AudioRegion, AudioSource, AudioSourceId, Command, Ticks};
use egui::{pos2, vec2, Align2, Id, Pos2, Rect};
use std::path::Path;

/// The empty slot under `p` in the last frame's grid.
pub fn slot_at(app: &CantusApp, p: Pos2) -> Option<SlotRef> {
    match app.session_view.geo.as_ref()?.hit(p)? {
        Hit::Slot(s) => Some(s),
        _ => None,
    }
}

/// Where a clip let go of over the arrangement lands: the time under the pointer, snapped
/// as the arrangement snaps.
fn arrangement_at(app: &CantusApp, p: Pos2) -> Option<Ticks> {
    let g = app.arrangement.geometry()?;
    let inside = p.x >= g.left && p.x <= g.left + g.width && p.y >= g.body_top;
    if !inside {
        return None;
    }
    let mut at = crate::views::beats_to_ticks(g.beats(p.x)).max(Ticks::ZERO);
    if app.arrangement.snap {
        at = crate::views::bars::snap_floor(app.project(), at, app.arrangement.grid);
    }
    Some(at)
}

/// A slot's clip let go of at `at`.
pub fn finish_slot_drag(app: &mut CantusApp, from: SlotRef, at: Option<Pos2>, copy: bool) {
    let Some(p) = at else { return };
    if let Some(to) = slot_at(app, p) {
        if to == from || app.project().slot_clip(to).is_some() {
            return;
        }
        let ok = if copy {
            let cmd = super::with_ids(app, |pr, ids| edit::duplicate_slot(pr, from, to, ids));
            cmd.is_some_and(|c| app.exec(c))
        } else {
            super::exec(app, SessionCommand::MoveSlotClip { from, to })
        };
        if ok {
            super::input::grid_choose(app, to);
        }
        return;
    }
    if let Some(t) = arrangement_at(app, p) {
        super::keys::to_arrangement(app, from, Some(t));
    }
}

/// While a slot's clip is dragged: a chip with its name at the pointer.
pub fn drag_feedback(app: &CantusApp, ctx: &egui::Context, from: SlotRef) {
    let Some(p) = ctx.input(|i| i.pointer.latest_pos()) else {
        return;
    };
    let Some(c) = app.project().slot_clip(from) else {
        return;
    };
    let s = Style::current();
    let k = s.k;
    let colour = app
        .project()
        .track(from.track)
        .map(|t| crate::theme::track_color(c.clip.color.unwrap_or(t.color)))
        .unwrap_or(s.t.selection);
    let lands = slot_at(app, p).is_some_and(|x| app.project().slot_clip(x).is_none())
        || arrangement_at(app, p).is_some();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("cantus-session-drag"),
    ));
    with_brush(&painter, |b| {
        let label = Text::new(&c.clip.name, Role::Body, s.t.text);
        let w = text::measure(b.ctx(), &label, s.tk()).x.min(200.0 * k) + 20.0 * k;
        let chip = Rect::from_min_size(p + vec2(12.0 * k, 10.0 * k), vec2(w, 22.0 * k));
        let a = if lands { 1.0 } else { 0.6 };
        b.rrect(
            chip,
            s.r(4.0),
            Fill::Solid(with_alpha(mix(s.t.panel_raised, colour, 0.35), a)),
        );
        b.rrect_stroke(chip, s.r(4.0), b.px(), with_alpha(colour, a));
        let tr = Rect::from_min_max(
            pos2(chip.left() + 8.0 * k, chip.top()),
            pos2(chip.right() - 6.0 * k, chip.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &label
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    ctx.request_repaint();
}

/// The arrangement's clip drag let go of over the grid (a marked line in the arrangement
/// asks): the selection's clips into slots from the one under the pointer down — each
/// track's into its own column. `true` when the grid took them (the arrangement then moves
/// nothing).
pub fn from_arrangement(app: &mut CantusApp, ctx: &egui::Context) -> bool {
    let Some(p) = ctx.input(|i| i.pointer.latest_pos()) else {
        return false;
    };
    let Some(slot) = slot_at(app, p) else {
        return false;
    };
    let clips: Vec<(cantus_core::TrackId, cantus_core::ClipId)> = app
        .clip_selection()
        .into_iter()
        .filter(|(t, c)| {
            app.project()
                .track(*t)
                .is_some_and(|tr| tr.clips.iter().any(|k| k.id == *c))
        })
        .collect();
    if clips.is_empty() {
        return false;
    }
    let first = app.project().session.scene_index(slot.scene).unwrap_or(0);
    let cmd = super::with_ids(app, |pr, ids| {
        edit::slots_from_arrangement(pr, &clips, first, ids)
    });
    match cmd {
        Some(c) => {
            app.exec(c);
            true
        }
        None => false,
    }
}

/// A sample from the browser let go of over the grid: an audio clip in the empty slot
/// under the pointer, as long as the file at the song's tempo, looping its bars.
pub fn sample(app: &mut CantusApp, path: &Path, pos: Pos2) {
    let Some(slot) = slot_at(app, pos) else {
        return;
    };
    if app.project().slot_clip(slot).is_some() {
        app.set_status(crate::i18n::t("session.status.slot_taken"));
        return;
    }
    let f = match crate::audio_files::read(path) {
        Ok(f) => f,
        Err(e) => {
            let file = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            app.set_error(crate::i18n::tf(
                "audioclips.status.import_failed",
                &[("file", &file), ("error", &e)],
            ));
            return;
        }
    };
    let base = app.audio_base().map(Path::to_path_buf);
    let id = AudioSourceId(app.bus.alloc_id());
    let frames = f.data.len_frames() as u64;
    let source = AudioSource {
        id,
        name: f.name.clone(),
        path: crate::audio_files::stored_path(base.as_deref(), &f.path),
        sample_rate: f.data.rate,
        channels: f.data.channels,
        frames,
        sha256: None,
        recorded: false,
    };
    let length = crate::audio_files::length_ticks(app.project(), Ticks::ZERO, frames, f.data.rate)
        .max(Ticks(1));
    let mut clip = app.bus.new_clip(f.name.clone(), Ticks::ZERO, length);
    clip.audio = Some(AudioRegion::of(id));
    clip.content_length = Some(length);
    let commands = vec![
        Command::InsertAudioSource { source },
        Command::Session(SessionCommand::InsertSlotClip {
            slot,
            clip: Box::new(SessionClip::new(clip)),
        }),
    ];
    if app.exec(Command::Batch {
        label_key: "cmd.session.sample".into(),
        commands,
    }) {
        app.peaks.insert_data(&f.path, f.data.clone());
        super::input::grid_choose(app, slot);
    }
}
