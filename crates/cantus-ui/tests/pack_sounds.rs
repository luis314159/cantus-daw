//! A pack's instrument, end to end (round 26, the Salamander Grand Piano): an installed
//! pack that carries an SFZ is offered in the instrument picker, choosing it gives a track
//! an Exemplar playing the pack's FLAC samples in one undo step, its velocity layers
//! thinned as the pack asks, and what Exemplar cannot play (controller-triggered pedal
//! noises) is left off the keys and said in the log. A synthetic pack in Salamander's
//! layout; nothing is downloaded.

mod support;

use cantus_content::manifest::{PackInstrument, Shelf, Text};
use cantus_content::Receipt;
use cantus_core::inst::sampler::{zf, Trigger};
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, TrackIcon};
use cantus_engine::{render, RenderOptions, SampleCache};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::picker_catalog as catalog;
use cantus_ui::views::inst::sampler::sfz;
use cantus_ui::views::samples_content::sounds;
use std::path::Path;
use std::sync::Arc;
use support::*;

mod measure;

/// A one-second sine at `amp`, 48 kHz stereo, as a FLAC file.
fn flac(path: &Path, freq: f32, amp: f32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let rate = 48_000;
    let mut samples = Vec::with_capacity(rate * 2);
    for i in 0..rate {
        let v = amp * (i as f32 * freq / rate as f32 * std::f32::consts::TAU).sin();
        samples.push(v);
        samples.push(v);
    }
    let a = cantus_export::Audio {
        rate: rate as u32,
        channels: 2,
        samples,
    };
    let mut f = std::fs::File::create(path).unwrap();
    cantus_export::encode::flac(
        &mut f,
        &a,
        cantus_export::Depth::Int24,
        cantus_export::Dither::Off,
        &cantus_export::Tags::default(),
        "",
        &mut |_| true,
    )
    .unwrap();
}

/// Salamander's layout in small: `Piano.sfz` beside `samples/`, four velocity layers per
/// range, a release (hammer) sample, pedal noises on CC 64, and `readme.txt`.
const SFZ: &str = r#"
//Notes
<group> amp_veltrack=73 ampeg_release=1

<region> sample=samples/C4v1.flac lokey=58 hikey=62 lovel=1 hivel=40 pitch_keycenter=60
<region> sample=samples/C4v2.flac lokey=58 hikey=62 lovel=41 hivel=80 pitch_keycenter=60
<region> sample=samples/C4v3.flac lokey=58 hikey=62 lovel=81 hivel=100 pitch_keycenter=60
<region> sample=samples/C4v4.flac lokey=58 hikey=62 lovel=101 pitch_keycenter=60
//HammerNoise
<group> trigger=release pitch_keytrack=0 volume=-37 amp_veltrack=82 rt_decay=2

<region> sample=samples/rel60.flac lokey=60 hikey=60
//pedalAction

<group> group=1 hikey=-1 lokey=-1 on_locc64=126 on_hicc64=127 off_by=2 volume=-20

<region> sample=samples/pedalD1.flac lorand=0 hirand=0.5
<region> sample=samples/pedalD2.flac lorand=0.5 hirand=1
"#;

struct Fixture {
    _dir: tempfile::TempDir,
    content: std::path::PathBuf,
}

