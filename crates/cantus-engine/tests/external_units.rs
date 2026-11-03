//! External units in the engine (round 36, ADR 0094): the hardware's measured trip is the
//! track's own latency, so the rest of the mix waits for it — proved with a virtual
//! interface (the input ring a test fills, as a unit's return would).

#![allow(clippy::type_complexity)]

use cantus_core::external_unit::{Measured, MidiRoute, Pair};
use cantus_core::*;
use cantus_engine::*;

const SR: f32 = 48_000.0;
const BLOCK: usize = 128;

/// A (the unit's return, input 0–1) and B (a plain monitored input, 2–3), both straight to
/// the master. `latency` frames on A's unit, or no unit at all.
fn project(latency: Option<u32>) -> Project {
    let mut p = Project::new("ext");
    for (name, first) in [("A", 0u16), ("B", 2)] {
        let mut t = p.make_track(name);
        t.input = Some(TrackInput {
            first,
            stereo: true,
            monitor: MonitorMode::On,
            trim_db: 0.0,
        });
        t.strip = None;
        p.tracks.push(t);
    }
    if let Some(frames) = latency {
        let a = p.tracks[0].id;
        let id = p.new_external_unit_id();
        let mut u = ExternalUnit::instrument(
            id,
            a,
            MidiRoute {
                port: "Synth".into(),
                ordinal: 0,
                channel: 0,
            },
            Pair::stereo(0),
        );
        u.latency = Some(Measured::bare(frames, 48_000, 30.0));
        p.external_units.push(u);
    }
    p
}

/// Play 4 000 frames with a hit on B's input at frame 0 and on A's at `a_at`; the master's
/// left channel.
fn play(p: &Project, a_at: usize) -> Vec<f32> {
    let (mut h, mut proc) = EngineHandle::manual(f64::from(SR));
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize);
    proc.attach_input(consumer, 4);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    let total = 4_000;
    let mut out = Vec::new();
    let mut buf = vec![0f32; BLOCK * 2];
    let mut at = 0;
    while at < total {
        for f in at..at + BLOCK {
            let a = if f == a_at { 0.5 } else { 0.0 };
            let b = if f == 0 { 0.25 } else { 0.0 };
            for x in [a, a, b, b] {
                ring.push(x).unwrap();
            }
        }
        proc.process(&mut buf, 2);
        h.poll();
        out.extend(buf.iter().step_by(2));
        at += BLOCK;
    }
    out
}

