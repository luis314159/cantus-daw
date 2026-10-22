//! Engine, values, download, manual and footer.

use yew::prelude::*;

use crate::icons;
use crate::lang::use_lang;
use crate::platform::Os;
use crate::shelf::use_shelf;
use crate::top::{DOWNLOAD, DOWNLOAD_LINUX, DOWNLOAD_MACOS, GITHUB, RELEASES, VERSION};

#[function_component(Engine)]
pub fn engine() -> Html {
    let l = use_lang().lang;
    // A console of 22 channels whose levels sway, each at its own pace (decorative).
    let chans = (0..22u32).map(|i| {
        let lvl = 38 + (i * 37 + 11) % 55;
        let dur = 0.9 + ((i * 53) % 17) as f32 / 10.0;
        let style = format!(
            "--lvl:{lvl};--dur:{dur:.1}s;animation-delay:-{:.1}s",
            (i % 7) as f32 * 0.37
        );
        html! { <div class="chan"><b {style}></b></div> }
    });
    html! {
        <section class="night" id="engine">
            <div class="wrap night-in">
                <div>
                    <h2>{ l.t("An engine that does not blink", "Un motor que no parpadea") }</h2>
                    <p class="lede">{ l.t("The audio thread is the one place where a mistake is heard. So it is built to be boring, and tested until it is.",
                                          "El hilo de audio es el único lugar donde un error se oye. Por eso está hecho para ser aburrido, y se prueba hasta que lo es.") }</p>
                    <ul class="facts">
                        <li><strong>{ l.t("It never allocates, locks or waits", "Nunca reserva memoria, bloquea ni espera") }</strong>
                            <span>{ l.t("A test counts every allocation on the audio thread. The number must be zero.", "Una prueba cuenta cada reserva de memoria en el hilo de audio. El número debe ser cero.") }</span></li>
                        <li><strong>{ l.t("Your export equals what you heard", "Tu exportación es lo que escuchaste") }</strong>
                            <span>{ l.t("Rendering offline and playing in real time produce identical samples, bit for bit.", "Renderizar sin conexión y reproducir en tiempo real produce muestras idénticas, bit a bit.") }</span></li>
                        <li><strong>{ l.t("As many cores as you give it", "Tantos núcleos como le des") }</strong>
                            <span>{ l.t("The live engine spreads the work over a pool of real-time workers — and sounds exactly the same on one core or eight.", "El motor en vivo reparte el trabajo en un grupo de hilos de tiempo real — y suena exactamente igual con uno o con ocho núcleos.") }</span></li>
                    </ul>
                </div>
                <div class="console" aria-hidden="true">{ for chans }</div>
            </div>
        </section>
    }
}

#[function_component(Free)]
pub fn free() -> Html {
    let l = use_lang().lang;
    let promise = |title: &'static str, text: &'static str| {
        html! { <li>{ icons::check() }<span><strong>{ title }{ " " }</strong>{ text }</span></li> }
    };
    html! {
        <section class="section free-wrap" id="free">
            <div class="wrap free">
                <div>
                    <h2>{ l.t("Free, and meant to stay that way", "Libre, y hecho para seguir así") }</h2>
                    <ul class="promises">
                        { promise(l.t("The whole DAW is free.", "El DAW completo es gratis."),
                                  l.t("Create, save, mix and export without paying. Nothing essential sits behind a paywall.", "Crea, guarda, mezcla y exporta sin pagar. Nada esencial está detrás de un muro de pago.")) }
                        { promise(l.t("No account, no telemetry.", "Sin cuenta, sin telemetría."),
                                  l.t("Your projects, audio and MIDI stay on your computer.", "Tus proyectos, audio y MIDI se quedan en tu computadora.")) }
                        { promise(l.t("Open source.", "Código abierto."),
                                  l.t("App GPL-3.0-or-later, engine MPL-2.0. Read it, build it, change it — and what you change and share stays open.", "App GPL-3.0-o-posterior, motor MPL-2.0. Léelo, compílalo, cámbialo; lo que cambies y compartas sigue abierto.")) }
                        { promise(l.t("Support is optional.", "Apoyar es opcional."),
                                  l.t("Donations and extra sound packs can fund the work, but never unlock the essentials.", "Las donaciones y los paquetes de sonidos adicionales pueden financiar el trabajo, pero nunca desbloquean lo esencial.")) }
                    </ul>
                </div>
                <figure class="verse">
                    <blockquote>
                        { l.t("\u{201C}Whatever you do, work at it with all your heart.\u{201D}",
                              "\u{201C}Y todo lo que hagáis, hacedlo de ánimo, como al Señor, y no á los hombres.\u{201D}") }
                    </blockquote>
                    <figcaption>{ l.t("Colossians 3:23 — the motto Cantus is built under.", "Colosenses 3:23 (Reina-Valera 1909) — el lema bajo el que se construye Cantus.") }</figcaption>
                </figure>
            </div>
        </section>
    }
}

