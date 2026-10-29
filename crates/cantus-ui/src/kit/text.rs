//! Every word in a box (ADR 0024 §3).
//!
//! A text is painted into a rect with a [`Policy`], **measured before it is painted**,
//! and never painted outside the rect: what does not fit is abbreviated, then elided —
//! at the end, or in the middle ("GHO…ICK", FL's way with track names, so both ends of a
//! name stay recognisable) — or turned on its side when a column is narrower than the
//! word (`Rotate90`, the mixer's strip names). This is what removes the collisions at
//! their root (`cantus-audit.md` §4.1: labels painted unmeasured).
//!
//! **Abbreviations.** [`Policy::Abbreviate`] needs the text's i18n *key*: it looks up
//! `<key>.short` in any locale file (`fx.eq.freq.short = FREQ`) and uses the full label
//! when there is no short form. Generic short forms (ATK, DEC, SUS, REL, RES, FREQ,
//! THRESH…) live in `locales/areas/kit.*.txt`; a device's own live in its locale file.
//!
//! Results are cached per (text, role, box width, policy, scale, pixels per point), so a
//! label that did not change costs a hash and a lookup.

use super::brush::Brush;
use super::cache::{self, KitCache};
pub use crate::design::role::Role;
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, Context, Galley, Pos2, Rect, Vec2};
use std::sync::Arc;

/// Where an overlong text loses its letters.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Elide {
    /// "Reverb ret…" — for prose-like labels.
    End,
    /// "GHO…ICK" — the default for names.
    #[default]
    Middle,
}

/// What to do with a text wider than its box.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Policy {
    Elide(Elide),
    /// Read bottom to top; the box's *height* is the text's width.
    Rotate90(Elide),
    /// Try the `<key>.short` form, then elide at the end.
    Abbreviate,
}

impl Default for Policy {
    fn default() -> Self {
        Policy::Elide(Elide::Middle)
    }
}

/// Where the text comes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Src<'a> {
    /// A string already in the interface's language.
    Str(&'a str),
    /// An i18n key, looked up (and cached) in the current locale.
    Key(&'a str),
}

/// A text to paint in a box.
#[derive(Copy, Clone, Debug)]
pub struct Text<'a> {
    pub src: Src<'a>,
    pub role: Role,
    /// Set in capitals (device labels, sections, micro captions).
    pub caps: bool,
    pub align: Align2,
    pub policy: Policy,
    pub color: Color32,
}

impl<'a> Text<'a> {
    pub fn new(s: &'a str, role: Role, color: Color32) -> Self {
        Text {
            src: Src::Str(s),
            role,
            caps: role.caps(),
            align: Align2::CENTER_CENTER,
            policy: Policy::default(),
            color,
        }
    }

    pub fn key(key: &'a str, role: Role, color: Color32) -> Self {
        Text {
            src: Src::Key(key),
            ..Text::new("", role, color)
        }
    }

    pub fn caps(mut self, on: bool) -> Self {
        self.caps = on;
        self
    }

    pub fn align(mut self, a: Align2) -> Self {
        self.align = a;
        self
    }

    pub fn policy(mut self, p: Policy) -> Self {
        self.policy = p;
        self
    }
}

/// What was painted.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Painted {
    /// The area the text's letters occupy (inside the box).
    pub rect: Rect,
    /// Whether letters were dropped or replaced to make it fit.
    pub shortened: bool,
}

/// A unit's lettering laid over a role (round 32, ADR 0074): another weight or width of
/// the kit's faces, extra space between letters, values in the plain mono face. The
/// default is the role's own face.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Face {
    /// The weight and whether condensed; `None` keeps the role's face.
    pub letters: Option<(crate::kit::skin::Weight, bool)>,
    /// Extra space between letters, in thousandths of an em, over the role's own.
    pub tracking_milli: i16,
    /// Values (and their units) in the plain mono face.
    pub mono: bool,
}

impl Face {
    /// The role's own face.
    pub const ROLE: Face = Face {
        letters: None,
        tracking_milli: 0,
        mono: false,
    };
}

