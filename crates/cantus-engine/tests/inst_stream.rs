//! Exemplar streaming from disk through the engine (ADR 0050): the sample cache streams
//! long samples (a cache file at the exact depth, heads in memory), a streamed instrument
//! renders offline exactly as in memory, real time driven at real-time pace with the real
//! reader thread has no late read and the same bits, and instrument files load in the
//! background with progress, a queue an undo empties, and an offline render that waits.

use cantus_core::inst::sampler::{zf, Zone};
use cantus_core::*;
use cantus_engine::inst as einst;
use cantus_engine::inst::sampler::tel;
use cantus_engine::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A 24-bit stereo WAV of `frames` frames: two sines and a little noise, so no two
/// blocks are alike.
fn write_wav24(path: &Path, frames: usize, seed: u32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let mut s = seed.max(1);
    for i in 0..frames {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        let t = i as f32 / 48_000.0;
        let n = (s >> 20) as i32 - 2_048;
        let l = (cantus_dsp::math::sin(t * 1_382.3) * 3_000_000.0) as i32 + n;
        let r = (cantus_dsp::math::sin(t * 2_011.7) * 2_500_000.0) as i32 - n;
        w.write_sample(l).unwrap();
        w.write_sample(r).unwrap();
    }
    w.finalize().unwrap();
}

/// This process's cache folder, `cantus-stream-tests-<pid>` in the temporary folder, as long
/// as a test uses it: removed when the last one using it ends, a panic included. A process
/// holds a lock on its folder's `.alive` file while it lives, so a later run removes the
/// folders of runs that died (killed, or a debugger stopped) — never a running one's.
struct CacheDir {
    path: PathBuf,
}

static USERS: std::sync::Mutex<usize> = std::sync::Mutex::new(0);
static ALIVE: std::sync::Mutex<Option<std::fs::File>> = std::sync::Mutex::new(None);
const PREFIX: &str = "cantus-stream-tests-";

fn folder_of(pid: u32) -> PathBuf {
    std::env::temp_dir().join(format!("{PREFIX}{pid}"))
}

