//! Measured, not claimed: the scorecard (ADR 0102) — start-up against FL Studio and REAPER
//! on one quiet machine, like against like (round 38: Cantus on an empty song beside REAPER's
//! empty project, on its *Beat* starter beside FL Studio's template), the row Cantus loses as
//! plainly as the ones it wins.

use yew::prelude::*;

use crate::lang::use_lang;
use crate::top::GITHUB;

#[function_component(Measured)]
pub fn measured() -> Html {
    let l = use_lang().lang;
    // (program — what it opens, window ready, quiet, private memory, at rest, ours) —
    // docs/scorecard/README.md, 2026-11-09.
    let rows = [
        (
            l.t(
                "Cantus 0.1.0-beta.13 — an empty song",
                "Cantus 0.1.0-beta.13 — una canción vacía",
            ),
            l.t("0.74 s", "0,74 s"),
            l.t("1.5 s", "1,5 s"),
            "237 MB",
            l.t("1.6 % of a core", "1,6 % de un núcleo"),
            true,
        ),
        (
            l.t(
                "REAPER 7.85 — an empty project",
                "REAPER 7.85 — un proyecto vacío",
            ),
            l.t("1.76 s", "1,76 s"),
            l.t("2.0 s", "2,0 s"),
            "62 MB",
            l.t("2.3–3.7 %", "2,3–3,7 %"),
            false,
        ),
        (
            l.t(
                "Cantus 0.1.0-beta.13 — the Beat starter",
                "Cantus 0.1.0-beta.13 — la plantilla Beat",
            ),
            l.t("0.76 s", "0,76 s"),
            l.t("1.6 s", "1,6 s"),
            "239 MB",
            l.t("1.8–2.0 %", "1,8–2,0 %"),
            true,
        ),
        (
            l.t(
                "FL Studio 24.1.2 — its template",
                "FL Studio 24.1.2 — su plantilla",
            ),
            l.t("2.49 s", "2,49 s"),
            l.t("5.8 s", "5,8 s"),
            "133 MB",
            l.t("6.6 %", "6,6 %"),
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
                    <p>{ l.t("Cantus, FL Studio and REAPER started five times each on the same quiet laptop, the same evening, watched from outside by the same script — each beside what it is compared with: an empty song beside REAPER's empty project, a beat starter beside FL Studio's template. Where Cantus loses, it says so.",
                             "Cantus, FL Studio y REAPER arrancados cinco veces cada uno en la misma laptop sin nada más abierto, la misma noche, observados desde fuera por el mismo script — cada uno junto a lo que se compara: una canción vacía junto al proyecto vacío de REAPER, una plantilla de beat junto a la plantilla de FL Studio. Donde Cantus pierde, lo dice.") }</p>
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
                        <p>{ l.t("Cantus's window is ready 2.4× sooner than REAPER's and 3.3× sooner than FL Studio's, and it is done working sooner too.",
                                 "La ventana de Cantus está lista 2,4 veces antes que la de REAPER y 3,3 veces antes que la de FL Studio, y también termina de trabajar antes.") }</p></li>
                    <li><h3>{ l.t("Quiet when nothing plays", "En silencio cuando nada suena") }</h3>
                        <p>{ l.t("At rest Cantus uses less CPU than both: every channel whose effects hear silence stops running them, to the same bits.",
                                 "En reposo Cantus usa menos CPU que los dos: cada canal cuyos efectos oyen silencio deja de procesarlos, con los mismos bits.") }</p></li>
                    <li><h3>{ l.t("Still to win", "Lo que falta ganar") }</h3>
                        <p>{ l.t("Memory: Cantus holds more than both, because its audio engine reserves in advance what its largest session could need, so the audio thread never waits for memory. Making that reservation grow with the song is the work ahead.",
                                 "La memoria: Cantus ocupa más que los dos, porque su motor de audio reserva por adelantado lo que podría necesitar su sesión más grande, para que el hilo de audio nunca espere memoria. Que esa reserva crezca con la canción es el trabajo que sigue.") }</p></li>
                    <li><h3>{ l.t("Capacity, honestly", "Capacidad, con honestidad") }</h3>
                        <p>{ l.t("On one core Cantus sustains 120 of its compressors or 40 synth voices at 64 samples — an estimate. The live DAWbench in every host comes next; until then no comparison is claimed.",
                                 "En un núcleo Cantus sostiene 120 de sus compresores o 40 voces de sintetizador a 64 muestras — una estimación. El DAWbench en vivo en cada programa viene después; hasta entonces no se afirma ninguna comparación.") }</p></li>
                    <li><h3>{ l.t("Repeat it", "Repítelo") }</h3>
                        <p>{ l.t("The script, the method, the raw numbers and a 97-row feature matrix are in the repository.", "El script, el método, los números crudos y una matriz de 97 funciones están en el repositorio.") }
                           { " " }<a href={ method }>{ l.t("Method and results", "Método y resultados") }</a>{ " · " }
                           <a href={ features }>{ l.t("Feature matrix", "Matriz de funciones") }</a></p></li>
                </ul>
                <p class="measured-note">{ l.t("AMD Ryzen 7 7735HS, 31 GB, Windows 11, High performance plan, nothing else open; at rest is one core's share 30–40 s after start (REAPER by thread time and by cycles). FL Studio is a trademark of Image-Line; REAPER of Cockos. Measured 2026-11-09.",
                                               "AMD Ryzen 7 7735HS, 31 GB, Windows 11, plan de alto rendimiento, nada más abierto; en reposo es la parte de un núcleo de 30 a 40 s tras arrancar (REAPER por tiempo de hilo y por ciclos). FL Studio es marca de Image-Line; REAPER de Cockos. Medido el 2026-11-09.") }</p>
            </div>
        </section>
    }
}
