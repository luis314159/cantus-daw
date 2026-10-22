//! Painting the piano roll with the kit (ADR 0024): FL's grid ladder, a real piano
//! keyboard, pastel top-lit notes, and the velocity lane's stems.
//!
//! Everything here reads and draws; nothing senses input and nothing edits, so the view
//! paints with the project borrowed and acts afterwards — which is what lets it stop
//! cloning the clip (every note) every frame. One brush per region: the grid and every
//! note in it are one mesh.

use super::{is_black, Geo, SelSet};
use crate::app::CantusApp;
use crate::kit::color::{mix, on_fill, shade, with_alpha, Hsl};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::{beats_to_ticks, ticks_to_beats};
use cantus_core::{Clip, Ticks};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

const GRID_TOKENS: &[Tok] = &[
    Tok::Glow,
    Tok::GlowCool,
    Tok::RowWhite,
    Tok::RowBlack,
    Tok::Ink,
    Tok::Grid,
    Tok::GridStrong,
    Tok::GridBar,
];
const KEY_TOKENS: &[Tok] = &[Tok::KeyWhite, Tok::KeyBlack, Tok::Selection];
const RULER_TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::RowBlack,
    Tok::GridStrong,
    Tok::GridBar,
    Tok::Text,
    Tok::TextDim,
    Tok::Playhead,
];
const NOTE_TOKENS: &[Tok] = &[Tok::Selection, Tok::TextDim, Tok::Danger];

/// The colours of a note, from its track's colour: FL's pastel with a top-lit gradient,
/// a darker outline, and a label in a deep tint of the same hue (FL's rule 5).
#[derive(Copy, Clone, Debug)]
pub(super) struct NoteInk {
    pub top: Color32,
    pub bottom: Color32,
    pub outline: Color32,
    pub label: Color32,
}

impl NoteInk {
    pub fn of(c: Color32, s: &Style) -> NoteInk {
        let h = Hsl::of(c);
        let sat = h.s.clamp(0.35, 0.75);
        let at = |l: f32| Hsl { h: h.h, s: sat, l }.rgb();
        if s.t.dark {
            NoteInk {
                top: at(0.86),
                bottom: at(0.78),
                outline: at(0.52),
                label: at(0.26),
            }
        } else {
            NoteInk {
                top: at(0.72),
                bottom: at(0.64),
                outline: at(0.40),
                label: at(0.16),
            }
        }
    }

    /// The selection's ink: the same recipe on the selection colour.
    pub fn selected(s: &Style) -> NoteInk {
        let c = s.t.selection;
        NoteInk {
            top: shade(c, 0.06),
            bottom: shade(c, -0.04),
            outline: shade(c, -0.32),
            label: on_fill(c),
        }
    }

    /// A muted note is a ghost: flat, the ground's colour lifted, its outline kept.
    pub fn muted(s: &Style) -> NoteInk {
        let ground = mix(s.t.row_white, s.t.text_dim, 0.28);
        NoteInk {
            top: ground,
            bottom: ground,
            outline: mix(ground, s.t.text_dim, 0.5),
            label: s.t.text_dim,
        }
    }
}

