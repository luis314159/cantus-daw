//! # A face of its own for each unit (round 32, BACKLOG §6h point 1, ADR 0074)
//!
//! Every effect and instrument of ours wears a **skin**: a style sheet that is data, not
//! painting code. A [`Skin`] names the unit's colour and material, its display's look, the
//! shape of its knobs and switches and its lettering; the kit's painters read it from the
//! [`Style`] they are handed and draw accordingly. The family's rules stay: label/value
//! units, nothing overlapping, resizable, keyboard and screen reader, the same parameter
//! menu on every control — a skin changes how a part looks, never what it does.
//!
//! ## How a skin reaches the pixels
//!
//! - [`of`] gives each [`Unit`] its sheet (one file per family under `kit/skin/`).
//! - The device shell ([`super::shell::Shell::skin`]) applies it: the style it hands the
//!   editor's body is [`Style::skinned`] — the theme's tokens with the unit's surface
//!   tint and accent laid over them ([`Skin::tokens`]), and `style.skin` set, so every
//!   painter below (knobs, switches, wells, cards, the header's nameplate, displays) can
//!   ask the sheet for its shape.
//! - The exported CLAP/VST3 plug-ins draw the same editors in the same shell
//!   (`cantus-plugins/src/editor.rs`), so they wear the same face with no code of theirs.
//! - [`PLAIN`] is the kit's one look (ADR 0024) — hosted plug-ins' Cantus view and
//!   everything that is not a unit keep it. A unit left on it fails
//!   `tests/skins.rs::every_unit_has_a_face_of_its_own`.
//!
//! ## Both themes, any scale
//!
//! Colours are [`Pair`]s — one [`Hsl`] for a dark theme, one for a light one — or a
//! [`Tint`] that moves the theme's own surface ladder toward a hue, so a skin follows the
//! theme's lightness structure and contrast in Slate and in Paper alike. Nothing is a
//! bitmap: every shape is vectors at the style's scale `k` (fine textures — brushed
//! grain, vinyl amp covering, carbon — are rasterised procedurally at the screen's own pixel size and
//! tiled, so they stay one texel per pixel at any scale).
//!
//! ## The skin a window wears
//!
//! While a unit's [`Shell`](super::shell::Shell) paints its header, its body and its
//! footer, the skin is **worn** on that thread ([`wear`]): [`Style::current`] returns the
//! skinned style, so every widget and helper inside the body that asks for "the current
//! style" draws in the unit's face without being handed it — an editor cannot lose its
//! skin by building its own style. Outside a unit's window the plain look is worn.
//!
//! ## What the painters do with the sheet
//!
//! - [`Material`]: the device ground, the cards and the header bar ([`materials`]).
//! - [`Display`]: [`display_ground`], [`display_ink`], [`displays::trace`],
//!   [`displays::graticule`], [`displays::cover`] — one way to light a display; the
//!   effects' `canvas::Ink` reads it, so every display of a unit takes its look.
//! - [`KnobShape`]: `knob::paint` ([`knobs`]); [`SwitchShape`]: `led::button`, the cards'
//!   power lamps and the segmented control's lit segment ([`switches`]).
//! - [`Lettering`]: the nameplate, the card titles, the section bands and the readouts.
//! - [`Plate`] and [`Ornaments`]: the header's nameplate and the face ([`plates`]).

use super::color::Hsl;
use super::knob::KnobLook;
use super::text::Face;
use super::{Brush, State, Style, Tok, Tokens};
use cantus_core::inst::InstrumentKind;
use cantus_core::EffectKind;
use egui::{Pos2, Rect};
use std::cell::Cell;

#[path = "skin/dynamics.rs"]
mod dynamics;
#[path = "skin/instruments.rs"]
mod instruments;
#[path = "skin/modulation.rs"]
mod modulation;
#[path = "skin/tone.rs"]
mod tone;

#[path = "skin/displays.rs"]
pub mod displays;
#[path = "skin/knobs.rs"]
pub mod knobs;
#[path = "skin/materials.rs"]
pub mod materials;
#[path = "skin/plates.rs"]
pub mod plates;
#[path = "skin/switches.rs"]
pub mod switches;
#[path = "skin/textures.rs"]
mod textures;

pub use displays::{
    ground as display_ground, ink as display_ink, style as display_style, DisplayInk,
};

/// One colour per theme mode.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pair {
    pub dark: Hsl,
    pub light: Hsl,
}

impl Pair {
    /// The same colour in both modes.
    pub const fn both(c: Hsl) -> Pair {
        Pair { dark: c, light: c }
    }

    /// The colour for a theme.
    pub fn get(&self, dark: bool) -> Hsl {
        if dark {
            self.dark
        } else {
            self.light
        }
    }
}

/// How the window's surface ladder (ink, well, panel, tiles, the device ground and its
/// cards) is coloured: moved toward `hue` with saturation `sat` by `amount` (0 keeps the
/// theme's own ladder, 1 is the hue at the ladder's lightness).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tint {
    /// Degrees, 0…360.
    pub hue: f32,
    /// 0…1.
    pub sat: f32,
    /// 0…1.
    pub amount: f32,
}

impl Tint {
    pub const NONE: Tint = Tint {
        hue: 0.0,
        sat: 0.0,
        amount: 0.0,
    };
}

/// The material of the unit's face: how its ground and cards are lit and textured.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    /// The kit's ground (ADR 0024).
    Plain,
    /// A flat, soft-lit paint (FabFilter's).
    Matte,
    /// Brushed metal: fine horizontal grain, a sheen band.
    Brushed,
    /// Anodised aluminium: a deep colour with a metallic gradient.
    Anodised,
    /// A glossy lacquer: a strong top highlight, a dark lower edge (a 70s keyboard's).
    Lacquer,
    /// Wood: a piano's case, an organ's cheeks — grain drawn as curves.
    Wood,
    /// Vinyl amp covering: an amplifier's pebbled vinyl, a fine texture. (The variant's
    /// name is the code's; users read "vinyl amp covering" — `name()` says "vinyl".)
    Tolex,
    /// Smoked glass over a dark ground.
    Glass,
    /// Printed paper or card (a vintage panel's legend plate).
    Paper,
    /// Carbon weave.
    Carbon,
}

