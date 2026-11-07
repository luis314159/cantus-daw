//! Factory presets as data (round 37, ADR 0101): one TOML file per unit under
//! `crates/cantus-ui/presets/`, built into the binary.
//!
//! The presets written as Rust arrays before round 37 stay where they are (starting
//! presets, demos and templates find them by key); a data file adds presets to a unit and
//! **tags** presets written in code, so every preset — code or data — reaches the menus, the
//! browser, the agents and the website through the same `factory_presets` functions. A
//! preset's name is a locale key, as every name in the interface (both languages, the i18n
//! test); its styles and roles are words of [`crate::preset_tags`].
//!
//! ```toml
//! [[preset]]
//! key = "unda.preset.night_drive"      # its name's locale key: stable, never reused
//! styles = ["house", "pop"]
//! roles = ["lead"]
//! [preset.values]                      # what it sets; every other key at its default
//! "osc1.uni" = 7
//! [preset.text]
//! "macro1.name" = "Width"
//!
//! [[tag]]                              # styles and roles of a preset written in code
//! key = "unda.preset.glass_choir"
//! styles = ["ambient", "film"]
//! roles = ["pad"]
//! ```
//!
//! A file that does not read is left out at run time (the presets in code still work) and
//! fails the tests, which also check every key, value, style and role.

use crate::preset_tags::{Role, Style};
use crate::views::inst::FactoryPreset;
use cantus_core::{EffectKind, InstrumentKind};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

/// What a preset file is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FileUnit {
    Instrument(InstrumentKind),
    Effect(EffectKind),
    /// Pulsus's kits (tags only: a kit is written in code).
    Kits,
}

/// One file: presets, and tags for presets written in code.
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct File {
    #[serde(default)]
    pub preset: Vec<DataPreset>,
    #[serde(default)]
    pub tag: Vec<TagRecord>,
}

/// A preset written as data.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DataPreset {
    pub key: String,
    /// What it is for and what to listen for, in a line (English; the website and the
    /// agents show it).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub values: BTreeMap<String, f32>,
    #[serde(default)]
    pub text: BTreeMap<String, String>,
    /// Descriptive tags beside the styles and roles (`organ.tag.soul`, `bass.tag.808`…).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// The styles and roles of a preset written in code.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TagRecord {
    pub key: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub roles: Vec<String>,
}

