//! The recording mode on the engine (round 36, ADR 0092): the whole plan baked plays the
//! live mix to the bit — at every block size, offline and on the pool, with the mix
//! controls live — an armed strip stays live and heard at once, and the mode's keys
//! invalidate exactly the strip that changed.

mod support;
use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::record_mode;
use cantus_core::*;
use cantus_engine::freeze::render::prepare;
use cantus_engine::*;
use std::path::PathBuf;
use support::*;

const SR: f64 = 48_000.0;

fn folder(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("record_mode")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pair(name: &str) -> (EngineHandle, Processor) {
    let (mut h, p) = EngineHandle::manual(SR);
    h.frozen.set_app_dir(folder(name));
    (h, p)
}

fn handle(name: &str) -> EngineHandle {
    pair(name).0
}

/// The demo song, a delay and a send into a reverb return on the lead's strip, a
/// compressor and a linear-phase EQ (a latency) on the bass's, a ride on the lead's fader.
fn song() -> Project {
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Delay);
    let lead = p.tracks[0].strip.unwrap();
    let ret = p.strips[5].id;
    let mut space = p.make_effect(EffectKind::Space);
    space.spec.set("mix", 1.0);
    p.strip_mut(ret).unwrap().effects.push(space);
    p.strip_mut(lead).unwrap().sends.push(Send {
        to: ret,
        level: 0.4,
        enabled: true,
        tap: SendTap::Post,
    });
    let comp = p.make_effect(EffectKind::Compressor);
    strip_of(&mut p, 1).effects.push(comp);
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0).unwrap();
    strip_of(&mut p, 1).effects.push(eq);
    let ride = p.make_automation_clip(
        "ride",
        cantus_core::automation::AutoTarget::StripGain(lead),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            cantus_core::automation::AutoPoint::new(Ticks::ZERO, 0.8),
            cantus_core::automation::AutoPoint::new(Ticks::beats(4), 0.3),
            cantus_core::automation::AutoPoint::new(Ticks::beats(8), 0.9),
        ],
    );
    p.tracks[0].clips.push(ride);
    p
}

/// What *Prepare to record* does, minus the interface: render each planned target into
/// the handle's folder and freeze it.
fn prepare_to_record(h: &mut EngineHandle, p: &mut Project) -> record_mode::Plan {
    let plan = record_mode::plan(p);
    for target in plan.bake.clone() {
        let job = prepare(h, p, target).expect("prepared");
        job.run(Vec::new(), Vec::new(), &RenderProgress::default())
            .expect("rendered");
        h.frozen.forget_missing();
        freeze::freeze_commands(p, target)
            .unwrap()
            .apply(p)
            .unwrap();
    }
    plan
}

