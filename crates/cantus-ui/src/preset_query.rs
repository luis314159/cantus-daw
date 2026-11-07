//! Finding a preset (round 37, ADR 0101): by style, by role and by words. Styles are
//! alternatives (a *trap* or a *lo-fi* preset), so are roles; the two together narrow
//! (a trap *bass*). Every word typed must be found — in the name, a style's or role's
//! name, or the unit's — without regard to case or accents (*electrico* finds
//! *Eléctrico*).

use crate::i18n::t;
use crate::preset_catalog::Entry;
use crate::preset_tags::{Role, Style};
use std::collections::BTreeSet;

/// What the browser asks for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub text: String,
    pub styles: BTreeSet<Style>,
    pub roles: BTreeSet<Role>,
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.styles.is_empty() && self.roles.is_empty()
    }

    /// Turn a style on or off.
    pub fn toggle_style(&mut self, s: Style) {
        if !self.styles.remove(&s) {
            self.styles.insert(s);
        }
    }

    /// Turn a role on or off.
    pub fn toggle_role(&mut self, r: Role) {
        if !self.roles.remove(&r) {
            self.roles.insert(r);
        }
    }

    /// Whether `e` is one of the answers.
    pub fn admits(&self, e: &Entry) -> bool {
        if !self.styles.is_empty() && !e.styles.iter().any(|s| self.styles.contains(s)) {
            return false;
        }
        if !self.roles.is_empty() && !e.roles.iter().any(|r| self.roles.contains(r)) {
            return false;
        }
        let words: Vec<String> = self.text.split_whitespace().map(fold).collect();
        if words.is_empty() {
            return true;
        }
        let mut hay = fold(&e.name);
        // A factory preset's English name too: a producer reading an English tutorial types
        // *tape* in a Spanish interface.
        let english = crate::i18n::t_in(crate::i18n::Locale::En, &e.id);
        if english != e.id && english != e.name {
            hay.push(' ');
            hay.push_str(&fold(&english));
        }
        for s in &e.styles {
            hay.push(' ');
            hay.push_str(&fold(&t(s.key())));
        }
        for r in &e.roles {
            hay.push(' ');
            hay.push_str(&fold(&t(r.key())));
        }
        hay.push(' ');
        hay.push_str(&fold(&t(e.unit.name_key())));
        words.iter().all(|w| hay.contains(w.as_str()))
    }

    /// What each chip would answer if it were clicked now: a style's count is of the
    /// presets the words and the roles let through that have it (styles are alternatives, so
    /// the other styles chosen do not narrow it), a role's likewise — the counts the chips
    /// show, so *Pop 5* beside a search that leaves one pop preset never reads 5.
    pub fn facets(&self, entries: &[Entry]) -> Counts {
        let but_styles = Query {
            styles: BTreeSet::new(),
            ..self.clone()
        };
        let but_roles = Query {
            roles: BTreeSet::new(),
            ..self.clone()
        };
        let for_styles: Vec<Entry> = entries
            .iter()
            .filter(|e| but_styles.admits(e))
            .cloned()
            .collect();
        let for_roles: Vec<Entry> = entries
            .iter()
            .filter(|e| but_roles.admits(e))
            .cloned()
            .collect();
        (counts(&for_styles).0, counts(&for_roles).1)
    }

    /// The indices of the answers in `entries`, in their order.
    pub fn run(&self, entries: &[Entry]) -> Vec<usize> {
        entries
            .iter()
            .enumerate()
            .filter(|(_, e)| self.admits(e))
            .map(|(i, _)| i)
            .collect()
    }
}

/// Each style's count and each role's.
pub type Counts = (Vec<(Style, usize)>, Vec<(Role, usize)>);

/// How many of `entries` have each style and each role — the counts the chips show.
pub fn counts(entries: &[Entry]) -> Counts {
    let styles = Style::ALL
        .into_iter()
        .map(|s| (s, entries.iter().filter(|e| e.styles.contains(&s)).count()))
        .collect();
    let roles = Role::ALL
        .into_iter()
        .map(|r| (r, entries.iter().filter(|e| e.roles.contains(&r)).count()))
        .collect();
    (styles, roles)
}

