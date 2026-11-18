//! A render that starts mid-song is the whole render from there (offline = real time's
//! rule, ADR 0005's null tests), a native effect's automated parameter too.
//!
//! Found by wave D's e2e (`crates/cantus-ui/tests/fxd_e2e.rs`,
//! `a_path_plays_in_time_with_the_song`): a native effect's unit is made at its *stored*
//! values, and at the first control point of a render that starts later its own de-zipper
//! (Aequalis's, about 20 ms) glided from the stored value to the automated one — the first
//! note after the start differed from the whole render's (up to 5.2e-2 measured for an EQ
//! band held at +9 dB by a lane). Fixed in fx/final (`DSP_VERSION` 8): where the playhead
//! jumps — a start, a seek, a loop's wrap, a sync start or jump — the processor snaps every
//! unit a lane drives (`EffectUnit::snap`) after the control point there, so it takes the
//! automated value at once; a change while playing still glides. Live playback started at
//! bar 3 (from a seek) is the render from bar 3, to the bit, too.

mod support;
use cantus_core::automation::range::range_of;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;
use support::heavy::{random_instrument, Rng};

const SR: u32 = 48_000;
/// 120 BPM: a beat is half a second.
const FRAMES_PER_BEAT: usize = SR as usize / 2;
const BEATS: i64 = 16;
/// Bar 3 (4/4).
const FROM_BEAT: i64 = 8;

/// A Firmus track playing A3 and A6, a sixteenth each, on every beat (Firmus's cutoff and
/// level left at its defaults: its own smoothers start there, the other finding of the
/// e2e), an Aequalis bell at 220 Hz on its strip stored flat (0 dB) and a lane holding that
/// band at +9 dB for the whole song.
fn song() -> Project {
    let mut p = Project::new("render from bar N");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let f = lead.instrument.firmus_mut().expect("Firmus");
    f.osc1_wave = Waveform::Sine;
    f.osc2_wave = Waveform::Sine;
    f.osc_mix = 0.0;
    f.osc2_detune_cents = 0.0;
    f.resonance = 0.0;
    f.filter_env_octaves = 0.0;
    f.key_track = 1.0;
    f.amp_attack_s = 0.0;
    f.amp_decay_s = 0.0;
    f.amp_sustain = 1.0;
    f.amp_release_s = 0.02;
    let mut clip = p.make_clip("beats", Ticks::ZERO, Ticks::beats(BEATS));
    for b in 0..BEATS {
        for k in [57u8, 93] {
            let n = p.make_note(Ticks::beats(b), Ticks(PPQ / 4), k, 100);
            clip.notes.push(n);
        }
    }
    lead.clips.push(clip);
    let strip = lead.strip.expect("a strip");
    p.tracks.push(lead);
    let mut eq = p.make_effect(EffectKind::Eq);
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.freq", 220.0),
        ("b0.q", 2.0),
        ("b0.gain", 0.0),
    ] {
        eq.spec.set(k, v).expect("an EQ parameter");
    }
    let id = eq.id;
    p.strip_mut(strip).unwrap().effects.push(eq);
    let target = AutoTarget::Effect {
        effect: id,
        key: "b0.gain".into(),
    };
    let held = range_of(&p, &target).expect("the band's range").pos(9.0);
    let lane = p.make_automation_clip(
        "band 0 at +9 dB",
        target,
        Ticks::ZERO,
        Ticks::beats(BEATS),
        vec![
            AutoPoint::new(Ticks::ZERO, held),
            AutoPoint::new(Ticks::beats(BEATS), held),
        ],
    );
    p.tracks[0].clips.push(lane);
    p
}

