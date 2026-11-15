//! The middle of an instrument's mixer: its strips under their family heads, folding, and
//! scrolling sideways with a fade at the edge it continues past and a pill in the heads'
//! row (*2 more ›*) that sits over no strip.

use super::strip::{self, meters};
use super::ui::{self as iu, words};
use super::{family_word, Edit, Edits, Strip, STRIP_W};
use crate::kit::color::with_alpha;
use crate::kit::{with_brush, Fill, State, Style};
use crate::views::mixer::look::{self, Pal, W};
use cantus_core::OutputFamily;
use egui::{pos2, vec2, Align2, Rect, Sense, Ui};

/// A run of strips of one family, in the instrument's order.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub family: OutputFamily,
    pub first: usize,
    pub len: usize,
}

/// The families' runs of `strips` (consecutive strips of one family make one run).
pub fn runs(strips: &[Strip]) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for (i, s) in strips.iter().enumerate() {
        match out.last_mut() {
            Some(r) if r.family == s.family => r.len += 1,
            _ => out.push(Run {
                family: s.family,
                first: i,
                len: 1,
            }),
        }
    }
    out
}

/// How wide the middle's content is: each open run's strips, each folded run one strip.
pub fn content_width(strips: &[Strip], folded: &[OutputFamily], s: &Style) -> f32 {
    runs(strips)
        .iter()
        .map(|r| {
            let n = if folded.contains(&r.family) { 1 } else { r.len };
            n as f32 * s.s(STRIP_W)
        })
        .sum()
}

/// What the middle reports besides the strips' edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    /// A family's head was clicked: fold it or unfold it.
    pub toggle: Option<OutputFamily>,
    /// Strips hidden past the right edge and the left.
    pub more_right: usize,
    pub more_left: usize,
    /// Where each strip was drawn (`None`: folded or scrolled away).
    pub at: Vec<Option<Rect>>,
}

/// Draw the middle: `heads` (the families' row, as wide as `mid`) over `mid` (the strips'
/// area), scrolled by `scroll` (clamped here). Returns the strips' edits and what else.
#[allow(clippy::too_many_arguments)]
pub fn middle(
    ui: &mut Ui,
    s: &Style,
    heads: Rect,
    mid: Rect,
    strips: &[Strip],
    folded: &[OutputFamily],
    scroll: &mut f32,
    salt: egui::Id,
) -> (Edits, Shown) {
    let p = Pal::of(s);
    let pitch = s.s(STRIP_W);
    let content = content_width(strips, folded, s);
    // Scrolling, the middle shows whole strips: never a stray part of one at its edge.
    let mid = if content > mid.width() + 0.5 {
        let whole = (mid.width() / pitch).floor().max(1.0) * pitch;
        Rect::from_min_max(mid.min, pos2(mid.left() + whole, mid.bottom()))
    } else {
        mid
    };
    let max = (content - mid.width()).max(0.0);
    // The wheel over the strips or the heads moves them sideways.
    let over = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|q| mid.union(heads).contains(q));
    if over && max > 0.0 {
        let d = ui.input(|i| i.smooth_scroll_delta);
        let by = if d.x.abs() > d.y.abs() { d.x } else { d.y };
        *scroll -= by;
    }
    *scroll = scroll.clamp(0.0, max);
    let mut shown = Shown {
        at: vec![None; strips.len()],
        ..Shown::default()
    };
    let mut edits = Vec::new();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(mid.union(heads))
            .id_salt(salt),
    );
    child.set_clip_rect(mid.union(heads).intersect(ui.clip_rect()));
    let mut x = mid.left() - *scroll;
    for run in runs(strips) {
        let fold = folded.contains(&run.family);
        let n = if fold { 1 } else { run.len };
        let w = n as f32 * pitch;
        let span = Rect::from_min_max(pos2(x, heads.top()), pos2(x + w, heads.bottom()));
        if span.right() > mid.left()
            && span.left() < mid.right()
            && head(&child, s, &p, span, run.family, fold, salt)
        {
            shown.toggle = Some(run.family);
        }
        if fold {
            let r = Rect::from_min_max(pos2(x, mid.top()), pos2(x + pitch, mid.bottom()));
            let members = &strips[run.first..run.first + run.len];
            if r.right() > mid.left()
                && r.left() < mid.right()
                && folded_strip(&child, s, &p, r, run.family, members, salt)
            {
                shown.toggle = Some(run.family);
            }
        } else {
            for (i, st) in strips.iter().enumerate().skip(run.first).take(run.len) {
                let r = Rect::from_min_max(pos2(x, mid.top()), pos2(x + pitch, mid.bottom()));
                if r.right() > mid.left() && r.left() < mid.right() {
                    let id = salt.with(("strip", i));
                    for e in strip::show(&child, s, r.shrink2(vec2(1.0 * s.k, 0.0)), st, id) {
                        edits.push((i, e));
                    }
                    shown.at[i] = Some(r);
                } else if r.left() >= mid.right() {
                    shown.more_right += 1;
                } else {
                    shown.more_left += 1;
                }
                x += pitch;
            }
            continue;
        }
        x += w;
    }
    // The fade where the strips go on past an edge, and the pills in the heads' row.
    with_brush(child.painter(), |b| {
        let fw = s.s(28.0);
        if shown.more_right > 0 {
            let r = Rect::from_min_max(pos2(mid.right() - fw, mid.top()), mid.max);
            b.hrect(
                r,
                &[
                    (0.0, with_alpha(p.base, 0.0)),
                    (1.0, with_alpha(p.base, 0.92)),
                ],
            );
        }
        if shown.more_left > 0 {
            let r = Rect::from_min_max(mid.min, pos2(mid.left() + fw, mid.bottom()));
            b.hrect(
                r,
                &[
                    (0.0, with_alpha(p.base, 0.92)),
                    (1.0, with_alpha(p.base, 0.0)),
                ],
            );
        }
    });
    if shown.more_right > 0 && pill(&child, s, &p, heads, shown.more_right, true, salt) {
        *scroll = (*scroll + mid.width() - pitch).clamp(0.0, max);
    }
    if shown.more_left > 0 && pill(&child, s, &p, heads, shown.more_left, false, salt) {
        *scroll = (*scroll - mid.width() + pitch).clamp(0.0, max);
    }
    (edits, shown)
}

