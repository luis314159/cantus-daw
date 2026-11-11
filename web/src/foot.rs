//! The footer (round 39, P2-5): four columns — Product, Learn, Community, Legal — then the
//! language and theme switches, the maker, the marks and the creed. Every link is a plain
//! address that works before the app runs (`router.rs` takes the site's own in place).

use yew::prelude::*;

use crate::icons;
use crate::lang::{use_lang, Lang};
use crate::route::{Page, Route};
use crate::router::use_page;
use crate::theme::{use_theme, Theme};
use crate::top::{GITHUB, RELEASES};

#[function_component(Footer)]
pub fn footer() -> Html {
    let ctx = use_lang();
    let l = ctx.lang;
    let page = use_page();
    let theme = use_theme();
    let to = |r: Route| Page::new(l, r).path();
    let gh = |p: &str| format!("{GITHUB}/{p}");
    let col = |title: &'static str, links: Vec<(String, &'static str)>| {
        html! {
            <div class="foot-col">
                <h2>{ title }</h2>
                <ul>{ for links.into_iter().map(|(href, t)| html! { <li><a {href}>{ t }</a></li> }) }</ul>
            </div>
        }
    };
    // The same page in each language: a link (it works without the app, in a new tab and for
    // crawlers); a click also remembers the choice, so `/` opens in it next time.
    let lang_link = |to_lang: Lang, label: &'static str, name: &'static str| {
        let here = to_lang == l;
        let onclick = Callback::from(move |_: MouseEvent| to_lang.remember());
        html! {
            <a href={page.in_lang(to_lang).path()} hreflang={to_lang.code()} lang={to_lang.code()}
               aria-current={here.then_some("page")} title={name} {onclick}>{ label }</a>
        }
    };
    let day = theme.theme == Theme::Day;
    html! {
        <footer class="foot">
            <div class="wrap">
                <div class="foot-cols">
                    <div class="foot-brand">
                        <a class="brand" href={to(Route::Home)} aria-label="Cantus™">{ icons::logo() }{ "Cantus" }<sup class="tm" aria-hidden="true">{ "™" }</sup></a>
                        <p>{ l.t("A free, open-source DAW written in Rust. No account, no subscription, no telemetry.",
                                 "Un DAW libre y de código abierto escrito en Rust. Sin cuenta, sin suscripción, sin telemetría.") }</p>
                    </div>
                    { col(l.t("Product", "Producto"), vec![
                        (to(Route::Download), l.t("Download", "Descargar")),
                        (to(Route::Instruments), l.t("Instruments and effects", "Instrumentos y efectos")),
                        (to(Route::Studio), l.t("The studio", "El estudio")),
                        (to(Route::Ai), l.t("AI on your computer", "IA en tu computadora")),
                        (to(Route::Measured), l.t("Measured", "Medido")),
                        (to(Route::Roadmap), l.t("Roadmap", "Hoja de ruta")),
                    ]) }
                    { col(l.t("Learn", "Aprende"), vec![
                        (to(Route::Learn), l.t("Learn Cantus", "Aprende Cantus")),
                        (l.t("/manual/en/index.html", "/manual/es/index.html").to_string(), l.t("Manual", "Manual")),
                        (Page::home_anchor(l, "faq"), l.t("Questions", "Preguntas")),
                        (gh("tree/master/docs"), l.t("Design notes", "Notas de diseño")),
                        ("/press/index.html".to_string(), l.t("Press kit", "Kit de prensa")),
                        (RELEASES.to_string(), l.t("Release notes", "Notas de versión")),
                    ]) }
                    { col(l.t("Community", "Comunidad"), vec![
                        (GITHUB.to_string(), "GitHub"),
                        (gh("issues"), l.t("Report a problem", "Reportar un problema")),
                        (gh("blob/master/CONTRIBUTING.md"), l.t("Contribute", "Contribuir")),
                        (format!("{}#support", to(Route::Download)), l.t("Support the project", "Apoya el proyecto")),
                        ("/releases.xml".to_string(), l.t("Releases feed", "Fuente de versiones")),
                    ]) }
                    { col(l.t("Legal", "Legal"), vec![
                        (gh("blob/master/PRIVACY.md"), l.t("Privacy", "Privacidad")),
                        (gh("blob/master/LICENSE"), l.t("Licence", "Licencia")),
                        (gh("blob/master/TRADEMARKS.md"), l.t("Trademarks", "Marcas")),
                        (gh("blob/master/THIRD-PARTY-NOTICES.md"), l.t("Third-party notices", "Avisos de terceros")),
                    ]) }
                </div>
                <div class="foot-bar">
                    <nav class="foot-lang" aria-label={l.t("Language", "Idioma")}>
                        { lang_link(Lang::En, "English", "English") }
                        { lang_link(Lang::Es, "Español", "Español") }
                    </nav>
                    <button type="button" class="foot-theme" onclick={theme.flip()} aria-pressed={day.to_string()}>
                        { if day { icons::moon() } else { icons::sun() } }
                        <span>{ if day { l.t("Night mode", "Modo noche") } else { l.t("Day mode", "Modo día") } }</span>
                    </button>
                    <p class="foot-legal">{ l.t("Created by Luis Ángel Almazán. Free software: app GPL-3.0-or-later, engine MPL-2.0. Cantus and its logos are trademarks of Luis Ángel Almazán. cantusdaw.com · cantusdaw.studio",
                             "Creado por Luis Ángel Almazán. Software libre: app GPL-3.0-o-posterior, motor MPL-2.0. Cantus y sus logotipos son marcas de Luis Ángel Almazán. cantusdaw.com · cantusdaw.studio") }</p>
                    <p class="creed">{ l.t("Christ is King", "Viva Cristo Rey") }</p>
                </div>
            </div>
        </footer>
    }
}
