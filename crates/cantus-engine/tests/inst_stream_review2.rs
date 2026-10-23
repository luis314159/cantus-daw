//! The second review of streaming (ADR 0050): a damaged or unreadable cache block never
//! silences a sample for the session nor makes an export differ (3, 5, 15); a dropped
//! unit's boards and streams are let go (1, 2); the sweep never deletes a file in use, nor
//! one whose drive is away (6, 7, 8); a note right after a zone's start changes plays in real
//! time as in memory (16); Draft at an extreme ratio never stalls an offline render (17).

use cantus_core::inst::sampler::Zone;
use cantus_core::*;
use cantus_engine::inst as einst;
use cantus_engine::*;
use einst::stream::LoadNote;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The cache folder is process-wide: these tests take turns.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        w.write_sample((cantus_dsp::math::sin(t * 1_382.3) * 3_000_000.0) as i32 + n)
            .unwrap();
        w.write_sample((cantus_dsp::math::sin(t * 911.1) * 2_000_000.0) as i32 - n)
            .unwrap();
    }
    w.finalize().unwrap();
}

fn cache_here(dir: &Path) -> PathBuf {
    // What earlier tests' loads had to say is theirs (the hub is the process's).
    let _ = einst::stream::hub().take_notes();
    // The machine's disk may be nearly full: these tests' cache files are small.
    einst::stream::set_free_floor(0);
    let c = dir.join("cache");
    einst::stream::hub().set_cache_dir(c.clone());
    c
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

fn project(spec: &NativeSpec, notes: &[(u32, u32, u8, u8)]) -> Project {
    let mut p = Project::new("review");
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

fn attach(p: &Project, cache: &mut SampleCache) -> Arc<Snapshot> {
    let mut s = Snapshot::compile(p, 1);
    assert!(cache.attach(&mut s).is_empty());
    Arc::new(s)
}

fn streaming() -> SampleCache {
    let mut c = SampleCache::new();
    c.set_streaming(true);
    c
}

fn file(dir: &Path, name: &str, frames: usize, seed: u32) -> PathBuf {
    let p = dir.join(name);
    write_wav24(&p, frames, seed);
    p
}

fn decode(p: &Path) -> Result<SampleData, String> {
    SampleCache::new().load(p).map(|a| (*a).clone())
}

fn opts() -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.5,
        ..Default::default()
    }
}

