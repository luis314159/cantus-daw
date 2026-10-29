//! The effects gallery: one real window per effect, what it does in a line, and the same
//! excerpt of a demo song with the effect off and on, switched without a gap.

use gloo_timers::callback::Interval;
use web_sys::HtmlAudioElement;
use yew::prelude::*;

use crate::lang::use_lang;

struct Fx {
    file: &'static str,
    name: &'static str,
    kind: (&'static str, &'static str),
    line: (&'static str, &'static str),
    size: (u32, u32),
    /// The demo song the A/B excerpt comes from (the one where the effect is most audible).
    song: &'static str,
}

const EFFECTS: [Fx; 7] = [
    Fx { file: "eq", name: "Aequalis", kind: ("EQ", "Ecualizador"), size: (1180, 640), song: "Midnight Static",
         line: ("24 bands, nine shapes, dynamic bands and three phase modes. The display is the control.",
                "24 bandas, nueve formas, bandas dinámicas y tres modos de fase. La pantalla es el control.") },
    Fx { file: "comp", name: "Comprimo", kind: ("Compressor", "Compresor"), size: (1260, 780), song: "Paper Lanterns",
         line: ("A compressor you can read: a history of what it did, the transfer curve and an EQ for its key, with measured auto gain.",
                "Un compresor que se lee: el historial de lo que hizo, la curva de transferencia y un EQ para su señal de control, con ganancia automática medida.") },
    Fx { file: "deesser", name: "Sibilus", kind: ("De-esser", "De-esser"), size: (1000, 640), song: "Midnight Static",
         line: ("Hears only the band where sibilance lives, and for a single voice can tell sibilance from everything else.",
                "Escucha solo la banda donde vive la sibilancia y, en una sola voz, distingue la sibilancia de todo lo demás.") },
    Fx { file: "gate", name: "Porta", kind: ("Gate", "Gate"), size: (1000, 640), song: "Midnight Static",
         line: ("Gate, expander and ducker around one detector. Drag the thresholds on the level history.",
                "Gate, expansor y ducker alrededor de un solo detector. Arrastra los umbrales sobre el historial de nivel.") },
    Fx { file: "limiter", name: "Terminus", kind: ("Limiter", "Limitador"), size: (1000, 640), song: "Midnight Static",
         line: ("A mastering limiter with measured true-peak, BS.1770 loudness and delivery profiles that tell you if you pass.",
                "Un limitador de masterización con true-peak medido, volumen BS.1770 y perfiles de entrega que te dicen si cumples.") },
    Fx { file: "multiband", name: "Multiplex", kind: ("Multiband", "Multibanda"), size: (1040, 640), song: "Midnight Static",
         line: ("Up to eight bands, each with dynamics above and below: hold the peaks down and lift the quiet parts at once.",
                "Hasta ocho bandas, cada una con dinámica por arriba y por abajo: baja los picos y sube lo suave a la vez.") },
    Fx { file: "space", name: "Spatium", kind: ("Reverb", "Reverb"), size: (1040, 640), song: "Aurora",
         line: ("Draw the decay as a curve in seconds. Four styles — Room, Plate, Vintage, Spring — and a freeze.",
                "Dibuja el decaimiento como una curva en segundos. Cuatro estilos — Room, Plate, Vintage, Spring — y un freeze.") },
];

/// The two excerpts of the selected effect, playing together; only one is audible.
struct Pair {
    index: usize,
    without: HtmlAudioElement,
    with: HtmlAudioElement,
}

fn make(file: &str, kind: &str) -> Option<HtmlAudioElement> {
    let a = HtmlAudioElement::new_with_src(&format!("fx/audio/{file}-{kind}.mp3")).ok()?;
    a.set_loop(true);
    Some(a)
}

