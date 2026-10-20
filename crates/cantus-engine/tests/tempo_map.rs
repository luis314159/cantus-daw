//! The engine through the tempo map (round 29, ADR 0060): a song with a ramp, a step and
//! metre changes plays the same at every block size, offline as in real time, in parallel
//! as serially; the click lands on every beat of the ramp to the sample and accents bar 1
//! of each metre; a note after the ramp sounds at its analytic sample; a synced gate and an
//! automation step stay on the beat through the ramp; a loop over a tempo change wraps
//! exactly.

#[path = "support/modfx.rs"]
mod modfx;
mod support;

use cantus_core::tempo::{MetreChange, TempoPoint};
use cantus_core::*;
use cantus_engine::*;
use modfx::{realtime, set_all};
use std::sync::Arc;
use support::{add_effect, demo_project, render_with};

const SR: f64 = 48_000.0;

fn pt(beats: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks::beats(beats),
        bpm,
        ramp,
    }
}

/// The demo's song with a tempo track: 90 BPM for a beat, a ramp to 140 over four beats,
/// held, then 110 from beat 7 — and a bar of 3/4 after the first 4/4 bar, then 4/4.
fn with_tempo(mut p: Project) -> Project {
    p.tempo_bpm = 90.0;
    p.tempo_points = vec![
        pt(0, 90.0, false),
        pt(1, 90.0, true),
        pt(5, 140.0, false),
        pt(7, 110.0, false),
    ];
    p.metre_changes = vec![
        MetreChange {
            tick: Ticks::ZERO,
            signature: TimeSignature::new(4, 4),
        },
        MetreChange {
            tick: Ticks::beats(4),
            signature: TimeSignature::new(3, 4),
        },
        MetreChange {
            tick: Ticks::beats(7),
            signature: TimeSignature::new(4, 4),
        },
    ];
    p
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!(
            "{what}: first difference at sample {i} ({} vs {})",
            a[i], b[i]
        );
    }
}

/// The demo with a synced tremolo gate and the click: everything that follows the tempo.
fn busy_song() -> Project {
    let mut p = with_tempo(demo_project());
    let fx = add_effect(&mut p, EffectKind::Tremolo);
    set_all(
        &mut p,
        fx,
        &[("depth", 1.0), ("div", 8.0), ("shape", 2.0), ("edge", 0.0)],
    );
    p.metronome = true;
    p.loop_enabled = false;
    p
}

#[test]
fn a_tempo_map_renders_the_same_at_every_block_size() {
    let p = busy_song();
    let reference = render_with(&p, 64);
    assert!(reference.iter().any(|x| x.abs() > 0.01), "it plays");
    for block in [1usize, 37, 512] {
        same_bits(
            &reference,
            &render_with(&p, block),
            &format!("block {block}"),
        );
    }
}

#[test]
fn offline_equals_real_time_across_a_ramp_and_a_metre_change() {
    // The click is the player's, never rendered: off for the comparison.
    let mut p = busy_song();
    p.metronome = false;
    let offline = render_with(&p, 256);
    same_bits(&offline, &realtime(&p, offline.len()), "real time");
}

#[test]
fn parallel_equals_serial_across_a_ramp() {
    let p = busy_song();
    let play = |workers: Workers| {
        let snap = Arc::new(Snapshot::compile(&p, 1));
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
        proc.set_workers(workers);
        for slot in snap.effect_slots() {
            tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
                slot, SR as f32,
            ))))
            .ok();
        }
        for unit in cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false) {
            tx.push(EngineMessage::AddInstrument(unit)).ok();
        }
        tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .ok();
        let plan = cantus_engine::graph::compile(&snap);
        tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
        proc.settle();
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::EffectNeed { effect, need } = ev {
                let memory = EffectMemory::build(need, SR as f32);
                tx.push(EngineMessage::EffectMemory { effect, memory }).ok();
                proc.settle();
            }
        }
        tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        tx.push(EngineMessage::Play).ok();
        let mut out = Vec::new();
        let mut buf = vec![0f32; 300 * 2];
        for _ in 0..1200 {
            proc.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            while rx.pop().is_ok() {}
        }
        out
    };
    let serial = play(Workers::SERIAL);
    same_bits(&serial, &play(Workers::parallel(3)), "three workers");
}

