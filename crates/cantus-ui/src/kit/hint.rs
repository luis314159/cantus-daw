//! The hint every kit widget writes when hovered or dragged (ADR 0024 §9): what the
//! control is called, its value, and what it can do. The toolbar's hint bar and a
//! device's help line read it.
//!
//! The slot lives in the egui context, so nothing needs a handle on the application:
//! a widget calls [`set`], the bar calls [`current`]. Writing reuses the slot's
//! buffers, so a control being dragged does not allocate every frame.

use egui::{Context, Id};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

/// What a control can do, shown as icons at the hint bar's right (FL's capability
/// strip).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Caps {
    /// Can be automated or modulated.
    pub automatable: bool,
    /// Has a right-click menu.
    pub menu: bool,
    /// Something needs attention (a plug-in adding latency, a missing file).
    pub warning: bool,
    /// Can be typed into.
    pub typed: bool,
}

impl Caps {
    pub const NONE: Caps = Caps {
        automatable: false,
        menu: false,
        warning: false,
        typed: false,
    };
    /// A parameter: automatable, with a menu, typed entry.
    pub const PARAM: Caps = Caps {
        automatable: true,
        menu: true,
        warning: false,
        typed: true,
    };
}

/// The hovered control's name, value and capabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hint {
    pub name: String,
    /// Empty when the control has no value to show.
    pub value: String,
    pub caps: Caps,
    /// The control is being dragged: the value is live.
    pub live: bool,
    /// Something drives the control — an automation clip, a modulator, a controller, a VCA
    /// (round 39, ADR 0109): the tag the application knows it by ([`drive`]), 0 when
    /// nothing does.
    pub drive: u64,
    /// What drives it and how to take it back, in words ([`resolve`]); empty until the
    /// application has said, and when nothing drives it.
    pub driven: String,
}

#[derive(Default)]
struct Slot {
    hint: Hint,
    pass: u64,
    /// The last tag the application put in words: the tag, its stamp and the words (kept
    /// while the pointer rests, so a hover or a drag re-reads them and writes nothing).
    resolved: (u64, u64, String),
    /// The pass a [`drive`] was written in: a [`set`] in the same pass keeps it (a control
    /// may say what drives it before or after it writes its hint).
    drive_pass: Option<u64>,
}

fn slot(ctx: &Context) -> Arc<Mutex<Slot>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("cantus-kit-hint"), || {
            Arc::new(Mutex::new(Slot::default()))
        })
        .clone()
    })
}

