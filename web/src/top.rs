//! The hero (round 39, Mock 1): the program full-width on the lapis, "Hear this song", the
//! measured proof strip under it, and the playable rack in its own section.

use yew::prelude::*;

use crate::icons;
use crate::lang::use_lang;
use crate::platform::Os;
use crate::rack::Rack;

pub const GITHUB: &str = "https://github.com/luis314159/cantus-daw";
pub const RELEASES: &str = "https://github.com/luis314159/cantus-daw/releases";
/// The newest release's installer, straight down: GitHub redirects `releases/latest/download/`
/// to the latest release's file of that name, which every release also publishes under this
/// stable name (ADR 0048 §7; an xtask test keeps it equal to the packager's and the release
/// workflow's). It answers only once the repository and its releases are public.
pub const DOWNLOAD: &str =
    "https://github.com/luis314159/cantus-daw/releases/latest/download/Cantus-windows-x64-setup.exe";
/// Linux's AppImage and the Mac's disk image, the same way (ADR 0067). The download card
/// links them only when the latest release holds them (`shelf.rs`).
pub const DOWNLOAD_LINUX: &str =
    "https://github.com/luis314159/cantus-daw/releases/latest/download/Cantus-linux-x86_64.AppImage";
pub const DOWNLOAD_MACOS: &str =
    "https://github.com/luis314159/cantus-daw/releases/latest/download/Cantus-macos-universal.dmg";
/// The stable names those links end in, which the latest release's file list is searched for.
pub const STABLE_APPIMAGE: &str = "Cantus-linux-x86_64.AppImage";
pub const STABLE_DMG: &str = "Cantus-macos-universal.dmg";
/// The DAW's version, from the repository's `Cargo.toml` at build time (`build.rs`).
pub const VERSION: &str = env!("CANTUS_VERSION");

/// `0.1.0-beta.13` → `beta 13`, as the hero's line says it.
pub fn version_short() -> String {
    match VERSION.split_once("-beta.") {
        Some((_, n)) => format!("beta {n}"),
        None => VERSION.to_string(),
    }
}

#[function_component(Hero)]
pub fn hero() -> Html {
    let l = use_lang().lang;
    let os = use_state(Os::detect);
    let shelf = crate::shelf::use_shelf();
    let to = crate::nav::href(l, "/download");
    let primary = match *os {
        Os::Mobile => html! { <crate::nav::SendLink /> },
        Os::Linux if shelf.linux => html! {
            <a class="btn btn-primary" href={to}>{ icons::arrow_down() }{ l.t("Download for Linux", "Descargar para Linux") }<small>{ "AppImage" }</small></a>
        },
        Os::MacOs if shelf.macos => html! {
            <a class="btn btn-primary" href={to}>{ icons::arrow_down() }{ l.t("Download the macOS preview", "Descargar la vista previa para macOS") }</a>
        },
        Os::Linux | Os::MacOs => html! {
            <a class="btn btn-primary" href={to}>{ icons::arrow_down() }{ l.t("Get Cantus", "Consigue Cantus") }</a>
        },
        Os::Windows => html! {
            <a class="btn btn-primary" href={to}>{ icons::arrow_down() }{ l.t("Download for Windows", "Descargar para Windows") }<small>{ "64-bit" }</small></a>
        },
    };
    let (w, h) = crate::picture::SIZE;
    html! {
        <>
        <section class="hero" id="top">
            <div class="wrap hero-copy">
                <span class="eyebrow">{ format!("{} · {}", l.t("Free · open source", "Libre · código abierto"), version_short()) }</span>
                <h1>{ l.t("Finish the song", "Termina la canción") }<span class="stop">{ "." }</span></h1>
                <p class="hero-sub">{ crate::counts::hero_lede(l) }</p>
                <div class="hero-cta">
                    { primary }
                    <a class="btn btn-quiet" href="#rack">{ l.t("Play a groove in your browser", "Toca un ritmo en tu navegador") }</a>
                </div>
                if *os == Os::Mobile {
                    <p class="hero-phone">{ icons::desktop() }{ l.t("Cantus is a desktop program: Windows, Linux, macOS preview", "Cantus es un programa de escritorio: Windows, Linux, vista previa para macOS") }</p>
                } else {
                    <p class="hero-facts">
                        <span><b>{ "Windows" }</b>{ l.t(" & ", " y ") }<b>{ "Linux" }</b></span>
                        <span>{ l.t("macOS preview", "Vista previa para macOS") }</span>
                        <span>{ "CLAP · VST3 · ARA 2" }</span>
                        <span>{ "ASIO · WASAPI · PipeWire" }</span>
                    </p>
                }
            </div>
            <div class="hero-stage">
                <figure class="hero-window">
                    <img src={crate::picture::program(l)} srcset={crate::picture::program_srcset(l)} sizes={crate::picture::PROGRAM_SIZES}
                         width={w.to_string()} height={h.to_string()}
                         alt={crate::picture::program_alt(l)} fetchpriority="high" decoding="async" />
                </figure>
                <HearSong />
                <p class="hero-real">{ l.t("The real program, playing Neon Tide", "El programa real, tocando Neon Tide") }</p>
            </div>
        </section>
        <Proof />
        </>
    }
}

