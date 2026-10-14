//! The round-26 review of streaming (ADR 0050): edits keep a streamed Exemplar playing and
//! make no new streams (13, 14), streams nobody holds are released (6, 14), a zone that
//! starts past its head and zones that share a file play exactly as in memory (5, 13), the
//! cache never serves another file's audio (7), an unwritable cache plays from memory and
//! says so (8, 17), old files load (9), and loads an offline render waited for still reach
//! the live engine (4, 15, 26).

use cantus_core::inst::sampler::{zf, Zone};
use cantus_core::*;
use cantus_engine::inst as einst;
use cantus_engine::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

fn cache_here(dir: &Path) {
    // The machine's disk may be nearly full: these tests' cache files are small.
    einst::stream::set_free_floor(0);
    einst::stream::hub().set_cache_dir(dir.join("cache"));
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

fn file(dir: &Path, name: &str, frames: usize, seed: u32) -> String {
    let p = dir.join(name);
    write_wav24(&p, frames, seed);
    p.to_string_lossy().into_owned()
}

fn decode(p: &Path) -> Result<SampleData, String> {
    SampleCache::new().load(p).map(|a| (*a).clone())
}

/// Review 13, 14: tune, transpose, bend, start.vel and a zone's start and range change
/// nothing about a file's stream: every zone keeps its audio at once, no new stream.
#[test]
fn edits_keep_every_streamed_zone_playing_and_make_no_new_stream() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 200_000, 1);
    let b = file(dir.path(), "b.wav", 150_000, 2);
    let zones = [
        Zone::new(Some(a), 30, 60, 48),
        Zone::new(Some(b), 61, 100, 72),
    ];
    let spec = spec_of(&zones);
    let mut cache = streaming();
    cache.set_background(true);
    let mut s = Snapshot::compile(&project(&spec, &[]), 1);
    cache.attach(&mut s);
    cache.wait_assets();
    let first = attach(&project(&spec, &[]), &mut cache);
    let heads: Vec<_> = first.tracks[0]
        .assets
        .iter()
        .map(|(_, a)| a.clone().unwrap())
        .collect();
    assert!(heads.iter().all(|h| einst::stream::lookup(h).is_some()));
    for (k, v) in [
        ("tune", 7.0),
        ("transpose", 13.0),
        ("bend", 24.0),
        ("start.vel", 800.0),
        ("z0.start", 90_000.0),
        ("z1.hi", 127.0),
        ("z1.root", 30.0),
    ] {
        let mut s = spec.clone();
        s.set(k, v).unwrap();
        let mut snap = Snapshot::compile(&project(&s, &[]), 2);
        assert!(cache.attach(&mut snap).is_empty());
        assert_eq!(cache.progress(), None, "{k}: nothing to load again");
        for (h, (_, now)) in heads.iter().zip(&snap.tracks[0].assets) {
            assert!(
                Arc::ptr_eq(h, now.as_ref().unwrap()),
                "{k}: the same stream"
            );
        }
    }
}

/// Review 6, 14: a stream no snapshot holds any more is released.
#[test]
fn a_stream_nothing_holds_is_released() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 120_000, 3);
    let spec = spec_of(&[Zone::new(Some(a), 0, 127, 60)]);
    let mut cache = streaming();
    let snap = attach(&project(&spec, &[]), &mut cache);
    let head = snap.tracks[0].assets[0].1.clone().unwrap();
    let weak = Arc::downgrade(&head);
    let stream = Arc::downgrade(&einst::stream::lookup(&head).unwrap());
    drop(head);
    // Still in a snapshot: kept.
    attach(&Project::new("empty"), &mut cache);
    assert!(weak.upgrade().is_some());
    drop(snap);
    attach(&Project::new("empty"), &mut cache);
    assert!(weak.upgrade().is_none(), "the head is gone");
    // The reader thread drops its copy as it next looks.
    let t = std::time::Instant::now();
    while stream.upgrade().is_some() && t.elapsed() < std::time::Duration::from_secs(2) {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(stream.upgrade().is_none(), "the stream is gone");
    assert_eq!(cache.streamed_files(), 0);
}

