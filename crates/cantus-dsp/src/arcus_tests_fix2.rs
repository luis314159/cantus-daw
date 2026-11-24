//! r42 fix2-arcus — the mid-round review's findings on Arcus's DSP, each held by a test that
//! failed before its fix: no key, bend, tuning or rate panics or leaves a number (finding 1);
//! a held key turned unmapped keeps its pitch (2); a slur moves between the two keys' tuned
//! pitches and the attack bows the tuned note (3); the lines reach an octave and a tone under
//! every instrument's lowest string (7); an overlapped sustain or tremolo slurs, a staccato
//! re-bows (8); the upper body's power gain by its lattice and its law (5).

use super::tables::Instrument;
use super::tests::{cents, measure_hz, SR};
use super::voice::{NoteTech, Phase};
use super::*;
use crate::math;
use crate::voices::PlayMode;

/// Render `frames` into fresh buffers in blocks of 256.
fn run(f: &mut Arcus, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        f.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    (l, r)
}

fn hz(semis: f64) -> f64 {
    440.0 * math::exp2((semis - 69.0) / 12.0)
}

/// Finding 1: the β's bounds crossed under a period of 14.3 samples and `f64::clamp`
/// panicked (A7 on the cello at 48 kHz, G7 at 44.1, the violin over 13.4 kHz). Every key,
/// bent both ways, an octave either way of its table, at three rates, every instrument,
/// bowed and plucked: no panic, every sample a number, nothing loud.
#[test]
fn no_key_bend_tuning_or_rate_panics_or_leaves_a_number() {
    let tables = [0.0f32, 12.0, -12.0].map(|o| [o; 128]);
    let mut worst = 0f32;
    for sr in [44_100.0f32, 48_000.0, 96_000.0] {
        for inst in Instrument::ALL {
            for tech in [Technique::Arco, Technique::Pizzicato] {
                let s = ArcusSettings {
                    instrument: inst,
                    technique: tech,
                    mode: PlayMode::Poly,
                    poly: MAX_VOICES,
                    ..ArcusSettings::default()
                };
                let mut f = Arcus::new(sr);
                f.set(&s);
                for table in &tables {
                    f.set_tuning(table);
                    for chord in (0u8..=127).collect::<Vec<_>>().chunks(MAX_VOICES) {
                        f.reset();
                        f.pitch_bend(0.0);
                        for &k in chord {
                            f.note_on(k, 0.9, k as u64 + 1);
                        }
                        let mut out = run(&mut f, 256).0;
                        f.pitch_bend(2.0);
                        out.extend(run(&mut f, 256).0);
                        f.pitch_bend(-2.0);
                        out.extend(run(&mut f, 256).0);
                        let bad = out.iter().position(|x| !x.is_finite());
                        assert!(
                            bad.is_none(),
                            "{inst:?} {tech:?} at {sr} Hz, table {:+}, keys {chord:?}: \
                             not a number at {bad:?}",
                            table[0]
                        );
                        let pk = out.iter().fold(0f32, |m, x| m.max(x.abs()));
                        worst = worst.max(pk);
                    }
                }
            }
        }
    }
    println!("the loudest sample anywhere: {worst}");
    // Eight plucks at the top of the cello's range together reach 7.7 (one each, about 1).
    assert!(worst < 12.0, "{worst}");
}

/// Finding 1's decision: past what the loop can play in tune the note fades out (a key above
/// the string's top is silent, not a wrong pitch), and every note an instrument has keeps
/// its level to the bit.
#[test]
fn a_note_past_the_loop_s_top_fades_out_and_the_range_keeps_its_level() {
    use super::voice::{top_gain, SHORTEST_PERIOD};
    assert_eq!(top_gain(SHORTEST_PERIOD, 12.0), 1.0);
    assert_eq!(top_gain(400.0, 12.0), 1.0);
    assert_eq!(top_gain(12.0, 12.0), 0.0);
    assert_eq!(top_gain(3.0, 12.0), 0.0);
    assert_eq!(top_gain(f64::NAN, 12.0), 0.0);
    let mid = top_gain(13.0, 12.0);
    assert!(mid > 0.0 && mid < 1.0, "{mid}");
    for inst in Instrument::ALL {
        let spec = inst.spec();
        let os = upper::factor(inst) as f64;
        // The top key a vibrato and a bend reach stays in the loop's range at 44.1 kHz.
        let top = hz(spec.top as f64 + 2.5);
        let period = os * 44_100.0 / top;
        assert!(period > SHORTEST_PERIOD, "{inst:?}: {period}");
    }
    // The cello's D8, at 48 kHz: past the top — silent after the first steps.
    let mut f = Arcus::new(48_000.0);
    f.set(&ArcusSettings {
        body_mix: 0.0,
        ..ArcusSettings::default()
    });
    f.note_on(110, 1.0, 1);
    let (l, _) = run(&mut f, 12_000);
    let late = l[6_000..].iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(late < 1e-6, "{late}");
}