/// Play `p` live from `from` for `frames` (the click sounds live, never in a render), in
/// blocks of `block`.
fn play(p: &Project, from: Ticks, frames: usize, block: usize) -> Vec<f32> {
    let mut snap = Snapshot::compile(p, 1);
    snap.loop_region = if p.loop_enabled {
        p.loop_region.clone()
    } else {
        None
    };
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Seek(from)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(frames * 2);
    let mut buf = vec![0f32; block * 2];
    while out.len() / 2 < frames {
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out.truncate(frames * 2);
    out
}

/// The first sample at or after `from` whose left channel is louder than `level`.
fn onset(out: &[f32], from: usize, level: f32) -> Option<usize> {
    (from..out.len() / 2).find(|i| out[2 * i].abs() > level)
}

#[test]
fn the_click_lands_on_every_beat_of_a_ramp_and_accents_each_bar_one() {
    let mut p = with_tempo(Project::new("click"));
    p.metronome = true;
    p.loop_enabled = false;
    // An empty song plays its click to its end: give it a silent clip to beat 12.
    let mut t = p.make_track("x");
    t.clips
        .push(p.make_clip("empty", Ticks::ZERO, Ticks::beats(12)));
    p.tracks.push(t);
    let map = p.tempo_map();
    let frames = map.sample_at(Ticks::beats(12).0 as f64, SR) as usize;
    let out = play(&p, Ticks::ZERO, frames, 128);
    let mut accents = Vec::new();
    let mut plain = Vec::new();
    // The click's own onset (its first sample may be silent): the same after every beat.
    let lead = onset(&out, 0, 1e-6).expect("the first click");
    assert!(lead <= 2, "{lead}");
    for (tick, accent) in map.beat_lines(Ticks::ZERO, Ticks::beats(11)) {
        let want = (map.sample_at(tick.0 as f64, SR) - 1e-6).ceil() as usize;
        let got = onset(&out, want.saturating_sub(4), 1e-6).expect("a click");
        assert_eq!(got, want + lead, "the click at {tick} (accent {accent})");
        let shape: Vec<u32> = (got..got + 200).map(|i| out[2 * i].to_bits()).collect();
        if accent {
            accents.push(shape);
        } else {
            plain.push(shape);
        }
    }
    // Bars 1 (4/4), 2 (3/4) and 3 (4/4) at beats 0, 4 and 7, then beat 11's.
    assert_eq!(accents.len(), 3, "bar 1 of each bar");
    assert!(accents.windows(2).all(|w| w[0] == w[1]));
    assert!(plain.windows(2).all(|w| w[0] == w[1]));
    assert_ne!(accents[0], plain[0], "the accent is its own sound");
}

#[test]
fn a_note_after_the_ramp_sounds_at_its_analytic_sample() {
    let mut p = with_tempo(Project::new("note"));
    p.loop_enabled = false;
    let mut t = p.make_track("Square");
    let f = t.instrument.firmus_mut().expect("Firmus");
    f.osc1_wave = Waveform::Square;
    f.amp_attack_s = 0.0;
    let at = Ticks::beats(6) + Ticks(123);
    let mut clip = p.make_clip("n", Ticks::ZERO, Ticks::beats(8));
    clip.notes.push(p.make_note(at, Ticks::QUARTER, 60, 120));
    t.clips.push(clip);
    p.tracks.push(t);
    let out = render_with(&p, 100);
    let map = p.tempo_map();
    let want = (map.sample_at(at.0 as f64, SR) - 1e-6).ceil() as usize;
    let got = onset(&out, 0, 1e-6).expect("the note sounds");
    assert!(
        (want..=want + 2).contains(&got),
        "the note at sample {got}, the map says {want}"
    );
}

#[test]
fn a_synced_gate_stays_on_the_beat_through_the_ramp() {
    let mut base = with_tempo(demo_project());
    base.tracks.truncate(1);
    base.loop_enabled = false;
    let mut p = base.clone();
    let fx = add_effect(&mut p, EffectKind::Tremolo);
    set_all(
        &mut p,
        fx,
        &[("depth", 1.0), ("div", 8.0), ("shape", 2.0), ("edge", 0.0)],
    );
    let out = render_with(&p, 256);
    let reference = render_with(&base, 256);
    let map = p.tempo_map();
    let frame = |beats: f64| map.sample_at(beats * PPQ as f64, SR) as usize;
    let mut checked = 0;
    for beat in 0..7 {
        let b = f64::from(beat);
        // The second half of every beat is shut, the first half open, through the ramp.
        for f in frame(b + 0.6)..frame(b + 0.9) {
            assert_eq!(out[2 * f], 0.0, "beat {beat}: frame {f} is in a shut half");
        }
        for f in frame(b + 0.1)..frame(b + 0.4) {
            assert_eq!(
                out[2 * f].to_bits(),
                reference[2 * f].to_bits(),
                "beat {beat}: frame {f} is in an open half"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn an_automation_step_after_the_ramp_lands_on_its_tick() {
    let mut base = with_tempo(demo_project());
    base.tracks.truncate(1);
    base.loop_enabled = false;
    let mut p = base.clone();
    let strip = p.tracks[0].strip.unwrap();
    let at = Ticks::beats(6);
    // The strip's own level (unity) until the step, then the bottom of the range.
    let unity = cantus_core::automation::range::range_of(&p, &AutoTarget::StripGain(strip))
        .unwrap()
        .pos(p.strip(strip).unwrap().gain_db);
    let points = vec![
        AutoPoint::new(Ticks::ZERO, unity),
        AutoPoint::new(at, unity),
        AutoPoint::new(at + Ticks(1), 0.0),
    ];
    let c = p.make_automation_clip(
        "gain",
        AutoTarget::StripGain(strip),
        Ticks::ZERO,
        Ticks::beats(8),
        points,
    );
    p.tracks[0].clips.push(c);
    let out = render_with(&p, 256);
    let reference = render_with(&base, 256);
    let map = p.tempo_map();
    let step = map.sample_at(at.0 as f64, SR) as usize;
    // Before the step, as without the lane; a control grid after it, silent.
    for f in step - 2000..step - 64 {
        assert!(
            (out[2 * f] - reference[2 * f]).abs() <= reference[2 * f].abs() * 0.02 + 1e-6,
            "frame {f} before the step"
        );
    }
    for f in step + 64..step + 2000 {
        assert!(
            out[2 * f].abs() < 1e-3,
            "frame {f} after the step: {}",
            out[2 * f]
        );
    }
}

#[test]
fn a_loop_over_a_tempo_change_wraps_exactly_in_the_engine() {
    // The click in a loop from beat 3 to beat 6 (over the ramp's end): each pass's first
    // click is the map's loop length after the last.
    let mut p = with_tempo(Project::new("loop"));
    p.metronome = true;
    let mut t = p.make_track("x");
    t.clips
        .push(p.make_clip("empty", Ticks::ZERO, Ticks::beats(12)));
    p.tracks.push(t);
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(3),
        end: Ticks::beats(6),
    });
    p.loop_enabled = true;
    let map = p.tempo_map();
    let len = map.seconds_between(Ticks::beats(3), Ticks::beats(6)) * SR;
    let out = play(&p, Ticks::beats(3), (len * 6.0) as usize, 257);
    // The click's onsets at the loop's start: the first at 0, then each pass later.
    let mut starts = vec![0usize];
    let mut from = 0;
    let lead = onset(&out, 0, 1e-6).expect("the first click");
    for k in 1..5 {
        let want = (len * k as f64).round() as usize + lead;
        from = from.max(want - 50);
        let got = onset(&out, from, 1e-6).expect("a click");
        starts.push(got);
        assert!(
            got.abs_diff(want) <= 1,
            "pass {k}: the loop's first click at {got}, the map says {want}"
        );
        from = got + 100;
    }
}

/// Found with the tempo map's loop test: after a wrap the playhead sits a fraction of a
/// sample past the loop's start (the overshoot carried so loops never drift), and the
/// events and the click were looked up from the next whole tick on — a note or a beat
/// exactly on the loop's start was skipped on every pass after the first, at any tempo
/// whose loop is no whole number of samples (here 133 BPM, one tempo).
#[test]
fn a_beat_on_the_loops_start_sounds_on_every_pass() {
    let mut p = Project::new("loop start");
    p.tempo_bpm = 133.0;
    p.metronome = true;
    let mut t = p.make_track("x");
    t.clips
        .push(p.make_clip("empty", Ticks::ZERO, Ticks::beats(12)));
    p.tracks.push(t);
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(1),
        end: Ticks::beats(2),
    });
    p.loop_enabled = true;
    let len = 60.0 / 133.0 * SR;
    let out = play(&p, Ticks::beats(1), (len * 5.5) as usize, 64);
    let lead = onset(&out, 0, 1e-6).expect("the first click");
    for k in 0..5 {
        let at = (len * k as f64).ceil() as usize + lead;
        let got = onset(&out, at.saturating_sub(10), 1e-6).expect("a click");
        assert!(
            got.abs_diff(at) <= 1,
            "pass {k}: the click at {got}, the loop's start at {at}"
        );
    }
}

/// Review 1 (findings 6, 8, 11): a synced unit behind latency (a limiter's look-ahead
/// before the tremolo) hears each tempo point and each ramp step at the sample its audio
/// reaches it, whatever the blocks: the passes are cut at the late unit's own steps too.
#[test]
fn a_late_synced_unit_hears_the_steps_at_every_block_size() {
    let mut p = with_tempo(demo_project());
    let _lim = add_effect(&mut p, EffectKind::Limiter);
    let fx = add_effect(&mut p, EffectKind::Tremolo);
    set_all(
        &mut p,
        fx,
        &[("depth", 1.0), ("div", 8.0), ("shape", 0.0), ("edge", 0.0)],
    );
    p.loop_enabled = false;
    let reference = render_with(&p, 64);
    for block in [1usize, 37, 512] {
        same_bits(
            &reference,
            &render_with(&p, block),
            &format!("block {block}"),
        );
    }
}

/// Review 1 (finding 18), review 2 (finding 21): an offline render's length and its stop
/// sample are measured through the map — a range to beat 6 over the ramp is the map's
/// seconds long plus the tail; a note exactly at its end is never heard (a half-second tail
/// would hear it), and a note one tick before it sounds from the map's sample. The stems
/// and a modulator's trace run the same length.
#[test]
fn a_rendered_range_is_as_long_as_the_map_says() {
    let song = |at: Ticks| {
        let mut p = with_tempo(Project::new("range"));
        p.loop_enabled = false;
        let mut t = p.make_track("Square");
        t.instrument.firmus_mut().unwrap().osc1_wave = Waveform::Square;
        let mut clip = p.make_clip("n", Ticks::ZERO, Ticks::beats(8));
        clip.notes.push(p.make_note(at, Ticks::QUARTER, 60, 120));
        t.clips.push(clip);
        p.tracks.push(t);
        p
    };
    let end = Ticks::beats(6);
    let opts = RenderOptions {
        end: Some(end),
        tail_seconds: 0.5,
        block_frames: 128,
        ..Default::default()
    };
    let on_end = song(end);
    let map = on_end.tempo_map();
    let out = render(Arc::new(Snapshot::compile(&on_end, 1)), &opts)
        .unwrap()
        .samples;
    let seconds = map.seconds_between(Ticks::ZERO, end);
    let want = ((seconds + 0.5) * SR).ceil() as usize;
    assert_eq!(out.len() / 2, want);
    assert!(
        out.iter().all(|x| *x == 0.0),
        "the note at the end never sounds, tail and all"
    );
    let before = song(end - Ticks(1));
    let out = render(Arc::new(Snapshot::compile(&before, 1)), &opts)
        .unwrap()
        .samples;
    let at = map.sample_at((end - Ticks(1)).0 as f64, SR).ceil() as usize;
    assert!(
        out[..2 * at].iter().all(|x| *x == 0.0),
        "nothing before the note"
    );
    let heard = onset(&out, at, 1e-4).expect("the note one tick before the end sounds");
    assert!(
        heard - at < 64,
        "it starts at the map's sample {at}, heard at {heard}"
    );
    // The stems: the same length, the same stop.
    let stem_opts = StemOptions {
        render: opts.clone(),
        keep_silent: true,
        ..Default::default()
    };
    let spec = StemSpec {
        source: StemSource::Track(0),
        file_name: "t.wav".into(),
    };
    let stems = render_stems(Arc::new(Snapshot::compile(&on_end, 1)), &stem_opts, &[spec]).unwrap();
    assert_eq!(stems[0].samples.len() / 2, want);
    assert!(stems[0].silent, "the stem never hears the note at the end");
    // A modulator's trace covers the range through the map: its last point is at the end.
    let mut m = before.clone();
    let strip = m.tracks[0].strip.unwrap();
    let lfo = Modulator::new(ModulatorId(m.alloc_id()), ModKind::Lfo);
    let id = lfo.id;
    let mut lfo = lfo;
    lfo.routes.push(ModRoute {
        target: AutoTarget::StripPan(strip),
        depth: 0.5,
    });
    m.tracks[0].modulators.push(lfo);
    let trace = cantus_engine::modulator::trace(
        Arc::new(Snapshot::compile(&m, 1)),
        &opts,
        Vec::new(),
        Vec::new(),
        id,
    );
    // (tick, value) pairs, a control point every 16 samples at most.
    let last = trace.last().expect("a trace").0;
    let e = end.0 as f64;
    assert!(
        last <= e && e - last < 2.0,
        "the trace ends at tick {last}, not at the range's end {e}"
    );
}

/// Review 1 (finding 19): played live — the click, a loop over the tempo change and an
/// automation lane — the same at every block size.
#[test]
fn the_click_a_loop_and_automation_live_are_the_same_at_every_block_size() {
    let mut p = busy_song();
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(3),
        end: Ticks::beats(7),
    });
    p.loop_enabled = true;
    let strip = p.tracks[0].strip.unwrap();
    let points = vec![
        AutoPoint::new(Ticks::ZERO, 0.3),
        AutoPoint::new(Ticks::beats(5), 0.9),
    ];
    let c = p.make_automation_clip(
        "g",
        AutoTarget::StripGain(strip),
        Ticks::ZERO,
        Ticks::beats(8),
        points,
    );
    p.tracks[0].clips.push(c);
    let frames = 200_000;
    let reference = play(&p, Ticks::ZERO, frames, 64);
    for block in [1usize, 37, 512] {
        same_bits(
            &reference,
            &play(&p, Ticks::ZERO, frames, block),
            &format!("block {block}"),
        );
    }
}

