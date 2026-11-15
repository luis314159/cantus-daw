//! DrumGizmo kits for Tympanum (round 40 phase 3, ADR 0124 §3): the kit reader on the
//! published DRSKit files (`tests/fixtures/drskit_minimal`, CC BY 4.0, the DrumGizmo team
//! and Jes Eiler — byte for byte: its channels, `main`, the MIDI map), and the build that
//! keeps every microphone — one mono or stereo 24-bit file per mic and hit, an SFZ
//! multi-mic kit, the main mics beside it.

use cantus_content::drumgizmo;
use cantus_content::kitbuild::mics::{mic_outs, render_mics, sidecar_path};
use cantus_content::manifest::{Build, Choke, KeyMap, Mic};
use std::path::Path;
use std::sync::atomic::AtomicBool;

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/drskit_minimal")
}

#[test]
fn drskits_published_files_read_with_their_main_mics_and_midi_map() {
    let kit = drumgizmo::read_kit(&fixture(), "DRSKit_minimal.xml").unwrap();
    assert_eq!(kit.channels.len(), 13);
    assert_eq!(kit.instruments.len(), 15);
    let crash = kit
        .instruments
        .iter()
        .find(|i| i.name == "Crash_left_shank")
        .unwrap();
    assert_eq!(crash.main, ["AmbL", "AmbR", "OHL", "OHR"]);
    let map = drumgizmo::read_midimap(&fixture(), "Midimap_minimal.xml").unwrap();
    assert_eq!(map.len(), 15);
    assert!(map.contains(&(44, "Hihat_foot".to_string())));
    // Its pairs are one stereo mic each: 13 channels, 11 mics.
    let outs = mic_outs(&kit.channels);
    assert_eq!(outs.len(), 11);
    assert!(outs
        .iter()
        .any(|m| m.name == "OH" && m.channels == ["OHL", "OHR"]));
    assert!(outs
        .iter()
        .any(|m| m.name == "Amb" && m.channels.len() == 2));
}

fn float_wav(path: &Path, channels: u16, frames: usize, f: impl Fn(usize, usize) -> f32) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        for c in 0..channels as usize {
            w.write_sample(f(i, c)).unwrap();
        }
    }
    w.finalize().unwrap();
}

#[test]
fn a_multichannel_build_keeps_every_mic_as_its_own_files_and_names_them_in_an_sfz() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    std::fs::write(
        d.join("kit.xml"),
        r#"<drumkit name="T"><channels><channel name="AmbL"/><channel name="AmbR"/><channel name="Kick"/></channels>
<instruments>
<instrument name="Kick" file="Kick/Kick.xml"><channelmap in="AmbL" out="AmbL"/><channelmap in="AmbR" out="AmbR"/><channelmap in="Kick" out="Kick" main="true"/></instrument>
<instrument name="HatOpen" file="HatOpen/HatOpen.xml"><channelmap in="AmbL" out="AmbL" main="true"/><channelmap in="AmbR" out="AmbR" main="true"/><channelmap in="Kick" out="Kick"/></instrument>
</instruments></drumkit>"#,
    )
    .unwrap();
    for inst in ["Kick", "HatOpen"] {
        let mut x = format!(r#"<instrument version="2.0" name="{inst}"><samples>"#);
        for n in 1..=2 {
            let file = format!("samples/{n}.wav");
            float_wav(&d.join(inst).join(&file), 3, 2_400, |i, c| {
                (c as f32 + 1.0) * 0.1 * n as f32 * (1.0 - i as f32 / 2_400.0)
            });
            x.push_str(&format!(
                r#"<sample name="{inst}-{n}" power="{}">"#,
                n as f32 * 0.4
            ));
            for (c, ch) in ["AmbL", "AmbR", "Kick"].iter().enumerate() {
                x.push_str(&format!(
                    r#"<audiofile channel="{ch}" file="{file}" filechannel="{}"/>"#,
                    c + 1
                ));
            }
            x.push_str("</sample>");
        }
        x.push_str("</samples></instrument>");
        std::fs::write(d.join(inst).join(format!("{inst}.xml")), x).unwrap();
    }
    // A pair is a stereo mic where the manifest pans it apart.
    let mic = |c: &str| Mic {
        channel: c.into(),
        gain_db: -2.0,
        pan: match c {
            "AmbL" => -100.0,
            "AmbR" => 100.0,
            _ => 0.0,
        },
        invert: c == "Kick",
        own: vec![],
        bleed_db: 0.0,
    };
    let key = |i: &str, k: u8, o: Option<f32>| KeyMap {
        instrument: i.into(),
        keys: vec![k],
        gain_db: 0.0,
        label: String::new(),
        openness: o,
    };
    let b = Build {
        kit: "kit.xml".into(),
        unpacked_size: 0,
        sfz: "Kit.sfz".into(),
        folder: "Mics".into(),
        gain_db: 0.0,
        mics: vec![mic("AmbL"), mic("AmbR"), mic("Kick")],
        keys: vec![key("Kick", 36, None), key("HatOpen", 46, Some(0.9))],
        chokes: vec![Choke {
            instruments: vec!["HatOpen".into()],
            release_ms: 60.0,
        }],
        round_robin: 2,
        range_db: 40.0,
        keep: vec![],
        multichannel: true,
    };
    let built = render_mics(
        &b,
        d,
        d,
        "A test kit",
        &AtomicBool::new(false),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(built.clamped, 0);
    // Two mics (Amb a stereo pair, Kick mono), every hit of each.
    let amb = cantus_content::wav::read(&d.join("Mics/Amb/Kick/01.wav")).unwrap();
    assert_eq!(amb.channels, 2);
    let kick = cantus_content::wav::read(&d.join("Mics/Kick/Kick/02.wav")).unwrap();
    assert_eq!(kick.channels, 1);
    assert!((kick.samples[0] - 0.6).abs() < 1e-5, "{}", kick.samples[0]);
    let sfz = std::fs::read_to_string(d.join("Kit.sfz")).unwrap();
    assert_eq!(sfz.matches("<master>").count(), 2);
    assert!(sfz.contains("amplitude=-100"), "the kick mic inverted");
    assert!(sfz.contains("locc4="), "the open hat on CC4");
    assert!(sfz.contains("group=1 off_by=1"));
    let side: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(d.join(sidecar_path("Kit.sfz"))).unwrap())
            .unwrap();
    assert_eq!(side["Kick"]["main"], serde_json::json!([36]));
    assert_eq!(side["Amb"]["main"], serde_json::json!([46]));
}