/// The fixture's folder goes with it — its streamed samples' cache too, never the shared
/// fallback: the streams let go of their files first, so Windows can delete them.
impl Drop for Fixture {
    fn drop(&mut self) {
        cantus_engine::inst::stream::hub().let_go_unheld();
        let dir = self._dir.path().to_path_buf();
        let t = std::time::Instant::now();
        while dir.exists() && t.elapsed() < std::time::Duration::from_secs(3) {
            if std::fs::remove_dir_all(&dir).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

/// The pack installed, with the receipt an install writes.
fn installed(layers: Option<u8>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let pack = content.join("fixture-piano");
    for (i, amp) in [0.05, 0.1, 0.2, 0.4].iter().enumerate() {
        flac(
            &pack.join(format!("samples/C4v{}.flac", i + 1)),
            261.63,
            *amp,
        );
    }
    flac(&pack.join("samples/rel60.flac"), 1000.0, 0.3);
    flac(&pack.join("samples/pedalD1.flac"), 80.0, 0.2);
    flac(&pack.join("samples/pedalD2.flac"), 90.0, 0.2);
    std::fs::write(pack.join("Piano.sfz"), SFZ).unwrap();
    std::fs::write(pack.join("readme.txt"), "CC BY 3.0").unwrap();
    let r = Receipt {
        id: "fixture-piano".into(),
        version: 1,
        name: Text {
            en: "Fixture Piano".into(),
            es: "Piano de prueba".into(),
        },
        licence: "CC-BY-3.0".into(),
        licence_url: "https://creativecommons.org/licenses/by/3.0/legalcode".into(),
        licence_file: "readme.txt".into(),
        credits: vec![],
        attribution: Some(cantus_content::Attribution {
            title: "Fixture Piano V3".into(),
            author: "Alex Example".into(),
            url: "https://example.org/piano".into(),
            line: String::new(),
            note: String::new(),
        }),
        instruments: vec![PackInstrument {
            id: "grand".into(),
            name: Text {
                en: "Fixture Grand".into(),
                es: "Cola de prueba".into(),
            },
            file: "Piano.sfz".into(),
            shelf: Shelf::Keys,
            layers,
            gain_db: 2.0,
            voices: None,
        }],
        installed_at: 0,
        files: 7,
        bytes: 1,
    };
    std::fs::write(
        pack.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&r).unwrap(),
    )
    .unwrap();
    // A cache folder of its own for anything this test streams.
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    Fixture { _dir: dir, content }
}

fn app_with(f: &Fixture) -> cantus_ui::CantusApp {
    let mut a = app(cantus_ui::starter_project());
    a.samples.content_root = Some(f.content.clone());
    a.samples.manifest = Some(cantus_content::Manifest {
        format: 2,
        updated: "2026-10-13".into(),
        packs: vec![],
    });
    a
}

#[test]
fn a_region_a_controller_triggers_becomes_a_zone_the_controller_starts() {
    let none = |_: &str| None;
    let meta = |_: &str| None;
    let imp = sfz::parse(SFZ, Path::new("C:/pack"), &none, &meta);
    assert_eq!(
        imp.zones.len(),
        7,
        "four layers, the release, two pedal noises"
    );
    let pedal: Vec<_> = imp
        .zones
        .iter()
        .filter(|z| z.sample.as_deref().unwrap_or("").contains("pedal"))
        .collect();
    assert_eq!(pedal.len(), 2);
    for (z, (rlo, rhi)) in pedal.iter().zip([(0.0, 0.5), (0.5, 1.0)]) {
        assert_eq!(Trigger::from_index(z.get(zf::TRIG)), Trigger::Controller);
        assert_eq!(z.get(zf::CC), 64.0);
        assert_eq!((z.get(zf::CCLO), z.get(zf::CCHI)), (126.0, 127.0));
        assert_eq!((z.get(zf::EXCL), z.get(zf::OFFBY)), (1.0, 2.0));
        assert_eq!(z.get(zf::GAIN), -20.0);
        assert_eq!((z.get(zf::RLO), z.get(zf::RHI)), (rlo, rhi));
        assert_eq!(z.lo(), z.hi(), "on one key of the map, not across it");
    }
    assert!(
        imp.log.iter().all(|l| !l.contains("on_locc")),
        "understood, not unsupported: {:?}",
        imp.log
    );
    // `lokey=-1` alone (neither a key nor a controller plays it) is left off, and said.
    let imp = sfz::parse(
        "<region> sample=a.wav lokey=-1 hikey=-1
<region> sample=b.wav key=60",
        Path::new("C:/pack"),
        &none,
        &meta,
    );
    assert_eq!(imp.zones.len(), 1);
    assert!(
        imp.log.iter().any(|l| l.contains("1 region")),
        "{:?}",
        imp.log
    );
    // Written back out, the trigger survives.
    let text = sfz::export(&imp_with_pedal(), &|p: &str| p.to_string());
    assert!(text.contains("on_locc64=126 on_hicc64=127"), "{text}");
}

fn imp_with_pedal() -> NativeSpec {
    let none = |_: &str| None;
    let meta = |_: &str| None;
    sounds::spec_of(&sfz::parse(SFZ, Path::new("C:/pack"), &none, &meta))
}

fn layers_sfz(n: u8) -> String {
    let mut s = String::from("<group> amp_veltrack=73\n");
    for range in [(21u8, 22u8, 21u8), (23, 25, 24)] {
        for v in 1..=n {
            let lo = 1 + (v as u32 - 1) * 127 / n as u32;
            let hi = v as u32 * 127 / n as u32;
            s.push_str(&format!(
                "<region> sample=s/{}v{v}.flac lokey={} hikey={} lovel={lo} hivel={hi} pitch_keycenter={}\n",
                range.2, range.0, range.1, range.2
            ));
        }
    }
    s.push_str("<group> trigger=release\n<region> sample=s/rel.flac lokey=21 hikey=25\n");
    s
}

#[test]
fn thinning_keeps_n_layers_the_loudest_among_them_and_every_velocity_covered() {
    let none = |_: &str| None;
    let meta = |_: &str| None;
    let mut imp = sfz::parse(&layers_sfz(16), Path::new("C:/p"), &none, &meta);
    assert_eq!(imp.zones.len(), 33);
    sounds::thin_layers(&mut imp, 8);
    let attack: Vec<_> = imp
        .zones
        .iter()
        .filter(|z| Trigger::from_index(z.get(zf::TRIG)) == Trigger::Attack)
        .collect();
    assert_eq!(attack.len(), 16, "eight layers in each of two ranges");
    for key in [21u8, 24] {
        for vel in 1..=127u8 {
            let n = attack
                .iter()
                .filter(|z| z.lo() <= key && key <= z.hi() && z.vlo() <= vel && vel <= z.vhi())
                .count();
            assert_eq!(n, 1, "key {key} velocity {vel}");
        }
        assert!(
            attack.iter().any(|z| z.lo() <= key
                && key <= z.hi()
                && z.sample.as_deref().unwrap().ends_with("v16.flac")),
            "the loudest layer is kept"
        );
        assert!(attack.iter().any(|z| z.lo() <= key
            && key <= z.hi()
            && z.sample.as_deref().unwrap().ends_with("v1.flac")));
    }
    assert!(
        imp.zones
            .iter()
            .any(|z| z.sample.as_deref().unwrap().ends_with("rel.flac")),
        "release samples are not layers"
    );
    assert!(
        imp.log.iter().any(|l| l.contains("8 of 16")),
        "{:?}",
        imp.log
    );
    // Asking for more layers than there are changes nothing.
    let mut all = sfz::parse(&layers_sfz(4), Path::new("C:/p"), &none, &meta);
    let before = all.zones.clone();
    sounds::thin_layers(&mut all, 8);
    assert_eq!(all.zones, before);
}

#[test]
fn an_installed_pack_instrument_is_in_the_picker_on_its_shelf() {
    let f = installed(Some(2));
    let mut a = app_with(&f);
    let found = a.samples.content().instruments();
    assert_eq!(found.len(), 1);
    a.picker_view.refresh(&a.plugins.db.clone(), &found);
    let d = a
        .picker_view
        .device("pack:fixture-piano/grand")
        .expect("offered once installed")
        .clone();
    assert_eq!(d.name, "Fixture Grand");
    assert_eq!(d.role, catalog::Role::Instrument);
    assert_eq!(d.primary(), catalog::Category::Keys);
    assert!(d.vendor.contains("Alex Example"), "{}", d.vendor);
    assert!(
        catalog::score(&d, "piano").is_some(),
        "found by its pack's name"
    );
    assert!(
        catalog::score(&d, "exemplar").is_some(),
        "and by what plays it"
    );
    // In Spanish, its Spanish name.
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    a.picker_view.refresh(&a.plugins.db.clone(), &found);
    assert_eq!(
        a.picker_view
            .device("pack:fixture-piano/grand")
            .unwrap()
            .name,
        "Cola de prueba"
    );
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    // Not installed: not offered.
    let empty = tempfile::tempdir().unwrap();
    let mut b = app(cantus_ui::starter_project());
    b.samples.content_root = Some(empty.path().to_path_buf());
    let none = b.samples.content().instruments();
    b.picker_view.refresh(&b.plugins.db.clone(), &none);
    assert!(b.picker_view.device("pack:fixture-piano/grand").is_none());
}

fn sound_choice(a: &mut cantus_ui::CantusApp) -> PickChoice {
    let found = a.samples.content().instruments();
    a.picker_view.refresh(&a.plugins.db.clone(), &found);
    a.picker_view
        .device("pack:fixture-piano/grand")
        .unwrap()
        .choice
        .clone()
}

fn sampler_of(a: &cantus_ui::CantusApp, track: cantus_core::TrackId) -> NativeSpec {
    match &a.project().track(track).unwrap().instrument {
        InstrumentSpec::Native(n) if n.kind == InstrumentKind::Sampler => n.clone(),
        other => panic!("not an Exemplar: {other:?}"),
    }
}

#[test]
fn choosing_it_gives_a_track_the_piano_in_one_undo_step() {
    let f = installed(Some(2));
    let mut a = app_with(&f);
    let choice = sound_choice(&mut a);
    let n = a.project().tracks.len();
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice.clone());
    assert_eq!(a.project().tracks.len(), n + 1);
    let t = a.project().tracks.last().unwrap();
    assert_eq!(t.name, "Fixture Grand");
    let id = t.id;
    let spec = sampler_of(&a, id);
    let zones: Vec<_> = (0..cantus_core::inst::sampler::MAX_ZONES)
        .filter_map(|i| cantus_core::inst::sampler::Zone::of(&spec, i))
        .collect();
    assert_eq!(
        zones.len(),
        5,
        "two layers (thinned from four), the release and the two pedal noises"
    );
    for z in &zones {
        let s = z.sample.as_deref().unwrap();
        assert!(Path::new(s).is_file(), "points at the installed pack: {s}");
    }
    assert_eq!(spec.get("gain"), 2.0, "the pack's calibration");
    a.undo();
    assert_eq!(a.project().tracks.len(), n, "one undo step");

    // On a track that is there: the same, its notes kept.
    let first = a.project().tracks[0].id;
    a.apply_pick(PickPurpose::ReplaceInstrument { track: first }, choice);
    assert_eq!(sampler_of(&a, first).kind, InstrumentKind::Sampler);
    a.undo();
    assert!(!matches!(
        &a.project().track(first).unwrap().instrument,
        InstrumentSpec::Native(n) if n.kind == InstrumentKind::Sampler
    ));
}

#[test]
fn it_plays_its_flac_samples_louder_as_the_keys_are_struck_harder() {
    let f = installed(None);
    let mut a = app_with(&f);
    let choice = sound_choice(&mut a);
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    let spec = a.project().tracks.last().unwrap().instrument.clone();
    let level = |vel: u8| {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let mut s = Song::new("pack", 120.0, 1);
        let part = s.part("piano", spec.clone(), colors::BASS, TrackIcon::Keys);
        s.feel(part, Feel::TIGHT);
        s.clip(part, "c", 0.0, 1.0, &[Ev::new(0.0, 1.0, 60, vel)]);
        let p = s.finish();
        let mut snap = cantus_core::Snapshot::compile(&p, 0);
        let errors = SampleCache::new().attach(&mut snap);
        assert!(errors.is_empty(), "{errors:?}");
        let r = render(
            Arc::new(snap),
            &RenderOptions {
                tail_seconds: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        measure::db(measure::rms(&r))
    };
    let (soft, mid, hard) = (level(20), level(90), level(127));
    assert!(soft > -80.0, "it sounds: {soft} dB");
    assert!(
        soft < mid && mid < hard,
        "{soft} < {mid} < {hard}: a harder strike picks a louder layer"
    );
}

/// The pack's pedal noises sound when the sustain pedal goes down (round 26: zones a
/// controller starts), from the installed FLAC files, and nothing sounds before.
#[test]
fn its_pedal_noise_sounds_when_the_pedal_goes_down() {
    let f = installed(None);
    let mut a = app_with(&f);
    let choice = sound_choice(&mut a);
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    let id = a.project().tracks.last().unwrap().id;
    let mut snap = cantus_core::Snapshot::compile(a.project(), 0);
    let errors = SampleCache::new().attach(&mut snap);
    assert!(errors.is_empty(), "{errors:?}");
    let t = snap.tracks.iter().find(|t| t.id == id).unwrap();
    let mut u = cantus_engine::inst::make(InstrumentKind::Sampler, 48_000.0);
    u.configure(t.instrument.native().unwrap(), &t.assets);
    let block = |u: &mut Box<dyn cantus_engine::inst::InstrumentUnit>| {
        let (mut l, mut r) = (vec![0.0f32; 4_800], vec![0.0f32; 4_800]);
        u.process(&mut l, &mut r);
        l.iter().map(|x| x * x).sum::<f32>()
    };
    assert_eq!(block(&mut u), 0.0, "silence before the pedal");
    u.controller(cantus_engine::inst::Controller::Sustain, 1.0);
    assert!(block(&mut u) > 1.0, "the pedal-down noise");
    assert_eq!(u.active_voices(), 1, "one of the two, by its random range");
}

/// The real Salamander Grand Piano, when it is on this machine (`CANTUS_SALAMANDER`: the
/// unpacked folder; `CANTUS_LAYERS`: how many velocity layers, default 8): how long it
/// takes to load, the memory its samples take, its level against ADR 0039's calibration
/// (a C-major triad at velocity 100 for two seconds, at unity; the dense riff's peak at
/// 127), and a phrase written to a WAV to listen to (`CANTUS_SHOTS`).
#[test]
#[ignore]
fn the_real_salamander_loads_plays_and_sits_at_the_calibrated_level() {
    let Ok(dir) = std::env::var("CANTUS_SALAMANDER") else {
        eprintln!("CANTUS_SALAMANDER is not set");
        return;
    };
    let layers: u8 = std::env::var("CANTUS_LAYERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    let gain: f32 = std::env::var("CANTUS_GAIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let choice = sounds::SoundChoice(cantus_content::InstalledInstrument {
        pack: "salamander-grand-piano".into(),
        pack_name: Text {
            en: "Salamander Grand Piano".into(),
            es: "Piano de cola Salamander".into(),
        },
        instrument: PackInstrument {
            id: "grand".into(),
            name: Text {
                en: "Salamander Grand Piano".into(),
                es: "Piano de cola Salamander".into(),
            },
            file: "SalamanderGrandPiano-V3+20200602.sfz".into(),
            shelf: Shelf::Keys,
            layers: (layers < 16).then_some(layers),
            gain_db: gain,
            voices: None,
        },
        path: Path::new(&dir).join("SalamanderGrandPiano-V3+20200602.sfz"),
        licence: "CC-BY-3.0".into(),
        author: "Alexander Holm".into(),
    });
    let t0 = std::time::Instant::now();
    let imp = sounds::import_of(&choice).unwrap();
    eprintln!(
        "import: {} zones, {:?}, log {:?}",
        imp.zones.len(),
        t0.elapsed(),
        imp.log
    );
    let spec = InstrumentSpec::Native(sounds::spec_of(&imp));
    let project = |evs: &[Ev], bars: f64| {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let mut s = Song::new("salamander", 120.0, 1);
        let part = s.part("piano", spec.clone(), colors::BASS, TrackIcon::Keys);
        s.feel(part, Feel::TIGHT);
        s.clip(part, "c", 0.0, bars, evs);
        s.finish()
    };
    // `CANTUS_STREAM=1`: stream from disk (ADR 0050), the cache files in
    // `CANTUS_STREAM_CACHE` (a temporary folder otherwise).
    let streaming = std::env::var("CANTUS_STREAM").is_ok_and(|v| v == "1");
    let own_cache = tempfile::tempdir().unwrap();
    match std::env::var("CANTUS_STREAM_CACHE") {
        Ok(d) => cantus_engine::inst::stream::hub().set_cache_dir(d.into()),
        Err(_) => cantus_engine::inst::stream::hub().set_cache_dir(own_cache.path().into()),
    }
    let mut cache = SampleCache::new();
    cache.set_streaming(streaming);
    eprintln!("streaming: {streaming}");
    let run = |cache: &mut SampleCache, evs: &[Ev], bars: f64, tail: f64| {
        let p = project(evs, bars);
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
                // A stream holds its heads, every level.
                Some(st) => st.heads.iter().flatten().map(|h| h.len() * 4).sum(),
                None => d.frames.len() * 4,
            })
            .sum();
        let r = render(
            Arc::new(snap),
            &RenderOptions {
                tail_seconds: tail,
                ..Default::default()
            },
        )
        .unwrap();
        (r, load, bytes)
    };
    // ADR 0039's staging: a C-major triad at velocity 100 for two seconds.
    let triad: Vec<Ev> = [60, 64, 67]
        .iter()
        .map(|&n| Ev::new(0.0, 4.0, n, 100))
        .collect();
    let (r, load, bytes) = run(&mut cache, &triad, 1.0, 0.0);
    eprintln!(
        "load (decode + mip-maps): {load:?}; samples in memory: {} MiB",
        bytes / 1_048_576
    );
    eprintln!(
        "triad at 100: {:.1} LUFS, true peak {:.1} dBFS (native instruments: median -18.7 LUFS)",
        measure::lufs(&r),
        measure::true_peak_db(&r)
    );
    // The headroom test's dense riff at 127.
    let line = [0, 0, 12, 0, 7, 0, 10, 12, 0, 0, 12, 3, 5, 7, 10, 12];
    let mut riff = Vec::new();
    for bar in 0..2 {
        for (i, step) in line.iter().enumerate() {
            riff.push(Ev::new(
                bar as f64 * 4.0 + i as f64 * 0.25,
                0.2,
                36 + step,
                127,
            ));
        }
        for beat in 0..4 {
            for n in [24, 28, 31, 36] {
                riff.push(Ev::new(bar as f64 * 4.0 + beat as f64, 0.9, 36 + n, 127));
            }
        }
    }
    let (r, _, _) = run(&mut cache, &riff, 2.0, 1.0);
    let peak = measure::true_peak_db(&r);
    eprintln!("dense riff at 127: true peak {peak:.1} dBFS (must stay below 0)");
    assert!(peak < 0.0);
    // A phrase to listen to: a scale soft to loud, then chords soft, medium and loud.
    let mut phrase = Vec::new();
    for (i, n) in [48, 50, 52, 53, 55, 57, 59, 60, 62, 64, 65, 67, 69, 71, 72]
        .iter()
        .enumerate()
    {
        phrase.push(Ev::new(i as f64 * 0.5, 0.45, *n, (30 + i * 6) as u8));
    }
    for (k, vel) in [(8.0, 35u8), (10.0, 80), (12.0, 120)] {
        for n in [36, 48, 55, 60, 64, 67, 72] {
            phrase.push(Ev::new(k, 1.8, n, vel));
        }
    }
    let (r, _, _) = run(&mut cache, &phrase, 4.0, 3.0);
    if streaming {
        let pool = cantus_engine::inst::stream::hub().pool().slots();
        eprintln!(
            "the stream pool after the renders: {pool} blocks, {} MiB",
            pool * cantus_dsp::exemplar::stream::BLOCK * 2 * 4 / 1_048_576
        );
    }
    if let Ok(shots) = std::env::var("CANTUS_SHOTS") {
        let path = Path::new(&shots).join(format!("salamander-{layers}-layers.wav"));
        let spec = hound::WavSpec {
            channels: r.channels,
            sample_rate: r.sample_rate,
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
}

/// *Sound packs…* names the instruments a pack plays as before the download, and once it
/// is installed offers a new track with each in one click.
#[test]
fn sound_packs_names_its_instruments_and_offers_a_track_once_installed() {
    let mut m = cantus_content::Manifest::parse(include_str!(
        "../../cantus-content/tests/fixtures/manifest-cc-by.json"
    ))
    .unwrap();
    let inst = PackInstrument {
        id: "grand".into(),
        name: Text {
            en: "Fixture Grand".into(),
            es: "Cola de prueba".into(),
        },
        file: "Piano.sfz".into(),
        shelf: Shelf::Keys,
        layers: Some(2),
        gain_db: 2.0,
        voices: None,
    };
    let piano = m
        .packs
        .iter_mut()
        .find(|p| p.id == "fixture-grand-piano")
        .unwrap();
    piano.instruments = vec![inst];
    let paint_window = |a: &mut cantus_ui::CantusApp| {
        a.samples.window.open = true;
        a.samples.window.selected = Some("fixture-grand-piano".into());
        let ctx = ctx();
        paint(&ctx, egui::vec2(1500.0, 900.0), 3, |ui| {
            let ctx = ui.ctx().clone();
            cantus_ui::views::samples_content::window(a, &ctx);
        })
    };
    let empty = tempfile::tempdir().unwrap();
    let mut a = app(cantus_ui::starter_project());
    a.samples.content_root = Some(empty.path().to_path_buf());
    a.samples.manifest = Some(m.clone());
    let before = paint_window(&mut a);
    assert!(
        before.contains("the instrument picker offers “Fixture Grand”"),
        "{before}"
    );
    assert!(!before.contains("New track with"), "{before}");
    // Installed (the fixture pack, renamed to the manifest's id).
    let f = installed(Some(2));
    let from = f.content.join("fixture-piano");
    let to = f.content.join("fixture-grand-piano");
    std::fs::rename(&from, &to).unwrap();
    let receipt = to.join(cantus_content::install::RECEIPT);
    let text = std::fs::read_to_string(&receipt)
        .unwrap()
        .replace("\"fixture-piano\"", "\"fixture-grand-piano\"");
    std::fs::write(&receipt, text).unwrap();
    let mut a = app(cantus_ui::starter_project());
    a.samples.content_root = Some(f.content.clone());
    a.samples.manifest = Some(m);
    let after = paint_window(&mut a);
    assert!(
        after.contains("In the instrument picker: “Fixture Grand”"),
        "{after}"
    );
    assert!(after.contains("New track with Fixture Grand"), "{after}");
}

/// Round 26 (ADR 0050 §7, the review's item 15): choosing the piano no longer stops the
/// window — its samples load on the loader threads (a slow decoder here: 300 ms a file)
/// while the frame that picked it returns at once, the status line and the instrument's
/// footer say how far it has got, in both languages, and once in, the zones play.
/// The slow decoder is process-wide: the two tests that use it run one at a time.
static SLOW: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn choosing_it_loads_off_the_interface_thread_and_shows_how_far() {
    let _slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let f = installed(None);
    let mut a = app_with(&f);
    a.engine.samples.set_background(true);
    a.engine.samples.set_streaming(true);
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::from_millis(300));
    let choice = sound_choice(&mut a);
    a.sync_engine();
    // The built-in sounds are computed once per process, beside the window at start-up
    // (`CantusApp::new`); here, before the clock starts.
    let _ = cantus_dsp::exemplar::builtin::builtins();
    let t = std::time::Instant::now();
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    a.sync_engine();
    let took = t.elapsed();
    assert!(
        // Seven files at 300 ms each: loaded here, it would take well over a second.
        took < std::time::Duration::from_millis(250),
        "the pick and its snapshot took {took:?}"
    );
    assert_eq!(
        a.engine.samples.progress(),
        Some((0, 7)),
        "seven files on their way"
    );
    let track = a.project().tracks.last().unwrap().id;
    let spec = sampler_of(&a, track);
    assert_eq!(
        cantus_ui::views::inst::sampler::loading(&a, &spec),
        Some((0, 7))
    );
    let mut chips = Vec::new();
    cantus_ui::views::inst::sampler::chips(
        &[0.0; cantus_engine::inst::TELEMETRY_LEN],
        Some((0, 7)),
        &mut chips,
        &mut Vec::new(),
    );
    assert!(chips[0].contains("Loading samples… 0 of 7"), "{chips:?}");
    let bar = |a: &mut cantus_ui::CantusApp| {
        paint(&ctx(), egui::vec2(1400.0, 22.0), 2, |ui| {
            cantus_ui::views::workspace_status::show(a, ui)
        })
    };
    let en = bar(&mut a);
    assert!(en.contains("Loading samples… 0 of 7"), "{en}");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let es = bar(&mut a);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(es.contains("Cargando muestras… 0 de 7"), "{es}");
    // The loads land; the frames pick them up.
    let t = std::time::Instant::now();
    while a.engine.samples.loading() && t.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        a.sync_engine();
    }
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::ZERO);
    assert_eq!(a.engine.samples.progress(), None);
    assert!(!bar(&mut a).contains("Loading samples"));
    let (p, rev) = (a.project().clone(), a.bus.revision());
    let snap = a.engine.compile(&p, rev);
    let t = snap.tracks.iter().find(|t| t.id == track).unwrap();
    assert!(
        t.assets.iter().all(|(_, d)| d.is_some()),
        "every zone has its sample"
    );
}

/// Undoing the pick while it loads leaves nothing behind: no progress, no error, the
/// loader idle; an export waits for a load still running and renders the whole piano.
#[test]
fn undo_during_the_load_lets_go_and_an_export_waits_for_it() {
    let _slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let f = installed(None);
    let mut a = app_with(&f);
    a.engine.samples.set_background(true);
    let choice = sound_choice(&mut a);
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::from_millis(200));
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice.clone());
    a.sync_engine();
    assert!(a.engine.samples.progress().is_some());
    a.undo();
    a.sync_engine();
    assert_eq!(
        a.engine.samples.progress(),
        None,
        "nothing asked for, nothing counted"
    );
    let t = std::time::Instant::now();
    while a.engine.samples.loading() && t.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        a.sync_engine();
    }
    assert!(!a.engine.samples.loading());
    // Picked again: the export's snapshot waits for the files still loading.
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    a.sync_engine();
    let track = a.project().tracks.last().unwrap().id;
    let (p, rev) = (a.project().clone(), a.bus.revision());
    let snap = a.engine.compile_for_render(&p, rev);
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::ZERO);
    let t = snap.tracks.iter().find(|t| t.id == track).unwrap();
    assert!(
        t.assets.iter().all(|(_, d)| d.is_some()),
        "the render waited"
    );
}

