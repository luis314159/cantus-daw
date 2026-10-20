//! Reading a `.dawproject` from a file (round 29's second review): the archive listed from
//! its directory, its audio copied straight to disk (a ZIP64 archive too, never whole in
//! memory), a part too large refused cleanly, every entry its own file; ZIP64 written as
//! the format says (checked by hand against APPNOTE) and read when another tool wrote it.

use cantus_core::*;
use cantus_project::dawproject::{self, zip};

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dawproject")
        .join(name)
}

/// A project with one audio clip of `bytes`, its file in `dir`.
fn project(dir: &std::path::Path, bytes: &[u8]) -> Project {
    let mut p = Project::new("Import");
    let mut t = p.make_track("Audio");
    std::fs::write(dir.join("take.wav"), bytes).unwrap();
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: "take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000,
        sha256: None,
        recorded: false,
    });
    let c = p.make_audio_clip("take", id, Ticks::ZERO, Ticks::beats(4));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// Finding 26, 51: a ZIP64 archive is read from its file — its audio copied to the media
/// folder byte for byte, the source pointed at it — and a part past the limit refuses the
/// file with a word, never an allocation of it.
#[test]
fn a_zip64_archive_is_read_from_its_file_its_audio_streamed() {
    let dir = tempfile::tempdir().unwrap();
    let audio: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let p = project(dir.path(), &audio);
    let file = dir.path().join("song.dawproject");
    // Every 32-bit field past 1 000 bytes: ZIP64 records everywhere.
    dawproject::export_to_within(&p, "test", Some(dir.path()), &file, 1_000).unwrap();
    let bytes = std::fs::read(&file).unwrap();
    assert!(
        bytes.windows(4).any(|w| w == [0x50, 0x4B, 0x06, 0x06]),
        "ZIP64"
    );
    let media = dir.path().join("media");
    let got = dawproject::import_file(&file, &media).unwrap();
    assert!(got.unwritten.is_empty());
    let s = &got.project.audio_pool[0];
    let path = std::path::Path::new(&s.path);
    assert!(path.starts_with(&media), "{}", s.path);
    assert_eq!(std::fs::read(path).unwrap(), audio);
    // A part past the limit: refused, said.
    let small = dawproject::import_file_within(&file, &dir.path().join("m2"), 64);
    assert!(
        matches!(
            small,
            Err(dawproject::DawProjectError::Zip(zip::ZipError::TooLarge(_)))
        ),
        "{small:?}"
    );
}

/// Findings 28, 50: a file the archive references outside itself, by a relative path, is
/// found next to the archive.
#[test]
fn an_external_file_next_to_the_archive_is_found() {
    let dir = tempfile::tempdir().unwrap();
    let p = project(dir.path(), b"RIFF");
    let mut bytes = dawproject::export_with(&p, "test", Some(dir.path())).bytes;
    // The archive without the file: rewritten to point at it, external, by a relative path.
    let files = zip::read(&bytes).unwrap();
    let mut z = zip::ZipWriter::new();
    for (name, data) in &files {
        if name == "project.xml" {
            let xml = String::from_utf8(data.clone()).unwrap();
            let inner = files
                .iter()
                .map(|f| f.0.clone())
                .find(|n| n.starts_with("audio/"))
                .unwrap();
            let xml = xml.replace(
                &format!("path=\"{inner}\" external=\"false\""),
                "path=\"sounds/take.wav\" external=\"true\"",
            );
            z.add(name, xml.as_bytes(), true);
        } else if !name.starts_with("audio/") {
            z.add(name, data, true);
        }
    }
    bytes = z.finish();
    let there = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(there.path().join("sounds")).unwrap();
    std::fs::write(there.path().join("sounds/take.wav"), b"RIFF").unwrap();
    let file = there.path().join("song.dawproject");
    std::fs::write(&file, &bytes).unwrap();
    let got = dawproject::import_file(&file, &there.path().join("media")).unwrap();
    let s = &got.project.audio_pool[0];
    assert_eq!(
        std::path::Path::new(&s.path),
        there.path().join("sounds/take.wav")
    );
}

/// Finding 36: entries of one name in two folders become two files — their folders kept,
/// nothing escaping the media folder, a name the Windows file system would take for
/// another numbered.
#[test]
fn every_entry_is_its_own_file() {
    let dir = std::path::Path::new("C:/media");
    let mut taken = std::collections::HashSet::new();
    let a = dawproject::media_path(dir, "audio/take1/vox.wav", &mut taken);
    let b = dawproject::media_path(dir, "audio/take2/vox.wav", &mut taken);
    let c = dawproject::media_path(dir, "audio/take1/Vox.wav", &mut taken);
    let d = dawproject::media_path(dir, "../../windows/evil.wav", &mut taken);
    let e = dawproject::media_path(dir, "C:/abs/x.wav", &mut taken);
    assert_eq!(a, dir.join("audio").join("take1").join("vox.wav"));
    assert_eq!(b, dir.join("audio").join("take2").join("vox.wav"));
    assert_eq!(c, dir.join("audio").join("take1").join("Vox 2.wav"));
    assert_eq!(d, dir.join("windows").join("evil.wav"));
    assert_eq!(e, dir.join("abs").join("x.wav"));
}

