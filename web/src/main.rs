//! cantusdaw.com — the Cantus website, a Yew single-page app (ADR 0034).

mod ahead;
mod audio;
mod body;
mod faces;
mod fx;
mod icons;
mod lang;
mod listen;
mod platform;
mod rack;
mod score;
mod session;
mod shelf;
mod theme;
mod top;
mod tour;

use yew::prelude::*;

use lang::{Lang, LangCtx};

#[function_component(App)]
fn app() -> Html {
    let lang = use_state(Lang::detect);
    let ctx = LangCtx {
        lang: *lang,
        set: {
            let lang = lang.clone();
            Callback::from(move |l: Lang| {
                l.remember();
                lang.set(l);
            })
        },
    };
    use_effect_with(*lang, |l| {
        l.remember();
    });
    html! {
        <ContextProvider<LangCtx> context={ctx}>
            <top::Nav />
            <main>
                <top::Hero />
                <tour::Tour />
                <listen::Listen />
                <tour::Features />
                <session::Session />
                <score::Score />
                <faces::Faces />
                <fx::Effects />
                <body::Engine />
                <body::Free />
                <body::Support />
                <body::Get />
                <ahead::Ahead />
                <body::More />
            </main>
            <body::Footer />
        </ContextProvider<LangCtx>>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