fn wait_until(what: &str, secs: u64, mut done: impl FnMut() -> bool) {
    let t = Instant::now();
    while !done() {
        assert!(t.elapsed() < Duration::from_secs(secs), "waited for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The source's cache file written (a first load waited for).
fn cached_one(src: &Path, _cache: &Path) {
    if let einst::stream::Loaded::Streamed(h) = einst::stream::load(src, &decode).unwrap() {
        wait_until("the cache file", 20, || !einst::stream::memory_backed(&h));
    }
}

/// The source's cache file, written (a first load waited for) and let go.
fn cached(src: &Path, cache: &Path) -> PathBuf {
    if let einst::stream::Loaded::Streamed(h) = einst::stream::load(src, &decode).unwrap() {
        wait_until("the cache file", 20, || !einst::stream::memory_backed(&h));
    }
    let files: Vec<PathBuf> = std::fs::read_dir(cache)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cstream"))
        .collect();
    assert_eq!(files.len(), 1);
    files[0].clone()
}

/// Zero frames `from..to` of a 24-bit stereo cache file's data (as a crash before the
/// deferred flush leaves it).
fn damage(file: &Path, frames: usize, from: usize, to: usize) {
    let mut bytes = std::fs::read(file).unwrap();
    let data = bytes.len() - frames * 6;
    bytes[data + from * 6..data + to * 6].fill(0);
    // The stream that wrote it lets go of it (and of its lock) as the reader next looks.
    einst::stream::hub().let_go_unheld();
    wait_until("the file let go", 5, || {
        std::fs::write(file, &bytes).is_ok()
    });
}

fn damaged_notes(cache: &mut SampleCache) -> usize {
    cache
        .take_notes()
        .iter()
        .filter(|n| matches!(n, LoadNote::Damaged(_)))
        .count()
}

/// Second review #3, #5, #15: a cache file damaged after its header (a crash before its
/// flush) is found at the first bad block; the source is decoded again in the session, the
/// offline render waits for it, so every export — the first included — equals the
/// in-memory render, and a fresh cache file is written. Said once.
#[test]
fn a_damaged_block_is_decoded_again_in_the_session_and_every_export_equals_memory() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let src = file(dir.path(), "a.wav", 200_000, 21);
    let cf = cached(&src, &cdir);
    damage(&cf, 200_000, 60_000, 150_000);
    let spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
    let p = project(&spec, &[(0, 8, 60, 100)]);
    let whole = render(attach(&p, &mut SampleCache::new()), &opts())
        .unwrap()
        .samples;
    // One cache for the session, as the application keeps.
    let mut cache = streaming();
    let first = render(attach(&p, &mut cache), &opts()).unwrap().samples;
    assert!(first == whole, "the first export waited for the decode");
    assert_eq!(damaged_notes(&mut cache), 1, "said once");
    let again = render(attach(&p, &mut cache), &opts()).unwrap().samples;
    assert!(again == whole, "and the next");
    assert_eq!(damaged_notes(&mut cache), 0, "once");
    // A fresh, whole cache file replaces the damaged one.
    wait_until("a fresh cache file", 20, || {
        einst::stream::read_whole(&src).is_some_and(|d| *d.frames == *decode(&src).unwrap().frames)
    });
}

/// Second review #5, #15 in real time: at most the one note that reaches the damaged block
/// fades, counted; the source is decoded again meanwhile, and the next note plays the
/// in-memory samples, not late.
#[test]
fn live_a_damaged_block_cuts_one_note_and_the_next_plays_as_in_memory() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let src = file(dir.path(), "a.wav", 200_000, 22);
    let cf = cached(&src, &cdir);
    damage(&cf, 200_000, 30_000, 190_000);
    let spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
    let p = project(&spec, &[]);
    let whole = attach(&p, &mut SampleCache::new());
    let mut cache = streaming();
    let streamed = attach(&p, &mut cache);
    // Real time: 10 ms blocks, a note held for 0.9 s.
    let play = |assets: &einst::Assets| {
        let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
        u.configure(&spec, assets);
        u.note_on(einst::NoteOn {
            key: 60,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let mut out = Vec::new();
        let mut late = 0;
        for _ in 0..90 {
            let t = Instant::now();
            let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
            u.process(&mut l, &mut r);
            late += u.take_late_reads();
            out.extend_from_slice(&l);
            if let Some(rest) = Duration::from_millis(10).checked_sub(t.elapsed()) {
                std::thread::sleep(rest);
            }
        }
        (out, late)
    };
    let (m, _) = play(&whole.tracks[0].assets);
    let (cut, late) = play(&streamed.tracks[0].assets);
    // At most the one note that reached the bad block before the decode was in (it is
    // usually in first: the reader finds the bad block a few blocks ahead of the voice).
    assert!(late <= 1, "{late}");
    if late == 0 {
        assert!(cut == m, "not late: the in-memory samples");
    }
    // The source decoded again beside the reader: the next note plays it.
    std::thread::sleep(Duration::from_millis(500));
    let (next, late) = play(&streamed.tracks[0].assets);
    assert_eq!(late, 0, "recovered");
    assert!(next == m, "the in-memory samples");
    assert_eq!(damaged_notes(&mut cache), 1);
}

/// Second review #15: when the source itself cannot be read either, an export fails
/// loudly, naming the file — never a bounce silently cut short — and the next snapshot
/// reports the file missing.
#[test]
fn an_unreadable_source_fails_the_export_naming_the_file() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let src = file(dir.path(), "gone.wav", 200_000, 23);
    let cf = cached(&src, &cdir);
    damage(&cf, 200_000, 60_000, 150_000);
    let spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
    let p = project(&spec, &[(0, 8, 60, 100)]);
    let mut cache = streaming();
    let snap = attach(&p, &mut cache);
    std::fs::remove_file(&src).unwrap();
    let t = Instant::now();
    let r = render(snap, &opts());
    assert!(
        t.elapsed() < Duration::from_secs(8),
        "no stall: {:?}",
        t.elapsed()
    );
    match r {
        Err(e) => assert!(e.to_string().contains("gone.wav"), "{e}"),
        Ok(_) => panic!("an export missing part of a sample must fail"),
    }
    assert!(cache
        .take_notes()
        .iter()
        .any(|n| matches!(n, LoadNote::Unreadable(p, _) if p == &src)));
    let mut next = Snapshot::compile(&p, 2);
    let missing = cache.attach(&mut next);
    assert!(
        missing.iter().any(|(p, _)| p == &src),
        "reported missing: {missing:?}"
    );
}

/// Second review #1: a dropped unit's boards are let go — the reader keeps no copy of them,
/// and their cells are zeroed as the unit goes, so nothing stale stays wanted.
#[test]
fn a_dropped_units_boards_are_let_go_and_zeroed() {
    use einst::InstrumentUnit;
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 300_000, 31);
    let mut z = Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60);
    z.set(cantus_core::inst::sampler::zf::START, 150_000.0);
    let spec = spec_of(&[z]);
    let snap = attach(&project(&spec, &[]), &mut streaming());
    let mut u = einst::sampler::Unit::new(48_000.0);
    u.configure(&spec, &snap.tracks[0].assets);
    let starts = u.dsp().starts().clone();
    let board = Arc::downgrade(u.dsp().board());
    assert!(
        (0..starts.len()).any(|i| starts.get(i) != 0),
        "a start pinned"
    );
    // The reader has copied the boards.
    std::thread::sleep(Duration::from_millis(100));
    drop(u);
    assert!(
        (0..starts.len()).all(|i| starts.get(i) == 0),
        "zeroed as the unit goes"
    );
    let weak = Arc::downgrade(&starts);
    drop(starts);
    wait_until("the boards to go", 3, || {
        weak.upgrade().is_none() && board.upgrade().is_none()
    });
}

/// Second review #2: a stream nothing plays is released while its instrument lives — a zone
/// removed from it, or a voice that ended on it before its zone moved to another file.
#[test]
fn a_stream_is_released_while_its_unit_lives() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 150_000, 32);
    let b = file(dir.path(), "b.wav", 150_000, 33);
    let c = file(dir.path(), "c.wav", 150_000, 34);
    let path = |p: &Path| Some(p.to_string_lossy().into_owned());
    let mut cache = streaming();
    let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
    // (a) A zone removed.
    let two = spec_of(&[
        Zone::new(path(&b), 0, 60, 48),
        Zone::new(path(&a), 61, 127, 72),
    ]);
    let one = spec_of(&[Zone::new(path(&b), 0, 60, 48)]);
    let snap = attach(&project(&two, &[]), &mut cache);
    u.configure(&two, &snap.tracks[0].assets);
    let gone = Arc::downgrade(
        &einst::stream::lookup(snap.tracks[0].assets[1].1.as_ref().unwrap()).unwrap(),
    );
    let next = attach(&project(&one, &[]), &mut cache);
    u.configure(&one, &next.tracks[0].assets);
    drop(snap);
    wait_until("the removed zone's stream to go", 3, || {
        cache.poll_loaded();
        gone.upgrade().is_none()
    });
    // (b) A voice ended on a zone's stream, then the zone moved to another file.
    let on_b = spec_of(&[Zone::new(path(&b), 0, 127, 60)]);
    let on_c = spec_of(&[Zone::new(path(&c), 0, 127, 60)]);
    let snap = attach(&project(&on_b, &[]), &mut cache);
    drop(next);
    u.configure(&on_b, &snap.tracks[0].assets);
    let gone = Arc::downgrade(
        &einst::stream::lookup(snap.tracks[0].assets[0].1.as_ref().unwrap()).unwrap(),
    );
    u.note_on(einst::NoteOn {
        key: 60,
        velocity: 1.0,
        id: 1,
        channel: 0,
    });
    let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
    for _ in 0..20 {
        u.process(&mut l, &mut r);
    }
    u.note_off(60, 1, 0.0);
    for _ in 0..2_000 {
        u.process(&mut l, &mut r);
        if u.active_voices() == 0 {
            break;
        }
    }
    assert_eq!(u.active_voices(), 0, "the voice ended");
    let next = attach(&project(&on_c, &[]), &mut cache);
    u.configure(&on_c, &next.tracks[0].assets);
    u.process(&mut l, &mut r);
    drop(snap);
    wait_until("the ended voice's stream to go", 3, || {
        cache.poll_loaded();
        gone.upgrade().is_none()
    });
    drop(u);
}

