//! MTS-ESP end to end (round 42, ADR 0156; the mid-round review's reach 6 and correctness
//! 1, 2, 3), on the boot path: an app made by `CantusApp::assemble` with the real engine
//! handle and the fake `LIBMTS` the `cantus-rt` tests build (the same C symbols, compiled
//! here with `rustc`), handed to the app as the boot hands it the standard library.
//!
//! - **Cantus follows a master**: a song in the master's tuning exports at the master's
//!   table; when the master changes its table the next sync recompiles (the live snapshot
//!   holds the new table) and the export follows; after the engine is replaced (an audio
//!   restart's `adopt_engine`) it still follows.
//! - **Cantus is the master**: *Be the master* publishes the project's table to a client,
//!   and an edit publishes the new one; a hosted plug-in left at *Tuning by: MTS-ESP* (an
//!   MTS-ESP client reads the table itself) is sent no per-note tuning — it is not tuned
//!   twice — and set to *note expression* it is sent the whole table (the CLAP test synth,
//!   which reads per-note tuning and not MTS-ESP, plays it then).
//!
//! The fake library is one per process: the tests take turns. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder. r42 fix-engine

use cantus_core::external::TuningBy;
use cantus_core::tuning::{Temperament, Tuning, TuningSystem, TuningTable};
use cantus_core::*;
use cantus_engine::mts::MtsLink;
use cantus_engine::tuning::ExternalTuning;
use cantus_engine::EngineHandle;
use cantus_host::{Backend, DbEntry, ScanStatus};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::tuning;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

const SR: f64 = 48_000.0;
/// The keys each track plays (60 BPM: 2 s a key).
const KEYS: [u8; 3] = [57, 63, 70];

fn data() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn turn() -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn rustc() -> std::ffi::OsString {
    if let Some(r) = std::env::var_os("RUSTC") {
        return r;
    }
    if let Some(cargo) = std::env::var_os("CARGO") {
        let beside =
            PathBuf::from(cargo).with_file_name(if cfg!(windows) { "rustc.exe" } else { "rustc" });
        if beside.is_file() {
            return beside.into_os_string();
        }
    }
    "rustc".into()
}

/// The fake `LIBMTS`, built once per run (`crates/cantus-rt/tests/fake_mts/fake_mts.rs`).
fn fake() -> &'static PathBuf {
    static LIB: OnceLock<PathBuf> = OnceLock::new();
    LIB.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fake-mts-ui");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(if cfg!(windows) {
            "fake_mts.dll"
        } else if cfg!(target_os = "macos") {
            "libfake_mts.dylib"
        } else {
            "libfake_mts.so"
        });
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../cantus-rt/tests/fake_mts/fake_mts.rs");
        let out = std::process::Command::new(rustc())
            .args(["--edition", "2021", "--crate-type", "cdylib"])
            .args(["--crate-name", "fake_mts", "-C", "opt-level=1", "-o"])
            .arg(&file)
            .arg(&src)
            .output()
            .expect("rustc runs");
        assert!(
            out.status.success(),
            "the fake LIBMTS did not build:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        file
    })
}

/// The CLAP test instrument, built from source as the host's tests build it.
fn test_synth() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let target = root.join("target").join("test-plugins");
    let file = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap_synth.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap_synth.dylib"
    } else {
        "libcantus_test_clap_synth.so"
    });
    if std::env::var_os("CANTUS_TEST_PLUGINS_PREBUILT").is_some() && file.is_file() {
        return file;
    }
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap-synth", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    file
}

/// One Exemplar track (its sine) playing [`KEYS`].
fn song() -> Project {
    let mut p = Project::new("mts");
    p.tempo_bpm = 60.0;
    let mut t = p.make_track("Exemplar");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler));
    let mut clip = p.make_clip("keys", Ticks::ZERO, Ticks::beats(6));
    for (j, &key) in KEYS.iter().enumerate() {
        let n = p.make_note(
            Ticks::beats(2 * j as i64),
            Ticks::beats(1) + Ticks::EIGHTH,
            key,
            100,
        );
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn assembled() -> CantusApp {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(SR);
    let mut app = CantusApp::assemble(&ctx, files, Some(song()), engine, None, None, None, false);
    // What the boot's `load_mts` does with the standard library, with the fake.
    tuning::install_mts(&mut app, MtsLink::open(fake()));
    app.sync_engine();
    app
}

/// The song exported as the user exports it; the left channel.
fn exported(a: &mut CantusApp, name: &str) -> Vec<f64> {
    let dir = tempfile::tempdir().unwrap().keep();
    let wav = dir.join(format!("{name}.wav"));
    a.export_to(wav.clone());
    let start = Instant::now();
    while a.export_in_progress() {
        a.poll_export();
        assert!(start.elapsed() < Duration::from_secs(600), "export hung");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!a.status.error, "{}", a.status.text);
    let mut r = hound::WavReader::open(&wav).unwrap();
    let ch = r.spec().channels as usize;
    let s: Vec<f64> = match r.spec().sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|x| f64::from(x.unwrap())).collect(),
        hound::SampleFormat::Int => {
            let full = f64::from(1u32 << (r.spec().bits_per_sample - 1));
            r.samples::<i32>()
                .map(|x| f64::from(x.unwrap()) / full)
                .collect()
        }
    };
    s.chunks_exact(ch).map(|f| f[0]).collect()
}

