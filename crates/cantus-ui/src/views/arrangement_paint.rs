//! Painting the arrangement: FL's playlist drawn with the kit (ADR 0024 §2, §4).
//!
//! Everything here reads the application and writes into a brush — nothing senses input
//! and nothing edits — so the view can paint a whole region as one mesh and handle its
//! input afterwards, with the project borrowed for exactly as long as the drawing takes.
//! Nothing is copied: the old view cloned every track, notes and all, every frame, which
//! on the stress project was 18 MB a frame (`rendering-performance.md` §2).

use super::{ArrangementView, Scratch, TimelineGeometry, LOOP_LANE_H};
use crate::app::CantusApp;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, Brush, Fill, Style, Tok};
use crate::views::clips_chrome::Tint;
use crate::views::ticks_to_beats;
use cantus_core::{ClipId, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect};

/// What the lanes paint with (for inspect mode).
const LANE_TOKENS: &[Tok] = &[
    Tok::RowBlack,
    Tok::RowWhite,
    Tok::Grid,
    Tok::GridStrong,
    Tok::GridBar,
];
/// What the ruler paints with.
const RULER_TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::RowBlack,
    Tok::GridStrong,
    Tok::GridBar,
    Tok::Text,
    Tok::TextDim,
    Tok::Highlight,
    Tok::Selection,
    Tok::Playhead,
];
/// What a header paints with besides its track's colour.
const HEADER_TOKENS: &[Tok] = &[Tok::Selection, Tok::Led, Tok::Highlight, Tok::Focus];
/// What a clip paints with besides its colour.
const CLIP_TOKENS: &[Tok] = &[Tok::RowBlack, Tok::RowWhite, Tok::Selection];

/// The visible columns of the timeline: the first and last beat drawn and how far apart
/// the lines are. Zoomed out, a beat is a pixel wide and a line per beat is a grey smear,
/// so the step doubles until lines are at least `min_px` apart.
pub(super) fn beat_step(px_per_beat: f32, beats_per_bar: f32, min_px: f32) -> f32 {
    let mut step = 1.0f32;
    while step * px_per_beat < min_px {
        step *= 2.0;
        if step >= beats_per_bar && step * px_per_beat < min_px {
            step = beats_per_bar;
            while step * px_per_beat < min_px {
                step *= 2.0;
            }
            break;
        }
    }
    step
}

/// The lanes' ground: a lane per track in the editor's row colour, every other bar a
/// step lighter so bars can be counted, and FL's line ladder — step, beat, and a bar line
/// three pixels wide with a dark core.
pub(super) fn lanes(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, rect: Rect, s: &Style) {
    b.register(rect, "lanes", LANE_TOKENS, &[]);
    let t = &s.t;
    let rect = b.snap_rect(rect);
    b.rect(rect, Fill::Solid(t.row_black));
    let ts = app.project().time_signature;
    let per_bar = ts.numerator as f32 * 4.0 / ts.denominator as f32;
    let first = geo.scroll_beats.max(0.0);
    let last = geo.beats(rect.right());
    // Alternate bars.
    let bar_px = per_bar * geo.px_per_beat;
    if bar_px >= 6.0 {
        let mut bar = (first / per_bar).floor();
        while bar * per_bar <= last {
            if bar as i64 % 2 == 1 {
                let x0 = geo.x(bar * per_bar).max(rect.left());
                let x1 = geo.x((bar + 1.0) * per_bar).min(rect.right());
                if x1 > x0 {
                    b.rect(
                        b.snap_rect(Rect::from_min_max(
                            pos2(x0, rect.top()),
                            pos2(x1, rect.bottom()),
                        )),
                        Fill::Solid(t.row_white),
                    );
                }
            }
            bar += 1.0;
        }
    }
    // The theme's glows (ADR 0046 §8): under the lines and the clips, over the rows.
    crate::kit::light::ambient(b, rect, t);
    // Lane separators: one line under each visible row, and on down the empty rows.
    let base = app.arrangement.track_height;
    let mut y = geo.bottom() + base;
    while base >= 8.0 && y < rect.bottom() {
        if y > rect.top() {
            b.hline(rect.left(), rect.right(), y - 0.5 * b.px(), b.px(), t.grid);
        }
        y += base;
    }
    for r in &geo.rows {
        let y = r.top + r.height;
        if y < rect.top() || r.top > rect.bottom() {
            continue;
        }
        b.hline(
            rect.left(),
            rect.right(),
            y - 0.5 * b.px(),
            b.px(),
            t.grid_strong,
        );
    }
    // The line ladder.
    let step = beat_step(geo.px_per_beat, per_bar, 7.0);
    let mut beat = (first / step).floor() * step;
    let bar_side = mix(t.grid_bar, t.row_black, 0.55);
    while beat <= last {
        let x = geo.x(beat);
        let is_bar = (beat / per_bar).fract().abs() < 1e-4;
        if is_bar {
            b.vline(x, rect.top(), rect.bottom(), 3.0 * b.px(), bar_side);
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_bar);
        } else {
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_strong);
        }
        beat += step;
    }
}