/// Second review #16: a note in the very block a zone's start changes in — or the first
/// block of a new instrument — plays in real time as in memory: the new start's blocks are
/// read before the snapshot reaches the audio thread (it is held back until they are).
#[test]
fn a_note_right_after_a_start_edit_plays_in_real_time_as_in_memory() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 300_000, 41);
    let zone = |start: f32| {
        let mut z = Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60);
        z.set(cantus_core::inst::sampler::zf::START, start);
        spec_of(&[z])
    };
    let (from, to) = (zone(0.0), zone(150_000.0));
    let note = |u: &mut Box<dyn einst::InstrumentUnit>| {
        u.note_on(einst::NoteOn {
            key: 60,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let (mut l, mut r) = (vec![0.0f32; 4_800], vec![0.0f32; 4_800]);
        u.process(&mut l, &mut r);
        (l, u.take_late_reads())
    };
    let whole = attach(&project(&to, &[]), &mut SampleCache::new());
    let mut m = einst::make(InstrumentKind::Sampler, 48_000.0);
    m.configure(&to, &whole.tracks[0].assets);
    let (want, _) = note(&mut m);
    assert!(want.iter().any(|x| *x != 0.0));
    let mut cache = streaming();
    let before = attach(&project(&from, &[]), &mut cache);
    let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
    u.configure(&from, &before.tracks[0].assets);
    std::thread::sleep(Duration::from_millis(200));
    // The edit: compiled and attached as the application does, then configured on the audio
    // thread with the note in the same block.
    let after = attach(&project(&to, &[]), &mut cache);
    // The engine sends it once its starts are read (third review #6: never a wait in the
    // attach itself); here, as `EngineHandle::flush_pending` does, beside.
    cache.wait_starts(Duration::from_millis(250));
    u.configure(&to, &after.tracks[0].assets);
    let (got, late) = note(&mut u);
    assert_eq!(late, 0, "the new start's blocks were ready");
    assert!(got == want, "the in-memory samples");
    // A new instrument's first block.
    let mut fresh = einst::make(InstrumentKind::Sampler, 48_000.0);
    fresh.configure(&to, &after.tracks[0].assets);
    let (got, late) = note(&mut fresh);
    assert_eq!(late, 0);
    assert!(got == want);
}

/// Second review #17, third #8 and #9: Draft at a ratio faster than the reader keeps up with
/// in real time (over 64 frames a frame at 48 kHz; here 127 semitones up, ~1 500) is decided
/// at the voice's start: it plays its head and ends there — offline at once, never a
/// 10-second stall, and the same samples in real time; never counted late, counted as a cut
/// note instead (the status line and the export say it).
#[test]
fn draft_at_an_extreme_ratio_plays_its_head_alike_offline_and_in_real_time() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "long.wav", 2_000_000, 51);
    let mut spec = spec_of(&[Zone::new(Some(a.to_string_lossy().into()), 0, 127, 0)]);
    spec.set("quality", 0.0).unwrap();
    let mut cache = streaming();
    let snap = attach(&project(&spec, &[]), &mut cache);
    let play = |offline: bool, key: u8| {
        let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
        u.set_offline(offline);
        u.configure(&spec, &snap.tracks[0].assets);
        u.note_on(einst::NoteOn {
            key,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let t = Instant::now();
        let mut out = Vec::new();
        for _ in 0..20 {
            let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
            u.process(&mut l, &mut r);
            out.extend_from_slice(&l);
        }
        let cut = u.take_cut_notes();
        (out, u.take_late_reads(), t.elapsed(), cut)
    };
    let (off, late_off, took, cut_off) = play(true, 127);
    let (rt, late_rt, _, cut_rt) = play(false, 127);
    assert_eq!((cut_off, cut_rt), (1, 1), "a cut note, counted alike");
    assert!(took < Duration::from_secs(2), "no stall offline: {took:?}");
    assert!(off.iter().any(|x| *x != 0.0), "the head plays");
    assert_eq!((late_off, late_rt), (0, 0), "never late");
    assert!(off == rt, "offline is real time");
    // Just below it (70 semitones up: ~57 frames a frame, a control step's reads wider than
    // the window) a voice streams as in memory, offline, with the real reader.
    let whole = attach(&project(&spec, &[]), &mut SampleCache::new());
    let mut m = einst::make(InstrumentKind::Sampler, 48_000.0);
    m.configure(&spec, &whole.tracks[0].assets);
    m.note_on(einst::NoteOn {
        key: 70,
        velocity: 1.0,
        id: 1,
        channel: 0,
    });
    let mut want = Vec::new();
    for _ in 0..20 {
        let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
        m.process(&mut l, &mut r);
        want.extend_from_slice(&l);
    }
    let (got, late, took, cut) = play(true, 70);
    assert_eq!(cut, 0);
    assert!(took < Duration::from_secs(5), "{took:?}");
    assert_eq!(late, 0);
    assert!(got == want, "streamed offline: the in-memory samples");
}

/// Second review #17: a bend that widens a Draft voice's reads past its window after its
/// start is read from the pool as far as the blocks there go, down to one frame at a time —
/// offline it waits for each, never the pool's whole patience: the in-memory samples.
#[test]
fn a_bend_that_widens_the_reads_mid_voice_streams_offline_as_in_memory() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "long.wav", 2_000_000, 52);
    let mut spec = spec_of(&[Zone::new(Some(a.to_string_lossy().into()), 0, 127, 0)]);
    spec.set("quality", 0.0).unwrap();
    spec.set("bend", 24.0).unwrap();
    let mut cache = streaming();
    let streamed = attach(&project(&spec, &[]), &mut cache);
    let whole = attach(&project(&spec, &[]), &mut SampleCache::new());
    let play = |assets: &einst::Assets, offline: bool| {
        let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
        u.set_offline(offline);
        u.configure(&spec, assets);
        u.note_on(einst::NoteOn {
            key: 70,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let mut out = Vec::new();
        for b in 0..20 {
            if b == 2 {
                // Two octaves up on top of ~57 frames a frame: ~230, decided at the start
                // to stream, its runs cut to what the window and the pool hold.
                u.controller(einst::Controller::PitchBend, 2.0);
            }
            let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
            u.process(&mut l, &mut r);
            out.extend_from_slice(&l);
        }
        (out, u.take_late_reads())
    };
    let (want, _) = play(&whole.tracks[0].assets, false);
    let t = Instant::now();
    let (got, late) = play(&streamed.tracks[0].assets, true);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert_eq!(late, 0);
    assert!(got == want, "the in-memory samples");
}

/// Second review #13: the progress an export waits on counts audio clips' sources loading
/// in the background too — never "0 of 0" while they load.
#[test]
fn the_loading_progress_counts_audio_clips() {
    let dir = tempfile::tempdir().unwrap();
    let a = file(dir.path(), "clip.wav", 48_000, 61);
    let b = file(dir.path(), "clip2.wav", 48_000, 62);
    let mut cache = SampleCache::new();
    cache.set_background(true);
    assert_eq!(cache.progress(), None, "nothing loading");
    assert!(
        cache.load_audio(&a.to_string_lossy(), 48_000).is_err(),
        "on its way"
    );
    assert!(cache.load_audio(&b.to_string_lossy(), 48_000).is_err());
    assert!(cache.loading());
    assert_eq!(cache.progress(), Some((0, 2)), "two of them asked for");
    wait_until("the clips", 10, || {
        cache.poll_loaded();
        !cache.loading()
    });
    assert!(cache.load_audio(&a.to_string_lossy(), 48_000).is_ok());
    assert_eq!(cache.progress(), None, "the batch is over");
}

/// Second review #9: a source changed in its middle with its size and time kept (a tool that
/// restores timestamps) has the same identity, so its old cache file opens — and the whole
/// file's hash, stored when the cache file was written and checked beside the reader once
/// it opens, finds it out: the stream is let go, said as a changed sample, and the next
/// snapshot plays the file as it is now.
#[test]
fn a_source_changed_in_its_middle_with_its_time_kept_is_found_out() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let src = file(dir.path(), "a.wav", 200_000, 71);
    cached(&src, &cdir);
    einst::stream::hub().let_go_unheld();
    let id = einst::stream_cache::identity(&src).unwrap();
    let when = std::fs::metadata(&src).unwrap().modified().unwrap();
    let mut bytes = std::fs::read(&src).unwrap();
    let mid = bytes.len() / 2;
    for b in &mut bytes[mid..mid + 60_000] {
        *b = b.wrapping_add(17);
    }
    std::fs::write(&src, &bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&src)
        .unwrap()
        .set_modified(when)
        .unwrap();
    assert_eq!(
        einst::stream_cache::identity(&src),
        Some(id),
        "the same identity"
    );
    let spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
    let p = project(&spec, &[(0, 8, 60, 100)]);
    let now = render(attach(&p, &mut SampleCache::new()), &opts())
        .unwrap()
        .samples;
    let mut cache = streaming();
    let old = attach(&p, &mut cache);
    let mut changed = false;
    wait_until("the check", 10, || {
        changed |= cache
            .take_notes()
            .iter()
            .any(|n| matches!(n, LoadNote::ChangedSinceCached(f) if f == &src));
        changed
    });
    drop(old);
    let fresh = render(attach(&p, &mut cache), &opts()).unwrap().samples;
    assert!(fresh == now, "the file as it is now");
}

