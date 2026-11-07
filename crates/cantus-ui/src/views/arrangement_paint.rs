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
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::clips_chrome::Tint;
use crate::views::ticks_to_beats;
use cantus_core::{ClipId, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect};

/// What the lanes paint with (for inspect mode).
const LANE_TOKENS: &[Tok] = &[
    Tok::Glow,
    Tok::GlowCool,
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
const HEADER_TOKENS: &[Tok] = &[
    Tok::Selection,
    Tok::Led,
    Tok::Highlight,
    Tok::Focus,
    Tok::TrackVeil,
];
/// What a clip paints with besides its colour.
const CLIP_TOKENS: &[Tok] = &[Tok::RowBlack, Tok::RowWhite, Tok::Selection, Tok::TrackVeil];

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
    // Through the metre changes (ADR 0060): every bar its own length.
    let map = crate::views::bars::map(app.project());
    let several = !map.is_one_metre();
    // r37:finish — zoomed out (a bar under 28 px) the ground alternates by four bars, a
    // phrase, and the bar lines thin: every bar shaded and 3 px wide read as a barcode.
    let narrow = per_bar * geo.px_per_beat < 28.0;
    let group: i64 = if narrow { 4 } else { 1 };
    if several && per_bar * geo.px_per_beat >= 6.0 {
        crate::views::bars::each_bar(&map, first, last, |n, start, len| {
            if (n - 1).div_euclid(group) % 2 == 1 {
                let x0 = geo.x(start).max(rect.left());
                let x1 = geo.x(start + len).min(rect.right());
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
        });
    }
    // Alternate bars.
    let bar_px = per_bar * geo.px_per_beat;
    if bar_px >= 6.0 && !several {
        let mut bar = (first / per_bar).floor();
        while bar * per_bar <= last {
            if (bar as i64).div_euclid(group) % 2 == 1 {
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
    // The line ladder, through the metre changes.
    let bar_side = mix(t.grid_bar, t.row_black, 0.55);
    let thin_bar = mix(t.grid_bar, t.grid_strong, 0.6); // r37:finish
    crate::views::bars::ladder(&map, first, last, geo.px_per_beat, 7.0, |l| {
        let x = geo.x(l.beat);
        if l.bar && narrow {
            // r37:finish — every fourth bar (a phrase) strong, the rest a hairline.
            let n = (l.beat / per_bar).round() as i64;
            let strong = several || n.rem_euclid(4) == 0;
            b.vline(
                x,
                rect.top(),
                rect.bottom(),
                b.px(),
                if strong { t.grid_bar } else { thin_bar },
            );
        } else if l.bar {
            b.vline(x, rect.top(), rect.bottom(), 3.0 * b.px(), bar_side);
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_bar);
        } else {
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_strong);
        }
    });
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
    // Under the marker lane and the punch lane, when they are there (ADR 0060).
    let numbers_top = geo
        .punch
        .map(|p| p.1)
        .or(geo.markers.map(|m| m.1))
        .unwrap_or(loop_lane.bottom());
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
    let map = crate::views::bars::map(app.project());
    // Ticks: beats short, bars full height of the number lane (through the metre changes).
    crate::views::bars::ladder(&map, first, last, geo.px_per_beat, 7.0, |l| {
        let x = geo.x(l.beat);
        let top = if l.bar {
            numbers_top
        } else {
            rect.bottom() - 4.0 * s.k
        };
        b.vline(
            x,
            top,
            rect.bottom(),
            b.px(),
            if l.bar { t.grid_bar } else { t.grid_strong },
        );
    });
    // Bar numbers, thinned by doubling so they stay ones a musician counts in.
    let bar_px = per_bar * geo.px_per_beat;
    let mut every = 1i64;
    while bar_px * (every as f32) < 44.0 * s.k {
        // r37:finish (28: dense)
        every *= 2;
    }
    let mut digits = crate::views::clips_chrome::Digits::default();
    if !map.is_one_metre() {
        // Each bar where it starts, numbered through the metre changes; the thinning by
        // the narrowest bar in view, so numbers never collide.
        let mut narrow = f32::MAX;
        crate::views::bars::each_bar(&map, first, last, |_, _, len| narrow = narrow.min(len));
        let mut every = 1i64;
        while narrow * geo.px_per_beat * (every as f32) < 44.0 * s.k && every < 1024 {
            // r37:finish
            every *= 2;
        }
        // A metre change's chip at its bar line, the bar's number after it.
        let chips = super::tempo::chips(app);
        crate::views::bars::each_bar(&map, first, last, |n, start, _| {
            let x = geo.x(start);
            let tick = crate::views::beats_to_ticks(start);
            let mut after = 0.0;
            if let Some((_, sig)) = chips.iter().find(|(t, _)| *t == tick) {
                let r = super::tempo::chip_rect(b.ctx(), x, numbers_top, rect.bottom(), *sig, s);
                after = super::tempo::paint_chip(b, r, *sig, false, s);
            }
            if (n - 1).rem_euclid(every) != 0 {
                return;
            }
            let label = Rect::from_min_max(
                pos2(x + 3.0 * s.k + after, numbers_top),
                pos2(
                    x + narrow * geo.px_per_beat * every as f32 - 2.0 * s.k,
                    rect.bottom() - 1.0,
                ),
            );
            let color = if (n - 1) % 4 == 0 { t.text } else { t.text_dim };
            let role = if (n - 1) % 4 == 0 {
                Role::Title
            } else {
                Role::Control
            }; // r37:finish
            let tx = Text::new(digits.fmt(n), role, color)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, label, &tx, s.tk());
        });
    }
    let mut bar = ((first / per_bar).floor() as i64 / every) * every;
    while map.is_one_metre() && (bar as f32) * per_bar <= last {
        let x = geo.x(bar as f32 * per_bar);
        let label = Rect::from_min_max(
            pos2(x + 3.0 * s.k, numbers_top),
            pos2(x + bar_px * every as f32 - 2.0 * s.k, rect.bottom() - 1.0),
        );
        // Every fourth bar is a phrase: its number in full-strength text, as in FL.
        let color = if (bar % 4) == 0 { t.text } else { t.text_dim };
        let n = digits.fmt(bar + 1);
        // r37:finish — the numbers a size up, a phrase's a size more.
        let role = if (bar % 4) == 0 {
            Role::Title
        } else {
            Role::Control
        };
        let tx = Text::new(n, role, color)
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
    let solo = pos2(line.right() - 14.0 * k, line.center().y); // r37:finish (room for the level meter)
    let mute = pos2(solo.x - (LED + 8.0) * k, line.center().y); // r37:finish
    let arm = pos2(mute.x - (LED + 8.0) * k, line.center().y); // r37:finish
    let icon = Rect::from_center_size(
        pos2(cell.right() + 12.0 * k, line.center().y),
        // No taller than the name line: on a short row a full-size icon overlapped the rows
        // round it.
        vec2(1.0, 1.0) * (15.0 * k).min(line.height()),
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
    /// How much the name's words are scaled to fit the name line (1 but on short rows,
    /// where the full-size words would overflow into the rows round it).
    pub fit: f32,
    /// The track's peak, 0…1 of the meter's range.
    pub level: f32,
}

/// One track header: FL's coloured block — the colour cell with the number, the icon's
/// place, the name elided in the middle and tinted toward the colour, the mute and solo
/// lamps, the song in miniature, and the level as a hairline along the bottom. Returns
/// where the name's letters were drawn, on the name line's height (round 27, ADR 0051: a
/// click there renames); `None` while it is being renamed or when nothing fitted.
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
) -> Option<Rect> {
    let track = app.project().tracks.get(index)?;
    b.register(r.row, "track_header", HEADER_TOKENS, &[]);
    let t = &s.t;
    // r37:finish — on a light theme the header stays neutral, the track's colour only in
    // its cell at the left (judge 8: a pastel wash on every surface read as candy and
    // lowered the text's contrast).
    let (face, on_face) = match (t.dark, st.selected) {
        (true, false) => (tint.large, tint.on_large),
        (true, true) => (shade(tint.large, 0.06), tint.on_large),
        (false, false) => (t.panel_raised, t.text),
        (false, true) => (t.strip_selected, t.text),
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
    let num = Text::new(n, Role::Label, tint.on_small) // r37:finish — a size up, one crisp digit
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
    let mut letters = None;
    if !st.renaming {
        let quiet = track.muted || st.silenced;
        let ink = if quiet {
            with_alpha(on_face, 0.55)
        } else {
            on_face
        };
        let name = Text::new(&track.name, Role::Body, ink)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        // r37:finish — a name a little too long is set a little smaller, whole, before it is
        // cut in the middle (judge 7: "Per…ion" where a few points were missing).
        let tk = s.tk() * st.fit;
        let need = text::measure(b.ctx(), &name, tk).x;
        let tk = if need > r.name.width() && need * 0.88 <= r.name.width() {
            tk * (r.name.width() / need)
        } else {
            tk
        };
        letters = text::paint(b, r.name, &name, tk)
            .map(|p| Rect::from_x_y_ranges(p.rect.x_range(), r.name.y_range()));
    }

    // r37:finish — mute and solo as labelled keys (M, S), lit when they act: a muted
    // track's M in the highlight, a soloed track's S in the selection's colour; a track
    // silenced by someone else's solo shows its S dimmed.
    let side = (LED + 6.0) * s.k; // r37:finish
    letter_key(
        b,
        r.mute,
        side,
        "M",
        track.muted,
        t.highlight,
        st.hovered_mute,
        1.0,
        s,
    );
    letter_key(
        b,
        r.solo,
        side,
        "S",
        track.soloed,
        t.selection,
        st.hovered_solo,
        if st.silenced { 0.5 } else { 1.0 },
        s,
    );

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
                // r37:finish — each clip its own segment, a pixel apart from the next, so
                // the map reads as the song's clips (a run read as one bar with a
                // glitch where a section was empty).
                b.rect(
                    Rect::from_min_max(
                        pos2(x0, inner.top()),
                        pos2((x1 - b.px()).max(x0 + 1.0), inner.bottom()),
                    ),
                    Fill::Solid(c),
                );
            }
        }
    }
    // r37:finish — the level as a slim meter standing at the header's right edge, in the
    // meter's colours (a hairline under the name read as a stray line).
    {
        let w = (3.0 * s.k).max(b.px() * 2.0);
        let well = Rect::from_min_max(
            pos2(r.row.right() - w - 2.0 * s.k, r.row.top() + 3.0 * s.k),
            pos2(r.row.right() - 2.0 * s.k, r.row.bottom() - 3.0 * s.k),
        );
        b.rect(b.snap_rect(well), Fill::Solid(t.meter_well));
        if st.level > 0.0 {
            let c = if st.level > 0.9 {
                t.meter_clip
            } else if st.level > 0.75 {
                t.meter_high
            } else {
                t.meter_low
            };
            let h = well.height() * st.level.clamp(0.0, 1.0);
            b.rect(
                b.snap_rect(Rect::from_min_max(
                    pos2(well.left(), well.bottom() - h),
                    well.max,
                )),
                Fill::Solid(c),
            );
        }
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
    letters
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
    /// The height the label's words need at full size: a clip shorter than that scales
    /// them, and hides them below a readable size (round 27).
    pub label_row: f32,
    /// Its name says what the clip above it already says (the same name from the same
    /// moment): no title bar, its content at the clip's full height (round 37, finish:
    /// a song's sections read once across the tracks, the music under them). // r37:finish
    pub quiet: bool,
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
            // r37:finish — a light theme's body nearer its colour (the judge, Paper: a
            // saturated header beside a pale body read as two themes).
            mix(look.tint.large, ground, if t.dark { 0.55 } else { 0.25 }),
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
    let title_h = if look.quiet {
        0.0
    } else {
        title_height(body, k)
    }; // r37:finish
    if title_h > 0.0 {
        let title = Rect::from_min_max(body.min, pos2(body.right(), body.top() + title_h));
        b.rrect(
            title.shrink(b.px()),
            (r - b.px()).max(0.0),
            Fill::V(shade(bar_c, 0.09 * t.depth), shade(bar_c, -0.06 * t.depth)), // r37:finish (depth)
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
    // the clip that is on screen. A clip with no bar (a short row) scales the words to its
    // height — never drawn outside it — and hides them below a readable size: the name is
    // then in the hover hint (round 27).
    let fit = label_fit(title_h > 0.0, body.height(), look.label_row);
    let fit = fit.filter(|_| !look.quiet); // r37:finish
    if let (false, true, Some(fit)) = (look.renaming, body.width() >= 14.0 * k, fit) {
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
                pos2(left, line.center().y - 4.0 * k * fit),
                egui::vec2(8.0 * k * fit, 8.0 * k * fit),
            );
            crate::kit::glyph::paint(b, g, crate::kit::glyph::Glyph::Menu, ink);
            left = g.right();
        }
        // A linked clip: the chain, then the name; the count at the right end.
        let mut right = body.right() - 3.0 * k - look.reserve;
        if let Some(count) = linked {
            if right - left > 30.0 * k {
                let g = Rect::from_min_size(
                    pos2(left + 2.0 * k, line.center().y - 5.0 * k * fit),
                    egui::vec2(10.0 * k * fit, 10.0 * k * fit),
                );
                crate::kit::glyph::paint(b, g, crate::kit::glyph::Glyph::Link, ink);
                left = g.right();
            }
            let w = (6.0 * count.chars().count() as f32 + 4.0) * k;
            if right - left > w + 24.0 * k {
                let count_box =
                    Rect::from_min_max(pos2(right - w, line.top()), pos2(right, line.bottom()));
                let tx = Text::new(count, Role::Label, ink).align(Align2::RIGHT_CENTER);
                let _ = text::paint(b, count_box, &tx, s.tk() * fit);
                right = count_box.left() - 2.0 * k;
            }
        }
        let name_box =
            Rect::from_min_max(pos2(left + 3.0 * k, line.top()), pos2(right, line.bottom()));
        if name_box.width() > 6.0 * k {
            let tx = Text::new(name, Role::Label, ink)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, name_box, &tx, s.tk() * fit);
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
        // r37:finish — a lighter shade of the clip's own hue, read from across the room.
        // r37:finish — judge 8: notes at full saturation read as dash patterns; the
        // clip's own tone lightened, a step toward the text.
        mix(shade(look.tint.large, 0.55), t.text, 0.35)
    } else {
        // r37:finish — a deep tint of the clip's own colour (not black), muted toward the
        // ink: bars, not noise.
        mix(shade(look.tint.large, -0.50), t.text, 0.25)
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

/// Below this share of its size a clip label is guessed at rather than read: hidden.
const MIN_LABEL_FIT: f32 = 0.6;

/// How much a clip label's words are scaled: 1 on a title bar (sized for them); on a clip
/// with no bar, enough for the words to fit inside its height (`label_row` is their height
/// at full size); `None` — hidden — below a readable size.
pub(super) fn label_fit(bar: bool, body_h: f32, label_row: f32) -> Option<f32> {
    if bar {
        return Some(1.0);
    }
    let fit = ((body_h - 1.0) / label_row.max(1.0)).min(1.0);
    (fit >= MIN_LABEL_FIT).then_some(fit)
}

/// The height of a clip's title bar: none on a body too short to hold one.
pub(super) fn title_height(body: Rect, k: f32) -> f32 {
    if body.height() >= 22.0 * k {
        14.0 * k
    } else {
        0.0
    }
}

/// A small key with a letter on it (a track header's M and S), centred on `c`, `side`
/// across: the kit's object, lit through in `on` while it acts. // r37:finish
#[allow(clippy::too_many_arguments)]
pub(super) fn letter_key(
    b: &mut Brush,
    c: egui::Pos2,
    side: f32,
    letter: &str,
    lit: bool,
    on: egui::Color32,
    hovered: bool,
    alpha: f32,
    s: &Style,
) {
    let rect = key_face(b, c, side, lit, on, hovered, alpha, s);
    let t = &s.t;
    let ink = if lit {
        crate::kit::color::on_fill(on)
    } else {
        t.label
    };
    let _ = text::paint(
        b,
        rect,
        &Text::new(letter, Role::Section, ink.gamma_multiply(alpha))
            .align(Align2::CENTER_CENTER)
            .caps(false),
        s.tk(),
    );
}

/// r37:finish — the record arm: a key with the record dot (not a second "R" beside the
/// automation badge's), the dot red at rest and bright on a red key while armed.
#[allow(clippy::too_many_arguments)]
pub(super) fn dot_key(
    b: &mut Brush,
    c: egui::Pos2,
    side: f32,
    lit: bool,
    on: egui::Color32,
    hovered: bool,
    alpha: f32,
    s: &Style,
) {
    let rect = key_face(b, c, side, lit, on, hovered, alpha, s);
    let dot = if lit {
        crate::kit::color::mix(on, s.t.text, 0.55)
    } else {
        with_alpha(on, 0.85)
    };
    b.circle(
        rect.center(),
        side * 0.2,
        Fill::Solid(dot.gamma_multiply(alpha)),
    );
}

#[allow(clippy::too_many_arguments)]
fn key_face(
    b: &mut Brush,
    c: egui::Pos2,
    side: f32,
    lit: bool,
    on: egui::Color32,
    hovered: bool,
    alpha: f32,
    s: &Style,
) -> Rect {
    let t = &s.t;
    let rect = Rect::from_center_size(c, vec2(side, side * 0.86));
    let m = light::Material::of(t);
    let base = if lit {
        on
    } else if hovered {
        shade(t.panel_raised, 0.06)
    } else {
        t.panel_raised
    };
    light::object(
        b,
        rect,
        s.r(2.5),
        base,
        &m,
        light::ObjectOpts {
            drop: false,
            k: s.k,
            alpha,
            ..Default::default()
        },
    );
    rect
}

#[cfg(test)]
mod tests {
    use super::{dashes, label_fit};

    /// A clip label fits its clip: full size on a bar or a tall body, scaled on a short
    /// one, hidden below six tenths of its size (round 27).
    #[test]
    fn a_clip_label_is_scaled_to_a_short_clip_or_hidden() {
        assert_eq!(label_fit(true, 8.0, 14.0), Some(1.0));
        assert_eq!(label_fit(false, 30.0, 14.0), Some(1.0));
        assert_eq!(label_fit(false, 11.5, 14.0), Some(10.5 / 14.0));
        assert_eq!(label_fit(false, 7.0, 14.0), None);
    }

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
