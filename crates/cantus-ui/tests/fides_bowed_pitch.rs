//! Fides's bowed presets in tune (round 42, Fides's model 2 — round 41's leftover): each
//! factory preset that bows a waveguide string (Bowed cello, Bowed strings pad), as the preset
//! browser applies it (a new instance's model, 2), plays single held notes across its range
//! through the real engine
//! (`Snapshot::compile` and `render`, the body and the output stage on) with its vibrato off
//! — and each note's fundamental, after its attack, is within a cent of its key. Model 1
//! (every song made before) is measured beside it and printed, not held: its STK bow moves
//! the pitch a few cents, as round 41 said. The bowed modal objects (Glass harmonica, Bowed
//! vibraphone) keep model 1's friction and sound their modes, whose pitch is the modes' own
//! (round 41's struck-pitch test holds them), not a string's fundamental: not measured here.

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, Snapshot, Ticks};
use cantus_dsp::math;
use cantus_ui::i18n::{set_thread_locale, Locale};
use cantus_ui::preset_audition as aud;
use cantus_ui::preset_catalog::{entries, Payload, Unit};
use std::sync::Arc;

const SR: u32 = 48_000;

/// The fundamental near `near` Hz over `from…to` s of `x` (mono): heterodyne, two boxcars of
/// four periods, the phase's least-squares slope (Fides's and Arcus's measure).
fn measure_hz(x: &[f32], near: f64, from: f64, to: f64) -> f64 {
    let sr = SR as f64;
    let len = ((4.0 * sr / near).round() as usize).max(4);
    let w = std::f64::consts::TAU * near / sr;
    let (a, b) = ((from * sr) as usize, ((to * sr) as usize).min(x.len()));
    let start = a.saturating_sub(2 * len);
    let (mut re, mut im) = (Vec::new(), Vec::new());
    for (n, v) in x.iter().enumerate().take(b).skip(start) {
        let (s, c) = math::sin_cos(w * n as f64);
        re.push(*v as f64 * c);
        im.push(-(*v as f64) * s);
    }
    let boxcar = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; v.len()];
        let mut acc = 0.0;
        for i in 0..v.len() {
            acc += v[i];
            if i >= len {
                acc -= v[i - len];
            }
            out[i] = acc / len as f64;
        }
        out
    };
    let (re, im) = (boxcar(&boxcar(&re)), boxcar(&boxcar(&im)));
    let (mut ts, mut ph) = (Vec::new(), Vec::new());
    let (mut last, mut unwrap) = (0.0f64, 0.0f64);
    for i in ((a - start)..re.len()).step_by(16) {
        let p = math::atan2(im[i], re[i]);
        if !ts.is_empty() {
            let d = p - last;
            if d > std::f64::consts::PI {
                unwrap -= std::f64::consts::TAU;
            } else if d < -std::f64::consts::PI {
                unwrap += std::f64::consts::TAU;
            }
        }
        last = p;
        ts.push((i + start) as f64 / sr);
        ph.push(p + unwrap);
    }
    let n = ts.len() as f64;
    let (mt, mp) = (ts.iter().sum::<f64>() / n, ph.iter().sum::<f64>() / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (t, p) in ts.iter().zip(&ph) {
        num += (t - mt) * (p - mp);
        den += (t - mt) * (t - mt);
    }
    near + num / den / std::f64::consts::TAU
}

/// One note of `key` held `hold` s on `spec`, rendered through the engine: the left channel.
fn held(spec: &NativeSpec, key: u8, hold: f64) -> Vec<f32> {
    let mut p: Project = aud::phrase_project(
        InstrumentSpec::Native(spec.clone()),
        cantus_ui::preset_tags::Role::Lead,
    );
    // At the audition's 120 BPM a beat is half a second.
    let beats = (2.0 * hold).round() as i64;
    let mut clip = p.make_clip("held", Ticks::ZERO, Ticks::beats(beats));
    clip.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(beats), key, 100));
    p.tracks[0].clips = vec![clip];
    let snap = Arc::new(Snapshot::compile(&p, 0));
    let r = cantus_engine::render(
        snap,
        &cantus_engine::RenderOptions {
            sample_rate: SR,
            start: Ticks::ZERO,
            end: Some(Ticks::beats(beats)),
            tail_seconds: 0.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("a render");
    let ch = r.channels as usize;
    r.samples.iter().step_by(ch).copied().collect()
}

#[test]
fn fides_s_bowed_presets_play_within_a_cent_of_their_keys() {
    set_thread_locale(Locale::En);
    let unit = Unit::Instrument(InstrumentKind::Resonator);
    let mut bowed = 0;
    let mut worst = (0f64, String::new());
    for e in entries(unit, false) {
        let Payload::Native(spec) = &e.payload else {
            continue;
        };
        // The bow on a waveguide string (`a.type` 0).
        if spec.get("exc.type") != 2.0 || spec.get("a.type") != 0.0 {
            continue;
        }
        bowed += 1;
        assert_eq!(spec.get("model"), 2.0, "{}: a new instance's model", e.id);
        // The attack and the vibrato's delay over before the measure starts.
        let attack = (spec.get("bow.attack") as f64 / 1000.0).max(0.1);
        let (from, hold) = (attack + 0.6, attack + 2.2);
        for model in [1.0f32, 2.0] {
            let mut s = spec.clone();
            s.set("model", model);
            s.set("vib.depth", 0.0);
            let mut line = String::new();
            let octave = s.get("octave");
            for key in [36u8, 43, 48, 55, 60, 67, 72, 79] {
                let x = held(&s, key, hold);
                let sounding = key as f64 + 12.0 * octave as f64;
                let want = 440.0 * math::exp2((sounding - 69.0) / 12.0);
                let c = 1200.0 * math::log2(measure_hz(&x, want, from, hold - 0.05) / want);
                line += &format!(" {key}: {c:+.2}");
                if model == 2.0 && c.abs() > worst.0 {
                    worst = (c.abs(), format!("{} key {key}: {c:+.3} ct", e.id));
                }
            }
            println!("{} model {model}:{line}", e.id);
        }
    }
    assert!(bowed >= 2, "only {bowed} bowed string presets");
    println!("model 2's worst: {}", worst.1);
    assert!(worst.0 <= 1.0, "model 2's worst: {}", worst.1);
}
