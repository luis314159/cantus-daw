//! `/learn` — one door to everything written about Cantus (round 39, P3): the manual's
//! chapters (web/assets/manual/{en,es}/), the design notes and decisions, the measurements
//! and the press kit, grouped by what you came to do. Every manual link names a file and
//! an anchor that exist in both languages; a test reads the manual to hold that.

use yew::prelude::*;

use crate::lang::{use_lang, Lang};
use crate::top::GITHUB;

/// A link in a group: its label and where it goes, in each language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: (&'static str, &'static str),
    pub to: To,
}

/// Where an item leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    /// A page of the manual and an anchor in it (empty for the top): `(en, es)` each.
    Manual {
        page: &'static str,
        anchor: (&'static str, &'static str),
    },
    /// A folder or file of the public repository's `docs/`.
    Docs(&'static str),
    /// A path on this site.
    Site(&'static str),
}

impl To {
    pub fn href(self, l: Lang) -> String {
        match self {
            To::Manual { page, anchor } => {
                let a = l.t(anchor.0, anchor.1);
                let hash = if a.is_empty() {
                    String::new()
                } else {
                    format!("#{a}")
                };
                format!("/manual/{}/{page}{hash}", l.code())
            }
            To::Docs(path) => format!("{GITHUB}/tree/master/docs/{path}"),
            To::Site(path) => path.to_string(),
        }
    }
}

/// A group of links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: &'static str,
    pub title: (&'static str, &'static str),
    pub lede: (&'static str, &'static str),
    pub items: &'static [Item],
}

const fn m(
    label: (&'static str, &'static str),
    page: &'static str,
    anchor: (&'static str, &'static str),
) -> Item {
    Item {
        label,
        to: To::Manual { page, anchor },
    }
}

const fn top(label: (&'static str, &'static str), page: &'static str) -> Item {
    m(label, page, ("", ""))
}

const fn docs(label: (&'static str, &'static str), path: &'static str) -> Item {
    Item {
        label,
        to: To::Docs(path),
    }
}

const IDX: &str = "index.html";

