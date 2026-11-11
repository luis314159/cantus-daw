//! r39:el-arrange — painting a track header, a folder's row and the add-track row (the
//! measures are `arrangement_head.rs`'s and the sheets').

use super::{body, rects, stripe, words, Key, Rects, Width, KEYS, TOKENS};
use crate::kit::{Brush, Fill, Style};
use crate::views::arrangement::slate::{Ink, W};
use cantus_core::Track;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// What a track header shows beyond its track.
#[derive(Copy, Clone, Debug, Default)]
pub struct Look {
    pub selected: bool,
    pub hovered: bool,
    /// The key under the pointer.
    pub hot: Option<Key>,
    pub silenced: bool,
    pub renaming: bool,
    pub drop_above: bool,
    pub divider: bool,
    /// The track's level, 0…1, and whether the song plays.
    pub level: f32,
    /// In a folder: its depth and the folder's colour.
    pub depth: usize,
    pub folder: Option<Color32>,
    /// It has automation lanes, shown or hidden.
    pub lanes: bool,
    pub lanes_open: bool,
    /// The input's level while armed, 0…1.
    pub input: f32,
}

/// How many keys a header of `w` carries.
pub fn keys_of(w: Width) -> usize {
    if w == Width::Narrow {
        2
    } else {
        4
    }
}