/// Third review #3: stems fail as the mix does when a streamed sample cannot be read —
/// naming the file, in memory and to a folder, which is left with no file.
#[test]
fn stems_fail_naming_an_unreadable_sample_and_leave_no_file() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let src = file(dir.path(), "gone.wav", 200_000, 81);
    let cf = cached(&src, &cdir);
    damage(&cf, 200_000, 60_000, 150_000);
    let spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
    let p = project(&spec, &[(0, 8, 60, 100)]);
    let mut cache = streaming();
    let snap = attach(&p, &mut cache);
    std::fs::remove_file(&src).unwrap();
    let specs = [StemSpec {
        source: StemSource::Master,
        file_name: "master.wav".into(),
    }];
    let o = StemOptions {
        render: opts(),
        keep_silent: true,
        ..Default::default()
    };
    match render_stems(snap.clone(), &o, &specs) {
        Err(e) => assert!(e.to_string().contains("gone.wav"), "{e}"),
        Ok(_) => panic!("stems cut short must fail"),
    }
    let out = tempfile::tempdir().unwrap();
    let r = render_stems_to_folder(snap, &o, &specs, out.path(), &mut |_| true);
    assert!(
        matches!(r, Err(StemsError::Unreadable(ref f)) if f.contains("gone.wav")),
        "{r:?}"
    );
    assert_eq!(
        std::fs::read_dir(out.path()).unwrap().count(),
        0,
        "no file left"
    );
}