#[function_component(Get)]
pub fn get() -> Html {
    let l = use_lang().lang;
    let os = use_state(Os::detect);
    // Which systems the latest release has a file for, asked of GitHub (shelf.rs): Linux
    // and macOS say "Available" only when it does; "Coming soon" until then, or when the
    // check fails.
    let shelf = use_shelf();
    let guide = format!("{GITHUB}/blob/master/docs/DEVELOPING.md");
    let soon = l.t("Coming soon", "Muy pronto");
    let available = l.t("Available", "Disponible");
    let state = |on: bool| if on { available } else { soon };
    // The card speaks to the visitor's system: each gets its own file straight down (the
    // newest release's, under its stable name — ADR 0048 §7, ADR 0067), with every other
    // file a link away; a system whose file is not out yet gets the honest answer (coming
    // soon, and building from source meanwhile); phones are pointed at a computer.
    let (headline, lead, primary, primary_href) = match *os {
        Os::Windows => (
            html! { <>{ icons::windows() }{ "Windows 11" }</> },
            l.t("The newest installer, no administrator rights needed. A portable zip, the plug-ins alone and the SHA-256 sums are on the downloads page. Prefer to build it yourself? The guide in the repository takes you from a clean machine to a running Cantus.",
                "El instalador más reciente, sin permisos de administrador. Un zip portátil, los plug-ins solos y las sumas SHA-256 están en la página de descargas. ¿Prefieres compilarlo tú? La guía del repositorio te lleva de una máquina limpia a un Cantus funcionando."),
            l.t("Download the installer", "Descargar el instalador"),
            DOWNLOAD.to_string(),
        ),
        Os::Linux if shelf.linux => (
            html! { <>{ "Linux" }{ " · " }{ available }</> },
            l.t("One AppImage for every distribution from Ubuntu 22.04 on: make it executable (chmod +x) and open it — nothing to install, no administrator. It updates itself. PipeWire, JACK and ALSA; the SHA-256 sums are on the downloads page.",
                "Un AppImage para toda distribución desde Ubuntu 22.04: hazlo ejecutable (chmod +x) y ábrelo — nada que instalar, sin administrador. Se actualiza solo. PipeWire, JACK y ALSA; las sumas SHA-256 están en la página de descargas."),
            l.t("Download the AppImage", "Descargar el AppImage"),
            DOWNLOAD_LINUX.to_string(),
        ),
        Os::MacOs if shelf.macos => (
            html! { <>{ "macOS" }{ " · " }{ available }</> },
            l.t("One disk image for Apple silicon and Intel Macs: open it and drag Cantus onto Applications. When a new version is out, Cantus downloads its disk image, checks it and opens it for you. The SHA-256 sums are on the downloads page.",
                "Una imagen de disco para Mac con Apple silicon e Intel: ábrela y arrastra Cantus a Aplicaciones. Cuando hay una versión nueva, Cantus descarga su imagen de disco, la comprueba y te la abre. Las sumas SHA-256 están en la página de descargas."),
            l.t("Download the disk image", "Descargar la imagen de disco"),
            DOWNLOAD_MACOS.to_string(),
        ),
        Os::MacOs | Os::Linux => {
            let n = if *os == Os::MacOs { "macOS" } else { "Linux" };
            (
                html! { <>{ n }{ " · " }{ soon }</> },
                l.t("The installer for your system is coming soon; Windows has its own today. Meanwhile you can build Cantus from source: the guide takes you from a clean machine to a running Cantus.",
                    "El instalador para tu sistema llega muy pronto; el de Windows ya está disponible. Mientras tanto puedes compilar Cantus desde el código: la guía te lleva de una máquina limpia a un Cantus funcionando."),
                l.t("Build from source", "Compilar desde el código"),
                guide.clone(),
            )
        }
        Os::Mobile => (
            html! { <>{ l.t("Open this on your computer", "Ábrelo en tu computadora") }</> },
            if shelf.linux && shelf.macos {
                l.t("Cantus is a desktop program for Windows, macOS and Linux. Come back from your computer to download it, or read the source meanwhile.",
                    "Cantus es un programa de escritorio para Windows, macOS y Linux. Vuelve desde tu computadora para descargarlo, o lee el código mientras tanto.")
            } else {
                l.t("Cantus is a desktop program for Windows, with Linux and macOS coming soon. Come back from your computer to download it, or read the source meanwhile.",
                    "Cantus es un programa de escritorio para Windows, y muy pronto para Linux y macOS. Vuelve desde tu computadora para descargarlo, o lee el código mientras tanto.")
            },
            l.t("See the downloads", "Ver las descargas"),
            RELEASES.to_string(),
        ),
    };
    // A file straight down: the downloads page is a link away.
    let direct = match *os {
        Os::Windows => true,
        Os::Linux => shelf.linux,
        Os::MacOs => shelf.macos,
        Os::Mobile => false,
    };
    let (system, audio) = match *os {
        Os::Linux => (
            l.t(
                "Linux x86-64, Ubuntu 22.04 or newer (or alike)",
                "Linux x86-64, Ubuntu 22.04 o más nuevo (o similar)",
            ),
            l.t("PipeWire, JACK and ALSA", "PipeWire, JACK y ALSA"),
        ),
        Os::MacOs => (
            l.t(
                "macOS, Apple silicon or Intel",
                "macOS, Apple silicon o Intel",
            ),
            l.t("Any Core Audio device", "Cualquier dispositivo Core Audio"),
        ),
        _ => (
            l.t(
                "Windows 11, x64 (Windows 10 untested)",
                "Windows 11, x64 (Windows 10 sin probar)",
            ),
            l.t(
                "Any device — WASAPI and ASIO",
                "Cualquier dispositivo — WASAPI y ASIO",
            ),
        ),
    };
    html! {
        <section class="section get" id="download">
            <div class="wrap">
                <h2>{ l.t("Get the beta", "Consigue la beta") }</h2>
                <div class="get-grid">
                    <article class="get-main">
                        <div class="platform">{ headline }</div>
                        <p class="ver">{ format!("Cantus {} · 64-bit", VERSION) }</p>
                        <div class="actions">
                            <a class="btn btn-primary" href={primary_href}>{ icons::arrow_down() }{ primary }</a>
                            <a class="btn btn-quiet" href={GITHUB}>{ icons::github() }{ l.t("Source code", "Código fuente") }</a>
                        </div>
                        <p class="note">{ lead }</p>
                        if direct {
                            <p class="note"><a href={RELEASES}>{ l.t("All downloads, release notes and checksums", "Todas las descargas, notas de la versión y sumas") }</a></p>
                        }
                    </article>
                    <aside class="get-side">
                        <h3>{ l.t("What you need", "Lo que necesitas") }</h3>
                        <dl>
                            <dt>{ l.t("System", "Sistema") }</dt><dd>{ system }</dd>
                            <dt>{ l.t("Audio", "Audio") }</dt><dd>{ audio }</dd>
                            <dt>{ l.t("Plug-ins", "Plug-ins") }</dt><dd>{ "CLAP, VST3" }</dd>
                            <dt>{ l.t("Cost", "Costo") }</dt><dd>{ l.t("Free", "Gratis") }</dd>
                            <dt>{ "Windows" }</dt><dd>{ available }</dd>
                            <dt>{ "macOS" }</dt><dd>{ state(shelf.macos) }</dd>
                            <dt>{ "Linux" }</dt><dd>{ state(shelf.linux) }</dd>
                        </dl>
                    </aside>
                </div>
            </div>
        </section>
    }
}