/// Review 1 (finding 24): the arithmetic by hand, not the map against itself — 120 BPM for
/// four beats (96 000 samples at 48 kHz), then 60: beat 6 is 96 000 + 96 000 samples in.
#[test]
fn an_event_after_a_step_is_due_at_the_hand_computed_sample() {
    let mut p = Project::new("hand");
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks::beats(4),
            bpm: 60.0,
            ramp: false,
        },
    ];
    let mut t = Transport::new(SR);
    t.set_map(Arc::new(p.tempo_map()));
    t.seek(Ticks::ZERO);
    t.play();
    assert_eq!(t.samples_until(Ticks::beats(6).0 as f64), 192_000);
    t.advance(150_000);
    // 54 000 samples past the point at 60 BPM: 1.125 beats past beat 4.
    assert!((t.position_f64() - 5.125 * 960.0).abs() < 1e-9);
}

/// ADR 0060 §4's measurement: what cutting the passes at every ramp step costs. The busy
/// song rendered over a ramp against the same song at one tempo, best of five (run with
/// `--ignored --nocapture`).
#[test]
#[ignore = "a measurement"]
fn measure_the_cost_of_ramp_steps() {
    let ramp = busy_song();
    let mut flat = ramp.clone();
    flat.tempo_points.clear();
    flat.metre_changes.clear();
    let time_at = |p: &Project, block: usize| {
        (0..5)
            .map(|_| {
                let t0 = std::time::Instant::now();
                let _ = render_with(p, block);
                t0.elapsed().as_secs_f64()
            })
            .fold(f64::MAX, f64::min)
    };
    let time = |p: &Project| time_at(p, 256);
    let (a, b) = (time(&ramp), time(&flat));
    // What the extra passes alone cost: the one-tempo song cut into the passes a ramp adds
    // (a 60-tick step every ~1 560 samples at these tempos on top of 256-sample blocks is
    // ~16 % more passes: blocks of 221).
    let (c, d) = (time_at(&flat, 256), time_at(&flat, 221));
    eprintln!("one tempo, 16 % more passes: ratio {:.3}", d / c);
    let secs = |p: &Project| p.tempo_map().seconds_between(Ticks::ZERO, p.content_end());
    let (sa, sb) = (secs(&ramp), secs(&flat));
    eprintln!(
        "ramp: {:.3} ms per song second; one tempo: {:.3} ms per song second; ratio {:.3}",
        a / sa * 1e3,
        b / sb * 1e3,
        (a / sa) / (b / sb)
    );
}