/// Finding 2: a key the table turns unmapped (`NAN`) under a held note — an MTS-ESP master's
/// filtered key, a `.kbm` changed while it sounds — reached the string at the next step.
/// It sounds on at its last pitch.
#[test]
fn a_held_note_whose_key_turns_unmapped_keeps_its_pitch() {
    for tech in [Technique::Arco, Technique::Pizzicato] {
        let s = ArcusSettings {
            technique: tech,
            vib_depth: 0.0,
            body_mix: 0.0,
            pizz_decay: 3.0,
            ..ArcusSettings::default()
        };
        let mut f = Arcus::new(SR);
        f.set(&s);
        let mut t = [0.0f32; 128];
        t[50] = 0.3;
        f.set_tuning(&t);
        f.note_on(50, 0.7, 1);
        let (mut l, _) = run(&mut f, 48_000);
        t[50] = f32::NAN;
        f.set_tuning(&t);
        let (l2, _) = run(&mut f, 48_000);
        assert!(l2.iter().all(|x| x.is_finite()), "{tech:?}: not a number");
        l.extend(l2);
        let want = hz(50.3);
        let before = cents(measure_hz(&l, want, 0.5, 0.98), want);
        let after = cents(measure_hz(&l, want, 1.1, 1.9), want);
        println!("{tech:?}: {before:+.3} ct before, {after:+.3} ct after the key went unmapped");
        assert!((after - before).abs() < 0.5, "{tech:?}: {before} → {after}");
    }
}

/// 31 equal steps to the octave on every key from middle C (ADR 0155's *own* tuning): key
/// `60 + n` sounds `n` steps of 38.71 cents up.
fn edo31() -> [f32; 128] {
    let mut t = [0.0f32; 128];
    for (k, o) in t.iter_mut().enumerate() {
        let n = k as f64 - 60.0;
        *o = (n * 12.0 / 31.0 - n) as f32;
    }
    t
}

/// Finding 3: a slur started at the old key's 12-TET pitch plus the new key's offset — in
/// 31-EDO a one-key slur began 61 cents low and glided up from there. It moves between the
/// two keys' tuned pitches: it starts where the finger was and ends on the new key's.
#[test]
fn a_slur_moves_between_the_two_keys_tuned_pitches() {
    let t = edo31();
    let s = ArcusSettings {
        vib_depth: 0.0,
        body_mix: 0.0,
        porta_ms: 3_000.0,
        mode: PlayMode::Legato,
        ..ArcusSettings::default()
    };
    let mut f = Arcus::new(SR);
    f.set(&s);
    f.set_tuning(&t);
    f.note_on(57, 0.7, 1);
    let (mut l, _) = run(&mut f, 72_000);
    let from = hz(57.0 + t[57] as f64);
    let to = hz(58.0 + t[58] as f64);
    f.note_on(58, 0.7, 2);
    f.note_off(57, 1);
    let (l2, _) = run(&mut f, 32);
    let first = f.newest().f0();
    l.extend(l2);
    let (l3, _) = run(&mut f, 4 * 48_000);
    l.extend(l3);
    let at_start = cents(first, from);
    let heard_start = cents(measure_hz(&l, from, 1.52, 1.62), from);
    let end = cents(measure_hz(&l, to, 4.6, 5.4), to);
    println!(
        "slur 57 → 58 in 31-EDO: its first step {at_start:+.3} ct from the old key's pitch \
         (heard {heard_start:+.2}), its end {end:+.3} ct from the new key's"
    );
    assert!(at_start.abs() < 0.5, "the slur's first step: {at_start:+.3} ct");
    assert!(heard_start.abs() < 5.0, "heard at the start: {heard_start:+.2} ct");
    assert!(end.abs() <= 1.0, "the slur's end: {end:+.3} ct");
}