/// Every file, with the path it is read from (for messages).
pub const FILES: &[(FileUnit, &str, &str)] = &[
    (
        FileUnit::Instrument(InstrumentKind::Wavetable),
        "presets/instruments/wavetable.toml",
        include_str!("../presets/instruments/wavetable.toml"),
    ),
    (
        FileUnit::Instrument(InstrumentKind::Analog),
        "presets/instruments/analog.toml",
        include_str!("../presets/instruments/analog.toml"),
    ),
    (
        FileUnit::Instrument(InstrumentKind::Fm),
        "presets/instruments/fm.toml",
        include_str!("../presets/instruments/fm.toml"),
    ),
    (
        FileUnit::Instrument(InstrumentKind::Sampler),
        "presets/instruments/sampler.toml",
        include_str!("../presets/instruments/sampler.toml"),
    ),
    (
        FileUnit::Instrument(InstrumentKind::Bass),
        "presets/instruments/bass.toml",
        include_str!("../presets/instruments/bass.toml"),
    ),
    (
        FileUnit::Instrument(InstrumentKind::ElectricPiano),
        "presets/instruments/electricpiano.toml",
        include_str!("../presets/instruments/electricpiano.toml"),
    ),
    (
        FileUnit::Instrument(InstrumentKind::Organ),
        "presets/instruments/organ.toml",
        include_str!("../presets/instruments/organ.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Eq),
        "presets/effects/eq.toml",
        include_str!("../presets/effects/eq.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Compressor),
        "presets/effects/compressor.toml",
        include_str!("../presets/effects/compressor.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Limiter),
        "presets/effects/limiter.toml",
        include_str!("../presets/effects/limiter.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Space),
        "presets/effects/space.toml",
        include_str!("../presets/effects/space.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Multiband),
        "presets/effects/multiband.toml",
        include_str!("../presets/effects/multiband.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::DeEsser),
        "presets/effects/deesser.toml",
        include_str!("../presets/effects/deesser.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Gate),
        "presets/effects/gate.toml",
        include_str!("../presets/effects/gate.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Saturator),
        "presets/effects/saturator.toml",
        include_str!("../presets/effects/saturator.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Stereo),
        "presets/effects/stereo.toml",
        include_str!("../presets/effects/stereo.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Maximizer),
        "presets/effects/maximizer.toml",
        include_str!("../presets/effects/maximizer.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Delay),
        "presets/effects/delay.toml",
        include_str!("../presets/effects/delay.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Reverb),
        "presets/effects/reverb.toml",
        include_str!("../presets/effects/reverb.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Chorus),
        "presets/effects/chorus.toml",
        include_str!("../presets/effects/chorus.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Flanger),
        "presets/effects/flanger.toml",
        include_str!("../presets/effects/flanger.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Phaser),
        "presets/effects/phaser.toml",
        include_str!("../presets/effects/phaser.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Tremolo),
        "presets/effects/tremolo.toml",
        include_str!("../presets/effects/tremolo.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Vibrato),
        "presets/effects/vibrato.toml",
        include_str!("../presets/effects/vibrato.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::RingMod),
        "presets/effects/ringmod.toml",
        include_str!("../presets/effects/ringmod.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::FreqShift),
        "presets/effects/freqshift.toml",
        include_str!("../presets/effects/freqshift.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::VolumeShaper),
        "presets/effects/volumeshaper.toml",
        include_str!("../presets/effects/volumeshaper.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Clipper),
        "presets/effects/clipper.toml",
        include_str!("../presets/effects/clipper.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Transient),
        "presets/effects/transient.toml",
        include_str!("../presets/effects/transient.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::TransientSplit),
        "presets/effects/transientsplit.toml",
        include_str!("../presets/effects/transientsplit.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::Analyzer),
        "presets/effects/analyzer.toml",
        include_str!("../presets/effects/analyzer.toml"),
    ),
    (
        FileUnit::Effect(EffectKind::AutoTune),
        "presets/effects/autotune.toml",
        include_str!("../presets/effects/autotune.toml"),
    ),
];

/// Read one file's text.
pub fn parse(text: &str) -> Result<File, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

/// The files, read once; one that does not read is left out (and reported once).
fn files() -> &'static [(FileUnit, File)] {
    static FILES_READ: OnceLock<Vec<(FileUnit, File)>> = OnceLock::new();
    FILES_READ.get_or_init(|| {
        FILES
            .iter()
            .filter_map(|(unit, path, text)| match parse(text) {
                Ok(f) => Some((*unit, f)),
                Err(e) => {
                    eprintln!("presets: {path} is left out: {e}");
                    None
                }
            })
            .collect()
    })
}

/// A data preset's tag keys: its styles, then its roles (words the vocabulary does not
/// know are dropped; the tests refuse them).
fn tag_keys(styles: &[String], roles: &[String]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = styles
        .iter()
        .filter_map(|s| Style::from_id(s))
        .map(Style::key)
        .collect();
    out.extend(roles.iter().filter_map(|r| Role::from_id(r)).map(Role::key));
    out
}

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// The data presets of `unit`, in file order.
pub fn presets_of(unit: FileUnit) -> impl Iterator<Item = &'static DataPreset> {
    files()
        .iter()
        .filter(move |(u, _)| *u == unit)
        .flat_map(|(_, f)| f.preset.iter())
}

/// The data presets of a catalogue unit (see [`presets_of`]).
pub fn presets_of_unit(
    unit: crate::preset_catalog::Unit,
) -> impl Iterator<Item = &'static DataPreset> {
    use crate::preset_catalog::Unit;
    presets_of(match unit {
        Unit::Instrument(k) => FileUnit::Instrument(k),
        Unit::Effect(k) => FileUnit::Effect(k),
        Unit::Kit => FileUnit::Kits,
    })
}

/// A kind's presets written in code, then its data presets — built once and kept for the
/// life of the process (`code` alone when the data adds none).
pub fn instrument_presets(
    kind: InstrumentKind,
    code: &'static [FactoryPreset],
) -> &'static [FactoryPreset] {
    static CELLS: [OnceLock<&'static [FactoryPreset]>; InstrumentKind::ALL.len()] =
        [const { OnceLock::new() }; InstrumentKind::ALL.len()];
    let Some(i) = InstrumentKind::ALL.iter().position(|k| *k == kind) else {
        return code;
    };
    CELLS[i].get_or_init(|| {
        let data: Vec<FactoryPreset> = presets_of(FileUnit::Instrument(kind))
            .map(|p| FactoryPreset {
                label_key: leak_str(&p.key),
                values: Box::leak(
                    p.values
                        .iter()
                        .map(|(k, v)| (leak_str(k), *v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                text: Box::leak(
                    p.text
                        .iter()
                        .map(|(k, v)| (leak_str(k), leak_str(v)))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                tags: Box::leak({
                    let mut all = tag_keys(&p.styles, &p.roles);
                    all.extend(p.tags.iter().map(|t| leak_str(t)));
                    all.into_boxed_slice()
                }),
            })
            .collect();
        if data.is_empty() {
            return code;
        }
        let mut all = code.to_vec();
        all.extend(data);
        Box::leak(all.into_boxed_slice())
    })
}

/// An effect kind's presets written in code, then its data presets.
pub fn effect_presets(
    kind: EffectKind,
    code: &'static [crate::views::fx::FactoryPreset],
) -> &'static [crate::views::fx::FactoryPreset] {
    static CELLS: [OnceLock<&'static [crate::views::fx::FactoryPreset]>; EffectKind::ALL.len()] =
        [const { OnceLock::new() }; EffectKind::ALL.len()];
    let Some(i) = EffectKind::ALL.iter().position(|k| *k == kind) else {
        return code;
    };
    CELLS[i].get_or_init(|| {
        let data: Vec<crate::views::fx::FactoryPreset> = presets_of(FileUnit::Effect(kind))
            .map(|p| {
                let values: &'static [(&'static str, f32)] = Box::leak(
                    p.values
                        .iter()
                        .map(|(k, v)| (leak_str(k), *v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                );
                (leak_str(&p.key), values)
            })
            .collect();
        if data.is_empty() {
            return code;
        }
        let mut all = code.to_vec();
        all.extend(data);
        Box::leak(all.into_boxed_slice())
    })
}

/// The tag keys the data gives a preset, by its key: a data preset's own, and the
/// `[[tag]]` records for presets written in code (several records add up).
pub fn data_tags(key: &str) -> &'static [&'static str] {
    static TAGS: OnceLock<HashMap<String, Vec<&'static str>>> = OnceLock::new();
    TAGS.get_or_init(|| {
        let mut map: HashMap<String, Vec<&'static str>> = HashMap::new();
        for (_, f) in files() {
            for p in &f.preset {
                map.entry(p.key.clone())
                    .or_default()
                    .extend(tag_keys(&p.styles, &p.roles));
                map.entry(p.key.clone())
                    .or_default()
                    .extend(p.tags.iter().map(|t| leak_str(t)));
            }
            for t in &f.tag {
                map.entry(t.key.clone())
                    .or_default()
                    .extend(tag_keys(&t.styles, &t.roles));
            }
        }
        map
    })
    .get(key)
    .map(Vec::as_slice)
    .unwrap_or(&[])
}

/// What is wrong with a file for `unit`, every problem once (empty: nothing). `known`
/// says whether a key names a preset written in code for that unit.
pub fn check(unit: FileUnit, file: &File, known: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut bad = Vec::new();
    let instrument = !matches!(unit, FileUnit::Effect(_));
    let words = |what: &str, styles: &[String], roles: &[String], need: bool| {
        let mut bad = Vec::new();
        for s in styles {
            if Style::from_id(s).is_none() {
                bad.push(format!("{what}: no style \"{s}\""));
            }
        }
        for r in roles {
            match Role::from_id(r) {
                None => bad.push(format!("{what}: no role \"{r}\"")),
                Some(role) if !role.fits(instrument) => {
                    bad.push(format!("{what}: \"{r}\" is not a role for this unit"))
                }
                _ => {}
            }
        }
        if need && (styles.is_empty() || roles.is_empty()) {
            bad.push(format!("{what}: a preset needs a style and a role"));
        }
        bad
    };
    for p in &file.preset {
        bad.extend(words(&p.key, &p.styles, &p.roles, true));
        if known(&p.key) {
            bad.push(format!("{}: a preset in code has this key", p.key));
        }
        match unit {
            FileUnit::Instrument(kind) => {
                for (k, v) in &p.values {
                    match kind.param(k) {
                        None => bad.push(format!("{}: {kind:?} has no value \"{k}\"", p.key)),
                        Some(i) if !(i.min..=i.max).contains(v) => bad.push(format!(
                            "{}: {k} = {v} is outside {}…{}",
                            p.key, i.min, i.max
                        )),
                        _ => {}
                    }
                }
                for k in p.text.keys() {
                    if !kind.is_text_key(k) {
                        bad.push(format!("{}: {kind:?} has no text \"{k}\"", p.key));
                    }
                }
            }
            FileUnit::Effect(kind) => {
                for (k, v) in &p.values {
                    match kind.params().iter().find(|i| i.key == k) {
                        None => bad.push(format!("{}: {kind:?} has no value \"{k}\"", p.key)),
                        Some(i) if !(i.min..=i.max).contains(v) => bad.push(format!(
                            "{}: {k} = {v} is outside {}…{}",
                            p.key, i.min, i.max
                        )),
                        _ => {}
                    }
                }
                if !p.text.is_empty() {
                    bad.push(format!("{}: an effect's preset has no texts", p.key));
                }
            }
            FileUnit::Kits => bad.push(format!("{}: a kit is written in code", p.key)),
        }
    }
    for t in &file.tag {
        bad.extend(words(&t.key, &t.styles, &t.roles, false));
        if !known(&t.key) {
            bad.push(format!("{}: tags for a preset that is not in code", t.key));
        }
    }
    bad
}

// ---- writing a file -------------------------------------------------------------------------

/// A TOML string, quoted and escaped.
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list(items: &[String]) -> String {
    let words: Vec<String> = items.iter().map(|s| quoted(s)).collect();
    format!("[{}]", words.join(", "))
}

/// A number as the shortest text that reads back to the same `f32` (an integer value is
/// written as one; TOML reads it back as a number).
fn number(v: f32) -> String {
    format!("{v}")
}

/// A file as text, in one order (presets then tags, each preset's keys sorted), so a tool
/// that rewrites a value changes one line. `header` is written first as comments.
pub fn write(file: &File, header: &str) -> String {
    let mut out = String::new();
    for line in header.lines() {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
    for p in &file.preset {
        out.push_str("\n[[preset]]\n");
        out.push_str(&format!("key = {}\n", quoted(&p.key)));
        if !p.note.is_empty() {
            out.push_str(&format!("note = {}\n", quoted(&p.note)));
        }
        out.push_str(&format!("styles = {}\n", list(&p.styles)));
        out.push_str(&format!("roles = {}\n", list(&p.roles)));
        if !p.tags.is_empty() {
            out.push_str(&format!("tags = {}\n", list(&p.tags)));
        }
        if !p.values.is_empty() {
            out.push_str("[preset.values]\n");
            for (k, v) in &p.values {
                out.push_str(&format!("{} = {}\n", quoted(k), number(*v)));
            }
        }
        if !p.text.is_empty() {
            out.push_str("[preset.text]\n");
            for (k, v) in &p.text {
                out.push_str(&format!("{} = {}\n", quoted(k), quoted(v)));
            }
        }
    }
    for t in &file.tag {
        out.push_str("\n[[tag]]\n");
        out.push_str(&format!("key = {}\n", quoted(&t.key)));
        out.push_str(&format!("styles = {}\n", list(&t.styles)));
        out.push_str(&format!("roles = {}\n", list(&t.roles)));
    }
    out
}

/// Where a unit's file lives under `crates/cantus-ui/presets/`.
pub fn path_of(unit: FileUnit) -> String {
    match unit {
        FileUnit::Instrument(k) => format!("instruments/{}.toml", format!("{k:?}").to_lowercase()),
        FileUnit::Effect(k) => format!("effects/{}.toml", format!("{k:?}").to_lowercase()),
        FileUnit::Kits => "kits.toml".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `key` names a preset written in code for `unit`.
    fn in_code(unit: FileUnit, key: &str) -> bool {
        match unit {
            FileUnit::Instrument(k) => crate::views::inst::code_presets(k)
                .iter()
                .any(|p| p.label_key == key),
            FileUnit::Effect(k) => crate::views::fx::code_presets(k)
                .iter()
                .any(|(l, _)| *l == key),
            FileUnit::Kits => cantus_core::PulsusParams::factory()
                .iter()
                .any(|f| f.label_key == key),
        }
    }

    #[test]
    fn every_preset_file_reads_and_holds_only_what_its_unit_has() {
        for (unit, path, text) in FILES {
            let file = parse(text).unwrap_or_else(|e| panic!("{path}: {e}"));
            let bad = check(*unit, &file, &|k| in_code(*unit, k));
            assert!(bad.is_empty(), "{path}: {bad:#?}");
            for p in &file.preset {
                for l in crate::i18n::Locale::ALL {
                    assert!(
                        crate::i18n::has(l, &p.key),
                        "{path}: {} has no {l:?} name",
                        p.key
                    );
                }
            }
        }
    }

    #[test]
    fn the_format_reads_presets_and_tags_and_the_check_refuses_what_is_wrong() {
        let text = r#"
[[preset]]
key = "test.preset.one"
styles = ["trap", "lofi"]
roles = ["bass"]
[preset.values]
"gain" = -3
"osc1.uni" = 5.0
[preset.text]
"macro1.name" = "Width"

[[tag]]
key = "unda.preset.glass_choir"
styles = ["ambient"]
roles = ["pad"]
"#;
        let f = parse(text).expect("it reads");
        assert_eq!(f.preset.len(), 1);
        assert_eq!(
            f.preset[0].values.get("gain"),
            Some(&-3.0),
            "an integer reads as a number"
        );
        assert_eq!(f.tag[0].roles, vec!["pad".to_string()]);
        let unit = FileUnit::Instrument(InstrumentKind::Wavetable);
        // What the check is told is written in code (the arrays are data now: a stand-in).
        let code = |k: &str| k == "unda.preset.glass_choir";
        let bad = check(unit, &f, &code);
        assert!(bad.is_empty(), "{bad:#?}");
        assert_eq!(
            tag_keys(&f.preset[0].styles, &f.preset[0].roles),
            vec!["preset.style.trap", "preset.style.lofi", "preset.role.bass"]
        );

        let wrong = parse(
            r#"
[[preset]]
key = "unda.preset.glass_choir"
styles = ["polka"]
roles = ["vocal"]
[preset.values]
"no.such.key" = 1
"gain" = 900
[preset.text]
"nope" = "x"

[[tag]]
key = "unda.preset.not_there"
"#,
        )
        .expect("it reads");
        let bad = check(unit, &wrong, &code);
        let all = bad.join("\n");
        for needle in [
            "no style \"polka\"",
            "\"vocal\" is not a role",
            "a preset in code has this key",
            "has no value \"no.such.key\"",
            "gain = 900 is outside",
            "has no text \"nope\"",
            "not in code",
        ] {
            assert!(all.contains(needle), "{needle} missing from:\n{all}");
        }
        assert!(
            parse("[[preset]]\nkey = \"x\"\ncolour = 3\n").is_err(),
            "an unknown field is refused"
        );
    }

    #[test]
    fn effect_files_take_effect_roles_and_no_texts() {
        let f = parse(
            r#"
[[preset]]
key = "test.preset.fx"
styles = ["pop"]
roles = ["vocal"]
[preset.values]
"drive" = 6
[preset.text]
"x" = "y"
"#,
        )
        .expect("it reads");
        let unit = FileUnit::Effect(EffectKind::Saturator);
        let bad = check(unit, &f, &|_| false);
        assert_eq!(
            bad,
            vec!["test.preset.fx: an effect's preset has no texts".to_string()]
        );
        let none = parse("[[preset]]\nkey = \"y\"\n").expect("it reads");
        let bad = check(unit, &none, &|_| false);
        assert_eq!(
            bad,
            vec!["y: a preset needs a style and a role".to_string()]
        );
    }
}
