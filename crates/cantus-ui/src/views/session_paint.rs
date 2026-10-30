//! The Session pane's painters (ADR 0077), on the kit's recipe: pure functions of a rect,
//! the style and what to show, so the shots and the pane draw the same thing.
//!
//! A slot with a clip is FL's clip face made a cell: a title line in the clip's colour
//! with the launcher at its left (▶; lit while it plays, blinking while queued, a red dot
//! recording), the name beside it, a miniature of its notes or its waveform under it on the
//! colour's deep ground, and the playing position as a bar along its foot. An empty slot
//! is a sunken well with its track's stop square (or, armed, the record dot).

use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style};
use crate::views::clips_chrome::Tint;
use cantus_core::Clip;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The Session glyph: a grid of cells, the one at the lower left playing.
pub fn glyph(b: &mut Brush, c: Pos2, u: f32, w: f32, color: Color32) {
    let p = |x: f32, y: f32| c + egui::Vec2::new(x * u, y * u);
    for row in 0..3 {
        for col in 0..3 {
            let x = -5.0 + col as f32 * 3.6;
            let y = -5.0 + row as f32 * 3.6;
            let r = Rect::from_min_max(p(x, y), p(x + 2.8, y + 2.8));
            if row == 2 && col == 0 {
                b.polygon(&[p(x, y), p(x + 2.8, y + 1.4), p(x, y + 2.8)], color);
            } else {
                b.rrect_stroke(r, 0.4 * u, w * 0.6, color);
            }
        }
    }
}

/// What a slot's launcher shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Launcher {
    /// Ready (▶ dim).
    Idle,
    /// Queued: blinking (`on` is this moment's half of the blink).
    Queued {
        on: bool,
    },
    Playing,
    /// Playing, and stopping at the boundary: blinking.
    Stopping {
        on: bool,
    },
    RecordQueued {
        on: bool,
    },
    Recording,
}

/// How a slot holding a clip looks this frame.
#[derive(Copy, Clone, Debug)]
pub struct SlotLook {
    pub tint: Tint,
    pub launcher: Launcher,
    /// Where it plays, 0…1 of its loop (or of its length).
    pub progress: Option<f32>,
    pub selected: bool,
    pub cursor: bool,
    pub hovered: bool,
    /// Its name is being typed in place.
    pub renaming: bool,
    /// Being dragged away: drawn faint.
    pub ghost: bool,
}

/// The launcher's square at a cell's left.
pub fn launcher_rect(cell: Rect, k: f32) -> Rect {
    let h = (16.0 * k).min(cell.height());
    Rect::from_min_size(pos2(cell.left(), cell.top()), vec2(h + 2.0 * k, h))
}

/// The title line's height.
fn title_h(cell: Rect, k: f32) -> f32 {
    (16.0 * k).min(cell.height())
}

fn launcher_glyph(b: &mut Brush, r: Rect, s: &Style, l: Launcher, ink: Color32) {
    let g = Rect::from_center_size(r.center(), vec2(9.0 * s.k, 9.0 * s.k));
    let (glyph, colour) = match l {
        Launcher::Idle => (Glyph::Play, with_alpha(ink, 0.75)),
        Launcher::Queued { on } => (
            Glyph::Play,
            if on { s.t.led } else { with_alpha(ink, 0.35) },
        ),
        Launcher::Playing => (Glyph::Play, s.t.led),
        Launcher::Stopping { on } => (
            Glyph::Play,
            if on {
                s.t.led
            } else {
                with_alpha(s.t.led, 0.35)
            },
        ),
        Launcher::RecordQueued { on } => (
            Glyph::Record,
            if on {
                s.t.danger
            } else {
                with_alpha(s.t.danger, 0.35)
            },
        ),
        Launcher::Recording => (Glyph::Record, s.t.danger),
    };
    if matches!(l, Launcher::Playing | Launcher::Recording) {
        b.circle(
            g.center(),
            6.5 * s.k,
            Fill::Solid(with_alpha(shade(colour, -0.5), 0.55)),
        );
    }
    glyph::paint(b, g, glyph, colour);
}

