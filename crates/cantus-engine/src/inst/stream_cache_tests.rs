//! The cache files (ADR 0050 §2; the round-26 review's 7, 9, 10, 11, 12, 20).

use super::*;

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn the_depth_is_the_narrowest_that_gives_the_bits_back_exactly() {
    let i16s: Vec<f32> = (-5..5).map(|k| k as f32 * 1_000.0 / 32_768.0).collect();
    let i24s: Vec<f32> = vec![-1.0, 3.0 / 8_388_608.0, 0.5, 8_388_607.0 / 8_388_608.0];
    let f32s = vec![0.1f32, -0.3];
    // Review 12: a negative zero has no integer form.
    let negzero = vec![0.5f32, -0.0, 0.25];
    for (data, depth) in [
        (i16s, Depth::I16),
        (i24s, Depth::I24),
        (f32s, Depth::F32),
        (negzero, Depth::F32),
    ] {
        assert_eq!(exact_depth(&data), depth);
        let mut bytes = Vec::new();
        encode(&data, depth, &mut bytes);
        let mut back = vec![0.0; data.len()];
        decode(&bytes, depth, &mut back);
        assert_eq!(bits(&back), bits(&data), "{depth:?}");
    }
    assert_eq!(
        exact_depth(&[1.0]),
        Depth::F32,
        "+1.0 fits no integer depth"
    );
}

fn source(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn data(frames: usize) -> Vec<f32> {
    (0..frames * 2)
        .map(|i| ((i * 7_919) % 600) as f32 / 8_388_608.0)
        .collect()
}

#[test]
fn a_cache_file_round_trips_block_by_block_and_names_its_source() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), "a.flac", b"source bytes");
    let id = identity(&src).unwrap();
    let d = data(10_000);
    let cache = dir.path().join("cache");
    let c = write(&cache, &src, &id, 2, 48_000, &d).unwrap();
    assert_eq!((c.depth, c.frames), (Depth::I24, 10_000));
    assert_eq!(open(&cache, &src, &id), Some(c.clone()));
    let mut f = File::open(&c.path).unwrap();
    let mut out = vec![0.0; 2 * 2 * BLOCK];
    let n = c.read_blocks(&mut f, 1, 2, &mut out).unwrap();
    assert_eq!(n, 10_000 - BLOCK, "the second block and the short last one");
    assert_eq!(bits(&out[..n * 2]), bits(&d[2 * BLOCK..]));
    // Nothing but the file itself is left.
    let names: Vec<_> = std::fs::read_dir(&cache).unwrap().flatten().collect();
    assert_eq!(names.len(), 1);
}

#[test]
fn a_changed_source_is_never_answered_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let src = source(dir.path(), "a.flac", &[1u8; 9_000]);
    let id = identity(&src).unwrap();
    write(&cache, &src, &id, 2, 48_000, &data(5_000)).unwrap();
    // Review 7: the same size and, as far as the system says, the same time — but other
    // bytes: the fingerprint tells them apart.
    let t = std::fs::metadata(&src).unwrap().modified().unwrap();
    std::fs::write(&src, [2u8; 9_000]).unwrap();
    File::options()
        .write(true)
        .open(&src)
        .unwrap()
        .set_modified(t)
        .unwrap();
    let now = identity(&src).unwrap();
    assert_eq!((now.size, now.mtime), (id.size, id.mtime));
    assert_ne!(now, id);
    assert_eq!(open(&cache, &src, &now), None);
}

#[test]
fn a_file_dated_before_1970_has_an_identity() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), "old.wav", b"old bytes");
    let past = std::time::UNIX_EPOCH - std::time::Duration::from_secs(86_400);
    File::options()
        .write(true)
        .open(&src)
        .unwrap()
        .set_modified(past)
        .unwrap();
    let id = identity(&src).expect("review 9: a pre-1970 file is still a file");
    assert!(id.mtime < 0);
    let cache = dir.path().join("cache");
    write(&cache, &src, &id, 2, 48_000, &data(100)).unwrap();
    assert!(open(&cache, &src, &id).is_some());
}