/// The grid: rows in the editor's two row colours (black-key rows darker, as on the
/// keyboard beside them; a kit's pads simply alternate), what lies outside the clip a
/// step darker, an octave line at each C, and FL's line ladder — step, beat, and a bar
/// line three pixels wide with a dark core.
pub(super) fn grid(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    clip: &Clip,
    grid_ticks: Ticks,
    s: &Style,
) {
    let t = &s.t;
    let g = b.snap_rect(geo.grid);
    b.register(g, "piano_grid", GRID_TOKENS, &[]);
    b.rect(g, Fill::Solid(t.row_white));
    let lo = geo.lane(g.bottom()).floor().max(0.0) as i32;
    let hi = (geo.lane(g.top()).ceil() as i32).min(geo.axis.len() as i32);
    for lane in lo..hi {
        let y1 = geo.y(lane as f32);
        let y0 = y1 - geo.key_h;
        let dark = if geo.axis.pads() {
            lane % 2 == 1
        } else {
            is_black(geo.axis.pitch(lane as usize))
        };
        if dark {
            b.rect(
                b.snap_rect(Rect::from_min_max(pos2(g.left(), y0), pos2(g.right(), y1))),
                Fill::Solid(t.row_black),
            );
        }
        if !geo.axis.pads() && geo.axis.pitch(lane as usize).is_multiple_of(12) {
            b.hline(
                g.left(),
                g.right(),
                y1 - 0.5 * b.px(),
                b.px(),
                t.grid_strong,
            );
        } else if geo.axis.pads() {
            b.hline(g.left(), g.right(), y1 - 0.5 * b.px(), b.px(), t.grid);
        }
    }
    // The theme's glows (ADR 0046 §8): over the rows, under the lines and the notes.
    crate::kit::light::ambient(b, g, t);
    // Before and after the clip's content: "not part of this clip", a step darker.
    let outside = shade(t.row_black, -0.07);
    let window = clip.period().map_or(clip.length, |p| p.min(clip.length));
    let start_x = geo.x(ticks_to_beats(clip.content_start()));
    if start_x > g.left() {
        b.rect(
            b.snap_rect(Rect::from_min_max(
                g.min,
                pos2(start_x.min(g.right()), g.bottom()),
            )),
            Fill::Solid(with_alpha(outside, 0.7)),
        );
    }
    let end_x = geo.x(ticks_to_beats(clip.content_start() + window));
    if end_x < g.right() {
        b.rect(
            b.snap_rect(Rect::from_min_max(
                pos2(end_x.max(g.left()), g.top()),
                g.max,
            )),
            Fill::Solid(with_alpha(outside, 0.7)),
        );
    }
    ladder(b, app, geo, g, clip, grid_ticks, s);
}

/// The vertical lines over `rect`: every grid step, every beat, every bar.
pub(super) fn ladder(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    rect: Rect,
    clip: &Clip,
    grid_ticks: Ticks,
    s: &Style,
) {
    let t = &s.t;
    let map = crate::views::bars::map(app.project());
    if !map.is_one_metre() {
        metre_ladder(b, &map, geo, rect, clip, grid_ticks, s);
        return;
    }
    let ts = app.project().time_signature;
    let (bar, beat) = (ts.ticks_per_bar(), ts.ticks_per_beat());
    let mut step = grid_ticks.max(Ticks(1));
    while ticks_to_beats(step) * geo.px_per_beat < 6.0 {
        step = step * 2;
    }
    let side = mix(t.grid_bar, t.row_black, 0.55);
    let first = beats_to_ticks(geo.beats(rect.left()).max(0.0)).snap_floor(step);
    let last = beats_to_ticks(geo.beats(rect.right()));
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        if tick.0 % bar.0 == 0 {
            b.vline(x, rect.top(), rect.bottom(), 3.0 * b.px(), side);
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_bar);
        } else if tick.0 % beat.0 == 0 {
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_strong);
        } else {
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid);
        }
        tick += step;
    }
}

