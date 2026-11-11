//! `/download` (round 39, Mock 4): the visitor's system first — its file straight down, with
//! the version, size, date and SHA-256 of the latest release read at build time
//! (`release.rs`) — then the truth before opening it (SmartScreen, Gatekeeper, chmod), how to
//! check the file is ours, the first ten minutes, the other systems, the requirements, and a
//! quiet "it stays free" last.

use yew::prelude::*;

use crate::icons;
use crate::lang::{use_lang, Lang};
use crate::nav::{manual, SendLink};
use crate::platform::Os;
use crate::release::{self, megabytes, RelFile, REL_DATE, REL_NOTES, REL_VERSION};
use crate::shelf::use_shelf;
use crate::top::{DOWNLOAD, DOWNLOAD_LINUX, DOWNLOAD_MACOS, GITHUB, RELEASES};

/// What the page says about one system.
struct System {
    file: Option<RelFile>,
    href: &'static str,
    cta: &'static str,
    /// The command that prints the file's SHA-256, before the file's name.
    hash_cmd: &'static str,
}

fn system(l: Lang, os: Os) -> System {
    match os {
        Os::Linux => System {
            file: release::file("linux-x86_64.AppImage"),
            href: DOWNLOAD_LINUX,
            cta: l.t("Download the AppImage", "Descargar el AppImage"),
            hash_cmd: "sha256sum",
        },
        Os::MacOs => System {
            file: release::file("macos-universal.dmg"),
            href: DOWNLOAD_MACOS,
            cta: l.t("Download the preview", "Descargar la vista previa"),
            hash_cmd: "shasum -a 256",
        },
        Os::Windows | Os::Mobile => System {
            file: release::file("windows-x64-setup.exe"),
            href: DOWNLOAD,
            cta: l.t("Download the installer", "Descargar el instalador"),
            hash_cmd: "Get-FileHash",
        },
    }
}

/// The hash and the line that checks it, with a copy button.
#[derive(Properties, PartialEq)]
struct HashProps {
    cmd: AttrValue,
    sha: AttrValue,
}

#[function_component(Hash)]
fn hash(p: &HashProps) -> Html {
    let l = use_lang().lang;
    let copied = use_state(|| false);
    let copy = {
        let (sha, copied) = (p.sha.clone(), copied.clone());
        Callback::from(move |_| {
            if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
                if let Ok(clip) = js_sys::Reflect::get(&nav, &"clipboard".into()) {
                    if let Ok(f) = js_sys::Reflect::get(&clip, &"writeText".into()) {
                        if let Ok(f) = wasm_bindgen::JsCast::dyn_into::<js_sys::Function>(f) {
                            let _ = f.call1(&clip, &sha.as_str().into());
                            copied.set(true);
                        }
                    }
                }
            }
        })
    };
    html! {
        <div class="hash">
            <code class="hash-cmd">{ p.cmd.clone() }</code>
            <div class="hash-row">
                <code class="hash-sum" aria-label="SHA-256">{ p.sha.clone() }</code>
                <button type="button" class="hash-copy" onclick={copy}>{ if *copied { l.t("Copied", "Copiado") } else { l.t("Copy", "Copiar") } }</button>
            </div>
        </div>
    }
}

