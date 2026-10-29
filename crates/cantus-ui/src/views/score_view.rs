//! The score on screen (round 32, ADR 0075): a staff under the piano roll (one long
//! system that follows the playhead and scrolls sideways with the wheel) and the Score
//! pane (the selected track's part in systems wrapped to the pane, scrolling down) — the
//! engraving cached per edit and size, notes selected with the piano roll's selection, a
//! click selecting, the keys editing (`crate::score::keys`).

use super::score_paint::{paint, Placement, ScoreInk};
use crate::app::CantusApp;
use crate::kit::Style;
use crate::score::{self, Scope};
use cantus_core::{ClipId, TrackId};
use cantus_score::layout::Format;
use egui::{pos2, vec2, Rect, Sense};

/// Where a score is drawn.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Split,
    Pane,
}

/// The margin round the engraving, points at 100 %.
const PAD: f32 = 10.0;

/// The staff under the piano roll: carve it off the bottom of `full` when it is open (a
/// handle on its top edge resizes it) and return what is left for the piano roll.
pub fn carve(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    clip: ClipId,
    full: Rect,
) -> Rect {
    if !app.score.split_open {
        return full;
    }
    let s = Style::current();
    let max_h = (full.height() - s.s(160.0)).max(s.s(60.0));
    let h = s.s(app.score.split_h).clamp(s.s(60.0), max_h);
    let rect = Rect::from_min_max(pos2(full.left(), full.bottom() - h), full.max);
    let handle = Rect::from_min_max(
        pos2(full.left(), rect.top() - s.s(3.0)),
        pos2(full.right(), rect.top() + s.s(3.0)),
    );
    let hr = ui.interact(handle, ui.id().with("score-split-handle"), Sense::drag());
    if hr.dragged() {
        app.score.split_h = ((h - hr.drag_delta().y) / s.k).clamp(60.0, 2000.0);
    }
    if hr.hovered() || hr.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let bar_h = s.s(super::clips_chrome::TOOLBAR_H);
    let bar = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + bar_h));
    let body_r = Rect::from_min_max(pos2(rect.left(), bar.bottom()), rect.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
    super::score_bar::toolbar(app, &mut child, Scope::Clip(track, clip), Place::Split);
    body(app, ui, body_r, Scope::Clip(track, clip), Place::Split);
    Rect::from_min_max(full.min, pos2(full.right(), rect.top() - s.s(4.0)))
}

/// The Score pane: the selected track's part.
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let Some(track) = app.selected_track else {
        let s = Style::current();
        let r = ui.available_rect_before_wrap();
        crate::kit::with_brush(ui.painter(), |b| {
            b.register(
                r,
                "empty",
                &[crate::kit::Tok::Panel, crate::kit::Tok::TextDim],
                &[],
            );
            b.rect(b.snap_rect(r), crate::kit::Fill::Solid(s.t.panel));
            let tx = crate::kit::text::Text::key(
                "score.none",
                crate::kit::text::Role::Title,
                s.t.text_dim,
            )
            .policy(crate::kit::text::Policy::Elide(
                crate::kit::text::Elide::End,
            ));
            let _ = crate::kit::text::paint(b, r.shrink(16.0), &tx, s.tk());
        });
        return;
    };
    super::score_bar::toolbar(app, ui, Scope::Track(track), Place::Pane);
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    body(app, ui, rect, Scope::Track(track), Place::Pane);
}

