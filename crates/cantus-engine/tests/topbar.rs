//! The top bar's engine side (round 28, ADR 0052): the metronome's voice and accent, the
//! master's loudness in `Meters`, and pattern mode played sample-accurately.

mod support;

use cantus_core::pattern::{audition, PatternRegion};
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;
use support::demo_project;

const SR: f64 = 48_000.0;

/// Play `snap` from `from` for `frames`, in blocks of uneven sizes; the output and the last
/// meters the engine sent.
fn play(snap: Snapshot, from: Ticks, frames: usize) -> (Vec<f32>, Meters) {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Seek(from)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let (mut out, mut meters) = (Vec::new(), Meters::default());
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while let Ok(e) = rx.pop() {
            if let EngineEvent::Meters(m) = e {
                meters = m;
            }
        }
    }
    (out, meters)
}

/// An empty song with the metronome on, its click as `set` makes it: one bar of output.
fn click_bar(set: impl FnOnce(&mut RecordSettings)) -> Vec<f32> {
    let mut p = Project::new("Click");
    p.metronome = true;
    p.loop_enabled = false;
    set(&mut p.recording);
    let frames = (SR * 2.0) as usize; // one 4/4 bar at 120 BPM
    play(Snapshot::compile(&p, 1), Ticks::ZERO, frames).0
}

/// The left channel of the beat starting at `beat` (0-based), 40 ms of it.
fn beat(out: &[f32], beat: usize) -> Vec<f32> {
    let at = beat * (SR / 2.0) as usize;
    (at..at + (SR * 0.04) as usize)
        .map(|i| out[i * 2])
        .collect()
}

#[test]
fn the_click_plays_the_chosen_voice_and_accent() {
    let classic = click_bar(|_| {});
    let wood = click_bar(|r| r.click_sound = ClickSound::Wood);
    let beep = click_bar(|r| r.click_sound = ClickSound::Beep);
    assert!(beat(&classic, 0).iter().any(|x| x.abs() > 0.1), "a click");
    assert_ne!(
        beat(&classic, 0),
        beat(&wood, 0),
        "wood is not the classic click"
    );
    assert_ne!(beat(&wood, 0), beat(&beep, 0), "beep is its own");
    // Accented: the downbeat differs from the second beat; without the accent, it is the
    // second beat to the bit — for every voice.
    for sound in ClickSound::ALL {
        let on = click_bar(|r| r.click_sound = sound);
        assert_ne!(beat(&on, 0), beat(&on, 1), "{sound:?} accented");
        let off = click_bar(|r| {
            r.click_sound = sound;
            r.click_accent = false;
        });
        assert_eq!(beat(&off, 0), beat(&off, 1), "{sound:?} without accent");
    }
}

/// The master's loudness reaches `Meters`: momentary, short-term and integrated LUFS and
/// the block's true peak, which is never below its sample peak.
#[test]
fn the_master_loudness_is_metered() {
    let p = demo_project();
    let mut snap = Snapshot::compile(&p, 1);
    snap.loop_region = None;
    let (out, m) = play(snap, Ticks::ZERO, (SR * 3.5) as usize);
    let l = m.master_loudness;
    assert!(
        l.momentary.is_finite() && l.momentary > -40.0 && l.momentary < 0.0,
        "{l:?}"
    );
    assert!(l.short_term.is_finite() && l.short_term < 0.0, "{l:?}");
    assert!(l.integrated.is_finite() && l.integrated < 0.0, "{l:?}");
    let block_peak = m.master[0].max(m.master[1]);
    assert!(
        l.true_peak >= block_peak,
        "true {} < sample {block_peak}",
        l.true_peak
    );
    assert!(out.iter().any(|x| x.abs() > 0.01));
    // Silence: nothing measured yet reads as minus infinity, not as a number.
    let quiet = Project::new("Quiet");
    let (_, m) = play(Snapshot::compile(&quiet, 1), Ticks::ZERO, 4_800);
    assert_eq!(m.master_loudness.momentary, f32::NEG_INFINITY);
    assert_eq!(m.master_loudness.true_peak, 0.0);
}

/// Pattern mode (`cantus_core::pattern`): the engine plays the pattern alone, looped over
/// its window, to the sample — exactly a song made of that pattern laid end to end.
#[test]
fn pattern_mode_plays_the_pattern_looped_to_the_sample() {
    let bar = Ticks::beats(4);
    let mut p = demo_project();
    // The demo's lead and bass start at bar 1; the lead's clip is the pattern being edited.
    let lead = p.tracks[0].id;
    let pattern_clip = p.tracks[0].clips[0].id;
    // A clip elsewhere in the song that pattern mode must not play.
    let mut far = p.make_clip("Far", bar * 6, bar);
    let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 84, 120);
    far.notes.push(n);
    p.tracks[0].clips.push(far);
    let region = PatternRegion::of(&p, lead, pattern_clip).expect("a pattern");
    let heard = audition(&p, Some(&region));
    let loops = 3usize;
    let secs = region.length.to_seconds(p.tempo_bpm);
    let frames = (secs * SR).round() as usize * loops;
    let (looped, _) = play(Snapshot::compile(&heard, 1), region.start, frames);

    // The same pattern, laid end to end three times as a song, nothing else.
    let mut song = p.clone();
    for t in &mut song.tracks {
        let keep = region.clip_of(t.id);
        let mut copies = Vec::new();
        for c in t.clips.iter().filter(|c| Some(c.id) == keep) {
            for k in 0..loops as i64 {
                let mut copy = c.clone();
                copy.start = c.start + region.length * k;
                copy.length = c.end().min(region.end()) - c.start;
                copy.content_length = None;
                copy.id = ClipId(10_000 + copy.id.0 * 10 + k as u64);
                copies.push(copy);
            }
        }
        t.clips = copies;
    }
    song.loop_enabled = false;
    let (laid, _) = play(Snapshot::compile(&song, 1), region.start, frames);
    assert_eq!(looped.len(), laid.len());
    let first_diff = looped
        .iter()
        .zip(&laid)
        .position(|(a, b)| a.to_bits() != b.to_bits());
    assert_eq!(
        first_diff, None,
        "pattern mode differs from the pattern laid out"
    );
    assert!(looped.iter().any(|x| x.abs() > 0.01), "the pattern sounds");
}
