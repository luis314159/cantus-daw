//! Painting the piano roll with the kit (ADR 0024): FL's grid ladder, a real piano
//! keyboard, pastel top-lit notes, and the velocity lane's stems.
//!
//! Everything here reads and draws; nothing senses input and nothing edits, so the view
//! paints with the project borrowed and acts afterwards — which is what lets it stop
//! cloning the clip (every note) every frame. One brush per region: the grid and every
//! note in it are one mesh.

use super::{is_black, Geo, SelSet};
use crate::app::CantusApp;
use crate::kit::color::{mix, on_fill, shade, with_alpha};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::{beats_to_ticks, ticks_to_beats};
use cantus_core::{Clip, Ticks};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// How much of the selection tint a selected note takes: a note is small (r39:lapis2).
const NOTE_TINT: f32 = 3.0;

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
    Tok::Readout,
    Tok::ReadoutSoft,
];
const NOTE_TOKENS: &[Tok] = &[
    Tok::Selection,
    Tok::TextDim,
    Tok::Danger,
    Tok::SelectionTint,
];

/// The colours of a note in the design's room (round 39, des-views): the track's hue
/// over the base, brighter with velocity; a 1-px edge toward the gutter; a brighter
/// attack column so repeated notes part; its name in a deep ink (Night) or white (Paper).
/// Paper darkens the hue so a note holds on the light rows.
#[derive(Copy, Clone, Debug)]
pub(super) struct NoteInk {
    dark: bool,
    base: Color32,
    gutter: Color32,
    /// The track's hue as notes spend it (Paper: darkened 38 %).
    hue: Color32,
    /// The track's own hue, for the selection's lift in Night.
    own: Color32,
    /// The theme's selection tint (clear in Slate; r39:lapis2).
    tint: Color32,
    white: Color32,
    black: Color32,
    pub ring: Color32,
    pub label: Color32,
    pub muted_label: Color32,
    /// The edge of a loud note: the tab over the keyboard.
    pub outline: Color32,
}

impl NoteInk {
    pub fn of(c: Color32, s: &Style) -> NoteInk {
        let r = crate::views::room::Room::of(&s.t);
        let hue = if r.dark { c } else { mix(c, r.black, 0.38) };
        let mut ink = NoteInk {
            dark: r.dark,
            base: r.base,
            gutter: r.gutter,
            hue,
            own: c,
            tint: s.t.selection_tint,
            white: r.white,
            black: r.black,
            ring: r.ink1,
            label: if r.dark {
                mix(r.gutter, r.black, 0.35)
            } else {
                r.white
            },
            muted_label: r.ink3,
            outline: hue,
        };
        ink.outline = ink.edge(ink.fill(1.0));
        ink
    }

    /// The fill at velocity `v` (0…1).
    pub fn fill(&self, v: f32) -> Color32 {
        let v = v.clamp(0.0, 1.0);
        let a = if self.dark {
            0.50 + 0.22 * v
        } else {
            0.58 + 0.30 * v
        };
        mix(self.base, self.hue, a)
    }

    /// The selection's fill: the hue lifted (Night) or whole (Paper).
    /// Through the theme's selection tint, strongly: a note is small (none in Slate).
    pub fn selected(&self) -> Color32 {
        let c = if self.dark {
            mix(self.own, self.white, 0.30)
        } else {
            self.hue
        };
        crate::kit::color::selected_fill(c, self.tint, NOTE_TINT)
    }

    pub fn edge(&self, fill: Color32) -> Color32 {
        mix(fill, self.gutter, if self.dark { 0.55 } else { 0.35 })
    }

    pub fn attack(&self, fill: Color32) -> Color32 {
        if self.dark {
            mix(fill, self.white, 0.35)
        } else {
            mix(fill, self.black, 0.18)
        }
    }

    /// A muted note: hollow — its edge and a faint fill in the hue.
    pub fn muted(&self) -> (Color32, Color32) {
        (
            mix(self.base, self.hue, 0.55),
            mix(self.base, self.hue, 0.10),
        )
    }