/// A family's head over its strips: a chevron (open ▾, folded ›), its word, a hairline to
/// its end. Returns whether it was clicked.
fn head(
    ui: &Ui,
    s: &Style,
    p: &Pal,
    span: Rect,
    f: OutputFamily,
    fold: bool,
    salt: egui::Id,
) -> bool {
    let word = family_word(f);
    let hit = Rect::from_min_max(
        span.min,
        pos2(
            span.right()
                .min(span.left() + s.s(24.0) + iu::width(ui, s, &word, 11.0, W::Medium)),
            span.bottom(),
        ),
    );
    let resp = ui.interact(hit, salt.with(("head", f as u8)), Sense::click());
    let resp = iu::hint(resp, || {
        crate::i18n::tf("inst_mixer.family_tip", &[("name", &word)])
    });
    with_brush(ui.painter(), |b| {
        let ink = if resp.hovered() { p.ink1 } else { p.ink2 };
        let cx = span.left() + s.s(9.0);
        let cy = span.center().y;
        if fold {
            let (h, d) = (s.s(3.5), s.s(2.0));
            b.polyline(
                &[pos2(cx - d, cy - h), pos2(cx + d, cy), pos2(cx - d, cy + h)],
                s.s(1.3),
                ink,
            );
        } else {
            crate::kit::well::chevron(b, pos2(cx, cy), s.s(7.0), s.s(4.0), false, s.s(1.3), ink);
        }
        let tr = Rect::from_min_max(
            pos2(span.left() + s.s(18.0), span.top()),
            pos2(span.right() - s.s(4.0), span.bottom()),
        );
        let tw = words(b, s, &word, tr, Align2::LEFT_CENTER, 11.0, W::Medium, ink);
        let lx = tr.left() + tw + s.s(8.0);
        if lx < span.right() - s.s(10.0) {
            b.hline(
                lx,
                span.right() - s.s(6.0),
                b.snap(cy) + b.px() * 0.5,
                b.px(),
                p.line,
            );
        }
    });
    resp.clicked()
}