/// A real keyboard: full white keys, each spanning half of the black rows beside it, so
/// the white keys have a piano's proportions, and short black keys over them with a lit
/// front — contrasting with the panel in every theme (`cantus-audit.md` #31: the old keys
/// vanished into the background). A kit's rows are named pads instead. `hover` lights the
/// key under the pointer.
pub(super) fn keys(b: &mut Brush, geo: &Geo<'_>, rect: Rect, hover: Option<usize>, s: &Style) {
    let t = &s.t;
    let rect = b.snap_rect(rect);
    b.register(rect, "keyboard", KEY_TOKENS, &[]);
    let lo = geo.lane(rect.bottom()).floor().max(0.0) as i32;
    let hi = (geo.lane(rect.top()).ceil() as i32).min(geo.axis.len() as i32);
    let ink = on_fill(t.key_white);
    let lit = |c: Color32, lane: usize| {
        if hover == Some(lane) {
            mix(c, t.selection, 0.45)
        } else {
            c
        }
    };
    if geo.axis.pads() {
        b.rect(rect, Fill::Solid(shade(t.key_white, -0.12)));
        for lane in lo..hi {
            let y1 = geo.y(lane as f32);
            let r = Rect::from_min_max(pos2(rect.left(), y1 - geo.key_h), pos2(rect.right(), y1));
            let base = if lane % 2 == 1 {
                shade(t.key_white, -0.05)
            } else {
                t.key_white
            };
            light::raised(
                b,
                r.shrink2(vec2(1.0, 0.5)),
                s.r(2.0),
                lit(base, lane as usize),
                RaisedOpts {
                    drop: false,
                    k: s.k,
                    depth: t.depth,
                    ..Default::default()
                },
            );
            if geo.key_h >= 9.0 {
                let name = Text::new(geo.axis.name_ref(lane as usize), Role::Label, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::Middle));
                let _ = text::paint(b, r.shrink2(vec2(6.0 * s.k, 0.0)), &name, s.tk());
            }
        }
        return;
    }
    // The white keys first, each stretched over half of each black row beside it.
    b.rect(rect, Fill::Solid(shade(t.key_white, -0.18)));
    let edge = shade(t.key_white, -0.20);
    for lane in (lo - 1).max(0)..(hi + 1).min(geo.axis.len() as i32) {
        let pitch = geo.axis.pitch(lane as usize);
        if is_black(pitch) {
            continue;
        }
        let half = geo.key_h * 0.5;
        let above = pitch < 127 && is_black(pitch + 1);
        let below = pitch > 0 && is_black(pitch - 1);
        let top = geo.y(lane as f32 + 1.0) - if above { half } else { 0.0 };
        let bottom = geo.y(lane as f32) + if below { half } else { 0.0 };
        let r = b.snap_rect(Rect::from_min_max(
            pos2(rect.left(), top),
            pos2(rect.right(), bottom),
        ));
        let face = lit(t.key_white, lane as usize);
        b.rect(r, Fill::V(shade(face, 0.02), shade(face, -0.03)));
        // The shade at the back of the key, where it goes under the case.
        b.rect(
            Rect::from_min_max(r.min, pos2(r.left() + 5.0 * s.k, r.bottom())),
            Fill::Solid(with_alpha(t.dark_shade, 0.08 * t.depth)),
        );
        // The key's front lip, where the finger lands: a step darker, as the light
        // falls off the rounded end.
        b.rect(
            Rect::from_min_max(pos2(r.right() - 4.0 * s.k, r.top()), r.max),
            Fill::Solid(shade(face, -0.07 * t.depth.max(0.3))),
        );
        b.hline(r.left(), r.right(), r.bottom() - 0.5 * b.px(), b.px(), edge);
        if pitch.is_multiple_of(12) && geo.key_h >= 8.0 {
            let label = Text::new(super::pitch_label(pitch), Role::Label, ink)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let lr = Rect::from_min_max(
                pos2(
                    r.left() + rect.width() * 0.62,
                    r.bottom() - geo.key_h.max(12.0 * s.k),
                ),
                pos2(r.right() - 4.0 * s.k, r.bottom()),
            );
            let _ = text::paint(b, lr, &label, s.tk());
        }
    }
    // The black keys over them: short, raised, their front lit.
    let bw = (rect.width() * 0.60).round();
    for lane in lo..hi {
        let pitch = geo.axis.pitch(lane as usize);
        if !is_black(pitch) {
            continue;
        }
        let y1 = geo.y(lane as f32);
        let r = Rect::from_min_max(
            pos2(rect.left(), y1 - geo.key_h + 0.5),
            pos2(rect.left() + bw, y1 - 0.5),
        );
        let face = lit(t.key_black, lane as usize);
        light::raised(
            b,
            r,
            s.r(1.5),
            face,
            RaisedOpts {
                drop: true,
                k: s.k * 0.6,
                depth: t.depth,
                ..Default::default()
            },
        );
        let front = Rect::from_min_max(
            pos2(r.right() - (r.width() * 0.18).max(4.0), r.top() + 1.5),
            pos2(r.right() - 1.5, r.bottom() - 1.5),
        );
        b.rrect(
            front,
            s.r(1.0),
            Fill::V(shade(face, 0.16), shade(face, 0.08)),
        );
    }
    b.vline(
        rect.right() - 0.5 * b.px(),
        rect.top(),
        rect.bottom(),
        b.px(),
        shade(t.key_white, -0.45),
    );
}