/// The ruler: the loop lane with its brace on top, the bar numbers below, the time
/// selection over both, and the playhead's marker.
pub(super) fn ruler(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, rect: Rect, s: &Style) {
    b.register(rect, "ruler", RULER_TOKENS, &[]);
    let t = &s.t;
    let rect = b.snap_rect(rect);
    let ground = mix(t.ink, t.row_black, 0.55);
    b.rect(rect, Fill::V(shade(ground, 0.02), ground));
    let loop_lane = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + LOOP_LANE_H));
    b.rect(b.snap_rect(loop_lane), Fill::Solid(t.ink));
    // Below the loop lane, the punch lane while punch is on (`arrangement_rec`).
    let numbers_top = geo.punch.map_or(loop_lane.bottom(), |(_, bottom)| bottom);
    b.hline(
        rect.left(),
        rect.right(),
        rect.bottom() - 0.5 * b.px(),
        b.px(),
        t.grid_bar,
    );

    let ts = app.project().time_signature;
    let per_bar = ts.numerator as f32 * 4.0 / ts.denominator as f32;
    let first = geo.scroll_beats.max(0.0);
    let last = geo.beats(rect.right());
    // Ticks: beats short, bars full height of the number lane.
    let step = beat_step(geo.px_per_beat, per_bar, 7.0);
    let mut beat = (first / step).floor() * step;
    while beat <= last {
        let x = geo.x(beat);
        let is_bar = (beat / per_bar).fract().abs() < 1e-4;
        let top = if is_bar {
            numbers_top
        } else {
            rect.bottom() - 4.0 * s.k
        };
        b.vline(
            x,
            top,
            rect.bottom(),
            b.px(),
            if is_bar { t.grid_bar } else { t.grid_strong },
        );
        beat += step;
    }
    // Bar numbers, thinned by doubling so they stay ones a musician counts in.
    let bar_px = per_bar * geo.px_per_beat;
    let mut every = 1i64;
    while bar_px * (every as f32) < 28.0 * s.k {
        every *= 2;
    }
    let mut bar = ((first / per_bar).floor() as i64 / every) * every;
    let mut digits = crate::views::clips_chrome::Digits::default();
    while (bar as f32) * per_bar <= last {
        let x = geo.x(bar as f32 * per_bar);
        let label = Rect::from_min_max(
            pos2(x + 3.0 * s.k, numbers_top),
            pos2(x + bar_px * every as f32 - 2.0 * s.k, rect.bottom() - 1.0),
        );
        // Every fourth bar is a phrase: its number in full-strength text, as in FL.
        let color = if (bar % 4) == 0 { t.text } else { t.text_dim };
        let n = digits.fmt(bar + 1);
        let tx = Text::new(n, Role::Label, color)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, label, &tx, s.tk());
        bar += every;
    }

    // The loop brace: a bracket, whose ends are what get dragged.
    if let Some(l) = &app.project().loop_region {
        let x0 = geo.x(ticks_to_beats(l.start));
        let x1 = geo.x(ticks_to_beats(l.end));
        let a = if app.project().loop_enabled { 1.0 } else { 0.4 };
        let c = with_alpha(t.highlight, a);
        let bar_r = Rect::from_min_max(
            pos2(x0, loop_lane.top() + 2.0),
            pos2(x1, loop_lane.bottom() - 2.0),
        );
        b.rect(
            b.snap_rect(bar_r),
            Fill::Solid(with_alpha(t.highlight, 0.55 * a)),
        );
        for x in [x0, x1] {
            b.rect(
                b.snap_rect(Rect::from_min_max(
                    pos2(x - 1.5, loop_lane.top() + 1.0),
                    pos2(x + 1.5, loop_lane.bottom()),
                )),
                Fill::Solid(c),
            );
        }
    }
    // The time selection: brightest here, where its edges are aimed at.
    if let Some((from, to)) = app.time_selection {
        let x0 = geo.x(ticks_to_beats(from));
        let x1 = geo.x(ticks_to_beats(to));
        let band = Rect::from_min_max(pos2(x0, numbers_top), pos2(x1, rect.bottom()));
        b.rect(
            b.snap_rect(band),
            Fill::Solid(with_alpha(t.selection, 0.45)),
        );
        for x in [x0, x1] {
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.selection);
        }
    }
    // The playhead's marker: a triangle on the ruler's floor, pointing at the lanes.
    let ph = geo.x(ticks_to_beats(app.engine.position()));
    if ph >= rect.left() - 6.0 && ph <= rect.right() + 6.0 {
        let k = s.k;
        b.polygon(
            &[
                pos2(ph - 5.0 * k, rect.bottom() - 8.0 * k),
                pos2(ph + 5.0 * k, rect.bottom() - 8.0 * k),
                pos2(ph, rect.bottom()),
            ],
            t.playhead,
        );
    }
}

