//! The multicore pool (ADR 0031 §2, round 22): the engine's graph run by W live workers and
//! the callback gives the serial engine's output **to the bit** — on random topologies
//! (buses, sends, keys from earlier and later strips, notes into effects, latent inserts,
//! automation riding faders, loops), at W ∈ {1, 3, 7}, under random block sizes, shuffled
//! priorities, clustering on and off and, with the `fuzz` feature, the PCT-shaped stalls of
//! `sched::fuzz` inside the engine's own pool. No borrow ever fails
//! (`schedule_conflicts == 0`), and the pool wakes a worker only at an idle → busy
//! transition.
//!
//! A failure prints its seed; `CANTUS_POOL_SEEDS=n` runs more.

mod support;
use cantus_core::automation::{AutoPoint, AutoTarget};
use cantus_core::*;
use cantus_engine::sched::Mode;
use cantus_engine::*;
use std::sync::Arc;
use support::heavy::{random_effect, random_instrument, Rng};
use support::*;

const SR: f64 = 48_000.0;

/// A processor playing a project as the application drives it: units, the automation
/// runtime and the plan sent before the snapshot, compensation and memory answered.
struct Rig {
    p: Processor,
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
}

impl Rig {
    fn new(project: &Project, workers: Workers) -> Rig {
        let snap = Arc::new(Snapshot::compile(project, 1));
        let effects = snap.effect_slots().count();
        let instruments = cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false);
        let (mut tx, rx_engine) =
            rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
        p.set_workers(workers);
        for slot in snap.effect_slots() {
            tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
                slot, SR as f32,
            ))))
            .ok();
        }
        for unit in instruments {
            tx.push(EngineMessage::AddInstrument(unit)).ok();
        }
        tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .ok();
        let plan = cantus_engine::graph::compile(&snap);
        tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
        p.settle();
        while let Ok(ev) = rx.pop() {
            match ev {
                EngineEvent::PdcNeed { count, delay } => {
                    tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                        count, delay,
                    )))
                    .ok();
                    p.settle();
                }
                EngineEvent::EffectNeed { effect, need } => {
                    let memory = EffectMemory::build(need, SR as f32);
                    tx.push(EngineMessage::EffectMemory { effect, memory }).ok();
                    p.settle();
                }
                _ => {}
            }
        }
        tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        tx.push(EngineMessage::Play).ok();
        Rig { p, tx, rx }
    }

    /// Blocks of the given sizes, back to back; the output, interleaved stereo.
    fn play(&mut self, blocks: &[usize]) -> Vec<f32> {
        let mut out = Vec::new();
        let mut buf = vec![0f32; MAX_BLOCK_FRAMES * 2];
        for &n in blocks {
            let b = &mut buf[..n * 2];
            self.p.process(b, 2);
            out.extend_from_slice(b);
            while self.rx.pop().is_ok() {}
        }
        out
    }
}

/// Random block sizes (1…1024, mostly the live ones), the same for every run of a seed.
fn blocks(rng: &mut Rng, total: usize) -> Vec<usize> {
    let mut v = Vec::new();
    let mut done = 0;
    while done < total {
        let n = match rng.range(0, 9) {
            0 => rng.range(1, 64),
            1 => rng.range(65, MAX_BLOCK_FRAMES),
            2..=4 => 128,
            5 | 6 => 256,
            _ => 64,
        };
        v.push(n);
        done += n;
    }
    v
}

const KINDS: [EffectKind; 11] = [
    EffectKind::Delay,
    EffectKind::Reverb,
    EffectKind::Eq,
    EffectKind::Compressor,
    EffectKind::Limiter,
    EffectKind::Space,
    EffectKind::Multiband,
    EffectKind::DeEsser,
    EffectKind::Gate,
    EffectKind::Saturator,
    EffectKind::Stereo,
];

const INSTRUMENTS: [InstrumentKind; 6] = [
    InstrumentKind::Wavetable,
    InstrumentKind::Analog,
    InstrumentKind::Fm,
    InstrumentKind::Bass,
    InstrumentKind::ElectricPiano,
    InstrumentKind::Organ,
];

fn pt(beats: f64, value: f32) -> AutoPoint {
    AutoPoint::new(Ticks((beats * PPQ as f64) as i64), value)
}