/// Paint one track header; returns where its name's letters are (a click there renames).
#[allow(clippy::too_many_arguments)]
pub fn track(
    b: &mut Brush,
    r: &Rects,
    tr: &Track,
    number: &str,
    line2: &str,
    level_text: &str,
    inserts: &str,
    frozen_word: &str,
    look: Look,
    s: &Style,
) -> Option<Rect> {
    let ink = Ink::of(s);
    let k = s.k;
    b.register(r.row, "track_header", TOKENS, &[]);
    let hue = crate::theme::track_color(tr.color);
    body(
        b,
        r.row,
        &ink,
        look.selected.then_some(hue),
        look.depth > 0,
        look.hovered,
    );
    let quiet = tr.muted || look.silenced;
    // The stripe: the folder's continues at x 0 for a child, its own a 2-px bar at the indent.
    match look.folder {
        Some(fh) => {
            stripe(b, r.row, ink.tint(fh, ink.stripe_rest() * 0.8), k);
            let x = r.row.left() + 12.0 * k * look.depth as f32 + 2.0 * k;
            let bar = Rect::from_min_max(
                pos2(x, r.row.top() + 4.0 * k),
                pos2(x + 2.0 * k, r.row.bottom() - 5.0 * k),
            );
            let own = if tr.muted {
                ink.ink4
            } else {
                ink.stripe(hue, look.selected)
            };
            b.rect(b.snap_rect(bar), Fill::Solid(own));
        }
        None => {
            let c = if tr.muted {
                ink.ink4
            } else {
                ink.stripe(hue, look.selected)
            };
            stripe(b, r.row, c, k);
        }
    }
    if tr.armed {
        b.vline(
            r.row.left() + 3.5 * k,
            r.row.top(),
            r.row.bottom(),
            b.px(),
            Ink::a(ink.rec, 0.7),
        );
    }
    // The number, right-aligned at x 24.
    let num_ink = if look.selected { ink.ink2 } else { ink.ink3 };
    words(
        b,
        r.number,
        number,
        W::Regular,
        10.0,
        num_ink,
        Align2::RIGHT_CENTER,
        s,
    );
    // The name.
    let mut letters = None;
    if !look.renaming {
        let (w, c) = if quiet {
            (W::Regular, ink.ink3)
        } else if look.selected {
            (W::Medium, ink.ink1)
        } else {
            (W::Regular, ink.ink1)
        };
        letters = words(b, r.name, &tr.name, w, 12.0, c, Align2::LEFT_CENTER, s)
            .map(|p| Rect::from_x_y_ranges(p.x_range(), r.name.y_range()));
    }
    // Line 2.
    if let Some(l2) = r.line2 {
        let c = if look.selected { ink.ink2 } else { ink.ink3 };
        let mut x = l2.left();
        if tr.frozen.is_some() {
            let w = super::super::slate::width(b.ctx(), frozen_word, W::SemiBold, 10.0, s);
            let tag = Rect::from_min_size(
                pos2(x, l2.center().y - 6.5 * k),
                vec2(w + 8.0 * k, 13.0 * k),
            );
            b.rrect(tag, s.r(3.0), Fill::Solid(Ink::a(ink.accent, 0.18)));
            words(
                b,
                tag,
                frozen_word,
                W::SemiBold,
                10.0,
                ink.accent,
                Align2::CENTER_CENTER,
                s,
            );
            x = tag.right() + 6.0 * k;
        }
        // A wide header shows the level at line 2's right.
        let wide = r.row.width() >= 260.0 * k && r.level.is_none();
        let mut l2 = l2;
        if wide {
            let vr = Rect::from_min_max(pos2(l2.right() - 48.0 * k, l2.top()), l2.max);
            words(
                b,
                vr,
                level_text,
                W::Regular,
                11.0,
                ink.ink3,
                Align2::RIGHT_CENTER,
                s,
            );
            l2.max.x = vr.left() - 4.0 * k;
        }
        let armed = tr.armed && tr.input.is_some();
        let text_c = if armed { ink.ink2 } else { c };
        let room = Rect::from_min_max(pos2(x, l2.top()), l2.max);
        let used = words(
            b,
            room,
            line2,
            W::Regular,
            11.0,
            text_c,
            Align2::LEFT_CENTER,
            s,
        );
        if armed {
            let mx = used.map_or(x, |u| u.right()) + 6.0 * k;
            let m = Rect::from_min_size(pos2(mx, l2.center().y - 1.5 * k), vec2(48.0 * k, 3.0 * k));
            if m.right() <= l2.right() {
                b.rect(
                    b.snap_rect(m),
                    Fill::Solid(Ink::to(ink.surface, ink.edge, 0.55)),
                );
                let f = Rect::from_min_size(
                    m.min,
                    vec2(m.width() * look.input.clamp(0.0, 1.0), m.height()),
                );
                b.rect(b.snap_rect(f), Fill::Solid(ink.play));
            }
        }
    }
    // Line 3: the level as a slider and its value.
    if let Some(lv) = r.level {
        let val_w = 44.0 * k;
        let groove = Rect::from_min_max(
            pos2(lv.left(), lv.center().y - 1.0 * k),
            pos2(lv.right() - val_w - 6.0 * k, lv.center().y + 1.0 * k),
        );
        b.rect(b.snap_rect(groove), Fill::Solid(Ink::a(ink.ink4, 0.6)));
        // 0 dB at 75 % of the groove, −60 at its start, +6 at its end.
        let pos = ((tr.gain_db + 60.0) / 66.0).clamp(0.0, 1.0);
        let x = groove.left() + groove.width() * pos;
        let fill = Rect::from_min_max(groove.min, pos2(x, groove.bottom()));
        b.rect(b.snap_rect(fill), Fill::Solid(Ink::a(ink.ink3, 0.75)));
        b.circle(pos2(x, groove.center().y), 4.0 * k, Fill::Solid(ink.edge));
        b.circle(pos2(x, groove.center().y), 3.0 * k, Fill::Solid(ink.raised));
        let vr = Rect::from_min_max(pos2(lv.right() - val_w, lv.top()), lv.max);
        words(
            b,
            vr,
            level_text,
            W::Regular,
            11.0,
            ink.ink2,
            Align2::RIGHT_CENTER,
            s,
        );
    }
    if let Some(ins) = r.inserts {
        words(
            b,
            ins,
            inserts,
            W::Regular,
            10.0,
            ink.ink3,
            Align2::LEFT_CENTER,
            s,
        );
    }
    keys(b, r, tr, look, &ink, s);
    meter(b, r.meter, look.level, &ink, s);
    edges(b, r.row, look, &ink, s);
    letters
}

