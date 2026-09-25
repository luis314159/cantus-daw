//! MIDI into effects (ADR 0022): a slot hears a track's notes, delivered on the audio
//! thread at their sample, and Porta uses them in its MIDI-trigger mode — a note-on opens
//! the gate, the last note-off lets it hold and release.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, peak};
use support::*;

/// A sustained square on the lead's strip (the sound the gate works on), and a second
/// track — muted, its notes only — that plays the gate. 120 BPM: a beat is 24 000 samples.
fn gated() -> (Project, EffectId, TrackId) {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    p.metronome = false;
    p.tracks.truncate(1);
    {
        let f = p.tracks[0].instrument.firmus_mut().expect("Firmus");
        f.osc1_wave = Waveform::Square;
        f.amp_attack_s = 0.0;
        f.amp_sustain = 1.0;
    }
    p.tracks[0].clips.clear();
    let mut drone = p.make_clip("drone", Ticks::ZERO, Ticks::beats(8));
    drone
        .notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(8), 45, 100));
    p.tracks[0].clips.push(drone);
    let mut trigger = p.make_track("Trigger");
    trigger.muted = true;
    trigger.strip = p.tracks[0].strip;
    let mut clip = p.make_clip("hits", Ticks::ZERO, Ticks::beats(8));
    for (beat, len, vel) in [(1, 1, 127), (3, 1, 40), (5, 2, 100)] {
        clip.notes
            .push(p.make_note(Ticks::beats(beat), Ticks::beats(len), 38, vel));
    }
    trigger.clips.push(clip);
    let tid = trigger.id;
    p.tracks.push(trigger);
    let s = p.tracks[0].strip.unwrap();
    let mut gate = p.make_effect(EffectKind::Gate);
    for (k, v) in [
        ("trig", 1.0),
        ("range", 100.0),
        ("att", 0.01),
        ("hold", 0.0),
        ("rel", 20.0),
    ] {
        gate.spec.set(k, v).unwrap();
    }
    gate.midi_from = Some(tid);
    let id = gate.id;
    p.strip_mut(s).unwrap().effects.push(gate);
    (p, id, tid)
}

fn frames_of(x: &[f32]) -> usize {
    x.len() / 2
}

fn at(x: &[f32], i: usize) -> f32 {
    x[2 * i].abs().max(x[2 * i + 1].abs())
}

/// Played by notes, the gate is shut — exact silence — until the note-on's sample, and
/// opens on that very sample.
#[test]
fn a_gate_in_midi_mode_opens_exactly_at_the_note_on() {
    let (p, _, _) = gated();
    let out = render_with(&p, 333);
    let beat = 24_000;
    assert!(frames_of(&out) > beat * 3);
    assert_eq!(
        peak(&out[..2 * beat]),
        0.0,
        "the gate let something through before the first note"
    );
    assert!(
        at(&out, beat) > 0.01,
        "the gate did not open on the note-on's sample ({})",
        at(&out, beat)
    );
    // After the note-off (beat 3) and a 20 ms release it is shut again, until beat 4's note.
    let shut = 2 * beat + 48 * 60;
    assert_eq!(
        peak(&out[2 * shut..2 * (3 * beat)]),
        0.0,
        "closed between the notes"
    );
    assert!(at(&out, 3 * beat) > 0.0, "the second note opens it again");
}

/// Velocity scales the opening: with `vel` at 24 dB, a note at velocity 40 opens it
/// 24 · (1 − 40/127) dB less than a note at 127.
#[test]
fn velocity_scales_the_opening() {
    let (mut p, id, _) = gated();
    let s = p.tracks[0].strip.unwrap();
    let full = render_with(&p, 512);
    p.strip_mut(s)
        .unwrap()
        .effect_mut(id)
        .unwrap()
        .spec
        .set("vel", 24.0)
        .unwrap();
    let scaled = render_with(&p, 512);
    let window = |x: &[f32], beat: usize| {
        let from = beat * 24_000 + 2_400;
        peak(&x[2 * from..2 * (from + 9_600)])
    };
    // Velocity 127 opens fully either way.
    assert_eq!(window(&full, 1), window(&scaled, 1));
    let want_db = -24.0 * (1.0 - 40.0 / 127.0);
    let got_db = 20.0 * (window(&scaled, 3) / window(&full, 3)).log10();
    assert!(
        (got_db - want_db).abs() < 0.05,
        "a soft note opened {got_db:.2} dB, not {want_db:.2} dB"
    );
}

/// Whatever the block size, the same samples: the notes land at their sample, not at a
/// block's edge — with lookahead on, too.
#[test]
fn a_gate_played_by_notes_is_block_size_independent() {
    let (mut p, id, _) = gated();
    let s = p.tracks[0].strip.unwrap();
    let spec = &mut p.strip_mut(s).unwrap().effect_mut(id).unwrap().spec;
    spec.set("look", 3.0).unwrap();
    spec.set("vel", 12.0).unwrap();
    spec.set("hold", 15.0).unwrap();
    let reference = render_with(&p, 4096);
    assert!(peak(&reference) > 0.05);
    for block in [1usize, 7, 64, 1000] {
        assert_null(
            &render_with(&p, block),
            &reference,
            0.0,
            &format!("block {block}"),
        );
    }
}

/// With no MIDI source, nothing changes to the bit: a gate triggered by its key renders
/// the same whether or not its slot also hears a track, and a project where no slot hears
/// one renders as it did before notes into effects existed.
#[test]
fn without_notes_nothing_changes() {
    let (mut p, id, _) = gated();
    let s = p.tracks[0].strip.unwrap();
    // The gate back on its key (the level), a threshold the drone crosses.
    let spec = &mut p.strip_mut(s).unwrap().effect_mut(id).unwrap().spec;
    spec.set("trig", 0.0).unwrap();
    spec.set("thr", -30.0).unwrap();
    let hearing = render_with(&p, 256);
    p.strip_mut(s).unwrap().effect_mut(id).unwrap().midi_from = None;
    let deaf = render_with(&p, 256);
    assert!(peak(&deaf) > 0.05);
    assert_null(
        &hearing,
        &deaf,
        0.0,
        "notes reached a gate that listens to its key",
    );
    // And no slot hearing anything: the demo renders as it always did.
    let plain = demo_project();
    assert_null(
        &render_with(&plain, 512),
        &render_with(&plain, 64),
        0.0,
        "the demo",
    );
}

/// The notes wait for the audio: behind a latent insert on the same strip, the gate
/// opens where the note's audio arrives, not where the note was written.
#[test]
fn notes_wait_for_the_latency_ahead_of_the_slot() {
    let (mut p, id, _) = gated();
    let s = p.tracks[0].strip.unwrap();
    // A linear-phase EQ, flat, ahead of the gate: a pure delay of its latency.
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0).unwrap();
    eq.spec.set("lp_res", 0.0).unwrap();
    p.strip_mut(s).unwrap().effects.insert(0, eq);
    let lat = cantus_dsp::aequalis::latency_of(
        cantus_dsp::aequalis::Mode::LinearPhase,
        cantus_dsp::aequalis::Resolution::Low,
        48_000.0,
    );
    let out = render_with(&p, 500);
    let beat = 24_000;
    assert_eq!(
        peak(&out[..2 * (beat + lat)]),
        0.0,
        "opened before its audio"
    );
    assert!(
        at(&out, beat + lat) > 0.01,
        "not open when the audio arrived"
    );
    let _ = id;
}