fn mix(h: &mut EngineHandle, p: &Project, block: usize) -> Vec<f32> {
    let snap = h.compile_for_render(p, 1);
    render(
        snap,
        &RenderOptions {
            block_frames: block,
            tail_seconds: 1.5,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
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
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

#[test]
fn the_whole_plan_baked_plays_the_live_mix_to_the_bit_at_every_block_size() {
    let mut h = handle("null");
    let mut p = song();
    let live = mix(&mut h, &p, 512);
    let plan = prepare_to_record(&mut h, &mut p);
    assert!(
        plan.bake.len() >= 2 && plan.live.is_empty(),
        "every strip that feeds the master: {plan:?}"
    );
    let snap = h.compile_for_render(&p, 1);
    // What only reaches a baked strip rests: the CPU is freed.
    assert!(
        snap.tracks.iter().all(|t| t.rest),
        "every source rests: its render plays"
    );
    same_bits(&mix(&mut h, &p, 512), &live, "baked");
    for block in [1, 37, 64, 1024] {
        same_bits(&mix(&mut h, &p, block), &live, &format!("block {block}"));
    }
}

#[test]
fn the_mix_controls_of_a_baked_strip_stay_live_and_equal_the_live_mix() {
    let mut h = handle("mix");
    let mut p = song();
    prepare_to_record(&mut h, &mut p);
    let bass = p.tracks[1].strip.unwrap();
    for cmd in [
        Command::SetStripGain {
            id: bass,
            gain_db: -6.0,
        },
        Command::SetStripPan { id: bass, pan: 0.7 },
        Command::SetStripMute {
            id: bass,
            muted: true,
        },
    ] {
        let mut thawed = p.clone();
        freeze::thaw(&mut thawed);
        cmd.clone().apply(&mut thawed).unwrap();
        cmd.apply(&mut p).unwrap();
        same_bits(
            &mix(&mut h, &p, 512),
            &mix(&mut h, &thawed, 512),
            "a live control of a baked strip",
        );
    }
}

#[test]
fn an_armed_track_stays_live_and_the_rest_is_still_the_live_mix_to_the_bit() {
    let mut h = handle("armed");
    let mut p = song();
    let bass = p.tracks[1].id;
    p.track_mut(bass).unwrap().armed = true;
    let live = mix(&mut h, &p, 512);
    let plan = prepare_to_record(&mut h, &mut p);
    let bass_strip = p.tracks[1].strip.unwrap();
    assert!(
        p.strip(bass_strip).unwrap().frozen.is_none(),
        "the armed track's strip is not baked: {plan:?}"
    );
    assert!(!plan.live.is_empty());
    let snap = h.compile_for_render(&p, 1);
    assert!(!snap.tracks[1].rest, "the armed track plays live");
    assert!(snap.tracks[0].rest, "the other is baked");
    same_bits(&mix(&mut h, &p, 512), &live, "armed live, rest baked");
}

#[test]
fn a_changed_strip_invalidates_exactly_its_own_render() {
    let h = handle("keys");
    let _ = &h;
    let mut p = song();
    let plan = record_mode::plan(&p);
    let keys = |p: &Project| -> Vec<(FreezeTarget, String)> {
        plan.bake
            .iter()
            .map(|t| {
                (
                    *t,
                    cantus_engine::freeze::render_key(&freeze::content_key(p, *t).unwrap(), 48_000),
                )
            })
            .collect()
    };
    let before = keys(&p);
    // A note on the bass moves: the bass's strip is another sound, nothing else is.
    let bass = p.tracks[1].id;
    let clip = p.tracks[1].clips[0].id;
    let note = p.tracks[1].clips[0].notes[0].id;
    let mut notes = p.tracks[1].clips[0].notes.clone();
    notes.iter_mut().find(|n| n.id == note).unwrap().pitch += 1;
    Command::SetNotes {
        track: bass,
        clip,
        notes,
    }
    .apply(&mut p)
    .unwrap();
    let after = keys(&p);
    let bass_strip = FreezeTarget::Strip(p.tracks[1].strip.unwrap());
    for ((t, a), (_, b)) in before.iter().zip(&after) {
        if *t == bass_strip {
            assert_ne!(a, b, "the changed strip is rendered again");
        } else {
            assert_eq!(a, b, "{t:?} keeps its render");
        }
    }
}

fn play(
    snap: &std::sync::Arc<Snapshot>,
    workers: Workers,
    block: usize,
    blocks: usize,
) -> Vec<f32> {
    let effects = snap.effect_slots().count();
    let instruments = cantus_engine::inst::units_for(snap, SR as f32, |_, _| false);
    let (mut tx, rx_engine) =
        rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    p.set_workers(workers);
    p.set_offline(true);
    for slot in snap.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, SR as f32,
        ))))
        .ok();
    }
    for unit in instruments {
        tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(snap)))
        .ok();
    tx.push(EngineMessage::SetSnapshotPlanned(
        snap.clone(),
        cantus_engine::graph::compile(snap),
    ))
    .ok();
    p.settle();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                count, delay,
            )))
            .ok();
            p.settle();
        }
    }
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    let mut buf = vec![0f32; block * 2];
    p.process(&mut buf, 2);
    tx.push(EngineMessage::Play).ok();
    let mut out = Vec::new();
    for _ in 0..blocks {
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

#[test]
fn the_baked_session_on_the_pool_is_the_serial_engines() {
    let mut h = handle("pool");
    let mut p = song();
    prepare_to_record(&mut h, &mut p);
    let snap = h.compile_for_render(&p, 1);
    let serial = play(&snap, Workers::SERIAL, 256, 700);
    for w in [1, 3] {
        same_bits(
            &play(&snap, Workers::parallel(w), 256, 700),
            &serial,
            &format!("W = {w}"),
        );
    }
}

// ---- low-latency monitoring in the mode ------------------------------------------------

const BLOCK: usize = 256;

/// A voice (armed, monitored, through Tonus: 128 samples) on its own strip, and a backing
/// with a linear-phase EQ (thousands of samples) that plays notes.
fn voice_and_backing() -> Project {
    let mut p = Project::new("rec");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    p.tracks.push(t);
    let mut backing = p.make_track("Backing");
    let mut c = p.make_clip("b", Ticks::ZERO, Ticks::beats(8));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(4), 48, 100));
    backing.clips.push(c);
    p.tracks.push(backing);
    let voice = p.tracks[0].strip.unwrap();
    let back = p.tracks[1].strip.unwrap();
    let tonus = p.make_effect(EffectKind::AutoTune);
    p.strip_mut(voice).unwrap().effects.push(tonus);
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0).unwrap();
    p.strip_mut(back).unwrap().effects.push(eq);
    p
}

