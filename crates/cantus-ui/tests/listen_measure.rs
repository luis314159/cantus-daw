//! Listening and the mastering assistant measured on the demo songs (round 34, ADRs
//! 0083–0084): `cargo test --release -p cantus-ui --test listen_measure -- --ignored
//! --nocapture`. What the reports of round 34 quote; ignored in CI (minutes of renders).

use cantus_core::{Snapshot, Ticks};
use cantus_engine::listen::{listen, Checkpoints, ListenOptions, Resumed};
use cantus_ui::ai_tools::listen::{mastering, run, targets};
use std::time::Instant;

const SR: u32 = 48_000;

fn sections(p: &cantus_core::Project) -> Vec<Ticks> {
    cantus_ui::ai_tools::listen::listening::section_starts(p)
}

/// The showcase: a whole render taking checkpoints, then a late section from its
/// checkpoint (time, frames, the same bits), and the same section pre-rolled 2–16 s
/// (how far from exact, in dB under the section's peak).
#[test]
#[ignore]
fn checkpoints_and_preroll_on_the_showcase() {
    let d = cantus_ui::demos::find("heart").unwrap();
    let p = (d.build)();
    let snap = Snapshot::compile(&p, 1);
    let secs = sections(&p);
    let opts = |start: Ticks| ListenOptions {
        sample_rate: SR,
        start,
        tail_seconds: 0.0,
        keep_audio: true,
        sections: secs.clone(),
        ..Default::default()
    };
    let mut cps = Checkpoints::default();
    let t0 = Instant::now();
    let full = listen(
        &snap,
        &opts(Ticks::ZERO),
        vec![],
        vec![],
        &mut cps,
        &mut |_| true,
    )
    .unwrap();
    let whole_s = t0.elapsed().as_secs_f64();
    println!(
        "whole song: {:.1} s of audio in {:.1} s, {} checkpoints",
        full.audio.len() as f64 / 2.0 / SR as f64,
        whole_s,
        full.checkpoints_taken
    );
    let late = *secs.iter().rev().nth(1).unwrap();
    let t1 = Instant::now();
    let h = listen(&snap, &opts(late), vec![], vec![], &mut cps, &mut |_| true).unwrap();
    let from_s = t1.elapsed().as_secs_f64();
    let s0 = cantus_engine::transport::sample_at_map(&snap.tempo, 0.0, late.0 as f64, SR as f64)
        as usize;
    let same = h.audio == full.audio[s0 * 2..s0 * 2 + h.audio.len()];
    println!(
        "from the checkpoint at bar {}: {:.1} s rendered in {:.2} s ({:?}), identical to the whole render: {same}",
        p.tempo_map().bbt(late).0,
        h.rendered_frames as f64 / SR as f64,
        from_s,
        h.resumed
    );
    assert!(same && matches!(h.resumed, Resumed::Checkpoint(_)));
    let peak = h.audio.iter().fold(0f32, |m, x| m.max(x.abs()));
    for pre in [2.0, 4.0, 8.0, 16.0] {
        // A hosted unit stands in so the engine cannot be copied: the range pre-rolls.
        let slot = p.master.effects[0].clone();
        let unit = cantus_engine::effects::EffectUnit::new(&slot, SR as f32);
        let o = ListenOptions {
            preroll_seconds: pre,
            sections: Vec::new(),
            ..opts(late)
        };
        let r = listen(
            &snap,
            &o,
            vec![unit],
            vec![],
            &mut Checkpoints::default(),
            &mut |_| true,
        )
        .unwrap();
        let worst = r
            .audio
            .iter()
            .zip(&full.audio[s0 * 2..])
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        println!(
            "pre-roll {pre:>4.1} s: largest difference {:.1} dB under the section's peak",
            20.0 * (peak / worst.max(1e-12)).log10()
        );
    }
}

