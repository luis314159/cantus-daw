//! Loquax: what it does to a voice — intelligibility, consonants, formant and band shift,
//! hold, the robot, the sidechain, the level, the spread.

use super::measure::{ncm, ncm_bands, phrase, rms_db, spectral_correlation};
use super::tests::{chord, render, SR};
use super::*;
use crate::pitch::testkit::{measured_f0, sibilant, voice, Vowel};

/// The chord the tests vocode: C3, G3, C4, E4.
const CHORD: [u8; 4] = [48, 55, 60, 64];

fn vocoded(s: &LoquaxSettings, modulator: &[f32], notes: &[NoteEvent]) -> (Vec<f32>, Vec<f32>) {
    let mut q = Loquax::new(SR);
    q.set(s);
    q.reset();
    let (mut l, mut r) = (modulator.to_vec(), modulator.to_vec());
    render(&mut q, &mut l, &mut r, None, notes, 256);
    (l, r)
}

fn on(bands: usize) -> LoquaxSettings {
    LoquaxSettings {
        mix: 1.0,
        bands,
        ..LoquaxSettings::default()
    }
}

/// The carrier alone, as heard with no voice shaping it: the same chord through the unit
/// with its followers held open at a constant level (the "no information" reference).
fn carrier_alone(len: usize) -> Vec<f32> {
    let mut q = Loquax::new(SR);
    q.set(&on(32));
    q.reset();
    let pink = super::tests::noise(len, 77, 0.3).0;
    let (mut l, mut r) = (pink.clone(), pink);
    render(&mut q, &mut l, &mut r, None, &chord(&CHORD), 256);
    l
}

