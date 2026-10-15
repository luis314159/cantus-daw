//! A pack built at install (round 28): a multi-microphone DrumGizmo kit mixed down to the
//! stereo instrument Exemplar plays — the WAV reader and writer, the kit reader, the mix,
//! the velocity layers and round robins, the instrument written, the install end to end
//! (only the instrument, its files and the licence stay), the manifest's rules.

mod support;

use cantus_content::fetch::{file_url, Net};
use cantus_content::install::Patience;
use cantus_content::kitbuild::{self, Hit};
use cantus_content::manifest::{
    Build, Choke, KeyMap, Mic, PackInstrument, Shelf, Source, Text, FORMAT_BUILD,
};
use cantus_content::{drumgizmo, wav, Manager, Pack};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use support::*;

/// A float WAV of `channels`, `frames` long, sample `f(frame, channel)`.
fn float_wav(channels: u16, rate: u32, frames: usize, f: impl Fn(usize, usize) -> f32) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut c = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
    for i in 0..frames {
        for ch in 0..channels as usize {
            w.write_sample(f(i, ch)).unwrap();
        }
    }
    w.finalize().unwrap();
    c.into_inner()
}

fn int_wav(bits: u16, channels: u16, values: &[i32]) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels,
        sample_rate: 44_100,
        bits_per_sample: bits,
        sample_format: hound::SampleFormat::Int,
    };
    let mut c = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
    for v in values {
        w.write_sample(*v).unwrap();
    }
    w.finalize().unwrap();
    c.into_inner()
}

#[test]
fn the_wav_reader_reads_float_and_pcm_of_any_width_and_refuses_cut_files() {
    let f = float_wav(13, 44_100, 100, |i, c| (i * 13 + c) as f32 / 4_096.0);
    let a = wav::parse(&f).unwrap();
    assert_eq!((a.channels, a.rate, a.frames()), (13, 44_100, 100));
    assert_eq!(a.samples[13 * 7 + 9], (7 * 13 + 9) as f32 / 4_096.0);
    let p16 = wav::parse(&int_wav(16, 1, &[0, 16_384, -32_768])).unwrap();
    assert_eq!(p16.samples, [0.0, 0.5, -1.0]);
    let p24 = wav::parse(&int_wav(24, 2, &[4_194_304, -8_388_608])).unwrap();
    assert_eq!(p24.samples, [0.5, -1.0]);
    // Cut inside the audio, a header that is not WAVE, no audio at all.
    assert!(wav::parse(&f[..f.len() - 7]).is_err());
    assert!(wav::parse(b"RIFF\0\0\0\0AVI LIST").is_err());
    assert!(wav::parse(&f[..36]).is_err());
}

#[test]
fn the_24_bit_writer_rounds_to_the_step_and_counts_what_it_clamps() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.wav");
    let step = 1.0 / 8_388_608.0;
    let samples = [0.0, 0.5, -0.5, 3.4 * step, -1.0, 1.5, -2.0, f32::NAN];
    let clamped = wav::write_stereo_24(&path, 44_100, &samples).unwrap();
    assert_eq!(clamped, 3, "1.5, −2 and NaN");
    let back = wav::read(&path).unwrap();
    assert_eq!((back.channels, back.rate), (2, 44_100));
    assert_eq!(&back.samples[..5], &[0.0, 0.5, -0.5, 3.0 * step, -1.0]);
    assert_eq!(back.samples[5], 8_388_607.0 * step);
    assert_eq!(&back.samples[6..], &[-1.0, 0.0]);
    // Another reader agrees.
    let h: Vec<i32> = hound::WavReader::open(&path)
        .unwrap()
        .samples::<i32>()
        .map(Result::unwrap)
        .collect();
    assert_eq!(h[1], 4_194_304);
}

#[test]
fn equal_power_pan_is_unity_at_the_sides_and_three_db_down_in_the_middle() {
    assert_eq!(kitbuild::pan_gains(-100.0), (1.0, 0.0));
    let (l, r) = kitbuild::pan_gains(0.0);
    assert!((l - 0.707_106_77).abs() < 1e-6 && (r - 0.707_106_77).abs() < 1e-6);
    let (l, r) = kitbuild::pan_gains(100.0);
    assert!(l.abs() < 1e-7 && (r - 1.0).abs() < 1e-7);
    assert!((kitbuild::db(-6.0) - 0.501_187_2).abs() < 1e-6);
}