#[function_component(More)]
pub fn more() -> Html {
    let l = use_lang().lang;
    html! {
        <section class="section more-wrap" id="more">
            <div class="wrap">
                <h2>{ l.t("Learn it, extend it, back it", "Aprende, amplía, apoya") }</h2>
                <div class="more-grid">
                    <div class="more-col">
                        <h3>{ l.t("Manual", "Manual") }</h3>
                        <p>{ l.t("The quick-start manual, in English and Spanish, from install to a finished song, and the decisions behind the design.", "El manual de inicio rápido, en inglés y español, de la instalación a una canción terminada, y las decisiones detrás del diseño.") }</p>
                        <a class="link" href={l.t("manual/en/index.html", "manual/es/index.html")}>{ l.t("Read the manual", "Leer el manual") }</a>
                        <a class="link" href={format!("{GITHUB}/tree/master/docs")}>{ l.t("Design notes", "Notas de diseño") }</a>
                        <a class="link" href="press/index.html">{ l.t("Press kit", "Kit de prensa") }</a>
                    </div>
                    <div class="more-col">
                        <h3>{ l.t("Sounds & instruments", "Sonidos e instrumentos") }<span class="soon">{ l.t("In the works", "En camino") }</span></h3>
                        <p>{ l.t("Free sample packs, presets and stand-alone plug-ins from Cantus, on cantusdaw.studio. Packs download on demand, never bundled.",
                                 "Paquetes de samples, presets y plug-ins independientes de Cantus, gratis, en cantusdaw.studio. Los paquetes se descargan cuando los pides, nunca van incluidos.") }</p>
                    </div>
                    <div class="more-col">
                        <h3>{ l.t("Back the project", "Apoya el proyecto") }</h3>
                        <p>{ l.t("Donations are voluntary and unlock nothing. The best help is still to make a song and tell us what got in your way.",
                                 "Las donaciones son voluntarias y no desbloquean nada. La mejor ayuda sigue siendo hacer una canción y contarnos qué te estorbó.") }</p>
                        <a class="link" href="#support">{ l.t("Ways to support", "Formas de apoyar") }</a>
                        <a class="link" href={format!("{GITHUB}/issues")}>{ l.t("Report a problem", "Reportar un problema") }</a>
                    </div>
                </div>
            </div>
        </section>
    }
}