fn hits(x: &[f32]) -> Vec<usize> {
    x.iter()
        .enumerate()
        .filter(|(_, v)| v.abs() > 0.1)
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn without_a_unit_the_two_returns_stay_where_they_arrive() {
    let x = play(&project(None), 1_000);
    let h = hits(&x);
    assert_eq!(h.len(), 2, "{h:?}");
    assert!(h[1] - h[0] >= 900, "{h:?}");
}

#[test]
fn a_units_measured_trip_makes_the_rest_of_the_mix_wait_for_it() {
    // The hardware's sound pops back 1 000 frames after it was asked for; the unit says so,
    // so B (popped at 0) waits 1 000 frames and the two meet.
    let x = play(&project(Some(1_000)), 1_000);
    let h = hits(&x);
    assert_eq!(h.len(), 1, "both land together: {h:?}");
}

#[test]
fn the_trim_moves_the_wait_and_the_off_switch_removes_it() {
    let mut p = project(Some(1_000));
    p.external_units[0].trim_ms = -10.0; // 480 frames less
    let x = play(&p, 1_000 - 480);
    assert_eq!(hits(&x).len(), 1, "{:?}", hits(&x));
    let mut p = project(Some(1_000));
    p.external_units[0].enabled = false;
    let x = play(&p, 1_000);
    assert_eq!(hits(&x).len(), 2, "off: as if the unit were not there");
}

#[test]
fn a_unit_never_measured_reports_no_latency() {
    let mut p = project(Some(1_000));
    p.external_units[0].latency = None;
    let x = play(&p, 1_000);
    assert_eq!(hits(&x).len(), 2);
}

// ---- notes out ---------------------------------------------------------------------------

mod notes_out {
    use super::*;
    use cantus_engine::transport::sync::{self, OutKind, Stamped, SyncHandle};
    use std::sync::Arc;

    fn rig(p: &Project) -> (Processor, SyncHandle) {
        let (mut tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let (port, handle) = sync::pair();
        proc.attach_sync(port, None);
        let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            p, 1,
        ))));
        let _ = tx.push(EngineMessage::Play);
        proc.settle();
        // Keep the senders alive for the test's life.
        std::mem::forget(tx);
        std::mem::forget(rx);
        (proc, handle)
    }

    fn song(with_unit: bool, channel: u8) -> Project {
        let mut p = Project::new("notes");
        p.tempo_bpm = 120.0;
        let mut t = p.make_track("Synth");
        t.strip = None;
        let mut c = p.make_clip("Line", Ticks::ZERO, Ticks::beats(8));
        for (beat, key) in [(0, 60u8), (2, 64)] {
            c.notes
                .push(p.make_note(Ticks::beats(beat), Ticks::beats(1), key, 100));
        }
        t.clips.push(c);
        let id = t.id;
        p.tracks.push(t);
        if with_unit {
            let uid = p.new_external_unit_id();
            p.external_units.push(ExternalUnit::instrument(
                uid,
                id,
                MidiRoute {
                    port: "Synth".into(),
                    ordinal: 0,
                    channel,
                },
                Pair::stereo(0),
            ));
        }
        p
    }

    fn stamped(h: &mut SyncHandle) -> Vec<Stamped> {
        let ring = h.out.as_mut().unwrap();
        let mut v = vec![];
        while let Ok(m) = ring.pop() {
            v.push(m);
        }
        v
    }

    fn run(proc: &mut Processor, frames: usize, block: usize) -> Vec<f32> {
        let mut out = vec![];
        let mut buf = vec![0f32; block * 2];
        let mut done = 0;
        while done < frames {
            proc.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            done += block;
        }
        out
    }

    #[test]
    fn a_clips_notes_leave_stamped_with_their_sample_route_and_channel() {
        let (mut proc, mut h) = rig(&song(true, 2));
        run(&mut proc, 96_000, 480);
        let got = stamped(&mut h);
        let notes: Vec<_> = got.iter().filter(|m| m.kind == OutKind::Note).collect();
        let p = song(true, 2);
        let u = &p.external_units[0];
        let r = cantus_core::external_unit::route_for(u.id, u.midi.as_ref().unwrap());
        let at = |m: &&Stamped| (m.at, m.msg().to_vec(), m.route);
        assert_eq!(
            notes.iter().map(at).collect::<Vec<_>>(),
            vec![
                (0, vec![0x92, 60, 100], r),
                (24_000, vec![0x82, 60, 0], r),
                (48_000, vec![0x92, 64, 100], r),
                (72_000, vec![0x82, 64, 0], r),
            ]
        );
    }

    #[test]
    fn the_notes_are_the_same_at_every_block_size() {
        let mut first: Option<Vec<(u64, Vec<u8>)>> = None;
        for block in [37usize, 128, 480, 1024] {
            let (mut proc, mut h) = rig(&song(true, 0));
            run(&mut proc, 96_000, block);
            let v: Vec<_> = stamped(&mut h)
                .iter()
                .filter(|m| m.kind == OutKind::Note)
                .map(|m| (m.at, m.msg().to_vec()))
                .collect();
            assert_eq!(v.len(), 4, "block {block}");
            match &first {
                None => first = Some(v),
                Some(f) => assert_eq!(&v, f, "block {block}"),
            }
        }
    }

    #[test]
    fn the_synth_in_the_computer_is_silent_while_the_outboard_one_is_asked() {
        let (mut with, _h) = rig(&song(true, 0));
        let (mut without, _h2) = rig(&song(false, 0));
        let a = run(&mut with, 48_000, 256);
        let b = run(&mut without, 48_000, 256);
        assert!(
            b.iter().any(|x| *x != 0.0),
            "the project sounds without a unit"
        );
        assert!(
            a.iter().all(|x| *x == 0.0),
            "with a unit its own instrument is not played"
        );
    }

    #[test]
    fn a_project_without_a_unit_stamps_nothing_and_is_the_same_bits() {
        let (mut proc, mut h) = rig(&song(false, 0));
        run(&mut proc, 48_000, 256);
        assert!(stamped(&mut h).iter().all(|m| m.kind != OutKind::Note));
        // A unit that is switched off is as if it were not there.
        let mut p = song(true, 0);
        p.external_units[0].enabled = false;
        let (mut off, _h) = rig(&p);
        let (mut plain, _h2) = rig(&song(false, 0));
        let a = run(&mut off, 48_000, 256);
        let b = run(&mut plain, 48_000, 256);
        assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
    }
}

// ---- the ping, through the processor -------------------------------------------------------

mod ping {
    use super::*;
    use cantus_engine::external_ping::{PingError, PingJob};
    use cantus_engine::transport::sync::{self, OutKind, SyncHandle};
    use std::sync::Arc;

    const CH: usize = 4;

    struct Rig {
        proc: Processor,
        tx: rtrb::Producer<EngineMessage>,
        rx: rtrb::Consumer<EngineEvent>,
        ring: rtrb::Producer<f32>,
        sync: SyncHandle,
    }