/// The level of hit `i` of an instrument: each a little harder than the last.
fn amp(base: f32, i: usize) -> f32 {
    base * 1.6f32.powi(i as i32)
}

/// A three-microphone kit in DrumGizmo's layout: a snare of six hits, a closed and an open
/// hi-hat of three each in the group "hihat", and one instrument the pack does not build.
/// Each hit is one three-channel file whose channels are stored B, A, C (the instrument
/// file says which is which): A at the hit's level, B at half of it, C a constant 0.3.
fn kit_entries() -> Vec<(String, Vec<u8>)> {
    let mut e: Vec<(String, Vec<u8>)> = vec![
        ("DRS/README.md".into(), b"DRS test kit. CC BY 4.0.".to_vec()),
        (
            "DRS/Kit_full.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<drumkit name="Test" description="three microphones">
  <channels><channel name="A"/><channel name="B"/><channel name="C"/></channels>
  <instruments>
    <instrument name="Snare" file="Snare/Snare.xml"/>
    <instrument name="HatClosed" group="hihat" file="HatClosed/HatClosed.xml"/>
    <instrument name="HatOpen" group="hihat" file="HatOpen/HatOpen.xml"/>
    <instrument name="Extra" file="Extra/Extra.xml"/>
  </instruments>
</drumkit>"#
                .to_vec(),
        ),
    ];
    for (name, hits, base, frames) in [
        ("Snare", 6usize, 0.02f32, 8_820usize),
        ("HatClosed", 3, 0.05, 4_410),
        ("HatOpen", 3, 0.04, 22_050),
        ("Extra", 1, 0.1, 441),
    ] {
        let mut xml = format!("<?xml version='1.0' encoding='UTF-8'?>\n<instrument version=\"2.0\" name=\"{name}\">\n <samples>\n");
        for i in 0..hits {
            let a = amp(base, i);
            let file = format!("samples/{}-{name}.wav", i + 1);
            xml += &format!("  <sample name=\"{name}-{}\" power=\"{}\">\n", i + 1, a * a);
            for (ch, fc) in [("B", 1), ("A", 2), ("C", 3)] {
                xml += &format!(
                    "   <audiofile channel=\"{ch}\" file=\"{file}\" filechannel=\"{fc}\"/>\n"
                );
            }
            xml += "  </sample>\n";
            let wav = float_wav(3, 44_100, frames, |_, c| match c {
                0 => 0.5 * a,
                1 => a,
                _ => 0.3,
            });
            e.push((format!("DRS/{name}/{file}"), wav));
        }
        xml += " </samples>\n</instrument>\n";
        e.push((format!("DRS/{name}/{name}.xml"), xml.into_bytes()));
    }
    e
}

fn kit_zip() -> Vec<u8> {
    let entries = kit_entries();
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    zip_bytes(&refs)
}

fn build_spec(unpacked: u64) -> Build {
    Build {
        kit: "Kit_full.xml".into(),
        unpacked_size: unpacked,
        sfz: "Kit.sfz".into(),
        folder: "Samples".into(),
        gain_db: 0.0,
        mics: vec![
            Mic {
                channel: "A".into(),
                gain_db: 0.0,
                pan: -100.0,
                invert: false,
                own: vec![],
                bleed_db: 0.0,
            },
            Mic {
                channel: "B".into(),
                gain_db: -6.0,
                pan: 100.0,
                invert: true,
                own: vec![],
                bleed_db: 0.0,
            },
        ],
        keys: vec![
            KeyMap {
                instrument: "Snare".into(),
                keys: vec![38, 40],
                gain_db: 0.0,
                label: String::new(),
            },
            KeyMap {
                instrument: "HatClosed".into(),
                keys: vec![42],
                gain_db: 0.0,
                label: String::new(),
            },
            KeyMap {
                instrument: "HatOpen".into(),
                keys: vec![46],
                gain_db: 0.0,
                label: String::new(),
            },
        ],
        chokes: vec![Choke {
            instruments: vec!["HatClosed".into(), "HatOpen".into()],
            release_ms: 60.0,
        }],
        round_robin: 2,
        range_db: 40.0,
        keep: vec!["README.md".into()],
    }
}

