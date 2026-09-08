//! Internationalization (ADR 0007). No UI string is written inline: every label goes
//! through [`t`] / [`tf`].

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    Es,
}

impl Locale {
    pub const ALL: [Locale; 2] = [Locale::En, Locale::Es];

    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Es => "es",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Es => "Español",
        }
    }

    pub fn from_code(code: &str) -> Option<Locale> {
        Locale::ALL.into_iter().find(|l| l.code() == code)
    }

    fn source(self) -> &'static str {
        match self {
            Locale::En => include_str!("../locales/en.txt"),
            Locale::Es => include_str!("../locales/es.txt"),
        }
    }
}

type Table = HashMap<&'static str, String>;

fn parse(source: &'static str) -> Table {
    let mut table = HashMap::new();
    for line in source.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            table.insert(k.trim(), v.trim().replace("\\n", "\n"));
        }
    }
    table
}

fn tables() -> &'static HashMap<Locale, Table> {
    static TABLES: OnceLock<HashMap<Locale, Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        Locale::ALL
            .into_iter()
            .map(|l| (l, parse(l.source())))
            .collect()
    })
}

fn current() -> &'static RwLock<Locale> {
    static CURRENT: OnceLock<RwLock<Locale>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(detect_system_locale()))
}

fn detect_system_locale() -> Locale {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LANGUAGE"))
        .unwrap_or_default()
        .to_lowercase();
    if lang.starts_with("es") {
        Locale::Es
    } else {
        Locale::En
    }
}

pub fn locale() -> Locale {
    *current().read().unwrap()
}

pub fn set_locale(locale: Locale) {
    *current().write().unwrap() = locale;
}

/// Look up a key. Falls back to English, then to the key itself (never panics: a missing
/// translation must never take the DAW down).
pub fn t(key: &str) -> String {
    let tables = tables();
    if let Some(v) = tables.get(&locale()).and_then(|t| t.get(key)) {
        return v.clone();
    }
    if let Some(v) = tables.get(&Locale::En).and_then(|t| t.get(key)) {
        return v.clone();
    }
    key.to_string()
}

/// Look up a key and substitute `{name}` placeholders.
pub fn tf(key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut s = t(key);
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), &value.to_string());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_has_every_english_key() {
        let tables = tables();
        let en = &tables[&Locale::En];
        for l in Locale::ALL {
            let table = &tables[&l];
            let missing: Vec<_> = en.keys().filter(|k| !table.contains_key(*k)).collect();
            assert!(missing.is_empty(), "{l:?} is missing {missing:?}");
            let extra: Vec<_> = table.keys().filter(|k| !en.contains_key(*k)).collect();
            assert!(extra.is_empty(), "{l:?} has extra keys {extra:?}");
        }
    }

    #[test]
    fn placeholders_substitute() {
        set_locale(Locale::En);
        assert_eq!(tf("track.new_name", &[("n", &3)]), "Track 3");
        assert_eq!(t("nonexistent.key"), "nonexistent.key");
    }
}
