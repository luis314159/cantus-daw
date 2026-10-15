//! The notes: the chord charts, the topline, and what every pitched part plays in each
//! section (the drums are in `drums.rs`).

use super::{drums, Band, FINAL};
use crate::demos::parts::{arp, bars, bass, comp, hold, lay, repeat, swell, Chart, Laid, B};
use crate::demos::song::{section as name_of, Song};
use crate::demos::theory::{melody, shift, transpose, Ev};

pub const VERSE: Chart = &[
    ("Fm9", 4.0),
    ("Dbmaj7", 4.0),
    ("Abadd9", 4.0),
    ("Eb", 2.0),
    ("Eb/G", 2.0),
];
pub const PRE: Chart = &[
    ("Bbm7", 4.0),
    ("Ab/C", 4.0),
    ("Dbmaj7", 4.0),
    ("Eb7sus4", 2.0),
    ("Eb", 2.0),
];
pub const CHORUS: Chart = &[
    ("Dbmaj7", 4.0),
    ("Eb", 4.0),
    ("Fm7", 4.0),
    ("Ab/C", 4.0),
    ("Dbmaj7", 4.0),
    ("Eb", 4.0),
    ("Cm7", 4.0),
    ("Fm7", 2.0),
    ("Eb/G", 2.0),
];
pub const BREAK: Chart = &[
    ("Dbmaj9", 4.0),
    ("Ab/C", 4.0),
    ("Bbm7", 4.0),
    ("Eb7sus4", 4.0),
];
pub const OUTRO: Chart = &[
    ("Fm9", 4.0),
    ("Dbmaj7", 4.0),
    ("Abadd9", 4.0),
    ("Eb", 4.0),
    ("Fm9", 4.0),
    ("Dbmaj9", 4.0),
    ("Abadd9", 8.0),
];

/// The chart a section plays (looped over its length).
pub fn chart(key: &str) -> Chart<'static> {
    match key {
        "intro" | "verse" => VERSE,
        "prechorus" => PRE,
        "chorus" => CHORUS,
        "break" => BREAK,
        _ => OUTRO,
    }
}

// ---- the topline (F minor; every note in the scale Tonus tunes to) ---------------------

const VERSE_A: &str = "r:1 C5:.5 C5:.5 Eb5:.75 C5:.75 Ab4:.5 | F4:1 r:1 Ab4:.5 C5:.5 Db5:.5 C5:.5 \
     | C5:1.5 Bb4:.5 Ab4:1 Bb4:.5 C5:.5 | Bb4:2.5 r:1.5";
const VERSE_B: &str = "r:1 C5:.5 C5:.5 Eb5:.75 F5:.75 Eb5:.5 | Db5:1 C5:1 Ab4:.5 Bb4:.5 C5:.5 Db5:.5 \
     | Eb5:1.5 Db5:.5 C5:1 Ab4:1 | Bb4:3 r:1";
const PRE_A: &str = "Db5:1 C5:.5 Bb4:.5 Db5:1 C5:.5 Bb4:.5 | C5:1 Ab4:.5 Ab4:.5 C5:1 Eb5:1 \
     | F5:1 Eb5:.5 Db5:.5 F5:1 Eb5:.5 Db5:.5 | Eb5:3 r:1";
const PRE_B: &str = "Db5:1 C5:.5 Bb4:.5 Db5:1 C5:.5 Bb4:.5 | C5:1 Ab4:.5 Ab4:.5 C5:1 Eb5:1 \
     | F5:1 Eb5:.5 Db5:.5 F5:1 Eb5:.5 Db5:.5 | Eb5:1 F5:1 G5:2";
/// The hook: four bars over Db · Eb · Fm7 · Ab/C, its answer over Db · Eb · Cm7 · Fm7.
pub const HOOK_A: &str = "r:.5 Ab4:.5 C5:.5 Db5:1 C5:.5 Ab4:1 | Bb4:.5 C5:.5 Eb5:1.5 Db5:.5 C5:1 \
     | C5:.5 Ab4:.5 F5:1.5 Eb5:.5 C5:1 | Eb5:1.5 Db5:.5 C5:2";
const HOOK_B: &str = "r:.5 Ab4:.5 C5:.5 Db5:1 C5:.5 Ab4:1 | Bb4:.5 C5:.5 Eb5:1.5 F5:.5 G5:1 \
     | G5:1.5 F5:.5 Eb5:1 C5:1 | F5:2 r:1 C5:.5 Eb5:.5";
const HOOK_END: &str = "r:.5 Ab4:.5 C5:.5 Db5:1 C5:.5 Ab4:1 | Bb4:.5 C5:.5 Eb5:1.5 F5:.5 G5:1 \
     | G5:1.5 F5:.5 Eb5:1 C5:1 | F5:3.5 r:.5";
