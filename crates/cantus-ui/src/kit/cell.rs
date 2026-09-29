//! `Cell` — FL LuxeVerb's control cell: **the value pill on top, the control, the label
//! below**, one object. The pill takes drag, wheel and typing like the control; while
//! the control is dragged the pill lights in the section's hue. This is the cure for
//! loose words: a value, its control and its name cannot drift apart.

use super::brush::{Brush, Fill};
use super::color::{ensure_contrast, mix, shade};
use super::fader::{self, FaderLook};
use super::knob::{self, KnobLook, KnobSize};
use super::text::{self, Policy, Role, Src, Text};
use super::{State, Style, Tok};

/// The tokens a cell paints with (for inspect mode); its control registers itself.
pub const TOKENS: &[Tok] = &[
    Tok::Deck,
    Tok::ValuePill,
    Tok::ValuePillText,
    Tok::Label,
    Tok::Focus,
];
use egui::{Align2, Pos2, Rect, Vec2};

/// The control inside a cell.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Control<'a> {
    Knob(KnobLook<'a>),
    /// A vertical fader this tall (points at 100 %) — LuxeVerb's DRY and WET.
    Fader(FaderLook<'a>, f32),
}

/// What a cell shows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CellLook<'a> {
    pub control: Control<'a>,
    /// The formatted value, shown in the pill.
    pub value: &'a str,
    /// The label; give an i18n key so `Abbreviate` can find its `.short` form.
    pub label: Src<'a>,
}

/// Where a cell's parts are.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CellRects {
    pub pill: Rect,
    pub control: Rect,
    pub label: Rect,
}

fn pill_h(size: KnobSize) -> f32 {
    match size {
        KnobSize::XL => 22.0,
        KnobSize::S | KnobSize::Mini => 14.0,
        _ => 16.0,
    }
}

/// The size of a cell around a knob of `size`, at 100 % (`plugin-instrument-uis.md`
/// §7.7: S 48, M 64, L 80, XL 112 wide).
pub fn size(size: KnobSize) -> Vec2 {
    let w = match size {
        KnobSize::Mini => 40.0,
        KnobSize::S => 48.0,
        KnobSize::M => 64.0,
        KnobSize::L => 80.0,
        KnobSize::XL => 112.0,
    };
    let h = 6.0 + pill_h(size) + 4.0 + size.diameter() + 4.0 + 14.0 + 6.0;
    Vec2::new(w, h)
}

/// The size of a cell around a fader of `height` points.
pub fn fader_size(height: f32) -> Vec2 {
    Vec2::new(64.0, 6.0 + 16.0 + 4.0 + height + 4.0 + 14.0 + 6.0)
}

pub fn rects(rect: Rect, s: &Style, look: &CellLook<'_>) -> CellRects {
    let (ph, ch) = match look.control {
        Control::Knob(k) => (pill_h(k.size), k.size.diameter()),
        Control::Fader(_, h) => (16.0, h),
    };
    let pad = s.s(6.0);
    let pill_w = (rect.width() - s.s(4.0)).min(s.s(if ph > 20.0 { 96.0 } else { 72.0 }));
    let pill = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + pad + s.s(ph) * 0.5),
        Vec2::new(pill_w, s.s(ph)),
    );
    let control = Rect::from_center_size(
        Pos2::new(rect.center().x, pill.bottom() + s.s(4.0) + s.s(ch) * 0.5),
        Vec2::new(rect.width() - 2.0 * pad, s.s(ch)),
    );
    let label = Rect::from_min_max(
        Pos2::new(rect.left() + s.s(3.0), control.bottom() + s.s(4.0)),
        Pos2::new(
            rect.right() - s.s(3.0),
            control.bottom() + s.s(4.0) + s.s(14.0),
        ),
    );
    CellRects {
        pill,
        control,
        label,
    }
}

/// Paint a cell in `rect` (of [`size`]).
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &CellLook<'_>) -> CellRects {
    b.register(rect, "cell", TOKENS, &[s.hue_tok]);
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let dark = s.t.dark;
    let mut bg = shade(s.t.deck, if dark { -0.035 } else { -0.03 });
    if st.hovered && !st.disabled {
        bg = shade(bg, if dark { 0.015 } else { -0.01 });
    }
    let rect = b.snap_rect(rect);
    b.rrect(rect, s.r(6.0), Fill::Solid(f(bg)));
    let r = rects(rect, s, look);

    // The pill: pale, dark text; lit in the hue while the control is dragged.
    let pill_fill = if st.active {
        mix(s.t.value_pill, s.hue, 0.45)
    } else {
        s.t.value_pill
    };
    let pill_text = ensure_contrast(s.t.value_pill_text, pill_fill, 4.5);
    let pill = b.snap_rect(r.pill);
    b.rrect(
        pill,
        s.r(3.0),
        Fill::V(
            f(shade(pill_fill, 0.02 * s.t.depth)),
            f(shade(pill_fill, -0.02 * s.t.depth)),
        ),
    );
    let role = match look.control {
        Control::Knob(k) if k.size == KnobSize::XL => Role::Hero,
        _ => Role::Value,
    };
    let _ = text::paint(
        b,
        pill.shrink2(Vec2::new(s.s(2.0), 0.0)),
        &Text::new(look.value, role, f(pill_text)).policy(Policy::Elide(text::Elide::End)),
        s.tk(),
    );

    // The control.
    let control_state = State {
        focused: false,
        ..st
    };
    match look.control {
        Control::Knob(k) => knob::paint(b, r.control.center(), s, control_state, &k),
        Control::Fader(fl, _) => fader::paint(b, r.control, s, control_state, &fl),
    }

    // The label, in caps, abbreviated when it must be.
    let t = Text {
        src: look.label,
        role: Role::Label,
        caps: true,
        align: Align2::CENTER_CENTER,
        policy: Policy::Abbreviate,
        color: f(s.t.label),
    };
    let _ = text::paint(b, r.label, &t, s.tk());
    if st.focused {
        super::focus_ring(b, rect, s.r(6.0), s);
    }
    r
}
