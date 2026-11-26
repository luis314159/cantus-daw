//! Light and dark, and everything a user can change about the look.
//!
//! - A **mode**: Dark, Light, or Follow the system.
//! - Each mode has a **slot**: its base theme (a shipped preset or one of the user's own
//!   themes — any of them can serve either mode) and the user's changes on top of it:
//!   recoloured tokens by stable key, the FL-style knobs, the form (depth, roundness,
//!   density, text size). Changes live in the slot, so switching modes, or giving a mode
//!   another base theme, never loses them.
//! - **Reset** a token, a group, or everything; **A/B**: compare the slot with a copy of
//!   itself and keep either; **save, duplicate, delete** the user's own themes;
//!   **share** one as JSON and import it back.
//! - **Old files keep loading**: a `theme.json` from before (a bare [`Theme`]) becomes a
//!   user theme, assigned to the mode it was made for.
//! - **The icon's colour** (ADR 0046): the window's and the taskbar's mark in its designed
//!   gradient, the theme's accent, a preset, or a colour of the user's ([`IconColour`]).
//!
//! The settings UI that drives this is ADR 0024 Phase B; the data and the rules are here.

use super::logo::LogoChoice;
use super::tokens::ThemeKnobs;
use super::tokens::{Group, KitMap, Tok};
use crate::theme::{self, Theme};
use egui::Color32;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Which theme the interface shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Mode {
    /// Slate Lapis Night unless the user chose another dark theme (ADR 0111, amended
    /// 2026-11-18): the default until 2026-11-20.
    Dark,
    Light,
    /// Dark or light, as the operating system is: **the default** (ADR 0111, amended
    /// 2026-11-20 — light unless the system is dark, as the AppImage catalogue's review
    /// asked and the owner accepted), Slate Lapis Paper on a light system and Slate Lapis
    /// Night on a dark one.
    #[default]
    System,
}

/// A base theme: `preset:<id>` (one of [`PRESETS`]) or `user:<name>`.
pub type BaseRef = String;

/// A shipped preset: its id and its constructor.
pub type PresetEntry = (&'static str, fn() -> Theme);

/// The shipped presets, by the id a [`BaseRef`] names them with, in the order the
/// Appearance page lists them (round 39, ADR 0111): Slate Lapis, the default (amended
/// 2026-11-18), then Slate, then the two optional families; the themes of rounds 24–38,
/// redrawn in round 39's design system and
/// under their old ids (`slate` is Pewter and `paper` is Chalk now, the FL-derived pair
/// renamed); the themes new in round 39 and round 42's Violet Paper; the high-contrast pair.
pub const PRESETS: [PresetEntry; 25] = [
    ("slate_lapis_night", theme::slate_lapis_night),
    ("slate_lapis_paper", theme::slate_lapis_paper),
    ("slate_night", theme::slate_night),
    ("slate_paper", theme::slate_paper),
    ("graphite_night", theme::graphite_night),
    ("graphite_paper", theme::graphite_paper),
    ("studio_ink_night", theme::studio_ink_night),
    ("studio_ink_paper", theme::studio_ink_paper),
    ("cantus_night", crate::theme_redrawn::cantus_night),
    ("cantus_paper", crate::theme_redrawn::cantus_paper),
    ("blue_hour", crate::theme_redrawn::blue_hour),
    ("daylight", crate::theme_redrawn::daylight),
    ("slate", crate::theme_redrawn::pewter),
    ("paper", crate::theme_redrawn::chalk),
    ("ink_gold", crate::theme_redrawn::ink_and_gold),
    ("parchment", crate::theme_redrawn::parchment),
    ("ultraviolet", crate::theme_redrawn::ultraviolet),
    ("lagoon", crate::theme_redrawn::lagoon),
    ("evergreen_night", crate::theme_added::evergreen_night),
    ("evergreen_paper", crate::theme_added::evergreen_paper),
    ("bordeaux", crate::theme_added::bordeaux),
    ("print", crate::theme_added::print),
    ("violet_paper", crate::theme_added::violet_paper),
    ("contrast", theme::high_contrast),
    ("contrast_light", theme::high_contrast_light),
];

/// The presets of rounds 24–38 as those releases wrote them (Classic: raised keys, the
/// gradients), by the id they had and have again. Named nowhere in the interface: an old
/// `theme.json` that is exactly one of them is that preset chosen, not a theme of the
/// user's ([`Appearance::from_legacy`]), and shows the preset of its id, redrawn.
///
/// Round 39 first retired these ten and showed Slate for a setting that named one; the
/// owner asked for them back (2026-11-10), so every id names its own theme again and no
/// setting is moved.
pub const CLASSIC: [PresetEntry; 10] = [
    ("cantus_night", theme::cantus_night),
    ("cantus_paper", theme::cantus_paper),
    ("daylight", theme::daylight),
    ("blue_hour", theme::blue_hour),
    ("ultraviolet", theme::ultraviolet),
    ("lagoon", theme::lagoon),
    ("slate", theme::slate),
    ("ink_gold", theme::ink_and_gold),
    ("paper", theme::paper),
    ("parchment", theme::parchment),
];

/// The preset a fresh install shows in each mode: Slate Lapis Night and Slate Lapis Paper
/// (ADR 0111, amended 2026-11-18; round 39's were Slate Night and Slate Paper).
pub const DEFAULT_DARK: &str = "slate_lapis_night";
pub const DEFAULT_LIGHT: &str = "slate_lapis_paper";

/// The file format [`Appearance`] writes. Version 3 (2026-11-18) records, per mode, whether
/// the user ever chose its theme ([`Slot::follows_default`]); version 4 (2026-11-20) is the
/// one whose default mode follows the system ([`Mode::System`]): a file before it that
/// shows the old default, Dark, without a theme of the user's in either mode, follows the
/// system from now on ([`Appearance::from_json`]).
pub const VERSION: u32 = 4;

/// The presets each version of the file shipped as the defaults of its modes (dark, light):
/// what an untouched mode of a file of that version names. Version 2 is round 39's file
/// (beta.14–15, Slate); a version-2 file of the releases before round 39 names their
/// default (Cantus Night) and was kept as a choice when round 39 changed the default, as it
/// is now. The last row is today's.
pub const DEFAULTS_BY_VERSION: [(u32, &str, &str); 3] = [
    (2, "slate_night", "slate_paper"),
    (3, "slate_lapis_night", "slate_lapis_paper"),
    (VERSION, DEFAULT_DARK, DEFAULT_LIGHT),
];

/// The shipped default of `mode` in a file of `version`, as a base reference — `None` for a
/// version no row of [`DEFAULTS_BY_VERSION`] names (one before round 39, or one a newer
/// release wrote).
fn default_base_of(version: u32, mode: Mode) -> Option<String> {
    DEFAULTS_BY_VERSION
        .iter()
        .find(|(v, _, _)| *v == version)
        .map(|(_, dark, light)| {
            let id = if mode == Mode::Dark { dark } else { light };
            format!("preset:{id}")
        })
}

/// The preset `mode` shows until the user chooses another.
pub fn default_base(mode: Mode) -> BaseRef {
    format!(
        "preset:{}",
        if mode == Mode::Dark {
            DEFAULT_DARK
        } else {
            DEFAULT_LIGHT
        }
    )
}

/// Whether `id` names a shipped preset.
pub fn is_preset(id: &str) -> bool {
    PRESETS.iter().any(|(k, _)| *k == id)
}

/// The i18n key of a preset's name (`theme.<id>`).
pub fn preset_name_key(id: &str) -> String {
    format!("theme.{id}")
}

/// What one mode shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub base: BaseRef,
    /// Kit tokens the user recoloured, by stable key.
    pub overrides: KitMap,
    /// Colours of today's views (the legacy palette) the user changed, by field name.
    pub legacy: BTreeMap<String, String>,
    /// FL's theme knobs; `None` keeps the base theme's.
    pub knobs: Option<ThemeKnobs>,
    /// Depth, roundness, density, text size; `None` keeps the base theme's.
    pub form: Option<super::tokens::Form>,
    /// Whether the user never chose this mode's theme: its base is the shipped default, and
    /// a release with another default shows that one instead (ADR 0111, amendment of
    /// 2026-11-18; [`Appearance::from_json`]). Choosing a theme for the mode clears it.
    pub follows_default: bool,
}