    fn rig(unit: bool) -> Rig {
        let (tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let (port, sync) = sync::pair();
        proc.attach_sync(port, None);
        let (ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 2);
        proc.attach_input(consumer, CH);
        let mut tx = tx;
        let mut p = Project::new("ping");
        let mut t = p.make_track("Synth");
        t.strip = None;
        let id = t.id;
        p.tracks.push(t);
        if unit {
            let uid = p.new_external_unit_id();
            p.external_units.push(ExternalUnit::instrument(
                uid,
                id,
                MidiRoute {
                    port: "Synth".into(),
                    ordinal: 0,
                    channel: 1,
                },
                Pair::stereo(2),
            ));
        }
        let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &p, 1,
        ))));
        proc.settle();
        Rig {
            proc,
            tx,
            rx,
            ring,
            sync,
        }
    }

    type World<'a> = dyn FnMut(usize, &[f32], &[(u64, Vec<u8>)]) -> Vec<f32> + 'a;

    /// Run until the job is back. `world` turns the block's output (4 channels) and the
    /// notes stamped so far into the next block's input (4 channels).
    fn run(r: &mut Rig, job: PingJob, block: usize, world: &mut World) -> PingJob {
        let _ = r.tx.push(EngineMessage::StartPing(Box::new(job)));
        let mut notes: Vec<(u64, Vec<u8>)> = vec![];
        let mut input = vec![0.0f32; block * CH];
        let mut out = vec![0.0f32; block * CH];
        let mut at = 0usize;
        for _ in 0..400_000 / block {
            for x in &input {
                r.ring.push(*x).unwrap();
            }
            r.proc.process(&mut out, CH);
            while let Ok(m) = r.sync.out.as_mut().unwrap().pop() {
                if m.kind == OutKind::Note {
                    notes.push((m.at, m.msg().to_vec()));
                }
            }
            while let Ok(ev) = r.rx.pop() {
                if let EngineEvent::PingDone(j) = ev {
                    return *j;
                }
            }
            input = world(at, &out, &notes);
            at += block;
        }
        panic!("the ping never came back");
    }

    /// The output's pair 2-3 (left) delayed by `delay` frames into the input's pair 2-3.
    fn wire(delay: usize) -> impl FnMut(usize, &[f32], &[(u64, Vec<u8>)]) -> Vec<f32> {
        let mut line: std::collections::VecDeque<f32> = vec![0.0; delay].into();
        move |_, out, _| {
            let mut back = vec![0.0f32; out.len()];
            for (i, f) in out.chunks(CH).enumerate() {
                let y = if delay == 0 {
                    f[2]
                } else {
                    line.push_back(f[2]);
                    line.pop_front().unwrap()
                };
                back[i * CH + 2] = y * 0.7;
                back[i * CH + 3] = y * 0.7;
            }
            back
        }
    }

    fn audio_job(unit: u64) -> PingJob {
        PingJob::audio(
            ExternalUnitId(unit),
            f64::from(SR),
            Pair::stereo(2),
            Pair::stereo(2),
        )
    }

    #[test]
    fn an_audio_ping_through_the_processor_measures_the_wire_and_the_block() {
        for block in [128usize, 480] {
            for delay in [1usize, 300, 2_500] {
                let mut r = rig(false);
                let done = run(&mut r, audio_job(7), block, &mut wire(delay));
                let res = done.evaluate().unwrap();
                assert_eq!(
                    res.frames as usize,
                    delay + block,
                    "block {block} delay {delay}"
                );
                assert_eq!(res.jitter, 0);
            }
        }
    }

    #[test]
    fn the_output_is_the_pings_alone_while_it_runs_and_nothing_leaks_to_the_master() {
        let mut r = rig(false);
        let _ = r.tx.push(EngineMessage::StartPing(Box::new(audio_job(7))));
        let mut out = vec![0.0f32; 256 * CH];
        let mut any_send = false;
        for _ in 0..40 {
            for _ in 0..256 * CH {
                r.ring.push(0.0).unwrap();
            }
            r.proc.process(&mut out, CH);
            for f in out.chunks(CH) {
                assert_eq!((f[0], f[1]), (0.0, 0.0), "the master pair stays silent");
                any_send |= f[2] != 0.0 || f[3] != 0.0;
            }
        }
        assert!(any_send, "the signal goes out on the send pair");
    }

    #[test]
    fn a_ping_while_another_runs_or_the_transport_plays_is_refused_and_comes_back() {
        let mut r = rig(false);
        let _ = r.tx.push(EngineMessage::StartPing(Box::new(audio_job(1))));
        let _ = r.tx.push(EngineMessage::StartPing(Box::new(audio_job(2))));
        let mut out = vec![0.0f32; 128 * CH];
        r.proc.process(&mut out, CH);
        let mut got = vec![];
        while let Ok(ev) = r.rx.pop() {
            if let EngineEvent::PingDone(j) = ev {
                got.push(j);
            }
        }
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].unit, ExternalUnitId(2));
        assert_eq!(got[0].evaluate().unwrap_err(), PingError::Busy);
        // The transport playing.
        let mut r = rig(false);
        let _ = r.tx.push(EngineMessage::Play);
        r.proc.process(&mut out, CH);
        let _ = r.tx.push(EngineMessage::StartPing(Box::new(audio_job(3))));
        r.proc.process(&mut out, CH);
        let mut busy = false;
        while let Ok(ev) = r.rx.pop() {
            if let EngineEvent::PingDone(j) = ev {
                busy = j.evaluate() == Err(PingError::Busy);
            }
        }
        assert!(busy);
    }

    #[test]
    fn an_onset_ping_through_the_processor_and_the_notes_measures_the_synths_trip() {
        // The synth answers a note-on 1 200 frames after its stamp with a tone on the return
        // pair, and stops at the note-off.
        let latency = 1_200u64;
        for block in [128usize, 512] {
            let mut r = rig(true);
            let job = PingJob::onset(ExternalUnitId(7), f64::from(SR), 7, 1, 60, Pair::stereo(2));
            let mut phase = 0.0f32;
            let mut world = move |at: usize, _out: &[f32], notes: &[(u64, Vec<u8>)]| {
                let mut back = vec![0.0f32; block * CH];
                for (i, f) in back.chunks_mut(CH).enumerate() {
                    let now = (at + i) as u64;
                    let last = notes.iter().rfind(|(t, _)| t + latency <= now);
                    if let Some((_, m)) = last {
                        if m[0] & 0xF0 == 0x90 {
                            phase += 0.05;
                            let v = 0.4 * cantus_dsp::math::sin(phase);
                            f[2] = v;
                            f[3] = v;
                        }
                    }
                }
                back
            };
            let done = run(&mut r, job, block, &mut world);
            let res = done.evaluate().unwrap();
            let got = i64::from(res.frames);
            assert!(
                (got - latency as i64).abs() <= 2 * block as i64,
                "block {block}: {got}"
            );
        }
    }

    #[test]
    fn a_silent_return_is_said_not_guessed_through_the_processor() {
        let mut r = rig(true);
        let job = PingJob::onset(ExternalUnitId(7), f64::from(SR), 1, 0, 60, Pair::stereo(2));
        let done = run(&mut r, job, 256, &mut |_, out, _| vec![0.0; out.len()]);
        assert_eq!(done.evaluate().unwrap_err(), PingError::NoNote);
        let mut r = rig(false);
        let done = run(&mut r, audio_job(7), 256, &mut |_, out, _| {
            vec![0.0; out.len()]
        });
        assert_eq!(done.evaluate().unwrap_err(), PingError::Silent);
    }
}

