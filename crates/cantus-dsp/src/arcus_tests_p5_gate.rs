//! Pass 5's gates (round 42, `docs/round42/arcus-match.md` *Pass 5*): what a musical demo
//! found by measurement must not come back — the tremolo in tune on every string, the
//! violin's top in tune at ff, no click at a bow change (against the references' own bow
//! changes), a spiccato that leaves the string (against the references' strokes).
//!
//! The references are `research/instruments-next/bowed-refdb/strokes.json`, measured by
//! `tools/arcus/pass5/strokes.py` on Philharmonia's détaché and spiccato phrases and VSCO's
//! solo violin and contrabass spiccato strokes; the same measures are taken here in Rust
//! (`tests_p5`: `tick_parts`, `bounce_at`), at the notes' own times.

use super::tables::Instrument;
use super::tests_p5::*;
use super::*;

/// The tremolo's pitch, by the study's tracker (YIN), on every instrument's every string,
/// open and a fourth up, alone and slurred into, soft and loud: each within 5 cents of its
/// key and the mean within 1.5. Round 42's tremolo was 8–255 cents out (the bass's lowest
/// never periodic), the demo's −80 cents on the cello's open A.
#[test]
fn the_tremolo_is_in_tune_on_every_string() {
    let mut all = Vec::new();
    let mut worst = (0f64, String::new());
    for inst in Instrument::ALL {
        for st in inst.spec().strings {
            for key in [st.open, st.open + 5] {
                for slurred in [false, true] {
                    for vel in [0.4f32, 0.8] {
                        let (c, q) = tremolo_cents(&plain(inst), key, vel, slurred);
                        let what = format!(
                            "{inst:?} key {key} {} v {vel}: {c:+.2} ct (periodic {:.2})",
                            if slurred { "slurred" } else { "alone" },
                            q[6]
                        );
                        assert!(c.is_finite(), "{what}: no periodic frame");
                        if c.abs() > worst.0 {
                            worst = (c.abs(), what);
                        }
                        all.push(c.abs());
                    }
                }
            }
        }
    }
    let mean = all.iter().sum::<f64>() / all.len() as f64;
    println!(
        "{} tremolos: mean |c| {mean:.2}, worst {}",
        all.len(),
        worst.1
    );
    assert!(worst.0 <= 5.0, "the worst {}", worst.1);
    assert!(mean <= 1.5, "the mean {mean:.2} ct");
}

/// The violin's top (E5–A6) bowed ff by the cantabile preset, the dynamic from CC1: within
/// a cent held, a cent and a half in a half-second note (the vibrato off: the pitch, not
/// its swing). The demo heard B5 flat at forte; measured, it was not (pass 5) — held here.
#[test]
fn the_violin_s_top_is_in_tune_at_ff() {
    let mut worst = (0f64, 0u8, 0f64);
    for key in 76u8..=93 {
        for (len, limit) in [(1.5, 1.0), (0.5, 1.5)] {
            let (c, ..) = held_cents(&violin_cantabile(false), key, 1.0, len);
            assert!(c.abs() <= limit, "key {key} held {len} s: {c:+.2} ct");
            if c.abs() > worst.0 {
                worst = (c.abs(), key, len);
            }
        }
    }
    println!(
        "the violin's top at ff: worst {:.2} ct (key {}, {} s)",
        worst.0, worst.1, worst.2
    );
}

/// The references' ticks (`strokes.json`'s `tick.over_post_db`, Philharmonia's détaché
/// phrases): each family's median and p90, dB — the violin's 26 changes pooled with the
/// viola's 205 (the upper strings').
const REF_TICK: [(Instrument, f64, f64); 4] = [
    (Instrument::Cello, 0.0, 8.5),
    (Instrument::Bass, 7.6, 16.2),
    (Instrument::Viola, -1.8, 2.7),
    (Instrument::Violin, -1.8, 2.7),
];