fn render_offline(p: &Project, cache: &mut SampleCache) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.5,
        ..Default::default()
    };
    render(attach(p, cache), &opts).unwrap().samples
}

/// Review 13 (starts) and 5 (shared files): a zone that starts past its head, a velocity
/// start, and two zones of different ranges sharing one file — legato carrying a note past
/// the lower zone's headroom — render streamed exactly as in memory.
#[test]
fn late_starts_and_shared_files_stream_exactly_as_in_memory() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 300_000, 5);
    let mut late = Zone::new(Some(a.clone()), 20, 39, 30);
    late.set(zf::START, 120_000.0);
    let low = Zone::new(Some(a.clone()), 40, 50, 45);
    let high = Zone::new(Some(a), 51, 100, 45);
    let mut spec = spec_of(&[late, low, high]);
    spec.set("start.vel", 900.0).unwrap();
    let mut legato = spec.clone();
    legato.set("mode", 2.0).unwrap();
    type Notes<'a> = &'a [(u32, u32, u8, u8)];
    let cases: [(&NativeSpec, Notes); 2] = [
        (&spec, &[(0, 3, 30, 40), (1, 2, 25, 127), (4, 4, 45, 90)]),
        (&legato, &[(0, 6, 45, 90), (2, 4, 100, 90)]),
    ];
    for (s, notes) in cases {
        let p = project(s, notes);
        let whole = render_offline(&p, &mut SampleCache::new());
        let streamed = render_offline(&p, &mut streaming());
        assert!(whole.iter().any(|x| *x != 0.0));
        let d = streamed.iter().zip(&whole).position(|(a, b)| a != b);
        assert!(d.is_none(), "first difference at {d:?}");
    }
}

/// Review 7: a file replaced while it is being decoded plays what was read, and the cache
/// never answers for the new file with it.
#[test]
fn a_file_changed_while_it_is_read_is_never_cached_as_the_new_one() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let src = PathBuf::from(file(dir.path(), "a.wav", 200_000, 7));
    let replace = |p: &Path| {
        let d = decode(p);
        write_wav24(p, 210_000, 8);
        d
    };
    match einst::stream::load(&src, &replace).unwrap() {
        einst::stream::Loaded::Whole { data, note } => {
            assert_eq!(data.len_frames(), 200_000, "what was read");
            assert_eq!(note, Some(einst::stream::LoadNote::Changed(src.clone())));
        }
        einst::stream::Loaded::Streamed(_) => panic!("cached the old audio"),
    }
    let decoded = std::cell::Cell::new(false);
    let watch = |p: &Path| {
        decoded.set(true);
        decode(p)
    };
    let again = einst::stream::load(&src, &watch).unwrap();
    assert!(decoded.get(), "the new file is decoded");
    match again {
        einst::stream::Loaded::Streamed(h) => {
            assert_eq!(einst::stream::lookup(&h).unwrap().frames[0], 210_000)
        }
        other => panic!("{other:?}"),
    }
}