/// The texts a frame painted, with where they are and the clip they are drawn in.
fn placed(out: &egui::FullOutput) -> Vec<(String, egui::Rect, egui::Rect)> {
    fn walk(
        shape: &egui::Shape,
        clip: egui::Rect,
        out: &mut Vec<(String, egui::Rect, egui::Rect)>,
    ) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                t.galley.rect.translate(t.pos.to_vec2()),
                clip,
            )),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for s in &out.shapes {
        walk(&s.shape, s.clip_rect, &mut v);
    }
    v
}

/// Once the pack is installed, its main action — a new track with its instrument — is in
/// view without scrolling, on a 1280×720 screen at 100 % and at 200 % (640×360 points),
/// in both languages.
#[test]
fn the_new_track_button_is_in_view_without_scrolling() {
    let f = installed(Some(2));
    let m = cantus_content::Manifest {
        format: 2,
        updated: "2026-10-13".into(),
        packs: vec![],
    };
    for (locale, label) in [
        (cantus_ui::i18n::Locale::En, "New track with Fixture Grand"),
        (
            cantus_ui::i18n::Locale::Es,
            "Pista nueva con Cola de prueba",
        ),
    ] {
        for screen in [egui::vec2(1280.0, 720.0), egui::vec2(640.0, 360.0)] {
            cantus_ui::i18n::set_locale(locale);
            let mut a = app(cantus_ui::starter_project());
            a.samples.content_root = Some(f.content.clone());
            // The installed pack is listed from its receipt even when the manifest has
            // forgotten it; select it.
            a.samples.manifest = Some(m.clone());
            a.samples.window.open = true;
            let receipt: Receipt = serde_json::from_str(
                &std::fs::read_to_string(
                    f.content
                        .join("fixture-piano")
                        .join(cantus_content::install::RECEIPT),
                )
                .unwrap(),
            )
            .unwrap();
            let mut with_pack = m.clone();
            with_pack.packs.push(cantus_content::Pack {
                id: receipt.id.clone(),
                version: 1,
                name: receipt.name.clone(),
                description: Text {
                    en: "A sampled grand in four velocity layers, with its release samples, to play in Exemplar from the instrument picker or straight from here once it is installed.".into(),
                    es: "Un piano de cola muestreado en cuatro capas de velocidad, con sus muestras de relajación, para tocar en Exemplar desde el selector de instrumentos o desde aquí una vez instalado.".into(),
                },
                category: cantus_content::Category::Instruments,
                recommended: false,
                licence: receipt.licence.clone(),
                licence_url: receipt.licence_url.clone(),
                licence_file: receipt.licence_file.clone(),
                credits: vec![cantus_content::manifest::Credit {
                    name: "Fixture Piano — Alex Example".into(),
                    url: "https://example.org/piano".into(),
                    licence_url: "https://example.org/piano/LICENSE".into(),
                }],
                checked: "2026-10-13".into(),
                installed_size: 1,
                source: cantus_content::Source::TarGz(cantus_content::manifest::ZipSource {
                    url: "https://example.org/p.tar.gz".into(),
                    size: 1,
                    sha256: "1".repeat(64),
                    strip: String::new(),
                    include: vec![],
                    exclude: vec![],
                }),
                attribution: receipt.attribution.clone(),
                build: None,
                instruments: receipt.instruments.clone(),
            });
            a.samples.manifest = Some(with_pack);
            a.samples.window.selected = Some("fixture-piano".into());
            let ctx = ctx();
            let mut shown = Vec::new();
            for i in 0..4 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
                    time: Some(i as f64 * 0.25),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    let ctx = ui.ctx().clone();
                    cantus_ui::views::samples_content::window(&mut a, &ctx);
                });
                out.textures_delta.clear();
                shown = placed(&out);
            }
            let (_, rect, clip) = shown
                .iter()
                .find(|(s, _, _)| s == label)
                .unwrap_or_else(|| panic!("{locale:?} {screen:?}: no {label:?}"));
            assert!(
                clip.contains_rect(*rect)
                    && egui::Rect::from_min_size(egui::Pos2::ZERO, screen).contains_rect(*rect),
                "{locale:?} {screen:?}: {label:?} at {rect:?} is outside the visible pane {clip:?}"
            );
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// `on_loccN=-1` is SFZ's "no controller trigger": a keyed region carrying it plays from
/// its keys; `0` or more is a trigger.
#[test]
fn an_unassigned_controller_trigger_keeps_a_keyed_region() {
    let none = |_: &str| None;
    let meta = |_: &str| None;
    let imp = sfz::parse(
        "<region> sample=a.wav lokey=60 hikey=62 on_locc64=-1 on_hicc64=-1\n\
         <region> sample=b.wav lokey=60 hikey=62 on_locc64=0 on_hicc64=1",
        Path::new("C:/pack"),
        &none,
        &meta,
    );
    // The second is a zone CC 64 starts, crossing into 0…1 (round 26, the sampler).
    assert_eq!(imp.zones.len(), 2, "{:?}", imp.log);
    assert!(imp.zones[0].sample.as_deref().unwrap().ends_with("a.wav"));
    assert_eq!(
        Trigger::from_index(imp.zones[0].get(zf::TRIG)),
        Trigger::Attack
    );
    assert_eq!(
        Trigger::from_index(imp.zones[1].get(zf::TRIG)),
        Trigger::Controller
    );
    assert_eq!(
        (imp.zones[1].get(zf::CCLO), imp.zones[1].get(zf::CCHI)),
        (0.0, 1.0)
    );
    assert!(
        !imp.log.iter().any(|l| l.contains("unsupported opcode on_")),
        "an unassigned trigger is not reported as unsupported: {:?}",
        imp.log
    );
}

