//! The song's video (ADR 0088 §6, ADR 0087): a project with a dancer rendered by the
//! engine, its timeline drawn at every frame's time.
//!
//! - An automation clip of `move` drawn over bars 3–4 (4.0 s…8.0 s at 120 BPM) changes the
//!   video's poses exactly on those bars' frames: at 30 fps frame 120 is the first that
//!   claps and frame 239 the last.
//! - The video's frame at time t is the screen's pose at t: the frames written in the file
//!   are, bit for bit, the renderer's picture of `pose_at(sample(timeline, t))`.

use cantus_core::inst::dancer::{self, Move};
use cantus_core::*;
use cantus_dance::render::{render_into, Background, Stage};
use cantus_dance::{cast, pose_at, Character};
use cantus_engine::dance::sample;
use cantus_engine::render::{render_with_progress, RenderOptions, RenderProgress};
use cantus_video::audio::Pcm;
use cantus_video::dance::{Dancer, SongFrames};
use cantus_video::{export, Container, Control, Quality, Request};
use std::io::Cursor;
use std::sync::Arc;

/// 120 BPM, 4/4, 16 bars of a lead; a dancer that claps over bars 3–4.
fn song() -> Project {
    let mut p = Project::new("dance");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let mut c = p.make_clip("beats", Ticks::ZERO, Ticks::beats(32));
    for b in 0..32 {
        c.notes
            .push(p.make_note(Ticks::beats(b), Ticks::EIGHTH, 60, 90));
    }
    lead.clips.push(c);
    p.tracks.push(lead);
    let mut t = p.make_track("Dancer");
    let id = t.id;
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Dancer));
    p.tracks.push(t);
    let clap = dancer::move_value(Move::Clap) / 16.0;
    let auto = p.make_automation_clip(
        "Clap",
        AutoTarget::Instrument {
            track: id,
            key: "move".into(),
        },
        Ticks::beats(8),
        Ticks::beats(8),
        vec![
            AutoPoint {
                tick: Ticks::ZERO,
                value: clap,
                tension: 0.0,
                shape: Shape::Hold,
            },
            AutoPoint {
                tick: Ticks::beats(8),
                value: 0.0,
                tension: 0.0,
                shape: Shape::Hold,
            },
        ],
    );
    p.track_mut(id).unwrap().clips.push(auto);
    p
}

/// The song rendered, and its dancer's timeline.
fn render(p: &Project) -> (Pcm, Vec<cantus_dance::DanceFrame>) {
    let progress = RenderProgress::default();
    progress.dance.want_timelines(); // r35:dancer
    let r = render_with_progress(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            sample_rate: 48_000,
            channels: 2,
            tail_seconds: 0.0,
            ..Default::default()
        },
        Vec::new(),
        Vec::new(),
        &progress,
    )
    .unwrap();
    let pcm = Pcm {
        rate: r.sample_rate,
        channels: r.channels,
        samples: r.samples,
    };
    (pcm, progress.dance.timelines().remove(0).1)
}

fn frames(timeline: Vec<cantus_dance::DanceFrame>, ch: Arc<dyn Character>) -> SongFrames {
    SongFrames {
        dancers: vec![Dancer {
            character: ch,
            timeline,
        }],
        stage: Stage::new(320, 180, Background::Transparent),
        start_seconds: 0.0,
        fps: 30,
        overlay: None,
    }
}

#[test]
fn automation_of_move_over_bars_3_and_4_changes_exactly_those_frames() {
    let (_, timeline) = render(&song());
    let ch = cast::default_character();
    let clapping = cantus_dance::pose::resolve(ch.as_ref(), Move::Clap);
    let src = frames(timeline.clone(), ch.clone());
    let claps: Vec<u32> = (0..16 * 30)
        .filter(|&n| src.poses(n)[0].mv == clapping)
        .collect();
    assert_eq!(claps.first(), Some(&120));
    assert_eq!(claps.last(), Some(&239));
    assert_eq!(claps.len(), 120, "every frame of bars 3–4, and only those");
    // The screen's rule at the same moment gives the same pose.
    for n in [0u32, 119, 120, 200, 239, 240, 400] {
        let t = n as f64 / 30.0;
        let screen = pose_at(&sample(&timeline, t).unwrap(), ch.as_ref(), t);
        assert_eq!(src.poses(n)[0], screen, "frame {n}");
    }
}

#[test]
fn the_files_frames_are_the_screens_pictures() {
    let (pcm, timeline) = render(&song());
    let ch = cast::default_character();
    let src = frames(timeline.clone(), ch.clone());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.mov");
    let req = Request {
        container: Container::MovPng,
        width: 320,
        height: 180,
        fps: 30,
        frames: 300,
        quality: Quality::High,
        alpha: true,
        audio: &pcm,
        workers: 0,
        replace: false,
        plan: None,
    };
    export(&req, &src, &path, &Control::new()).unwrap();
    let file = std::fs::read(&path).unwrap();
    let moov = cantus_video::bmff::top(&file, b"moov").unwrap();
    let trak = moov.child(b"trak").unwrap();
    let stbl = trak.find("mdia/minf/stbl").unwrap();
    let (stsz, stco, stsc) = (
        stbl.child(b"stsz").unwrap(),
        stbl.child(b"stco").unwrap(),
        stbl.child(b"stsc").unwrap(),
    );
    assert_eq!(stsc.u32_at(4), 1, "one frame a chunk (audio between)");
    for n in [0usize, 119, 120, 239, 240, 299] {
        let (at, len) = (
            stco.u32_at(8 + 4 * n) as usize,
            stsz.u32_at(12 + 4 * n) as usize,
        );
        let mut d = png::Decoder::new(Cursor::new(&file[at..at + len]))
            .read_info()
            .unwrap();
        let mut got = vec![0; d.output_buffer_size().unwrap()];
        d.next_frame(&mut got).unwrap();
        // The screen's picture at the same moment, straight alpha.
        let t = n as f64 / 30.0;
        let pose = pose_at(&sample(&timeline, t).unwrap(), ch.as_ref(), t);
        let mut pm = tiny_skia::Pixmap::new(320, 180).unwrap();
        render_into(
            &mut pm,
            &Stage::new(320, 180, Background::Transparent),
            &[(ch.as_ref(), pose)],
        );
        let mut want = pm.data().to_vec();
        cantus_video::yuv::demultiply(&mut want);
        assert!(got == want, "frame {n} is the screen's picture");
    }
}
