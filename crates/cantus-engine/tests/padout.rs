//! Pulsus III's per-pad outputs on the engine (round 37, ADR 0100): a kit split to strips,
//! every new strip at unity, sums to the unsplit kit **to the bit** — at every block size,
//! offline and in real time, serial and on the pool — and freeze, the recording mode, solo
//! and mute, VCAs, sends, delay compensation and stems all see the new strips.

use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::padout;
use cantus_core::record_mode;
use cantus_core::*;
use cantus_engine::freeze::render::prepare;
use cantus_engine::*;
use std::path::PathBuf;
use std::sync::Arc;

const SR: f64 = 48_000.0;

/// A busy two-bar groove on every pad of the studio kit, overlapping itself.
fn kit_project() -> Project {
    let mut p = Project::new("Kit");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let pitches: Vec<u8> = t
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .map(|x| x.pitch)
        .collect();
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        let pick = |k: i64| pitches[((step * 5 + k * 3) as usize) % pitches.len()];
        for k in 0..1 + (step % 3) {
            let vel = 50 + ((step * 17 + k * 29) % 77) as u8;
            c.notes.push(p.make_note(at, Ticks(PPQ / 3), pick(k), vel));
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// `p` with its kit (track 0) split to strips, as the menu does it.
fn split(p: &Project) -> Project {
    let mut q = p.clone();
    let view = q.clone();
    let cmd = padout::split_with(&view, view.tracks[0].id, |n| q.make_strip(n)).unwrap();
    cmd.apply(&mut q).unwrap();
    q
}

fn pad_strips(p: &Project) -> Vec<StripId> {
    p.tracks[0].instrument.output_strips()
}

/// `p` playing only the notes of pad `k` (by pitch).
fn only_pad(p: &Project, k: usize) -> Project {
    let mut q = p.clone();
    let pitch = q.tracks[0].instrument.pulsus().unwrap().pads[k].pitch;
    for c in &mut q.tracks[0].clips {
        c.notes.retain(|n| n.pitch == pitch);
    }
    q
}

/// `p` without the notes of pad `k`.
fn without_pad(p: &Project, k: usize) -> Project {
    let mut q = p.clone();
    let pitch = q.tracks[0].instrument.pulsus().unwrap().pads[k].pitch;
    for c in &mut q.tracks[0].clips {
        c.notes.retain(|n| n.pitch != pitch);
    }
    q
}

fn render_at(p: &Project, block: usize) -> Vec<f32> {
    render(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            block_frames: block,
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a.iter().zip(b).position(|(x, y)| x != y) {
        panic!("{what}: sample {i} differs: {:e} vs {:e}", a[i], b[i]);
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

fn close(a: &[f32], b: &[f32], tol: f32, what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    let worst = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    assert!(worst <= tol, "{what}: max |d| {worst:e} > {tol:e}");
}

fn peak(a: &[f32]) -> f32 {
    a.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

#[test]
fn a_split_kit_at_unity_sums_to_the_unsplit_kit_to_the_bit() {
    let p = kit_project();
    assert_eq!(
        p.tracks[0].instrument.pulsus().unwrap().version,
        PULSUS_MODEL
    );
    let q = split(&p);
    assert_eq!(pad_strips(&q).len(), 14, "a strip per pad");
    // Under the project's own law (−3 dB, the default): the kit's strip places the kit as
    // before; each pad's strip is a balance.
    assert_eq!(q.pan_law, PanLaw::Minus3dB);
    let whole = render_at(&p, 512);
    assert!(peak(&whole) > 0.1, "the groove sounds");
    same_bits(&render_at(&q, 512), &whole, "split at unity");
    // Block-size independence at tolerance 0, split and unsplit.
    for block in [1usize, 64, 333, 1024, 4096] {
        same_bits(
            &render_at(&q, block),
            &whole,
            &format!("split, block {block}"),
        );
    }
    // Merged back: the kit again, to the bit.
    let mut m = q.clone();
    padout::merge(&q, q.tracks[0].id)
        .unwrap()
        .apply(&mut m)
        .unwrap();
    assert!(m.strips.len() == p.strips.len());
    same_bits(&render_at(&m, 512), &whole, "merged back");
}

#[test]
fn a_pad_strips_fader_moves_that_pad_and_nothing_else() {
    let q = split(&kit_project());
    let k = 2;
    let s = pad_strips(&q)[k];
    let mut down = q.clone();
    down.strip_mut(s).unwrap().gain_db = -12.0;
    let a = render_at(&down, 512);
    let b = render_at(&q, 512);
    // What changed is pad k's sound times (g − 1), nothing else.
    let alone = render_at(&only_pad(&q, k), 512);
    let g = db_to_linear(-12.0) - 1.0;
    let diff: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
    let want: Vec<f32> = alone.iter().map(|x| x * g).collect();
    assert!(peak(&want) > 0.01);
    close(&diff, &want, 2e-6, "the pad's fader");
}

/// A processor driven as the callback drives it, on `workers`.
fn realtime(p: &Project, workers: Workers, len: usize) -> (Vec<f32>, u64) {
    let snap = Arc::new(Snapshot::compile(p, 1));
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
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
        .ok();
    let plan = cantus_engine::graph::compile(&snap);
    tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
    proc.settle();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                count, delay,
            )))
            .ok();
            proc.settle();
        }
    }
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    tx.push(EngineMessage::Play).ok();
    let sizes = [128usize, 333, 512, 96, 1024, 17, 2048];
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len {
        let n = sizes[i % sizes.len()].min((len - out.len()) / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    (out, proc.stats().schedule_conflicts)
}

#[test]
fn offline_is_real_time_and_the_pool_is_the_serial_engine() {
    let p = kit_project();
    let q = split(&p);
    let whole = render_at(&p, 512);
    let len = whole.len().min(4 * 48_000 * 2);
    let (serial, c0) = realtime(&q, Workers::SERIAL, len);
    same_bits(&serial, &whole[..len], "real time, serial");
    for w in [1usize, 3] {
        let (par, c) = realtime(&q, Workers::parallel(w), len);
        same_bits(&par, &whole[..len], &format!("real time on {w} workers"));
        assert_eq!(c, 0, "no borrow failed on {w} workers");
    }
    assert_eq!(c0, 0);
}

#[test]
fn mute_and_solo_reach_the_pad_strips() {
    let q = split(&kit_project());
    // A pad in no choke group: taking its notes away changes no other pad.
    let k = q.tracks[0]
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .position(|x| x.choke == 0 && x.name.contains("Clap"))
        .unwrap();
    let s = pad_strips(&q)[k];
    // The kit's own mute silences every pad strip.
    let mut muted = q.clone();
    muted.tracks[0].muted = true;
    assert_eq!(peak(&render_at(&muted, 512)), 0.0, "a muted kit is silent");
    // A pad strip's mute takes that pad out, and only it.
    let mut one_off = q.clone();
    one_off.strip_mut(s).unwrap().muted = true;
    close(
        &render_at(&one_off, 512),
        &render_at(&without_pad(&q, k), 512),
        // Within rounding: without its notes, the other pads' voices take other slots of
        // the pool, and a pad sums its own voices in slot order.
        1e-6,
        "a pad strip muted",
    );
    // A pad strip soloed: that pad alone, through the kit's strip.
    let mut solo = q.clone();
    solo.strip_mut(s).unwrap().soloed = true;
    close(
        &render_at(&solo, 512),
        &render_at(&only_pad(&q, k), 512),
        // Within rounding: without its notes, the other pads' voices take other slots of
        // the pool, and a pad sums its own voices in slot order.
        1e-6,
        "a pad strip soloed",
    );
    // The kit's strip soloed keeps every pad (they feed it).
    let mut kit_solo = q.clone();
    let kit_strip = q.tracks[0].strip.unwrap();
    kit_solo.strip_mut(kit_strip).unwrap().soloed = true;
    same_bits(
        &render_at(&kit_solo, 512),
        &render_at(&q, 512),
        "the kit's strip soloed",
    );
}

#[test]
fn a_vca_on_a_pad_strip_is_its_fader() {
    let q = split(&kit_project());
    let s = pad_strips(&q)[1];
    let mut by_fader = q.clone();
    by_fader.strip_mut(s).unwrap().gain_db = -9.0;
    let mut by_vca = q.clone();
    let mut vca = by_vca.make_strip("VCA");
    vca.kind = cantus_core::vca::StripKind::Vca;
    vca.gain_db = -9.0;
    let vid = vca.id;
    by_vca.strips.push(vca);
    by_vca.strip_mut(s).unwrap().vcas.push(vid);
    same_bits(
        &render_at(&by_vca, 512),
        &render_at(&by_fader, 512),
        "a VCA over a pad strip",
    );
}

#[test]
fn a_send_from_a_pad_strip_carries_that_pad() {
    let q = split(&kit_project());
    let k = 3;
    let s = pad_strips(&q)[k];
    let kit_strip = q.tracks[0].strip.unwrap();
    let with_send = |p: &Project| {
        let mut p = p.clone();
        let mut ret = p.make_strip("Return");
        ret.gain_db = -3.0;
        let rid = ret.id;
        p.strips.push(ret);
        p.strip_mut(s).unwrap().sends.push(Send {
            to: rid,
            level: 0.5,
            enabled: true,
            tap: SendTap::Post,
        });
        // Only the return is heard: the kit's strip muted.
        p.strip_mut(kit_strip).unwrap().muted = true;
        p
    };
    let all = render_at(&with_send(&q), 512);
    assert!(peak(&all) > 0.001, "the return hears the pad");
    same_bits(
        &all,
        &render_at(&with_send(&only_pad(&q, k)), 512),
        "the send carries that pad alone",
    );
}

#[test]
fn delay_compensation_keeps_the_other_pads_aligned_with_a_latent_pad_strip() {
    let p = kit_project();
    let k = 0;
    // A linear-phase EQ on pad k's strip: thousands of samples late.
    let mut q = split(&without_pad(&p, k));
    let s = pad_strips(&q)[k];
    let mut eq = q.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0);
    q.strip_mut(s).unwrap().effects.push(eq);
    let compensated = |p: &Project| {
        render(
            Arc::new(Snapshot::compile(p, 1)),
            &RenderOptions {
                block_frames: 512,
                tail_seconds: 1.0,
                compensate_latency: true,
                ..Default::default()
            },
        )
        .unwrap()
        .samples
    };
    // Every other pad waits for it and comes out where it did without it.
    same_bits(
        &compensated(&q),
        &compensated(&without_pad(&p, k)),
        "compensated around a latent pad strip",
    );
}

/// The same with a pad's chain (the kit not split, r37:drums review 2): a linear-phase EQ
/// in the chain of a pad that does not play — every other pad waits for it and comes out
/// where it did without the chain.
#[test]
fn delay_compensation_keeps_the_other_pads_aligned_with_a_latent_pads_chain() {
    let p = kit_project();
    let k = 0;
    let mut q = without_pad(&p, k);
    let t = q.tracks[0].id;
    let pad = q.tracks[0].instrument.pulsus().unwrap().pads[k].id;
    let mut eq = q.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0);
    let view = q.clone();
    let cmd = padout::chain_add(&view, t, pad, eq, |n| q.make_strip(n)).unwrap();
    cmd.apply(&mut q).unwrap();
    let compensated = |p: &Project| {
        render(
            Arc::new(Snapshot::compile(p, 1)),
            &RenderOptions {
                block_frames: 512,
                tail_seconds: 1.0,
                compensate_latency: true,
                ..Default::default()
            },
        )
        .unwrap()
        .samples
    };
    // The chain's latency counts: uncompensated, the other pads come out late.
    assert_ne!(
        render_at(&q, 512),
        render_at(&without_pad(&p, k), 512),
        "the chain is latent and every other pad waits for it"
    );
    same_bits(
        &compensated(&q),
        &compensated(&without_pad(&p, k)),
        "compensated around a latent pad's chain",
    );
}

fn folder(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("padout")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn handle(name: &str) -> EngineHandle {
    let (mut h, _p) = EngineHandle::manual(SR);
    h.frozen.set_app_dir(folder(name));
    h
}

fn mix(h: &mut EngineHandle, p: &Project, block: usize) -> Vec<f32> {
    let snap = h.compile_for_render(p, 1);
    render(
        snap,
        &RenderOptions {
            block_frames: block,
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn freeze_now(h: &mut EngineHandle, p: &mut Project, target: FreezeTarget) {
    let job = prepare(h, p, target).expect("prepared");
    job.run(Vec::new(), Vec::new(), &RenderProgress::default())
        .expect("rendered");
    h.frozen.forget_missing();
    freeze::freeze_commands(p, target)
        .unwrap()
        .apply(p)
        .unwrap();
}

#[test]
fn freezing_the_kits_strip_or_a_pad_strip_plays_the_live_mix_to_the_bit() {
    let mut h = handle("freeze");
    let q = split(&kit_project());
    let live = mix(&mut h, &q, 512);
    // The kit's strip: its region holds every pad strip and the kit.
    let kit_strip = q.tracks[0].strip.unwrap();
    let r = freeze::region(&q, FreezeTarget::Strip(kit_strip));
    assert!(pad_strips(&q).iter().all(|s| r.strips.contains(s)));
    let mut a = q.clone();
    freeze_now(&mut h, &mut a, FreezeTarget::Strip(kit_strip));
    let snap = h.compile_for_render(&a, 1);
    assert!(
        snap.tracks.iter().all(|t| t.rest),
        "the kit and its taps rest while the render plays"
    );
    same_bits(&mix(&mut h, &a, 512), &live, "the kit's strip frozen");
    // One pad's strip: the kit keeps playing the others.
    let s = pad_strips(&q)[2];
    let mut b = q.clone();
    freeze_now(&mut h, &mut b, FreezeTarget::Strip(s));
    let snap = h.compile_for_render(&b, 1);
    assert!(!snap.tracks[0].rest, "the kit runs for its other pads");
    same_bits(&mix(&mut h, &b, 512), &live, "a pad strip frozen");
    for block in [64usize, 1024] {
        same_bits(
            &mix(&mut h, &b, block),
            &live,
            &format!("frozen, block {block}"),
        );
    }
    // The kit's own track cannot be frozen: its sound is several outputs.
    assert_eq!(
        freeze::freeze_commands(&q, FreezeTarget::Track(q.tracks[0].id)).err(),
        Some(freeze::CannotFreeze::SplitKit(q.tracks[0].id))
    );
    assert_eq!(
        q.freeze_target_of_track(q.tracks[0].id),
        Some(FreezeTarget::Strip(kit_strip)),
        "Freeze on the kit freezes its strip"
    );
}

#[test]
fn the_recording_mode_bakes_a_split_kit_to_the_bit() {
    let mut h = handle("record");
    let mut q = split(&kit_project());
    let live = mix(&mut h, &q, 512);
    let plan = record_mode::plan(&q);
    assert!(plan.live.is_empty(), "{plan:?}");
    assert_eq!(
        plan.bake,
        vec![FreezeTarget::Strip(q.tracks[0].strip.unwrap())],
        "the kit's strip, with its pads inside"
    );
    for target in plan.bake.clone() {
        freeze_now(&mut h, &mut q, target);
    }
    same_bits(&mix(&mut h, &q, 512), &live, "baked");
    // A split kit straight to the master: its pad strips are baked, the track stays live.
    let mut straight = kit_project();
    straight.tracks[0].strip = None;
    let straight = split(&straight);
    let plan = record_mode::plan(&straight);
    assert_eq!(plan.bake.len(), 14, "every pad strip: {plan:?}");
    assert_eq!(
        plan.live,
        vec![(
            FreezeTarget::Track(straight.tracks[0].id),
            record_mode::Why::SplitKit(straight.tracks[0].id)
        )]
    );
}

#[test]
fn stems_see_the_pad_strips_and_the_kit_track_holds_all_its_pads() {
    let p = kit_project();
    let q = split(&p);
    let snap = Arc::new(Snapshot::compile(&q, 1));
    let opts = StemOptions {
        render: RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut sources: Vec<StemSpec> = (0..q.strips.len())
        .map(|i| StemSpec {
            source: StemSource::Strip(i),
            file_name: format!("s{i}"),
        })
        .collect();
    sources.push(StemSpec {
        source: StemSource::Track(0),
        file_name: "kit".into(),
    });
    sources.push(StemSpec {
        source: StemSource::Master,
        file_name: "master".into(),
    });
    let stems = render_stems(snap, &opts, &sources).unwrap();
    let get = |name: &str| {
        stems
            .iter()
            .find(|s| s.spec.file_name == name)
            .unwrap()
            .samples
            .clone()
    };
    for s in pad_strips(&q) {
        let i = q.strip_index(s).unwrap();
        assert!(
            peak(&get(&format!("s{i}"))) > 0.0,
            "pad strip {i} has a stem"
        );
    }
    // The kit track's stem is every pad: the unsplit kit's track stem, within rounding.
    let unsplit = render_stems(
        Arc::new(Snapshot::compile(&p, 1)),
        &opts,
        &[StemSpec {
            source: StemSource::Track(0),
            file_name: "kit".into(),
        }],
    )
    .unwrap();
    close(
        &get("kit"),
        &unsplit[0].samples,
        1e-6,
        "the kit's track stem",
    );
    // And the master is the master.
    same_bits(
        &get("master")[..],
        &render_at(&p, 512)[..get("master").len()],
        "master stem",
    );
}

/// The studio kit and two more pads: sixteen, every one sounding in the groove.
fn sixteen_pads() -> Project {
    let mut p = kit_project();
    {
        let kit = p.tracks[0].instrument.pulsus_mut().unwrap();
        let mut a = DrumPad::new(54, "Tambourine", PadVoice::HiHat);
        a.model = 1;
        let mut b = DrumPad::new(60, "Hi bongo", PadVoice::Tom);
        b.tune = 12.0;
        kit.pads.push(a);
        kit.pads.push(b);
        kit.pads.sort_by_key(|x| x.pitch);
        kit.assign_ids();
    }
    let mut c = p.make_clip("More", Ticks::ZERO, Ticks::beats(8));
    for step in 0..16i64 {
        let pitch = if step % 2 == 0 { 54 } else { 60 };
        c.notes
            .push(p.make_note(Ticks(step * PPQ / 2), Ticks(PPQ / 4), pitch, 90));
    }
    p.tracks[0].clips.push(c);
    p
}

/// Every pad's chain gets an insert, as the pad's card does it.
fn chain_every_pad(p: &Project) -> Project {
    let mut q = p.clone();
    let t = q.tracks[0].id;
    let pads: Vec<PadId> = q.tracks[0]
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .map(|x| x.id)
        .collect();
    for (i, pad) in pads.into_iter().enumerate() {
        let kind = [
            EffectKind::Eq,
            EffectKind::Compressor,
            EffectKind::Saturator,
        ][i % 3];
        let mut fx = q.make_effect(kind);
        cantus_core::starting::apply(&mut fx.spec);
        let view = q.clone();
        let cmd = padout::chain_add(&view, t, pad, fx, |n| q.make_strip(n)).unwrap();
        cmd.apply(&mut q).unwrap();
    }
    q
}

/// The owner's bar: sixteen pads, each with its own chain, split to sixteen strips in one
/// click — and the split kit sums to the unsplit one to the bit.
#[test]
fn sixteen_chained_pads_split_to_sixteen_strips_sum_to_the_kit_to_the_bit() {
    let chained = chain_every_pad(&sixteen_pads());
    let t = chained.tracks[0].id;
    let unsplit = render_at(&chained, 512);
    assert!(peak(&unsplit) > 0.1);
    let q = split(&chained);
    let k = q.tracks[0].instrument.pulsus().unwrap();
    assert_eq!(k.pads.len(), 16);
    let own: Vec<StripId> = k
        .pads
        .iter()
        .filter_map(|x| padout::heard_on(&q, &q.tracks[0], x))
        .collect();
    assert_eq!(own.len(), 16, "sixteen strips, one per pad");
    assert!(padout::is_split(&q, t));
    same_bits(&render_at(&q, 512), &unsplit, "sixteen chained pads split");
    same_bits(&render_at(&q, 333), &unsplit, "split, 333 frames");
    // And through the real-time engine on the pool.
    let len = unsplit.len().min(2 * 48_000 * 2);
    let (par, c) = realtime(&q, Workers::parallel(3), len);
    same_bits(&par, &unsplit[..len], "on the pool");
    assert_eq!(c, 0);
}

/// A chain at rest costs nothing: a pad that does not play leaves its chain resting —
/// counted, and not one sample different from the chain running.
#[test]
fn a_chain_at_rest_costs_nothing() {
    let p = kit_project();
    let chained = chain_every_pad(&p);
    // Only the kick plays: every other chain rests.
    let kick = chained.tracks[0].instrument.pulsus().unwrap().pads[0].pitch;
    let mut only_kick = chained.clone();
    for c in &mut only_kick.tracks[0].clips {
        c.notes.retain(|n| n.pitch == kick);
    }
    let len = 2 * 48_000 * 2;
    let (out, _) = realtime(&only_kick, Workers::SERIAL, len);
    assert!(peak(&out) > 0.05);
    let rests = chain_rests(&only_kick, len);
    let passes = (len / 2 / 1024) as u64;
    assert!(
        rests >= 13 * passes,
        "the thirteen silent chains rest every pass: {rests} rests in {passes} passes"
    );
    // Timed: processing only (the units built first), the kit with every chain and only
    // the kick playing, against the same kit with no chain at all.
    let mut bare = p.clone();
    for c in &mut bare.tracks[0].clips {
        c.notes.retain(|n| n.pitch == kick);
    }
    let best = |p: &Project| {
        (0..3)
            .map(|_| processing_time(p, len))
            .fold(f64::MAX, f64::min)
    };
    let with = best(&only_kick);
    let without = best(&bare);
    eprintln!(
        "chains at rest: {with:.4} s processing with thirteen resting chains (one running),          {without:.4} s with no chain"
    );
    // One chain runs (the kick's): what is left is its cost and the taps' copies.
    assert!(
        with < without * 3.0 + 0.02,
        "{with:.4} s against {without:.4} s"
    );
}

/// Seconds spent in `process` for `len` samples (the processor and its units built first).
fn processing_time(p: &Project, len: usize) -> f64 {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, SR as f32,
        ))))
        .ok();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).ok();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    tx.push(EngineMessage::Play).ok();
    let mut buf = vec![0f32; 512 * 2];
    proc.process(&mut buf, 2);
    while rx.pop().is_ok() {}
    let t0 = std::time::Instant::now();
    let mut done = 0;
    while done < len {
        proc.process(&mut buf, 2);
        done += buf.len();
        while rx.pop().is_ok() {}
    }
    t0.elapsed().as_secs_f64()
}

/// How many strip passes a pad's chain rested over `len` samples, playing live.
fn chain_rests(p: &Project, len: usize) -> u64 {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, SR as f32,
        ))))
        .ok();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).ok();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    tx.push(EngineMessage::Play).ok();
    let mut done = 0;
    while done < len {
        let mut buf = vec![0f32; 1024 * 2];
        proc.process(&mut buf, 2);
        done += buf.len();
        while rx.pop().is_ok() {}
    }
    proc.stats().chain_rests
}

