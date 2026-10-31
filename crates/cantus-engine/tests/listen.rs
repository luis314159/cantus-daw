//! Listening (round 34, ADR 0083): a listen is the render, sample for sample, with every
//! tap on; a section rendered from its checkpoint is the same frames of the whole render
//! to the bit — also after an edit that comes later in the song — and live playback through
//! the application's own handle is the same samples as the render an agent hears.

mod support;
use cantus_core::*;
use cantus_engine::listen::{listen, Checkpoints, Heard, ListenOptions, Resumed};
use cantus_engine::stems::StemSource;
use cantus_engine::*;
use std::sync::Arc;
use support::heavy::heavy;

const SR: u32 = 48_000;

fn bar() -> Ticks {
    Ticks::beats(4)
}

/// Every native instrument, Firmus and Pulsus, each on a strip of its own carrying some of
/// every effect kind at its heaviest; a send into a return with a reverb; the master chain
/// with an EQ, a compressor and the limiter's look-ahead; a fader ride and an effect's
/// lane across the section changes, and an LFO on a pan. Chords held over the changes and
/// notes exactly on them. Three bars of 120 BPM.
fn rich() -> Project {
    let mut p = Project::new("listen");
    p.tempo_bpm = 120.0;
    let mut kinds: Vec<InstrumentSpec> = InstrumentKind::ALL
        .iter()
        .map(|k| InstrumentSpec::Native(NativeSpec::new(*k)))
        .collect();
    kinds.push(InstrumentSpec::Firmus(FirmusParams::default()));
    kinds.push(InstrumentSpec::Pulsus(PulsusParams::default()));
    for (i, spec) in kinds.into_iter().enumerate() {
        let drums = matches!(spec, InstrumentSpec::Pulsus(_));
        let mut t = p.make_track(format!("T{i}"));
        t.instrument = spec;
        let mut c = p.make_clip("c", Ticks::ZERO, bar() * 3);
        let root = 48 + (i as u8 * 5) % 12;
        if drums {
            for s in 0..24i64 {
                let pitch = [36u8, 42, 38, 42][(s % 4) as usize];
                c.notes
                    .push(p.make_note(Ticks::EIGHTH * s, Ticks::SIXTEENTH, pitch, 100));
            }
        } else {
            // A chord held over the first change, one starting on the second, a line.
            for k in [0u8, 4, 7] {
                c.notes
                    .push(p.make_note(Ticks::beats(2), Ticks::beats(4), root + k, 90));
                c.notes
                    .push(p.make_note(bar() * 2, Ticks::beats(3), root + k + 2, 80));
            }
            for s in 0..12i64 {
                c.notes.push(p.make_note(
                    Ticks::QUARTER * s,
                    Ticks::EIGHTH,
                    root + 12 + (s % 5) as u8,
                    70 + s as u8,
                ));
            }
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    // Every effect kind, three to a strip.
    let strips: Vec<StripId> = p.tracks.iter().map(|t| t.strip.unwrap()).collect();
    for (i, kind) in EffectKind::ALL.iter().enumerate() {
        let mut slot = p.make_effect(*kind);
        slot.spec = heavy(*kind);
        let s = strips[i / 3 % strips.len()];
        p.strip_mut(s).unwrap().effects.push(slot);
    }
    // A return with a reverb, fed from two strips.
    let ret = p.strips.last().unwrap().id;
    let mut verb = p.make_effect(EffectKind::Reverb);
    verb.spec.set("mix", 1.0);
    p.strip_mut(ret).unwrap().effects.push(verb);
    for s in &strips[..2] {
        p.strip_mut(*s).unwrap().sends.push(Send {
            to: ret,
            level: 0.5,
            enabled: true,
            tap: SendTap::Post,
        });
    }
    // The master: EQ, glue, the limiter's look-ahead.
    for kind in [EffectKind::Eq, EffectKind::Compressor, EffectKind::Limiter] {
        let slot = p.make_effect(kind);
        p.master.effects.push(slot);
    }
    // A fader ride and an effect's lane across the changes, an LFO on a pan.
    let ride = p.make_automation_clip(
        "ride",
        AutoTarget::StripGain(strips[0]),
        Ticks::beats(2),
        Ticks::beats(8),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.8),
            AutoPoint::new(Ticks::beats(4), 0.4),
            AutoPoint::new(Ticks::beats(8), 0.9),
        ],
    );
    p.tracks[0].clips.push(ride);
    let sat = p
        .strip(strips[2])
        .unwrap()
        .effects
        .iter()
        .find(|e| e.spec.kind == EffectKind::Saturator)
        .map(|e| e.id);
    if let Some(sat) = sat {
        let lane = p.make_automation_clip(
            "drive",
            AutoTarget::Effect {
                effect: sat,
                key: "drive".into(),
            },
            Ticks::ZERO,
            bar() * 3,
            vec![
                AutoPoint::new(Ticks::ZERO, 0.1),
                AutoPoint::new(bar() * 2, 0.5),
                AutoPoint::new(bar() * 3, 0.7),
            ],
        );
        p.tracks[2].clips.push(lane);
    }
    let lead = p.tracks[1].id;
    let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    lfo.routes = vec![ModRoute {
        target: AutoTarget::StripPan(strips[1]),
        depth: 0.6,
    }];
    p.modulators_mut(ModHost::Track(lead)).unwrap().push(lfo);
    p
}