/// The ruler over the grid: bar numbers in the *arrangement's* count, thinned by doubling
/// as they crowd, beat ticks, and the playhead's marker.
pub(super) fn ruler(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    rect: Rect,
    clip: &Clip,
    playhead: Option<f32>,
    s: &Style,
) {
    let t = &s.t;
    let rect = b.snap_rect(rect);
    b.register(rect, "piano_ruler", RULER_TOKENS, &[]);
    let ground = mix(t.ink, t.row_black, 0.55);
    b.rect(rect, Fill::V(shade(ground, 0.02), ground));
    b.hline(
        rect.left(),
        rect.right(),
        rect.bottom() - 0.5 * b.px(),
        b.px(),
        t.grid_bar,
    );
    let map = crate::views::bars::map(app.project());
    if !map.is_one_metre() {
        metre_ruler(b, &map, geo, rect, clip, s);
        playhead_mark(b, rect, playhead, s);
        return;
    }
    let ts = app.project().time_signature;
    let bar = ts.ticks_per_bar();
    let beat = ts.ticks_per_beat();
    let bar_px = ticks_to_beats(bar) * geo.px_per_beat;
    let mut per_label = 1i64;
    while bar_px * (per_label as f32) < super::RULER_LABEL_GAP * s.k {
        per_label *= 2;
    }
    let step = bar * per_label;
    let to_timeline = clip.start - clip.offset;
    let first = (beats_to_ticks(geo.beats(rect.left()).max(0.0)) + to_timeline).snap_floor(step)
        - to_timeline;
    let last = beats_to_ticks(geo.beats(rect.right()));
    // Beat ticks along the floor.
    if ticks_to_beats(beat) * geo.px_per_beat >= 6.0 {
        let mut tick = beats_to_ticks(geo.beats(rect.left()).max(0.0)).snap_floor(beat);
        while tick <= last {
            let x = geo.x(ticks_to_beats(tick));
            b.vline(
                x,
                rect.bottom() - 4.0 * s.k,
                rect.bottom(),
                b.px(),
                t.grid_strong,
            );
            tick += beat;
        }
    }
    let mut digits = crate::views::clips_chrome::Digits::default();
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        let (n, _, _) = ts.bbt(tick + to_timeline);
        b.vline(x, rect.top() + 3.0 * s.k, rect.bottom(), b.px(), t.grid_bar);
        let color = if (n - 1) % 4 == 0 { t.text } else { t.text_dim };
        let label = Text::new(digits.fmt(n), Role::Label, color)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let lr = Rect::from_min_max(
            pos2(x + 3.0 * s.k, rect.top()),
            pos2(x + bar_px * per_label as f32 - 2.0, rect.bottom()),
        );
        let _ = text::paint(b, lr, &label, s.tk());
        tick += step;
    }
    if let Some(x) = playhead {
        if x >= rect.left() - 6.0 && x <= rect.right() + 6.0 {
            let k = s.k;
            b.polygon(
                &[
                    pos2(x - 5.0 * k, rect.bottom() - 8.0 * k),
                    pos2(x + 5.0 * k, rect.bottom() - 8.0 * k),
                    pos2(x, rect.bottom()),
                ],
                t.playhead,
            );
        }
    }
}

/// How much detail a note is worth at its size on screen. Zoomed out, a note is a
/// fraction of a pixel and its corners and outline cost triangles nobody can see.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Lod {
    /// One flat quad.
    Speck,
    /// An outline and a face, square: four triangles.
    Block,
    /// The full recipe: rounded, top-lit, a highlight edge, a label when it fits.
    Full,
}

impl Lod {
    pub fn of(r: Rect) -> Lod {
        let (w, h) = (r.width(), r.height());
        if w < 4.0 || h < 4.0 {
            Lod::Speck
        } else if w < 8.0 || h < 8.0 {
            Lod::Block
        } else {
            Lod::Full
        }
    }
}