// ---- an external effect's send ----------------------------------------------------------------

mod send {
    use super::*;
    use cantus_core::audio::{MonitorMode, TrackInput};

    const CH: usize = 4;

    /// One track playing a short line, its return (the input pair 3-4) monitored; with a unit
    /// its send goes out of outputs 3-4.
    fn song(with_unit: bool) -> Project {
        let mut p = Project::new("send");
        p.tempo_bpm = 120.0;
        let mut t = p.make_track("Line");
        t.strip = None;
        let mut c = p.make_clip("Line", Ticks::ZERO, Ticks::beats(8));
        for (beat, key) in [(0, 60u8), (1, 67), (2, 64)] {
            c.notes
                .push(p.make_note(Ticks::beats(beat), Ticks::beats(1), key, 100));
        }
        t.clips.push(c);
        let id = t.id;
        if with_unit {
            t.input = Some(TrackInput {
                first: 2,
                stereo: true,
                monitor: MonitorMode::On,
                trim_db: 0.0,
            });
        }
        p.tracks.push(t);
        if with_unit {
            let uid = p.new_external_unit_id();
            p.external_units.push(ExternalUnit::effect(
                uid,
                id,
                Pair::stereo(2),
                Pair::stereo(2),
            ));
        }
        p
    }

    /// Four channels out; the send comes back `wire` frames later (or never).
    fn play(p: &Project, wire: Option<usize>, frames: usize, block: usize) -> Vec<[f32; CH]> {
        let (tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 2);
        proc.attach_input(consumer, CH);
        let mut tx = tx;
        let _ = tx.push(EngineMessage::SetSnapshot(std::sync::Arc::new(
            Snapshot::compile(p, 1),
        )));
        let _ = tx.push(EngineMessage::Play);
        proc.settle();
        let mut line: std::collections::VecDeque<[f32; 2]> =
            vec![[0.0; 2]; wire.unwrap_or(0)].into();
        let mut out = vec![0.0f32; block * CH];
        let mut all = Vec::new();
        let mut back = vec![[0.0f32; 2]; block];
        while all.len() < frames {
            for f in &back {
                for x in [0.0, 0.0, f[0], f[1]] {
                    ring.push(x).unwrap();
                }
            }
            proc.process(&mut out, CH);
            for (i, f) in out.chunks(CH).enumerate() {
                all.push([f[0], f[1], f[2], f[3]]);
                back[i] = match wire {
                    Some(0) => [f[2], f[3]],
                    Some(_) => {
                        line.push_back([f[2], f[3]]);
                        line.pop_front().unwrap()
                    }
                    None => [0.0, 0.0],
                };
            }
        }
        drop((tx, rx));
        all
    }