/// A random session: tracks on their own strips, buses and returns at higher levels
/// (routes only go up, so nothing loops), sends, keys from any strip (earlier or later in
/// the order), inserts that hear a track's notes, latent inserts, mutes, polarity,
/// automation and a loop.
fn session(seed: u64) -> Project {
    let mut rng = Rng::new(seed);
    let mut p = Project::new("pool");
    p.tempo_bpm = 90.0 + rng.range(0, 80) as f64;
    let tracks = rng.range(1, 12);
    for t in 0..tracks {
        let mut tr = p.make_track(format!("T{t}"));
        if rng.chance(0.6) {
            let kind = INSTRUMENTS[rng.range(0, INSTRUMENTS.len() - 1)];
            tr.instrument = InstrumentSpec::Native(random_instrument(kind, 0.2, &mut rng));
        }
        let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(8));
        for _ in 0..rng.range(2, 24) {
            let start = Ticks(rng.range(0, 7 * PPQ as usize) as i64);
            let len = Ticks(rng.range(PPQ as usize / 8, 2 * PPQ as usize) as i64);
            let n = p.make_note(
                start,
                len,
                rng.range(36, 84) as u8,
                rng.range(30, 127) as u8,
            );
            clip.notes.push(n);
        }
        tr.clips.push(clip);
        tr.gain_db = -(rng.range(0, 12) as f32);
        tr.pan = rng.unit() * 2.0 - 1.0;
        if rng.chance(0.1) {
            tr.muted = true;
        }
        p.tracks.push(tr);
    }
    // Buses (level 1) and returns (level 2).
    let mut levels: Vec<Vec<StripId>> = vec![Vec::new(), Vec::new(), Vec::new()];
    for t in 0..tracks {
        if let Some(s) = p.tracks[t].strip {
            levels[0].push(s);
        }
    }
    for (level, ids) in levels.iter_mut().enumerate().skip(1) {
        for b in 0..rng.range(0, 3) {
            let s = p.make_strip(format!("L{level}B{b}"));
            ids.push(s.id);
            p.strips.push(s);
        }
    }
    let all: Vec<(usize, StripId)> = levels
        .iter()
        .enumerate()
        .flat_map(|(l, v)| v.iter().map(move |s| (l, *s)))
        .collect();
    let track_ids: Vec<TrackId> = p.tracks.iter().map(|t| t.id).collect();
    for &(level, id) in &all {
        let higher: Vec<StripId> = all
            .iter()
            .filter(|(l, _)| *l > level)
            .map(|(_, s)| *s)
            .collect();
        let others: Vec<StripId> = all.iter().map(|(_, s)| *s).filter(|s| *s != id).collect();
        let mut effects = Vec::new();
        for _ in 0..rng.range(0, 3) {
            let kind = KINDS[rng.range(0, KINDS.len() - 1)];
            let mut slot = p.make_effect(kind);
            slot.spec = random_effect(kind, 0.3, &mut rng);
            if rng.chance(0.15) {
                slot.mix = rng.unit();
            }
            if rng.chance(0.1) {
                slot.bypass = true;
            }
            // A key from a strip below (heard) — the order puts it first.
            let lower: Vec<StripId> = all
                .iter()
                .filter(|(l, _)| *l < level)
                .map(|(_, s)| *s)
                .collect();
            if !lower.is_empty() && rng.chance(0.25) {
                slot.sidechain = Some(lower[rng.range(0, lower.len() - 1)]);
            }
            if rng.chance(0.2) {
                slot.midi_from = Some(track_ids[rng.range(0, track_ids.len() - 1)]);
            }
            effects.push(slot);
        }
        let s = p.strip_mut(id).unwrap();
        s.effects = effects;
        s.gain_db = -(rng.range(0, 9) as f32);
        s.pan = rng.unit() * 2.0 - 1.0;
        s.phase_invert = rng.chance(0.1);
        s.muted = rng.chance(0.05);
        s.swap_channels = rng.chance(0.05);
        if !higher.is_empty() && rng.chance(0.6) {
            s.output = Some(higher[rng.range(0, higher.len() - 1)]);
        }
        for _ in 0..rng.range(0, 2) {
            if higher.is_empty() {
                break;
            }
            let to = higher[rng.range(0, higher.len() - 1)];
            if !s.sends.iter().any(|x| x.to == to) && s.output != Some(to) {
                // Both taps (round 25): a pre-fader send reads the pair before the
                // fader, or a copy of it when automation rides the fader.
                let tap = if rng.chance(0.4) {
                    SendTap::Pre
                } else {
                    SendTap::Post
                };
                s.sends.push(Send {
                    to,
                    level: rng.unit(),
                    enabled: true,
                    tap,
                });
            }
        }
        let _ = others;
    }
    if rng.chance(0.5) {
        let lim = p.make_effect(EffectKind::Limiter);
        p.master.effects.push(lim);
    }
    if rng.chance(0.5) {
        let strip = all[rng.range(0, all.len() - 1)].1;
        let c = p.make_automation_clip(
            "a",
            AutoTarget::StripGain(strip),
            Ticks::beats(1),
            Ticks::beats(4),
            vec![pt(0.0, 0.2), pt(4.0, 0.9)],
        );
        p.tracks[0].clips.push(c);
    }
    if rng.chance(0.4) {
        p.loop_region = Some(LoopRegion {
            start: Ticks::beats(rng.range(0, 3) as i64),
            end: Ticks::beats(rng.range(4, 8) as i64),
        });
        p.loop_enabled = true;
    }
    p
}