/// Review 24: the controller a region answers is the first one *assigned* (`>= 0`), however
/// the opcodes sort — an unassigned CC 1 from `<global>` does not hide the pedal's CC 64.
#[test]
fn the_first_assigned_controller_trigger_is_the_one_used() {
    let none = |_: &str| None;
    let meta = |_: &str| None;
    let imp = sfz::parse(
        "<global> on_locc1=-1 on_hicc1=-1\n\
         <region> sample=pedal.wav lokey=-1 hikey=-1 on_locc64=126 on_hicc64=127",
        Path::new("C:/pack"),
        &none,
        &meta,
    );
    assert_eq!(imp.zones.len(), 1, "{:?}", imp.log);
    let z = &imp.zones[0];
    assert_eq!(Trigger::from_index(z.get(zf::TRIG)), Trigger::Controller);
    assert_eq!(
        (z.get(zf::CC), z.get(zf::CCLO), z.get(zf::CCHI)),
        (64.0, 126.0, 127.0)
    );
}

/// Review 25: the inspector's small picture leaves controller zones off the keys, as the
/// editor's map does.
#[test]
fn the_inspector_preview_leaves_controller_zones_off_the_keys() {
    let none = |_: &str| None;
    let meta = |_: &str| None;
    let imp = sfz::parse(SFZ, Path::new("C:/pack"), &none, &meta);
    let spec = sounds::spec_of(&imp);
    let text = paint(&ctx(), egui::vec2(300.0, 120.0), 2, |ui| {
        cantus_ui::views::inst::sampler::preview(ui, &spec);
    });
    assert!(text.contains("5 zones"), "five zones on the keys: {text}");
}

