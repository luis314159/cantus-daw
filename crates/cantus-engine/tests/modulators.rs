//! Modulators on the audio thread (ADR 0057): absent, muted or at depth 0 they are a null;
//! present they are block-size independent, bit-identical between real time and the
//! offline render, a function of the song position where they are synced (a render from
//! bar 5 hears bars 5… of a full render), and each kind does what it says — an LFO moves a
//! native effect's parameter, an envelope opens a filter with each note, a follower ducks
//! a strip.

use cantus_core::modulator::*;
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, compare};
use cantus_engine::*;
use std::sync::Arc;

const SR: f64 = 48_000.0;

/// A Firmus sine on the lead (a note on every beat for 16 bars, released before the
/// next), on a strip with a wide EQ bell at 1 kHz; a held organ chord on a second track.
fn base_project() -> Project {
    let mut p = Project::new("mods");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let f = lead.instrument.firmus_mut().unwrap();
    f.osc1_wave = Waveform::Sine;
    f.amp_attack_s = 0.0;
    f.amp_release_s = 0.02;
    let mut clip = p.make_clip("beats", Ticks::ZERO, Ticks::beats(64));
    for b in 0..64 {
        let n = p.make_note(Ticks::beats(b), Ticks::QUARTER - Ticks(PPQ / 4), 83, 100);
        clip.notes.push(n);
    }
    lead.clips.push(clip);
    let strip = lead.strip.unwrap();
    p.tracks.push(lead);
    let mut eq = p.make_effect(EffectKind::Eq);
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.freq", 1_000.0),
        ("b0.q", 0.5),
        ("b0.gain", 0.0),
    ] {
        eq.spec.set(k, v).unwrap();
    }
    p.strip_mut(strip).unwrap().effects.push(eq);
    let mut organ = p.make_track("Organ");
    organ.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Organ));
    let mut chord = p.make_clip("chord", Ticks::beats(1), Ticks::beats(30));
    for k in [60u8, 64, 67] {
        chord
            .notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(30), k, 90));
    }
    organ.clips.push(chord);
    p.tracks.push(organ);
    p
}

fn eq_of(p: &Project) -> EffectId {
    let s = p.tracks[0].strip.unwrap();
    p.strip(s).unwrap().effects[0].id
}

fn add(p: &mut Project, host: ModHost, kind: ModKind, routes: Vec<(AutoTarget, f32)>) -> ModulatorId {
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
    m.routes = routes
        .into_iter()
        .map(|(target, depth)| ModRoute { target, depth })
        .collect();
    let id = m.id;
    p.modulators_mut(host).unwrap().push(m);
    id
}

fn edit(p: &mut Project, id: ModulatorId, f: impl FnOnce(&mut Modulator)) {
    let (host, i, _) = p.modulator(id).unwrap();
    f(&mut p.modulators_mut(host).unwrap()[i]);
}

/// Every kind at once, on every kind of destination: an LFO on the EQ's gain, steps on the
/// strip's pan, a random source on the track's level, an envelope from the lead's notes on
/// Firmus's cutoff, a follower on the master (hearing the lead's strip) on the organ's
/// strip fader, and a second random source nested into the LFO's amount.
fn modulated_project() -> Project {
    let mut p = base_project();
    let (lead, organ) = (p.tracks[0].id, p.tracks[1].id);
    let lead_strip = p.tracks[0].strip.unwrap();
    let organ_strip = p.tracks[1].strip.unwrap();
    let eq = eq_of(&p);
    let lfo = add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Lfo,
        vec![(
            AutoTarget::Effect {
                effect: eq,
                key: "b0.gain".into(),
            },
            0.3,
        )],
    );
    add(
        &mut p,
        ModHost::Strip(lead_strip),
        ModKind::Step,
        vec![(AutoTarget::StripPan(lead_strip), 0.4)],
    );
    add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Random,
        vec![(AutoTarget::TrackGain(lead), -0.2)],
    );
    add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Envelope,
        vec![(
            AutoTarget::Firmus {
                track: lead,
                param: FirmusParam::Cutoff,
            },
            0.5,
        )],
    );
    let follow = add(
        &mut p,
        ModHost::Master,
        ModKind::Follower,
        vec![(AutoTarget::StripGain(organ_strip), -0.4)],
    );
    edit(&mut p, follow, |m| {
        if let ModSource::Follower(f) = &mut m.source {
            f.listen = Listen::Strip(lead_strip);
        }
    });
    let nest = add(
        &mut p,
        ModHost::Track(organ),
        ModKind::Random,
        vec![(AutoTarget::ModAmount(lfo), -0.6)],
    );
    edit(&mut p, nest, |m| {
        if let ModSource::Random(r) = &mut m.source {
            r.smooth = true;
            r.rate = ModRate::Hz { hz: 3.0 };
        }
    });
    p
}