fn render_beats(p: &Project, from: i64, to: i64) -> Vec<f32> {
    let opts = RenderOptions {
        start: Ticks::beats(from),
        end: Some(Ticks::beats(to)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("the render")
        .samples
}

fn worst(a: &[f32], b: &[f32]) -> (usize, f32) {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .enumerate()
        .fold((0, 0.0), |m, (i, d)| if d > m.1 { (i / 2, d) } else { m })
}

/// From bar 3 the render is the whole render's bars 3… to the bit — every sample, the
/// first note's too — with an EQ band automated.
#[test]
fn a_render_from_bar_3_is_the_whole_render_from_bar_3_with_an_effect_automated() {
    let p = song();
    let whole = render_beats(&p, 0, BEATS);
    let late = render_beats(&p, FROM_BEAT, BEATS);
    let tail = &whole[FROM_BEAT as usize * FRAMES_PER_BEAT * 2..];
    assert_eq!(late.len(), tail.len(), "the same length from bar 3");
    // The lane is heard: band 0 at +9 dB is not the flat EQ.
    let mut flat = p.clone();
    flat.tracks[0].clips.retain(|c| c.automation.is_none());
    let dry = render_beats(&flat, 0, BEATS);
    assert!(worst(&whole, &dry).1 > 0.1, "the lane moves the band");
    // From the second beat of bar 3 on it is already the whole render's, to the bit.
    let one = FRAMES_PER_BEAT * 2;
    let (at, d) = worst(&late[one..], &tail[one..]);
    assert_eq!(
        d,
        0.0,
        "from beat 9 on: {d:e} at frame {}",
        at + FRAMES_PER_BEAT
    );
    // The first note of bar 3: the unit should start at the automated +9 dB, not glide
    // there from its stored 0 dB (measured before the fix: up to ~5e-2 in its first 20 ms).
    let (at, d) = worst(&late[..one], &tail[..one]);
    assert_eq!(
        d,
        0.0,
        "bar 3's first beat differs by {d:e} at frame {at} ({:.1} ms after the start)",
        at as f64 * 1_000.0 / f64::from(SR)
    );
}

/// The live engine over `p` (blocks of varied sizes, the events answered after each as the
/// backend does): the playhead moved by `seeks` (each a `Seek` while stopped, some blocks
/// run there), then played — `frames` frames of what it plays.
fn live(p: &Project, seeks: &[i64], frames: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        let unit = EffectUnit::new(slot, SR as f32);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    let mut run = |proc: &mut Processor, tx: &mut rtrb::Producer<EngineMessage>, n: usize| {
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::EffectNeed {
                effect,
                pedal,
                need,
            } = ev
            {
                let memory = EffectMemory::build(need.need(), SR as f32);
                tx.push(EngineMessage::EffectMemory {
                    effect,
                    pedal,
                    memory,
                })
                .unwrap();
            }
        }
        buf
    };
    // Stopped a while at each place it is moved to, as a user would.
    for &beat in seeks {
        tx.push(EngineMessage::Seek(Ticks::beats(beat))).unwrap();
        for n in [700, 64] {
            let _ = run(&mut proc, &mut tx, n);
        }
    }
    tx.push(EngineMessage::Play).unwrap();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut out = Vec::with_capacity(frames * 2);
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        out.extend_from_slice(&run(&mut proc, &mut tx, n));
    }
    out
}

/// Live playback started at bar 3 — moved there while stopped, from the start or from later
/// in the song — is the render from bar 3 to the bit, its first note's 20 ms too; played from
/// the start it is the whole render.
#[test]
fn live_playback_from_bar_3_is_the_render_from_bar_3_with_an_effect_automated() {
    let p = song();
    let frames = FRAMES_PER_BEAT * 4;
    let late = render_beats(&p, FROM_BEAT, BEATS);
    for seeks in [&[FROM_BEAT][..], &[0, 12, FROM_BEAT][..]] {
        let heard = live(&p, seeks, frames);
        let (at, d) = worst(&heard, &late[..frames * 2]);
        assert_eq!(
            d,
            0.0,
            "played after seeking to {seeks:?}: {d:e} at frame {at} ({:.1} ms in)",
            at as f64 * 1_000.0 / f64::from(SR)
        );
    }
    let whole = render_beats(&p, 0, BEATS);
    let heard = live(&p, &[0], frames);
    let (at, d) = worst(&heard, &whole[..frames * 2]);
    assert_eq!(d, 0.0, "played from the start: {d:e} at frame {at}");
}

