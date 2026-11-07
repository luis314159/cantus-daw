//! The instruments and the effects (round 32's faces, round 36's pages): a card per unit —
//! its window, its name, what it is, why it is worth a try — and, on a click, its page: the
//! window large in either theme, what it does, what stands out, its presets and, for the
//! effects that have one, the same excerpt with and without it. Keyboard: Enter opens, the
//! arrows go to the next or the previous unit, Esc closes and gives the focus back.

use wasm_bindgen::{closure::Closure, JsCast};
use yew::prelude::*;

use crate::fx::AbPlayer;
use crate::lang::use_lang;
use crate::sounds::{CardSound, PresetSounds};
use crate::units_data::{EFFECTS, INSTRUMENTS};

/// One native unit. Text is `(English, Spanish)`.
#[derive(PartialEq)]
pub struct Unit {
    /// The pictures' name in `assets/units/`: `-thumb`, `-dark`, `-light` `.webp`.
    pub file: &'static str,
    pub name: &'static str,
    pub kind: (&'static str, &'static str),
    /// One line: why it is worth a try.
    pub pitch: (&'static str, &'static str),
    /// What it does, in two or three sentences.
    pub what: (&'static str, &'static str),
    pub features: &'static [(&'static str, &'static str)],
    /// Factory presets (kits, for Pulsus II), counted in the program.
    pub presets: Option<u32>,
    /// Exported as a CLAP and VST3 plug-in for other DAWs.
    pub plugin: bool,
    /// The with/without excerpts (`assets/fx/audio/<name>-with.mp3`) and their demo song.
    pub ab: Option<(&'static str, &'static str)>,
    /// The large pictures' and the thumbnail's size, pixels.
    pub big: (u32, u32),
    pub thumb: (u32, u32),
}

/// The pictures' language: the Spanish page shows the editors in Spanish.
pub fn pic_lang(l: crate::lang::Lang) -> &'static str {
    l.t("", "-es")
}

/// The page's theme now (`dark` or `light`), the picture shown first.
pub fn shade_now() -> &'static str {
    let light = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|e| e.get_attribute("data-theme"))
        .is_some_and(|t| t == "light");
    if light {
        "light"
    } else {
        "dark"
    }
}

fn group(tab: usize) -> &'static [Unit] {
    if tab == 0 {
        &INSTRUMENTS[..]
    } else {
        &EFFECTS[..]
    }
}

fn body() -> Option<web_sys::HtmlElement> {
    web_sys::window()?.document()?.body()
}

