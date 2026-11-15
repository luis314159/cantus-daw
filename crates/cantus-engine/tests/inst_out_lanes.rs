//! fix-e (round 40, the review of 40a27bb7): lanes on an instrument's outputs, through the
//! real engine. A lane on an output's delay gives the unit its line from the start — even
//! at 0 — and a ramp glides with no silent gap and no step (ADR 0123, fix-e amendment); a
//! lane on a mic's level sets the mix alone, never a whole configure at every control
//! point (`InstrumentUnit::configure_keys`). Each renders the same bits at 512 and 333
//! frames.

use cantus_core::automation::{AutoPoint, AutoTarget};
use cantus_core::inst::tympanum::KIT_KEY;
use cantus_core::*;
use cantus_engine::inst::{tympanum, UnitKind};
use cantus_engine::{render, RenderOptions};
use std::sync::Arc;

/// A sampler holding the built-in sine for four seconds.
fn held_sine() -> Project {
    let mut p = Project::new("sine");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Sine");
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    s.set("z0.gen", 1.0);
    s.set("z0.gain", -6.0);
    // Looped: the one-cycle sine sounds for as long as the note.
    s.set("z0.mode", 2.0);
    s.set_text("z0.name", Some("sine".into())).unwrap();
    t.instrument = InstrumentSpec::Native(s);
    let mut c = p.make_clip("Hold", Ticks::ZERO, Ticks::beats(8));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(8), 60, 100));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// A modelled studio kit playing a groove for four seconds.
