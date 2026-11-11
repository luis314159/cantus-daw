//! A unit's presets, heard (round 37, ADR 0101): on its card a ▶ that plays its first
//! excerpt, and on its page every excerpt with its name and what it is for — an effect's
//! with the dry source to compare. The excerpts are rendered by Cantus itself
//! (`units_sounds.rs`, generated) at the presets' own levels, with no processing after.

use web_sys::HtmlAudioElement;
use yew::prelude::*;

use crate::lang::use_lang;
use crate::units_sounds::{Sound, SOUNDS, SOURCES};

/// A card's excerpts.
pub fn of(file: &str) -> &'static [Sound] {
    SOUNDS
        .iter()
        .find(|(f, _)| *f == file)
        .map(|(_, s)| *s)
        .unwrap_or(&[])
}

fn src(file: &str) -> String {
    format!("/units/sounds/{file}.mp3")
}

/// The one excerpt playing on the page, so a second play stops the first.
#[derive(Default)]
struct Playing {
    audio: Option<HtmlAudioElement>,
    file: String,
}

impl Playing {
    fn stop(&mut self) {
        if let Some(a) = self.audio.take() {
            let _ = a.pause();
        }
        self.file.clear();
    }
}

fn play_icon(on: bool) -> Html {
    if on {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="4" y="4" width="8" height="8" rx="1" fill="currentColor"/></svg> }
    } else {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5 3.5v9l7.5-4.5z" fill="currentColor"/></svg> }
    }
}

/// Play or stop `file` (a click on the one playing stops it).
fn toggle(p: &mut Playing, file: &str, now: &UseStateHandle<String>) {
    let same = p.file == file;
    p.stop();
    if same {
        now.set(String::new());
        return;
    }
    if let Ok(a) = HtmlAudioElement::new_with_src(&src(file)) {
        let _ = a.play();
        let done = now.clone();
        let ended =
            wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || done.set(String::new()));
        a.set_onended(Some(wasm_bindgen::JsCast::unchecked_ref(ended.as_ref())));
        ended.forget();
        p.audio = Some(a);
        p.file = file.to_string();
        now.set(file.to_string());
    }
}

#[derive(Properties, PartialEq)]
pub struct Props {
    /// The card's `file` (`unda`, `fervor`…).
    pub unit: AttrValue,
}

/// The page's list: every excerpt of the unit, and an effect's dry source.
#[function_component(PresetSounds)]
pub fn preset_sounds(p: &Props) -> Html {
    let l = use_lang().lang;
    let now = use_state(String::new);
    let playing = use_mut_ref(Playing::default);
    {
        let playing = playing.clone();
        use_effect_with(p.unit.clone(), move |_| move || playing.borrow_mut().stop());
    }
    let sounds = of(&p.unit);
    if sounds.is_empty() {
        return html! {};
    }
    let row = |file: String, name: String, what: String| {
        let on = *now == file;
        let click = {
            let playing = playing.clone();
            let now = now.clone();
            let file = file.clone();
            Callback::from(move |_| toggle(&mut playing.borrow_mut(), &file, &now))
        };
        html! {
            <li class={classes!("ps-row", on.then_some("is-on"))}>
                <button type="button" class={classes!("ab-play", on.then_some("is-on"))} onclick={click}
                        aria-label={format!("{} {name}", if on { l.t("Stop", "Parar") } else { l.t("Play", "Reproducir") })}
                        aria-pressed={on.to_string()}>
                    { play_icon(on) }
                </button>
                <span class="ps-name">{ name }</span>
                <span class="ps-what">{ what }</span>
            </li>
        }
    };
    let effect_source = sounds.first().map(|s| s.on).filter(|on| !on.is_empty());
    let dry = effect_source
        .and_then(|on| SOURCES.iter().find(|(r, _)| *r == on))
        .map(|(r, name)| {
            row(
                format!("source-{r}"),
                l.t("Without the effect", "Sin el efecto").to_string(),
                l.t(name.0, name.1).to_string(),
            )
        });
    html! {
        <div class="up-sounds">
            <h4>{ l.t("Hear its presets", "Escucha sus presets") }</h4>
            <ul class="ps-list">
                if let Some(d) = dry { { d } }
                { for sounds.iter().map(|s| row(s.file.to_string(), l.t(s.name.0, s.name.1).to_string(), l.t(s.what.0, s.what.1).to_string())) }
            </ul>
            <p class="ps-note">{ l.t(
                "Rendered by Cantus from its factory presets, each at its own level, with no processing after.",
                "Renderizados por Cantus desde sus presets de fábrica, cada uno a su propio nivel, sin ningún proceso después."
            ) }</p>
        </div>
    }
}

/// The card's ▶: its first excerpt, played over the page.
#[function_component(CardSound)]
pub fn card_sound(p: &Props) -> Html {
    let l = use_lang().lang;
    let now = use_state(String::new);
    let playing = use_mut_ref(Playing::default);
    {
        let playing = playing.clone();
        use_effect_with((), move |_| move || playing.borrow_mut().stop());
    }
    let Some(first) = of(&p.unit).first() else {
        return html! {};
    };
    let on = *now == first.file;
    let click = {
        let playing = playing.clone();
        let now = now.clone();
        let file = first.file.to_string();
        Callback::from(move |e: MouseEvent| {
            // The card opens its page on a click: this one only plays.
            e.stop_propagation();
            toggle(&mut playing.borrow_mut(), &file, &now)
        })
    };
    html! {
        <button type="button" class={classes!("card-play", on.then_some("is-on"))} onclick={click}
                title={format!("{} — {}", l.t(first.name.0, first.name.1), l.t(first.what.0, first.what.1))}
                aria-label={format!("{} {}", if on { l.t("Stop", "Parar") } else { l.t("Hear", "Escuchar") }, l.t(first.name.0, first.name.1))}>
            { play_icon(on) }
        </button>
    }
}
