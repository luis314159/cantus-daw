//! Hear the difference: the effects with an excerpt of a demo song with the effect off and
//! on, switched without a gap — and the player the unit pages use for the same excerpts.

use gloo_timers::callback::Interval;
use web_sys::HtmlAudioElement;
use yew::prelude::*;

use crate::lang::use_lang;
use crate::units::{pic_lang, shade_now, Unit};
use crate::units_data::EFFECTS;

/// The two excerpts of one effect, playing together; only one is audible.
struct Pair {
    without: HtmlAudioElement,
    with: HtmlAudioElement,
}

fn make(file: &str, kind: &str) -> Option<HtmlAudioElement> {
    let a = HtmlAudioElement::new_with_src(&format!("fx/audio/{file}-{kind}.mp3")).ok()?;
    a.set_loop(true);
    Some(a)
}

#[derive(Properties, PartialEq)]
pub struct AbProps {
    /// The excerpts' name in `assets/fx/audio` (`eq` → `eq-with.mp3`, `eq-without.mp3`).
    pub file: AttrValue,
    /// The demo song they come from.
    pub song: AttrValue,
}

/// Play, *Without* / *With*, a progress bar and the song's name. Give it a `key` per effect:
/// a new effect starts silent, and leaving stops it.
#[function_component(AbPlayer)]
pub fn ab_player(p: &AbProps) -> Html {
    let l = use_lang().lang;
    let playing = use_state(|| false);
    let with = use_state(|| false);
    let pos = use_state(|| 0.0f64);
    let pair = use_mut_ref(|| None::<Pair>);

    // A playing excerpt moves the little progress bar.
    {
        let pair = pair.clone();
        let pos = pos.clone();
        use_effect_with(*playing, move |on| {
            let timer = on.then(|| {
                Interval::new(120, move || {
                    if let Some(p) = pair.borrow().as_ref() {
                        let len = p.with.duration();
                        if len.is_finite() && len > 0.0 {
                            pos.set(p.with.current_time() / len);
                        }
                    }
                })
            });
            move || drop(timer)
        });
    }
    // Leaving silences it.
    {
        let pair = pair.clone();
        use_effect_with((), move |_| {
            move || {
                if let Some(p) = pair.borrow_mut().take() {
                    let _ = p.without.pause();
                    let _ = p.with.pause();
                }
            }
        });
    }

    let toggle_play = {
        let (pair, playing, with, pos) = (pair.clone(), playing.clone(), with.clone(), pos.clone());
        let file = p.file.clone();
        Callback::from(move |_| {
            if *playing {
                if let Some(p) = pair.borrow_mut().take() {
                    let _ = p.without.pause();
                    let _ = p.with.pause();
                }
                playing.set(false);
                pos.set(0.0);
                return;
            }
            let (Some(without), Some(with_a)) = (make(&file, "without"), make(&file, "with"))
            else {
                return;
            };
            let loud = *with;
            without.set_volume(if loud { 0.0 } else { 1.0 });
            with_a.set_volume(if loud { 1.0 } else { 0.0 });
            let _ = without.play();
            let _ = with_a.play();
            *pair.borrow_mut() = Some(Pair {
                without,
                with: with_a,
            });
            playing.set(true);
        })
    };

    // Switch which excerpt is heard; both keep running, so the music does not move.
    let choose = |want_with: bool| {
        let (pair, with) = (pair.clone(), with.clone());
        Callback::from(move |_| {
            if let Some(p) = pair.borrow().as_ref() {
                let (from, to) = if want_with {
                    (&p.without, &p.with)
                } else {
                    (&p.with, &p.without)
                };
                to.set_current_time(from.current_time());
                to.set_volume(1.0);
                from.set_volume(0.0);
            }
            with.set(want_with);
        })
    };

    let icon = if *playing {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3" width="3" height="10" rx="1" fill="currentColor"/><rect x="9.5" y="3" width="3" height="10" rx="1" fill="currentColor"/></svg> }
    } else {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 2.5v11l9-5.5z" fill="currentColor"/></svg> }
    };
    html! {
        <div class="ab" role="group" aria-label={l.t("Hear it with and without the effect", "Escúchalo con y sin el efecto")}>
            <button type="button" class={classes!("ab-play", playing.then_some("is-on"))} onclick={toggle_play}
                    aria-label={if *playing { l.t("Stop", "Detener") } else { l.t("Play", "Reproducir") }}>{ icon }</button>
            <div class="ab-seg">
                <button type="button" aria-pressed={(!*with).to_string()} onclick={choose(false)}>{ l.t("Without", "Sin") }</button>
                <button type="button" aria-pressed={with.to_string()} onclick={choose(true)}>{ l.t("With", "Con") }</button>
            </div>
            <div class="ab-bar" aria-hidden="true"><i style={format!("width:{:.1}%", *pos * 100.0)}></i></div>
            <span class="ab-note">{ format!("{} {}", l.t("An excerpt of the demo song", "Un fragmento de la canción de demostración"), p.song) }</span>
        </div>
    }
}

/// The effects that have an excerpt.
fn heard() -> Vec<&'static Unit> {
    EFFECTS.iter().filter(|u| u.ab.is_some()).collect()
}

#[function_component(Effects)]
pub fn effects() -> Html {
    let l = use_lang().lang;
    let sel = use_state(|| 0usize);
    let list = heard();
    let fx = list[(*sel).min(list.len() - 1)];
    let (file, song) = fx.ab.unwrap_or_default();

    let items = list.iter().enumerate().map(|(i, e)| {
        let sel = sel.clone();
        html! {
            <li>
                <button type="button" class="fx-item" aria-pressed={(i == *sel).to_string()}
                        onclick={Callback::from(move |_| sel.set(i))}>
                    <strong>{ e.name }</strong><span>{ l.t(e.kind.0, e.kind.1) }</span>
                </button>
            </li>
        }
    });
    let shade = shade_now();

    html! {
        <section class="section fx-wrap" id="effects">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Hear what they do", "Escucha lo que hacen") }</h2>
                    <p>{ l.t("The same excerpt of a demo song with every slot of one effect bypassed, then as the song ships. Switch while it plays: the music does not move, only the effect comes and goes.",
                             "El mismo fragmento de una canción de demostración con cada ranura de un efecto en bypass, y luego tal como viene la canción. Cambia mientras suena: la música no se mueve, solo entra y sale el efecto.") }</p>
                </header>
                <div class="fx">
                    <ul class="fx-list">{ for items }</ul>
                    <figure class="fx-stage">
                        <div class="fx-window">
                            <img src={format!("units/{}-{}{}.webp", fx.file, shade, pic_lang(l))} width={fx.big.0.to_string()} height={fx.big.1.to_string()}
                                 alt={format!("{} — {}", fx.name, l.t(fx.kind.0, fx.kind.1))} loading="lazy" />
                        </div>
                        <figcaption><strong>{ fx.name }</strong>{ " — " }{ l.t(fx.pitch.0, fx.pitch.1) }</figcaption>
                        <AbPlayer key={file} file={file} song={song} />
                    </figure>
                </div>
            </div>
        </section>
    }
}
