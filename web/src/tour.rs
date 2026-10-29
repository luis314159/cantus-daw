//! The product tour: three crops of a real screenshot of Cantus, and the feature list.

use yew::prelude::*;

use crate::icons;
use crate::lang::use_lang;

/// One crop of `app-mixer-xl.png` (1912×1022), in source pixels.
struct Crop {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

const ARRANGEMENT: Crop = Crop {
    x: 277,
    y: 92,
    w: 1340,
    h: 410,
};
const MIXER: Crop = Crop {
    x: 277,
    y: 515,
    w: 1340,
    h: 475,
};
const INSPECTOR: Crop = Crop {
    x: 1622,
    y: 96,
    w: 290,
    h: 700,
};

#[function_component(Tour)]
pub fn tour() -> Html {
    let l = use_lang().lang;
    let tab = use_state(|| 0usize);
    let tabs = [
        (l.t("Arrange", "Arreglo"), &ARRANGEMENT, l.t("Cantus — arrangement", "Cantus — arreglo"),
         l.t("<b>Clips on a timeline you can read.</b> Colour-coded tracks, a snapping grid and sections you can name — Intro, Build, Drop — so the shape of the song is always in view.",
             "<b>Clips en una línea de tiempo que se lee.</b> Pistas con color, cuadrícula con imán y secciones con nombre — Intro, Build, Drop — para ver siempre la forma de la canción.")),
        (l.t("Mix", "Mezcla"), &MIXER, l.t("Cantus — mixer", "Cantus — mezclador"),
         l.t("<b>A mixer in the spirit of FL Studio's.</b> Faders, pan, sends, inserts and groups on every strip, in three widths, with meters that never leave the screen.",
             "<b>Un mezclador al espíritu del de FL Studio.</b> Faders, paneo, envíos, inserts y grupos en cada canal, en tres anchos, con medidores siempre a la vista.")),
        (l.t("Shape", "Moldea"), &INSPECTOR, l.t("Cantus — inspector", "Cantus — inspector"),
         l.t("<b>Everything about what you selected, in one place.</b> The inspector follows your selection: track, pad, inserts and the channel sheet, with clear labels and units on every value.",
             "<b>Todo lo que seleccionaste, en un solo lugar.</b> El inspector sigue tu selección: pista, pad, inserts y la hoja del canal, con etiquetas y unidades claras en cada valor.")),
    ];
    let (_, crop, title, caption) = tabs[*tab];
    let style = format!(
        "--sw:1912;--sh:1022;--cx:{};--cy:{};--cw:{};--ch:{}",
        crop.x, crop.y, crop.w, crop.h
    );
    let buttons = tabs.iter().enumerate().map(|(i, (name, ..))| {
        let tab = tab.clone();
        html! {
            <button type="button" role="tab" class="tab" aria-selected={(i == *tab).to_string()}
                    onclick={Callback::from(move |_| tab.set(i))}>{ *name }</button>
        }
    });
    let narrow = (*tab == 2).then_some("max-width:22rem");

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
                    <div class="backdrop"><div class="window" style={narrow}>
                        <div class="window-bar"><i></i><i></i><i></i><span>{ title }</span></div>
                        <div class="crop" role="img" aria-label={title} {style}></div></div>
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
         l.t("Sends, groups, submixes and a master strip with a chain, in the FL Studio tradition.",
             "Envíos, grupos, submezclas y un máster con cadena, en la tradición de FL Studio.")),
        (icons::wave(), l.t("Instruments and effects of its own", "Instrumentos y efectos propios"),
         l.t("Wavetable, analog, FM, sampler, bass, organ, electric piano, a modelled grand piano and drums; twenty-five effects from EQ and compressor to chorus, flanger, clipper and analyser — twenty-three of them with a face of their own.",
             "Wavetable, analógico, FM, sampler, bajo, órgano, piano eléctrico, un piano de cola modelado y batería; veinticinco efectos, del EQ y el compresor al chorus, el flanger, el clipper y el analizador, veintitrés de ellos con su propia cara.")),
        (icons::plug(), l.t("Works with the rest of your studio", "Convive con el resto de tu estudio"),
         l.t("Hosts CLAP and VST3 plug-ins, exports its own effects as CLAP and VST3, and reads and writes MIDI files, stems and DAWproject.",
             "Aloja plug-ins CLAP y VST3, exporta sus efectos como CLAP y VST3, y lee y escribe archivos MIDI, stems y DAWproject.")),
        (icons::mic(), l.t("Record, with ASIO or WASAPI", "Graba, con ASIO o WASAPI"),
         l.t("One build, every audio driver. Monitor while you record and keep going if the machine hiccups: projects recover after a crash.",
             "Un solo programa, todos los drivers de audio. Monitorea mientras grabas y sigue si algo falla: los proyectos se recuperan tras un cierre inesperado.")),
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