/// A slot holding `clip`.
pub fn slot_clip(
    b: &mut Brush,
    cell: Rect,
    s: &Style,
    clip: &Clip,
    wave: Option<&[(f32, f32)]>,
    look: SlotLook,
) {
    let k = s.k;
    let t = &s.t;
    let cell = b.snap_rect(cell);
    let r = s.r(2.5);
    let alpha = if look.ghost { 0.35 } else { 1.0 };
    let ground = if t.dark { t.row_black } else { t.row_white };
    let bar_c = look.tint.large;
    let body_c = mix(bar_c, ground, if t.dark { 0.55 } else { 0.45 });
    let body_c = if look.selected {
        shade(body_c, if t.dark { 0.08 } else { -0.05 })
    } else {
        body_c
    };
    b.rrect(
        cell,
        r,
        Fill::Solid(with_alpha(shade(body_c, -0.14), alpha)),
    );
    b.rrect(
        cell.shrink(b.px()),
        (r - b.px()).max(0.0),
        Fill::Solid(with_alpha(body_c, alpha)),
    );
    let th = title_h(cell, k);
    let title = Rect::from_min_max(cell.min, pos2(cell.right(), cell.top() + th));
    b.rrect(
        title.shrink(b.px()),
        (r - b.px()).max(0.0),
        Fill::V(
            with_alpha(shade(bar_c, 0.04 * t.depth), alpha),
            with_alpha(shade(bar_c, -0.04 * t.depth), alpha),
        ),
    );
    b.hline(
        title.left() + b.px(),
        title.right() - b.px(),
        title.top() + 1.5 * b.px(),
        b.px(),
        shade(bar_c, 0.12 * t.depth),
    );
    let lr = launcher_rect(cell, k);
    launcher_glyph(b, lr, s, look.launcher, look.tint.on_large);
    // The name, after the launcher.
    if !look.renaming {
        let name_box = Rect::from_min_max(
            pos2(lr.right() + 1.0 * k, title.top()),
            pos2(title.right() - 4.0 * k, title.bottom()),
        );
        if name_box.width() > 8.0 * k {
            let tx = Text::new(
                &clip.name,
                Role::Label,
                with_alpha(look.tint.on_large, alpha),
            )
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, name_box, &tx, s.tk());
        }
    }
    // The miniature under the title line.
    let mini = Rect::from_min_max(
        pos2(cell.left() + 4.0 * k, title.bottom() + 2.0 * k),
        pos2(cell.right() - 4.0 * k, cell.bottom() - 4.0 * k),
    );
    if mini.height() >= 4.0 * k && mini.width() > 8.0 * k {
        let ink = with_alpha(mix(bar_c, t.text, 0.35), 0.9 * alpha);
        match wave {
            Some(w) if clip.audio.is_some() => waveform(b, mini, s, w, ink),
            _ => notes(b, mini, s, clip, ink),
        }
    }
    // Where it plays: a bar along the foot, in the playing colour.
    if let Some(p) = look.progress {
        let foot = Rect::from_min_max(
            pos2(cell.left() + 2.0 * k, cell.bottom() - 3.0 * k),
            pos2(cell.right() - 2.0 * k, cell.bottom() - 1.0 * k),
        );
        b.rect(foot, Fill::Solid(with_alpha(shade(body_c, -0.25), alpha)));
        let x = foot.left() + foot.width() * p.clamp(0.0, 1.0);
        let c = if look.launcher == Launcher::Recording {
            t.danger
        } else {
            t.led
        };
        b.rect(
            Rect::from_min_max(foot.min, pos2(x, foot.bottom())),
            Fill::Solid(c),
        );
    }
    outline(b, cell, s, look.selected, look.cursor, look.hovered);
    if look.launcher == Launcher::Recording {
        b.rrect_stroke(cell.shrink(0.75 * k), r, 1.5 * k, t.danger);
    }
}

/// The selection's ring, the cursor's, the hover's.
fn outline(b: &mut Brush, cell: Rect, s: &Style, selected: bool, cursor: bool, hovered: bool) {
    let k = s.k;
    let r = s.r(2.5);
    if selected {
        b.rrect_stroke(cell.shrink(0.75 * k), r, 1.5 * k, s.t.selection);
    } else if hovered {
        b.rrect_stroke(cell.shrink(0.5 * k), r, k, with_alpha(s.t.focus, 0.6));
    }
    if cursor {
        b.rrect_stroke(cell.expand(1.0 * k), r + k, 1.25 * k, s.t.focus);
    }
}

/// What an empty slot shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Empty {
    /// Its stop square: launching it stops the track.
    Stop,
    /// No stop button: launching the scene leaves the track playing.
    NoStop,
    /// The track is armed: its record dot.
    Record,
    RecordQueued {
        on: bool,
    },
    /// Recording a new clip: the well washed red, the dot lit.
    Recording,
}

