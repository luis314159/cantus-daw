//! Measured, not claimed: the first scorecard (round 37, ADR 0102) — start-up against FL
//! Studio and REAPER on one machine, the rows Cantus loses as plainly as the one it wins.

use yew::prelude::*;

use crate::lang::use_lang;
use crate::top::GITHUB;

#[function_component(Measured)]
pub fn measured() -> Html {
    let l = use_lang().lang;
    // (program, window ready, quiet, private memory, at rest) — docs/scorecard/README.md.
    let rows = [
        (
            "Cantus 0.1.0-beta.12",
            l.t("1.31 s", "1,31 s"),
            "6–42 s",
            "267 MB",
            l.t("29 % of a core", "29 % de un núcleo"),
            true,
        ),
        (
            "FL Studio 24.1.2",
            l.t("2.82 s", "2,82 s"),
            l.t("3.6 s", "3,6 s"),
            "133 MB",
            l.t("6.2 %", "6,2 %"),
            false,
        ),
        (
            "REAPER 7.85",
            l.t("1.94 s", "1,94 s"),
            l.t("1.9 s", "1,9 s"),
            "62 MB",
            l.t("0.3 %", "0,3 %"),
            false,
        ),
    ];
    let body = rows
        .into_iter()
        .map(|(name, window, quiet, mem, rest, ours)| {
            html! {
                <tr class={ if ours { "ours" } else { "" } }>
                    <th scope="row">{ name }</th>
                    <td class="win">{ window }</td>
                    <td>{ quiet }</td>
                    <td>{ mem }</td>
                    <td>{ rest }</td>
                </tr>
            }
        });
    let method = format!("{GITHUB}/blob/master/docs/scorecard/README.md");
    let features = format!("{GITHUB}/blob/master/docs/scorecard/features.md");
    html! {
        <section class="section measured" id="measured">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Measured, not claimed", "Medido, no prometido") }</h2>
                    <p>{ l.t("The first scorecard: Cantus, FL Studio and REAPER started five times each on the same laptop, the same day, watched from outside by the same script. Where Cantus loses, it says so.",
                             "El primer scorecard: Cantus, FL Studio y REAPER arrancados cinco veces cada uno en la misma laptop, el mismo día, observados desde fuera por el mismo script. Donde Cantus pierde, lo dice.") }</p>
                </header>
                <div class="measured-table">
                    <table>
                        <thead><tr>
                            <th scope="col">{ l.t("Program", "Programa") }</th>
                            <th scope="col">{ l.t("Window ready", "Ventana lista") }</th>
                            <th scope="col">{ l.t("Quiet", "En reposo") }</th>
                            <th scope="col">{ l.t("Private memory", "Memoria privada") }</th>
                            <th scope="col">{ l.t("CPU at rest", "CPU en reposo") }</th>
                        </tr></thead>
                        <tbody>{ for body }</tbody>
                    </table>
                </div>
                <ul class="score-points">
                    <li><h3>{ l.t("First on screen", "Primero en pantalla") }</h3>
                        <p>{ l.t("Cantus's window is ready 2.1× sooner than FL Studio's and 1.5× sooner than REAPER's.",
                                 "La ventana de Cantus está lista 2,1 veces antes que la de FL Studio y 1,5 veces antes que la de REAPER.") }</p></li>
                    <li><h3>{ l.t("Still to win", "Lo que falta ganar") }</h3>
                        <p>{ l.t("It keeps working for seconds after its window is up, holds more memory, and at rest its demo's engine and dancer cost more CPU than the others' first screens. That is the next round's work.",
                                 "Sigue trabajando unos segundos tras abrir la ventana, ocupa más memoria, y en reposo el motor y el bailarín de su demo cuestan más CPU que la primera pantalla de los otros. Es el trabajo de la siguiente ronda.") }</p></li>
                    <li><h3>{ l.t("Capacity, honestly", "Capacidad, con honestidad") }</h3>
                        <p>{ l.t("On one core Cantus sustains 120 of its compressors or 40 synth voices at 64 samples — an estimate. The live DAWbench in every host comes next; until then no comparison is claimed.",
                                 "En un núcleo Cantus sostiene 120 de sus compresores o 40 voces de sintetizador a 64 muestras — una estimación. El DAWbench en vivo en cada programa viene después; hasta entonces no se afirma ninguna comparación.") }</p></li>
                    <li><h3>{ l.t("Repeat it", "Repítelo") }</h3>
                        <p>{ l.t("The script, the method, the raw numbers and a 97-row feature matrix are in the repository.", "El script, el método, los números crudos y una matriz de 97 funciones están en el repositorio.") }
                           { " " }<a href={ method }>{ l.t("Method and results", "Método y resultados") }</a>{ " · " }
                           <a href={ features }>{ l.t("Feature matrix", "Matriz de funciones") }</a></p></li>
                </ul>
                <p class="measured-note">{ l.t("AMD Ryzen 7 7735HS, 31 GB, Windows 11, High performance plan; each program's own first screen (Cantus its demo song, FL Studio its template, REAPER an empty project). FL Studio is a trademark of Image-Line; REAPER of Cockos. Measured 2026-11-06.",
                                               "AMD Ryzen 7 7735HS, 31 GB, Windows 11, plan de alto rendimiento; la primera pantalla de cada programa (Cantus su canción demo, FL Studio su plantilla, REAPER un proyecto vacío). FL Studio es marca de Image-Line; REAPER de Cockos. Medido el 2026-11-06.") }</p>
            </div>
        </section>
    }
}