    #[test]
    fn the_source_goes_out_of_the_send_pair_and_is_not_heard_on_the_master() {
        let plain = play(&song(false), None, 48_000, 256);
        let with = play(&song(true), None, 48_000, 256);
        assert!(plain.iter().any(|f| f[0] != 0.0), "the line sounds");
        assert!(
            with.iter().all(|f| f[0] == 0.0 && f[1] == 0.0),
            "nothing of the source on the master while the return is silent"
        );
        // The send is the source itself, before the track's level: the same numbers the
        // plain render's master has, up to the track's own level and balance (unity here).
        let send_peak = with.iter().map(|f| f[2].abs()).fold(0.0, f32::max);
        let plain_peak = plain.iter().map(|f| f[0].abs()).fold(0.0, f32::max);
        assert!(send_peak > 0.0);
        let c = send_peak / plain_peak;
        for (a, b) in with.iter().zip(&plain) {
            assert!((a[2] - b[0] * c).abs() < 1e-6 && (a[3] - b[1] * c).abs() < 1e-6);
        }
    }

    #[test]
    fn what_comes_back_is_the_track_and_is_bit_exact_to_the_plain_render_shifted() {
        for block in [64usize, 256, 480] {
            for wire in [1usize, 300, 2_000] {
                let plain = play(&song(false), None, 30_000, block);
                let with = play(&song(true), Some(wire), 30_000, block);
                let shift = wire + block;
                // The send is the source before the track's level; what returns goes
                // through the same level and balance as the plain render's source, so the
                // master is the plain master again after the trip.
                for i in shift..with.len() {
                    for c in 0..2 {
                        let (got, want) = (with[i][c], plain[i - shift][c]);
                        assert!(
                            (got - want).abs() < 1e-6,
                            "block {block} wire {wire} frame {i}: {got} vs {want}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_send_on_the_masters_outputs_is_not_made() {
        // The unit would play on outputs 1-2: no send is made, the track plays as it does
        // without a unit (and the screen says why, `Availability::SendOnMaster`).
        let mut p = song(true);
        p.external_units[0].send = Some(Pair::stereo(0));
        let out = play(&p, None, 24_000, 256);
        let plain = play(&song(false), None, 24_000, 256);
        assert!(out.iter().all(|f| f[2] == 0.0 && f[3] == 0.0));
        assert!(out
            .iter()
            .zip(&plain)
            .all(|(a, b)| a[0].to_bits() == b[0].to_bits()));
    }

    #[test]
    fn a_project_without_a_unit_is_the_same_bits_with_the_send_code_in() {
        let a = play(&song(false), None, 24_000, 256);
        let mut p = song(false);
        // A unit on a track that is not there changes nothing.
        let uid = p.new_external_unit_id();
        p.external_units.push(ExternalUnit {
            track: TrackId(u64::MAX),
            ..ExternalUnit::effect(uid, TrackId(1), Pair::stereo(2), Pair::stereo(2))
        });
        let b = play(&p, None, 24_000, 256);
        assert!(a
            .iter()
            .zip(&b)
            .all(|(x, y)| x.iter().zip(y).all(|(u, v)| u.to_bits() == v.to_bits())));
    }
}

// ---- no stuck notes ---------------------------------------------------------------------------

mod stuck {
    use super::*;
    use cantus_core::external_unit::route_for;
    use cantus_engine::transport::sync::{self, OutKind, SyncHandle};
    use std::sync::Arc;

    struct Rig {
        proc: Processor,
        tx: rtrb::Producer<EngineMessage>,
        _rx: rtrb::Consumer<EngineEvent>,
        sync: SyncHandle,
    }

    fn rig(p: &Project) -> Rig {
        let (mut tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let (port, sync) = sync::pair();
        proc.attach_sync(port, None);
        let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            p, 1,
        ))));
        let _ = tx.push(EngineMessage::Play);
        proc.settle();
        Rig {
            proc,
            tx,
            _rx: rx,
            sync,
        }
    }

    impl Rig {
        fn run(&mut self, frames: usize) {
            let mut buf = vec![0f32; 256 * 2];
            for _ in 0..frames / 256 {
                self.proc.process(&mut buf, 2);
            }
        }
        fn snapshot(&mut self, p: &Project) {
            let _ = self
                .tx
                .push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
                    p, 2,
                ))));
        }
        fn notes(&mut self) -> Vec<(u64, Vec<u8>)> {
            let ring = self.sync.out.as_mut().unwrap();
            let mut v = vec![];
            while let Ok(m) = ring.pop() {
                if m.kind == OutKind::Note {
                    v.push((m.route, m.msg().to_vec()));
                }
            }
            v
        }
    }

    /// A synth track holding middle C for a whole bar (from 0), its unit on channel 3.
    fn song() -> Project {
        let mut p = Project::new("stuck");
        p.tempo_bpm = 120.0;
        let mut t = p.make_track("Synth");
        t.strip = None;
        let mut c = p.make_clip("Held", Ticks::ZERO, Ticks::beats(8));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(4), 60, 100));
        t.clips.push(c);
        let id = t.id;
        p.tracks.push(t);
        let uid = p.new_external_unit_id();
        p.external_units.push(ExternalUnit::instrument(
            uid,
            id,
            MidiRoute {
                port: "Synth A".into(),
                ordinal: 0,
                channel: 2,
            },
            Pair::stereo(0),
        ));
        p
    }

    fn route(p: &Project) -> u64 {
        let u = &p.external_units[0];
        route_for(u.id, u.midi.as_ref().unwrap())
    }

    /// Change the song mid-note with `edit`; what goes out for the held key.
    fn mid_note(edit: impl Fn(&mut Project)) -> (u64, Vec<(u64, Vec<u8>)>) {
        let p = song();
        let old = route(&p);
        let mut r = rig(&p);
        r.run(12_000);
        assert!(
            r.notes().contains(&(old, vec![0x92, 60, 100])),
            "the note went out"
        );
        let mut q = p.clone();
        edit(&mut q);
        r.snapshot(&q);
        r.run(512);
        (old, r.notes())
    }

    fn released(old: u64, got: &[(u64, Vec<u8>)]) {
        assert!(
            got.contains(&(old, vec![0x82, 60, 0])),
            "its note-off on the old route and channel: {got:?}"
        );
        assert!(
            got.contains(&(old, vec![0xB2, 123, 0])),
            "and an all-notes-off there: {got:?}"
        );
    }

    #[test]
    fn removing_a_unit_mid_note_releases_what_it_held() {
        let (old, got) = mid_note(|q| q.external_units.clear());
        released(old, &got);
    }

    #[test]
    fn switching_a_unit_off_mid_note_releases_what_it_held() {
        let (old, got) = mid_note(|q| q.external_units[0].enabled = false);
        released(old, &got);
    }

    #[test]
    fn another_channel_mid_note_releases_on_the_old_channel() {
        let (old, got) = mid_note(|q| q.external_units[0].midi.as_mut().unwrap().channel = 5);
        released(old, &got);
        assert!(!got.iter().any(|(_, m)| m == &vec![0x85, 60, 0]));
    }

    #[test]
    fn another_port_mid_note_releases_on_the_old_route() {
        let (old, got) =
            mid_note(|q| q.external_units[0].midi.as_mut().unwrap().port = "Synth B".into());
        released(old, &got);
        let new = route(&{
            let mut q = song();
            q.external_units[0].midi.as_mut().unwrap().port = "Synth B".into();
            q
        });
        assert_ne!(old, new, "another port is another route");
    }

    #[test]
    fn a_full_ring_never_leaves_a_note_on() {
        // Many short notes; the ring is not drained for a while (a stalled MIDI-out thread).
        let mut p = song();
        let t = &mut p.tracks[0];
        let clip = &mut t.clips[0];
        clip.notes.clear();
        let mut notes = vec![];
        for k in 0..900i64 {
            notes.push((Ticks(k * 2), Ticks(1), 40 + (k % 40) as u8));
        }
        for (at, len, key) in notes {
            let n = p.make_note(at, len, key, 90);
            p.tracks[0].clips[0].notes.push(n);
        }
        let mut r = rig(&p);
        r.run(48_000);
        let mut got = r.notes();
        // Drained now: the offs that waited go out in the next blocks.
        for _ in 0..20 {
            r.run(2_048);
            got.extend(r.notes());
        }
        let mut held = [0i32; 128];
        for (_, m) in &got {
            match m[0] & 0xF0 {
                0x90 => held[usize::from(m[1])] += 1,
                0x80 => held[usize::from(m[1])] -= 1,
                _ => {}
            }
        }
        assert!(
            held.iter().all(|h| *h == 0),
            "every note-on that went out has its note-off"
        );
        // The ring filled to its reserve (1 024 places, 256 kept for note-offs): note-ons
        // that found no room were not sent, so they hold nothing.
        assert!(
            got.len() >= 1_024 - 256,
            "the ring reached its reserve: {}",
            got.len()
        );
    }
}