impl Default for Slot {
    fn default() -> Self {
        Slot::shipped(Mode::Dark)
    }
}

impl Slot {
    /// A mode showing preset `id`, chosen.
    pub fn preset(id: &str) -> Slot {
        Slot {
            base: format!("preset:{id}"),
            overrides: KitMap::new(),
            legacy: BTreeMap::new(),
            knobs: None,
            form: None,
            follows_default: false,
        }
    }

    /// A mode as a fresh install has it: the shipped default, never chosen.
    pub fn shipped(mode: Mode) -> Slot {
        Slot {
            base: default_base(mode),
            follows_default: true,
            ..Slot::preset("")
        }
    }

    /// Whether the user changed anything on top of the base theme.
    pub fn is_pristine(&self) -> bool {
        self.overrides.is_empty()
            && self.legacy.is_empty()
            && self.knobs.is_none()
            && self.form.is_none()
    }
}

/// The two settings being compared, and which is showing.
#[derive(Clone, Debug, PartialEq)]
pub struct Compare {
    pub mode: Mode,
    /// The setting not showing.
    pub other: Slot,
    /// Whether B (the copy being edited) is the one showing.
    pub showing_b: bool,
}

/// The look, as the user set it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// The file format; files without it are a bare theme from before.
    pub version: u32,
    pub mode: Mode,
    pub dark: Slot,
    pub light: Slot,
    pub logo: LogoChoice,
    /// The colour of the window's and the taskbar's icon. Absent in files written before
    /// round 25: the mark as designed.
    pub icon: IconColour,
    /// The user's own themes.
    pub themes: Vec<Theme>,
    /// The cap every fader wears (round 37): the theme's own in files written before it,
    /// and in those that said `Classic` before the theme's own was a choice (beta.14).
    pub fader: super::fader::FaderCap,
    #[serde(skip)]
    pub compare: Option<Compare>,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            version: VERSION,
            mode: Mode::default(),
            dark: Slot::shipped(Mode::Dark),
            light: Slot::shipped(Mode::Light),
            logo: LogoChoice::default(),
            icon: IconColour::default(),
            themes: Vec::new(),
            fader: super::fader::FaderCap::Theme,
            compare: None,
        }
    }
}

impl Appearance {
    /// The mode actually shown: Dark or Light.
    pub fn effective(&self, system_dark: bool) -> Mode {
        match self.mode {
            Mode::System if system_dark => Mode::Dark,
            Mode::System => Mode::Light,
            m => m,
        }
    }

    pub fn slot(&self, mode: Mode) -> &Slot {
        match mode {
            Mode::Dark => &self.dark,
            _ => &self.light,
        }
    }

    pub fn slot_mut(&mut self, mode: Mode) -> &mut Slot {
        match mode {
            Mode::Dark => &mut self.dark,
            _ => &mut self.light,
        }
    }

    /// The theme a base reference names: a preset, a user theme, or — when neither
    /// exists any more — the mode's shipped default.
    pub fn base_theme(&self, base: &str, mode: Mode) -> Theme {
        if let Some(id) = base.strip_prefix("preset:") {
            if let Some((_, make)) = PRESETS.iter().find(|(k, _)| *k == id) {
                return make();
            }
        }
        if let Some(name) = base.strip_prefix("user:") {
            if let Some(t) = self.themes.iter().find(|t| t.name == name) {
                return t.clone();
            }
        }
        if mode == Mode::Dark {
            theme::default_theme()
        } else {
            theme::default_light_theme()
        }
    }

    /// The concrete theme `mode` shows: its base, with the slot's changes on top.
    pub fn resolve_mode(&self, mode: Mode) -> Theme {
        let slot = self.slot(mode);
        let mut t = self.base_theme(&slot.base, mode);
        if !slot.legacy.is_empty() {
            if let Ok(mut v) = serde_json::to_value(&t) {
                if let Some(obj) = v.as_object_mut() {
                    for (k, hex) in &slot.legacy {
                        if theme::parse_hex(hex).is_some()
                            && obj.get(k).is_some_and(|x| x.is_string())
                        {
                            obj.insert(k.clone(), serde_json::Value::String(hex.clone()));
                        }
                    }
                }
                if let Ok(back) = serde_json::from_value::<Theme>(v) {
                    t = back;
                }
            }
        }
        if t.kit.is_empty() {
            // A theme from before the kit: write its derived tokens out, so an override
            // lands on a complete set.
            t.kit = super::tokens::to_map(&t.kit_tokens());
        }
        for (k, v) in &slot.overrides {
            t.kit.insert(k.clone(), v.clone());
        }
        if let Some(k) = slot.knobs {
            t.knobs = k;
        }
        if let Some(f) = slot.form {
            t.form = f;
        }
        t.logo = self.logo;
        t
    }