/// The time selection's wash and the playhead's line over the lanes, after the clips.
pub(super) fn overlay(
    b: &mut Brush,
    app: &CantusApp,
    v: &ArrangementView,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let t = &s.t;
    if let Some((from, to)) = app.time_selection {
        let x0 = geo.x(ticks_to_beats(from));
        let x1 = geo.x(ticks_to_beats(to));
        let column = Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.bottom()));
        b.rect(
            b.snap_rect(column),
            Fill::Solid(with_alpha(t.selection, 0.10)),
        );
        for x in [x0, x1] {
            b.vline(
                x,
                rect.top(),
                rect.bottom(),
                b.px(),
                with_alpha(t.selection, 0.8),
            );
        }
    }
    if let Some((a, c)) = v.marquee_rect() {
        let r = Rect::from_two_pos(a, c);
        b.rect(r, Fill::Solid(with_alpha(t.selection, 0.16)));
        b.rrect_stroke(r, 0.0, b.px(), t.selection);
    }
    let ph = geo.x(ticks_to_beats(app.engine.position()));
    if ph >= rect.left() && ph <= rect.right() {
        b.vline(ph, rect.top(), rect.bottom(), 1.5 * s.k, t.playhead);
    }
}

/// Whether a clip is in the selection, without copying the selection.
pub(super) fn is_selected(app: &CantusApp, r: (TrackId, ClipId)) -> bool {
    if app.selected_clips.is_empty() {
        app.selected_track == Some(r.0) && app.selected_clip == Some(r.1)
    } else {
        app.selected_clips.contains(&r)
    }
}