fn kit_pack(dir: &Path) -> Pack {
    let bytes = kit_zip();
    let path = dir.join("drs.zip");
    std::fs::write(&path, &bytes).unwrap();
    let unpacked: u64 = kit_entries().iter().map(|e| e.1.len() as u64).sum();
    let mut p = cc_by(
        zip_pack("drs", file_url(&path), &bytes, "DRS/", &[], "README.md"),
        "4.0",
    );
    p.installed_size = 200_000;
    p.instruments = vec![PackInstrument {
        id: "kit".into(),
        name: Text {
            en: "Test kit".into(),
            es: "Batería de prueba".into(),
        },
        file: "Kit.sfz".into(),
        shelf: Shelf::Drums,
        layers: None,
        gain_db: 0.0,
        voices: Some(64),
    }];
    p.build = Some(build_spec(unpacked));
    p
}

fn installed(p: &Pack, root: &Path) -> Result<cantus_content::Receipt, String> {
    let mut m = Manager::with_transport(
        root,
        manifest(vec![p.clone()]),
        Arc::new(Net::new()),
        Patience::none(),
    );
    m.install(&p.id);
    m.wait_idle(Duration::from_secs(60))
        .remove(0)
        .result
        .map_err(|e| e.to_string())
}

/// Every file under `dir`, relative, sorted.
fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path());
            } else {
                let rel = e
                    .path()
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_drumgizmo_kit_is_read_with_its_channels_groups_and_files() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in kit_entries() {
        let rel = name.strip_prefix("DRS/").unwrap();
        let p = drumgizmo::path_of(dir.path(), rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    let kit = drumgizmo::read_kit(dir.path(), "Kit_full.xml").unwrap();
    assert_eq!(kit.channels, ["A", "B", "C"]);
    let names: Vec<&str> = kit.instruments.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["Snare", "HatClosed", "HatOpen", "Extra"]);
    assert_eq!(kit.instruments[1].group.as_deref(), Some("hihat"));
    assert_eq!(kit.instruments[0].group, None);
    let s = &kit.instruments[0].samples[2];
    assert_eq!(s.audio.len(), 3);
    let a = s.audio.iter().find(|a| a.channel == "A").unwrap();
    assert_eq!(a.file, "Snare/samples/3-Snare.wav");
    assert_eq!(
        a.file_channel, 1,
        "the second channel of the file, counted from 0"
    );
    assert_eq!(drumgizmo::files(&kit).len(), 13);
    // A file outside the kit's folder, or a channel the kit does not declare, is refused.
    let snare = dir.path().join("Snare").join("Snare.xml");
    let text = std::fs::read_to_string(&snare).unwrap();
    std::fs::write(&snare, text.replace("samples/1-Snare.wav", "../../x.wav")).unwrap();
    assert!(drumgizmo::read_kit(dir.path(), "Kit_full.xml").is_err());
    std::fs::write(&snare, text.replace("channel=\"C\"", "channel=\"D\"")).unwrap();
    let e = drumgizmo::read_kit(dir.path(), "Kit_full.xml").unwrap_err();
    assert!(e.contains("channel D"), "{e}");
}

fn hit(file: &str, level: f32) -> Hit {
    Hit {
        file: file.into(),
        level,
    }
}

#[test]
fn hits_become_velocity_layers_by_level_whose_turns_come_in_order() {
    // Seven hits from −12 to 0 dB, given out of order.
    let levels = [-8.0, 0.0, -12.0, -2.0, -4.0, -10.0, -6.0];
    let hits: Vec<Hit> = levels
        .iter()
        .enumerate()
        .map(|(i, l)| hit(&format!("h{i}"), *l))
        .collect();
    let (regions, dropped) = kitbuild::layers(&hits, 2, 48.0);
    assert_eq!(dropped, 0);
    assert_eq!(regions.len(), 7);
    // Three layers (2, 2, 3 hits), softest first, covering 1…127 without a gap.
    let ranges: Vec<(u8, u8)> = regions.iter().map(|r| (r.lovel, r.hivel)).collect();
    let mut layers = ranges.clone();
    layers.dedup();
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].0, 1);
    assert_eq!(layers[2].1, 127);
    for w in layers.windows(2) {
        assert_eq!(w[0].1 + 1, w[1].0, "{layers:?}");
    }
    // Each layer's hits take turns 1…n of n, softest first.
    let turns: Vec<(u8, u8)> = regions.iter().map(|r| (r.seq, r.seqlen)).collect();
    assert_eq!(
        turns,
        [(1, 2), (2, 2), (1, 2), (2, 2), (1, 3), (2, 3), (3, 3)]
    );
    let files: Vec<&str> = regions.iter().map(|r| r.file.as_str()).collect();
    assert_eq!(files, ["h2", "h5", "h0", "h6", "h4", "h3", "h1"]);
    // Every hit plays at the velocity curve: its level plus its gain is the loudest hit's.
    for (r, l) in regions
        .iter()
        .zip([-12.0, -10.0, -8.0, -6.0, -4.0, -2.0, 0.0])
    {
        assert_eq!(r.volume, -l, "{}", r.file);
    }
    // A layer sits on the velocities whose curve level is near its own: velocity
    // 127·10^(L/40) plays a hit L dB under the loudest. The softest layer's centre is
    // −11 dB, the next one's −7 dB: the first ends half-way, at −9 dB, velocity 76.
    assert_eq!(layers[0].1, 76);
}