    /// The theme to show now.
    pub fn resolve(&self, system_dark: bool) -> Theme {
        self.resolve_mode(self.effective(system_dark))
    }

    /// Recolour a token in `mode`.
    pub fn set_token(&mut self, mode: Mode, tok: Tok, c: Color32) {
        self.slot_mut(mode)
            .overrides
            .insert(tok.key().to_string(), theme::to_hex(c));
    }

    /// Give a token back its base theme's colour.
    pub fn reset_token(&mut self, mode: Mode, tok: Tok) {
        self.slot_mut(mode).overrides.remove(tok.key());
    }

    /// Give a whole group back its base theme's colours.
    pub fn reset_group(&mut self, mode: Mode, group: Group) {
        let slot = self.slot_mut(mode);
        for t in group.tokens() {
            slot.overrides.remove(t.key());
        }
    }

    /// Drop every change in `mode`: the base theme as shipped.
    pub fn reset_all(&mut self, mode: Mode) {
        let slot = self.slot(mode);
        let (base, follows_default) = (slot.base.clone(), slot.follows_default);
        *self.slot_mut(mode) = Slot {
            base,
            follows_default,
            ..Slot::preset("paper")
        };
    }

    /// Give `mode` another base theme — the user's choice, kept whatever a later release
    /// makes the default. The mode's own changes stay.
    pub fn assign(&mut self, mode: Mode, base: BaseRef) {
        let slot = self.slot_mut(mode);
        slot.base = base;
        slot.follows_default = false;
    }

    /// Start comparing `mode`'s setting (A) with a copy of it (B), which is then shown and
    /// edited.
    pub fn start_compare(&mut self, mode: Mode) {
        let a = self.slot(mode).clone();
        self.compare = Some(Compare {
            mode,
            other: a,
            showing_b: true,
        });
    }

    /// Show the other of A and B.
    pub fn toggle_compare(&mut self) {
        if let Some(mut c) = self.compare.take() {
            let showing = self.slot(c.mode).clone();
            *self.slot_mut(c.mode) = std::mem::replace(&mut c.other, showing);
            c.showing_b = !c.showing_b;
            self.compare = Some(c);
        }
    }

    /// Stop comparing and keep what is showing.
    pub fn end_compare(&mut self) {
        self.compare = None;
    }

    /// Save what `mode` shows as a user theme called `name` (replacing one of that
    /// name), and make it the mode's base with no changes on top.
    pub fn save_as(&mut self, mode: Mode, name: &str) -> Theme {
        let mut t = self.resolve_mode(mode);
        t.name = name.to_string();
        self.put(t.clone());
        *self.slot_mut(mode) = Slot {
            base: format!("user:{name}"),
            ..Slot::preset("paper")
        };
        t
    }

    fn put(&mut self, t: Theme) {
        match self.themes.iter_mut().find(|x| x.name == t.name) {
            Some(x) => *x = t,
            None => self.themes.push(t),
        }
    }

    /// Copy the user theme or preset `base` as a new user theme `name`.
    pub fn duplicate(&mut self, base: &str, name: &str) -> Theme {
        let mut t = self.base_theme(base, Mode::Light);
        t.name = name.to_string();
        self.put(t.clone());
        t
    }

    /// Delete a user theme. A mode that was based on it falls back to its default preset,
    /// and follows the default again.
    pub fn delete(&mut self, name: &str) {
        self.themes.retain(|t| t.name != name);
        let gone = format!("user:{name}");
        for mode in [Mode::Dark, Mode::Light] {
            if self.slot(mode).base == gone {
                let slot = self.slot_mut(mode);
                slot.base = default_base(mode);
                slot.follows_default = true;
            }
        }
    }

    /// Import a shared theme (this format or any older theme file) as a user theme,
    /// renamed if the name is taken. Returns its name.
    pub fn import(&mut self, json: &str) -> Result<String, String> {
        let mut t = import_theme(json)?;
        let base = t.name.clone();
        let mut n = 2;
        while self.themes.iter().any(|x| x.name == t.name) {
            t.name = format!("{base} {n}");
            n += 1;
        }
        let name = t.name.clone();
        self.themes.push(t);
        Ok(name)
    }