/// Review 19: showing a streamed zone's waveform reads the file beside the window and
/// keeps it only while that zone is shown — the engine's cache never takes the whole file.
#[test]
fn a_streamed_zones_waveform_is_read_beside_the_window_and_not_kept() {
    let f = installed(None);
    let mut a = app_with(&f);
    a.engine.samples.set_streaming(true);
    let cache = tempfile::tempdir().unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(cache.path().to_path_buf());
    let choice = sound_choice(&mut a);
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    a.sync_engine();
    let track = a.project().tracks.last().unwrap().id;
    let spec = sampler_of(&a, track);
    let z0 = cantus_core::inst::sampler::Zone::of(&spec, 0).unwrap();
    let path = std::path::PathBuf::from(z0.sample.clone().unwrap());
    a.open_inst_editor(track);
    a.inst.view = cantus_ui::views::inst::View::Edit;
    a.inst.sampler.selected = Some(0);
    let paint_inst = |a: &mut cantus_ui::CantusApp| {
        paint(&ctx(), egui::vec2(1400.0, 1000.0), 1, |ui| {
            cantus_ui::views::inst::window_in(a, ui);
        })
    };
    let t = std::time::Instant::now();
    let first = paint_inst(&mut a);
    assert!(
        t.elapsed() < std::time::Duration::from_millis(500),
        "{:?}",
        t.elapsed()
    );
    assert!(first.contains("Reading the sample"), "{first}");
    let t = std::time::Instant::now();
    while cantus_ui::views::inst::sampler::wave_file_reading(&a)
        && t.elapsed() < std::time::Duration::from_secs(10)
    {
        std::thread::sleep(std::time::Duration::from_millis(10));
        paint_inst(&mut a);
    }
    let later = paint_inst(&mut a);
    assert!(!later.contains("cannot be read"), "{later}");
    assert!(
        a.engine.samples.whole(&path).is_none(),
        "the engine keeps only its stream"
    );
}