/// Finding 3, the attack: the bowing and the string the note starts on hear the key's tuned
/// pitch — a violin's key 60 tuned up an octave starts as its key 72 does.
#[test]
fn the_attack_bows_the_tuned_note() {
    let s = ArcusSettings {
        instrument: Instrument::Violin,
        vib_depth: 0.0,
        ..ArcusSettings::default()
    };
    let start = |key: u8, offset: f32| {
        let mut f = Arcus::new(SR);
        f.set(&s);
        let mut t = [0.0f32; 128];
        t[key as usize] = offset;
        f.set_tuning(&t);
        f.note_on(key, 0.8, 1);
        run(&mut f, 32);
        let v = f.newest();
        (v.string_index(), v.bow_force().to_bits(), v.f0())
    };
    let (ix, force, f0) = start(60, 12.0);
    let (ix2, force2, f02) = start(72, 0.0);
    assert_eq!(ix, ix2, "the string");
    assert_eq!(force, force2, "the attack's force");
    assert!((f0 / f02 - 1.0).abs() < 1e-6, "{f0} {f02}");
}

/// Finding 7: the lines held 25 Hz at the instance's rate — the violin's 100 Hz at 4×, so a
/// tuning an octave down played its G string's bottom at the wrong pitch, the bass's E1
/// likewise. They hold an octave and a whole tone under each lowest open string at every
/// rate, and a plucked open string an octave down sounds its pitch.
#[test]
fn the_lines_reach_an_octave_and_a_tone_under_every_lowest_string() {
    for sr in [44_100.0f32, 48_000.0, 96_000.0] {
        for inst in Instrument::ALL {
            let mut f = Arcus::new(sr);
            f.set(&ArcusSettings {
                instrument: inst,
                ..ArcusSettings::default()
            });
            let low = inst.spec().bottom();
            f.note_on(low, 0.7, 1);
            run(&mut f, 64);
            let reach = f.newest().string.min_hz();
            let want = hz(low as f64 - voice::REACH_ST);
            assert!(reach <= want, "{inst:?} at {sr}: {reach:.2} Hz > {want:.2} Hz");
        }
    }
    let mut worst = (0f64, String::new());
    for inst in Instrument::ALL {
        let s = ArcusSettings {
            instrument: inst,
            technique: Technique::Pizzicato,
            body_mix: 0.0,
            pizz_decay: 3.0,
            ..ArcusSettings::default()
        };
        let mut f = Arcus::new(SR);
        f.set(&s);
        f.set_tuning(&[-12.0; 128]);
        let low = inst.spec().bottom();
        f.note_on(low, 0.8, 1);
        let (l, _) = run(&mut f, 96_000);
        let want = hz(low as f64 - 12.0);
        let e = cents(measure_hz(&l, want, 0.3, 1.9), want);
        println!("{inst:?}'s open string an octave down ({want:.2} Hz): {e:+.3} ct");
        if e.abs() > worst.0 {
            worst = (e.abs(), format!("{inst:?}: {e:+.3} ct"));
        }
    }
    assert!(worst.0 < 0.5, "{}", worst.1);
}

/// Finding 8: an overlapped note marked sustain or tremolo slurs out of a sustained note, as
/// Exemplar's scripted legato reads it (ADR 0158) — it was re-bowed; a staccato is its own
/// stroke; a tremolo slurred into takes the tremolo's strokes.
#[test]
fn an_overlapped_sustain_or_tremolo_slurs_and_a_staccato_re_bows() {
    let held = || {
        let mut f = Arcus::new(SR);
        f.set(&ArcusSettings::default());
        f.note_on(50, 0.7, 1);
        run(&mut f, 24_000);
        assert_eq!(f.newest().phase(), Phase::Hold);
        f
    };
    for (art, slurs, tech) in [
        (Art::Sustain, true, NoteTech::Arco),
        (Art::Legato, true, NoteTech::Arco),
        (Art::Tremolo, true, NoteTech::Tremolo),
        (Art::Staccato, false, NoteTech::Staccato),
        (Art::Spiccato, false, NoteTech::Spiccato),
    ] {
        let mut f = held();
        f.note_art(2, Some(art), None);
        f.note_on(53, 0.7, 2);
        let v = f.newest();
        let rebowed = v.phase() == Phase::Change;
        assert_eq!(!rebowed, slurs, "{art:?}: phase {:?}", v.phase());
        assert_eq!(v.technique(), tech, "{art:?}");
        let (l, _) = run(&mut f, 12_000);
        assert!(l.iter().all(|x| x.is_finite()), "{art:?}");
    }
    // A note without one keeps the instrument's rule: an arco cello slurs.
    let mut f = held();
    f.note_on(53, 0.7, 2);
    assert_eq!(f.newest().phase(), Phase::Hold);
}