fn groove() -> Project {
    let mut p = Project::new("drums");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
    s.set_text(KIT_KEY, Some("model:studio".into())).unwrap();
    t.instrument = InstrumentSpec::Native(s);
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        let key = match step % 4 {
            0 => 36,
            2 => 38,
            _ => 42,
        };
        c.notes.push(p.make_note(at, Ticks(PPQ / 8), key, 100));
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// A lane on the first track's `key`, from `from` to `to` (the key's own units) over
/// `beats`.
fn lane(p: &mut Project, key: &str, from: f32, to: f32, beats: i64) {
    let track = p.tracks[0].id;
    let kind = p.tracks[0].instrument.native().unwrap().kind;
    let info = kind.param(key).unwrap();
    let pos = |v: f32| (v - info.min) / (info.max - info.min);
    let c = p.make_automation_clip(
        key,
        AutoTarget::Instrument {
            track,
            key: key.into(),
        },
        Ticks::ZERO,
        Ticks::beats(beats),
        vec![
            AutoPoint::new(Ticks::ZERO, pos(from)),
            AutoPoint::new(Ticks::beats(beats), pos(to)),
        ],
    );
    p.tracks[0].clips.push(c);
}

fn render_of(p: &Project, block: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    render(
        snap,
        &RenderOptions {
            tail_seconds: 0.2,
            block_frames: block,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| (v + 0.0).to_bits()).collect()
}

fn left(x: &[f32]) -> Vec<f32> {
    x.chunks(2).map(|f| f[0]).collect()
}

fn max_step(x: &[f32]) -> f32 {
    x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

#[test]
fn a_lane_on_an_outputs_delay_gives_it_a_line_from_zero_and_the_ramp_glides_without_a_gap() {
    let plain = held_sine();
    let mut laned = plain.clone();
    lane(&mut laned, "o0.delay", 0.0, 20.0, 6);
    // The unit the snapshot asks for: a line on output 0, though its delay is 0 at first.
    let s = Snapshot::compile(&laned, 1);
    assert_eq!(
        UnitKind::of_track(&s, &s.tracks[0]),
        Some(UnitKind::Exemplar {
            zones: 1024,
            delays: 0b1,
            room: false
        })
    );
    let s = Snapshot::compile(&plain, 1);
    assert_eq!(
        UnitKind::of_track(&s, &s.tracks[0]),
        Some(UnitKind::Native(InstrumentKind::Sampler)),
        "no lane, no delay: no line"
    );
    // Tympanum by the same rule.
    let mut drums = groove();
    lane(&mut drums, "o1.delay", 0.0, 10.0, 4);
    let s = Snapshot::compile(&drums, 1);
    assert_eq!(
        UnitKind::of_track(&s, &s.tracks[0]),
        Some(UnitKind::Drums {
            room: 0,
            delays: 0b10
        })
    );
    // The ramp: the tone never drops out and never steps beyond its own slope.
    let a = render_of(&laned, 512);
    assert_eq!(
        bits(&a),
        bits(&render_of(&laned, 333)),
        "512 and 333 frames"
    );
    let reference = left(&render_of(&plain, 512));
    let got = left(&a);
    let (from, to) = (9_600, 3 * 48_000);
    let want = max_step(&reference[from..to]);
    let step = max_step(&got[from..to]);
    assert!(
        want > 0.0 && step < want * 1.2 + 1e-4,
        "{step} against {want}"
    );
    for (i, (w, g)) in reference[from..to]
        .chunks(2_400)
        .zip(got[from..to].chunks(2_400))
        .enumerate()
    {
        assert!(
            rms(g) > 0.9 * rms(w),
            "a gap at window {i}: {} of {}",
            rms(g),
            rms(w)
        );
    }
}

#[test]
fn a_lane_on_a_mics_level_sets_the_mix_alone_never_a_whole_configure_per_point() {
    let mut p = groove();
    lane(&mut p, "o2.level", -20.0, 0.0, 8);
    lane(&mut p, "humanize", 0.0, 0.4, 8);
    let before = tympanum::configures();
    let a = render_of(&p, 512);
    let whole = tympanum::configures() - before;
    // Four seconds: 12 000 control points on the grid; a handful of whole configures (the
    // snapshot's), the lanes through `configure_keys`.
    assert!(whole < 50, "{whole} whole configures");
    assert!(a.iter().any(|x| *x != 0.0), "the kit plays");
    assert_eq!(bits(&a), bits(&render_of(&p, 333)), "512 and 333 frames");
    // The lane is heard: the same groove without it differs.
    assert_ne!(bits(&a), bits(&render_of(&groove(), 512)));
}

/// Two built-in sounds in groups 0 and 1 (output 1 on a strip), a filter and an LFO on it
/// through the matrix: what a recorded Chorda or a Sampler automates beside its zones.
fn exemplar_spec() -> NativeSpec {
    use cantus_core::inst_out::{OutputFamily, OutputId, OutputSlot};
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for (n, gen) in [1.0f32, 2.0].into_iter().enumerate() {
        s.set(&format!("z{n}.gen"), gen);
        s.set(&format!("z{n}.group"), n as f32);
        s.set(&format!("z{n}.mode"), 2.0);
        s.set_text(&format!("z{n}.name"), Some(format!("z{n}")))
            .unwrap();
    }
    s.set("g1.out", 1.0);
    s.outputs.push(OutputSlot {
        id: OutputId(1),
        name: "o1".into(),
        family: OutputFamily::Perspective,
        strip: StripId(11),
    });
    s.set("filter.type", 1.0);
    s.set_text("mod1.src", Some("lfo1".into())).unwrap();
    s.set_text("mod1.dst", Some("filter.cutoff".into()))
        .unwrap();
    s.set("mod1.amt", 0.3);
    s
}

const EX_LANES: [&str; 8] = [
    "filter.cutoff",
    "amp.release",
    "gain",
    "g1.gain",
    "mod1.amt",
    "o1.level",
    "lfo1.rate",
    "transpose",
];

/// Three held notes through an Exemplar, the lanes moving every 16 frames: through
/// `configure_keys` (`cheap`) or a whole `configure` at each control point.
fn exemplar_drive(cheap: bool) -> Vec<u32> {
    use cantus_engine::inst::{sampler, InstrumentUnit, NoteOn};
    let mut s = exemplar_spec();
    let mut u = sampler::Unit::with(48_000.0, 1024, 0, false);
    u.configure(&s, &[]);
    let keys: Vec<String> = EX_LANES.iter().map(|k| k.to_string()).collect();
    let mut out = Vec::new();
    for t in (0..48_000usize).step_by(16) {
        for (k, id) in [(48u8, 1u64), (60, 2), (67, 3)] {
            if t == 0 {
                u.note_on(NoteOn {
                    key: k,
                    velocity: 0.8,
                    id,
                    channel: 0,
                });
            } else if t == 24_000 {
                u.note_off(k, id, 0.5);
            }
        }
        let x = t as f32 / 48_000.0;
        let v = [
            2_000.0 + 6_000.0 * x,
            200.0 + 500.0 * x,
            -6.0 * x,
            -12.0 * x,
            0.3 + 0.5 * x,
            -9.0 * x,
            1.0 + 4.0 * x,
            (x * 4.0).floor(),
        ];
        for (k, v) in EX_LANES.iter().zip(v) {
            s.params.insert(k.to_string(), v);
        }
        if cheap {
            assert!(u.configure_keys(&s, &[], &keys), "a lane on {keys:?}");
        } else {
            u.configure(&s, &[]);
        }
        let (mut l, mut r) = ([0.0f32; 16], [0.0f32; 16]);
        let mut bus = [0.0f32; 32];
        u.process_buses(&mut l, &mut r, &mut |_, o, a, b| {
            for i in 0..a.len() {
                bus[2 * (o + i)] += a[i];
                bus[2 * (o + i) + 1] += b[i];
            }
        });
        for i in 0..16 {
            out.push((l[i] + 0.0).to_bits());
            out.push((r[i] + 0.0).to_bits());
        }
        out.extend(bus.iter().map(|x| (x + 0.0).to_bits()));
    }
    out
}

#[test]
fn a_lane_on_a_global_a_group_or_the_matrix_sets_it_alone_to_the_bits_of_a_whole_configure() {
    use cantus_engine::inst::{sampler, InstrumentUnit};
    let cheap = exemplar_drive(true);
    let whole = exemplar_drive(false);
    assert!(cheap.iter().any(|b| *b != 0), "it plays");
    if let Some(i) = cheap.iter().zip(&whole).position(|(a, b)| a != b) {
        panic!("the lanes' path differs from a whole configure at value {i}");
    }
    // A zone's key goes the whole way.
    let mut u = sampler::Unit::with(48_000.0, 1024, 0, false);
    let s = exemplar_spec();
    u.configure(&s, &[]);
    assert!(!u.configure_keys(&s, &[], &["z0.tune".to_string()]));
    // Through the real engine: a lane on the filter and the gain never configures whole.
    let mut p = held_sine();
    lane(&mut p, "filter.cutoff", 500.0, 8_000.0, 8);
    lane(&mut p, "gain", -12.0, 0.0, 8);
    let before = sampler::configures();
    let a = render_of(&p, 512);
    let whole = sampler::configures() - before;
    assert!(
        whole < 100,
        "{whole} whole configures for ~12 000 control points"
    );
    assert_eq!(bits(&a), bits(&render_of(&p, 333)), "512 and 333 frames");
}
