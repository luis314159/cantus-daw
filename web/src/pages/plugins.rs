//! `/plugins/` and `/plugins/<effect>/` (round 42, BACKLOG §6a): Cantus's effects as CLAP and
//! VST3 plug-ins on their own, for people who come for the effects and not the DAW. Each
//! button is live only when the release holds its file (`plugins.rs`), with the file's size and
//! SHA-256; then how to install it, what an unsigned file asks of the system, the licence, and
//! the way to the whole DAW. An effect's page reuses its picture, its with/without excerpt and
//! its "Try it" from the effect's own page (`fx.rs`, `tryit.rs`).
//!
//! **Drawn as HTML, not as components** (round 42): as Yew components these pages put the
//! WebAssembly 12 KB over its 250 KB budget, and nearly all of them is text. The pages are
//! written as strings (the same function in the pre-renderer and in the browser) and inserted
//! as they are; only the players are components. The system is a radio group (Windows, Linux,
//! macOS) whose choice shows that system's card and steps by CSS alone (`plugins.css`), so the
//! static page works before the app runs; the app then ticks the visitor's own system.

use std::fmt::Write;

use yew::prelude::*;

use crate::fx::AbPlayer;
use crate::lang::{use_lang, Lang};
use crate::nav::href;
use crate::plugins::{self, paras, Got, Sys};
use crate::release::megabytes;
use crate::units::{pic_lang, shade_now, Unit};
use crate::units_data::EFFECTS;

/// The effects exported as plug-ins, in the site's order.
fn effects() -> impl Iterator<Item = &'static Unit> {
    EFFECTS.iter().filter(|u| u.plugin)
}

