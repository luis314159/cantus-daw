//! Pre-fader sends (round 25, piece F; design §7.3, ADR 0041 §2): null tests first.
//!
//! A pre-fader send taps the strip after its inserts, EQ, width and swap — **before** the
//! fader and the pan law — and mute and solo still silence it. A post-fader send renders
//! exactly as it did before the tap existed (the golden below was computed on the
//! contract commit `57f3ccc`, before the engine knew about taps).

mod support;
use cantus_core::automation::{AutoPoint, AutoTarget};
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, compare, peak};
use cantus_engine::*;
use std::sync::Arc;
use support::*;

const SR: f64 = 48_000.0;

fn send(to: StripId, level: f32, tap: SendTap) -> Send {
    Send {
        to,
        level,
        enabled: true,
        tap,
    }
}

fn ramp(p: &mut Project, target: AutoTarget, a: f32, b: f32) {
    let c = p.make_automation_clip(
        "ride",
        target,
        Ticks::ZERO,
        Ticks::beats(6),
        vec![
            AutoPoint::new(Ticks::ZERO, a),
            AutoPoint::new(Ticks::beats(6), b),
        ],
    );
    p.tracks[0].clips.push(c);
}

fn sha(samples: &[f32]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for s in samples {
        h.update(s.to_le_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Every way a post-fader send is fed: undelayed at a level, through a compensation line
/// (its source's output into a latent bus), from a fader under automation (the ramp scales
/// the pair itself), from a muted strip, switched off. With `tap`, every send so tapped.
fn sends_project(tap: SendTap) -> Project {
    let mut p = demo_project();
    let (lead, bass) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    let (aux, ret, bus, quiet) = (
        p.strips[3].id,
        p.strips[4].id,
        p.strips[5].id,
        p.strips[6].id,
    );
    let delay = p.make_effect(EffectKind::Delay);
    p.strip_mut(aux).unwrap().effects.push(delay);
    let space = p.make_effect(EffectKind::Space);
    p.strip_mut(ret).unwrap().effects.push(space);
    // A look-ahead limiter on the bus: the bass's sends must wait for it (lines).
    let lim = p.make_effect(EffectKind::Limiter);
    p.strip_mut(bus).unwrap().effects.push(lim);
    p.strip_mut(bass).unwrap().output = Some(bus);
    let eq = {
        let mut e = p.make_effect(EffectKind::Eq);
        e.spec.set("b0.used", 1.0).unwrap();
        e.spec.set("b0.gain", 5.0).unwrap();
        e
    };
    p.strip_mut(lead).unwrap().effects.push(eq);
    p.strip_mut(lead).unwrap().width = 0.6;
    p.strip_mut(lead).unwrap().sends = vec![send(aux, 0.5, tap), send(ret, 0.8, tap)];
    p.strip_mut(bass).unwrap().gain_db = -4.5;
    p.strip_mut(bass).unwrap().pan = 0.4;
    p.strip_mut(bass).unwrap().sends = vec![send(ret, 0.7, tap), {
        let mut s = send(aux, 0.9, tap);
        s.enabled = false;
        s
    }];
    // A muted strip with a send: nothing from it, either tap.
    p.strip_mut(quiet).unwrap().muted = true;
    p.strip_mut(quiet).unwrap().sends = vec![send(aux, 1.0, tap)];
    ramp(&mut p, AutoTarget::StripGain(lead), 0.3, 0.9);
    ramp(&mut p, AutoTarget::StripPan(lead), 0.2, 0.8);
    p
}

fn render_at(p: &Project, block: usize) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.5,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .unwrap()
        .samples
}

/// The render of [`sends_project`] with every send post-fader, on the contract commit
/// (57f3ccc, `DSP_VERSION` 3), before the engine read the tap.
const GOLDEN_POST_SENDS: &str = "3af01508d64d8685bab2025c1f49201b4d2327e97c3bf94b281e4c0226701960";

#[test]
fn a_post_send_renders_exactly_as_it_did_before_the_tap() {
    let out = render_at(&sends_project(SendTap::Post), 512);
    assert!(peak(&out) > 0.05, "silent render");
    assert_eq!(
        sha(&out),
        GOLDEN_POST_SENDS,
        "post-fader sends no longer render as they did (peak {})",
        peak(&out)
    );
}

/// The lead alone, its output into a muted sink, so the master hears only what its send
/// carries into the aux.
fn only_the_send(tap: SendTap) -> (Project, StripId) {
    let mut p = demo_project();
    p.tracks[1].muted = true;
    let lead = p.tracks[0].strip.unwrap();
    let (aux, sink) = (p.strips[3].id, p.strips[6].id);
    p.strip_mut(sink).unwrap().muted = true;
    let delay = p.make_effect(EffectKind::Delay);
    p.strip_mut(aux).unwrap().effects.push(delay);
    let s = p.strip_mut(lead).unwrap();
    s.output = Some(sink);
    s.sends = vec![send(aux, 0.5, tap)];
    (p, lead)
}

/// A change to the project, named.
type Move = (&'static str, Box<dyn Fn(&mut Project)>);

#[test]
fn a_pre_send_does_not_follow_its_fader_or_its_pan() {
    let (p, lead) = only_the_send(SendTap::Pre);
    let reference = render_of(&p);
    assert!(peak(&reference) > 0.02, "the pre send carried nothing");
    let moves: [Move; 5] = [
        (
            "fader up",
            Box::new(move |p| p.strip_mut(lead).unwrap().gain_db = 6.0),
        ),
        (
            "fader down",
            Box::new(move |p| p.strip_mut(lead).unwrap().gain_db = -40.0),
        ),
        (
            "fader closed",
            Box::new(move |p| p.strip_mut(lead).unwrap().gain_db = -200.0),
        ),
        (
            "pan",
            Box::new(move |p| p.strip_mut(lead).unwrap().pan = 0.9),
        ),
        (
            "fader and pan automated",
            Box::new(move |p| {
                ramp(p, AutoTarget::StripGain(lead), 0.1, 0.95);
                ramp(p, AutoTarget::StripPan(lead), 0.0, 1.0);
            }),
        ),
    ];
    for (what, change) in moves {
        let mut q = p.clone();
        change(&mut q);
        assert_null(&render_of(&q), &reference, 0.0, what);
    }
    // The same send post-fader does follow the fader: the test can tell.
    let (mut post, lead) = only_the_send(SendTap::Post);
    let before = render_of(&post);
    post.strip_mut(lead).unwrap().gain_db = -12.0;
    assert!(compare(&render_of(&post), &before).max_abs_diff > 1e-3);
}

/// At a centred pan a pre-fader send *is* the post-fader one with the fader's gain moved
/// into its level — to the bit: the post send scales the pair by `(gain · law) · level`,
/// the pre send by its level alone, one multiplication each.
///
/// The design's literal case — a 0 dB fader, centre pan and the 0 dB law, pre equal to
/// post — is this with a law gain of 1. In `f32` the 0 dB law's centre is
/// `cos(π/4)·√2 = 0.99999994`, one step under unity (pinned below), so there the two
/// differ by that step (≤ 6e-8 of the signal) unless the pre send's level carries it, as
/// here. Every fader and law at centre is checked the same way.
#[test]
fn at_a_0_db_fader_centre_pan_and_the_0_db_law_pre_is_post_to_the_bit() {
    assert_eq!(
        PanLaw::ZeroDb.gains(0.0),
        (0.999_999_94, 0.999_999_94),
        "the 0 dB law's centre moved: the note above (and maybe this test) is stale"
    );
    let centred = |tap: SendTap, law: PanLaw, gain_db: f32| {
        let mut p = sends_project(tap);
        p.pan_law = law;
        // No rides: the faders and pans hold for the whole render.
        p.tracks[0].clips.retain(|c| c.automation.is_none());
        let fader = db_to_linear(gain_db) * law.gains(0.0).0;
        for s in p.strips.iter_mut() {
            s.gain_db = gain_db;
            s.pan = 0.0;
            if tap == SendTap::Pre {
                for x in s.sends.iter_mut() {
                    x.level *= fader;
                }
            }
        }
        p
    };
    for (law, gain_db) in [
        (PanLaw::ZeroDb, 0.0),
        (PanLaw::Minus6dB, -4.5),
        (PanLaw::Minus3dB, 0.0),
    ] {
        let post = render_at(&centred(SendTap::Post, law, gain_db), 512);
        assert!(peak(&post) > 0.01, "{law:?}: silent");
        assert_null(
            &render_at(&centred(SendTap::Pre, law, gain_db), 512),
            &post,
            0.0,
            &format!("pre against post, {law:?} at {gain_db} dB"),
        );
    }
}

#[test]
fn a_pre_send_renders_the_same_at_every_block_size() {
    let p = sends_project(SendTap::Pre);
    let reference = render_at(&p, 512);
    assert!(peak(&reference) > 0.05);
    for block in [7usize, 1, 64, 1000] {
        assert_null(
            &render_at(&p, block),
            &reference,
            0.0,
            &format!("block {block}"),
        );
    }
    // And it is not the post-fader render: the tap reached the engine.
    let post = render_at(&sends_project(SendTap::Post), 512);
    assert!(compare(&reference, &post).max_abs_diff > 1e-4);
}

/// Drive a processor as the application does — units, automation, the plan, compensation
/// and memory answered — at irregular block sizes.
fn play_live(project: &Project, frames: usize) -> Vec<f32> {
    let mut snap = Snapshot::compile(project, 1);
    snap.loop_region = None;
    snap.metronome = false;
    let snap = Arc::new(snap);
    let effects = snap.effect_slots().count();
    let instruments = cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false);
    let (mut tx, rx_engine) =
        rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
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
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut out = Vec::with_capacity(frames * 2);
    let mut buf = vec![0f32; MAX_BLOCK_FRAMES * 2];
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let b = &mut buf[..n * 2];
        p.process(b, 2);
        out.extend_from_slice(b);
        while rx.pop().is_ok() {}
    }
    out
}

#[test]
fn a_pre_send_renders_the_same_in_real_time_as_offline() {
    let p = sends_project(SendTap::Pre);
    let offline = render_at(&p, 256);
    let live = play_live(&p, offline.len() / 2);
    assert_null(&offline, &live, 0.0, "real time against offline");
}

#[test]
fn mute_and_solo_silence_a_pre_send() {
    let (mut p, lead) = only_the_send(SendTap::Pre);
    assert!(peak(&render_of(&p)) > 0.02);
    p.strip_mut(lead).unwrap().muted = true;
    assert_eq!(
        peak(&render_of(&p)),
        0.0,
        "a muted strip leaked through its pre send"
    );
    // Solo: `a_solo_elsewhere_silences_a_pre_send_into_a_destination_it_keeps` below.
    // Muted under automation (the ramp path): silent too.
    let (mut p, lead) = only_the_send(SendTap::Pre);
    ramp(&mut p, AutoTarget::StripGain(lead), 0.2, 0.9);
    p.strip_mut(lead).unwrap().muted = true;
    assert_eq!(peak(&render_of(&p)), 0.0, "a muted, ridden strip leaked");
}

/// Solo silences a pre send too (owner's answer 4, as amended) — proven where it can fail
/// (finding 21). Solo is FL's *solo with connected routes* (`Project::connected_strips`):
/// the soloed strip, everything it feeds and everything feeding it keep sounding. So the
/// case that tells is a soloed strip X whose output goes into the aux the lead's pre send
/// also feeds: the aux keeps sounding (X reaches the master through it), and the lead,
/// connected to X by nothing, is silenced — its pre send with it. Held: the solo render
/// nulls, at tolerance 0, against the same solo with the lead's send removed, and that
/// render is not silent (X is heard through the aux); while without the solo the lead's
/// pre send does reach the aux (the comparison could tell).
#[test]
fn a_solo_elsewhere_silences_a_pre_send_into_a_destination_it_keeps() {
    let (mut p, lead) = only_the_send(SendTap::Pre);
    let aux = p.strips[3].id;
    let sink = p.strips[6].id;
    p.tracks[1].muted = false;
    let x = p.tracks[1].strip.unwrap();
    assert!(
        x != lead && x != aux && x != sink,
        "X is a strip of its own"
    );
    p.strip_mut(x).unwrap().output = Some(aux);
    let without_send = |p: &Project| {
        let mut q = p.clone();
        q.strip_mut(lead).unwrap().sends.clear();
        q
    };
    // Not soloed: the lead's pre send is heard in the aux beside X.
    let both = render_of(&p);
    assert!(
        compare(&both, &render_of(&without_send(&p))).max_abs_diff > 1e-2,
        "the lead's pre send carried nothing into the aux: the test proves nothing"
    );
    // X soloed: the aux is kept (X feeds it), the lead is not — nor is its pre send.
    p.strip_mut(x).unwrap().soloed = true;
    let resolved = Snapshot::compile(&p, 1);
    let index = |id: StripId| p.strip_index(id).unwrap();
    assert!(
        !resolved.strips[index(aux)].muted && resolved.strips[index(lead)].muted,
        "the solo keeps the aux and silences the lead"
    );
    let soloed = render_of(&p);
    let only_x = render_of(&without_send(&p));
    assert!(
        peak(&only_x) > 0.02,
        "the soloed path through the aux is silent"
    );
    assert_null(
        &soloed,
        &only_x,
        0.0,
        "a strip soloed away leaked through its pre send into a kept destination",
    );
}