/// The drop line of a header being moved, and the resize line on its foot.
fn edges(b: &mut Brush, row: Rect, look: Look, ink: &Ink, s: &Style) {
    if look.drop_above {
        b.hline(row.left(), row.right(), row.top(), 2.0 * s.k, ink.accent);
    }
    if look.divider {
        b.hline(
            row.left(),
            row.right(),
            row.bottom() - 0.5 * s.k,
            2.0 * s.k,
            ink.accent,
        );
        let c = pos2(row.center().x, row.bottom());
        for d in [-4.0, 0.0, 4.0] {
            b.circle(c + vec2(d * s.k, 0.0), 1.1 * s.k, Fill::Solid(ink.ink1));
        }
    }
}

/// The meter: a groove at rest, the level in the meter's greens while it sounds.
pub fn meter(b: &mut Brush, m: Rect, level: f32, ink: &Ink, _s: &Style) {
    b.rect(
        b.snap_rect(m),
        Fill::Solid(Ink::to(ink.surface, ink.edge, 0.55)),
    );
    if level > 0.0 {
        let h = m.height() * level.clamp(0.0, 1.0);
        let lit = Rect::from_min_max(pos2(m.left(), m.bottom() - h), m.max);
        b.rect(b.snap_rect(lit), Fill::Solid(ink.meter_lo));
        let mid = m.bottom() - m.height() * 0.72;
        if lit.top() < mid {
            let top = Rect::from_min_max(lit.min, pos2(m.right(), mid));
            b.rect(b.snap_rect(top), Fill::Solid(ink.meter_mid));
        }
    }
}

/// Whether key `key` is engaged on `tr`.
pub fn engaged(tr: &Track, key: Key, look: Look) -> bool {
    match key {
        Key::Mute => tr.muted,
        Key::Solo => tr.soloed,
        Key::Arm => tr.armed,
        Key::Auto => look.lanes && look.lanes_open,
    }
}

/// The keys: letters at rest, one well under the pointer, the engaged one lit.
fn keys(b: &mut Brush, r: &Rects, tr: &Track, look: Look, ink: &Ink, s: &Style) {
    let k = s.k;
    if look.hovered {
        b.rrect(r.well, s.r(3.0), Fill::Solid(ink.edge));
        b.rrect(r.well.shrink(b.px()), s.r(3.0), Fill::Solid(ink.well));
        for i in 1..r.n {
            let x = r.keys[i].left();
            b.vline(
                x,
                r.well.top() + 3.0 * k,
                r.well.bottom() - 3.0 * k,
                b.px(),
                ink.edge,
            );
        }
        // "More": three dots.
        for d in [-3.5, 0.0, 3.5] {
            b.circle(
                r.more.center() + vec2(d * k, 0.0),
                1.1 * k,
                Fill::Solid(ink.ink2),
            );
        }
    }
    for (i, key) in KEYS.iter().copied().enumerate().take(r.n) {
        let kr = r.keys[i];
        let on = engaged(tr, key, look);
        let hot = look.hot == Some(key);
        let rest = if look.hovered || hot {
            ink.ink2
        } else {
            ink.ink3
        };
        let rest = if hot { ink.ink1 } else { rest };
        match key {
            Key::Mute | Key::Solo => {
                let letter = if key == Key::Mute { "M" } else { "S" };
                let pill_c = if key == Key::Mute { ink.ink2 } else { ink.solo };
                if on {
                    b.rrect(kr.shrink(2.0 * k), s.r(2.0), Fill::Solid(pill_c));
                    words(
                        b,
                        kr,
                        letter,
                        W::SemiBold,
                        10.0,
                        ink.base,
                        Align2::CENTER_CENTER,
                        s,
                    );
                } else {
                    if key == Key::Mute && look.silenced {
                        b.rrect_stroke(kr.shrink(2.5 * k), s.r(2.0), b.px(), Ink::a(ink.solo, 0.7));
                    }
                    words(
                        b,
                        kr,
                        letter,
                        W::SemiBold,
                        10.0,
                        rest,
                        Align2::CENTER_CENTER,
                        s,
                    );
                }
            }
            Key::Arm => {
                if on {
                    b.rrect(
                        kr.shrink(2.0 * k),
                        s.r(2.0),
                        Fill::Solid(Ink::to(ink.well, ink.rec, 0.24)),
                    );
                    b.circle(kr.center(), 3.8 * k, Fill::Solid(ink.rec));
                } else {
                    let c = if look.hovered {
                        rest
                    } else {
                        Ink::a(ink.ink3, 0.7)
                    };
                    b.circle(kr.center(), 3.2 * k, Fill::Solid(c));
                }
            }
            Key::Auto => {
                let c = if on {
                    b.rrect(
                        kr.shrink(2.0 * k),
                        s.r(2.0),
                        Fill::Solid(Ink::to(ink.well, ink.auto, 0.24)),
                    );
                    ink.auto
                } else if look.lanes {
                    Ink::a(ink.auto, 0.75)
                } else if look.hovered {
                    rest
                } else {
                    ink.ink4
                };
                envelope(b, kr.shrink2(vec2(4.5 * k, 4.5 * k)), c, k);
            }
        }
    }
}

