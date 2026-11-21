//! r41:contract (ADR 0153) — the song's key reaches every effect: the engine hands
//! `Snapshot::key` to each unit right after its spec ([`FxUnit::set_song_key`]), on a strip
//! and on the master, and a new snapshot with another key (or none) hands that.

mod support;
use cantus_core::scale::{Key, ScaleKind};
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::FxUnit;
use cantus_engine::*;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use support::*;

/// A wire that keeps the root of the key it was last handed (−1: none, −2: never called).
#[derive(Debug)]
struct Probe(Arc<AtomicI32>);

impl FxUnit for Probe {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn set_song_key(&mut self, key: Option<Key>) {
        self.0.store(
            key.map(|k| i32::from(k.root)).unwrap_or(-1),
            Ordering::SeqCst,
        );
    }
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32], _k: Option<(&[f32], &[f32])>) {}
    fn reset(&mut self) {}
}

fn heard(key: Option<Key>) -> (i32, i32) {
    let mut p = Project::new("key");
    let t = p.make_track("Keys");
    let strip = t.strip.expect("a strip");
    p.tracks.push(t);
    let on_strip = p.make_effect(EffectKind::Utility);
    let on_master = p.make_effect(EffectKind::Utility);
    let (a, b) = (on_strip.id, on_master.id);
    p.strip_mut(strip).unwrap().effects.push(on_strip);
    p.master.effects.push(on_master);
    p.key = key;
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let (sa, sb) = (Arc::new(AtomicI32::new(-2)), Arc::new(AtomicI32::new(-2)));
    let units = snap
        .effect_slots()
        .map(|s| {
            let probe = if s.id == a { sa.clone() } else { sb.clone() };
            assert!(s.id == a || s.id == b);
            EffectUnit::from_fx(s, Box::new(Probe(probe)))
        })
        .collect();
    let opts = RenderOptions {
        end: Some(Ticks::beats(1)),
        tail_seconds: 0.0,
        ..RenderOptions::default()
    };
    render_with_units(snap, &opts, units).expect("render");
    (sa.load(Ordering::SeqCst), sb.load(Ordering::SeqCst))
}

#[test]
fn every_effect_hears_the_song_s_key_on_a_strip_and_on_the_master() {
    assert_eq!(heard(Some(Key::new(7, ScaleKind::Major))), (7, 7));
    assert_eq!(heard(Some(Key::new(2, ScaleKind::Minor))), (2, 2));
    assert_eq!(heard(None), (-1, -1));
}

const IN_C: Option<Key> = Some(Key {
    root: 0,
    scale: ScaleKind::Major,
});
const IN_G: Option<Key> = Some(Key {
    root: 7,
    scale: ScaleKind::Major,
});

/// Nitor at heavy settings following the song (its tonic sub and lift tune to the key).
fn nitor_following() -> EffectSpec {
    let mut s = support::heavy::heavy(EffectKind::Enhancer);
    s.set("key", 0.0).unwrap();
    s
}

/// Round 41's review: `EffectSpec::reads_song_key` is the one place that says an effect
/// follows the song's key (a freeze keeps the key then). Every kind at heavy settings, and
/// Nitor following the song, rendered in C and in G: a sound that moves with the key is one
/// that says so.
#[test]
fn an_effect_whose_sound_moves_with_the_key_says_it_reads_it() {
    let specs = EffectKind::ALL
        .iter()
        .map(|k| support::heavy::heavy(*k))
        .chain(std::iter::once(nitor_following()));
    let mut moved = 0;
    for spec in specs {
        let in_key = |key: Option<Key>| {
            let mut p = demo_project();
            let id = add_effect(&mut p, spec.kind);
            p.effect_mut(p.tracks[0].strip.unwrap(), id).unwrap().spec = spec.clone();
            p.key = key;
            render_with(&p, 512)
        };
        let (c, g) = (in_key(IN_C), in_key(IN_G));
        let moves = c != g;
        moved += usize::from(moves);
        assert!(
            !moves || spec.reads_song_key(),
            "{:?}: its sound follows the song's key, and reads_song_key says no",
            spec.kind
        );
    }
    assert!(moved >= 1, "Nitor following the song moves with it");
}

/// A project of one strip: the lead alone, Nitor following the song on the master.
fn lead_into_nitor(key: Option<Key>) -> Project {
    let mut p = demo_project();
    p.tracks.truncate(1);
    let mut slot = p.make_effect(EffectKind::Enhancer);
    slot.spec = nitor_following();
    p.master.effects.push(slot);
    p.key = key;
    p
}

/// Round 41's review: a stem through the master chain hands each copy of the master's
/// units the song's key (it was tuned to C). The lead alone, through Nitor following a song
/// in G, is the master's own output.
#[test]
fn a_stem_through_the_master_hears_the_song_s_key() {
    let opts = StemOptions {
        render: RenderOptions {
            tail_seconds: 0.25,
            block_frames: 333,
            ..Default::default()
        },
        through_master: true,
        ..Default::default()
    };
    let specs = [
        StemSpec {
            source: StemSource::Strip(0),
            file_name: stem_file_name(1, "lead"),
        },
        StemSpec {
            source: StemSource::Master,
            file_name: stem_file_name(0, "Master"),
        },
    ];
    let p = lead_into_nitor(IN_G);
    let stems = render_stems(Arc::new(Snapshot::compile(&p, 1)), &opts, &specs).unwrap();
    assert!(stems[0].peak > 0.01);
    assert_null(
        &stems[0].samples,
        &stems[1].samples,
        1e-6,
        "the lead through the master, in G",
    );
    // The key is heard: in C the lead's stem is another sound.
    let in_c = lead_into_nitor(IN_C);
    let c = render_stems(Arc::new(Snapshot::compile(&in_c, 1)), &opts, &specs).unwrap();
    let apart = c[0]
        .samples
        .iter()
        .zip(&stems[0].samples)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(apart > 1e-3, "C and G differ by {apart}");
}

/// Round 41's review: the mastering assistant's trial (`listen::run_chain`) hands the
/// chain the song's key, so a trial of a chain with Nitor following the song is what the
/// real render with that chain on the master plays.
#[test]
fn a_trial_of_a_master_chain_is_the_real_render_in_the_song_s_key() {
    let mut p = lead_into_nitor(IN_G);
    let real = render_with(&p, 512);
    p.master.fx_enabled = false;
    let mix = render_with(&p, 512);
    let trial = listen::run_chain(&p.master.effects, true, 48_000, p.key, &mix);
    assert_null(&trial, &real, 1e-6, "the trial against the real render");
    let in_c = listen::run_chain(&p.master.effects, true, 48_000, IN_C, &mix);
    assert!(in_c != real, "the key is heard in the trial");
}