/// Where the parts of a track header go, so the painter and the input agree.
#[derive(Copy, Clone, Debug)]
pub(super) struct HeaderRects {
    pub row: Rect,
    /// The colour cell with the track's number: the grab handle for reordering.
    pub cell: Rect,
    pub icon: Rect,
    pub name: Rect,
    pub mute: Pos2,
    pub solo: Pos2,
    /// The record-arm lamp, left of mute (ADR 0025; painted by `arrangement_rec`).
    pub arm: Pos2,
    /// The whole piece in miniature, when the row is tall enough to hold it.
    pub map: Option<Rect>,
}

/// The LEDs' diameter at 100 %.
pub(super) const LED: f32 = 11.0;

pub(super) fn header_rects(row: Rect, s: &Style) -> HeaderRects {
    let k = s.k;
    let inner = Rect::from_min_max(
        pos2(row.left() + 1.0, row.top()),
        pos2(row.right() - 1.0, row.bottom() - 1.0),
    );
    let cell = Rect::from_min_max(inner.min, pos2(inner.left() + 18.0 * k, inner.bottom()));
    // The name line: the top of a tall row (the map goes under it), the middle of a short one.
    let tall = inner.height() >= 34.0 * k;
    let line_h = 20.0 * k;
    let line = if tall {
        Rect::from_min_max(
            pos2(cell.right(), inner.top() + 2.0 * k),
            pos2(inner.right(), inner.top() + 2.0 * k + line_h),
        )
    } else {
        Rect::from_center_size(
            pos2((cell.right() + inner.right()) * 0.5, inner.center().y),
            vec2(inner.right() - cell.right(), line_h.min(inner.height())),
        )
    };
    let solo = pos2(line.right() - 8.0 * k, line.center().y);
    let mute = pos2(solo.x - (LED + 5.0) * k, line.center().y);
    let arm = pos2(mute.x - (LED + 5.0) * k, line.center().y);
    let icon = Rect::from_center_size(
        pos2(cell.right() + 12.0 * k, line.center().y),
        vec2(15.0 * k, 15.0 * k),
    );
    let name = Rect::from_min_max(
        pos2(icon.right() + 5.0 * k, line.top()),
        pos2(arm.x - LED * 0.5 * k - 6.0 * k, line.bottom()),
    );
    let map = tall.then(|| {
        Rect::from_min_max(
            pos2(cell.right() + 6.0 * k, line.bottom() + 3.0 * k),
            pos2(
                inner.right() - 6.0 * k,
                (inner.bottom() - 5.0 * k).max(line.bottom() + 6.0 * k),
            ),
        )
    });
    HeaderRects {
        row: inner,
        cell,
        icon,
        name,
        mute,
        solo,
        arm,
        map,
    }
}

/// What a header shows beyond what the track says: the pointer and the view's state.
#[derive(Copy, Clone, Debug, Default)]
pub(super) struct HeaderState {
    pub selected: bool,
    pub hovered_mute: bool,
    pub hovered_solo: bool,
    /// Silenced by another track's solo.
    pub silenced: bool,
    /// Being renamed: the name is an editor, not text.
    pub renaming: bool,
    /// Where a dragged header would land: a line on this row's top edge.
    pub drop_above: bool,
    /// Its bottom edge is being dragged or aimed at.
    pub divider: bool,
    /// The track's peak, 0…1 of the meter's range.
    pub level: f32,
}

