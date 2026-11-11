//! Aula, the convolution reverb (round 39, ADR 0112), through the engine: files written by
//! other programs read to the value, the four paths of a true-stereo file where they
//! belong, the null at 0 % wet through the engine's unit, the kernel's round trip off the
//! audio thread, and a render that hears the tail of a response read from a file.

use cantus_core::{EffectId, EffectKind, EffectSlot, EffectSpec, IrFile};
use cantus_dsp::aula_kernel::Format;
use cantus_engine::fx::aula_load::{read, read_wav};
use cantus_engine::fx::aula_store;
use cantus_engine::{EffectMemory, EffectUnit};
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// A reader written from the RIFF/WAVE specification alone (Microsoft's 1991 Multimedia
/// Programming Interface and Data Specifications 1.0, "WAVE"), to check the one Aula uses
/// against: the `fmt ` chunk's format tag (1 PCM, 3 IEEE float, 0xFFFE extensible: the
/// sub-format GUID's first two bytes), channels, rate and bits; `data` as little-endian
/// integers (8-bit unsigned, the rest signed) or 32-bit floats, interleaved.
fn by_the_spec(bytes: &[u8]) -> (u32, Vec<Vec<f64>>) {
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    let u16le = |b: &[u8]| u16::from_le_bytes([b[0], b[1]]);
    let u32le = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let (mut tag, mut ch, mut rate, mut bits) = (0u16, 0usize, 0u32, 0u16);
    let mut at = 12;
    let mut out = Vec::new();
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32le(&bytes[at + 4..]) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        if id == b"fmt " {
            tag = u16le(body);
            ch = u16le(&body[2..]) as usize;
            rate = u32le(&body[4..]);
            bits = u16le(&body[14..]);
            if tag == 0xFFFE {
                tag = u16le(&body[24..]);
            }
        } else if id == b"data" {
            let width = bits as usize / 8;
            out = vec![Vec::new(); ch];
            for (i, s) in body.chunks_exact(width).enumerate() {
                let v = match (tag, bits) {
                    (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
                    (1, 8) => (s[0] as f64 - 128.0) / 128.0,
                    (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f64 / 32_768.0,
                    (1, 24) => {
                        let x = i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8;
                        x as f64 / 8_388_608.0
                    }
                    (1, 32) => {
                        i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64 / 2_147_483_648.0
                    }
                    other => panic!("format {other:?}"),
                };
                out[i % ch].push(v);
            }
        }
        at += 8 + len + (len & 1);
    }
    (rate, out)
}

#[test]
fn files_written_by_libsndfile_and_python_read_to_the_value() {
    for rel in [
        "../cantus-dsp/tests/data/sectio/libsndfile-cues-mono16.wav",
        "../cantus-dsp/tests/data/sectio/libsndfile-smpl-stereo24.wav",
        "../cantus-dsp/tests/data/sectio/libsndfile-cues-smpl-float.wav",
        "tests/data/aula/python-wave-true-stereo-24.wav",
    ] {
        let bytes = std::fs::read(data(rel)).unwrap();
        let (rate, want) = by_the_spec(&bytes);
        let ir = read_wav(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(ir.rate, rate as f32, "{rel}");
        assert_eq!(ir.format, Format::of_channels(want.len()), "{rel}");
        assert_eq!(ir.channels.len(), want.len().clamp(1, 4), "{rel}");
        for (c, w) in want.iter().enumerate() {
            assert_eq!(ir.channels[c].len(), w.len(), "{rel} channel {c}");
            for (i, v) in w.iter().enumerate() {
                assert!(
                    (ir.channels[c][i] as f64 - v).abs() <= 1e-7,
                    "{rel} channel {c} frame {i}: {} against {v}",
                    ir.channels[c][i]
                );
            }
        }
        // Through the file path too (by extension).
        assert_eq!(read(&data(rel)).unwrap(), ir);
    }
}

#[test]
fn a_true_stereo_file_puts_each_path_where_it_belongs() {
    let bytes = std::fs::read(data("tests/data/aula/python-wave-true-stereo-24.wav")).unwrap();
    let ir = read_wav(&bytes).unwrap();
    assert_eq!(ir.format, Format::TrueStereo);
    // Each path's impulse, as Python wrote it: (c + 1) · 1e6 / 2^23 at 10 + 100c.
    for c in 0..4 {
        let at = 10 + 100 * c;
        let want = ((c + 1) as f64 * 1_000_000.0 / 8_388_608.0) as f32;
        assert_eq!(ir.channels[c][at], want, "path {c}");
    }
    // Played raw at its own rate: a click on the left comes out on the left after 10
    // samples (L→L) and on the right after 110 (L→R); one on the right after 210 (R→L)
    // and 310 (R→R).
    let cap = cantus_dsp::aula::capacity_for(ir.len());
    let k = std::sync::Arc::new(cantus_dsp::aula_kernel::Kernel::new(
        ir.format,
        &ir.channels,
        cap,
    ));
    for (side, (to_l, to_r)) in [(0usize, (10usize, 110usize)), (1, (210, 310))] {
        let mut a = cantus_dsp::aula::Aula::with_kernel(44_100.0, k.clone());
        a.set(&cantus_dsp::aula::AulaSettings {
            mix: 1.0,
            ..Default::default()
        });
        // Past the mix's glide first.
        let (mut l, mut r) = (vec![0.0f32; 4_096], vec![0.0f32; 4_096]);
        a.process(&mut l, &mut r);
        let (mut l, mut r) = (vec![0.0f32; 2_000], vec![0.0f32; 2_000]);
        if side == 0 {
            l[0] = 1.0;
        } else {
            r[0] = 1.0;
        }
        a.process(&mut l, &mut r);
        let first = |x: &[f32]| x.iter().position(|v| v.abs() > 1e-6);
        assert_eq!(first(&l), Some(to_l), "side {side}: left");
        assert_eq!(first(&r), Some(to_r), "side {side}: right");
    }
}

fn unit(spec: &EffectSpec) -> EffectUnit {
    let slot = EffectSlot {
        id: EffectId(1),
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: spec.clone(),
    };
    EffectUnit::new(&slot, SR)
}

#[test]
fn at_zero_wet_the_engines_unit_passes_the_dry_to_the_bit() {
    let mut spec = EffectSpec::new(EffectKind::Aula);
    for (k, v) in [
        ("ir", 9.0),
        ("predelay", 80.0),
        ("width", 0.4),
        ("damp", 0.5),
    ] {
        spec.set(k, v);
    }
    let mut u = unit(&spec);
    let x: Vec<f32> = (0..30_000)
        .map(|i| cantus_dsp::math::sin(i as f32 * 0.031) * 0.7)
        .collect();
    let (mut l, mut r) = (x.clone(), x.clone());
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(a, b);
    }
    assert!(l.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(r.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()));
}

#[test]
fn a_new_response_is_asked_for_built_off_the_thread_and_swapped_in() {
    let mut spec = EffectSpec::new(EffectKind::Aula);
    spec.set("mix", 1.0);
    spec.set("ir", 0.0);
    let mut u = unit(&spec);
    assert!(
        u.memory_need().is_none(),
        "a new unit is built with its response"
    );
    spec.set("ir", 8.0);
    u.configure(&spec);
    let need = u.take_request().expect("a new response is asked for");
    assert!(u.take_request().is_none(), "asked once");
    let back = u.supply(EffectMemory::build(need.need(), SR));
    assert!(back.is_some(), "the box goes back to be freed");
    assert!(u.memory_need().is_none());
    // The large hall's tail is heard (and the studio's would be over by 1 s).
    let n = 3 * SR as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    l[0] = 1.0;
    for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
        u.process(a, b);
    }
    let after = |x: &[f32], s: f32| x[(s * SR) as usize..].iter().map(|v| v * v).sum::<f32>();
    assert!(after(&r, 1.0) > 1e-5, "no tail on the right after a second");
}

