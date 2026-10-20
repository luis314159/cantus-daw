//! *Bounce warped audio* and a `.dawproject`'s carried audio (round 29, ADR 0061): the
//! bounced file is exactly what the clip played — a transposed render, a window trimmed
//! from the left whose warp plays as the source, a looped clip's first piece — and the
//! clip then plays it unwarped; imported media never written over. Its own test binary:
//! these point the data folder (`CANTUS_DATA_DIR`) elsewhere for the whole process.

mod support;

use cantus_core::warp::{Warp, WarpOp};
use cantus_core::{Clip, Command, Project, Ticks, TrackId};
use cantus_dsp::beat::testkit::click_track;
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use support::app;

/// The data folder of this binary, set once.
fn data_dir() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn wav(path: &Path, rate: u32, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for v in x {
        w.write_sample(*v).unwrap();
    }
    w.finalize().unwrap();
}

fn read_wav(path: &Path) -> Vec<f32> {
    let mut r = hound::WavReader::open(path).unwrap();
    r.samples::<f32>().map(Result::unwrap).collect()
}

fn project(bpm: f64) -> Project {
    let mut p = Project::empty("Warp");
    p.tempo_bpm = bpm;
    let t = p.make_track("Keys");
    p.tracks.push(t);
    p
}

fn audio_clip(a: &CantusApp) -> (TrackId, Clip) {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.clone())))
        .find(|(_, c)| c.audio.is_some())
        .unwrap()
}

