//! Instrument gain staging (round 24, ADR 0039): Lamina and Exemplar's placeholder sat
//! ~8 dB under the other native instruments. A new instance is now as loud as the rest —
//! and a project saved before keeps its sound, to the bit:
//!
//! - `tests/data/v20-quiet-instruments.cantus` was written by the build before the change
//!   (schema 20); its render then hashed to [`OLD_RENDER`]. Opened now (migrated to 21),
//!   it renders to the same bits.
//! - The same instruments made new are louder by the calibration, and still pass no 0 dBFS
//!   on the headroom test's riff (`instrument_levels.rs`).

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, TrackIcon};
use cantus_engine::{render, RenderOptions, Rendered};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use std::path::PathBuf;
use std::sync::Arc;

mod measure;
use measure::{db, lufs};

/// The hash of the fixture's render by the build before the change (master cead440).
const OLD_RENDER: &str = "78d18d0c8901cfa1-240000";

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

fn triad() -> Vec<Ev> {
    [60u8, 64, 67]
        .iter()
        .enumerate()
        .map(|(i, &n)| Ev::new(i as f64 * 0.5, 3.0, n, 100))
        .collect()
}

/// Four parts: Lamina at its default output, Lamina with its gain moved (−6 dB) and a
/// modulation route on it, Exemplar with nothing loaded (its placeholder sine), and
/// Exemplar playing a built-in sound in a zone of its own.
fn quiet_instruments() -> Project {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut s = Song::new("Quiet instruments", 120.0, 2);
    let mut moved = NativeSpec::new(InstrumentKind::ElectricPiano);
    moved.set("gain", -6.0).unwrap();
    moved.set("macro1", 0.4).unwrap();
    moved.set_text("mod1.src", Some("macro1".into())).unwrap();
    moved.set_text("mod1.dst", Some("gain".into())).unwrap();
    moved.set("mod1.amt", 0.1).unwrap();
    let mut zoned = NativeSpec::new(InstrumentKind::Sampler);
    zoned.set("z0.gen", 3.0).unwrap();
    let parts = [
        (
            "Lamina",
            InstrumentSpec::Native(NativeSpec::new(InstrumentKind::ElectricPiano)),
        ),
        ("Lamina moved", InstrumentSpec::Native(moved)),
        (
            "Exemplar",
            InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler)),
        ),
        ("Exemplar zone", InstrumentSpec::Native(zoned)),
    ];
    for (name, spec) in parts {
        let part = s.part(name, spec, colors::KEYS, TrackIcon::Keys);
        s.feel(part, Feel::TIGHT);
        s.clip(part, "triad", 0.0, 1.0, &triad());
    }
    s.finish()
}

