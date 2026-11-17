//! Cedo, the ducker, through the whole engine (ADR 0144, provisional): at its defaults the
//! mix untouched, to the bit, no latency; the starting preset (the classic pump on the
//! tempo) sounds with no routing, the same at every block size, live as offline and from
//! bar 2 as the whole render's bar 2; the sidechain from another strip ducks it, and the
//! strip stays awake while its key plays though its own input is silent; the notes of
//! another track fire the shape (`midi_from`); the lookahead and the linear-phase split
//! are reported as latency and compensated; a fork carries on as the unit does.
//! // fxb:cedo

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::EffectUnit;
use modfx::*;
use support::*;

const SR: f32 = 48_000.0;

fn differs(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0f32, |m, (x, y)| m.max((x - y).abs()))
}

fn unit_of(p: &mut Project, fx: EffectId) -> EffectUnit {
    let slot = strip_of(p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    EffectUnit::new(&slot, SR)
}

/// `out` is `reference` exactly `latency` frames later (interleaved stereo).
fn assert_delayed(reference: &[f32], out: &[f32], latency: usize, what: &str) {
    let shift = 2 * latency;
    assert!(
        out[..shift].iter().all(|x| *x == 0.0),
        "{what}: sound before the latency"
    );
    assert_null(
        &reference[..reference.len() - shift],
        &out[shift..],
        0.0,
        what,
    );
}

#[test]
fn at_its_defaults_it_touches_no_sample() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Ducker);
    assert_eq!(unit_of(&mut p, fx).latency(), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Cedo at its defaults");
    // Every trigger at depth 0 ducks nothing, the tempo's and the MIDI's too.
    for trig in [1.0, 2.0] {
        let mut q = p.clone();
        set(&mut q, fx, "trig", trig);
        assert_null(&reference, &render_of(&q), 0.0, "a trigger at depth 0");
    }
}

#[test]
fn the_starting_preset_pumps_with_no_routing_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Ducker);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Ducker);
    strip_of(&mut p, 0).effects.push(slot);
    let a = render_of(&p);
    assert!(differs(&a, &render_of(&demo_project())) > 1e-2, "it pumps");
    let b = realtime(&p, a.len() / 2);
    assert_null(&a[..b.len()], &b, 0.0, "real time equals offline");
    for block in [1usize, 37, 64, 512] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("blocks of {block}"),
        );
    }
}

/// From bar 5 (beat 16): the pump is a function of the song's sample, so a render started
/// on a bar line hears what the whole render hears there (the riff plays from the start
/// and again from bar 5, so the input from bar 5 is the same in both).
#[test]
fn the_pump_from_bar_five_is_bar_five_of_the_whole_render() {
    let mut p = lead_only();
    p.tempo_bpm = 120.0;
    let mut c = p.tracks[0].clips[0].clone();
    c.id = ClipId(p.alloc_id());
    c.start = Ticks::beats(16);
    p.tracks[0].clips.push(c);
    let mut slot = p.make_effect(EffectKind::Ducker);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Ducker);
    slot.spec.set("swing", 0.4).unwrap();
    strip_of(&mut p, 0).effects.push(slot);
    let bar5 = 16 * 24_000; // frames at 120 BPM, 48 kHz
    let whole = render_from(&p, Ticks::beats(0), 256);
    let part = render_from(&p, Ticks::beats(16), 300);
    let n = (part.len() / 2).min(whole.len() / 2 - bar5);
    assert!(n > 96_000, "the song runs past bar 5");
    assert!(part[..2 * n].iter().any(|v| v.abs() > 0.01), "bar 5 sounds");
    assert_null(
        &whole[2 * bar5..2 * (bar5 + n)],
        &part[..2 * n],
        0.0,
        "from bar 5",
    );
}

/// The lead ducked by the bass: the bass's strip routed as the slot's key.
fn keyed_by_the_bass() -> Project {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Ducker);
    set_all(&mut p, fx, &[("depth", 18.0), ("thr", -30.0)]);
    let bass = p.strips[1].id;
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    p
}

#[test]
fn the_sidechain_from_another_strip_ducks_it() {
    let p = keyed_by_the_bass();
    let mut unrouted = p.clone();
    strip_of(&mut unrouted, 0).effects[0].sidechain = None;
    let (a, b) = (render_of(&p), render_of(&unrouted));
    assert_null(
        &b,
        &render_of(&demo_project()),
        0.0,
        "no key: nothing ducks",
    );
    assert!(
        differs(&a, &b) > 1e-2,
        "the key ducks ({})",
        differs(&a, &b)
    );
    let live = realtime(&p, a.len() / 2);
    assert_null(&a[..live.len()], &live, 0.0, "keyed: live = offline");
    for block in [1usize, 37, 512] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("keyed, blocks of {block}"),
        );
    }
}