fn bin(x: &[f64], start: usize, n: usize, hz: f64) -> (f64, f64) {
    let w = std::f64::consts::TAU * hz / SR;
    let (mut re, mut im) = (0.0, 0.0);
    for i in 0..n {
        let win = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
        let v = x[start + i] * win;
        re += v * (w * i as f64).cos();
        im -= v * (w * i as f64).sin();
    }
    (re, im)
}

const N: usize = 32_768;

/// The strongest frequency within ±2.5 % of `near`, refined by its phase slope
/// (`tuning_engine_e2e`'s measure).
fn partial(x: &[f64], start: usize, near: f64) -> f64 {
    let mut best = (0.0, near);
    for i in 0..=500 {
        let f = near * (0.975 + 0.05 * i as f64 / 500.0);
        let (re, im) = bin(x, start, N, f);
        if re * re + im * im > best.0 {
            best = (re * re + im * im, f);
        }
    }
    let mut f = best.1;
    for hop in [256usize, 4096, 8192] {
        let (a_re, a_im) = bin(x, start, N, f);
        let (b_re, b_im) = bin(x, start + hop, N, f);
        let d = b_im.atan2(b_re) - a_im.atan2(a_re);
        let want = std::f64::consts::TAU * f * hop as f64 / SR;
        let r = (d - want + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        f += r / (std::f64::consts::TAU * hop as f64 / SR);
    }
    f
}

/// Each key's offset from 12-TET at A440 as `x` plays it (track `track` of the song, which
/// starts at `8 × track` seconds), in cents.
fn played(x: &[f64], track: usize, table: &TuningTable) -> Vec<f64> {
    KEYS.iter()
        .enumerate()
        .map(|(j, &key)| {
            let et = 440.0 * 2f64.powf((f64::from(key) - 69.0) / 12.0);
            let near = et * 2f64.powf(f64::from(table.at(key)) / 12.0);
            let start = ((8 * track + 2 * j) as f64 * SR) as usize + (0.3 * SR) as usize;
            1200.0 * (partial(x, start, near) / et).log2()
        })
        .collect()
}

/// Whether `x` plays `table` within 0.1 cent on every key of track `track`.
fn plays(x: &[f64], track: usize, table: &TuningTable) -> bool {
    let got = played(x, track, table);
    let want: Vec<f64> = KEYS
        .iter()
        .map(|k| 100.0 * f64::from(table.at(*k)))
        .collect();
    println!("track {track}: played {got:.3?} c, table {want:.3?} c");
    got.iter().zip(&want).all(|(g, w)| (g - w).abs() < 0.1)
}

fn temper(id: Temperament, key: u8, a4: f32) -> Tuning {
    Tuning {
        a4_hz: a4,
        system: TuningSystem::Temperament {
            id,
            key,
            follow_song_key: false,
        },
        map: None,
    }
}

fn set_project_tuning(a: &mut CantusApp, t: Tuning) {
    assert!(a.exec(Command::SetProjectTuning {
        tuning: Box::new(t),
    }));
}

/// The live snapshot's table for the song's first track (what Chorda designs from and the
/// strips' effects read).
fn live_table(a: &CantusApp) -> TuningTable {
    *a.engine.sent_snapshot().expect("a snapshot sent").tracks[0]
        .tuning
        .table
}

#[test]
fn a_song_in_the_masters_tuning_follows_it_through_changes_exports_and_a_new_engine() {
    let _t = turn();
    let mut a = assembled();
    assert!(
        format!("{:?}", a.engine.external_tuning()).contains("MtsLink"),
        "the link reached the engine"
    );
    // Another program is the master, in Werckmeister III in E flat at A415.
    let master = MtsLink::open(fake());
    let werck = temper(Temperament::WerckmeisterIii, 3, 415.0).offsets();
    master
        .become_master(false, &werck, "Werckmeister III")
        .expect("the fake master");
    set_project_tuning(
        &mut a,
        Tuning {
            system: TuningSystem::External,
            ..Tuning::default()
        },
    );
    a.sync_engine();
    let mut read = TuningTable::EQUAL;
    assert!(a.tuning.mts.as_ref().unwrap().read(&mut read));
    assert!(plays(&exported(&mut a, "werck"), 0, &read), "the master's");
    assert_eq!(live_table(&a), read, "the live snapshot holds it");

    // The master moves to ¼-comma meantone: the next sync recompiles (no edit of ours).
    let meantone = temper(Temperament::MeantoneQuarter, 0, 440.0).offsets();
    master.publish_all(&meantone, "Meantone");
    let compiles = a.engine.compiles();
    a.sync_engine();
    assert!(a.engine.compiles() > compiles, "the master's change recompiled");
    let mut read2 = TuningTable::EQUAL;
    assert!(a.tuning.mts.as_ref().unwrap().read(&mut read2));
    assert_ne!(read2, read);
    assert_eq!(live_table(&a), read2);
    // Unchanged, it compiles nothing more.
    let compiles = a.engine.compiles();
    a.sync_engine();
    assert_eq!(a.engine.compiles(), compiles, "a still master costs nothing");
    assert!(plays(&exported(&mut a, "meantone"), 0, &read2));

    // An audio restart's new engine follows the same link.
    let (engine, _proc) = EngineHandle::manual(SR);
    a.adopt_engine(engine);
    assert!(format!("{:?}", a.engine.external_tuning()).contains("MtsLink"));
    assert!(plays(&exported(&mut a, "restarted"), 0, &read2), "after a restart");

    // The master goes: the song plays equal temperament at its diapason, live too.
    master.resign_master();
    a.sync_engine();
    assert!(live_table(&a).is_equal());
    assert!(plays(&exported(&mut a, "gone"), 0, &TuningTable::EQUAL));
}

/// The app's plug-in database knows the CLAP test synth, as after a scan; the picker puts
/// it on a new second track playing [`KEYS`] from 8 s.
fn host_the_synth(a: &mut CantusApp) -> TrackId {
    a.plugins.db_path = None;
    let dir = tempfile::tempdir().unwrap().keep();
    let file = dir.join("Cantus Test Synth.clap");
    std::fs::copy(test_synth(), &file).unwrap();
    let plugins = cantus_host::clap::ClapBackend.scan(&file).unwrap();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap();
    a.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    let info = a.hosted_instruments()[0].clone();
    let mut t = a.bus.new_track("Synth");
    let mut clip = a.bus.new_clip("keys", Ticks::beats(8), Ticks::beats(6));
    for (j, &key) in KEYS.iter().enumerate() {
        let n = a.bus.new_note(
            Ticks::beats(2 * j as i64),
            Ticks::beats(1) + Ticks::EIGHTH,
            key,
            100,
        );
        clip.notes.push(n);
    }
    t.clips.push(clip);
    let track = t.id;
    let index = a.project().tracks.len();
    assert!(a.exec(Command::InsertTrack {
        index,
        track: Box::new(t),
    }));
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info)),
    );
    let ext = a.project().track(track).unwrap().instrument.external().cloned();
    assert_eq!(ext.map(|x| x.tuning_by), Some(TuningBy::MtsEsp), "the default");
    track
}

