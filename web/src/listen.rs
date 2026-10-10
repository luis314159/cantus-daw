//! "Hear what it makes": excerpts of the six demo songs that ship with Cantus, rendered by
//! the program itself (`cargo run --release -p cantus-ui --example render_demos`).

use gloo_timers::callback::Interval;
use web_sys::HtmlAudioElement;
use yew::prelude::*;

use crate::lang::{use_lang, Lang};

struct Song {
    id: &'static str,
    title: &'static str,
    style: (&'static str, &'static str),
}

const SONGS: [Song; 6] = [
    Song { id: "house", title: "Neon Tide", style: ("House", "House") },
    Song { id: "ballad", title: "Morning Light", style: ("Pop ballad", "Balada pop") },
    Song { id: "funk", title: "Sunday Strut", style: ("Funk and soul", "Funk y soul") },
    Song { id: "lofi", title: "Paper Lanterns", style: ("Lo-fi hip-hop", "Hip-hop lo-fi") },
    Song { id: "ambient", title: "Aurora", style: ("Cinematic", "Cinemática") },
    Song { id: "trap", title: "Midnight Static", style: ("Trap", "Trap") },
];

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[function_component(Listen)]
pub fn listen() -> Html {
    let l: Lang = use_lang().lang;
    let current = use_state(|| None::<usize>);
    let playing = use_state(|| false);
    let progress = use_state(|| (0.0f64, 0.0f64)); // (position, length) in seconds
    let audio = use_mut_ref(|| None::<HtmlAudioElement>);

    // While something plays, follow it ten times a second.
    {
        let audio = audio.clone();
        let playing_h = playing.clone();
        let progress = progress.clone();
        use_effect_with(*playing, move |on| {
            let timer = on.then(|| {
                Interval::new(100, move || {
                    if let Some(a) = audio.borrow().as_ref() {
                        let len = a.duration();
                        progress.set((a.current_time(), if len.is_finite() { len } else { 0.0 }));
                        if a.ended() {
                            playing_h.set(false);
                        }
                    }
                })
            });
            move || drop(timer)
        });
    }
    // Leaving the page (or this component) silences it.
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

    let rows = SONGS.iter().enumerate().map(|(i, song)| {
        let is_cur = *current == Some(i);
        let is_playing = is_cur && *playing;
        let (pos, len) = if is_cur { *progress } else { (0.0, 0.0) };
        let frac = if len > 0.0 { (pos / len * 100.0).clamp(0.0, 100.0) } else { 0.0 };
        let click = {
            let (audio, current, playing, progress) = (audio.clone(), current.clone(), playing.clone(), progress.clone());
            Callback::from(move |_| {
                let same = *current == Some(i);
                if same {
                    if let Some(a) = audio.borrow().as_ref() {
                        if *playing {
                            let _ = a.pause();
                            playing.set(false);
                        } else {
                            if a.ended() {
                                a.set_current_time(0.0);
                            }
                            let _ = a.play();
                            playing.set(true);
                        }
                    }
                    return;
                }
                if let Some(old) = audio.borrow_mut().take() {
                    let _ = old.pause();
                }
                if let Ok(a) = HtmlAudioElement::new_with_src(&format!("demos/{}.mp3", SONGS[i].id)) {
                    let _ = a.play();
                    *audio.borrow_mut() = Some(a);
                    progress.set((0.0, 0.0));
                    current.set(Some(i));
                    playing.set(true);
                }
            })
        };
        let label = format!("{}: {}", if is_playing { l.t("Pause", "Pausar") } else { l.t("Play", "Reproducir") }, song.title);
        let icon = if is_playing {
            html! { <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3" width="3" height="10" rx="1" fill="currentColor"/><rect x="9.5" y="3" width="3" height="10" rx="1" fill="currentColor"/></svg> }
        } else {
            html! { <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 2.5v11l9-5.5z" fill="currentColor"/></svg> }
        };
        html! {
            <li class={classes!("song", is_cur.then_some("is-current"), is_playing.then_some("is-playing"))}>
                <button type="button" class="song-btn" onclick={click} aria-label={label}>{ icon }</button>
                <div class="song-name"><strong>{ song.title }</strong><span>{ l.t(song.style.0, song.style.1) }</span></div>
                <div class="song-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={format!("{frac:.0}")}>
                    <i style={format!("width:{frac:.1}%")}></i>
                </div>
                <span class="song-time">{ if is_cur && len > 0.0 { format!("{} / {}", clock(pos), clock(len)) } else { String::from("\u{2013}") } }</span>
            </li>
        }
    });

    html! {
        <section class="night listen" id="listen">
            <div class="wrap">
                <div class="sec-head">
                    <h2>{ l.t("Hear what it makes", "Escucha lo que hace") }</h2>
                    <p class="lede">{ l.t("Excerpts of the six demo songs that come with Cantus. Every sound is one of its own instruments and effects, mastered on the song's own master channel.",
                                          "Fragmentos de las seis canciones de demostración que vienen con Cantus. Cada sonido es de sus propios instrumentos y efectos, masterizado en el canal master de la canción.") }</p>
                </div>
                <ol class="songs">{ for rows }</ol>
            </div>
        </section>
    }
}
