//! The site's addresses (round 39, P1-2/P1-3): every page in English at `/…/` and in Spanish
//! at `/es/…/`, each pre-rendered as its own `index.html` (`prerender.rs`) and moved between
//! in the browser without a reload (`router.rs`). One list, [`Page::all`], feeds the
//! pre-renderer, the sitemap and the tests, so a page cannot exist in one and not the other.

use crate::lang::Lang;
use crate::units_data::{EFFECTS, INSTRUMENTS};

#[cfg(any(feature = "ssr", test))] // the pre-renderer's and the sitemap's
pub const ORIGIN: &str = "https://cantusdaw.com";

/// Which page, whatever the language.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    /// The home: at most nine chapters.
    Home,
    /// Every system's download, the first steps and the ways to support.
    Download,
    /// Every instrument's and effect's card (`/instruments/`; `/effects/` shows the same).
    Instruments,
    Effects,
    /// One instrument's page; the slug is its `units_data` `file`.
    Instrument(&'static str),
    /// One effect's page.
    Effect(&'static str),
    /// The scorecard in full.
    Measured,
    /// Recording mode, the professional studio, session view, score, engine, every feature.
    Studio,
    Ai,
    Roadmap,
    /// The manual, tutorials and design notes (`web-extra`'s `pages::learn`).
    Learn,
}

/// A page in a language: one address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Page {
    pub lang: Lang,
    pub route: Route,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // read by the browser only
fn slug(list: &'static [crate::units::Unit], s: &str) -> Option<&'static str> {
    list.iter().find(|u| u.file == s).map(|u| u.file)
}

impl Route {
    /// The path after the language prefix, with its trailing slash (`""` for the home).
    pub fn tail(self) -> String {
        match self {
            Route::Home => String::new(),
            Route::Download => "download/".into(),
            Route::Instruments => "instruments/".into(),
            Route::Effects => "effects/".into(),
            Route::Instrument(s) => format!("instruments/{s}/"),
            Route::Effect(s) => format!("effects/{s}/"),
            Route::Measured => "measured/".into(),
            Route::Studio => "studio/".into(),
            Route::Ai => "ai/".into(),
            Route::Roadmap => "roadmap/".into(),
            Route::Learn => "learn/".into(),
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // read by the browser only
    fn parse_tail(parts: &[&str]) -> Option<Route> {
        Some(match parts {
            [] => Route::Home,
            ["download"] => Route::Download,
            ["instruments"] => Route::Instruments,
            ["effects"] => Route::Effects,
            ["instruments", s] => Route::Instrument(slug(&INSTRUMENTS, s)?),
            ["effects", s] => Route::Effect(slug(&EFFECTS, s)?),
            ["measured"] => Route::Measured,
            ["studio"] => Route::Studio,
            ["ai"] => Route::Ai,
            ["roadmap"] => Route::Roadmap,
            ["learn"] => Route::Learn,
            _ => return None,
        })
    }

    /// Every route, in the sitemap's order.
    #[cfg(any(feature = "ssr", test))] // the pre-renderer's and the sitemap's
    pub fn all() -> Vec<Route> {
        let mut v = vec![
            Route::Home,
            Route::Download,
            Route::Measured,
            Route::Studio,
            Route::Ai,
            Route::Roadmap,
            Route::Learn,
            Route::Instruments,
            Route::Effects,
        ];
        v.extend(INSTRUMENTS.iter().map(|u| Route::Instrument(u.file)));
        v.extend(EFFECTS.iter().map(|u| Route::Effect(u.file)));
        v
    }
}

impl Page {
    pub const fn new(lang: Lang, route: Route) -> Self {
        Page { lang, route }
    }

    /// The path, from the root: `/`, `/es/`, `/download/`, `/es/instruments/unda/`.
    pub fn path(self) -> String {
        match self.lang {
            Lang::En => format!("/{}", self.route.tail()),
            Lang::Es => format!("/es/{}", self.route.tail()),
        }
    }

    /// The full address, for canonical links, hreflang and the sitemap.
    #[cfg(any(feature = "ssr", test))] // the pre-renderer's and the sitemap's
    pub fn url(self) -> String {
        format!("{ORIGIN}{}", self.path())
    }

    /// The same page in the other language.
    pub fn in_lang(self, lang: Lang) -> Page {
        Page { lang, ..self }
    }

    /// A path as the browser has it (`/es/download`, `/es/download/`, `/download/index.html`),
    /// or `None` when it is no page of the app's.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // read by the browser only
    pub fn parse(path: &str) -> Option<Page> {
        let path = path.split(['?', '#']).next().unwrap_or("");
        let mut parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.last() == Some(&"index.html") {
            parts.pop();
        }
        let (lang, rest) = match parts.first() {
            Some(&"es") => (Lang::Es, &parts[1..]),
            Some(&"en") => (Lang::En, &parts[1..]),
            _ => (Lang::En, &parts[..]),
        };
        Route::parse_tail(rest).map(|route| Page { lang, route })
    }

    /// Every page in both languages.
    #[cfg(any(feature = "ssr", test))] // the pre-renderer's and the sitemap's
    pub fn all() -> Vec<Page> {
        Route::all()
            .into_iter()
            .flat_map(|r| [Page::new(Lang::En, r), Page::new(Lang::Es, r)])
            .collect()
    }

    /// The home's chapter `id` on this page's language: `/es/#faq`.
    pub fn home_anchor(lang: Lang, id: &str) -> String {
        format!("{}#{id}", Page::new(lang, Route::Home).path())
    }

    /// The page's `<title>`.
    pub fn title(self) -> String {
        let l = self.lang;
        let tail = " · Cantus™";
        match self.route {
            Route::Home => l
                .t(
                    "Cantus™ — a free, open-source DAW",
                    "Cantus™ — un DAW libre y de código abierto",
                )
                .to_string(),
            Route::Download => format!("{}{tail}", l.t("Download", "Descargar")),
            Route::Instruments => format!("{}{tail}", l.t("Instruments", "Instrumentos")),
            Route::Effects => format!("{}{tail}", l.t("Effects", "Efectos")),
            Route::Instrument(_) | Route::Effect(_) => {
                let u = unit(self.route).expect("a unit route has its unit");
                format!("{} — {}{tail}", u.name, l.t(u.kind.0, u.kind.1))
            }
            Route::Measured => format!(
                "{}{tail}",
                l.t("Measured, not claimed", "Medido, no prometido")
            ),
            Route::Studio => format!("{}{tail}", l.t("The studio", "El estudio")),
            Route::Ai => format!(
                "{}{tail}",
                l.t("AI on your computer", "IA en tu computadora")
            ),
            Route::Roadmap => format!("{}{tail}", l.t("Roadmap", "Hoja de ruta")),
            Route::Learn => format!("{}{tail}", l.t("Learn", "Aprende")),
        }
    }

    /// The page's meta description: one or two sentences, only what is built.
    pub fn description(self) -> String {
        let l = self.lang;
        let s = match self.route {
            Route::Home => l.t(
                "Cantus is a free, open-source DAW written in Rust: arrangement, piano roll, channel rack, a mixer, instruments and effects of its own, and a recording mode that bakes the song so only your take is live. Windows and Linux, macOS in preview. No account, no subscription, no telemetry.",
                "Cantus es un DAW libre y de código abierto escrito en Rust: arreglo, piano roll, channel rack, un mezclador, instrumentos y efectos propios, y un modo de grabación que pre-renderiza la canción para que solo tu toma esté en vivo. Windows y Linux, macOS en vista previa. Sin cuenta, sin suscripción, sin telemetría.",
            ),
            Route::Download => l.t(
                "Download Cantus for Windows, Linux or the macOS preview: the newest beta, its checksums, what you need and your first steps. Free, with no account.",
                "Descarga Cantus para Windows, Linux o la vista previa de macOS: la beta más reciente, sus sumas, lo que necesitas y tus primeros pasos. Gratis, sin cuenta.",
            ),
            Route::Instruments | Route::Effects => l.t(
                "Every instrument and effect of Cantus's own: synths, a sampler, drums, an organ, a piano, and effects from EQ to reverb, each with its window, its presets heard and its own page. All free.",
                "Cada instrumento y efecto propio de Cantus: sintetizadores, un sampler, batería, un órgano, un piano y efectos del EQ a la reverb, cada uno con su ventana, sus presets para escuchar y su propia página. Todos gratis.",
            ),
            Route::Instrument(_) | Route::Effect(_) => {
                let u = unit(self.route).expect("a unit route has its unit");
                return format!(
                    "{} — {}. {}",
                    u.name,
                    l.t(u.kind.0, u.kind.1),
                    l.t(u.pitch.0, u.pitch.1)
                );
            }
            Route::Measured => l.t(
                "Cantus measured against FL Studio and REAPER on the same machine, like against like: the rows it wins and the row it loses, with the method and the raw numbers.",
                "Cantus medido frente a FL Studio y REAPER en la misma máquina, en igualdad de condiciones: las filas que gana y la que pierde, con el método y los números en bruto.",
            ),
            Route::Studio => l.t(
                "The recording mode, VCA faders, snapshots, an audio editor, external gear, control surfaces, ARA 2, the session view, the score and an engine that does not blink.",
                "El modo de grabación, faders VCA, snapshots, un editor de audio, equipo externo, superficies de control, ARA 2, la vista de sesión, la partitura y un motor que no parpadea.",
            ),
            Route::Ai => l.t(
                "AI that works with you, on your computer: an MCP server that lets an assistant operate Cantus with your permission, stem separation, mastering and audio to MIDI.",
                "IA que trabaja contigo, en tu computadora: un servidor MCP que deja a un asistente operar Cantus con tu permiso, separación de pistas, masterización y audio a MIDI.",
            ),
            Route::Roadmap => l.t(
                "Where Cantus is and where it goes: what works today, the beta's last mile and what comes later, from the project's own status notes.",
                "Dónde está Cantus y a dónde va: lo que funciona hoy, el último tramo de la beta y lo que viene después, de las notas de estado del propio proyecto.",
            ),
            Route::Learn => l.t(
                "Learn Cantus: the manual in English and Spanish, from install to a finished song, the design notes and the press kit.",
                "Aprende Cantus: el manual en inglés y español, de la instalación a una canción terminada, las notas de diseño y el kit de prensa.",
            ),
        };
        s.to_string()
    }
}

/// The instrument or effect a unit route shows.
pub fn unit(route: Route) -> Option<&'static crate::units::Unit> {
    match route {
        Route::Instrument(s) => INSTRUMENTS.iter().find(|u| u.file == s),
        Route::Effect(s) => EFFECTS.iter().find(|u| u.file == s),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_parses_back_to_itself() {
        for p in Page::all() {
            assert_eq!(Page::parse(&p.path()), Some(p), "{}", p.path());
            let bare = p.path().trim_end_matches('/').to_string();
            assert_eq!(Page::parse(&bare), Some(p), "{bare}");
            assert_eq!(Page::parse(&format!("{}index.html", p.path())), Some(p));
        }
    }

    #[test]
    fn unknown_addresses_are_no_page() {
        for p in ["/nope/", "/instruments/nope/", "/es/es/", "/download/x/"] {
            assert_eq!(Page::parse(p), None, "{p}");
        }
    }

    #[test]
    fn homes_and_languages() {
        assert_eq!(Page::new(Lang::En, Route::Home).path(), "/");
        assert_eq!(Page::new(Lang::Es, Route::Home).path(), "/es/");
        assert_eq!(
            Page::new(Lang::Es, Route::Instrument("unda")).url(),
            "https://cantusdaw.com/es/instruments/unda/"
        );
        assert_eq!(Page::home_anchor(Lang::Es, "faq"), "/es/#faq");
    }

    #[test]
    fn every_page_has_its_own_title_and_description() {
        let all = Page::all();
        // Unique within a language (an effect may be called the same in both, "Phaser").
        let titles: std::collections::HashSet<_> =
            all.iter().map(|p| (p.lang, p.title())).collect();
        assert_eq!(titles.len(), all.len());
        for p in all {
            assert!(p.description().len() > 40, "{:?}", p);
        }
    }
}