/// The automation envelope glyph: a rise, a hold, a fall.
pub fn envelope(b: &mut Brush, r: Rect, c: Color32, k: f32) {
    let pts = [
        pos2(r.left(), r.bottom()),
        pos2(r.left() + r.width() * 0.4, r.top()),
        pos2(r.right(), r.top() + r.height() * 0.35),
    ];
    b.polyline(&pts, 1.3 * k, c);
}

/// A folder's chevron: right when closed, down when open.
pub fn chevron(b: &mut Brush, c: Pos2, open: bool, ink: Color32, k: f32) {
    let d = 3.0 * k;
    let pts = if open {
        [
            c + vec2(-d, -d * 0.5),
            c + vec2(0.0, d * 0.5),
            c + vec2(d, -d * 0.5),
        ]
    } else {
        [
            c + vec2(-d * 0.5, -d),
            c + vec2(d * 0.5, 0.0),
            c + vec2(-d * 0.5, d),
        ]
    };
    b.polyline(&pts, 1.3 * k, ink);
}

/// What a folder's row shows.
#[derive(Copy, Clone, Debug, Default)]
pub struct FolderLook {
    pub open: bool,
    pub selected: bool,
    pub hovered: bool,
    pub hot: Option<Key>,
    pub muted: bool,
    pub soloed: bool,
    pub renaming: bool,
}

