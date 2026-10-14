//! The arrangement's **paint tool** (round 26, ADR 0037): FL Studio's playlist paint tool
//! (B), painting the clip browser's chosen tile.
//!
//! With the tool out, the lanes take the pointer (an overlay sensed above the timeline):
//! a **click** places a linked copy at the snapped time on the row; a **drag** paints copies
//! end to end along the row it was pressed on, from the snapped press point one clip
//! length apart, as far as the pointer goes (either way, never before the song's start),
//! skipping a place where a copy already starts; a **right-drag** (or a drag with Alt)
//! erases the copies of the source it passes over on that row — only those: FL's paint
//! tool erases any clip, Cantus's erases only the clip it paints, so a stroke never takes
//! a neighbour along. Each stroke is one undo step, made when it ends; while it lasts the
//! copies to be placed are drawn as ghosts and those to be erased are crossed out. **Esc**
//! puts the tool away and lets the source go. The hint bar names the source.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::tf;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::{with_brush, Fill, Style};
use crate::views::arrangement::TimelineGeometry;
use cantus_core::{Ticks, TrackId};
use egui::{pos2, Id, PointerButton, Rect, Sense, Ui};

/// A stroke in progress: on which row, from where (beats, where it was pressed) to where
/// the pointer is now, painting or erasing.
#[derive(Copy, Clone, Debug)]
pub struct Stroke {
    erase: bool,
    row: usize,
    track: TrackId,
    from: f32,
    to: f32,
}

/// What a stroke would do, in ticks: the starts of the copies to paint, or the range to
/// erase.
fn starts(app: &CantusApp, source: ClipRef, st: &Stroke) -> Vec<Ticks> {
    let Some(clip) = crate::clips::clip_of(app.project(), source) else {
        return Vec::new();
    };
    let len = clip.length.max(Ticks(1));
    let snap = |t: Ticks| {
        if app.arrangement.snap {
            t.snap_floor(app.arrangement.grid.max(Ticks(1)))
        } else {
            t
        }
    };
    let first = snap(crate::views::beats_to_ticks(st.from).max(Ticks::ZERO));
    let to = crate::views::beats_to_ticks(st.to).max(Ticks::ZERO);
    let mut out = vec![first];
    if to >= first {
        let mut at = first + len;
        while at <= to {
            out.push(at);
            at += len;
        }
    } else {
        // Leftward too, as far as the copy under the pointer (its span holds it).
        let mut at = first - len;
        while !at.is_negative() && at + len > to {
            out.push(at);
            at -= len;
        }
    }
    out
}

fn range(st: &Stroke) -> (Ticks, Ticks) {
    let a = crate::views::beats_to_ticks(st.from.min(st.to)).max(Ticks::ZERO);
    let b = crate::views::beats_to_ticks(st.from.max(st.to)).max(Ticks::ZERO);
    (a, b)
}