/// One note: FL's pastel, top-lit, outlined; its name inside when there is room.
/// `vel` fades a quiet note toward the row it sits on.
pub(super) fn note(
    b: &mut Brush,
    r: Rect,
    ink: &NoteInk,
    vel: f32,
    ground: Color32,
    label: Option<&str>,
    s: &Style,
) {
    let fade = (1.0 - vel.clamp(0.0, 1.0)) * 0.45;
    let top = mix(ink.top, ground, fade);
    let bottom = mix(ink.bottom, ground, fade);
    match Lod::of(r) {
        Lod::Speck => b.rect(r, Fill::Solid(mix(top, bottom, 0.5))),
        Lod::Block => {
            b.rect(r, Fill::Solid(ink.outline));
            b.rect(r.shrink(b.px()), Fill::V(top, bottom));
        }
        Lod::Full => {
            let rad = s.r(2.0);
            b.rrect(r, rad, Fill::Solid(ink.outline));
            let inner = r.shrink(b.px());
            b.rrect(inner, (rad - b.px()).max(0.0), Fill::V(top, bottom));
            b.hline(
                inner.left() + rad,
                inner.right() - rad,
                inner.top() + 0.5 * b.px(),
                b.px(),
                shade(top, 0.06 * s.t.depth),
            );
            if let Some(l) = label {
                if r.width() > 22.0 * s.k && r.height() >= 10.0 * s.k {
                    let tx = Text::new(l, Role::Micro, ink.label)
                        .caps(false)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End));
                    let _ = text::paint(
                        b,
                        Rect::from_min_max(
                            pos2(r.left() + 3.0 * s.k, r.top()),
                            pos2(r.right() - 2.0, r.bottom()),
                        ),
                        &tx,
                        s.tk(),
                    );
                }
            }
        }
    }
}

/// The notes on screen: the visible range only (notes are sorted by start, so the loop
/// stops at the right edge), each drawn at its level of detail into the one mesh.
/// Returns how many were drawn.
#[allow(clippy::too_many_arguments)]
pub(super) fn notes(
    b: &mut Brush,
    geo: &Geo<'_>,
    clip: &Clip,
    sel: &SelSet,
    selected_ids: &[cantus_core::NoteId],
    track_color: Color32,
    s: &Style,
) -> usize {
    let g = geo.grid;
    b.register(g, "notes", NOTE_TOKENS, &[]);
    let normal = NoteInk::of(track_color, s);
    let chosen = NoteInk::selected(s);
    let ghost = NoteInk::muted(s);
    let visible_start = beats_to_ticks(geo.beats(g.left())) - Ticks::beats(64);
    let visible_end = beats_to_ticks(geo.beats(g.right()));
    let lo_lane = geo.lane(g.bottom()).floor() as i32 - 1;
    let hi_lane = geo.lane(g.top()).ceil() as i32 + 1;
    let mut drawn = 0usize;
    // Sorted by start: begin at the first note that can reach the left edge.
    let mut specks: [[Option<Color32>; 8]; 2] = [[None; 8]; 2];
    let first = clip.notes.partition_point(|n| n.start < visible_start);
    for n in &clip.notes[first..] {
        if n.start > visible_end {
            break; // sorted by start
        }
        let Some(lane) = geo.axis.lane_of(n.pitch) else {
            continue;
        };
        if n.end() < visible_start || (lane as i32) < lo_lane || (lane as i32) > hi_lane {
            continue;
        }
        let r = geo.note_rect(n);
        if r.right() < g.left() || r.left() > g.right() {
            continue;
        }
        let selected = sel.contains(selected_ids, n.id);
        let ink = if selected {
            &chosen
        } else if n.muted {
            &ghost
        } else {
            &normal
        };
        let ground = if geo.axis.pads() || !is_black(n.pitch) {
            s.t.row_white
        } else {
            s.t.row_black
        };
        let vel = if selected {
            1.0
        } else {
            n.velocity as f32 / 127.0
        };
        if Lod::of(r) == Lod::Speck && !selected && !n.muted {
            // Specks come in eight shades per row colour, worked out once a frame: the
            // whole piece zoomed out is twenty thousand of them.
            let row = usize::from(ground != s.t.row_white);
            let v = (n.velocity as usize * 8 / 128).min(7);
            let c = *specks[row][v].get_or_insert_with(|| {
                let fade = (1.0 - (v as f32 + 0.5) / 8.0) * 0.45;
                mix(
                    mix(normal.top, ground, fade),
                    mix(normal.bottom, ground, fade),
                    0.5,
                )
            });
            b.rect(r, Fill::Solid(c));
        } else {
            let label = (geo.key_h >= 11.0).then(|| geo.axis.name_ref(lane));
            note(b, r, ink, vel, ground, label, s);
        }
        drawn += 1;
    }
    drawn
}

