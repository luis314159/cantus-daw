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

    /// The main table, then one file per plug-in (`locales/fx/`), so each plug-in's
    /// strings are owned by its own file.
    fn sources(self) -> &'static [&'static str] {
        match self {
            Locale::En => &[
                include_str!("../locales/en.txt"),
                include_str!("../locales/fx/eq.en.txt"),
                include_str!("../locales/fx/compressor.en.txt"),
                include_str!("../locales/fx/limiter.en.txt"),
                include_str!("../locales/fx/space.en.txt"),
                include_str!("../locales/fx/multiband.en.txt"),
                include_str!("../locales/fx/deesser.en.txt"),
                include_str!("../locales/fx/gate.en.txt"),
                include_str!("../locales/areas/host.en.txt"),
                include_str!("../locales/areas/export.en.txt"),
                include_str!("../locales/areas/engine.en.txt"),
                include_str!("../locales/areas/interop.en.txt"),
                include_str!("../locales/areas/instruments.en.txt"),
                include_str!("../locales/inst/wavetable.en.txt"),
                include_str!("../locales/inst/analog.en.txt"),
                include_str!("../locales/inst/fm.en.txt"),
                include_str!("../locales/inst/sampler.en.txt"),
                include_str!("../locales/inst/bass.en.txt"),
                include_str!("../locales/inst/epiano.en.txt"),
                include_str!("../locales/inst/organ.en.txt"),
                include_str!("../locales/inst/pulsus.en.txt"),
                include_str!("../locales/areas/kit.en.txt"),
                include_str!("../locales/areas/workspace.en.txt"),
                include_str!("../locales/areas/mixer.en.txt"),
                include_str!("../locales/areas/inspector.en.txt"),
                include_str!("../locales/areas/effects.en.txt"),
                include_str!("../locales/areas/views.en.txt"),
                include_str!("../locales/fx/saturator.en.txt"),
                include_str!("../locales/fx/stereo.en.txt"),
                include_str!("../locales/fx/maximizer.en.txt"),
                include_str!("../locales/areas/recording.en.txt"),
                include_str!("../locales/areas/audioclips.en.txt"),
                include_str!("../locales/areas/master.en.txt"),
                include_str!("../locales/areas/demos.en.txt"),
                include_str!("../locales/areas/devices.en.txt"),
            ],
            Locale::Es => &[
                include_str!("../locales/es.txt"),
                include_str!("../locales/fx/eq.es.txt"),
                include_str!("../locales/fx/compressor.es.txt"),
                include_str!("../locales/fx/limiter.es.txt"),
                include_str!("../locales/fx/space.es.txt"),
                include_str!("../locales/fx/multiband.es.txt"),
                include_str!("../locales/fx/deesser.es.txt"),
                include_str!("../locales/fx/gate.es.txt"),
                include_str!("../locales/areas/host.es.txt"),
                include_str!("../locales/areas/export.es.txt"),
                include_str!("../locales/areas/engine.es.txt"),
                include_str!("../locales/areas/interop.es.txt"),
                include_str!("../locales/areas/instruments.es.txt"),
                include_str!("../locales/inst/wavetable.es.txt"),
                include_str!("../locales/inst/analog.es.txt"),
                include_str!("../locales/inst/fm.es.txt"),
                include_str!("../locales/inst/sampler.es.txt"),
                include_str!("../locales/inst/bass.es.txt"),
                include_str!("../locales/inst/epiano.es.txt"),
                include_str!("../locales/inst/organ.es.txt"),
                include_str!("../locales/inst/pulsus.es.txt"),
                include_str!("../locales/areas/kit.es.txt"),
                include_str!("../locales/areas/workspace.es.txt"),
                include_str!("../locales/areas/mixer.es.txt"),
                include_str!("../locales/areas/inspector.es.txt"),
                include_str!("../locales/areas/effects.es.txt"),
                include_str!("../locales/areas/views.es.txt"),
                include_str!("../locales/fx/saturator.es.txt"),
                include_str!("../locales/fx/stereo.es.txt"),
                include_str!("../locales/fx/maximizer.es.txt"),
                include_str!("../locales/areas/recording.es.txt"),
                include_str!("../locales/areas/audioclips.es.txt"),
                include_str!("../locales/areas/master.es.txt"),
                include_str!("../locales/areas/demos.es.txt"),
                include_str!("../locales/areas/devices.es.txt"),
            ],
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
            .map(|l| {
                let mut table = Table::new();
                for source in l.sources() {
                    table.extend(parse(source));
                }
                (l, table)
            })
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

thread_local! {
    /// The language this thread has chosen, over the process-wide one. The application
    /// sets both from its one UI thread, so for it they never differ; tests run on many
    /// threads at once, and each one keeps the language it asked for instead of racing
    /// the others (and the machine's `LANG`).
    static THIS_THREAD: std::cell::Cell<Option<Locale>> = const { std::cell::Cell::new(None) };
}

pub fn locale() -> Locale {
    THIS_THREAD
        .with(|l| l.get())
        .unwrap_or_else(|| *current().read().unwrap())
}

/// Change the interface language: for the process, and for this thread.
pub fn set_locale(locale: Locale) {
    *current().write().unwrap() = locale;
    THIS_THREAD.with(|l| l.set(Some(locale)));
}

/// Give this thread `locale` unless it already chose one. For test helpers: a test that
/// reads painted text gets English whatever the machine's `LANG`, and a test that switched
/// to Spanish keeps it.
pub fn default_thread_locale(locale: Locale) {
    THIS_THREAD.with(|l| {
        if l.get().is_none() {
            l.set(Some(locale));
        }
    });
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
