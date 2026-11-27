//! `/plugins/` and `/plugins/<effect>/` (round 42, BACKLOG §6a): Cantus's effects as CLAP and
//! VST3 plug-ins on their own, for people who come for the effects and not the DAW. The
//! visitor's system is chosen for them (`platform.rs`) and can be changed; each button is
//! live only when the release holds its file (`plugins.rs`), with the file's size and SHA-256;
//! then how to install it, what an unsigned file asks of the system, the licence, and the way
//! to the whole DAW. An effect's page reuses its picture, its with/without excerpt and its
//! "Try it" from the effect's own page (`unit.rs`, `fx.rs`, `tryit.rs`).

use yew::prelude::*;

use crate::fx::AbPlayer;
use crate::icons;
use crate::lang::{use_lang, Lang};
use crate::nav::href;
use crate::pages::download::Hash;
use crate::platform::Os;
use crate::plugins::{self, paras, Got, Sys};
use crate::release::megabytes;
use crate::units::{pic_lang, shade_now, Unit};
use crate::units_data::EFFECTS;

/// The effects exported as plug-ins, in the site's order.
fn effects() -> impl Iterator<Item = &'static Unit> {
    EFFECTS.iter().filter(|u| u.plugin)
}

/// The facts and text are here: at once natively and on a first load of these pages (main.rs
/// waits for them); after a move from another page, once `/plugins.json` has arrived.
#[hook]
fn use_ready() -> bool {
    let ready = use_state(plugins::loaded);
    {
        let ready = ready.clone();
        use_effect_with((), move |_| {
            #[cfg(target_arch = "wasm32")]
            if !*ready {
                wasm_bindgen_futures::spawn_local(async move {
                    plugins::load().await;
                    ready.set(plugins::loaded());
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _ = ready;
        });
    }
    *ready
}

/// The visitor's system, chosen for them and changeable.
#[hook]
fn use_sys() -> UseStateHandle<Sys> {
    use_state(|| Sys::of(Os::detect()))
}

#[derive(Properties, PartialEq)]
struct PickProps {
    sys: UseStateHandle<Sys>,
}

/// Windows, Linux, macOS: which system's file and steps the page shows.
#[function_component(SysPick)]
fn sys_pick(p: &PickProps) -> Html {
    let l = use_lang().lang;
    let buttons = Sys::ALL.iter().map(|&s| {
        let sys = p.sys.clone();
        html! {
            <button type="button" aria-pressed={(s == *p.sys).to_string()}
                    onclick={Callback::from(move |_| sys.set(s))}>{ s.name() }</button>
        }
    });
    html! {
        <div class="ab-seg pl-sys" role="group" aria-label={l.t("Your system", "Tu sistema")}>{ for buttons }</div>
    }
}

#[derive(Properties, PartialEq)]
struct GetProps {
    file: Option<Got>,
    sys: Sys,
    label: AttrValue,
}

/// The button, live only with the release's file, its size, what it runs on and its SHA-256.
#[function_component(Get)]
fn get(p: &GetProps) -> Html {
    let l = use_lang().lang;
    let needs = p.sys.needs(l);
    match &p.file {
        Some(f) => {
            let cmd = if p.sys == Sys::Windows {
                format!("{} .\\{}", p.sys.hash_cmd(), f.name)
            } else {
                format!("{} {}", p.sys.hash_cmd(), f.name)
            };
            html! {
                <div class="pl-get">
                    <a class="btn btn-primary" href={f.href.clone()}>{ icons::arrow_down() }{ p.label.clone() }<small>{ megabytes(f.size) }</small></a>
                    <p class="dl-meta"><b>{ f.name.clone() }</b><span>{ needs }</span></p>
                    if !f.sha256.is_empty() {
                        <Hash cmd={cmd} sha={f.sha256.clone()} />
                    }
                </div>
            }
        }
        None => html! {
            <div class="pl-get">
                <span class="btn btn-primary is-off" aria-disabled="true">{ icons::arrow_down() }{ p.label.clone() }</span>
                <p class="dl-meta"><b>{ l.t("Not out yet", "Aún no publicado") }</b><span>{ needs }</span></p>
                <p class="pl-soon" role="note">{ for paras("soon", l) }</p>
            </div>
        },
    }
}

/// Installing, the unsigned files, the licence and the whole DAW, for the chosen system: HTML
/// made by the pre-renderer (`plugins::notes_html`; the WebAssembly reads it, its budget).
fn notes(sys: Sys, l: Lang) -> Html {
    let key = format!("html-notes-{}", sys.key());
    html! {
        <section class="section pl-notes" id="install">
            { Html::from_html_unchecked(AttrValue::from(plugins::piece(&key, l))) }
        </section>
    }
}

/// The head of both pages on the lapis: where you are, what it is, and the download card.
fn band(l: Lang, unit: Option<&Unit>, lede: Html, card: Html) -> Html {
    let crumb = match unit {
        Some(u) => html! {
            <>
                <a href={href(l, "/plugins")}>{ "Plug-ins" }</a><span aria-hidden="true">{ "/" }</span>
                <span aria-current="page">{ u.name }</span>
            </>
        },
        None => html! { <span aria-current="page">{ "Plug-ins" }</span> },
    };
    let (eyebrow, title) = match unit {
        Some(u) => (
            format!("{} · plug-in", l.t(u.kind.0, u.kind.1)),
            u.name.to_string(),
        ),
        None => (
            l.t(
                "Plug-ins · free, for any DAW",
                "Plug-ins · gratis, para cualquier DAW",
            )
            .to_string(),
            l.t(
                "Cantus's effects, in your DAW",
                "Los efectos de Cantus, en tu DAW",
            )
            .to_string(),
        ),
    };
    html! {
        <div class="page-band lapis"><div class="wrap dl-in">
            <div class="dl-main">
                <nav class="crumbs" aria-label={l.t("Where you are", "Dónde estás")}>
                    <a href={href(l, "/")}>{ "Cantus" }</a><span aria-hidden="true">{ "/" }</span>
                    { crumb }
                </nav>
                <span class="eyebrow">{ eyebrow }</span>
                <h1>{ title }</h1>
                <p class="lede">{ lede }</p>
                <ul class="pl-chips" aria-label={l.t("Formats", "Formatos")}>
                    <li>{ "CLAP" }</li><li>{ "VST3" }</li>
                    <li class="is-soon">{ l.t("AU · coming", "AU · pronto") }</li>
                </ul>
                <p class="dl-other">{ for paras("face", l) }</p>
            </div>
            <aside class="dl-before" id="all">{ card }</aside>
        </div></div>
    }
}

/// The download card's button for `sys`.
fn download_label(l: Lang, sys: Sys) -> String {
    format!("{} {}", l.t("Download for", "Descargar para"), sys.name())
}

/// `/plugins/`: every effect, all at once, and how to install them.
#[function_component(PluginsIndex)]
pub fn plugins_index() -> Html {
    let l = use_lang().lang;
    let _ready = use_ready();
    let sys = use_sys();
    let n = effects().count();
    let card = html! {
        <>
            <h2 class="plain">{ format!("{} {n} {}", l.t("All", "Los"), l.t("effects at once", "efectos a la vez")) }</h2>
            <SysPick sys={sys.clone()} />
            <Get file={plugins::all(*sys)} sys={*sys} label={download_label(l, *sys)} />
        </>
    };
    html! {
        <>
            { band(l, None, html! { for paras("lede", l) }, card) }
            <section class="section">
                <div class="wrap">
                    <header class="sec-head">
                        <h2>{ l.t("One at a time", "Uno a uno") }</h2>
                        <p>{ l.t("Each effect's page has its download for your system, its picture and, for most, its sound.", "La página de cada efecto tiene su descarga para tu sistema, su imagen y, en la mayoría, su sonido.") }</p>
                    </header>
                    { Html::from_html_unchecked(AttrValue::from(plugins::piece("html-grid", l))) }
                </div>
            </section>
            { notes(*sys, l) }
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct PluginProps {
    /// The effect's `units_data` `file`: its plug-in's slug.
    pub file: AttrValue,
}

/// `/plugins/<effect>/`: its download per system, its picture, its sound, how to install it.
#[function_component(PluginPage)]
pub fn plugin_page(p: &PluginProps) -> Html {
    let l = use_lang().lang;
    let _ready = use_ready();
    let sys = use_sys();
    let list: Vec<&'static Unit> = effects().collect();
    let Some(i) = list.iter().position(|u| u.file == p.file.as_str()) else {
        return html! {
            <section class="page-band lapis"><div class="wrap">
                <h1>{ l.t("No such plug-in", "No hay tal plug-in") }</h1>
                <p><a href={href(l, "/plugins")}>{ l.t("See them all →", "Verlos todos →") }</a></p>
            </div></section>
        };
    };
    let u = list[i];
    let (prev, next) = (
        list[(i + list.len() - 1) % list.len()],
        list[(i + 1) % list.len()],
    );
    let big = format!("/units/{}-{}{}.webp", u.file, shade_now(), pic_lang(l));
    let others = Sys::ALL.iter().filter(|&&s| s != *sys).map(|&s| {
        let f = plugins::file(&plugins::zip_name(u.file, s));
        html! {
            <li><strong>{ s.name() }</strong>
                if let Some(f) = f {
                    <a href={f.href}>{ format!("{} · {}", l.t("Download", "Descargar"), megabytes(f.size)) }</a>
                } else {
                    <span>{ l.t("Next beta", "Próxima beta") }</span>
                }
            </li>
        }
    });
    let card = html! {
        <>
            <h2 class="plain">{ format!("{} {}", l.t("Get", "Consigue"), u.name) }</h2>
            <SysPick sys={sys.clone()} />
            <Get file={plugins::file(&plugins::zip_name(u.file, *sys))} sys={*sys} label={download_label(l, *sys)} />
            <ul class="pl-others" aria-label={l.t("Other systems", "Otros sistemas")}>{ for others }</ul>
            <p class="dl-other"><a href={format!("{}#all", href(l, "/plugins"))}>{ l.t("Or every effect in one zip →", "O todos los efectos en un zip →") }</a></p>
        </>
    };
    let pager = |w: &Unit, label: &'static str| {
        html! { <a href={href(l, &format!("/plugins/{}", w.file))}><small>{ label }</small><b>{ w.name }</b></a> }
    };
    html! {
        <article class="unit-full">
            { band(l, Some(u), html! { crate::unit_text::pitch(u, l) }, card) }
            <div class="section uf-body"><div class="wrap">
                <figure class="uf-shot">
                    <img key={big.clone()} src={big} width={u.big_in(l).0.to_string()} height={u.big_in(l).1.to_string()}
                         alt={format!("{} — {}", u.name, l.t("its window", "su ventana"))} />
                </figure>
                <div class="up-body">
                    <div class="up-main">
                        <p class="up-what">{ crate::unit_text::what(u, l) }</p>
                        <p><a class="link" href={href(l, &format!("/effects/{}", u.file))}>{ format!("{} {} →", l.t("All about", "Todo sobre"), u.name) }</a></p>
                    </div>
                    if let Some((file, song)) = u.ab {
                        <div class="up-side up-ab">
                            <h2 class="h4">{ l.t("Hear it, with and without", "Escúchalo, con y sin") }</h2>
                            <AbPlayer key={file} file={file} song={song} />
                        </div>
                    }
                </div>
                if crate::tryit::has(u.file) {
                    <crate::tryit::TryIt key={u.file} unit={AttrValue::from(u.file)} />
                }
            </div></div>
            { notes(*sys, l) }
            <div class="section"><div class="wrap">
                <nav class="uf-pager" aria-label={l.t("Next and previous", "Siguiente y anterior")}>
                    { pager(prev, l.t("← Previous", "← Anterior")) }
                    { pager(next, l.t("Next →", "Siguiente →")) }
                </nav>
            </div></div>
        </article>
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use crate::route::{Page, Route};

    const TEMPLATE: &str = "<!doctype html><html lang=\"en\"><head><!--page-head--><title>x</title><!--/page-head--></head><body><div id=\"app\"></div></body></html>";

    fn render(page: Page) -> String {
        crate::prerender::fill(TEMPLATE, page).expect("a page")
    }

    /// Before a release holds the zips, no page links one: every button is the disabled one,
    /// with its note; the instruments get a note and no button.
    #[test]
    fn no_page_links_a_zip_the_release_lacks() {
        for lang in [Lang::En, Lang::Es] {
            let mut pages = vec![Page::new(lang, Route::Plugins)];
            pages.extend(effects().map(|u| Page::new(lang, Route::Plugin(u.file))));
            for p in pages {
                let html = render(p);
                for name in html.match_indices(plugins::DOWNLOADS).map(|(at, _)| {
                    let rest = &html[at + plugins::DOWNLOADS.len()..];
                    rest[..rest.find('"').unwrap_or(rest.len())].to_string()
                }) {
                    if !name.starts_with("Cantus-plugin") {
                        continue; // the DAW's own files (the bar's download)
                    }
                    assert!(
                        plugins::file(&name).is_some(),
                        "{} links {name}, which the release lacks",
                        p.path()
                    );
                }
                assert!(
                    html.contains("CLAP") && html.contains("VST3"),
                    "{}",
                    p.path()
                );
                assert!(html.contains("GPL-3.0-or-later"), "{}", p.path());
                assert!(html.contains("TRADEMARKS.md"), "{}", p.path());
                assert!(
                    html.contains(r"C:\Program Files\Common Files\VST3"),
                    "{}",
                    p.path()
                );
                if crate::release::REL_PLUGINS.is_empty() && p.route != Route::Plugins {
                    assert!(html.contains("aria-disabled=\"true\""), "{}", p.path());
                }
            }
        }
        let index = render(Page::new(Lang::En, Route::Plugins));
        assert!(index.contains("are not plug-ins yet"));
        for u in effects() {
            assert!(
                index.contains(&format!("href=\"/plugins/{}/\"", u.file)),
                "{}",
                u.file
            );
        }
        let es = render(Page::new(Lang::Es, Route::Plugin("aequalis")));
        assert!(es.contains("href=\"/es/plugins/\"") && es.contains("Aún sin firma de código"));
    }
}
