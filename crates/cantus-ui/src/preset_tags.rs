//! What a preset is *for* (round 37, ADR 0101): its **styles** — the music it was made for,
//! from the styles most produced today — and its **roles** — the part it plays in a song
//! (an instrument's) or where it sits in a mix (an effect's). The browser filters by them,
//! the agents' `list_presets` too, the audit counts them and the render check picks a
//! preset's test phrase by its first role.
//!
//! A preset carries them as tag keys (`preset.style.*`, `preset.role.*`), beside the older
//! descriptive tags (`preset.tag.keys`, `organ.tag.jazz`, `bass.tag.808`…), which are read
//! here too so the presets written before round 37 sort where they belong.

use cantus_core::{EffectKind, InstrumentKind};

/// A style of music a preset was made for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Style {
    TrapHipHop,
    Pop,
    HouseEdm,
    RnB,
    LoFi,
    Rock,
    Latin,
    Ambient,
    Film,
    JazzSoul,
    Worship,
}

impl Style {
    pub const ALL: [Style; 11] = [
        Style::TrapHipHop,
        Style::Pop,
        Style::HouseEdm,
        Style::RnB,
        Style::LoFi,
        Style::Rock,
        Style::Latin,
        Style::Ambient,
        Style::Film,
        Style::JazzSoul,
        Style::Worship,
    ];

    /// The word a data file and an agent use (`styles = ["trap", "lofi"]`).
    pub fn id(self) -> &'static str {
        match self {
            Style::TrapHipHop => "trap",
            Style::Pop => "pop",
            Style::HouseEdm => "house",
            Style::RnB => "rnb",
            Style::LoFi => "lofi",
            Style::Rock => "rock",
            Style::Latin => "latin",
            Style::Ambient => "ambient",
            Style::Film => "film",
            Style::JazzSoul => "jazz",
            Style::Worship => "worship",
        }
    }

    /// Its tag key, which is also its name's key in the locale files.
    pub fn key(self) -> &'static str {
        match self {
            Style::TrapHipHop => "preset.style.trap",
            Style::Pop => "preset.style.pop",
            Style::HouseEdm => "preset.style.house",
            Style::RnB => "preset.style.rnb",
            Style::LoFi => "preset.style.lofi",
            Style::Rock => "preset.style.rock",
            Style::Latin => "preset.style.latin",
            Style::Ambient => "preset.style.ambient",
            Style::Film => "preset.style.film",
            Style::JazzSoul => "preset.style.jazz",
            Style::Worship => "preset.style.worship",
        }
    }

    pub fn from_id(id: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|s| s.id() == id)
    }

    /// The style a tag names: a `preset.style.*` key, or one of the older descriptive tags
    /// that says a style plainly (Organum's *jazz*, *soul*, *gospel*, *rock*; Gravis's
    /// *trap*).
    pub fn from_tag(tag: &str) -> Option<Style> {
        if let Some(s) = Style::ALL.into_iter().find(|s| s.key() == tag) {
            return Some(s);
        }
        match tag {
            "organ.tag.jazz" | "organ.tag.soul" => Some(Style::JazzSoul),
            "organ.tag.gospel" => Some(Style::Worship),
            "organ.tag.rock" => Some(Style::Rock),
            "bass.tag.trap" | "bass.tag.808" => Some(Style::TrapHipHop),
            _ => None,
        }
    }
}

/// The part a preset plays: an instrument's in a song, an effect's in a mix.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Bass,
    Lead,
    Pad,
    Keys,
    Pluck,
    Fx,
    Drums,
    Vocal,
    Bus,
    Master,
    Creative,
}