fn snapshot(p: &Project) -> Snapshot {
    Snapshot::compile(p, 1)
}

/// The whole song, latency compensated, as an export renders it.
fn whole(p: &Project) -> Rendered {
    render(
        Arc::new(snapshot(p)),
        &RenderOptions {
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("renders")
}

fn every_tap(p: &Project) -> Vec<StemSource> {
    let mut t: Vec<StemSource> = (0..p.tracks.len()).map(StemSource::Track).collect();
    t.extend((0..p.strips.len()).map(StemSource::Strip));
    t.push(StemSource::Direct);
    t
}

fn hear(p: &Project, start: Ticks, cps: &mut Checkpoints, taps: Vec<StemSource>) -> Heard {
    listen(
        &snapshot(p),
        &ListenOptions {
            start,
            tail_seconds: 1.0,
            keep_audio: true,
            sections: vec![bar(), bar() * 2],
            taps,
            ..Default::default()
        },
        Vec::new(),
        Vec::new(),
        cps,
        &mut |_| true,
    )
    .expect("listens")
}

/// Sample `tick`'s frame in a render from the song's start.
fn frame(p: &Project, tick: Ticks) -> usize {
    let s = snapshot(p);
    cantus_engine::transport::sample_at_map(&s.tempo, 0.0, tick.0 as f64, SR as f64) as usize
}

fn first_difference(a: &[f32], b: &[f32]) -> Option<(usize, f32)> {
    if a.len() != b.len() {
        return Some((a.len().min(b.len()) / 2, f32::NAN));
    }
    let at = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())?;
    let most = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0f32, f32::max);
    Some((at / 2, most))
}

/// Hearing the whole song with a tap on every track and strip is the export's render to
/// the bit (the taps read, never write), and it leaves a checkpoint at each section.
#[test]
fn the_taps_change_no_sample() {
    let p = rich();
    let full = whole(&p);
    let mut cps = Checkpoints::default();
    let h = hear(&p, Ticks::ZERO, &mut cps, every_tap(&p));
    assert_eq!(h.resumed, Resumed::Start);
    assert_eq!(first_difference(&h.audio, &full.samples), None);
    assert_eq!(h.checkpoints_taken, 2);
    assert_eq!(cps.ticks(), vec![bar(), bar() * 2]);
    // Every tap heard the range from its first sample.
    for (s, ear) in &h.taps {
        assert_eq!(ear.frames(), h.master.frames(), "{s:?}");
    }
    assert!(h.latency > 0, "the limiter's look-ahead is compensated");
}