/// The assistant on every demo, for three platforms: the proposal's trial and a real
/// render of the song with the chain in place, against the target and the ceiling.
#[test]
#[ignore]
fn the_assistant_on_the_demo_songs() {
    let mut worst_lu = 0f64;
    let mut worst_tp = f64::NEG_INFINITY;
    for d in cantus_ui::demos::ALL.iter() {
        let p = (d.build)();
        for id in ["spotify", "apple_music", "club"] {
            let platform = targets::platform(id).unwrap();
            let heard = run::assist_project(&p, true);
            let snap = Snapshot::compile(&heard, 1);
            let mut cps = Checkpoints::default();
            let o = ListenOptions {
                sample_rate: SR,
                tail_seconds: 0.0,
                keep_audio: true,
                ..Default::default()
            };
            let t0 = Instant::now();
            let mix = listen(&snap, &o, vec![], vec![], &mut cps, &mut |_| true).unwrap();
            let mut scratch = p.clone();
            let target = mastering::Target::genre(platform, &targets::CURVES[0]);
            let prop = mastering::propose(
                &mix.audio,
                SR,
                &target,
                &mastering::Post::of(&p.master),
                &mut |k| scratch.make_effect(k),
                &|| false,
            )
            .unwrap();
            let think = t0.elapsed().as_secs_f64();
            let mut q = p.clone();
            q.master.effects = prop.chain.clone();
            q.master.fx_enabled = true;
            let out = listen(
                &Snapshot::compile(&q, 2),
                &ListenOptions {
                    keep_audio: false,
                    ..o.clone()
                },
                vec![],
                vec![],
                &mut Checkpoints::default(),
                &mut |_| true,
            )
            .unwrap();
            let r = out.master.whole();
            let got = r.integrated_lufs.unwrap();
            let err = got - platform.lufs;
            let over = r.true_peak_dbtp as f64 - platform.ceiling_dbtp as f64;
            worst_lu = worst_lu.max(err.abs());
            worst_tp = worst_tp.max(over);
            println!(
                "{:<10} {:<12} mix {:>6.1} LUFS → {:>6.2} LUFS (target {:>5.1}, {:+.2} LU), true peak {:>6.2} dBTP (ceiling {:>4.1}, {:+.2}), predicted {:>6.2}; {} units, {:.1} s",
                d.id,
                id,
                prop.before.integrated_lufs.unwrap_or(f64::NAN),
                got,
                platform.lufs,
                err,
                r.true_peak_dbtp,
                platform.ceiling_dbtp,
                over,
                prop.predicted.integrated_lufs.unwrap_or(f64::NAN),
                prop.chain.len(),
                think
            );
        }
    }
    println!(
        "worst: {worst_lu:.2} LU from the target, true peak {worst_tp:+.2} dB against the ceiling"
    );
    assert!(worst_lu <= 0.5);
    assert!(worst_tp <= 0.0);
}

