//! # The component kit (ADR 0024 §4)
//!
//! Everything Cantus shows is drawn by the painters in this module, in vectors, with one
//! lighting recipe. egui stays underneath as input, layout and text shaping — the layer
//! under the look, not the look.
//!
//! ## How it is built
//!
//! - **Pure painters.** Every part is a function `paint(brush, rect, &Style, State, …)`:
//!   given a rect, a state and the tokens it draws the same thing, touching no input.
//!   Every state — rest, hover, active, disabled, focus, lit — goes through the same
//!   function, so the gallery (`tests/kit_gallery.rs`) can draw them all, and the
//!   exported plug-in windows can reuse them.
//! - **Thin widgets** in [`widgets`] allocate a rect, sense input, turn it into a
//!   [`State`], write a [`hint`], and call the painter.
//! - **One brush.** Painters write into a [`Brush`]: one mesh per flush, on one texture
//!   (the layer atlas of [`cache`]), with text deferred after it. A widget flushes its
//!   own brush; a view that paints many parts can own one brush and flush once — two
//!   draw calls for the view.
//! - **Static richness is cached** ([`cache`]): knob bodies, glows and logos are
//!   rasterised once per pixel size and theme, drawn as one textured quad; only what
//!   moves (values, meters, curves) is tessellated each frame. All the meters of a view
//!   go into one batched mesh ([`meter::MeterBatch`]).
//!
//! ## Conventions
//!
//! - **Sizes are logical points at 100 %**, multiplied by the style's scale `k` (a
//!   device window's own zoom, ADR 0024 §8); egui's `pixels_per_point` multiplies on top.
//! - **Hairlines are one physical pixel** and edges sit on the pixel grid: painters snap
//!   their rects with [`Brush::snap_rect`] and draw lines with [`Brush::hline`].
//! - **The lighting recipe** ([`light`]) is the only way anything gets depth: light
//!   from above; raised = outline −20 % L, face +6 % → −6 % L, top edge +15 % L, a 2-px
//!   drop; sunken = darker toward the bottom, border −8 % L, inner top line −12 % L. A
//!   lit part is the same recipe on the role colour.
//! - **Every word is in a box** ([`text`]), measured before it is painted, with a policy
//!   for what does not fit.
//! - **Abbreviations**: [`text::Policy::Abbreviate`] looks up `<label_key>.short` in any
//!   locale file (`fx.eq.freq.short = FREQ`) and falls back to the full label. Generic
//!   short forms live in `locales/areas/kit.{en,es}.txt`; each device's own in its locale
//!   file.
//! - **Colour**: tokens only ([`tokens`]). A function hue ([`Function`]) marks arcs, LEDs,
//!   curves and a selected segment's underline — never a surface bigger than a chip.
//! - **Hints**: every widget, when hovered or dragged, writes a [`hint::Hint`] (`name`,
//!   `value`, `caps`) into the context, where the hint bar and a device's help line read
//!   it.
//!
//! ## Configurable to the last colour
//!
//! - **Every painted colour is a named token** ([`Tok`]): a stable key
//!   (`mixer.fader_cap`), a human name and a [`Group`] in both languages
//!   (`kit.token.<key>`, `kit.group.<group>`), searchable with [`tokens::search`]. No
//!   painter spells a colour out (`tests/kit_lint.rs`); shades of a token are the
//!   lighting recipe's, scaled by the theme's depth.
//! - **Inspect mode** ([`inspect`]): every painter registers its rect and the tokens it
//!   paints with ([`Brush::register`]); [`inspect::hit`] returns the topmost element at a
//!   point, and [`inspect::paint_pick`] shows it. `tests/kit_tokens.rs` proves each part
//!   registers exactly the tokens its pixels depend on, in Slate and in Paper.
//! - **The form** ([`Form`]): depth (flat … FL's relief), roundness, density, text size,
//!   applied through [`Style::r`], [`Style::d`] and [`Style::tk`] and the recipe in
//!   [`light`]. With [`ThemeKnobs`] (hue, saturation, brightness, contrast, text) and the
//!   logo choice, all of it lives in the theme's JSON.
//! - **Light and dark** ([`appearance`]): Dark / Light / Follow the system; a base theme
//!   per mode (any preset or user theme); overrides per token kept per mode; reset per
//!   token, per group, or all; A/B compare; save, duplicate, delete, share and import;
//!   old `theme.json` files load as a user theme.
//!
//! ## For the builders who migrate the views (ADR 0024 Phase B)
//!
//! - **Style**: `Style::current()` (the installed tokens, 100 %), then `.function(f)`
//!   for a section's hue and `.scaled(k)` inside a device window. Tokens are installed by
//!   `theme::apply`; `Theme::kit_tokens()` gives them for any theme.
//! - **Widgets** ([`widgets`]) for interactive controls: `Param` (a knob, or a `Cell`
//!   with `.cell(label)`), `fader`, `led_button`, `segmented`, `menu_field`,
//!   `toolbar_key`, `tile`, `tab`, `splitter`, `scrollbar`, `meter`, `menu_list`,
//!   `readout`. Each writes its hint.
//! - **Painters** for everything static or batched: call them inside
//!   `with_brush(painter, |b| …)`, or own a [`Brush`] per view (`begin(ctx)`, paint,
//!   `flush(painter)`) so the whole view is one mesh plus its text. Paint the meters of a
//!   view through one [`meter::MeterBatch`].
//! - **Text**: `text::paint(b, rect, &Text::key("…", Role::Label, colour), k)`; pick a
//!   [`text::Policy`] — `Elide::Middle` for names, `Rotate90` for narrow strips,
//!   `Abbreviate` for control labels (give the i18n key and add `<key>.short`).
//! - **Device windows**: [`shell::Shell`]`::new(id).show(ui, &header, &footer, |ui,
//!   style, body| …)`; lay cards out with [`shell::Grid`] and [`band::card`]; the
//!   instrument bands are [`shell::layout_bands`], [`shell::macro_card`],
//!   [`shell::mod_card`], [`shell::keyboard_card`]. Act on the returned events.
//! - **Logos**: [`logo::paint`] (worker-rendered, cached); call
//!   [`logo::set_disk_cache`] once with the app's cache directory.
//! - **Looks are reviewed**: a painter change moves its golden images
//!   (`CANTUS_BLESS=1 cargo test -p cantus-ui --test kit_gallery`), and
//!   `cargo run -p cantus-ui --example proof` renders the mock screens.