fn job(s: &str, role: Role, k: f32, face: Face) -> LayoutJob {
    let mut j = LayoutJob::default();
    let format = |r: Role| {
        let font_id = match face.letters {
            _ if face.mono && matches!(role, Role::Value | Role::Hero) => r.mono_plain(k),
            Some((w, c)) if r == role => r.lettered(w, c, k),
            _ => r.font(k),
        };
        let extra = if r == role {
            face.tracking_milli as f32 * 0.001 * r.size() * k
        } else {
            0.0
        };
        TextFormat {
            font_id,
            extra_letter_spacing: r.tracking() * k + extra,
            color: Color32::PLACEHOLDER,
            valign: Align::BOTTOM,
            ..Default::default()
        }
    };
    // A value's unit ("2.40 kHz", "-12.5 dB") is set after the number in the smaller
    // condensed face, as FL sets an LCD's fraction smaller: the digits keep their mono
    // columns and the whole still fits a 64-pt cell.
    match (role, s.split_once(' ')) {
        (Role::Value | Role::Hero, Some((num, unit))) if !unit.is_empty() => {
            j.append(num, 0.0, format(role));
            j.append(unit, 2.0 * k, format(Role::Label));
        }
        _ => j.append(s, 0.0, format(role)),
    }
    j.wrap.max_rows = 1;
    j
}

fn resolve(src: Src<'_>, caps: bool) -> String {
    let s = match src {
        Src::Str(s) => s.to_string(),
        Src::Key(k) => crate::i18n::t(k),
    };
    if caps {
        s.to_uppercase()
    } else {
        s
    }
}

/// What a text is set in: its role, the scale and the lettering.
type Set = (Role, f32, Face);

fn layout(ctx: &Context, s: &str, (role, k, face): Set) -> Arc<Galley> {
    // A name in a script no bundled font has asks for the system's faces (round 26).
    crate::font_fallback::note_text(ctx, s);
    ctx.fonts_mut(|f| f.layout_job(job(s, role, k, face)))
}

/// The longest candidate `make(n)` (n = 0…max) whose width fits `w`, by bisection.
fn longest_fitting(
    ctx: &Context,
    set: Set,
    w: f32,
    max: usize,
    make: impl Fn(usize) -> String,
) -> Option<Arc<Galley>> {
    let fits = |n: usize| {
        let g = layout(ctx, &make(n), set);
        (g.size().x <= w + 0.01).then_some(g)
    };
    let mut best = fits(0)?;
    let (mut lo, mut hi) = (0usize, max);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        match fits(mid) {
            Some(g) => {
                best = g;
                lo = mid;
            }
            None => hi = mid - 1,
        }
    }
    Some(best)
}

fn elide(ctx: &Context, s: &str, set: Set, w: f32, how: Elide) -> Option<Arc<Galley>> {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    match how {
        Elide::End => longest_fitting(ctx, set, w, n.saturating_sub(1), |m| {
            let mut t: String = chars[..m].iter().collect::<String>().trim_end().to_string();
            t.push('…');
            t
        }),
        Elide::Middle => longest_fitting(ctx, set, w, n.saturating_sub(1), |m| {
            let head = m.div_ceil(2);
            let tail = m / 2;
            let mut t: String = chars[..head]
                .iter()
                .collect::<String>()
                .trim_end()
                .to_string();
            t.push('…');
            t.extend(chars[n - tail..].iter());
            t
        }),
    }
}

/// The galley for `text` fitted to `width` points, and whether it was shortened.
/// `None` when not even an ellipsis fits.
pub fn fit(
    ctx: &Context,
    c: &mut KitCache,
    text: &Text<'_>,
    k: f32,
    width: f32,
) -> Option<(Arc<Galley>, bool)> {
    fit_face(ctx, c, text, k, width, Face::ROLE)
}

/// [`fit`] in a unit's lettering.
pub fn fit_face(
    ctx: &Context,
    c: &mut KitCache,
    text: &Text<'_>,
    k: f32,
    width: f32,
    face: Face,
) -> Option<(Arc<Galley>, bool)> {
    let ppp = ctx.pixels_per_point();
    let wq = (width * ppp).floor() as i32;
    let locale = matches!(text.src, Src::Key(_)).then(crate::i18n::locale);
    let base = cache::key((
        text.src,
        locale,
        text.role,
        text.caps,
        text.policy,
        k.to_bits(),
        ppp.to_bits(),
    ));
    // The role's own face keeps the key it always had; a lettering is part of the key.
    let key = if face == Face::ROLE {
        cache::key((base, wq))
    } else {
        cache::key((base, wq, face))
    };
    c.fitted(key, || {
        let full = resolve(text.src, text.caps);
        fit_uncached(ctx, text, &full, (text.role, k, face), width)
    })
}

