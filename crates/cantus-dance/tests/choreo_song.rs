//! The choreography end to end (ADR 0087): a project rendered by the engine, its dancer's
//! timeline sampled at a video's frame times, each frame posed.
//!
//! - With no automation an included character dances its routine to the beat.
//! - An automation clip of `move` drawn over bars 3–4 changes the pose exactly from bar 3's
//!   first beat to bar 5's: at 48 kHz and 30 frames a second, frame 120 (4.0 s, 120 BPM)
//!   is the first that claps and frame 239 the last.
//! - Notes on the dancer's track play their moves from their starts.
//! - The same song gives the same poses at 30 and at 144 frames a second where their frame
//!   times meet.

use cantus_core::inst::dancer::{self, Move};
use cantus_core::*;
use cantus_dance::{cast, pose_at};
use cantus_engine::dance::{sample, DanceFrame};
use cantus_engine::render::{render_with_progress, RenderOptions, RenderProgress};
use std::sync::Arc;

/// 120 BPM, 4/4; a lead playing so the song has length; a dancer at its defaults.
fn song() -> (Project, TrackId) {
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
    (p, id)
}

fn timeline(p: &Project) -> Vec<DanceFrame> {
    let progress = RenderProgress::default();
    progress.dance.want_timelines();
    render_with_progress(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            tail_seconds: 0.0,
            ..Default::default()
        },
        Vec::new(),
        Vec::new(),
        &progress,
    )
    .unwrap();
    progress.dance.timelines().remove(0).1
}

fn track_of(p: &mut Project, id: TrackId) -> &mut Track {
    p.track_mut(id).unwrap()
}

#[test]
fn with_no_automation_it_dances_its_routine_to_the_beat() {
    let (p, _) = song();
    let t = timeline(&p);
    let ch = cast::default_character();
    let routine: Vec<Move> = ch
        .routine()
        .iter()
        .map(|m| cantus_dance::pose::resolve(ch.as_ref(), *m))
        .collect();
    assert!(!routine.is_empty());
    for frame in 0..(16 * 30) {
        let s = f64::from(frame) / 30.0;
        let f = sample(&t, s).unwrap();
        let pose = pose_at(&f, ch.as_ref(), 0.0);
        let bar = (s / 2.0).floor() as usize;
        assert!(pose.playing, "frame {frame}");
        assert_eq!(pose.bar as usize, bar, "frame {frame}");
        assert_eq!(pose.mv, routine[bar % routine.len()], "frame {frame}");
        // On the beat: the phase of a one-beat move is the beat's.
        if ch.move_beats(pose.mv) == 1.0 {
            assert!(
                (pose.phase - pose.beat_phase).abs() < 1e-3
                    || (pose.phase - pose.beat_phase).abs() > 0.999,
                "frame {frame}: {} vs {}",
                pose.phase,
                pose.beat_phase
            );
        }
    }
}

#[test]
fn an_automation_clip_changes_the_moves_exactly_on_the_bars_it_was_drawn() {
    let (mut p, id) = song();
    let clap = dancer::move_value(Move::Clap) / 16.0;
    // Bars 3–4: beats 8…16, 4.0 s…8.0 s.
    let auto = p.make_automation_clip(
        "Clap",
        AutoTarget::Instrument {
            track: id,
            key: "move".into(),
        },
        Ticks::beats(8),
        Ticks::beats(8),
        // Clap, held, then back to the routine where the clip ends (between clips the
        // value holds where the last one left it: ADR 0027).
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
    track_of(&mut p, id).clips.push(auto);
    let t = timeline(&p);
    let ch = cast::default_character();
    let clapping = cantus_dance::pose::resolve(ch.as_ref(), Move::Clap);
    // The routine never claps in these bars by itself (a test of the test).
    let plays = |frame: u32| {
        let f = sample(&t, f64::from(frame) / 30.0).unwrap();
        pose_at(&f, ch.as_ref(), 0.0)
    };
    let mut first = None;
    let mut last = None;
    for frame in 0..(12 * 30) {
        let pose = plays(frame);
        let by_automation = sample(&t, f64::from(frame) / 30.0)
            .unwrap()
            .value("move")
            .unwrap()
            >= 1.0;
        if by_automation {
            first.get_or_insert(frame);
            last = Some(frame);
            assert_eq!(pose.mv, clapping, "frame {frame}");
        }
    }
    assert_eq!(first, Some(120), "from bar 3's first beat (4.0 s)");
    assert_eq!(last, Some(239), "to bar 5's (8.0 s, not included)");
    assert_eq!(plays(120).phase, 0.0, "the clap lands on the beat");
    assert_eq!(plays(240).bar, 4);
}

#[test]
fn notes_play_their_moves_from_their_starts() {
    let (mut p, id) = song();
    let mut c = p.make_clip("moves", Ticks::beats(4), Ticks::beats(8));
    for (beat, m) in [(0, Move::Jump), (2, Move::Spin), (5, Move::Wave)] {
        let key = dancer::BASE_KEY + m.index() as u8;
        c.notes
            .push(p.make_note(Ticks::beats(beat), Ticks::QUARTER, key, 120));
    }
    track_of(&mut p, id).clips.push(c);
    let t = timeline(&p);
    let ch = cast::default_character();
    let at = |beats: f64| {
        let f = sample(&t, beats * 0.5).unwrap();
        pose_at(&f, ch.as_ref(), 0.0)
    };
    let r = |m| cantus_dance::pose::resolve(ch.as_ref(), m);
    // Each move's cycle is the character's own (`move_beats`): the expectations follow it.
    let cycle = |m| f64::from(ch.move_beats(r(m)));
    assert_eq!(at(4.5).mv, r(Move::Jump));
    let want = (0.5 / cycle(Move::Jump)).fract() as f32;
    assert!(
        (at(4.5).phase - want).abs() < 1e-3,
        "{} vs {want}",
        at(4.5).phase
    );
    // Spin at beat 6, a tap: one whole cycle (until Wave takes over at beat 9).
    let spin_end = 6.0 + cycle(Move::Spin).min(3.0);
    assert_eq!(at(6.0 + (spin_end - 6.0) * 0.9).mv, r(Move::Spin));
    let half = (1.5 / cycle(Move::Spin)).fract() as f32;
    assert!((at(7.5).phase - half).abs() < 1e-3 || cycle(Move::Spin) < 1.5);
    assert_eq!(at(9.5).mv, r(Move::Wave));
    let want = (0.5 / cycle(Move::Wave)).fract() as f32;
    assert!((at(9.5).phase - want).abs() < 1e-3);
    assert!(at(9.5).energy > 0.6, "velocity 120 dances big");
}

#[test]
fn thirty_and_a_hundred_and_forty_four_frames_a_second_agree() {
    let (mut p, id) = song();
    let spec = track_of(&mut p, id).instrument.native_mut().unwrap();
    spec.set("step", 3.0).unwrap();
    let t = timeline(&p);
    let ch = cast::default_character();
    // 30 and 144 meet every 5/24... at whole multiples of 1/6 s.
    for k in 0..60 {
        let s = f64::from(k) / 6.0;
        let a = pose_at(
            &sample(&t, (f64::from(k * 5)) / 30.0).unwrap(),
            ch.as_ref(),
            0.0,
        );
        let b = pose_at(
            &sample(&t, f64::from(k * 24) / 144.0).unwrap(),
            ch.as_ref(),
            7.0,
        );
        assert_eq!(a, b, "{s} s");
    }
}