fn snap(p: &Project) -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(p, 1))
}

fn opts(block: usize) -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.0,
        end: Some(Ticks::beats(16)),
        block_frames: block,
        ..Default::default()
    }
}

#[test]
fn absent_muted_or_zero_depth_modulators_render_exactly_as_none() {
    let plain = render(snap(&base_project()), &opts(512)).unwrap();
    assert!(plain.peak > 0.05);
    let mut p = base_project();
    let lead = p.tracks[0].id;
    let s = p.tracks[0].strip.unwrap();
    // Muted, at depth 0, on a target that is gone, and one that drives nothing at all.
    let a = add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Lfo,
        vec![(AutoTarget::StripGain(s), 0.8)],
    );
    edit(&mut p, a, |m| m.muted = true);
    add(
        &mut p,
        ModHost::Strip(s),
        ModKind::Follower,
        vec![(AutoTarget::StripPan(s), 0.0)],
    );
    add(
        &mut p,
        ModHost::Master,
        ModKind::Random,
        vec![(AutoTarget::TrackGain(TrackId(9_999_999)), 0.5)],
    );
    add(&mut p, ModHost::Master, ModKind::Envelope, vec![]);
    assert!(Snapshot::compile(&p, 1).modulators.is_empty());
    let r = render(snap(&p), &opts(512)).unwrap();
    assert_null(&plain.samples, &r.samples, 0.0, "absent modulators");
}

#[test]
fn modulation_is_heard_and_block_size_independent() {
    let s = snap(&modulated_project());
    assert_eq!(s.modulators.len(), 6);
    let reference = render(s.clone(), &opts(512)).unwrap();
    assert!(reference.peak > 0.05 && reference.peak.is_finite());
    let plain = render(snap(&base_project()), &opts(512)).unwrap();
    assert!(
        !compare(&plain.samples, &reference.samples).passes(1e-3),
        "modulation must be heard"
    );
    for block in [1usize, 7, 16, 37, 64, 333, 1024, 8192] {
        let r = render(s.clone(), &opts(block)).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("block {block}"),
        );
    }
}

/// The processor driven the way the audio callback drives it, blocks of every size, equals
/// the offline render to the bit.
#[test]
fn realtime_processing_matches_the_offline_render() {
    let s = snap(&modulated_project());
    let offline = render(s.clone(), &opts(256)).unwrap();
    let live = live_render(&s, offline.frames());
    assert_null(&offline.samples, &live, 0.0, "realtime vs offline");
}

fn live_render(s: &Arc<Snapshot>, total: usize) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    let mut live = (**s).clone();
    live.loop_region = None;
    live.metronome = false;
    let live = Arc::new(live);
    for slot in live.effect_slots() {
        let unit = EffectUnit::new(slot, SR as f32);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    for unit in cantus_engine::inst::units_for(&live, SR as f32, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&live)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(live)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17, 5];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

