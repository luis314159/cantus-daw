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
    /// The default: Cantus Night (ADR 0046), the website's night stage.
    #[default]
    Dark,
    Light,
    /// Dark or light, as the operating system is.
    System,
}

/// A base theme: `preset:<id>` (one of [`PRESETS`]) or `user:<name>`.
pub type BaseRef = String;

/// A shipped preset: its id and its constructor.
pub type PresetEntry = (&'static str, fn() -> Theme);

/// The shipped presets, by the id a [`BaseRef`] names them with, in the order the
/// Appearance page lists them: round 25's family (ADR 0046), then the older presets.
pub const PRESETS: [PresetEntry; 12] = [
    ("cantus_night", theme::cantus_night),
    ("cantus_paper", theme::cantus_paper),
    ("daylight", theme::daylight),
    ("blue_hour", theme::blue_hour),
    ("ultraviolet", theme::ultraviolet),
    ("lagoon", theme::lagoon),
    ("contrast", theme::high_contrast),
    ("contrast_light", theme::high_contrast_light),
    ("slate", theme::slate),
    ("ink_gold", theme::ink_and_gold),
    ("paper", theme::paper),
    ("parchment", theme::parchment),
];

/// The preset a fresh install shows in each mode: Cantus Night and its light twin.
pub const DEFAULT_DARK: &str = "cantus_night";
pub const DEFAULT_LIGHT: &str = "cantus_paper";

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
}

impl Default for Slot {
    fn default() -> Self {
        Slot::preset(DEFAULT_DARK)
    }
}

impl Slot {
    pub fn preset(id: &str) -> Slot {
        Slot {
            base: format!("preset:{id}"),
            overrides: KitMap::new(),
            legacy: BTreeMap::new(),
            knobs: None,
            form: None,
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
    #[serde(skip)]
    pub compare: Option<Compare>,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            version: 2,
            mode: Mode::default(),
            dark: Slot::preset(DEFAULT_DARK),
            light: Slot::preset(DEFAULT_LIGHT),
            logo: LogoChoice::default(),
            icon: IconColour::default(),
            themes: Vec::new(),
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
            theme::cantus_night()
        } else {
            theme::cantus_paper()
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
        let base = self.slot(mode).base.clone();
        *self.slot_mut(mode) = Slot {
            base,
            ..Slot::preset("paper")
        };
    }

    /// Give `mode` another base theme. The mode's own changes stay.
    pub fn assign(&mut self, mode: Mode, base: BaseRef) {
        self.slot_mut(mode).base = base;
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

    /// Delete a user theme. A mode that was based on it falls back to its default preset.
    pub fn delete(&mut self, name: &str) {
        self.themes.retain(|t| t.name != name);
        let gone = format!("user:{name}");
        for mode in [Mode::Dark, Mode::Light] {
            if self.slot(mode).base == gone {
                self.slot_mut(mode).base = if mode == Mode::Dark {
                    format!("preset:{DEFAULT_DARK}")
                } else {
                    format!("preset:{DEFAULT_LIGHT}")
                };
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
    pub fn from_json(text: &str) -> Result<Appearance, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v.get("version").is_some() {
            return serde_json::from_value(v).map_err(|e| e.to_string());
        }
        let t: Theme = serde_json::from_value(v).map_err(|e| e.to_string())?;
        Ok(Appearance::from_legacy(t))
    }

    /// An appearance around an old theme file.
    pub fn from_legacy(t: Theme) -> Appearance {
        let mut a = Appearance::default();
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
        assert_eq!(a.resolve(true).name, "Cantus Night");
        assert_eq!(a.resolve(false).name, "Cantus Paper");
        a.mode = Mode::Dark;
        assert_eq!(a.resolve(false).name, "Cantus Night");
        // Any preset may serve either mode.
        a.assign(Mode::Light, "preset:parchment".into());
        assert_eq!(a.resolve_mode(Mode::Light).name, "Parchment");
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
        assert_eq!(a.dark.base, "preset:cantus_night");
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
        a.duplicate("preset:paper", "My paper");
        assert!(a.themes.iter().any(|t| t.name == "My paper"));
        a.delete("Warm slate");
        assert_eq!(
            a.dark.base, "preset:cantus_night",
            "a mode loses a deleted base gracefully"
        );
    }

    #[test]
    fn an_old_theme_json_keeps_loading() {
        // A file written before the kit, the knobs and the modes: the 22 colours only.
        let mut v = serde_json::to_value(theme::ink_and_gold()).unwrap();
        for k in ["kit", "knobs", "form", "logo"] {
            v.as_object_mut().unwrap().remove(k);
        }
        let old = serde_json::to_string(&v).unwrap();
        let a = Appearance::from_json(&old).unwrap();
        assert_eq!(a.mode, Mode::Dark);
        let t = a.resolve(false);
        assert_eq!(t.palette(), theme::ink_and_gold().palette());
        // Its kit tokens are derived from its colours, and legible.
        assert!(t.kit_tokens().legibility_failures().is_empty());
        // And an appearance file round-trips.
        let back = Appearance::from_json(&a.to_json()).unwrap();
        assert_eq!(back, a);
        // An old theme also imports as a shared theme.
        assert!(import_theme(&old).is_ok());
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
        // Cantus Night's 3-point corners, twice as round.
        assert_eq!(k.radius, 6.0);
        assert!(k.density < 1.0 && (k.text_scale - 1.2).abs() < 1e-6);
        assert_eq!(t.logo.number, 7);
    }

    #[test]
    fn a_fresh_install_shows_the_website_s_theme_and_a_saved_choice_is_kept() {
        let a = Appearance::default();
        assert_eq!(a.mode, Mode::Dark);
        assert_eq!(a.dark.base, "preset:cantus_night");
        assert_eq!(a.light.base, "preset:cantus_paper");
        // A file a user saved before round 25 names Slate and Paper: it keeps them.
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
        assert_eq!(back.resolve_mode(Mode::Dark).name, "Slate");
        assert_eq!(back.resolve_mode(Mode::Light).name, "Paper");
        // …and, written before the icon existed, the automatic mark (as designed on a dark
        // taskbar, which is where every earlier release's icon was seen).
        assert_eq!(back.icon, IconColour::Auto);
        // Every preset is reachable by its id, and named in both languages.
        for (id, make) in PRESETS {
            assert_eq!(a.base_theme(&format!("preset:{id}"), Mode::Dark), make());
            for lang in crate::i18n::Locale::ALL {
                crate::i18n::set_locale(lang);
                let key = preset_name_key(id);
                assert_ne!(crate::i18n::t(&key), key, "{lang:?}: {id} has no name");
            }
        }
        crate::i18n::set_locale(crate::i18n::Locale::En);
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
}