fn seeds() -> u64 {
    std::env::var("CANTUS_POOL_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12)
}

/// The serial engine's output for a project and its block sizes.
fn serial(p: &Project, blocks: &[usize]) -> Vec<f32> {
    let mut r = Rig::new(p, Workers::SERIAL);
    let out = r.play(blocks);
    assert_eq!(r.p.stats().schedule_conflicts, 0);
    out
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i} differs: {} vs {}", a[i], b[i]);
    }
}

#[test]
fn the_pool_gives_the_serial_engines_bits_on_random_sessions() {
    for seed in 0..seeds() {
        let mut rng = Rng::new(seed);
        let b = blocks(&mut rng, 4 * 48_000);
        // A random session can be silent by construction (its only track's strip muted):
        // take the seed's next variant, so every seed compares something audible.
        let (p, reference) = (0..8u64)
            .map(|k| {
                let p = session(0x9001 + seed + 1_000 * k);
                let reference = serial(&p, &b);
                (p, reference)
            })
            .find(|(_, r)| r.iter().any(|x| *x != 0.0))
            .unwrap_or_else(|| panic!("seed {seed}: every variant is silent"));
        for w in [1usize, 3, 7] {
            let mut workers = Workers::parallel(w);
            workers.shuffle = Some(seed * 31 + w as u64);
            let mut r = Rig::new(&p, workers);
            let out = r.play(&b);
            same_bits(&reference, &out, &format!("seed {seed}, W = {w}"));
            let s = *r.p.stats();
            assert_eq!(s.schedule_conflicts, 0, "seed {seed}, W = {w}: conflicts");
        }
    }
}

#[test]
fn the_demo_and_a_busy_desk_are_the_same_at_every_w_and_mode() {
    let mut busy = demo_project();
    add_effect(&mut busy, EffectKind::Compressor);
    add_effect(&mut busy, EffectKind::Space);
    let lim = busy.make_effect(EffectKind::Limiter);
    busy.master.effects.push(lim);
    for (name, p) in [("demo", demo_project()), ("busy", busy)] {
        let b = vec![128; 3 * 375];
        let reference = serial(&p, &b);
        for (w, mode) in [(1, Mode::Parallel), (3, Mode::Auto), (7, Mode::Parallel)] {
            let workers = Workers {
                mode,
                ..Workers::parallel(w)
            };
            let mut r = Rig::new(&p, workers);
            let out = r.play(&b);
            same_bits(&reference, &out, &format!("{name}, W = {w}, {mode:?}"));
            assert_eq!(r.p.stats().schedule_conflicts, 0);
        }
    }
}