/// Synced sources and the notes' envelope on stateless destinations (the strip's fader
/// and pan, the track's level): a render from bar 5 is bars 5… of the full render, to the
/// bit — the lead alone, whose every note starts after the last one ended.
#[test]
fn a_render_from_bar_5_hears_what_a_full_render_hears_there() {
    let mut base = base_project();
    base.tracks.truncate(1);
    base.strips.iter_mut().for_each(|s| s.effects.clear());
    let lead = base.tracks[0].id;
    let s = base.tracks[0].strip.unwrap();
    let mut p = base.clone();
    let lfo = add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Lfo,
        vec![(AutoTarget::StripGain(s), 0.2)],
    );
    edit(&mut p, lfo, |m| {
        if let ModSource::Lfo(l) = &mut m.source {
            l.rate = ModRate::Sync { division: 6 }; // a dotted quarter: not on the bar
            l.phase = 0.1;
        }
    });
    add(
        &mut p,
        ModHost::Strip(s),
        ModKind::Step,
        vec![(AutoTarget::StripPan(s), 0.5)],
    );
    add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Random,
        vec![(AutoTarget::TrackGain(lead), -0.3)],
    );
    let env = add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Envelope,
        vec![(AutoTarget::TrackPan(lead), 0.6)],
    );
    edit(&mut p, env, |m| {
        if let ModSource::Envelope(e) = &mut m.source {
            e.attack_ms = 30.0;
            e.release_ms = 50.0;
        }
    });
    let bar5 = Ticks::beats(16);
    let from = |p: &Project, start: Ticks| {
        let o = RenderOptions {
            start,
            end: Some(Ticks::beats(24)),
            tail_seconds: 0.0,
            block_frames: 300,
            ..Default::default()
        };
        render(snap(p), &o).unwrap().samples
    };
    // The lead itself starts from bar 5 as it plays there (the premise).
    let offset = (16.0 * 0.5 * SR) as usize * 2;
    let full_plain = from(&base, Ticks::ZERO);
    let late_plain = from(&base, bar5);
    assert_null(
        &full_plain[offset..],
        &late_plain,
        0.0,
        "the unmodulated lead from bar 5",
    );
    let full = from(&p, Ticks::ZERO);
    let late = from(&p, bar5);
    assert!(
        !compare(&full_plain[offset..], &full[offset..]).passes(1e-3),
        "the modulation is heard after bar 5"
    );
    assert_null(&full[offset..], &late, 0.0, "modulated, from bar 5");
}

/// RMS of the left channel over `[a, b)` seconds.
fn rms(samples: &[f32], a: f64, b: f64) -> f32 {
    let (i, j) = ((a * SR) as usize, (b * SR) as usize);
    let s: f64 = (i..j).map(|f| f64::from(samples[2 * f]).powi(2)).sum();
    ((s / (j - i) as f64).sqrt()) as f32
}

/// An LFO on a native effect's parameter: a square, a whole note long, on the EQ bell's
/// gain at depth 0.25 (±15 dB around 0): the lead is louder through the high halves.
#[test]
fn an_lfo_moves_a_native_effect_parameter() {
    let mut p = base_project();
    p.tracks.truncate(1);
    let lead = p.tracks[0].id;
    let eq = eq_of(&p);
    let lfo = add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Lfo,
        vec![(
            AutoTarget::Effect {
                effect: eq,
                key: "b0.gain".into(),
            },
            0.25,
        )],
    );
    edit(&mut p, lfo, |m| {
        if let ModSource::Lfo(l) = &mut m.source {
            l.shape = LfoShape::Square;
            l.rate = ModRate::Sync { division: 3 }; // four beats a cycle
        }
    });
    let out = render(snap(&p), &opts(512)).unwrap().samples;
    // At 120 BPM a cycle is 2 s: high for the first second, low for the next.
    let mut checked = 0;
    for cycle in 0..3 {
        let c = f64::from(cycle) * 2.0;
        let high = rms(&out, c + 0.3, c + 0.9);
        let low = rms(&out, c + 1.3, c + 1.9);
        let db = 20.0 * (high / low).log10();
        assert!(db > 20.0, "cycle {cycle}: {db:.1} dB between the halves");
        checked += 1;
    }
    assert_eq!(checked, 3);
}

