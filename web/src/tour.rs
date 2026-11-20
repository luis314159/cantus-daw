//! The product tour: four views of the real program, and the feature list. The arrangement is
//! a crop of the program picture the hero shows (`picture.rs`, *Neon Tide*); the piano roll,
//! the channel rack and the mixer are pictures of their own, of a second song, *Sunday Strut*
//! (round 41: `cargo run --release -p cantus-ui --example shoot_tour -- <dir> night [es]`, then
//! `python tools/shots/place.py <dir> tour`).

use yew::prelude::*;

use crate::icons;
use crate::lang::{use_lang, Lang};

/// One tab's picture and the part of it the tour shows, in the picture's pixels.
struct Crop {
    /// The picture, in the page's language (its interface is in it).
    pic: fn(Lang) -> &'static str,
    /// Its size in pixels.
    size: (u32, u32),
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// The tour's pictures of *Sunday Strut* (`shoot_tour`): the program picture's width, 760 px
/// tall. Each shows the view between the browser and the inspector, its tabs to the status bar.
const TOUR_SIZE: (u32, u32) = (1912, 760);

const ARRANGEMENT: Crop = Crop {
    pic: crate::picture::program,
    size: crate::picture::SIZE,
    x: 277,
    y: 92,
    w: 1340,
    h: 410,
};
const PIANO_ROLL: Crop = Crop {
    pic: |l| {
        l.t(
            "/program/tour-pianoroll.webp",
            "/program/tour-pianoroll-es.webp",
        )
    },
    size: TOUR_SIZE,
    x: 278,
    y: 75,
    w: 1343,
    h: 658,
};
const RACK: Crop = Crop {
    pic: |l| l.t("/program/tour-rack.webp", "/program/tour-rack-es.webp"),
    size: TOUR_SIZE,
    x: 278,
    y: 75,
    w: 1343,
    h: 658,
};
const MIXER: Crop = Crop {
    pic: |l| l.t("/program/tour-mixer.webp", "/program/tour-mixer-es.webp"),
    size: TOUR_SIZE,
    x: 278,
    y: 75,
    w: 1343,
    h: 658,
};

#[function_component(Tour)]
pub fn tour() -> Html {
    let l = use_lang().lang;
    let tab = use_state(|| 0usize);
    let tabs = [
        (l.t("Arrange", "Arreglo"), &ARRANGEMENT, l.t("Cantus — arrangement", "Cantus — arreglo"),
         l.t("<b>Clips on a timeline you can read.</b> Colour-coded tracks, a snapping grid and sections you can name — Intro, Build, Drop — so the shape of the song is always in view.",
             "<b>Clips en una línea de tiempo que se lee.</b> Pistas con color, cuadrícula con imán y secciones con nombre — Intro, Build, Drop — para ver siempre la forma de la canción.")),
        (l.t("Piano roll", "Piano roll"), &PIANO_ROLL, l.t("Cantus — piano roll", "Cantus — piano roll"),
         l.t("<b>Melodies, note by note.</b> Each note's velocity stands in the lane beneath it: drag a stem to play it softer or harder. Scales, the other clips' notes as ghosts, and lanes for pitch bend, pressure and any CC.",
             "<b>Melodías, nota a nota.</b> La velocidad de cada nota está en el carril de abajo: arrastra una barra para tocarla más suave o más fuerte. Escalas, las notas de los otros clips como fantasmas y carriles para pitch bend, presión y cualquier CC.")),
        (l.t("Channel rack", "Rack de canales"), &RACK, l.t("Cantus — channel rack", "Cantus — rack de canales"),
         l.t("<b>Program a groove in steps, the FL Studio way.</b> Every track is a row and every step is a note — the same notes the piano roll edits, so switching views converts nothing.",
             "<b>Programa un ritmo por pasos, al estilo de FL Studio.</b> Cada pista es una fila y cada paso es una nota — las mismas notas que edita el piano roll, así que cambiar de vista no convierte nada.")),
        (l.t("Mix", "Mezcla"), &MIXER, l.t("Cantus — mixer", "Cantus — mezclador"),
         l.t("<b>A mixer in the spirit of FL Studio's.</b> Faders, pan, sends, inserts and groups on every strip, in three widths, with meters that never leave the screen.",
             "<b>Un mezclador al espíritu del de FL Studio.</b> Faders, paneo, envíos, inserts y grupos en cada canal, en tres anchos, con medidores siempre a la vista.")),
    ];
    let (_, crop, title, caption) = tabs[*tab];
    let style = format!(
        "--pic:url({});--sw:{};--sh:{};--cx:{};--cy:{};--cw:{};--ch:{}",
        (crop.pic)(l),
        crop.size.0,
        crop.size.1,
        crop.x,
        crop.y,
        crop.w,
        crop.h
    );
    let buttons = tabs.iter().enumerate().map(|(i, (name, ..))| {
        let tab = tab.clone();
        html! {
            <button type="button" role="tab" class="tab" aria-selected={(i == *tab).to_string()}
                    onclick={Callback::from(move |_| tab.set(i))}>{ *name }</button>
        }
    });

    html! {
        <section class="section showcase" id="product">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("A workspace drawn by hand", "Un espacio de trabajo dibujado a mano") }</h2>
                    <p>{ l.t("Every knob, fader and meter is painted by Cantus's own vector kit, so it stays crisp at any zoom. This is the real program.",
                             "Cada perilla, fader y medidor lo pinta el kit vectorial propio de Cantus, y se ve nítido a cualquier zoom. Este es el programa real.") }</p>
                </header>
                <div class="tabs" role="tablist" aria-label={l.t("Views", "Vistas")}>{ for buttons }</div>
                <div class="stage">
                    <div class="backdrop"><div class="window">
                        <div class="window-bar"><i></i><i></i><i></i><span>{ title }</span></div>
                        <div class={classes!("crop", (*tab == 0).then_some("is-arrange"))} role="img" aria-label={title} {style}></div></div>
                    </div>
                    <p class="caption">{ Html::from_html_unchecked(AttrValue::from(caption)) }</p>
                </div>
            </div>
        </section>
    }
}