/// With the `fuzz` feature: the PCT-shaped change points stall whichever thread of the
/// engine's own pool reaches the scheduler's k-th site — mid-push, mid-steal, with a task in
/// hand — and the output is still the serial engine's.
#[cfg(feature = "fuzz")]
#[test]
fn the_schedule_fuzz_over_the_corpus_changes_no_sample() {
    use cantus_engine::sched::fuzz::{ChangePoint, Pct};
    use std::time::Duration;
    for seed in 0..seeds() {
        let p = session(0x5EED + seed);
        let mut rng = Rng::new(seed ^ 0xF00);
        let b = blocks(&mut rng, 2 * 48_000);
        let reference = serial(&p, &b);
        for w in [1usize, 3, 7] {
            let changes = (0..3)
                .map(|k| ChangePoint {
                    block: 1 + rng.range(0, b.len().min(400)) as u64,
                    step: rng.range(0, 40) as u64,
                    stall: Duration::from_micros(50 + 200 * k),
                })
                .collect();
            let workers = Workers {
                chaos: Some(Arc::new(Pct::new(changes))),
                shuffle: Some(seed + w as u64),
                ..Workers::parallel(w)
            };
            let mut r = Rig::new(&p, workers);
            let out = r.play(&b);
            same_bits(&reference, &out, &format!("fuzz seed {seed}, W = {w}"));
            assert_eq!(r.p.stats().schedule_conflicts, 0);
        }
    }
}

/// Blocks back to back, each checked: a block may wake a worker only when the pool had a
/// moment to fall idle before it — the previous block ran past the spin window (the spin
/// share of a period from its arm) or a gap opened between blocks longer than the spin grace after a
/// worker's last task. Returns the wakes, and the blocks that had a reason to wake.
fn busy(r: &mut Rig, blocks: usize) -> (u64, usize) {
    use std::time::{Duration, Instant};
    let half = Duration::from_secs_f64(128.0 / SR * f64::from(Workers::parallel(1).spin_share));
    let grace = Duration::from_micros(40);
    let mut buf = vec![0f32; 256];
    let mut prev: Option<(Instant, Instant)> = None;
    let (mut wakes, mut idle) = (0u64, 0usize);
    for _ in 0..blocks {
        let before = r.p.stats().wakes;
        let start = Instant::now();
        r.p.process(&mut buf, 2);
        let end = Instant::now();
        while r.rx.pop().is_ok() {}
        let woke = r.p.stats().wakes - before;
        let had_reason = prev.is_none_or(|(s, e)| e - s > half - grace || start - e > grace);
        if woke > 0 {
            assert!(
                had_reason,
                "a wake with the workers spinning: block of {:?} after a gap of {:?}",
                prev.map(|(s, e)| e - s),
                prev.map(|(_, e)| start - e)
            );
        }
        idle += usize::from(had_reason);
        wakes += woke;
        prev = Some((start, end));
    }
    (wakes, idle)
}

#[test]
fn a_parked_pool_is_woken_once_per_worker_after_silence_and_not_while_busy() {
    let p = demo_project();
    let mut r = Rig::new(&p, Workers::parallel(3));
    // The first block publishes the period and starts the workers.
    r.play(&[128]);
    let t = std::time::Instant::now();
    while r.p.stats().workers < 3 && t.elapsed() < std::time::Duration::from_secs(5) {
        r.play(&[128]);
    }
    assert_eq!(r.p.stats().workers, 3, "every worker joined");
    // Back to back: they spin through, and no block wakes one without a reason.
    r.play(&[128; 20]);
    let (wakes, idle) = busy(&mut r, 300);
    eprintln!("busy: {wakes} wakes over 300 blocks, {idle} of them with an idle moment");
    // Stop, wait 200 ms: every worker parks. The next block wakes each once.
    r.tx.push(EngineMessage::Stop).ok();
    r.play(&[128]);
    std::thread::sleep(std::time::Duration::from_millis(200));
    r.tx.push(EngineMessage::Play).ok();
    let before = r.p.stats().wakes;
    r.play(&[128]);
    assert_eq!(r.p.stats().wakes - before, 3, "one wake per parked worker");
    let (wakes, idle) = busy(&mut r, 300);
    eprintln!("busy again: {wakes} wakes over 300 blocks, {idle} with an idle moment");
    assert_eq!(r.p.stats().schedule_conflicts, 0);
}

// ---- voices on several threads (ADR 0038) -----------------------------------------------