// ---- review: bounds, availability, re-based trips, an aborted ping ----------------------------

mod review {
    use super::*;
    use cantus_core::external_unit::Now;
    use cantus_engine::external_ping::{PingError, PingJob};
    use cantus_engine::external_presence::{apply, ExternalPresence};
    use cantus_engine::transport::sync::{self, OutKind};
    use std::sync::Arc;

    fn effect_song() -> Project {
        let mut p = Project::new("fx");
        p.tempo_bpm = 120.0;
        let mut t = p.make_track("Line");
        t.strip = None;
        let mut c = p.make_clip("Line", Ticks::ZERO, Ticks::beats(8));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(2), 60, 100));
        t.clips.push(c);
        let id = t.id;
        p.tracks.push(t);
        let uid = p.new_external_unit_id();
        p.external_units.push(ExternalUnit::effect(
            uid,
            id,
            Pair::stereo(2),
            Pair::stereo(2),
        ));
        p
    }

    fn processor(p: &Project) -> Processor {
        let (mut tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            p, 1,
        ))));
        let _ = tx.push(EngineMessage::Play);
        proc.settle();
        std::mem::forget((tx, rx));
        proc
    }

    #[test]
    fn a_send_past_the_devices_channels_never_lands_on_another_channel() {
        // Two channels (an offline render's, a stereo device): the send pair 3-4 does not exist.
        let mut proc = processor(&effect_song());
        let mut buf = vec![0f32; 256 * 2];
        let mut any = 0usize;
        for _ in 0..100 {
            proc.process(&mut buf, 2);
            any += buf.iter().filter(|x| **x != 0.0).count();
        }
        assert_eq!(
            any, 0,
            "nothing of the send on L/R (it used to land on the next frame)"
        );
    }

    #[test]
    fn an_audio_ping_past_the_devices_channels_writes_nothing() {
        let (mut tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, _rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize);
        proc.attach_input(consumer, 2);
        let _ = tx.push(EngineMessage::StartPing(Box::new(PingJob::audio(
            ExternalUnitId(1),
            f64::from(SR),
            Pair::stereo(2),
            Pair::stereo(0),
        ))));
        let mut buf = vec![0f32; 256 * 2];
        for _ in 0..40 {
            for _ in 0..512 {
                let _ = ring.push(0.0);
            }
            proc.process(&mut buf, 2);
            assert!(buf.iter().all(|x| *x == 0.0));
        }
    }

    #[test]
    fn a_unit_that_is_not_available_gives_no_link_and_its_track_plays_as_without_it() {
        let p = effect_song();
        let mut snap = Snapshot::compile(&p, 1);
        assert_eq!(snap.tracks[0].external.len(), 1);
        apply(&mut snap, &ExternalPresence::default());
        assert!(snap.tracks[0].external.is_empty(), "not ready: no link");
        // Rendered: the plain track's sound, not a muted source.
        let mut q = p.clone();
        q.external_units.clear();
        let render = |snap: Snapshot| {
            let (mut tx, rxe) = rtrb::RingBuffer::new(64);
            let (txe, rx) = rtrb::RingBuffer::new(8192);
            let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
            let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(snap)));
            let _ = tx.push(EngineMessage::Play);
            proc.settle();
            let mut out = vec![];
            let mut buf = vec![0f32; 256 * 2];
            for _ in 0..80 {
                proc.process(&mut buf, 2);
                out.extend_from_slice(&buf);
            }
            drop((tx, rx));
            out
        };
        let gated = render(snap);
        let plain = render(Snapshot::compile(&q, 1));
        assert!(plain.iter().any(|x| *x != 0.0));
        assert!(gated
            .iter()
            .zip(&plain)
            .all(|(a, b)| a.to_bits() == b.to_bits()));
    }

    #[test]
    fn a_ready_units_trip_is_rebased_by_the_interfaces_round_trip_now() {
        let mut p = effect_song();
        p.external_units[0].latency = Some(Measured {
            rtt: 600,
            buffer: 256,
            device: "Studio Interface".into(),
            ..Measured::bare(1_000, 48_000, 30.0)
        });
        let id = p.external_units[0].id;
        let mut snap = Snapshot::compile(&p, 1);
        apply(
            &mut snap,
            &ExternalPresence {
                ready: vec![id],
                now: Some(Now {
                    device: "Studio Interface".into(),
                    rtt: 1_112,
                    buffer: 512,
                    rate: 48_000,
                }),
            },
        );
        assert_eq!(snap.tracks[0].external[0].latency_at(48_000.0), 1_512);
    }

    #[test]
    fn the_transport_starting_mid_ping_aborts_it_and_releases_its_note() {
        let (mut tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, mut rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let (port, mut sync) = sync::pair();
        proc.attach_sync(port, None);
        let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize);
        proc.attach_input(consumer, 2);
        let _ = tx.push(EngineMessage::StartPing(Box::new(PingJob::onset(
            ExternalUnitId(1),
            f64::from(SR),
            77,
            0,
            60,
            Pair::stereo(0),
        ))));
        let mut buf = vec![0f32; 256 * 2];
        // Past the first note-on (0.15 s), then Play.
        for _ in 0..40 {
            for _ in 0..512 {
                let _ = ring.push(0.0);
            }
            proc.process(&mut buf, 2);
        }
        let _ = tx.push(EngineMessage::Play);
        let mut done = None;
        for _ in 0..4 {
            for _ in 0..512 {
                let _ = ring.push(0.0);
            }
            proc.process(&mut buf, 2);
            while let Ok(ev) = rx.pop() {
                if let EngineEvent::PingDone(j) = ev {
                    done = Some(j);
                }
            }
        }
        let job = done.expect("the ping came back");
        assert_eq!(job.evaluate().unwrap_err(), PingError::Aborted);
        let mut notes = vec![];
        while let Ok(m) = sync.out.as_mut().unwrap().pop() {
            if m.kind == OutKind::Note {
                notes.push((m.route, m.msg().to_vec()));
            }
        }
        assert!(notes.contains(&(77, vec![0x90, 60, 100])));
        assert_eq!(notes.last(), Some(&(77, vec![0x80, 60, 0])), "{notes:?}");
    }
}