/// What the lane for velocities draws: FL's event lane — a stem at each note's start with
/// a knob on top and a line along the note's length, over the grid's own ladder.
#[allow(clippy::too_many_arguments)]
pub(super) fn velocity(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    lane: Rect,
    clip: &Clip,
    sel: &SelSet,
    selected_ids: &[cantus_core::NoteId],
    track_color: Color32,
    grid_ticks: Ticks,
    s: &Style,
) {
    let t = &s.t;
    let lane = b.snap_rect(lane);
    b.register(lane, "velocity_lane", GRID_TOKENS, &[]);
    b.rect(lane, Fill::Solid(mix(t.row_black, t.ink, 0.25)));
    ladder(b, app, geo, lane, clip, grid_ticks, s);
    let (top, bottom) = (lane.top() + 8.0 * s.k, lane.bottom() - 3.0 * s.k);
    let normal = NoteInk::of(track_color, s);
    let chosen = NoteInk::selected(s);
    let end = beats_to_ticks(geo.beats(lane.right()));
    // Knobs and tails while a sixteenth is wider than a knob; hairlines below that.
    let heads = geo.px_per_beat >= 24.0;
    let (mut last_px, mut last_y) = (i32::MIN, f32::MAX);
    let from = beats_to_ticks(geo.beats(lane.left())) - Ticks::beats(64);
    let first = clip.notes.partition_point(|n| n.start < from);
    for n in &clip.notes[first..] {
        if n.start > end {
            break;
        }
        let x = geo.x(ticks_to_beats(n.start));
        let x1 = geo.x(ticks_to_beats(n.end())).min(lane.right());
        if x1 < lane.left() - 4.0 {
            continue;
        }
        let y = bottom - (bottom - top) * n.velocity as f32 / 127.0;
        let selected = sel.contains(selected_ids, n.id);
        let c = if selected {
            chosen.top
        } else if n.muted {
            t.text_dim
        } else {
            normal.bottom
        };
        if !heads {
            // Many notes start in one pixel column: the tallest stem is the one seen.
            let px = (x * b.ppp()).round() as i32;
            if px == last_px && y >= last_y {
                continue;
            }
            if px != last_px {
                last_y = f32::MAX;
            }
            last_px = px;
            last_y = last_y.min(y);
            // Zoomed out, stems are a pixel apart: a hairline each, no knob.
            b.vline(x, y, bottom, b.px(), c);
            continue;
        }
        b.vline(x, y, bottom, (1.5 * s.k).max(b.px()), c);
        if x1 > x + 3.0 {
            b.hline(x, x1, y, b.px(), with_alpha(c, 0.8));
        }
        b.circle(Pos2::new(x, y), 3.0 * s.k, Fill::Solid(shade(c, -0.25)));
        b.circle(Pos2::new(x, y), 2.0 * s.k, Fill::Solid(c));
    }
}

/// The notes being recorded, from their start to the playhead.
pub(super) fn recording(b: &mut Brush, app: &CantusApp, geo: &Geo<'_>, clip: &Clip, s: &Style) {
    if !app.recorder.is_recording() {
        return;
    }
    let ph = ticks_to_beats(clip.content_of(app.engine.position()));
    for p in app.recorder.pending() {
        let x0 = geo.x(ticks_to_beats(clip.content_of(p.start)));
        let x1 = geo.x(ph).max(x0 + 2.0);
        // A live note on a pitch this kit has no pad for has nowhere to be drawn yet;
        // committing the take gives it a row of its own (`Axis::build`).
        let Some(y0) = geo.y_of_pitch(p.pitch) else {
            continue;
        };
        let r = Rect::from_min_max(pos2(x0, y0 + 1.0), pos2(x1, y0 + geo.key_h - 1.0));
        b.rrect(r, s.r(2.0), Fill::Solid(with_alpha(s.t.danger, 0.8)));
    }
}