/// Where an impulse at frame 2 000 of the input comes out, frames after it, the figure the
/// engine says it waits, and the master's arrival.
fn heard_after(h: &mut EngineHandle, mut proc: Processor, p: &Project) -> (usize, u64, u64) {
    let (mut input, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 4);
    proc.attach_input(consumer, 1);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    h.send(EngineMessage::SetLowLatency(Some(144)));
    let mut x = vec![0f32; 24_000];
    x[2_000] = 1.0;
    let mut buf = vec![0f32; BLOCK * 2];
    for _ in 0..4 {
        proc.process(&mut buf, 2);
        h.poll();
    }
    let mut out: Vec<f32> = Vec::new();
    for chunk in x.chunks(BLOCK) {
        for v in chunk {
            input.push(*v).unwrap();
        }
        proc.process(&mut buf[..chunk.len() * 2], 2);
        h.poll();
        out.extend(buf[..chunk.len() * 2].iter().step_by(2));
    }
    let at = out
        .iter()
        .position(|v| v.abs() > 1e-4)
        .expect("the voice is heard");
    let s = proc.stats();
    (
        at - 2_000,
        s.low_latency.monitored_samples,
        s.latency_samples,
    )
}

#[test]
fn the_armed_voice_is_not_held_back_by_the_baked_backings_latency() {
    // Live, as the monitoring has always been: the voice waits for its own chain only.
    let (mut h, proc) = pair("ll_live");
    let p = voice_and_backing();
    let (live_heard, live_said, live_arrival) = heard_after(&mut h, proc, &p);
    assert!((100..200).contains(&live_heard), "Tonus: {live_heard}");
    assert!(live_arrival > 1_000, "the backing's EQ sets the arrival");
    // Baked: the backing's render keeps its latency, the voice's way is the same.
    let (mut h, proc) = pair("ll_baked");
    let mut p = voice_and_backing();
    let plan = prepare_to_record(&mut h, &mut p);
    assert!(
        plan.live
            .iter()
            .any(|(_, w)| matches!(w, record_mode::Why::Armed(_))),
        "{plan:?}"
    );
    assert!(plan
        .bake
        .contains(&FreezeTarget::Strip(p.tracks[1].strip.unwrap())));
    let (heard, said, arrival) = heard_after(&mut h, proc, &p);
    assert_eq!(heard, live_heard, "the voice is heard as soon as live");
    assert_eq!(said, live_said);
    assert_eq!(
        arrival, live_arrival,
        "the backing stays aligned: its render holds the latency"
    );
}

// ---- the baked master (ADR 0092 §3) ---------------------------------------------------