/// How the unit's characteristic display (its curve, scope, room, tines…) is lit.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Display {
    /// The kit's ink well.
    Plain,
    /// A CRT's phosphor: a dark glass, traces that glow and persist.
    Phosphor,
    /// A liquid-crystal panel: a light, slightly green ground, dark segments.
    Lcd,
    /// A vacuum-fluorescent display: cyan-green light on black.
    Vfd,
    /// Glass over the face: the face's colour seen through a darkened pane.
    Glass,
    /// Ink on paper: a printed chart.
    Paper,
    /// An oscilloscope's graticule: a grid of divisions with sub-ticks.
    Scope,
}

/// A knob painter of a unit's own (drawn in its family's sheet file): same contract as
/// [`super::knob::paint`] — the part covers the knob's square, every state drawn.
pub type KnobPainter = fn(&mut Brush, Pos2, &Style, State, &KnobLook<'_>);

/// The shape of the unit's knobs.
#[derive(Copy, Clone, Debug)]
pub enum KnobShape {
    /// The kit's knob.
    Plain,
    /// A value ring only, no body (FabFilter's).
    Arc,
    /// A skirted knob: a wide fluted skirt under a small cap, a line on the skirt.
    Skirted,
    /// A chicken-head pointer knob (a vintage amplifier's).
    ChickenHead,
    /// A soft-touch rubber knob with a dot.
    Soft,
    /// A machined metal cap with knurling (a 70s synthesiser's).
    Cap,
    /// A dial with a graduated scale printed round it (no numbers: the value pill says
    /// the value in the parameter's own units).
    Dial,
    /// The unit's own painter.
    Custom(KnobPainter),
}

/// A switch painter of a unit's own: covers `rect`, every state drawn (`lit` is on).
pub type SwitchPainter = fn(&mut Brush, Rect, &Style, State);

/// The shape of the unit's on/off switches and lamp buttons.
#[derive(Copy, Clone, Debug)]
pub enum SwitchShape {
    /// The kit's LED button.
    Plain,
    /// A bat-handle toggle.
    Bat,
    /// A rocker.
    Rocker,
    /// A pill that slides.
    Pill,
    /// A square lamp button that glows when on (a drum machine's).
    Lamp,
    /// The unit's own painter.
    Custom(SwitchPainter),
}

/// Letter case of the unit's titles (the nameplate, card titles).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Case {
    /// As written.
    AsIs,
    Upper,
}

/// The weight of the unit's titles.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Medium,
    SemiBold,
}

/// How the unit sets its words: a typography accent over the kit's roles.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Lettering {
    pub case: Case,
    /// Extra space between letters of titles, in ems (0 = the font's own).
    pub tracking: f32,
    pub weight: Weight,
    /// Titles in the condensed face.
    pub condensed: bool,
    /// Readouts in the mono face (a lab instrument's).
    pub mono_values: bool,
}

impl Lettering {
    /// The kit's own lettering: titles in condensed semibold capitals with the role's
    /// tracking, values in the mono medium face. Any other value is taken literally:
    /// `AsIs` titles are set as written, `condensed: false` is IBM Plex Sans.
    pub const PLAIN: Lettering = Lettering {
        case: Case::AsIs,
        tracking: 0.0,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: false,
    };

    /// How a title (the nameplate, a card's, a section band's) is set: its face and
    /// whether in capitals.
    pub fn title(&self) -> (Face, bool) {
        if self.title_is_plain() {
            return (Face::ROLE, true);
        }
        let face = Face {
            letters: Some((self.weight, self.condensed)),
            tracking_milli: (self.tracking * 1000.0).round().clamp(-200.0, 600.0) as i16,
            mono: false,
        };
        (face, self.case == Case::Upper)
    }

    /// How a value (a readout, a cell's pill) is set.
    pub fn value(&self) -> Face {
        Face {
            mono: self.mono_values,
            ..Face::ROLE
        }
    }

    fn title_is_plain(&self) -> bool {
        Lettering {
            mono_values: false,
            ..*self
        } == Lettering::PLAIN
    }
}

/// How the unit's name sits in the header.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Plate {
    /// The kit's name in the device hue.
    Plain,
    /// Letters cut into the face.
    Engraved,
    /// A raised badge with the name on it.
    Badge,
    /// A painted stripe under the name, across the header.
    Stripe,
    /// A screen-printed legend in a box.
    Legend,
}

/// Small things on the face. All off is the kit's.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Ornaments {
    /// Screw heads in the corners.
    pub screws: bool,
    /// Vent slots by the header.
    pub vents: bool,
    /// Rules between card groups, printed on the face.
    pub rules: bool,
    /// Rack ears at the sides.
    pub rack_ears: bool,
}

/// A unit's style sheet.
#[derive(Copy, Clone, Debug)]
pub struct Skin {
    /// Stable id: `fx.<kind>` or `inst.<kind>` (`plain` for [`PLAIN`]). Two skins are the
    /// same skin when their keys are.
    pub key: &'static str,
    /// The surface ladder's tint.
    pub surface: Tint,
    /// The unit's own hue: arcs, LEDs, curves, a selected segment — where the kit uses the
    /// function hue. `None` keeps the function hue.
    pub accent: Option<Pair>,
    /// A second voice: a display's second trace, a lamp of another kind.
    pub accent2: Option<Pair>,
    pub material: Material,
    pub display: Display,
    pub knob: KnobShape,
    pub switch: SwitchShape,
    pub lettering: Lettering,
    pub plate: Plate,
    pub ornaments: Ornaments,
}

