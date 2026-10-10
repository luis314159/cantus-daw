//! Where Cantus is going, and the questions people ask first.

use yew::prelude::*;

use crate::lang::use_lang;
use crate::top::GITHUB;

#[function_component(Ahead)]
pub fn ahead() -> Html {
    let l = use_lang().lang;

    let col = |title: &'static str, items: Vec<&'static str>| {
        html! {
            <div class="road-col">
                <h3>{ title }</h3>
                <ul>{ for items.into_iter().map(|t| html! { <li>{ t }</li> }) }</ul>
            </div>
        }
    };

    let faq = |q: &'static str, a: Html| {
        html! { <details class="qa"><summary>{ q }</summary><div class="qa-a">{ a }</div></details> }
    };

    html! {
        <section class="section ahead" id="ahead">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Where it is and where it goes", "Dónde está y a dónde va") }</h2>
                    <p>{ l.t("Cantus is a beta, built in the open. This is the honest picture, taken from the project's own status notes.",
                             "Cantus es una beta, hecha a la vista de todos. Este es el panorama honesto, tomado de las notas de estado del propio proyecto.") }</p>
                </header>

                <div class="road">
                    { col(l.t("Works today", "Funciona hoy"), vec![
                        l.t("Record, sequence, mix and export a whole song", "Grabar, secuenciar, mezclar y exportar una canción completa"),
                        l.t("Eight instruments and ten effects of its own", "Ocho instrumentos y diez efectos propios"),
                        l.t("A multicore live engine; WASAPI and ASIO", "Un motor en vivo multinúcleo; WASAPI y ASIO"),
                        l.t("Hosts CLAP and VST3 plug-ins", "Aloja plug-ins CLAP y VST3"),
                        l.t("Export to WAV, FLAC, MP3 and Ogg", "Exportar a WAV, FLAC, MP3 y Ogg"),
                    ]) }
                    { col(l.t("Next: making the beta solid", "Siguiente: dejar sólida la beta"), vec![
                        l.t("More audio interfaces tried in duplex, and a two-hour soak", "Más interfaces de audio probadas en dúplex, y una prueba de dos horas"),
                        l.t("Copy and paste in the piano roll and the arrangement", "Copiar y pegar en el piano roll y el arreglo"),
                        l.t("Clips that stay linked until you make them unique", "Clips que siguen enlazados hasta que los haces únicos"),
                        l.t("The installer, checked on clean machines", "El instalador, comprobado en máquinas limpias"),
                        l.t("A new default look: white with blue, and a dark twin", "Un nuevo aspecto por defecto: blanco con azul, y su gemelo oscuro"),
                    ]) }
                    { col(l.t("Later", "Después"), vec![
                        l.t("A mixer with cables between channels", "Un mezclador con cables entre canales"),
                        l.t("More speed from splitting a single instrument's voices across cores", "Más velocidad al repartir las voces de un solo instrumento entre núcleos"),
                        l.t("Pitch and time editing of recordings, and a real-time auto-tuner", "Edición de afinación y tiempo de grabaciones, y un afinador automático en tiempo real"),
                        l.t("Sound packs and stand-alone plug-ins on cantusdaw.studio", "Paquetes de sonidos y plug-ins independientes en cantusdaw.studio"),
                        l.t("Assistants that can operate the DAW, with your permission", "Asistentes que puedan operar el DAW, con tu permiso"),
                    ]) }
                </div>

                <div class="faq" id="faq">
                    <h3>{ l.t("Questions people ask first", "Las preguntas que más se hacen") }</h3>
                    { faq(l.t("Is it really free?", "¿De verdad es gratis?"), html! { <p>{ l.t(
                        "Yes. The whole DAW is free and nothing essential sits behind a paywall. Optional sound packs and donations may fund the work later; they never unlock the essentials.",
                        "Sí. El DAW completo es gratis y nada esencial está detrás de un muro de pago. Los paquetes de sonidos opcionales y las donaciones podrán financiar el trabajo más adelante; nunca desbloquean lo esencial.") }</p> }) }
                    { faq(l.t("Why does it not ask me to sign in?", "¿Por qué no me pide iniciar sesión?"), html! { <p>{ l.t(
                        "There is nothing to sign in to. Your projects, audio and MIDI stay on your computer and Cantus has no telemetry. The privacy notice in the repository says exactly what, if anything, leaves your machine.",
                        "No hay nada a lo que iniciar sesión. Tus proyectos, audio y MIDI se quedan en tu computadora y Cantus no tiene telemetría. El aviso de privacidad del repositorio dice exactamente qué sale de tu máquina, si algo sale.") }</p> }) }
                    { faq(l.t("Does it work with my plug-ins?", "¿Funciona con mis plug-ins?"), html! { <p>{ l.t(
                        "It hosts CLAP and VST3 plug-ins, and ships its own effects as CLAP and VST3 so you can use them in other DAWs.",
                        "Aloja plug-ins CLAP y VST3, y entrega sus propios efectos como CLAP y VST3 para que los uses en otros DAW.") }</p> }) }
                    { faq(l.t("Which audio interfaces work?", "¿Qué interfaces de audio funcionan?"), html! { <p>{ l.t(
                        "Any Windows audio device through WASAPI, and ASIO drivers (ASIO4ALL and the makers' own). It is a beta and has been tried on a small number of devices, so tell us what you used and how it went.",
                        "Cualquier dispositivo de audio de Windows mediante WASAPI, y drivers ASIO (ASIO4ALL y los de cada fabricante). Es una beta y se ha probado en pocos dispositivos, así que cuéntanos qué usaste y cómo te fue.") }</p> }) }
                    { faq(l.t("Is there a Mac or Linux version?", "¿Hay versión para Mac o Linux?"), html! { <p>{ l.t(
                        "Not yet as a download. The code builds for development on both; Windows is what ships today.",
                        "Todavía no como descarga. El código compila para desarrollo en ambos; Windows es lo que se distribuye hoy.") }</p> }) }
                    { faq(l.t("Can I release the music I make?", "¿Puedo publicar la música que hago?"), html! { <p>{ l.t(
                        "Yes. The GPL covers the program, not what you create with it; your songs are yours. Each sound pack states its own licence, so check it for the samples you use.",
                        "Sí. La GPL cubre el programa, no lo que creas con él; tus canciones son tuyas. Cada paquete de sonidos indica su propia licencia, así que revísala para los samples que uses.") }</p> }) }
                    { faq(l.t("Where do I report a problem?", "¿Dónde reporto un problema?"), html! { <p><a href={format!("{GITHUB}/issues")}>{ l.t("On GitHub, in the issue tracker", "En GitHub, en el seguimiento de problemas") }</a>{ l.t(". Say what you did, what you expected and what happened.", ". Cuenta qué hiciste, qué esperabas y qué pasó.") }</p> }) }
                    { faq(l.t("Why \u{201C}Cantus\u{201D}?", "¿Por qué «Cantus»?"), html! { <p>{ l.t(
                        "From the cantus firmus: the fixed melody on which polyphony is built. The engine is that fixed line — deterministic and sample-accurate — and everything else is built on top of it.",
                        "Del cantus firmus: la melodía fija sobre la que se construye la polifonía. El motor es esa línea fija — determinista y exacto a la muestra — y todo lo demás se construye encima.") }</p> }) }
                </div>
            </div>
        </section>
    }
}