/// The master baked: the whole project (but what stays live) as one stereo render, played
/// by the master stage in place of the master chain and fader.
fn bake_master(h: &mut EngineHandle, p: &Project, live: &[FreezeTarget]) -> String {
    let job = cantus_engine::record_mode::prepare_master(h, p, live);
    let key = job.key.clone();
    job.run(&RenderProgress::default())
        .expect("the master rendered");
    h.frozen.forget_missing();
    h.frozen.set_master(Some(key.clone()));
    key
}

fn with_master_chain(mut p: Project) -> Project {
    let mut lim = p.make_effect(EffectKind::Limiter);
    lim.spec.set("look", 5.0).unwrap();
    p.master.effects.push(lim);
    p.master.gain_db = -1.5;
    p.master.pan = 0.1;
    p
}

#[test]
fn a_baked_master_plays_the_live_masters_output_to_the_bit() {
    let mut h = handle("master");
    let mut p = with_master_chain(song());
    let live = mix(&mut h, &p, 512);
    prepare_to_record(&mut h, &mut p);
    bake_master(&mut h, &p, &[]);
    let snap = h.compile_for_render(&p, 1);
    assert!(snap.master_frozen.as_ref().is_some_and(|f| f.plays()));
    assert!(snap.master_effects.is_empty(), "the chain is in the render");
    assert!(snap.tracks.iter().all(|t| t.rest), "every source rests");
    assert!(
        snap.strips
            .iter()
            .filter(|s| s.frozen.is_some())
            .all(|s| s.rest),
        "every baked strip rests"
    );
    for block in [512, 64, 1] {
        same_bits(
            &mix(&mut h, &p, block),
            &live,
            &format!("baked master, block {block}"),
        );
    }
    // The master unbaked: the strips' renders again, the same mix.
    h.frozen.set_master(None);
    same_bits(&mix(&mut h, &p, 512), &live, "the master live again");
}

#[test]
fn an_armed_track_plays_live_over_the_baked_master() {
    let mut h = handle("master_armed");
    let mut p = with_master_chain(song());
    let bass = p.tracks[1].id;
    p.track_mut(bass).unwrap().armed = true;
    let plan = prepare_to_record(&mut h, &mut p);
    let live_roots: Vec<FreezeTarget> = plan.live.iter().map(|(t, _)| *t).collect();
    bake_master(&mut h, &p, &live_roots);
    let snap = h.compile_for_render(&p, 1);
    assert!(!snap.tracks[1].rest, "the armed track plays");
    assert!(snap.tracks[0].rest, "the rest is in the master's render");
    // The armed track muted: the reference alone, which is the live mix without it.
    let mut muted = p.clone();
    muted.track_mut(bass).unwrap().muted = true;
    let mut live_without = muted.clone();
    freeze::thaw(&mut live_without);
    h.frozen.set_master(None);
    let reference = mix(&mut h, &live_without, 512);
    let key = cantus_engine::record_mode::master_key(&p, &live_roots);
    h.frozen.set_master(Some(key));
    same_bits(&mix(&mut h, &muted, 512), &reference, "the reference");
    // Unmuted, the armed track is heard over it.
    let with = mix(&mut h, &p, 512);
    assert!(
        with.iter().zip(&reference).any(|(a, b)| a != b),
        "the armed track sounds"
    );
}

#[test]
fn the_master_key_follows_what_it_holds() {
    let p = song();
    let a = cantus_engine::record_mode::master_key(&p, &[]);
    assert_eq!(a.len(), 64);
    assert_eq!(a, cantus_engine::record_mode::master_key(&p, &[]));
    let mut q = p.clone();
    q.master.gain_db = -3.0;
    assert_ne!(
        a,
        cantus_engine::record_mode::master_key(&q, &[]),
        "the master's fader"
    );
    let live = [FreezeTarget::Strip(p.tracks[1].strip.unwrap())];
    assert_ne!(
        a,
        cantus_engine::record_mode::master_key(&p, &live),
        "what stays live"
    );
}