/// Lower case, without the accents Spanish and the other Latin-script languages put on
/// letters, so a search typed either way finds the name.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset_catalog::{Payload, Source, Unit};
    use cantus_core::EffectKind;

    fn entry(name: &str, styles: &[Style], roles: &[Role]) -> Entry {
        Entry {
            id: name.into(),
            unit: Unit::Effect(EffectKind::Saturator),
            name: name.into(),
            source: Source::Factory,
            styles: styles.to_vec(),
            roles: roles.to_vec(),
            payload: Payload::Effect(Default::default()),
        }
    }

    #[test]
    fn styles_are_alternatives_roles_too_and_the_two_narrow() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let all = vec![
            entry("Tape bus", &[Style::LoFi], &[Role::Bus]),
            entry("Trap vocal", &[Style::TrapHipHop], &[Role::Vocal]),
            entry("Glue", &[Style::Pop, Style::TrapHipHop], &[Role::Bus]),
            entry("Bare", &[], &[]),
        ];
        let mut q = Query::default();
        assert_eq!(q.run(&all), vec![0, 1, 2, 3], "no filter: everything");
        q.toggle_style(Style::TrapHipHop);
        assert_eq!(q.run(&all), vec![1, 2]);
        q.toggle_style(Style::LoFi);
        assert_eq!(q.run(&all), vec![0, 1, 2], "a style or the other");
        q.toggle_role(Role::Bus);
        assert_eq!(q.run(&all), vec![0, 2], "and a bus");
        q.toggle_style(Style::LoFi);
        q.toggle_style(Style::TrapHipHop);
        assert!(q.styles.is_empty(), "a second click turns it off");
        assert_eq!(q.run(&all), vec![0, 2]);
    }

    #[test]
    fn every_word_must_be_found_without_case_or_accents() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let all = vec![
            entry("Piano Eléctrico", &[], &[]),
            entry("Trap vocal", &[Style::TrapHipHop], &[Role::Vocal]),
        ];
        let q = |s: &str| Query {
            text: s.into(),
            ..Default::default()
        };
        assert_eq!(q("ELECTRICO").run(&all), vec![0]);
        assert_eq!(q("piano  eléc").run(&all), vec![0]);
        assert_eq!(q("trap voc").run(&all), vec![1]);
        assert_eq!(q("hip-hop").run(&all), vec![1], "a style's name finds it");
        assert_eq!(q("fervor").run(&all), vec![0, 1], "the unit's name too");
        assert!(q("trap piano").run(&all).is_empty(), "every word, not any");
        assert_eq!(fold("ÑANDÚ Çà"), "nandu ca");
    }

    #[test]
    fn the_chips_count_what_a_click_would_answer() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let all = vec![
            entry("Tape bus", &[Style::LoFi, Style::Pop], &[Role::Bus]),
            entry("Trap vocal", &[Style::TrapHipHop], &[Role::Vocal]),
            entry("Glue", &[Style::Pop], &[Role::Bus]),
            entry("Tape drums", &[Style::LoFi], &[Role::Drums]),
        ];
        let count = |c: &Counts, s: Style| c.0.iter().find(|(x, _)| *x == s).unwrap().1;
        let role = |c: &Counts, r: Role| c.1.iter().find(|(x, _)| *x == r).unwrap().1;
        let mut q = Query::default();
        assert_eq!(
            q.facets(&all),
            counts(&all),
            "nothing chosen: the unit's counts"
        );
        q.text = "tape".into();
        let c = q.facets(&all);
        assert_eq!(
            (count(&c, Style::Pop), count(&c, Style::LoFi)),
            (1, 2),
            "within the search"
        );
        assert_eq!(role(&c, Role::Vocal), 0);
        q.toggle_style(Style::Pop);
        let c = q.facets(&all);
        assert_eq!(
            count(&c, Style::LoFi),
            2,
            "another style is an alternative: not narrowed"
        );
        assert_eq!(
            (role(&c, Role::Bus), role(&c, Role::Drums)),
            (1, 0),
            "a role is narrowed by it"
        );
    }

    #[test]
    fn a_factory_preset_is_found_by_its_english_name_in_any_language() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::Es);
        let id = "preset.sat.tape_bus";
        let mut e = entry(id, &[], &[]);
        e.name = crate::i18n::t(id);
        assert_ne!(
            e.name,
            crate::i18n::t_in(crate::i18n::Locale::En, id),
            "a Spanish name"
        );
        let q = Query {
            text: "tape".into(),
            ..Default::default()
        };
        assert_eq!(q.run(&[e]), vec![0]);
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
    }
}
