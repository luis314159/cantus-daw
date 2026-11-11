//! How many instruments and effects the page says Cantus has: counted from the cards'
//! data (`units_data.rs`), never typed into the copy (round 39: the copy said twenty-four
//! effects beside a tab that counted twenty-five). Each sentence is made once per language
//! and kept, so the copy stays `&'static str` like every other string on the page.

use std::sync::OnceLock;

use crate::lang::Lang;
use crate::units_data::{EFFECTS, INSTRUMENTS};

/// The instruments with a card, and Firmus, the simple synth every new track starts with,
/// which has none.
pub fn instruments_with_firmus() -> usize {
    INSTRUMENTS.len() + 1
}

/// A count in words, as the copy reads it, before a masculine noun in Spanish
/// ("veintiún efectos"); figures past thirty.
pub fn words(n: usize, l: Lang) -> String {
    const EN: [&str; 31] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
        "twenty-one",
        "twenty-two",
        "twenty-three",
        "twenty-four",
        "twenty-five",
        "twenty-six",
        "twenty-seven",
        "twenty-eight",
        "twenty-nine",
        "thirty",
    ];
    const ES: [&str; 31] = [
        "cero",
        "un",
        "dos",
        "tres",
        "cuatro",
        "cinco",
        "seis",
        "siete",
        "ocho",
        "nueve",
        "diez",
        "once",
        "doce",
        "trece",
        "catorce",
        "quince",
        "dieciséis",
        "diecisiete",
        "dieciocho",
        "diecinueve",
        "veinte",
        "veintiún",
        "veintidós",
        "veintitrés",
        "veinticuatro",
        "veinticinco",
        "veintiséis",
        "veintisiete",
        "veintiocho",
        "veintinueve",
        "treinta",
    ];
    let table = match l {
        Lang::En => &EN,
        Lang::Es => &ES,
    };
    table
        .get(n)
        .map_or_else(|| n.to_string(), |w| w.to_string())
}

/// The same, starting a sentence.
fn cap(s: String) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn once(cell: &'static OnceLock<[String; 2]>, l: Lang, make: fn(Lang) -> String) -> &'static str {
    let pair = cell.get_or_init(|| [make(Lang::En), make(Lang::Es)]);
    match l {
        Lang::En => &pair[0],
        Lang::Es => &pair[1],
    }
}

/// The instruments section's lede.
pub fn units_lede(l: Lang) -> &'static str {
    static CELL: OnceLock<[String; 2]> = OnceLock::new();
    once(&CELL, l, |l| {
        let (i, e) = (cap(words(INSTRUMENTS.len(), l)), words(EFFECTS.len(), l));
        // Round 39: every effect has a window of its own (Delay's came last, ADR 0115).
        match l {
            Lang::En => format!("{i} instruments and {e} effects come with Cantus, free, each with a face of its own; each researched against the best on the market, measured, and drawn in vectors in light and dark themes. Open any of them to see it large."),
            Lang::Es => format!("{i} instrumentos y {e} efectos vienen con Cantus, gratis, cada uno con cara propia; cada uno estudiado frente a lo mejor del mercado, medido y dibujado en vectores, en tema claro y oscuro. Abre cualquiera para verlo en grande."),
        }
    })
}

/// The features list's line on the units.
pub fn features_units(l: Lang) -> &'static str {
    static CELL: OnceLock<[String; 2]> = OnceLock::new();
    once(&CELL, l, |l| {
        let e = words(EFFECTS.len(), l);
        match l {
            Lang::En => format!("Wavetable, analog, FM, sampler, bass, organ, electric piano, a modelled grand piano and drums; {e} effects from EQ and compressor to chorus, flanger, clipper and analyser — each with a face of its own."),
            Lang::Es => format!("Wavetable, analógico, FM, sampler, bajo, órgano, piano eléctrico, un piano de cola modelado y batería; {e} efectos, del EQ y el compresor al chorus, el flanger, el clipper y el analizador, cada uno con su propia cara."),
        }
    })
}

/// The roadmap's "works today" line on the units.
pub fn ahead_units(l: Lang) -> &'static str {
    static CELL: OnceLock<[String; 2]> = OnceLock::new();
    once(&CELL, l, |l| {
        let (i, e) = (
            cap(words(instruments_with_firmus(), l)),
            words(EFFECTS.len(), l),
        );
        match l {
            Lang::En => format!(
                "{i} instruments and {e} effects of its own, a modelled grand piano among them"
            ),
            Lang::Es => format!(
                "{i} instrumentos y {e} efectos propios, entre ellos un piano de cola modelado"
            ),
        }
    })
}

/// The menu's line under Instruments (round 39).
pub fn nav_instruments(l: Lang) -> &'static str {
    static CELL: OnceLock<[String; 2]> = OnceLock::new();
    once(&CELL, l, |l| {
        let i = cap(words(INSTRUMENTS.len(), l));
        match l {
            Lang::En => format!("{i} of its own, each heard"),
            Lang::Es => format!("{i} propios, cada uno se escucha"),
        }
    })
}

/// The menu's line under Effects.
pub fn nav_effects(l: Lang) -> &'static str {
    static CELL: OnceLock<[String; 2]> = OnceLock::new();
    once(&CELL, l, |l| {
        let e = cap(words(EFFECTS.len(), l));
        let ab = words(EFFECTS.iter().filter(|u| u.ab.is_some()).count(), l);
        match l {
            Lang::En => format!("{e}; {ab} heard with and without"),
            Lang::Es => format!("{e}; {ab} se escuchan con y sin"),
        }
    })
}

/// The hero's lede: what Cantus is, with its counts.
pub fn hero_lede(l: Lang) -> &'static str {
    static CELL: OnceLock<[String; 2]> = OnceLock::new();
    once(&CELL, l, |l| {
        let (i, e) = (words(INSTRUMENTS.len(), l), words(EFFECTS.len(), l));
        match l {
            Lang::En => format!("A complete DAW — arrange, record, mix and master — with {i} instruments and {e} effects of its own. No account, no subscription, no telemetry."),
            Lang::Es => format!("Un DAW completo — arregla, graba, mezcla y masteriza — con {i} instrumentos y {e} efectos propios. Sin cuenta, sin suscripción, sin telemetría."),
        }
    })
}

/// The spotlight's headline, three lines: (instruments, effects, the close).
pub fn spotlight_head(l: Lang) -> (&'static str, &'static str) {
    static CELL: OnceLock<[String; 2]> = OnceLock::new();
    static CELL2: OnceLock<[String; 2]> = OnceLock::new();
    let a = once(&CELL, l, |l| {
        let i = cap(words(INSTRUMENTS.len(), l));
        match l {
            Lang::En => format!("{i} instruments."),
            Lang::Es => format!("{i} instrumentos."),
        }
    });
    let b = once(&CELL2, l, |l| {
        let e = cap(words(EFFECTS.len(), l));
        match l {
            Lang::En => format!("{e} effects."),
            Lang::Es => format!("{e} efectos."),
        }
    });
    (a, b)
}
