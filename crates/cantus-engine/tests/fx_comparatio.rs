//! Comparatio, the reference comparator, through the whole engine (ADR 0146, provisional):
//! at its defaults, and asked for the reference with none loaded, the mix untouched to the
//! bit; a reference supplied is heard at the mix's loudness and the mix comes back to the
//! bit; a swap while the reference is heard fades through the mix (no cut); **no render
//! ever holds the reference**, whatever the panel is set to; offline equals real time and
//! every block size gives the same bits; a fork carries on; a reference reads from a file
//! through the store. // fxc:comparatio

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::effects::{EffectMemory, MemoryNeed};
use cantus_engine::fx::comparatio::{t, R_SHORT};
use cantus_engine::fx::comparatio_store::{self, RefTrack};
use cantus_engine::fx::SongPosition;
use cantus_engine::EffectUnit;
use modfx::*;
use std::sync::Arc;
use support::*;

const SR: f32 = 48_000.0;

fn slot_of(spec: EffectSpec) -> EffectSlot {
    let mut p = demo_project();
    let mut s = p.make_effect(spec.kind);
    s.spec = spec;
    s
}

/// A reference: pink-ish noise at −14 dBFS and a 90 Hz tone, `seconds` long, unrelated to
/// any mix the tests play.
fn reference_audio(seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (seconds * SR) as usize;
    let mut seed = 0x1234_5678u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let mut lp = 0.0f32;
    let l: Vec<f32> = (0..n)
        .map(|i| {
            lp += 0.2 * (noise() - lp);
            0.4 * lp + 0.1 * cantus_dsp::math::sin(i as f32 / SR * 90.0 * std::f32::consts::TAU)
        })
        .collect();
    let r = l.iter().map(|v| v * 0.9).collect();
    (l, r)
}

fn track(path: &str, seconds: f32) -> Arc<RefTrack> {
    let (l, r) = reference_audio(seconds);
    comparatio_store::insert_for_test(path, SR as u32, l, r)
}

fn spec_with(path: Option<&str>, kv: &[(&str, f32)]) -> EffectSpec {
    let mut s = EffectSpec::new(EffectKind::Reference);
    s.ir = path.map(|p| Box::new(IrFile::new(p)));
    for (k, v) in kv {
        s.set(k, *v).unwrap();
    }
    s
}

fn playing(at: i64) -> SongPosition {
    SongPosition {
        playing: true,
        tempo_bpm: 120.0,
        tempo_now: 120.0,
        time_sig_num: 4,
        time_sig_den: 4,
        samples: at,
        project_samples: at,
        ..Default::default()
    }
}

/// A mix the unit hears: a 440 Hz tone at −18 dBFS swelling at 1 Hz.
fn mix_block(from: usize, n: usize) -> (Vec<f32>, Vec<f32>) {
    let l: Vec<f32> = (from..from + n)
        .map(|i| {
            let x = i as f32 / SR;
            0.125
                * cantus_dsp::math::sin(x * 440.0 * std::f32::consts::TAU)
                * (0.75 + 0.25 * cantus_dsp::math::sin(x * std::f32::consts::TAU))
        })
        .collect();
    let r = l.clone();
    (l, r)
}

/// Run `blocks` blocks of `n`; returns the output and the input.
fn run(u: &mut EffectUnit, from: usize, frames: usize, n: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut out, mut dry) = (Vec::new(), Vec::new());
    let mut at = from;
    while at < from + frames {
        let m = n.min(from + frames - at);
        let (mut l, mut r) = mix_block(at, m);
        dry.extend_from_slice(&l);
        u.set_position(&playing(at as i64));
        u.process(&mut l, &mut r);
        out.extend_from_slice(&l);
        at += m;
    }
    (out, dry)
}

fn supplied(spec: EffectSpec, rate: f32) -> EffectUnit {
    let mut u = EffectUnit::new(&slot_of(spec), rate);
    if let Some(MemoryNeed::Comparatio(n)) = u.memory_need() {
        let m = EffectMemory::build_live(MemoryNeed::Comparatio(n), rate);
        assert!(u.supply(m).is_none(), "taken");
    }
    u
}

#[test]
fn at_its_defaults_and_with_no_reference_it_touches_no_sample() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Reference);
    assert_null(
        &reference,
        &render_of(&p),
        0.0,
        "Comparatio at its defaults",
    );
    set_all(
        &mut p,
        fx,
        &[("listen", 1.0), ("match", 1.0), ("offset", 6.0)],
    );
    assert_null(
        &reference,
        &render_of(&p),
        0.0,
        "the reference asked for, none loaded",
    );
    let slot = slot_of(EffectSpec::new(EffectKind::Reference));
    assert_eq!(EffectUnit::new(&slot, SR).latency(), 0);
}