/// One track header: FL's coloured block — the colour cell with the number, the icon's
/// place, the name elided in the middle and tinted toward the colour, the mute and solo
/// lamps, the song in miniature, and the level as a hairline along the bottom.
#[allow(clippy::too_many_arguments)]
pub(super) fn header(
    b: &mut Brush,
    app: &CantusApp,
    index: usize,
    r: &HeaderRects,
    st: HeaderState,
    tint: Tint,
    song_len: f32,
    digits: &mut crate::views::clips_chrome::Digits,
    s: &Style,
) {
    let Some(track) = app.project().tracks.get(index) else {
        return;
    };
    b.register(r.row, "track_header", HEADER_TOKENS, &[]);
    let t = &s.t;
    let face = if st.selected {
        shade(tint.large, if t.dark { 0.06 } else { -0.05 })
    } else {
        tint.large
    };
    light::raised(
        b,
        r.row,
        s.r(2.0),
        face,
        RaisedOpts {
            drop: false,
            k: s.k,
            depth: t.depth,
            ..Default::default()
        },
    );
    // The colour cell: the pastel of the same hue, raised, with the track's number.
    light::raised(
        b,
        r.cell.shrink(2.0 * s.k),
        s.r(2.0),
        tint.small,
        RaisedOpts {
            drop: false,
            k: s.k,
            depth: t.depth,
            ..Default::default()
        },
    );
    if st.selected {
        b.rrect_stroke(r.cell.shrink(1.0 * s.k), s.r(3.0), 1.5 * s.k, t.selection);
    }
    let n = digits.fmt(index as i64 + 1);
    let num = Text::new(n, Role::Micro, tint.on_small)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let num_box = if r.map.is_some() {
        Rect::from_min_size(
            r.cell.min + vec2(0.0, 4.0 * s.k),
            vec2(r.cell.width(), 16.0 * s.k),
        )
    } else {
        r.cell
    };
    let _ = text::paint(b, num_box.shrink2(vec2(2.0 * s.k, 0.0)), &num, s.tk());

    // The name, tinted toward the colour; dimmed when the track is not sounding.
    if !st.renaming {
        let quiet = track.muted || st.silenced;
        let ink = if quiet {
            with_alpha(tint.on_large, 0.55)
        } else {
            tint.on_large
        };
        let name = Text::new(&track.name, Role::Body, ink)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        let _ = text::paint(b, r.name, &name, s.tk());
    }

    // Mute is FL's enable lamp — lit while the track sounds; solo lights in the
    // highlight. A track silenced by someone else's solo keeps its lamp, dimmed.
    let d = LED * s.k;
    let on_alpha = if st.silenced { 0.5 } else { 1.0 };
    led::paint(
        b,
        r.mute,
        d,
        !track.muted,
        s,
        Tok::Led,
        Tok::Panel,
        on_alpha,
    );
    led::paint(
        b,
        r.solo,
        d,
        track.soloed,
        s,
        Tok::Highlight,
        Tok::Panel,
        1.0,
    );
    for (c, hovered) in [(r.mute, st.hovered_mute), (r.solo, st.hovered_solo)] {
        if hovered {
            b.circle_stroke(c, d * 0.5 + 1.5 * s.k, b.px(), t.text);
        }
    }

    // The song in miniature: every clip to scale on a faint line, so the chorus can be
    // found without scrolling — the best thing FL's headers do. A line and not a slot:
    // a slot with one long clip in it reads as a progress bar.
    if let Some(map) = r.map {
        if map.height() >= 3.0 && song_len > 0.0 {
            let line = Rect::from_min_max(
                pos2(map.left(), map.bottom() - 4.0 * s.k),
                pos2(map.right(), map.bottom() - 1.0 * s.k),
            );
            b.rect(
                b.snap_rect(line),
                Fill::Solid(with_alpha(
                    shade(face, if t.dark { -0.12 } else { -0.10 }),
                    0.8,
                )),
            );
            let inner = line;
            for clip in &track.clips {
                let x0 = inner.left() + clip.start.0 as f32 / song_len * inner.width();
                let x1 = inner.left() + clip.end().0 as f32 / song_len * inner.width();
                let c = if clip.muted {
                    with_alpha(tint.small, 0.35)
                } else {
                    tint.small
                };
                b.rect(
                    Rect::from_min_max(
                        pos2(x0, inner.top()),
                        pos2(x1.max(x0 + 1.0), inner.bottom()),
                    ),
                    Fill::Solid(c),
                );
            }
        }
    }
    // The level: a hairline along the bottom, in the meter's colours.
    if st.level > 0.0 {
        let w = (r.row.width() - r.cell.width()) * st.level.clamp(0.0, 1.0);
        let c = if st.level > 0.9 {
            t.meter_clip
        } else if st.level > 0.75 {
            t.meter_high
        } else {
            t.meter_low
        };
        b.rect(
            Rect::from_min_size(
                pos2(r.cell.right(), r.row.bottom() - 2.0 * s.k),
                vec2(w, 2.0 * s.k),
            ),
            Fill::Solid(c),
        );
    }
    if st.drop_above {
        b.hline(
            r.row.left(),
            r.row.right(),
            r.row.top(),
            2.0 * s.k,
            t.selection,
        );
    }
    if st.divider {
        b.hline(
            r.row.left(),
            r.row.right(),
            r.row.bottom() + 0.5,
            2.0 * s.k,
            t.focus,
        );
    }
}