/// A folded family: its word, its members named, their meters side by side, *Unfold*.
fn folded_strip(
    ui: &Ui,
    s: &Style,
    p: &Pal,
    r: Rect,
    f: OutputFamily,
    members: &[Strip],
    salt: egui::Id,
) -> bool {
    let r = r.shrink2(vec2(s.s(1.0), 0.0));
    let unfold = Rect::from_min_max(
        pos2(r.left() + s.s(6.0), r.bottom() - s.s(28.0)),
        pos2(r.right() - s.s(6.0), r.bottom() - s.s(6.0)),
    );
    let resp = iu::button(
        ui,
        s,
        unfold,
        salt.with(("unfold", f as u8)),
        &crate::i18n::t("inst_mixer.unfold"),
        iu::Lead::None,
        false,
        true,
    );
    let word = family_word(f);
    let resp = iu::hint(resp, || {
        crate::i18n::tf("inst_mixer.unfold_tip", &[("name", &word)])
    });
    with_brush(ui.painter(), |b| {
        look::body(b, r, p, None);
        let nr = Rect::from_min_size(
            pos2(r.left() + s.s(3.0), r.top() + s.s(6.0)),
            vec2(r.width() - s.s(6.0), s.s(15.0)),
        );
        words(
            b,
            s,
            &word,
            nr,
            Align2::CENTER_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        let mut y = nr.bottom() + s.s(4.0);
        for m in members.iter().take(4) {
            let lr = Rect::from_min_size(
                pos2(r.left() + s.s(8.0), y),
                vec2(r.width() - s.s(12.0), s.s(14.0)),
            );
            b.circle(
                pos2(lr.left(), lr.center().y),
                s.s(1.5),
                Fill::Solid(p.ink3),
            );
            let tr = Rect::from_min_max(pos2(lr.left() + s.s(5.0), lr.top()), lr.max);
            words(
                b,
                s,
                &m.name,
                tr,
                Align2::LEFT_CENTER,
                10.0,
                W::Regular,
                p.ink2,
            );
            y += s.s(15.0);
        }
        let n = crate::i18n::tf("inst_mixer.folded", &[("n", &members.len())]);
        let cr = Rect::from_min_size(pos2(r.left(), y + s.s(4.0)), vec2(r.width(), s.s(14.0)));
        words(
            b,
            s,
            &n,
            cr,
            Align2::CENTER_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
        let levels: Vec<Option<[f32; 2]>> = members.iter().map(|m| m.meter).collect();
        let mr = Rect::from_min_max(
            pos2(r.left() + s.s(6.0), cr.bottom() + s.s(14.0)),
            pos2(r.right() - s.s(6.0), unfold.top() - s.s(30.0)),
        );
        if mr.height() > s.s(20.0) {
            meters(b, s, p, mr, &levels);
        }
    });
    resp.clicked()
}

/// The *n more ›* pill at the heads' right (or *‹ n* at their left), over no strip.
fn pill(ui: &Ui, s: &Style, p: &Pal, heads: Rect, n: usize, right: bool, salt: egui::Id) -> bool {
    let text = if right {
        format!(
            "{} \u{203a}",
            crate::i18n::tf("inst_mixer.more", &[("n", &n)])
        )
    } else {
        format!("\u{2039} {n}")
    };
    let w = iu::width(ui, s, &text, 11.0, W::Medium) + s.s(16.0);
    let h = (heads.height() - s.s(2.0)).max(s.s(16.0));
    let r = if right {
        Rect::from_min_size(
            pos2(heads.right() - w - s.s(2.0), heads.center().y - h * 0.5),
            vec2(w, h),
        )
    } else {
        Rect::from_min_size(
            pos2(heads.left() + s.s(2.0), heads.center().y - h * 0.5),
            vec2(w, h),
        )
    };
    let resp = ui.interact(r, salt.with(("pill", right)), Sense::click());
    let resp = iu::hint(resp, || {
        crate::i18n::tf("inst_mixer.more_tip", &[("n", &n)])
    });
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        // Its own ground first, so the hairline of the head under it stops at its edge.
        b.rect(r.expand(s.s(3.0)), Fill::Solid(p.base));
        iu::paint_button(b, s, r, st, &text, iu::Lead::None, true);
    });
    resp.clicked()
}

/// The edits of one strip of `edits`, in order.
pub fn of(edits: &Edits, i: usize) -> impl Iterator<Item = Edit> + '_ {
    edits.iter().filter(move |(j, _)| *j == i).map(|(_, e)| *e)
}
