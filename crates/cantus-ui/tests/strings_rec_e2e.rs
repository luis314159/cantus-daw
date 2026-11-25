//! r42:strings-rec — the recorded strings end to end (ADR 0158), through `CantusApp::assemble`
//! as the app starts: a strings pack is installed from a local file (*Install from a file…*:
//! the archive checked by its size and SHA-256, its instrument written as SFZ at install from
//! its measured zones — loops found by `strings_analysis` in the archive's own bytes), the
//! pack's instrument is put on the track as the picker puts it, and a four-bar line plays: a
//! legato pair (the second note slurred out of the first, no gap, gliding to its pitch), a
//! long note under a mod-wheel swell (its level rises with the wheel, continuously) and a
//! pizzicato by the note's own articulation.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_content::manifest::{
    Category, Credit, Pack, PackInstrument, Shelf, Source, Text, ZipSource,
};
use cantus_content::strings_analysis as an;
use cantus_content::stringsbuild::{StringsBuild, StringsInstrument, StringsZone};
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::note_art::{Articulation, NoteArt};
use cantus_core::{
    AutoPoint, InstrumentKind, InstrumentSpec, NativeSpec, Project, Snapshot, Ticks,
};
use cantus_engine::{render, EngineHandle, RenderOptions, SampleCache};
use cantus_ui::app::Files;
use cantus_ui::picker::PickPurpose;
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::Arc;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

const RATE: u32 = 48_000;

/// A bowed-like tone at `hz`: `secs` long, a 120 ms attack, a slow 5 Hz shimmer, partials
/// falling `bright` dB less an octave the louder it is, decaying `decay` per second (0: held).
fn tone(hz: f64, amp: f32, bright: f32, secs: f32, decay: f32) -> Vec<f32> {
    let n = (RATE as f32 * secs) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let mut v = 0.0f64;
            for k in 1..=8 {
                let g = 10f64.powf(-(k as f64).log2() * (12.0 - bright as f64) / 20.0);
                v += g * (std::f64::consts::TAU * hz * k as f64 * t).sin();
            }
            let a = (t / 0.12).min(1.0) * (1.0 + 0.03 * (std::f64::consts::TAU * 5.0 * t).sin());
            let d = if decay > 0.0 {
                (-decay as f64 * t).exp()
            } else {
                1.0
            };
            (v * a * d * amp as f64 * 0.4) as f32
        })
        .collect()
}

fn wav_bytes(x: &[f32]) -> Vec<u8> {
    let mut c = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
    for v in x {
        w.write_sample((v.clamp(-1.0, 1.0) * 8_388_607.0) as i32)
            .unwrap();
    }
    w.finalize().unwrap();
    c.into_inner()
}