#[test]
fn a_response_from_a_file_is_read_where_a_snapshot_is_compiled_and_heard() {
    // A file of our own making is fine here: what is under test is the path from the
    // project's `ir` to the kernel, not the reader.
    let dir = std::env::temp_dir().join(format!("cantus-aula-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("decay.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..44_100 {
        let g = cantus_dsp::math::exp(-4.0 * i as f32 / 44_100.0);
        let s = if i % 97 == 0 { g } else { 0.0 };
        w.write_sample(s).unwrap();
        w.write_sample(s * 0.5).unwrap();
    }
    w.finalize().unwrap();
    let stored = path.to_string_lossy().to_string();
    let mut fx = EffectSpec::new(EffectKind::Aula);
    fx.set("mix", 1.0);
    fx.ir = Some(Box::new(IrFile::new(stored.clone())));
    // Before the file is read, the unit builds silence and keeps asking.
    let mut p = cantus_core::Project::new("aula");
    let mut slot = p.make_effect(EffectKind::Aula);
    slot.spec = fx.clone();
    p.master.effects.push(slot);
    let snap = cantus_core::Snapshot::compile(&p, 1);
    let errors = aula_store::prepare(&snap, &|s| PathBuf::from(s));
    assert!(errors.is_empty(), "{errors:?}");
    assert!(matches!(
        aula_store::status(&stored),
        aula_store::IrStatus::Loaded {
            rate: 44_100,
            channels: 2,
            ..
        }
    ));
    let mut u = unit(&fx);
    let n = SR as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    l[0] = 1.0;
    r[0] = 1.0;
    for (a, b) in l.chunks_mut(300).zip(r.chunks_mut(300)) {
        u.process(a, b);
    }
    let e = |x: &[f32]| x[12_000..].iter().map(|v| v * v).sum::<f32>();
    assert!(e(&l) > 0.0 && e(&r) > 0.0, "the file's tail is heard");
    assert!(
        e(&l) > 2.0 * e(&r),
        "its right side is quieter, as the file has it"
    );
    // A missing file is a status, not a crash; it builds silence.
    let missing = dir.join("nope.wav").to_string_lossy().to_string();
    fx.ir = Some(Box::new(IrFile::new(missing.clone())));
    let mut p2 = p.clone();
    p2.master.effects[0].spec = fx.clone();
    let errs = aula_store::prepare(&cantus_core::Snapshot::compile(&p2, 2), &|s| {
        PathBuf::from(s)
    });
    assert_eq!(errs.len(), 1);
    assert!(matches!(
        aula_store::status(&missing),
        aula_store::IrStatus::Failed(_)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Published impulse responses (OpenAIR, CC BY 4.0 — the pack's files), read to the value
/// against the specification's reader, and one played raw through the convolver against
/// its own samples. They are not in the repository (ADR 0030): the test runs where
/// `CANTUS_AULA_OPENAIR` names a folder holding them, and says so when it does not.
#[test]
fn published_responses_read_to_the_value_and_play_as_recorded() {
    let Some(dir) = std::env::var_os("CANTUS_AULA_OPENAIR") else {
        eprintln!("CANTUS_AULA_OPENAIR not set: the published files are not checked here");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("wav") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let (rate, want) = by_the_spec(&bytes);
        let ir = read_wav(&bytes).unwrap();
        assert_eq!(ir.rate, rate as f32);
        for (c, w) in want.iter().enumerate().take(ir.channels.len()) {
            let worst = w
                .iter()
                .zip(&ir.channels[c])
                .fold(0.0f64, |m, (a, b)| m.max((a - *b as f64).abs()));
            assert!(
                worst <= 1e-7,
                "{}: channel {c} off by {worst:e}",
                path.display()
            );
        }
        // Played raw (no shaping, no loudness) at its own rate: a click gives the file.
        let cap = cantus_dsp::aula::capacity_for(ir.len());
        let k = std::sync::Arc::new(cantus_dsp::aula_kernel::Kernel::new(
            ir.format,
            &ir.channels,
            cap,
        ));
        let mut c = cantus_dsp::aula_conv::Convolver::new(
            Box::new(cantus_dsp::aula_conv::State::new(cap)),
            k,
        );
        let n = ir.len();
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        l[0] = 1.0;
        for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            c.process(a, b);
        }
        let peak = ir.channels[0].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let err = l
            .iter()
            .zip(&ir.channels[0])
            .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        eprintln!(
            "{}: {} Hz, {} ch, {:.2} s: played back within {:.1} dB of its peak",
            path.file_name().unwrap().to_string_lossy(),
            rate,
            ir.channels.len(),
            ir.seconds(),
            20.0 * cantus_dsp::math::log10(err.max(1e-12) / peak)
        );
        assert!(err <= 1e-5 * peak, "{}: {err:e}", path.display());
        checked += 1;
    }
    assert!(checked > 0, "no WAV in {}", dir.display());
}

/// What Aula costs: seconds of audio processed per second of CPU, heavy (the cathedral half
/// again as long, true stereo) and a studio room, in blocks of 64 and 512. Ignored: run it
/// by name with `--ignored` on a quiet machine.
#[test]
#[ignore]
fn what_it_costs() {
    for (label, ir, stretch) in [("cathedral x1.5", 9.0, 1.5), ("studio room", 0.0, 1.0)] {
        let mut spec = EffectSpec::new(EffectKind::Aula);
        spec.set("ir", ir);
        spec.set("stretch", stretch);
        spec.set("mix", 0.5);
        for block in [64usize, 512] {
            let mut u = unit(&spec);
            let secs = 20.0;
            let n = (secs * SR) as usize;
            let mut seed = 5u32;
            let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
            let t = std::time::Instant::now();
            let mut worst = std::time::Duration::ZERO;
            for _ in 0..n / block {
                for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    *a = (seed as f32 / u32::MAX as f32 - 0.5) * 0.3;
                    *b = *a;
                }
                let b0 = std::time::Instant::now();
                u.process(&mut l, &mut r);
                worst = worst.max(b0.elapsed());
            }
            let took = t.elapsed().as_secs_f64();
            eprintln!(
                "{label}, blocks of {block}: {:.0}x real time, {:.2} % of a core, worst block {:.0} us of {:.0} us",
                secs as f64 / took,
                100.0 * took / secs as f64,
                worst.as_secs_f64() * 1e6,
                block as f64 / SR as f64 * 1e6
            );
        }
    }
}
