//! Round 37's second fixes review, item 1: every path that makes a track takes its strip by
//! the one rule (`Project::free_strips`) — never a split kit's pad strip (inserted in the
//! middle of the desk), a pad's chain or a VCA, nor a strip one of those plays into. Each
//! path as a user takes it, through `CantusApp::assemble`: a sample dropped on the channel
//! rack, audio imported, a MIDI file imported, *Convert to MIDI…* of an audio clip, and
//! *Double* of a voice. Its own binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ml::transcribe::spec::{Mode, NoteEvent, Transcription};
use cantus_ui::app::Files;
use cantus_ui::transcribe_ui as tr;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

const SR: u32 = 48_000;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// A sung-like tone: a vowel-ish 220 Hz with a slow vibrato, `secs` long.
fn wav(dir: &Path, name: &str, secs: f32) -> PathBuf {
    let path = dir.join(name);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let n = (secs * SR as f32) as usize;
    let mut ph = 0f32;
    for i in 0..n {
        let t = i as f32 / SR as f32;
        let f = 220.0 * (1.0 + 0.004 * (2.0 * std::f32::consts::PI * 5.0 * t).sin());
        ph += 2.0 * std::f32::consts::PI * f / SR as f32;
        let x = 0.3 * ph.sin() + 0.12 * (2.0 * ph).sin() + 0.06 * (3.0 * ph).sin();
        w.write_sample(x).unwrap();
    }
    w.finalize().unwrap();
    path
}

/// A kit split to strips — its pad strips right after its own, before the desk's empty
/// strips — a chain on the kick, and a VCA.
fn booted() -> (CantusApp, TrackId) {
    let _ = data();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut p = Project::new("Free");
    p.tempo_bpm = 120.0;
    let mut kit = p.make_track("Drums");
    kit.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    p.tracks.push(kit);
    let mut vca = p.make_strip("VCA");
    vca.kind = vca::StripKind::Vca;
    p.strips.insert(1, vca);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(p),
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    let t = app.project().tracks[0].id;
    let kick = app.project().tracks[0]
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .find(|x| x.pitch == 36)
        .unwrap()
        .id;
    assert!(app.add_to_pad_chain(t, kick, EffectKind::Eq));
    assert!(app.split_kit(t));
    // Pad strips sit before free audio strips: a rule that skips nothing hands them out.
    let p = app.project();
    let first_pad = p.strips.iter().position(|s| s.kind == vca::StripKind::Pad);
    let first_empty = p.strips.iter().position(|s| p.strip_is_free(s));
    assert!(first_pad.is_some() && first_empty.is_some() && first_pad < first_empty);
    (app, t)
}

/// The strips nothing new may take: pad-made strips, VCAs, and every strip one of those
/// plays into, as the kit left them.
fn kept(p: &Project) -> Vec<StripId> {
    let mut out: Vec<StripId> = p
        .strips
        .iter()
        .filter(|s| s.kind != vca::StripKind::Audio)
        .map(|s| s.id)
        .collect();
    for s in &p.strips {
        if s.kind != vca::StripKind::Audio {
            out.extend(s.output);
        }
    }
    out
}

/// Each track `after` holds that `before` did not: on no strip it may not take.
fn check(before: &Project, after: &Project, what: &str) {
    let kept = kept(before);
    let new: Vec<&Track> = after
        .tracks
        .iter()
        .filter(|t| before.track(t.id).is_none())
        .collect();
    assert!(!new.is_empty(), "{what}: made a track");
    for t in new {
        if let Some(s) = t.strip {
            assert!(
                !kept.contains(&s),
                "{what}: {} took {:?}, a strip of the kit's or a VCA",
                t.name,
                after.strip(s).map(|x| (&x.name, x.kind))
            );
        }
    }
}

#[test]
fn a_sample_dropped_on_the_rack_takes_no_pad_strip() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = booted();
    let before = app.project().clone();
    let path = wav(dir.path(), "hit.wav", 0.3);
    assert!(app.add_sample_channel(&path, 1).is_some());
    check(&before, app.project(), "a sample on the rack");
}

#[test]
fn imported_audio_takes_no_pad_strip() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = booted();
    let before = app.project().clone();
    let a = wav(dir.path(), "a.wav", 0.5);
    let b = wav(dir.path(), "b.wav", 0.5);
    assert_eq!(app.import_audio(&[a, b], Ticks::ZERO, None), 2);
    check(&before, app.project(), "imported audio");
}

#[test]
fn an_imported_midi_file_takes_no_pad_strip() {
    let (mut app, _) = booted();
    let before = app.project().clone();
    // A format-0 file, 480 ticks a quarter: one middle C a beat long.
    let mut smf: Vec<u8> = b"MThd".to_vec();
    smf.extend_from_slice(&[0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xE0]);
    let track = [
        0x00, 0x90, 0x3C, 0x64, 0x83, 0x60, 0x80, 0x3C, 0x00, 0x00, 0xFF, 0x2F, 0x00,
    ];
    smf.extend_from_slice(b"MTrk");
    smf.extend_from_slice(&(track.len() as u32).to_be_bytes());
    smf.extend_from_slice(&track);
    assert!(cantus_ui::views::interop::import_midi(
        &mut app,
        &smf,
        "one.mid",
        true,
        false,
        Ticks::ZERO,
        None
    ));
    check(&before, app.project(), "a MIDI import");
}

#[test]
fn convert_to_midi_and_double_take_no_pad_strip() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = booted();
    let voice = wav(dir.path(), "voice.wav", 2.0);
    assert_eq!(app.import_audio(&[voice], Ticks::ZERO, None), 1);
    let (vt, vc) = {
        let t = app.project().tracks.last().unwrap();
        (t.id, t.clips[0].id)
    };
    // *Convert to MIDI…*: a new instrument track under the voice.
    let before = app.project().clone();
    let src = tr::source(&app, vt, vc).unwrap();
    let heard = Transcription {
        mode: Mode::Melody,
        duration: 2.0,
        notes: vec![NoteEvent {
            start: 0.1,
            end: 1.0,
            pitch: 57,
            velocity: 90,
            amplitude: 0.7,
            bends: Vec::new(),
            curve: Vec::new(),
        }],
        hits: Vec::new(),
    };
    let settings = tr::Settings {
        mode: Mode::Melody,
        ..tr::Settings::default()
    };
    let planned = tr::plan(&mut app.bus, &src, &settings, &heard).unwrap();
    assert!(app.exec(planned.command));
    check(&before, app.project(), "Convert to MIDI");
    // *Double*: a second voice on a new track, once the voice is analysed.
    let start = std::time::Instant::now();
    while !matches!(app.pitch_take(vt, vc), cantus_ui::pitch::Take::Ready(_))
        && start.elapsed() < std::time::Duration::from_secs(60)
    {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let before = app.project().clone();
    if app
        .pitch_double(vt, vc, cantus_core::pitch::midi::Interval::Third)
        .is_some()
    {
        check(&before, app.project(), "Double");
    } else {
        panic!("Double made nothing of the voice");
    }
}

#[test]
fn the_rule_is_one_and_skips_every_strip_of_the_kits() {
    let (app, kit) = booted();
    let p = app.project();
    let free: Vec<StripId> = p.free_strips().collect();
    assert_eq!(p.first_free_strip(), free.first().copied());
    for s in &free {
        let st = p.strip(*s).unwrap();
        assert_eq!(st.kind, vca::StripKind::Audio, "{}", st.name);
        assert!(p.tracks_on_strip(*s).next().is_none());
        assert!(!p.tracks.iter().any(|t| t.id == kit && t.strip == Some(*s)));
    }
}