/// The rest check sees the key: the lead's own input is silent (its notes gone) while its
/// key, the bass, plays. Listening to the key, the lead's strip sends the bass's filtered
/// key out — so it ran every pass the key sounded; a strip that rested on its silent input
/// would have sent nothing.
#[test]
fn a_silent_input_with_a_playing_key_never_rests() {
    let mut p = keyed_by_the_bass();
    p.tracks[0].clips[0].notes.clear();
    let id = strip_of(&mut p, 0).effects[0].id;
    set(&mut p, id, "listen", 2.0);
    let mut unrouted = p.clone();
    strip_of(&mut unrouted, 0).effects[0].sidechain = None;
    let (a, b) = (render_of(&p), render_of(&unrouted));
    assert!(
        differs(&a, &b) > 1e-2,
        "the key is heard through the silent strip ({})",
        differs(&a, &b)
    );
    let live = realtime(&p, a.len() / 2);
    assert_null(&a[..live.len()], &live, 0.0, "live = offline");
}

/// The notes of another track fire the shape (`midi_from`, ADR 0022): with them the lead
/// ducks on each of the bass's notes; without a source nothing fires. Live is the offline
/// render, and the block size changes nothing.
#[test]
fn the_notes_of_another_track_fire_the_shape() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Ducker);
    set_all(&mut p, fx, &[("trig", 2.0), ("depth", 18.0), ("vel", 1.0)]);
    let deaf = p.clone();
    let bass = p.tracks[1].id;
    strip_of(&mut p, 0).effects[0].midi_from = Some(bass);
    let (a, b) = (render_of(&p), render_of(&deaf));
    let lead_notes_fire = differs(&b, &render_of(&demo_project()));
    assert!(
        differs(&a, &b) > 1e-2,
        "the bass's notes are heard ({}; the lead's own {lead_notes_fire})",
        differs(&a, &b)
    );
    let live = realtime(&p, a.len() / 2);
    assert_null(&a[..live.len()], &live, 0.0, "notes: live = offline");
    for block in [1usize, 64] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("notes, blocks of {block}"),
        );
    }
}

/// The lookahead and the linear-phase kernel are latency, reported and compensated: at
/// depth 0 the mix comes out later by exactly what the unit says (the bass held back with
/// the lead); the linear-phase split's delay is its floor's, whatever the crossover: Auto's
/// as committed (60 Hz by default), or a fixed floor's (fxb:cedo).
#[test]
fn the_lookahead_and_the_linear_phase_are_reported_and_compensated() {
    let reference = render_of(&demo_project());
    let lin = cantus_dsp::cedo::band::linear_latency(60.0, SR);
    let lin40 = cantus_dsp::cedo::band::linear_latency(40.0, SR);
    let lin100 = cantus_dsp::cedo::band::linear_latency(100.0, SR);
    for (what, kv, want) in [
        ("lookahead 5 ms", &[("look", 5.0)][..], 240),
        (
            "lows, minimum phase",
            &[("band", 1.0), ("xhz", 200.0)][..],
            0,
        ),
        (
            "lows, linear phase",
            &[("band", 1.0), ("xover", 1.0), ("xhz", 200.0)][..],
            lin,
        ),
        (
            "highs, linear phase at the floor, lookahead",
            &[("band", 2.0), ("xover", 1.0), ("xhz", 40.0), ("look", 2.0)][..],
            lin + 96,
        ),
        // fxb:cedo — Auto committed for a 200 Hz crossover; the fixed 40 Hz floor.
        (
            "lows, linear phase, Auto at 100 Hz",
            &[("band", 1.0), ("xover", 1.0), ("xhz", 200.0), ("lfhz", 100.0)][..],
            lin100,
        ),
        (
            "lows, linear phase, the 40 Hz floor",
            &[("band", 1.0), ("xover", 1.0), ("xhz", 200.0), ("lfloor", 1.0)][..],
            lin40,
        ),
    ] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Ducker);
        set_all(&mut p, fx, kv);
        assert_eq!(unit_of(&mut p, fx).latency(), want, "{what}");
        assert_delayed(&reference, &render_of(&p), want, what);
    }
    assert!(
        (lin as f32 / SR * 1_000.0 - 36.8).abs() < 0.1,
        "the 60 Hz floor's kernel: {lin} samples"
    );
}

/// A fork carries on as the unit does (the envelope, the pump's ring, the band split, the
/// lookahead lines copied).
#[test]
fn a_fork_carries_on_from_where_the_unit_is() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Ducker);
    slot.spec = support::heavy::heavy(EffectKind::Ducker);
    let mut a = EffectUnit::new(&slot, SR);
    let music = |i: usize| {
        let t = i as f32 / SR;
        0.4 * cantus_dsp::math::sin(t * 82.0 * std::f32::consts::TAU)
    };
    let kick = |i: usize| if i % 12_000 < 2_000 { 0.9 } else { 0.0 };
    let block = |u: &mut EffectUnit, at: usize| {
        let mut l: Vec<f32> = (at..at + 480).map(music).collect();
        let mut r = l.clone();
        let k: Vec<f32> = (at..at + 480).map(kick).collect();
        u.process_keyed(&mut l, &mut r, Some((&k, &k)));
        l
    };
    let mut at = 0;
    for _ in 0..100 {
        block(&mut a, at);
        at += 480;
    }
    let mut b = a.fork().expect("a fork");
    for _ in 0..100 {
        let x = block(&mut a, at);
        let y = block(&mut b, at);
        assert_eq!(x, y, "at {at}");
        at += 480;
    }
}