/// Text for HTML: `&`, `<`, `>` and `"` escaped.
fn esc(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// One of the site's line icons (`icons.rs`), as HTML.
fn icon(d: &str) -> String {
    format!(
        r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="{d}"></path></svg>"#
    )
}

const ARROW: &str = "M12 4v12M6 11l6 6 6-6M5 20h14";

fn raw(s: String) -> Html {
    Html::from_html_unchecked(AttrValue::from(s))
}

/// The radio group that picks the system; the first is ticked in the static page.
fn picker(l: Lang) -> String {
    let mut o = format!(
        r#"<div class="ab-seg pl-sys" role="radiogroup" aria-label="{}">"#,
        l.t("Your system", "Tu sistema")
    );
    for (i, s) in Sys::ALL.iter().enumerate() {
        let _ = write!(
            o,
            r#"<label><input type="radio" name="pl-sys" id="pl-sys-{}" value="{}"{}>{}</label>"#,
            s.key(),
            s.key(),
            if i == 0 { " checked" } else { "" },
            s.name()
        );
    }
    o + "</div>"
}

/// A button, live only with the release's file, its size, what it runs on and its SHA-256.
fn get(l: Lang, sys: Sys, file: Option<Got>) -> String {
    let label = format!("{} {}", l.t("Download for", "Descargar para"), sys.name());
    let needs = sys.needs(l);
    match file {
        Some(f) => {
            let mut o = format!(
                r#"<div class="pl-get"><a class="btn btn-primary" href="{}">{}{label}<small>{}</small></a><p class="dl-meta"><b>{}</b><span>{needs}</span></p>"#,
                esc(&f.href),
                icon(ARROW),
                megabytes(f.size),
                esc(&f.name)
            );
            if !f.sha256.is_empty() {
                let dot = if sys == Sys::Windows { ".\\" } else { "" };
                let _ = write!(
                    o,
                    r#"<div class="hash"><code class="hash-cmd">{} {dot}{}</code><div class="hash-row"><code class="hash-sum" aria-label="SHA-256">{}</code></div></div>"#,
                    sys.hash_cmd(),
                    esc(&f.name),
                    esc(&f.sha256)
                );
            }
            o + "</div>"
        }
        None => format!(
            r#"<div class="pl-get"><span class="btn btn-primary is-off" aria-disabled="true">{}{label}</span><p class="dl-meta"><b>{}</b><span>{needs}</span></p><p class="pl-soon" role="note">{}</p></div>"#,
            icon(ARROW),
            l.t("Not out yet", "Aún no publicado"),
            esc(&paras("soon", l).concat())
        ),
    }
}

/// The head of both pages on the lapis — where you are, what it is — and the download card,
/// one per system (`card`).
fn band(l: Lang, unit: Option<&Unit>, card: &dyn Fn(Sys) -> String) -> String {
    let home = href(l, "/");
    let (crumb, eyebrow, title, lede, h2) = match unit {
        Some(u) => (
            format!(
                r#"<a href="{}">Plug-ins</a><span aria-hidden="true">/</span><span aria-current="page">{}</span>"#,
                href(l, "/plugins"),
                esc(u.name)
            ),
            format!("{} · plug-in", l.t(u.kind.0, u.kind.1)),
            u.name.to_string(),
            crate::unit_text::pitch(u, l),
            format!("{} {}", l.t("Get", "Consigue"), u.name),
        ),
        None => (
            r#"<span aria-current="page">Plug-ins</span>"#.to_string(),
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
            paras("lede", l).concat(),
            format!(
                "{} {} {}",
                l.t("All", "Los"),
                effects().count(),
                l.t("effects at once", "efectos a la vez")
            ),
        ),
    };
    let mut cards = String::new();
    for s in Sys::ALL {
        let _ = write!(
            cards,
            r#"<div class="pl-for pl-for-{}">{}</div>"#,
            s.key(),
            card(s)
        );
    }
    format!(
        r#"<div class="page-band lapis"><div class="wrap dl-in"><div class="dl-main"><nav class="crumbs" aria-label="{}"><a href="{home}">Cantus</a><span aria-hidden="true">/</span>{crumb}</nav><span class="eyebrow">{}</span><h1>{}</h1><p class="lede">{}</p><ul class="pl-chips" aria-label="{}"><li>CLAP</li><li>VST3</li><li class="is-soon">{}</li></ul><p class="dl-other">{}</p></div><aside class="dl-before" id="all"><h2 class="plain">{}</h2>{}{cards}</aside></div></div>"#,
        l.t("Where you are", "Dónde estás"),
        esc(&eyebrow),
        esc(&title),
        esc(&lede),
        l.t("Formats", "Formatos"),
        l.t("AU · coming", "AU · pronto"),
        esc(&paras("face", l).concat()),
        esc(&h2),
        picker(l)
    )
}

/// Installing, the unsigned files, the licence and the whole DAW, one block per system.
fn notes(l: Lang) -> String {
    let gh = crate::top::GITHUB;
    let mut o = String::from(r#"<section class="section pl-notes" id="install">"#);
    for sys in Sys::ALL {
        let steps = paras(&format!("steps-{}", sys.key()), l);
        let step = |i: usize| esc(steps.get(i).map(String::as_str).unwrap_or(""));
        let mut folders = String::new();
        for (std, you, all) in sys.folders() {
            let _ = write!(
                folders,
                "<div><dt>{std}</dt><dd><span>{}</span><code>{}</code></dd><dd><span>{}</span><code>{}</code></dd></div>",
                l.t("For you", "Para ti"),
                esc(you),
                l.t("For everyone", "Para todos"),
                esc(all)
            );
        }
        let hosts: String = paras("hosts", l)
            .iter()
            .map(|h| format!("<li>{}</li>", esc(h)))
            .collect();
        let unsigned = paras(&format!("unsigned-{}", sys.key()), l);
        let mut side = format!(
            "<h3>{}{}</h3><p>{}</p>",
            icon("M12 3l10 18H2zM12 10v5M12 18v.01"),
            l.t("Not code-signed yet", "Aún sin firma de código"),
            esc(unsigned.first().map(String::as_str).unwrap_or(""))
        );
        if sys == Sys::Mac {
            for f in ["CLAP/Cantus*.clap", "VST3/Cantus*.vst3"] {
                let _ = write!(
                    side,
                    r#"<code class="pl-cmd">xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/{f}</code>"#
                );
            }
        }
        for t in unsigned.iter().skip(1) {
            let _ = write!(side, "<p>{}</p>", esc(t));
        }
        let _ = write!(
            side,
            "<h3>{}{}</h3>",
            icon("M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6zM8.5 12l2.5 2.5 4.5-5"),
            l.t("Licence", "Licencia")
        );
        for t in paras("licence", l) {
            let _ = write!(side, "<p>{}</p>", esc(&t));
        }
        let _ = write!(
            o,
            r#"<div class="wrap pl-for pl-for-{}"><h2>{} {}</h2><div class="pl-cols"><ol class="pl-steps"><li>{}</li><li>{}<dl class="pl-folders">{folders}</dl></li><li>{}<ul class="pl-hosts">{hosts}</ul></li></ol><div class="pl-side">{side}<p class="pl-links"><a href="{gh}/blob/master/LICENSE">GPL-3.0-or-later</a> · <a href="{gh}/blob/master/TRADEMARKS.md">{}</a> · <a href="{gh}/tree/master/crates/cantus-plugins">{}</a></p></div></div></div>"#,
            sys.key(),
            l.t("Install on", "Instalar en"),
            sys.name(),
            step(0),
            step(1),
            step(2),
            l.t("Trademarks", "Marcas"),
            l.t("Source", "Código")
        );
    }
    let _ = write!(
        o,
        r#"<div class="wrap"><div class="stays-free"><p>{}</p><a class="btn btn-quiet" href="{}">{}</a></div></div></section>"#,
        esc(&paras("daw", l).concat()),
        href(l, "/download"),
        l.t("Get the whole DAW", "Consigue el DAW completo")
    );
    o
}

/// `/plugins/`'s body: the head with the zip with all, every effect's card, the notes.
fn index_html(l: Lang) -> String {
    let mut o = band(l, None, &|s| get(l, s, plugins::all(s)));
    let _ = write!(
        o,
        r#"<section class="section"><div class="wrap"><header class="sec-head"><h2>{}</h2><p>{}</p></header><ul class="unit-grid">"#,
        l.t("One at a time", "Uno a uno"),
        l.t(
            "Each effect's page has its download for your system, its picture and, for most, its sound.",
            "La página de cada efecto tiene su descarga para tu sistema, su imagen y, en la mayoría, su sonido."
        )
    );
    for u in effects() {
        let (w, h) = u.thumb_in(l);
        let _ = write!(
            o,
            r#"<li class="uc-li"><a class="unit-card" href="{}"><span class="uc-pic"><img src="/units/{}-thumb{}.webp" width="{w}" height="{h}" loading="lazy" alt=""></span><span class="uc-text"><strong>{}</strong><span class="uc-kind">{}</span><span class="uc-pitch">{}</span><span class="uc-more">CLAP · VST3 · Windows · Linux · macOS</span></span></a></li>"#,
            href(l, &format!("/plugins/{}", u.file)),
            u.file,
            pic_lang(l),
            esc(u.name),
            esc(l.t(u.kind.0, u.kind.1)),
            esc(&crate::unit_text::pitch(u, l))
        );
    }
    let _ = write!(
        o,
        r#"</ul><p class="pl-inst" role="note"><b>{} {} · {}</b> {}</p></div></section>"#,
        crate::units_data::INSTRUMENTS.len(),
        l.t("instruments", "instrumentos"),
        l.t("coming", "pronto"),
        esc(&paras("instruments", l).concat())
    );
    o + &notes(l)
}

/// An effect's card for `sys`: its button, the other systems, the zip with all.
fn effect_card(l: Lang, u: &Unit, sys: Sys) -> String {
    let mut o = get(l, sys, plugins::file(&plugins::zip_name(u.file, sys)));
    let _ = write!(
        o,
        r#"<ul class="pl-others" aria-label="{}">"#,
        l.t("Other systems", "Otros sistemas")
    );
    for s in Sys::ALL.into_iter().filter(|&s| s != sys) {
        let link = match plugins::file(&plugins::zip_name(u.file, s)) {
            Some(f) => format!(
                r#"<a href="{}">{} · {}</a>"#,
                esc(&f.href),
                l.t("Download", "Descargar"),
                megabytes(f.size)
            ),
            None => format!("<span>{}</span>", l.t("Next beta", "Próxima beta")),
        };
        let _ = write!(o, "<li><strong>{}</strong>{link}</li>", s.name());
    }
    let _ = write!(
        o,
        r##"</ul><p class="dl-other"><a href="{}#all">{}</a></p>"##,
        href(l, "/plugins"),
        l.t(
            "Or every effect in one zip →",
            "O todos los efectos en un zip →"
        )
    );
    o
}

/// The visitor's own system ticked, once the page is drawn (the static page ticks Windows).
#[hook]
fn use_own_system(ready: bool) {
    use_effect_with(ready, |_| {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let id = format!("pl-sys-{}", Sys::of(crate::platform::Os::detect()).key());
            if let Some(i) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(&id))
                .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
            {
                i.set_checked(true);
            }
        }
    });
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

/// `/plugins/`: every effect, all at once, and how to install them.
#[function_component(PluginsIndex)]
pub fn plugins_index() -> Html {
    let l = use_lang().lang;
    let ready = use_ready();
    use_own_system(ready);
    html! { <div class="pl-page">{ raw(index_html(l)) }</div> }
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
    let ready = use_ready();
    use_own_system(ready);
    let list: Vec<&'static Unit> = effects().collect();
    let Some(i) = list.iter().position(|u| u.file == p.file.as_str()) else {
        return html! {};
    };
    let u = list[i];
    let (prev, next) = (
        list[(i + list.len() - 1) % list.len()],
        list[(i + 1) % list.len()],
    );
    let (w, h) = u.big_in(l);
    let top = format!(
        r#"{}<div class="section uf-body"><div class="wrap"><figure class="uf-shot"><img src="/units/{}-{}{}.webp" width="{w}" height="{h}" alt="{} — {}"></figure><div class="up-body"><div class="up-main"><p class="up-what">{}</p><p><a class="link" href="{}">{} {} →</a></p></div></div></div></div>"#,
        band(l, Some(u), &|s| effect_card(l, u, s)),
        u.file,
        shade_now(),
        pic_lang(l),
        esc(u.name),
        l.t("its window", "su ventana"),
        esc(&crate::unit_text::what(u, l)),
        href(l, &format!("/effects/{}", u.file)),
        l.t("All about", "Todo sobre"),
        esc(u.name)
    );
    let pager = |w: &Unit, label: &str| {
        format!(
            r#"<a href="{}"><small>{label}</small><b>{}</b></a>"#,
            href(l, &format!("/plugins/{}", w.file)),
            esc(w.name)
        )
    };
    let bottom = format!(
        r#"{}<div class="section"><div class="wrap"><nav class="uf-pager" aria-label="{}">{}{}</nav></div></div>"#,
        notes(l),
        l.t("Next and previous", "Siguiente y anterior"),
        pager(prev, l.t("← Previous", "← Anterior")),
        pager(next, l.t("Next →", "Siguiente →"))
    );
    html! {
        <article class="unit-full pl-page">
            { raw(top) }
            if u.ab.is_some() || crate::tryit::has(u.file) {
                <div class="section pl-hear"><div class="wrap">
                    if let Some((file, song)) = u.ab {
                        <div class="up-ab">
                            <h2 class="h4">{ l.t("Hear it, with and without", "Escúchalo, con y sin") }</h2>
                            <AbPlayer key={file} file={file} song={song} />
                        </div>
                    }
                    if crate::tryit::has(u.file) {
                        <crate::tryit::TryIt key={u.file} unit={AttrValue::from(u.file)} />
                    }
                </div></div>
            }
            { raw(bottom) }
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

    /// No page links a zip the release lacks: before a release holds them every button is
    /// the disabled one, with its note; the instruments get a note and no button; every system
    /// has its card and its steps, Windows ticked in the static page.
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
                for s in Sys::ALL {
                    assert!(
                        html.contains(&format!("pl-for pl-for-{}\"", s.key())),
                        "{} {s:?}",
                        p.path()
                    );
                }
                assert!(html.contains("id=\"pl-sys-windows\" value=\"windows\" checked>"));
                assert!(html.contains("GPL-3.0-or-later") && html.contains("TRADEMARKS.md"));
                assert!(html.contains(r"C:\Program Files\Common Files\VST3"));
                assert!(html.contains("~/Library/Audio/Plug-Ins/VST3"));
                if crate::release::REL_PLUGINS.is_empty() && p.route != Route::Plugins {
                    assert!(html.contains("aria-disabled=\"true\""), "{}", p.path());
                }
                assert_eq!(html.matches("<h1").count(), 1, "{}", p.path());
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
        let es = render(Page::new(Lang::Es, Route::Plugin("fragor")));
        assert!(es.contains("href=\"/es/plugins/\"") && es.contains("Aún sin firma de código"));
        assert!(es.contains("class=\"section pl-hear\""), "Fragor's Try it");
    }

    #[test]
    fn text_is_escaped() {
        assert_eq!(esc("a < b & \"c\""), "a &lt; b &amp; &quot;c&quot;");
    }
}
