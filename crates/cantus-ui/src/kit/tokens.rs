//! The kit's tokens (ADR 0024 §2): FL's hue-locked neutral ladder, a few light accents
//! used only on small things, six function hues, and the controls' own colours.
//!
//! Tokens are **data**. A [`Theme`](crate::theme::Theme) carries them in its `kit`
//! object (hex strings, each optional); a theme file written before the kit existed
//! derives them from its older colours ([`Tokens::derive_from_palette`]). The FL-style
//! [`ThemeKnobs`] then shift the whole ladder at once, and a contrast floor keeps every
//! text colour at 4.5:1 whatever the knobs say.
//!
//! Four presets ship: **Slate** (dark, FL-derived, the default-to-be), **Paper** (light,
//! rebuilt on the same rules and hue), and **Parchment** and **Ink & Gold** re-expressed.

use super::color::{contrast, ensure_contrast, ensure_contrast_all, Hsl};
use egui::Color32;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// A kind of section, and the hue that marks it (`plugin-instrument-uis.md` §7.2). A
/// function hue colours arcs, LEDs, curves and a selected segment's underline — never a
/// surface bigger than a chip.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Function {
    /// Oscillators, generators, samples (blue).
    Source,
    /// Filters, EQ (teal).
    #[default]
    Tone,
    /// Envelopes of level, compressors, gates, drive (amber).
    Dynamics,
    /// LFOs, modulation, macros (violet).
    Mod,
    /// Delay, reverb, chorus, width (rose).
    Space,
    /// Voice, global, output, routing (olive).
    Output,
}

impl Function {
    /// The token of this kind's hue.
    pub fn tok(self) -> Tok {
        match self {
            Function::Source => Tok::HueSource,
            Function::Tone => Tok::HueTone,
            Function::Dynamics => Tok::HueDynamics,
            Function::Mod => Tok::HueMod,
            Function::Space => Tok::HueSpace,
            Function::Output => Tok::HueOutput,
        }
    }

    pub const ALL: [Function; 6] = [
        Function::Source,
        Function::Tone,
        Function::Dynamics,
        Function::Mod,
        Function::Space,
        Function::Output,
    ];
}

/// FL's theme knobs (`fl-studio.md` §12.9). All zero is the theme exactly as written.
#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeKnobs {
    /// Rotates every ladder colour's hue, in degrees (−180…180).
    pub hue: f32,
    /// Scales the ladder's saturation: −1 (grey) … 0 … +1 (double).
    pub saturation: f32,
    /// Shifts the ladder's lightness: −1 … +1 is −20 % … +20 % L.
    pub brightness: f32,
    /// Scales the lightness distance between ladder steps around `panel`: −1 (flat) … +1
    /// (double).
    pub contrast: f32,
    /// Shifts the text colours' lightness: −1 … +1 is −20 % … +20 % L.
    pub text: f32,
}

impl ThemeKnobs {
    pub fn is_neutral(&self) -> bool {
        *self == ThemeKnobs::default()
    }

    /// A ladder colour moved by the hue, saturation, brightness and contrast knobs.
    /// `mid` is the lightness contrast pivots around.
    pub fn surface(&self, c: Color32, mid: f32) -> Color32 {
        if self.is_neutral() {
            return c;
        }
        let mut h = Hsl::of(c);
        h.h += self.hue;
        h.s = (h.s * (1.0 + self.saturation.clamp(-1.0, 1.0))).clamp(0.0, 1.0);
        h.l = mid + (h.l - mid) * (1.0 + self.contrast.clamp(-1.0, 1.0));
        h.l = (h.l + 0.2 * self.brightness.clamp(-1.0, 1.0)).clamp(0.0, 1.0);
        h.rgb()
    }

    /// A text colour moved by the hue and text knobs (text keeps its own lightness
    /// against the brightness knob: the floor below restores contrast either way).
    pub fn text(&self, c: Color32) -> Color32 {
        if self.is_neutral() {
            return c;
        }
        let mut h = Hsl::of(c);
        h.h += self.hue;
        h.l = (h.l + 0.2 * self.text.clamp(-1.0, 1.0)).clamp(0.0, 1.0);
        h.rgb()
    }
}

/// How much room things get.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Density {
    Compact,
    #[default]
    Normal,
    Comfortable,
}

impl Density {
    /// The spacing multiplier.
    pub fn factor(self) -> f32 {
        match self {
            Density::Compact => 0.85,
            Density::Normal => 1.0,
            Density::Comfortable => 1.15,
        }
    }
}

/// A theme's form, beside its colours: how deep the relief, how round the corners, how
/// dense the spacing, how large the text. All one in a theme as shipped.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Form {
    /// How much of the lighting recipe: 0 flat … 1 FL's relief … 1.5 deeper.
    pub depth: f32,
    /// Corner roundness: 0 square … 1 as designed … 2.5 soft.
    pub roundness: f32,
    pub density: Density,
    /// Text size: 0.8 … 1.4.
    pub text_size: f32,
}

impl Default for Form {
    fn default() -> Self {
        Form {
            depth: 1.0,
            roundness: 1.0,
            density: Density::Normal,
            text_size: 1.0,
        }
    }
}

impl Form {
    pub fn is_default(&self) -> bool {
        *self == Form::default()
    }

    /// Put this form into resolved tokens.
    pub fn apply(&self, t: &mut Tokens) {
        t.depth = self.depth.clamp(0.0, 1.5);
        t.round = self.roundness.clamp(0.0, 2.5);
        t.radius *= t.round;
        t.density = self.density.factor();
        t.text_scale = self.text_size.clamp(0.8, 1.4);
    }
}

/// Bumped every time tokens are installed; caches key on it (§13).
static GENERATION: AtomicU64 = AtomicU64::new(1);

/// The current theme generation. A cached texture or galley made under an older one is
/// stale.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// The groups a theme editor lists the tokens under.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    Surfaces,
    Lines,
    Text,
    Accents,
    Meters,
    Mixer,
    Knobs,
    Devices,
    Steps,
    Keys,
    /// The editing views' grounds and playhead.
    Editors,
    Menus,
    Hues,
    Depth,
}

