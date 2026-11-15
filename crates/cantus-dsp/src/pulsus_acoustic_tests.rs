//! Model 4's own rules: silent without its state, the same bits however a block is cut,
//! no sound below the drum's lowest mode (the strike's push is not heard as a thump), a
//! harder hit brighter as well as louder.

use super::super::{DrumVoice, PadSettings, Pulsus};

fn pad(voice: DrumVoice, tune: f32) -> PadSettings {
    let mut s = PadSettings::of(voice, 0);
    s.model = 4;
    s.tune = tune;
    s.decay = 0.4;
    s.damping = 0.45;
    s.position = 0.3;
    s.hardness = 0.6;
    s.glide = 2.0;
    s.shell = 0.8;
    s.mix = 0.5;
    s.modes = 40;
    s.click = 0.5;
    s.tone = 2_500.0;
    s.wires = if voice == DrumVoice::Snare { 0.9 } else { 0.0 };
    s.noise_decay = 0.3;
    s.swell = 0.1;
    s.open_decay = 2.0;
    s
}

fn hit(voice: DrumVoice, tune: f32, vel: u8, n: usize, block: usize, on: bool) -> Vec<f32> {
    let mut p = Pulsus::new(48_000.0);
    if on {
        p.enable_acoustic();
    }
    p.set_kit(2, 3, 4, false);
    p.set_pad_count(1);
    p.set_pad(0, 60, &pad(voice, tune));
    p.note_on(60, vel, 1);
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    let mut at = 0;
    while at < n {
        let e = (at + block).min(n);
        p.process_add(&mut l[at..e], &mut r[at..e]);
        at = e;
    }
    l
}

/// Energy below `hz` (a one-pole low-pass twice) against all of it.
fn low_share(x: &[f32], hz: f32) -> f32 {
    let a = 1.0 - crate::math::exp(-std::f32::consts::TAU * hz / 48_000.0);
    let (mut y1, mut y2, mut lo, mut all) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for v in x {
        y1 += a * (v - y1);
        y2 += a * (y1 - y2);
        lo += y2 * y2;
        all += v * v;
    }
    lo / all.max(1e-30)
}

#[test]
fn without_its_state_a_model_4_hit_is_silent() {
    assert!(hit(DrumVoice::Tom, 0.0, 110, 4_800, 64, false)
        .iter()
        .all(|x| *x == 0.0));
}

#[test]
fn every_acoustic_voice_renders_the_same_bits_however_the_block_is_cut() {
    for v in [
        DrumVoice::Kick,
        DrumVoice::Snare,
        DrumVoice::Tom,
        DrumVoice::HiHat,
    ] {
        let a = hit(v, -6.0, 100, 24_000, 512, true);
        assert!(a.iter().any(|x| x.abs() > 1e-4), "{v:?} sounds");
        for b in [1, 37, 256] {
            assert!(a == hit(v, -6.0, 100, 24_000, b, true), "{v:?} block {b}");
        }
    }
}

#[test]
fn a_kick_has_no_thump_below_its_lowest_mode() {
    // A 55 Hz kick (220 Hz − 24 st): next to nothing below 30 Hz.
    let k = hit(DrumVoice::Kick, -24.0, 120, 48_000, 512, true);
    let share = low_share(&k, 25.0);
    assert!(share < 0.05, "below 25 Hz: {share}");
}

#[test]
fn a_harder_hit_is_brighter_not_only_louder() {
    // The attack's brightness: the first 3 ms's energy above a two-pole 2 kHz low-pass's,
    // against all of it. (A recorded snare's *body* is darker hard than soft — the wires
    // lead a ghost note — but its attack is brighter: so is this.)
    let bright = |x: &[f32]| 1.0 - low_share(&x[..144], 2_000.0);
    for v in [DrumVoice::Snare, DrumVoice::Tom, DrumVoice::Kick] {
        let soft = bright(&hit(v, -6.0, 30, 4_800, 512, true));
        let hard = bright(&hit(v, -6.0, 127, 4_800, 512, true));
        assert!(hard > soft * 1.1, "{v:?}: {soft} → {hard}");
    }
}

/// A cymbal pad as Tympanum's voicing makes one (`tympanum_voicing.rs`).
fn metal(
    hz: f32,
    decay: f32,
    pos: f32,
    hard: f32,
    tone: f32,
    swell_ms: f32,
    click: f32,
) -> PadSettings {
    let mut s = pad(DrumVoice::HiHat, 12.0 * crate::math::log2(hz / 330.0));
    s.decay = decay;
    s.position = pos;
    s.hardness = hard;
    s.tone = tone;
    s.swell = swell_ms / 1000.0;
    s.click = click;
    s.band_mix = 0.6;
    s.pedal = false;
    s
}

fn render_pad(s: &PadSettings, vel: u8, n: usize, cc4: Option<f32>) -> Vec<f32> {
    let mut p = Pulsus::new(48_000.0);
    p.enable_acoustic();
    p.set_kit(2, 3, 4, false);
    p.set_pad_count(1);
    p.set_pad(0, 60, s);
    if let Some(v) = cc4 {
        p.controller(4, v);
    }
    p.note_on(60, vel, 1);
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    p.process_add(&mut l, &mut r);
    l
}

/// Write single hits for listening and measuring (`CANTUS_ACOUSTIC_DUMP=<dir>`).
#[test]
#[ignore]
fn dump_hits() {
    let Ok(dir) = std::env::var("CANTUS_ACOUSTIC_DUMP") else {
        return;
    };
    let mut hats = metal(330.0, 0.25, 0.65, 0.75, 9_000.0, 12.0, 0.55);
    hats.pedal = true;
    hats.open_decay = 3.0;
    let list: Vec<(&str, Vec<f32>)> = vec![
        ("kick", hit(DrumVoice::Kick, -21.9, 110, 96_000, 512, true)),
        ("snare", hit(DrumVoice::Snare, -1.0, 110, 96_000, 512, true)),
        ("tom", hit(DrumVoice::Tom, -6.0, 110, 96_000, 512, true)),
        (
            "crash",
            render_pad(
                &metal(444.0, 3.8, 0.9, 0.7, 5_000.0, 110.0, 0.35),
                120,
                192_000,
                None,
            ),
        ),
        (
            "crash-soft",
            render_pad(
                &metal(444.0, 3.8, 0.9, 0.7, 5_000.0, 110.0, 0.35),
                50,
                192_000,
                None,
            ),
        ),
        (
            "ride",
            render_pad(
                &metal(400.0, 5.5, 0.5, 0.85, 4_500.0, 320.0, 0.8),
                110,
                192_000,
                None,
            ),
        ),
        ("hat-closed", render_pad(&hats, 110, 48_000, Some(0.0))),
        ("hat-open", render_pad(&hats, 110, 144_000, Some(1.0))),
    ];
    for (name, x) in list {
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + 4 * x.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&48_000u32.to_le_bytes());
        b.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&32u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(4 * x.len() as u32).to_le_bytes());
        for s in &x {
            b.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(format!("{dir}/{name}.wav"), b).unwrap();
    }
}