/// An empty slot.
#[allow(clippy::too_many_arguments)]
pub fn slot_empty(
    b: &mut Brush,
    cell: Rect,
    s: &Style,
    what: Empty,
    selected: bool,
    cursor: bool,
    hovered: bool,
    drop: bool,
) {
    let k = s.k;
    let cell = b.snap_rect(cell);
    let r = s.r(2.5);
    b.rrect(cell, r, Fill::Solid(s.t.well));
    b.hline(
        cell.left() + r,
        cell.right() - r,
        cell.top() + b.px(),
        b.px(),
        shade(s.t.well, -0.12),
    );
    let g = Rect::from_center_size(
        pos2(cell.left() + 10.0 * k, cell.center().y),
        vec2(8.0 * k, 8.0 * k),
    );
    match what {
        Empty::Stop => {
            let c = if hovered {
                s.t.text_dim
            } else {
                with_alpha(s.t.text_dim, 0.5)
            };
            b.rrect(
                Rect::from_center_size(g.center(), vec2(6.0 * k, 6.0 * k)),
                0.8 * k,
                Fill::Solid(c),
            );
        }
        Empty::NoStop => {}
        Empty::Record => {
            let c = if hovered {
                s.t.danger
            } else {
                with_alpha(s.t.danger, 0.6)
            };
            b.circle(g.center(), 3.6 * k, Fill::Solid(c));
        }
        Empty::RecordQueued { on } => {
            let c = if on {
                s.t.danger
            } else {
                with_alpha(s.t.danger, 0.3)
            };
            b.circle(g.center(), 3.6 * k, Fill::Solid(c));
        }
        Empty::Recording => {
            b.rrect(cell, r, Fill::Solid(with_alpha(s.t.danger, 0.28)));
            b.rrect_stroke(cell.shrink(0.75 * k), r, 1.5 * k, s.t.danger);
            b.circle(g.center(), 3.6 * k, Fill::Solid(s.t.danger));
            let tx = Text::key("session.slot.recording", Role::Label, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(
                b,
                Rect::from_min_max(
                    pos2(g.right() + 4.0 * k, cell.top()),
                    pos2(cell.right() - 4.0 * k, cell.bottom()),
                ),
                &tx,
                s.tk(),
            );
        }
    }
    if drop {
        b.rrect(cell, r, Fill::Solid(with_alpha(s.t.focus, 0.25)));
        b.rrect_stroke(cell.shrink(0.75 * k), r, 1.5 * k, s.t.focus);
    }
    outline(b, cell, s, selected, cursor, hovered && !drop);
}

/// A clip's notes over its loop, small (at most 160).
pub fn notes(b: &mut Brush, r: Rect, s: &Style, clip: &Clip, ink: Color32) {
    let k = s.k;
    if clip.notes.is_empty() {
        return;
    }
    let from = clip.offset.0;
    let span = clip.content_length.unwrap_or(clip.length).0.max(1) as f32;
    let shown = clip
        .notes
        .iter()
        .filter(|n| n.start.0 + n.length.0 > from && ((n.start.0 - from) as f32) < span)
        .take(160);
    let (lo, hi) = shown.clone().fold((u8::MAX, u8::MIN), |(lo, hi), n| {
        (lo.min(n.pitch), hi.max(n.pitch))
    });
    if lo > hi {
        return;
    }
    let rows = (hi - lo) as f32 + 1.0;
    let h = (r.height() / rows).clamp(1.2 * k, 3.0 * k);
    for n in shown {
        let x0 = r.left() + (((n.start.0 - from) as f32 / span).max(0.0)) * r.width();
        let x1 = r.left() + (((n.start.0 + n.length.0 - from) as f32 / span).min(1.0)) * r.width();
        let yc = r.bottom() - ((n.pitch - lo) as f32 + 0.5) / rows * r.height();
        b.rect(
            Rect::from_min_max(
                pos2(x0, yc - h * 0.5),
                pos2(x1.max(x0 + 1.2 * k), yc + h * 0.5),
            ),
            Fill::Solid(ink),
        );
    }
}

/// An audio clip's waveform: min/max columns over `r`.
pub fn waveform(b: &mut Brush, r: Rect, s: &Style, w: &[(f32, f32)], ink: Color32) {
    if w.is_empty() {
        return;
    }
    let k = s.k;
    let step = r.width() / w.len() as f32;
    let mid = r.center().y;
    let half = r.height() * 0.5;
    for (i, &(lo, hi)) in w.iter().enumerate() {
        let x = r.left() + (i as f32 + 0.5) * step;
        let y0 = mid - hi.clamp(-1.0, 1.0) * half;
        let y1 = mid - lo.clamp(-1.0, 1.0) * half;
        let half_w = (step * 0.35).max(0.5 * k);
        b.rect(
            Rect::from_min_max(
                pos2(x - half_w, y0.min(mid - 0.5 * k)),
                pos2(x + half_w, y1.max(mid + 0.5 * k)),
            ),
            Fill::Solid(ink),
        );
    }
}

/// A track's head over its column: the colour band, the name, and — while the session
/// holds the track — the *back to arrangement* key lit (Live's orange).
pub struct HeadLook<'a> {
    pub name: &'a str,
    pub tint: Tint,
    pub held: bool,
    pub selected: bool,
}