// ---- offline renders play no gear: the choke point ------------------------------------------

mod offline {
    use super::*;
    use cantus_engine::external_presence::ExternalPresence;
    use cantus_engine::render::{render, RenderOptions};
    use std::sync::Arc;

    /// A line on one track, the track holding an instrument unit (with a measured trip) and an
    /// effect unit (its send on 3-4).
    fn song(units: bool) -> Project {
        let mut p = Project::new("offline");
        p.tempo_bpm = 120.0;
        let mut t = p.make_track("Line");
        t.strip = None;
        let mut c = p.make_clip("Line", Ticks::ZERO, Ticks::beats(4));
        for (beat, key) in [(0, 60u8), (1, 67), (2, 64)] {
            c.notes
                .push(p.make_note(Ticks::beats(beat), Ticks::beats(1), key, 100));
        }
        t.clips.push(c);
        let id = t.id;
        p.tracks.push(t);
        if units {
            let a = p.new_external_unit_id();
            let mut inst = ExternalUnit::instrument(
                a,
                id,
                MidiRoute {
                    port: "Synth".into(),
                    ordinal: 0,
                    channel: 0,
                },
                Pair::stereo(0),
            );
            inst.latency = Some(Measured::bare(2_000, 48_000, 30.0));
            p.external_units.push(inst);
            let b = p.new_external_unit_id();
            p.external_units.push(ExternalUnit::effect(
                b,
                id,
                Pair::stereo(2),
                Pair::stereo(2),
            ));
        }
        p
    }