/// Chords on the instruments that split their voices — Calor, Unda, Exemplar — overlapping,
/// with short and long notes so voices end mid-block (and Calor steals into its ghosts),
/// `alone`: only the first of them (one track: the plan's critical path is all of it).
fn chords(alone: bool) -> Project {
    let mut p = Project::new("voices");
    p.tempo_bpm = 120.0;
    let kinds = [
        InstrumentKind::Analog,
        InstrumentKind::Wavetable,
        InstrumentKind::Sampler,
    ];
    let kinds = if alone { &kinds[..1] } else { &kinds[..] };
    let mut rng = Rng::new(0xC0DE);
    for (t, kind) in kinds.iter().enumerate() {
        let mut tr = p.make_track(format!("{kind:?}"));
        tr.instrument = InstrumentSpec::Native(NativeSpec::new(*kind));
        let mut clip = p.make_clip("chords", Ticks::ZERO, Ticks::beats(8));
        for bar in 0..4i64 {
            for k in 0..10u8 {
                let start = Ticks::beats(bar * 2) + Ticks(rng.range(0, PPQ as usize / 4) as i64);
                let len = Ticks(rng.range(PPQ as usize / 16, 3 * PPQ as usize) as i64);
                let pitch = 40 + t as u8 * 5 + k * 4;
                clip.notes.push(p.make_note(start, len, pitch, 70 + k * 5));
            }
        }
        tr.clips.push(clip);
        p.tracks.push(tr);
    }
    p
}

#[test]
fn heavy_instruments_split_their_voices_on_the_pool_and_keep_the_serial_bits() {
    for (name, p) in [("three", chords(false)), ("alone", chords(true))] {
        let mut rng = Rng::new(name.len() as u64);
        let b = blocks(&mut rng, 3 * 48_000);
        let reference = serial(&p, &b);
        assert!(reference.iter().any(|x| *x != 0.0), "{name}: silent");
        for (w, mode) in [
            (1, Mode::Parallel),
            (3, Mode::Parallel),
            (3, Mode::Auto),
            (7, Mode::Parallel),
        ] {
            let workers = Workers {
                mode,
                ..Workers::parallel(w)
            };
            let mut r = Rig::new(&p, workers);
            let out = r.play(&b);
            let what = format!("{name}, W = {w}, {mode:?}");
            same_bits(&reference, &out, &what);
            let s = *r.p.stats();
            assert_eq!(s.schedule_conflicts, 0, "{what}: conflicts");
            assert!(s.voice_forks > 0, "{what}: no voice was ever forked");
            assert!(
                s.voice_jobs_helped > 0,
                "{what}: {} forks, and no other thread took a voice",
                s.voice_forks
            );
            eprintln!(
                "{what}: {} forks, {} voices rendered by helpers",
                s.voice_forks, s.voice_jobs_helped
            );
        }
        // Splitting off: no fork, the same bits.
        let mut r = Rig::new(&p, Workers::parallel(3));
        r.p.set_voice_split(false);
        let out = r.play(&b);
        same_bits(&reference, &out, &format!("{name}, splitting off"));
        assert_eq!(
            r.p.stats().voice_forks,
            0,
            "{name}: forked with splitting off"
        );
    }
}

/// One heavy instrument playing one note at a time is one long chain with nothing to split:
/// the serial hint must still hold it to one thread. The hint gives way only to a track
/// that has had two voices or more since the last clustering (ADR 0038 §6) — not to every
/// track whose instrument *could* split.
#[test]
fn a_single_voice_line_keeps_the_serial_hint() {
    let mut p = Project::new("mono");
    p.tempo_bpm = 120.0;
    for kind in [
        InstrumentKind::Analog,
        InstrumentKind::Wavetable,
        InstrumentKind::Sampler,
    ] {
        p.tracks.clear();
        let mut tr = p.make_track(format!("{kind:?}"));
        tr.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        // A short note every two beats (a second): even Exemplar's release ends in between.
        let mut clip = p.make_clip("line", Ticks::ZERO, Ticks::beats(8));
        for k in 0..4i64 {
            let note = p.make_note(Ticks::beats(2 * k), Ticks(PPQ / 16), 48 + k as u8, 90);
            clip.notes.push(note);
        }
        tr.clips.push(clip);
        p.tracks.push(tr);
        let workers = Workers {
            mode: Mode::Auto,
            ..Workers::parallel(3)
        };
        let mut r = Rig::new(&p, workers);
        let mut most = 0;
        for _ in 0..(2 * 48_000 / 256 / 4) {
            r.play(&[256; 4]);
            most = most.max(r.p.stats().active_voices);
        }
        assert!(most <= 1, "{kind:?}: the line overlapped ({most} voices)");
        assert!(
            r.p.plan_held_serial(),
            "{kind:?}: one voice at a time, and the plan was not held to one thread"
        );
        assert_eq!(r.p.stats().voice_forks, 0, "{kind:?}: forked one voice");
    }
}