/// Third review #5: the whole-source check runs once per file as it is, per session — a
/// stream opened again (a preset switched back) is not hashed again.
#[test]
fn the_whole_source_check_runs_once_per_file() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let src = file(dir.path(), "a.wav", 200_000, 91);
    cached(&src, &cdir);
    einst::stream::hub().let_go_unheld();
    let open = || match einst::stream::load(&src, &decode).unwrap() {
        einst::stream::Loaded::Streamed(h) => h,
        _ => panic!("streamed"),
    };
    let before = einst::stream::sources_hashed();
    let h = open();
    wait_until("the check", 10, || einst::stream::sources_hashed() > before);
    drop(h);
    einst::stream::hub().let_go_unheld();
    let again = open();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(einst::stream::sources_hashed(), before + 1, "hashed once");
    drop(again);
}

/// Third review #4: many damaged cache files met at once are recovered one at a time (one
/// decode, counted against the writers' bound before it is made) — and every export still
/// equals the in-memory render.
#[test]
fn many_damaged_files_at_once_are_recovered_one_at_a_time() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let mut zones = Vec::new();
    let mut notes = Vec::new();
    for i in 0..6u8 {
        let src = file(dir.path(), &format!("m{i}.wav"), 200_000, 100 + i as u32);
        if let einst::stream::Loaded::Streamed(h) = einst::stream::load(&src, &decode).unwrap() {
            wait_until("the cache file", 20, || !einst::stream::memory_backed(&h));
        }
        let key = 40 + i * 5;
        zones.push(Zone::new(
            Some(src.to_string_lossy().into()),
            key,
            key + 4,
            key,
        ));
        notes.push((0, 8, key, 100));
    }
    einst::stream::hub().let_go_unheld();
    let files: Vec<PathBuf> = std::fs::read_dir(&cdir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cstream"))
        .collect();
    assert_eq!(files.len(), 6);
    for f in &files {
        damage(f, 200_000, 30_000, 190_000);
    }
    let spec = spec_of(&zones);
    let p = project(&spec, &notes);
    let whole = render(attach(&p, &mut SampleCache::new()), &opts())
        .unwrap()
        .samples;
    let mut cache = streaming();
    let out = render(attach(&p, &mut cache), &opts()).unwrap().samples;
    assert!(
        out == whole,
        "every damaged file recovered: the in-memory samples"
    );
    assert_eq!(damaged_notes(&mut cache), 6);
    assert_eq!(einst::stream::recoveries_at_once_peak(), 1, "one at a time");
}

/// Third review #6: dragging a full-range zone's start (a sample dropped on Exemplar: seven
/// mip levels) never blocks the compile — the interface's thread — even on a slow disk: the
/// starts are handed to the reader and the snapshot waits beside, ready a moment later.
#[test]
fn a_start_drag_never_blocks_the_compile() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 480_000, 111);
    let zone = |start: f32| {
        let mut z = Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60);
        z.set(cantus_core::inst::sampler::zf::START, start);
        spec_of(&[z])
    };
    let mut cache = streaming();
    attach(&project(&zone(0.0), &[]), &mut cache);
    einst::stream::set_read_delay(Duration::from_millis(5));
    // The second-worst attach: a shared machine can preempt this thread once inside one
    // (GitHub's Windows runner: 9.5 ms once, 2026-10-22); a blocking compile would put
    // every attach of the drag over the budget, not one.
    let mut times = Vec::with_capacity(40);
    for step in 1..=40 {
        let t = Instant::now();
        attach(&project(&zone(step as f32 * 7_000.0), &[]), &mut cache);
        times.push(t.elapsed());
    }
    times.sort();
    let worst = times[times.len() - 2];
    let ready = {
        let t = Instant::now();
        cache.wait_starts(Duration::from_millis(500));
        t.elapsed()
    };
    einst::stream::set_read_delay(Duration::ZERO);
    eprintln!("the second-slowest attach of the drag: {worst:?}; ready {ready:?} after");
    assert!(
        worst < Duration::from_millis(8),
        "a frame's budget: {worst:?}"
    );
    assert!(cache.starts_ready());
}