#[test]
fn hits_out_of_range_are_left_out_and_a_layer_without_a_velocity_joins_its_neighbour() {
    let hits = [
        hit("a", 0.0),
        hit("b", -0.1),
        hit("c", -0.2),
        hit("d", -60.0),
    ];
    let (regions, dropped) = kitbuild::layers(&hits, 1, 24.0);
    assert_eq!(dropped, 1, "−60 dB is outside 24 dB");
    // Three single-hit layers within 0.2 dB of each other: −0.2 dB plays up to 126, and
    // the two loudest cannot each have a velocity of their own at 127 — they share it,
    // taking turns.
    assert_eq!(regions.len(), 3);
    assert_eq!(
        (regions[0].file.as_str(), regions[0].lovel, regions[0].hivel),
        ("c", 1, 126)
    );
    for r in &regions[1..] {
        assert_eq!((r.lovel, r.hivel, r.seqlen), (127, 127, 2), "{}", r.file);
    }
    // The gain stops at +24 dB.
    let (r, _) = kitbuild::layers(&[hit("loud", 0.0), hit("soft", -40.0)], 1, 48.0);
    assert_eq!(r[0].volume, kitbuild::MAX_VOLUME);
}

#[test]
fn keys_in_a_row_share_a_region() {
    assert_eq!(kitbuild::runs(&[50, 47, 48]), [(47, 48), (50, 50)]);
    assert_eq!(kitbuild::runs(&[36]), [(36, 36)]);
}

#[test]
fn the_kit_installs_as_its_instrument_its_stereo_files_and_its_licence_only() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let p = kit_pack(src.path());
    let r = installed(&p, root.path()).unwrap();
    let dir = root.path().join("drs");
    let files = tree(&dir);
    let mut expect = vec![
        "Kit.sfz".to_string(),
        "README.md".into(),
        "cantus-pack.json".into(),
    ];
    for (name, n) in [("HatClosed", 3), ("HatOpen", 3), ("Snare", 6)] {
        for i in 1..=n {
            expect.push(format!("Samples/{name}/{i:02}.wav"));
        }
    }
    expect.sort();
    assert_eq!(
        files, expect,
        "the multi-channel files, the kit's XML and Extra are gone"
    );
    assert_eq!(
        r.files,
        files.len() as u64 - 1,
        "the receipt counts what is there"
    );
    let bytes: u64 = files
        .iter()
        .filter(|f| *f != "cantus-pack.json")
        .map(|f| std::fs::metadata(dir.join(f)).unwrap().len())
        .sum();
    assert_eq!(r.bytes, bytes);
    assert!(!root.path().join(".staging").read_dir().unwrap().any(|e| e
        .unwrap()
        .file_type()
        .unwrap()
        .is_dir()));
}

#[test]
fn each_microphone_is_heard_at_its_gain_pan_and_polarity() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    installed(&kit_pack(src.path()), root.path()).unwrap();
    // Snare hit 4: A = a hard left at 0 dB; B = a/2, −6 dB, hard right, polarity reversed;
    // C is not in the mix.
    let a = amp(0.02, 3);
    let w = wav::read(&root.path().join("drs/Samples/Snare/04.wav")).unwrap();
    assert_eq!((w.channels, w.rate, w.frames()), (2, 44_100, 8_820));
    let step = 1.0 / 8_388_608.0;
    let (l, r) = (w.samples[200], w.samples[201]);
    assert!((l - a).abs() <= step, "left {l} against {a}");
    let right = -0.5 * a * 0.501_187_2;
    assert!((r - right).abs() <= 2.0 * step, "right {r} against {right}");
}

