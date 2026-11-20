//! The frame every page plugs into (round 39, P1-3): the nav, one page by the address, the
//! footer; the language, theme and route contexts. The same component renders in the
//! browser and in the pre-renderer (`prerender.rs`), from the [`Page`] it is given.
//!
//! The home has at most nine chapters: hero and proof, tour, instruments and effects, listen,
//! measured, free and open, download, FAQ, footer. What the long page held beyond them moved,
//! whole, to its own page: /studio, /ai, /roadmap, /measured, /download, /learn and one page
//! per instrument and effect.

use yew::prelude::*;

use crate::lang::{Lang, LangCtx};
use crate::route::{Page, Route};
use crate::router::{Arrive, RouteCtx, Visit};
use crate::theme::{Theme, ThemeCtx};
use crate::{ahead, ai, body, foot, fx, listen, nav, pages, proof, score, session};
use crate::{spotlight, studio, top, tour, why};

#[derive(Properties, PartialEq)]
pub struct AppProps {
    pub page: Page,
}

#[function_component(App)]
pub fn app(p: &AppProps) -> Html {
    let at = use_state(|| Visit {
        page: p.page,
        arrive: Arrive::Loaded,
        seq: 0,
    });
    let page = at.page;
    // The count lives outside the state: the document's listener keeps the callback of the
    // first render, whose state handle would always count from 0.
    let count = use_mut_ref(|| 0u32);
    let go = {
        let at = at.clone();
        Callback::from(move |(page, arrive): (Page, Arrive)| {
            let seq = {
                let mut c = count.borrow_mut();
                *c = c.wrapping_add(1);
                *c
            };
            at.set(Visit { page, arrive, seq })
        })
    };

    #[cfg(target_arch = "wasm32")]
    {
        let go = go.clone();
        use_effect_with((), move |_| crate::router::browser::listen(go));
        use_effect_with(*at, |v| crate::router::browser::arrived(v.page, v.arrive));
    }

    let route_ctx = RouteCtx {
        page,
        go: {
            let go = go.clone();
            Callback::from(move |to: Page| {
                #[cfg(target_arch = "wasm32")]
                crate::router::browser::push(to, "");
                go.emit((to, Arrive::Fresh))
            })
        },
    };
    let lang_ctx = LangCtx {
        lang: page.lang,
        set: {
            let go = route_ctx.go.clone();
            Callback::from(move |l: Lang| {
                l.remember();
                go.emit(page.in_lang(l));
            })
        },
    };
    let theme = use_state(Theme::detect);
    let theme_ctx = ThemeCtx {
        theme: *theme,
        set: {
            let theme = theme.clone();
            Callback::from(move |t: Theme| {
                t.apply(true);
                theme.set(t);
            })
        },
    };

    // A page the visitor went to settles in (motion.css); the one the address opened does not,
    // and a jump within the page keeps what its page was drawn with.
    let mount = use_mut_ref(|| (String::new(), false));
    let arriving = {
        let mut m = mount.borrow_mut();
        let path = page.path();
        if m.0 != path {
            *m = (path, at.arrive != Arrive::Loaded);
        }
        m.1
    };

    html! {
        <ContextProvider<RouteCtx> context={route_ctx}>
        <ContextProvider<LangCtx> context={lang_ctx}>
        <ContextProvider<ThemeCtx> context={theme_ctx}>
            <a class="skip" href="#main">{ page.lang.t("Skip to the content", "Saltar al contenido") }</a>
            <nav::Nav />
            <main id="main" key={page.path()} class={classes!(arriving.then_some("is-arriving"))}>
                { body_of(page) }
            </main>
            // The gold cable down the page's margin (motion.css): lit above the middle of the
            // screen, a pulse running down it, a branch to each chapter's mark as it arrives.
            <div class="cable" aria-hidden="true"><i class="cable-lit"></i><i class="cable-pulse"></i></div>
            <foot::Footer />
        </ContextProvider<ThemeCtx>>
        </ContextProvider<LangCtx>>
        </ContextProvider<RouteCtx>>
    }
}