impl PartialEq for Skin {
    fn eq(&self, other: &Skin) -> bool {
        self.key == other.key
    }
}

/// The kit's one look (ADR 0024): what everything that is not one of our units wears.
pub const PLAIN: Skin = Skin {
    key: "plain",
    surface: Tint::NONE,
    accent: None,
    accent2: None,
    material: Material::Plain,
    display: Display::Plain,
    knob: KnobShape::Plain,
    switch: SwitchShape::Plain,
    lettering: Lettering::PLAIN,
    plate: Plate::Plain,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: false,
        rack_ears: false,
    },
};

impl Skin {
    /// The kit's plain look, with another key: where a family's sheet starts.
    pub const fn plain(key: &'static str) -> Skin {
        Skin { key, ..PLAIN }
    }

    /// Whether this is the kit's plain look in all but its key — what
    /// `every_unit_has_a_face_of_its_own` refuses for a unit.
    pub fn is_plain(&self) -> bool {
        self.surface.amount <= 0.0
            && self.accent.is_none()
            && self.accent2.is_none()
            && self.material == Material::Plain
            && self.display == Display::Plain
            && matches!(self.knob, KnobShape::Plain)
            && matches!(self.switch, SwitchShape::Plain)
            && self.lettering == Lettering::PLAIN
            && self.plate == Plate::Plain
            && self.ornaments == Ornaments::default()
    }

    /// Whether the sheet changes the theme's tokens: a tint, an accent, or a material
    /// whose light and shade the text must be floored over.
    pub fn recolours(&self) -> bool {
        self.surface.amount > 0.0 || self.accent.is_some() || self.material != Material::Plain
    }

    /// The theme's tokens as this unit wears them: the surface ladder tinted, the accent
    /// over every function hue but modulation's (ADR 0057's colour stays the cue for
    /// what moves, pushed away from the accent when the two are close), then the theme's
    /// contrast floor run again
    /// (`Tokens::floor`: text 4.5:1 on what it is read on, the knob track and outlines
    /// 3:1 on the deck, the hues 4.5:1 on the deck and the ink) — a tint never breaks the
    /// kit's promises, in any preset. The plain skin returns them unchanged.
    pub fn tokens(&self, base: &Tokens) -> Tokens {
        if !self.recolours() {
            return *base;
        }
        // Made once per skin and theme (the shell skins its style every frame).
        let key = super::cache::key((
            self.colour_key(),
            base.generation,
            base.dark,
            super::cache::ckey(base.panel),
            super::cache::ckey(base.deck),
            super::cache::ckey(base.ink),
            super::cache::ckey(base.text),
            super::cache::ckey(base.hue_tone),
            super::cache::ckey(base.knob_track),
        ));
        if let Some(t) = TINTED.with(|c| {
            c.borrow()
                .iter()
                .find(|(k, b, _)| *k == key && b == base)
                .map(|(_, _, t)| *t)
        }) {
            return t;
        }
        let t = self.tokens_uncached(base);
        TINTED.with(|c| {
            let mut c = c.borrow_mut();
            // Room for every unit open at once and the gallery's sheets.
            if c.len() >= 48 {
                c.remove(0);
            }
            c.push((key, *base, t));
        });
        t
    }

    fn tokens_uncached(&self, base: &Tokens) -> Tokens {
        COUNTS.with(|c| c.set((c.get().0 + 1, c.get().1, c.get().2)));
        let mut t = *base;
        if self.surface.amount > 0.0 {
            // The hue moves, the luminance stays: a yellow at a grey's HSL lightness is far
            // brighter than the grey, and the ladder's contrasts are luminance's.
            let tint = |c: egui::Color32| -> egui::Color32 {
                let h = Hsl::of(c);
                let a = self.surface.amount.clamp(0.0, 1.0);
                let toward = Hsl {
                    h: self.surface.hue,
                    s: h.s + (self.surface.sat - h.s) * a,
                    l: h.l,
                };
                with_luminance(toward, super::color::luminance(c))
            };
            for c in [
                &mut t.ink,
                &mut t.well,
                &mut t.panel,
                &mut t.panel_raised,
                &mut t.panel_light,
                &mut t.device_ground,
                &mut t.deck,
                &mut t.deck_title,
                &mut t.value_pill,
                &mut t.meter_well,
                &mut t.knob_body,
                &mut t.knob_rim,
                &mut t.knob_track,
            ] {
                *c = tint(*c);
            }
        }
        if let Some(a) = self.accent {
            let c = a.get(base.dark).rgb();
            t.hue_source = c;
            t.hue_tone = c;
            t.hue_dynamics = c;
            t.hue_space = c;
            t.hue_output = c;
            t.hue_mod = apart_from(base.hue_mod, c);
        }
        // The theme's floor holds already for its own tokens; again only when the sheet
        // moved them.
        if self.surface.amount > 0.0 || self.accent.is_some() {
            t.floor();
        }
        // Over a material's light and shade too (the integrator's review, 4).
        materials::floor(&mut t, self.material);
        t
    }

    /// What the sheet's colours are made of — its tint, its accents and its material — as a
    /// cache key: two sheets under one key (a test's) never share another's tokens.
    pub fn colour_key(&self) -> u64 {
        let hsl = |c: Hsl| (c.h.to_bits(), c.s.to_bits(), c.l.to_bits());
        super::cache::key((
            self.key,
            self.surface.hue.to_bits(),
            self.surface.sat.to_bits(),
            self.surface.amount.to_bits(),
            self.accent.map(|p| (hsl(p.dark), hsl(p.light))),
            self.accent2.map(|p| (hsl(p.dark), hsl(p.light))),
            self.material,
        ))
    }

    /// The accent in a theme, if the skin has one.
    pub fn accent_in(&self, dark: bool) -> Option<egui::Color32> {
        self.accent.map(|a| a.get(dark).rgb())
    }