/// Split = unsplit to the bit holds with the kit's own level and balance at unity (a
/// product of a sum is not the sum of the products); at any other level, and under an
/// automated level, the two agree within rounding.
#[test]
fn at_a_kit_level_off_unity_the_split_agrees_within_rounding() {
    let mut p = kit_project();
    p.tracks[0].gain_db = -4.5;
    p.tracks[0].pan = 0.3;
    let q = split(&p);
    let a = render_at(&p, 512);
    let b = render_at(&q, 512);
    close(&a, &b, 1e-6, "a kit level of −4.5 dB, balance 0.3");
    // An automated level.
    let mut r = kit_project();
    let id = r.tracks[0].id;
    let ride = r.make_automation_clip(
        "ride",
        cantus_core::automation::AutoTarget::TrackGain(id),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            cantus_core::automation::AutoPoint::new(Ticks::ZERO, 0.9),
            cantus_core::automation::AutoPoint::new(Ticks::beats(6), 0.2),
        ],
    );
    r.tracks[0].clips.push(ride);
    let s = split(&r);
    close(
        &render_at(&r, 512),
        &render_at(&s, 512),
        1e-6,
        "an automated kit level",
    );
}

/// Resting changes no sample: the very same chains as ordinary strips (which never rest)
/// render the same bits — chains rest, wake on their pad's first hit, and rest again.
#[test]
fn a_resting_chain_is_exact() {
    let mut chained = chain_every_pad(&kit_project());
    // Every pad waits a bar before it plays: its chain rests, then wakes.
    for c in &mut chained.tracks[0].clips {
        for n in &mut c.notes {
            n.start += Ticks::beats(4);
        }
    }
    let mut plain = chained.clone();
    for s in &mut plain.strips {
        if s.kind == cantus_core::vca::StripKind::Chain {
            s.kind = cantus_core::vca::StripKind::Audio;
        }
    }
    let rests = chain_rests(&chained, 4 * 48_000 * 2);
    assert!(rests > 0, "the chains rested");
    assert_eq!(chain_rests(&plain, 4 * 48_000 * 2), 0);
    for block in [512usize, 333] {
        same_bits(
            &render_at(&chained, block),
            &render_at(&plain, block),
            &format!("resting chains against running ones, block {block}"),
        );
    }
}

/// A factory kit with its calibrated gain (ADR 0100 §8), split to strips: still the kit to
/// the bit — the gain is on each lane before the sum.
#[test]
fn a_calibrated_factory_kit_split_is_still_itself_to_the_bit() {
    let mut p = kit_project();
    let house = PulsusParams::factory()
        .into_iter()
        .find(|f| f.label_key == "pulsus.kit.house")
        .unwrap()
        .kit;
    assert!(house.kit.gain_db > 0.0, "the house kit is calibrated");
    p.tracks[0].instrument = InstrumentSpec::Pulsus(house);
    let whole = render_at(&p, 512);
    assert!(peak(&whole) > 0.05);
    same_bits(
        &render_at(&split(&p), 512),
        &whole,
        "the calibrated kit split",
    );
}
