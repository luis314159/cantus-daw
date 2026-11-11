//! Painting the arrangement: FL's playlist drawn with the kit (ADR 0024 §2, §4).
//!
//! Everything here reads the application and writes into a brush — nothing senses input
//! and nothing edits — so the view can paint a whole region as one mesh and handle its
//! input afterwards, with the project borrowed for exactly as long as the drawing takes.
//! Nothing is copied: the old view cloned every track, notes and all, every frame, which
//! on the stress project was 18 MB a frame (`rendering-performance.md` §2).

use super::Scratch;
use crate::app::CantusApp;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::clips_chrome::Tint;
use cantus_core::{ClipId, TrackId};
use egui::{pos2, Align2, Rect};

/// How much of the selection tint a selected clip's body takes (r39:lapis2).
const CLIP_TINT: f32 = 1.5;

/// What a clip paints with besides its colour.
const CLIP_TOKENS: &[Tok] = &[Tok::RowBlack, Tok::RowWhite, Tok::Selection, Tok::TrackVeil];

/// Whether a clip is in the selection, without copying the selection.
pub(super) fn is_selected(app: &CantusApp, r: (TrackId, ClipId)) -> bool {
    if app.selected_clips.is_empty() {
        app.selected_track == Some(r.0) && app.selected_clip == Some(r.1)
    } else {
        app.selected_clips.contains(&r)
    }
}

/// r39:lo-arrange — how far a clip another track's solo silences (`dim`) steps toward the
/// lane, and how strong its notes stay: silenced at a glance, its music still legible.
const DIM: f32 = 0.62;
const DIM_NOTES: f32 = 0.32;

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
    /// Under the playhead while the song plays (r39:el-views, des-views: a step brighter
    /// with a lit top).
    pub playing: bool,
    /// The zoom is near or close (20 px a beat and more): a short lane's clip 80 px wide
    /// names itself there (r39:el-views).
    pub near: bool,
    /// r39:lo-arrange — des-arrange's `lit`: a clip of the selected track (the selection's
    /// tint, no edge); a selected clip is lit and edged.
    pub lit: bool,
    /// r39:lo-arrange — des-arrange's `dim`: another track's solo silences it (its tint at
    /// 60 %, its notes at half).
    pub dim: bool,
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
    let bar_c = if look.selected || look.lit {
        shade(bar_c, if t.dark { 0.06 } else { -0.04 })
    } else {
        bar_c
    };
    // r39:lo-arrange — lit: the body a step toward its colour; dim: toward the lane.
    let body_c = if (look.selected || look.lit) && !look.muted {
        mix(body_c, look.tint.large, 0.10)
    } else {
        body_c
    };
    // r39:lapis2 — a selected clip's body through the theme's selection tint (none in
    // Slate).
    let body_c = if look.selected && !look.muted {
        crate::kit::color::selected_fill(body_c, t.selection_tint, CLIP_TINT)
    } else {
        body_c
    };
    let (bar_c, body_c) = if look.dim && !look.muted {
        (mix(bar_c, ground, DIM), mix(body_c, ground, DIM))
    } else {
        (bar_c, body_c)
    };
    let r = s.r(2.0);
    let body = b.snap_rect(body);
    // r39:el-views — playing: a step brighter (des-views: the rest tint + 6 %).
    let body_c = if look.playing && !look.muted {
        mix(body_c, look.tint.large, 0.06)
    } else {
        body_c
    };
    b.rrect(body, r, Fill::Solid(shade(body_c, -0.12)));
    b.rrect(
        body.shrink(b.px()),
        (r - b.px()).max(0.0),
        Fill::Solid(body_c),
    );
    if look.muted {
        muted_hatch(b, body.shrink(b.px()), visible, s);
    }
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
    if look.playing && !look.muted {
        // Its lit top: a line of the colour lifted toward white.
        let room = crate::views::room::Room::of(t);
        b.hline(
            body.left() + b.px(),
            body.right() - b.px(),
            body.top() + 1.5 * b.px(),
            b.px(),
            mix(look.tint.large, room.white, 0.45),
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
                                           // r39:el-views — des-views: a lane without the strip names a clip over its content
                                           // only when it is selected, or 80 px wide at the near and close zooms; elsewhere the
                                           // content has the room (the name is in the hover hint).
    let shown_w = body.right().min(visible.right()) - body.left().max(visible.left());
    let fit = fit.filter(|_| title_h > 0.0 || look.selected || (look.near && shown_w >= 80.0 * k));
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
        } else if look.dim {
            with_alpha(ink, 0.45)
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
pub(super) fn dashed_rect(
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

/// The body's ground under a clip's content, as [`clip_chrome`] paints it (r39:el-views).
pub(super) fn body_ground(look: &ClipLook, s: &Style) -> egui::Color32 {
    let t = &s.t;
    let ground = if t.dark { t.row_black } else { t.row_white };
    if look.muted {
        return mix(mix(look.tint.large, ground, 0.65), ground, 0.5);
    }
    let c = mix(look.tint.large, ground, if t.dark { 0.55 } else { 0.25 });
    let c = if look.selected || look.lit {
        mix(c, look.tint.large, 0.10)
    } else {
        c
    };
    if look.dim {
        mix(c, ground, DIM)
    } else {
        c
    }
}

/// The colour a clip's notes are drawn in on its body.
pub(super) fn note_ink(look: &ClipLook, s: &Style) -> egui::Color32 {
    let ink = note_ink_full(look, s);
    if look.dim && !look.muted {
        with_alpha(ink, DIM_NOTES)
    } else {
        ink
    }
}

/// [`note_ink`] before a solo dims it.
fn note_ink_full(look: &ClipLook, s: &Style) -> egui::Color32 {
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

/// The height of a clip's name strip: none on a body too short to hold one.
///
/// r39:el-views — des-views: a 13-px strip in lanes of 44 and taller. This branch's
/// default lane is 40 (the merged tree's 44): the strip starts at a 40-px lane, so the
/// default keeps it either way.
pub(super) fn title_height(body: Rect, k: f32) -> f32 {
    if body.height() >= 36.0 * k {
        13.0 * k
    } else {
        0.0
    }
}

/// A muted clip's hatch (des-views): 1-px diagonals every 6 px in ink 4 at 35 %, drawn
/// only where the clip is on screen, anchored to the clip so they do not crawl.
fn muted_hatch(b: &mut Brush, body: Rect, visible: Rect, s: &Style) {
    let k = s.k;
    let room = crate::views::room::Room::of(&s.t);
    let c = with_alpha(room.ink4, 0.35);
    let step = 6.0 * k;
    let h = body.height();
    let (l, r) = (
        body.left().max(visible.left()),
        body.right().min(visible.right()),
    );
    if r <= l || h <= 0.0 {
        return;
    }
    // A line from (x, bottom) to (x + h, top), clipped to [l, r].
    let first = body.left() + ((l - h - body.left()) / step).floor().max(0.0) * step;
    let mut x = first;
    while x < r {
        let (x0, x1) = (x, x + h);
        let (a, z) = (x0.max(l), x1.min(r));
        if z > a {
            let y = |xx: f32| body.bottom() - (xx - x0);
            b.segment(pos2(a, y(a)), pos2(z, y(z)), b.px(), c, false);
        }
        x += step;
    }
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
