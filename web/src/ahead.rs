//! Where Cantus is going (the /roadmap page) and the questions people ask first (the home's
//! FAQ chapter): one section until round 39 split the home into chapters and pages.

use yew::prelude::*;

use crate::lang::use_lang;
use crate::shelf::use_shelf;
use crate::top::GITHUB;

#[function_component(Roadmap)]
pub fn roadmap() -> Html {
    let l = use_lang().lang;
    // Once the latest release carries the macOS file (shelf.rs), the roadmap says so.
    let mac = use_shelf().macos;
    let three = if mac {
        l.t(
            "macOS out of preview: signed, notarised and tried on real Macs",
            "macOS fuera de vista previa: firmado, notarizado y probado en Mac reales",
        )
    } else {
        l.t(
            "macOS, first as a preview, then signed and tried on real Macs",
            "macOS, primero como vista previa, luego firmado y probado en Mac reales",
        )
    };
    let col = |title: &'static str, items: Vec<&'static str>| {
        html! {
            <div class="road-col">
                <h3>{ title }</h3>
                <ul>{ for items.into_iter().map(|t| html! { <li>{ t }</li> }) }</ul>
            </div>
        }
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
                        l.t("Record, sequence, mix and export a whole song, on Windows and Linux", "Grabar, secuenciar, mezclar y exportar una canción completa, en Windows y Linux"),
                        crate::counts::ahead_units(l),
                        l.t("The recording mode: the song baked, only your take live", "El modo de grabación: la canción pre-renderizada, solo tu toma en vivo"),
                        l.t("VCA faders, mixer snapshots and scene cues", "Faders VCA, snapshots del mezclador y cues de escena"),
                        l.t("An audio editor with spectral repair and noise reduction", "Un editor de audio con reparación espectral y reducción de ruido"),
                        l.t("Warp, a tempo track, freeze, take comping and pitch editing", "Warp, pista de tempo, congelar, comping de tomas y edición de afinación"),
                        l.t("A session view, Ableton Link, MIDI clock and MPE", "Una vista de sesión, Ableton Link, reloj MIDI y MPE"),
                        l.t("Hosts CLAP, VST3 and ARA 2 plug-ins; CLAP and VST3 out of process", "Aloja plug-ins CLAP, VST3 y ARA 2; CLAP y VST3 fuera de proceso"),
                        l.t("AI assistants that operate the DAW through MCP, with your permission", "Asistentes de IA que operan el DAW por MCP, con tu permiso"),
                    ]) }
                    { col(l.t("Next: the beta's last mile", "Siguiente: el último tramo de la beta"), vec![
                        l.t("The macOS preview tried on real Macs", "La vista previa de macOS probada en Mac reales"),
                        l.t("Mackie Control and HUI tried on real surfaces; outboard gear's ping on real synths", "Mackie Control y HUI probados en superficies reales; el ping del equipo externo en sintes reales"),
                        l.t("More audio interfaces tried in duplex, at their lowest buffers", "Más interfaces de audio probadas en dúplex, en sus búferes más bajos"),
                        l.t("Signed downloads, so you know the file is ours", "Descargas firmadas, para saber que el archivo es nuestro"),
                        l.t("Whole songs made with it, and every rough edge they find smoothed", "Canciones completas hechas con él, y cada aspereza que encuentren, pulida"),
                    ]) }
                    { col(l.t("Later", "Después"), vec![
                        three,
                        l.t("ARA plug-ins in their own process", "Plug-ins ARA en su propio proceso"),
                        l.t("Sound packs and stand-alone plug-ins on cantusdaw.studio", "Paquetes de sonidos y plug-ins independientes en cantusdaw.studio"),
                        l.t("An assistant panel inside Cantus, with your own key or a local model", "Un panel de asistente dentro de Cantus, con tu propia clave o un modelo local"),
                    ]) }
                </div>

            </div>
        </section>
    }
}