#[function_component(Footer)]
pub fn footer() -> Html {
    let l = use_lang().lang;
    html! {
        <footer class="foot">
            <div class="wrap foot-in">
                <a class="brand" href="#top">{ icons::logo() }{ "Cantus" }</a>
                <nav aria-label={l.t("Footer", "Pie de página")}>
                    <a href={GITHUB}>{ "GitHub" }</a>
                    <a href={format!("{GITHUB}/blob/master/PRIVACY.md")}>{ l.t("Privacy", "Privacidad") }</a>
                    <a href={format!("{GITHUB}/blob/master/LICENSE")}>{ l.t("Licence", "Licencia") }</a>
                    <a href={format!("{GITHUB}/blob/master/CONTRIBUTING.md")}>{ l.t("Contribute", "Contribuir") }</a>
                    <a href={format!("{GITHUB}/blob/master/THIRD-PARTY-NOTICES.md")}>{ l.t("Third-party notices", "Avisos de terceros") }</a>
                </nav>
                <p>{ l.t("Created by Luis Ángel Almazán. Free software: app GPL-3.0-or-later, engine MPL-2.0. Cantus and its logos are trademarks of Luis Ángel Almazán. cantusdaw.com · cantusdaw.studio",
                         "Creado por Luis Ángel Almazán. Software libre: app GPL-3.0-o-posterior, motor MPL-2.0. Cantus y sus logotipos son marcas de Luis Ángel Almazán. cantusdaw.com · cantusdaw.studio") }</p>
                <p class="creed">{ l.t("Christ is King", "Viva Cristo Rey") }</p>
            </div>
        </footer>
    }
}