    fn opts() -> RenderOptions {
        RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        }
    }

    #[test]
    fn an_offline_render_with_the_links_still_in_its_snapshot_plays_no_gear() {
        // The snapshot is compiled straight from the project, links and all — as a future
        // offline path that forgets the handle's stripping would: the processor offline
        // plays the track's own instrument, sends nothing, reports no gear latency.
        let with = render(Arc::new(Snapshot::compile(&song(true), 1)), &opts()).unwrap();
        let without = render(Arc::new(Snapshot::compile(&song(false), 1)), &opts()).unwrap();
        assert!(without.samples.iter().any(|x| *x != 0.0));
        assert_eq!(with.samples.len(), without.samples.len(), "no gear latency");
        assert!(with
            .samples
            .iter()
            .zip(&without.samples)
            .all(|(a, b)| a.to_bits() == b.to_bits()));
    }

    #[test]
    fn going_offline_after_the_snapshot_is_installed_drops_the_gear_too() {
        let (mut tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, rx) = rtrb::RingBuffer::new(8192);
        let mut proc = Processor::without_midi(f64::from(SR), rxe, txe);
        let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &song(true),
            1,
        ))));
        let _ = tx.push(EngineMessage::Play);
        proc.settle();
        proc.set_offline(true);
        let mut buf = vec![0f32; 256 * 2];
        let mut any = false;
        for _ in 0..40 {
            proc.process(&mut buf, 2);
            any |= buf.iter().any(|x| *x != 0.0);
        }
        assert!(any, "the track's own instrument plays offline");
        drop((tx, rx));
    }

    #[test]
    fn the_handle_strips_every_link_from_a_render_compile_and_keeps_them_live() {
        let p = song(true);
        let (mut h, _proc) = EngineHandle::manual(f64::from(SR));
        h.set_external_presence(ExternalPresence {
            ready: p.external_units.iter().map(|u| u.id).collect(),
            now: None,
        });
        assert_eq!(
            h.compile(&p, 1).tracks[0].external.len(),
            2,
            "live: ready units link"
        );
        assert!(
            h.compile_for_render(&p, 2).tracks[0].external.is_empty(),
            "a render compile: none, even for ready units"
        );
    }
}