fn settle(a: &mut CantusApp) {
    let t = Instant::now();
    while a.poll_warp() && t.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// What clip `r` plays now, as an export would (its first piece's data, interleaved).
fn playing(a: &mut CantusApp, t: TrackId, id: cantus_core::ClipId) -> Vec<f32> {
    let snap = a.engine.compile_for_render(&a.project().clone(), 1);
    let track = snap.track(t).unwrap();
    track
        .audio
        .iter()
        .find(|c| c.clip == id)
        .and_then(|c| c.data.clone())
        .unwrap()
        .frames
        .to_vec()
}

/// Bounce clip `r` and wait for it; the file the clip then plays.
fn bounce(a: &mut CantusApp, r: (TrackId, cantus_core::ClipId)) -> Vec<f32> {
    assert!(a.bounce_warped(r));
    settle(a);
    let c = a.project().track(r.0).unwrap().clip(r.1).unwrap().clone();
    let region = c.audio.unwrap();
    assert!(region.warp.is_none(), "it plays the bounce, unwarped");
    assert_eq!(c.offset, Ticks::ZERO);
    let s = a.project().audio_source(region.source).unwrap();
    let p = Path::new(&s.path);
    let file = if p.exists() {
        p.to_path_buf()
    } else {
        a.audio_base().unwrap().join(p)
    };
    read_wav(&file)
}

#[test]
fn a_transposed_clip_bounces_to_exactly_what_it_played() {
    let dir = data_dir().join("transposed");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clicks.wav");
    let m = click_track(48_000.0, 100.0, 4, 0.0);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clip(&a);
    let r = (t, c.id);
    let mut w = Warp::identity(48_000, 100.0, 0);
    w.cents = -300;
    a.commit_warp(r, Some(w), WarpOp::Set, false);
    let before = playing(&mut a, t, c.id);
    let after = bounce(&mut a, r);
    // The file holds the render, channel by channel as played (a mono file of a mono
    // render).
    let mono: Vec<f32> = before.to_vec();
    assert_eq!(after.len(), mono.len());
    assert!(after == mono, "the bounce is the render");
    a.undo();
    assert!(a.clip_warp(r).is_some(), "one undo step back");
}

/// A window trimmed from the left whose warp plays as the source: the bounce is that
/// window, not the whole file (round 29's review: offset zero then replayed the trim).
#[test]
fn a_trimmed_window_bounces_to_its_own_audio() {
    let dir = data_dir().join("trimmed");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clicks.wav");
    let m = click_track(48_000.0, 120.0, 4, 0.0);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clip(&a);
    let r = (t, c.id);
    // Trimmed a bar from the left, then made to follow (a warp that plays as it is).
    a.exec(Command::SetClipOffset {
        track: t,
        clip: c.id,
        offset: Ticks::beats(4),
    });
    a.exec(Command::SetClipBounds {
        track: t,
        clip: c.id,
        start: Ticks::beats(4),
        length: Ticks::beats(8),
    });
    assert!(a.set_warp_follow(r, true));
    let after = bounce(&mut a, r);
    // Its first sample is the source's two seconds in (a bar at 120).
    let from = 2 * 48_000;
    assert!(after.len() >= 4 * 48_000 - 1);
    for (k, v) in after.iter().take(48_000).enumerate() {
        assert_eq!(v.to_bits(), m.audio[from + k].to_bits(), "frame {k}");
    }
}

/// A looped clip bounces its first piece.
#[test]
fn a_looped_clip_bounces_its_first_piece() {
    let dir = data_dir().join("looped");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clicks.wav");
    let m = click_track(48_000.0, 100.0, 2, 0.0);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clip(&a);
    let r = (t, c.id);
    a.exec(Command::SetClipLoop {
        track: t,
        clip: c.id,
        content_length: Some(Ticks::beats(4)),
    });
    a.exec(Command::SetClipBounds {
        track: t,
        clip: c.id,
        start: Ticks::ZERO,
        length: Ticks::beats(12),
    });
    let mut w = Warp::identity(48_000, 100.0, 0);
    w.cents = 200;
    a.commit_warp(r, Some(w), WarpOp::Set, false);
    let before = playing(&mut a, t, c.id);
    let after = bounce(&mut a, r);
    assert!(after == before, "the first piece, as played");
}

/// A `.dawproject` imported twice under one name never writes over the first one's audio.
#[test]
fn imported_media_never_writes_over_an_earlier_import() {
    let _ = data_dir();
    let mut p = project(120.0);
    let id = cantus_core::AudioSourceId(p.alloc_id());
    p.audio_pool.push(cantus_core::AudioSource {
        id,
        name: "x".into(),
        path: "audio/1 x.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 4,
        sha256: None,
        recorded: false,
    });
    let media = vec![("audio/1 x.wav".to_string(), b"one".to_vec())];
    let mut first = p.clone();
    assert!(cantus_ui::warp::place_media(&mut first, &media, "song.dawproject").is_empty());
    let media2 = vec![("audio/1 x.wav".to_string(), b"two".to_vec())];
    let mut second = p.clone();
    cantus_ui::warp::place_media(&mut second, &media2, "song.dawproject");
    let (a, b) = (&first.audio_pool[0].path, &second.audio_pool[0].path);
    assert_ne!(a, b);
    assert_eq!(std::fs::read(a).unwrap(), b"one");
    assert_eq!(std::fs::read(b).unwrap(), b"two");
}

/// Round 29's second review (finding 30): a bounce whose clip was edited while it was
/// written is not applied (the file holds the old window) — and one made for another
/// project never lands on this one's clip of the same ids.
#[test]
fn a_bounce_lands_only_on_the_clip_it_was_made_from() {
    let dir = data_dir().join("stale");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 4, 0.0).audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clip(&a);
    let r = (t, c.id);
    let mut w = Warp::identity(48_000, 100.0, 0);
    w.cents = -300;
    a.commit_warp(r, Some(w), WarpOp::Set, false);
    // Edited while it is written.
    assert!(a.bounce_warped(r));
    a.exec(Command::SetClipOffset {
        track: t,
        clip: c.id,
        offset: Ticks::beats(1),
    });
    settle(&mut a);
    let now = a.project().track(t).unwrap().clip(c.id).unwrap().clone();
    assert!(now.audio.as_ref().unwrap().warp.is_some(), "not applied");
    assert_eq!(now.offset, Ticks::beats(1));
    // Another project (the same, opened again) while it is written.
    assert!(a.bounce_warped(r));
    let again = a.project().clone();
    a.replace_project(again);
    settle(&mut a);
    let now = a.project().track(t).unwrap().clip(c.id).unwrap().clone();
    assert!(now.audio.as_ref().unwrap().warp.is_some(), "not applied");
}