pub const GROUPS: &[Group] = &[
    Group {
        id: "start",
        title: ("Start here", "Empieza aquí"),
        lede: (
            "From the download to the first sound, in an evening.",
            "De la descarga al primer sonido, en una tarde.",
        ),
        items: &[
            m(
                ("Install", "Instalación"),
                IDX,
                ("1-install", "1-instalación"),
            ),
            m(
                (
                    "First run: set up your audio",
                    "Primer inicio: configura tu audio",
                ),
                IDX,
                (
                    "2-first-run-set-up-your-audio",
                    "2-primer-inicio-configura-tu-audio",
                ),
            ),
            m(
                ("The workspace", "El espacio de trabajo"),
                IDX,
                ("3-the-workspace", "3-el-espacio-de-trabajo"),
            ),
            top(
                (
                    "Cantus on Linux: installing and setting up audio",
                    "Cantus en Linux: instalación y configuración del audio",
                ),
                "linux.html",
            ),
            m(
                (
                    "Known issues and limitations",
                    "Problemas conocidos y limitaciones",
                ),
                IDX,
                (
                    "14-known-issues-and-limitations",
                    "14-problemas-conocidos-y-limitaciones",
                ),
            ),
        ],
    },
    Group {
        id: "record",
        title: ("Record", "Grabar"),
        lede: (
            "Takes, clips and the gear around them.",
            "Tomas, clips y el equipo a su alrededor.",
        ),
        items: &[
            m(
                ("Record audio and MIDI", "Grabar audio y MIDI"),
                IDX,
                ("4-record-audio-and-midi", "4-grabar-audio-y-midi"),
            ),
            top(
                ("The recording mode", "El modo de grabación"),
                "recording-mode.html",
            ),
            m(
                (
                    "Edit: clips, piano roll, automation",
                    "Editar: clips, piano roll, automatización",
                ),
                IDX,
                (
                    "5-edit-clips-piano-roll-automation",
                    "5-editar-clips-piano-roll-automatización",
                ),
            ),
            top(
                ("The audio editor", "El editor de audio"),
                "audio-editor.html",
            ),
            top(
                (
                    "Editing a clip with an ARA plug-in",
                    "Editar un clip con un plug-in ARA",
                ),
                "ara.html",
            ),
            top(
                (
                    "External gear and control surfaces",
                    "Equipo externo y superficies de control",
                ),
                "external-gear.html",
            ),
        ],
    },
    Group {
        id: "mix",
        title: ("Mix", "Mezclar"),
        lede: (
            "The mixer, the master and the finished file.",
            "La mesa, el máster y el archivo terminado.",
        ),
        items: &[
            m(
                (
                    "Mix: the mixer, inserts and sends",
                    "Mezclar: el mezclador, inserciones y envíos",
                ),
                IDX,
                (
                    "6-mix-the-mixer-inserts-and-sends",
                    "6-mezclar-el-mezclador-inserciones-y-envíos",
                ),
            ),
            top(
                (
                    "VCA faders and mixer snapshots",
                    "Faders VCA e instantáneas de la mesa",
                ),
                "mixing-studio.html",
            ),
            m(
                (
                    "Master: the master chain",
                    "Masterizar: la cadena del máster",
                ),
                IDX,
                (
                    "7-master-the-master-chain-fervor-latitudo-culmen",
                    "7-masterizar-la-cadena-del-máster-fervor-latitudo-culmen",
                ),
            ),
            m(("Export", "Exportar"), IDX, ("8-export", "8-exportar")),
            m(
                (
                    "Projects, autosave and settings",
                    "Proyectos, autoguardado y ajustes",
                ),
                IDX,
                (
                    "11-projects-autosave-and-settings",
                    "11-proyectos-autoguardado-y-ajustes",
                ),
            ),
        ],
    },
    Group {
        id: "instruments",
        title: ("Instruments", "Instrumentos"),
        lede: (
            "The instruments, their presets and the sounds.",
            "Los instrumentos, sus presets y los sonidos.",
        ),
        items: &[
            m(
                ("The device picker", "El selector de dispositivos"),
                IDX,
                (
                    "10-instruments-and-effects-the-device-picker",
                    "10-instrumentos-y-efectos-el-selector-de-dispositivos",
                ),
            ),
            top(
                (
                    "Finding and hearing presets",
                    "Encontrar y escuchar presets",
                ),
                "presets.html",
            ),
            top(
                (
                    "Pulsus III: drum kits, a chain per pad",
                    "Pulsus III: kits de batería, una cadena por pad",
                ),
                "drum-kits.html",
            ),
            m(
                (
                    "Sounds: the browser and sound packs",
                    "Sonidos: el explorador y los paquetes de sonidos",
                ),
                IDX,
                (
                    "9-sounds-the-browser-and-sound-packs",
                    "9-sonidos-el-explorador-y-los-paquetes-de-sonidos",
                ),
            ),
            docs(
                (
                    "Each instrument's design notes",
                    "Las notas de diseño de cada instrumento",
                ),
                "instruments",
            ),
        ],
    },
    Group {
        id: "effects",
        title: ("Effects", "Efectos"),
        lede: (
            "What each effect does, and why it is built that way.",
            "Qué hace cada efecto y por qué está hecho así.",
        ),
        items: &[
            m(
                ("Inserting an effect", "Insertar un efecto"),
                IDX,
                (
                    "10-instruments-and-effects-the-device-picker",
                    "10-instrumentos-y-efectos-el-selector-de-dispositivos",
                ),
            ),
            m(
                (
                    "Fervor, Latitudo and Culmen on the master",
                    "Fervor, Latitudo y Culmen en el máster",
                ),
                IDX,
                (
                    "7-master-the-master-chain-fervor-latitudo-culmen",
                    "7-masterizar-la-cadena-del-máster-fervor-latitudo-culmen",
                ),
            ),
            docs(
                (
                    "Each effect's design notes",
                    "Las notas de diseño de cada efecto",
                ),
                "plugins",
            ),
        ],
    },
    Group {
        id: "ai",
        title: ("AI", "IA"),
        lede: (
            "Assistants that work on your computer, and agents you connect.",
            "Asistentes que trabajan en tu computadora y agentes que conectas.",
        ),
        items: &[
            m(
                ("AI agents (MCP)", "Agentes de IA (MCP)"),
                IDX,
                ("ai-agents-mcp", "agentes-de-ia-mcp"),
            ),
            top(
                (
                    "What an AI agent can do in Cantus",
                    "Lo que un agente de IA puede hacer en Cantus",
                ),
                "ai-tools.html",
            ),
            m(
                ("The mastering assistant", "El asistente de mastering"),
                IDX,
                ("the-mastering-assistant", "el-asistente-de-mastering"),
            ),
            top(("Separating stems", "Separar stems"), "stems.html"),
            top(
                ("Convert audio to MIDI", "Convertir audio a MIDI"),
                "audio-to-midi.html",
            ),
            m(
                ("Listening for AI agents", "Escuchar para agentes de IA"),
                IDX,
                ("listening-for-ai-agents", "escuchar-para-agentes-de-ia"),
            ),
        ],
    },
    Group {
        id: "reference",
        title: ("Reference", "Referencia"),
        lede: (
            "The whole manual, the decisions behind it, and the measurements.",
            "El manual entero, las decisiones detrás y las mediciones.",
        ),
        items: &[
            top(
                (
                    "The whole quick-start manual",
                    "El manual de inicio rápido completo",
                ),
                IDX,
            ),
            m(
                ("Keyboard shortcuts", "Atajos de teclado"),
                IDX,
                ("12-keyboard-shortcuts", "12-atajos-de-teclado"),
            ),
            m(
                ("Privacy and updates", "Privacidad y actualizaciones"),
                IDX,
                ("13-privacy-and-updates", "13-privacidad-y-actualizaciones"),
            ),
            m(
                ("Report a bug", "Reportar un error"),
                IDX,
                ("15-report-a-bug", "15-reportar-un-error"),
            ),
            docs(
                (
                    "Every design decision (ADRs, in English)",
                    "Cada decisión de diseño (ADR, en inglés)",
                ),
                "adr",
            ),
            docs(
                (
                    "The scorecard: how Cantus measures against the others",
                    "La tabla de mediciones: cómo se compara Cantus",
                ),
                "scorecard",
            ),
            Item {
                label: ("Press kit", "Kit de prensa"),
                to: To::Site("/press/index.html"),
            },
        ],
    },
];