fn body_of(page: Page) -> Html {
    let l = page.lang;
    let to = |r: Route| Page::new(l, r).path();
    match page.route {
        Route::Home => html! {
            <>
                // 1. Hero and proof (the measured strip is in the hero; GitHub's counts stay
                //    hidden below their floor), and the playable rack.
                <top::Hero />
                <proof::Proof />
                <top::Groove />
                // 2. Tour
                <tour::Tour />
                <More href={to(Route::Studio)} label={l.t("Recording mode, the studio, the session view and the score", "Modo de grabación, el estudio, la vista de sesión y la partitura")} />
                // 3. Instruments and effects
                <spotlight::Spotlight />
                <fx::Effects />
                <More href={format!("{}#try", to(Route::Effects))} label={l.t("Try them on a guitar: each effect's presets, dry or wet", "Pruébalos en una guitarra: los presets de cada efecto, secos o con efecto")} />
                // 4. Listen
                <listen::Listen />
                // 5. Measured
                <pages::measured::MeasuredSection />
                // 6. Free and open
                <body::Free />
                // 7. Download
                <body::Get />
                <More href={format!("{}#support", to(Route::Download))} label={l.t("Every system, first steps and ways to support", "Cada sistema, primeros pasos y formas de apoyar")} />
                // 8. FAQ
                <ahead::Faq />
                <More href={to(Route::Roadmap)} label={l.t("Where it is and where it goes", "Dónde está y a dónde va")} />
                // 9. The footer, after <main>.
            </>
        },
        Route::Download => html! {
            <>
                <pages::download::DownloadPage />
                <body::Support />
            </>
        },
        Route::Instruments => html! { <pages::unit::UnitIndex /> },
        Route::Effects => html! { <pages::unit::UnitIndex effects=true /> },
        Route::Instrument(name) | Route::Effect(name) => {
            html! { <pages::unit::UnitPage file={name} /> }
        }
        Route::Measured => html! { <pages::measured::MeasuredPage /> },
        Route::Studio => html! {
            <>
                <PageHead title={l.t("The studio", "El estudio")}
                          lede={l.t("Record, mix, perform and write: everything beyond the tour.", "Graba, mezcla, toca y escribe: todo lo que va más allá del recorrido.")} />
                <studio::RecordMode />
                <studio::Studio />
                <session::Session />
                <score::Score />
                <body::Engine />
                <tour::Features />
            </>
        },
        Route::Ai => html! {
            <>
                <PageHead title={l.t("AI on your computer", "IA en tu computadora")} />
                <ai::Ai />
            </>
        },
        Route::Roadmap => html! {
            <>
                <PageHead title={l.t("Roadmap", "Hoja de ruta")} />
                <ahead::Roadmap />
            </>
        },
        Route::Learn => html! { <pages::learn::Learn /> },
        Route::Why => html! {
            <>
                <PageHead title={l.t("Why I'm building Cantus", "Por qué construyo Cantus")} />
                <why::Why />
            </>
        },
    }
}

#[derive(Properties, PartialEq)]
pub struct PageHeadProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub lede: Option<AttrValue>,
}

/// A page's own heading (its one `<h1>`) under a way back home. Pages whose content brings
/// its own `<h1>` (a unit's page) need none.
#[function_component(PageHead)]
pub fn page_head(p: &PageHeadProps) -> Html {
    let l = crate::lang::use_lang().lang;
    html! {
        <header class="page-head">
            <div class="wrap">
                <p class="crumbs"><a href={Page::new(l, Route::Home).path()}>{ "Cantus" }</a></p>
                <h1>{ p.title.clone() }</h1>
                if let Some(lede) = &p.lede { <p class="lede">{ lede.clone() }</p> }
            </div>
        </header>
    }
}

#[derive(Properties, PartialEq)]
struct MoreProps {
    href: String,
    label: AttrValue,
}

/// A chapter's way on to the page that holds the rest of it, in the chapter's own band.
#[function_component(More)]
fn more(p: &MoreProps) -> Html {
    html! {
        <div class="chapter-more">
            <p class="wrap"><a class="link" href={p.href.clone()}>{ p.label.clone() }<i aria-hidden="true">{ " →" }</i></a></p>
        </div>
    }
}