    /// The ring of the note under the pointer.
    pub fn hover(&self, fill: Color32) -> Color32 {
        mix(fill, self.white, 0.60)
    }
}

/// How a note is shown.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum Look {
    Plain(f32),
    Selected,
    Muted,
}

/// The grid's inks in the design's room (round 39, des-views): white-key rows in the
/// base, black-key rows toward the gutter, the octave line under each C, a quieter line
/// at E|F, bars, beats and snap steps, and the veil outside the clip.
pub(super) struct GridInk {
    pub white: Color32,
    pub black: Color32,
    pub octave: Color32,
    pub ef: Color32,
    pub bar: Color32,
    pub beat: Color32,
    pub step: Color32,
    pub veil: Color32,
}

impl GridInk {
    pub fn of(s: &Style) -> GridInk {
        let r = crate::views::room::Room::of(&s.t);
        GridInk {
            white: r.base,
            black: mix(r.base, r.gutter, if r.dark { 0.62 } else { 0.30 }),
            octave: mix(r.grid_bar, r.ink4, 0.35),
            ef: mix(r.base, r.grid_bar, 0.55),
            bar: mix(r.grid_bar, r.ink4, 0.45),
            beat: r.grid_bar,
            step: r.grid_beat,
            veil: with_alpha(r.gutter, if r.dark { 0.55 } else { 0.42 }),
        }
    }
}

/// The grid: rows in the room's two values (black-key rows darker, as on the keyboard
/// beside them; a kit's pads simply alternate), the row of the key under the pointer
/// lit, an octave line at each C and a quieter one at E|F, the ladder, and the veil over
/// what lies outside the clip.
#[allow(clippy::too_many_arguments)]
pub(super) fn grid(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    clip: &Clip,
    grid_ticks: Ticks,
    lit_row: Option<usize>,
    s: &Style,
) {
    let t = &s.t;
    let ink = GridInk::of(s);
    let g = b.snap_rect(geo.grid);
    b.register(g, "piano_grid", GRID_TOKENS, &[]);
    b.rect(g, Fill::Solid(ink.white));
    let lo = geo.lane(g.bottom()).floor().max(0.0) as i32;
    let hi = (geo.lane(g.top()).ceil() as i32).min(geo.axis.len() as i32);
    let row = |b: &Brush, lane: i32| {
        let y1 = geo.y(lane as f32);
        b.snap_rect(Rect::from_min_max(
            pos2(g.left(), y1 - geo.key_h),
            pos2(g.right(), y1),
        ))
    };
    for lane in lo..hi {
        let pitch = geo.axis.pitch(lane as usize);
        let dark = if geo.axis.pads() {
            lane % 2 == 1
        } else {
            is_black(pitch)
        };
        let r = row(b, lane);
        if dark {
            b.rect(r, Fill::Solid(ink.black));
        }
        if lit_row == Some(lane as usize) {
            b.rect(r, Fill::Solid(with_alpha(t.selection, 0.07)));
        }
        let under = r.bottom() - 0.5 * b.px();
        if geo.axis.pads() {
            b.hline(g.left(), g.right(), under, b.px(), ink.step);
        } else if pitch.is_multiple_of(12) {
            b.hline(g.left(), g.right(), under, b.px(), ink.octave);
        } else if pitch % 12 == 5 {
            b.hline(g.left(), g.right(), under, b.px(), ink.ef);
        }
    }
    // The theme's glows (ADR 0046 §8): over the rows, under the lines and the notes.
    crate::kit::light::ambient(b, g, t);
    ladder(b, app, geo, g, clip, grid_ticks, s);
    // Before and after the clip's content: "not part of this clip", under a veil.
    let window = clip.period().map_or(clip.length, |p| p.min(clip.length));
    let start_x = geo.x(ticks_to_beats(clip.content_start()));
    if start_x > g.left() {
        b.rect(
            b.snap_rect(Rect::from_min_max(
                g.min,
                pos2(start_x.min(g.right()), g.bottom()),
            )),
            Fill::Solid(ink.veil),
        );
    }
    let end_x = geo.x(ticks_to_beats(clip.content_start() + window));
    if end_x < g.right() {
        b.rect(
            b.snap_rect(Rect::from_min_max(
                pos2(end_x.max(g.left()), g.top()),
                g.max,
            )),
            Fill::Solid(ink.veil),
        );
    }
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
    // r39:el-views — the room's ladder: bars, beats, and snap steps 6 px apart or more.
    let ink = GridInk::of(s);
    let (bar_ink, beat_ink, step_ink) = (ink.bar, ink.beat, ink.step);
    let first = beats_to_ticks(geo.beats(rect.left()).max(0.0)).snap_floor(step);
    let last = beats_to_ticks(geo.beats(rect.right()));
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        if tick.0 % bar.0 == 0 {
            b.vline(x, rect.top(), rect.bottom(), b.px(), bar_ink);
        } else if tick.0 % beat.0 == 0 {
            b.vline(x, rect.top(), rect.bottom(), b.px(), beat_ink);
        } else {
            b.vline(x, rect.top(), rect.bottom(), b.px(), step_ink);
        }
        tick += step;
    }
}