/// Write the hint for the control under the pointer: its `name`, and a value written
/// into the slot's own buffer by `value` (so formatting a number does not allocate).
pub fn set(
    ctx: &Context,
    name: &str,
    caps: Caps,
    live: bool,
    value: impl FnOnce(&mut String) -> std::fmt::Result,
) {
    let s = slot(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.pass = ctx.cumulative_pass_nr();
    let keep = s.drive_pass == Some(s.pass);
    let h = &mut s.hint;
    h.name.clear();
    h.name.push_str(name);
    h.value.clear();
    let _ = value(&mut h.value);
    // A control with no name of its own (a menu field, a choice) is named by what it shows
    // (round 39, ADR 0109: the help line is never empty).
    if h.name.is_empty() {
        std::mem::swap(&mut h.name, &mut h.value);
    }
    h.caps = caps;
    h.live = live;
    if !keep {
        h.drive = 0;
        h.driven.clear();
    }
}

/// The control whose hint was just [`set`] is driven by something the application knows
/// by `tag` (round 39, ADR 0109): the hint carries the words the application gave that
/// tag ([`resolve`]) — at once when it has them, from the next frame otherwise. A tag of 0
/// is nothing.
pub fn drive(ctx: &Context, tag: u64) {
    if tag == 0 {
        return;
    }
    let s = slot(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.drive_pass = Some(ctx.cumulative_pass_nr());
    let Slot { hint, resolved, .. } = &mut *s;
    hint.drive = tag;
    hint.driven.clear();
    if resolved.0 == tag {
        hint.driven.push_str(&resolved.2);
    }
}

/// The tag of the driven control under the pointer (written this frame or the last) and
/// whether the words kept for it are already those of `stamp`: what the application reads
/// once a frame to know whether it must put a tag in words ([`resolve`]).
pub fn wanted(ctx: &Context, stamp: u64) -> Option<(u64, bool)> {
    let s = slot(ctx);
    let s = s.lock().unwrap_or_else(|e| e.into_inner());
    let now = ctx.cumulative_pass_nr();
    let fresh = now.saturating_sub(s.pass) <= 1 && !s.hint.name.is_empty();
    (fresh && s.hint.drive != 0).then(|| {
        let tag = s.hint.drive;
        (tag, s.resolved.0 == tag && s.resolved.1 == stamp)
    })
}

/// The application's words for `tag` as of `stamp` — what drives the control and how to
/// take it back; empty when nothing does after all: kept for the next hovers, and given to
/// the hint now when it is that control's.
pub fn resolve(ctx: &Context, tag: u64, stamp: u64, words: &str) {
    let s = slot(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    let Slot { hint, resolved, .. } = &mut *s;
    resolved.0 = tag;
    resolved.1 = stamp;
    resolved.2.clear();
    resolved.2.push_str(words);
    if hint.drive == tag {
        hint.driven.clear();
        hint.driven.push_str(words);
    }
}

/// Whether a control has written the hint in this pass (round 39, ADR 0109): a fallback
/// — a text field's own words, a resize edge's — writes only where nothing more precise
/// did.
pub fn written(ctx: &Context) -> bool {
    let s = slot(ctx);
    let s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.pass == ctx.cumulative_pass_nr() && !s.hint.name.is_empty()
}

/// [`set_text`] unless a control has written the hint in this pass already.
pub fn fallback(ctx: &Context, name: &str, value: &str, caps: Caps) {
    if !written(ctx) {
        set_text(ctx, name, value, caps);
    }
}

/// [`set`] with a value that is already text.
pub fn set_text(ctx: &Context, name: &str, value: &str, caps: Caps) {
    set(ctx, name, caps, false, |b| b.write_str(value));
}

/// The hint written this frame or the last, if any control is hovered.
pub fn current(ctx: &Context) -> Option<Hint> {
    let s = slot(ctx);
    let s = s.lock().unwrap_or_else(|e| e.into_inner());
    let now = ctx.cumulative_pass_nr();
    (now.saturating_sub(s.pass) <= 1 && !s.hint.name.is_empty()).then(|| s.hint.clone())
}

/// Read the current hint without cloning it.
pub fn read<R>(ctx: &Context, f: impl FnOnce(Option<&Hint>) -> R) -> R {
    let s = slot(ctx);
    let s = s.lock().unwrap_or_else(|e| e.into_inner());
    let now = ctx.cumulative_pass_nr();
    let fresh = now.saturating_sub(s.pass) <= 1 && !s.hint.name.is_empty();
    f(fresh.then_some(&s.hint))
}

/// The tooltip and its keycaps, drawn (round 39, `des-controls`' tips sheet): a raised
/// sheet (corners 5, a 1-px edge, a shadow 0 3 10), padding 7 / 10 / 8; the name Medium 12
/// in ink 1 with the value Regular 12 after it; what it does Regular 11 in ink 2; how,
/// Regular 10.5 in ink 3. A driven control's sign leads its first line.
pub mod tip {
    use super::super::brush::{Brush, Fill};
    use super::super::color::{mix, with_alpha};
    use super::super::drive_sign::{paint_sign, Sign};
    use super::super::text::{self, Role, Text};
    use super::super::{Style, Tok};
    use egui::{Align2, Pos2, Rect, Vec2};

    /// The tokens a tooltip paints with (for inspect mode).
    pub const TOKENS: &[Tok] = &[
        Tok::Menu,
        Tok::Text,
        Tok::TextDim,
        Tok::Placeholder,
        Tok::Dark,
        Tok::Auto,
        Tok::Modulated,
    ];

    /// What a tooltip says.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct Look<'a> {
        pub name: &'a str,
        pub value: &'a str,
        /// What the control does.
        pub what: &'a str,
        /// How: the gesture, the key.
        pub how: &'a str,
        /// What drives it, leading the first line.
        pub sign: Option<Sign>,
    }

    fn lines<'a>(look: &Look<'a>, s: &Style) -> [(Text<'a>, f32); 4] {
        let t = &s.t;
        [
            (Text::new(look.name, Role::Control, t.text), s.tk()),
            (Text::new(look.value, Role::Body, t.text), s.tk()),
            (Text::new(look.what, Role::Label, t.text_dim), s.tk()),
            (
                Text::new(look.how, Role::Label, t.placeholder),
                s.tk() * 10.5 / 11.0,
            ),
        ]
    }

    /// The tooltip's size for `look`.
    pub fn size(ctx: &egui::Context, s: &Style, look: &Look<'_>) -> Vec2 {
        let l = lines(look, s);
        let w = |i: usize| text::measure(ctx, &l[i].0, l[i].1).x;
        let sign = look.sign.map_or(0.0, |_| s.s(7.0 * 1.8 + 6.0));
        let first = sign
            + w(0)
            + if look.value.is_empty() {
                0.0
            } else {
                s.s(6.0) + w(1)
            };
        let mut h = s.s(7.0 + 16.0 + 8.0);
        let mut wide = first;
        for i in [2, 3] {
            if ![look.what, look.how][i - 2].is_empty() {
                wide = wide.max(w(i));
                h += s.s(15.0);
            }
        }
        Vec2::new(wide + s.s(20.0) + 2.0, h)
    }

    /// Paint the tooltip for `look` with its top-left at `at`; its rect.
    pub fn paint(b: &mut Brush, at: Pos2, s: &Style, look: &Look<'_>) -> Rect {
        let rect = b.snap_rect(Rect::from_min_size(at, size(b.ctx(), s, look)));
        b.register(rect, "tooltip", TOKENS, &[]);
        let t = &s.t;
        let r = s.s(5.0);
        b.rrect_soft(
            rect.translate(Vec2::new(0.0, s.s(3.0))),
            r + s.s(2.0),
            Fill::Solid(with_alpha(t.dark_shade, if t.dark { 0.45 } else { 0.16 })),
            s.s(5.0),
        );
        let edge = if t.dark {
            mix(t.menu, t.text, 0.09)
        } else {
            t.grid_bar
        };
        b.rrect(rect, r, Fill::Solid(edge));
        b.rrect(
            rect.shrink(b.px()),
            (r - b.px()).max(0.0),
            Fill::Solid(t.menu),
        );
        let l = lines(look, s);
        let mut x = rect.left() + s.s(10.0);
        let mut y = rect.top() + s.s(7.0);
        let row = |x: f32, y: f32| {
            Rect::from_min_max(Pos2::new(x, y), Pos2::new(rect.right(), y + s.s(16.0)))
        };
        if let Some(g) = look.sign {
            let h = s.s(7.0);
            paint_sign(
                b,
                Pos2::new(x + h * 0.9, y + s.s(8.0)),
                h,
                g,
                g.color(s),
                None,
            );
            x += h * 1.8 + s.s(6.0);
        }
        let name = l[0].0.align(Align2::LEFT_CENTER);
        let nw = text::measure(b.ctx(), &name, l[0].1).x;
        let _ = text::paint(b, row(x, y), &name, l[0].1);
        if !look.value.is_empty() {
            let vr = Rect::from_min_max(
                Pos2::new(x + nw + s.s(6.0), y),
                Pos2::new(rect.right(), y + s.s(16.0)),
            );
            let _ = text::paint(b, vr, &l[1].0.align(Align2::LEFT_CENTER), l[1].1);
        }
        let x0 = rect.left() + s.s(10.0);
        y += s.s(16.0);
        for i in [2, 3] {
            if ![look.what, look.how][i - 2].is_empty() {
                let rr =
                    Rect::from_min_max(Pos2::new(x0, y), Pos2::new(rect.right(), y + s.s(15.0)));
                let _ = text::paint(b, rr, &l[i].0.align(Align2::LEFT_CENTER), l[i].1);
                y += s.s(15.0);
            }
        }
        rect
    }
}

/// The help line with its keys as keycaps (round 39, `des-controls`' help line: "Drag ·
/// Shift for fine · double-click to type", every key name boxed). Laid out without
/// allocating: plain words run together between keys.
pub mod keys {
    use super::super::brush::Brush;
    use super::super::text::{self, Role, Text};
    use super::super::Style;
    use egui::{Align2, Color32, Pos2, Rect};

    const NAMED: [&str; 22] = [
        "Esc",
        "Enter",
        "Intro",
        "Tab",
        "Space",
        "Espacio",
        "Del",
        "Supr",
        "Delete",
        "Backspace",
        "Retroceso",
        "Home",
        "End",
        "PgUp",
        "PgDn",
        "Ctrl",
        "Shift",
        "Alt",
        "Cmd",
        "Mayús",
        "Opción",
        "Option",
    ];
    const MODIFIERS: [&str; 7] = ["Ctrl", "Shift", "Alt", "Cmd", "Mayús", "Opción", "Option"];

    /// The part of `word` that names keys ("Ctrl+Shift+Z", "Alt" of "Alt+drag", "F5") and
    /// the rest ("+drag", "," after a key); `None` when it names none.
    pub fn split_key(word: &str) -> Option<(&str, &str)> {
        let trimmed = word.trim_end_matches([',', '.', ';', ':', ')']);
        let tail = &word[trimmed.len()..];
        let is_key = |k: &str| {
            NAMED.contains(&k)
                || (k.len() <= 3
                    && k.starts_with('F')
                    && k[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)))
                || (k.chars().count() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()))
        };
        if NAMED.contains(&trimmed) || is_key_f(trimmed) {
            return Some((trimmed, tail));
        }
        // A chord: modifiers joined by '+', ending in a key — or in a gesture ("+drag").
        let first = trimmed.split('+').next()?;
        if !MODIFIERS.contains(&first) || !trimmed.contains('+') {
            return None;
        }
        let mut end = 0;
        for (i, part) in trimmed.split('+').enumerate() {
            let at = if i == 0 { 0 } else { end + 1 };
            if !(MODIFIERS.contains(&part) || is_key(part)) {
                return Some((&trimmed[..end], &word[end..]));
            }
            end = at + part.len();
        }
        Some((trimmed, tail))
    }

    fn is_key_f(k: &str) -> bool {
        k.len() <= 3
            && k.starts_with('F')
            && k[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n))
    }

    /// Walk the runs of `words`: `f(run, is_key)` for each, plain runs whole between keys.
    fn runs<'a>(words: &'a str, mut f: impl FnMut(&'a str, bool)) {
        let mut plain_start = 0;
        let mut at = 0;
        for word in words.split(' ') {
            let start = at;
            at += word.len() + 1;
            if let Some((key, rest)) = split_key(word) {
                if start > plain_start {
                    f(&words[plain_start..start], false);
                }
                f(key, true);
                // What follows the key ("+drag", a comma) starts the next plain run.
                plain_start = start + key.len();
                let _ = rest;
            }
        }
        if plain_start < words.len() {
            f(&words[plain_start..], false);
        }
    }

    /// The width `words` take with their keycaps, in `role` at `k`.
    pub fn width(ctx: &egui::Context, s: &Style, words: &str, role: Role, k: f32) -> f32 {
        let mut w = 0.0;
        runs(words, |run, key| {
            w += if key {
                cap_width(ctx, s, run) + s.s(4.0)
            } else {
                text::measure(ctx, &Text::new(run, role, s.t.text), k).x
            };
        });
        w
    }

    fn cap_width(ctx: &egui::Context, s: &Style, key: &str) -> f32 {
        let k = s.tk() * 9.5 / 11.0;
        text::measure(ctx, &Text::new(key, Role::Section, s.t.text_dim), k).x + s.s(8.0)
    }

    /// Paint `words` left-aligned in `rect`, its keys as keycaps; the width painted.
    pub fn paint(
        b: &mut Brush,
        rect: Rect,
        s: &Style,
        words: &str,
        role: Role,
        ink: Color32,
        k: f32,
    ) -> f32 {
        let mut x = rect.left();
        let y = rect.center().y;
        runs(words, |run, key| {
            if key {
                let w = super::super::dialog::keycap(b, Pos2::new(x + s.s(2.0), y), s, run);
                x += w + s.s(4.0);
            } else {
                let t = Text::new(run, role, ink).align(Align2::LEFT_CENTER);
                let w = text::measure(b.ctx(), &t, k).x;
                let r = Rect::from_min_max(
                    Pos2::new(x, rect.top()),
                    Pos2::new(x + w + 1.0, rect.bottom()),
                );
                let _ = text::paint(b, r, &t, k);
                x += w;
            }
        });
        x - rect.left()
    }

    #[cfg(test)]
    mod tests {
        use super::split_key;

        #[test]
        fn key_names_are_found_and_gestures_left_as_words() {
            assert_eq!(split_key("Ctrl+Z"), Some(("Ctrl+Z", "")));
            assert_eq!(split_key("Ctrl+Shift+Z,"), Some(("Ctrl+Shift+Z", ",")));
            assert_eq!(split_key("Alt+drag"), Some(("Alt", "+drag")));
            assert_eq!(split_key("Esc"), Some(("Esc", "")));
            assert_eq!(split_key("F5."), Some(("F5", ".")));
            assert_eq!(split_key("Shift"), Some(("Shift", "")));
            assert_eq!(split_key("drag"), None);
            assert_eq!(split_key("Double-click"), None);
            assert_eq!(split_key("Fine"), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hint_is_read_back_and_expires() {
        let ctx = Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            set(ui.ctx(), "Cutoff", Caps::PARAM, true, |b| {
                write!(b, "{:.1} kHz", 2.4)
            });
        });
        out.textures_delta.clear();
        let h = current(&ctx).expect("a hint was written");
        assert_eq!(h.name, "Cutoff");
        assert_eq!(h.value, "2.4 kHz");
        assert!(h.caps.automatable && h.live);
        for _ in 0..3 {
            ctx.run_ui(Default::default(), |_| {})
                .textures_delta
                .clear();
        }
        assert!(current(&ctx).is_none(), "a hint outlives the hover");
    }

    /// Round 39 (ADR 0109): a driven control's hint asks for its words once; the words the
    /// application gives are kept for the next frames' hovers and dropped by a control
    /// nothing drives.
    #[test]
    fn a_driven_hint_asks_for_its_words_once() {
        let ctx = Context::default();
        let frame = |tag: u64| {
            ctx.run_ui(Default::default(), |ui| {
                set_text(ui.ctx(), "Hardness", "0.25", Caps::PARAM);
                drive(ui.ctx(), tag);
            })
            .textures_delta
            .clear();
        };
        frame(7);
        assert_eq!(wanted(&ctx, 1), Some((7, false)), "not in words yet");
        assert!(current(&ctx).unwrap().driven.is_empty());
        resolve(&ctx, 7, 1, "Automated by the clip");
        assert_eq!(current(&ctx).unwrap().driven, "Automated by the clip");
        assert_eq!(wanted(&ctx, 1), Some((7, true)));
        assert_eq!(wanted(&ctx, 2), Some((7, false)), "a new stamp asks again");
        frame(7);
        assert_eq!(current(&ctx).unwrap().driven, "Automated by the clip");
        frame(0);
        let h = current(&ctx).unwrap();
        assert_eq!((h.drive, h.driven.as_str()), (0, ""));
        assert_eq!(wanted(&ctx, 1), None);
    }
}
