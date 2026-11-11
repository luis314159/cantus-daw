//! Ghost notes (round 39, des-views): the other clips' notes, faint, behind the clip being
//! edited — the same track's other clips in its own hue, other tracks' in neutral slate —
//! and the head names whose they are ("Pads · Bass · Lead"; FL shows ghosts but not
//! whose). Inset a pixel top and bottom, r 2, no edge, no name: they are context, never
//! confused with the notes being edited, and never hit.

use super::Geo;
use crate::app::CantusApp;
use crate::kit::color::mix;
use crate::kit::{Brush, Fill, Style};
use crate::views::room::Room;
use crate::views::ticks_to_beats;
use cantus_core::{ClipId, Ticks, TrackId};
use egui::{pos2, Rect};
use std::cell::RefCell;

thread_local! {
    /// A clip's repetitions of one note, reused frame to frame.
    static AT: RefCell<Vec<Ticks>> = const { RefCell::new(Vec::new()) };
}

/// The most tracks the head names.
const NAMED: usize = 3;

/// Paint the ghosts of every other clip overlapping the view into the grid under the
/// clip `(track, clip)`'s notes; the tracks shown are written into `names` ("Pads · Bass ·
/// Lead"), cleared first.
pub(super) fn paint(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    track: TrackId,
    clip: ClipId,
    names: &mut String,
    s: &Style,
) {
    names.clear();
    // Zoomed out past a sixteenth a pixel, a ghost is a speck that says nothing: none
    // drawn, none paid for (the whole piece, 20 000 notes, holds its budget).
    if geo.px_per_beat < 4.0 {
        return;
    }
    let project = app.project();
    let Some(own) = project.track(track).and_then(|t| t.clip(clip)) else {
        return;
    };
    let room = Room::of(&s.t);
    let to_timeline = own.start - own.offset;
    let g = geo.grid;
    let from = crate::views::beats_to_ticks(geo.beats(g.left())) + to_timeline;
    let to = crate::views::beats_to_ticks(geo.beats(g.right())) + to_timeline;
    let (lo_lane, hi_lane) = (
        geo.lane(g.bottom()).floor() as i32 - 1,
        geo.lane(g.top()).ceil() as i32 + 1,
    );
    let mut named = 0usize;
    AT.with(|cell| {
        let Ok(mut at) = cell.try_borrow_mut() else {
            return;
        };
        for t in &project.tracks {
            let same = t.id == track;
            let hue = crate::theme::track_color(t.color);
            let c = match (same, room.dark) {
                (true, true) => mix(room.base, hue, 0.20),
                (true, false) => mix(room.base, hue, 0.22),
                (false, true) => mix(room.base, room.ink3, 0.24),
                (false, false) => mix(room.base, room.ink3, 0.22),
            };
            let mut shown = false;
            for other in &t.clips {
                if (same && other.id == clip) || other.notes.is_empty() || other.lane > 0 {
                    continue;
                }
                if other.start > to || other.start + other.length < from {
                    continue;
                }
                // Sorted by start: a clip that does not repeat starts at the first note
                // that can reach the view (a long lookback for held notes).
                let skip = if other.period().is_none() {
                    let first = from - (other.start - other.offset) - Ticks::beats(64);
                    other.notes.partition_point(|n| n.start < first)
                } else {
                    0
                };
                for n in &other.notes[skip..] {
                    if other.period().is_none() && n.start + (other.start - other.offset) > to {
                        break;
                    }
                    let Some(lane) = geo.axis.lane_of(n.pitch) else {
                        continue;
                    };
                    if (lane as i32) < lo_lane || (lane as i32) > hi_lane {
                        continue;
                    }
                    other.timeline_positions(n.start, &mut at);
                    for on in at.iter().copied() {
                        if on > to || on + n.length < from {
                            continue;
                        }
                        let x0 = geo.x(ticks_to_beats(on - to_timeline)).max(g.left());
                        let x1 = geo
                            .x(ticks_to_beats(on + n.length - to_timeline))
                            .min(g.right());
                        if x1 <= x0 {
                            continue;
                        }
                        let y1 = geo.y(lane as f32);
                        let r = Rect::from_min_max(
                            pos2(x0, y1 - geo.key_h + s.k),
                            pos2(x1.max(x0 + 2.0 * b.px()), y1 - s.k),
                        );
                        // Context, not the notes being edited: a plain rect a ghost (a
                        // rounded one costs four times the triangles, unseen at this size).
                        b.rect(b.snap_rect(r), Fill::Solid(c));
                        shown = true;
                    }
                }
            }
            if shown && !same {
                if named < NAMED {
                    if !names.is_empty() {
                        names.push_str(" \u{b7} ");
                    }
                    names.push_str(&t.name);
                } else if named == NAMED {
                    names.push_str(" \u{2026}");
                }
                named += 1;
            }
        }
    });
}
