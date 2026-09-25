//! The plug-ins' words exist in both languages: every label, choice and group name a host
//! is shown (ADR 0007: no string without its twin).

use cantus_plugins::desc;
use cantus_plugins::params::{self, choice_keys};
use cantus_plugins::text::{has, Lang};

const EXPORT_KEYS: &[&str] = &[
    "export.off",
    "export.on",
    "export.bits",
    "export.bypass",
    "export.sidechain",
    "export.sidechain.own",
    "export.sidechain.external",
    "export.param.in_group",
    "export.group.band",
    "export.group.key_band",
    "export.group.decay_band",
    "export.group.post_band",
    "export.bus.input",
    "export.bus.output",
    "export.bus.sidechain",
];

#[test]
fn every_word_a_host_is_shown_exists_in_both_languages() {
    let mut missing = Vec::new();
    for lang in Lang::ALL {
        for key in EXPORT_KEYS {
            if !has(lang, key) {
                missing.push(format!("{lang:?} {key}"));
            }
        }
        for d in &desc::ALL {
            for info in d.kind.params() {
                if !has(lang, info.label_key) {
                    missing.push(format!("{lang:?} {}", info.label_key));
                }
            }
            for key in choice_keys(d.kind) {
                if !has(lang, &key) {
                    missing.push(format!("{lang:?} {key}"));
                }
            }
        }
    }
    missing.dedup();
    assert!(missing.is_empty(), "missing: {missing:?}");
}

#[test]
fn every_parameter_has_a_name_and_every_choice_list_its_length() {
    for lang in Lang::ALL {
        for d in &desc::ALL {
            let t = params::table_in(d.kind, lang);
            for p in &t.params {
                assert!(
                    !p.name.is_empty() && !p.name.contains("fx."),
                    "{:?} {}",
                    lang,
                    p.key
                );
                if !p.choices.is_empty() {
                    assert_eq!(p.choices.len() as i32, p.step_count() + 1, "{}", p.key);
                }
            }
        }
    }
    // Bands read as people count them.
    let t = params::table_in(cantus_core::EffectKind::Eq, Lang::En);
    let p = t.by_key("b3.gain").unwrap();
    assert_eq!(p.group, "Band 4");
    assert_eq!(p.name, "Band 4 Gain");
}
