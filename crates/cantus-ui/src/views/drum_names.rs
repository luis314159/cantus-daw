//! A drum kit's row names in the interface's language (round 28): General MIDI's names for
//! the keys a kit leaves unnamed, and the names a pack gives its drums (DRSKit's labels,
//! written into its instrument at install in English) — both looked up in
//! `locales/areas/drums.*`. A name nothing translates (an SFZ's own group label) is shown
//! as it is.

use crate::i18n::t;

/// The key a label is translated by: `drums.label.` and its words, lower case, joined by
/// `_` ("Hi-hat closed, tip" → `drums.label.hi_hat_closed_tip`).
pub fn label_key(label: &str) -> String {
    let mut k = String::from("drums.label.");
    let mut gap = false;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !k.ends_with('.') {
                k.push('_');
            }
            k.push(c.to_ascii_lowercase());
            gap = false;
        } else {
            gap = true;
        }
    }
    k
}

/// A name as written (a pack's label, or General MIDI's English name for `pitch`) in the
/// interface's language.
pub fn name(raw: &str, pitch: u8) -> String {
    if cantus_core::inst::sampler::gm_drum_name(pitch) == Some(raw) {
        return t(&format!("drums.gm.{pitch}"));
    }
    let key = label_key(raw);
    let tr = t(&key);
    if tr != key {
        tr
    } else {
        raw.to_string()
    }
}

/// A row's name: a drum on several keys carries ` · <pitch>` after its name.
pub fn row_name(raw: &str, pitch: u8) -> String {
    match raw.split_once(" \u{b7} ") {
        Some((head, tail)) => format!("{} \u{b7} {tail}", name(head, pitch)),
        None => name(raw, pitch),
    }
}

/// A group's name in Exemplar's editor (no pitch: only a pack's labels translate).
pub fn group_name(raw: &str) -> String {
    let key = label_key(raw);
    let tr = t(&key);
    if tr != key {
        tr
    } else {
        raw.to_string()
    }
}
