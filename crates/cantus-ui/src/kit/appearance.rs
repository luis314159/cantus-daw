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
    /// The default (ADR 0024 §2): Slate, derived from FL Studio.
    #[default]
    Dark,
    Light,
    /// Dark or light, as the operating system is.
    System,
}

/// A base theme: `preset:<id>` (slate, paper, parchment, ink_gold) or `user:<name>`.
pub type BaseRef = String;

/// A shipped preset: its id and its constructor.
pub type PresetEntry = (&'static str, fn() -> Theme);

/// The shipped presets, by the id a [`BaseRef`] names them with.
pub const PRESETS: [PresetEntry; 4] = [
    ("slate", theme::slate),
    ("paper", theme::paper),
    ("parchment", theme::parchment),
    ("ink_gold", theme::ink_and_gold),
];

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
        Slot::preset("slate")
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
            dark: Slot::preset("slate"),
            light: Slot::preset("paper"),
            logo: LogoChoice::default(),
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
            theme::slate()
        } else {
            theme::paper()
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
                    "preset:slate".into()
                } else {
                    "preset:paper".into()
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
        assert_eq!(a.resolve(true).name, "Slate");
        assert_eq!(a.resolve(false).name, "Paper");
        a.mode = Mode::Dark;
        assert_eq!(a.resolve(false).name, "Slate");
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
        assert_eq!(a.dark.base, "preset:slate");
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
            a.dark.base, "preset:slate",
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
        assert_eq!(k.radius, 4.0);
        assert!(k.density < 1.0 && (k.text_scale - 1.2).abs() < 1e-6);
        assert_eq!(t.logo.number, 7);
    }
}