/// The rects of a head's three switches (arm, mute, solo) and its back-to-arrangement key.
pub fn head_keys(head: Rect, k: f32) -> [Rect; 4] {
    let y = head.bottom() - 17.0 * k;
    let w = 18.0 * k;
    let x = head.left() + 3.0 * k;
    let at =
        |i: usize| Rect::from_min_size(pos2(x + i as f32 * (w + 2.0 * k), y), vec2(w, 14.0 * k));
    let back = Rect::from_min_size(
        pos2(head.right() - 3.0 * k - 22.0 * k, y),
        vec2(22.0 * k, 14.0 * k),
    );
    [at(0), at(1), at(2), back]
}

pub fn head(b: &mut Brush, rect: Rect, s: &Style, look: &HeadLook<'_>) {
    let k = s.k;
    let rect = b.snap_rect(rect);
    let r = s.r(2.5);
    b.rrect(rect, r, Fill::Solid(s.t.header_cell));
    if look.selected {
        b.rrect_stroke(rect.shrink(0.75 * k), r, 1.25 * k, s.t.selection);
    }
    let band = Rect::from_min_size(rect.min, vec2(rect.width(), 4.0 * k));
    b.rrect(band, r, Fill::Solid(look.tint.small));
    let name = Rect::from_min_max(
        pos2(rect.left() + 5.0 * k, band.bottom() + 1.0 * k),
        pos2(rect.right() - 5.0 * k, rect.top() + 22.0 * k),
    );
    let tx = Text::new(look.name, Role::Body, s.t.text)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, name, &tx, s.tk());
}

/// A small key with a word or a glyph (the heads' arm / M / S, the header's switches).
pub fn key(b: &mut Brush, rect: Rect, s: &Style, word: &str, lit: Option<Color32>, hovered: bool) {
    let k = s.k;
    let rect = b.snap_rect(rect);
    let r = s.r(2.0);
    let face = match lit {
        Some(c) => c,
        None if hovered => shade(s.t.panel_raised, 0.06),
        None => s.t.panel_raised,
    };
    b.rrect(
        rect,
        r,
        Fill::V(
            shade(face, 0.05 * s.t.depth),
            shade(face, -0.05 * s.t.depth),
        ),
    );
    b.rrect_stroke(rect.shrink(b.px() * 0.5), r, b.px(), shade(face, -0.25));
    let ink = if lit.is_some() {
        crate::kit::color::on_fill(face)
    } else {
        s.t.text_dim
    };
    let tx = Text::new(word, Role::Micro, ink)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, rect.shrink2(vec2(1.0 * k, 0.0)), &tx, s.tk());
    let _ = k;
}

/// A glyph key.
pub fn glyph_key(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    g: Glyph,
    lit: Option<Color32>,
    hovered: bool,
) {
    let rect = b.snap_rect(rect);
    let r = s.r(2.0);
    let face = match lit {
        Some(c) => c,
        None if hovered => shade(s.t.panel_raised, 0.06),
        None => s.t.panel_raised,
    };
    b.rrect(
        rect,
        r,
        Fill::V(
            shade(face, 0.05 * s.t.depth),
            shade(face, -0.05 * s.t.depth),
        ),
    );
    b.rrect_stroke(rect.shrink(b.px() * 0.5), r, b.px(), shade(face, -0.25));
    let ink = if lit.is_some() {
        crate::kit::color::on_fill(face)
    } else {
        s.t.text_dim
    };
    let side = rect.height().min(rect.width()) * 0.62;
    glyph::paint(
        b,
        Rect::from_center_size(rect.center(), vec2(side, side)),
        g,
        ink,
    );
}

