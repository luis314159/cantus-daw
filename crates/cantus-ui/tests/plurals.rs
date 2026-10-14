//! Messages that count: one thing reads in the singular, in every language
//! (`i18n::tf_count` picks the `_one` key when there is one).

use cantus_ui::i18n::{self, Locale};

const COUNTED: &[&str] = &[
    "smp.sample_changed",
    "smp.stream_late_tip",
    "smp.cache_unwritable",
];

#[test]
fn one_reads_in_the_singular_and_more_in_the_plural() {
    for l in Locale::ALL {
        i18n::set_locale(l);
        for key in COUNTED {
            let args: &[(&str, &dyn std::fmt::Display)] = &[("error", &"denied")];
            let one = i18n::tf_count(key, 1, args);
            let three = i18n::tf_count(key, 3, args);
            assert!(!one.contains('{'), "{l:?} {key}: {one}");
            assert!(!three.contains('{'), "{l:?} {key}: {three}");
            assert_ne!(
                one,
                i18n::tf(key, &[("n", &1), ("error", &"denied")]),
                "{l:?} {key} has no singular"
            );
            assert!(!one.contains("1 "), "{l:?} {key} reads '1 …': {one}");
            assert!(three.contains('3'), "{l:?} {key}: {three}");
        }
    }
}