fn render_all(p: &Project) -> Rendered {
    let snap = cantus_core::Snapshot::compile(p, 0);
    render(
        Arc::new(snap),
        &RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .expect("it renders")
}

/// FNV-1a over the samples' bits: equal hashes, equal bits.
fn hash(samples: &[f32]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for s in samples {
        for b in s.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}-{}", samples.len())
}

/// Writes the fixture and prints its render's hash: run once, by the build *before* the
/// change, to make `tests/data/v20-quiet-instruments.cantus` and [`OLD_RENDER`].
#[test]
#[ignore]
fn write_the_old_fixture() {
    let p = quiet_instruments();
    let path = data("v20-quiet-instruments.cantus");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    cantus_project::save(&p, &path, "0.1.0-beta.1").unwrap();
    let back = cantus_project::load(&path, true).unwrap().project;
    let r = render_all(&back);
    println!("schema {}", cantus_project::SCHEMA_VERSION);
    println!("OLD_RENDER = {}", hash(&r.samples));
    println!("peak {:.2} dBFS, {:.2} LUFS", db(r.peak), lufs(&r));
}

#[test]
fn a_project_saved_before_renders_to_the_same_bits() {
    let loaded = cantus_project::load(&data("v20-quiet-instruments.cantus"), true).unwrap();
    assert_eq!(loaded.migrated_from, Some(20));
    // The migration wrote the old level down: every Lamina and Exemplar has a trim of 0.
    for t in &loaded.project.tracks {
        if let InstrumentSpec::Native(n) = &t.instrument {
            assert!(n.is_stored(cantus_core::inst::TRIM_KEY), "{}", t.name);
            assert_eq!(n.get(cantus_core::inst::TRIM_KEY), 0.0, "{}", t.name);
        }
    }
    assert_eq!(hash(&render_all(&loaded.project).samples), OLD_RENDER);
    // Saved again and reopened (schema 21 now), still the same bits.
    let dir = tempfile::tempdir().unwrap();
    let again = dir.path().join("again.cantus");
    cantus_project::save(&loaded.project, &again, "t").unwrap();
    let back = cantus_project::load(&again, true).unwrap();
    assert_eq!(back.migrated_from, None);
    assert_eq!(hash(&render_all(&back.project).samples), OLD_RENDER);
}

/// One part of `spec` playing the triad, rendered.
fn one(spec: NativeSpec) -> Rendered {
    let mut s = Song::new("one", 120.0, 2);
    let part = s.part(
        "p",
        InstrumentSpec::Native(spec),
        colors::KEYS,
        TrackIcon::Keys,
    );
    s.feel(part, Feel::TIGHT);
    s.clip(part, "triad", 0.0, 1.0, &triad());
    render_all(&s.finish())
}

#[test]
fn a_new_instance_is_louder_by_its_calibration_and_an_old_one_is_not() {
    use cantus_core::inst::TRIM_KEY;
    for (kind, cal) in [
        (InstrumentKind::ElectricPiano, 5.0f32),
        (InstrumentKind::Sampler, 8.0),
    ] {
        assert_eq!(kind.calibration_db(), cal, "{kind:?}");
        let new = NativeSpec::new(kind);
        assert!(!new.is_stored(TRIM_KEY), "a new instance stores nothing");
        assert_eq!(new.get(TRIM_KEY), cal);
        let mut old = NativeSpec::new(kind);
        old.set(TRIM_KEY, 0.0).unwrap();
        let (a, b) = (one(new), one(old));
        let rise = db(a.peak) - db(b.peak);
        assert!((rise - cal).abs() < 0.05, "{kind:?}: {rise:.3} dB");
        let lift = lufs(&a) - lufs(&b);
        assert!((lift - cal as f64).abs() < 0.1, "{kind:?}: {lift:.3} LU");
    }
    // Every other kind has no trim at all: nothing about it changed.
    for kind in InstrumentKind::ALL {
        if !matches!(
            kind,
            InstrumentKind::ElectricPiano | InstrumentKind::Sampler
        ) {
            assert_eq!(kind.calibration_db(), 0.0, "{kind:?}");
            assert!(kind.param(TRIM_KEY).is_none(), "{kind:?}");
        }
    }
}

#[test]
fn an_exemplar_with_something_loaded_is_not_lifted() {
    use cantus_core::inst::TRIM_KEY;
    let mut new = NativeSpec::new(InstrumentKind::Sampler);
    new.set("z0.gen", 3.0).unwrap();
    let mut old = new.clone();
    old.set(TRIM_KEY, 0.0).unwrap();
    assert_eq!(hash(&one(new).samples), hash(&one(old).samples));
}

#[test]
fn a_user_preset_saved_before_keeps_its_level() {
    use cantus_core::inst::TRIM_KEY;
    // A preset written before round 24: no trim in it.
    let old: NativeSpec =
        serde_json::from_str(r#"{"kind":"ElectricPiano","params":{"gain":-2.0}}"#).unwrap();
    let read = cantus_ui::views::inst::preset_from_file(old);
    assert_eq!(read.get(TRIM_KEY), 0.0);
    assert_eq!(read.get("gain"), -2.0);
    // A preset written now names its trim, so it reads back as it was saved.
    let now = NativeSpec::new(InstrumentKind::ElectricPiano);
    let written = cantus_ui::views::inst::preset_to_file(&now);
    assert_eq!(written.get(TRIM_KEY), 5.0);
    assert!(written.is_stored(TRIM_KEY));
    assert_eq!(
        cantus_ui::views::inst::preset_from_file(written).get(TRIM_KEY),
        5.0
    );
    // Kinds without a trim are left alone.
    let organ = NativeSpec::new(InstrumentKind::Organ);
    assert_eq!(cantus_ui::views::inst::preset_to_file(&organ), organ);
    assert_eq!(
        cantus_ui::views::inst::preset_from_file(organ.clone()),
        organ
    );
}

/// The demos whose keys are Lamina took the calibration out of their channel trims
/// (+8/+8/+9 → +3/+3/+4 dB): each sounds as it did — against the same demo rebuilt the
/// old way (trim 0, the channel 5 dB higher), eight bars through the whole mix and
/// master chain (Culmen, Terminus: gain decisions that magnify a rounding) differ by less
/// than −60 dB of the peak — float rounding, not a level.
#[test]
fn the_demos_with_lamina_sound_as_they_did() {
    use cantus_core::inst::TRIM_KEY;
    use cantus_core::Ticks;
    // The ballad played Lamina until round 28 gave its piano part to Chorda.
    for id in ["funk", "lofi"] {
        let new = (cantus_ui::demos::find(id).unwrap().build)();
        let mut old = new.clone();
        let mut lamina = 0;
        for t in &mut old.tracks {
            if let InstrumentSpec::Native(n) = &mut t.instrument {
                if n.kind == InstrumentKind::ElectricPiano {
                    n.set(TRIM_KEY, 0.0).unwrap();
                    t.gain_db += 5.0;
                    lamina += 1;
                }
            }
        }
        assert_eq!(lamina, 1, "{id}");
        let opts = RenderOptions {
            start: Ticks::beats(32),
            end: Some(Ticks::beats(64)),
            tail_seconds: 0.0,
            ..Default::default()
        };
        let r = |p: &Project| {
            render(Arc::new(cantus_core::Snapshot::compile(p, 0)), &opts).expect("renders")
        };
        let (a, b) = (r(&new), r(&old));
        assert_eq!(a.samples.len(), b.samples.len());
        let diff = a
            .samples
            .iter()
            .zip(&b.samples)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        let rel = db(diff) - db(a.peak);
        eprintln!(
            "{id}: peak {:.2} dBFS, largest difference {rel:.1} dB re the peak",
            db(a.peak)
        );
        assert!(rel < -60.0, "{id}: {rel:.1} dB");
    }
}
