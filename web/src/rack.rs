//! The hero's playable rack: sixteen steps, four voices, a tempo. It plays a groove the
//! moment you press the button, and every cell can be changed while it runs.

use std::collections::VecDeque;

use gloo_timers::callback::Interval;
use yew::prelude::*;

use crate::audio::Synth;
use crate::lang::use_lang;

const STEPS: usize = 16;
const ROWS: usize = 4;
type Pattern = [[bool; STEPS]; ROWS];

/// A bass line in A minor, one pitch per step (Hz); the row decides which steps sound.
const BASS_HZ: [f32; STEPS] = [
    55.0, 55.0, 65.41, 55.0, 82.41, 55.0, 73.42, 65.41, 55.0, 55.0, 65.41, 55.0, 98.0, 82.41, 73.42, 65.41,
];

fn groove() -> Pattern {
    let mut p = [[false; STEPS]; ROWS];
    for s in [0, 4, 8, 12] {
        p[0][s] = true;
    }
    for s in [4, 12] {
        p[1][s] = true;
    }
    for s in [2, 6, 10, 14, 15] {
        p[2][s] = true;
    }
    for s in [0, 3, 6, 8, 11, 14] {
        p[3][s] = true;
    }
    p
}

/// What the scheduler shares with the page.
struct Sched {
    synth: Option<Synth>,
    next_time: f64,
    next_step: usize,
    /// (audio time, step) pairs waiting for the clock to reach them, to move the light.
    lights: VecDeque<(f64, usize)>,
}

fn play_step(synth: &Synth, pat: &Pattern, step: usize, t: f64, spb: f64) {
    let _ = (|| {
        if pat[0][step] {
            synth.kick(t)?;
        }
        if pat[1][step] {
            synth.clap(t)?;
        }
        if pat[2][step] {
            synth.hat(t, step % 4 == 2 && step != 14)?;
        }
        if pat[3][step] {
            synth.bass(t, BASS_HZ[step], spb * 0.9)?;
        }
        Ok::<(), wasm_bindgen::JsValue>(())
    })();
}