/// What the assistant's panel holds at the cap: a 19-minute song (one Firmus part, 570
/// bars at 120 BPM, 48 kHz) listened to and mastered by the panel's own worker, the
/// process's working set sampled every 20 ms — the peak over the start is what ADR 0084
/// quotes.
#[test]
#[ignore]
fn the_assistants_memory_at_the_cap() {
    use cantus_core::*;
    let mut p = Project::new("long");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(570 * 4));
    for b in 0..570 * 4 {
        c.notes
            .push(p.make_note(Ticks::beats(b), Ticks::QUARTER, 60 + (b % 7) as u8, 100));
    }
    t.clips.push(c);
    p.tracks.push(t);
    let mut app =
        cantus_ui::CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
    let ctx = egui::Context::default();
    let base = cantus_rt::usage::working_set_bytes().unwrap_or(0);
    let peak = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(base));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (pk, dn) = (peak.clone(), done.clone());
    let sampler = std::thread::spawn(move || {
        while !dn.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(w) = cantus_rt::usage::working_set_bytes() {
                pk.fetch_max(w, std::sync::atomic::Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });
    let t0 = Instant::now();
    cantus_ui::ai_tools::listen::panel::open(&mut app);
    cantus_ui::ai_tools::listen::panel::start(&mut app);
    while app.listen.panel.done.is_none() && app.listen.panel.error.is_none() {
        cantus_ui::ai_tools::listen::panel::poll(&mut app, &ctx);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    sampler.join().unwrap();
    assert!(
        app.listen.panel.error.is_none(),
        "{:?}",
        app.listen.panel.error
    );
    let held = cantus_rt::usage::working_set_bytes().unwrap_or(0);
    println!(
        "19-minute song: peak {:.0} MB over the start, {:.0} MB held after, {:.0} s",
        (peak.load(std::sync::atomic::Ordering::Relaxed) - base) as f64 / 1e6,
        held.saturating_sub(base) as f64 / 1e6,
        t0.elapsed().as_secs_f64()
    );
}

/// What the checkpoints of a long song hold: the 19-minute song of
/// [`the_assistants_memory_at_the_cap`] heard without its audio, every eight bars a place
/// (71), the working set before and after with the checkpoints kept, then without.
#[test]
#[ignore]
fn the_checkpoints_memory_on_a_long_song() {
    use cantus_core::*;
    let mut p = Project::new("long");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(570 * 4));
    for b in 0..570 * 4 {
        c.notes
            .push(p.make_note(Ticks::beats(b), Ticks::QUARTER, 60 + (b % 7) as u8, 100));
    }
    t.clips.push(c);
    p.tracks.push(t);
    let snap = Snapshot::compile(&p, 1);
    let opts = ListenOptions {
        sample_rate: SR,
        start: Ticks::ZERO,
        tail_seconds: 0.0,
        keep_audio: false,
        sections: sections(&p),
        ..Default::default()
    };
    let mut cps = Checkpoints::default();
    let base = cantus_rt::usage::working_set_bytes().unwrap_or(0);
    let h = listen(&snap, &opts, vec![], vec![], &mut cps, &mut |_| true).unwrap();
    let with = cantus_rt::usage::working_set_bytes().unwrap_or(0);
    let kept = cps.len();
    let estimated = cps.bytes();
    drop(cps);
    let without = cantus_rt::usage::working_set_bytes().unwrap_or(0);
    println!(
        "{} places, {} taken, {kept} kept: {:.0} MB held with them, {:.0} MB without ({:.2} MB each; estimated {:.2})",
        opts.sections.len(),
        h.checkpoints_taken,
        with.saturating_sub(base) as f64 / 1e6,
        without.saturating_sub(base) as f64 / 1e6,
        with.saturating_sub(without) as f64 / 1e6 / kept.max(1) as f64,
        estimated as f64 / 1e6 / kept.max(1) as f64
    );
}

/// What a checkpoint holds on each demo song (review 3: the integrator asked for the
/// showcase and a three-minute demo): the whole song heard with its sections, every
/// checkpoint kept (no budget), the working set with them and without, and the estimate
/// the budget goes by.
#[test]
#[ignore]
fn the_checkpoints_memory_on_the_demos() {
    for d in cantus_ui::demos::ALL.iter() {
        let p = (d.build)();
        let snap = Snapshot::compile(&p, 1);
        let opts = ListenOptions {
            sample_rate: SR,
            tail_seconds: 0.0,
            sections: sections(&p),
            ..Default::default()
        };
        let mut cps = Checkpoints::default();
        cps.budget_bytes = usize::MAX;

        let h = listen(&snap, &opts, vec![], vec![], &mut cps, &mut |_| true).unwrap();
        let with = cantus_rt::usage::working_set_bytes().unwrap_or(0);
        let kept = cps.len();
        let estimated = cps.bytes();
        drop(cps);
        let without = cantus_rt::usage::working_set_bytes().unwrap_or(0);

        println!(
            "{}: {:.0} s, {} tracks, {kept} checkpoints: {:.2} MB each measured, {:.2} estimated",
            d.id,
            h.range_frames as f64 / SR as f64,
            p.tracks.len(),
            with.saturating_sub(without) as f64 / 1e6 / kept.max(1) as f64,
            estimated as f64 / 1e6 / kept.max(1) as f64
        );
    }
}
