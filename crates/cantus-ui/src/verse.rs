//! A verse at every start (BACKLOG §6c) — Cantus is made with Christian values; its motto
//! is Colossians 3:23.
//!
//! - **The list.** A curated list of passages — music and song, praise, work, gifts and
//!   making, strength, trust, peace, light — in `locales/verses/tools/list.txt`. Each
//!   translation is one file (`locales/verses/<file>.txt`, `REF | text` per line), built
//!   from its source by `tools/build_verses.py`, never typed from memory; sources, dates
//!   and licences are in `locales/verses/README.md`.
//! - **Keyed by reference.** Every translation holds the same passages, so the verse on
//!   screen is a [`Reference`]: when the interface language changes while it shows, the
//!   same passage is shown in the other translation.
//! - **Translations.** English: the World English Bible (public domain; the edition that
//!   says "LORD", as English readers know it). Spanish: the public-domain Reina-Valera
//!   1909 — and the Reina-Valera 1960, the owner's choice, once its publisher's written
//!   permission is in ([`RVR1960_PERMITTED`]; the American Bible Society's terms ask it of
//!   applications, `docs/rvr1960-request.md`). Until then the build neither carries nor
//!   offers the RVR1960 text; with it, the RVR1960 is the default and the 1909 one
//!   setting away.
//! - **On by default, with an off switch** (the owner: so someone who does not share our
//!   beliefs can turn it off): `Settings ▸ General`, and *don't show again* on the verse.
//! - **Never in the way.** The files are compiled in and parsed on first use (a few
//!   hundred short lines); the launch reads one small settings file before the window
//!   opens, to size the splash.

use crate::i18n::{self, Locale};
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::OnceLock;

/// Whether the Reina-Valera 1960's publisher has given Cantus written permission. The
/// American Bible Society's terms allow up to 500 verses without it in non-commercial
/// works, but ask written permission of applications; the request is
/// `docs/rvr1960-request.md`. **Set it to `true` only once the permission is in hand**:
/// the build then carries the RVR1960 file, offers it and makes it the default.
pub const RVR1960_PERMITTED: bool = false;

/// The Spanish translation the verse is taken from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Spanish {
    /// Reina-Valera 1960, within its publisher's permission (the owner's choice).
    Rvr1960,
    /// Reina-Valera 1909, public domain.
    Rv1909,
}

impl Default for Spanish {
    fn default() -> Self {
        if RVR1960_PERMITTED {
            Spanish::Rvr1960
        } else {
            Spanish::Rv1909
        }
    }
}

impl Spanish {
    pub const ALL: [Spanish; 2] = [Spanish::Rvr1960, Spanish::Rv1909];

    /// The choices Settings offers: both once the RVR1960 is permitted, else the 1909 only.
    pub fn offered() -> &'static [Spanish] {
        if RVR1960_PERMITTED {
            &Spanish::ALL
        } else {
            &[Spanish::Rv1909]
        }
    }

    /// The translation this choice reads: a saved RVR1960 reads the 1909 until the
    /// RVR1960 is permitted.
    pub fn translation(self) -> Translation {
        match self {
            Spanish::Rvr1960 if RVR1960_PERMITTED => Translation::Rvr1960,
            Spanish::Rvr1960 | Spanish::Rv1909 => Translation::Rv1909,
        }
    }
}

/// What `settings.json` remembers about the verse.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VerseSettings {
    /// Show a verse at start: on unless the person turned it off.
    pub show: bool,
    pub spanish: Spanish,
}

impl Default for VerseSettings {
    fn default() -> Self {
        VerseSettings {
            show: true,
            spanish: Spanish::default(),
        }
    }
}

/// A translation bundled with Cantus.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Translation {
    Web,
    Rvr1960,
    Rv1909,
}

impl Translation {
    pub const ALL: [Translation; 3] = [Translation::Web, Translation::Rvr1960, Translation::Rv1909];