    /// The second accent in a theme, if the skin has one.
    pub fn accent2_in(&self, dark: bool) -> Option<egui::Color32> {
        self.accent2.map(|a| a.get(dark).rgb())
    }
}

impl Style {
    /// This style in a unit's skin: its tokens tinted ([`Skin::tokens`]), its hue the
    /// accent, and `skin` set for the painters that draw shapes.
    ///
    /// A style already in `skin` is returned as it is (the tint is not laid twice).
    pub fn skinned(mut self, skin: &'static Skin) -> Style {
        if self.skin == skin {
            return self;
        }
        self.t = skin.tokens(&self.t);
        // From the floored tokens (the integrator's review, 1): the accent as the floor
        // left it — what the worn style and the header paint — whatever the style's
        // function, modulation's included (a modulation unit wears its accent; the cues
        // read `t.hue_mod`, kept apart).
        if skin.accent.is_some() {
            self.hue = self.t.hue_tone;
            let function = |t: Tok| super::tokens::Function::ALL.iter().any(|f| f.tok() == t);
            if self.hue_tok.is_some_and(function) {
                self.hue_tok = Some(Tok::HueTone);
            }
        } else if let Some(tok) = self.hue_tok {
            self.hue = self.t.get(tok);
        }
        self.skin = skin;
        self
    }
}

/// How far apart two hues sit on the wheel, in degrees (0…180).
fn hue_gap(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// The least hue gap between modulation's colour and an accent (degrees).
pub const MOD_GAP: f32 = 50.0;

/// Modulation's colour `m` kept apart from the accent `a`: as it is when the two are
/// [`MOD_GAP`] apart on the wheel (or either is nearly grey), else turned away from the
/// accent, on its own side, until they are.
pub(crate) fn apart_from(m: egui::Color32, a: egui::Color32) -> egui::Color32 {
    let (hm, ha) = (Hsl::of(m), Hsl::of(a));
    if hm.s < 0.15 || ha.s < 0.15 || hue_gap(hm.h, ha.h) >= MOD_GAP {
        return m;
    }
    let side = if (hm.h - ha.h).rem_euclid(360.0) <= 180.0 {
        1.0
    } else {
        -1.0
    };
    // A little past the gap, so the floor's lightness steps never bring it back.
    Hsl {
        h: (ha.h + side * (MOD_GAP + 6.0)).rem_euclid(360.0),
        ..hm
    }
    .rgb()
}

/// A third colour with a meaning of its own inside a display — a loudness beside the
/// output and the ceiling, a post EQ beside a decay curve — for a section of function `f`
/// (the review of delivery 2: in a face every function hue is the accent, and colours that
/// told two things apart became one). Plain (no accent), the theme's function hue as ever.
/// In a face, the theme's hue for `f` ([`Style::cues`]), saturated enough to read, turned in
/// 30° steps until it is 24° or more from every colour in `from` (the display's voices and
/// its danger), then held at 3:1 on `ground`.
pub fn distinct_hue(
    s: &Style,
    f: super::tokens::Function,
    from: &[egui::Color32],
    ground: egui::Color32,
) -> egui::Color32 {
    if s.skin.accent.is_none() {
        return s.t.hue(f);
    }
    let i = super::tokens::Function::ALL
        .iter()
        .position(|x| *x == f)
        .unwrap_or(0);
    let mut h = Hsl::of(s.cues[i]);
    h.s = h.s.max(0.55);
    let apart = |h: &Hsl| {
        from.iter().all(|c| {
            let o = Hsl::of(*c);
            o.s < 0.15 || hue_gap(o.h, h.h) >= 24.0
        })
    };
    for _ in 0..12 {
        if apart(&h) {
            break;
        }
        h.h = (h.h + 30.0).rem_euclid(360.0);
    }
    super::color::ensure_contrast(h.rgb(), ground, 3.0)
}

/// The colour of a modulation cue for a source of function `f` — a knob's ring for an
/// LFO, an envelope, a follower, steps or random (the integrator's review of modulation):
/// the theme's function hue as ever; in a face with an accent, the theme's hue for `f` as
/// the style carries it ([`Style::cues`]: the face lays its accent over the tokens'
/// function hues) turned [`MOD_GAP`] away from the accent and kept at 3:1 on the deck —
/// so no ring or source chip takes the value arc's colour.
pub fn cue_hue(s: &Style, f: super::tokens::Function) -> egui::Color32 {
    if s.skin.accent.is_none() {
        return s.t.hue(f);
    }
    let theme = if f == super::tokens::Function::Mod {
        s.t.hue_mod
    } else {
        let i = super::tokens::Function::ALL
            .iter()
            .position(|x| *x == f)
            .unwrap_or(0);
        s.cues[i]
    };
    super::color::ensure_contrast(apart_from(theme, s.t.hue_tone), s.t.deck, 3.0)
}

/// `c` with its lightness moved (hue and saturation kept) until its relative luminance is
/// `target` — by bisection; luminance grows with lightness at a fixed hue and saturation.
fn with_luminance(c: Hsl, target: f32) -> egui::Color32 {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..18 {
        let mid = (lo + hi) * 0.5;
        if super::color::luminance(Hsl { l: mid, ..c }.rgb()) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Hsl {
        l: (lo + hi) * 0.5,
        ..c
    }
    .rgb()
}

/// One of our units: what has a skin.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Effect(EffectKind),
    Instrument(InstrumentKind),
    /// Pulsus, the drum machine.
    Pulsus,
}

impl Unit {
    /// Every unit that has an editor of ours (`views::fx::Editor::of`, the instruments'
    /// frame, Pulsus): each must have a face of its own.
    pub fn all() -> Vec<Unit> {
        let mut v: Vec<Unit> = EffectKind::ALL
            .iter()
            .copied()
            .filter(|k| crate::views::fx::Editor::of(*k).is_some())
            .map(Unit::Effect)
            .collect();
        v.extend(InstrumentKind::ALL.iter().copied().map(Unit::Instrument));
        v.push(Unit::Pulsus);
        v
    }
}

/// The skin of a unit (its family's sheet); [`PLAIN`] for what has none — the legacy
/// delay and reverb and hosted plug-ins.
pub fn of(unit: Unit) -> &'static Skin {
    if !enabled() {
        return &PLAIN;
    }
    if let Some(s) = OVERRIDE.with(|o| o.get()) {
        return s;
    }
    if let Some(by) = OVERRIDE_BY.with(|o| o.get()) {
        return by(unit);
    }
    match unit {
        Unit::Effect(kind) => dynamics::of(kind)
            .or_else(|| modulation::of(kind))
            .or_else(|| tone::of(kind))
            .unwrap_or(&PLAIN),
        Unit::Instrument(kind) => instruments::of(kind),
        Unit::Pulsus => &instruments::PULSUS,
    }
}