#[test]
fn a_damaged_block_is_refused_and_so_is_a_damaged_header() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), "a.flac", b"src");
    let id = identity(&src).unwrap();
    let cache = dir.path().join("cache");
    let c = write(&cache, &src, &id, 2, 48_000, &data(3 * BLOCK)).unwrap();
    // Review 10: zeroes where the data of the second block was (a lost write).
    let mut bytes = std::fs::read(&c.path).unwrap();
    let at = bytes.len() - 2 * BLOCK * 6 + 100;
    bytes[at..at + 600].fill(0);
    std::fs::write(&c.path, &bytes).unwrap();
    let c = open(&cache, &src, &id).expect("the header is whole");
    let mut f = File::open(&c.path).unwrap();
    let mut out = vec![0.0; 2 * BLOCK];
    assert!(c.read_blocks(&mut f, 0, 1, &mut out).is_ok());
    assert!(matches!(
        c.read_blocks(&mut f, 1, 1, &mut out),
        Err(ReadError::Damaged)
    ));
    // A damaged header (the source's path) is not ours at all.
    bytes[FIXED] ^= 0xFF;
    std::fs::write(&c.path, &bytes).unwrap();
    assert_eq!(open(&cache, &src, &id), None);
}

#[test]
fn a_sweep_removes_stale_parts_gone_sources_and_the_least_recently_used() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let mut made = Vec::new();
    for i in 0..4 {
        let src = source(
            dir.path(),
            &format!("s{i}.flac"),
            format!("src {i}").as_bytes(),
        );
        let id = identity(&src).unwrap();
        let c = write(&cache, &src, &id, 2, 48_000, &data(BLOCK)).unwrap();
        // Oldest first.
        File::options()
            .write(true)
            .open(&c.path)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(100 - i))
            .unwrap();
        made.push((src, c.path));
    }
    // Review 20: a stray part from a crash an hour ago, and one being written now.
    let old = cache.join("dead.part999");
    std::fs::write(&old, b"half").unwrap();
    File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3_600))
        .unwrap();
    let fresh = cache.join("live.part1");
    std::fs::write(&fresh, b"half").unwrap();
    // Review 11: the second source is removed (its pack uninstalled).
    std::fs::remove_file(&made[1].0).unwrap();
    let one = std::fs::metadata(&made[0].1).unwrap().len();
    // Room for two: the oldest of the three left goes, unless it is in use.
    let in_use = |p: &Path| p == made[0].1;
    let s = sweep(&cache, 2 * one, &in_use, &[]);
    assert!(!old.exists() && fresh.exists());
    assert!(!made[1].1.exists(), "its source is gone");
    assert!(made[0].1.exists(), "in use: kept");
    assert!(!made[2].1.exists(), "the least recently used of the rest");
    assert!(made[3].1.exists());
    assert_eq!(s.bytes, 2 * one);
    assert_eq!(size_of(&cache), 2 * one);
}

fn aged(p: &Path, secs: u64) {
    File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(secs))
        .unwrap();
}

fn entry(dir: &Path, cache: &Path, name: &str, age: u64) -> (PathBuf, PathBuf) {
    if let Some(d) = dir.join(name).parent() {
        std::fs::create_dir_all(d).unwrap();
    }
    let src = source(dir, name, name.as_bytes());
    let id = identity(&src).unwrap();
    let c = write(cache, &src, &id, 2, 48_000, &data(BLOCK)).unwrap();
    aged(&c.path, age);
    (src, c.path)
}

