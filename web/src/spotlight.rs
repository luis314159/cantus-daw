//! The instruments and effects, one at a time (round 39, Mock 2): the chosen unit's window
//! large, what it is and what stands out, its presets playable under it, the shelf of the
//! others, and a link to its own page (`/instruments/<name>`, `/effects/<name>`).

use gloo_timers::callback::Interval;
use web_sys::HtmlAudioElement;
use yew::prelude::*;

use crate::icons;
use crate::lang::use_lang;
use crate::nav::href;
use crate::units::{pic_lang, shade_now, Unit};
use crate::units_data::{EFFECTS, INSTRUMENTS};

/// The group's units and its path: 0 instruments, 1 effects.
pub fn group(g: usize) -> (&'static [Unit], &'static str) {
    if g == 0 {
        (&INSTRUMENTS[..], "/instruments/")
    } else {
        (&EFFECTS[..], "/effects/")
    }
}

/// One playing excerpt for a whole component: a second play stops the first.
#[derive(Clone, PartialEq, Default)]
pub struct Now {
    pub file: String,
    pub frac: f64,
}

/// A list of excerpts with one player: what plays now, and play-or-stop by file.
#[hook]
pub fn use_excerpts() -> (UseStateHandle<Now>, Callback<String>) {
    let now = use_state(Now::default);
    let audio = use_mut_ref(|| None::<HtmlAudioElement>);
    // The file playing as of now (an interval keeps the value of its own render otherwise).
    let cur = use_mut_ref(String::new);
    {
        let (audio, now, cur) = (audio.clone(), now.clone(), cur.clone());
        let on = !now.file.is_empty();
        use_effect_with(on, move |on| {
            let timer = on.then(|| {
                Interval::new(150, move || {
                    if let Some(a) = audio.borrow().as_ref() {
                        let len = a.duration();
                        let frac = if len.is_finite() && len > 0.0 {
                            (a.current_time() / len).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        if a.ended() {
                            cur.borrow_mut().clear();
                        }
                        now.set(Now {
                            file: cur.borrow().clone(),
                            frac,
                        });
                    }
                })
            });
            move || drop(timer)
        });
    }
    {
        let audio = audio.clone();
        use_effect_with((), move |_| {
            move || {
                if let Some(a) = audio.borrow_mut().take() {
                    let _ = a.pause();
                }
            }
        });
    }
    let toggle = {
        let now = now.clone();
        Callback::from(move |file: String| {
            if let Some(a) = audio.borrow_mut().take() {
                let _ = a.pause();
            }
            let same = *cur.borrow() == file;
            cur.borrow_mut().clear();
            if same {
                now.set(Now::default());
                return;
            }
            if let Ok(a) = HtmlAudioElement::new_with_src(&format!("/units/sounds/{file}.mp3")) {
                let _ = a.play();
                *audio.borrow_mut() = Some(a);
                *cur.borrow_mut() = file.clone();
                now.set(Now { file, frac: 0.0 });
            }
        })
    };
    (now, toggle)
}

#[function_component(Spotlight)]
pub fn spotlight() -> Html {
    let l = use_lang().lang;
    let tab = use_state(|| 0usize);
    let pick = use_state(|| 0usize);
    let (now, toggle) = use_excerpts();
    let (units, base) = group(*tab);
    let u = &units[(*pick).min(units.len() - 1)];
    let (head_i, head_e) = crate::counts::spotlight_head(l);

    let tabs = [
        (l.t("Instruments", "Instrumentos"), INSTRUMENTS.len()),
        (l.t("Effects", "Efectos"), EFFECTS.len()),
    ];
    let tab_buttons = tabs.iter().enumerate().map(|(i, (name, n))| {
        let (tab, pick, toggle, now) = (tab.clone(), pick.clone(), toggle.clone(), now.clone());
        html! {
            <button type="button" role="tab" class="tab" aria-selected={(i == *tab).to_string()} aria-controls="spot-panel"
                    onclick={Callback::from(move |_| {
                        if !now.file.is_empty() { toggle.emit(now.file.clone()); }
                        tab.set(i);
                        pick.set(0);
                    })}>
                { *name }<span class="tab-n">{ *n }</span>
            </button>
        }
    });

    let feats = crate::unit_text::features(u, l)
        .into_iter()
        .take(4)
        .map(|f| html! { <li>{ f }</li> });
    let presets = crate::sounds::of(u.file).iter().take(4).map(|s| {
        let on = now.file == s.file;
        let file = s.file.to_string();
        let toggle = toggle.clone();
        let name = l.t(s.name.0, s.name.1);
        html! {
            <li class={classes!("spot-preset", on.then_some("is-on"))}>
                <button type="button" onclick={Callback::from(move |_| toggle.emit(file.clone()))} aria-pressed={on.to_string()}>
                    <span class="spot-play">{ if on { icons::pause() } else { icons::play() } }</span>
                    <span class="spot-name"><b>{ name }</b><small>{ l.t(s.what.0, s.what.1) }</small></span>
                </button>
                <span class="spot-bar" role="progressbar" aria-label={format!("{name}, {}", l.t("played", "reproducido"))}
                      aria-valuemin="0" aria-valuemax="100" aria-valuenow={format!("{:.0}", if on { now.frac * 100.0 } else { 0.0 })}>
                    <i style={format!("width:{:.1}%", if on { now.frac * 100.0 } else { 0.0 })}></i>
                </span>
            </li>
        }
    });
    let shelf = units.iter().enumerate().map(|(i, s)| {
        let (pick, toggle, now) = (pick.clone(), toggle.clone(), now.clone());
        let chosen = std::ptr::eq(s, u);
        html! {
            <li>
                <button type="button" class={classes!("shelf-item", chosen.then_some("is-on"))} aria-pressed={chosen.to_string()}
                        onclick={Callback::from(move |_| {
                            if !now.file.is_empty() { toggle.emit(now.file.clone()); }
                            pick.set(i);
                        })}>
                    <img src={format!("/units/{}-thumb{}.webp", s.file, pic_lang(l))} width={s.thumb.0.to_string()} height={s.thumb.1.to_string()} loading="lazy" alt="" />
                    <span>{ s.name }</span>
                </button>
            </li>
        }
    });
    let big = format!("/units/{}-{}{}.webp", u.file, shade_now(), pic_lang(l));
    let page = href(l, &format!("{base}{}", u.file));
    html! {
        <section class="section spot-wrap units-wrap" id="units">
            <div class="wrap">
                <div class="spot-head">
                    <h2>{ head_i }<br />{ head_e }<br /><span class="dim">{ l.t("All its own, all free.", "Todos propios, todos gratis.") }</span></h2>
                    <div class="tabs" role="tablist" aria-label={l.t("Units", "Unidades")}>{ for tab_buttons }</div>
                </div>
                <div class="spot" id="spot-panel">
                    <div class="spot-main">
                        <a class="spot-pic" href={page.clone()}>
                            <img key={big.clone()} src={big} width={u.big.0.to_string()} height={u.big.1.to_string()} loading="lazy"
                                 alt={format!("{} — {}", u.name, l.t("its window in Cantus", "su ventana en Cantus"))} />
                        </a>
                        <div class="spot-text">
                            <span class="eyebrow">{ l.t(u.kind.0, u.kind.1) }</span>
                            <h3>{ u.name }</h3>
                            <p class="spot-pitch">{ l.t(u.pitch.0, u.pitch.1) }</p>
                            <ul class="spot-feats">{ for feats }</ul>
                            <a class="btn btn-quiet" href={page}>{ format!("{} {} →", l.t("Everything about", "Todo sobre"), u.name) }</a>
                        </div>
                    </div>
                    <ul class="spot-presets">{ for presets }</ul>
                </div>
                <ul class="spot-shelf" aria-label={tabs[*tab].0}>{ for shelf }</ul>
                <p class="units-foot">{ l.t("Every preset above is rendered by Cantus itself, with no processing after; the pictures are the program's own windows. Plus Firmus, the simple synth every new track starts with, a plain Reverb, and any CLAP or VST3 plug-in you own.",
                                            "Cada preset de arriba lo renderiza Cantus mismo, sin ningún proceso después; las imágenes son las ventanas del propio programa. Además Firmus, el sintetizador sencillo con el que empieza cada pista, una Reverb sencilla y cualquier plug-in CLAP o VST3 que tengas.") }</p>
            </div>
        </section>
    }
}