// ---- a pre-fader sender keeps its fader (ADR 0092 §2) ---------------------------------

#[test]
fn a_pre_fader_sender_keeps_a_live_fader_and_the_mix_stays_the_live_one_to_the_bit() {
    let mut h = handle("pre_fader");
    let mut p = song();
    let lead = p.tracks[0].strip.unwrap();
    let ret = p.strips[5].id;
    // The lead's send into the return taps before its fader; its own rides go (the static
    // fader is what is moved here).
    p.strip_mut(lead).unwrap().sends[0].tap = SendTap::Pre;
    for t in &mut p.tracks {
        t.clips.retain(|c| {
            !matches!(
                c.automation.as_ref().map(|a| &a.target),
                Some(cantus_core::automation::AutoTarget::StripGain(_))
                    | Some(cantus_core::automation::AutoTarget::StripPan(_))
            )
        });
    }
    let live = mix(&mut h, &p, 512);
    prepare_to_record(&mut h, &mut p);
    assert!(p.frozen(FreezeTarget::Strip(ret)).is_some());
    same_bits(&mix(&mut h, &p, 512), &live, "baked, unmoved");
    let key = freeze::content_key(&p, FreezeTarget::Strip(ret)).unwrap();
    for cmd in [
        Command::SetStripGain {
            id: lead,
            gain_db: -9.0,
        },
        Command::SetStripPan {
            id: lead,
            pan: -0.6,
        },
    ] {
        assert!(
            freeze::refuses(&p, &cmd).is_none(),
            "the pre-fader sender's fader and pan are live"
        );
        let mut thawed = p.clone();
        freeze::thaw(&mut thawed);
        cmd.clone().apply(&mut thawed).unwrap();
        cmd.apply(&mut p).unwrap();
        assert_eq!(
            freeze::content_key(&p, FreezeTarget::Strip(ret)).unwrap(),
            key,
            "the return's render still holds what it hears"
        );
        same_bits(
            &mix(&mut h, &p, 512),
            &mix(&mut h, &thawed, 512),
            "baked, the fader moved",
        );
    }
    // Its width shapes the send: still locked, as its mute and the send itself.
    assert!(freeze::refuses(
        &p,
        &Command::SetStripWidth {
            id: lead,
            width: 0.5
        }
    )
    .is_some());
    let mut send = p.strip(lead).unwrap().sends[0];
    send.tap = SendTap::Post;
    assert!(freeze::refuses(&p, &Command::SetStripSend { id: lead, send }).is_some());
}

#[test]
fn a_post_fader_sender_keeps_its_fader_locked() {
    let mut h = handle("post_fader");
    let mut p = song();
    prepare_to_record(&mut h, &mut p);
    let lead = p.tracks[0].strip.unwrap();
    assert!(freeze::refuses(
        &p,
        &Command::SetStripGain {
            id: lead,
            gain_db: -9.0
        }
    )
    .is_some());
}

// ---- the mix cues (ADR 0093) with freeze and the mode ----------------------------------

/// A cue at `beat` recalling a snapshot of the whole desk as `edit` leaves it.
fn cue_at(p: &mut Project, beat: i64, edit: impl Fn(&mut cantus_core::mix_snap::MixState)) {
    use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
    let mut state = capture(p, &[], false);
    edit(&mut state);
    let sid = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: sid,
        name: format!("at {beat}"),
        state,
    });
    let cid = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cid,
        at: Ticks::beats(beat),
        snapshot: sid,
        scope: Vec::new(),
    });
}