/// "Hear this song": Neon Tide, the house demo, made and mastered in Cantus — the excerpt
/// the listening section plays (`assets/demos/house.mp3`).
#[function_component(HearSong)]
pub fn hear_song() -> Html {
    let l = use_lang().lang;
    let playing = use_state(|| false);
    let frac = use_state(|| 0.0f64);
    let audio = use_mut_ref(|| None::<web_sys::HtmlAudioElement>);
    {
        let (audio, playing_h, frac) = (audio.clone(), playing.clone(), frac.clone());
        use_effect_with(*playing, move |on| {
            let timer = on.then(|| {
                gloo_timers::callback::Interval::new(200, move || {
                    if let Some(a) = audio.borrow().as_ref() {
                        let len = a.duration();
                        if len.is_finite() && len > 0.0 {
                            frac.set((a.current_time() / len).clamp(0.0, 1.0));
                        }
                        if a.ended() {
                            playing_h.set(false);
                        }
                    }
                })
            });
            move || drop(timer)
        });
    }
    {
        let audio = audio.clone();
        use_effect_with((), move |_| {
            move || {
                if let Some(a) = audio.borrow_mut().take() {
                    let _ = a.pause();
                }
            }
        });
    }
    let click = {
        let (audio, playing) = (audio.clone(), playing.clone());
        Callback::from(move |_| {
            let mut slot = audio.borrow_mut();
            if slot.is_none() {
                *slot = web_sys::HtmlAudioElement::new_with_src("/demos/house.mp3").ok();
            }
            if let Some(a) = slot.as_ref() {
                if *playing {
                    let _ = a.pause();
                    playing.set(false);
                } else {
                    if a.ended() {
                        a.set_current_time(0.0);
                    }
                    let _ = a.play();
                    playing.set(true);
                }
            }
        })
    };
    let pct = format!("{:.0}", *frac * 100.0);
    html! {
        <div class={classes!("hear", playing.then_some("is-playing"))}>
            <button type="button" class="hear-btn" onclick={click} aria-pressed={playing.to_string()}
                    aria-label={if *playing { l.t("Pause Neon Tide", "Pausar Neon Tide") } else { l.t("Hear Neon Tide, a song made in Cantus", "Escuchar Neon Tide, una canción hecha en Cantus") }}>
                { if *playing { icons::pause() } else { icons::play() } }
            </button>
            <span class="hear-text"><b>{ l.t("Hear this song", "Escucha esta canción") }</b>
                <small>{ l.t("Neon Tide · made and mastered in Cantus", "Neon Tide · hecha y masterizada en Cantus") }</small></span>
            <span class="hear-wave" aria-hidden="true">{ for [30, 70, 45, 100, 60, 85, 40, 65, 30].iter().enumerate().map(|(i, h)| html! { <i style={format!("--i:{i};height:{h}%")}></i> }) }</span>
            <span class="hear-bar" role="progressbar" aria-label={l.t("Neon Tide, played", "Neon Tide, reproducido")}
                  aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct.clone()}><i style={format!("width:{pct}%")}></i></span>
        </div>
    }
}

/// The measured proof under the hero: three rows won and the one lost, each with what it is
/// compared with (docs/scorecard/README.md, 2026-11-09 and 2026-11-11).
#[function_component(Proof)]
pub fn proof() -> Html {
    let l = use_lang().lang;
    let stat = |won: bool, n: &'static str, unit: &'static str, what: &'static str| {
        html! {
            <li class={classes!("stat", (!won).then_some("lost"))}>
                <span class="tag">{ if won { l.t("Won", "Ganado") } else { l.t("Lost", "Perdido") } }</span>
                <span class="n">{ n }<small>{ unit }</small></span>
                <p>{ what }</p>
            </li>
        }
    };
    html! {
        <section class="proof" aria-label={l.t("Measured against FL Studio and REAPER", "Medido frente a FL Studio y REAPER")}>
            <div class="wrap proof-in">
                <ul class="stats-row">
                    { stat(true, l.t("0.76", "0,76"), " s", l.t("to a ready window on an empty song — 2.3× sooner than REAPER", "hasta la ventana lista con una canción vacía — 2,3× antes que REAPER")) }
                    { stat(true, l.t("3.2×", "3,2×"), "", l.t("sooner than FL Studio, each on a comparable starter", "antes que FL Studio, cada uno con una plantilla comparable")) }
                    { stat(true, l.t("1.1", "1,1"), " %", l.t("of a core at rest — REAPER 2.3–3.7 %; on a starter 1.7–2.0 % against FL Studio's 6.6 %", "de un núcleo en reposo — REAPER 2,3–3,7 %; con una plantilla 1,7–2,0 % frente al 6,6 % de FL Studio")) }
                    { stat(false, "140", " MB", l.t("of memory on an empty song, down from 237 — REAPER 62 MB; on a starter 143 against FL Studio's 133", "de memoria con una canción vacía, antes 237 — REAPER 62 MB; con una plantilla 143 frente a los 133 de FL Studio")) }
                </ul>
                <p class="how">{ l.t("Same quiet laptop, same script, five starts each (Cantus beta.14 2026-11-11, the others 2026-11-09). ", "La misma laptop tranquila, el mismo script, cinco arranques cada uno (Cantus beta.14 el 2026-11-11, los otros el 2026-11-09). ") }
                    <a href={crate::nav::href(l, "/measured")}>{ l.t("See how it was measured →", "Mira cómo se midió →") }</a></p>
            </div>
        </section>
    }
}

/// The playable channel rack, in a section of its own under the proof (round 39: the hero
/// shows the program; the rack stays one click away, "Play a groove in your browser").
#[function_component(Groove)]
pub fn groove() -> Html {
    let l = use_lang().lang;
    html! {
        <section class="section groove" id="play">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Play a groove first", "Toca un ritmo primero") }</h2>
                    <p>{ l.t("A channel rack like the program's, in your browser: click the cells, change the tempo, press play.", "Un rack de canales como el del programa, en tu navegador: pulsa las celdas, cambia el tempo, dale a reproducir.") }</p>
                </header>
                <Rack />
            </div>
        </section>
    }
}