#[function_component(Rack)]
pub fn rack() -> Html {
    let l = use_lang().lang;
    let pattern = use_state(groove);
    let playing = use_state(|| false);
    let now = use_state(|| None::<usize>);
    let bpm = use_state(|| 124u32);

    // The scheduler reads the live pattern and tempo without being rebuilt on every edit.
    let live = use_mut_ref(|| (groove(), 124u32));
    *live.borrow_mut() = (*pattern, *bpm);
    let sched = use_mut_ref(|| Sched { synth: None, next_time: 0.0, next_step: 0, lights: VecDeque::new() });

    {
        let sched = sched.clone();
        let live = live.clone();
        let now = now.clone();
        use_effect_with(*playing, move |on| {
            let timer = on.then(|| {
                Interval::new(25, move || {
                    let mut s = sched.borrow_mut();
                    let Some(clock) = s.synth.as_ref().map(|y| y.now()) else { return };
                    let (pat, tempo) = *live.borrow();
                    let spb = 60.0 / tempo as f64 / 4.0;
                    while s.next_time < clock + 0.12 {
                        let (t, step) = (s.next_time, s.next_step);
                        if let Some(y) = s.synth.as_ref() {
                            play_step(y, &pat, step, t, spb);
                        }
                        s.lights.push_back((t, step));
                        s.next_time += spb;
                        s.next_step = (step + 1) % STEPS;
                    }
                    let mut lit = None;
                    while s.lights.front().is_some_and(|(t, _)| *t <= clock) {
                        lit = s.lights.pop_front().map(|(_, st)| st);
                    }
                    if lit.is_some() {
                        now.set(lit);
                    }
                })
            });
            move || drop(timer)
        });
    }

    // The page can react to playback (the hero's C of waves breathes with it).
    use_effect_with(*playing, |on| {
        if let Some(body) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.body()) {
            let _ = body.class_list().toggle_with_force("is-playing", *on);
        }
    });

    let toggle_play = {
        let playing = playing.clone();
        let sched = sched.clone();
        let now = now.clone();
        Callback::from(move |_| {
            if *playing {
                let mut s = sched.borrow_mut();
                s.synth = None; // closes the audio context: silence at once
                s.lights.clear();
                now.set(None);
                playing.set(false);
            } else {
                let mut s = sched.borrow_mut();
                match Synth::new() {
                    Ok(y) => {
                        s.next_time = y.now() + 0.06;
                        s.next_step = 0;
                        s.synth = Some(y);
                        playing.set(true);
                    }
                    Err(_) => playing.set(false),
                }
            }
        })
    };

    let clear = {
        let pattern = pattern.clone();
        Callback::from(move |_| pattern.set([[false; STEPS]; ROWS]))
    };
    let reset = {
        let pattern = pattern.clone();
        Callback::from(move |_| pattern.set(groove()))
    };
    let on_bpm = {
        let bpm = bpm.clone();
        Callback::from(move |e: InputEvent| {
            use wasm_bindgen::JsCast;
            if let Some(i) = e.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                if let Ok(v) = i.value().parse::<u32>() {
                    bpm.set(v.clamp(70, 180));
                }
            }
        })
    };

    let names = [
        l.t("Kick", "Bombo"),
        l.t("Clap", "Palmas"),
        l.t("Hi-hat", "Hi-hat"),
        l.t("Bass", "Bajo"),
    ];

    let rows = names.iter().enumerate().map(|(r, name)| {
        let cells = (0..STEPS).map(|s| {
            let on = pattern[r][s];
            let lit = *now == Some(s);
            let click = {
                let pattern = pattern.clone();
                Callback::from(move |_| {
                    let mut p = *pattern;
                    p[r][s] = !p[r][s];
                    pattern.set(p);
                })
            };
            let label = format!("{} {}", name, s + 1);
            html! {
                <button type="button" class={classes!("cell", lit.then_some("now"))}
                        aria-pressed={on.to_string()} aria-label={label} onclick={click}></button>
            }
        });
        html! { <div class="row"><div class="row-name">{ *name }</div><div class="cells">{ for cells }</div></div> }
    });

    let play_label = if *playing { l.t("Stop", "Detener") } else { l.t("Play", "Reproducir") };
    let icon = if *playing {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3" y="3" width="10" height="10" rx="1.5" fill="currentColor"/></svg> }
    } else {
        html! { <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 2.5v11l9-5.5z" fill="currentColor"/></svg> }
    };

    html! {
        <div class={classes!("rack", playing.then_some("is-playing"))} id="rack">
            <div class="rack-bar">
                <div class="rack-title"><i></i>{ l.t("Channel rack — try it", "Channel rack — pruébalo") }</div>
                <label class="tempo">
                    <span>{ l.t("Tempo", "Tempo") }</span>
                    <input type="range" min="70" max="180" value={bpm.to_string()} oninput={on_bpm}
                           aria-label={l.t("Tempo in BPM", "Tempo en BPM")} />
                    <output>{ format!("{} BPM", *bpm) }</output>
                </label>
                <button type="button" class="rack-btn" onclick={clear}>{ l.t("Clear", "Vaciar") }</button>
                <button type="button" class="rack-btn" onclick={reset}>{ l.t("Groove", "Ritmo") }</button>
                <button type="button" class="rack-btn play" onclick={toggle_play}>{ icon }{ play_label }</button>
            </div>
            <div class="rack-grid">{ for rows }</div>
            <p class="rack-foot">
                { l.t("Click any cell. The sound is made in your browser, right now — nothing is downloaded.",
                      "Haz clic en cualquier celda. El sonido se genera en tu navegador, ahora mismo — no se descarga nada.") }
            </p>
        </div>
    }
}