/// An envelope from the lead's own notes on Firmus's cutoff (a dark saw, the envelope
/// opening it): each note is bright at its start and dark once the envelope has decayed.
#[test]
fn an_envelope_opens_a_filter_with_each_note() {
    let mut p = base_project();
    p.tracks.truncate(1);
    p.strips.iter_mut().for_each(|s| s.effects.clear());
    let lead = p.tracks[0].id;
    {
        let f = p.tracks[0].instrument.firmus_mut().unwrap();
        f.osc1_wave = Waveform::Saw;
        f.cutoff_hz = 200.0;
    }
    let plain = render(snap(&p), &opts(512)).unwrap().samples;
    let env = add(
        &mut p,
        ModHost::Track(lead),
        ModKind::Envelope,
        vec![(
            AutoTarget::Firmus {
                track: lead,
                param: FirmusParam::Cutoff,
            },
            0.6,
        )],
    );
    edit(&mut p, env, |m| {
        if let ModSource::Envelope(e) = &mut m.source {
            e.attack_ms = 1.0;
            e.decay_ms = 80.0;
            e.sustain = 0.0;
        }
    });
    let out = render(snap(&p), &opts(512)).unwrap().samples;
    // Brightness: the first difference's RMS over the signal's.
    let bright = |s: &[f32], a: f64, b: f64| {
        let (i, j) = ((a * SR) as usize, (b * SR) as usize);
        let d: f64 = (i + 1..j)
            .map(|f| f64::from(s[2 * f] - s[2 * f - 2]).powi(2))
            .sum();
        let e: f64 = (i..j).map(|f| f64::from(s[2 * f]).powi(2)).sum();
        (d / e.max(1e-12)).sqrt()
    };
    for beat in [2.0f64, 5.0, 9.0] {
        let t = beat * 0.5;
        let early = bright(&out, t + 0.005, t + 0.035);
        let late = bright(&out, t + 0.15, t + 0.2);
        let plain_early = bright(&plain, t + 0.005, t + 0.035);
        assert!(
            early > 2.0 * late,
            "beat {beat}: early {early:.4} late {late:.4}"
        );
        assert!(early > 2.0 * plain_early, "beat {beat}: opened");
    }
}

/// A follower on the organ's strip hearing the lead's strip, depth −0.5 on the organ's
/// fader: the organ dips under every lead note and comes back between them.
#[test]
fn a_follower_ducks_a_strip() {
    let base = base_project();
    let lead_strip = base.tracks[0].strip.unwrap();
    let organ_strip = base.tracks[1].strip.unwrap();
    let mut p = base.clone();
    let f = add(
        &mut p,
        ModHost::Strip(organ_strip),
        ModKind::Follower,
        vec![(AutoTarget::StripGain(organ_strip), -0.5)],
    );
    edit(&mut p, f, |m| {
        if let ModSource::Follower(fs) = &mut m.source {
            fs.listen = Listen::Strip(lead_strip);
            fs.attack_ms = 2.0;
            fs.release_ms = 40.0;
            fs.gain_db = 6.0;
        }
    });
    let total_plain = render(snap(&base), &opts(512)).unwrap().samples;
    let total_mod = render(snap(&p), &opts(512)).unwrap().samples;
    // The lead alone (the follower drives only the organ, so the lead is the same).
    let mut lead_only = base.clone();
    lead_only.tracks[1].clips.clear();
    let lead = render(snap(&lead_only), &opts(512)).unwrap().samples;
    let organ_gain = |a: f64, b: f64| {
        let organ_plain: Vec<f32> = total_plain.iter().zip(&lead).map(|(t, l)| t - l).collect();
        let organ_mod: Vec<f32> = total_mod.iter().zip(&lead).map(|(t, l)| t - l).collect();
        rms(&organ_mod, a, b) / rms(&organ_plain, a, b)
    };
    for beat in [3.0f64, 6.0, 11.0] {
        let t = beat * 0.5;
        let under = organ_gain(t + 0.02, t + 0.1);
        let between = organ_gain(t + 0.45, t + 0.5);
        assert!(under < 0.5, "beat {beat}: the organ under the lead at {under:.3}");
        assert!(
            between > under * 1.5,
            "beat {beat}: it comes back ({between:.3} after {under:.3})"
        );
    }
}
