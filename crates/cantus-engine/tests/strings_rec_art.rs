//! r42:strings-rec — the note's articulation through the engine (ADR 0158): a clip's note
//! hands its `art` to the instrument just before it starts (`fire`, the one place every
//! arrangement note, every session clip's projected note, every offline render and every
//! block of the real-time pass turns an event into a note-on), so Exemplar plays the zones of
//! that articulation; a note without one — and a note whose articulation the instrument has
//! no zones of — changes no sample, to the bit; block sizes give the same bits; a render
//! started in the middle of the clip hands the art of every note it starts.

use cantus_core::note_art::{Articulation, NoteArt};
use cantus_core::session::play::{plan, project_play, Input, Timed};
use cantus_core::session::*;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

const BAR: Ticks = Ticks::beats(4);

/// An Exemplar of two built-in sines: a sustain zone (full level) and a pizzicato zone
/// (−20 dB) on every key — or, `tagged` false, the same two zones with no articulation.
fn exemplar(tagged: bool) -> NativeSpec {
    let mut spec = NativeSpec::new(InstrumentKind::Sampler);
    for (n, (art, gain)) in [(1.0, 0.0), (5.0, -20.0)].into_iter().enumerate() {
        spec.set(&format!("z{n}.gen"), 1.0);
        spec.set(&format!("z{n}.mode"), 2.0);
        spec.set(&format!("z{n}.gain"), gain);
        if tagged {
            spec.set(&format!("z{n}.art"), art);
        }
    }
    spec.set("amp.release", 5.0);
    spec
}

fn art(a: Articulation) -> Option<NoteArt> {
    Some(NoteArt {
        articulation: a,
        bow: None,
        start: None,
        part: None,
        string: None,
    })
}

/// One track playing four quarter notes a beat apart from bar 1, each with `arts[i]`.
fn song(spec: NativeSpec, arts: [Option<NoteArt>; 4]) -> Project {
    let mut p = Project::new("arts");
    let mut t = p.make_track("Strings");
    t.instrument = InstrumentSpec::Native(spec);
    p.tracks.push(t);
    let mut c = p.make_clip("line", BAR, BAR);
    for (i, a) in arts.into_iter().enumerate() {
        let mut n = p.make_note(Ticks::beats(i as i64), Ticks(800), 60 + i as u8, 100);
        n.art = a;
        c.notes.push(n);
    }
    p.tracks[0].clips.push(c);
    p
}

fn offline(p: &Project, start: Ticks, block: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let opts = RenderOptions {
        start,
        end: Some(BAR * 2 + Ticks::beats(1)),
        block_frames: block,
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| v.to_bits()).collect()
}

