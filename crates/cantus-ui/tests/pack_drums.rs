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
        label: String::new(),
    };
    let b = Build {
        kit: "kit.xml".into(),
        unpacked_size: 1,
        sfz: "Kit.sfz".into(),
        folder: "Samples".into(),
        gain_db: 0.0,
        mics: vec![mic("A", -50.0), mic("B", 50.0), mic("C", 0.0)],
        keys: vec![
            // One drum on two keys (as DRSKit's toms are): its turns are the drum's.
            key("Snare", &[38, 40]),
            key("HatClosed", &[42]),
            key("HatOpen", &[46]),
        ],
        chokes: vec![Choke {
            instruments: vec!["HatClosed".into(), "HatOpen".into()],
            release_ms: 30.0,
        }],
        round_robin: 2,
        range_db: 24.0,
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
    // A kit: the piano roll and the channel rack show a row per drum, by name.
    assert_eq!(spec.get("kit"), 1.0);
    let inst = InstrumentSpec::Native(spec);
    let clip = cantus_core::Project::new("k").make_clip(
        "c",
        cantus_core::Ticks::ZERO,
        cantus_core::Ticks::beats(4),
    );
    let axis = cantus_ui::views::piano_roll::Axis::build(&inst, &clip);
    let rows: Vec<(u8, String)> = (0..axis.len())
        .map(|i| (axis.pitch(i), axis.name(i)))
        .collect();
    assert_eq!(
        rows,
        [
            (38, "Snare \u{b7} D2".into()),
            (40, "Snare \u{b7} E2".into()),
            (42, "HatClosed".into()),
            (46, "HatOpen".into())
        ]
    );
    assert!(axis.pads(), "named rows, as a kit's");
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
fn a_drum_on_two_keys_and_an_accent_layer_take_their_own_turns() {
    // A roll alternating the snare's two keys (a General MIDI part reaching one drum on
    // both), then strokes with an accent on every fourth: no hit twice in a row on the
    // drum, and the accents themselves alternate.
    let f = installed();
    let spec = kit_spec(&f);
    let per = 12_000;
    let notes: Vec<(f64, u8, u8)> = (0..8)
        .map(|i| (i as f64 * 0.5, if i % 2 == 0 { 38 } else { 40 }, 100))
        .collect();
    let out = play(&spec, &notes, 0.5);
    let pitches: Vec<f32> = (0..8).map(|i| pitch(&out[i * per..])).collect();
    for w in pitches.windows(2) {
        assert!(
            (w[0] - w[1]).abs() > 10.0,
            "a hit repeated across the keys: {pitches:?}"
        );
    }
    let notes: Vec<(f64, u8, u8)> = (0..16)
        .map(|i| (i as f64 * 0.5, 38, if i % 4 == 0 { 127 } else { 40 }))
        .collect();
    let out = play(&spec, &notes, 0.5);
    let accents: Vec<f32> = (0..4).map(|i| pitch(&out[i * 4 * per..])).collect();
    for w in accents.windows(2) {
        assert!(
            (w[0] - w[1]).abs() > 10.0,
            "the accent repeated: {accents:?}"
        );
    }
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

/// A rock groove: four bars at 120 bpm, a crash on the one, eighth-note hats with an
/// accent, kick and snare, an open hat at the end of bar 2 and a tom fill in bar 4.
pub fn groove(vel: impl Fn(u8) -> u8) -> Vec<(f64, u8, u8)> {
    let mut v = vec![(0.0, 49, vel(110))];
    for bar in 0..4 {
        let b = bar as f64 * 4.0;
        v.push((b, 36, vel(112)));
        v.push((b + 2.5, 36, vel(100)));
        if bar < 3 {
            v.push((b + 2.0, 38, vel(118)));
        }
        v.push((b + 1.0, 38, vel(115)));
        v.push((b + 3.0, 38, vel(116)));
        for e in 0..8 {
            if bar == 1 && e == 7 {
                v.push((b + 3.5, 46, vel(100)));
            } else if bar < 3 || e < 4 {
                v.push((
                    b + e as f64 * 0.5,
                    42,
                    vel(if e % 2 == 0 { 92 } else { 70 }),
                ));
            }
        }
    }
    for (i, k) in [48u8, 48, 47, 47, 43, 43, 41, 41].iter().enumerate() {
        v.push((14.0 + i as f64 * 0.25, *k, vel(105 + i as u8 % 2 * 10)));
    }
    v
}

/// The real kit, built and installed (`CANTUS_DRSKIT`: the pack's folder, or a folder the
/// `drumkit_mix` example rendered): it imports with the GM keys, loads streaming, plays a
/// groove at the calibrated level with headroom at full strength, a real roll never plays
/// a sample twice in a row, and the groove is written to listen to (`CANTUS_SHOTS`).
#[test]
#[ignore]
fn the_real_drskit_plays_a_groove_at_the_calibrated_level() {
    let Ok(dir) = std::env::var("CANTUS_DRSKIT") else {
        eprintln!("CANTUS_DRSKIT is not set");
        return;
    };
    let pack = cantus_content::Manifest::bundled()
        .pack("drskit")
        .unwrap()
        .clone();
    let inst = pack.instruments[0].clone();
    let gain: f32 = std::env::var("CANTUS_GAIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(inst.gain_db);
    let choice = cantus_ui::views::samples_content::sounds::SoundChoice(
        cantus_content::InstalledInstrument {
            pack: "drskit".into(),
            pack_name: pack.name.clone(),
            path: Path::new(&dir).join(&inst.file),
            instrument: PackInstrument {
                gain_db: gain,
                ..inst
            },
            licence: pack.licence.clone(),
            author: "DrumGizmo".into(),
        },
    );
    let t0 = std::time::Instant::now();
    let imp = cantus_ui::views::samples_content::sounds::import_of(&choice).unwrap();
    eprintln!(
        "import: {} zones, {:?}, log {:?}",
        imp.zones.len(),
        t0.elapsed(),
        imp.log
    );
    assert!(
        imp.log.iter().all(|l| !l.contains("unsupported")),
        "{:?}",
        imp.log
    );
    let spec = InstrumentSpec::Native(cantus_ui::views::samples_content::sounds::spec_of(&imp));
    let own_cache = tempfile::tempdir().unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(own_cache.path().into());
    let mut cache = SampleCache::new();
    cache.set_streaming(true);
    let mut run = |notes: &[(f64, u8, u8)], bars: f64| {
        let mut s = Song::new("drskit", 120.0, 1);
        let part = s.part("drums", spec.clone(), colors::BASS, TrackIcon::Drums);
        s.feel(part, Feel::TIGHT);
        let ev: Vec<Ev> = notes
            .iter()
            .map(|(b, k, v)| Ev::new(*b, 0.25, *k, *v))
            .collect();
        s.clip(part, "c", 0.0, bars, &ev);
        let p = s.finish();
        let mut snap = cantus_core::Snapshot::compile(&p, 0);
        let t = std::time::Instant::now();
        let errors = cache.attach(&mut snap);
        assert!(errors.is_empty(), "{errors:?}");
        let load = t.elapsed();
        let mut seen = std::collections::HashSet::new();
        let bytes: usize = snap
            .tracks
            .iter()
            .flat_map(|t| t.assets.iter())
            .filter_map(|(_, d)| d.as_ref())
            .filter(|d| seen.insert(Arc::as_ptr(d) as usize))
            .map(|d| match cantus_engine::inst::stream::lookup(d) {
                Some(st) => st.heads.iter().flatten().map(|h| h.len() * 4).sum(),
                None => d.frames.len() * 4,
            })
            .sum();
        eprintln!(
            "in memory: {} MiB of sample heads ({} files)",
            bytes / 1_048_576,
            seen.len()
        );
        let t = std::time::Instant::now();
        let r = render(
            Arc::new(snap),
            &RenderOptions {
                tail_seconds: 2.0,
                ..Default::default()
            },
        )
        .unwrap();
        (r, load, t.elapsed())
    };
    let (r, load, took) = run(&groove(|v| v), 4.0);
    let secs = r.samples.len() as f64 / 2.0 / 48_000.0;
    let (lufs, peak) = (measure::lufs(&r), measure::true_peak_db(&r));
    eprintln!(
        "groove: first load {load:?}, render {took:?} for {secs:.1} s ({:.1}× real time); {lufs:.1} LUFS, true peak {peak:.1} dBFS",
        secs / took.as_secs_f64()
    );
    if let Ok(shots) = std::env::var("CANTUS_SHOTS") {
        let path = Path::new(&shots).join("drskit-groove.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for s in &r.samples {
            w.write_sample(*s).unwrap();
        }
        w.finalize().unwrap();
        eprintln!("wrote {}", path.display());
    }
    // Headroom (ADR 0039: no factory sound past 0 dBFS at full strength): the same
    // groove with every hit at 127, and kick, crash and a tom together on every sixteenth
    // of the fill. It sets the pack's gain (0 dB): the most the rule allows.
    let mut loud = groove(|_| 127);
    loud.extend((0..8).map(|i| (14.0 + i as f64 * 0.25, 49, 127)));
    loud.extend((0..8).map(|i| (14.0 + i as f64 * 0.25, 36, 127)));
    let (r, _, _) = run(&loud, 4.0);
    let peak = measure::true_peak_db(&r);
    eprintln!("everything at 127: true peak {peak:.1} dBFS");
    assert!(peak < 0.0, "headroom: {peak}");
    // A raw kit is peaky: at that gain an ordinary groove sits near the sampled piano
    // (Salamander −24.4 LUFS) and Lamina (−22.9), a little under them.
    assert!(
        (-28.0..=-24.0).contains(&lufs),
        "the groove's level: {lufs}"
    );
    // Every velocity layer of every drum has at least two hits taking turns in order
    // (`rr.order` 1): a roll never plays the same sample twice in a row.
    use cantus_core::inst::sampler::zf;
    let mut layers = std::collections::BTreeMap::new();
    for z in &imp.zones {
        let k = (z.get(zf::LO) as u8, z.get(zf::VLO) as u8);
        layers.insert(k, z.get(zf::SEQLEN));
    }
    eprintln!("{} velocity layers", layers.len());
    assert!(layers.values().all(|n| *n >= 2.0), "{layers:?}");
    assert_eq!(
        cantus_ui::views::samples_content::sounds::spec_of(&imp).get("rr.order"),
        1.0
    );
}

#[test]
fn a_kits_rows_read_in_the_interfaces_language() {
    let f = installed();
    let inst = kit_spec(&f);
    // A drum with no name of its own takes General MIDI's, in the interface's language.
    let mut unnamed = NativeSpec::new(cantus_core::InstrumentKind::Sampler);
    let z = cantus_core::inst::sampler::Zone::new(Some("C:/kit/s.wav".into()), 38, 38, 38);
    for (k, v) in z.values_for(0) {
        unnamed.set(&k, v).unwrap();
    }
    for (k, t) in z.texts_for(0) {
        unnamed.set_text(&k, Some(t)).unwrap();
    }
    unnamed.set("kit", 1.0).unwrap();
    let unnamed = InstrumentSpec::Native(unnamed);
    let clip = cantus_core::Project::new("k").make_clip(
        "c",
        cantus_core::Ticks::ZERO,
        cantus_core::Ticks::beats(4),
    );
    let names = |i: &InstrumentSpec| {
        let axis = cantus_ui::views::piano_roll::Axis::build(i, &clip);
        (0..axis.len()).map(|r| axis.name(r)).collect::<Vec<_>>()
    };
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    let (kit, gm) = (names(&inst), names(&unnamed));
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert_eq!(kit[0], "Caja \u{b7} D2", "{kit:?}");
    assert_eq!(gm, ["Caja acústica"]);
    assert_eq!(names(&unnamed), ["Acoustic Snare"]);
}

/// Sound review 2: the channel rack and the piano roll keep their kit rows built — and a
/// language switch rebuilds them (they kept the old language until the next edit).
#[test]
fn kit_rows_follow_a_language_switch_in_the_rack_and_the_piano_roll() {
    let mut unnamed = NativeSpec::new(InstrumentKind::Sampler);
    let z = cantus_core::inst::sampler::Zone::new(Some("C:/kit/s.wav".into()), 38, 38, 38);
    for (k, v) in z.values_for(0) {
        unnamed.set(&k, v).unwrap();
    }
    for (k, t) in z.texts_for(0) {
        unnamed.set_text(&k, Some(t)).unwrap();
    }
    unnamed.set("kit", 1.0).unwrap();
    let mut p = cantus_ui::starter_project();
    p.tracks[0].instrument = InstrumentSpec::Native(unnamed);
    let track = p.tracks[0].id;
    let clip = p.tracks[0].clips.first().map(|c| c.id);
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut a = support::app(p);
    a.selected_track = Some(track);
    a.selected_clip = clip;
    let ctx = support::ctx();
    let views = [
        |a: &mut cantus_ui::CantusApp, ui: &mut egui::Ui| {
            cantus_ui::views::channel_rack::show(a, ui)
        },
        |a: &mut cantus_ui::CantusApp, ui: &mut egui::Ui| cantus_ui::views::piano_roll::show(a, ui),
    ];
    for (i, view) in views.iter().enumerate() {
        if i == 1 && clip.is_none() {
            continue;
        }
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let en = support::paint(&ctx, egui::vec2(1400.0, 900.0), 2, |ui| view(&mut a, ui));
        assert!(en.contains("Acoustic Snare"), "view {i}: {en}");
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
        let es = support::paint(&ctx, egui::vec2(1400.0, 900.0), 2, |ui| view(&mut a, ui));
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        assert!(
            es.contains("Caja acústica"),
            "view {i} after the switch: {es}"
        );
        // The piano roll's note count too (review of fix3).
        if i == 1 {
            assert!(en.contains(" notes") && es.contains(" notas"), "{es}");
        }
    }
}
