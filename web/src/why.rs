//! `/why/` — why Cantus is made, in its author's words (round 41; the owner approved the text
//! on 2026-11-20: "tiene mis valores cristianos y mis ganas de hacer algo excelente con la
//! ambición de un futuro ser el mejor del mercado y completamente gratis"). The same text is
//! `docs/WHY.md`, for the repository.

use yew::prelude::*;

use crate::lang::use_lang;
use crate::unit_text::page;

/// The four paragraphs, English and Spanish: compiled natively only (the pre-renderer and the
/// tests); the WebAssembly reads them from `/unit-text.json` (`unit_text::page`).
#[cfg(not(target_arch = "wasm32"))]
pub const PARAGRAPHS: [(&str, &str); 4] = [
    (
        "I make Cantus because a musician should be able to finish a song without paying a subscription, creating an account or wondering whether their tools will still be theirs next year. So Cantus is completely free — every instrument, every effect, every feature, for everyone. It sends nothing about you anywhere, and its source is open: the application under the GPL, the engine under the MPL, so it cannot be taken back.",
        "Hago Cantus porque un músico debería poder terminar una canción sin pagar una suscripción, sin crear una cuenta y sin preguntarse si sus herramientas seguirán siendo suyas el año que viene. Por eso Cantus es completamente gratis: cada instrumento, cada efecto, cada función, para todos. No envía nada sobre ti a ningún lado y su código es abierto —la aplicación bajo la GPL, el motor bajo la MPL—, así que nadie puede quitártelo.",
    ),
    (
        "Free must not mean second best. My ambition is for Cantus to become, one day, undeniably and measurably the best DAW on the market — free or paid — in features, performance, quality and stability. It is not there yet, and this site says so: it shows the numbers measured against FL Studio and REAPER, the row Cantus loses as plainly as the ones it wins.",
        "Gratis no puede significar de segunda. Mi ambición es que Cantus llegue a ser, algún día, de forma innegable y medible, el mejor DAW del mercado —gratuito o de pago— en funciones, rendimiento, calidad y estabilidad. Todavía no lo es, y este sitio lo dice: muestra las cifras medidas frente a FL Studio y REAPER, y la fila que Cantus pierde con la misma claridad que las que gana.",
    ),
    (
        "I am a Christian, and Cantus is built under one verse: \u{201C}And whatever you do, work heartily, as for the Lord and not for men\u{201D} (Colossians 3:23, World English Bible). For me that means doing this work with excellence and honesty: finishing what is started, testing what is claimed, and studying humbly those who did it well before us. It is why the footer says \u{201C}Christ is King\u{201D}.",
        "Soy cristiano, y Cantus se construye bajo un versículo: \u{201C}Y todo lo que hagáis, hacedlo de ánimo, como al Señor, y no á los hombres\u{201D} (Colosenses 3:23, Reina-Valera 1909). Para mí eso significa hacer este trabajo con excelencia y honestidad: terminar lo que se empieza, probar lo que se afirma y estudiar con humildad a quienes lo hicieron bien antes que nosotros. Por eso el pie de página dice \u{201C}Viva Cristo Rey\u{201D}.",
    ),
    (
        "Cantus never asks you to share that belief. A verse greets you when it starts, in your language, and one switch turns it off for good. Everyone gets the same program, whatever they believe, whatever they give — or nothing at all.",
        "Cantus nunca te pide compartir esa creencia. Un versículo te saluda al arrancar, en tu idioma, y un interruptor lo apaga para siempre. Todos reciben el mismo programa, crean lo que crean y den lo que den, o nada.",
    ),
];

/// The page's body: the four paragraphs and the signature (the heading is `PageHead`'s).
#[function_component(Why)]
pub fn why() -> Html {
    let l = use_lang().lang;
    html! {
        <section class="section why">
            <div class="wrap">
                { for (0..).map_while(|i| page("why", i, l)).map(|p| html! { <p class="why-p">{ p }</p> }) }
                <p class="why-sign">{ "— Luis Ángel Almazán" }</p>
            </div>
        </section>
    }
}