    /// The language it is in.
    pub fn locale(self) -> Locale {
        match self {
            Translation::Web => Locale::En,
            Translation::Rvr1960 | Translation::Rv1909 => Locale::Es,
        }
    }

    /// The short name shown after the reference.
    pub fn abbrev(self) -> &'static str {
        match self {
            Translation::Web => "WEB",
            Translation::Rvr1960 => "RVR1960",
            Translation::Rv1909 => "RV1909",
        }
    }

    /// The i18n key of its full name.
    pub fn name_key(self) -> &'static str {
        match self {
            Translation::Web => "verse.tr.web",
            Translation::Rvr1960 => "verse.tr.rvr1960",
            Translation::Rv1909 => "verse.tr.rv1909",
        }
    }

    /// The i18n key of its licence line (the RVR1960's acknowledgement, word for word).
    pub fn licence_key(self) -> &'static str {
        match self {
            Translation::Web => "verse.licence.web",
            Translation::Rvr1960 => "verse.licence.rvr1960",
            Translation::Rv1909 => "verse.licence.rv1909",
        }
    }

    /// The bundled file.
    pub fn source(self) -> &'static str {
        match self {
            Translation::Web => include_str!("../locales/verses/en.txt"),
            // Not in the build until its publisher's written permission is in.
            Translation::Rvr1960 if !RVR1960_PERMITTED => "",
            Translation::Rvr1960 => include_str!("../locales/verses/es.txt"),
            Translation::Rv1909 => include_str!("../locales/verses/es-1909.txt"),
        }
    }

    /// Its passages, parsed once. A file that does not parse is a build bug the tests
    /// catch; at run time it counts as empty, and [`Translation::for_locale`] falls back.
    pub fn verses(self) -> &'static [Verse] {
        static PARSED: OnceLock<[Vec<Verse>; 3]> = OnceLock::new();
        let all = PARSED.get_or_init(|| {
            Translation::ALL.map(|t| {
                parse(t.source()).unwrap_or_else(|e| {
                    log::error!("verses: {} does not parse: {e}", t.abbrev());
                    Vec::new()
                })
            })
        });
        let i = Translation::ALL
            .iter()
            .position(|t| *t == self)
            .unwrap_or(0);
        &all[i]
    }

    /// The passage at `r` in this translation.
    pub fn passage(self, r: Reference) -> Option<&'static Verse> {
        self.verses().iter().find(|v| v.reference == r)
    }

    /// The translation for an interface language: English is the WEB; Spanish is the one
    /// chosen, falling back to the public-domain 1909 if the chosen file is empty.
    pub fn for_locale(locale: Locale, spanish: Spanish) -> Translation {
        match locale {
            Locale::En => Translation::Web,
            Locale::Es => {
                let t = spanish.translation();
                if t.verses().is_empty() {
                    Translation::Rv1909
                } else {
                    t
                }
            }
        }
    }

    /// The translation for the interface's language now.
    pub fn current(spanish: Spanish) -> Translation {
        Translation::for_locale(i18n::locale(), spanish)
    }
}

/// A passage: a book (its USFM code, `PSA`), a chapter and a verse or a run of verses.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Reference {
    pub book: &'static str,
    pub chapter: u16,
    pub first: u16,
    pub last: u16,
}

impl Reference {
    /// `PSA 33:3` or `LAM 3:22-23`.
    pub fn parse(s: &'static str) -> Option<Reference> {
        let (book, cv) = s.trim().split_once(' ')?;
        let ok = book.len() == 3
            && book
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        if !ok {
            return None;
        }
        let (c, v) = cv.split_once(':')?;
        let (first, last) = match v.split_once('-') {
            Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
            None => {
                let n = v.parse().ok()?;
                (n, n)
            }
        };
        let chapter: u16 = c.parse().ok()?;
        (chapter > 0 && first > 0 && last >= first).then_some(Reference {
            book,
            chapter,
            first,
            last,
        })
    }

    /// How many verses it quotes.
    pub fn count(self) -> u16 {
        self.last - self.first + 1
    }