/// The reference supplied (the live engine's answer) and pressed: it is heard, matched to
/// the mix's integrated loudness within 0.5 LU after a few seconds; back on the mix, the
/// mix to the bit once the fade is over.
#[test]
fn a_reference_is_heard_at_the_mix_s_loudness_and_the_mix_comes_back_to_the_bit() {
    let _t = track("references/match.wav", 30.0);
    let spec = spec_with(Some("references/match.wav"), &[]);
    let mut u = supplied(spec.clone(), SR);
    assert!(u.memory_need().is_none(), "it has its reference");
    let (out, dry) = run(&mut u, 0, 48_000 * 4, 480);
    assert_eq!(
        out, dry,
        "the mix heard: untouched while the reference plays beside it"
    );
    let mut on = spec.clone();
    on.set("listen", 1.0).unwrap();
    u.configure(&on);
    let (out, dry) = run(&mut u, 48_000 * 4, 48_000 * 8, 480);
    assert_ne!(out[48_000..], dry[48_000..], "the reference is heard");
    let tel = u.telemetry().unwrap();
    assert_eq!(tel[t::HEARD], 1.0);
    // The reference's short-term loudness, as heard (after its gain), against the mix's.
    let mix_s = tel[t::MIX + R_SHORT];
    let ref_s = tel[t::REF + R_SHORT] + tel[t::GAIN];
    assert!((mix_s - ref_s).abs() < 0.5, "{mix_s} against {ref_s} LUFS");
    u.configure(&spec);
    let _ = run(&mut u, 48_000 * 12, 4_800, 480);
    let (out, dry) = run(&mut u, 48_000 * 12 + 4_800, 48_000, 480);
    assert_eq!(out, dry, "back on the mix: to the bit");
}

/// A swap while the reference is heard fades to the mix first: the new reference waits,
/// no sample steps more than the signals' own steps, and once the mix is heard the new one
/// is taken and the old let go (the store keeps its sibling).
#[test]
fn a_swap_while_the_reference_is_heard_fades_through_the_mix() {
    let a = track("references/swap a.wav", 20.0);
    let b = track("references/swap b.wav", 20.0);
    let heard = spec_with(Some("references/swap a.wav"), &[("listen", 1.0)]);
    let mut u = supplied(heard.clone(), SR);
    let _ = run(&mut u, 0, 48_000 * 2, 480);
    let mut swapped = heard.clone();
    swapped.ir = Some(Box::new(IrFile::new("references/swap b.wav")));
    u.configure(&swapped);
    let Some(MemoryNeed::Comparatio(n)) = u.memory_need() else {
        panic!("it asks for the new one")
    };
    assert_eq!(n.key, b.key);
    assert!(u
        .supply(EffectMemory::build_live(MemoryNeed::Comparatio(n), SR))
        .is_none());
    let (out, _) = run(&mut u, 48_000 * 2, 48_000, 64);
    let step = out.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
    assert!(step < 0.2, "no cut: the largest step {step}");
    assert_eq!(
        u.telemetry().unwrap()[t::HEARD],
        1.0,
        "the new one is heard"
    );
    assert!(
        Arc::strong_count(&a) >= 2,
        "the store keeps the old one's sibling"
    );
}

/// **No render holds the reference**: a Comparatio set to the reference, its reference in
/// the store, renders the dry project to the bit — through `render` straight from a
/// snapshot (its units' needs are answered without references), and through the engine's
/// compile for a render, which takes the reference away.
#[test]
fn no_render_ever_holds_the_reference() {
    let _t = track("references/never.wav", 30.0);
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Reference);
    let slot = strip_of(&mut p, 0)
        .effects
        .iter_mut()
        .find(|e| e.id == fx)
        .unwrap();
    slot.spec = spec_with(
        Some("references/never.wav"),
        &[("listen", 1.0), ("match", 2.0), ("offset", 12.0)],
    );
    assert_null(
        &dry,
        &render_of(&p),
        0.0,
        "render: the mix, never the reference",
    );
    // On the master too.
    let mut p = demo_project();
    let mut m = p.make_effect(EffectKind::Reference);
    m.spec = spec_with(Some("references/never.wav"), &[("listen", 2.0)]);
    p.master.effects.push(m);
    assert_null(&dry, &render_of(&p), 0.0, "on the master, blind X");
    // The compile for a render takes the reference away and sets the mix.
    let mut snap = Snapshot::compile(&p, 1);
    comparatio_store::prepare(&mut snap, &|s| s.into(), SR as u32, true, false);
    let s = &snap.master_effects.last().unwrap().spec;
    assert!(s.ir.is_none());
    assert_eq!(s.get("listen"), 0.0);
}