fn fit_uncached(
    ctx: &Context,
    text: &Text<'_>,
    full: &str,
    set: Set,
    width: f32,
) -> Option<(Arc<Galley>, bool)> {
    let g = layout(ctx, full, set);
    if g.size().x <= width + 0.01 {
        return Some((g, false));
    }
    let how = match text.policy {
        Policy::Elide(e) | Policy::Rotate90(e) => e,
        Policy::Abbreviate => {
            if let Src::Key(key) = text.src {
                let short_key = format!("{key}.short");
                let short = crate::i18n::t(&short_key);
                if short != short_key {
                    let short = if text.caps {
                        short.to_uppercase()
                    } else {
                        short
                    };
                    let gs = layout(ctx, &short, set);
                    if gs.size().x <= width + 0.01 {
                        return Some((gs, true));
                    }
                    return elide(ctx, &short, set, width, Elide::End).map(|g| (g, true));
                }
            }
            Elide::End
        }
    };
    elide(ctx, full, set, width, how).map(|g| (g, true))
}

/// The natural size of a text, unshortened.
pub fn measure(ctx: &Context, text: &Text<'_>, k: f32) -> Vec2 {
    measure_face(ctx, text, k, Face::ROLE)
}

/// [`measure`] in a unit's lettering.
pub fn measure_face(ctx: &Context, text: &Text<'_>, k: f32, face: Face) -> Vec2 {
    cache::with(ctx, |c| {
        let key = cache::key((
            text.src,
            matches!(text.src, Src::Key(_)).then(crate::i18n::locale),
            text.role,
            text.caps,
            k.to_bits(),
            ctx.pixels_per_point().to_bits(),
            "measure",
        ));
        let key = if face == Face::ROLE {
            key
        } else {
            cache::key((key, face))
        };
        c.galley(ctx, key, || {
            job(&resolve(text.src, text.caps), text.role, k, face)
        })
        .size()
    })
}

/// Paint `text` inside `rect` at scale `k`: fitted by its policy, aligned, never outside.
/// Returns `None` when nothing could be painted (the box is too small for an ellipsis).
pub fn paint(brush: &mut Brush, rect: Rect, text: &Text<'_>, k: f32) -> Option<Painted> {
    paint_face(brush, rect, text, k, Face::ROLE)
}

/// [`paint`] in a unit's lettering ([`Face`]).
pub fn paint_face(
    brush: &mut Brush,
    rect: Rect,
    text: &Text<'_>,
    k: f32,
    face: Face,
) -> Option<Painted> {
    let rotated = matches!(text.policy, Policy::Rotate90(_));
    let (w, h) = if rotated {
        (rect.height(), rect.width())
    } else {
        (rect.width(), rect.height())
    };
    let (g, shortened) = brush.with_cache(|ctx, c| fit_face(ctx, c, text, k, w, face))?;
    let size = g.size();
    // A box too short for the line is a layout bug; paint nothing rather than overlap.
    if size.y > h * 1.34 + 1.0 {
        return None;
    }
    let snap = |v: f32| brush.snap(v);
    if rotated {
        // Rotated by −90°: the galley's width runs up the box, its height across.
        let x = match text.align.x() {
            Align::Min => rect.left(),
            Align::Center => rect.center().x - size.y * 0.5,
            Align::Max => rect.right() - size.y,
        };
        let y = match text.align.y() {
            Align::Min => rect.top() + size.x,
            Align::Center => rect.center().y + size.x * 0.5,
            Align::Max => rect.bottom(),
        };
        let pos = Pos2::new(snap(x), snap(y));
        let painted =
            Rect::from_min_size(Pos2::new(pos.x, pos.y - size.x), Vec2::new(size.y, size.x));
        brush.text(pos, g, text.color, -std::f32::consts::FRAC_PI_2);
        return Some(Painted {
            rect: painted,
            shortened,
        });
    }
    let r = text.align.align_size_within_rect(size, rect);
    let pos = Pos2::new(snap(r.min.x), snap(r.min.y));
    brush.text(pos, g, text.color, 0.0);
    Some(Painted {
        rect: Rect::from_min_size(pos, size),
        shortened,
    })
}