/// The paint tool over the lanes, once a frame after the timeline (so it is on top of it).
pub fn paint_tool(app: &mut CantusApp, ui: &mut Ui, lanes: Rect, geo: &TimelineGeometry) {
    if !app.clip_browser.painting {
        app.clip_browser.paint_ghosts.clear();
        return;
    }
    let source = app
        .clip_browser
        .brush
        .filter(|b| crate::clips::clip_of(app.project(), *b).is_some());
    let Some(source) = source else {
        app.clip_browser.set_painting(false);
        app.clip_browser.brush = None;
        return;
    };
    // Esc is not read here: it reaches the tool through ClearSelection, which gives it its
    // claim only when the arrangement has the edit focus and nothing else takes Esc.
    app.clip_browser.paint_ghosts.clear();
    let resp = ui.interact(
        lanes,
        Id::new("cantus-arrangement-paint"),
        Sense::click_and_drag(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        let name = crate::clips::clip_of(app.project(), source)
            .map(|c| c.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| crate::i18n::t("links.picker.unnamed"));
        let text = tf("links.paint.hint", &[("name", &name)]);
        hint::set_text(ui.ctx(), &text, "", Caps::NONE);
    }
    let (origin, alt) = ui.input(|i| (i.pointer.press_origin(), i.modifiers.alt));
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let primary =
        resp.drag_started_by(PointerButton::Primary) || resp.clicked_by(PointerButton::Primary);
    let secondary =
        resp.drag_started_by(PointerButton::Secondary) || resp.clicked_by(PointerButton::Secondary);
    if app.clip_browser.stroke.is_none() && (primary || secondary) {
        app.clip_browser.stroke = begin(app, geo, source, origin.or(pointer), secondary || alt);
        if app.clip_browser.stroke.is_some() {
            // A stroke is an arrangement gesture: B and Delete act on the arrangement after.
            app.focus_arrangement();
        }
    }
    if let (Some(s), Some(p)) = (app.clip_browser.stroke.as_mut(), pointer) {
        s.to = geo.beats(p.x);
    }
    let done = resp.drag_stopped() || resp.clicked() || resp.secondary_clicked();
    let Some(stroke) = app.clip_browser.stroke else {
        return;
    };
    if done {
        app.clip_browser.stroke = None;
        if stroke.erase {
            let (a, b) = range(&stroke);
            app.erase_linked_copies(source, stroke.track, a, b);
        } else {
            let at = starts(app, source, &stroke);
            app.paint_linked_copies(source, stroke.track, &at);
        }
        return;
    }
    let ghosts = preview(app, ui, lanes, geo, source, &stroke);
    app.clip_browser.paint_ghosts = ghosts;
}

/// A stroke pressed at `at`: from the time there, on the row there — or, for an automation
/// source, on the track it automates, where its copies land (and so its ghosts and its
/// erasing).
fn begin(
    app: &CantusApp,
    geo: &TimelineGeometry,
    source: ClipRef,
    at: Option<egui::Pos2>,
    erase: bool,
) -> Option<Stroke> {
    let o = at?;
    let under = geo.row_at(o.y)?;
    let automation = crate::clips::clip_of(app.project(), source)?
        .automation
        .is_some();
    let row = if automation {
        app.project().track_index(source.0)?
    } else {
        under
    };
    let track = app.project().tracks.get(row)?.id;
    let b = geo.beats(o.x);
    Some(Stroke {
        erase,
        row,
        track,
        from: b,
        to: b,
    })
}

/// While a stroke lasts: ghosts where copies would be painted (only those the stroke will
/// place — none over a copy already there), the copies to be erased crossed out. Returns
/// where the ghosts were drawn.
fn preview(
    app: &CantusApp,
    ui: &Ui,
    lanes: Rect,
    geo: &TimelineGeometry,
    source: ClipRef,
    st: &Stroke,
) -> Vec<Rect> {
    let s = Style::current();
    let k = s.k;
    let mut ghosts = Vec::new();
    let Some(row) = geo.rows.get(st.row) else {
        return ghosts;
    };
    let Some(clip) = crate::clips::clip_of(app.project(), source) else {
        return ghosts;
    };
    let colour = app
        .project()
        .track(source.0)
        .map(|t| crate::theme::track_color(clip.color.unwrap_or(t.color)))
        .unwrap_or(s.t.selection);
    let x = |t: Ticks| geo.x(crate::views::ticks_to_beats(t));
    let at = |start: Ticks, len: Ticks| {
        Rect::from_min_max(
            pos2(x(start), row.top + 1.0),
            pos2(x(start + len), row.top + row.height - 1.0),
        )
        .intersect(lanes)
    };
    let painter = ui.painter().with_clip_rect(lanes);
    with_brush(&painter, |b| {
        if st.erase {
            let (lo, hi) = range(st);
            let link = clip.link;
            if let Some(t) = app.project().track(st.track) {
                for c in &t.clips {
                    let mine = c.id == clip.id || (link.is_some() && c.link == link);
                    if mine && c.start <= hi && c.end() > lo {
                        let r = at(c.start, c.length);
                        if !r.is_positive() {
                            continue;
                        }
                        b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(s.t.danger, 0.25)));
                        b.segment(r.left_top(), r.right_bottom(), 1.5 * k, s.t.danger, false);
                        b.segment(r.left_bottom(), r.right_top(), 1.5 * k, s.t.danger, false);
                    }
                }
            }
            return;
        }
        let wanted = starts(app, source, st);
        let placed = crate::links::paintable(app.project(), source, st.track, &wanted);
        for start in placed.map(|(_, w)| w).unwrap_or_default() {
            let r = at(start, clip.length);
            if !r.is_positive() {
                continue;
            }
            ghosts.push(r);
            b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(colour, 0.35)));
            b.rrect_stroke(r, s.r(3.0), 1.5 * k, s.t.focus);
        }
    });
    ghosts
}