impl Role {
    pub const ALL: [Role; 11] = [
        Role::Bass,
        Role::Lead,
        Role::Pad,
        Role::Keys,
        Role::Pluck,
        Role::Fx,
        Role::Drums,
        Role::Vocal,
        Role::Bus,
        Role::Master,
        Role::Creative,
    ];
    /// What an instrument's preset can be.
    pub const INSTRUMENT: [Role; 7] = [
        Role::Bass,
        Role::Lead,
        Role::Pad,
        Role::Keys,
        Role::Pluck,
        Role::Fx,
        Role::Drums,
    ];
    /// Where an effect's preset is meant to go.
    pub const EFFECT: [Role; 5] = [
        Role::Vocal,
        Role::Drums,
        Role::Bus,
        Role::Master,
        Role::Creative,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Role::Bass => "bass",
            Role::Lead => "lead",
            Role::Pad => "pad",
            Role::Keys => "keys",
            Role::Pluck => "pluck",
            Role::Fx => "fx",
            Role::Drums => "drums",
            Role::Vocal => "vocal",
            Role::Bus => "bus",
            Role::Master => "master",
            Role::Creative => "creative",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Role::Bass => "preset.role.bass",
            Role::Lead => "preset.role.lead",
            Role::Pad => "preset.role.pad",
            Role::Keys => "preset.role.keys",
            Role::Pluck => "preset.role.pluck",
            Role::Fx => "preset.role.fx",
            Role::Drums => "preset.role.drums",
            Role::Vocal => "preset.role.vocal",
            Role::Bus => "preset.role.bus",
            Role::Master => "preset.role.master",
            Role::Creative => "preset.role.creative",
        }
    }

    pub fn from_id(id: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.id() == id)
    }

    /// Whether an instrument's (else an effect's) preset may have this role.
    pub fn fits(self, instrument: bool) -> bool {
        if instrument {
            Role::INSTRUMENT.contains(&self)
        } else {
            Role::EFFECT.contains(&self)
        }
    }

    /// The role a tag names: a `preset.role.*` key, or an older descriptive tag (a bell
    /// is plucked, brass leads, strings and choirs pad, an organ is keys, percussion
    /// is drums).
    pub fn from_tag(tag: &str) -> Option<Role> {
        if let Some(r) = Role::ALL.into_iter().find(|r| r.key() == tag) {
            return Some(r);
        }
        match tag {
            "preset.tag.bass" => Some(Role::Bass),
            "preset.tag.lead" | "preset.tag.brass" => Some(Role::Lead),
            "preset.tag.pad" | "preset.tag.strings" => Some(Role::Pad),
            "preset.tag.keys" | "preset.tag.organ" => Some(Role::Keys),
            "preset.tag.pluck" | "preset.tag.bell" => Some(Role::Pluck),
            "preset.tag.fx" => Some(Role::Fx),
            "preset.tag.drum" | "preset.tag.perc" => Some(Role::Drums),
            _ => None,
        }
    }
}

/// The role an instrument plays when its preset names none: what the instrument is.
pub fn natural_role(kind: InstrumentKind) -> Option<Role> {
    match kind {
        InstrumentKind::Bass => Some(Role::Bass),
        InstrumentKind::ElectricPiano | InstrumentKind::Organ | InstrumentKind::Chorda => {
            Some(Role::Keys)
        }
        _ => None,
    }
}

/// The role an effect plays when its preset names none: the mastering units master,
/// the vocal tools sit on voices, the rest is creative.
pub fn natural_effect_role(kind: EffectKind) -> Option<Role> {
    match kind {
        EffectKind::Maximizer | EffectKind::Limiter => Some(Role::Master),
        EffectKind::DeEsser | EffectKind::AutoTune => Some(Role::Vocal),
        EffectKind::Analyzer => Some(Role::Master),
        _ => None,
    }
}

/// The styles and roles a preset's tags name, each once, in the vocabulary's order.
pub fn read_tags(tags: &[&str]) -> (Vec<Style>, Vec<Role>) {
    let mut styles: Vec<Style> = tags.iter().filter_map(|t| Style::from_tag(t)).collect();
    let mut roles: Vec<Role> = tags.iter().filter_map(|t| Role::from_tag(t)).collect();
    styles.sort();
    styles.dedup();
    roles.sort();
    roles.dedup();
    (styles, roles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_reads_back_and_every_key_is_named_in_both_languages() {
        for s in Style::ALL {
            assert_eq!(Style::from_id(s.id()), Some(s));
            assert_eq!(Style::from_tag(s.key()), Some(s));
            for l in crate::i18n::Locale::ALL {
                assert!(crate::i18n::has(l, s.key()), "{} in {l:?}", s.key());
            }
        }
        for r in Role::ALL {
            assert_eq!(Role::from_id(r.id()), Some(r));
            assert_eq!(Role::from_tag(r.key()), Some(r));
            for l in crate::i18n::Locale::ALL {
                assert!(crate::i18n::has(l, r.key()), "{} in {l:?}", r.key());
            }
        }
    }

    #[test]
    fn the_older_tags_sort_where_they_belong() {
        let (s, r) = read_tags(&["preset.tag.keys", "organ.tag.gospel", "preset.tag.keys"]);
        assert_eq!(s, vec![Style::Worship]);
        assert_eq!(r, vec![Role::Keys]);
        let (s, r) = read_tags(&["bass.tag.808", "bass.tag.trap"]);
        assert_eq!(s, vec![Style::TrapHipHop]);
        assert!(r.is_empty(), "Gravis's role comes from what it is");
        assert_eq!(natural_role(InstrumentKind::Bass), Some(Role::Bass));
    }
}