/// A Firmus track whose cutoff and level are far from the factory's, playing A3 on every
/// beat; `skip` tracks made first and left out, so its id is another song's.
fn bright(name: &str, skip: usize, cutoff_hz: f32, gain_db: f32) -> Project {
    let mut p = Project::new(name);
    p.tempo_bpm = 120.0;
    for _ in 0..skip {
        let _ = p.make_track("left out");
    }
    let mut t = p.make_track(name);
    let f = t.instrument.firmus_mut().expect("Firmus");
    f.cutoff_hz = cutoff_hz;
    f.gain_db = gain_db;
    f.resonance = 0.0;
    f.filter_env_octaves = 0.0;
    let mut clip = p.make_clip("beats", Ticks::ZERO, Ticks::beats(4));
    for b in 0..4 {
        let n = p.make_note(Ticks::beats(b), Ticks(PPQ / 2), 57, 100);
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

/// fx/final (`lenis_e2e`'s finding) — playback started from stop is the offline render, to
/// the bit, from the first note, whatever the processor heard before: a song replaced by
/// another whose track has the old one's id (the slot kept) or another id (the slot reset),
/// with no block run between the two songs or after the old one was heard stopped. The
/// slot's Firmus held the old track's cutoff and level and, with no idle block run since
/// the new song came, glided 20 ms from them; a render's synth is new and takes its first
/// settings at once. At `Play` every idle Firmus snaps.
#[test]
fn playback_from_stop_after_another_song_is_the_render() {
    let old = bright("Old", 0, 300.0, -18.0);
    let frames = FRAMES_PER_BEAT * 2;
    for (skip, stopped) in [(0usize, 0usize), (0, 700), (3, 0), (3, 700)] {
        let new = bright("New", skip, 16_000.0, 0.0);
        assert_eq!(
            old.tracks[0].id == new.tracks[0].id,
            skip == 0,
            "the same id, or another"
        );
        let whole = render_beats(&new, 0, 4);
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
        tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &old, 1,
        ))))
        .unwrap();
        // `stopped` 0: both songs reach the processor in one block's messages.
        let mut buf = vec![0f32; 2 * 64];
        for _ in 0..stopped / 64 {
            proc.process(&mut buf, 2);
        }
        tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &new, 1,
        ))))
        .unwrap();
        tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let mut heard = Vec::with_capacity(frames * 2);
        let sizes = [128usize, 333, 512, 96, 1024, 17];
        let mut i = 0;
        while heard.len() / 2 < frames {
            let n = sizes[i % sizes.len()].min(frames - heard.len() / 2);
            i += 1;
            let mut b = vec![0f32; n * 2];
            proc.process(&mut b, 2);
            heard.extend_from_slice(&b);
        }
        assert!(worst(&heard, &vec![0.0; heard.len()]).1 > 0.05, "it plays");
        let (at, d) = worst(&heard, &whole[..frames * 2]);
        assert_eq!(
            d,
            0.0,
            "{} id, stopped {stopped} frames on the old song: {d:e} at frame {at} ({:.1} ms in)",
            if skip == 0 { "the same" } else { "another" },
            at as f64 * 1_000.0 / f64::from(SR)
        );
    }
}