#[test]
fn the_instrument_has_the_keys_layers_turns_and_the_hi_hat_choke() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    installed(&kit_pack(src.path()), root.path()).unwrap();
    let text = std::fs::read_to_string(root.path().join("drs/Kit.sfz")).unwrap();
    assert!(text.starts_with("// "), "credited at the top");
    assert!(text.contains("default_path=Samples/"));
    assert!(text.contains("loop_mode=one_shot amp_veltrack=100 pitch_keytrack=0"));
    let regions: Vec<&str> = text.lines().filter(|l| l.starts_with("<region>")).collect();
    // Snare on 38 and 40 (one run: two keys apart, so two runs) × 6 hits, hats 3 + 3.
    assert_eq!(regions.len(), 6 * 2 + 3 + 3);
    let snare: Vec<&&str> = regions.iter().filter(|l| l.contains("Snare/")).collect();
    assert!(snare.iter().all(|l| l.contains("seq_length=2")));
    assert_eq!(
        snare
            .iter()
            .filter(|l| l.contains("lokey=38 hikey=38"))
            .count(),
        6
    );
    assert_eq!(
        snare
            .iter()
            .filter(|l| l.contains("lokey=40 hikey=40"))
            .count(),
        6
    );
    // The loudest snare hit (06) has no gain; the softest the most.
    let r06 = snare.iter().find(|l| l.ends_with("Snare/06.wav")).unwrap();
    assert!(!r06.contains("volume="), "{r06}");
    assert!(r06.contains("hivel=127"));
    // The hats silence each other with a 60 ms fade; the snare does not.
    let groups: Vec<&str> = text.lines().filter(|l| l.starts_with("<group>")).collect();
    assert_eq!(groups.len(), 3);
    assert!(groups[0].contains("group_label=Snare") && !groups[0].contains("off_by"));
    for g in &groups[1..] {
        assert!(
            g.contains("group=1 off_by=1 off_mode=normal ampeg_release=0.06"),
            "{g}"
        );
    }
}

#[test]
fn the_unpack_is_held_to_the_unpacked_size_the_build_names() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut p = kit_pack(src.path());
    // The pack weighs 200 kB built, but its archive unpacks to far more: the build's
    // `unpacked_size` is the budget (with its quarter and megabyte of slack). Name it
    // 1 byte — a megabyte in all, under the kit's 1.6 MB — and the archive is refused.
    if let Some(b) = p.build.as_mut() {
        b.unpacked_size = 1;
    }
    let e = installed(&p, root.path()).unwrap_err();
    assert!(e.contains("unpacks to more"), "{e}");
    assert!(!root.path().join("drs").exists());
}

#[test]
fn a_build_that_fails_installs_nothing_and_leaves_nothing() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut p = kit_pack(src.path());
    if let Some(b) = p.build.as_mut() {
        b.keys[0].instrument = "Cowbell".into();
    }
    let e = installed(&p, root.path()).unwrap_err();
    assert!(e.contains("Cowbell"), "{e}");
    assert!(!root.path().join("drs").exists());
    let left: Vec<_> = std::fs::read_dir(root.path().join(".staging"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_type().unwrap().is_dir())
        .collect();
    assert!(left.is_empty(), "no staging folder is left");
}