#[function_component(Effects)]
pub fn effects() -> Html {
    let l = use_lang().lang;
    let sel = use_state(|| 0usize);
    let playing = use_state(|| false);
    let with = use_state(|| false);
    let pos = use_state(|| 0.0f64);
    let pair = use_mut_ref(|| None::<Pair>);
    let fx = &EFFECTS[*sel];

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

    let stop = {
        let (pair, playing, pos) = (pair.clone(), playing.clone(), pos.clone());
        move || {
            if let Some(p) = pair.borrow_mut().take() {
                let _ = p.without.pause();
                let _ = p.with.pause();
            }
            playing.set(false);
            pos.set(0.0);
        }
    };

    let items = EFFECTS.iter().enumerate().map(|(i, e)| {
        let (sel, stop) = (sel.clone(), stop.clone());
        html! {
            <li>
                <button type="button" class="fx-item" aria-pressed={(i == *sel).to_string()}
                        onclick={Callback::from(move |_| { if *sel != i { stop(); sel.set(i); } })}>
                    <strong>{ e.name }</strong><span>{ l.t(e.kind.0, e.kind.1) }</span>
                </button>
            </li>
        }
    });

    let toggle_play = {
        let (pair, playing, with, sel, stop) = (
            pair.clone(),
            playing.clone(),
            with.clone(),
            sel.clone(),
            stop.clone(),
        );
        Callback::from(move |_| {
            if *playing {
                stop();
                return;
            }
            let file = EFFECTS[*sel].file;
            let (Some(without), Some(with_a)) = (make(file, "without"), make(file, "with")) else {
                return;
            };
            let loud = *with;
            without.set_volume(if loud { 0.0 } else { 1.0 });
            with_a.set_volume(if loud { 1.0 } else { 0.0 });
            let _ = without.play();
            let _ = with_a.play();
            *pair.borrow_mut() = Some(Pair {
                index: *sel,
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

    let _ = |p: &Pair| p.index;
    let song_line = if fx.song.is_empty() {
        String::new()
    } else {
        format!("{} {}", l.t("An excerpt of", "Un fragmento de"), fx.song)
    };
    let icon = if *playing {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3" width="3" height="10" rx="1" fill="currentColor"/><rect x="9.5" y="3" width="3" height="10" rx="1" fill="currentColor"/></svg> }
    } else {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 2.5v11l9-5.5z" fill="currentColor"/></svg> }
    };

    html! {
        <section class="section fx-wrap" id="effects">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Effects with a window of their own", "Efectos con una ventana propia") }</h2>
                    <p>{ l.t("Each effect shows what it is doing, on the audio that goes through it. Seven of the twelve that ship with Cantus; saturation, a maximizer, stereo control, a delay and a reverb complete the set.",
                             "Cada efecto muestra lo que hace, sobre el audio que pasa por él. Siete de los doce que vienen con Cantus; saturación, un maximizador, control estéreo, un delay y una reverb completan el conjunto.") }</p>
                </header>
                <div class="fx">
                    <ul class="fx-list">{ for items }</ul>
                    <figure class="fx-stage">
                        <div class="fx-window">
                            <img src={format!("fx/{}.png", fx.file)} width={fx.size.0.to_string()} height={fx.size.1.to_string()}
                                 alt={format!("{} — {}", fx.name, l.t(fx.kind.0, fx.kind.1))} loading="lazy" />
                        </div>
                        <figcaption><strong>{ fx.name }</strong>{ " — " }{ l.t(fx.line.0, fx.line.1) }</figcaption>
                        <div class="ab" role="group" aria-label={l.t("Hear it with and without the effect", "Escúchalo con y sin el efecto")}>
                            <button type="button" class={classes!("ab-play", playing.then_some("is-on"))} onclick={toggle_play}
                                    aria-label={if *playing { l.t("Stop", "Detener") } else { l.t("Play", "Reproducir") }}>{ icon }</button>
                            <div class="ab-seg">
                                <button type="button" aria-pressed={(!*with).to_string()} onclick={choose(false)}>{ l.t("Without", "Sin") }</button>
                                <button type="button" aria-pressed={with.to_string()} onclick={choose(true)}>{ l.t("With", "Con") }</button>
                            </div>
                            <div class="ab-bar" aria-hidden="true"><i style={format!("width:{:.1}%", *pos * 100.0)}></i></div>
                            <span class="ab-note">{ song_line }</span>
                        </div>
                    </figure>
                </div>
            </div>
        </section>
    }
}