/// Third review #7: while another zone's start is edited every 100 ms, a voice playing a
/// different sample in real time is never late — the reader serves playing voices first.
#[test]
fn start_edits_never_make_a_playing_voice_late() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 480_000, 121);
    let b = file(dir.path(), "b.wav", 480_000, 122);
    let spec_b = |start: f32| {
        let mut z = Zone::new(Some(b.to_string_lossy().into()), 0, 127, 60);
        z.set(cantus_core::inst::sampler::zf::START, start);
        spec_of(&[z])
    };
    let spec_a = spec_of(&[Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60)]);
    let two = |start: f32| {
        let mut p = project(&spec_a, &[]);
        let q = project(&spec_b(start), &[]);
        p.tracks.extend(q.tracks);
        p
    };
    let mut cache = streaming();
    let snap = attach(&two(0.0), &mut cache);
    let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
    u.configure(&spec_a, &snap.tracks[0].assets);
    std::thread::sleep(Duration::from_millis(100));
    u.note_on(einst::NoteOn {
        key: 71,
        velocity: 1.0,
        id: 1,
        channel: 0,
    });
    // A slow disk (10 ms a block): the start blocks' reads take long enough to
    // starve the voice if they came first.
    einst::stream::set_read_delay(Duration::from_millis(10));
    let mut late = 0;
    let mut held = Vec::new();
    for block in 0..300 {
        let t = Instant::now();
        if block % 10 == 5 {
            // An edit of the other zone's start, compiled as the application does.
            held.push(attach(&two(30_000.0 + block as f32 * 1_500.0), &mut cache));
        }
        let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
        u.process(&mut l, &mut r);
        late += u.take_late_reads();
        if let Some(rest) = Duration::from_millis(10).checked_sub(t.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    einst::stream::set_read_delay(Duration::ZERO);
    assert_eq!(late, 0, "the playing voice was served first");
}

/// Third review #6: the engine handle holds back a snapshot whose zone starts are not read
/// yet — never waiting itself — and sends it from the next frames once they are.
#[test]
fn a_snapshot_waits_beside_for_its_starts() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 480_000, 131);
    let zone = |start: f32| {
        let mut z = Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60);
        z.set(cantus_core::inst::sampler::zf::START, start);
        spec_of(&[z])
    };
    let mut h = EngineHandle::dummy();
    h.samples.set_streaming(true);
    let s = h.compile(&project(&zone(0.0), &[]), 1);
    h.set_snapshot(s);
    // A full-range zone keeps the blocks after its head ready too: sent once they are.
    wait_until("the first starts", 2, || !h.flush_pending());
    einst::stream::set_read_delay(Duration::from_millis(5));
    let t = Instant::now();
    let s = h.compile(&project(&zone(200_000.0), &[]), 2);
    h.set_snapshot(s);
    let took = t.elapsed();
    assert!(h.snapshot_pending(), "held back for its starts");
    wait_until("the starts", 2, || !h.flush_pending());
    einst::stream::set_read_delay(Duration::ZERO);
    assert!(took < Duration::from_millis(8), "{took:?}");
}

/// Third review #8: a fresh note played fast on a sample dropped across the keyboard (root
/// 60) — Draft and Normal, 16 to 48 frames of its file a frame — streams in real time as in
/// memory, never late: the blocks right after each level's head are kept ready, and the
/// reader keeps up below the head-only bound.
#[test]
fn fast_fresh_notes_stream_in_real_time_as_in_memory() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    for (i, (quality, key)) in [(0.0, 108u8), (0.0, 120), (0.0, 127), (1.0, 108), (1.0, 127)]
        .into_iter()
        .enumerate()
    {
        let src = file(dir.path(), &format!("f{i}.wav"), 600_000, 140 + i as u32);
        // From its cache file, as a later session plays it: nothing of it in the pool.
        cached_one(&src, &cdir);
        einst::stream::hub().let_go_unheld();
        let mut spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
        spec.set("quality", quality).unwrap();
        let p = project(&spec, &[]);
        let whole = attach(&p, &mut SampleCache::new());
        let mut cache = streaming();
        let snap = attach(&p, &mut cache);
        cache.wait_starts(Duration::from_millis(250));
        let play = |assets: &einst::Assets, paced: bool| {
            let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
            u.configure(&spec, assets);
            u.note_on(einst::NoteOn {
                key,
                velocity: 1.0,
                id: 1,
                channel: 0,
            });
            let mut out = Vec::new();
            let mut late = 0;
            for _ in 0..40 {
                let t = Instant::now();
                let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
                u.process(&mut l, &mut r);
                let lt = u.take_late_reads();
                if paced && std::env::var("CANTUS_READER_DEBUG").is_ok() {
                    eprintln!(
                        "block at {:?} late {lt}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                            % 100000
                    );
                }
                late += lt;
                out.extend_from_slice(&l);
                if paced {
                    if let Some(rest) = Duration::from_millis(10).checked_sub(t.elapsed()) {
                        std::thread::sleep(rest);
                    }
                }
            }
            (out, late)
        };
        let (want, _) = play(&whole.tracks[0].assets, false);
        let (got, late) = play(&snap.tracks[0].assets, true);
        let d = got.iter().zip(&want).position(|(a, b)| a != b);
        eprintln!("quality {quality} key {key}: late {late}, first difference {d:?}");
        assert_eq!(late, 0, "quality {quality} key {key}: late");
        assert!(
            d.is_none(),
            "quality {quality} key {key}: first difference at {d:?}"
        );
    }
}

/// Third review #9: an export of a note too fast to stream says so (its count), whole
/// otherwise — the same as real time.
#[test]
fn an_export_counts_the_notes_that_played_their_start_only() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "long.wav", 600_000, 151);
    let mut spec = spec_of(&[Zone::new(Some(a.to_string_lossy().into()), 0, 127, 0)]);
    spec.set("quality", 0.0).unwrap();
    let p = project(&spec, &[(0, 2, 120, 100), (2, 2, 40, 100)]);
    let r = render(attach(&p, &mut streaming()), &opts()).unwrap();
    assert_eq!(r.cut_notes, 1, "the note 120 semitones up");
}

