//! Every unit's own face: the instruments and the effects, each with its colour, material,
//! controls and the display that tells what it does (the program's own windows).

use yew::prelude::*;

use crate::lang::use_lang;

struct Unit {
    file: &'static str,
    name: &'static str,
    kind: (&'static str, &'static str),
}

const fn u(file: &'static str, name: &'static str, en: &'static str, es: &'static str) -> Unit {
    Unit {
        file,
        name,
        kind: (en, es),
    }
}

const INSTRUMENTS: [Unit; 9] = [
    u(
        "inst-wavetable",
        "Unda",
        "Wavetable synth",
        "Sintetizador wavetable",
    ),
    u(
        "inst-analog",
        "Calor",
        "Analog modelling",
        "Modelado analógico",
    ),
    u("inst-fm", "Modulus", "8-operator FM", "FM de 8 operadores"),
    u("inst-sampler", "Exemplar", "Multisampler", "Multisampler"),
    u(
        "inst-bass",
        "Gravis",
        "808, synth and string bass",
        "Bajo 808, de sinte y de cuerda",
    ),
    u(
        "inst-organ",
        "Organum",
        "Tonewheel organ",
        "Órgano de ruedas fónicas",
    ),
    u("inst-epiano", "Lamina", "Electric piano", "Piano eléctrico"),
    u(
        "inst-chorda",
        "Chorda",
        "Modelled grand piano",
        "Piano de cola modelado",
    ),
    u("inst-pulsus", "Pulsus II", "Drums", "Batería"),
];

const EFFECTS: [Unit; 23] = [
    u("fx-eq", "Aequalis", "EQ", "Ecualizador"),
    u("fx-comp", "Comprimo", "Compressor", "Compresor"),
    u("fx-limiter", "Terminus", "Limiter", "Limitador"),
    u("fx-maximizer", "Culmen", "Maximizer", "Maximizador"),
    u("fx-multiband", "Multiplex", "Multiband", "Multibanda"),
    u("fx-deesser", "Sibilus", "De-esser", "De-esser"),
    u("fx-gate", "Porta", "Gate", "Gate"),
    u("fx-tonsor", "Tonsor", "Clipper", "Clipper"),
    u("fx-impetus", "Impetus", "Transients", "Transitorios"),
    u(
        "fx-discrimen",
        "Discrimen",
        "Transient split",
        "Separador de transitorios",
    ),
    u("fx-space", "Spatium", "Reverb", "Reverb"),
    u("fx-saturator", "Fervor", "Saturation", "Saturación"),
    u("fx-stereo", "Latitudo", "Stereo width", "Anchura estéreo"),
    u("fx-speculum", "Speculum", "Analyser", "Analizador"),
    u(
        "fx-tonus",
        "Tonus",
        "Pitch correction",
        "Corrección de afinación",
    ),
    u("fx-concentus", "Concentus", "Chorus", "Chorus"),
    u("fx-flexus", "Flexus", "Flanger", "Flanger"),
    u("fx-phasis", "Phasis", "Phaser", "Phaser"),
    u("fx-tremor", "Tremor", "Tremolo", "Trémolo"),
    u("fx-vibrans", "Vibrans", "Vibrato", "Vibrato"),
    u(
        "fx-anulus",
        "Anulus",
        "Ring modulator",
        "Modulador en anillo",
    ),
    u(
        "fx-translatio",
        "Translatio",
        "Frequency shifter",
        "Desplazador de frecuencia",
    ),
    u(
        "fx-fluctus",
        "Fluctus",
        "Volume shaper",
        "Modelador de volumen",
    ),
];

#[function_component(Faces)]
pub fn faces() -> Html {
    let l = use_lang().lang;
    let tab = use_state(|| 0usize);
    let tabs = [
        (l.t("Instruments", "Instrumentos"), &INSTRUMENTS[..]),
        (l.t("Effects", "Efectos"), &EFFECTS[..]),
    ];
    let buttons = tabs.iter().enumerate().map(|(i, (name, _))| {
        let tab = tab.clone();
        html! {
            <button type="button" role="tab" class="tab" aria-selected={(i == *tab).to_string()}
                    onclick={Callback::from(move |_| tab.set(i))}>{ *name }</button>
        }
    });
    let cards = tabs[*tab].1.iter().map(|u| {
        html! {
            <li>
                <figure class="face">
                    <img src={format!("faces/{}.webp", u.file)} width="640" height="400" loading="lazy" alt={u.name} />
                    <figcaption><strong>{ u.name }</strong><span>{ l.t(u.kind.0, u.kind.1) }</span></figcaption>
                </figure>
            </li>
        }
    });
    html! {
        <section class="section faces-wrap" id="faces">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("Each one with a face of its own", "Cada uno con su propia cara") }</h2>
                    <p>{ l.t("Every instrument and effect has its colour, its material, its controls and the display that tells what it does — drawn in vectors, crisp at any zoom, in light and dark themes. The CLAP and VST3 versions wear the same face in any DAW; one switch brings back the plain look.",
                             "Cada instrumento y efecto tiene su color, su material, sus controles y la pantalla que cuenta lo que hace —dibujados en vectores, nítidos a cualquier zoom, en temas claro y oscuro—. Las versiones CLAP y VST3 llevan la misma cara en cualquier DAW; un interruptor devuelve el aspecto sencillo.") }</p>
                </header>
                <div class="tabs" role="tablist" aria-label={l.t("Units", "Unidades")}>{ for buttons }</div>
                <ul class="faces">{ for cards }</ul>
            </div>
        </section>
    }
}