/// An empty row below the last track: the header's shape, flat and faint, with the name
/// the next track would get (FL's "Track 31").
pub(super) fn empty_header(b: &mut Brush, row: Rect, label: &str, s: &Style) {
    let t = &s.t;
    b.register(row, "empty_header", &[Tok::Panel, Tok::Placeholder], &[]);
    let inner = Rect::from_min_max(
        pos2(row.left() + 1.0, row.top()),
        pos2(row.right() - 1.0, row.bottom() - 1.0),
    );
    let face = shade(t.panel, -0.03);
    b.rrect(b.snap_rect(inner), s.r(2.0), Fill::Solid(face));
    let tx = Text::new(label, Role::Body, t.placeholder)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let name = Rect::from_min_max(
        pos2(inner.left() + 24.0 * s.k, inner.top()),
        pos2(
            inner.right() - 6.0 * s.k,
            inner.top() + (20.0 * s.k).min(inner.height()),
        ),
    );
    let _ = text::paint(b, name, &tx, s.tk());
}

/// How a clip looks this frame.
#[derive(Copy, Clone, Debug)]
pub(super) struct ClipLook {
    pub selected: bool,
    pub muted: bool,
    pub tint: Tint,
    /// Leave the name out: it is being edited in place.
    pub renaming: bool,
    /// Room kept free at the title bar's right (an audio clip's gain and reverse badges).
    pub reserve: f32,
    /// A linked copy of a selected clip (ADR 0037): outlined faintly in the selection's
    /// colour, so the whole group shows when one member is picked.
    pub sibling: bool,
}

