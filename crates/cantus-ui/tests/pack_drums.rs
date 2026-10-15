//! A drum kit built at install, end to end (round 28: DrumGizmo's DRSKit, mixed to stereo
//! by `cantus-content`): the installed kit is offered on the picker's Drums shelf, choosing
//! it gives a track an Exemplar with its voices and a group per drum, and it plays as a kit
//! should — the velocity curve continuous across layers, a roll that never repeats a
//! sample, the open hi-hat silenced by the closed one. A synthetic three-microphone kit
//! in DrumGizmo's layout; nothing is downloaded.

mod support;

use cantus_content::kitbuild;
use cantus_content::manifest::{Build, Choke, KeyMap, Mic, PackInstrument, Shelf, Text};
use cantus_content::Receipt;
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, TrackIcon};
use cantus_engine::{render, RenderOptions, SampleCache};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use cantus_ui::picker::{PickChoice, PickPurpose};
use std::path::Path;
use std::sync::Arc;
use support::*;

mod measure;

const RATE: u32 = 44_100;

/// A three-channel float hit: A a decaying tone at `freq`, B the same at half, C noise.
fn hit(path: &Path, freq: f32, amp: f32, secs: f32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 3,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let n = (secs * RATE as f32) as usize;
    let mut seed = (freq * 1000.0) as u32 | 1;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let env = cantus_dsp::math::exp(-t * 3.0 / secs);
        let x = amp * env * cantus_dsp::math::sin(std::f32::consts::TAU * freq * t);
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as f32 / u32::MAX as f32 - 0.5) * 0.02 * env;
        for v in [0.5 * x, x, noise] {
            w.write_sample(v).unwrap();
        }
    }
    w.finalize().unwrap();
}

/// The instrument file of a DrumGizmo instrument whose hits are files 1…n.
fn instrument_xml(name: &str, n: usize) -> String {
    let mut x = format!("<instrument version=\"2.0\" name=\"{name}\"><samples>");
    for i in 1..=n {
        x += &format!("<sample name=\"{name}-{i}\" power=\"{i}\">");
        for (ch, fc) in [("B", 1), ("A", 2), ("C", 3)] {
            x += &format!(
                "<audiofile channel=\"{ch}\" file=\"samples/{i}.wav\" filechannel=\"{fc}\"/>"
            );
        }
        x += "</sample>";
    }
    x + "</samples></instrument>"
}

struct Fixture {
    _dir: tempfile::TempDir,
    content: std::path::PathBuf,
}