/// Review 16: an export asked for while the piano still loads does not stop the window: it
/// waits (the dialog's progress says for what), starts once the samples are in, and its
/// file holds the piano.
#[test]
fn an_export_while_loading_waits_beside_the_window() {
    let _slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let f = installed(None);
    let mut a = app_with(&f);
    a.engine.samples.set_background(true);
    let choice = sound_choice(&mut a);
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::from_millis(300));
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    a.sync_engine();
    let out = tempfile::tempdir().unwrap();
    let target = out.path().join("piano.wav");
    let t = std::time::Instant::now();
    a.export_to(target.clone());
    assert!(
        t.elapsed() < std::time::Duration::from_millis(250),
        "{:?}",
        t.elapsed()
    );
    assert!(a.export.waiting(), "it waits for the samples");
    assert_eq!(a.export.progress().map(|p| p.0), Some("smp.export_waiting"));
    let t = std::time::Instant::now();
    while a.export.waiting() && t.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        a.poll_export();
    }
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::ZERO);
    assert!(!a.export.waiting());
    // Second review #14: an export asked for without the dialog opens none when it starts.
    assert!(
        a.export.dialog.is_none(),
        "no dialog appeared after the wait"
    );
    cantus_ui::views::export::wait(&mut a);
    assert!(target.is_file(), "the export ran: {:?}", a.export.last);
    let r = hound::WavReader::open(&target).unwrap();
    assert!(r.len() > 0);
}