/// Fourth review #5: a block that fails to read for good (a bad sector) is read again once,
/// then the source is decoded — the stream plays on from the decode, every export equals
/// the in-memory render, and the recovery does not loop.
#[test]
fn a_block_that_never_reads_is_decoded_from_the_source_once() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let src = file(dir.path(), "bad.wav", 200_000, 161);
    let cf = cached(&src, &cdir);
    einst::stream::hub().let_go_unheld();
    let spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
    let p = project(&spec, &[(0, 8, 60, 100)]);
    let whole = render(attach(&p, &mut SampleCache::new()), &opts())
        .unwrap()
        .samples;
    let before = einst::stream::recoveries();
    // The bad sector is the old file's: the cache file written anew (held back here until
    // the fault is gone) is elsewhere on the disk.
    einst::stream::set_write_delay(Duration::from_millis(1_500));
    einst::stream_cache::set_read_fault(Some((cf.clone(), 9)));
    let t = Instant::now();
    let out = render(attach(&p, &mut streaming()), &opts())
        .unwrap()
        .samples;
    let took = t.elapsed();
    let made = einst::stream::recoveries() - before;
    einst::stream_cache::set_read_fault(None);
    einst::stream::set_write_delay(Duration::ZERO);
    assert!(
        out == whole,
        "decoded from the source: the in-memory samples"
    );
    assert!(made <= 2, "{made} recoveries: no loop");
    assert!(took < Duration::from_secs(8), "{took:?}");
}

/// Fourth review #6: a full-range zone's start edit builds its start's mip levels from one
/// read of the level-0 span the deepest level covers — not a read from the file for every
/// level — and every level still plays as in memory.
#[test]
fn a_starts_mip_levels_come_from_one_read() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 3_000_000, 181);
    cached_one(&a, &cdir);
    let zone = |start: f32| {
        let mut z = Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60);
        z.set(cantus_core::inst::sampler::zf::START, start);
        let mut s = spec_of(&[z]);
        s.set("quality", 2.0).unwrap();
        s
    };
    let mut cache = streaming();
    attach(&project(&zone(0.0), &[]), &mut cache);
    cache.wait_starts(Duration::from_secs(2));
    std::thread::sleep(Duration::from_millis(100));
    let mut last = None;
    for start in [300_000.0, 520_192.0, 1_003_520.0, 1_589_247.0] {
        let to = zone(start);
        let before = einst::stream::level0_frames_read();
        let snap = attach(&project(&to, &[]), &mut cache);
        cache.wait_starts(Duration::from_secs(5));
        assert!(cache.starts_ready());
        let read = einst::stream::level0_frames_read() - before;
        let pins = einst::sampler::start_pins(&to, &snap.tracks[0].assets);
        assert_eq!(pins.len(), 1);
        let p = pins[0];
        assert!(p.levels >= 5, "a full-range zone: {} levels", p.levels);
        let deepest = (p.count * cantus_dsp::exemplar::stream::BLOCK) << (p.levels - 1);
        let bound = deepest + deepest / 4 + (p.count + 8) * cantus_dsp::exemplar::stream::BLOCK;
        eprintln!("start {start}: read {read} level-0 frames; the deepest level spans {deepest}");
        assert!(read as usize <= bound, "start {start}: {read} > {bound}");
        last = Some((to, snap));
    }
    let (to, snap) = last.unwrap();
    // Played at its start two and four octaves up: the in-memory samples.
    let whole = attach(&project(&to, &[]), &mut SampleCache::new());
    for key in [84u8, 108] {
        let note = |assets: &einst::Assets| {
            let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
            u.configure(&to, assets);
            u.note_on(einst::NoteOn {
                key,
                velocity: 1.0,
                id: 1,
                channel: 0,
            });
            let (mut l, mut r) = (vec![0.0f32; 4_800], vec![0.0f32; 4_800]);
            u.process(&mut l, &mut r);
            (l, u.take_late_reads())
        };
        let (want, _) = note(&whole.tracks[0].assets);
        let (got, late) = note(&snap.tracks[0].assets);
        assert_eq!(late, 0, "key {key}");
        assert!(got == want, "key {key}: the in-memory samples");
    }
}

/// A project of an Exemplar track (one full-range zone starting at `start`) and an armed
/// audio track with an input.
fn exemplar_and_voice(a: &Path, start: f32, mute: bool, armed: bool) -> Project {
    let mut z = Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60);
    z.set(cantus_core::inst::sampler::zf::START, start);
    let mut p = project(&spec_of(&[z]), &[]);
    p.tracks[0].muted = mute;
    let mut v = p.make_track("Voice");
    v.input = Some(TrackInput::mono(0));
    v.armed = armed;
    p.tracks.push(v);
    p
}

fn sent_start(h: &EngineHandle) -> f32 {
    let s = h.sent_snapshot().expect("a snapshot sent");
    s.tracks[0]
        .instrument
        .native()
        .map_or(-1.0, |n| n.get("z0.start"))
}

