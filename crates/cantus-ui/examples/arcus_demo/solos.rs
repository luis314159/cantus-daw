//! r42 arcus-expr — the solo showcase: one line for every instrument (written for the violin
//! in D major, moved for the others: the viola a fifth down in G, the cello an octave and a
//! fifth down, the bass two octaves and a fifth down), built and exported as the quartet is,
//! plain and performed.
//!
//! Two sections: a *cantabile* (a pickup up-bow, long slurred bows, two slides on shifts, a
//! messa di voce, a leading tone, a dying close) and an *articulated* passage (a spiccato
//! scale, a sforzando into a slur, martelé, portato inside a slur, an fp held, a diminuendo
//! to the end).

use super::build::{self, Piece, Style};
use super::measure;
use super::perform::Role;
use super::score::{Part, Section, Who};
use std::path::Path;

pub const BPM: f64 = 72.0;

pub const SECTIONS: [Section; 2] = [
    Section {
        bar: 1,
        bars: 8,
        name: "Cantabile",
        what: "Una frase cantabile: anacrusa arco arriba, arcos largos ligados, dos portamentos en cambios de posición, messa di voce y un final que se apaga.",
    },
    Section {
        bar: 9,
        bars: 7,
        name: "Articulado",
        what: "Spiccato, sforzando, martelé, portato dentro de una ligadura, fp sostenido y diminuendo final.",
    },
];

const CANTABILE: &str = "r:3 A4:1 | (D5:2 E5:1 F#5:1) | (G5:1.5 F#5:0.5 E5:1 D5:1) | \
     (C#5:2 A5:2^) , | (A5:3 G5:0.5 F#5:0.5) | (E5:1 B5:2^ A5:1) | (G5:1 F#5:1 E5:1 C#5:1) | \
     D5:3.5 r:0.5";
const CANTABILE_CC1: &[(f64, f32)] = &[
    (0.0, 0.36),
    (6.0, 0.5),
    (14.0, 0.44),
    (18.0, 0.56),
    (24.0, 0.66),
    (29.0, 0.5),
    (32.0, 0.3),
];
const ARTICULATED: &str = "A4:0.5i B4:0.5i C#5:0.5i D5:0.5i E5:0.5i F#5:0.5i G5:0.5i A5:0.5i | \
     B5:1z (A5:1 G5:1 F#5:1) | E5:1m E5:1m A4:1m A4:1m | \
     (D5:0.5 E5:0.5- F#5:0.5- G5:0.5-) (A5:1 F#5:1) | D5:4f | (E5:2 C#5:2) | D5:3.5 r:0.5";
const ARTICULATED_CC1: &[(f64, f32)] = &[
    (0.0, 0.5),
    (4.0, 0.62),
    (8.0, 0.6),
    (12.0, 0.55),
    (16.0, 0.42),
    (20.0, 0.5),
    (24.0, 0.48),
    (28.0, 0.3),
];

macro_rules! solo_parts {
    ($who:expr) => {
        &[
            Part {
                who: $who,
                section: 0,
                vel: 64,
                notes: CANTABILE,
                cc1: CANTABILE_CC1,
            },
            Part {
                who: $who,
                section: 1,
                vel: 64,
                notes: ARTICULATED,
                cc1: ARTICULATED_CC1,
            },
        ]
    };
}

fn transpose(who: Who) -> i32 {
    match who {
        Who::Violin => 0,
        Who::Viola => -7,
        Who::Cello => -19,
        Who::Bass => -31,
    }
}

fn role(_: Who, _: usize) -> Role {
    Role::Melody
}

const TEMPO: &[(u32, f64, bool)] = &[(1, BPM, false), (14, BPM, true), (16, 60.0, false)];

macro_rules! solo {
    ($title:expr, $who:expr, $whos:expr) => {
        Piece {
            title: $title,
            bpm: BPM,
            tempo: TEMPO,
            sections: &SECTIONS,
            parts: solo_parts!($who),
            who: $whos,
            transpose,
            role,
            climax: None,
            intimate: None,
            solo: true,
        }
    };
}

pub const SOLOS: [(Who, &str, Piece); 4] = [
    (
        Who::Violin,
        "violin",
        solo!("Arcus · violín solo", Who::Violin, &[Who::Violin]),
    ),
    (
        Who::Viola,
        "viola",
        solo!("Arcus · viola sola", Who::Viola, &[Who::Viola]),
    ),
    (
        Who::Cello,
        "violonchelo",
        solo!("Arcus · violonchelo solo", Who::Cello, &[Who::Cello]),
    ),
    (
        Who::Bass,
        "contrabajo",
        solo!("Arcus · contrabajo solo", Who::Bass, &[Who::Bass]),
    ),
];

/// Every solo, plain and performed, into `out`: `<name>-plano.wav`, `<name>-interpretado.wav`
/// (the export's, 48 kHz 24-bit at −16 LUFS) and the project of each. Checks each: a second
/// export the same to the byte, under −1 dBTP with no full-scale sample, no click at a note's
/// boundary. Returns whether all pass.
pub fn render(out: &Path) -> bool {
    std::fs::create_dir_all(out).expect("the solos' folder");
    let mut ok = true;
    for (who, name, piece) in &SOLOS {
        for (style, tag) in [(Style::Plain, "plano"), (Style::Performed, "interpretado")] {
            let data = tempfile::tempdir().expect("a data folder");
            let (mut app, _proc) =
                super::app_on(data.path(), cantus_core::Project::new(piece.title));
            let built = build::build(&mut app, piece, style);
            let stem = format!("{name}-{tag}");
            app.save_to_path(&out.join(format!("{stem}.cantus")));
            let wav = out.join(format!("{stem}.wav"));
            let said = super::export(&mut app, &wav, false);
            let again = out.join(format!(".{stem}-again.wav"));
            super::export(&mut app, &again, false);
            let same = measure::same_bytes(&wav, &again);
            let _ = std::fs::remove_file(&again);
            let a = measure::read_wav(&wav);
            let lv = measure::level(&a);
            let x = measure::mono(&a);
            let p = app.project();
            let tm = p.tempo_map();
            let (track, _) = built.tracks[who];
            let mut at: Vec<(f64, &'static str)> = Vec::new();
            for c in &p.track(track).expect("the track").clips {
                for n in &c.notes {
                    at.push((tm.seconds_at((c.start + n.start).0 as f64), "start"));
                    at.push((
                        tm.seconds_at((c.start + n.start + n.length).0 as f64),
                        "end",
                    ));
                }
            }
            let edges = measure::edges(&x, a.rate, &at);
            let worst = edges
                .iter()
                .map(|e| e.over_db)
                .fold(f64::NEG_INFINITY, f64::max);
            let pass = same && lv.dbtp <= -0.9 && lv.full_scale == 0 && worst < 15.0;
            ok &= pass;
            println!(
                "{stem}: {said}; {:.1} LUFS, {:.2} dBTP, the same bytes again {same}, the worst \
                 boundary {worst:.1} dB over its music ({} measured) — {}",
                lv.lufs.unwrap_or(-200.0),
                lv.dbtp,
                edges.len(),
                if pass { "pass" } else { "FAIL" }
            );
        }
    }
    ok
}