#[test]
fn the_output_follows_the_voice_and_more_bands_carry_more() {
    let speech = phrase(SR, 6.0, 0.5);
    let notes = chord(&CHORD);
    let base = carrier_alone(speech.len());
    let (base_ncm, base_corr) = (
        ncm(&speech, &base, SR),
        spectral_correlation(&speech, &base, SR),
    );
    eprintln!(
        "carrier shaped by noise, not the voice: NCM {base_ncm:.3}, spectral r {base_corr:.3}"
    );
    let mut last = 0.0;
    let mut scores = Vec::new();
    for &n in &[8usize, 16, 32, 64] {
        let (l, _) = vocoded(&on(n), &speech, &notes);
        let (bands, score) = ncm_bands(&speech, &l, SR);
        let corr = spectral_correlation(&speech, &l, SR);
        eprintln!(
            "{n:2} bands: NCM {score:.3} (octaves {}), spectral r {corr:.3}",
            bands
                .iter()
                .map(|r| format!("{r:.2}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        scores.push((n, score, corr));
        assert!(score > base_ncm + 0.2, "{n}: {score} vs {base_ncm}");
        assert!(corr > base_corr + 0.2, "{n}: {corr} vs {base_corr}");
        // Speech carried by a vocoder gains with its channels up to about sixteen and then
        // saturates (Shannon et al. 1995; Friesen et al. 2001): 8 → 16 must gain, 32 must
        // hold; at 64 a chord's sparse partials leave many narrow bands to its noise and
        // the measure eases (said, not hidden).
        match n {
            16 => assert!(score > last + 0.1, "{n} bands: NCM {score} after {last}"),
            32 => assert!(score > last - 0.05, "{n} bands: NCM {score} after {last}"),
            64 => assert!(score > last - 0.1, "{n} bands: NCM {score} after {last}"),
            _ => {}
        }
        last = score;
    }
    let (_, ncm32, corr32) = scores[2];
    assert!(ncm32 > 0.6, "{ncm32}");
    assert!(corr32 > 0.75, "{corr32}");
}

#[test]
fn the_consonants_come_through() {
    let speech = phrase(SR, 6.0, 0.5);
    let notes = chord(&CHORD);
    let mut dry = on(32);
    dry.sibilance = 0.0;
    dry.unvoiced = 0.0;
    let (plain, _) = vocoded(&dry, &speech, &notes);
    let (full, _) = vocoded(&on(32), &speech, &notes);
    let (bp, np) = ncm_bands(&speech, &plain, SR);
    let (bf, nf) = ncm_bands(&speech, &full, SR);
    eprintln!(
        "consonants off: NCM {np:.3}, 4 k {:.2}, 8 k {:.2}",
        bp[5], bp[6]
    );
    eprintln!(
        "consonants on:  NCM {nf:.3}, 4 k {:.2}, 8 k {:.2}",
        bf[5], bf[6]
    );
    assert!(bf[6] > bp[6] + 0.025, "8 kHz: {} vs {}", bf[6], bp[6]);
    assert!(bf[5] >= bp[5], "4 kHz: {} vs {}", bf[5], bp[5]);
    assert!(nf > np, "{nf} vs {np}");
}

#[test]
fn the_detector_tells_a_vowel_from_a_sibilant() {
    let mut v = voicing::Voicing::new(SR);
    let vowel = voice(SR, 0.5, Vowel::A, 0.5, |_| 130.0);
    let mut gate = 0.0;
    for &x in &vowel {
        gate = v.run(x).0;
    }
    assert!(gate < 0.05, "a vowel opens the gate: {gate}");
    let s = sibilant(SR, 0.3, 0.3, 7);
    for &x in &s {
        gate = v.run(x).0;
    }
    assert!(gate > 0.95, "a sibilant leaves it shut: {gate}");
    // Silence shuts it, to +0.0.
    for _ in 0..48_000 {
        gate = v.run(0.0).0;
    }
    assert_eq!(gate.to_bits(), 0);
    assert!(v.at_rest());
}

/// The spectral centroid of `x` between 200 Hz and 8 kHz.
fn centroid(x: &[f32]) -> f32 {
    let frames = measure::power_frames(x);
    let bin = SR / measure::FRAME as f32;
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for f in &frames {
        for (k, p) in f.iter().enumerate() {
            let hz = k as f32 * bin;
            if (200.0..8_000.0).contains(&hz) {
                num += f64::from(hz) * f64::from(*p);
                den += f64::from(*p);
            }
        }
    }
    (num / den) as f32
}

#[test]
fn a_formant_shift_moves_the_voice_and_not_the_notes() {
    let vowel = voice(SR, 2.0, Vowel::A, 0.5, |_| 120.0);
    let notes = chord(&[45]);
    let mut s = on(32);
    s.sibilance = 0.0;
    s.unvoiced = 0.0;
    s.synth.noise = 0.0;
    s.synth.unison = 1;
    let (flat, _) = vocoded(&s, &vowel, &notes);
    s.formant = 7.0;
    let (up, _) = vocoded(&s, &vowel, &notes);
    s.formant = -7.0;
    let (down, _) = vocoded(&s, &vowel, &notes);
    let (c0, cu, cd) = (
        centroid(&flat[24_000..]),
        centroid(&up[24_000..]),
        centroid(&down[24_000..]),
    );
    eprintln!("centroid: −7 st {cd:.0} Hz, 0 {c0:.0} Hz, +7 st {cu:.0} Hz");
    assert!(cu > c0 * 1.2 && cd < c0 / 1.2, "{cd} {c0} {cu}");
    // The pitch is the carrier's (A2, 110 Hz) whatever the formants do.
    for x in [&flat, &up, &down] {
        let f0 = measured_f0(&x[24_000..], SR, 110.0);
        assert!((f0 - 110.0).abs() < 1.0, "{f0}");
    }
}

#[test]
fn a_band_shift_moves_a_band_to_its_neighbour() {
    // A sine in band 10's centre: shifted by +2, band 12 speaks and band 10 is quiet.
    let s0 = on(16);
    let q = Loquax::new(SR);
    let mut probe = q.clone();
    probe.set(&s0);
    let mut view = BandView::default();
    probe.bands(&mut view);
    let f = view.analysis_hz[10];
    let tone: Vec<f32> = (0..48_000)
        .map(|i| 0.3 * math::sin(core::f32::consts::TAU * f * i as f32 / SR))
        .collect();
    for (shift, loud) in [(0.0f32, 10usize), (2.0, 12)] {
        let mut s = s0;
        s.band_shift = shift;
        s.sibilance = 0.0;
        let mut q = Loquax::new(SR);
        q.set(&s);
        q.reset();
        let (mut l, mut r) = (tone.clone(), tone.clone());
        render(&mut q, &mut l, &mut r, None, &chord(&CHORD), 256);
        q.bands(&mut view);
        let top = (0..16)
            .max_by(|&a, &b| view.output_db[a].total_cmp(&view.output_db[b]))
            .unwrap_or(0);
        assert_eq!(top, loud, "shift {shift}: {:?}", &view.output_db[..16]);
    }
}

#[test]
fn hold_keeps_the_vowel_after_the_voice_stops() {
    let mut s = on(32);
    let vowel = voice(SR, 0.6, Vowel::A, 0.5, |_| 120.0);
    let mut q = Loquax::new(SR);
    q.set(&s);
    q.reset();
    let notes = chord(&CHORD);
    let (mut l, mut r) = (vowel.clone(), vowel.clone());
    render(&mut q, &mut l, &mut r, None, &notes, 256);
    let mut before = BandView::default();
    q.bands(&mut before);
    s.hold = true;
    q.set(&s);
    let (mut l, mut r) = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
    render(&mut q, &mut l, &mut r, None, &[], 256);
    let mut after = BandView::default();
    q.bands(&mut after);
    assert_eq!(before.modulator_db, after.modulator_db);
    assert!(
        rms_db(&l[24_000..]) > -30.0,
        "{} voices {} carrier {} mod {:?}",
        rms_db(&l[24_000..]),
        q.tel[t::VOICES],
        q.tel[t::CARRIER_DB],
        &after.modulator_db[..8]
    );
    // Released, the bands fall at their release (40 ms: −109 dB in half a second).
    s.hold = false;
    q.set(&s);
    let (mut l, mut r) = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
    render(&mut q, &mut l, &mut r, None, &[], 256);
    assert!(rms_db(&l[24_000..]) < -90.0, "{}", rms_db(&l[24_000..]));
}

#[test]
fn the_robot_drones_its_note_while_the_voice_speaks() {
    let mut s = on(16);
    s.robot = true;
    s.robot_note = 48.0;
    s.synth.unison = 1;
    s.synth.noise = 0.0;
    s.sibilance = 0.0;
    let vowel = voice(SR, 1.0, Vowel::U, 0.5, |t| 100.0 + 40.0 * t as f32);
    let mut q = Loquax::new(SR);
    q.set(&s);
    q.reset();
    let (mut l, mut r) = (vowel.clone(), vowel.clone());
    render(&mut q, &mut l, &mut r, None, &[], 256);
    assert_eq!(q.tel[t::ROBOT], 1.0);
    // C3 whatever the voice sings.
    let f0 = measured_f0(&l[12_000..], SR, 130.8);
    assert!((f0 - 130.81).abs() < 1.0, "{f0}");
    // Silent, the drone stops and the unit comes to rest.
    let mut secs = 0.0;
    while !q.at_rest() {
        let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
        q.process(&mut l, &mut r, None, &[]);
        secs += 0.01;
        assert!(secs < 15.0, "{:?}", q.rest_blocker());
    }
}

#[test]
fn a_sidechain_carries_and_swap_trades_the_roles() {
    let speech = phrase(SR, 4.0, 0.5);
    // The "pad" on the sidechain: the unit's own synthesiser rendered alone.
    let mut pad = carrier::Synth::new(SR);
    for &k in &CHORD {
        pad.note(
            NoteKind::On {
                key: k,
                velocity: 0.8,
            },
            true,
        );
    }
    let (pl, pr): (Vec<f32>, Vec<f32>) = (0..speech.len()).map(|_| pad.tick()).unzip();
    let mut s = on(32);
    s.source = Source::Sidechain;
    let mut q = Loquax::new(SR);
    q.set(&s);
    q.reset();
    let (mut l, mut r) = (speech.clone(), speech.clone());
    render(&mut q, &mut l, &mut r, Some((&pl, &pr)), &[], 256);
    let a = ncm(&speech, &l, SR);
    // Swapped: the strip plays the pad, the sidechain speaks.
    s.swap = true;
    let mut q = Loquax::new(SR);
    q.set(&s);
    q.reset();
    let (mut l2, mut r2) = (pl.clone(), pr.clone());
    render(&mut q, &mut l2, &mut r2, Some((&speech, &speech)), &[], 256);
    let b = ncm(&speech, &l2, SR);
    eprintln!("sidechain carrier: NCM {a:.3}; swapped: NCM {b:.3}");
    assert!(a > 0.6 && b > 0.6, "{a} {b}");
    assert!((a - b).abs() < 0.05);
    // No sidechain routed: the carrier is silent but for the consonants' noise.
    s.swap = false;
    s.unvoiced = 0.0;
    s.sibilance = 0.0;
    let (l, _) = vocoded(&s, &speech, &[]);
    assert!(l.iter().all(|x| x.to_bits() == 0));
}

#[test]
fn the_vocoded_voice_is_about_as_loud_as_the_voice() {
    let speech = phrase(SR, 6.0, 0.5);
    let (l, r) = vocoded(&on(32), &speech, &chord(&CHORD));
    let (dv, dl, dr) = (rms_db(&speech), rms_db(&l), rms_db(&r));
    eprintln!("voice {dv:.1} dBFS, vocoded {dl:.1} / {dr:.1} dBFS");
    assert!((dl - dv).abs() < 3.0, "{dl} vs {dv}");
}

#[test]
fn spread_sends_alternate_bands_left_and_right() {
    let speech = phrase(SR, 3.0, 0.5);
    let mut s = on(16);
    s.spread = 1.0;
    s.sibilance = 0.0;
    s.unvoiced = 0.0;
    s.synth.spread = 0.0;
    let (l, r) = vocoded(&s, &speech, &chord(&CHORD));
    let diff: f64 = l.iter().zip(&r).map(|(a, b)| f64::from(a - b).abs()).sum();
    assert!(diff > 1.0);
    // Band 6 (even) is left only: a tone at its centre comes out of the left.
    let mut view = BandView::default();
    let mut probe = Loquax::new(SR);
    probe.set(&s);
    probe.bands(&mut view);
    let f = view.analysis_hz[6];
    let tone: Vec<f32> = (0..48_000)
        .map(|i| 0.3 * math::sin(core::f32::consts::TAU * f * i as f32 / SR))
        .collect();
    let (l, r) = vocoded(&s, &tone, &chord(&[36, 48]));
    let (el, er) = (rms_db(&l[24_000..]), rms_db(&r[24_000..]));
    // The tone leaks into bands 5 and 7 (odd: right) well down.
    assert!(el > er + 12.0, "{el} {er}");
}

#[test]
fn the_synthesiser_steals_the_oldest_voice_and_a_zero_velocity_releases() {
    let mut s = carrier::Synth::new(SR);
    for k in 40..60u8 {
        s.note(
            NoteKind::On {
                key: k,
                velocity: 0.7,
            },
            true,
        );
        s.tick();
    }
    assert_eq!(s.sounding(), carrier::MAX_VOICES);
    // Velocity 0 is a release, as MIDI has it; the voice fades and ends.
    for k in 40..60u8 {
        s.note(
            NoteKind::On {
                key: k,
                velocity: 0.0,
            },
            true,
        );
    }
    assert!(!s.any_held());
    for _ in 0..(SR as usize * 4) {
        s.tick();
    }
    assert_eq!(s.sounding(), 0);
    assert!(s.at_rest());
}