/// The pack: a small cello in the Bigcat's layout (sustains at two dynamics, both bows, on
/// C3 and G3; a pizzicato on C3), its zip in `dir`, its manifest entry built from what the
/// analysis measures in those bytes.
fn pack(dir: &Path) -> (Pack, std::path::PathBuf) {
    let mut files: Vec<(String, Vec<u8>)> =
        vec![("Tiny_cello/LICENSE".into(), b"CC0 1.0 Universal".to_vec())];
    let mut zones = Vec::new();
    for (name, key) in [("C3", 48u8), ("G3", 55)] {
        let hz = an::hz_of_key(key as f64);
        for (dynamic, amp, bright, xf) in [
            ("p", 0.25f32, 0.0f32, [0u8, 0, 0, 127]),
            ("f", 1.0, 5.0, [0, 127, 127, 127]),
        ] {
            for bow in ["d", "g"] {
                let x = tone(hz, amp, bright, 3.0, 0.0);
                let lp = an::find_loop(&x, 1, RATE as f32, hz).expect("a loop in a held tone");
                let rel = format!("Samples/sus/{name}_{dynamic}_{bow}.wav");
                files.push((format!("Tiny_cello/{rel}"), wav_bytes(&x)));
                zones.push(StringsZone {
                    file: rel,
                    art: "sustain".into(),
                    bow: Some(if bow == "d" { "down" } else { "up" }.into()),
                    lo: if key == 48 { 40 } else { 52 },
                    hi: if key == 48 { 51 } else { 62 },
                    root: key,
                    tune: 0.0,
                    xf,
                    looped: Some([lp.ls, lp.le]),
                    xf_ms: lp.xf as f32 / RATE as f32 * 1000.0,
                    ..StringsZone::default()
                });
            }
        }
    }
    let pizz = tone(an::hz_of_key(48.0), 0.8, 3.0, 1.0, 6.0);
    files.push(("Tiny_cello/Samples/pizz/C3_1.wav".into(), wav_bytes(&pizz)));
    zones.push(StringsZone {
        file: "Samples/pizz/C3_1.wav".into(),
        art: "pizzicato".into(),
        lo: 40,
        hi: 62,
        root: 48,
        xf: [0, 0, 127, 127],
        ..StringsZone::default()
    });
    let zip_path = dir.join("tiny-cello.zip");
    let mut z = cantus_project::dawproject::zip::ZipWriter::new();
    for (name, bytes) in &files {
        z.add(name, bytes, true);
    }
    let bytes = z.finish();
    std::fs::write(&zip_path, &bytes).unwrap();
    let sha = cantus_content::fetch::sha256_file(&zip_path).unwrap();
    let text = |en: &str| Text {
        en: en.into(),
        es: en.into(),
    };
    let p = Pack {
        id: "tiny-cello".into(),
        version: 1,
        name: text("Tiny Cello"),
        description: text("A test pack."),
        category: Category::Instruments,
        recommended: false,
        licence: "CC0-1.0".into(),
        licence_url: "https://creativecommons.org/publicdomain/zero/1.0/legalcode".into(),
        licence_file: "LICENSE".into(),
        credits: vec![Credit {
            name: "Cantus's tests".into(),
            url: "https://example.org".into(),
            licence_url: "https://creativecommons.org/publicdomain/zero/1.0/legalcode".into(),
        }],
        checked: "2026-11-22".into(),
        installed_size: bytes.len() as u64 * 2,
        source: Source::Zip(ZipSource {
            url: "https://example.org/tiny-cello.zip".into(),
            size: bytes.len() as u64,
            sha256: sha,
            strip: "Tiny_cello/".into(),
            include: vec!["Samples/**".into(), "LICENSE".into()],
            exclude: vec![],
        }),
        attribution: None,
        instruments: vec![PackInstrument {
            id: "cello".into(),
            name: text("Tiny cello"),
            file: "Cantus/cello.sfz".into(),
            shelf: Shelf::Samplers,
            layers: None,
            gain_db: 0.0,
            voices: Some(48),
            plays: Default::default(),
        }],
        build: None,
        mode: Default::default(),
        author_page: None,
        piano: None,
        strings: Some(StringsBuild {
            instruments: vec![StringsInstrument {
                sfz: "Cantus/cello.sfz".into(),
                name: "Tiny cello".into(),
                switch: Some(24),
                like: None,
                default_art: None,
                legato: Some([60.0, 50.0, 100.0]),
                dynamics_cc: 1,
                gain_db: 0.0,
                release_ms: [300.0, 200.0],
                zones,
            }],
        }),
    };
    (p, zip_path)
}