/// Second review #6: a file a stream reads — held open with its shared lock, as another
/// Cantus does — is never deleted, by the size limit nor by *Clear cache*; once let go it is.
#[test]
fn a_sweep_never_deletes_a_file_a_stream_holds() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let (_, held) = entry(dir.path(), &cache, "a.flac", 3_600);
    let f = open_locked(&held).expect("open and locked");
    let s = sweep(&cache, 0, &|_| false, &[]);
    assert!(held.exists(), "streamed by another Cantus: kept ({s:?})");
    drop(f);
    sweep(&cache, 0, &|_| false, &[]);
    assert!(!held.exists(), "let go: removed");
}

/// Second review #6: a file a load touched since it was listed (it opened it, about to
/// stream) is never deleted — the grace window is checked again at each deletion.
#[test]
fn a_file_touched_while_the_sweep_runs_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let (src, touched) = entry(dir.path(), &cache, "a.flac", 3_600);
    let id = identity(&src).unwrap();
    // The load runs while the sweep decides: modelled by the check the sweep makes.
    let in_use = |p: &Path| {
        if p == touched {
            assert!(open(&cache, &src, &id).is_some(), "the touch");
        }
        false
    };
    sweep(&cache, 0, &in_use, &[]);
    assert!(touched.exists(), "touched during the sweep: kept");
}

/// Second review #7: a source whose drive is away (its folder is not there) is not gone —
/// its entry stays; one deleted from a folder that is still there goes, and so does one
/// under a folder known to be removed (a pack uninstalled).
#[test]
fn a_source_whose_drive_is_away_is_not_gone() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let (_, away) = entry(dir.path(), &cache, "drive/lib/a.flac", 3_600);
    let (deleted, gone) = entry(dir.path(), &cache, "here/b.flac", 3_600);
    source(&dir.path().join("here"), "other.flac", b"other");
    let (_, pack) = entry(dir.path(), &cache, "pack/c.flac", 3_600);
    std::fs::rename(dir.path().join("drive"), dir.path().join("unplugged")).unwrap();
    std::fs::remove_file(&deleted).unwrap();
    std::fs::remove_dir_all(dir.path().join("pack")).unwrap();
    sweep(&cache, 1 << 40, &|_| false, &[dir.path().join("pack")]);
    assert!(away.exists(), "the drive is only away");
    assert!(!gone.exists(), "deleted from its folder");
    assert!(!pack.exists(), "its pack removed");
}

/// Second review #8: sweeps at once delete only the excess, never twice it.
#[test]
fn sweeps_at_once_delete_only_the_excess() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let made: Vec<PathBuf> = (0..24)
        .map(|i| entry(dir.path(), &cache, &format!("s{i:02}.flac"), 3_600 + i).1)
        .collect();
    let one = std::fs::metadata(&made[0]).unwrap().len();
    let start = std::sync::Barrier::new(4);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                start.wait();
                sweep(&cache, 12 * one, &|_| false, &[]);
            });
        }
    });
    let left = made.iter().filter(|p| p.exists()).count();
    assert_eq!(left, 12, "the excess, once");
    assert_eq!(size_of(&cache), 12 * one);
}

/// Third review #2: a removed pack's entry a stream held while the pack was removed goes at
/// a later sweep, once let go — the removed folder is remembered beside the cache until no
/// entry under it is left.
#[test]
fn a_removed_packs_entry_held_at_removal_goes_at_a_later_sweep() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let (_, held) = entry(dir.path(), &cache, "pack/samples/a.flac", 3_600);
    let f = open_locked(&held).expect("a stream reads it");
    std::fs::remove_dir_all(dir.path().join("pack")).unwrap();
    sweep(&cache, 1 << 40, &|_| false, &[dir.path().join("pack")]);
    assert!(held.exists(), "held: kept for now");
    drop(f);
    sweep(&cache, 1 << 40, &|_| false, &[]);
    assert!(!held.exists(), "let go: a later sweep takes it");
    assert!(
        !cache.join(REMOVED_ROOTS).exists(),
        "nothing left under it: forgotten"
    );
}