/// Finding 5: the upper body's power gain at any size and ring by its lattice and its law,
/// against the exact measure every design took before (within 0.1 dB over the knobs'
/// ranges); at the defaults the measure itself, to the bit; and the exact measures a sweep
/// of the size and the ring takes are bounded by the lattice.
#[test]
fn the_body_s_power_law_holds_and_its_measures_are_few() {
    use upper::body::{shape_of, UpperBody, LATTICE, SIZE_LATTICE};
    let mut worst = 0f64;
    for inst in [Instrument::Violin, Instrument::Viola] {
        let mut b = UpperBody::new(SR);
        let at = |size: f32, ring: f32| {
            shape_of(&ArcusSettings {
                instrument: inst,
                body_size: size,
                body_ring: ring,
                ..ArcusSettings::default()
            })
            .unwrap()
        };
        let (exact, law) = b.powers(&at(1.0, 1.0));
        assert_eq!(exact.map(f64::to_bits), law.map(f64::to_bits), "{inst:?}: defaults");
        for size in [0.8f32, 0.9, 1.05, 1.25] {
            for ring in [0.25f32, 0.6, 1.0, 1.7, 4.0] {
                let (exact, law) = b.powers(&at(size, ring));
                for ch in 0..2 {
                    let db = 10.0 * math::log10(law[ch] / exact[ch]);
                    worst = worst.max(db.abs());
                    assert!(db.abs() < 0.1, "{inst:?} size {size} ring {ring} ch {ch}: {db:+.4} dB");
                }
            }
        }
        // A sweep: the size alone measures at most its lattice's points at ring 1, each
        // once however often it passes them; the ring and the size at most the lattice.
        let mut b = UpperBody::new(SR);
        b.set(Some(at(1.0, 1.0)));
        assert_eq!(b.measures(), 1, "{inst:?}: the default");
        for i in 0..4_000 {
            let x = (i % 1_000) as f32 / 999.0;
            b.set(Some(at(0.8 + 0.45 * x, 1.0)));
        }
        assert!(b.measures() as usize <= SIZE_LATTICE, "{inst:?}: {}", b.measures());
        for i in 0..4_000 {
            let x = (i % 1_000) as f32 / 999.0;
            b.set(Some(at(0.8 + 0.45 * x, 0.25 * math::exp2(4.0 * x))));
        }
        assert!(b.measures() as usize <= LATTICE, "{inst:?}: {}", b.measures());
        println!("{inst:?}: {} exact measures in 8 000 changes", b.measures());
    }
    println!("the law's worst against the exact measure: {worst:.4} dB");
}

/// Finding 5's measure (`cargo test --release -p cantus-dsp probe_body_modulation_cpu --
/// --ignored --nocapture`): one second of a violin voice (the reference, timed beside) against
/// one second of a body under a moving route — 1 500 changes of its size and ring — as it is
/// designed now (`UpperBody::set`) and as every change was before (the modes and the exact
/// measure). Each keeps its least disturbed of five runs, alternating.
#[test]
#[ignore = "timing: run in release with --ignored"]
fn probe_body_modulation_cpu() {
    use std::time::Instant;
    use upper::body::{shape_of, UpperBody};
    let shape = |i: usize| {
        let x = (i % 200) as f32 / 199.0;
        shape_of(&ArcusSettings {
            instrument: Instrument::Violin,
            body_size: 0.8 + 0.45 * x,
            body_ring: 0.5 + x,
            ..ArcusSettings::default()
        })
        .unwrap()
    };
    let changes = (SR as usize) / CTRL;
    let (mut voice, mut now, mut before) = (f64::MAX, f64::MAX, f64::MAX);
    for _ in 0..5 {
        let mut f = Arcus::new(SR);
        f.set(&ArcusSettings {
            instrument: Instrument::Violin,
            body_mix: 0.0,
            ..ArcusSettings::default()
        });
        f.note_on(69, 0.7, 1);
        let t = Instant::now();
        let _ = run(&mut f, SR as usize);
        voice = voice.min(t.elapsed().as_secs_f64());
        let mut b = UpperBody::new(SR);
        let t = Instant::now();
        for i in 0..changes {
            b.set(Some(shape(i)));
        }
        now = now.min(t.elapsed().as_secs_f64());
        let mut b = UpperBody::new(SR);
        let t = Instant::now();
        for i in 0..changes {
            std::hint::black_box(b.powers(&shape(i)));
        }
        before = before.min(t.elapsed().as_secs_f64());
    }
    println!(
        "one violin voice, 1 s: {:.2} ms; a body changed {changes} times: now {:.2} ms \
         ({:.3} × the voice), before {:.2} ms ({:.2} × the voice)",
        voice * 1e3,
        now * 1e3,
        now / voice,
        before * 1e3,
        before / voice
    );
}