/// The chrome of one clip: a body in a deep (or pale) version of its colour, a title bar
/// in the colour itself with FL's ▸ and the name tinted toward the colour, kept on
/// screen while the clip's start scrolls off to the left. A linked clip (ADR 0037) shows
/// a chain mark before its name and how many copies share it ("×3", `linked`) at the
/// bar's right. Returns the body's inner rect for the preview.
pub(super) fn clip_chrome(
    b: &mut Brush,
    body: Rect,
    visible: Rect,
    name: &str,
    linked: Option<&str>,
    look: ClipLook,
    s: &Style,
) -> Rect {
    b.register(body, "clip", CLIP_TOKENS, &[]);
    let t = &s.t;
    let k = s.k;
    let ground = if t.dark { t.row_black } else { t.row_white };
    let (bar_c, body_c) = if look.muted {
        let grey = mix(look.tint.large, ground, 0.65);
        (grey, mix(grey, ground, 0.5))
    } else {
        (
            look.tint.large,
            mix(look.tint.large, ground, if t.dark { 0.55 } else { 0.45 }),
        )
    };
    let bar_c = if look.selected {
        shade(bar_c, if t.dark { 0.06 } else { -0.04 })
    } else {
        bar_c
    };
    let r = s.r(2.0);
    let body = b.snap_rect(body);
    b.rrect(body, r, Fill::Solid(shade(body_c, -0.12)));
    b.rrect(
        body.shrink(b.px()),
        (r - b.px()).max(0.0),
        Fill::Solid(body_c),
    );
    let title_h = title_height(body, k);
    if title_h > 0.0 {
        let title = Rect::from_min_max(body.min, pos2(body.right(), body.top() + title_h));
        b.rrect(
            title.shrink(b.px()),
            (r - b.px()).max(0.0),
            Fill::V(shade(bar_c, 0.03 * t.depth), shade(bar_c, -0.03 * t.depth)),
        );
        b.hline(
            title.left() + b.px(),
            title.right() - b.px(),
            title.top() + 1.5 * b.px(),
            b.px(),
            shade(bar_c, 0.12 * t.depth),
        );
    }
    if look.selected {
        b.rrect_stroke(body.shrink(0.75 * k), r, 1.5 * k, t.selection);
    } else if look.sibling {
        // Dashed in the selection's colour: part of what is picked, not picked itself.
        dashed_rect(
            b,
            body.shrink(0.75 * k),
            visible,
            1.25 * k,
            t.selection,
            4.0 * k,
            3.0 * k,
        );
    }
    // The name: on the bar, or on the body when there is no bar; kept inside the part of
    // the clip that is on screen.
    if !look.renaming && body.width() >= 14.0 * k {
        let line = if title_h > 0.0 {
            Rect::from_min_max(body.min, pos2(body.right(), body.top() + title_h))
        } else {
            body
        };
        let mut left = body.left().max(visible.left()) + 2.0 * k;
        let ink = if title_h > 0.0 {
            look.tint.on_large
        } else {
            t.text
        };
        let ink = if look.muted {
            with_alpha(ink, 0.6)
        } else {
            ink
        };
        // FL's ▸ at the start of a clip's bar: the clip has a menu (right-click).
        if title_h > 0.0 && body.right() - left > 36.0 * k {
            let g = Rect::from_min_size(
                pos2(left, line.center().y - 4.0 * k),
                egui::vec2(8.0 * k, 8.0 * k),
            );
            crate::kit::glyph::paint(b, g, crate::kit::glyph::Glyph::Menu, ink);
            left = g.right();
        }
        // A linked clip: the chain, then the name; the count at the right end.
        let mut right = body.right() - 3.0 * k - look.reserve;
        if let Some(count) = linked {
            if right - left > 30.0 * k {
                let g = Rect::from_min_size(
                    pos2(left + 2.0 * k, line.center().y - 5.0 * k),
                    egui::vec2(10.0 * k, 10.0 * k),
                );
                crate::kit::glyph::paint(b, g, crate::kit::glyph::Glyph::Link, ink);
                left = g.right();
            }
            let w = (6.0 * count.chars().count() as f32 + 4.0) * k;
            if right - left > w + 24.0 * k {
                let count_box =
                    Rect::from_min_max(pos2(right - w, line.top()), pos2(right, line.bottom()));
                let tx = Text::new(count, Role::Label, ink).align(Align2::RIGHT_CENTER);
                let _ = text::paint(b, count_box, &tx, s.tk());
                right = count_box.left() - 2.0 * k;
            }
        }
        let name_box =
            Rect::from_min_max(pos2(left + 3.0 * k, line.top()), pos2(right, line.bottom()));
        if name_box.width() > 6.0 * k {
            let tx = Text::new(name, Role::Label, ink)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, name_box, &tx, s.tk());
        }
    }
    let top = body.top()
        + if title_h > 0.0 {
            title_h + 2.0 * k
        } else {
            3.0 * k
        };
    Rect::from_min_max(
        pos2(body.left() + 2.0 * k, top),
        pos2(body.right() - 2.0 * k, body.bottom() - 2.0 * k),
    )
}

