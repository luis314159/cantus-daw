//! The piano build (round 40 phase 3, ADR 0125; `chorda-src`): the Salamander/bitKlavier
//! naming read from **published file names** — bitKlavier Grand's own README listing
//! (`fixtures/bitklavier-readme-listing.txt`) and the Salamander V3 pack's names — and the
//! SFZ written from them.

use cantus_content::manifest::{PianoBuild, PianoNaming, PianoPerspective, Text};
use cantus_content::pianobuild::{self, Kind, Perspective, Scan, NOISE_OUTPUT};

fn listing() -> Vec<String> {
    include_str!("fixtures/bitklavier-readme-listing.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .flat_map(|l| l.split_whitespace())
        // "All .wav audio files" in the heading is not a file.
        .filter(|w| w.ends_with(".wav") && w.len() > 4)
        .map(str::to_string)
        .collect()
}

#[test]
fn bitklaviers_published_names_read_as_its_library_says() {
    let names = listing();
    assert_eq!(
        names.len(),
        662,
        "the README lists every sample of one image"
    );
    let s = Scan::of_names(names);
    // 16 velocity layers at every minor third from A0 (30 roots), 3 layers of release
    // resonance, a hammer per key and two takes of the pedal each way — the library's specs.
    assert_eq!(s.notes.len(), 480);
    assert_eq!(s.layers(), 16);
    let roots: std::collections::BTreeSet<u8> = s.notes.iter().map(|n| n.0).collect();
    assert_eq!(roots.len(), 30);
    assert_eq!(roots.first(), Some(&21), "A0");
    assert_eq!(roots.last(), Some(&108), "C8");
    assert!(roots
        .iter()
        .zip(roots.iter().skip(1))
        .all(|(a, b)| b - a == 3));
    assert_eq!(s.harm.len(), 90);
    assert_eq!(s.hammers.len(), 88);
    assert_eq!(s.hammers.first().map(|h| h.0), Some(21), "rel1 is A0");
    assert_eq!(s.hammers.last().map(|h| h.0), Some(108), "rel88 is C8");
    assert_eq!(s.pedals.len(), 4);
    assert!(s.unknown.is_empty(), "{:?}", s.unknown);
    // The README's own examples (formats.md §6).
    assert_eq!(
        pianobuild::parse("D#5v13"),
        Some(Kind::Note {
            root: 75,
            layer: 13
        })
    );
    assert_eq!(
        pianobuild::parse("harmC2v2"),
        Some(Kind::Harm { root: 36, layer: 2 })
    );
    assert_eq!(pianobuild::parse("rel23"), Some(Kind::Hammer { key: 43 }));
    assert_eq!(
        pianobuild::parse("pedalU2"),
        Some(Kind::Pedal { down: false, n: 2 })
    );
    assert_eq!(pianobuild::note_key("C4"), Some(60));
    assert_eq!(pianobuild::note_key("Bb2"), Some(46));
}

#[test]
fn the_salamanders_names_read_the_same_way() {
    // From the Salamander Grand V3 pack (FLAC edition): notes as bitKlavier's; its own
    // resonance names (`harmL…`, `harmS…`, `harmV3…`) are not the shared naming and are
    // set aside, never guessed.
    let s = Scan::of_names(
        [
            "A0v1.flac",
            "C1v16.flac",
            "F#6v2.flac",
            "harmLA0.flac",
            "harmSD#5.flac",
            "harmV3C4.flac",
            "rel1.flac",
            "rel88.flac",
            "pedalD1.flac",
            "pedalU2.flac",
            "readme.txt",
            "._A0v1.flac",
        ]
        .map(String::from),
    );
    assert_eq!(s.notes.len(), 3);
    assert_eq!(s.notes[2].0, 90, "F#6");
    assert_eq!(s.hammers.len(), 2);
    assert_eq!(s.pedals.len(), 2);
    assert_eq!(s.unknown.len(), 3, "{:?}", s.unknown);
}

#[test]
fn keys_and_velocities_follow_bitklaviers_rules() {
    let roots: Vec<u8> = (21..=108).step_by(3).collect();
    let r = pianobuild::key_ranges(&roots);
    assert_eq!(r[0], (21, 20, 22));
    assert_eq!(r[1], (24, 23, 25));
    assert_eq!(r[13], (60, 59, 61));
    assert_eq!(*r.last().unwrap(), (108, 107, 109));
    // Every key from A0 to C8 is played by exactly one root.
    for k in 21u8..=108 {
        assert_eq!(r.iter().filter(|x| x.1 <= k && k <= x.2).count(), 1, "{k}");
    }
    assert_eq!(pianobuild::velocities(1, 16), (1, 7));
    assert_eq!(pianobuild::velocities(2, 16), (8, 15));
    assert_eq!(pianobuild::velocities(16, 16), (120, 127));
    for v in 1u8..=127 {
        let n = (1..=16)
            .filter(|k| {
                let (lo, hi) = pianobuild::velocities(*k, 16);
                lo <= v && v <= hi
            })
            .count();
        assert_eq!(n, 1, "velocity {v}");
    }
    assert_eq!(pianobuild::harm_velocities(1, 3), (1, 39));
    assert_eq!(pianobuild::harm_velocities(3, 3), (85, 127));
}

#[test]
fn the_sfz_sends_each_image_to_its_output_and_the_noises_to_releases() {
    let s = Scan::of_names(listing());
    let p = [Perspective {
        folder: "samples",
        name: "Stage Lid ORTF",
        output: 6,
        scan: &s,
    }];
    let text = pianobuild::write_sfz("bitKlavier Grand", &p);
    assert_eq!(text.matches("<region>").count(), 662);
    assert!(text.contains("<group> group_label=Stage Lid ORTF output=6 amp_veltrack=0"));
    assert!(text.contains(&format!(
        "group_label=Releases output={NOISE_OUTPUT} trigger=release"
    )));
    assert!(text.contains("sample=samples/D#5v13.wav lokey=74 hikey=76 pitch_keycenter=75"));
    assert!(text.contains("sample=samples/rel23.wav lokey=43 hikey=43"));
    assert!(text.contains("on_locc64=64 on_hicc64=127"));
    // The undamped top: F#6 and up ring on after the key.
    assert!(text.contains("notes without dampers"));
}

#[test]
fn a_build_writes_the_sfz_into_the_pack() {
    let dir = std::env::temp_dir().join(format!("cantus-piano-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let img = dir.join("img");
    std::fs::create_dir_all(&img).unwrap();
    for n in listing().iter().take(40) {
        std::fs::write(img.join(n), b"RIFF").unwrap();
    }
    let piano = PianoBuild {
        naming: PianoNaming::BitKlavier,
        perspectives: vec![PianoPerspective {
            folder: "img".into(),
            name: Text {
                en: "Piano Bar".into(),
                es: "Barra del piano".into(),
            },
            output: Some(5),
        }],
        sfz: "grand.sfz".into(),
        set: Some("bitklavier-grand".into()),
        unpacked_size: None,
    };
    let built = pianobuild::build(
        &piano,
        &dir,
        "bitKlavier Grand",
        &Default::default(),
        &mut |_, _| {},
    )
    .expect("built");
    assert_eq!(built.regions, 40);
    let text = std::fs::read_to_string(dir.join("grand.sfz")).unwrap();
    assert!(text.contains("output=5"));
    // A folder with no note in it is refused, with what it is.
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    let mut bad = piano.clone();
    bad.perspectives[0].folder = "empty".into();
    assert!(
        pianobuild::build(&bad, &dir, "x", &Default::default(), &mut |_, _| {})
            .unwrap_err()
            .contains("empty")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the FLAC step (the coordinator's decision: transcode at install, checked) ---------

fn wav(path: &std::path::Path, bits: u16, channels: u16, n: usize) -> Vec<i32> {
    let spec = hound::WavSpec {
        channels,
        sample_rate: 48_000,
        bits_per_sample: bits,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let top = (1i64 << (bits - 1)) - 1;
    let mut out = Vec::new();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in 0..n * channels as usize {
        // A tone and noise, the extremes of the range among them.
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let v = match i % 997 {
            0 => top as i32,
            1 => (-top - 1) as i32,
            _ => ((x % (2 * top as u64)) as i64 - top) as i32 / 3,
        };
        w.write_sample(v).unwrap();
        out.push(v);
    }
    w.finalize().unwrap();
    out
}

#[test]
fn a_wav_is_kept_as_flac_that_decodes_to_its_samples_exactly() {
    use cantus_content::pianobuild_flac::{self as pf, Done};
    let dir = tempfile::tempdir().unwrap();
    for (name, bits, ch) in [("C4v1.wav", 24u16, 2u16), ("rel40.wav", 16, 1)] {
        let samples = wav(&dir.path().join(name), bits, ch, 20_000);
        let pcm = pf::read_pcm(&std::fs::read(dir.path().join(name)).unwrap()).unwrap();
        assert_eq!(pcm.samples, samples, "{name} read as written");
        let (out, done) = pf::transcode(dir.path(), name);
        assert_eq!(done, Done::Flac, "{name}");
        assert!(out.ends_with(".flac") && !dir.path().join(name).exists());
        let flac = std::fs::read(dir.path().join(&out)).unwrap();
        assert!(pf::same(&flac, &pcm));
        // A sample changed: not the same.
        let mut other = pcm.clone();
        other.samples[1234] ^= 1;
        assert!(!pf::same(&flac, &other));
    }
    // Not integer PCM: kept as it came.
    let f = dir.path().join("float.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&f, spec).unwrap();
    w.write_sample(0.5f32).unwrap();
    w.finalize().unwrap();
    assert!(matches!(
        pf::transcode(dir.path(), "float.wav").1,
        Done::Kept(_)
    ));
    assert!(f.exists());
}

/// A real bitKlavier file, when one is on this machine (`CANTUS_BITKLAVIER_WAV`): the
/// published bytes through the same step.
#[test]
fn a_published_bitklavier_wav_survives_the_flac_step() {
    use cantus_content::pianobuild_flac as pf;
    let Ok(path) = std::env::var("CANTUS_BITKLAVIER_WAV") else {
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let pcm = pf::read_pcm(&bytes).expect("24-bit PCM");
    assert_eq!((pcm.bits, pcm.channels, pcm.rate), (24, 2, 48_000));
    let flac = pf::encode(&pcm).unwrap();
    assert!(pf::same(&flac, &pcm));
    assert!(flac.len() < bytes.len());
}