pub mod appearance;
pub mod automate;
pub mod brush;
pub mod cache;
pub mod color;
pub mod glyph;
pub mod hint;
pub mod inspect;
pub mod light;
pub mod text;
pub mod tokens;

pub mod band;
pub mod cap;
pub mod cell;
pub mod chrome;
pub mod fader;
pub mod knob;
pub mod led;
pub mod logo;
pub mod logo_small;
pub mod menu;
pub mod meter;
pub mod segmented;
pub mod shell;
pub mod tile;
pub mod well;
pub mod widgets;

pub mod gallery;

pub use brush::{Brush, Fill};
pub use tokens::{Density, Form, Function, Group, ThemeKnobs, Tok, Tokens};

use egui::{Color32, Painter, Response};
use std::cell::RefCell;

/// What a painter needs besides its rect and state: the tokens, the function hue of the
/// section it sits in (and the token it came from), and the per-window scale `k`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Style {
    pub t: Tokens,
    pub hue: Color32,
    /// The token `hue` resolves from; `None` for a colour that is user data (a track's).
    pub hue_tok: Option<Tok>,
    pub k: f32,
}

impl Style {
    pub fn new(t: Tokens) -> Style {
        Style {
            hue: t.hue_tone,
            hue_tok: Some(Tok::HueTone),
            t,
            k: 1.0,
        }
    }

    /// This style with its hue taken from a token (the brand gold on A|B, the highlight
    /// on the mixer's width modes).
    pub fn hue_of(mut self, tok: Tok) -> Style {
        self.hue = self.t.get(tok);
        self.hue_tok = Some(tok);
        self
    }

    /// A corner radius at this style's scale and the theme's roundness.
    #[inline]
    pub fn r(&self, v: f32) -> f32 {
        v * self.k * self.t.round
    }