/// Review 18: with no audio device (the dummy engine) the window still loads instruments
/// beside itself — picking the piano returns at once and its files arrive later.
#[test]
fn without_a_device_instruments_still_load_beside_the_window() {
    let _slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let f = installed(None);
    let mut a = app_with(&f);
    assert!(!a.engine.is_live(), "no device");
    a.load_beside_the_window();
    assert!(a.engine.samples.streaming());
    let choice = sound_choice(&mut a);
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::from_millis(300));
    let t = std::time::Instant::now();
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    a.sync_engine();
    let took = t.elapsed();
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::ZERO);
    assert!(took < std::time::Duration::from_millis(250), "{took:?}");
    assert!(a.engine.samples.loading());
}

/// Review 11: Settings shows the sample cache — its limit, what it holds, *Clear cache* —
/// and the limit set there is the one the cache keeps to.
#[test]
fn settings_show_the_sample_cache_and_its_limit_reaches_the_cache() {
    // The hub's limit is the process's: the tests that set it take turns.
    let _slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let f = installed(None);
    let mut a = app_with(&f);
    // Settings' Sample cache page (round 30: Settings has pages).
    cantus_ui::views::workspace_settings::open(
        &mut a,
        cantus_ui::views::workspace_settings::SettingsPage::Cache,
    );
    a.settings.sample_cache_gb = Some(3.0);
    a.load_beside_the_window();
    assert_eq!(
        cantus_engine::inst::stream::hub().cache_limit(),
        3 * (1u64 << 30),
        "the setting is the cache's limit"
    );
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 3000.0),
        )),
        ..Default::default()
    };
    let mut text = String::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()));
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    for want in [
        "SAMPLE CACHE",
        "Size limit",
        "3GB",
        "Clear cache",
        "GB now",
        "first",
        "This disk",
        "GB free",
        "Automatic",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
    cantus_engine::inst::stream::hub().set_cache_auto();
}