#[test]
fn as_the_master_cantus_publishes_its_tuning_and_tunes_no_client_twice() {
    let _t = turn();
    let mut a = assembled();
    let synth = host_the_synth(&mut a);
    let client = MtsLink::open(fake());
    let werck = temper(Temperament::WerckmeisterIii, 3, 415.0);
    set_project_tuning(&mut a, werck.clone());
    let table = werck.offsets();

    // No master anywhere: the hosted synth is sent the table per note (it plays it).
    let alone = exported(&mut a, "alone");
    assert!(plays(&alone, 0, &table), "Exemplar");
    assert!(plays(&alone, 1, &table), "the synth, by note expression");

    // Be the master: the client reads the project's table.
    tuning::set_master(&mut a, true, false);
    assert!(a.tuning.mts.as_ref().unwrap().is_master());
    let mut heard = TuningTable::EQUAL;
    assert!(client.read(&mut heard));
    for k in 0..128 {
        assert!((heard.0[k] - table.0[k]).abs() < 1e-5, "key {k}");
    }
    // A plug-in at *Tuning by: MTS-ESP* retunes itself: Cantus sends it nothing per note —
    // the test synth, which does not read MTS-ESP, therefore plays 12-TET at A440 (an
    // MTS-ESP client would play the table once, never twice).
    let led = exported(&mut a, "led");
    assert!(plays(&led, 0, &table), "Exemplar reads the table itself");
    assert!(
        plays(&led, 1, &TuningTable::EQUAL),
        "no per-note tuning on top of MTS-ESP"
    );
    // Set to note expression (a plug-in that is no MTS-ESP client): the whole table again.
    assert!(a.exec(Command::SetInstrumentTuningBy {
        track: synth,
        by: TuningBy::NoteExpression,
    }));
    assert!(plays(&exported(&mut a, "by-note"), 1, &table));
    a.undo();
    assert_eq!(
        a.project().track(synth).unwrap().instrument.external().unwrap().tuning_by,
        TuningBy::MtsEsp
    );

    // An edit publishes the new table.
    let vallotti = temper(Temperament::Vallotti, 0, 440.0);
    set_project_tuning(&mut a, vallotti.clone());
    let ctx = egui::Context::default();
    tuning::frame(&mut a, &ctx);
    assert!(client.read(&mut heard));
    let want = vallotti.offsets();
    for k in 0..128 {
        assert!((heard.0[k] - want.0[k]).abs() < 1e-5, "republished, key {k}");
    }
    tuning::set_master(&mut a, false, false);
    assert!(!client.read(&mut heard), "resigned");
}