/// Fourth review #2: while a zone's start is dragged for two seconds on a slow disk, every
/// other change — a mute here — reaches the engine in the same frame; only that zone's table
/// waits for its start's blocks, never past 250 ms from the first change it waited for.
#[test]
fn a_start_drag_never_holds_back_the_rest_of_the_project() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 2_000_000, 191);
    let (mut h, _proc) = EngineHandle::manual(48_000.0);
    h.samples.set_streaming(true);
    let s = h.compile(&exemplar_and_voice(&a, 0.0, false, false), 1);
    h.set_snapshot(s);
    wait_until("the first starts", 3, || !h.flush_pending());
    einst::stream::set_read_delay(Duration::from_millis(20));
    let t0 = Instant::now();
    // When each start was asked for: the oldest edit not landed yet is the lag.
    let mut asked: Vec<(f32, Instant)> = Vec::new();
    let mut worst = Duration::ZERO;
    let mut rev = 2;
    let mut start = 0.0f32;
    while t0.elapsed() < Duration::from_secs(2) {
        start += 7_000.0;
        asked.push((start, Instant::now()));
        let mute = rev % 2 == 0;
        h.flush_pending();
        let s = h.compile(&exemplar_and_voice(&a, start, mute, false), rev);
        h.set_snapshot(s);
        rev += 1;
        let sent = h.sent_snapshot().unwrap();
        assert_eq!(!sent.tracks[0].audible, mute, "the mute in the same frame");
        let landed = sent_start(&h);
        if let Some(&(_, at)) = asked.iter().find(|(s, _)| *s > landed) {
            worst = worst.max(at.elapsed());
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    einst::stream::set_read_delay(Duration::ZERO);
    eprintln!("the zone's table waited at most {worst:?}");
    assert!(
        worst < Duration::from_millis(250 + 40),
        "never past the absolute bound: {worst:?}"
    );
}

/// Fourth review #3: a recording started while a zone's table waits for its start records
/// the track just armed.
#[test]
fn recording_during_a_waiting_zone_change_records_the_armed_track() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 2_000_000, 192);
    let (mut h, _proc) = EngineHandle::manual(48_000.0);
    h.samples.set_streaming(true);
    let s = h.compile(&exemplar_and_voice(&a, 0.0, false, false), 1);
    h.set_snapshot(s);
    wait_until("the first starts", 3, || !h.flush_pending());
    einst::stream::set_read_delay(Duration::from_millis(20));
    let s = h.compile(&exemplar_and_voice(&a, 700_000.0, false, false), 2);
    h.set_snapshot(s);
    let s = h.compile(&exemplar_and_voice(&a, 700_000.0, false, true), 3);
    h.set_snapshot(s);
    let takes = tempfile::tempdir().unwrap();
    let armed = h.begin_recording(takes.path());
    einst::stream::set_read_delay(Duration::ZERO);
    assert_eq!(armed, Ok(1), "the armed track records");
    let _ = h.finish_recording();
}

/// Last review #1: a block that fails inside a start's chain span — outside the pin's own
/// blocks — is recovered like any other: the starts become ready, the source decoded at most
/// once more, the span not re-read look after look, and the start plays an octave up as in
/// memory.
#[test]
fn a_bad_block_in_a_starts_chain_is_recovered() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    let a = file(dir.path(), "chain.wav", 3_000_000, 201);
    let cf = cached(&a, &cdir);
    einst::stream::hub().let_go_unheld();
    let mut z = Zone::new(Some(a.to_string_lossy().into()), 0, 127, 60);
    z.set(cantus_core::inst::sampler::zf::START, 1_003_520.0);
    let mut spec = spec_of(&[z]);
    spec.set("quality", 2.0).unwrap();
    let p = project(&spec, &[]);
    let whole = attach(&p, &mut SampleCache::new());
    einst::stream_cache::set_read_fault(Some((cf.clone(), 194)));
    einst::stream::set_write_delay(Duration::from_millis(1_500));
    let (before, read_before) = (
        einst::stream::recoveries(),
        einst::stream::level0_frames_read(),
    );
    let mut cache = streaming();
    let snap = attach(&p, &mut cache);
    let pins = einst::sampler::start_pins(&spec, &snap.tracks[0].assets);
    assert_eq!(pins[0].block, 245, "the pin starts past the bad block");
    cache.wait_starts(Duration::from_secs(5));
    let ready = cache.starts_ready();
    std::thread::sleep(Duration::from_millis(300));
    let made = einst::stream::recoveries() - before;
    let read = einst::stream::level0_frames_read() - read_before;
    einst::stream_cache::set_read_fault(None);
    einst::stream::set_write_delay(Duration::ZERO);
    assert!(ready, "the starts are read");
    assert!(made <= 2, "{made} recoveries");
    assert!(
        read < 6_000_000,
        "{read} level-0 frames: not re-read look after look"
    );
    let note = |assets: &einst::Assets| {
        let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
        u.configure(&spec, assets);
        u.note_on(einst::NoteOn {
            key: 72,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let (mut l, mut r) = (vec![0.0f32; 4_800], vec![0.0f32; 4_800]);
        u.process(&mut l, &mut r);
        (l, u.take_late_reads())
    };
    let (want, _) = note(&whole.tracks[0].assets);
    let (got, late) = note(&snap.tracks[0].assets);
    assert_eq!(late, 0);
    assert!(got == want, "an octave up: the in-memory samples");
}

/// Last review #2: in ordinary playback — the real reader, a fresh note at unity past its
/// head, callbacks at real-time pace, nothing pre-read — a voice crossing into blocks the
/// reader already keeps ready wakes it not once, on a normal disk and a slow one.
#[test]
fn ordinary_playback_wakes_the_reader_not_once() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cdir = cache_here(dir.path());
    for (i, delay) in [0u64, 2].into_iter().enumerate() {
        let a = file(
            dir.path(),
            &format!("unity{i}.wav"),
            1_500_000,
            211 + i as u32,
        );
        cached_one(&a, &cdir);
        einst::stream::hub().let_go_unheld();
        einst::stream::set_read_delay(Duration::from_millis(delay));
        let spec = spec_of(&[Zone::new(Some(a.to_string_lossy().into()), 60, 60, 60)]);
        let mut cache = streaming();
        let snap = attach(&project(&spec, &[]), &mut cache);
        cache.wait_starts(Duration::from_secs(2));
        let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
        u.configure(&spec, &snap.tracks[0].assets);
        u.note_on(einst::NoteOn {
            key: 60,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
        let (mut wakes, mut late) = (0, 0);
        for _ in 0..300 {
            let t = Instant::now();
            let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
            u.process(&mut l, &mut r);
            wakes += u.take_stream_wakes();
            late += u.take_late_reads();
            if let Some(rest) = Duration::from_millis(10).checked_sub(t.elapsed()) {
                std::thread::sleep(rest);
            }
        }
        einst::stream::set_read_delay(Duration::ZERO);
        eprintln!("read delay {delay} ms: {wakes} wakes, {late} late in 3 s (~31 blocks)");
        assert_eq!(late, 0, "delay {delay}");
        assert!(wakes <= 1, "delay {delay}: {wakes} wakes in steady state");
    }
}
