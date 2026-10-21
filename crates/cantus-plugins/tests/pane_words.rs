//! The DAW's controllers pane names an effect's choices as these plug-ins name them for a
//! host (round 29, review 4 of ADR 0062, #39): `cantus_ui::controllers_words` keeps its own
//! table (the pane must not depend on the plug-ins' build), and this test holds the two
//! equal, in both languages, for every stepped parameter of every kind.

use cantus_core::EffectKind;
use cantus_plugins::params::table_in;
use cantus_plugins::text::Lang;
use cantus_ui::controllers_words::effect_choice_words;
use cantus_ui::i18n::Locale;

#[test]
fn the_pane_names_every_choice_as_the_plug_ins_do() {
    let mut named = 0;
    for kind in EffectKind::ALL {
        for (lang, locale) in [(Lang::En, Locale::En), (Lang::Es, Locale::Es)] {
            let table = table_in(kind, lang);
            for p in table.params[..table.spec_count].iter().filter(|p| p.stepped) {
                match effect_choice_words(kind, p.key, locale) {
                    Some(words) => {
                        assert_eq!(words, p.choices, "{kind:?} {} ({locale:?})", p.key);
                        named += 1;
                    }
                    // No names in the pane: the plug-ins name none either, or only a
                    // unitless 0…1's Off and On (which the pane reads as Off and On too).
                    None => assert!(
                        p.choices.is_empty()
                            || (p.min == 0.0 && p.max == 1.0 && p.unit.is_empty()),
                        "{kind:?} {}: the plug-ins name {:?} ({locale:?})",
                        p.key,
                        p.choices
                    ),
                }
            }
        }
    }
    assert!(named > 40, "{named}");
}
