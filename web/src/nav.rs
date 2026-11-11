//! The navigation bar (round 39, Mock 5): the chapters, the language and the theme, and
//! Download; below 52 rem a burger opens a sheet with the chapters, the switches and "Send
//! the link to my computer" (a phone cannot run Cantus). Also the [`SendLink`] button the
//! hero and the download page share, and [`href`], the one place a path gets its language.

use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use yew::prelude::*;

use crate::icons;
use crate::lang::{use_lang, Lang};
use crate::theme::{use_theme, Theme};

/// The site's address, for links sent elsewhere.
pub const SITE: &str = "https://cantusdaw.com";

/// A path of the site in this language: the Spanish pages live under `/es/` (web-struct's
/// routes). Anchors on the home keep their `#`. A page's path gets its trailing slash
/// (`/download` → `/download/`): each page is a folder's `index.html`, and the host would
/// otherwise answer with a redirect first.
pub fn href(l: Lang, path: &str) -> String {
    let slashed;
    let path = if path.ends_with('/') || path.contains(['#', '.', '?']) {
        path
    } else {
        slashed = format!("{path}/");
        &slashed
    };
    match l {
        Lang::En => path.to_string(),
        Lang::Es if path == "/" => "/es/".to_string(),
        Lang::Es if path.starts_with("/#") => format!("/es/{}", &path[1..]),
        Lang::Es => format!("/es{path}"),
    }
}

/// The manual in this language.
pub fn manual(l: Lang) -> &'static str {
    l.t("/manual/en/index.html", "/manual/es/index.html")
}

/// One chapter in the bar and the sheet: (label, what it holds, path).
fn chapters(l: Lang) -> [(&'static str, &'static str, String); 6] {
    [
        (
            l.t("Tour", "Recorrido"),
            l.t(
                "Record, mix, perform, write",
                "Graba, mezcla, toca, escribe",
            ),
            href(l, "/#product"),
        ),
        (
            l.t("Instruments", "Instrumentos"),
            crate::counts::nav_instruments(l),
            href(l, "/#units"),
        ),
        (
            l.t("Effects", "Efectos"),
            crate::counts::nav_effects(l),
            href(l, "/#effects"),
        ),
        (
            l.t("Listen", "Escucha"),
            l.t(
                "Six demo songs made in Cantus",
                "Seis canciones de demostración hechas en Cantus",
            ),
            href(l, "/#listen"),
        ),
        (
            l.t("Measured", "Medido"),
            l.t(
                "Against FL Studio and REAPER",
                "Frente a FL Studio y REAPER",
            ),
            href(l, "/measured"),
        ),
        (
            l.t("Manual", "Manual"),
            l.t("English and Spanish", "Inglés y español"),
            manual(l).to_string(),
        ),
    ]
}

fn body_class(name: &str, on: bool) {
    if let Some(b) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    {
        let _ = if on {
            b.class_list().add_1(name)
        } else {
            b.class_list().remove_1(name)
        };
    }
}

fn focus(id: &str) {
    if let Some(e) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = e.focus();
    }
}