/// The ladder under metre changes (ADR 0060): the arrangement's bars and beats over the
/// clip's content (content tick `c` is timeline tick `c + start − offset`), and the grid's
/// steps counted from each bar's start.
#[allow(clippy::too_many_arguments)]
fn metre_ladder(
    b: &mut Brush,
    map: &cantus_core::TempoMap,
    geo: &Geo<'_>,
    rect: Rect,
    clip: &Clip,
    grid_ticks: Ticks,
    s: &Style,
) {
    let t = &s.t;
    let to_timeline = clip.start - clip.offset;
    let side = mix(t.grid_bar, t.row_black, 0.55);
    let mut step = grid_ticks.max(Ticks(1));
    while ticks_to_beats(step) * geo.px_per_beat < 6.0 {
        step = step * 2;
    }
    let first = beats_to_ticks(geo.beats(rect.left()).max(0.0)) + to_timeline;
    let last = beats_to_ticks(geo.beats(rect.right())) + to_timeline;
    for bar in map.bar_lines(map.bar_start(first), last + Ticks(1)) {
        let end = map.bar_end(bar);
        let beat = map.beat_len_at(bar).max(Ticks(1));
        let mut tick = bar;
        while tick < end && tick <= last {
            let x = geo.x(ticks_to_beats(tick - to_timeline));
            if tick == bar {
                b.vline(x, rect.top(), rect.bottom(), 3.0 * b.px(), side);
                b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_bar);
            } else if (tick - bar).0 % beat.0 == 0 {
                b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid_strong);
            } else {
                b.vline(x, rect.top(), rect.bottom(), b.px(), t.grid);
            }
            tick += step;
        }
    }
}

/// The ruler's bar numbers and beat ticks under metre changes (ADR 0060), in the
/// arrangement's count.
fn metre_ruler(
    b: &mut Brush,
    map: &cantus_core::TempoMap,
    geo: &Geo<'_>,
    rect: Rect,
    clip: &Clip,
    s: &Style,
) {
    let t = &s.t;
    let to_timeline = clip.start - clip.offset;
    let first = beats_to_ticks(geo.beats(rect.left()).max(0.0)) + to_timeline;
    let last = beats_to_ticks(geo.beats(rect.right())) + to_timeline;
    let beat_px = ticks_to_beats(map.beat_len_at(first)) * geo.px_per_beat;
    if beat_px >= 6.0 {
        for (tick, _) in map.beat_lines(first, last + Ticks(1)) {
            let x = geo.x(ticks_to_beats(tick - to_timeline));
            b.vline(
                x,
                rect.bottom() - 4.0 * s.k,
                rect.bottom(),
                b.px(),
                t.grid_strong,
            );
        }
    }
    // Thinned by the narrowest bar in view, so numbers never collide.
    let mut narrow = f32::MAX;
    for bar in map.bar_lines(map.bar_start(first), last + Ticks(1)) {
        narrow = narrow.min(ticks_to_beats(map.bar_end(bar) - bar) * geo.px_per_beat);
    }
    let mut every = 1i64;
    while narrow * (every as f32) < super::RULER_LABEL_GAP * s.k && every < 1024 {
        every *= 2;
    }
    let mut digits = crate::views::clips_chrome::Digits::default();
    for bar in map.bar_lines(map.bar_start(first), last + Ticks(1)) {
        let (n, _, _) = map.bbt(bar);
        if (n - 1).rem_euclid(every) != 0 {
            continue;
        }
        let x = geo.x(ticks_to_beats(bar - to_timeline));
        b.vline(x, rect.top() + 3.0 * s.k, rect.bottom(), b.px(), t.grid_bar);
        let color = if (n - 1) % 4 == 0 { t.text } else { t.text_dim };
        let label = Text::new(digits.fmt(n), Role::Label, color)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let lr = Rect::from_min_max(
            pos2(x + 3.0 * s.k, rect.top()),
            pos2(x + narrow * every as f32 - 2.0, rect.bottom()),
        );
        let _ = text::paint(b, lr, &label, s.tk());
    }
}

/// The playhead's marker on a ruler.
fn playhead_mark(b: &mut Brush, rect: Rect, playhead: Option<f32>, s: &Style) {
    let t = &s.t;
    if let Some(x) = playhead {
        if x >= rect.left() - 6.0 && x <= rect.right() + 6.0 {
            let k = s.k;
            b.polygon(
                &[
                    pos2(x - 5.0 * k, rect.bottom() - 8.0 * k),
                    pos2(x + 5.0 * k, rect.bottom() - 8.0 * k),
                    pos2(x, rect.bottom()),
                ],
                t.playhead,
            );
        }
    }
}
