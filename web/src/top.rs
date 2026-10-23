//! The navigation bar and the hero.

use yew::prelude::*;

use crate::icons;
use crate::lang::{use_lang, Lang};
use crate::rack::Rack;
use crate::theme::Theme;

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
/// GitHub's API for the latest release (its files' names), asked from the visitor's browser.
pub const LATEST_API: &str = "https://api.github.com/repos/luis314159/cantus-daw/releases/latest";
pub const VERSION: &str = "0.1.0-beta.4";

#[function_component(Nav)]
pub fn nav() -> Html {
    let theme = use_state(Theme::detect);
    let flip = {
        let theme = theme.clone();
        Callback::from(move |_| {
            let next = theme.toggled();
            next.apply(true);
            theme.set(next);
        })
    };
    let to_day = *theme == Theme::Night;
    let ctx = use_lang();
    let l = ctx.lang;
    let scrolled = use_state(|| false);
    {
        let scrolled = scrolled.clone();
        use_effect_with((), move |_| {
            use wasm_bindgen::{closure::Closure, JsCast};
            let win = web_sys::window().expect("window");
            let w2 = win.clone();
            let cb = Closure::<dyn FnMut()>::new(move || {
                scrolled.set(w2.scroll_y().unwrap_or(0.0) > 8.0)
            });
            let _ = win.add_event_listener_with_callback("scroll", cb.as_ref().unchecked_ref());
            move || {
                let _ =
                    win.remove_event_listener_with_callback("scroll", cb.as_ref().unchecked_ref());
            }
        });
    }
    let pick = |lang: Lang| {
        let set = ctx.set.clone();
        Callback::from(move |_| set.emit(lang))
    };
    html! {
        <header class={classes!("nav", scrolled.then_some("is-scrolled"))}>
            <div class="wrap nav-in">
                <a class="brand" href="#top" aria-label="Cantus">{ icons::logo() }{ "Cantus" }<sup class="tm" aria-hidden="true">{ "™" }</sup></a>
                <nav class="nav-links" aria-label={l.t("Sections", "Secciones")}>
                    <a href="#product">{ l.t("Product", "Producto") }</a>
                    <a href="#listen">{ l.t("Listen", "Escucha") }</a>
                    <a href="#features">{ l.t("Features", "Funciones") }</a>
                    <a href="#engine">{ l.t("Engine", "Motor") }</a>
                    <a href="#free">{ l.t("Free for good", "Libre para siempre") }</a>
                    <a href="#faq">{ l.t("FAQ", "Preguntas") }</a>
                    <a href="#support">{ l.t("Support", "Apoyar") }</a>
                    <a href="#more">{ l.t("Manual & more", "Manual y más") }</a>
                </nav>
                <div class="lang" role="group" aria-label={l.t("Language", "Idioma")}>
                    <button type="button" aria-pressed={(l == Lang::En).to_string()} onclick={pick(Lang::En)}>{ "EN" }</button>
                    <button type="button" aria-pressed={(l == Lang::Es).to_string()} onclick={pick(Lang::Es)}>{ "ES" }</button>
                </div>
                <button type="button" class="theme-toggle" onclick={flip}
                        aria-label={if to_day { l.t("Switch to day mode", "Cambiar a modo día") } else { l.t("Switch to night mode", "Cambiar a modo noche") }}
                        title={if to_day { l.t("Day mode", "Modo día") } else { l.t("Night mode", "Modo noche") }}>
                    { if to_day { icons::sun() } else { icons::moon() } }
                </button>
                <a class="btn btn-primary" href="#download">{ l.t("Download", "Descargar") }</a>
            </div>
        </header>
    }
}

/// The logo's C of waves, drawn large. Rays sit on a ring open to the east, where the
/// playhead stands; while the rack plays they breathe, each a little out of step.
fn sigil() -> Html {
    const N: usize = 46;
    let rays = (0..N).map(|i| {
        let u = i as f32 / (N - 1) as f32;
        let a = (36.0 + u * 288.0).to_radians();
        // a swell in the lengths gives the logo's waveform silhouette
        let swell = (u * std::f32::consts::PI * 5.0).sin().abs() * 0.55
            + (u * std::f32::consts::PI).sin() * 0.45;
        let (r0, r1) = (92.0, 118.0 + 62.0 * swell);
        let (c, s) = (a.cos(), a.sin());
        let style = format!("--i:{i};--dx:{:.1};--dy:{:.1}", c * 9.0, s * 9.0);
        let stroke = ray_colour(u);
        html! {
            <line class="ray" {style} {stroke}
                  x1={format!("{:.1}", 190.0 + c * r0)} y1={format!("{:.1}", 200.0 + s * r0)}
                  x2={format!("{:.1}", 190.0 + c * r1)} y2={format!("{:.1}", 200.0 + s * r1)} />
        }
    });
    html! {
        <svg class="sigil" viewBox="0 0 400 400" aria-hidden="true">
            { for rays }
            <rect class="bar" x="326" y="60" width="9" height="280" rx="4.5"/>
            <rect class="bar" x="300" y="112" width="61" height="9" rx="4.5"/>
            <path class="bar" d="M313 56 L348 56 L330.5 74 Z"/>
        </svg>
    }
}

#[function_component(Hero)]
pub fn hero() -> Html {
    let l = use_lang().lang;
    html! {
        <section class="hero" id="top">
            <div class="wrap hero-in">
                <div class="hero-top">
                    <div>
                        <h1>{ l.t("Finish the song", "Termina la canción") }<span class="stop">{ "." }</span></h1>
                        <p class="hero-sub">
                            { l.t(
                                "Cantus is a free, open-source DAW written in Rust. Sequence, record, mix and export \u{2014} with no account, no subscription and no telemetry.",
                                "Cantus es un DAW libre y gratuito escrito en Rust. Secuencia, graba, mezcla y exporta \u{2014} sin cuenta, sin suscripción y sin telemetría.") }
                        </p>
                        <div class="hero-cta">
                            <a class="btn btn-primary" href="#download">{ icons::arrow_down() }{ l.t("Download the beta", "Descargar la beta") }</a>
                            <a class="btn btn-quiet" href="#rack">{ l.t("Play a groove first", "Toca un ritmo primero") }</a>
                        </div>
                        <p class="hero-meta">{ format!("{} · Windows · Linux · {}", VERSION, l.t("app GPL-3.0-or-later · engine MPL-2.0", "app GPL-3.0-o-posterior · motor MPL-2.0")) }</p>
                    </div>
                    { sigil() }
                </div>
                <Rack />
            </div>
        </section>
    }
}

/// The logo's ramp along the ring: pale gold, copper, deep red.
fn ray_colour(u: f32) -> String {
    let stops = [
        (0.0, [255.0, 217.0, 138.0]),
        (0.5, [224.0, 122.0, 46.0]),
        (1.0, [156.0, 48.0, 12.0]),
    ];
    let (a, b) = if u < 0.5 {
        (stops[0], stops[1])
    } else {
        (stops[1], stops[2])
    };
    let k = (u - a.0) / (b.0 - a.0);
    let m = |i: usize| (a.1[i] + (b.1[i] - a.1[i]) * k).round() as u8;
    format!("#{:02x}{:02x}{:02x}", m(0), m(1), m(2))
}