fn body(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect, scope: Scope, place: Place) {
    let s = Style::current();
    let pass = ui.ctx().cumulative_pass_nr();
    app.score.drawn_pass = pass;
    let space = s.k
        * match place {
            Place::Split => app.score.split_space,
            Place::Pane => app.score.pane_space,
        };
    let pad = s.s(PAD);
    let format = match place {
        Place::Split => Format::Continuous,
        Place::Pane => Format::Wrap {
            width: ((rect.width() - 2.0 * pad) / space).max(30.0),
        },
    };
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, s.t.well);
    let mut cache = match place {
        Place::Split => app.score.split_cache.take(),
        Place::Pane => app.score.pane_cache.take(),
    };
    let resp = ui.interact(
        rect,
        ui.id().with(("score-body", place == Place::Split)),
        Sense::click(),
    );
    {
        let a: &CantusApp = app;
        let c = score::engraved(a, &mut cache, scope, format);
        let Some(page) = c.engraved.pages.first() else {
            put_back(app, place, cache);
            return;
        };
        // Scroll and follow.
        let content = vec2(
            page.list.width * space + 2.0 * pad,
            page.list.height * space + 2.0 * pad,
        );
        let pos = a.engine.position().0;
        let playing = a.engine.is_playing();
        let (mut sx, mut sy) = match place {
            Place::Split => (a.score.split_scroll, 0.0),
            Place::Pane => (0.0, a.score.pane_scroll),
        };
        if resp.hovered() {
            let (d, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.command));
            if !zoom {
                match place {
                    Place::Split => sx -= if d.x != 0.0 { d.x } else { d.y },
                    Place::Pane => sy -= d.y,
                }
            }
        }
        if a.score.follow && playing {
            if let Some((_, si, x)) = c.engraved.locate(pos) {
                match place {
                    Place::Split => {
                        let px = x * space + pad;
                        if px - sx > rect.width() * 0.8 || px < sx {
                            sx = px - rect.width() * 0.25;
                        }
                    }
                    Place::Pane => {
                        let sys = &page.systems[si];
                        let (top, bottom) = (
                            (sys.top - 6.0) * space + pad,
                            (sys.bottom + 6.0) * space + pad,
                        );
                        if bottom - sy > rect.height() || top < sy {
                            sy = top;
                        }
                    }
                }
            }
        }
        sx = sx.clamp(0.0, (content.x - rect.width()).max(0.0));
        sy = sy.clamp(0.0, (content.y - rect.height()).max(0.0));
        let pl = Placement {
            origin: rect.min + vec2(pad - sx, pad - sy),
            space,
        };
        let ink = ScoreInk {
            ink: s.t.text,
            selected: s.t.selection,
        };
        let sel = &a.selected_notes;
        paint(
            &painter,
            &page.list,
            &pl,
            &ink,
            &|id| sel.iter().any(|n| n.0 == id),
            rect,
        );
        // The playhead, through the system it is in.
        if let Some((_, si, x)) = c.engraved.locate(pos) {
            let sys = &page.systems[si];
            let a0 = pl.at(x, sys.top - 2.0);
            let a1 = pl.at(x, sys.bottom + 2.0);
            painter.line_segment([a0, a1], egui::Stroke::new(1.5 * s.k, s.t.playhead));
        }
        // The entry cursor.
        if let Some(e) = a.score.entry {
            if let Some((_, si, x)) = c.engraved.locate(e.at) {
                let sys = &page.systems[si];
                let r = Rect::from_min_max(
                    pl.at(x - 0.6, sys.top - 1.5),
                    pl.at(x + 1.8, sys.bottom + 1.5),
                );
                painter.rect_filled(
                    r,
                    2.0 * s.k,
                    crate::kit::color::with_alpha(s.t.selection, 0.22),
                );
            }
        }
        let click = resp
            .clicked()
            .then(|| resp.interact_pointer_pos())
            .flatten();
        let shift = ui.input(|i| i.modifiers.shift);
        let hit = click.map(|p| {
            let (x, y) = pl.back(p);
            let note = page.list.note_at(x, y, 0.8);
            let tick = page
                .systems
                .iter()
                .find(|sy| y >= sy.top - 4.0 && y <= sy.bottom + 4.0)
                .and_then(|sy| sy.tick_at(x));
            (note, tick)
        });
        let zoom = resp
            .hovered()
            .then(|| ui.input(|i| i.modifiers.command.then_some(i.smooth_scroll_delta.y)))
            .flatten();
        let _ = a;
        match place {
            Place::Split => app.score.split_scroll = sx,
            Place::Pane => app.score.pane_scroll = sy,
        }
        if let Some(dz) = zoom.filter(|d| *d != 0.0) {
            let v = match place {
                Place::Split => &mut app.score.split_space,
                Place::Pane => &mut app.score.pane_space,
            };
            *v = (*v * (1.0 + dz * 0.002)).clamp(score::SPACE_RANGE.0, score::SPACE_RANGE.1);
        }
        if let Some((note, tick)) = hit {
            app.score.focused = true;
            click_at(app, scope, note, tick, shift);
        }
    }
    // A press elsewhere takes the keys away from the score.
    if ui.input(|i| i.pointer.primary_pressed())
        && !ui.rect_contains_pointer(rect)
        && !app.score.bar_hovered
    {
        app.score.focused = false;
    }
    put_back(app, place, cache);
}

fn put_back(app: &mut CantusApp, place: Place, cache: Option<score::Cached>) {
    match place {
        Place::Split => app.score.split_cache = cache,
        Place::Pane => app.score.pane_cache = cache,
    }
}

/// A click on the staff: a notehead selects its note (Shift adds or takes it away; in a
/// track's score, its clip becomes the selected one); while entering notes, the cursor
/// moves to the click's beat.
fn click_at(app: &mut CantusApp, scope: Scope, note: Option<u64>, tick: Option<i64>, shift: bool) {
    match note {
        Some(id) => {
            let id = cantus_core::NoteId(id);
            if let Scope::Track(t) = scope {
                let clip = app
                    .project()
                    .track(t)
                    .and_then(|tr| tr.clips.iter().find(|c| c.notes.iter().any(|n| n.id == id)))
                    .map(|c| c.id);
                if let Some(c) = clip {
                    if app.selected_clip != Some(c) {
                        app.selected_clip = Some(c);
                        app.selected_notes.clear();
                    }
                }
            }
            if shift {
                if let Some(i) = app.selected_notes.iter().position(|n| *n == id) {
                    app.selected_notes.remove(i);
                } else {
                    app.selected_notes.push(id);
                }
            } else {
                app.selected_notes = vec![id];
            }
        }
        None => {
            if !shift {
                app.selected_notes.clear();
            }
            let map = app.project().tempo_map();
            if let (Some(e), Some(t)) = (app.score.entry.as_mut(), tick) {
                // To the beat (or the value's grid) under the click.
                let bar = map.bar_start(cantus_core::Ticks(t)).0;
                let step = e.dur.ticks().min(cantus_core::time::PPQ);
                e.at = bar + ((t - bar) / step) * step;
            }
        }
    }
}