/// A folder's row: its stripe, the chevron in the number's place, a Medium name, M S;
/// closed, its summary on line 2. Returns the name's letters.
#[allow(clippy::too_many_arguments)]
pub fn folder(
    b: &mut Brush,
    row: Rect,
    name: &str,
    hue: Color32,
    line2: &str,
    look: FolderLook,
    s: &Style,
) -> (Rects, Option<Rect>) {
    let ink = Ink::of(s);
    let k = s.k;
    let r = rects(row, 0, 2, false, s);
    b.register(row, "folder_header", TOKENS, &[]);
    body(
        b,
        row,
        &ink,
        look.selected.then_some(hue),
        false,
        look.hovered,
    );
    let c = if look.muted {
        ink.ink4
    } else {
        ink.stripe(hue, look.selected)
    };
    stripe(b, row, c, k);
    chevron(
        b,
        pos2(r.number.right() - 4.0 * k, r.number.center().y),
        look.open,
        ink.ink2,
        k,
    );
    let mut letters = None;
    if !look.renaming {
        let c = if look.muted { ink.ink3 } else { ink.ink1 };
        letters = words(b, r.name, name, W::Medium, 12.0, c, Align2::LEFT_CENTER, s)
            .map(|p| Rect::from_x_y_ranges(p.x_range(), r.name.y_range()));
    }
    if let (false, Some(l2)) = (look.open, r.line2) {
        words(
            b,
            l2,
            line2,
            W::Regular,
            11.0,
            ink.ink3,
            Align2::LEFT_CENTER,
            s,
        );
    }
    // M S, as a track's.
    if look.hovered {
        b.rrect(r.well, s.r(3.0), Fill::Solid(ink.edge));
        b.rrect(r.well.shrink(b.px()), s.r(3.0), Fill::Solid(ink.well));
        b.vline(
            r.keys[1].left(),
            r.well.top() + 3.0 * k,
            r.well.bottom() - 3.0 * k,
            b.px(),
            ink.edge,
        );
    }
    for (i, (letter, on, pill)) in [("M", look.muted, ink.ink2), ("S", look.soloed, ink.solo)]
        .into_iter()
        .enumerate()
    {
        let kr = r.keys[i];
        let hot = look.hot == Some(if i == 0 { Key::Mute } else { Key::Solo });
        if on {
            b.rrect(kr.shrink(2.0 * k), s.r(2.0), Fill::Solid(pill));
            words(
                b,
                kr,
                letter,
                W::SemiBold,
                10.0,
                ink.base,
                Align2::CENTER_CENTER,
                s,
            );
        } else {
            let c = if hot {
                ink.ink1
            } else if look.hovered {
                ink.ink2
            } else {
                ink.ink3
            };
            words(
                b,
                kr,
                letter,
                W::SemiBold,
                10.0,
                c,
                Align2::CENTER_CENTER,
                s,
            );
        }
    }
    (r, letters)
}

/// The add-track row: a plus and "Add track"; under the pointer a well and its key.
pub fn add_row(b: &mut Brush, row: Rect, label: &str, key: &str, hover: bool, s: &Style) {
    let ink = Ink::of(s);
    let k = s.k;
    b.register(row, "add_track", TOKENS, &[]);
    b.rect(
        b.snap_rect(row),
        Fill::Solid(Ink::to(ink.surface, ink.base, 0.55)),
    );
    let (fg, plus_c) = if hover {
        let w = row.shrink2(vec2(8.0 * k, 5.0 * k));
        b.rrect(w, s.r(3.0), Fill::Solid(ink.well_hi));
        (ink.ink1, ink.ink1)
    } else {
        (ink.ink3, ink.ink3)
    };
    let c = pos2(row.left() + 22.0 * k, row.center().y);
    b.hline(c.x - 4.0 * k, c.x + 4.0 * k, c.y, 1.2 * k, plus_c);
    b.vline(c.x, c.y - 4.0 * k, c.y + 4.0 * k, 1.2 * k, plus_c);
    let text = Rect::from_min_max(
        pos2(row.left() + 32.0 * k, row.top()),
        pos2(row.right() - 50.0 * k, row.bottom()),
    );
    words(b, text, label, W::Regular, 11.0, fg, Align2::LEFT_CENTER, s);
    if hover {
        let kr = Rect::from_min_max(
            pos2(row.right() - 56.0 * k, row.top()),
            pos2(row.right() - 14.0 * k, row.bottom()),
        );
        words(
            b,
            kr,
            key,
            W::Medium,
            9.0,
            ink.ink3,
            Align2::RIGHT_CENTER,
            s,
        );
    }
}

/// Faint rows' ground below the last header.
pub fn below(b: &mut Brush, rect: Rect, s: &Style) {
    let ink = Ink::of(s);
    b.rect(b.snap_rect(rect), Fill::Solid(ink.surface));
}