/// Finding 43: an archive another tool wrote with ZIP64 records everywhere (Python's
/// `zipfile`, its limits lowered: `python-zip64.py`) reads.
#[test]
fn a_zip64_archive_another_tool_wrote_reads() {
    let bytes = std::fs::read(fixture("python-zip64.zip")).unwrap();
    let files = zip::read(&bytes).unwrap();
    assert_eq!(files[0].0, "project.xml");
    assert_eq!(files[0].1, b"<Project version=\"1.0\"/>");
    assert_eq!(files[1].0, "audio/take 1/vox.wav");
    let want: Vec<u8> = (0..40).flat_map(|_| 0..=255u8).collect();
    assert_eq!(files[1].1, want);
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

/// Finding 43: what our writer puts down for ZIP64, read by hand at the places APPNOTE
/// 6.3.10 gives (never through our own reader): the local header's sizes in its extra, the
/// central record's full fields in order (size, compressed size, offset), each entry's
/// version needed the same in both, the ZIP64 end record and its locator.
#[test]
fn our_zip64_records_are_where_appnote_puts_them() {
    let mut z = zip::ZipStream::with_limit(std::io::Cursor::new(Vec::new()), 10);
    z.add("a.bin", &[7u8; 20], false).unwrap();
    z.add("b.bin", &[9u8; 5], false).unwrap();
    let b = z.finish().unwrap().into_inner();
    // The first local header: version 45, both sizes full, a 20-byte extra (id 1, 16).
    assert_eq!(u32_at(&b, 0), 0x0403_4B50);
    assert_eq!(u16_at(&b, 4), 45);
    assert_eq!((u32_at(&b, 18), u32_at(&b, 22)), (u32::MAX, u32::MAX));
    let (name, extra) = (u16_at(&b, 26) as usize, u16_at(&b, 28) as usize);
    assert_eq!((name, extra), (5, 20));
    let x = 30 + name;
    assert_eq!((u16_at(&b, x), u16_at(&b, x + 2)), (1, 16));
    assert_eq!((u64_at(&b, x + 4), u64_at(&b, x + 12)), (20, 20));
    // The second entry is small, but past the limit in offset: its local header says 45 too.
    let second = 30 + name + extra + 20;
    assert_eq!(u32_at(&b, second), 0x0403_4B50);
    assert_eq!(u16_at(&b, second + 4), 45);
    // The end record (no comment), the locator before it, the ZIP64 record it points to.
    let eocd = b.len() - 22;
    assert_eq!(u32_at(&b, eocd), 0x0605_4B50);
    let loc = eocd - 20;
    assert_eq!(u32_at(&b, loc), 0x0706_4B50);
    assert_eq!(u32_at(&b, loc + 16), 1, "one disk");
    let rec = u64_at(&b, loc + 8) as usize;
    assert_eq!(u32_at(&b, rec), 0x0606_4B50);
    assert_eq!(u64_at(&b, rec + 4), 44, "its size past the field");
    assert_eq!(u64_at(&b, rec + 24), 2);
    assert_eq!(u64_at(&b, rec + 32), 2);
    let dir = u64_at(&b, rec + 48) as usize;
    assert_eq!(dir + u64_at(&b, rec + 40) as usize, rec);
    // The central records: the first's full fields in order (size, compressed, its offset
    // under the limit kept 32-bit); the second's offset full.
    assert_eq!(u32_at(&b, dir), 0x0201_4B50);
    assert_eq!(u16_at(&b, dir + 6), 45);
    let (n1, e1, c1) = (
        u16_at(&b, dir + 28) as usize,
        u16_at(&b, dir + 30) as usize,
        u16_at(&b, dir + 32) as usize,
    );
    assert_eq!(u32_at(&b, dir + 42), 0, "offset 0 fits");
    let ex = dir + 46 + n1;
    assert_eq!((u16_at(&b, ex), u16_at(&b, ex + 2)), (1, 16));
    assert_eq!((u64_at(&b, ex + 4), u64_at(&b, ex + 12)), (20, 20));
    let d2 = dir + 46 + n1 + e1 + c1;
    assert_eq!(u32_at(&b, d2), 0x0201_4B50);
    assert_eq!(u16_at(&b, d2 + 6), 45);
    assert_eq!(u32_at(&b, d2 + 42), u32::MAX);
    let n2 = u16_at(&b, d2 + 28) as usize;
    let ex2 = d2 + 46 + n2;
    assert_eq!((u16_at(&b, ex2), u16_at(&b, ex2 + 2)), (1, 8));
    assert_eq!(u64_at(&b, ex2 + 4) as usize, second);
}