/// What a scene's launcher shows.
pub struct SceneLook<'a> {
    pub number: usize,
    pub name: &'a str,
    /// Its tempo and metre tag ("120 · 4/4"), when it sets them.
    pub tag: Option<&'a str>,
    pub colour: Option<Color32>,
    pub launcher: Launcher,
    pub selected: bool,
    pub hovered: bool,
    pub renaming: bool,
}

pub fn scene(b: &mut Brush, rect: Rect, s: &Style, look: &SceneLook<'_>) {
    let k = s.k;
    let rect = b.snap_rect(rect);
    let r = s.r(2.5);
    let face = look
        .colour
        .map_or(s.t.panel_raised, |c| mix(s.t.panel_raised, c, 0.45));
    let face = if look.hovered {
        shade(face, 0.05)
    } else {
        face
    };
    b.rrect(
        rect,
        r,
        Fill::V(
            shade(face, 0.04 * s.t.depth),
            shade(face, -0.04 * s.t.depth),
        ),
    );
    b.rrect_stroke(rect.shrink(b.px() * 0.5), r, b.px(), shade(face, -0.25));
    if let Some(c) = look.colour {
        b.rect(
            Rect::from_min_size(
                rect.min + vec2(b.px(), b.px()),
                vec2(3.0 * k, rect.height() - 2.0 * b.px()),
            ),
            Fill::Solid(c),
        );
    }
    let lr = Rect::from_min_size(
        pos2(rect.left() + 3.0 * k, rect.top()),
        vec2(16.0 * k, rect.height()),
    );
    launcher_glyph(b, lr, s, look.launcher, s.t.text);
    let mut right = rect.right() - 5.0 * k;
    if let Some(tag) = look.tag {
        let t = Text::new(tag, Role::Micro, s.t.text_dim).align(Align2::RIGHT_CENTER);
        let w = text::measure(b.ctx(), &t, s.tk()).x.min(rect.width() * 0.5);
        let _ = text::paint(
            b,
            Rect::from_min_max(pos2(right - w, rect.top()), pos2(right, rect.bottom())),
            &t,
            s.tk(),
        );
        right -= w + 4.0 * k;
    }
    if !look.renaming {
        let label = if look.name.is_empty() {
            look.number.to_string()
        } else {
            look.name.to_string()
        };
        let tx = Text::new(&label, Role::Label, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(
            b,
            Rect::from_min_max(
                pos2(lr.right() + 2.0 * k, rect.top()),
                pos2(right.max(lr.right() + 4.0 * k), rect.bottom()),
            ),
            &tx,
            s.tk(),
        );
    }
    if look.selected {
        b.rrect_stroke(rect.shrink(0.75 * k), r, 1.5 * k, s.t.selection);
    }
}

/// A track's stop button: lit while its track plays a clip, blinking while it stops.
pub fn stop_button(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    playing: bool,
    blink: Option<bool>,
    hovered: bool,
) {
    let k = s.k;
    let rect = b.snap_rect(rect);
    let r = s.r(2.0);
    let face = if hovered {
        shade(s.t.panel_raised, 0.06)
    } else {
        s.t.panel_raised
    };
    b.rrect(
        rect,
        r,
        Fill::V(
            shade(face, 0.05 * s.t.depth),
            shade(face, -0.05 * s.t.depth),
        ),
    );
    b.rrect_stroke(rect.shrink(b.px() * 0.5), r, b.px(), shade(face, -0.25));
    let ink = match blink {
        Some(true) => s.t.danger,
        Some(false) => with_alpha(s.t.danger, 0.3),
        None if playing => s.t.danger,
        None => s.t.text_dim,
    };
    let sq = Rect::from_center_size(rect.center(), vec2(8.0 * k, 8.0 * k));
    b.rrect(sq, 0.8 * k, Fill::Solid(ink));
}

/// The session ring a grid controller shows: a box around its slots.
pub fn ring(b: &mut Brush, rect: Rect, s: &Style) {
    let k = s.k;
    b.rrect_stroke(rect.expand(2.0 * k), s.r(3.0), 2.0 * k, s.t.highlight);
}