#[function_component(DownloadPage)]
pub fn download_page() -> Html {
    let l = use_lang().lang;
    let os = *use_state(Os::detect);
    let shelf = use_shelf();
    // A system whose file is not out yet is shown Windows' (and told so) — never a dead link.
    let shown = match os {
        Os::Linux if shelf.linux => Os::Linux,
        Os::MacOs if shelf.macos => Os::MacOs,
        _ => Os::Windows,
    };
    let sys = system(l, shown);
    let zip = release::file("windows-x64-portable.zip");
    let title = match shown {
        Os::Linux => l.t("Cantus for Linux", "Cantus para Linux"),
        Os::MacOs => l.t(
            "Cantus for macOS · preview",
            "Cantus para macOS · vista previa",
        ),
        _ => l.t("Cantus for Windows", "Cantus para Windows"),
    };
    let lede = match shown {
        Os::Linux => l.t("The whole DAW, nothing locked. One AppImage for every distribution from Ubuntu 22.04 on: nothing to install, no administrator, and it updates itself.",
                         "El DAW completo, nada bloqueado. Un AppImage para toda distribución desde Ubuntu 22.04: nada que instalar, sin administrador, y se actualiza solo."),
        Os::MacOs => l.t("The whole DAW, nothing locked. One universal disk image for Apple silicon and Intel — a preview, built and tested on GitHub's macOS machines but not yet opened on a real Mac.",
                         "El DAW completo, nada bloqueado. Una imagen de disco universal para Apple silicon e Intel — una vista previa, compilada y probada en las máquinas macOS de GitHub pero aún no abierta en un Mac real."),
        _ => l.t("The whole DAW, nothing locked. It installs for you alone — no administrator rights needed.",
                 "El DAW completo, nada bloqueado. Se instala solo para ti — sin permisos de administrador."),
    };
    let requirement = match shown {
        Os::Linux => l.t(
            "x86-64 · Ubuntu 22.04 or newer (or alike)",
            "x86-64 · Ubuntu 22.04 o más nuevo (o similar)",
        ),
        Os::MacOs => l.t("Apple silicon or Intel", "Apple silicon o Intel"),
        _ => l.t(
            "Windows 11 · x64 (10 untested)",
            "Windows 11 · x64 (10 sin probar)",
        ),
    };
    let meta = match sys.file {
        Some(f) => format!("{REL_VERSION} · {} · {REL_DATE}", megabytes(f.size)),
        None => format!("{REL_VERSION} · {REL_DATE}"),
    };
    let file_name = sys.file.map(|f| f.name).unwrap_or("");
    let cmd = if shown == Os::Windows {
        format!("{} .\\{file_name}", sys.hash_cmd)
    } else {
        format!("{} {file_name}", sys.hash_cmd)
    };
    let before: Html = match shown {
        Os::Linux => html! {
            <li>{ icons::warn() }<div><strong>{ l.t("Make it executable, then open it", "Hazlo ejecutable y ábrelo") }</strong>
                <p>{ l.t("Once: chmod +x on the file (or Properties → Allow executing). PipeWire, JACK and ALSA are found on their own.", "Una vez: chmod +x sobre el archivo (o Propiedades → Permitir ejecutar). PipeWire, JACK y ALSA se encuentran solos.") }</p>
                <p class="keys"><kbd>{ format!("chmod +x {file_name}") }</kbd></p></div></li>
        },
        Os::MacOs => html! {
            <li>{ icons::warn() }<div><strong>{ l.t("Not signed yet — macOS will block it once", "Aún sin firmar — macOS la bloqueará una vez") }</strong>
                <p>{ l.t("The first time, open System Settings → Privacy & Security and choose Open Anyway. Please tell us what goes wrong.", "La primera vez, abre Ajustes del Sistema → Privacidad y seguridad y elige Abrir igualmente. Cuéntanos lo que falle.") }</p>
                <p class="keys"><kbd>{ l.t("Privacy & Security", "Privacidad y seguridad") }</kbd><span aria-hidden="true">{ "→" }</span><kbd>{ l.t("Open Anyway", "Abrir igualmente") }</kbd></p></div></li>
        },
        _ => html! {
            <li>{ icons::warn() }<div><strong>{ l.t("Not signed yet — Windows will ask once", "Aún sin firmar — Windows preguntará una vez") }</strong>
                <p>{ l.t("The installer has no code-signing certificate yet, so SmartScreen shows a blue window the first time. This is the way through:", "El instalador aún no tiene certificado de firma, así que SmartScreen muestra una ventana azul la primera vez. Así se pasa:") }</p>
                <p class="keys"><kbd>{ l.t("More info", "Más información") }</kbd><span aria-hidden="true">{ "→" }</span><kbd>{ l.t("Run anyway", "Ejecutar de todas formas") }</kbd></p></div></li>
        },
    };
    let others = [Os::Windows, Os::Linux, Os::MacOs].map(|o| {
        let s = system(l, o);
        let on = match o {
            Os::Linux => shelf.linux,
            Os::MacOs => shelf.macos,
            _ => true,
        };
        let (name, what) = match o {
            Os::Linux => ("Linux", l.t("AppImage · Ubuntu 22.04 and later", "AppImage · Ubuntu 22.04 y posteriores")),
            Os::MacOs => (l.t("macOS · preview", "macOS · vista previa"), l.t("Universal disk image · unsigned", "Imagen de disco universal · sin firmar")),
            _ => ("Windows", l.t("Installer or portable zip · x64", "Instalador o zip portátil · x64")),
        };
        let size = s.file.map(|f| format!(" · {}", megabytes(f.size))).unwrap_or_default();
        html! {
            <li class={classes!("sys-card", (o == shown).then_some("is-here"))}>
                <div><strong>{ name }</strong><span>{ format!("{what}{size}") }</span>
                    if o == shown { <span class="here">{ l.t("This computer", "Esta computadora") }</span> }</div>
                if on {
                    <a class={classes!("btn", if o == shown { "btn-primary" } else { "btn-quiet" })} href={s.href}>{ l.t("Download", "Descargar") }</a>
                } else {
                    <span class="platform-soon">{ l.t("Coming soon", "Muy pronto") }</span>
                }
            </li>
        }
    });
    html! {
        <>
        <section class="dl-hero page-band lapis" id="download">
            <div class="wrap dl-in">
                <div class="dl-main">
                    <span class="eyebrow">{ l.t("Download · free, for good", "Descarga · libre, para siempre") }</span>
                    <h1>{ title }</h1>
                    <p class="lede">{ lede }</p>
                    if os == Os::Mobile {
                        <p class="dl-phone">{ icons::desktop() }{ l.t("You are on a phone: Cantus is a desktop program. Send yourself the link and open it on your computer.", "Estás en un teléfono: Cantus es un programa de escritorio. Envíate el enlace y ábrelo en tu computadora.") }</p>
                        <SendLink />
                    } else {
                        <div class="dl-actions">
                            <a class="btn btn-primary" href={sys.href}>{ icons::arrow_down() }{ sys.cta }</a>
                            if shown == Os::Windows {
                                if let Some(z) = zip {
                                    <a class="btn btn-quiet" href={z.url}>{ l.t("Portable zip", "Zip portátil") }<small>{ megabytes(z.size) }</small></a>
                                }
                            }
                        </div>
                    }
                    <p class="dl-meta"><b>{ meta }</b><span>{ requirement }</span></p>
                    <p class="dl-other">{ l.t("Other systems, the plug-ins alone and every file: ", "Otros sistemas, los plug-ins solos y cada archivo: ") }
                        <a href="#systems">{ l.t("below", "abajo") }</a>{ " · " }<a href={REL_NOTES}>{ l.t("release notes", "notas de la versión") }</a>{ " · " }
                        <a href={format!("{GITHUB}/blob/master/docs/DEVELOPING.md")}>{ l.t("build it yourself", "compílalo tú") }</a></p>
                </div>
                <aside class="dl-before">
                    <h2 class="plain">{ l.t("Before you open it", "Antes de abrirlo") }</h2>
                    <ul>
                        { before }
                        <li>{ icons::shield() }<div><strong>{ l.t("Check it is ours", "Comprueba que es nuestro") }</strong>
                            <p>{ l.t("Built in the open by GitHub Actions from the tagged source. Compare its SHA-256:", "Compilado a la vista por GitHub Actions desde el código etiquetado. Compara su SHA-256:") }</p>
                            if let Some(f) = sys.file.filter(|f| !f.sha256.is_empty()) {
                                <Hash cmd={cmd.clone()} sha={f.sha256} />
                            } else {
                                <p><a href={RELEASES}>{ "SHA256SUMS.txt" }</a></p>
                            }
                        </div></li>
                        <li>{ icons::no_signal() }<div><strong>{ l.t("No account, no telemetry", "Sin cuenta, sin telemetría") }</strong>
                            <p>{ l.t("Your projects, audio and MIDI stay on your computer. It asks GitHub once a day whether there is a newer version — one switch turns that off.", "Tus proyectos, audio y MIDI se quedan en tu computadora. Pregunta a GitHub una vez al día si hay una versión nueva — un interruptor lo apaga.") }
                                { " " }<a href={format!("{GITHUB}/blob/master/PRIVACY.md")}>{ l.t("Privacy", "Privacidad") }</a></p></div></li>
                    </ul>
                </aside>
            </div>
        </section>
        <section class="section dl-next">
            <div class="wrap">
                <h2>{ l.t("Your first ten minutes", "Tus primeros diez minutos") }</h2>
                <ol class="steps">
                    <li><span class="step-n">{ "1" }</span><h3>{ l.t("Pick your audio device", "Elige tu dispositivo de audio") }</h3>
                        <p>{ l.t("ASIO or WASAPI on Windows; PipeWire, JACK or ALSA on Linux. The status bar shows the latency your driver reports, so you know what you are playing through.", "ASIO o WASAPI en Windows; PipeWire, JACK o ALSA en Linux. La barra de estado muestra la latencia que informa tu driver, así sabes con qué estás tocando.") }</p></li>
                    <li><span class="step-n">{ "2" }</span><h3>{ l.t("Open a starter or a demo", "Abre una plantilla o una demo") }</h3>
                        <p>{ l.t("Six demo songs come with it — open Neon Tide and press Space. Every sound in it is Cantus's own.", "Vienen seis canciones de demostración — abre Neon Tide y pulsa Espacio. Cada sonido es de Cantus.") }</p></li>
                    <li><span class="step-n">{ "3" }</span><h3>{ l.t("Make it yours", "Hazla tuya") }</h3>
                        <p>{ l.t("Draw a pattern in the channel rack, play it into the piano roll, mix it. ", "Dibuja un patrón en el rack de canales, tócalo en el piano roll, mézclalo. ") }
                           <a href={manual(l)}>{ l.t("The manual walks the whole path, in English and Spanish.", "El manual recorre todo el camino, en inglés y español.") }</a></p></li>
                </ol>
                <ul class="sys-row" id="systems" aria-label={l.t("Every system", "Cada sistema")}>{ for others }</ul>
                <dl class="reqs">
                    <div><dt>{ l.t("Audio", "Audio") }</dt><dd>{ l.t("Any device: ASIO, WASAPI, PipeWire, JACK, ALSA, Core Audio", "Cualquier dispositivo: ASIO, WASAPI, PipeWire, JACK, ALSA, Core Audio") }</dd></div>
                    <div><dt>{ l.t("Plug-ins", "Plug-ins") }</dt><dd>{ l.t("CLAP and VST3; ARA 2 on audio clips", "CLAP y VST3; ARA 2 en clips de audio") }</dd></div>
                    <div><dt>{ l.t("Files", "Archivos") }</dt><dd>{ l.t("WAV, FLAC, MP3, Ogg; MIDI, stems, DAWproject", "WAV, FLAC, MP3, Ogg; MIDI, stems, DAWproject") }</dd></div>
                    <div><dt>{ l.t("Source", "Código") }</dt><dd><a href={GITHUB}>{ l.t("App GPL-3.0-or-later · engine MPL-2.0", "App GPL-3.0-o-posterior · motor MPL-2.0") }</a></dd></div>
                </dl>
                <div class="stays-free">
                    <p><strong>{ l.t("It stays free.", "Sigue siendo libre.") }</strong>{ " " }
                       { l.t("If Cantus helps you finish songs, you can help it grow — the program is the same whatever you give, or nothing.", "Si Cantus te ayuda a terminar canciones, puedes ayudarle a crecer — el programa es el mismo, des lo que des, o nada.") }</p>
                    <a class="btn btn-quiet" href="#support">{ l.t("Ways to support", "Formas de apoyar") }</a>
                </div>
            </div>
        </section>
        </>
    }
}
