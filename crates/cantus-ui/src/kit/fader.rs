//! `Fader` — a 4-pt groove with ticks every 6 dB and FL's cap: 16 × 25 pt, lit above its
//! groove line and hollow below it, the lightest object on a strip. Lime (the selection
//! colour) when its strip is selected, with the groove below the cap filled lime too.

use super::brush::{Brush, Fill};
use super::color::{mix, shade, with_alpha};
use super::light::{self, Material, Raised, RaisedOpts};
use super::{State, Style, Tok};

/// The tokens a fader paints with in a dark theme (for inspect mode).
pub const TOKENS: &[Tok] = &[Tok::Cap, Tok::Selection, Tok::Well, Tok::Text, Tok::Focus];
/// In a light theme (its ticks are the secondary text).
pub const LIGHT_TOKENS: &[Tok] = &[
    Tok::Cap,
    Tok::Selection,
    Tok::Well,
    Tok::TextDim,
    Tok::Focus,
];
use egui::{Pos2, Rect, Vec2};

pub const METAL_TOKENS: &[Tok] = &[
    Tok::Cap,
    Tok::Selection,
    Tok::Well,
    Tok::Text,
    Tok::Focus,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::Drop,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
];
/// In a light theme (its ticks are the secondary text).
pub const METAL_LIGHT_TOKENS: &[Tok] = &[
    Tok::Cap,
    Tok::Selection,
    Tok::Well,
    Tok::TextDim,
    Tok::Focus,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::Drop,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
];

/// Which cap a fader wears (round 37, the owner: "Los dos están chidos, ¿puedes hacerlo
/// configurable?"): an appearance preference, Classic by default.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum FaderCap {
    /// Round 36's ivory cap, lit above its groove and hollow below.
    #[default]
    Classic,
    /// Round 37's metal cap: a drop shadow, an engraved index, the theme's specular.
    Metal,
}

impl FaderCap {
    pub const ALL: [FaderCap; 2] = [FaderCap::Classic, FaderCap::Metal];

    /// Its name's i18n key.
    pub fn name_key(self) -> &'static str {
        match self {
            FaderCap::Classic => "kit.fader.classic",
            FaderCap::Metal => "kit.fader.metal",
        }
    }
}

static CAP_STYLE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The cap every fader wears from now on (set from the appearance when it is applied: a
/// change shows on the next frame).
pub fn set_cap(c: FaderCap) {
    let v = match c {
        FaderCap::Classic => 0,
        FaderCap::Metal => 1,
    };
    CAP_STYLE.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// The cap faders wear.
pub fn cap() -> FaderCap {
    match CAP_STYLE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => FaderCap::Metal,
        _ => FaderCap::Classic,
    }
}

/// The cap's size at 100 %.
pub const CAP: Vec2 = Vec2::new(14.0, 20.0); // r37:finish (was 16 × 25: judge 9, "chunky plastic faders")

/// What a fader shows. Positions are 0 (bottom) … 1 (top).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FaderLook<'a> {
    pub pos: f32,
    /// Tick positions (every 6 dB, usually), drawn beside the groove.
    pub ticks: &'a [f32],
    /// The unity mark, drawn a little longer.
    pub unity: Option<f32>,
}

/// The cap's rect for `pos` in a fader occupying `rect`.
pub fn cap_rect(rect: Rect, s: &Style, pos: f32) -> Rect {
    let cap = CAP * s.k;
    let travel = (rect.height() - cap.y).max(0.0);
    let cy = rect.bottom() - cap.y * 0.5 - travel * pos.clamp(0.0, 1.0);
    Rect::from_center_size(Pos2::new(rect.center().x, cy), cap)
}

/// The tip of the mark beside a fader in `rect` where modulation has it now: at the height
/// the cap would sit at `live` (review of fix3), on the groove's right edge.
pub fn live_mark_tip(rect: Rect, s: &Style, live: f32) -> Pos2 {
    Pos2::new(
        rect.right() - s.s(1.0),
        cap_rect(rect, s, live.clamp(0.0, 1.0)).center().y,
    )
}

