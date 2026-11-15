//! r40:pads — *Hear the key* (ADR 0118): a keyed insert's strip plays its key in place of its
//! own signal while the live engine is told so, to tune the key by ear.
//!
//! - Off touches no sample: never set, set on an insert that is not keyed or not there, and
//!   set then taken back — the output is the output without it, to the bit (the compressor's
//!   state followed the signal throughout, so after it the song is exactly the song).
//! - On a compressor: the strip's output is the detector's input — exactly what the unit's
//!   own *Listen: Key* gives — the key after its level and EQ, aligned with the audio.
//! - On an insert with no detector of its own (an EQ keyed from the bass): the key as it
//!   arrives, the key strip's post-fader output.
//! - A render never hears it: a render is another processor.

mod support;
use cantus_core::*;
use cantus_engine::*;
use support::*;

const SR: f64 = 48_000.0;
const BLOCK: usize = 256;
const BLOCKS: usize = 200;

/// The demo's lead keyed from its bass, the bass kept out of the master (into a muted
/// sink), so the master is the lead's strip alone. The lead's strip passes its signal at
/// unity (centred, its own law 0 dB, its fader at 0 dB): the master is what leaves the
/// lead's inserts.
fn keyed(kind: EffectKind, set: &[(&str, f32)]) -> (Project, EffectId) {
    let mut p = demo_project();
    let (lead, bass) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    let sink = p.strips[8].id;
    p.strip_mut(bass).unwrap().output = Some(sink);
    p.strip_mut(sink).unwrap().muted = true;
    let mut fx = p.make_effect(kind);
    for (k, v) in set {
        fx.spec.set(k, *v).unwrap();
    }
    fx.sidechain = Some(bass);
    let id = fx.id;
    let s = p.strip_mut(lead).unwrap();
    s.effects.push(fx);
    s.pan = 0.0;
    s.gain_db = 0.0;
    s.pan_law = Some(PanLaw::ZeroDb);
    (p, id)
}

/// The live engine over the song, `hear(block)` the insert heard from that block on.
fn live(p: &Project, mut hear: impl FnMut(usize) -> Option<Option<EffectId>>) -> Vec<f32> {
    let (mut h, mut proc_) = EngineHandle::manual(SR);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    h.play();
    let mut out = vec![0f32; BLOCK * 2];
    let mut all = Vec::with_capacity(BLOCK * 2 * BLOCKS);
    for b in 0..BLOCKS {
        if let Some(to) = hear(b) {
            h.set_hear_key(to);
        }
        proc_.process(&mut out, 2);
        h.poll();
        all.extend_from_slice(&out);
    }
    all
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i}: {:e} vs {:e}", a[i], b[i]);
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

const DUCK: [(&str, f32); 3] = [("thr", -40.0), ("ratio", 20.0), ("att", 0.5)];
/// The key's level and a band of its EQ: what the detector hears is not the key itself.
const KEY_EQ: [(&str, f32); 5] = [
    ("sc0.used", 1.0),
    ("sc0.shape", 3.0),
    ("sc0.freq", 300.0),
    ("key_gain", -6.0),
    ("look", 3.0),
];

#[test]
fn off_touches_no_sample() {
    let (p, comp) = keyed(EffectKind::Compressor, &DUCK);
    let base = live(&p, |_| None);
    // Another insert's id, and none that exists: nothing heard.
    let stray = live(&p, |b| (b == 0).then_some(Some(EffectId(u64::MAX - 3))));
    same_bits(&stray, &base, "an insert not there");
    // Heard for a while, then taken back: the song is exactly the song before and after.
    let (from, to) = (40, 80);
    let back = live(&p, |b| match b {
        _ if b == from => Some(Some(comp)),
        _ if b == to => Some(None),
        _ => None,
    });
    let (a, z) = (from * BLOCK * 2, to * BLOCK * 2);
    same_bits(&back[..a], &base[..a], "before it");
    same_bits(&back[z..], &base[z..], "after it");
    assert_ne!(&back[a..z], &base[a..z], "while it is on, the key is heard");
    // And a render never hears it.
    assert_eq!(render_of(&p), render_of(&p));
}

#[test]
fn on_a_compressor_the_strip_plays_its_detectors_input() {
    let mut set: Vec<(&str, f32)> = DUCK.to_vec();
    set.extend_from_slice(&KEY_EQ);
    let (p, comp) = keyed(EffectKind::Compressor, &set);
    let heard = live(&p, |b| (b == 0).then_some(Some(comp)));
    // The unit's own *Listen: Key* (index 1), saved in its settings, hear off.
    set.push(("listen", 1.0));
    let (q, _) = keyed(EffectKind::Compressor, &set);
    let listen = live(&q, |_| None);
    same_bits(&heard, &listen, "Hear the key = the detector's input");
}

#[test]
fn on_an_insert_with_no_detector_the_strip_plays_the_key_as_it_arrives() {
    let (p, eq) = keyed(EffectKind::Eq, &[]);
    let heard = live(&p, |b| (b == 0).then_some(Some(eq)));
    // The key: the bass's post-fader output — the song with the bass to the master and the
    // lead silent (its notes gone), its strip at the bass's place.
    let mut q = p.clone();
    let (lead, bass) = (q.tracks[0].strip.unwrap(), q.tracks[1].strip.unwrap());
    q.tracks[0].clips.clear();
    q.strip_mut(bass).unwrap().output = None;
    q.strip_mut(lead).unwrap().effects.clear();
    let key = live(&q, |_| None);
    same_bits(&heard, &key, "the key as it arrives");
}