    /// Read an appearance file — or a `theme.json` from before modes existed, which
    /// becomes a user theme assigned to the mode it was made for (and that mode shown).
    ///
    /// **What counts as a choice** (ADR 0111, amendment of 2026-11-18). The file keeps each
    /// mode's theme by value, and it is written whenever anything on the look changes (the
    /// mode, the icon, the faders' cap…), so naming the default is not choosing it. A mode
    /// *never chose* its theme when it is untouched (no colour, knob or form of the user's
    /// on top) and names the default of the release that wrote the file, and — from
    /// version 3, which records it — no theme was ever picked for it. Such a mode shows
    /// today's default. Anything else is the user's choice, kept as it is: another preset,
    /// one of their own themes, a theme with their changes on top, a base edited by hand,
    /// and, from version 3, the default itself picked on the Appearance page. A version-2
    /// file (beta.14–15) cannot tell an untouched Slate from Slate picked again, so its
    /// untouched Slate Night and Slate Paper count as never chosen.
    pub fn from_json(text: &str) -> Result<Appearance, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if let Some(version) = v.get("version").and_then(|x| x.as_u64()) {
            let mut a: Appearance = serde_json::from_value(v).map_err(|e| e.to_string())?;
            a.settle(version as u32);
            return Ok(a);
        }
        if v.get("version").is_some() {
            return serde_json::from_value(v).map_err(|e| e.to_string());
        }
        let t: Theme = serde_json::from_value(v).map_err(|e| e.to_string())?;
        Ok(Appearance::from_legacy(t))
    }

    /// Bring a file of `version` to today's: each mode that never chose its theme (see
    /// [`Appearance::from_json`]) shows today's default; every other mode is a choice.
    fn settle(&mut self, version: u32) {
        for mode in [Mode::Dark, Mode::Light] {
            let then = default_base_of(version, mode);
            let slot = self.slot_mut(mode);
            // Version 2 has no record of a choice; from version 3 the file says.
            let unpicked = version < 3 || slot.follows_default;
            let never_chose =
                unpicked && slot.is_pristine() && then.is_some_and(|b| b == slot.base);
            if never_chose {
                slot.base = default_base(mode);
            }
            if version < 3 {
                slot.follows_default = never_chose;
            }
        }
        // Before version 4 the default mode was Dark, and a file says nothing of whether the
        // user picked it: a Dark with neither mode's theme ever chosen or changed is the old
        // default, and follows the system as a fresh install does (ADR 0111, amended
        // 2026-11-20). Light or System, or a theme chosen or changed in either mode, is a
        // choice and stays.
        let untouched = |s: &Slot| s.follows_default && s.is_pristine();
        if version < 4 && self.mode == Mode::Dark && untouched(&self.dark) && untouched(&self.light)
        {
            self.mode = Mode::System;
        }
        if version <= VERSION {
            self.version = VERSION;
        }
    }

    /// An appearance around an old theme file.
    pub fn from_legacy(t: Theme) -> Appearance {
        let mut a = Appearance::default();
        // A `theme.json` that is exactly one of the old presets (what earlier releases
        // wrote for a preset chosen) is no theme of the user's: it is that preset chosen,
        // and shows it, redrawn (round 39), in its own mode. Paper, which every release of
        // that era wrote on its first run before anyone chose anything, leaves the
        // appearance as a fresh install has it (ADR 0046).
        let classic = CLASSIC
            .iter()
            .find(|(_, make)| t.knobs.is_neutral() && make().palette() == t.palette());
        if let Some((id, _)) = classic {
            a.logo = t.logo;
            if *id != "paper" {
                let mode = if t.dark { Mode::Dark } else { Mode::Light };
                a.assign(mode, format!("preset:{id}"));
                a.mode = mode;
            }
            return a;
        }
        let mode = if t.dark { Mode::Dark } else { Mode::Light };
        let name = if t.name.trim().is_empty() {
            "My theme".to_string()
        } else {
            t.name.clone()
        };
        a.logo = t.logo;
        a.themes.push(Theme {
            name: name.clone(),
            ..t
        });
        a.assign(mode, format!("user:{name}"));
        a.mode = mode;
        a
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// The colour of the window's and the taskbar's icon (ADR 0046): the website's mark, no
/// tile behind it, in its designed gradient unless the user picks another.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum IconColour {
    /// As designed on a dark taskbar, and its deep copper twin on a light one, following
    /// Windows' setting as it changes (the default; the designed pale amber is 1.4:1 on a
    /// white taskbar).
    #[default]
    Auto,
    /// Amber → copper, as designed, whatever the taskbar.
    Original,
    /// The theme's own accent (its brand colour), following every theme change.
    Theme,
    /// One of the shipped colours.
    Preset(IconPreset),
    /// A colour of the user's (sRGB), made into a gradient the way the presets are.
    Custom([u8; 3]),
}

/// The icon's shipped colours: a few that stand out on a dark taskbar, a few on a light
/// one (Graphite is the one for a white taskbar).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconPreset {
    Amber,
    Copper,
    Verdigris,
    Blue,
    Violet,
    Rose,
    Silver,
    Graphite,
}

impl IconPreset {
    pub const ALL: [IconPreset; 8] = [
        IconPreset::Amber,
        IconPreset::Copper,
        IconPreset::Verdigris,
        IconPreset::Blue,
        IconPreset::Violet,
        IconPreset::Rose,
        IconPreset::Silver,
        IconPreset::Graphite,
    ];

    /// The colour the gradient is made from.
    pub fn rgb(self) -> [u8; 3] {
        match self {
            IconPreset::Amber => [0xf5, 0xad, 0x3d],
            IconPreset::Copper => [0xd4, 0x62, 0x2a],
            IconPreset::Verdigris => [0x3e, 0x8e, 0x7e],
            IconPreset::Blue => [0x2f, 0x7c, 0xf6],
            IconPreset::Violet => [0x9b, 0x6b, 0xff],
            IconPreset::Rose => [0xe8, 0x50, 0x7a],
            IconPreset::Silver => [0xe4, 0xe6, 0xea],
            IconPreset::Graphite => [0x2b, 0x30, 0x38],
        }
    }

    /// Its name's i18n key.
    pub fn name_key(self) -> &'static str {
        match self {
            IconPreset::Amber => "look.icon.amber",
            IconPreset::Copper => "look.icon.copper",
            IconPreset::Verdigris => "look.icon.verdigris",
            IconPreset::Blue => "look.icon.blue",
            IconPreset::Violet => "look.icon.violet",
            IconPreset::Rose => "look.icon.rose",
            IconPreset::Silver => "look.icon.silver",
            IconPreset::Graphite => "look.icon.graphite",
        }
    }
}

impl IconColour {
    /// The mark's colours for this choice, under the theme whose tokens are `t`, on a dark
    /// taskbar.
    pub fn mark(self, t: &super::tokens::Tokens) -> super::mark::MarkColours {
        self.mark_on(t, false)
    }

    /// The mark's colours for this choice, under the theme whose tokens are `t`, on a
    /// taskbar that is light or not (only [`IconColour::Auto`] looks).
    pub fn mark_on(
        self,
        t: &super::tokens::Tokens,
        light_taskbar: bool,
    ) -> super::mark::MarkColours {
        use super::mark::MarkColours;
        match self {
            IconColour::Auto if light_taskbar => MarkColours::ON_LIGHT,
            IconColour::Auto | IconColour::Original => MarkColours::ORIGINAL,
            IconColour::Theme => {
                let [r, g, b, _] = super::color::opaque(t.brand).to_array();
                MarkColours::from_one([r, g, b])
            }
            IconColour::Preset(p) => MarkColours::from_one(p.rgb()),
            IconColour::Custom(c) => MarkColours::from_one(c),
        }
    }
}

