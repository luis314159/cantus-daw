//! Helpers every plug-in's engine tests share: `mod support;` at the top of a test file.
//! A small project that plays (a lead and a bass, each on a strip of its own) and a
//! renderer that turns a project into interleaved stereo samples.
#![allow(dead_code)]

use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

/// The strip a track feeds.
pub fn strip_of(p: &mut Project, track: usize) -> &mut MixerStrip {
    let id = p.tracks[track].strip.expect("track feeds a strip");
    p.strip_mut(id).unwrap()
}

/// A lead riff with a chord and a tail, and a square bass, eight beats at 128 BPM.
pub fn demo_project() -> Project {
    let mut p = Project::new("null");
    p.tempo_bpm = 128.0;
    let mut lead = p.make_track("Lead");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    let riff = [
        60u8, 63, 67, 70, 72, 70, 67, 63, 60, 62, 65, 69, 72, 69, 65, 62,
    ];
    for (i, pitch) in riff.iter().enumerate() {
        let n = p.make_note(
            Ticks::EIGHTH * i as i64,
            Ticks::EIGHTH - Ticks(40),
            *pitch,
            70 + (i as u8 * 3),
        );
        clip.notes.push(n);
    }
    // A chord with overlapping notes and a tail past the clip end.
    clip.notes
        .push(p.make_note(Ticks::beats(6), Ticks::beats(4), 48, 100));
    clip.notes
        .push(p.make_note(Ticks::beats(6), Ticks::beats(4), 55, 100));
    lead.clips.push(clip);
    p.tracks.push(lead);
    {
        let s = strip_of(&mut p, 0);
        s.pan = -0.3;
        s.gain_db = -3.0;
    }

    let mut bass = p.make_track("Bass");
    let f = bass.instrument.firmus_mut().expect("Firmus");
    f.cutoff_hz = 400.0;
    f.osc1_wave = Waveform::Square;
    f.amp_release_s = 0.05;
    let mut clip = p.make_clip("Bass", Ticks::beats(2), Ticks::beats(6));
    for i in 0..6 {
        let n = p.make_note(
            Ticks::beats(i),
            Ticks::QUARTER - Ticks(100),
            36 + (i as u8 % 2) * 7,
            110,
        );
        clip.notes.push(n);
    }
    bass.clips.push(clip);
    p.tracks.push(bass);
    strip_of(&mut p, 1).pan = 0.2;
    p
}

/// Render a project offline, 48 kHz stereo, with a quarter-second tail.
pub fn render_of(project: &Project) -> Vec<f32> {
    render_with(project, 256)
}

/// Render at a given block size — the block-size-independence tests compare these.
pub fn render_with(project: &Project, block_frames: usize) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(project, 1)), &opts)
        .unwrap()
        .samples
}

/// Put an effect of `kind` on the lead's strip; returns its id.
pub fn add_effect(p: &mut Project, kind: EffectKind) -> EffectId {
    let slot = p.make_effect(kind);
    let id = slot.id;
    strip_of(p, 0).effects.push(slot);
    id
}