impl Group {
    pub const ALL: [Group; 14] = [
        Group::Surfaces,
        Group::Lines,
        Group::Text,
        Group::Accents,
        Group::Meters,
        Group::Mixer,
        Group::Knobs,
        Group::Devices,
        Group::Steps,
        Group::Keys,
        Group::Editors,
        Group::Menus,
        Group::Hues,
        Group::Depth,
    ];

    /// The group's stable key (the first part of its tokens' keys).
    pub fn key(self) -> &'static str {
        match self {
            Group::Surfaces => "surface",
            Group::Lines => "lines",
            Group::Text => "text",
            Group::Accents => "accent",
            Group::Meters => "meters",
            Group::Mixer => "mixer",
            Group::Knobs => "knob",
            Group::Devices => "device",
            Group::Steps => "steps",
            Group::Keys => "keys",
            Group::Editors => "editor",
            Group::Menus => "menu",
            Group::Hues => "hue",
            Group::Depth => "depth",
        }
    }

    /// The i18n key of its human name.
    pub fn name_key(self) -> String {
        format!("kit.group.{}", self.key())
    }

    /// Its tokens, in editor order.
    pub fn tokens(self) -> impl Iterator<Item = Tok> {
        Tok::ALL.iter().copied().filter(move |t| t.group() == self)
    }
}