/// Review 8, 17, 9: a cache that cannot be written leaves the sample playing from memory
/// (said once); a file dated before 1970 streams.
#[test]
fn an_unwritable_cache_plays_from_memory_and_an_old_file_streams() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("not-a-folder");
    std::fs::write(&blocked, b"a file where the cache folder should be").unwrap();
    einst::stream::hub().set_cache_dir(blocked);
    let a = file(dir.path(), "a.wav", 150_000, 9);
    let spec = spec_of(&[Zone::new(Some(a), 0, 127, 60)]);
    let mut cache = streaming();
    let snap = attach(&project(&spec, &[]), &mut cache);
    let got = snap.tracks[0].assets[0]
        .1
        .clone()
        .expect("the zone has its audio");
    // It plays from its decode, which it keeps since no cache file can be written.
    let t = std::time::Instant::now();
    let mut notes = Vec::new();
    while notes.is_empty() && t.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(10));
        notes = cache.take_notes();
    }
    assert!(einst::stream::memory_backed(&got), "from memory, for good");
    let p = project(&spec, &[(0, 4, 60, 100)]);
    assert!(
        render_offline(&p, &mut cache) == render_offline(&p, &mut SampleCache::new()),
        "the in-memory samples"
    );
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, einst::stream::LoadNote::CacheUnwritable(_))),
        "{notes:?}"
    );
    assert!(cache.take_notes().is_empty(), "said once");
    // A file dated before 1970.
    cache_here(dir.path());
    let old = PathBuf::from(file(dir.path(), "old.wav", 150_000, 10));
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH - std::time::Duration::from_secs(86_400))
        .unwrap();
    let l = einst::stream::load(&old, &decode);
    assert!(matches!(l, Ok(einst::stream::Loaded::Streamed(_))), "{l:?}");
}

/// Review 4, 15, 26: loads an offline render waited for are still reported to the next
/// poll, so the live engine gets them too.
#[test]
fn loads_an_offline_render_waited_for_still_reach_the_next_poll() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 100_000, 11);
    let spec = spec_of(&[Zone::new(Some(a), 0, 127, 60)]);
    let mut cache = streaming();
    cache.set_background(true);
    let mut s = Snapshot::compile(&project(&spec, &[]), 1);
    cache.attach(&mut s);
    assert!(cache.wait_assets());
    assert!(cache.poll_loaded(), "the next poll says they arrived");
    assert!(!cache.poll_loaded(), "once");
}