/// The modulation's live mark beside a fader: the desk's and the inspector's pinned strip's
/// (one drawing, so the two never disagree; review of fix4).
pub fn live_mark(b: &mut Brush, rect: Rect, s: &Style, live: f32) {
    let tip = live_mark_tip(rect, s, live);
    let d = s.s(4.0);
    b.polygon(
        &[
            tip,
            Pos2::new(tip.x + d, tip.y - d),
            Pos2::new(tip.x + d, tip.y + d),
        ],
        s.t.hue_mod,
    );
}

/// The position (0…1) a pointer at `y` asks for, for a fader in `rect`.
pub fn pos_at(rect: Rect, s: &Style, y: f32) -> f32 {
    let cap = CAP.y * s.k;
    let travel = (rect.height() - cap).max(1.0);
    ((rect.bottom() - cap * 0.5 - y) / travel).clamp(0.0, 1.0)
}

/// Paint a vertical fader in `rect`; `st.lit` is "the strip is selected" — with the cap the
/// appearance asks for ([`cap`]).
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &FaderLook<'_>) {
    paint_as(b, rect, s, st, look, cap());
}

/// [`paint`] with the cap `with` (the gallery draws both).
pub fn paint_as(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    look: &FaderLook<'_>,
    with: FaderCap,
) {
    if s.t.system.recessed() {
        // Slate and its siblings (round 39, `des-controls`): one cap, lit from above.
        let d = super::automate::current_driving();
        super::fader_slate::paint(b, rect, s, st, look, d);
        return;
    }
    match with {
        FaderCap::Classic => paint_classic(b, rect, s, st, look),
        FaderCap::Metal => paint_metal(b, rect, s, st, look),
    }
}

/// The classic fader (round 36's, the owner's): a 4-pt groove and FL's cap, lit above its
/// groove line and hollow below it, the lightest object on a strip.
fn paint_classic(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &FaderLook<'_>) {
    let toks = if s.t.dark { TOKENS } else { LIGHT_TOKENS };
    b.register(rect, "fader", toks, &[]);
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let cap = cap_rect(rect, s, look.pos);
    let cx = b.snap(rect.center().x);
    let gw = s.s(4.0);
    let top = rect.top() + CAP.y * s.k * 0.5;
    let bottom = rect.bottom() - CAP.y * s.k * 0.5;
    let groove = Rect::from_min_max(
        Pos2::new(cx - gw * 0.5, top - gw),
        Pos2::new(cx + gw * 0.5, bottom + gw),
    );

    // Ticks either side of the groove.
    let tick = with_alpha(if s.t.dark { s.t.text } else { s.t.text_dim }, 0.30);
    for &t in look.ticks {
        let y = bottom - (bottom - top) * t;
        let long = look.unity.is_some_and(|u| (u - t).abs() < 1e-3);
        let len = s.s(if long { 5.0 } else { 3.0 });
        let y = b.snap_mid(y);
        b.hline(
            groove.left() - s.s(2.0) - len,
            groove.left() - s.s(2.0),
            y,
            b.px(),
            f(tick),
        );
        b.hline(
            groove.right() + s.s(2.0),
            groove.right() + s.s(2.0) + len,
            y,
            b.px(),
            f(tick),
        );
    }

    let groove_base = shade(s.t.well, if s.t.dark { -0.10 } else { -0.18 });
    light::sunken(b, groove, gw * 0.5, groove_base, a, s.t.depth);
    if st.lit {
        // The groove below the cap lights up in the selection colour.
        let lit =
            Rect::from_min_max(Pos2::new(groove.left(), cap.center().y), groove.max).shrink(b.px());
        b.rrect(lit, gw * 0.5, Fill::Solid(f(shade(s.t.selection, -0.08))));
    }

    // The cap. r37:finish — selected, the same cap tinted by the selection (it read as
    // another component in the selection's full colour).
    let base = if st.lit {
        mix(s.t.cap, s.t.selection, 0.35)
    } else {
        s.t.cap
    };
    let base = if st.active {
        shade(base, -0.03)
    } else if st.hovered {
        shade(base, 0.03)
    } else {
        base
    };
    // FL's cap: lit above its groove, hollow below; the theme's depth scales the relief.
    let d = s.t.depth;
    let stops = [
        (0.0, f(shade(base, -0.10 * d))),
        (0.43, f(shade(base, 0.14 * d))),
        (0.48, f(shade(base, 0.08 * d))),
        (0.52, f(shade(base, -0.30 * d))), // r37:finish (was −0.42: muddy)
        (1.0, f(shade(base, -0.08 * d))),
    ];
    let c = Raised::at(base, d);
    // r37:finish — a tight shadow under the cap (a hard edge a pixel and a half down), not
    // the kit's soft drop, which read as a blurred halo round it.
    if !st.active && d > 0.0 {
        let off = (1.5 * s.k).max(b.px());
        b.rrect_soft(
            b.snap_rect(cap).translate(Vec2::new(0.0, off)),
            s.r(2.0),
            Fill::Solid(f(c.drop)),
            s.k,
        );
    }
    light::raised_face(
        b,
        cap,
        s.r(2.0),
        &c,
        Fill::Stops(&stops),
        RaisedOpts {
            groove: Some(0.5),
            drop: false, // r37:finish (drawn above, tight)
            k: s.k,
            alpha: a,
            depth: d,
        },
    );
    if st.focused {
        super::focus_ring(b, cap, s.r(2.0), s);
    }
}