/// The breakdown's tune, on the piano in octaves.
const BREAK_TUNE: &str = "Ab5:1.5 F5:.5 Eb5:1 C5:1 | Eb5:1.5 C5:.5 Bb4:1 Ab4:1 \
     | Db5:1.5 F5:.5 Eb5:1 Db5:1 | Bb4:2 Ab4:1 Bb4:1 \
     | Ab5:1.5 F5:.5 Eb5:1 C5:1 | Eb5:1.5 C5:.5 Bb4:1 Ab4:1 \
     | Db5:1.5 F5:.5 Eb5:1 Db5:1 | Bb4:2 C5:1 Eb5:1";

/// The chorus's sixteen bars of topline.
pub fn hook() -> Vec<Ev> {
    let mut v = melody(HOOK_A, 96, 0.92);
    v.extend(shift(&melody(HOOK_B, 98, 0.92), 16.0));
    v.extend(shift(&melody(HOOK_A, 98, 0.92), 32.0));
    v.extend(shift(&melody(HOOK_END, 100, 0.95), 48.0));
    v
}

/// `laid`'s events repeated over `bars` bars.
fn over(evs: &[Ev], laid: &Laid, bars: f64) -> Vec<Ev> {
    repeat(evs, laid.beats, ((bars * 4.0) / laid.beats).round() as usize)
}

pub fn section(s: &mut Song, b: &Band, key: &str, at: f64, len: f64) {
    drums::section(s, b, key, at, len);
    let name = name_of(key);
    let chart = chart(key);
    let last = at >= FINAL;
    let beats = len * 4.0;

    // ---- Chorda ------------------------------------------------------------------------
    let rh = lay(chart, 4, 58, 77, 67);
    let piano: Vec<Ev> = match key {
        "intro" => {
            let a = over(&arp(&rh, 36, &[0, 1, 2, 3, 4, 3, 2, 1], 0.5, 1.2, 54), &rh, len);
            swell(&a, beats, 0.85, 1.1)
        }
        "verse" => {
            let mut rhythm = vec![(0.0, 1.9, 70u8), (2.5, 1.4, 58)];
            let mut v = Vec::new();
            for half in 0..2 {
                if half == 1 {
                    rhythm.extend([(1.5, 0.5, 50), (3.5, 0.45, 50)]);
                }
                let mut h = comp(&rh, 4.0, &rhythm);
                h.extend(bass(
                    &rh,
                    36,
                    4.0,
                    &[(0.0, 1.9, 74, B::R), (0.0, 1.9, 62, B::O), (2.0, 1.9, 62, B::O)],
                ));
                v.extend(shift(&repeat(&h, rh.beats, 2), half as f64 * 32.0));
            }
            v
        }
        "prechorus" => {
            let mut h = comp(
                &rh,
                4.0,
                &[
                    (0.0, 0.45, 72),
                    (0.5, 0.45, 54),
                    (1.0, 0.45, 62),
                    (1.5, 0.45, 54),
                    (2.0, 0.45, 68),
                    (2.5, 0.45, 54),
                    (3.0, 0.45, 62),
                    (3.5, 0.45, 56),
                ],
            );
            h.extend(bass(&rh, 36, 1.0, &[(0.0, 0.9, 72, B::R)]));
            swell(&over(&h, &rh, len), beats, 0.8, 1.15)
        }
        "chorus" => {
            let rh = lay(chart, 4, 60, 79, 70);
            let mut h = comp(
                &rh,
                4.0,
                &[(0.0, 1.4, 86), (1.5, 0.9, 74), (2.5, 1.4, 82), (3.5, 0.45, 68)],
            );
            h.extend(bass(
                &rh,
                36,
                4.0,
                &[
                    (0.0, 1.4, 84, B::R),
                    (0.0, 1.4, 72, B::O),
                    (1.5, 0.9, 70, B::O),
                    (2.5, 1.4, 80, B::R),
                    (2.5, 1.4, 68, B::O),
                ],
            ));
            over(&h, &rh, len)
        }
        "break" => {
            let lh = lay(chart, 3, 50, 65, 57);
            let mut v = over(&hold(&lh, 56, 0.98), &lh, len);
            v.extend(over(&bass(&lh, 36, 4.0, &[(0.0, 3.9, 64, B::R)]), &lh, len));
            let tune = melody(BREAK_TUNE, 84, 0.95);
            v.extend(transpose(&tune, -12).iter().map(|e| Ev { vel: e.vel - 16, ..*e }));
            v.extend(tune);
            v
        }
        _ => {
            // The outro: the intro's arpeggio, softer, then the last chord rolled and held.
            let mut v: Vec<Ev> = arp(&rh, 36, &[0, 1, 2, 3, 4, 3, 2, 1], 0.5, 1.2, 52)
                .into_iter()
                .filter(|e| e.at < 24.0)
                .collect();
            v = swell(&v, 24.0, 1.0, 0.8);
            let (_, _, c, voicing) = rh.at(24.0);
            v.push(Ev::new(24.0, 7.9, c.bass_at(36), 60));
            for (i, p) in voicing.iter().enumerate() {
                v.push(Ev::new(24.08 + i as f64 * 0.06, 7.8, *p, 54));
            }
            v
        }
    };
    s.clip(b.piano, &name, at, len, &piano);

    // ---- Gravis --------------------------------------------------------------------------
    let low = lay(chart, 3, 50, 65, 57);
    let line: Vec<Ev> = match key {
        "verse" => {
            let mut v = over(&bass(&low, 29, 2.0, &[(0.0, 1.9, 92, B::R)]), &low, 8.0);
            let walk = bass(
                &low,
                29,
                4.0,
                &[(0.0, 1.9, 96, B::R), (2.0, 1.4, 86, B::R), (3.5, 0.45, 84, B::A)],
            );
            v.extend(shift(&over(&walk, &low, 8.0), 32.0));
            v
        }
        "prechorus" => {
            let eighths: Vec<(f64, f64, u8, B)> = (0..8)
                .map(|i| (i as f64 * 0.5, 0.45, if i % 2 == 0 { 96 } else { 80 }, B::R))
                .collect();
            let v = over(&bass(&low, 29, 4.0, &eighths), &low, len);
            v.into_iter().filter(|e| e.at < beats - 1.0).collect()
        }
        "chorus" => over(
            &bass(
                &low,
                29,
                4.0,
                &[
                    (0.0, 0.45, 100, B::R),
                    (0.5, 0.4, 84, B::R),
                    (1.0, 0.45, 96, B::R),
                    (1.5, 0.4, 84, B::O),
                    (2.0, 0.45, 98, B::R),
                    (2.5, 0.4, 84, B::R),
                    (3.0, 0.45, 96, B::F),
                    (3.5, 0.4, 88, B::A),
                ],
            ),
            &low,
            len,
        ),
        "break" => {
            let v = over(&bass(&low, 29, 4.0, &[(0.0, 3.9, 80, B::R)]), &low, len);
            v.into_iter().filter(|e| e.at >= 16.0).collect()
        }
        "outro" => {
            let v = over(&bass(&low, 29, 4.0, &[(0.0, 3.9, 84, B::R)]), &low, len);
            v.into_iter().filter(|e| e.at < 16.0).collect()
        }
        _ => vec![],
    };
    if !line.is_empty() {
        s.clip(b.bass, &name, at, len, &line);
    }

    keys_and_pads(s, b, key, &name, at, len, chart, last);
    toplines(s, b, key, &name, at, len, last);
}

