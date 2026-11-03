use super::*;

const RATE: u32 = 48_000;

fn noise(seed: u32, n: usize, amp: f32) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((s >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * amp
        })
        .collect()
}

fn stereo(frames: usize) -> AudioBuf {
    AudioBuf::new(RATE, 2, noise(5, frames * 2, 0.4))
}

fn bits(a: &AudioBuf, b: &AudioBuf) -> bool {
    a.rate == b.rate
        && a.channels == b.channels
        && a.data.len() == b.data.len()
        && a.data
            .iter()
            .zip(&b.data)
            .all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Raw joins: these tests hold the edits to the sample (the joins have their own, in
/// `audio_edit_tests2.rs`).
fn run(b: &AudioBuf, s: Sel, e: Edit) -> Edited {
    apply_with(b, s, &e, &Joins::RAW).unwrap()
}

#[test]
fn identity_edits_return_the_bits() {
    let b = stereo(30_000);
    let s = Sel::new(1000, 20_000);
    // Gain of 0 dB, and a normalize already at its target.
    let e = run(&b, s, Edit::Gain { db: 0.0 });
    assert!(bits(&e.buf, &b) && !e.changed);
    // Reverse twice, invert twice.
    let r = run(&b, s, Edit::Reverse);
    assert!(r.changed);
    assert!(bits(&run(&r.buf, s, Edit::Reverse).buf, &b));
    let i = run(&b, s, Edit::Invert);
    assert!(bits(&run(&i.buf, s, Edit::Invert).buf, &b));
    // A cut put back where it was.
    let clip = b.slice(s);
    let cut = run(&b, s, Edit::Delete);
    assert_eq!(cut.delta_frames, -(s.len() as i64));
    let back = run(
        &cut.buf,
        Sel::new(1000, 1000),
        Edit::Paste { clip: clip.clone() },
    );
    assert!(bits(&back.buf, &b));
    assert_eq!(back.sel, s);
    // Noise reduction at an amount of 0, and a region at 0 dB.
    let profile = NoiseProfile::learn(
        &[
            b.channel(0)[..10_000].to_vec(),
            b.channel(1)[..10_000].to_vec(),
        ],
        RATE,
    )
    .unwrap();
    let d = run(
        &b,
        s,
        Edit::Denoise {
            profile,
            params: Params {
                amount: 0.0,
                ..Params::default()
            },
        },
    );
    assert!(bits(&d.buf, &b) && !d.changed);
    let a = run(
        &b,
        s,
        Edit::Attenuate {
            lo_hz: 500.0,
            hi_hz: 2000.0,
            gain_db: 0.0,
        },
    );
    assert!(bits(&a.buf, &b) && !a.changed);
}

#[test]
fn an_edit_touches_only_its_selection() {
    let b = stereo(20_000);
    let s = Sel::new(5000, 9000);
    for e in [
        Edit::Silence,
        Edit::Gain { db: -6.0 },
        Edit::Invert,
        Edit::Reverse,
        Edit::DcRemove,
        Edit::FadeIn {
            curve: FadeCurve::Linear,
        },
        Edit::FadeOut {
            curve: FadeCurve::SCurve,
        },
        Edit::Normalize {
            level: Level::Peak,
            target_db: -1.0,
            ceiling_db: None,
        },
    ] {
        let name = e.name();
        let out = run(&b, s, e).buf;
        assert_eq!(out.frames(), b.frames(), "{name}");
        assert!(
            out.data[..5000 * 2]
                .iter()
                .zip(&b.data)
                .all(|(x, y)| x.to_bits() == y.to_bits())
                && out.data[9000 * 2..]
                    .iter()
                    .zip(&b.data[9000 * 2..])
                    .all(|(x, y)| x.to_bits() == y.to_bits()),
            "{name} reached outside"
        );
    }
}

#[test]
fn an_attenuated_region_leaves_far_samples_alone() {
    let b = stereo(40_000);
    let out = run(
        &b,
        Sel::new(18_000, 22_000),
        Edit::Attenuate {
            lo_hz: 1000.0,
            hi_hz: 4000.0,
            gain_db: -30.0,
        },
    );
    assert!(out.changed);
    let n = 2;
    assert!(out.buf.data[..10_000 * n]
        .iter()
        .zip(&b.data)
        .all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(out.buf.data[30_000 * n..]
        .iter()
        .zip(&b.data[30_000 * n..])
        .all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(out.buf.rms(Sel::new(19_000, 21_000)) < b.rms(Sel::new(19_000, 21_000)));
}

#[test]
fn fades_run_from_silence_to_unity() {
    let b = AudioBuf::new(RATE, 1, vec![1.0; 1000]);
    let s = Sel::new(100, 600);
    for curve in FadeCurve::ALL {
        let f = run(&b, s, Edit::FadeIn { curve }).buf;
        assert_eq!(f.data[100], 0.0, "{curve:?}");
        assert!(f.data[599] > 0.9 && f.data[599] <= 1.0);
        assert_eq!(f.data[600], 1.0);
        let o = run(&b, s, Edit::FadeOut { curve }).buf;
        assert_eq!(o.data[599], 0.0, "{curve:?}");
        assert!(o.data[100] > 0.9 && o.data[100] <= 1.0);
        assert_eq!(o.data[99], 1.0);
    }
}

#[test]
fn normalize_reaches_its_target_by_peak_rms_and_loudness() {
    let b = stereo(RATE as usize * 3);
    let whole = Sel::whole(b.frames());
    let p = run(
        &b,
        whole,
        Edit::Normalize {
            level: Level::Peak,
            target_db: -1.0,
            ceiling_db: None,
        },
    )
    .buf;
    assert!((20.0 * cantus_dsp::math::log10(p.peak(whole)) + 1.0).abs() < 0.01);
    // Doing it again changes nothing.
    let again = run(
        &p,
        whole,
        Edit::Normalize {
            level: Level::Peak,
            target_db: -1.0,
            ceiling_db: None,
        },
    );
    assert!(
        !again.changed
            || (20.0 * cantus_dsp::math::log10(again.buf.peak(whole)) + 1.0).abs() < 0.01
    );
    let r = run(
        &b,
        whole,
        Edit::Normalize {
            level: Level::Rms,
            target_db: -20.0,
            ceiling_db: None,
        },
    )
    .buf;
    assert!((20.0 * cantus_dsp::math::log10(r.rms(whole)) + 20.0).abs() < 0.01);
    let l = run(
        &b,
        whole,
        Edit::Normalize {
            level: Level::Lufs,
            target_db: -16.0,
            ceiling_db: None,
        },
    )
    .buf;
    let got = l.lufs(whole).unwrap();
    assert!((got + 16.0).abs() < 0.05, "{got}");
    // Silence is left alone, said as unchanged.
    let z = AudioBuf::new(RATE, 1, vec![0.0; 5000]);
    assert!(
        !run(
            &z,
            Sel::whole(5000),
            Edit::Normalize {
                level: Level::Peak,
                target_db: -1.0,
                ceiling_db: None
            }
        )
        .changed
    );
}

#[test]
fn dc_is_taken_out_per_channel() {
    let mut b = stereo(10_000);
    for f in b.data.chunks_exact_mut(2) {
        f[0] += 0.25;
        f[1] -= 0.1;
    }
    let out = run(&b, Sel::whole(10_000), Edit::DcRemove).buf;
    for c in 0..2 {
        let mean: f64 = out.channel(c).iter().map(|v| *v as f64).sum::<f64>() / 10_000.0;
        assert!(mean.abs() < 1e-5, "channel {c}: {mean}");
    }
}

fn phrases() -> AudioBuf {
    // 0.5 s of tone, 2 s of silence, 0.5 s of tone, 0.2 s of silence, 0.5 s of tone.
    let tone = |n: usize| -> Vec<f32> {
        (0..n)
            .map(|i| 0.5 * cantus_dsp::math::sin(i as f32 * 0.2))
            .collect()
    };
    let r = RATE as usize;
    let mut d = tone(r / 2);
    d.extend(vec![0.0; r * 2]);
    d.extend(tone(r / 2));
    d.extend(vec![0.0; r / 5]);
    d.extend(tone(r / 2));
    AudioBuf::new(RATE, 1, d)
}

#[test]
fn remove_silence_cuts_long_quiet_and_keeps_short_breaths() {
    let b = phrases();
    let s = silence::Settings {
        threshold_db: -50.0,
        min_ms: 300.0,
        keep_ms: 100.0,
    };
    let runs = silence::find(&b, Sel::whole(b.frames()), &s);
    assert_eq!(runs.len(), 1, "{runs:?}");
    let out = run(&b, Sel::whole(b.frames()), Edit::RemoveSilence(s));
    // The 2 s gap leaves 100 ms; the 0.2 s one is under the minimum and stays.
    let expect = b.frames() - (RATE as usize * 2 - RATE as usize / 10);
    assert!(
        (out.buf.frames() as i64 - expect as i64).abs() <= 480,
        "{} vs {expect}",
        out.buf.frames()
    );
    assert_eq!(
        out.delta_frames,
        out.buf.frames() as i64 - b.frames() as i64
    );
    // The phrases are intact (peak and rms of the first, to the bit).
    assert!(bits(
        &out.buf.slice(Sel::new(0, 20_000)),
        &b.slice(Sel::new(0, 20_000))
    ));
    // Nothing quiet enough to cut: unchanged, no new file.
    let loud = AudioBuf::new(RATE, 1, noise(2, 20_000, 0.5));
    let same = run(&loud, Sel::whole(20_000), Edit::RemoveSilence(s));
    assert!(!same.changed && bits(&same.buf, &loud));
    // Only inside the selection.
    let part = run(&b, Sel::new(0, 20_000), Edit::RemoveSilence(s));
    assert!(!part.changed);
}

#[test]
fn bad_requests_are_refused() {
    let b = stereo(1000);
    let empty = Sel::new(10, 10);
    assert_eq!(
        apply(&b, empty, &Edit::Delete),
        Err(EditError::NeedsSelection)
    );
    assert_eq!(
        apply(&b, Sel::whole(1000), &Edit::Delete),
        Err(EditError::WouldBeEmpty)
    );
    let other = AudioBuf::new(44_100, 2, vec![0.0; 10]);
    assert_eq!(
        apply(&b, empty, &Edit::Paste { clip: other }),
        Err(EditError::Mismatch)
    );
    assert!(apply(&b, Sel::new(0, 10), &Edit::Gain { db: f32::NAN }).is_err());
    assert!(apply(
        &b,
        Sel::new(0, 10),
        &Edit::Attenuate {
            lo_hz: 3000.0,
            hi_hz: 1000.0,
            gain_db: -6.0
        }
    )
    .is_err());
    // A selection past the end is clamped, not a panic.
    assert!(apply(&b, Sel::new(500, 99_999), &Edit::Invert).is_ok());
}

#[test]
fn insert_silence_and_trim() {
    let b = stereo(1000);
    let i = run(&b, Sel::new(200, 200), Edit::InsertSilence { frames: 50 });
    assert_eq!(i.buf.frames(), 1050);
    assert!(i.buf.data[200 * 2..250 * 2].iter().all(|v| *v == 0.0));
    assert_eq!(i.sel, Sel::new(200, 250));
    let t = run(&b, Sel::new(100, 300), Edit::Trim);
    assert_eq!(t.buf.frames(), 200);
    assert!(bits(&t.buf, &b.slice(Sel::new(100, 300))));
}

#[test]
fn a_denoised_selection_is_the_selection_only() {
    let n = RATE as usize * 2;
    let mut d = noise(9, n, 0.02);
    for (i, v) in d[RATE as usize..].iter_mut().enumerate() {
        *v += 0.3 * cantus_dsp::math::sin(i as f32 * 0.07);
    }
    let b = AudioBuf::new(RATE, 1, d);
    let profile = NoiseProfile::learn(&[b.channel(0)[..RATE as usize / 2].to_vec()], RATE).unwrap();
    let out = run(
        &b,
        Sel::whole(n),
        Edit::Denoise {
            profile,
            params: Params::default(),
        },
    );
    assert!(out.changed);
    let hiss = Sel::new(2000, RATE as usize - 4000);
    assert!(out.buf.rms(hiss) < b.rms(hiss) * 0.25);
}

#[test]
fn the_file_is_deterministic_and_named_by_its_content() {
    let b = stereo(4000);
    let bytes = wav::encode(&b);
    assert_eq!(bytes, wav::encode(&b));
    assert!(bits(&wav::parse(&bytes).unwrap(), &b));
    let h = wav::sha256_hex(&bytes);
    assert_eq!(h.len(), 64);
    assert_eq!(wav::file_name(&h), format!("edit-{}.wav", &h[..16]));
    let other = run(&b, Sel::new(0, 100), Edit::Invert).buf;
    assert_ne!(wav::sha256_hex(&wav::encode(&other)), h);
}