/// A real keyboard: full white keys, each spanning half of the black rows beside it, so
/// the white keys have a piano's proportions, and short black keys over them with a lit
/// front — contrasting with the panel in every theme (`cantus-audit.md` #31: the old keys
/// vanished into the background). A kit's rows are named pads instead. `hover` lights the
/// key under the pointer.
pub(super) fn keys(
    b: &mut Brush,
    geo: &Geo<'_>,
    rect: Rect,
    light: &super::keys::KeyLight,
    s: &Style,
) {
    let hover = light.hover;
    let t = &s.t;
    let rect = b.snap_rect(rect);
    b.register(rect, "keyboard", KEY_TOKENS, &[]);
    // r31 midi (ADR 0069): a keyboard folded to the key is drawn as named rows there.
    if !geo.axis.pads() && !geo.axis.is_chromatic() {
        return;
    }
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
    // r39:el-views — the keyboard as an instrument (des-views).
    super::keys::piano(b, geo, rect, light, s);
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
    // r39:el-views — the ruler on the room's surface, a hairline at its foot.
    let room = crate::views::room::Room::of(t);
    b.rect(rect, Fill::Solid(room.surface));
    b.hline(
        rect.left(),
        rect.right(),
        rect.bottom() - 0.5 * b.px(),
        b.px(),
        room.edge,
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
    // Sixteenths as 3-px ticks, beats as 6-px ticks named "41.2" where they have room.
    let beat_px = ticks_to_beats(beat) * geo.px_per_beat;
    let sixteenth = Ticks(beat.0 / 4).max(Ticks(1));
    let fine = ticks_to_beats(sixteenth) * geo.px_per_beat >= 6.0;
    if beat_px >= 6.0 {
        let step = if fine { sixteenth } else { beat };
        let mut tick = beats_to_ticks(geo.beats(rect.left()).max(0.0)).snap_floor(step);
        let mut digits = crate::views::clips_chrome::Digits::default();
        while tick <= last {
            let x = geo.x(ticks_to_beats(tick));
            let on_beat = tick.0 % beat.0 == 0;
            let h = if on_beat { 6.0 } else { 3.0 } * s.k;
            b.vline(x, rect.bottom() - h, rect.bottom(), b.px(), room.ink4);
            if on_beat && tick.0 % bar.0 != 0 && beat_px >= 34.0 * s.k {
                let (n, bt, _) = ts.bbt(tick + to_timeline);
                let label = Text::new(digits.fmt_pair(n, bt), Role::Micro, room.ink3)
                    .caps(false)
                    .align(Align2::LEFT_CENTER);
                let lr = Rect::from_min_max(
                    pos2(x + 3.0 * s.k, rect.top() + rect.height() * 0.45),
                    pos2(x + beat_px - 2.0, rect.bottom()),
                );
                let _ = text::paint(b, lr, &label, s.tk());
            }
            tick += step;
        }
    }
    let mut digits = crate::views::clips_chrome::Digits::default();
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        let (n, _, _) = ts.bbt(tick + to_timeline);
        b.vline(
            x,
            rect.bottom() - 9.0 * s.k,
            rect.bottom(),
            b.px(),
            room.ink3,
        );
        let label = Text::new(digits.fmt(n), Role::Section, t.readout_or(room.ink1, false))
            .caps(false)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let lr = Rect::from_min_max(
            pos2(x + 3.0 * s.k, rect.top() + rect.height() * 0.45),
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

/// One note (round 39, des-views): the row's full height, a 1-px edge, r 2, its fill by
/// velocity, a brighter attack column, its name inside from 20 px wide; selected, lit and
/// ringed in ink 1; muted, hollow; under the pointer, a light ring.
pub(super) fn note(
    b: &mut Brush,
    r: Rect,
    ink: &NoteInk,
    look: Look,
    hover: bool,
    label: Option<&str>,
    s: &Style,
) {
    let (fill, edge) = match look {
        Look::Plain(v) => {
            let f = ink.fill(v);
            (f, ink.edge(f))
        }
        Look::Selected => {
            let f = ink.selected();
            (f, ink.edge(f))
        }
        Look::Muted => {
            let (e, f) = ink.muted();
            (f, e)
        }
    };
    let px = b.px();
    match Lod::of(r) {
        Lod::Speck => b.rect(r, Fill::Solid(fill)),
        Lod::Block => {
            b.rect(r, Fill::Solid(edge));
            b.rect(r.shrink(px), Fill::Solid(fill));
        }
        Lod::Full => {
            let rad = s.r(2.0);
            if look == Look::Selected {
                b.rrect(r.expand(px), rad + px, Fill::Solid(ink.ring));
            } else if hover {
                b.rrect(r.expand(px), rad + px, Fill::Solid(ink.hover(fill)));
            }
            b.rrect(r, rad, Fill::Solid(edge));
            let inner = r.shrink(px);
            b.rrect(inner, (rad - px).max(0.0), Fill::Solid(fill));
            if look != Look::Muted {
                let w = (2.0 * px).max(s.k);
                let col = Rect::from_min_max(
                    inner.min,
                    pos2((inner.left() + w).min(inner.right()), inner.bottom()),
                );
                b.rect(b.snap_rect(col), Fill::Solid(ink.attack(fill)));
            }
            if let Some(l) = label {
                if r.width() >= 20.0 * s.k && r.height() >= 8.0 * s.k {
                    let c = if look == Look::Muted {
                        ink.muted_label
                    } else {
                        ink.label
                    };
                    let tx = Text::new(l, Role::Micro, c)
                        .caps(false)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End));
                    let _ = text::paint(
                        b,
                        Rect::from_min_max(
                            pos2(r.left() + 4.0 * s.k, r.top()),
                            pos2(r.right() - 2.0 * s.k, r.bottom()),
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
    hover: Option<cantus_core::NoteId>,
    s: &Style,
) -> usize {
    let g = geo.grid;
    b.register(g, "notes", NOTE_TOKENS, &[]);
    let ink = NoteInk::of(track_color, s);
    let visible_start = beats_to_ticks(geo.beats(g.left())) - Ticks::beats(64);
    let visible_end = beats_to_ticks(geo.beats(g.right()));
    let lo_lane = geo.lane(g.bottom()).floor() as i32 - 1;
    let hi_lane = geo.lane(g.top()).ceil() as i32 + 1;
    let mut drawn = 0usize;
    // Specks come in eight fills, worked out once a frame: the whole piece zoomed out is
    // twenty thousand of them.
    let specks: [Color32; 8] = std::array::from_fn(|v| ink.fill((v as f32 + 0.5) / 8.0));
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
        if Lod::of(r) == Lod::Speck && !selected && !n.muted {
            b.rect(
                r,
                Fill::Solid(specks[(n.velocity as usize * 8 / 128).min(7)]),
            );
        } else {
            let look = if selected {
                Look::Selected
            } else if n.muted {
                Look::Muted
            } else {
                Look::Plain(n.velocity as f32 / 127.0)
            };
            let label = (geo.key_h >= 9.0).then(|| geo.axis.name_ref(lane));
            note(b, r, &ink, look, hover == Some(n.id), label, s);
        }
        drawn += 1;
    }
    drawn
}

/// What the lane for velocities draws (round 39, des-views): over the grid's own ladder
/// and the lines at 127 and 64, one stem a note in the note's fill with a short cap —
/// the selection's stem wider and its cap in ink 1, a muted note's stem dotted and its
/// cap hollow. Where many notes start in one pixel column the tallest stem is the one
/// seen, and a chord shares one clean cap.
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
    field: NoteField,
    s: &Style,
) {
    let t = &s.t;
    let lane = b.snap_rect(lane);
    b.register(lane, "velocity_lane", GRID_TOKENS, &[]);
    let gi = GridInk::of(s);
    b.rect(lane, Fill::Solid(gi.white));
    ladder(b, app, geo, lane, clip, grid_ticks, s);
    let (top, bottom) = (lane.top() + 8.0 * s.k, lane.bottom() - 3.0 * s.k);
    let level = |v: f32| bottom - (bottom - top) * v / 127.0;
    for v in [127.0, 64.0] {
        b.hline(lane.left(), lane.right(), level(v), b.px(), gi.beat);
    }
    let ink = NoteInk::of(track_color, s);
    let room = crate::views::room::Room::of(t);
    let px = b.px();
    let end = beats_to_ticks(geo.beats(lane.right()));
    let (mut last_px, mut last_y) = (i32::MIN, f32::MAX);
    let from = beats_to_ticks(geo.beats(lane.left())) - Ticks::beats(64);
    let first = clip.notes.partition_point(|n| n.start < from);
    // Caps once the notes stand apart; hairlines below that.
    let caps = geo.px_per_beat >= 12.0;
    for n in &clip.notes[first..] {
        if n.start > end {
            break;
        }
        let x = b.snap_mid(geo.x(ticks_to_beats(n.start)));
        if x < lane.left() - 6.0 {
            continue;
        }
        let value = match field {
            NoteField::Velocity => n.velocity,
            NoteField::Release => n.release,
        };
        let y = level(value as f32);
        let selected = sel.contains(selected_ids, n.id);
        // A note with no release velocity (ADR 0068): a hollow ring on the floor.
        if field == NoteField::Release && value == 0 {
            let c = if selected { room.ink1 } else { room.ink3 };
            b.circle_stroke(Pos2::new(x, bottom), 2.5 * s.k, px, c);
            continue;
        }
        let fill = if selected {
            ink.selected()
        } else if n.muted {
            ink.muted().0
        } else {
            ink.fill(n.velocity as f32 / 127.0)
        };
        // Many notes start in one pixel column: the tallest stem is the one seen (a
        // chord's caps would stack into a smear), unless a selected one comes later.
        let col = (x * b.ppp()).round() as i32;
        if col == last_px && y >= last_y && !selected {
            continue;
        }
        if col != last_px {
            last_y = f32::MAX;
        }
        last_px = col;
        last_y = last_y.min(y);
        let w = if selected {
            2.0 * px.max(0.5 * s.k)
        } else {
            px.max(s.k)
        };
        if n.muted && !selected {
            let mut yy = y;
            while yy < bottom {
                b.vline(x, yy, (yy + 2.0 * s.k).min(bottom), w, fill);
                yy += 4.0 * s.k;
            }
        } else {
            b.vline(x, y, bottom, w, fill);
        }
        if caps {
            let cap =
                Rect::from_center_size(Pos2::new(x, y + s.k), egui::vec2(w + 5.0 * s.k, 2.0 * s.k));
            let cap = b.snap_rect(cap);
            if selected {
                b.rrect(cap, s.k, Fill::Solid(room.ink1));
            } else if n.muted {
                b.rrect_stroke(cap, s.k, px, fill);
            } else {
                b.rrect(cap, s.k, Fill::Solid(fill));
            }
        }
    }
}

/// What a notes lane draws (ADR 0068): each note's velocity, or its release velocity.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum NoteField {
    Velocity,
    Release,
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
    let to_timeline = clip.start - clip.offset;
    let ink = GridInk::of(s);
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
                b.vline(x, rect.top(), rect.bottom(), b.px(), ink.bar);
            } else if (tick - bar).0 % beat.0 == 0 {
                b.vline(x, rect.top(), rect.bottom(), b.px(), ink.beat);
            } else {
                b.vline(x, rect.top(), rect.bottom(), b.px(), ink.step);
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
        // r39:lapis2 — the bar numbers are readouts (the logo's gold in Slate Lapis).
        let color = if (n - 1) % 4 == 0 {
            t.readout_or(t.text, false)
        } else {
            t.readout_or(t.text_dim, true)
        };
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

/// A white key's rows within its octave, `(from, to)` in semitones from the octave's C:
/// C, D, E share the five rows C…E (5/3 each), F, G, A, B the seven rows F…B (7/4 each),
/// as a piano's white keys are of one width over black keys set between them. `None` for
/// a black key. // r37:finish
pub(crate) fn white_rows(pitch: u8) -> Option<(f32, f32)> {
    const LOW: [u8; 3] = [0, 2, 4];
    const HIGH: [u8; 4] = [5, 7, 9, 11];
    let n = pitch % 12;
    if let Some(i) = LOW.iter().position(|&w| w == n) {
        let w = 5.0 / 3.0;
        return Some((i as f32 * w, (i + 1) as f32 * w));
    }
    let i = HIGH.iter().position(|&w| w == n)?;
    let w = 7.0 / 4.0;
    Some((5.0 + i as f32 * w, 5.0 + (i + 1) as f32 * w))
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;

    /// The white keys against the note rows: seven keys fill the octave's twelve rows
    /// without a gap or an overlap; B|C and E|F are row edges (no black key between);
    /// every other boundary lies inside the row of the black key between its two keys;
    /// within each group the keys are of one width.
    #[test]
    fn the_white_keys_fall_on_a_keyboards_rows() {
        let whites: Vec<u8> = (0..12).filter(|p| !is_black(*p)).collect();
        assert_eq!(whites.len(), 7);
        let spans: Vec<(f32, f32)> = whites.iter().map(|&p| white_rows(p).unwrap()).collect();
        assert_eq!(spans[0].0, 0.0);
        assert!((spans[6].1 - 12.0).abs() < 1e-5);
        for w in spans.windows(2) {
            assert!(
                (w[0].1 - w[1].0).abs() < 1e-5,
                "no gap, no overlap: {spans:?}"
            );
        }
        // Each boundary: between C|D, D|E, F|G, G|A, A|B — inside the black key's row.
        for (i, pair) in whites.windows(2).enumerate() {
            let edge = spans[i].1;
            if pair[1] - pair[0] == 1 {
                assert!(
                    (edge - pair[1] as f32).abs() < 1e-5,
                    "E|F on a row edge: {edge}"
                );
            } else {
                let black = (pair[0] + 1) as f32;
                assert!(
                    edge > black + 0.2 && edge < black + 0.8,
                    "{}|{} under the black key's row {black}: {edge}",
                    pair[0],
                    pair[1]
                );
            }
        }
        let width = |p: u8| {
            let (a, b) = white_rows(p).unwrap();
            b - a
        };
        assert!((width(0) - width(4)).abs() < 1e-5 && (width(5) - width(11)).abs() < 1e-5);
        assert!(
            (width(0) - width(5)).abs() < 0.1,
            "the two groups' keys nearly one width"
        );
        for p in 0..12u8 {
            assert_eq!(white_rows(p).is_none(), is_black(p), "{p}");
            assert_eq!(white_rows(p + 60), white_rows(p), "every octave alike");
        }
    }
}