/// `set_voice_split` re-clusters at the next block: off, a one-track chord is one long
/// chain again (held to one thread); on, it is parallel again — so an A/B run's "off" side
/// is the engine without the split, not one still clustered for it.
#[test]
fn switching_the_split_reclusters_at_the_next_block() {
    let p = chords(true);
    let workers = Workers {
        mode: Mode::Auto,
        ..Workers::parallel(3)
    };
    let mut r = Rig::new(&p, workers);
    r.play(&vec![256; 48_000 / 256]);
    assert!(
        !r.p.plan_held_serial(),
        "a chord on the only track: the plan is not held to one thread"
    );
    r.p.set_voice_split(false);
    r.play(&[256, 256]);
    assert!(r.p.plan_held_serial(), "split off: held to one thread");
    r.p.set_voice_split(true);
    r.play(&[256, 256]);
    assert!(!r.p.plan_held_serial(), "split on again: parallel again");
}

/// With the `fuzz` feature: the schedule fuzz's stalls land while voices are forked — a
/// worker stalled with a task in hand or before it waits, and **inside the fork protocol**
/// (`Site::Fork*`: a forker that just opened its slot or is closing it, a helper registered
/// but not yet re-reading the state, a thread holding a claimed voice) — and the bits hold.
#[cfg(feature = "fuzz")]
#[test]
fn the_schedule_fuzz_over_forked_voices_changes_no_sample() {
    use cantus_engine::sched::fuzz::{site_index, ChangePoint, Pct, Stall};
    use cantus_engine::sched::Site;
    use std::time::Duration;
    let p = chords(false);
    let mut in_forks = 0u32;
    for seed in 0..seeds().min(6) {
        let mut rng = Rng::new(seed ^ 0xF0F0);
        let b = blocks(&mut rng, 48_000);
        let reference = serial(&p, &b);
        for w in [1usize, 3, 7] {
            let changes = (0..3)
                .map(|k| ChangePoint {
                    block: 1 + rng.range(0, b.len().min(200)) as u64,
                    step: rng.range(0, 40) as u64,
                    stall: Duration::from_micros(50 + 200 * k),
                })
                .collect();
            let pct = Arc::new(Pct::new(changes));
            let workers = Workers {
                chaos: Some(pct.clone()),
                shuffle: Some(seed + w as u64),
                ..Workers::parallel(w)
            };
            let mut r = Rig::new(&p, workers);
            let out = r.play(&b);
            same_bits(&reference, &out, &format!("fuzz seed {seed}, W = {w}"));
            assert_eq!(r.p.stats().schedule_conflicts, 0);
            in_forks += [
                Site::ForkOpened,
                Site::ForkRegistered,
                Site::ForkClaimed,
                Site::ForkClosing,
            ]
            .iter()
            .map(|s| pct.fired_at[site_index(*s)].load(std::sync::atomic::Ordering::Relaxed))
            .sum::<u32>();
        }
    }
    assert!(in_forks > 0, "no change point ever landed inside a fork");
    // And a stall at each of the protocol's sites in every block, at a varying hit.
    let mut rng = Rng::new(0x5_7A11);
    let b = blocks(&mut rng, 48_000);
    let reference = serial(&p, &b);
    for site in [
        Site::ForkOpened,
        Site::ForkRegistered,
        Site::ForkClaimed,
        Site::ForkClosing,
    ] {
        for w in [3usize, 7] {
            let mut stall = Stall::new(site, 0, Duration::from_micros(40));
            stall.spread = 5;
            let stall = Arc::new(stall);
            let workers = Workers {
                chaos: Some(stall.clone()),
                ..Workers::parallel(w)
            };
            let mut r = Rig::new(&p, workers);
            let out = r.play(&b);
            let what = format!("stalls at {site:?}, W = {w}");
            same_bits(&reference, &out, &what);
            assert_eq!(r.p.stats().schedule_conflicts, 0, "{what}");
            let fired = stall.fired.load(std::sync::atomic::Ordering::Relaxed);
            assert!(fired > 0, "{what}: never stalled");
            eprintln!("{what}: {fired} stalls, {} forks", r.p.stats().voice_forks);
        }
    }
}
