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
                        l.t("Ten instruments and twenty-five effects of its own, a modelled grand piano among them", "Diez instrumentos y veinticinco efectos propios, entre ellos un piano de cola modelado"),
                        l.t("Pitch editing of recordings, note by note, and a real-time auto-tuner", "Edición de afinación de grabaciones, nota por nota, y un afinador automático en tiempo real"),
                        l.t("Modulators on any parameter, and take comping", "Moduladores en cualquier parámetro, y comping de tomas"),
                        l.t("A multicore live engine; WASAPI and ASIO", "Un motor en vivo multinúcleo; WASAPI y ASIO"),
                        l.t("Hosts CLAP and VST3 plug-ins; exports WAV, FLAC, MP3 and Ogg", "Aloja plug-ins CLAP y VST3; exporta WAV, FLAC, MP3 y Ogg"),
                        l.t("Cables between mixer channels, linked clips, copy and paste", "Cables entre canales del mezclador, clips enlazados, copiar y pegar"),
                    ]) }
                    { col(l.t("Next: arriving with the beta, in testing now", "Siguiente: llega con la beta, en pruebas ahora"), vec![
                        l.t("Warping: audio that follows the tempo, with time-stretch and pitch-shift", "Warp: audio que sigue el tempo, con estiramiento de tiempo y cambio de tono"),
                        l.t("A tempo track with ramps and metre changes, and named markers", "Una pista de tempo con rampas y cambios de compás, y marcadores con nombre"),
                        l.t("MIDI learn for any parameter, with your own controller", "MIDI learn para cualquier parámetro, con tu propio controlador"),
                        l.t("Plug-ins in their own process: one that crashes never takes your song down", "Plug-ins en su propio proceso: uno que falla nunca tumba tu canción"),
                        l.t("Freeze, bounce in place and low-latency monitoring while you record", "Congelar, bounce in place y monitoreo de baja latencia al grabar"),
                        l.t("More audio interfaces tried in duplex, ten minutes live without a dropout", "Más interfaces de audio probadas en dúplex, diez minutos en vivo sin un corte"),
                        l.t("The installer, checked on clean machines", "El instalador, comprobado en máquinas limpias"),
                        l.t("Signed downloads, so you know the file is ours", "Descargas firmadas, para saber que el archivo es nuestro"),
                        l.t("Whole songs made with it, and every rough edge they find smoothed", "Canciones completas hechas con él, y cada aspereza que encuentren, pulida"),
                    ]) }
                    { col(l.t("Later", "Después"), vec![
                        l.t("Coming soon: installers for Linux and macOS, next to Windows's — one Cantus on all three systems", "Muy pronto: instaladores para Linux y macOS junto al de Windows — un mismo Cantus en los tres sistemas"),
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
                        "Coming soon. Cantus is made for Windows, Linux and macOS: Windows has its installer today, and the Linux and macOS installers are next. Meanwhile you can build it from source.",
                        "Muy pronto. Cantus está hecho para Windows, Linux y macOS: Windows tiene su instalador hoy, y los de Linux y macOS son lo siguiente. Mientras tanto puedes compilarlo desde el código.") }</p> }) }
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