#[test]
fn a_build_step_is_checked_by_the_manifest() {
    let src = tempfile::tempdir().unwrap();
    let good = kit_pack(src.path());
    let mut m = manifest(vec![good.clone()]);
    assert_eq!(m.format, FORMAT_BUILD);
    assert_eq!(m.validate(), Ok(()));
    m.format = FORMAT_BUILD - 1;
    assert!(m.validate().unwrap_err().to_string().contains("format 3"));
    let reject = |edit: &dyn Fn(&mut Pack), why: &str| {
        let mut p = good.clone();
        edit(&mut p);
        assert!(manifest(vec![p]).validate().is_err(), "{why}");
    };
    fn b(p: &mut Pack) -> &mut Build {
        p.build.as_mut().unwrap()
    }
    reject(&|p| b(p).keep.clear(), "the licence must be kept");
    reject(
        &|p| b(p).sfz = "Other.sfz".into(),
        "no instrument plays what it writes",
    );
    reject(&|p| b(p).keys[1].keys = vec![38], "a key used twice");
    reject(&|p| b(p).keys[0].keys.clear(), "an instrument with no key");
    reject(
        &|p| b(p).keys[0].label = "Snare=1".into(),
        "a label that breaks the SFZ",
    );
    reject(&|p| b(p).mics.clear(), "no microphones");
    reject(&|p| b(p).mics[1].channel = "A".into(), "a microphone twice");
    reject(&|p| b(p).mics[0].pan = 120.0, "a pan past the side");
    reject(&|p| b(p).round_robin = 0, "no turns");
    reject(&|p| b(p).range_db = 2.0, "a range too small");
    reject(
        &|p| b(p).chokes[0].instruments.push("Extra".into()),
        "a choke of an instrument not built",
    );
    reject(
        &|p| b(p).folder = "../Samples".into(),
        "a folder outside the pack",
    );
    reject(&|p| b(p).unpacked_size = 0, "no unpacked size");
    reject(
        &|p| {
            let z = match &p.source {
                Source::Zip(z) => z.clone(),
                _ => unreachable!(),
            };
            p.source = Source::Files(vec![cantus_content::manifest::FileEntry {
                url: z.url,
                to: "README.md".into(),
                size: z.size,
                sha256: z.sha256,
            }]);
        },
        "a build needs an archive",
    );
    reject(
        &|p| p.instruments[0].voices = Some(65),
        "more voices than Exemplar has",
    );
}

#[test]
fn a_cancelled_build_stops() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in kit_entries() {
        let rel = name.strip_prefix("DRS/").unwrap();
        let p = drumgizmo::path_of(dir.path(), rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let e = kitbuild::build(&build_spec(1), dir.path(), "x", &cancel, &mut |_, _| {}).unwrap_err();
    assert_eq!(e, "cancelled");
}

#[test]
fn a_microphone_is_heard_at_its_bleed_level_on_the_drums_it_does_not_face() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut p = kit_pack(src.path());
    if let Some(b) = p.build.as_mut() {
        // A is the snare's microphone: on the hats it is 20 dB down (a gate, as a mixing
        // engineer would set one).
        b.mics[0].own = vec!["Snare".into()];
        b.mics[0].bleed_db = -20.0;
    }
    installed(&p, root.path()).unwrap();
    let step = 1.0 / 8_388_608.0;
    let left = |f: &str| wav::read(&root.path().join(f)).unwrap().samples[200];
    let snare = left("drs/Samples/Snare/02.wav");
    assert!(
        (snare - amp(0.02, 1)).abs() <= step,
        "the snare's own microphone at 0 dB"
    );
    let hat = left("drs/Samples/HatClosed/02.wav");
    assert!(
        (hat - amp(0.05, 1) * 0.1).abs() <= step,
        "20 dB down on a hat: {hat}"
    );
}

#[test]
fn an_instrument_may_be_raised_or_lowered_in_the_mix() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut p = kit_pack(src.path());
    if let Some(b) = p.build.as_mut() {
        b.keys[1].gain_db = 6.0; // the closed hat
    }
    installed(&p, root.path()).unwrap();
    let step = 1.0 / 8_388_608.0;
    let left = |f: &str| wav::read(&root.path().join(f)).unwrap().samples[200];
    let hat = left("drs/Samples/HatClosed/01.wav");
    assert!(
        (hat - amp(0.05, 0) * 1.995_262_3).abs() <= 2.0 * step,
        "+6 dB: {hat}"
    );
    let open = left("drs/Samples/HatOpen/01.wav");
    assert!(
        (open - amp(0.04, 0)).abs() <= step,
        "the open hat as it was: {open}"
    );
}

