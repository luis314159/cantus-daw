//! Saltus, the pitch unit, through the whole engine (ADR 0132): at its defaults
//! the dry mix later by exactly the latency it reports — delay compensation holding the bass
//! back with the lead — and so the dry of every engine (mono 128, poly 4 095, the analog
//! octave none); the same render at every block size while it shifts; offline equal to real
//! time; the notes of another track heard at their sample (the harmony's chord, the whammy
//! pressed); and on Tabula's board, a board of one is the insert, to the bit. // fx3:saltus

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::fx::tabula::{Board, Pedal, PedalId};
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::EffectUnit;
use modfx::*;
use support::*;

const SR: f32 = 48_000.0;

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

fn latency_of(p: &mut Project, fx: EffectId) -> usize {
    let slot = strip_of(p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    EffectUnit::new(&slot, SR).latency()
}

#[test]
fn at_its_defaults_it_is_the_dry_mix_delayed_by_its_latency() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::PitchShift);
    assert_eq!(latency_of(&mut p, fx), 128);
    assert_delayed(&reference, &render_of(&p), 128, "Saltus at its defaults");
}

/// Each engine's dry alone (no voice, the dry at 100 %) is the mix later by that engine's
/// latency: what it reports is where the sound comes out, the bass held back to match.
#[test]
fn each_engines_dry_comes_out_where_its_latency_says() {
    let reference = render_of(&demo_project());
    for (what, kv, want) in [
        ("mono", &[][..], 128),
        ("poly", &[("engine", 1.0)][..], 4_095),
        ("analog octave", &[("mode", 2.0), ("sub1", 0.0)][..], 0),
        (
            "clean octave",
            &[("mode", 2.0), ("ostyle", 1.0), ("sub1", 0.0)][..],
            128,
        ),
        ("harmony", &[("mode", 1.0)][..], 128),
        ("whammy poly", &[("mode", 3.0), ("engine", 1.0)][..], 4_095),
    ] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::PitchShift);
        set_all(&mut p, fx, &[("lvl1", 0.0), ("dry", 1.0)]);
        set_all(&mut p, fx, kv);
        assert_eq!(latency_of(&mut p, fx), want, "{what}");
        assert_delayed(&reference, &render_of(&p), want, what);
    }
}

/// The starting preset (the twelve-string, poly) and the harmony on a chord held by the
/// bass track: the same bits at every block size.
#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_shifts() {
    let mut cases = Vec::new();
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::PitchShift);
    let cantus_core::starting::Start::Preset(start) =
        cantus_core::starting::start_of(EffectKind::PitchShift)
    else {
        panic!("a starting preset");
    };
    set_all(&mut p, fx, start.params);
    cases.push(("the twelve-string", p));
    cases.push(("harmony on the bass's notes", harmony_on_the_bass()));
    cases.push(("whammy by the bass's notes", whammy_by_the_bass()));
    for (what, p) in cases {
        let reference = render_with(&p, 512);
        for block in [1usize, 37, 64, 4_096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("{what}, block {block}"),
            );
        }
    }
}

/// The lead through the harmony, counting its degrees in the bass's notes (`midi_from`).
fn harmony_on_the_bass() -> Project {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::PitchShift);
    set_all(
        &mut p,
        fx,
        &[("mode", 1.0), ("hsrc", 1.0), ("dry", 1.0), ("lvl1", 0.7)],
    );
    let bass = p.tracks[1].id;
    strip_of(&mut p, 0).effects[0].midi_from = Some(bass);
    p
}

/// The lead through the whammy, pressed to an octave down by the bass's notes.
fn whammy_by_the_bass() -> Project {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::PitchShift);
    set_all(
        &mut p,
        fx,
        &[("mode", 3.0), ("wsrc", 1.0), ("toe", -12.0), ("glide", 5.0)],
    );
    let bass = p.tracks[1].id;
    strip_of(&mut p, 0).effects[0].midi_from = Some(bass);
    p
}

/// The notes are heard: with the bass's notes the harmony sings and the whammy moves; with
/// no source the chord's voice is silent (the dry alone) and the whammy stays at its heel.
#[test]
fn the_notes_of_another_track_reach_it() {
    for (what, p) in [
        ("harmony", harmony_on_the_bass()),
        ("whammy", whammy_by_the_bass()),
    ] {
        let mut deaf = p.clone();
        strip_of(&mut deaf, 0).effects[0].midi_from = None;
        let (a, b) = (render_of(&p), render_of(&deaf));
        let moved = a
            .iter()
            .zip(&b)
            .fold(0f32, |m, (x, y)| m.max((x - y).abs()));
        assert!(moved > 1e-2, "{what}: the notes change the sound ({moved})");
    }
    // Harmony on a chord with nothing held: the dry alone, later by the latency.
    let mut deaf = harmony_on_the_bass();
    strip_of(&mut deaf, 0).effects[0].midi_from = None;
    assert_delayed(
        &render_of(&demo_project()),
        &render_of(&deaf),
        128,
        "harmony on a chord, nothing held",
    );
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    for (what, p) in [
        ("harmony", harmony_on_the_bass()),
        ("whammy", whammy_by_the_bass()),
    ] {
        let offline = render_of(&p);
        assert_null(&offline, &realtime(&p, offline.len()), 0.0, what);
    }
}

/// On Tabula's board (ADR 0131): a board holding one Saltus renders what the insert does,
/// to the bit, the board's slot hearing the notes for its pedal — and reports its latency.
#[test]
fn a_board_of_one_saltus_is_the_insert() {
    for (what, p) in [
        ("harmony", harmony_on_the_bass()),
        ("whammy", whammy_by_the_bass()),
    ] {
        let mut q = p.clone();
        let slot = strip_of(&mut q, 0).effects.remove(0);
        let mut board = q.make_effect(EffectKind::Board);
        board.midi_from = slot.midi_from;
        board.spec.board = Some(Box::new(Board {
            pedals: vec![Pedal::new(PedalId(1), slot.spec.clone())],
            macros: Vec::new(),
        }));
        assert_eq!(
            EffectUnit::new(&board, SR).latency(),
            EffectUnit::new(&slot, SR).latency(),
            "{what}: the board reports the pedal's latency"
        );
        strip_of(&mut q, 0).effects.push(board);
        assert_null(&render_of(&p), &render_of(&q), 0.0, what);
    }
}