macro_rules! kit_tokens {
    ($( $group:ident => { $( $(#[doc = $doc:literal])* $variant:ident $name:ident $key:literal ),* $(,)? } )*) => {
        /// Every colour the kit paints with, and the finish every part is drawn to.
        /// `Copy`: take one per frame, pass by reference to the painters.
        #[derive(Copy, Clone, Debug, PartialEq)]
        pub struct Tokens {
            $( $( $(#[doc = $doc])* pub $name: Color32, )* )*
            /// A dark theme: raised parts get light edges, displays stay dark.
            pub dark: bool,
            /// Corner radius of caps and tiles, in points (roundness included).
            pub radius: f32,
            /// How round every other corner is: 0 square … 1 as designed … 2 soft.
            pub round: f32,
            /// How much of the lighting recipe to apply: 0 flat … 1 FL's relief.
            pub depth: f32,
            /// Spacing: 0.85 compact, 1 normal, 1.15 comfortable.
            pub density: f32,
            /// Every text role's size, multiplied.
            pub text_scale: f32,
            /// The theme generation these tokens were installed under.
            pub generation: u64,
        }

        /// A named token: every colour the kit paints resolves from one of these, so
        /// a theme editor can list, search, pick and recolour each.
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Tok { $( $( $(#[doc = $doc])* $variant, )* )* }

        impl Tok {
            /// Every token, in editor order.
            pub const ALL: &'static [Tok] = &[ $( $( Tok::$variant, )* )* ];

            /// The stable key, `group.name`: what theme files and overrides store.
            pub fn key(self) -> &'static str {
                match self { $( $( Tok::$variant => $key, )* )* }
            }

            pub fn group(self) -> Group {
                match self { $( $( Tok::$variant => Group::$group, )* )* }
            }

            /// The token of a stable key.
            pub fn from_key(key: &str) -> Option<Tok> {
                match key { $( $( $key => Some(Tok::$variant), )* )* _ => None }
            }

            /// The field in [`Tokens`] (for code search).
            pub fn field(self) -> &'static str {
                match self { $( $( Tok::$variant => stringify!($name), )* )* }
            }
        }

        impl Tokens {
            /// The colour of a token.
            pub fn get(&self, t: Tok) -> Color32 {
                match t { $( $( Tok::$variant => self.$name, )* )* }
            }

            /// Set the colour of a token.
            pub fn set(&mut self, t: Tok, c: Color32) {
                match t { $( $( Tok::$variant => self.$name = c, )* )* }
            }
        }
    };
}

kit_tokens! {
    Surfaces => {
        /// Deepest ground: lists, browser, displays.
        Ink ink "surface.ink",
        /// Sunken fields: dropdowns, readouts, number boxes.
        Well well "surface.well",
        /// Window chrome, title rows, the desk.
        Panel panel "surface.panel",
        /// The top toolbar.
        Toolbar toolbar "surface.toolbar",
        /// Tiles: FX slots, list rows.
        PanelRaised panel_raised "surface.tile",
        /// Surfaces behind tiles (inspector). No body text directly on it.
        PanelLight panel_light "surface.behind_tiles",
    }
    Lines => {
        Grid grid "lines.grid",
        GridStrong grid_strong "lines.beat",
        GridBar grid_bar "lines.bar",
        /// Outlines that bound a control: display and field edges, dividers.
        Edge edge "lines.control_edge",
    }
    Text => {
        Text text "text.primary",
        /// Secondary text; 4.5:1 where it is read.
        TextDim text_dim "text.secondary",
        /// Control and section labels.
        Label label "text.label",
        /// "Slot 5", "Insert 12": placeholders only, deliberately faint.
        Placeholder placeholder "text.placeholder",
    }
    Accents => {
        /// Cantus's own colour: the logo, the modified-preset dot, the splash.
        Brand brand "accent.brand",
        BrandBright brand_bright "accent.brand_bright",
        /// Active toolbar keys and toggles (FL's orange).
        Highlight highlight "accent.highlight",
        /// The pale band through the middle of a highlighted key.
        HighlightBand highlight_band "accent.highlight_band",
        /// The glyph on a highlighted key.
        HighlightGlyph highlight_glyph "accent.highlight_glyph",
        /// Selected strips, faders, channels.
        Selection selection "accent.selection",
        /// The keyboard focus ring.
        Focus focus "accent.focus",
        /// Mute / on LEDs.
        Led led "accent.led_on",
        /// Record, clip, destructive.
        Danger danger "accent.record",
        /// The FX-active bulb.
        FxOn fx_on "accent.fx_bulb",
    }
    Meters => {
        /// Behind the meters.
        MeterWell meter_well "meters.well",
        MeterLow meter_low "meters.low",
        /// From −6 dB.
        MeterHigh meter_high "meters.high",
        /// From 0 dB.
        MeterClip meter_clip "meters.clip",
    }
    Mixer => {
        /// Mixer strip bodies.
        Strip strip "mixer.strip",
        /// The selected strip's body.
        StripSelected strip_selected "mixer.strip_selected",
        /// The neutral track-number cell.
        HeaderCell header_cell "mixer.number_cell",
        /// A fader cap at rest (the lightest object on a strip).
        Cap cap "mixer.fader_cap",
    }
    Knobs => {
        KnobBody knob_body "knob.body",
        /// The 1-px rim around a dark knob body.
        KnobRim knob_rim "knob.rim",
        /// The 270° ring a value arc runs in.
        KnobTrack knob_track "knob.track",
        KnobPointer knob_pointer "knob.pointer",
    }
    Devices => {
        /// A device's ground, showing in the gutters between cards.
        DeviceGround device_ground "device.ground",
        /// A card's face.
        Deck deck "device.card",
        /// A card's title band.
        DeckTitle deck_title "device.card_title",
        /// A value pill's fill (FL LuxeVerb) and its text.
        ValuePill value_pill "device.value_pill",
        ValuePillText value_pill_text "device.value_pill_text",
    }
    Steps => {
        /// Step keys, odd and even groups of four, at rest and lit.
        StepOdd step_odd "steps.odd",
        StepEven step_even "steps.even",
        StepOddLit step_odd_lit "steps.odd_lit",
        StepEvenLit step_even_lit "steps.even_lit",
    }
    Keys => {
        /// A keyboard's white and black keys.
        KeyWhite key_white "keys.white",
        KeyBlack key_black "keys.black",
    }
    Editors => {
        /// The piano roll's rows of white keys; alternate bars in the arrangement.
        RowWhite row_white "editor.row_white",
        /// The piano roll's rows of black keys, and the arrangement's and rack's ground.
        RowBlack row_black "editor.row_black",
        /// The playhead's line and its marker in the rulers.
        Playhead playhead "editor.playhead",
    }
    Menus => {
        Menu menu "menu.face",
        MenuBand menu_band "menu.band",
        MenuHover menu_hover "menu.hover",
        MenuText menu_text "menu.text",
        /// Text on a hovered row.
        MenuTextHover menu_text_hover "menu.text_hover",
    }
    Hues => {
        HueSource hue_source "hue.source",
        HueTone hue_tone "hue.tone",
        HueDynamics hue_dynamics "hue.dynamics",
        HueMod hue_mod "hue.modulation",
        HueSpace hue_space "hue.space",
        HueOutput hue_output "hue.output",
    }
    Depth => {
        /// Drop shadows under floating surfaces.
        Shadow shadow "depth.shadow",
        /// The light that falls on raised parts: top edges, speculars, gloss.
        Light light "depth.light",
        /// The shade under parts and inside bevels.
        Dark dark_shade "depth.dark",
    }
}

impl Tok {
    /// The i18n key of its human name.
    pub fn name_key(self) -> String {
        format!("kit.token.{}", self.key())
    }
}

/// Tokens by stable key, as hex (`#rrggbb`, or `#rrggbbaa`): a theme's kit colours,
/// or a user's overrides of them. Keys a build does not know are kept and ignored.
pub type KitMap = std::collections::BTreeMap<String, String>;

/// Every token of `t`, written out.
pub fn to_map(t: &Tokens) -> KitMap {
    Tok::ALL
        .iter()
        .map(|k| (k.key().to_string(), crate::theme::to_hex(t.get(*k))))
        .collect()
}

/// Apply the tokens `map` sets; unknown keys and unparseable colours are ignored.
pub fn apply_map(t: &mut Tokens, map: &KitMap) {
    for (k, v) in map {
        if let (Some(tok), Some(c)) = (Tok::from_key(k), crate::theme::parse_hex(v)) {
            t.set(tok, c);
        }
    }
}

/// The tokens whose key or human name (in the current language) contains `query`,
/// ignoring case — for the theme editor's search box.
pub fn search(query: &str) -> Vec<Tok> {
    let q = query.trim().to_lowercase();
    Tok::ALL
        .iter()
        .copied()
        .filter(|t| {
            q.is_empty()
                || t.key().to_lowercase().contains(&q)
                || crate::i18n::t(&t.name_key()).to_lowercase().contains(&q)
                || crate::i18n::t(&t.group().name_key())
                    .to_lowercase()
                    .contains(&q)
        })
        .collect()
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const fn hex(v: u32) -> Color32 {
    rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

impl Tokens {
    /// **Slate**: FL Studio's default theme, measured (`fl-studio.md` §1, §12.1). One hue
    /// (≈ 202°) for every neutral, lightness steps only; light saturated accents on small
    /// things. Device decks sit darker than the desk so the six function hues reach
    /// 4.5:1 without turning pastel.
    pub const fn slate() -> Tokens {
        Tokens {
            ink: hex(0x1f282d),
            well: hex(0x3b4145),
            meter_well: hex(0x3f494d),
            strip: hex(0x454f55),
            strip_selected: hex(0x4c575d),
            panel: hex(0x4e585e),
            toolbar: hex(0x596267),
            panel_raised: hex(0x606b71),
            panel_light: hex(0x727c81),
            header_cell: hex(0x768086),
            device_ground: hex(0x262f34),
            deck: hex(0x3d474d),
            deck_title: hex(0x333c42),
            grid: hex(0x394b56),
            grid_strong: hex(0x2e404b),
            grid_bar: hex(0x1a2c37),
            edge: hex(0x8b979e),
            text: hex(0xeef1ef),
            text_dim: hex(0xcdd3d9),
            label: hex(0xc4cbd2),
            placeholder: hex(0x9ca2a8),
            brand: hex(0xf2c14e),
            brand_bright: hex(0xffd97a),
            highlight: hex(0xffaa54),
            highlight_glyph: hex(0xb13f00),
            selection: hex(0xc3ff5a),
            focus: hex(0x3fc3ff),
            led: hex(0x8def11),
            danger: hex(0xff6135),
            fx_on: hex(0xffe34f),
            meter_low: hex(0xc4fc55),
            meter_high: hex(0xffdf5a),
            meter_clip: hex(0xff8d3e),
            cap: hex(0xc9ccd6),
            knob_body: hex(0x2a3237),
            knob_rim: hex(0x8b979e),
            knob_track: hex(0x8b979e),
            knob_pointer: hex(0xeef1ef),
            value_pill: hex(0xc9d6dc),
            value_pill_text: hex(0x1f282d),
            step_odd: hex(0x6d7478),
            step_even: hex(0x7f6c6d),
            step_odd_lit: hex(0xd1e3ee),
            step_even_lit: hex(0xfdccce),
            menu: hex(0xb8c3c9),
            menu_band: hex(0x8f9da5),
            menu_hover: hex(0x5a6c76),
            menu_text: hex(0x1f282d),
            menu_text_hover: hex(0xffffff),
            hue_source: hex(0x8fb8ff),
            hue_tone: hex(0x4dc7bd),
            hue_dynamics: hex(0xe0ae52),
            hue_mod: hex(0xc1acfc),
            hue_space: hex(0xf49aca),
            hue_output: hex(0x9ccf6a),
            shadow: Color32::from_rgba_premultiplied(0, 0, 0, 0x80),
            highlight_band: hex(0xffeb9d),
            key_white: hex(0xdadee6),
            key_black: hex(0x404143),
            row_white: hex(0x3e4f5a),
            row_black: hex(0x374954),
            playhead: hex(0x9be34f),
            light: hex(0xffffff),
            dark_shade: hex(0x000000),
            dark: true,
            radius: 2.0,
            round: 1.0,
            depth: 1.0,
            density: 1.0,
            text_scale: 1.0,
            generation: 0,
        }
    }

    /// **Paper**, rebuilt on Slate's rules (`fl-studio.md` §12.2): the ladder inverted,
    /// the same hue, one strong accent, meters kept dark inside so lime stays readable.
    pub const fn paper() -> Tokens {
        Tokens {
            ink: hex(0xf7f9fa),
            well: hex(0xcfd7dc),
            meter_well: hex(0x2f3a40),
            strip: hex(0xe4e9ec),
            strip_selected: hex(0xd8e0e5),
            panel: hex(0xdfe4e7),
            toolbar: hex(0xd5dbdf),
            panel_raised: hex(0xf8fafb),
            panel_light: hex(0xe9edf0),
            header_cell: hex(0xc9d1d6),
            device_ground: hex(0xd3dade),
            deck: hex(0xf5f7f8),
            deck_title: hex(0xe8ecef),
            grid: hex(0xdbe1e5),
            grid_strong: hex(0xc5ced4),
            grid_bar: hex(0x9fabb3),
            edge: hex(0x7f8a92),
            text: hex(0x172024),
            text_dim: hex(0x4d5a62),
            label: hex(0x3b474e),
            placeholder: hex(0x8a959c),
            brand: hex(0x956608),
            brand_bright: hex(0xb8801a),
            highlight: hex(0xc9650f),
            highlight_glyph: hex(0x4a1d00),
            selection: hex(0x4d8a00),
            focus: hex(0x0a74c2),
            led: hex(0x62c400),
            danger: hex(0xc4321a),
            fx_on: hex(0xf0c000),
            meter_low: hex(0xc4fc55),
            meter_high: hex(0xffdf5a),
            meter_clip: hex(0xff8d3e),
            cap: hex(0xeef0f3),
            knob_body: hex(0x333d44),
            knob_rim: hex(0x333d44),
            knob_track: hex(0x808b94),
            knob_pointer: hex(0xffffff),
            value_pill: hex(0xe3e8eb),
            value_pill_text: hex(0x172024),
            step_odd: hex(0xe3e7ea),
            step_even: hex(0xeddadb),
            step_odd_lit: hex(0x9fc6e0),
            step_even_lit: hex(0xf2a6ab),
            menu: hex(0xf8fafb),
            menu_band: hex(0xe3e8eb),
            menu_hover: hex(0x2f6fa8),
            menu_text: hex(0x172024),
            menu_text_hover: hex(0xffffff),
            hue_source: hex(0x2f62c8),
            hue_tone: hex(0x0d736d),
            hue_dynamics: hex(0x8f5c00),
            hue_mod: hex(0x7a4cc2),
            hue_space: hex(0xa94275),
            hue_output: hex(0x4f6b2c),
            shadow: Color32::from_rgba_premultiplied(0x1a, 0x1d, 0x23, 0x38),
            highlight_band: hex(0xf6c48a),
            key_white: hex(0xfbfcfd),
            key_black: hex(0x404143),
            row_white: hex(0xf4f6f7),
            row_black: hex(0xeaeef1),
            playhead: hex(0x3f8a00),
            light: hex(0xffffff),
            dark_shade: hex(0x000000),
            dark: false,
            radius: 2.0,
            round: 1.0,
            depth: 1.0,
            density: 1.0,
            text_scale: 1.0,
            generation: 0,
        }
    }

    /// **Ink & Gold**, the original dark theme, re-expressed in the kit's tokens.
    pub const fn ink_and_gold() -> Tokens {
        Tokens {
            ink: hex(0x101218),
            well: hex(0x0c0e13),
            meter_well: hex(0x0b0d12),
            strip: hex(0x1b1e27),
            strip_selected: hex(0x232835),
            panel: hex(0x171a22),
            toolbar: hex(0x1c2029),
            panel_raised: hex(0x1f232d),
            panel_light: hex(0x262b36),
            header_cell: hex(0x2f3542),
            device_ground: hex(0x0b0d12),
            deck: hex(0x1b1f28),
            deck_title: hex(0x232834),
            grid: hex(0x232733),
            grid_strong: hex(0x2f3442),
            grid_bar: hex(0x3b4152),
            edge: hex(0x6b7385),
            text: hex(0xe6e2d8),
            text_dim: hex(0x8f939e),
            label: hex(0xc9ccd3),
            placeholder: hex(0x5d626d),
            brand: hex(0xd4a24c),
            brand_bright: hex(0xf2c14e),
            highlight: hex(0xf2c14e),
            highlight_glyph: hex(0x3a2604),
            selection: hex(0x7fb2ff),
            focus: hex(0xffe4a8),
            led: hex(0x5cb86e),
            danger: hex(0xd95d5d),
            fx_on: hex(0xf2c14e),
            meter_low: hex(0x5cb86e),
            meter_high: hex(0xd4a24c),
            meter_clip: hex(0xd95d5d),
            cap: hex(0xb9bcc6),
            knob_body: hex(0x2c313c),
            knob_rim: hex(0x8f97a8),
            knob_track: hex(0x6b7385),
            knob_pointer: hex(0xffffff),
            value_pill: hex(0x232834),
            value_pill_text: hex(0xe6e2d8),
            step_odd: hex(0x2a2f3b),
            step_even: hex(0x3a2f33),
            step_odd_lit: hex(0xb9c7e8),
            step_even_lit: hex(0xe8b9bd),
            menu: hex(0x1f232d),
            menu_band: hex(0x2f3442),
            menu_hover: hex(0x3b4152),
            menu_text: hex(0xe6e2d8),
            menu_text_hover: hex(0xffffff),
            hue_source: hex(0x74a7ff),
            hue_tone: hex(0x3fc2b6),
            hue_dynamics: hex(0xe0ae52),
            hue_mod: hex(0xa98bfa),
            hue_space: hex(0xf07ab8),
            hue_output: hex(0x9ccf6a),
            shadow: Color32::from_rgba_premultiplied(0, 0, 0, 0x8c),
            highlight_band: hex(0xffe7a0),
            key_white: hex(0xd8d3c6),
            key_black: hex(0x14161c),
            row_white: hex(0x191c24),
            row_black: hex(0x13151b),
            playhead: hex(0xf2c14e),
            light: hex(0xffffff),
            dark_shade: hex(0x000000),
            dark: true,
            radius: 3.0,
            round: 1.0,
            depth: 1.0,
            density: 1.0,
            text_scale: 1.0,
            generation: 0,
        }
    }

    /// **Parchment**, the warm light theme, re-expressed in the kit's tokens.
    pub const fn parchment() -> Tokens {
        Tokens {
            ink: hex(0xf3efe6),
            well: hex(0xd9d2c2),
            meter_well: hex(0x2c2822),
            strip: hex(0xe4dece),
            strip_selected: hex(0xd9d2c1),
            panel: hex(0xe9e4d8),
            toolbar: hex(0xdfd9cb),
            panel_raised: hex(0xf3efe6),
            panel_light: hex(0xeee9df),
            header_cell: hex(0xcfc7b6),
            device_ground: hex(0xd9d2c3),
            deck: hex(0xf3efe6),
            deck_title: hex(0xe8e2d5),
            grid: hex(0xd6d0c2),
            grid_strong: hex(0xc4bdad),
            grid_bar: hex(0xa9a190),
            edge: hex(0x8a8170),
            text: hex(0x1e1b16),
            text_dim: hex(0x666158),
            label: hex(0x3f3a31),
            placeholder: hex(0x9a9384),
            brand: hex(0x875c08),
            brand_bright: hex(0xad7810),
            highlight: hex(0xb2560c),
            highlight_glyph: hex(0x3a1a00),
            selection: hex(0x2f6fd6),
            focus: hex(0x6b4506),
            led: hex(0x4fae2a),
            danger: hex(0xb03333),
            fx_on: hex(0xe0a810),
            meter_low: hex(0xc4fc55),
            meter_high: hex(0xffdf5a),
            meter_clip: hex(0xff8d3e),
            cap: hex(0xf6f2ea),
            knob_body: hex(0x3a342b),
            knob_rim: hex(0x3a342b),
            knob_track: hex(0x8f8674),
            knob_pointer: hex(0xfbf8f0),
            value_pill: hex(0xe3dccd),
            value_pill_text: hex(0x1e1b16),
            step_odd: hex(0xe2dccf),
            step_even: hex(0xead3cc),
            step_odd_lit: hex(0xa9c3dd),
            step_even_lit: hex(0xe9a9a0),
            menu: hex(0xfbf8f0),
            menu_band: hex(0xe8e2d5),
            menu_hover: hex(0x7a561a),
            menu_text: hex(0x1e1b16),
            menu_text_hover: hex(0xffffff),
            hue_source: hex(0x2c5bb8),
            hue_tone: hex(0x0e7069),
            hue_dynamics: hex(0x875c08),
            hue_mod: hex(0x7045b3),
            hue_space: hex(0xa13d6e),
            hue_output: hex(0x4a6428),
            shadow: Color32::from_rgba_premultiplied(0x1a, 0x15, 0x0e, 0x40),
            highlight_band: hex(0xeab27a),
            key_white: hex(0xfbf8f0),
            key_black: hex(0x2a2722),
            row_white: hex(0xf0ebe0),
            row_black: hex(0xe6e0d2),
            playhead: hex(0xad7810),
            light: hex(0xffffff),
            dark_shade: hex(0x000000),
            dark: false,
            radius: 3.0,
            round: 1.0,
            depth: 1.0,
            density: 1.0,
            text_scale: 1.0,
            generation: 0,
        }
    }

    /// The hue of a kind of section.
    pub fn hue(&self, f: Function) -> Color32 {
        match f {
            Function::Source => self.hue_source,
            Function::Tone => self.hue_tone,
            Function::Dynamics => self.hue_dynamics,
            Function::Mod => self.hue_mod,
            Function::Space => self.hue_space,
            Function::Output => self.hue_output,
        }
    }
}

use super::color::{mix, on_fill, shade};
use crate::theme::{Palette, Theme};

impl Tokens {
    /// Tokens for a theme file that predates the kit: every kit colour derived from the
    /// colours it does have, by the ladder's rules (sunken = darker, raised = lighter,
    /// one hue), then held to the contrast floor.
    pub fn derive_from_palette(p: &Palette) -> Tokens {
        let dark = p.dark;
        let up = |c, d: f32| shade(c, if dark { d } else { -d });
        let base = if dark {
            Tokens::ink_and_gold()
        } else {
            Tokens::paper()
        };
        let deck = if dark {
            shade(p.panel, 0.02)
        } else {
            p.panel_raised
        };
        let deck_title = shade(deck, -0.03);
        let menu = p.panel_raised;
        let menu_hover = mix(p.selection, menu, 0.25);
        let mut t = Tokens {
            ink: p.ink,
            well: shade(p.panel, if dark { -0.05 } else { -0.08 }),
            meter_well: if dark {
                shade(p.ink, -0.02)
            } else {
                base.meter_well
            },
            strip: up(p.panel, -0.03),
            strip_selected: up(p.panel, 0.01),
            panel: p.panel,
            toolbar: up(p.panel, 0.03),
            panel_raised: p.panel_raised,
            panel_light: up(p.panel_raised, 0.04),
            header_cell: up(p.panel_raised, 0.08),
            device_ground: if dark {
                shade(p.ink, -0.01)
            } else {
                shade(p.panel, -0.05)
            },
            deck,
            deck_title,
            grid: p.grid,
            grid_strong: p.grid_strong,
            grid_bar: p.grid_bar,
            edge: ensure_contrast(p.grid_bar, deck, 3.0),
            text: p.text,
            text_dim: p.text_dim,
            label: mix(p.text, p.text_dim, 0.5),
            placeholder: mix(p.text_dim, p.panel_raised, 0.35),
            brand: p.gold,
            brand_bright: p.gold_bright,
            highlight: p.gold_bright,
            highlight_glyph: on_fill(p.gold_bright),
            selection: p.selection,
            focus: p.focus,
            led: p.ok,
            danger: p.danger,
            fx_on: p.gold_bright,
            meter_low: p.meter_low,
            meter_high: p.meter_high,
            meter_clip: p.meter_clip,
            cap: if dark {
                mix(p.text, p.panel_raised, 0.2)
            } else {
                p.panel_raised
            },
            knob_body: if dark {
                shade(p.ink, 0.06)
            } else {
                mix(p.text, p.panel, 0.2)
            },
            knob_rim: if dark {
                ensure_contrast(shade(p.ink, 0.3), deck, 3.0)
            } else {
                mix(p.text, p.panel, 0.2)
            },
            knob_track: ensure_contrast(p.grid_bar, deck, 3.0),
            knob_pointer: if dark { p.text } else { Color32::WHITE },
            value_pill: if dark {
                shade(deck, 0.04)
            } else {
                shade(deck, -0.06)
            },
            value_pill_text: p.text,
            step_odd: up(p.panel_raised, 0.1),
            step_even: mix(up(p.panel_raised, 0.1), p.danger, 0.12),
            step_odd_lit: base.step_odd_lit,
            step_even_lit: base.step_even_lit,
            menu,
            menu_band: up(menu, 0.06),
            menu_hover,
            menu_text: p.text,
            menu_text_hover: on_fill(menu_hover),
            hue_source: base.hue_source,
            hue_tone: base.hue_tone,
            hue_dynamics: base.hue_dynamics,
            hue_mod: base.hue_mod,
            hue_space: base.hue_space,
            hue_output: base.hue_output,
            shadow: p.shadow,
            highlight_band: mix(p.gold_bright, Color32::WHITE, 0.55),
            key_white: p.white_key,
            key_black: p.black_key,
            row_white: p.row_white,
            row_black: p.row_black,
            playhead: p.gold_bright,
            light: Color32::WHITE,
            dark_shade: Color32::BLACK,
            dark,
            radius: p.corner_radius as f32,
            round: 1.0,
            depth: 1.0,
            density: 1.0,
            text_scale: 1.0,
            generation: 0,
        };
        t.floor();
        t
    }

    /// The tokens a theme file describes: derived from its colours, overridden by its
    /// `kit` object, shifted by its knobs.
    pub fn for_theme(theme: &Theme) -> Tokens {
        let mut t = Tokens::derive_from_palette(&theme.base_palette());
        apply_map(&mut t, &theme.kit);
        t.dark = theme.dark;
        theme.form.apply(&mut t);
        t.with_knobs(&theme.knobs)
    }

    /// These tokens with FL's theme knobs applied to the ladder. Where the shifted
    /// ladder would put text under 4.5:1, the brightness and contrast knobs are eased
    /// back until it does not ("derive clamps lightness until it passes").
    pub fn with_knobs(&self, knobs: &ThemeKnobs) -> Tokens {
        if knobs.is_neutral() {
            return *self;
        }
        for ease in [1.0, 0.8, 0.6, 0.4, 0.2, 0.0] {
            let k = ThemeKnobs {
                brightness: knobs.brightness * ease,
                contrast: knobs.contrast * ease,
                ..*knobs
            };
            let mut t = self.shifted(&k);
            t.floor();
            if t.legibility_failures().is_empty() {
                return t;
            }
        }
        let mut t = self.shifted(&ThemeKnobs {
            brightness: 0.0,
            contrast: 0.0,
            ..*knobs
        });
        t.floor();
        t
    }

    fn shifted(&self, k: &ThemeKnobs) -> Tokens {
        let mid = Hsl::of(self.panel).l;
        let s = |c| k.surface(c, mid);
        let x = |c| k.text(c);
        Tokens {
            ink: s(self.ink),
            well: s(self.well),
            meter_well: s(self.meter_well),
            strip: s(self.strip),
            strip_selected: s(self.strip_selected),
            panel: s(self.panel),
            toolbar: s(self.toolbar),
            panel_raised: s(self.panel_raised),
            panel_light: s(self.panel_light),
            header_cell: s(self.header_cell),
            device_ground: s(self.device_ground),
            deck: s(self.deck),
            deck_title: s(self.deck_title),
            grid: s(self.grid),
            grid_strong: s(self.grid_strong),
            grid_bar: s(self.grid_bar),
            edge: s(self.edge),
            text: x(self.text),
            text_dim: x(self.text_dim),
            label: x(self.label),
            placeholder: x(self.placeholder),
            cap: s(self.cap),
            knob_body: s(self.knob_body),
            knob_rim: s(self.knob_rim),
            knob_track: s(self.knob_track),
            value_pill: s(self.value_pill),
            menu: s(self.menu),
            menu_band: s(self.menu_band),
            menu_hover: s(self.menu_hover),
            row_white: s(self.row_white),
            row_black: s(self.row_black),
            ..*self
        }
    }

    /// Push every text colour to 4.5:1 on the surfaces it is read on, and outlines to
    /// 3:1 on the deck.
    fn floor(&mut self) {
        let text_on = [
            self.panel,
            self.strip,
            self.strip_selected,
            self.panel_raised,
            self.deck,
            self.deck_title,
            self.well,
            self.ink,
        ];
        self.text = ensure_contrast_all(self.text, &text_on, 4.5);
        self.text_dim =
            ensure_contrast_all(self.text_dim, &[self.panel, self.strip, self.deck], 4.5);
        self.label =
            ensure_contrast_all(self.label, &[self.strip, self.deck, self.deck_title], 4.5);
        self.menu_text = ensure_contrast_all(self.menu_text, &[self.menu, self.menu_band], 4.5);
        self.menu_text_hover = ensure_contrast(self.menu_text_hover, self.menu_hover, 4.5);
        self.value_pill_text = ensure_contrast(self.value_pill_text, self.value_pill, 4.5);
        // Placeholders stay faint on purpose, but never vanish.
        self.placeholder = ensure_contrast(self.placeholder, self.panel_raised, 1.8);
        // The focus ring and the selection must show on whatever surface they land on,
        // wherever the knobs moved the ladder.
        self.focus = ensure_contrast_all(self.focus, &[self.panel, self.ink], 3.0);
        self.selection = ensure_contrast_all(self.selection, &[self.strip, self.panel], 3.0);
        self.edge = ensure_contrast(self.edge, self.deck, 3.0);
        if contrast(self.knob_rim, self.deck).max(contrast(self.knob_body, self.deck)) < 3.0 {
            self.knob_rim = ensure_contrast(self.knob_rim, self.deck, 3.0);
        }
        self.knob_track = ensure_contrast(self.knob_track, self.deck, 3.0);
        for f in Function::ALL {
            let h = ensure_contrast_all(self.hue(f), &[self.deck, self.deck_title, self.ink], 4.5);
            match f {
                Function::Source => self.hue_source = h,
                Function::Tone => self.hue_tone = h,
                Function::Dynamics => self.hue_dynamics = h,
                Function::Mod => self.hue_mod = h,
                Function::Space => self.hue_space = h,
                Function::Output => self.hue_output = h,
            }
        }
    }

    /// Every pairing the kit promises (ADR 0024 §14), as (what, fg, bg, needed ratio).
    /// Text 4.5:1 on its surface; function hues 4.5:1 on their decks and displays;
    /// outlines, tracks and the focus ring 3:1.
    pub fn pairings(&self) -> Vec<(String, Color32, Color32, f32)> {
        let mut v: Vec<(String, Color32, Color32, f32)> = Vec::new();
        let mut add = |what: &str, fg, bg, need| v.push((what.to_string(), fg, bg, need));
        for (n, bg) in [
            ("ink", self.ink),
            ("panel", self.panel),
            ("strip", self.strip),
            ("strip_selected", self.strip_selected),
            ("panel_raised", self.panel_raised),
            ("deck", self.deck),
            ("deck_title", self.deck_title),
            ("well", self.well),
        ] {
            add(&format!("text on {n}"), self.text, bg, 4.5);
        }
        for (n, bg) in [
            ("panel", self.panel),
            ("strip", self.strip),
            ("deck", self.deck),
        ] {
            add(&format!("text_dim on {n}"), self.text_dim, bg, 4.5);
        }
        for (n, bg) in [
            ("strip", self.strip),
            ("deck", self.deck),
            ("deck_title", self.deck_title),
        ] {
            add(&format!("label on {n}"), self.label, bg, 4.5);
        }
        add("menu text on menu", self.menu_text, self.menu, 4.5);
        add("menu text on band", self.menu_text, self.menu_band, 4.5);
        add("hover text", self.menu_text_hover, self.menu_hover, 4.5);
        add("value pill", self.value_pill_text, self.value_pill, 4.5);
        for f in Function::ALL {
            for (n, bg) in [
                ("deck", self.deck),
                ("deck_title", self.deck_title),
                ("display", self.ink),
            ] {
                add(&format!("{f:?} hue on {n}"), self.hue(f), bg, 4.5);
            }
        }
        add("edge on deck", self.edge, self.deck, 3.0);
        add("knob track on deck", self.knob_track, self.deck, 3.0);
        let rim = contrast(self.knob_rim, self.deck);
        let boundary = if rim > contrast(self.knob_body, self.deck) {
            self.knob_rim
        } else {
            self.knob_body
        };
        add("knob boundary on deck", boundary, self.deck, 3.0);
        add("focus on panel", self.focus, self.panel, 3.0);
        add("focus on ink", self.focus, self.ink, 3.0);
        add("selection on strip", self.selection, self.strip, 3.0);
        add("selection on panel", self.selection, self.panel, 3.0);
        // Placeholders are faint on purpose (FL's "Slot 5"), but never invisible.
        add(
            "placeholder on a tile",
            self.placeholder,
            self.panel_raised,
            1.8,
        );
        v
    }

    /// The pairings that fall short, with the ratio they reach.
    pub fn legibility_failures(&self) -> Vec<(String, f32)> {
        self.pairings()
            .into_iter()
            .filter_map(|(what, fg, bg, need)| {
                let got = contrast(fg, bg);
                (got + 1e-3 < need).then(|| (format!("{what} (needs {need})"), got))
            })
            .collect()
    }
}

fn installed() -> &'static std::sync::RwLock<Tokens> {
    static T: std::sync::OnceLock<std::sync::RwLock<Tokens>> = std::sync::OnceLock::new();
    T.get_or_init(|| std::sync::RwLock::new(Tokens::for_theme(&crate::theme::default_theme())))
}

/// The tokens in use. Painters never read this — they take tokens through their
/// [`Style`](super::Style) — only widget wrappers do, for convenience.
pub fn current() -> Tokens {
    *installed().read().unwrap_or_else(|e| e.into_inner())
}

/// Install tokens for every widget and bump the theme generation, so cached layers
/// re-render.
pub fn install(mut t: Tokens) {
    let g = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    t.generation = g;
    *installed().write().unwrap_or_else(|e| e.into_inner()) = t;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presets() -> [(&'static str, Tokens); 4] {
        [
            ("Slate", Tokens::slate()),
            ("Paper", Tokens::paper()),
            ("Parchment", Tokens::parchment()),
            ("Ink & Gold", Tokens::ink_and_gold()),
        ]
    }

    #[test]
    fn every_preset_keeps_every_promise() {
        for (name, t) in presets() {
            let bad = t.legibility_failures();
            assert!(bad.is_empty(), "{name}: {bad:#?}");
        }
    }

    #[test]
    fn slate_is_fl_s_ladder_one_hue_steps_in_lightness() {
        let t = Tokens::slate();
        let ladder = [
            t.ink,
            t.well,
            t.strip,
            t.panel,
            t.toolbar,
            t.panel_raised,
            t.panel_light,
        ];
        let mut last = -1.0;
        for c in ladder {
            let h = Hsl::of(c);
            assert!((195.0..=210.0).contains(&h.h), "{c:?} hue {}", h.h);
            assert!(h.s < 0.2, "{c:?} is not a neutral");
            assert!(h.l > last, "the ladder must step up in lightness");
            last = h.l;
        }
        // Paper keeps the same hue.
        let p = Hsl::of(Tokens::paper().panel);
        assert!((195.0..=210.0).contains(&p.h));
    }

    #[test]
    fn neutral_knobs_are_the_identity_and_every_knob_setting_stays_legible() {
        for (name, t) in presets() {
            assert_eq!(t.with_knobs(&ThemeKnobs::default()), t);
            for hue in [-120.0, 0.0, 90.0] {
                for saturation in [-1.0, 0.0, 1.0] {
                    for brightness in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                        for contrast in [-1.0, 0.0, 1.0] {
                            for text in [-1.0, 0.0, 1.0] {
                                let k = ThemeKnobs {
                                    hue,
                                    saturation,
                                    brightness,
                                    contrast,
                                    text,
                                };
                                let bad = t.with_knobs(&k).legibility_failures();
                                assert!(bad.is_empty(), "{name} at {k:?}: {bad:#?}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_brightness_knob_moves_the_ladder() {
        let t = Tokens::slate();
        let k = ThemeKnobs {
            brightness: 0.3,
            ..Default::default()
        };
        let lit = t.with_knobs(&k);
        assert!(Hsl::of(lit.panel).l > Hsl::of(t.panel).l + 0.04);
        // Role colours are picked on their own: the knobs leave them alone. (The focus
        // ring, the selection and the section hues only move when the shifted ladder
        // would put them under their contrast floor.)
        assert_eq!(lit.meter_low, t.meter_low);
        assert_eq!(lit.led, t.led);
        assert_eq!(lit.danger, t.danger);
    }

    #[test]
    fn a_kit_map_round_trips_and_every_token_has_a_unique_key() {
        for (_, t) in presets() {
            let map = to_map(&t);
            let mut back = Tokens::derive_from_palette(&crate::theme::paper_palette());
            back.dark = t.dark;
            back.radius = t.radius;
            apply_map(&mut back, &map);
            assert_eq!(back, t);
        }
        let mut keys: Vec<&str> = Tok::ALL.iter().map(|t| t.key()).collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "two tokens share a key");
        for t in Tok::ALL {
            assert_eq!(Tok::from_key(t.key()), Some(*t));
            assert!(
                t.key().starts_with(t.group().key()),
                "{} is filed under {:?}",
                t.key(),
                t.group()
            );
        }
        // Keys unknown to this build are ignored, not an error.
        let mut map = KitMap::new();
        map.insert("future.token".into(), "#123456".into());
        let mut t = Tokens::slate();
        apply_map(&mut t, &map);
        assert_eq!(t, Tokens::slate());
    }

    #[test]
    fn every_token_and_group_is_named_in_both_languages() {
        for lang in crate::i18n::Locale::ALL {
            crate::i18n::set_locale(lang);
            for t in Tok::ALL {
                let k = t.name_key();
                assert_ne!(
                    crate::i18n::t(&k),
                    k,
                    "{lang:?} has no name for {}",
                    t.key()
                );
            }
            for g in Group::ALL {
                let k = g.name_key();
                assert_ne!(
                    crate::i18n::t(&k),
                    k,
                    "{lang:?} has no name for group {g:?}"
                );
            }
        }
        crate::i18n::set_locale(crate::i18n::Locale::En);
        assert!(search("fader").contains(&Tok::Cap));
        assert!(search("MIXER").contains(&Tok::Strip));
    }

    #[test]
    fn old_theme_files_derive_legible_kit_tokens() {
        for p in [
            crate::theme::paper_palette(),
            crate::theme::parchment_palette(),
            crate::theme::ink_and_gold_palette(),
        ] {
            let bad = Tokens::derive_from_palette(&p).legibility_failures();
            assert!(bad.is_empty(), "{bad:#?}");
        }
    }
}