const KOFI: &str = "https://ko-fi.com/luisalmazan314159";
const LIBERAPAY: &str = "https://liberapay.com/lalmazan";
const SPONSORS: &str = "https://github.com/sponsors/luis314159";
/// Flip to `true` when the GitHub Sponsors profile is approved (and add `github:` to FUNDING.yml).
const SPONSORS_LIVE: bool = false;

#[function_component(Support)]
pub fn support() -> Html {
    let l = use_lang().lang;
    let way = |name: &'static str,
               line: &'static str,
               href: &'static str,
               cta: &'static str,
               primary: bool| {
        html! {
            <li class="way">
                <div><strong>{ name }</strong><span>{ line }</span></div>
                <a class={classes!("btn", if primary { "btn-primary" } else { "btn-quiet" })} href={href} rel="noopener">{ cta }</a>
            </li>
        }
    };
    let tiers = ["$3", "$5", "$10", "$25"];
    html! {
        <section class="section support" id="support">
            <div class="wrap support-in">
                <div>
                    <h2>{ l.t("Help keep it free and growing", "Ayuda a que siga libre y creciendo") }</h2>
                    <p class="lede">{ l.t("Cantus is free and stays free. If it helps you make music, you can help us keep building it. Everyone gets the same program, whatever they give, or nothing at all.",
                                          "Cantus es gratis y seguirá siéndolo. Si te ayuda a hacer música, puedes ayudarnos a seguir construyéndolo. Todos reciben el mismo programa, den lo que den, o nada.") }</p>
                    <ul class="tiers" aria-label={l.t("Suggested monthly amounts, in US dollars", "Montos mensuales sugeridos, en dólares")}>
                        { for tiers.iter().map(|t| html! { <li>{ *t }<small>{ l.t("/ month", "/ mes") }</small></li> }) }
                        <li class="any">{ l.t("or any amount", "o la cantidad que quieras") }</li>
                    </ul>
                    <p class="where">{ l.t("It pays for maintenance, fixes, new features and the site. We will publish how the funds are used.",
                                           "Paga el mantenimiento, las correcciones, las funciones nuevas y el sitio. Publicaremos cómo se usan los fondos.") }</p>
                </div>
                <ul class="ways">
                    { way("Ko-fi", l.t("One-time or monthly, by card or PayPal.", "Una vez o cada mes, con tarjeta o PayPal."), KOFI, l.t("Support on Ko-fi", "Apoyar en Ko-fi"), true) }
                    { way("Liberapay", l.t("Recurring and built for open source; the platform takes no cut of your gift (payment processors still charge their fees).", "Recurrente y pensado para el código abierto; la plataforma no se queda con una parte (los procesadores de pago sí cobran su comisión)."), LIBERAPAY, l.t("Give on Liberapay", "Dar en Liberapay"), false) }
                    { if SPONSORS_LIVE {
                        way("GitHub Sponsors", l.t("Monthly, from your GitHub account.", "Cada mes, desde tu cuenta de GitHub."), SPONSORS, l.t("Sponsor on GitHub", "Patrocinar en GitHub"), false)
                    } else {
                        html! { <li class="way soon-way"><div><strong>{ "GitHub Sponsors" }</strong><span>{ l.t("Coming soon.", "Muy pronto.") }</span></div></li> }
                    } }
                </ul>
            </div>
        </section>
    }
}