#[function_component(Nav)]
pub fn nav() -> Html {
    // One theme for the bar, the sheet and the footer (`theme::ThemeCtx`, from `app.rs`).
    let theme_ctx = use_theme();
    let theme = theme_ctx.theme;
    let ctx = use_lang();
    let l = ctx.lang;
    let open = use_state(|| false);
    let scrolled = use_state(|| false);
    {
        let scrolled = scrolled.clone();
        use_effect_with((), move |_| {
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
    // While the sheet is open: the page behind holds still, Esc closes it and gives the focus
    // back to the burger.
    {
        let open_h = open.clone();
        use_effect_with(*open, move |on| {
            let mut listener = None;
            body_class("menu-open", *on);
            if *on {
                focus("menu-first");
                let open_h = open_h.clone();
                let cb = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
                    move |e: web_sys::KeyboardEvent| {
                        if e.key() == "Escape" {
                            open_h.set(false);
                            focus("menu-burger");
                        }
                    },
                );
                if let Some(d) = web_sys::window().and_then(|w| w.document()) {
                    let _ =
                        d.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
                }
                listener = Some(cb);
            }
            move || {
                if let Some(cb) = listener {
                    if let Some(d) = web_sys::window().and_then(|w| w.document()) {
                        let _ = d.remove_event_listener_with_callback(
                            "keydown",
                            cb.as_ref().unchecked_ref(),
                        );
                    }
                }
                body_class("menu-open", false);
            }
        });
    }
    let set_theme = |t: Theme| {
        let set = theme_ctx.set.clone();
        Callback::from(move |_| set.emit(t))
    };
    let flip = theme_ctx.flip();
    let to_day = theme == Theme::Night;
    let pick = |lang: Lang| {
        let set = ctx.set.clone();
        Callback::from(move |_| set.emit(lang))
    };
    let toggle = {
        let open = open.clone();
        Callback::from(move |_| open.set(!*open))
    };
    let close = {
        let open = open.clone();
        Callback::from(move |_| open.set(false))
    };
    let links = chapters(l)
        .into_iter()
        .map(|(name, _, to)| html! { <a href={to}>{ name }</a> });
    let sheet = chapters(l).into_iter().enumerate().map(|(i, (name, what, to))| {
        html! {
            <li><a href={to} onclick={close.clone()} id={(i == 0).then_some("menu-first")}>
                <span><strong>{ name }</strong><small>{ what }</small></span>{ icons::chevron() }
            </a></li>
        }
    });
    html! {
        <header class={classes!("nav", (*scrolled || *open).then_some("is-scrolled"), open.then_some("is-open"))}>
            <div class="wrap nav-in">
                <a class="brand" href={href(l, "/")}>{ icons::logo() }{ "Cantus" }<sup class="tm">{ "™" }</sup></a>
                <nav class="nav-links" aria-label={l.t("Chapters", "Capítulos")}>{ for links }</nav>
                <div class="lang" role="group" aria-label={l.t("Language", "Idioma")}>
                    <button type="button" aria-pressed={(l == Lang::En).to_string()} onclick={pick(Lang::En)} lang="en">{ "EN" }</button>
                    <button type="button" aria-pressed={(l == Lang::Es).to_string()} onclick={pick(Lang::Es)} lang="es">{ "ES" }</button>
                </div>
                <button type="button" class="theme-toggle" onclick={flip}
                        aria-label={if to_day { l.t("Switch to day mode", "Cambiar a modo día") } else { l.t("Switch to night mode", "Cambiar a modo noche") }}
                        title={if to_day { l.t("Day mode", "Modo día") } else { l.t("Night mode", "Modo noche") }}>
                    { if to_day { icons::sun() } else { icons::moon() } }
                </button>
                <a class="btn btn-primary nav-get" href={href(l, "/download")}>{ l.t("Download", "Descargar") }</a>
                <button type="button" class="burger" id="menu-burger" onclick={toggle}
                        aria-expanded={open.to_string()} aria-controls="menu-sheet"
                        aria-label={if *open { l.t("Close the menu", "Cerrar el menú") } else { l.t("Open the menu", "Abrir el menú") }}>
                    { if *open { icons::close() } else { icons::menu() } }
                </button>
            </div>
            if *open {
                <div class="sheet" id="menu-sheet">
                    <nav aria-label={l.t("Chapters", "Capítulos")}><ul class="sheet-list">{ for sheet }</ul></nav>
                    <div class="sheet-switches">
                        <div class="seg" role="group" aria-label={l.t("Language", "Idioma")}>
                            <button type="button" aria-pressed={(l == Lang::En).to_string()} onclick={pick(Lang::En)} lang="en">{ "English" }</button>
                            <button type="button" aria-pressed={(l == Lang::Es).to_string()} onclick={pick(Lang::Es)} lang="es">{ "Español" }</button>
                        </div>
                        <div class="seg" role="group" aria-label={l.t("Theme", "Tema")}>
                            <button type="button" aria-pressed={(theme == Theme::Night).to_string()} onclick={set_theme(Theme::Night)}>{ l.t("Night", "Noche") }</button>
                            <button type="button" aria-pressed={(theme == Theme::Day).to_string()} onclick={set_theme(Theme::Day)}>{ l.t("Day", "Día") }</button>
                        </div>
                    </div>
                    <SendLink />
                    <p class="sheet-note">{ l.t("Cantus runs on Windows, Linux and macOS (preview).", "Cantus funciona en Windows, Linux y macOS (vista previa).") }</p>
                </div>
            }
        </header>
    }
}

/// `navigator.<name>`, if the browser has it.
fn navigator_get(name: &str) -> Option<JsValue> {
    let nav = web_sys::window()?.navigator();
    js_sys::Reflect::get(&nav, &name.into())
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
}

/// Hands the download page's address to the phone's share sheet (mail it, message it to
/// yourself), or copies it when the browser has no share sheet. Nothing is sent by the site.
#[function_component(SendLink)]
pub fn send_link() -> Html {
    let l = use_lang().lang;
    let copied = use_state(|| false);
    let url = format!("{SITE}{}", href(l, "/download"));
    let click = {
        let copied = copied.clone();
        let url = url.clone();
        Callback::from(move |_| {
            let Some(nav) = web_sys::window().map(|w| w.navigator()) else {
                return;
            };
            if let Some(share) =
                navigator_get("share").and_then(|f| f.dyn_into::<js_sys::Function>().ok())
            {
                let data = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&data, &"title".into(), &"Cantus".into());
                let _ = js_sys::Reflect::set(
                    &data,
                    &"text".into(),
                    &l.t(
                        "Cantus, a free DAW — download it on your computer",
                        "Cantus, un DAW libre — descárgalo en tu computadora",
                    )
                    .into(),
                );
                let _ = js_sys::Reflect::set(&data, &"url".into(), &url.as_str().into());
                // A dismissed sheet rejects its promise: nothing to do.
                if let Ok(p) = share.call1(&nav, &data) {
                    if let Ok(p) = p.dyn_into::<js_sys::Promise>() {
                        wasm_bindgen_futures::spawn_local(async move {
                            let _ = wasm_bindgen_futures::JsFuture::from(p).await;
                        });
                    }
                }
                return;
            }
            if let Some(clip) = navigator_get("clipboard") {
                if let Some(write) = js_sys::Reflect::get(&clip, &"writeText".into())
                    .ok()
                    .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
                {
                    let _ = write.call1(&clip, &url.as_str().into());
                    copied.set(true);
                }
            }
        })
    };
    html! {
        <div class="send-link">
            <button type="button" class="btn btn-primary" onclick={click}>
                { icons::share() }{ l.t("Send the link to my computer", "Enviar el enlace a mi computadora") }
            </button>
            <p class="send-done" role="status">
                if *copied { { l.t("Link copied — paste it in a message to yourself.", "Enlace copiado — pégalo en un mensaje para ti.") } }
            </p>
        </div>
    }
}
