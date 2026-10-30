//! The session view, in sync with the rest of the studio, with per-note expression (round 33).

use yew::prelude::*;

use crate::lang::use_lang;

#[function_component(Session)]
pub fn session() -> Html {
    let l = use_lang().lang;
    let points = [
        (l.t("Launched on the grid", "Lanzado a compás"),
         l.t("A clip or a whole scene starts on the next bar, or held as a gate. Follow actions, envelopes, a scene's own tempo and metre — and Arrangement record writes what you play.",
             "Un clip o una escena entera arranca en el siguiente compás, o mantenido como puerta. Acciones de seguimiento, envolventes, el tempo y el compás de cada escena — y Grabar arreglo escribe lo que tocas.")),
        (l.t("Launchpad and APC", "Launchpad y APC"),
         l.t("Plug in a Launchpad Mini MK3, X or Pro MK3, an APC mini mk2 or an APC40 mkII: the pads launch and light in the clips' colours. Or learn any pad, key or note.",
             "Conecta un Launchpad Mini MK3, X o Pro MK3, un APC mini mk2 o un APC40 mkII: los pads lanzan y se iluminan con los colores de los clips. O aprende cualquier pad, tecla o nota.")),
        (l.t("Ableton Link, MIDI clock, MTC", "Ableton Link, reloj MIDI, MTC"),
         l.t("Share tempo, beat and start with Live and every Link app on the network, or follow and send MIDI clock and time code.",
             "Comparte tempo, pulso y arranque con Live y cada app Link de la red, o sigue y envía reloj MIDI y código de tiempo.")),
        (l.t("MPE, note by note", "MPE, nota a nota"),
         l.t("Bend, pressure and timbre from your controller, recorded on each note and drawn in the piano roll; the instruments and MPE plug-ins play them.",
             "Inflexión, presión y timbre desde tu controlador, grabados en cada nota y dibujados en el piano roll; los instrumentos y los plug-ins MPE los tocan.")),
    ];
    let rows = points.into_iter().map(|(title, text)| {
        html! { <li><h3>{ title }</h3><p>{ text }</p></li> }
    });
    html! {
        <section class="section session-wrap" id="session">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Play it live: the session view", "Tócala en vivo: la vista de sesión") }</h2>
                    <p>{ l.t("Press Tab and the arrangement gives way to a grid of clips and scenes to launch, loop and record — in time with the rest of your studio.",
                             "Pulsa Tab y el arreglo deja paso a una rejilla de clips y escenas para lanzar, repetir y grabar — a tiempo con el resto de tu estudio.") }</p>
                </header>
                <div class="score-stage"><div class="backdrop">
                    <div class="window">
                        <div class="window-bar"><i></i><i></i><i></i><span>{ l.t("Cantus — session view", "Cantus — vista de sesión") }</span></div>
                        <img src="session/grid.webp" width="1400" height="720" loading="lazy"
                             alt={ l.t("The session view: an 8 × 8 set playing, a clip's panel open", "La vista de sesión: un set de 8 × 8 sonando, el panel de un clip abierto") } />
                    </div>
                </div></div>
                <div class="session-bar">
                    <img src="session/sync-bar.webp" width="874" height="54" loading="lazy"
                         alt={ l.t("The top bar with Link on: two peers, quantum 4, the tempo it follows", "La barra superior con Link encendido: dos pares, cuanto 4, el tempo que sigue") } />
                </div>
                <div class="score-more">
                    <figure>
                        <img src="session/mpe-lanes.webp" width="1200" height="720" loading="lazy"
                             alt={ l.t("The piano roll's per-note lanes: bend, pressure and timbre", "Los carriles por nota del piano roll: inflexión, presión y timbre") } />
                        <figcaption>{ l.t("Each note keeps its own bend, pressure and timbre.", "Cada nota guarda su inflexión, su presión y su timbre.") }</figcaption>
                    </figure>
                    <figure>
                        <img src="session/midi-ports.webp" width="778" height="760" loading="lazy"
                             alt={ l.t("Settings ▸ MIDI: each port's notes, controls, clock, time code, MPE and grid", "Ajustes ▸ MIDI: notas, controles, reloj, código de tiempo, MPE y rejilla de cada puerto") } />
                        <figcaption>{ l.t("Every port in one row: notes, controls, clock, time code, MPE, grid.", "Cada puerto en una fila: notas, controles, reloj, código de tiempo, MPE, rejilla.") }</figcaption>
                    </figure>
                </div>
                <ul class="score-points">{ for rows }</ul>
            </div>
        </section>
    }
}