/// A kit of a snare (eight hits, each at its own pitch so a hit can be told from another,
/// 2 dB apart), a closed and an open hi-hat that silence each other, built as the install
/// builds it into an installed pack.
fn installed() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let kit = dir.path().join("kit");
    for i in 1..=8 {
        let amp = 0.5 * cantus_dsp::math::powf(10f32, -2.0 * (8 - i) as f32 / 20.0);
        hit(
            &kit.join(format!("Snare/samples/{i}.wav")),
            180.0 + 20.0 * i as f32,
            amp,
            0.4,
        );
    }
    for i in 1..=2 {
        hit(
            &kit.join(format!("HatClosed/samples/{i}.wav")),
            3_000.0 + i as f32,
            0.2,
            0.08,
        );
        hit(
            &kit.join(format!("HatOpen/samples/{i}.wav")),
            4_000.0 + i as f32,
            0.2,
            2.0,
        );
    }
    for (name, n) in [("Snare", 8), ("HatClosed", 2), ("HatOpen", 2)] {
        std::fs::write(
            kit.join(format!("{name}/{name}.xml")),
            instrument_xml(name, n),
        )
        .unwrap();
    }
    std::fs::write(
        kit.join("kit.xml"),
        "<drumkit name=\"T\"><channels><channel name=\"A\"/><channel name=\"B\"/><channel name=\"C\"/></channels><instruments>\
         <instrument name=\"Snare\" file=\"Snare/Snare.xml\"/>\
         <instrument name=\"HatClosed\" group=\"hihat\" file=\"HatClosed/HatClosed.xml\"/>\
         <instrument name=\"HatOpen\" group=\"hihat\" file=\"HatOpen/HatOpen.xml\"/>\
         </instruments></drumkit>",
    )
    .unwrap();
    let mic = |channel: &str, pan: f32| Mic {
        channel: channel.into(),
        gain_db: 0.0,
        pan,
        invert: false,
        own: vec![],
        bleed_db: 0.0,
    };
    let key = |name: &str, keys: &[u8]| KeyMap {
        instrument: name.into(),
        keys: keys.to_vec(),
        gain_db: 0.0,
    };
    let b = Build {
        kit: "kit.xml".into(),
        unpacked_size: 1,
        sfz: "Kit.sfz".into(),
        folder: "Samples".into(),
        gain_db: 0.0,
        mics: vec![mic("A", -50.0), mic("B", 50.0), mic("C", 0.0)],
        keys: vec![
            key("Snare", &[38]),
            key("HatClosed", &[42]),
            key("HatOpen", &[46]),
        ],
        chokes: vec![Choke {
            instruments: vec!["HatClosed".into(), "HatOpen".into()],
            release_ms: 30.0,
        }],
        round_robin: 2,
        range_db: 30.0,
        keep: vec![],
    };
    let content = dir.path().join("Content");
    let pack = content.join("fixture-kit");
    std::fs::create_dir_all(&pack).unwrap();
    let never = std::sync::atomic::AtomicBool::new(false);
    kitbuild::render(&b, &kit, &pack, "Fixture kit", &never, &mut |_, _| {}).unwrap();
    std::fs::write(pack.join("LICENSE"), "CC BY 4.0").unwrap();
    let r = Receipt {
        id: "fixture-kit".into(),
        version: 1,
        name: Text {
            en: "Fixture Kit".into(),
            es: "Batería de prueba".into(),
        },
        licence: "CC-BY-4.0".into(),
        licence_url: "https://creativecommons.org/licenses/by/4.0/legalcode".into(),
        licence_file: "LICENSE".into(),
        credits: vec![],
        attribution: None,
        instruments: vec![PackInstrument {
            id: "kit".into(),
            name: Text {
                en: "Fixture Drums".into(),
                es: "Batería de prueba".into(),
            },
            file: "Kit.sfz".into(),
            shelf: Shelf::Drums,
            layers: None,
            gain_db: 0.0,
            voices: Some(48),
        }],
        installed_at: 0,
        files: 1,
        bytes: 1,
    };
    std::fs::write(
        pack.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&r).unwrap(),
    )
    .unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    Fixture { _dir: dir, content }
}

fn app_with(f: &Fixture) -> cantus_ui::CantusApp {
    let mut a = app(cantus_ui::starter_project());
    a.samples.content_root = Some(f.content.clone());
    a.samples.manifest = Some(cantus_content::Manifest {
        format: 3,
        updated: "2026-10-15".into(),
        packs: vec![],
    });
    a
}

fn choice(a: &mut cantus_ui::CantusApp) -> PickChoice {
    let found = a.samples.content().instruments();
    a.picker_view.refresh(&a.plugins.db.clone(), &found);
    a.picker_view
        .device("pack:fixture-kit/kit")
        .expect("offered in the picker")
        .choice
        .clone()
}

/// The kit as a track's instrument, from the picker.
fn kit_spec(f: &Fixture) -> InstrumentSpec {
    let mut a = app_with(f);
    let c = choice(&mut a);
    a.apply_pick(PickPurpose::AddInstrument { index: None }, c);
    a.project().tracks.last().unwrap().instrument.clone()
}

/// Render `notes` (`(beat, key, velocity)`, a sixteenth long) at 120 bpm: left channel.
fn play(spec: &InstrumentSpec, notes: &[(f64, u8, u8)], tail: f64) -> Vec<f32> {
    let mut s = Song::new("kit", 120.0, 1);
    let part = s.part("drums", spec.clone(), colors::BASS, TrackIcon::Drums);
    s.feel(part, Feel::TIGHT);
    let ev: Vec<Ev> = notes
        .iter()
        .map(|(b, k, v)| Ev::new(*b, 0.25, *k, *v))
        .collect();
    s.clip(part, "c", 0.0, 4.0, &ev);
    let p = s.finish();
    let mut snap = cantus_core::Snapshot::compile(&p, 0);
    let errors = SampleCache::new().attach(&mut snap);
    assert!(errors.is_empty(), "{errors:?}");
    let r = render(
        Arc::new(snap),
        &RenderOptions {
            tail_seconds: tail,
            ..Default::default()
        },
    )
    .unwrap();
    r.samples.chunks_exact(2).map(|f| f[0]).collect()
}