/// Third review #10: the automatic limit reads whole in Spanish at the dialog's own width —
/// the limit in force on its line, the disk's numbers on the next — and the knob says
/// *Auto* rather than a rounded or clamped figure.
#[test]
fn the_automatic_cache_limit_reads_whole_in_spanish() {
    let _slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::Es);
    let f = installed(None);
    let mut a = app_with(&f);
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::Es);
    // Settings' Sample cache page (round 30: Settings has pages).
    cantus_ui::views::workspace_settings::open(
        &mut a,
        cantus_ui::views::workspace_settings::SettingsPage::Cache,
    );
    a.settings.sample_cache_gb = None;
    a.load_beside_the_window();
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let size = egui::vec2(1400.0, 3000.0);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    let mut text = String::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()));
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let auto = text
        .split(" | ")
        .find(|l| l.starts_with("Límite automático"))
        .unwrap_or_else(|| panic!("the automatic line: {text}"));
    assert!(
        !auto.contains('…') && auto.ends_with("5 GB libres)"),
        "whole: {auto}"
    );
    let disk = text
        .split(" | ")
        .find(|l| l.starts_with("Este disco"))
        .unwrap_or_else(|| panic!("the disk's line: {text}"));
    assert!(disk.ends_with("libres"), "whole: {disk}");
    assert!(text.contains(" | Auto | "), "the knob: {text}");
    if let Some(dir) = std::env::var_os("CANTUS_SHOT_DIR") {
        let p = std::path::PathBuf::from(dir).join("settings_cache_es.png");
        cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::Es);
        cantus_ui::raster::shoot(&ctx, egui::vec2(1400.0, 1400.0), 1.0, 2, Some(&p), |root| {
            a.dialogs(root.ctx());
        });
        cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    }
    cantus_engine::inst::stream::hub().set_cache_auto();
}

/// The instrument's preset field names what was loaded, not "Init": the pack instrument's
/// name, with its velocity layers when the pack thins them, in the interface's language.
#[test]
fn the_preset_field_names_the_pack_instrument() {
    let f = installed(Some(2));
    for (locale, want) in [
        (cantus_ui::i18n::Locale::En, "Fixture Grand · 2 layers"),
        (cantus_ui::i18n::Locale::Es, "Cola de prueba · 2 capas"),
    ] {
        cantus_ui::i18n::set_locale(locale);
        let mut a = app_with(&f);
        let choice = sound_choice(&mut a);
        a.apply_pick(PickPurpose::AddInstrument { index: None }, choice.clone());
        let added = a.project().tracks.last().unwrap().id;
        assert_eq!(
            a.inst.preset.get(&added).map(String::as_str),
            Some(want),
            "{locale:?}"
        );
        // In place of a track's instrument too.
        let first = a.project().tracks[0].id;
        a.apply_pick(PickPurpose::ReplaceInstrument { track: first }, choice);
        assert_eq!(
            a.inst.preset.get(&first).map(String::as_str),
            Some(want),
            "{locale:?}"
        );
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    // All its layers: its own name says so already.
    let f = installed(None);
    let mut a = app_with(&f);
    let choice = sound_choice(&mut a);
    a.apply_pick(PickPurpose::AddInstrument { index: None }, choice);
    let added = a.project().tracks.last().unwrap().id;
    assert_eq!(
        a.inst.preset.get(&added).map(String::as_str),
        Some("Fixture Grand")
    );
}