#[allow(clippy::too_many_arguments)]
fn keys_and_pads(
    s: &mut Song,
    b: &Band,
    key: &str,
    name: &str,
    at: f64,
    len: f64,
    chart: Chart,
    last: bool,
) {
    let beats = len * 4.0;
    // Lamina: held under the verse's second half and the pre-chorus, off-beat stabs in
    // the choruses (Tremor pans them, Fluctus pumps them).
    let k = lay(chart, 4, 55, 74, 64);
    let keys: Vec<Ev> = match key {
        "verse" => over(&hold(&k, 60, 0.97), &k, len)
            .into_iter()
            .filter(|e| e.at >= 32.0)
            .collect(),
        "prechorus" => over(&hold(&k, 64, 0.97), &k, len),
        "chorus" => over(
            &comp(
                &k,
                4.0,
                &[(0.5, 0.35, 74), (1.5, 0.35, 62), (2.5, 0.35, 72), (3.5, 0.35, 62)],
            ),
            &k,
            len,
        ),
        _ => vec![],
    };
    if !keys.is_empty() {
        s.clip(b.keys, name, at, len, &keys);
    }

    // Calor's pad: from the intro's second half to the end.
    let pad = lay(chart, 4, 53, 72, 62);
    let vel = match key {
        "intro" => 58,
        "verse" => 64,
        "prechorus" => 70,
        "chorus" => 78,
        "break" => 72,
        _ => 60,
    };
    let mut v = over(&hold(&pad, vel, 0.99), &pad, len);
    if key == "intro" {
        v.retain(|e| e.at >= 16.0);
    }
    s.clip(b.pad, name, at, len, &v);

    // Unda's pluck arpeggio: eighths in the verse's second half and the pre-chorus,
    // sixteenths in the choruses.
    let a = lay(chart, 4, 62, 81, 72);
    let notes: Vec<Ev> = match key {
        "verse" => arp(&a, 48, &[1, 2, 3, 4, 3, 2], 0.5, 0.4, 56)
            .into_iter()
            .map(|e| Ev { at: e.at + 32.0, ..e })
            .filter(|e| e.at < beats)
            .collect(),
        "prechorus" => {
            let mut v: Vec<Ev> = arp(&a, 48, &[1, 2, 3, 4, 3, 2], 0.5, 0.4, 60)
                .into_iter()
                .filter(|e| e.at < 16.0)
                .collect();
            v.extend(
                arp(&a, 48, &[1, 2, 3, 4, 2, 3, 4, 3], 0.25, 0.22, 60)
                    .into_iter()
                    .map(|e| Ev { at: e.at + 16.0, ..e })
                    .filter(|e| e.at < beats - 1.0),
            );
            v
        }
        "chorus" => {
            let one = arp(&a, 48, &[1, 2, 3, 4, 2, 3, 4, 3], 0.25, 0.22, 64);
            over(&one, &a, len)
        }
        _ => vec![],
    };
    if !notes.is_empty() {
        s.clip(b.arp, name, at, len, &notes);
    }

    // Exemplar's nylon guitar, picked on a syncopated sixteenth figure in the choruses.
    if key == "chorus" {
        let g = lay(chart, 3, 55, 72, 64);
        const FIGURE: [(f64, usize, u8); 6] = [
            (0.0, 0, 84),
            (0.75, 2, 70),
            (1.5, 1, 76),
            (2.0, 2, 68),
            (2.75, 0, 74),
            (3.5, 1, 70),
        ];
        let v = bars(len as usize, 4.0, |bar| {
            FIGURE
                .iter()
                .map(|(t, k, vel)| {
                    let voicing = &g.at(bar as f64 * 4.0 + t).3;
                    Ev::new(*t, 0.3, voicing[*k % voicing.len()], *vel)
                })
                .collect()
        });
        s.clip(b.guitar, name, at, len, &v);
    }

    // Exemplar's choir: the breakdown's chords, and under the last chorus.
    if key == "break" || (key == "chorus" && last) {
        let c = lay(chart, 3, 57, 74, 65);
        let v = over(&hold(&c, if key == "break" { 84 } else { 70 }, 0.99), &c, len);
        s.clip(b.choir, name, at, len, &v);
    }
}