/// The metal fader (round 37): a slot cut into the strip, a cap with a drop shadow, a face
/// lit above and turned from the light below an engraved index line, the specular across
/// its top.
fn paint_metal(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &FaderLook<'_>) {
    let toks = if s.t.dark {
        METAL_TOKENS
    } else {
        METAL_LIGHT_TOKENS
    };
    b.register(rect, "fader", toks, &[]);
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let m = Material::of(&s.t);
    let cap = cap_rect(rect, s, look.pos);
    let cx = b.snap(rect.center().x);
    let gw = s.s(4.0);
    let top = rect.top() + CAP.y * s.k * 0.5;
    let bottom = rect.bottom() - CAP.y * s.k * 0.5;
    let groove = Rect::from_min_max(
        Pos2::new(cx - gw * 0.5, top - gw),
        Pos2::new(cx + gw * 0.5, bottom + gw),
    );

    // Ticks either side of the slot.
    let tick = with_alpha(if s.t.dark { s.t.text } else { s.t.text_dim }, 0.30);
    for &t in look.ticks {
        let y = bottom - (bottom - top) * t;
        let long = look.unity.is_some_and(|u| (u - t).abs() < 1e-3);
        let len = s.s(if long { 5.0 } else { 3.0 });
        let y = b.snap_mid(y);
        b.hline(
            groove.left() - s.s(2.0) - len,
            groove.left() - s.s(2.0),
            y,
            b.px(),
            f(tick),
        );
        b.hline(
            groove.right() + s.s(2.0),
            groove.right() + s.s(2.0) + len,
            y,
            b.px(),
            f(tick),
        );
    }

    // The slot: cut into the strip.
    let groove_base = shade(s.t.well, if s.t.dark { -0.10 } else { -0.18 });
    light::recess(b, groove, gw * 0.5, groove_base, &m, a, s.k);
    if st.lit {
        // The slot below the cap lights up in the selection colour.
        let lit =
            Rect::from_min_max(Pos2::new(groove.left(), cap.center().y), groove.max).shrink(b.px());
        b.rrect(lit, gw * 0.5, Fill::Solid(f(shade(s.t.selection, -0.08))));
    }
    paint_cap(b, cap, s, st, &m);
    if st.focused {
        super::focus_ring(b, cap, s.r(2.0), s);
    }
}