/// Gives the focus to the element with this id, if it is there.
fn focus_id(id: &str) {
    if let Some(e) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = e.focus();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Nav {
    Prev,
    Next,
    Close,
}

#[function_component(Units)]
pub fn units() -> Html {
    let l = use_lang().lang;
    let tab = use_state(|| 0usize);
    // The open page: (group, index).
    let open = use_state(|| None::<(usize, usize)>);
    // The theme of the large picture; `None`: the page's.
    let shade = use_state(|| None::<&'static str>);
    // The open page as of now, for the keys (a handle keeps the value of its own render).
    let current = use_mut_ref(|| None::<(usize, usize)>);
    *current.borrow_mut() = *open;

    let go = {
        let (open, current) = (open.clone(), current.clone());
        Callback::from(move |n: Nav| {
            let Some((g, i)) = *current.borrow() else {
                return;
            };
            let len = group(g).len();
            match n {
                Nav::Prev => open.set(Some((g, (i + len - 1) % len))),
                Nav::Next => open.set(Some((g, (i + 1) % len))),
                Nav::Close => {
                    open.set(None);
                    focus_id(&format!("unit-{}", group(g)[i].file));
                }
            }
        })
    };

    // While a page is open: the page behind does not scroll, Esc and the arrows work
    // wherever the focus is, and Tab stays inside the page.
    {
        let go = go.clone();
        use_effect_with(open.is_some(), move |on| {
            let mut listener = None;
            if *on {
                if let Some(b) = body() {
                    let _ = b.class_list().add_1("unit-open");
                }
                focus_id("unit-close");
                let cb = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
                    move |e: web_sys::KeyboardEvent| {
                        match e.key().as_str() {
                            "Escape" => go.emit(Nav::Close),
                            "ArrowLeft" => go.emit(Nav::Prev),
                            "ArrowRight" => go.emit(Nav::Next),
                            "Tab" => trap(&e),
                            _ => return,
                        }
                        if e.key() != "Tab" {
                            e.prevent_default();
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
                if let Some(b) = body() {
                    let _ = b.class_list().remove_1("unit-open");
                }
            }
        });
    }

    let tabs = [
        (l.t("Instruments", "Instrumentos"), INSTRUMENTS.len()),
        (l.t("Effects", "Efectos"), EFFECTS.len()),
    ];
    let buttons = tabs.iter().enumerate().map(|(i, (name, n))| {
        let tab = tab.clone();
        html! {
            <button type="button" role="tab" class="tab" aria-selected={(i == *tab).to_string()}
                    aria-controls="unit-grid" onclick={Callback::from(move |_| tab.set(i))}>
                { *name }<span class="tab-n">{ *n }</span>
            </button>
        }
    });
    let cards = group(*tab).iter().enumerate().map(|(i, u)| {
        let (open, g) = (open.clone(), *tab);
        let heard = u.ab.is_some();
        html! {
            <li class="uc-li">
                <button type="button" class="unit-card" id={format!("unit-{}", u.file)} aria-haspopup="dialog"
                        onclick={Callback::from(move |_| open.set(Some((g, i))))}>
                    <span class="uc-pic">
                        <img src={format!("units/{}-thumb{}.webp", u.file, pic_lang(l))} width={u.thumb.0.to_string()} height={u.thumb.1.to_string()}
                             loading="lazy" alt="" />
                        if heard {
                            <span class="uc-badge">{ l.t("Listen", "Escúchalo") }</span>
                        }
                    </span>
                    <span class="uc-text">
                        <strong>{ u.name }</strong>
                        <span class="uc-kind">{ l.t(u.kind.0, u.kind.1) }</span>
                        <span class="uc-pitch">{ l.t(u.pitch.0, u.pitch.1) }</span>
                        <span class="uc-more" aria-hidden="true">{ l.t("See it large", "Verlo en grande") }{ " →" }</span>
                    </span>
                </button>
                <CardSound unit={u.file} />
            </li>
        }
    });

    let page = open.map(|(g, i)| {
        let list = group(g);
        let u = &list[i];
        let shown = shade.unwrap_or_else(shade_now);
        let pick = |s: &'static str| {
            let shade = shade.clone();
            Callback::from(move |_| shade.set(Some(s)))
        };
        let nav = |n: Nav| {
            let go = go.clone();
            Callback::from(move |_| go.emit(n))
        };
        let close_scrim = {
            let go = go.clone();
            Callback::from(move |e: MouseEvent| {
                // Only a click on the dimmed page itself, not one inside the panel.
                if e.target() == e.current_target() {
                    go.emit(Nav::Close);
                }
            })
        };
        let big = format!("units/{}-{}{}.webp", u.file, shown, pic_lang(l));
        let feats = u.features.iter().map(|f| html! {
            <li><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8.5l3 3 7-7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg><span>{ l.t(f.0, f.1) }</span></li>
        });
        let presets = u.presets.map(|n| {
            if u.file == "pulsus" {
                format!("{n} {}", l.t("factory kits", "kits de fábrica"))
            } else {
                format!("{n} {}", l.t("factory presets", "presets de fábrica"))
            }
        });
        // r37:library — Delay is edited in the inspector: its picture is that view.
        let inspector = u.file == "delay";
        html! {
            <div class="unit-scrim" onclick={close_scrim}>
                <div class="unit-page" role="dialog" aria-modal="true" aria-labelledby="unit-title" id="unit-page">
                    <header class="up-head">
                        <div class="up-title">
                            <span class="up-kind">{ l.t(u.kind.0, u.kind.1) }</span>
                            <h3 id="unit-title">{ u.name }</h3>
                        </div>
                        <span class="up-count">{ format!("{} / {}", i + 1, list.len()) }</span>
                        <div class="up-nav">
                            <button type="button" class="up-btn" onclick={nav(Nav::Prev)} aria-label={l.t("Previous", "Anterior")}>
                                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M10 3L5 8l5 5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>
                            </button>
                            <button type="button" class="up-btn" onclick={nav(Nav::Next)} aria-label={l.t("Next", "Siguiente")}>
                                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 3l5 5-5 5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>
                            </button>
                            <button type="button" class="up-btn" id="unit-close" onclick={nav(Nav::Close)} aria-label={l.t("Close", "Cerrar")}>
                                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
                            </button>
                        </div>
                    </header>
                    <figure class="up-shot">
                        <a href={big.clone()} target="_blank" rel="noopener" class="up-shot-link"
                           aria-label={format!("{} — {}", u.name, l.t("open the picture full size", "abrir la imagen a tamaño completo"))}>
                            <img key={big.clone()} src={big} width={u.big.0.to_string()} height={u.big.1.to_string()}
                                 alt={format!("{} — {}", u.name, if inspector { l.t("in the inspector of Cantus", "en el inspector de Cantus") } else { l.t("its window in Cantus", "su ventana en Cantus") })} />
                        </a>
                        <figcaption>
                            <div class="ab-seg up-shade" role="group" aria-label={l.t("Theme of the picture", "Tema de la imagen")}>
                                <button type="button" aria-pressed={(shown == "dark").to_string()} onclick={pick("dark")}>{ l.t("Dark theme", "Tema oscuro") }</button>
                                <button type="button" aria-pressed={(shown == "light").to_string()} onclick={pick("light")}>{ l.t("Light theme", "Tema claro") }</button>
                            </div>
                            <span>{ if inspector {
                                l.t("Its view in the program's inspector, drawn headless: it has no window of its own. Click it to open it full size.", "Su vista en el inspector del programa, dibujada sin pantalla: no tiene ventana propia. Haz clic para abrirla a tamaño completo.")
                            } else {
                                l.t("The program's own window, drawn headless. Click it to open it full size.", "La ventana del propio programa, dibujada sin pantalla. Haz clic para abrirla a tamaño completo.")
                            } }</span>
                        </figcaption>
                    </figure>
                    <div class="up-body">
                        <div class="up-main">
                            <p class="up-pitch">{ l.t(u.pitch.0, u.pitch.1) }</p>
                            <p class="up-what">{ l.t(u.what.0, u.what.1) }</p>
                            if let Some((file, song)) = u.ab {
                                <div class="up-ab">
                                    <h4>{ l.t("Hear it", "Escúchalo") }</h4>
                                    <AbPlayer key={file} file={file} song={song} />
                                </div>
                            }
                            <PresetSounds key={u.file} unit={u.file} />
                        </div>
                        <div class="up-side">
                            <h4>{ l.t("What stands out", "Lo que destaca") }</h4>
                            <ul class="up-feats">{ for feats }</ul>
                            <ul class="up-meta">
                                if let Some(p) = presets { <li>{ p }</li> }
                                if u.plugin {
                                    <li>{ l.t("Also a CLAP and VST3 plug-in for other DAWs, with the same face", "También plug-in CLAP y VST3 para otros DAW, con la misma cara") }</li>
                                } else {
                                    <li>{ l.t("Every change one undo step, saved with the song", "Cada cambio, un paso de deshacer, guardado con la canción") }</li>
                                }
                                <li>{ l.t("Free, inside Cantus", "Gratis, dentro de Cantus") }</li>
                            </ul>
                        </div>
                    </div>
                </div>
            </div>
        }
    });

    html! {
        <section class="section units-wrap" id="units">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Instruments and effects of its own", "Instrumentos y efectos propios") }</h2>
                    <p>{ l.t("Nine instruments and twenty-four effects come with Cantus, free — twenty-three with a face of their own, and Delay, edited in the inspector; each researched against the best on the market, measured, and drawn in vectors in light and dark themes. Open any of them to see it large.",
                             "Nueve instrumentos y veinticuatro efectos vienen con Cantus, gratis — veintitrés con cara propia, y Delay, que se edita en el inspector; cada uno estudiado frente a lo mejor del mercado, medido y dibujado en vectores, en tema claro y oscuro. Abre cualquiera para verlo en grande.") }</p>
                </header>
                <div class="tabs" role="tablist" aria-label={l.t("Units", "Unidades")}>{ for buttons }</div>
                <ul class={classes!("unit-grid", (*tab == 0).then_some("is-inst"))} id="unit-grid">{ for cards }</ul>
                <p class="units-foot">{ l.t("Plus Firmus, the simple synth every new track starts with, a Delay and a Reverb — and any CLAP or VST3 plug-in you own.",
                                            "Además Firmus, el sintetizador sencillo con el que empieza cada pista, un Delay y una Reverb — y cualquier plug-in CLAP o VST3 que tengas.") }</p>
            </div>
            { page.unwrap_or_default() }
        </section>
    }
}

/// Tab and Shift+Tab go round the open page's buttons and links, never behind it.
fn trap(e: &web_sys::KeyboardEvent) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(page) = doc.get_element_by_id("unit-page") else {
        return;
    };
    let Ok(all) = page.query_selector_all("button, a[href]") else {
        return;
    };
    let n = all.length();
    if n == 0 {
        return;
    }
    let first = all.item(0);
    let last = all.item(n - 1);
    let active = doc.active_element().map(web_sys::Node::from);
    let inside = active.as_ref().is_some_and(|a| page.contains(Some(a)));
    let to = if e.shift_key() {
        (!inside || active == first).then_some(last)
    } else {
        (!inside || active == last).then_some(first)
    };
    if let Some(Some(t)) = to {
        if let Ok(h) = t.dyn_into::<web_sys::HtmlElement>() {
            e.prevent_default();
            let _ = h.focus();
        }
    }
}