/// The real DRSKit archive, when it is on this machine (`CANTUS_DRSKIT_ARCHIVE`): installed
/// from the file through the bundled manifest's entry — its hash, its build — into the
/// stereo instrument: the files and bytes the manifest says, nothing clamped, every
/// General MIDI drum key the kit has, the credit in the receipt.
#[test]
#[ignore]
fn the_real_drskit_installs_mixed_from_its_archive() {
    let Ok(archive) = std::env::var("CANTUS_DRSKIT_ARCHIVE") else {
        eprintln!("CANTUS_DRSKIT_ARCHIVE is not set");
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let mut m = Manager::with_transport(
        root.path(),
        cantus_content::Manifest::bundled(),
        Arc::new(Net::new()),
        Patience::none(),
    );
    let t = std::time::Instant::now();
    assert!(m.install_from_file("drskit", Path::new(&archive)));
    let r = m
        .wait_idle(Duration::from_secs(3600))
        .remove(0)
        .result
        .expect("installed");
    eprintln!(
        "installed in {:?}: {} files, {} bytes",
        t.elapsed(),
        r.files,
        r.bytes
    );
    let pack = m.manifest().pack("drskit").unwrap().clone();
    assert_eq!(
        r.bytes, pack.installed_size,
        "the manifest's size is the build's"
    );
    let dir = m.pack_dir("drskit");
    let files = tree(&dir);
    assert_eq!(
        r.files as usize + 1,
        files.len(),
        "the receipt counts all but itself"
    );
    assert!(files.iter().all(|f| f == "README.md"
        || f == "DRSKit.sfz"
        || f == "cantus-pack.json"
        || (f.starts_with("Samples/") && f.ends_with(".wav"))));
    let mut peak = 0f32;
    for f in files.iter().filter(|f| f.ends_with(".wav")) {
        let a = wav::read(&dir.join(f)).unwrap();
        assert_eq!((a.channels, a.rate), (2, 44_100), "{f}");
        peak = a.samples.iter().fold(peak, |p, x| p.max(x.abs()));
    }
    eprintln!("peak {:.2} dBFS", 20.0 * peak.log10());
    assert!(peak < 0.89, "1 dB of headroom at least: {peak}");
    let text = std::fs::read_to_string(dir.join("DRSKit.sfz")).unwrap();
    assert!(text.contains("DrumGizmo"), "credited at the top");
    for key in [
        35, 36, 37, 38, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 53, 55, 57, 59,
    ] {
        assert!(
            text.contains(&format!("lokey={key} ")) || text.contains(&format!("hikey={key} ")),
            "General MIDI key {key}"
        );
    }
    assert!(r.credit_line().unwrap().contains("DrumGizmo"));
    assert_eq!(m.instruments().len(), 1);
    assert_eq!(m.instruments()[0].key(), "pack:drskit/kit");
}

#[test]
fn a_built_pack_reports_its_unpacking_then_its_mixing_hit_by_hit() {
    use cantus_content::install::{self, Job, Layout, Phase};
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let p = kit_pack(src.path());
    let layout = Layout::new(root.path());
    let net = Net::new();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let job = Job {
        layout: &layout,
        transport: &net,
        cancel: &cancel,
        patience: Patience::none(),
        archive: None,
    };
    let mut phases = Vec::new();
    install::install(&job, &p, &mut |ph| phases.push(ph)).unwrap();
    // The unpacking fills the first half of its share (its total doubled)…
    let last_unpack = phases
        .iter()
        .rev()
        .find_map(|ph| match ph {
            Phase::Unpacking { done, total } => Some((*done, *total)),
            _ => None,
        })
        .unwrap();
    assert_eq!(last_unpack.0 * 2, last_unpack.1);
    // …then the mixing counts every hit the pack builds, 6 + 3 + 3, from 0, after it.
    let building: Vec<(u64, u64)> = phases
        .iter()
        .filter_map(|ph| match ph {
            Phase::Building { done, total } => Some((*done, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(building.first(), Some(&(0, 12)));
    assert_eq!(building.last(), Some(&(12, 12)));
    let first_build = phases
        .iter()
        .position(|ph| matches!(ph, Phase::Building { .. }))
        .unwrap();
    assert!(phases[first_build..]
        .iter()
        .all(|ph| matches!(ph, Phase::Building { .. })));
}

#[test]
fn a_drum_is_labelled_as_the_manifest_names_it() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut p = kit_pack(src.path());
    if let Some(b) = p.build.as_mut() {
        b.keys[1].label = "Hi-hat closed".into();
    }
    installed(&p, root.path()).unwrap();
    let text = std::fs::read_to_string(root.path().join("drs/Kit.sfz")).unwrap();
    assert!(
        text.contains("<group> group_label=Hi-hat closed group=1"),
        "{text}"
    );
    assert!(
        text.lines().any(|l| l == "<group> group_label=Snare"),
        "unlabelled: the kit's name"
    );
}