/// A section rendered from its checkpoint is the same frames as the whole render, to the
/// bit, for a range from the section's start and for one that starts inside it — and the
/// engine rendered only from the checkpoint on.
#[test]
fn a_section_from_its_checkpoint_is_the_whole_render_to_the_bit() {
    let p = rich();
    let full = whole(&p);
    let mut cps = Checkpoints::default();
    hear(&p, Ticks::ZERO, &mut cps, Vec::new());
    for start in [
        bar() * 2,
        bar() * 2 + Ticks::beats(1),
        bar() + Ticks::EIGHTH,
    ] {
        let h = hear(&p, start, &mut cps, every_tap(&p));
        let want = if start >= bar() * 2 { bar() * 2 } else { bar() };
        assert_eq!(h.resumed, Resumed::Checkpoint(want), "from {start:?}");
        let s0 = frame(&p, start);
        assert_eq!(
            first_difference(&h.audio, &full.samples[s0 * 2..]),
            None,
            "from {start:?}"
        );
        let skipped = frame(&p, want);
        assert!(
            h.rendered_frames + skipped <= full.frames() + h.latency + 16,
            "rendered {} frames of {}",
            h.rendered_frames,
            full.frames()
        );
    }
}

/// An agent changes the song after a checkpoint — notes rewritten in the last section, one
/// added on its first beat, the effect's lane moved later — and the section is heard from
/// the checkpoint exactly as a whole render of the changed song gives it. A change before
/// the checkpoint is never heard from it.
#[test]
fn an_edit_after_the_checkpoint_plays_from_it_as_a_whole_render_would() {
    let p = rich();
    let mut cps = Checkpoints::default();
    hear(&p, Ticks::ZERO, &mut cps, Vec::new());
    let mut q = p.clone();
    for t in q
        .tracks
        .iter_mut()
        .filter(|t| t.clips[0].automation.is_none())
    {
        let c = &mut t.clips[0];
        for n in c.notes.iter_mut().filter(|n| n.start >= bar() * 2) {
            n.pitch = (n.pitch + 3).min(120);
            n.velocity = 60;
        }
    }
    let extra = q.make_note(bar() * 2, Ticks::QUARTER, 84, 127);
    q.tracks[3].clips[0].notes.push(extra);
    if let Some(lane) = q.tracks[2]
        .clips
        .iter_mut()
        .find(|c| c.automation.is_some())
    {
        if let Some(a) = lane.automation.as_mut() {
            if let Some(last) = a.points.last_mut() {
                last.value = 0.2;
            }
        }
    }
    let full = whole(&q);
    let h = hear(&q, bar() * 2, &mut cps, Vec::new());
    assert_eq!(h.resumed, Resumed::Checkpoint(bar() * 2));
    let s0 = frame(&q, bar() * 2);
    assert_eq!(first_difference(&h.audio, &full.samples[s0 * 2..]), None);

    // Changed in the middle of the second bar: the first checkpoint still holds for a
    // range in the third, the second does not.
    let mut r = p.clone();
    let late = r.make_note(bar() + Ticks::beats(2), Ticks::QUARTER, 90, 110);
    r.tracks[4].clips[0].notes.push(late);
    let full = whole(&r);
    let h = hear(&r, bar() * 2, &mut cps, Vec::new());
    assert_eq!(h.resumed, Resumed::Checkpoint(bar()));
    assert_eq!(first_difference(&h.audio, &full.samples[s0 * 2..]), None);

    // A fader sounds from the start: no checkpoint serves it.
    let mut f = p.clone();
    let s = f.tracks[5].strip.unwrap();
    f.strip_mut(s).unwrap().gain_db = -4.0;
    let mut fresh = Checkpoints::default();
    hear(&p, Ticks::ZERO, &mut fresh, Vec::new());
    let h = hear(&f, bar() * 2, &mut fresh, Vec::new());
    assert_eq!(h.resumed, Resumed::Start);
    let full = whole(&f);
    assert_eq!(first_difference(&h.audio, &full.samples[s0 * 2..]), None);
}

