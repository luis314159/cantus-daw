//! r41:contract (ADR 0153) — the song's key reaches every effect: the engine hands
//! `Snapshot::key` to each unit right after its spec ([`FxUnit::set_song_key`]), on a strip
//! and on the master, and a new snapshot with another key (or none) hands that.

use cantus_core::scale::{Key, ScaleKind};
use cantus_core::*;
use cantus_engine::fx::FxUnit;
use cantus_engine::*;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

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