/// The RMS of beat `beat` (counted from the render's start) of a stereo render at 120 BPM,
/// 48 kHz: its middle half.
fn level(x: &[f32], beat: usize) -> f32 {
    let frames = 24_000;
    let (a, b) = (
        (beat * frames + frames / 8) * 2,
        (beat * frames + frames / 2) * 2,
    );
    let s = &x[a.min(x.len())..b.min(x.len())];
    (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

#[test]
fn a_notes_articulation_chooses_exemplars_zones_from_a_clip() {
    let none = offline(&song(exemplar(true), [None; 4]), Ticks::ZERO, 256);
    let marked = offline(
        &song(
            exemplar(true),
            [
                art(Articulation::Sustain),
                art(Articulation::Pizzicato),
                None,
                art(Articulation::Staccato), // no staccato: the nearest is the sustain
            ],
        ),
        Ticks::ZERO,
        256,
    );
    let (both, sus, pizz) = (level(&none, 4), level(&marked, 4), level(&marked, 5));
    assert!(both > 0.01, "it plays: {both}");
    assert!(
        sus < both && sus > both * 0.85,
        "the sustain alone: {sus} of {both}"
    );
    assert!(pizz < both * 0.15, "the pizzicato alone: {pizz} of {both}");
    let (plain, stac) = (level(&marked, 6), level(&marked, 7));
    assert!(
        (plain / level(&none, 6) - 1.0).abs() < 1e-4,
        "no art: both zones"
    );
    assert!(
        (stac / sus - 1.0).abs() < 0.05,
        "staccato played as the sustain: {stac} {sus}"
    );
    // Block sizes: the same bits.
    let song_marked = song(
        exemplar(true),
        [
            art(Articulation::Pizzicato),
            None,
            art(Articulation::Sustain),
            None,
        ],
    );
    assert_eq!(
        bits(&offline(&song_marked, Ticks::ZERO, 64)),
        bits(&offline(&song_marked, Ticks::ZERO, 333))
    );
}

#[test]
fn a_note_without_art_or_with_one_the_instrument_lacks_changes_no_sample() {
    let plain = offline(&song(exemplar(false), [None; 4]), Ticks::ZERO, 256);
    // An instrument with no articulation zones: every art plays as none did, to the bit.
    let arts = [
        art(Articulation::Sustain),
        art(Articulation::Pizzicato),
        art(Articulation::Tremolo),
        art(Articulation::Spiccato),
    ];
    let marked = offline(&song(exemplar(false), arts), Ticks::ZERO, 256);
    assert_eq!(bits(&plain), bits(&marked));
    // And the articulation's command undone gives the song back as it played.
    let mut p = song(exemplar(true), [None; 4]);
    let before = offline(&p, Ticks::ZERO, 256);
    let (track, clip) = (p.tracks[0].id, p.tracks[0].clips[0].id);
    let ids: Vec<NoteId> = p.tracks[0].clips[0].notes.iter().map(|n| n.id).collect();
    let inverse = Command::SetNoteArt {
        track,
        clip,
        arts: ids
            .iter()
            .map(|i| (*i, art(Articulation::Pizzicato)))
            .collect(),
    }
    .apply(&mut p)
    .unwrap();
    assert_ne!(bits(&offline(&p, Ticks::ZERO, 256)), bits(&before));
    inverse.apply(&mut p).unwrap();
    assert_eq!(bits(&offline(&p, Ticks::ZERO, 256)), bits(&before));
}

#[test]
fn a_render_started_mid_clip_hands_the_art_of_every_note_it_starts() {
    let p = song(
        exemplar(true),
        [
            art(Articulation::Pizzicato),
            art(Articulation::Pizzicato),
            art(Articulation::Pizzicato),
            art(Articulation::Pizzicato),
        ],
    );
    let whole = offline(&p, Ticks::ZERO, 256);
    // From the third note's beat: what it renders from there is the whole render's.
    let from = offline(&p, BAR + Ticks::beats(2), 256);
    let at = (6 * 24_000) * 2;
    assert_eq!(bits(&from[..48_000]), bits(&whole[at..at + 48_000]));
    assert!(
        level(&from, 0)
            < level(
                &offline(&song(exemplar(true), [None; 4]), Ticks::ZERO, 256),
                6
            ) * 0.15
    );
}

#[test]
fn a_session_clips_notes_carry_their_art_into_the_projection() {
    let mut p = Project::new("live");
    let mut t = p.make_track("Strings");
    t.instrument = InstrumentSpec::Native(exemplar(true));
    p.tracks.push(t);
    let scene = SceneId(p.alloc_id());
    p.session.scenes.push(Scene::new(scene, "S"));
    let mut c = p.make_clip("c", Ticks::ZERO, BAR);
    c.content_length = Some(BAR);
    for i in 0..4 {
        let mut n = p.make_note(Ticks::beats(i), Ticks(800), 60, 100);
        n.art = if i % 2 == 1 {
            art(Articulation::Pizzicato)
        } else {
            None
        };
        c.notes.push(n);
    }
    p.tracks[0].slots = vec![Slot {
        scene,
        clip: Some(SessionClip::new(c)),
        stop: true,
    }];
    let perf = plan(
        &p,
        &[Timed::at(
            Ticks(10),
            Input::LaunchScene {
                scene,
                velocity: 1.0,
            },
        )],
        BAR * 3,
    )
    .perf;
    let q = project_play(&p, &perf);
    let heard: Vec<Option<NoteArt>> = q.tracks[0]
        .clips
        .iter()
        .flat_map(|c| c.notes.iter().map(|n| n.art))
        .collect();
    assert!(heard.contains(&art(Articulation::Pizzicato)), "{heard:?}");
    let snap = Snapshot::compile(&q, 1);
    let ons: Vec<Option<NoteArt>> = snap.tracks[0]
        .events
        .iter()
        .filter(|e| e.is_on())
        .map(|e| e.art)
        .collect();
    assert!(ons.contains(&art(Articulation::Pizzicato)) && ons.contains(&None));
    // And the instrument hears it: the pizzicato beats are the quiet ones.
    let out = render(
        Arc::new(snap),
        &RenderOptions {
            end: Some(BAR * 2),
            tail_seconds: 0.0,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    let beats: Vec<f32> = (4..8).map(|b| level(&out, b)).collect();
    assert!(
        beats[1] < beats[0] * 0.2 && beats[3] < beats[2] * 0.2,
        "{beats:?}"
    );
}
