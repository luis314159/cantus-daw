//! Round 42 (ADR 0155), end to end on the boot path: an app made by `CantusApp::assemble`
//! with the real engine handle; a song with Chorda, Exemplar and the hosted CLAP test synth
//! (put on its track as the picker puts it) set to Werckmeister III in E flat at A415 by the
//! command the window sends; exported to a WAV through the export the user runs. The three
//! play in tune with each other: the test synth's and Exemplar's sines sit at the table's
//! pitch within 0.1 cent (the synth hears it as CLAP's per-note tuning expression), and
//! Chorda — which keeps its own stretch and inharmonicity, as a piano technician's tuning
//! does — moves from its untuned pitch by exactly the table's offset (0.1 cent). Undo gives
//! the untuned song back, to the bit. Its own test binary: it points `CANTUS_DATA_DIR` at a
//! temporary folder.

mod support;
use cantus_core::tuning::{Temperament, Tuning, TuningSystem};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_host::{Backend, DbEntry, ScanStatus};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use std::path::Path;
use std::time::{Duration, Instant};

const SR: f64 = 48_000.0;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// The test instrument, built from source as the host's tests build it.
fn test_synth() -> std::path::PathBuf {
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

/// Keys each instrument plays, in its own stretch of the song (60 BPM: 2 s a key).
const KEYS: [u8; 3] = [57, 63, 70];
const CHORDA: usize = 0;
const EXEMPLAR: usize = 1;
const SYNTH: usize = 2;

fn song() -> Project {
    let mut p = Project::new("tuned");
    p.tempo_bpm = 60.0;
    for (i, kind) in [
        Some(InstrumentKind::Chorda),
        Some(InstrumentKind::Sampler),
        None, // the hosted synth, put on by the picker
    ]
    .into_iter()
    .enumerate()
    {
        let mut t = p.make_track(["Chorda", "Exemplar", "Synth"][i]);
        if let Some(k) = kind {
            t.instrument = InstrumentSpec::Native(NativeSpec::new(k));
        }
        let at = Ticks::beats(8 * i as i64);
        let mut clip = p.make_clip("keys", at, Ticks::beats(6));
        for (j, &key) in KEYS.iter().enumerate() {
            let n = p.make_note(Ticks::beats(2 * j as i64), Ticks::beats(1) + Ticks::EIGHTH, key, 100);
            clip.notes.push(n);
        }
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

fn assembled() -> CantusApp {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    CantusApp::assemble(&ctx, files, Some(song()), engine, None, None, None, false)
}

/// The app's plug-in database knows the test synth, as after a scan; the picker puts it on
/// the third track.
fn host_the_synth(a: &mut CantusApp) {
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
    let track = a.project().tracks[SYNTH].id;
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info)),
    );
    assert!(a.project().tracks[SYNTH].instrument.external().is_some());
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
    assert_eq!(r.spec().sample_rate, 48_000);
    let ch = r.spec().channels as usize;
    let s: Vec<f64> = match r.spec().sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|x| f64::from(x.unwrap())).collect(),
        hound::SampleFormat::Int => {
            let full = f64::from(1u32 << (r.spec().bits_per_sample - 1));
            r.samples::<i32>().map(|x| f64::from(x.unwrap()) / full).collect()
        }
    };
    s.chunks_exact(ch).map(|f| f[0]).collect()
}

/// The Hann-windowed spectrum of `x[start..start + n]` at `hz`.
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

/// The strongest frequency within ±2.5 % of `near`, refined by its phase slope over two
/// hops: a steady partial to a millionth.
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

/// The fundamental of instrument `inst`'s key `j` (its partial 1), near `hz`.
fn heard(x: &[f64], inst: usize, j: usize, hz: f64) -> f64 {
    let start = ((8 * inst + 2 * j) as f64 * SR) as usize + (0.3 * SR) as usize;
    partial(x, start, hz)
}

fn werckmeister_in_e_flat_at_415() -> Tuning {
    Tuning {
        a4_hz: 415.0,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 3,
            follow_song_key: false,
        },
        map: None,
    }
}

#[test]
fn chorda_exemplar_and_a_hosted_clap_synth_play_werckmeister_iii_in_e_flat_at_415_together() {
    let mut a = assembled();
    host_the_synth(&mut a);
    let plain = exported(&mut a, "plain");
    let tuning = werckmeister_in_e_flat_at_415();
    let table = tuning.offsets();
    a.bus
        .execute(
            Command::SetProjectTuning {
                tuning: Box::new(tuning),
            },
            Origin::User,
        )
        .expect("the window's command");
    let tuned = exported(&mut a, "tuned");
    let et = |key: u8| 440.0 * 2f64.powf((f64::from(key) - 69.0) / 12.0);
    let cents = |a: f64, b: f64| 1200.0 * (a / b).log2();
    for (j, &key) in KEYS.iter().enumerate() {
        let want = et(key) * 2f64.powf(f64::from(table.at(key)) / 12.0);
        // The sines: at the table's pitch.
        for (inst, name) in [(EXEMPLAR, "Exemplar"), (SYNTH, "the CLAP synth")] {
            let f = heard(&tuned, inst, j, want);
            let off = cents(f, want);
            println!("{name:15} key {key}: {f:.4} Hz for {want:.4} ({off:+.4} c)");
            assert!(off.abs() < 0.1, "{name} key {key}: {off:+.4} c");
            // And at 12-TET before.
            let f0 = heard(&plain, inst, j, et(key));
            assert!(cents(f0, et(key)).abs() < 0.1, "{name} untuned key {key}");
        }
        // Chorda: its own voicing moved by the table's offset exactly.
        let before = heard(&plain, CHORDA, j, et(key));
        let after = heard(&tuned, CHORDA, j, before * (want / et(key)));
        let moved = cents(after, before);
        let off = moved - f64::from(table.at(key)) * 100.0;
        println!(
            "Chorda          key {key}: {before:.4} → {after:.4} Hz, moved {moved:+.4} c ({off:+.4} c)"
        );
        assert!(off.abs() < 0.1, "Chorda key {key}: {off:+.4} c off the table");
        // In tune with the sines: what it adds of its own is what it added untuned.
        let own = cents(before, et(key));
        let with = cents(after, want);
        assert!((with - own).abs() < 0.1, "Chorda key {key}: {with:+.4} vs {own:+.4} c");
    }
    // Undone: the untuned song, to the bit.
    a.undo();
    assert!(a.project().tuning.is_default());
    assert_eq!(exported(&mut a, "undone"), plain);
}
