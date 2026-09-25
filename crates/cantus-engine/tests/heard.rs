//! What is heard, with latency (ADR 0022): the output lags the engine by the slowest
//! path's latency, so the position the engine reports as heard, a note recorded while
//! hearing a beat, the click, and a strip's take all allow for it.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, peak};
use cantus_engine::*;
use std::sync::Arc;
use support::*;

const DELAY: usize = 100;

/// A pure delay of `n` samples that says so.
#[derive(Debug)]
struct PureDelay {
    l: Vec<f32>,
    r: Vec<f32>,
    pos: usize,
}

impl PureDelay {
    fn new(n: usize) -> Self {
        PureDelay {
            l: vec![0.0; n],
            r: vec![0.0; n],
            pos: 0,
        }
    }
}

impl FxUnit for PureDelay {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, l: &mut [f32], r: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        for i in 0..l.len() {
            let (ol, or) = (self.l[self.pos], self.r[self.pos]);
            self.l[self.pos] = l[i];
            self.r[self.pos] = r[i];
            self.pos = (self.pos + 1) % self.l.len();
            l[i] = ol;
            r[i] = or;
        }
    }
    fn reset(&mut self) {
        self.l.iter_mut().for_each(|x| *x = 0.0);
        self.r.iter_mut().for_each(|x| *x = 0.0);
    }
    fn latency(&self) -> usize {
        self.l.len()
    }
}

/// A processor for a project whose `latent` slots are pure delays of [`DELAY`], with a
/// capture ring, settled the way an offline render is.
struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    cap: rtrb::Consumer<f32>,
    p: Processor,
    events: Vec<EngineEvent>,
    took: Vec<f32>,
}

impl Rig {
    fn new(project: &Project, latent: &[EffectId]) -> Self {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let (cap_tx, cap) = rtrb::RingBuffer::<f32>::new(CAPTURE_SAMPLES);
        let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
        p.attach_capture(cap_tx);
        let mut snap = Snapshot::compile(project, 1);
        snap.loop_region = None;
        for strip in &snap.strips {
            for slot in &strip.effects {
                let unit = if latent.contains(&slot.id) {
                    EffectUnit::from_fx(slot, Box::new(PureDelay::new(DELAY)))
                } else {
                    EffectUnit::new(slot, 48_000.0)
                };
                tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
            }
        }
        tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
        p.settle();
        Rig {
            tx,
            rx,
            cap,
            p,
            events: Vec::new(),
            took: Vec::new(),
        }
    }

    fn send(&mut self, m: EngineMessage) {
        self.tx.push(m).unwrap();
    }

    /// Play `frames` frames in irregular blocks; returns the output.
    fn play(&mut self, frames: usize) -> Vec<f32> {
        let sizes = [128usize, 333, 512, 96, 1024, 17];
        let mut out = Vec::new();
        let mut i = 0;
        while out.len() / 2 < frames {
            let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            self.p.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            while let Ok(ev) = self.rx.pop() {
                self.events.push(ev);
            }
            while let Ok(x) = self.cap.pop() {
                self.took.push(x);
            }
        }
        out
    }

    fn last_position(&self) -> Option<(Ticks, Ticks)> {
        self.events.iter().rev().find_map(|e| match e {
            EngineEvent::Position { tick, heard, .. } => Some((*tick, *heard)),
            _ => None,
        })
    }

    fn note_inputs(&self) -> Vec<NoteInput> {
        self.events
            .iter()
            .filter_map(|e| match e {
                EngineEvent::NoteInput(n) => Some(*n),
                _ => None,
            })
            .collect()
    }
}

/// One track playing into a strip with a latent insert (so the output lags by [`DELAY`]),
/// at 120 BPM so a beat is 24 000 samples and a tick 25.
fn latent_project() -> (Project, EffectId, StripId) {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    p.metronome = false;
    p.tracks.truncate(1);
    let s = p.tracks[0].strip.unwrap();
    let slot = p.make_effect(EffectKind::Limiter);
    let id = slot.id;
    p.strip_mut(s).unwrap().effects.push(slot);
    (p, id, s)
}

/// The project with its latent slot as a pure delay, or without the slot at all.
fn without_latency_unless(latent: bool, mut p: Project, id: EffectId) -> (Project, Vec<EffectId>) {
    if latent {
        return (p, vec![id]);
    }
    for s in &mut p.strips {
        s.effects.retain(|e| e.id != id);
    }
    (p, vec![])
}