    /// The book's name in the interface's language.
    pub fn book_name(self) -> String {
        i18n::t(&format!("verse.book.{}", self.book))
    }

    /// "Colossians 3:23", "Lamentaciones 3:22–23", in the interface's language.
    pub fn label(self) -> String {
        let book = self.book_name();
        if self.first == self.last {
            format!("{book} {}:{}", self.chapter, self.first)
        } else {
            format!(
                "{book} {}:{}\u{2013}{}",
                self.chapter, self.first, self.last
            )
        }
    }
}

/// One passage of one translation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Verse {
    pub reference: Reference,
    pub text: &'static str,
}

/// Parse a verse file: `REF | text` per line; blank lines and `#` comments skipped.
pub fn parse(src: &'static str) -> Result<Vec<Verse>, String> {
    let mut out: Vec<Verse> = Vec::new();
    for (n, line) in src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (r, text) = line
            .split_once(" | ")
            .ok_or_else(|| format!("line {}: no ` | ` between reference and text", n + 1))?;
        let reference =
            Reference::parse(r).ok_or_else(|| format!("line {}: bad reference {r:?}", n + 1))?;
        let text = text.trim();
        if text.is_empty() {
            return Err(format!("line {}: {r} has no text", n + 1));
        }
        if out.iter().any(|v| v.reference == reference) {
            return Err(format!("line {}: {r} twice", n + 1));
        }
        out.push(Verse { reference, text });
    }
    Ok(out)
}

/// Every passage, in the list's order (the English file's: every file holds the same).
pub fn references() -> Vec<Reference> {
    Translation::Web
        .verses()
        .iter()
        .map(|v| v.reference)
        .collect()
}

/// A passage chosen by `seed`.
pub fn pick(seed: u64) -> Option<Reference> {
    let all = Translation::Web.verses();
    if all.is_empty() {
        return None;
    }
    // splitmix64: neighbouring seeds land far apart.
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    Some(all[(z % all.len() as u64) as usize].reference)
}

/// A seed that differs at every start.
pub fn seed_now() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (u64::from(std::process::id()) << 32)
}

/// The verse as it shows: a passage, in whatever language the interface speaks now.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub reference: Reference,
    pub spanish: Spanish,
}

impl Shown {
    pub fn translation(self) -> Translation {
        Translation::current(self.spanish)
    }

    /// The text, in the current language's translation.
    pub fn text(self) -> &'static str {
        let t = self.translation();
        t.passage(self.reference)
            .or_else(|| {
                Translation::Rv1909
                    .passage(self.reference)
                    .filter(|_| t.locale() == Locale::Es)
            })
            .or_else(|| Translation::Web.passage(self.reference))
            .map_or("", |v| v.text)
    }

    /// "Colossians 3:23 · WEB".
    pub fn citation(self) -> String {
        format!(
            "{} \u{00b7} {}",
            self.reference.label(),
            self.translation().abbrev()
        )
    }
}

/// What the launch knows before the window opens: whether to show a verse, and which.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    pub shown: Option<Shown>,
}

impl Launch {
    /// Read the settings (one small file) for the verse switch and the language — the
    /// `--locale` flag wins — and choose a passage. Sets the interface language, as the
    /// settings thread would a moment later, so the splash speaks it from its first frame.
    pub fn read(dir: &Path, locale_flag: Option<Locale>, seed: u64) -> Launch {
        let settings = Settings::load(&crate::settings::settings_path(dir));
        match locale_flag {
            Some(l) => i18n::set_locale(l),
            None => {
                if let Some(l) = settings.locale.as_deref().and_then(Locale::from_code) {
                    i18n::set_locale(l);
                }
            }
        }
        Launch::from_settings(&settings, seed)
    }

    pub fn from_settings(settings: &Settings, seed: u64) -> Launch {
        let shown = settings
            .verse
            .show
            .then(|| pick(seed))
            .flatten()
            .map(|reference| Shown {
                reference,
                spanish: settings.verse.spanish,
            });
        Launch { shown }
    }
}