/// The cap as an object, in `cap`.
fn paint_cap(b: &mut Brush, cap: Rect, s: &Style, st: State, m: &Material) {
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let d = m.depth;
    let k = s.k;
    let base = if st.lit {
        mix(s.t.cap, s.t.selection, 0.22)
    } else {
        s.t.cap
    };
    let base = if st.active {
        shade(base, -0.03)
    } else if st.hovered {
        shade(base, 0.03)
    } else {
        base
    };
    let cap = b.snap_rect(cap);
    let px = b.px();
    let r = s.r(2.0);
    // Its shadow on the strip: below it, a little wider, soft. Held, it sits down.
    if !st.active && m.drop.a() > 0 {
        // r37:finish — judge 8: a soft shadow under it, two points down, its edge two
        // points soft (the cap stands off the strip).
        let off = 2.0 * k;
        b.rrect_soft(
            cap.translate(Vec2::new(0.0, off)),
            r,
            Fill::Solid(f(m.drop)),
            2.0 * k,
        );
    }
    // The silhouette, then the face: a chamfer of light at the top, the upper half lit,
    // the index, the lower half turned from the light, a bounce of light at the foot.
    let rim = if s.t.dark {
        0.30 + 0.14 * d
    } else {
        0.16 + 0.10 * d
    };
    b.rrect(cap, r, Fill::Solid(f(shade(base, -rim))));
    let inner = cap.shrink(px);
    let ri = (r - px).max(0.0);
    // Metal on a glossy theme (a strong specular), plastic on a matte one: the same
    // stops, the specular's strength decides.
    let gloss = (m.specular.a() as f32 / 255.0).clamp(0.3, 1.0);
    // r37:finish — one soft fall of light from top to foot (judge 8: the lit upper half
    // over a dark lower half read as an awkward grey band); the grip line says the rest.
    let stops = [
        (0.0, f(shade(base, (0.12 + 0.06 * gloss) * d))),
        (0.25, f(shade(base, 0.05 * d))),
        (0.75, f(shade(base, -0.06 * d))),
        (1.0, f(shade(base, -(0.12 + 0.04 * gloss) * d))),
    ];
    b.rrect(inner, ri, Fill::Stops(&stops));
    // The sides turn from the light: a cylinder, not a slab.
    let side = (1.5 * k).max(px);
    for x in [inner.left(), inner.right() - side] {
        b.rect(
            Rect::from_min_max(
                Pos2::new(x, inner.top() + ri),
                Pos2::new(x + side, inner.bottom() - ri),
            ),
            Fill::Solid(f(with_alpha(m.bevel_shade, 0.30))),
        );
    }
    // The specular across the top: the metal's shine (faint on a matte theme).
    let spec = Rect::from_min_max(
        Pos2::new(inner.left() + side, inner.top() + px),
        Pos2::new(inner.right() - side, inner.top() + inner.height() * 0.30),
    );
    b.rect(
        spec,
        Fill::V(
            f(m.specular.gamma_multiply(0.55)),
            egui::Color32::TRANSPARENT,
        ),
    );
    b.hline(
        inner.left() + ri,
        inner.right() - ri,
        inner.top() + px * 0.5,
        px,
        f(m.bevel),
    );
    // The index line across the middle: where the level is read.
    let line = (1.5 * k).max(px);
    let y = b.snap(inner.center().y - line * 0.5);
    let index = if st.lit {
        s.t.selection
    } else {
        shade(base, -0.55 * d.max(0.6))
    };
    b.rect(
        Rect::from_min_max(
            Pos2::new(inner.left() + px, y),
            Pos2::new(inner.right() - px, y + b.snap(line).max(px)),
        ),
        Fill::Solid(f(index)),
    );
    b.hline(
        inner.left() + px,
        inner.right() - px,
        y + b.snap(line).max(px) + px * 0.5,
        px,
        f(m.bevel.gamma_multiply(0.9)),
    );
}