fn toplines(s: &mut Song, b: &Band, key: &str, name: &str, at: f64, len: f64, last: bool) {
    let vox: Vec<Ev> = match key {
        "verse" => {
            let a = melody(VERSE_A, 88, 0.9);
            let bb = melody(VERSE_B, 90, 0.9);
            let mut v = a.clone();
            v.extend(shift(&bb, 16.0));
            v.extend(shift(&a, 32.0));
            v.extend(shift(&bb, 48.0));
            v
        }
        "prechorus" => {
            let mut v = melody(PRE_A, 92, 0.9);
            v.extend(shift(&melody(PRE_B, 96, 0.92), 16.0));
            v
        }
        "chorus" => hook(),
        _ => vec![],
    };
    if !vox.is_empty() {
        s.clip(b.vox, name, at, len, &vox);
    }
    // The last chorus: Calor's sync lead doubles the hook an octave up.
    if key == "chorus" && last {
        let v: Vec<Ev> = transpose(&hook(), 12)
            .into_iter()
            .map(|e| Ev { vel: 84, ..e })
            .collect();
        s.clip(b.lead, name, at, len, &v);
    }
    // Modulus's bells answer the breakdown's tune, ring-modulated as it grows.
    if key == "break" {
        let answer = "r:4 | r:2 Ab5:.5 C6:.5 Eb6:1 | r:4 | r:2 Bb5:.5 Eb6:.5 F6:1";
        let v = repeat(&melody(answer, 80, 0.9), 16.0, 2);
        s.clip(b.bells, name, at, len, &v);
    }
    // The riser into each chorus: the last four bars before it.
    if key == "prechorus" || key == "break" {
        s.clip(
            b.rise,
            &name_of("rise"),
            at + len - 4.0,
            4.0,
            &[Ev::new(0.0, 16.0, 69, 100)],
        );
    }
    // The sub drop under each new section's first bar.
    if matches!(key, "verse" | "chorus") {
        s.clip(
            b.drop,
            &name_of("drop"),
            at,
            2.0,
            &[Ev::new(0.0, 7.5, 41, if key == "verse" { 84 } else { 112 })],
        );
    }
}