/// Review 13: in real time, a zone that starts past its head finds its first blocks in the
/// pool — its start is kept ready from the moment the zone exists — so the note is not late.
#[test]
fn a_late_start_is_kept_ready_and_plays_in_real_time() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 300_000, 12);
    let mut z = Zone::new(Some(a), 0, 127, 60);
    z.set(zf::START, 150_000.0);
    let spec = spec_of(&[z]);
    let p = project(&spec, &[]);
    let whole = attach(&p, &mut SampleCache::new());
    let streamed = attach(&p, &mut streaming());
    let play = |assets: &einst::Assets, wait: bool| {
        let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
        u.configure(&spec, assets);
        if wait {
            // The reader's next looks (it sleeps 20 ms between them when nothing reads).
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
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
    let (m, _) = play(&whole.tracks[0].assets, false);
    let (s, late) = play(&streamed.tracks[0].assets, true);
    assert_eq!(late, 0, "the start's blocks were ready");
    assert!(m.iter().any(|x| *x != 0.0));
    assert!(s == m, "the same samples");
}

/// Review 10: a cache file whose data was lost after its header (a crash before the disk
/// had it) is never played as silence or garbage: the block's checksum fails, the file is
/// deleted, and the source is decoded again — in the same session, the render waiting for
/// it (second review #3: the in-memory samples), and a whole file written in its place.
#[test]
fn a_damaged_cache_block_is_never_played_and_the_source_is_decoded_again() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let src = PathBuf::from(file(dir.path(), "a.wav", 200_000, 13));
    // Written once (beside the load: waited for), then its data zeroed past the head.
    let first = einst::stream::load(&src, &decode).unwrap();
    if let einst::stream::Loaded::Streamed(h) = &first {
        let t = std::time::Instant::now();
        while einst::stream::memory_backed(h) && t.elapsed() < std::time::Duration::from_secs(20) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    drop(first);
    let cached: Vec<PathBuf> = std::fs::read_dir(dir.path().join("cache"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(cached.len(), 1);
    let mut bytes = std::fs::read(&cached[0]).unwrap();
    let n = bytes.len();
    bytes[n - 400_000..n - 100_000].fill(0);
    // The stream that wrote it lets go of it (and of its lock) as the reader next looks.
    einst::stream::hub().let_go_unheld();
    let t = std::time::Instant::now();
    while std::fs::write(&cached[0], &bytes).is_err() {
        assert!(t.elapsed() < std::time::Duration::from_secs(5), "let go");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let damaged = std::fs::read(&cached[0]).unwrap();
    let spec = spec_of(&[Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60)]);
    let p = project(&spec, &[(0, 8, 60, 100)]);
    let whole = render_offline(&p, &mut SampleCache::new());
    let t = std::time::Instant::now();
    let out = render_offline(&p, &mut streaming());
    assert!(t.elapsed() < std::time::Duration::from_secs(30));
    assert!(
        out == whole,
        "the damaged blocks never played: the source's samples"
    );
    // The damaged file goes; a whole one is written in its place.
    let t = std::time::Instant::now();
    while std::fs::read(&cached[0]).is_ok_and(|b| b == damaged)
        && t.elapsed() < std::time::Duration::from_secs(20)
    {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let t = std::time::Instant::now();
    while !cached[0].exists() && t.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(std::fs::read(&cached[0]).unwrap() != damaged, "rewritten");
    assert!(einst::stream::read_whole(&src).unwrap().frames == decode(&src).unwrap().frames);
}

/// A note pitched up two or three octaves that starts past its head reads a higher mip level
/// from its first sample: its start is kept ready at that level too — no late read, the
/// in-memory samples.
#[test]
fn a_late_start_two_or_three_octaves_up_is_kept_ready_at_its_level() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 600_000, 14);
    let mut z = Zone::new(Some(a), 0, 127, 60);
    z.set(zf::START, 200_000.0);
    let spec = spec_of(&[z]);
    let p = project(&spec, &[]);
    let whole = attach(&p, &mut SampleCache::new());
    let streamed = attach(&p, &mut streaming());
    for key in [84u8, 96] {
        let play = |assets: &einst::Assets, wait: bool| {
            let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
            u.configure(&spec, assets);
            if wait {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
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
        let (m, _) = play(&whole.tracks[0].assets, false);
        let (s, late) = play(&streamed.tracks[0].assets, true);
        assert_eq!(late, 0, "+{} st: the start's blocks were ready", key - 60);
        assert!(m.iter().any(|x| *x != 0.0));
        assert!(s == m, "+{} st: the same samples", key - 60);
    }
}

/// A first load plays at once from the decoded data while its cache file is written beside
/// it, then switches to the file: the same samples before and after the switch, and the
/// decoded data let go after it.
#[test]
fn a_first_load_plays_from_its_decode_until_the_cache_file_is_written() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    cache_here(dir.path());
    let a = file(dir.path(), "a.wav", 400_000, 15);
    let spec = spec_of(&[Zone::new(Some(a), 0, 127, 60)]);
    let p = project(&spec, &[(0, 6, 60, 100), (1, 4, 67, 90)]);
    let whole = render_offline(&p, &mut SampleCache::new());
    einst::stream::set_write_delay(std::time::Duration::from_millis(1_500));
    let mut cache = streaming();
    let t = std::time::Instant::now();
    let snap = attach(&p, &mut cache);
    assert!(
        t.elapsed() < std::time::Duration::from_millis(1_000),
        "the load did not wait for the file: {:?}",
        t.elapsed()
    );
    let head = snap.tracks[0].assets[0].1.clone().unwrap();
    assert!(
        einst::stream::memory_backed(&head),
        "playing from the decode"
    );
    let opts = RenderOptions {
        tail_seconds: 0.5,
        ..Default::default()
    };
    let before = render(snap.clone(), &opts).unwrap().samples;
    assert!(before == whole, "from the decode: the in-memory samples");
    let t = std::time::Instant::now();
    while einst::stream::memory_backed(&head) && t.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    einst::stream::set_write_delay(std::time::Duration::ZERO);
    assert!(
        !einst::stream::memory_backed(&head),
        "switched to the file, the decode let go"
    );
    let after = render(snap, &opts).unwrap().samples;
    assert!(after == whole, "from the file: the same samples");
}
