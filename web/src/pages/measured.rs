//! Measured, not claimed (round 39, Mock 3): the scorecard as paired bars — each program
//! beside the one it is fair to compare it with — the row Cantus loses drawn as plainly as the
//! ones it wins, and the feature matrix's summary with a link to all 97 rows. Every number is
//! `docs/scorecard/README.md`'s (2026-11-09 and 2026-11-11) or `features.md`'s (2026-11-04).
//!
//! [`MeasuredSection`] is the home's chapter; [`MeasuredPage`] is `/measured`: the same, then
//! the method, the capacity estimate and the 22 rows a leader has more.

use yew::prelude::*;

use crate::lang::{use_lang, Lang};
use crate::top::GITHUB;

/// One bar: (who, value as shown, value to draw, is Cantus).
type Bar = (&'static str, &'static str, f32, bool);

/// One card: (won, title, unit line, [(pair's label, [Bar; 2]); 2], the note under it).
struct Card {
    won: bool,
    title: &'static str,
    unit: &'static str,
    pairs: [(&'static str, [Bar; 2]); 2],
    note: &'static str,
}

fn cards(l: Lang) -> [Card; 4] {
    let empty = l.t("An empty song", "Una canción vacía");
    let beat = l.t("A beat starter", "Una plantilla de beat");
    let c = "Cantus";
    let lower = |en: &'static str, es: &'static str| l.t(en, es);
    [
        Card {
            won: true,
            title: l.t("Window ready", "Ventana lista"),
            unit: lower("seconds · lower is better", "segundos · menos es mejor"),
            pairs: [
                (empty, [(c, l.t("0.76", "0,76"), 0.76, true), ("REAPER", l.t("1.76", "1,76"), 1.76, false)]),
                (beat, [(c, l.t("0.77", "0,77"), 0.77, true), ("FL Studio", l.t("2.49", "2,49"), 2.49, false)]),
            ],
            note: l.t("2.3× sooner than REAPER, 3.2× sooner than FL Studio.", "2,3× antes que REAPER, 3,2× antes que FL Studio."),
        },
        Card {
            won: true,
            title: l.t("Quiet", "En calma"),
            unit: lower("seconds · lower is better", "segundos · menos es mejor"),
            pairs: [
                (empty, [(c, l.t("1.5", "1,5"), 1.5, true), ("REAPER", l.t("2.0", "2,0"), 2.0, false)]),
                (beat, [(c, l.t("1.5", "1,5"), 1.5, true), ("FL Studio", l.t("5.8", "5,8"), 5.8, false)]),
            ],
            note: l.t("Done working after it opens: under a tenth of a core, sooner.", "Termina de trabajar tras abrir: menos de una décima de núcleo, antes."),
        },
        Card {
            won: true,
            title: l.t("At rest", "En reposo"),
            unit: lower("% of a core · lower is better", "% de un núcleo · menos es mejor"),
            pairs: [
                (empty, [(c, l.t("1.1–1.5", "1,1–1,5"), 1.5, true), ("REAPER", l.t("2.3–3.7", "2,3–3,7"), 3.7, false)]),
                (beat, [(c, l.t("1.7–2.0", "1,7–2,0"), 2.0, true), ("FL Studio", l.t("6.6–6.7", "6,6–6,7"), 6.7, false)]),
            ],
            note: l.t("Every channel that hears silence stops running, to the same bits.", "Cada canal que oye silencio deja de procesar, con los mismos bits."),
        },
        Card {
            won: false,
            title: l.t("Memory", "Memoria"),
            unit: lower("MB · lower is better", "MB · menos es mejor"),
            pairs: [
                (empty, [(c, "140", 140.0, true), ("REAPER", "62", 62.0, false)]),
                (beat, [(c, "143", 143.0, true), ("FL Studio", "133", 133.0, false)]),
            ],
            note: l.t("Round 39 made the engine's reservation grow with the song: 237 MB became 140 on an empty song (−41 %), 239 became 143 on a starter. About a fifth of what remains is Cantus's own live memory; the rest — the graphics driver's buffers, thread stacks, the audio drivers' heaps — is the work ahead. Still lost, by 78 MB and 10 MB.",
                      "La ronda 39 hizo que la reserva del motor crezca con la canción: 237 MB pasaron a 140 con una canción vacía (−41 %), 239 a 143 con una plantilla. Cerca de una quinta parte de lo que queda es memoria viva de Cantus; el resto — los búferes del driver de gráficos, las pilas de los hilos, los heaps de los drivers de audio — es el trabajo que sigue. Aún perdido, por 78 MB y 10 MB."),

        },
    ]
}

fn card(l: Lang, c: &Card) -> Html {
    let pairs = c.pairs.iter().map(|(label, bars)| {
        let max = bars.iter().map(|b| b.2).fold(0.0f32, f32::max).max(f32::EPSILON);
        let rows = bars.iter().map(|(who, shown, v, ours)| {
            html! {
                <li class={classes!("bar", ours.then_some("ours"))}>
                    <span class="who">{ *who }</span>
                    <span class="track" aria-hidden="true"><i style={format!("width:{:.1}%", v / max * 100.0)}></i></span>
                    <span class="val">{ *shown }</span>
                </li>
            }
        });
        html! { <div class="pair"><p class="pair-l">{ *label }</p><ul>{ for rows }</ul></div> }
    });
    html! {
        <article class={classes!("m-card", (!c.won).then_some("lost"))}>
            <span class="tag">{ if c.won { l.t("Won", "Ganado") } else { l.t("Lost", "Perdido") } }</span>
            <header><h3>{ c.title }</h3><span class="unit">{ c.unit }</span></header>
            { for pairs }
            <p class="m-note">{ c.note }</p>
        </article>
    }
}

/// The feature matrix's summary: 97 rows, 22 where a leader has more, 3 no commercial leader
/// has (docs/scorecard/features.md).
fn matrix(l: Lang, rows: usize) -> Html {
    // (feature, how many of the eight commercial leaders have it)
    // (Slicer, browser previews and per-pad drum outputs were in the 22 and are built since,
    // round 37; they are not listed as missing.)
    let more: [(&str, &str, u8); 8] = [
        ("Convolution reverb", "Reverb por convolución", 7),
        (
            "Ripple edit, insert time",
            "Edición ripple, insertar tiempo",
            6,
        ),
        ("Clip groups", "Grupos de clips", 6),
        ("AU hosting on macOS", "Alojar AU en macOS", 5),
        ("Video track", "Pista de vídeo", 5),
        ("ARA 2, whole", "ARA 2, completo", 5),
        ("Surround / Atmos", "Envolvente / Atmos", 4),
        (
            "Granular / spectral sampler",
            "Sampler granular / espectral",
            4,
        ),
    ];
    let more = more.iter().take(rows).map(|(en, es, n)| {
        html! { <li><span>{ l.t(en, es) }</span><span class="n">{ format!("{n} {}", l.t("leaders", "líderes")) }</span></li> }
    });
    let only = [
        (
            l.t(
                "An agent interface over MCP, each change one undo step",
                "Una interfaz para agentes por MCP, cada cambio un paso de deshacer",
            ),
            l.t("only", "solo"),
        ),
        (
            l.t(
                "Open source, at this class",
                "Código abierto, en esta clase",
            ),
            l.t("only", "solo"),
        ),
        (
            l.t(
                "Offline equals real time, the same bits on three systems",
                "Sin conexión igual a tiempo real, los mismos bits en tres sistemas",
            ),
            l.t("tested", "probado"),
        ),
    ];
    let only = only.iter().map(
        |(what, tag)| html! { <li><span>{ *what }</span><span class="n ours">{ *tag }</span></li> },
    );
    html! {
        <div class="matrix">
            <article class="mx-card">
                <h3>{ l.t("Features, against twelve DAWs", "Funciones, frente a doce DAW") }</h3>
                <p class="big"><b>{ "97" }</b>{ l.t(" rows, each from the vendor's own manual", " filas, cada una del manual del propio fabricante") }</p>
                <p class="small">{ l.t("FL Studio, Live, Bitwig, Logic, Cubase, Pro Tools, Studio Pro, REAPER and four free DAWs. Checked 2026-11-04; re-run every release.",
                                       "FL Studio, Live, Bitwig, Logic, Cubase, Pro Tools, Studio Pro, REAPER y cuatro DAW libres. Revisado el 2026-11-04; se repite en cada versión.") }</p>
            </article>
            <article class="mx-card">
                <h3>{ l.t("22 rows where a leader has more", "22 filas donde un líder tiene más") }<small>{ l.t(" (missing or partial in Cantus, 2026-11-04)", " (faltan o están a medias en Cantus, 2026-11-04)") }</small></h3>
                <ul class="mx-list">{ for more }</ul>
            </article>
            <article class="mx-card">
                <h3>{ l.t("3 rows no commercial leader has", "3 filas que ningún líder comercial tiene") }</h3>
                <ul class="mx-list">{ for only }</ul>
            </article>
        </div>
    }
}

fn chips(l: Lang) -> Html {
    html! {
        <ul class="chips">
            <li>{ "Ryzen 7 7735HS · 31 GB" }</li>
            <li>{ "Windows 11" }</li>
            <li>{ "2026-11-09 · 2026-11-11" }</li>
            <li>{ l.t("beta.14 · REAPER 7.85 · FL Studio 24.1.2", "beta.14 · REAPER 7.85 · FL Studio 24.1.2") }</li>
        </ul>
    }
}

fn head(l: Lang, page: bool) -> Html {
    let title = html! { <>{ l.t("Faster to start.", "Arranca antes.") }<br />{ l.t("Quieter at rest.", "Descansa en calma.") }<br />{ l.t("Heavier on memory.", "Pesa más en memoria.") }</> };
    html! {
        <div class="m-head">
            <div>
                <span class="eyebrow">{ l.t("Measured, not claimed", "Medido, no prometido") }</span>
                if page { <h1>{ title }</h1> } else { <h2>{ title }</h2> }
            </div>
            <div>
                <p class="lede">{ l.t("Cantus, REAPER and FL Studio, five starts each on the same quiet laptop (Cantus beta.14 on 11 November, the others on 9 November), watched by the same script — each beside what it is fair to compare it with. Where Cantus loses, it says so here first.",
                                      "Cantus, REAPER y FL Studio, cinco arranques cada uno en la misma laptop sin nada más abierto (Cantus beta.14 el 11 de noviembre, los otros el 9 de noviembre), observados por el mismo script — cada uno junto a lo que es justo compararlo. Donde Cantus pierde, aquí lo dice primero.") }</p>
                { chips(l) }
            </div>
        </div>
    }
}

fn body(l: Lang, rows: usize) -> Html {
    let cs = cards(l);
    html! {
        <>
            <div class="m-cards">{ for cs.iter().map(|c| card(l, c)) }</div>
            { matrix(l, rows) }
        </>
    }
}

fn links(l: Lang) -> Html {
    html! {
        <p class="m-foot">
            <span>{ l.t("The script, the raw numbers and the whole matrix are in the repository. Run it on your machine.", "El script, los números crudos y la matriz completa están en el repositorio. Ejecútalo en tu máquina.") }</span>
            <span class="m-links">
                <a href={format!("{GITHUB}/blob/master/docs/scorecard/README.md")}>{ l.t("How it was measured →", "Cómo se midió →") }</a>
                <a href={format!("{GITHUB}/blob/master/docs/scorecard/rows.md")}>{ l.t("All 97 rows →", "Las 97 filas →") }</a>
            </span>
        </p>
    }
}

/// The home's chapter.
#[function_component(MeasuredSection)]
pub fn measured_section() -> Html {
    let l = use_lang().lang;
    html! {
        <section class="section measured" id="measured">
            <div class="wrap">
                { head(l, false) }
                { body(l, 5) }
                <p class="m-foot"><span>{ l.t("The method, the capacity estimate and every row a leader has more:", "El método, la estimación de capacidad y cada fila donde un líder tiene más:") }</span>
                    <span class="m-links"><a href={crate::nav::href(l, "/measured")}>{ l.t("The whole scorecard →", "El marcador completo →") }</a></span></p>
            </div>
        </section>
    }
}

/// `/measured`: the chapter, then how it was measured.
#[function_component(MeasuredPage)]
pub fn measured_page() -> Html {
    let l = use_lang().lang;
    html! {
        <>
        <div class="page-band lapis"><div class="wrap">{ head(l, true) }</div></div>
        <section class="section measured" id="measured">
            <div class="wrap">
                { body(l, 8) }
                <div class="m-method">
                    <article>
                        <h3>{ l.t("How it was measured", "Cómo se midió") }</h3>
                        <ul>
                            <li>{ l.t("Five measured starts of each program after one warm-up, each closed politely, by tools/scorecard/startup.ps1.", "Cinco arranques medidos de cada programa tras uno de calentamiento, cada uno cerrado con cortesía, con tools/scorecard/startup.ps1.") }</li>
                            <li>{ l.t("Window ready: from the start to a main window that is visible and answers (not a splash, not a dialog).", "Ventana lista: del arranque a una ventana principal visible que responde (no una pantalla de carga ni un diálogo).") }</li>
                            <li>{ l.t("Quiet: the program uses under a tenth of one core. At rest: one core's share from 30 s to 40 s after the start, by thread time and by cycles; both are published.", "En calma: el programa usa menos de una décima de un núcleo. En reposo: la parte de un núcleo de 30 s a 40 s tras arrancar, por tiempo de hilo y por ciclos; se publican ambas.") }</li>
                            <li>{ l.t("Like against like: Cantus on an empty song beside REAPER's empty project, on its Beat starter beside FL Studio's template.", "Lo comparable con lo comparable: Cantus con una canción vacía junto al proyecto vacío de REAPER, con su plantilla Beat junto a la plantilla de FL Studio.") }</li>
                            <li>{ l.t("AMD Ryzen 7 7735HS, 31 GB, Windows 11, High performance plan, nothing else open. FL Studio is a trademark of Image-Line; REAPER of Cockos.", "AMD Ryzen 7 7735HS, 31 GB, Windows 11, plan de alto rendimiento, nada más abierto. FL Studio es marca de Image-Line; REAPER de Cockos.") }</li>
                        </ul>
                    </article>
                    <article>
                        <h3>{ l.t("Capacity, honestly", "Capacidad, con honestidad") }</h3>
                        <p>{ l.t("On one core Cantus sustains 120 of its compressors or 40 synth voices at 64 samples — its own offline estimate. The live DAWbench, the same session rebuilt in every host on a real interface, comes next; until it is made, no capacity comparison is claimed.",
                                 "En un núcleo Cantus sostiene 120 de sus compresores o 40 voces de sintetizador a 64 muestras — su propia estimación sin conexión. El DAWbench en vivo, la misma sesión reconstruida en cada programa con una interfaz real, viene después; hasta hacerlo no se afirma ninguna comparación de capacidad.") }</p>
                    </article>
                </div>
                { links(l) }
            </div>
        </section>
        </>
    }
}