    /// A spacing at this style's scale and the theme's density.
    #[inline]
    pub fn d(&self, v: f32) -> f32 {
        v * self.k * self.t.density
    }

    /// The scale text is set at: the window's, times the theme's text size.
    #[inline]
    pub fn tk(&self) -> f32 {
        self.k * self.t.text_scale
    }

    /// The installed tokens at 100 %.
    pub fn current() -> Style {
        Style::new(tokens::current())
    }

    /// This style in the hue of a kind of section.
    pub fn function(mut self, f: Function) -> Style {
        self.hue = self.t.hue(f);
        self.hue_tok = Some(f.tok());
        self
    }

    /// This style in a colour that is user data (a track's colour), not a theme token.
    pub fn with_hue(mut self, hue: Color32) -> Style {
        self.hue = hue;
        self.hue_tok = None;
        self
    }

    pub fn scaled(mut self, k: f32) -> Style {
        self.k = k;
        self
    }

    /// A logical size at this style's scale.
    #[inline]
    pub fn s(&self, v: f32) -> f32 {
        v * self.k
    }
}

/// How a part is drawn right now. Flags combine: a focused, hovered, lit button is all
/// three at once.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct State {
    pub hovered: bool,
    /// Pressed or being dragged.
    pub active: bool,
    pub disabled: bool,
    /// Holds keyboard focus.
    pub focused: bool,
    /// On: a toggle that is set, a selected segment, a lamp that is lit.
    pub lit: bool,
}

impl State {
    pub const REST: State = State {
        hovered: false,
        active: false,
        disabled: false,
        focused: false,
        lit: false,
    };
    pub const HOVER: State = State {
        hovered: true,
        ..State::REST
    };
    pub const ACTIVE: State = State {
        hovered: true,
        active: true,
        ..State::REST
    };
    pub const DISABLED: State = State {
        disabled: true,
        ..State::REST
    };
    pub const FOCUS: State = State {
        focused: true,
        ..State::REST
    };
    pub const LIT: State = State {
        lit: true,
        ..State::REST
    };

    /// Every state the gallery draws, with its name.
    pub const ALL: [(&'static str, State); 6] = [
        ("rest", State::REST),
        ("hover", State::HOVER),
        ("active", State::ACTIVE),
        ("disabled", State::DISABLED),
        ("focus", State::FOCUS),
        ("lit", State::LIT),
    ];

    /// The state a response describes; `lit` is the caller's.
    pub fn of(r: &Response, lit: bool) -> State {
        State {
            hovered: r.hovered() || r.dragged(),
            active: r.is_pointer_button_down_on() || r.dragged(),
            disabled: !r.enabled(),
            focused: r.has_focus(),
            lit,
        }
    }

    pub fn lit(mut self, on: bool) -> State {
        self.lit = on;
        self
    }

    /// Opacity multiplier: disabled parts are painted at 45 % (`plugin-instrument-uis.md`
    /// §7.7), everything else whole.
    pub fn alpha(&self) -> f32 {
        if self.disabled {
            0.45
        } else {
            1.0
        }
    }
}

thread_local! {
    static BRUSH: RefCell<Brush> = RefCell::new(Brush::default());
}

/// Paint through this thread's brush into `painter`, flushing at the end. Re-entrant: a
/// nested call gets a brush of its own.
pub fn with_brush<R>(painter: &Painter, f: impl FnOnce(&mut Brush) -> R) -> R {
    BRUSH.with(|cell| match cell.try_borrow_mut() {
        Ok(mut b) => {
            b.begin(painter.ctx());
            let r = f(&mut b);
            b.flush(painter);
            r
        }
        Err(_) => {
            let mut b = Brush::default();
            b.begin(painter.ctx());
            let r = f(&mut b);
            b.flush(painter);
            r
        }
    })
}

/// The focus ring every focusable part wears when it holds keyboard focus: 2 points
/// outside its rect, in the focus colour.
pub fn focus_ring(b: &mut Brush, rect: egui::Rect, radius: f32, s: &Style) {
    let off = s.s(2.0);
    let radius = radius.max(0.0);
    b.rrect_stroke(rect.expand(off), radius + off, s.s(1.5), s.t.focus);
}
