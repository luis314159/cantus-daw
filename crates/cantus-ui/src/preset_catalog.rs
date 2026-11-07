//! Every preset a unit can load, as one list the browser, the agents and the audit read
//! (round 37, ADR 0101): the factory's (in code and as data, `preset_data`) and the user's
//! own, each with its name in the interface's language, its styles and roles, and what
//! loading it sets.
//!
//! The three kinds of unit keep their own preset types — an instrument's `NativeSpec`, an
//! effect's values, a Pulsus kit — and their own ways of loading (each one command, so one
//! step of undo); the catalogue only lists them side by side.

use crate::i18n::t;
use crate::preset_tags::{natural_effect_role, natural_role, read_tags, Role, Style};
use cantus_core::{EffectKind, InstrumentKind, NativeSpec, PulsusParams};
use std::collections::BTreeMap;

/// What a preset loads into.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Instrument(InstrumentKind),
    Effect(EffectKind),
    /// Pulsus, the drum machine: its presets are kits.
    Kit,
}

impl Unit {
    /// Whether its presets play notes (an instrument or a kit) rather than process sound.
    pub fn is_instrument(self) -> bool {
        !matches!(self, Unit::Effect(_))
    }

    /// Whether it makes sound to audition: the dancer (ADR 0087) does not, nor does
    /// a hosted plug-in.
    pub fn audible(self) -> bool {
        !matches!(
            self,
            Unit::Instrument(InstrumentKind::Dancer) | Unit::Effect(EffectKind::External)
        )
    }

    /// Its name's locale key (Unda, Fervor, Pulsus…).
    pub fn name_key(self) -> &'static str {
        match self {
            Unit::Instrument(k) => k.name_key(),
            Unit::Effect(k) => k.name_key(),
            Unit::Kit => "picker.dev.pulsus.name",
        }
    }
}

/// Where a preset comes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Factory,
    User,
}

/// What loading a preset sets.
#[derive(Clone, Debug, PartialEq)]
pub enum Payload {
    Native(NativeSpec),
    Effect(BTreeMap<String, f32>),
    Kit(PulsusParams),
}

/// One preset.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Stable: a factory preset's key (its name's locale key), `user:<file name>` for the
    /// user's own.
    pub id: String,
    pub unit: Unit,
    /// In the interface's language (a user preset: its file's name).
    pub name: String,
    pub source: Source,
    pub styles: Vec<Style>,
    pub roles: Vec<Role>,
    pub payload: Payload,
}

impl Entry {
    /// The role it plays: its first, else what its unit is, else a keys part for an
    /// instrument and a creative insert for an effect. The audition and the render check
    /// play it on this role's phrase.
    pub fn role(&self) -> Role {
        self.roles.first().copied().unwrap_or(match self.unit {
            Unit::Instrument(k) => natural_role(k).unwrap_or(Role::Keys),
            Unit::Effect(k) => natural_effect_role(k).unwrap_or(Role::Creative),
            Unit::Kit => Role::Drums,
        })
    }
}

/// Styles and roles from a preset's own tags and the data's, plus the unit's natural role
/// when it names none.
fn sorted(unit: Unit, own: &[&str], key: &str) -> (Vec<Style>, Vec<Role>) {
    let mut all: Vec<&str> = own.to_vec();
    all.extend_from_slice(crate::preset_data::data_tags(key));
    let (styles, mut roles) = read_tags(&all);
    let instrument = unit.is_instrument();
    roles.retain(|r| r.fits(instrument));
    if roles.is_empty() {
        let natural = match unit {
            Unit::Instrument(k) => natural_role(k),
            Unit::Effect(k) => natural_effect_role(k),
            Unit::Kit => Some(Role::Drums),
        };
        roles.extend(natural);
    }
    (styles, roles)
}