#[function_component(Features)]
pub fn features() -> Html {
    let l = use_lang().lang;
    let items = [
        (icons::timeline(), l.t("Arrangement, piano roll, channel rack", "Arreglo, piano roll, channel rack"),
         l.t("Draw patterns, program drums, write melodies and lay it all out on a timeline. Dock, split and resize every panel.",
             "Dibuja patrones, programa batería, escribe melodías y ordénalo todo en la línea de tiempo. Acopla, divide y redimensiona cada panel.")),
        (icons::mixer(), l.t("A real mixer", "Un mezclador de verdad"),
         l.t("Sends, groups, submixes, VCA faders, snapshots and a master strip with a chain, in the FL Studio tradition.",
             "Envíos, grupos, submezclas, faders VCA, snapshots y un máster con cadena, en la tradición de FL Studio.")),
        (icons::wave(), l.t("Instruments and effects of its own", "Instrumentos y efectos propios"),
         crate::counts::features_units(l)),
        (icons::grid(), l.t("A session view, in sync", "Una vista de sesión, sincronizada"),
         l.t("Launch clips and scenes from a grid, a Launchpad or an APC; share the tempo over Ableton Link or MIDI clock; play and edit MPE note by note.",
             "Lanza clips y escenas desde una rejilla, un Launchpad o un APC; comparte el tempo por Ableton Link o reloj MIDI; toca y edita MPE nota a nota.")),
        (icons::spark(), l.t("AI that works with you", "IA que trabaja contigo"),
         l.t("Let Claude or another assistant work in Cantus through MCP, each change one undo step; separate stems, turn audio into MIDI and master with an assistant that says why.",
             "Deja que Claude u otro asistente trabaje en Cantus por MCP, cada cambio un paso de deshacer; separa stems, convierte audio en MIDI y masteriza con un asistente que dice por qué.")),
        (icons::plug(), l.t("Works with the rest of your studio", "Convive con el resto de tu estudio"),
         l.t("Hosts CLAP, VST3 and ARA 2 plug-ins; your outboard synths and effects, on time after one ping; a Mackie Control or HUI on the mixer; its own effects as CLAP and VST3; MIDI files, stems and DAWproject in and out.",
             "Aloja plug-ins CLAP, VST3 y ARA 2; tus sintes y efectos externos, a tiempo tras un ping; un Mackie Control o HUI en el mezclador; sus propios efectos como CLAP y VST3; archivos MIDI, stems y DAWproject de ida y vuelta.")),
        (icons::mic(), l.t("Record at a low buffer", "Graba con un búfer bajo"),
         l.t("The recording mode bakes the song so only your take is live. ASIO and WASAPI on Windows, PipeWire, JACK and ALSA on Linux; projects recover after a crash.",
             "El modo de grabación pre-renderiza la canción para que solo tu toma esté en vivo. ASIO y WASAPI en Windows, PipeWire, JACK y ALSA en Linux; los proyectos se recuperan tras un cierre inesperado.")),
        (icons::export(), l.t("Export it, finished", "Expórtala, terminada"),
         l.t("WAV, FLAC, MP3 and Ogg, with loudness presets for the places you will publish.",
             "WAV, FLAC, MP3 y Ogg, con preajustes de volumen para los lugares donde la vas a publicar.")),
    ];
    let rows = items.into_iter().map(|(icon, title, text)| {
        html! { <li><span class="ico">{ icon }</span><div><h3>{ title }</h3><p>{ text }</p></div></li> }
    });
    html! {
        <section class="section features-wrap" id="features">
            <div class="wrap features">
                <div class="features-head">
                    <h2>{ l.t("From first idea to last bounce", "De la primera idea al último render") }</h2>
                    <p>{ l.t("Cantus is a beta: the whole path from a blank project to an exported song works today, and it is where we spend our time.",
                             "Cantus está en beta: el camino completo, de un proyecto en blanco a una canción exportada, funciona hoy, y ahí ponemos nuestro tiempo.") }</p>
                </div>
                <ul class="flist">{ for rows }</ul>
            </div>
        </section>
    }
}