#[function_component(Faq)]
pub fn faq() -> Html {
    let l = use_lang().lang;
    // Once the latest release carries the Linux and macOS files (shelf.rs), the answer about
    // other systems says so instead of "coming soon".
    let shelf = use_shelf();
    let mac = shelf.macos;
    let other_systems = if mac {
        l.t("Yes. Cantus runs on Windows and Linux — an installer for Windows and one AppImage for every Linux distribution, the whole test suite run on both every round — and macOS has a preview: one universal disk image for Apple silicon and Intel, built and tested on GitHub's macOS machines, not yet signed and not yet opened on a real Mac. The first time, macOS blocks it: System Settings → Privacy & Security → Open Anyway. Please report what goes wrong.",
            "Sí. Cantus funciona en Windows y Linux — un instalador para Windows y un AppImage para toda distribución Linux, con todas las pruebas pasadas en ambos en cada ronda — y macOS tiene una vista previa: una imagen de disco universal para Apple silicon e Intel, compilada y probada en las máquinas macOS de GitHub, aún sin firmar y aún no abierta en un Mac real. La primera vez macOS la bloquea: Ajustes del Sistema → Privacidad y seguridad → Abrir igualmente. Cuéntanos lo que falle.")
    } else if shelf.linux {
        l.t("Yes for Linux: one AppImage for every distribution from Ubuntu 22.04 on, with the whole test suite run on Linux every round. macOS is next, first as a preview.",
            "Sí para Linux: un AppImage para toda distribución desde Ubuntu 22.04, con todas las pruebas pasadas en Linux en cada ronda. macOS es lo siguiente, primero como vista previa.")
    } else {
        l.t("Cantus is made for Windows, Linux and macOS: Windows has its installer today, and the Linux and macOS files are next. Meanwhile you can build it from source.",
            "Cantus está hecho para Windows, Linux y macOS: Windows tiene su instalador hoy, y los archivos de Linux y macOS son lo siguiente. Mientras tanto puedes compilarlo desde el código.")
    };

    // The first two answers stand open: they are what most visitors came to ask.
    let asked = std::cell::Cell::new(0);
    let faq = |q: &'static str, a: Html| {
        let open = asked.get() < 2;
        asked.set(asked.get() + 1);
        html! { <details class="qa" {open}><summary>{ q }</summary><div class="qa-a">{ a }</div></details> }
    };

    html! {
        <section class="section ahead faq-wrap" id="faq">
            <div class="wrap">
                <div class="faq">
                    <h2>{ l.t("Questions people ask first", "Las preguntas que más se hacen") }</h2>
                    { faq(l.t("Is it really free?", "¿De verdad es gratis?"), html! { <p>{ l.t(
                        "Yes. The whole DAW is free and nothing essential sits behind a paywall. Optional sound packs and donations may fund the work later; they never unlock the essentials.",
                        "Sí. El DAW completo es gratis y nada esencial está detrás de un muro de pago. Los paquetes de sonidos opcionales y las donaciones podrán financiar el trabajo más adelante; nunca desbloquean lo esencial.") }</p> }) }
                    { faq(l.t("Why does it not ask me to sign in?", "¿Por qué no me pide iniciar sesión?"), html! { <p>{ l.t(
                        "There is nothing to sign in to. Your projects, audio and MIDI stay on your computer and Cantus has no telemetry. The privacy notice in the repository says exactly what, if anything, leaves your machine.",
                        "No hay nada a lo que iniciar sesión. Tus proyectos, audio y MIDI se quedan en tu computadora y Cantus no tiene telemetría. El aviso de privacidad del repositorio dice exactamente qué sale de tu máquina, si algo sale.") }</p> }) }
                    { faq(l.t("Does it work with my plug-ins?", "¿Funciona con mis plug-ins?"), html! { <p>{ l.t(
                        "It hosts CLAP and VST3 plug-ins, and ARA 2 plug-ins on audio clips; it ships its own effects as CLAP and VST3 so you can use them in other DAWs. ARA hosting is new: tell us which ARA plug-ins you tried and how they behaved.",
                        "Aloja plug-ins CLAP y VST3, y plug-ins ARA 2 en clips de audio; entrega sus propios efectos como CLAP y VST3 para que los uses en otros DAW. El alojamiento ARA es nuevo: cuéntanos qué plug-ins ARA probaste y cómo se comportaron.") }</p> }) }
                    { faq(l.t("Which audio interfaces work?", "¿Qué interfaces de audio funcionan?"), html! { <p>{ l.t(
                        "On Windows, any device through WASAPI and ASIO drivers (ASIO4ALL and the makers' own); on Linux, PipeWire, JACK and ALSA; on the macOS preview, Core Audio. It is a beta and has been tried on a small number of devices, so tell us what you used and how it went.",
                        "En Windows, cualquier dispositivo por WASAPI y drivers ASIO (ASIO4ALL y los de cada fabricante); en Linux, PipeWire, JACK y ALSA; en la vista previa de macOS, Core Audio. Es una beta y se ha probado en pocos dispositivos, así que cuéntanos qué usaste y cómo te fue.") }</p> }) }
                    { faq(l.t("Is there a Mac or Linux version?", "¿Hay versión para Mac o Linux?"), html! { <p>{ other_systems }</p> }) }
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