/// One native instrument track at `spec`, a chord (drum notes among it) on its first beat
/// and another on its third; `skip` tracks made first and left out, so its id is another
/// song's.
fn native_song(name: &str, skip: usize, spec: NativeSpec) -> Project {
    let mut p = Project::new(name);
    p.tempo_bpm = 120.0;
    for _ in 0..skip {
        let _ = p.make_track("left out");
    }
    let mut t = p.make_track(name);
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("chords", Ticks::ZERO, Ticks::beats(4));
    for b in [0, 2] {
        for k in [36u8, 38, 42, 48, 60, 64, 67] {
            let n = p.make_note(Ticks::beats(b), Ticks(PPQ), k, 100);
            clip.notes.push(n);
        }
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

/// The engine the application drives (`EngineHandle`, its units made and handed over as
/// the backend does): `old` sent, `stopped` frames run stopped on it, then `new` sent, a
/// seek to the start and `Play` — `frames` frames of what it plays.
fn live_after(old: &Project, new: &Project, stopped: usize, frames: usize) -> Vec<f32> {
    let (mut h, mut proc_) = EngineHandle::manual(f64::from(SR));
    let s = h.compile(old, 1);
    h.set_snapshot(s);
    let mut buf = vec![0f32; 2 * 64];
    for _ in 0..stopped / 64 {
        proc_.process(&mut buf, 2);
        h.poll();
    }
    let s = h.compile(new, 2);
    h.set_snapshot(s);
    h.seek(Ticks::ZERO);
    h.play();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut out = Vec::with_capacity(frames * 2);
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut b = vec![0f32; n * 2];
        proc_.process(&mut b, 2);
        h.poll();
        out.extend_from_slice(&b);
    }
    out
}

/// What [`every_native_instrument_from_stop_after_another_song_is_the_render`] checks for
/// `kind`, over `cases` of (tracks skipped before the new one — 0: the old track's id —,
/// frames run stopped on the old song): the failures, described.
fn from_stop_failures(kind: InstrumentKind, cases: &[(usize, usize)]) -> Vec<String> {
    let frames = FRAMES_PER_BEAT * 3;
    let mut rng = Rng::new(0x51a9_e000 + kind as u64);
    let from = random_instrument(kind, 1.0, &mut rng);
    let to = random_instrument(kind, 1.0, &mut rng);
    let old = native_song("Old", 0, from);
    let mut failures = Vec::new();
    for &(skip, stopped) in cases {
        let new = native_song("New", skip, to.clone());
        assert_eq!(old.tracks[0].id == new.tracks[0].id, skip == 0);
        let whole = render_beats(&new, 0, 4);
        let heard = live_after(&old, &new, stopped, frames);
        let (at, d) = worst(&heard, &whole[..frames * 2]);
        if d != 0.0 {
            failures.push(format!(
                "{kind:?}, {} id, stopped {stopped}: {d:e} at frame {at} ({:.1} ms in)",
                if skip == 0 { "the same" } else { "another" },
                at as f64 * 1_000.0 / f64::from(SR)
            ));
        }
    }
    failures
}

/// fx/final — `playback_from_stop_after_another_song_is_the_render` for every native
/// instrument: a song replaced by another whose track plays the same kind at other settings
/// (drawn over every parameter's range, Gravis's Free phase among them) — with the old
/// track's id, the unit kept and configured again (it glided from the old settings into its
/// first note: Bass 2.9e-1, Electric piano 7.2e-2, Tympanum 2.1e-1, Chorda 1.5e-2, Organ
/// 2.9e-4 before the fix), or with another id, a unit made for it, nothing run between or
/// 700 frames stopped on the old song — played from stop, it is the offline render to the
/// bit. At `Play` every idle instrument snaps (`InstrumentUnit::snap_idle`). A unit kept and
/// heard stopped on the old song is the next test.
#[test]
fn every_native_instrument_from_stop_after_another_song_is_the_render() {
    let mut failures = Vec::new();
    for kind in InstrumentKind::ALL {
        failures.extend(from_stop_failures(kind, &[(0, 0), (3, 0), (3, 700)]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// fx/final — a unit kept across a song replaced (the same track id) and heard stopped on
/// the old song for over a second, silent, starts the new song as a render's new unit does,
/// to the bit: at `Play` a native unit with no voice whose output has been exact zero for
/// `QUIET_FOR_A_NEW_START_S` is reset, its free-running state where a new unit's starts.
/// Before: Analog 3.7e-1 at 1.38 s (drift, noise, LFOs), Electric piano 6.1e-4 at 9.5 ms
/// (its tremolo), Organ 1.5e-4 at 1.1 s (the rotors' and the scanner's phase), Chorda
/// 4.3e-6 at 30 ms. A unit silent for less is left as it is: a tail may still be on its way
/// through a line inside it.
#[test]
fn a_kept_unit_heard_stopped_starts_as_a_render_does() {
    let mut failures = Vec::new();
    for kind in InstrumentKind::ALL {
        failures.extend(from_stop_failures(kind, &[(0, 50_000)]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// fx/final — Gravis at the Free oscillator phase, played by a fresh engine from the start,
/// is the offline render to the bit. Before, an ended voice's oscillators ran on to the end
/// of its block and stood in idle ones, so the next note's phase was the block split's
/// (8.2e-1 at the second chord, 1.009 s); the DSP is held to it by
/// `gravis::tests::free_phase_renders_do_not_depend_on_the_block_size`.
#[test]
fn gravis_free_phase_live_is_the_render() {
    let mut rng = Rng::new(0x51a9_e000 + InstrumentKind::Bass as u64);
    let _ = random_instrument(InstrumentKind::Bass, 1.0, &mut rng);
    let mut to = random_instrument(InstrumentKind::Bass, 1.0, &mut rng);
    to.params.insert("phase".into(), 2.0);
    let new = native_song("New", 3, to);
    let frames = FRAMES_PER_BEAT * 3;
    let whole = render_beats(&new, 0, 4);
    let heard = live_after(&Project::new("empty"), &new, 0, frames);
    let (at, d) = worst(&heard, &whole[..frames * 2]);
    assert_eq!(d, 0.0, "{d:e} at frame {at}");
}

/// fx/final — the reset at `Play` never cuts a tail: every native instrument at its
/// defaults plays a chord, the song is stopped while the chord's tail still rings and, a
/// tenth of a second later, played from an empty bar; what it plays is what it played
/// staying stopped, to the bit (the tail goes on). Analog's, FM's, Bass's, Electric piano's
/// and Tympanum's tails ring there (the others are silent by then).
#[test]
fn play_from_stop_never_cuts_a_ringing_tail() {
    for kind in InstrumentKind::ALL {
        let mut p = native_song("Tail", 0, NativeSpec::new(kind));
        p.tracks[0].clips[0]
            .notes
            .retain(|n| n.start == Ticks::ZERO);
        let run = |play_again: bool| {
            let (mut h, mut proc_) = EngineHandle::manual(f64::from(SR));
            let s = h.compile(&p, 1);
            h.set_snapshot(s);
            h.seek(Ticks::ZERO);
            h.play();
            let mut out = Vec::new();
            let block = |proc_: &mut Processor, h: &mut EngineHandle, n: usize| {
                let mut b = vec![0f32; n * 2];
                proc_.process(&mut b, 2);
                h.poll();
                b
            };
            // The chord (half a second) and its release, then stopped.
            for _ in 0..(SR as usize * 6 / 10) / 256 {
                block(&mut proc_, &mut h, 256);
            }
            h.stop();
            for _ in 0..(SR as usize / 10) / 256 {
                block(&mut proc_, &mut h, 256);
            }
            h.seek(Ticks::beats(8));
            if play_again {
                h.play();
            }
            for _ in 0..(SR as usize / 2) / 256 {
                out.extend(block(&mut proc_, &mut h, 256));
            }
            out
        };
        let (played, stayed) = (run(true), run(false));
        let ringing = stayed[..512].iter().any(|v| *v != 0.0);
        if matches!(
            kind,
            InstrumentKind::Analog
                | InstrumentKind::Fm
                | InstrumentKind::Bass
                | InstrumentKind::ElectricPiano
                | InstrumentKind::AcousticDrums
        ) {
            assert!(ringing, "{kind:?}: its tail rings when it is played again");
        }
        let (at, d) = worst(&played, &stayed);
        assert_eq!(
            d, 0.0,
            "{kind:?}: played from stop over its ringing tail ({ringing}) differs by {d:e} at frame {at}"
        );
    }
}