/// Any block size gives the same samples, from the start or from a checkpoint.
#[test]
fn the_block_size_changes_no_sample() {
    let p = rich();
    let full = whole(&p);
    for block in [1usize, 77, 4_096] {
        let mut cps = Checkpoints::default();
        let opts = |start| ListenOptions {
            start,
            block_frames: block,
            tail_seconds: 1.0,
            keep_audio: true,
            sections: vec![bar() * 2],
            ..Default::default()
        };
        let first = listen(
            &snapshot(&p),
            &opts(Ticks::ZERO),
            vec![],
            vec![],
            &mut cps,
            &mut |_| true,
        )
        .unwrap();
        assert_eq!(
            first_difference(&first.audio, &full.samples),
            None,
            "block {block}"
        );
        let h = listen(
            &snapshot(&p),
            &opts(bar() * 2),
            vec![],
            vec![],
            &mut cps,
            &mut |_| true,
        )
        .unwrap();
        assert_eq!(h.resumed, Resumed::Checkpoint(bar() * 2));
        let s0 = frame(&p, bar() * 2);
        assert_eq!(
            first_difference(&h.audio, &full.samples[s0 * 2..]),
            None,
            "block {block}"
        );
    }
}

/// A range that ends before the song stops there and rings out, as an export's does: the
/// same samples as `render` with that end.
#[test]
fn a_range_that_ends_early_is_the_export_of_that_range() {
    let p = rich();
    let end = bar() * 2 + Ticks::beats(2);
    let want = render(
        Arc::new(snapshot(&p)),
        &RenderOptions {
            end: Some(end),
            tail_seconds: 0.5,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut cps = Checkpoints::default();
    let opts = |start| ListenOptions {
        start,
        end: Some(end),
        tail_seconds: 0.5,
        keep_audio: true,
        sections: vec![bar() * 2],
        ..Default::default()
    };
    let h = listen(
        &snapshot(&p),
        &opts(Ticks::ZERO),
        vec![],
        vec![],
        &mut cps,
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(first_difference(&h.audio, &want.samples), None);
    let h = listen(
        &snapshot(&p),
        &opts(bar() * 2),
        vec![],
        vec![],
        &mut cps,
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(h.resumed, Resumed::Checkpoint(bar() * 2));
    let s0 = frame(&p, bar() * 2);
    assert_eq!(first_difference(&h.audio, &want.samples[s0 * 2..]), None);
    assert_eq!(h.range_frames, frame(&p, end) - s0);
}

/// A hosted plug-in's state cannot be copied: the range pre-rolls and says why.
#[test]
fn a_hosted_unit_pre_rolls_and_says_so() {
    let p = rich();
    let slot = p.master.effects[0].clone();
    let unit = EffectUnit::new(&slot, SR as f32);
    let mut cps = Checkpoints::default();
    let h = listen(
        &snapshot(&p),
        &ListenOptions {
            start: bar() * 2,
            preroll_seconds: 1.5,
            sections: vec![bar()],
            ..Default::default()
        },
        vec![unit],
        vec![],
        &mut cps,
        &mut |_| true,
    )
    .unwrap();
    match &h.resumed {
        Resumed::Preroll { seconds, why, .. } => {
            assert!((seconds - 1.5).abs() < 0.01, "{seconds}");
            assert!(why.contains("hosted"), "{why}");
        }
        other => panic!("{other:?}"),
    }
    assert!(!h.resumed.is_exact());
    assert!(cps.is_empty(), "nothing it cannot copy is kept");
}

/// Stopped from the progress callback, a listen ends with `Cancelled`.
#[test]
fn a_listen_stops_when_asked() {
    let p = rich();
    let mut cps = Checkpoints::default();
    let mut calls = 0;
    let r = listen(
        &snapshot(&p),
        &ListenOptions::default(),
        vec![],
        vec![],
        &mut cps,
        &mut |_| {
            calls += 1;
            calls < 5
        },
    );
    assert!(matches!(r, Err(RenderError::Cancelled)));
}

/// Live = offline: the project played through the application's own engine handle — its
/// snapshot, units and automation sent as playback sends them, delay compensation answered
/// as the UI answers it, irregular blocks — is what an agent's listen hears, to the bit.
#[test]
fn live_playback_is_what_a_listen_hears() {
    let p = rich();
    let mut cps = Checkpoints::default();
    let heard = hear(&p, Ticks::ZERO, &mut cps, Vec::new());
    let (mut h, mut proc) = EngineHandle::manual(SR as f64);
    let snap = h.compile(&p, 1);
    h.set_snapshot(snap);
    for _ in 0..8 {
        proc.settle();
        h.poll();
    }
    h.seek(Ticks::ZERO);
    h.play();
    let latency = heard.latency;
    let frames = heard.audio.len() / 2 + latency;
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut out = Vec::with_capacity(frames * 2);
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        h.poll();
    }
    assert_eq!(proc.stats().latency_samples as usize, latency);
    assert_eq!(first_difference(&out[latency * 2..], &heard.audio), None);
}

/// A pre-roll is the render from where it starts, to the bit (here, a song with no memory
/// longer than its pre-roll, it is the whole render's frames too) — at a tempo whose ticks
/// fall between samples (116 BPM) and pre-rolls of odd lengths, so the transport must start
/// on the whole sample a render from the top reaches there (review 1: it started on the
/// tick's fractional sample, every event a fraction of a sample late); one that would reach
/// back past the song's start is the exact render from the start, and says so.
#[test]
fn a_preroll_is_the_render_from_where_it_starts() {
    let mut p = support::demo_project();
    p.tempo_bpm = 116.0;
    let extra = p.make_effect(EffectKind::Eq);
    p.master.effects.push(extra.clone());
    let full = whole(&p);
    let start = Ticks::beats(6);
    let s0 = frame(&p, start);
    for pre in [0.73, 1.37, 60.0] {
        let unit = EffectUnit::new(&extra, SR as f32);
        let h = listen(
            &snapshot(&p),
            &ListenOptions {
                start,
                tail_seconds: 1.0,
                keep_audio: true,
                preroll_seconds: pre,
                ..Default::default()
            },
            vec![unit],
            vec![],
            &mut Checkpoints::default(),
            &mut |_| true,
        )
        .unwrap();
        if pre >= 60.0 {
            assert_eq!(h.resumed, Resumed::Start, "back past the start: exact");
        } else {
            assert!(
                matches!(h.resumed, Resumed::Preroll { .. }),
                "{:?}",
                h.resumed
            );
        }
        assert_eq!(
            first_difference(&h.audio, &full.samples[s0 * 2..]),
            None,
            "pre-roll {pre}"
        );
    }
}

/// Review 2: at a real section count (a section every beat: twelve places), the song and a
/// solo both keep all their checkpoints, and an edit late in the song replaces only that
/// setup's stale ones after it — the count stays.
#[test]
fn a_dozen_sections_and_a_variant_fit_and_a_late_edit_replaces_only_its_own() {
    let p = rich();
    let sections: Vec<Ticks> = (1..12).map(Ticks::beats).collect();
    let opts = |start: Ticks| ListenOptions {
        start,
        tail_seconds: 1.0,
        keep_audio: true,
        sections: sections.clone(),
        ..Default::default()
    };
    let go = |q: &Project, start: Ticks, cps: &mut Checkpoints| {
        listen(&snapshot(q), &opts(start), vec![], vec![], cps, &mut |_| {
            true
        })
        .unwrap()
    };
    // The count is what this holds (the budget in bytes has a test of its own: this
    // project's every unit makes its images about 40 MB each).
    let mut cps = Checkpoints::default();
    cps.budget_bytes = usize::MAX;
    go(&p, Ticks::ZERO, &mut cps);
    assert_eq!(cps.len(), 11);
    let mut solo = p.clone();
    solo.tracks[1].soloed = true;
    go(&solo, Ticks::ZERO, &mut cps);
    assert_eq!(cps.len(), 22, "both setups kept whole");
    // An edit after beat 9: the song's checkpoints at 10 and 11 go stale and are replaced.
    let mut q = p.clone();
    let late = q.make_note(Ticks::beats(9) + Ticks::EIGHTH, Ticks::QUARTER, 77, 100);
    q.tracks[4].clips[0].notes.push(late);
    go(&q, Ticks::ZERO, &mut cps);
    assert_eq!(
        cps.len(),
        22,
        "the stale two replaced, nothing else evicted"
    );
    let full = whole(&p);
    let h = go(&p, Ticks::beats(8), &mut cps);
    assert_eq!(h.resumed, Resumed::Checkpoint(Ticks::beats(8)));
    let s0 = frame(&p, Ticks::beats(8));
    assert_eq!(first_difference(&h.audio, &full.samples[s0 * 2..]), None);
}

/// Review 1: a solo, a mute, the master out are other snapshots with checkpoints of their
/// own — hearing one never throws the song's own away.
#[test]
fn a_variant_keeps_the_songs_own_checkpoints() {
    let p = rich();
    let full = whole(&p);
    let mut cps = Checkpoints::default();
    hear(&p, Ticks::ZERO, &mut cps, Vec::new());
    let mut solo = p.clone();
    solo.tracks[1].soloed = true;
    hear(&solo, Ticks::ZERO, &mut cps, Vec::new());
    assert_eq!(cps.len(), 4, "two places, two snapshots");
    let h = hear(&p, bar() * 2, &mut cps, Vec::new());
    assert_eq!(h.resumed, Resumed::Checkpoint(bar() * 2));
    let s0 = frame(&p, bar() * 2);
    assert_eq!(first_difference(&h.audio, &full.samples[s0 * 2..]), None);
}

/// Review 1: a sample streaming from disk makes the engine uncopiable — the range pre-rolls
/// and says why, instead of rendering from the top in silence — and a render that began
/// inexactly never leaves a checkpoint, even at a section inside its pre-roll.
#[test]
fn a_streamed_sample_pre_rolls_and_an_inexact_render_keeps_no_checkpoint() {
    let p = rich();
    let mut snap = snapshot(&p);
    let streamed = cantus_core::SampleData {
        rate: SR,
        channels: 1,
        frames: std::sync::Arc::from(vec![0.0f32; 16]),
        stream: Some(cantus_core::StreamRef(std::sync::Arc::new(()))),
    };
    // On the Firmus track, which plays none: the engine's state itself stays copyable.
    let firmus = p
        .tracks
        .iter()
        .position(|t| matches!(t.instrument, InstrumentSpec::Firmus(_)))
        .unwrap();
    snap.tracks[firmus].samples = vec![Some(Arc::new(streamed))];
    let mut cps = Checkpoints::default();
    let start = bar() * 2;
    let h = listen(
        &snap,
        &ListenOptions {
            start,
            preroll_seconds: 1.3,
            // A section inside the pre-roll.
            sections: vec![start - Ticks::QUARTER],
            ..Default::default()
        },
        vec![],
        vec![],
        &mut cps,
        &mut |_| true,
    )
    .unwrap();
    match &h.resumed {
        Resumed::Preroll { why, .. } => assert!(why.contains("stream"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(cps.is_empty(), "nothing kept from an inexact render");
    // Review 2: asked to be exact, the same setup renders from the top — and, its engine
    // known to be uncopiable, takes no checkpoint (it tried at every place).
    let h = listen(
        &snap,
        &ListenOptions {
            start,
            preroll_seconds: f64::INFINITY,
            sections: vec![bar(), start - Ticks::QUARTER],
            ..Default::default()
        },
        vec![],
        vec![],
        &mut cps,
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(h.resumed, Resumed::Start);
    assert!(cps.is_empty(), "an uncopiable setup leaves no checkpoint");
}

/// Review 1: why a processor cannot be copied, in words — here the application's own
/// handle's, which carries a sync port.
#[test]
fn a_fork_that_fails_says_why() {
    let (_h, proc_) = EngineHandle::manual(SR as f64);
    assert!(proc_.fork(8, 8).is_none());
    let r = proc_.fork_refusal().unwrap();
    assert_eq!(r.why, "a sync port is a device");
    assert!(r.lasting);
}

/// What keeps a pre-roll from converging, unit by unit — every native instrument, and Firmus
/// through each effect (`cargo test -p cantus-engine --test
/// listen -- --ignored --nocapture preroll_by_unit`): a held chord through one effect at
/// its defaults (a chord struck on every beat), the last 2 s pre-rolled 3 s, the largest difference from the whole render
/// under the section's peak. ADR 0083 quotes it.
#[test]
#[ignore]
fn preroll_by_unit() {
    let base = |inst: Option<InstrumentKind>| {
        let mut p = Project::new("preroll");
        p.tempo_bpm = 116.0;
        let mut t = p.make_track("Pad");
        if let Some(k) = inst {
            t.instrument = InstrumentSpec::Native(NativeSpec::new(k));
        }
        let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(24));
        for b in 0..24i64 {
            for k in [57u8, 60, 64] {
                c.notes
                    .push(p.make_note(Ticks::beats(b), Ticks::beats(1) - Ticks(40), k, 90));
            }
        }
        t.clips.push(c);
        p.tracks.push(t);
        p
    };
    // Every native instrument through no effect, then Firmus through each effect.
    let mut cases: Vec<(String, Option<InstrumentKind>, Option<EffectKind>)> = InstrumentKind::ALL
        .iter()
        .map(|k| (format!("{k:?}"), Some(*k), None))
        .collect();
    let kinds = [
        None,
        Some(EffectKind::Phaser),
        Some(EffectKind::Chorus),
        Some(EffectKind::Flanger),
        Some(EffectKind::Tremolo),
        Some(EffectKind::Vibrato),
        Some(EffectKind::VolumeShaper),
        Some(EffectKind::Delay),
        Some(EffectKind::Reverb),
        Some(EffectKind::Space),
        Some(EffectKind::Compressor),
        Some(EffectKind::Limiter),
    ];
    cases.extend(kinds.iter().map(|k| {
        (
            k.map_or("Firmus".to_string(), |k| format!("Firmus > {k:?}")),
            None,
            *k,
        )
    }));
    for (name, inst, kind) in cases {
        let mut p = base(inst);
        if let Some(k) = kind {
            let slot = p.make_effect(k);
            support::strip_of(&mut p, 0).effects.push(slot);
        }
        let extra = p.make_effect(EffectKind::Eq);
        p.master.effects.push(extra.clone());
        let full = whole(&p);
        let start = Ticks::beats(20);
        let s0 = frame(&p, start);
        let h = listen(
            &snapshot(&p),
            &ListenOptions {
                start,
                tail_seconds: 1.0,
                keep_audio: true,
                preroll_seconds: 3.0,
                ..Default::default()
            },
            vec![EffectUnit::new(&extra, SR as f32)],
            vec![],
            &mut Checkpoints::default(),
            &mut |_| true,
        )
        .unwrap();
        let n = h.audio.len().min(full.samples.len() - s0 * 2);
        let peak = full.samples[s0 * 2..s0 * 2 + n]
            .iter()
            .fold(0f32, |m, x| m.max(x.abs()))
            .max(1e-9);
        let worst = h.audio[..n]
            .iter()
            .zip(&full.samples[s0 * 2..s0 * 2 + n])
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        let db = if worst == 0.0 {
            f32::INFINITY
        } else {
            20.0 * cantus_dsp::math::log10(peak / worst)
        };
        println!("{name:<22} {db:>7.1} dB under the peak");
    }
}