/// The skin of an effect kind.
pub fn of_effect(kind: EffectKind) -> &'static Skin {
    of(Unit::Effect(kind))
}

/// The skin of an instrument kind.
pub fn of_instrument(kind: InstrumentKind) -> &'static Skin {
    of(Unit::Instrument(kind))
}

/// A sheet per unit (the frame budget's override).
pub type SkinBy = fn(Unit) -> &'static Skin;

/// A worn skin's tokens: (theme generation, dark, the sheet's colour key, the tokens).
type WornEntry = (u64, bool, u64, Tokens);

/// How often this thread made a skin's tokens, missed the worn style's cache, and made a
/// display's colours: the frame budget asserts they stay still in steady state.
pub fn counts() -> (u64, u64, u64) {
    COUNTS.with(|c| c.get())
}

pub(crate) fn count_display() {
    COUNTS.with(|c| c.set((c.get().0, c.get().1, c.get().2 + 1)));
}

thread_local! {
    static COUNTS: Cell<(u64, u64, u64)> = const { Cell::new((0, 0, 0)) };
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    static OVERRIDE: Cell<Option<&'static Skin>> = const { Cell::new(None) };
    static OVERRIDE_BY: Cell<Option<SkinBy>> = const { Cell::new(None) };
    static WORN: Cell<&'static Skin> = const { Cell::new(&PLAIN) };
    /// The last few skins' tokens, by skin and theme.
    static TINTED: std::cell::RefCell<Vec<(u64, Tokens, Tokens)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// The worn skins' tokens, made once per theme generation and skin — a few, so the
    /// open units' windows painted one after another do not make each other's again.
    static WORN_TOKENS: std::cell::RefCell<[Option<WornEntry>; 4]> =
        const { std::cell::RefCell::new([None; 4]) };
}

/// Every unit wears `skin` on this thread (`None`: each its own) — for the tests that
/// prove a body paints in its unit's face, and for looking at a sheet on every editor.
pub fn set_override(skin: Option<&'static Skin>) {
    OVERRIDE.with(|o| o.set(skin));
}

/// Every unit wears the skin `by` picks for it on this thread (`None`: each its own) —
/// for the frame budget, which measures distinct rich faces side by side. A skin set with
/// [`set_override`] wins over it.
pub fn set_override_by(by: Option<SkinBy>) {
    OVERRIDE_BY.with(|o| o.set(by));
}

/// The skin worn on this thread right now: a unit's while its shell paints, else
/// [`PLAIN`].
pub fn worn() -> &'static Skin {
    WORN.with(|w| w.get())
}