/// The presets of `unit`: the factory's in their order, then the user's by name (when
/// `user`).
pub fn entries(unit: Unit, user: bool) -> Vec<Entry> {
    let mut out = Vec::new();
    match unit {
        Unit::Instrument(kind) => {
            for p in crate::views::inst::factory_presets(kind) {
                let (styles, roles) = sorted(unit, p.tags, p.label_key);
                out.push(Entry {
                    id: p.label_key.to_string(),
                    unit,
                    name: t(p.label_key),
                    source: Source::Factory,
                    styles,
                    roles,
                    payload: Payload::Native(crate::views::inst::preset_spec(kind, p)),
                });
            }
            if user {
                for (name, spec) in crate::views::inst::user_presets(kind) {
                    let (styles, roles) = sorted(unit, &[], "");
                    out.push(Entry {
                        id: format!("user:{name}"),
                        unit,
                        name,
                        source: Source::User,
                        styles,
                        roles,
                        payload: Payload::Native(spec),
                    });
                }
            }
        }
        Unit::Effect(kind) => {
            for (label, values) in crate::views::fx::factory_presets(kind) {
                let (styles, roles) = sorted(unit, &[], label);
                out.push(Entry {
                    id: label.to_string(),
                    unit,
                    name: t(label),
                    source: Source::Factory,
                    styles,
                    roles,
                    payload: Payload::Effect(
                        values.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
                    ),
                });
            }
            if user {
                for (name, values) in crate::views::fx::user_presets(kind) {
                    let (styles, roles) = sorted(unit, &[], "");
                    out.push(Entry {
                        id: format!("user:{name}"),
                        unit,
                        name,
                        source: Source::User,
                        styles,
                        roles,
                        payload: Payload::Effect(values),
                    });
                }
            }
        }
        Unit::Kit => {
            for f in PulsusParams::factory() {
                let (styles, roles) = sorted(unit, f.tags, f.label_key);
                out.push(Entry {
                    id: f.label_key.to_string(),
                    unit,
                    name: t(f.label_key),
                    source: Source::Factory,
                    styles,
                    roles,
                    payload: Payload::Kit(f.kit),
                });
            }
        }
    }
    out
}

/// Every unit with presets of its own, in the picker's order: the instruments, Pulsus,
/// then the effects (hosted plug-ins keep their makers' presets).
pub fn units() -> Vec<Unit> {
    let mut out: Vec<Unit> = InstrumentKind::ALL
        .into_iter()
        .map(Unit::Instrument)
        .collect();
    out.push(Unit::Kit);
    out.extend(
        EffectKind::ALL
            .into_iter()
            .filter(|k| *k != EffectKind::External)
            .map(Unit::Effect),
    );
    out
}

/// The entry a data preset makes, read straight from a file being edited (the library's
/// tool fits levels on it before the file is built in).
pub fn entry_of_data(unit: Unit, p: &crate::preset_data::DataPreset) -> Entry {
    let mut tags: Vec<String> = p
        .styles
        .iter()
        .filter_map(|s| Style::from_id(s))
        .map(|s| s.key().to_string())
        .collect();
    tags.extend(
        p.roles
            .iter()
            .filter_map(|r| Role::from_id(r))
            .map(|r| r.key().to_string()),
    );
    tags.extend(p.tags.iter().cloned());
    let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    let (styles, mut roles) = read_tags(&tags);
    roles.retain(|r| r.fits(unit.is_instrument()));
    let payload = match unit {
        Unit::Instrument(kind) => {
            let mut s = NativeSpec::new(kind);
            for (k, v) in &p.values {
                s.set(k, *v);
            }
            for (k, v) in &p.text {
                let _ = s.set_text(k, Some(v.clone()));
            }
            Payload::Native(s)
        }
        Unit::Effect(_) => Payload::Effect(p.values.clone()),
        Unit::Kit => Payload::Kit(PulsusParams::default()),
    };
    Entry {
        id: p.key.clone(),
        unit,
        name: t(&p.key),
        source: Source::Factory,
        styles,
        roles,
        payload,
    }
}