/// The `/learn` hub.
#[function_component(Learn)]
pub fn learn() -> Html {
    let l = use_lang().lang;
    let t = |p: (&'static str, &'static str)| l.t(p.0, p.1);
    html! {
        <section class="section learn" id="learn" aria-labelledby="learn-title">
            <div class="wrap">
                <h1 id="learn-title">{ l.t("Learn Cantus", "Aprende Cantus") }</h1>
                <p class="learn-lede">{ l.t(
                    "The manual, chapter by chapter, and the notes behind every instrument and effect. The manual is in English and Spanish; the design notes are in English.",
                    "El manual, capítulo por capítulo, y las notas detrás de cada instrumento y efecto. El manual está en inglés y en español; las notas de diseño, en inglés.") }</p>
                <nav class="learn-jump" aria-label={l.t("Topics", "Temas")}>
                    { for GROUPS.iter().map(|g| html! { <a href={format!("#learn-{}", g.id)}>{ t(g.title) }</a> }) }
                </nav>
                <div class="learn-grid">
                    { for GROUPS.iter().map(|g| html! {
                        <section class="learn-group" id={format!("learn-{}", g.id)} aria-labelledby={format!("learn-{}-h", g.id)}>
                            <h2 id={format!("learn-{}-h", g.id)}>{ t(g.title) }</h2>
                            <p>{ t(g.lede) }</p>
                            <ul>
                                { for g.items.iter().map(|i| {
                                    let outside = matches!(i.to, To::Docs(_));
                                    html! {
                                        <li><a href={i.to.href(l)} class={classes!(outside.then_some("is-outside"))}>
                                            { t(i.label) }
                                            if outside { <span class="learn-where">{ " · GitHub" }</span> }
                                        </a></li>
                                    }
                                }) }
                            </ul>
                        </section>
                    }) }
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn web() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn every_manual_link_lands_in_both_languages() {
        for g in GROUPS {
            assert!(!g.items.is_empty(), "{} has no links", g.id);
            for i in g.items {
                let To::Manual { page, anchor } = i.to else {
                    continue;
                };
                for (code, a) in [("en", anchor.0), ("es", anchor.1)] {
                    let file = web().join("assets/manual").join(code).join(page);
                    let text = std::fs::read_to_string(&file)
                        .unwrap_or_else(|_| panic!("{} is missing", file.display()));
                    if !a.is_empty() {
                        assert!(
                            text.contains(&format!("id=\"{a}\"")),
                            "{code}/{page}#{a} does not exist ({})",
                            i.label.0
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_docs_link_is_a_folder_of_the_repository() {
        for g in GROUPS {
            for i in g.items {
                match i.to {
                    To::Docs(p) => assert!(web().join("../docs").join(p).exists(), "docs/{p}"),
                    To::Site(p) => {
                        let local = p.trim_start_matches('/');
                        assert!(
                            web().join("assets").join(local).exists(),
                            "{p} is not in web/assets"
                        )
                    }
                    To::Manual { .. } => {}
                }
            }
        }
    }

    #[test]
    fn hrefs_take_the_language() {
        let i = m(
            ("Install", "Instalación"),
            IDX,
            ("1-install", "1-instalación"),
        );
        assert_eq!(i.to.href(Lang::En), "/manual/en/index.html#1-install");
        assert_eq!(i.to.href(Lang::Es), "/manual/es/index.html#1-instalación");
        assert_eq!(
            top(("", ""), "ara.html").to.href(Lang::Es),
            "/manual/es/ara.html"
        );
    }

    /// The dancer's chapters: the owner took the dancer off the site until it is
    /// reviewed and restarted (BACKLOG §6j); the manual keeps them.
    const WITHDRAWN: &[&str] = &[
        "dancer.html",
        "stage.html",
        "video.html",
        "import-characters.html",
    ];

    #[test]
    fn every_manual_page_is_reached() {
        let linked: Vec<&str> = GROUPS
            .iter()
            .flat_map(|g| g.items)
            .filter_map(|i| match i.to {
                To::Manual { page, .. } => Some(page),
                _ => None,
            })
            .collect();
        for e in std::fs::read_dir(web().join("assets/manual/en")).unwrap() {
            let name = e.unwrap().file_name().into_string().unwrap();
            if name.ends_with(".html") && !WITHDRAWN.contains(&name.as_str()) {
                assert!(
                    linked.contains(&name.as_str()),
                    "manual/en/{name} is not on /learn"
                );
            }
        }
    }
}