/// The line: C3 → G3 slurred (overlapping by 30 ms), then E3 held two bars under a mod-wheel
/// swell (0.1 → 1.0), then a pizzicato C3 by its own articulation.
fn song() -> Project {
    let mut p = Project::new("strings");
    let mut t = p.make_track("Cello");
    t.strip = Some(p.strips[0].id);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler));
    let mut c = p.make_clip("Line", Ticks::ZERO, Ticks::beats(16));
    let overlap = Ticks(58);
    for (beat, len, key, art) in [
        (0, Ticks::beats(2) + overlap, 48u8, None),
        (2, Ticks::beats(2), 55, None),
        (4, Ticks::beats(8), 52, None),
        (
            12,
            Ticks::beats(1),
            48,
            Some(NoteArt {
                articulation: Articulation::Pizzicato,
                bow: None,
                start: None,
                part: None,
                string: None,
            }),
        ),
    ] {
        let mut n = p.make_note(Ticks::beats(beat), len, key, 90);
        n.art = art;
        c.notes.push(n);
    }
    c.lanes.push(ClipLane::new(
        LaneKind::Cc(1),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.5),
            AutoPoint::new(Ticks::beats(4), 0.1),
            AutoPoint::new(Ticks::beats(12), 1.0),
        ],
    ));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn master(project: &Project) -> Vec<f32> {
    let mut snap = Snapshot::compile(project, 1);
    let errors = SampleCache::new().attach(&mut snap);
    assert!(errors.is_empty(), "{errors:?}");
    render(
        Arc::new(snap),
        &RenderOptions {
            end: Some(Ticks::beats(16)),
            tail_seconds: 0.3,
            block_frames: 333,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

/// Mono, and the RMS of `[a, b)` seconds.
fn mono(x: &[f32]) -> Vec<f32> {
    x.chunks(2).map(|f| 0.5 * (f[0] + f[1])).collect()
}

fn rms(x: &[f32], a: f32, b: f32) -> f32 {
    let r = RATE as f32;
    let s = &x[(a * r) as usize..((b * r) as usize).min(x.len())];
    (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

#[test]
fn a_strings_pack_from_a_file_plays_a_legato_pair_a_mod_wheel_swell_and_a_pizzicato() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    std::fs::create_dir_all(&content).unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    let (pack, zip) = pack(dir.path());
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(RATE as f64);
    let mut app = CantusApp::assemble(&ctx, files, Some(song()), engine, None, None, None, false);
    app.samples.content_root = Some(content.clone());
    let manifest = cantus_content::Manifest {
        format: cantus_content::manifest::FORMAT,
        updated: "2026-11-22".into(),
        packs: vec![pack],
    };
    manifest.validate().expect("a valid manifest");
    app.samples.manifest = Some(manifest);

    // Install from a file, as the Content window's button does.
    assert!(app.samples.content().install_from_file("tiny-cello", &zip));
    let done = app
        .samples
        .content()
        .wait_idle(std::time::Duration::from_secs(120));
    assert!(!done.is_empty(), "the install finished");
    let sfz = content.join("tiny-cello").join("Cantus").join("cello.sfz");
    let text = std::fs::read_to_string(&sfz).expect("the SFZ written at install");
    assert!(
        text.contains("loop_mode=loop_continuous") && text.contains("xfin_locc1=0 xfin_hicc1=127")
    );

    // The pack's instrument on the track, as the picker puts it.
    let track = app.project().tracks[0].id;
    let choice = app
        .sound_choices()
        .into_iter()
        .find(|c| c.0.instrument.id == "cello")
        .expect("the pack's instrument offered");
    assert!(app.apply_sound(PickPurpose::ReplaceInstrument { track }, &choice));
    let spec = app.project().tracks[0].instrument.native().unwrap().clone();
    assert_eq!(spec.kind, InstrumentKind::Sampler);
    assert_eq!(spec.get("legato"), 1.0, "the scripted legato on");

    let out = mono(&master(app.project()));
    let beat = 0.5f32;
    // It plays all along.
    for b in [0.5, 1.5, 2.5, 5.0, 8.0, 11.0, 12.2] {
        assert!(
            rms(&out, b * beat, b * beat + 0.1) > 1e-3,
            "silent at beat {b}"
        );
    }
    // The legato pair: no gap through the slur, and G3 once it has glided.
    let before = rms(&out, 1.5 * beat, 1.9 * beat);
    for k in 0..20 {
        let a = 2.0 * beat - 0.05 + k as f32 * 0.01;
        let r = rms(&out, a, a + 0.01);
        assert!(r > before * 0.3, "a gap at {a:.2} s: {r} of {before}");
    }
    let (a, b) = (
        (2.0 * beat + 0.35) * RATE as f32,
        (4.0 * beat - 0.05) * RATE as f32,
    );
    let heard = an::nsdf_cents(
        &out,
        RATE as f32,
        a as usize,
        b as usize,
        an::hz_of_key(55.0),
    )
    .unwrap();
    assert!(
        (heard - 5_500.0).abs() < 5.0,
        "G3 after the slur: {heard} ct"
    );
    // The swell: the held E3's level rises beat by beat with the wheel.
    let levels: Vec<f32> = (5..12)
        .map(|b| rms(&out, b as f32 * beat, b as f32 * beat + 0.2))
        .collect();
    for w in levels.windows(2) {
        assert!(w[1] > w[0], "the level follows the wheel up: {levels:?}");
    }
    assert!(
        levels[6] > levels[0] * 1.4,
        "piano towards forte: {levels:?}"
    );
    // The pizzicato by its own articulation: it decays, where a sustain would hold.
    let (early, late) = (
        rms(&out, 12.0 * beat + 0.02, 12.0 * beat + 0.1),
        rms(&out, 13.6 * beat, 13.9 * beat),
    );
    assert!(late < early * 0.3, "a plucked decay: {early} then {late}");
}