#[test]
fn a_cue_that_flips_or_mutes_a_frozen_strip_plays_as_the_unfrozen_strip_to_the_bit() {
    let mut h = handle("cue_freeze");
    let mut p = song();
    let lead = p.tracks[0].strip.unwrap();
    let bass = p.tracks[1].strip.unwrap();
    // From beat 2 the lead flips its polarity, swaps and moves its EQ; from beat 4 the
    // bass is muted too.
    let flip = move |s: &mut cantus_core::mix_snap::MixState| {
        if let Some(x) = s.strips.iter_mut().find(|x| x.id == lead) {
            x.phase_invert = true;
            x.swap_channels = true;
            x.eq.bands[0].gain_db = 6.0;
        }
    };
    cue_at(&mut p, 2, flip);
    cue_at(&mut p, 4, move |s| {
        flip(s);
        if let Some(x) = s.strips.iter_mut().find(|x| x.id == bass) {
            x.muted = true;
        }
    });
    let live = mix(&mut h, &p, 512);
    // The whole plan baked: the section cues play the live mix, to the bit.
    prepare_to_record(&mut h, &mut p);
    assert!(p.frozen(FreezeTarget::Strip(lead)).is_some());
    for block in [512, 64] {
        same_bits(
            &mix(&mut h, &p, block),
            &live,
            &format!("cues, block {block}"),
        );
    }
}

#[test]
fn a_cue_never_brings_a_live_strip_into_the_baked_master() {
    let mut h = handle("cue_master");
    let mut p = song();
    let bass_track = p.tracks[1].id;
    let bass = p.tracks[1].strip.unwrap();
    p.track_mut(bass_track).unwrap().armed = true;
    // A cue that raises the armed bass strip 6 dB, unmuted, and solos it.
    cue_at(&mut p, 2, move |s| {
        if let Some(x) = s.strips.iter_mut().find(|x| x.id == bass) {
            x.gain_db += 6.0;
            x.muted = false;
            x.soloed = true;
        }
    });
    let plan = prepare_to_record(&mut h, &mut p);
    let live_roots: Vec<FreezeTarget> = plan.live.iter().map(|(t, _)| *t).collect();
    bake_master(&mut h, &p, &live_roots);
    // The armed track muted: the reference alone, which is the live mix without it (and
    // without the solo the cue gives it: nothing of the mix is soloed in a render).
    let mut muted = p.clone();
    muted.track_mut(bass_track).unwrap().muted = true;
    let mut live_without = muted.clone();
    freeze::thaw(&mut live_without);
    for snap in &mut live_without.mix.snapshots {
        for st in &mut snap.state.strips {
            st.soloed = false;
        }
    }
    let key = h.frozen.master().unwrap().to_string();
    h.frozen.set_master(None);
    let reference = mix(&mut h, &live_without, 512);
    h.frozen.set_master(Some(key));
    same_bits(
        &mix(&mut h, &muted, 512),
        &reference,
        "the reference, cue and all",
    );
}

#[test]
fn a_cue_that_solos_a_strip_inside_a_return_keeps_the_return_live_and_the_mix_to_the_bit() {
    let mut h = handle("cue_solo");
    let mut p = song();
    let lead = p.tracks[0].strip.unwrap();
    let ret = p.strips[5].id;
    // The bass sends into the return too: soloing the lead silences the bass's send live,
    // which the return's render (no solo in it) would still hold.
    let bass = p.tracks[1].strip.unwrap();
    p.strip_mut(bass).unwrap().sends.push(Send {
        to: ret,
        level: 0.5,
        enabled: true,
        tap: SendTap::Post,
    });
    // From beat 2 a cue solos the lead, which sends into the reverb return: inside the
    // return's region.
    cue_at(&mut p, 2, move |s| {
        if let Some(x) = s.strips.iter_mut().find(|x| x.id == lead) {
            x.soloed = true;
        }
    });
    let live = mix(&mut h, &p, 512);
    let plan = prepare_to_record(&mut h, &mut p);
    assert!(
        plan.live
            .contains(&(FreezeTarget::Strip(ret), record_mode::Why::CueSolo(lead))),
        "{plan:?}"
    );
    assert!(p.frozen(FreezeTarget::Strip(ret)).is_none());
    assert!(
        p.frozen(FreezeTarget::Strip(lead)).is_some(),
        "the lead itself bakes"
    );
    for block in [512, 64] {
        same_bits(
            &mix(&mut h, &p, block),
            &live,
            &format!("across the cue's solo, block {block}"),
        );
    }
}