/// Every block size gives the same bits with the reference heard (the switch, the match
/// moving, a switch back), and a fork carries on as the unit does.
#[test]
fn block_sizes_and_forks_agree_to_the_bit() {
    let _t = track("references/blocks.wav", 20.0);
    let spec = spec_with(
        Some("references/blocks.wav"),
        &[("listen", 1.0), ("match", 1.0)],
    );
    let whole = |n: usize| {
        let mut u = supplied(spec.clone(), SR);
        let (a, _) = run(&mut u, 0, 48_000 * 3, n);
        let mut back = spec.clone();
        back.set("listen", 0.0).unwrap();
        u.configure(&back);
        let (b, _) = run(&mut u, 48_000 * 3, 48_000, n);
        [a, b].concat()
    };
    let want = whole(512);
    for n in [1usize, 37, 64, 4_096] {
        assert_eq!(whole(n), want, "blocks of {n}");
    }
    let mut a = supplied(spec.clone(), SR);
    let _ = run(&mut a, 0, 48_000 * 2, 480);
    let mut b = a.fork().expect("a fork");
    assert_eq!(
        run(&mut a, 96_000, 48_000, 480).0,
        run(&mut b, 96_000, 48_000, 480).0
    );
}

/// A reference read from a file through the store: a WAV the test writes, read at the
/// engine's rate (resampled from 44.1 kHz), its whole file analysed, named by the compile
/// once it is in; a missing file is reported and the snapshot names no reference.
#[test]
fn a_reference_reads_from_a_file_through_the_store() {
    let dir = std::env::temp_dir().join(format!("cantus-comparatio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Reference master.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..44_100 * 4 {
        let v = (0.25
            * cantus_dsp::math::sin(i as f32 / 44_100.0 * 300.0 * std::f32::consts::TAU)
            * 32_767.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
    let mut p = demo_project();
    let mut m = p.make_effect(EffectKind::Reference);
    let stored = path.to_string_lossy().to_string();
    m.spec = spec_with(Some(&stored), &[]);
    m.spec.ir.as_mut().unwrap().path = stored.clone();
    p.master.effects.push(m);
    let mut snap = Snapshot::compile(&p, 1);
    let errors = comparatio_store::prepare(&mut snap, &|s| s.into(), 48_000, false, false);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(
        snap.master_effects.last().unwrap().spec.ir.is_some(),
        "named once in"
    );
    let t = comparatio_store::track(&stored).expect("read");
    assert_eq!(t.rate, 48_000);
    assert!((t.seconds() - 4.0).abs() < 0.01, "{}", t.seconds());
    let lufs = t.whole_lufs().expect("analysed");
    // A 300 Hz sine at −12 dBFS peak on both channels: −0.691 + 10·log₁₀(2 · 0.25²/2)
    // = −12.73 LUFS, less K's −0.2 dB at 300 Hz.
    assert!((lufs + 12.9).abs() < 0.3, "{lufs} LUFS");
    // Missing: reported, and not named.
    let mut p2 = p.clone();
    p2.master.effects.last_mut().unwrap().spec.ir = Some(Box::new(IrFile::new(
        dir.join("missing.wav").to_string_lossy().to_string(),
    )));
    let mut snap = Snapshot::compile(&p2, 1);
    let errors = comparatio_store::prepare(&mut snap, &|s| s.into(), 48_000, false, false);
    assert_eq!(errors.len(), 1);
    assert!(snap.master_effects.last().unwrap().spec.ir.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Offline equals real time with the reference asked for (no render hears it: both are the
/// mix), and the defaults' rest is exact (the contract's twins hold it per kind).
#[test]
fn live_equals_offline() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Reference);
    set_all(&mut p, fx, &[("match", 1.0), ("sync", 1.0)]);
    let a = render_of(&p);
    let b = realtime(&p, a.len() / 2);
    assert_null(&a[..b.len()], &b, 0.0, "real time equals offline");
}

/// A stereo 16-bit WAV of a sine at `hz`, `amp` peak, `seconds` long at 44.1 kHz.
fn write_wav(path: &std::path::Path, hz: f32, amp: f32, seconds: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..(44_100.0 * seconds) as usize {
        let v = (amp
            * cantus_dsp::math::sin(i as f32 / 44_100.0 * hz * std::f32::consts::TAU)
            * 32_767.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

/// Phase B: a list of references — the one compared read for the unit, every other one read
/// once for its own summary (its own loudness, curves and overview), each keeping it; the
/// fingerprint tells a changed file from the one loaded; a missing file is said so.
#[test]
fn every_reference_in_the_list_keeps_its_own_analysis() {
    use cantus_core::fx::comparatio::{Reference, References};
    let dir = std::env::temp_dir().join(format!("cantus-comparatio-list-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let loud = dir.join("Loud master.wav");
    let quiet = dir.join("Quiet master.wav");
    write_wav(&loud, 300.0, 0.5, 3.0);
    write_wav(&quiet, 1_000.0, 0.05, 2.0);
    let (lp, qp) = (
        loud.to_string_lossy().to_string(),
        quiet.to_string_lossy().to_string(),
    );
    let mut p = demo_project();
    let mut m = p.make_effect(EffectKind::Reference);
    m.spec.refs = Some(Box::new(References {
        list: vec![Reference::new(lp.clone()), Reference::new(qp.clone())],
        eq: None,
    }));
    m.spec.ir = Some(Box::new(IrFile::new(lp.clone())));
    p.master.effects.push(m);
    let mut snap = Snapshot::compile(&p, 1);
    let errors = comparatio_store::prepare(&mut snap, &|s| s.into(), 48_000, false, false);
    assert!(errors.is_empty(), "{errors:?}");
    let a = comparatio_store::summary(&lp).expect("the one compared, summarised");
    let b = comparatio_store::summary(&qp).expect("the other, summarised too");
    let (la, lb) = (a.lufs.unwrap(), b.lufs.unwrap());
    // 20 dB apart in level, less K-weighting's tilt: +0.69 dB at 1 kHz, ~0 at 300 Hz.
    assert!(((la - lb) - 19.3).abs() < 0.3, "{la} against {lb}");
    assert!((a.seconds - 3.0).abs() < 0.01 && (b.seconds - 2.0).abs() < 0.01);
    assert_eq!(a.overview.len(), 240);
    assert!(a.overview.iter().all(|v| (0.0..=1.0).contains(v)));
    // Each curve's loudest band is its own tone's.
    let peak = |c: &cantus_dsp::comparatio::tonal::Curve| {
        let n = c.bands.count;
        let i = (0..n)
            .max_by(|x, y| c.db[*x].partial_cmp(&c.db[*y]).unwrap())
            .unwrap();
        c.bands.centre[i]
    };
    assert!(
        (peak(&a.mid[0]) / 300.0 - 1.0).abs() < 0.15,
        "{}",
        peak(&a.mid[0])
    );
    assert!(
        (peak(&b.mid[1]) / 1_000.0 - 1.0).abs() < 0.1,
        "{}",
        peak(&b.mid[1])
    );
    // The fingerprint: the file as it is, and a file changed in its middle is another.
    let (bytes, hash) = comparatio_store::fingerprint(&loud).unwrap();
    assert_eq!((a.bytes, a.hash.as_str()), (bytes, hash.as_str()));
    assert_eq!(bytes, std::fs::metadata(&loud).unwrap().len());
    let mut raw = std::fs::read(&loud).unwrap();
    let mid = raw.len() / 63 * 31 + 10;
    raw[mid] ^= 0x55;
    let changed = dir.join("Changed.wav");
    std::fs::write(&changed, &raw).unwrap();
    let (b2, h2) = comparatio_store::fingerprint(&changed).unwrap();
    assert_eq!(b2, bytes);
    assert_ne!(h2, hash, "a sample changed in the middle is seen");
    // Missing: a failure that names it.
    let gone = dir.join("Gone.wav");
    let e = comparatio_store::read(1, &gone, 48_000).unwrap_err();
    assert!(e.contains("not there") && e.contains("Gone.wav"), "{e}");
    // A plug-in's path: asked for on a thread of its own, waited for by the builder.
    let other = dir.join("Third.wav");
    write_wav(&other, 500.0, 0.2, 1.0);
    let op = other.to_string_lossy().to_string();
    let _ = comparatio_store::request(&op, &other, 44_100).unwrap();
    let key = cantus_engine::fx::aula_store::file_key(&op);
    let t = comparatio_store::wait_for(key, 44_100, std::time::Duration::from_secs(20))
        .expect("read on its thread and waited for");
    assert_eq!(t.rate, 44_100);
    assert!(comparatio_store::wait_for(99, 44_100, std::time::Duration::from_millis(1)).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
