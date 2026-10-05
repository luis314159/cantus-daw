//! The plug-ins' words: the same locale files the DAW reads (ADR 0007), so a parameter is
//! called in another host what it is called in Cantus.
//!
//! A host has no language negotiation in either standard, so the language is chosen once,
//! when the plug-in is first loaded: `CANTUS_LANG`, then `LANG`, `es…` for Spanish and
//! English otherwise. Hosts identify parameters by id, never by name, so a project moves
//! between languages untouched.

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Es];

    fn sources(self) -> &'static [&'static str] {
        match self {
            Lang::En => &[
                include_str!("../../cantus-ui/locales/en.txt"),
                include_str!("../../cantus-ui/locales/fx/eq.en.txt"),
                include_str!("../../cantus-ui/locales/fx/compressor.en.txt"),
                include_str!("../../cantus-ui/locales/fx/limiter.en.txt"),
                include_str!("../../cantus-ui/locales/fx/space.en.txt"),
                include_str!("../../cantus-ui/locales/fx/multiband.en.txt"),
                include_str!("../../cantus-ui/locales/fx/deesser.en.txt"),
                include_str!("../../cantus-ui/locales/fx/gate.en.txt"),
                include_str!("../../cantus-ui/locales/fx/stereo.en.txt"),
                include_str!("../../cantus-ui/locales/fx/maximizer.en.txt"),
                include_str!("../../cantus-ui/locales/fx/saturator.en.txt"),
                include_str!("../../cantus-ui/locales/areas/export.en.txt"),
            ],
            Lang::Es => &[
                include_str!("../../cantus-ui/locales/es.txt"),
                include_str!("../../cantus-ui/locales/fx/eq.es.txt"),
                include_str!("../../cantus-ui/locales/fx/compressor.es.txt"),
                include_str!("../../cantus-ui/locales/fx/limiter.es.txt"),
                include_str!("../../cantus-ui/locales/fx/space.es.txt"),
                include_str!("../../cantus-ui/locales/fx/multiband.es.txt"),
                include_str!("../../cantus-ui/locales/fx/deesser.es.txt"),
                include_str!("../../cantus-ui/locales/fx/gate.es.txt"),
                include_str!("../../cantus-ui/locales/fx/stereo.es.txt"),
                include_str!("../../cantus-ui/locales/fx/maximizer.es.txt"),
                include_str!("../../cantus-ui/locales/fx/saturator.es.txt"),
                include_str!("../../cantus-ui/locales/areas/export.es.txt"),
            ],
        }
    }
}

type Table = HashMap<&'static str, String>;

/// The same format `cantus_ui::i18n` reads: `key = value`, `#` comments.
fn parse(source: &'static str, into: &mut Table) {
    for line in source.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            into.insert(k.trim(), v.trim().replace("\\n", "\n"));
        }
    }
}

fn table(lang: Lang) -> &'static Table {
    static EN: OnceLock<Table> = OnceLock::new();
    static ES: OnceLock<Table> = OnceLock::new();
    let cell = match lang {
        Lang::En => &EN,
        Lang::Es => &ES,
    };
    cell.get_or_init(|| {
        let mut t = Table::new();
        for s in lang.sources() {
            parse(s, &mut t);
        }
        t
    })
}

/// The language the plug-ins speak in this process.
pub fn lang() -> Lang {
    static LANG: OnceLock<Lang> = OnceLock::new();
    *LANG.get_or_init(|| {
        let wanted = std::env::var("CANTUS_LANG")
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default()
            .to_lowercase();
        if wanted.starts_with("es") {
            Lang::Es
        } else {
            Lang::En
        }
    })
}

/// A key's text in `lang`, falling back to English and then to the key itself.
pub fn t_in(lang: Lang, key: &str) -> String {
    table(lang)
        .get(key)
        .or_else(|| table(Lang::En).get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// A key's text in the process's language.
pub fn t(key: &str) -> String {
    t_in(lang(), key)
}

/// Whether a key exists in `lang` (tests hold every key the plug-ins use to it).
pub fn has(lang: Lang, key: &str) -> bool {
    table(lang).contains_key(key)
}

/// A key's text with `{name}` placeholders filled.
pub fn tf_in(lang: Lang, key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut s = t_in(lang, key);
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), &value.to_string());
    }
    s
}