/// With latency L, what the engine reports as heard is its position L samples ago — held
/// at the start until the start has come out — and a note played while hearing beat 2
/// is recorded on beat 2, not L later.
#[test]
fn a_note_recorded_while_hearing_a_beat_lands_on_it() {
    let beat = Ticks::beats(1);
    for (latent, lag) in [(true, DELAY), (false, 0)] {
        let (p, id, _) = latent_project();
        let (p, ids) = without_latency_unless(latent, p, id);
        let mut rig = Rig::new(&p, &ids);
        assert_eq!(rig.p.stats().latency_samples as usize, lag);
        let track = p.tracks[0].id;
        rig.send(EngineMessage::SetLiveTrack(Some(track)));
        rig.send(EngineMessage::SetRecording(true));
        rig.send(EngineMessage::Play);
        rig.play(40);
        if latent {
            let (tick, heard) = rig.last_position().unwrap();
            assert_eq!(tick, Ticks(1), "40 samples in, the engine is past tick 1");
            assert_eq!(heard, Ticks::ZERO, "but nothing of the start is heard yet");
        }
        // Play up to the moment beat 2 comes out of the speakers: the engine is `lag`
        // samples further on.
        rig.play(24_000 + lag - 40);
        let (tick, heard) = rig.last_position().unwrap();
        assert_eq!(heard, beat, "heard, with latency {lag}");
        assert_eq!(
            tick,
            Ticks(beat.0 + (lag / 25) as i64),
            "the engine is ahead by L"
        );
        rig.send(EngineMessage::NoteOn {
            track,
            pitch: 64,
            velocity: 100,
        });
        rig.play(64);
        let notes = rig.note_inputs();
        assert_eq!(notes.len(), 1);
        assert_eq!(
            notes[0].tick, beat,
            "with latency {lag}, the note heard on beat 2 was recorded at {:?}",
            notes[0].tick
        );
    }
}

fn onset(x: &[f32], from: usize) -> Option<usize> {
    (from..x.len() / 2).find(|i| x[2 * i] != 0.0 || x[2 * i + 1] != 0.0)
}

/// The click comes out with the music: with latency L, a click and a note on the same
/// beat both come out exactly L later than they do without it — together, as they were
/// written.
#[test]
fn the_click_and_a_note_on_the_same_beat_come_out_together() {
    let mut on_beat = Vec::new();
    for latent in [false, true] {
        let (mut p, id, _) = latent_project();
        let track = &mut p.tracks[0];
        track.clips.clear();
        let mut clip = p.make_clip("beat", Ticks::ZERO, Ticks::beats(4));
        clip.notes
            .push(p.make_note(Ticks::beats(1), Ticks::QUARTER, 60, 110));
        p.tracks[0].clips.push(clip);
        let (p, ids) = without_latency_unless(latent, p, id);
        // The note alone.
        let mut rig = Rig::new(&p, &ids);
        rig.send(EngineMessage::Play);
        let note = rig.play(30_000);
        // The click alone: the track muted, the metronome on.
        let mut clicks = p.clone();
        clicks.metronome = true;
        clicks.tracks[0].muted = true;
        let mut rig = Rig::new(&clicks, &ids);
        rig.send(EngineMessage::Play);
        let click = rig.play(30_000);
        let note_at = onset(&note, 0).expect("the note sounds");
        let click_at = onset(&click, 20_000).expect("the second beat clicks");
        assert_eq!(
            onset(&click, 0),
            Some(1 + if latent { DELAY } else { 0 }),
            "the first click (its first sample is sin 0 = 0)"
        );
        on_beat.push((note_at, click_at));
    }
    let (plain, late) = (on_beat[0], on_beat[1]);
    assert_eq!(late.0, plain.0 + DELAY, "the note moved by the latency");
    assert_eq!(
        late.1,
        plain.1 + DELAY,
        "the click moved by the latency too — it lines up with the note"
    );
    assert_eq!(plain.1, 24_001, "beat 2's click starts on its sample");
}

/// The mix recorder on a strip, in a project with latency elsewhere: the take is delayed
/// to the master's time, so it is sample for sample what that strip contributes to the
/// output — the same file an offline render of the strip alone gives.
#[test]
fn a_strip_take_is_aligned_with_the_offline_render() {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    p.metronome = false;
    let s0 = p.tracks[0].strip.unwrap();
    let s2 = p.tracks[1].strip.unwrap();
    let slot = p.make_effect(EffectKind::Limiter);
    let id = slot.id;
    p.strip_mut(s0).unwrap().effects.push(slot);
    // Live, with the take on the bass's strip.
    let mut rig = Rig::new(&p, &[id]);
    rig.send(EngineMessage::SetCapture(CaptureTarget::Strip(s2)));
    rig.send(EngineMessage::Play);
    let out = rig.play(96_000);
    assert!(peak(&out) > 0.05);
    let take = std::mem::take(&mut rig.took);
    // Offline, the bass alone (the lead muted, its latent strip still in place).
    let mut alone = p.clone();
    alone.tracks[0].muted = true;
    let mut rig = Rig::new(&alone, &[id]);
    rig.send(EngineMessage::Play);
    let render = rig.play(96_000);
    assert_eq!(rig.p.stats().latency_samples as usize, DELAY);
    assert!(peak(&render) > 0.05);
    assert_eq!(take.len(), render.len());
    assert_null(
        &take,
        &render,
        0.0,
        "the strip's take is not in time with what is heard",
    );
    // And the master's take is the output, latency and all.
    let mut rig = Rig::new(&p, &[id]);
    rig.send(EngineMessage::SetCapture(CaptureTarget::Master));
    rig.send(EngineMessage::Play);
    let out = rig.play(48_000);
    assert_null(&rig.took, &out, 0.0, "the master take is the output");
}