/// Remove the folders other runs left: those whose `.alive` lock nobody holds.
fn sweep_stale() {
    let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    let me = folder_of(std::process::id());
    for e in rd.flatten() {
        let p = e.path();
        let ours = p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(PREFIX));
        if !ours || p == me {
            continue;
        }
        let free = std::fs::File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(p.join(".alive"))
            .is_ok_and(|f| f.try_lock().is_ok());
        if free {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

fn cache_dir() -> CacheDir {
    static SWEPT: std::sync::Once = std::sync::Once::new();
    SWEPT.call_once(sweep_stale);
    let path = folder_of(std::process::id());
    let mut users = USERS.lock().unwrap_or_else(|e| e.into_inner());
    if *users == 0 {
        std::fs::create_dir_all(&path).unwrap();
        let alive = std::fs::File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.join(".alive"))
            .unwrap();
        alive.lock().unwrap();
        *ALIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(alive);
    }
    *users += 1;
    einst::stream::hub().set_cache_dir(path.clone());
    CacheDir { path }
}

impl Drop for CacheDir {
    fn drop(&mut self) {
        let mut users = USERS.lock().unwrap_or_else(|e| e.into_inner());
        *users -= 1;
        if *users > 0 {
            return;
        }
        // The streams that read its files let them go first (the reader closes them as it
        // next looks).
        einst::stream::hub().let_go_unheld();
        *ALIVE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let t = Instant::now();
        while self.path.exists() && t.elapsed() < Duration::from_secs(5) {
            if std::fs::remove_dir_all(&self.path).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Run this test binary again, without this test, and see that it leaves no folder behind
/// (twice: a second run finds none of the first's either).
#[test]
fn the_suite_leaves_no_cache_folder_behind() {
    if std::env::var_os("CANTUS_STREAM_TESTS_CHILD").is_some() {
        return;
    }
    let exe = std::env::current_exe().unwrap();
    for run in 0..2 {
        let child = std::process::Command::new(&exe)
            .args([
                "--skip",
                "the_suite_leaves_no_cache_folder_behind",
                "--quiet",
            ])
            .env("CANTUS_STREAM_TESTS_CHILD", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "run {run} passed");
        assert!(
            !folder_of(pid).exists(),
            "run {run} left {}",
            folder_of(pid).display()
        );
    }
}

fn spec_of(zones: &[Zone]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for (n, z) in zones.iter().enumerate() {
        for (k, v) in z.values_for(n) {
            s.set(&k, v).unwrap();
        }
        for (k, t) in z.texts_for(n) {
            s.set_text(&k, Some(t)).unwrap();
        }
    }
    s
}

/// Two long samples, a third that loops (it stays in memory) and a short one.
fn instrument(dir: &Path) -> NativeSpec {
    let mut zones = Vec::new();
    for (i, (lo, hi, root, frames)) in [
        (36u8, 59u8, 48u8, 240_000usize),
        (60, 96, 72, 200_000),
        (97, 110, 100, 120_000),
        (111, 127, 115, 6_000),
    ]
    .iter()
    .enumerate()
    {
        let p = dir.join(format!("s{i}.wav"));
        write_wav24(&p, *frames, 3 + i as u32);
        let mut z = Zone::new(Some(p.to_string_lossy().into()), *lo, *hi, *root);
        if i == 2 {
            z.set(zf::MODE, 2.0)
                .set(zf::LS, 1_000.0)
                .set(zf::LE, 100_000.0);
        }
        zones.push(z);
    }
    spec_of(&zones)
}

fn project(spec: &NativeSpec, notes: &[(u32, u32, u8, u8)]) -> Project {
    let mut p = Project::new("stream");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Exemplar");
    t.instrument = InstrumentSpec::Native(spec.clone());
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(16));
    for (start, len, key, vel) in notes {
        clip.notes.push(p.make_note(
            Ticks::beats(*start as i64),
            Ticks::beats(*len as i64),
            *key,
            *vel,
        ));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn snapshot(p: &Project, cache: &mut SampleCache) -> Arc<Snapshot> {
    let mut s = Snapshot::compile(p, 1);
    assert!(cache.attach(&mut s).is_empty());
    Arc::new(s)
}

/// Notes on every zone, at the root and transposed, long enough to stream far past the
/// heads, overlapping.
const NOTES: &[(u32, u32, u8, u8)] = &[
    (0, 8, 48, 100),
    (0, 6, 55, 80),
    (1, 7, 72, 110),
    (2, 3, 79, 60),
    (2, 4, 100, 90),
    (3, 2, 115, 100),
    (4, 4, 66, 127),
];

#[test]
fn a_streamed_instrument_renders_offline_exactly_as_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let _cache = cache_dir();
    let spec = instrument(dir.path());
    let p = project(&spec, NOTES);
    let opts = RenderOptions {
        tail_seconds: 1.0,
        ..Default::default()
    };
    let whole = render(snapshot(&p, &mut SampleCache::new()), &opts).unwrap();
    let mut cache = SampleCache::new();
    cache.set_streaming(true);
    let snap = snapshot(&p, &mut cache);
    // The two long, plain samples stream; the looped one and the short one do not.
    let assets = &snap.tracks[0].assets;
    let streamed: Vec<bool> = (0..4)
        .map(|n| {
            let a = einst::asset(assets, &format!("z{n}.sample")).unwrap();
            einst::stream::lookup(a).is_some()
        })
        .collect();
    assert_eq!(streamed, [true, true, false, false]);
    let a = einst::asset(assets, "z0.sample").unwrap();
    assert!(a.len_frames() < 40_000, "only the head in memory");
    assert!(
        matches!(
            cache.status(Path::new(&spec.text("z0.sample").unwrap())),
            SampleStatus::Loaded {
                frames: 240_000,
                ..
            }
        ),
        "the status names the whole file"
    );
    for block in [512usize, 61] {
        let o = RenderOptions {
            block_frames: block,
            ..opts.clone()
        };
        let streamed = render(snap.clone(), &o).unwrap();
        assert!(streamed.peak > 0.01);
        assert!(
            streamed.samples == whole.samples,
            "block {block}: streamed differs from in memory"
        );
    }
    // A second cache finds the files written: heads only, the same stream.
    let mut again = SampleCache::new();
    again.set_streaming(true);
    let t = Instant::now();
    let s2 = snapshot(&p, &mut again);
    eprintln!(
        "second load (heads from the cache files): {:?}",
        t.elapsed()
    );
    let b = einst::asset(&s2.tracks[0].assets, "z0.sample").unwrap();
    assert!(Arc::ptr_eq(a, b), "one stream per file and plan, shared");
}

fn play_unit(
    u: &mut dyn einst::InstrumentUnit,
    frames: usize,
    paced: bool,
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    let start = Instant::now();
    for (i, (a, b)) in l.chunks_mut(256).zip(r.chunks_mut(256)).enumerate() {
        if paced {
            // The audio callback's pace: block i is due at i × 256 / 48 000 s.
            let due = Duration::from_secs_f64(i as f64 * 256.0 / 48_000.0);
            if let Some(wait) = due.checked_sub(start.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        u.process(a, b);
    }
    (l, r)
}

fn unit_for(spec: &NativeSpec, assets: &einst::Assets) -> Box<dyn einst::InstrumentUnit> {
    let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
    u.configure(spec, assets);
    u
}

#[test]
fn real_time_with_the_reader_thread_has_no_late_read_and_the_same_bits() {
    let dir = tempfile::tempdir().unwrap();
    let _cache = cache_dir();
    let spec = instrument(dir.path());
    let p = project(&spec, &[]);
    let whole = snapshot(&p, &mut SampleCache::new());
    let mut cache = SampleCache::new();
    cache.set_streaming(true);
    let streamed = snapshot(&p, &mut cache);
    let mut a = unit_for(&spec, &whole.tracks[0].assets);
    let mut b = unit_for(&spec, &streamed.tracks[0].assets);
    assert_eq!(b.telemetry()[tel::STREAMED], 2.0);
    for u in [&mut a, &mut b] {
        for (i, key) in [48u8, 52, 72, 79].iter().enumerate() {
            u.note_on(einst::NoteOn {
                key: *key,
                velocity: 0.8,
                id: i as u64 + 1,
                channel: 0,
            });
        }
    }
    let frames = 48_000 * 3;
    let (al, ar) = play_unit(a.as_mut(), frames, false);
    let (bl, br) = play_unit(b.as_mut(), frames, true);
    assert_eq!(b.take_late_reads(), 0, "no block came late");
    assert!(
        bl == al && br == ar,
        "real time equals in memory, bit for bit"
    );
}

#[test]
fn instrument_files_load_in_the_background_with_progress_and_an_undo_empties_the_queue() {
    let dir = tempfile::tempdir().unwrap();
    let _cache = cache_dir();
    let spec = instrument(dir.path());
    let p = project(&spec, NOTES);
    let mut cache = SampleCache::new();
    cache.set_streaming(true);
    cache.set_background(true);
    // A slow decoder (300 ms a file): loaded here, attach would take over a second.
    cantus_engine::samples::assets::set_load_delay(Duration::from_millis(300));
    let t = Instant::now();
    let mut s = Snapshot::compile(&p, 1);
    assert!(cache.attach(&mut s).is_empty());
    let took = t.elapsed();
    assert!(
        took < Duration::from_millis(250),
        "attach returned in {took:?}"
    );
    let (done, total) = cache.progress().expect("loading, with a count");
    assert_eq!(total, 4);
    assert!(done <= total);
    assert!(
        s.tracks[0].assets.iter().any(|(_, a)| a.is_none()),
        "what has not arrived is silent for now"
    );
    // The wait an offline render does, then everything is there.
    assert!(cache.wait_assets());
    cantus_engine::samples::assets::set_load_delay(Duration::ZERO);
    assert_eq!(cache.progress(), None);
    let mut s = Snapshot::compile(&p, 2);
    assert!(cache.attach(&mut s).is_empty());
    assert!(s.tracks[0].assets.iter().all(|(_, a)| a.is_some()));
    let full = render(Arc::new(s), &RenderOptions::default()).unwrap();
    let whole = render(
        snapshot(&p, &mut SampleCache::new()),
        &RenderOptions::default(),
    )
    .unwrap();
    assert!(
        full.samples == whole.samples,
        "loaded in the background, rendered the same"
    );

    // A pick undone while it loads: the next snapshot no longer asks, the queue lets go.
    let other = tempfile::tempdir().unwrap();
    let spec2 = instrument(other.path());
    let mut fresh = SampleCache::new();
    fresh.set_background(true);
    let mut s = Snapshot::compile(&project(&spec2, &[]), 3);
    fresh.attach(&mut s);
    let mut empty = Snapshot::compile(&Project::new("undone"), 4);
    fresh.attach(&mut empty);
    let t = Instant::now();
    while fresh.loading() && t.elapsed() < Duration::from_secs(20) {
        fresh.poll_loaded();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!fresh.loading(), "nothing left on its way");
    assert_eq!(
        fresh.progress(),
        None,
        "no progress for files nobody asks for"
    );
}

/// A note that starts before its zone's sample has arrived plays nothing for that zone
/// and does not start it later; the next note, after the sample is in, plays it — the rule
/// for a missing file, deterministic (ADR 0050 §7).
#[test]
fn a_note_before_its_sample_arrives_stays_silent_and_the_next_one_plays() {
    let dir = tempfile::tempdir().unwrap();
    let _cache = cache_dir();
    let spec = instrument(dir.path());
    let p = project(&spec, &[]);
    let mut cache = SampleCache::new();
    cache.set_background(true);
    let mut early = Snapshot::compile(&p, 1);
    cache.attach(&mut early);
    let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
    // Nothing has arrived yet: the zones exist, silent.
    let none: Vec<(String, Option<Arc<SampleData>>)> = early.tracks[0]
        .assets
        .iter()
        .map(|(k, _)| (k.clone(), None))
        .collect();
    u.configure(&spec, &none);
    let on = |id: u64| einst::NoteOn {
        key: 48,
        velocity: 0.9,
        id,
        channel: 0,
    };
    u.note_on(on(1));
    let (l, _) = play_unit(u.as_mut(), 4_800, false);
    assert!(l.iter().all(|x| *x == 0.0), "no sample yet: silence");
    cache.wait_assets();
    let mut ready = Snapshot::compile(&p, 2);
    assert!(cache.attach(&mut ready).is_empty());
    u.configure(&spec, &ready.tracks[0].assets);
    let (l, _) = play_unit(u.as_mut(), 4_800, false);
    assert!(
        l.iter().all(|x| *x == 0.0),
        "the held note does not start late"
    );
    u.note_on(on(2));
    let (l, _) = play_unit(u.as_mut(), 4_800, false);
    assert!(l.iter().any(|x| *x != 0.0), "the next note plays");
}