/// A colour of the icon's, as egui's.
pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Windows' own taskbar colours, dark and light, for the icon chips to stand on: the icon
/// is seen on the taskbar, not on the theme.
pub fn taskbar_previews() -> [Color32; 2] {
    [
        Color32::from_rgb(0x20, 0x20, 0x20),
        Color32::from_rgb(0xee, 0xee, 0xee),
    ]
}

/// A theme as JSON, to share.
pub fn export_theme(t: &Theme) -> String {
    serde_json::to_string_pretty(t).unwrap_or_default()
}

/// A shared theme back from JSON; themes written by any earlier version load (missing
/// tokens are derived, the knobs and form default).
pub fn import_theme(json: &str) -> Result<Theme, String> {
    let mut t: Theme = serde_json::from_str(json).map_err(|e| e.to_string())?;
    t.fill_missing();
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::tokens::Form;

    #[test]
    fn each_mode_has_its_base_and_follows_the_system() {
        let mut a = Appearance {
            mode: Mode::System,
            ..Default::default()
        };
        assert_eq!(a.resolve(true).name, "Slate Lapis Night");
        assert_eq!(a.resolve(false).name, "Slate Lapis Paper");
        a.mode = Mode::Dark;
        assert_eq!(a.resolve(false).name, "Slate Lapis Night");
        // Any preset may serve either mode.
        a.assign(Mode::Light, "preset:graphite_night".into());
        assert_eq!(a.resolve_mode(Mode::Light).name, "Graphite Night");
    }

    #[test]
    fn switching_modes_never_loses_overrides() {
        let mut a = Appearance::default();
        let red = Color32::from_rgb(200, 30, 40);
        a.set_token(Mode::Dark, Tok::Cap, red);
        a.mode = Mode::Light;
        a.set_token(Mode::Light, Tok::Cap, Color32::from_rgb(1, 2, 3));
        a.assign(Mode::Dark, "preset:ink_gold".into());
        a.mode = Mode::Dark;
        let t = a.resolve(false).kit_tokens();
        assert_eq!(
            t.cap, red,
            "the dark mode's override survived a mode switch and a new base"
        );
        let l = a.resolve_mode(Mode::Light).kit_tokens();
        assert_eq!(l.cap, Color32::from_rgb(1, 2, 3));
    }

    #[test]
    fn reset_by_token_and_by_group() {
        let mut a = Appearance::default();
        let c = Color32::from_rgb(9, 9, 9);
        a.set_token(Mode::Dark, Tok::MeterLow, c);
        a.set_token(Mode::Dark, Tok::MeterClip, c);
        a.set_token(Mode::Dark, Tok::Cap, c);
        a.reset_token(Mode::Dark, Tok::Cap);
        assert!(!a.dark.overrides.contains_key(Tok::Cap.key()));
        a.reset_group(Mode::Dark, Group::Meters);
        assert!(a.dark.overrides.is_empty());
        a.dark.form = Some(Form {
            depth: 0.2,
            ..Form::default()
        });
        a.reset_all(Mode::Dark);
        assert!(a.dark.is_pristine());
        assert_eq!(a.dark.base, "preset:slate_lapis_night");
    }

    #[test]
    fn a_b_compare_swaps_and_keeps() {
        let mut a = Appearance::default();
        a.start_compare(Mode::Dark);
        a.set_token(Mode::Dark, Tok::Selection, Color32::from_rgb(0, 200, 255));
        let b = a.dark.clone();
        a.toggle_compare();
        assert!(
            a.dark.overrides.is_empty(),
            "A is the setting before the change"
        );
        a.toggle_compare();
        assert_eq!(a.dark, b);
        a.end_compare();
        assert_eq!(a.dark, b);
    }

    #[test]
    fn save_duplicate_delete_share() {
        let mut a = Appearance::default();
        a.dark.knobs = Some(ThemeKnobs {
            hue: 40.0,
            ..Default::default()
        });
        let saved = a.save_as(Mode::Dark, "Warm slate");
        assert_eq!(a.dark.base, "user:Warm slate");
        assert!(a.dark.is_pristine());
        assert_eq!(a.resolve_mode(Mode::Dark), saved);
        let json = export_theme(&saved);
        let name = a.import(&json).unwrap();
        assert_eq!(name, "Warm slate 2");
        a.duplicate("preset:slate_paper", "My paper");
        assert!(a.themes.iter().any(|t| t.name == "My paper"));
        a.delete("Warm slate");
        assert_eq!(
            a.dark.base, "preset:slate_lapis_night",
            "a mode loses a deleted base gracefully"
        );
    }

    #[test]
    fn an_old_theme_json_keeps_loading() {
        // A file written before the kit, the knobs and the modes: the 22 colours only —
        // a theme of the user's own (one colour changed from a preset; an untouched preset
        // is that preset chosen).
        let mut mine = theme::ink_and_gold();
        mine.panel = "#20232b".into();
        let mut v = serde_json::to_value(&mine).unwrap();
        for k in ["kit", "knobs", "form", "logo"] {
            v.as_object_mut().unwrap().remove(k);
        }
        let old = serde_json::to_string(&v).unwrap();
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(a.mode, Mode::Dark);
        let t = a.resolve(false);
        assert_eq!(t.palette(), mine.palette());
        // Its kit tokens are derived from its colours, and legible.
        assert!(t.kit_tokens().legibility_failures().is_empty());
        // And an appearance file round-trips.
        let back = Appearance::from_json(&a.to_json()).unwrap();
        assert_eq!(back, a);
        // An old theme also imports as a shared theme.
        assert!(import_theme(&old).is_ok());
    }

    #[test]
    fn the_fader_cap_is_the_themes_in_old_files_and_survives_a_round_trip() {
        use super::super::fader::FaderCap;
        // A file from before round 37 has no `fader`: the theme's own cap.
        let old = Appearance {
            mode: Mode::Light,
            ..Default::default()
        };
        let json = old.to_json().replace("\"fader\"", "\"fader_unknown\"");
        let back = Appearance::from_json(&json).unwrap();
        assert_eq!(back.fader, FaderCap::Theme);
        // beta.14 wrote `Classic` for the default, which every round-39 look drew as its
        // own cap: it reads as the theme's.
        let beta14 = old
            .to_json()
            .replace("\"fader\": \"Theme\"", "\"fader\": \"Classic\"");
        assert!(beta14.contains("\"Classic\""), "{beta14}");
        assert_eq!(
            Appearance::from_json(&beta14).unwrap().fader,
            FaderCap::Theme
        );
        for c in FaderCap::ALL {
            let a = Appearance {
                fader: c,
                ..Default::default()
            };
            let back = Appearance::from_json(&a.to_json()).unwrap();
            assert_eq!(back.fader, c);
        }
    }

    #[test]
    fn the_form_and_logo_reach_the_tokens() {
        let mut a = Appearance {
            mode: Mode::Dark,
            ..Default::default()
        };
        a.dark.form = Some(Form {
            depth: 0.0,
            roundness: 2.0,
            density: crate::kit::tokens::Density::Compact,
            text_size: 1.2,
        });
        a.logo = LogoChoice {
            number: 7,
            tint: crate::kit::logo::Tint::Theme,
        };
        let t = a.resolve(false);
        let k = t.kit_tokens();
        assert_eq!(k.depth, 0.0);
        // Slate's 4-point corners, twice as round.
        assert_eq!(k.radius, 8.0);
        assert!(k.density < 1.0 && (k.text_scale - 1.2).abs() < 1e-6);
        assert_eq!(t.logo.number, 7);
    }

    #[test]
    fn a_fresh_install_shows_slate_lapis_and_an_old_choice_shows_its_theme() {
        let a = Appearance::default();
        // The mode follows the system (ADR 0111, amended 2026-11-20).
        assert_eq!(a.mode, Mode::System);
        assert_eq!(a.dark.base, "preset:slate_lapis_night");
        assert_eq!(a.light.base, "preset:slate_lapis_paper");
        // A file a user saved before round 39 names a theme of rounds 24–38 (here round 24's
        // FL-derived Slate and Paper): it shows that theme, redrawn — Pewter and Chalk.
        let mut old = serde_json::to_value(Appearance::default()).unwrap();
        let o = old.as_object_mut().unwrap();
        o.insert(
            "dark".into(),
            serde_json::to_value(Slot::preset("slate")).unwrap(),
        );
        o.insert(
            "light".into(),
            serde_json::to_value(Slot::preset("paper")).unwrap(),
        );
        o.remove("icon");
        let back = Appearance::from_json(&old.to_string()).unwrap();
        assert_eq!(back.dark.base, "preset:slate");
        assert_eq!(back.resolve_mode(Mode::Dark).name, "Pewter");
        assert_eq!(back.resolve_mode(Mode::Light).name, "Chalk");
        // …and, written before the icon existed, the automatic mark (as designed on a dark
        // taskbar, which is where every earlier release's icon was seen).
        assert_eq!(back.icon, IconColour::Auto);
        // Every preset is reachable by its id, and named in both languages.
        for (id, make) in PRESETS {
            assert_eq!(a.base_theme(&format!("preset:{id}"), Mode::Dark), make());
            for lang in crate::i18n::Locale::ALL {
                crate::i18n::set_thread_locale(lang);
                let key = preset_name_key(id);
                assert_ne!(crate::i18n::t(&key), key, "{lang:?}: {id} has no name");
            }
        }
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
    }

    #[test]
    fn the_icon_colour_round_trips_and_makes_a_mark() {
        use crate::kit::mark::MarkColours;
        let t = crate::theme::daylight().kit_tokens();
        for c in [
            IconColour::Auto,
            IconColour::Original,
            IconColour::Theme,
            IconColour::Preset(IconPreset::Verdigris),
            IconColour::Custom([10, 200, 120]),
        ] {
            let a = Appearance {
                icon: c,
                ..Default::default()
            };
            let back = Appearance::from_json(&a.to_json()).unwrap();
            assert_eq!(back.icon, c);
        }
        assert_eq!(IconColour::Original.mark(&t), MarkColours::ORIGINAL);
        // The theme's accent: Daylight's blue lands in the middle of the wave.
        assert_eq!(IconColour::Theme.mark(&t).wave[1], [0x1f, 0x6f, 0xe5]);
        // Every preset is its own colour.
        let mut seen: Vec<MarkColours> = IconPreset::ALL
            .iter()
            .map(|p| IconColour::Preset(*p).mark(&t))
            .collect();
        seen.dedup();
        assert_eq!(seen.len(), IconPreset::ALL.len());
    }

    /// The default icon reads on either taskbar: the designed gradient on a dark one, its
    /// deep twin on a light one — every stroke at 3:1 or better (WCAG's floor for graphics),
    /// where the designed pale amber is 1.4:1 on white. Only the automatic choice looks at
    /// the taskbar.
    #[test]
    fn the_automatic_icon_reads_on_a_dark_and_a_light_taskbar() {
        use crate::kit::mark::MarkColours;
        assert_eq!(Appearance::default().icon, IconColour::Auto);
        let t = crate::theme::daylight().kit_tokens();
        assert_eq!(IconColour::Auto.mark_on(&t, false), MarkColours::ORIGINAL);
        assert_eq!(IconColour::Auto.mark_on(&t, true), MarkColours::ON_LIGHT);
        assert_eq!(
            IconColour::Original.mark_on(&t, true),
            MarkColours::ORIGINAL
        );
        assert_eq!(
            IconColour::Preset(IconPreset::Blue).mark_on(&t, true),
            IconColour::Preset(IconPreset::Blue).mark(&t)
        );
        let [dark, light] = taskbar_previews();
        let floor = |m: MarkColours, back: Color32, what: &str| {
            for c in m.wave.iter().chain([&m.playhead]) {
                let k = super::super::color::contrast(rgb(*c), back);
                assert!(k >= 3.0, "{what}: {c:02x?} is {k:.2}:1");
            }
        };
        floor(MarkColours::ON_LIGHT, light, "deep mark on a light taskbar");
        // Windows 11's light taskbar is a little lighter than the chips' preview.
        floor(
            MarkColours::ON_LIGHT,
            Color32::from_rgb(0xf3, 0xf3, 0xf3),
            "on #f3f3f3",
        );
        floor(
            MarkColours::ORIGINAL,
            dark,
            "designed mark on a dark taskbar",
        );
        // The reason for the twin: the designed gradient fails on a light taskbar.
        let pale = super::super::color::contrast(rgb(MarkColours::ORIGINAL.wave[0]), light);
        assert!(
            pale < 3.0,
            "the designed amber reads on white after all ({pale:.2}:1)"
        );
    }

    #[test]
    fn every_old_preset_id_names_its_own_theme_again_keeping_the_slot_s_changes() {
        // Round 39 retired the ten themes of rounds 24–38 and showed Slate for them; the
        // owner asked for them back (2026-11-10): each id names its theme, redrawn, in its
        // own light, and a mode's own changes stay on top.
        let mut names = std::collections::BTreeSet::new();
        for (id, classic) in CLASSIC {
            let (_, make) = PRESETS.iter().find(|(k, _)| *k == id).expect(id);
            let redrawn = make();
            assert_eq!(redrawn.dark, classic().dark, "{id} keeps its light");
            assert!(redrawn.system.recessed(), "{id} is of round 39's system");
            assert_ne!(redrawn.palette(), classic().palette(), "{id} is redrawn");
            assert!(!redrawn.name.starts_with("Slate"), "{id}: {}", redrawn.name);
            names.insert(redrawn.name.clone());
            for mode in [Mode::Dark, Mode::Light] {
                let mut a = Appearance::default();
                a.assign(mode, format!("preset:{id}"));
                a.set_token(mode, Tok::Cap, Color32::from_rgb(1, 2, 3));
                let back = Appearance::from_json(&a.to_json()).unwrap();
                assert_eq!(back.slot(mode).base, format!("preset:{id}"), "{id}");
                assert_eq!(
                    back.slot(mode)
                        .overrides
                        .get(Tok::Cap.key())
                        .map(String::as_str),
                    Some("#010203"),
                    "{id}: the mode's own changes stay"
                );
                assert_eq!(back.base_theme(&format!("preset:{id}"), mode), redrawn);
                assert_eq!(
                    back.resolve_mode(mode).kit_tokens().cap,
                    Color32::from_rgb(1, 2, 3)
                );
            }
        }
        assert_eq!(names.len(), CLASSIC.len(), "each its own name");
        // The FL-derived pair carry no product's name and do not collide with Slate.
        assert_eq!(crate::theme_redrawn::pewter().name, "Pewter");
        assert_eq!(crate::theme_redrawn::chalk().name, "Chalk");
        // A user's own theme is theirs, untouched.
        let mut a = Appearance::default();
        let mine = theme::cantus_night();
        a.themes.push(Theme {
            name: "Mine".into(),
            ..mine
        });
        a.assign(Mode::Dark, "user:Mine".into());
        let back = Appearance::from_json(&a.to_json()).unwrap();
        assert_eq!(back.dark.base, "user:Mine");
        assert_eq!(back.resolve_mode(Mode::Dark).name, "Mine");
        // A `theme.json` of the old days that is exactly an old dark preset: that preset,
        // redrawn, in the dark mode; a light one in the light mode.
        let old = serde_json::to_string(&theme::lagoon()).unwrap();
        let a = Appearance::from_json(&old).unwrap();
        assert!(a.themes.is_empty(), "no user theme made of a preset");
        assert_eq!(
            (a.mode, a.dark.base.as_str(), a.resolve(false).name.as_str()),
            (Mode::Dark, "preset:lagoon", "Lagoon")
        );
        let old = serde_json::to_string(&theme::daylight()).unwrap();
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(
            (
                a.mode,
                a.light.base.as_str(),
                a.resolve(false).name.as_str()
            ),
            (Mode::Light, "preset:daylight", "Daylight")
        );
        // Paper, which every release of that era wrote on its first run: a fresh install.
        let old = serde_json::to_string(&theme::paper()).unwrap();
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(a, Appearance::default());
    }

    #[test]
    fn a_file_before_version_4_that_shows_the_old_default_follows_the_system() {
        // ADR 0111, amendment of 2026-11-20: a fresh install follows the system.
        assert_eq!(Appearance::default().mode, Mode::System);
        assert_eq!(Mode::default(), Mode::System);
        let fresh = Appearance::default();
        assert_eq!(fresh.resolve(false).name, "Slate Lapis Paper");
        assert_eq!(fresh.resolve(true).name, "Slate Lapis Night");

        // A file of `version` in `mode`, its slots as `edit` leaves them.
        let file = |version: u32, mode: Mode, edit: &dyn Fn(&mut Appearance)| {
            let mut a = Appearance {
                mode,
                ..Default::default()
            };
            edit(&mut a);
            let mut v = serde_json::to_value(&a).unwrap();
            v["version"] = version.into();
            Appearance::from_json(&v.to_string()).unwrap()
        };
        let none = |_: &mut Appearance| {};
        // beta.15 (version 3): the old default, untouched — the system's now.
        assert_eq!(file(3, Mode::Dark, &none).mode, Mode::System);
        // A choice of mode stays.
        assert_eq!(file(3, Mode::Light, &none).mode, Mode::Light);
        assert_eq!(file(3, Mode::System, &none).mode, Mode::System);
        // Dark with a theme picked for either mode, or a recolour on top, is a choice.
        let picked = |a: &mut Appearance| a.assign(Mode::Light, "preset:slate_paper".into());
        assert_eq!(file(3, Mode::Dark, &picked).mode, Mode::Dark);
        let recoloured = |a: &mut Appearance| a.set_token(Mode::Dark, Tok::Cap, Color32::RED);
        assert_eq!(file(3, Mode::Dark, &recoloured).mode, Mode::Dark);
        // From version 4 the file's Dark is the user's.
        assert_eq!(file(VERSION, Mode::Dark, &none).mode, Mode::Dark);
        let back = Appearance::from_json(&Appearance::default().to_json()).unwrap();
        assert_eq!(back, Appearance::default());
        // beta.14–15 (version 2), untouched Slate and Dark: today's default pair, following
        // the system.
        let old = version_2(
            Slot::preset("slate_night"),
            Slot::preset("slate_paper"),
            |v| v["mode"] = "Dark".into(),
        );
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(a.mode, Mode::System);
        assert_eq!(a.resolve(false).name, "Slate Lapis Paper");
    }

    /// A file as beta.14–15 wrote it (version 2, no record of a choice), its modes naming
    /// `dark` and `light`, with `edit` applied to its value.
    fn version_2(dark: Slot, light: Slot, edit: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut v = serde_json::to_value(Appearance {
            dark,
            light,
            ..Default::default()
        })
        .unwrap();
        let o = v.as_object_mut().unwrap();
        o.insert("version".into(), 2.into());
        for mode in ["dark", "light"] {
            o.get_mut(mode)
                .and_then(|s| s.as_object_mut())
                .unwrap()
                .remove("follows_default");
        }
        edit(&mut v);
        v.to_string()
    }

    #[test]
    fn a_mode_that_never_chose_its_theme_shows_today_s_default_and_a_choice_stays() {
        // ADR 0111, amendment of 2026-11-18: Slate Lapis is the default. The file keeps a
        // mode's theme by value and is written whenever anything on the look changes, so
        // naming the default is no choice.
        let fresh = Appearance::default();
        assert_eq!(fresh.version, VERSION);
        assert_eq!(VERSION, 4);
        assert!(fresh.dark.follows_default && fresh.light.follows_default);
        assert_eq!(fresh.resolve_mode(Mode::Dark), theme::default_theme());
        assert_eq!(
            fresh.resolve_mode(Mode::Light),
            theme::default_light_theme()
        );
        assert_eq!(
            DEFAULTS_BY_VERSION.last(),
            Some(&(VERSION, DEFAULT_DARK, DEFAULT_LIGHT))
        );

        // beta.14–15: untouched Slate, though the user changed the mode and the icon — the
        // file was written, no theme was chosen. Both modes show Slate Lapis now, and the
        // rest of the file is kept.
        let old = version_2(
            Slot::preset("slate_night"),
            Slot::preset("slate_paper"),
            |v| {
                v["mode"] = "System".into();
                v["icon"] = serde_json::to_value(IconColour::Original).unwrap();
            },
        );
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(a.dark.base, "preset:slate_lapis_night");
        assert_eq!(a.light.base, "preset:slate_lapis_paper");
        assert!(a.dark.follows_default && a.light.follows_default);
        assert_eq!((a.mode, a.icon), (Mode::System, IconColour::Original));
        assert_eq!(a.version, VERSION, "it is written back as today's file");
        assert_eq!(a.resolve(true).name, "Slate Lapis Night");
        assert_eq!(a.resolve(false).name, "Slate Lapis Paper");

        // beta.14–15: another theme chosen for one mode — kept; the other mode, untouched,
        // follows the default.
        let old = version_2(
            Slot::preset("graphite_night"),
            Slot::preset("slate_paper"),
            |_| {},
        );
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(a.dark.base, "preset:graphite_night");
        assert!(!a.dark.follows_default);
        assert_eq!(a.light.base, "preset:slate_lapis_paper");

        // beta.14–15: Slate Night with the user's own colours, knobs or form on top: the
        // look was made on Slate, and stays on it.
        let mut recoloured = Slot::preset("slate_night");
        recoloured
            .overrides
            .insert(Tok::Cap.key().to_string(), "#010203".into());
        let mut knobbed = Slot::preset("slate_paper");
        knobbed.knobs = Some(ThemeKnobs {
            hue: 30.0,
            ..Default::default()
        });
        let a = Appearance::from_json(&version_2(recoloured, knobbed, |_| {})).unwrap();
        assert_eq!(a.dark.base, "preset:slate_night");
        assert_eq!(a.light.base, "preset:slate_paper");
        assert!(!a.dark.follows_default && !a.light.follows_default);
        assert_eq!(
            a.resolve_mode(Mode::Dark).kit_tokens().cap,
            Color32::from_rgb(1, 2, 3)
        );

        // A version-2 file of the releases before round 39 names their default, Cantus
        // Night: round 39 kept it as a choice, and so does this change.
        let old = version_2(
            Slot::preset("cantus_night"),
            Slot::preset("cantus_paper"),
            |_| {},
        );
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(a.dark.base, "preset:cantus_night");
        assert_eq!(a.light.base, "preset:cantus_paper");

        // From version 3 the file records a choice. Slate Night picked on the Appearance
        // page stays Slate Night; so does the default itself, picked — a later default
        // will not move it.
        let mut a = Appearance::default();
        a.assign(Mode::Dark, "preset:slate_night".into());
        a.assign(Mode::Light, "preset:slate_lapis_paper".into());
        assert!(!a.dark.follows_default && !a.light.follows_default);
        let back = Appearance::from_json(&a.to_json()).unwrap();
        assert_eq!(back, a);
        assert_eq!(back.resolve_mode(Mode::Dark).name, "Slate Night");
        // An untouched install's file, written for the mode, reads back as it was.
        let a = Appearance {
            mode: Mode::Light,
            ..Default::default()
        };
        assert!(a.to_json().contains("\"follows_default\": true"));
        assert_eq!(Appearance::from_json(&a.to_json()).unwrap(), a);
        // A base edited by hand is the user's choice, whatever the record says.
        let hand = a
            .to_json()
            .replacen("preset:slate_lapis_night", "preset:slate_night", 1);
        let back = Appearance::from_json(&hand).unwrap();
        assert_eq!(back.dark.base, "preset:slate_night");
        assert_eq!(back.resolve_mode(Mode::Dark).name, "Slate Night");
        // Deleting the theme of one's own a mode was based on gives it back to the default.
        let mut a = Appearance::default();
        a.themes.push(Theme {
            name: "Mine".into(),
            ..theme::graphite_night()
        });
        a.assign(Mode::Dark, "user:Mine".into());
        assert!(!a.dark.follows_default);
        a.delete("Mine");
        assert_eq!(a.dark, Slot::shipped(Mode::Dark));
        // Resetting a mode keeps whether it chose.
        let mut a = Appearance::default();
        a.assign(Mode::Dark, "preset:slate_night".into());
        a.set_token(Mode::Dark, Tok::Cap, Color32::from_rgb(1, 2, 3));
        a.reset_all(Mode::Dark);
        assert_eq!(a.dark, Slot::preset("slate_night"));
        let mut a = Appearance::default();
        a.set_token(Mode::Light, Tok::Cap, Color32::from_rgb(1, 2, 3));
        a.reset_all(Mode::Light);
        assert_eq!(a.light, Slot::shipped(Mode::Light));
    }
}