/// A rectangle's outline in dashes of `dash` with gaps of `gap`, each side on its own.
/// A dashed outline of `r`, drawn only where it meets `visible`: a long clip zoomed in is
/// tens of thousands of pixels wide, nearly all of them off screen. The dashes keep their
/// places on the clip (they start at its corner), so the pattern does not crawl as the
/// view scrolls.
#[allow(clippy::too_many_arguments)]
fn dashed_rect(
    b: &mut Brush,
    r: Rect,
    visible: Rect,
    w: f32,
    color: egui::Color32,
    dash: f32,
    gap: f32,
) {
    let step = (dash + gap).max(1.0);
    for x in dashes(r.left(), r.right(), visible.left(), visible.right(), step) {
        let x1 = (x + dash).min(r.right());
        b.hline(x, x1, r.top(), w, color);
        b.hline(x, x1, r.bottom() - w, w, color);
    }
    let shown = |x: f32| x + w >= visible.left() && x <= visible.right();
    for y in dashes(r.top(), r.bottom(), visible.top(), visible.bottom(), step) {
        let y1 = (y + dash).min(r.bottom());
        if shown(r.left()) {
            b.vline(r.left(), y, y1, w, color);
        }
        if shown(r.right() - w) {
            b.vline(r.right() - w, y, y1, w, color);
        }
    }
}

/// Where the dashes of an edge from `lo` to `hi` start (every `step`, from `lo`), keeping
/// only those that reach into `vis_lo..vis_hi`.
fn dashes(lo: f32, hi: f32, vis_lo: f32, vis_hi: f32, step: f32) -> impl Iterator<Item = f32> {
    let first = if vis_lo > lo {
        ((vis_lo - lo) / step).floor()
    } else {
        0.0
    };
    let end = hi.min(vis_hi);
    (first as u32..)
        .map(move |i| lo + i as f32 * step)
        .take_while(move |x| *x < end)
}

/// The colour a clip's notes are drawn in on its body.
pub(super) fn note_ink(look: &ClipLook, s: &Style) -> egui::Color32 {
    let t = &s.t;
    if look.muted {
        t.text_dim
    } else if t.dark {
        shade(look.tint.small, 0.10)
    } else {
        shade(look.tint.large, -0.32)
    }
}

/// Where a scratch buffer is needed while painting: the clip repeats' positions and the
/// label with its "(muted)" suffix.
pub(super) fn clip_label<'a>(scratch: &'a mut Scratch, name: &str, muted: bool) -> &'a str {
    scratch.label.clear();
    scratch.label.push_str(name);
    if muted {
        scratch.label.push_str(" (");
        scratch.label.push_str(&scratch.muted_word);
        scratch.label.push(')');
    }
    &scratch.label
}

/// The height of a clip's title bar: none on a body too short to hold one.
pub(super) fn title_height(body: Rect, k: f32) -> f32 {
    if body.height() >= 22.0 * k {
        14.0 * k
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::dashes;

    /// A 64-bar clip at the widest zoom is about 100 000 px; only the dashes on screen are
    /// drawn, and they sit where they would if the whole edge were drawn.
    #[test]
    fn only_the_dashes_on_screen_are_drawn_and_they_do_not_crawl() {
        let step = 7.0;
        let all: Vec<f32> = dashes(-50_000.0, 52_000.0, f32::MIN, f32::MAX, step).collect();
        assert!(all.len() > 14_000);
        let shown: Vec<f32> = dashes(-50_000.0, 52_000.0, 0.0, 1_200.0, step).collect();
        assert!(shown.len() <= 1_200 / 7 + 2, "{}", shown.len());
        assert!(shown[0] <= 0.0 && shown[0] + step > 0.0);
        assert!(shown.iter().all(|x| *x < 1_200.0));
        // On the same grid as the whole edge.
        for x in &shown {
            let q = (x + 50_000.0) / step;
            assert!((q - q.round()).abs() < 1e-2, "{x}");
        }
        // A clip that starts on screen starts its dashes at its corner.
        let near: Vec<f32> = dashes(100.0, 130.0, 0.0, 1_200.0, step).collect();
        assert_eq!(near, vec![100.0, 107.0, 114.0, 121.0, 128.0]);
    }
}