/// A bow change does not click: on every instrument, a détaché and a plain re-bow on one
/// pitch, its ticks' median under the references' p90 and its loudest under their p90 + 6
/// dB; the demo's bass line (section III, the cantabile preset) under the bass's median
/// — round 42's stood 17 dB over its own high end there, a click 4–6 ms wide.
#[test]
fn a_bow_change_does_not_click() {
    for (inst, _med, p90) in REF_TICK {
        let st = strokes_of(&ArcusSettings {
            instrument: inst,
            ..ArcusSettings::default()
        });
        for (name, t) in [("détaché", &st.detache), ("re-bow", &st.rebow)] {
            let (m, _, _) = spread(t);
            let mx = t.iter().cloned().fold(f64::MIN, f64::max);
            println!(
                "{inst:?} {name}: median {m:+.1} dB, loudest {mx:+.1} (references' p90 {p90:+.1})"
            );
            assert!(
                m <= p90,
                "{inst:?} {name}: median {m:+.1} dB over the references' p90"
            );
            assert!(mx <= p90 + 6.0, "{inst:?} {name}: a tick {mx:+.1} dB");
        }
    }
    let (score, spans) = phrase_score(&BASS_III, &[(0.0, 0.38), (22.0, 0.48)], 74.0);
    let x = play(&bass_cantabile(), &score, spans.last().unwrap().2 + 0.5);
    let post: Vec<f64> = parts_of(&x, &score).iter().map(|p| p.1).collect();
    let (m, _, _) = spread(&post);
    let mx = post.iter().cloned().fold(f64::MIN, f64::max);
    println!("the demo's bass line: median {m:+.1} dB, loudest {mx:+.1}");
    assert!(
        m <= REF_TICK[1].1,
        "the bass line's ticks: median {m:+.1} dB"
    );
    assert!(
        mx <= REF_TICK[1].2,
        "the bass line's loudest tick {mx:+.1} dB"
    );
}

/// The references' strokes (`strokes.json`): how far a spiccato stroke falls 150 and
/// 250 ms after its peak, p10 and p90, dB — VSCO's single strokes for the violin and the
/// bass, Philharmonia's phrases for the cello and the viola (their room fills the falls in:
/// the lower edges are loose).
const REF_FALL: [(Instrument, (f64, f64), (f64, f64)); 4] = [
    (Instrument::Violin, (10.1, 23.9), (21.8, 37.0)),
    (Instrument::Bass, (4.4, 14.8), (7.1, 23.0)),
    (Instrument::Cello, (-6.3, 15.8), (-6.3, 23.4)),
    (Instrument::Viola, (2.9, 16.7), (5.1, 15.1)),
];

/// A spiccato leaves the string: at 74 BPM eighths each stroke falls within the references'
/// band 150 ms after its peak (and, for the single strokes', 250 ms), and the level between
/// two strokes drops at least 12 dB under the stroke's peak (the phrases' p10 is 9–18). Round
/// 42's fell 4–5 dB by 150 ms and 8–12 dB at most before the next: a light détaché.
#[test]
fn a_spiccato_leaves_the_string() {
    for (inst, f150, f250) in REF_FALL {
        let st = strokes_of(&ArcusSettings {
            instrument: inst,
            ..ArcusSettings::default()
        });
        let (a, b, gap) = (
            spread(&st.falls[2]).0,
            spread(&st.falls[3]).0,
            spread(&st.gaps).0,
        );
        println!("{inst:?}: falls {a:.1} dB at 150 ms, {b:.1} at 250, gap {gap:.1}");
        assert!(
            a >= f150.0 && a <= f150.1,
            "{inst:?}: {a:.1} dB at 150 ms, the references {f150:?}"
        );
        if matches!(inst, Instrument::Violin | Instrument::Bass) {
            assert!(
                b >= f250.0 && b <= f250.1,
                "{inst:?}: {b:.1} dB at 250 ms, the references {f250:?}"
            );
        }
        assert!(gap >= 12.0, "{inst:?}: the gap {gap:.1} dB");
    }
}