/// While alive, `skin` is worn on this thread; dropped, the skin worn before is back.
#[must_use = "the skin is taken off when this is dropped"]
pub struct Wearing(&'static Skin);

/// Wear `skin` on this thread until the guard is dropped ([`Style::current`] returns the
/// style in it).
pub fn wear(skin: &'static Skin) -> Wearing {
    Wearing(WORN.with(|w| w.replace(skin)))
}

impl Drop for Wearing {
    fn drop(&mut self) {
        WORN.with(|w| w.set(self.0));
    }
}

/// The installed tokens in the worn skin (made once per theme and skin, then copied).
pub(crate) fn worn_style(base: Tokens) -> Style {
    let skin = worn();
    if !skin.recolours() {
        // No colour of the sheet's: the theme's tokens as they are.
        let mut s = Style::new(base);
        s.skin = skin;
        return s;
    }
    let key = (base.generation, base.dark, skin.colour_key());
    let cached = if base.generation != 0 {
        WORN_TOKENS.with(|c| {
            c.borrow()
                .iter()
                .flatten()
                .find(|(g, d, k, _)| (*g, *d, *k) == key)
                .map(|e| e.3)
        })
    } else {
        None
    };
    let t = match cached {
        Some(t) => t,
        None => {
            COUNTS.with(|c| c.set((c.get().0, c.get().1 + 1, c.get().2)));
            let t = skin.tokens(&base);
            WORN_TOKENS.with(|c| {
                let mut c = c.borrow_mut();
                c.rotate_right(1);
                c[0] = Some((key.0, key.1, key.2, t));
            });
            t
        }
    };
    // The accent, when there is one, is every function hue of these tokens; the cues keep
    // the theme's.
    let mut s = Style::new(t);
    s.skin = skin;
    s.cues = Style::new(base).cues;
    s
}

/// Whether units wear their own faces (Settings ▸ Appearance; on by default). Off, every
/// unit wears [`PLAIN`]. Per thread (the thread that draws: the app's UI thread, an
/// exported plug-in's window thread), so tests that turn it off touch no other test; the
/// app sets it from the settings in its boot stages and when they change.
pub fn enabled() -> bool {
    ENABLED.with(|e| e.get())
}

/// Turn units' own faces on or off for this thread.
pub fn set_enabled(on: bool) {
    ENABLED.with(|e| e.set(on));
}

/// Where `skin` breaks the kit's contrast promises (`Tokens::legibility_failures`) in any
/// shipped theme preset, as "preset: pairing (ratio)" — empty when it keeps them all. Each
/// family's sheet test calls it for its units.
pub fn contrast_failures(skin: &Skin) -> Vec<String> {
    let mut out = Vec::new();
    for (key, make) in crate::theme::PRESETS {
        let t = skin.tokens(&make().kit_tokens());
        for (what, got) in t.legibility_failures() {
            out.push(format!("{} {key}: {what} at {got:.2}", skin.key));
        }
        for (what, got) in material_failures(&t, skin.material) {
            out.push(format!("{} {key}: {what} at {got:.2}", skin.key));
        }
    }
    out
}

/// Where text, dim text and labels (on a card and the header) and the function hues (on a
/// card) fall below 4.5:1 over the darkest or the lightest colour `material` makes of it
/// in tokens `t`.
pub fn material_failures(t: &Tokens, material: Material) -> Vec<(String, f32)> {
    use super::color::contrast;
    use materials::{extremes, Surface};
    let mut out = Vec::new();
    let card = extremes(material, Surface::Card, t, t.deck);
    let head = extremes(material, Surface::Header, t, t.panel);
    let mut check = |what: &str, fg: egui::Color32, on: &[egui::Color32], name: &str| {
        for (i, bg) in on.iter().enumerate() {
            let got = contrast(fg, *bg);
            if got < 4.5 {
                let side = if i % 2 == 0 { "darkest" } else { "lightest" };
                out.push((
                    format!("{what} on the {} {name} ({side})", material.name()),
                    got,
                ));
            }
        }
    };
    check("text", t.text, &card, "card");
    check("text_dim", t.text_dim, &card, "card");
    check("label", t.label, &card, "card");
    check("text", t.text, &head, "header");
    check("text_dim", t.text_dim, &head, "header");
    for f in super::tokens::Function::ALL {
        // On cards, where the arcs and the lamps are read (the kit promises no hue on the
        // panel the header is painted in; the header's stripe is not text).
        check(&format!("{f:?} hue"), t.hue(f), &card, "card");
    }
    out
}

/// A sheet as text that is the same in every process: its fields, with a unit's own knob
/// or switch painter named by its kind, never by its address (`Debug` of a `fn` prints
/// the address, which ASLR moves every launch — the picker's thumbnails would all miss;
/// the integrator's review). A painter whose drawing changes bumps `picker_thumbs::REV`.
pub fn stable(skin: &Skin) -> String {
    let plain = Skin {
        knob: if matches!(skin.knob, KnobShape::Custom(_)) {
            KnobShape::Plain
        } else {
            skin.knob
        },
        switch: if matches!(skin.switch, SwitchShape::Custom(_)) {
            SwitchShape::Plain
        } else {
            skin.switch
        },
        ..*skin
    };
    format!(
        "{plain:?} knob={} switch={}",
        skin.knob.name(),
        skin.switch.name()
    )
}

/// A digest of every unit's sheet as worn on this thread right now (the switch, an
/// override): the picker's thumbnails are keyed by it, so a face that changes — on, off,
/// a sheet edited in a release — never shows an old picture (the integrator's review, 2).
pub fn sheets_digest() -> u64 {
    thread_local! {
        static DIGEST: Cell<Option<(bool, usize, u64)>> = const { Cell::new(None) };
    }
    let on = enabled();
    let ov = OVERRIDE
        .with(|o| o.get())
        .map_or(0, |s| s as *const Skin as usize)
        ^ OVERRIDE_BY
            .with(|o| o.get())
            .map_or(0, |f| (f as usize).rotate_left(17));
    if let Some((e, o, d)) = DIGEST.with(|c| c.get()) {
        if (e, o) == (on, ov) {
            return d;
        }
    }
    let sheets: Vec<String> = Unit::all().into_iter().map(|u| stable(of(u))).collect();
    let d = super::cache::key((on, sheets));
    DIGEST.with(|c| c.set(Some((on, ov, d))));
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The integrator's finding from the instruments' sheets: tinting the ladder, even a
    /// little, moved pairs below the floor in some presets (Cantus Night's knob track on
    /// the deck at 2.6–2.95, Parchment's dim text on the panel at 4.49). `Skin::tokens`
    /// runs the floor again; every tint, in every preset, keeps the promises.
    #[test]
    fn a_tinted_skin_keeps_the_kit_s_contrast_floor_in_every_preset() {
        let mut bad = Vec::new();
        for hue in (0..360).step_by(30) {
            for (sat, amount) in [(0.04, 0.3), (0.04, 1.0), (0.3, 0.6), (0.6, 1.0), (0.9, 0.5)] {
                for accent in [
                    None,
                    Some(Pair {
                        dark: Hsl {
                            h: hue as f32 + 90.0,
                            s: 0.7,
                            l: 0.6,
                        },
                        light: Hsl {
                            h: hue as f32 + 90.0,
                            s: 0.7,
                            l: 0.4,
                        },
                    }),
                ] {
                    let skin: &'static Skin = Box::leak(Box::new(Skin {
                        surface: Tint {
                            hue: hue as f32,
                            sat,
                            amount,
                        },
                        accent,
                        ..Skin::plain("test.tint")
                    }));
                    bad.extend(contrast_failures(skin).into_iter().map(|f| {
                        format!(
                            "hue {hue} sat {sat} amount {amount} accent {}: {f}",
                            accent.is_some()
                        )
                    }));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} failures, first: {:#?}",
            bad.len(),
            &bad[..bad.len().min(12)]
        );
    }

    /// The integrator's review, 1: a style's hue is the accent as the floor left it — a
    /// light accent (a pale yellow) reaches 4.5:1 on the deck in every preset, and is the
    /// hue the worn style (the header, `Style::current()`) paints.
    #[test]
    fn a_skinned_style_s_hue_is_the_floored_accent_the_worn_style_paints() {
        static PALE: Skin = Skin {
            accent: Some(Pair::both(Hsl {
                h: 50.0,
                s: 0.9,
                l: 0.75,
            })),
            ..Skin::plain("test.pale")
        };
        let mut bad = Vec::new();
        for (key, make) in crate::theme::PRESETS {
            let base = make().kit_tokens();
            let s = Style::new(base).skinned(&PALE);
            let got = super::super::color::contrast(s.hue, s.t.deck);
            if got < 4.5 {
                bad.push(format!("{key}: the hue on the deck at {got:.2}"));
            }
            let worn = {
                let _on = wear(&PALE);
                worn_style(base)
            };
            if worn.hue != s.hue {
                bad.push(format!("{key}: {:?} skinned, {:?} worn", s.hue, worn.hue));
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// The integrator's review, 3: an accent never takes modulation's colour (ADR 0057's
    /// cue for what moves): it stays the theme's, and is turned away from an accent that
    /// comes within `MOD_GAP` of it — in every preset, for an accent anywhere on the wheel.
    #[test]
    fn modulation_keeps_its_own_colour_apart_from_any_accent() {
        let mut bad = Vec::new();
        for (key, make) in crate::theme::PRESETS {
            let base = make().kit_tokens();
            let theirs = Hsl::of(base.hue_mod);
            for h in (0..360).step_by(10) {
                let accent = Hsl {
                    h: h as f32,
                    s: 0.75,
                    l: if base.dark { 0.6 } else { 0.4 },
                };
                let skin: &'static Skin = Box::leak(Box::new(Skin {
                    accent: Some(Pair::both(accent)),
                    ..Skin::plain("test.mod")
                }));
                let t = skin.tokens(&base);
                let (m, a) = (Hsl::of(t.hue_mod), Hsl::of(t.hue_tone));
                let gap = hue_gap(m.h, a.h);
                if gap < MOD_GAP - 4.0 {
                    bad.push(format!("{key} accent {h}: modulation {gap:.0}° from it"));
                }
                if hue_gap(theirs.h, accent.h) >= MOD_GAP + 4.0 && hue_gap(m.h, theirs.h) > 4.0 {
                    bad.push(format!("{key} accent {h}: modulation's own hue moved"));
                }
                if t.hue_mod == t.hue_tone {
                    bad.push(format!("{key} accent {h}: modulation is the accent"));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} failures: {:#?}",
            bad.len(),
            &bad[..bad.len().min(12)]
        );
    }

    /// The integrator's note on the modulation family: a modulation unit's device function
    /// is `Function::Mod`, and its nameplate, stripes, lamps and cards must wear its accent
    /// — only the cues (`t.hue_mod`) keep modulation's colour. Plain, nothing changes.
    #[test]
    fn a_modulation_unit_wears_its_accent_and_its_cues_keep_modulation_s_colour() {
        use super::super::tokens::Function;
        static MODUNIT: Skin = Skin {
            accent: Some(Pair::both(Hsl {
                h: 280.0,
                s: 0.6,
                l: 0.55,
            })),
            ..Skin::plain("test.modunit")
        };
        for (key, make) in crate::theme::PRESETS {
            let base = make().kit_tokens();
            let body = Style::new(base).skinned(&MODUNIT);
            let accent = body.t.hue_tone;
            for f in Function::ALL {
                assert_eq!(body.function(f).hue, accent, "{key}: a {f:?} section");
            }
            // The header's path: the function first, then the face.
            let header = Style::new(base).function(Function::Mod).skinned(&MODUNIT);
            assert_eq!(header.hue, accent, "{key}: the header");
            assert_eq!(header.function_hue(Function::Mod), accent);
            // The cues: modulation's own colour, apart from the accent.
            assert_ne!(body.t.hue_mod, accent, "{key}: the cues");
            let gap = hue_gap(Hsl::of(body.t.hue_mod).h, Hsl::of(accent).h);
            assert!(
                gap >= MOD_GAP - 4.0,
                "{key}: the cues {gap:.0}° from the accent"
            );
            // Plain: as before.
            let plain = Style::new(base).function(Function::Mod);
            assert_eq!(plain.hue, base.hue_mod, "{key}: plain");
            assert_eq!(plain.hue_tok, Some(Tok::HueMod));
        }
    }

    /// The integrator's review of modulation: a knob's modulation rings are cues — in a face
    /// they never take the accent (the value arc's colour); plain, the theme's hues.
    #[test]
    fn modulation_rings_never_wear_the_accent() {
        use super::super::tokens::Function;
        for (key, make) in crate::theme::PRESETS {
            let base = make().kit_tokens();
            for h in (0..360).step_by(30) {
                let skin: &'static Skin = Box::leak(Box::new(Skin {
                    accent: Some(Pair::both(Hsl {
                        h: h as f32,
                        s: 0.75,
                        l: if base.dark { 0.6 } else { 0.4 },
                    })),
                    ..Skin::plain("test.rings")
                }));
                let s = Style::new(base).skinned(skin);
                for f in Function::ALL {
                    let c = cue_hue(&s, f);
                    let gap = hue_gap(Hsl::of(c).h, Hsl::of(s.t.hue_tone).h);
                    assert!(
                        Hsl::of(c).s < 0.15 || gap >= MOD_GAP - 6.0,
                        "{key} accent {h}: a {f:?} ring {gap:.0}° from the accent"
                    );
                    assert!(
                        super::super::color::contrast(c, s.t.deck) >= 2.99,
                        "{key} accent {h}: a {f:?} ring unreadable on the deck"
                    );
                }
            }
            let plain = Style::new(base);
            for f in Function::ALL {
                assert_eq!(cue_hue(&plain, f), base.hue(f), "{key}: plain");
            }
        }
    }

    /// The integrator's review: the picker's thumbnails are keyed by a digest of the sheets,
    /// and `Debug` of a unit's own painter printed its address — a new digest every launch.
    /// No sheet's stable text holds an address, a custom painter's included.
    #[test]
    fn no_sheet_s_digest_text_holds_an_address() {
        fn knob(_: &mut Brush, _: Pos2, _: &Style, _: State, _: &KnobLook<'_>) {}
        fn switch(_: &mut Brush, _: Rect, _: &Style, _: State) {}
        static CUSTOM: Skin = Skin {
            knob: KnobShape::Custom(knob),
            switch: SwitchShape::Custom(switch),
            ..Skin::plain("test.custom")
        };
        let mut all: Vec<&Skin> = Unit::all().into_iter().map(of).collect();
        all.push(&CUSTOM);
        for s in all {
            let text = stable(s);
            assert!(!text.contains("0x"), "{}: {text}", s.key);
        }
        assert!(stable(&CUSTOM).contains("knob=custom switch=custom"));
    }

    /// The digest is the same in another process of this binary (ASLR moves code between
    /// launches on Linux and macOS; a digest that held an address would differ).
    #[test]
    fn the_sheets_digest_is_the_same_in_every_process() {
        const CHILD: &str = "CANTUS_SHEETS_DIGEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            println!("DIGEST={:016x}", sheets_digest());
            return;
        }
        let run = || {
            let exe = std::env::current_exe().expect("the test binary");
            let out = std::process::Command::new(exe)
                .args([
                    "--exact",
                    "kit::skin::tests::the_sheets_digest_is_the_same_in_every_process",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, "1")
                .output()
                .expect("a child process");
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.lines()
                .find_map(|l| l.split("DIGEST=").nth(1).map(|d| d.trim().to_owned()))
                .unwrap_or_else(|| panic!("no digest from the child: {text}"))
        };
        let (a, b) = (run(), run());
        assert_eq!(a, b, "two processes, two digests");
        assert_eq!(
            a,
            format!("{:016x}", sheets_digest()),
            "the child and this process"
        );
    }

    /// The integrator's review of delivery 2: the cues read the installed theme's hues, not
    /// the style's. Each preset's own function hues come through skinning: a cue whose
    /// theme hue is far from the accent is that preset's hue, in every preset.
    #[test]
    fn cues_are_the_style_s_own_theme_s_hues_in_every_preset() {
        use super::super::tokens::Function;
        let mut bad = Vec::new();
        for (key, make) in crate::theme::PRESETS {
            let base = make().kit_tokens();
            for h in (0..360).step_by(30) {
                let skin: &'static Skin = Box::leak(Box::new(Skin {
                    accent: Some(Pair::both(Hsl {
                        h: h as f32,
                        s: 0.75,
                        l: if base.dark { 0.6 } else { 0.4 },
                    })),
                    ..Skin::plain("test.cues")
                }));
                let s = Style::new(base).skinned(skin);
                // Worn, too: the shell's path.
                let worn = {
                    let _on = wear(skin);
                    worn_style(base)
                };
                for f in Function::ALL {
                    if f == Function::Mod {
                        continue;
                    }
                    let own = Hsl::of(base.hue(f));
                    if own.s < 0.15 || hue_gap(own.h, Hsl::of(s.t.hue_tone).h) < MOD_GAP + 4.0 {
                        continue;
                    }
                    for (path, st) in [("skinned", s), ("worn", worn)] {
                        let got = Hsl::of(cue_hue(&st, f)).h;
                        if hue_gap(got, own.h) > 3.0 {
                            bad.push(format!(
                                "{key} accent {h} {path}: {f:?} cue {got:.0}°, the preset's {:.0}°",
                                own.h
                            ));
                        }
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} failures: {:#?}",
            bad.len(),
            &bad[..bad.len().min(12)]
        );
    }

    /// The integrator's review, 4: a material's light and shade never take text, dim text,
    /// labels or the floored hues below 4.5:1 on a card or the header — every material,
    /// alone and over a tint with an accent, in every preset.
    #[test]
    fn every_material_keeps_text_and_hues_legible_on_its_darkest_and_lightest() {
        let mut bad = Vec::new();
        for m in Material::ALL {
            for tinted in [false, true] {
                let skin: &'static Skin = Box::leak(Box::new(Skin {
                    material: m,
                    surface: Tint {
                        hue: 30.0,
                        sat: 0.4,
                        amount: if tinted { 0.7 } else { 0.0 },
                    },
                    accent: tinted.then_some(Pair::both(Hsl {
                        h: 50.0,
                        s: 0.9,
                        l: 0.75,
                    })),
                    ..Skin::plain("test.material")
                }));
                for (key, make) in crate::theme::PRESETS {
                    let t = skin.tokens(&make().kit_tokens());
                    for (what, got) in material_failures(&t, m) {
                        bad.push(format!("{key} tinted {tinted}: {what} at {got:.2}"));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} failures: {:#?}",
            bad.len(),
            &bad[..bad.len().min(12)]
        );
    }

    #[test]
    fn the_worn_skin_is_what_the_current_style_wears_and_comes_off() {
        static TINTED: Skin = Skin {
            surface: Tint {
                hue: 300.0,
                sat: 0.5,
                amount: 1.0,
            },
            ..Skin::plain("test.worn")
        };
        assert_eq!(Style::current().skin.key, "plain");
        {
            let _on = wear(&TINTED);
            let s = Style::current();
            assert_eq!(s.skin.key, "test.worn");
            assert_eq!(
                s.t.deck,
                TINTED.tokens(&super::super::tokens::current()).deck
            );
            // Skinning again lays nothing twice.
            assert_eq!(s.skinned(&TINTED), s);
        }
        assert_eq!(Style::current().skin.key, "plain");
    }
}