fn sampler(spec: &InstrumentSpec) -> NativeSpec {
    match spec {
        InstrumentSpec::Native(n) if n.kind == InstrumentKind::Sampler => n.clone(),
        other => panic!("not an Exemplar: {other:?}"),
    }
}

#[test]
fn the_kit_is_on_the_drums_shelf_and_comes_with_its_voices_and_a_group_per_drum() {
    use cantus_ui::picker_catalog as catalog;
    let f = installed();
    let mut a = app_with(&f);
    let found = a.samples.content().instruments();
    a.picker_view.refresh(&a.plugins.db.clone(), &found);
    let d = a
        .picker_view
        .device("pack:fixture-kit/kit")
        .unwrap()
        .clone();
    assert_eq!(d.primary(), catalog::Category::Drums);
    assert!(catalog::score(&d, "drums").is_some());
    let spec = sampler(&kit_spec(&f));
    assert_eq!(spec.get("voices"), 48.0, "the pack's voices");
    assert_eq!(spec.get("rr.order"), 1.0, "round robin in order");
    let names: Vec<String> = (0..cantus_core::inst::sampler::MAX_GROUPS)
        .filter_map(|m| spec.text(&format!("g{m}.name")).map(str::to_string))
        .collect();
    assert_eq!(
        names,
        ["Snare", "HatClosed", "HatOpen"],
        "a group per drum, named"
    );
}

/// The frequency a hit sounds at, from its zero crossings over its first 50 ms.
fn pitch(x: &[f32]) -> f32 {
    let n = (RATE as usize / 20).min(x.len());
    let crossings = x[..n]
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    crossings as f32 * 20.0
}

#[test]
fn a_roll_never_plays_the_same_hit_twice_in_a_row() {
    let f = installed();
    let spec = kit_spec(&f);
    // Sixteen snare strokes at one velocity, a sixteenth apart (125 ms at 120 bpm): a
    // layer of two hits, so they must alternate.
    let notes: Vec<(f64, u8, u8)> = (0..16).map(|i| (i as f64 * 0.5, 38, 100)).collect();
    let out = play(&spec, &notes, 0.5);
    let per = 12_000; // half a beat at 48 kHz
    let pitches: Vec<f32> = (0..16).map(|i| pitch(&out[i * per..])).collect();
    for w in pitches.windows(2) {
        assert!((w[0] - w[1]).abs() > 10.0, "a hit repeated: {pitches:?}");
    }
    let mut distinct = pitches.clone();
    distinct.sort_by(f32::total_cmp);
    distinct.dedup_by(|a, b| (*a - *b).abs() < 10.0);
    assert_eq!(
        distinct.len(),
        2,
        "the layer's two hits in turn: {pitches:?}"
    );
}

#[test]
fn velocity_follows_one_curve_across_the_layers() {
    let f = installed();
    let spec = kit_spec(&f);
    // The loudness 30 ms in, for single strokes: SFZ's curve, 40·log10(v/127) dB.
    let level = |vel: u8| {
        let out = play(&spec, &[(0.0, 38, vel)], 0.3);
        let w = &out[1_200..1_200 + 960];
        measure::db((w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32).sqrt())
    };
    let top = level(127);
    for vel in [96u8, 64, 40] {
        let want = 40.0 * (vel as f32 / 127.0).log10();
        let got = level(vel) - top;
        // A layer's hits differ from the curve only by how their decays differ from the
        // loudest one's over the 50 ms they are measured on.
        assert!(
            (got - want).abs() < 1.5,
            "velocity {vel}: {got:.2} dB, curve {want:.2}"
        );
    }
}

#[test]
fn the_closed_hi_hat_silences_the_open_one() {
    let f = installed();
    let spec = kit_spec(&f);
    let open_alone = play(&spec, &[(0.0, 46, 110)], 0.5);
    let both = play(&spec, &[(0.0, 46, 110), (1.0, 42, 110)], 0.5);
    // A beat (24 000 frames) after the open hat, the closed one; 100 ms later the closed
    // hit (80 ms) is over, and the open one has faded over its 30 ms.
    let at = 24_000 + 4_800;
    let tail = |x: &[f32]| x[at..at + 4_800].iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(tail(&open_alone) > 0.01, "the open hat rings on alone");
    assert!(tail(&both) < 1e-4, "silenced: {}", tail(&both));
}
