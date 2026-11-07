//! The voicing lab: Chorda's DSP driven directly (no project, no engine) with a voicing
//! other than the shipped one, so a comparison can try a change without a rebuild. What it
//! renders is what the engine renders for the same notes, the same model and settings —
//! the engine's own path is `render.rs`'s, used for every number the report states.

use super::phrases::Phrase;
use super::render::{Stereo, SR};
use cantus_dsp::chorda::voicing::Voicing;
use cantus_dsp::chorda::{Chorda, ChordaSettings};

/// `key=v,key=v`: a voicing's keys, and the settings `room`, `noise`, `width`, `body`,
/// `hardness`, `tilt`, `decay`, `model`.
pub fn parse(spec: &str) -> (Voicing, ChordaSettings, f64) {
    let mut v = Voicing::default();
    let mut s = ChordaSettings {
        model: 2,
        ..ChordaSettings::default()
    };
    let mut hardness = 0.0;
    for kv in spec.split(',').filter(|x| !x.is_empty()) {
        let (k, val) = kv.split_once('=').expect("key=value");
        let x: f64 = val.parse().expect("a number");
        match k {
            "room" => s.room = x,
            "noise" => s.noise = x,
            "width" => s.width = x,
            "body" => s.body = x,
            "tilt" => s.tilt_db = x,
            "decay" => s.voice.decay = x,
            "hardness" => hardness = x,
            "model" => s.model = x as u8,
            "persp" => s.persp = x,
            "lid" => s.lid = x,
            "roomsize" => s.size = x,
            _ => assert!(v.set(k, x), "no voicing key {k}"),
        }
    }
    (v, s, hardness)
}

/// Render `phrase` through the DSP with `spec`.
pub fn render(spec: &str, phrase: &Phrase) -> Stereo {
    let (v, s, hardness) = parse(spec);
    let kb = cantus_engine::inst::chorda::ensure(Default::default(), SR as f32);
    let mut c = Chorda::new(SR as f32);
    c.set_keyboard(kb);
    c.set_voicing(&v);
    c.set(&s, hardness);
    let sec = |t: f64| (t * SR as f64).round() as usize;
    // Events: (frame, order, what) — offs before ons at one frame, the pedal first.
    #[derive(Clone, Copy)]
    enum E {
        Pedal(f32),
        Off(u8, u64),
        On(u8, f32, u64),
    }
    let mut ev: Vec<(usize, u8, E)> = Vec::new();
    for (i, (t, len, key, vel)) in phrase.notes.iter().enumerate() {
        ev.push((sec(*t), 2, E::On(*key, *vel as f32 / 127.0, i as u64 + 1)));
        ev.push((sec(t + len), 1, E::Off(*key, i as u64 + 1)));
    }
    for (t, p) in &phrase.pedal {
        ev.push((sec(*t), 0, E::Pedal(*p)));
    }
    ev.sort_by_key(|e| (e.0, e.1));
    let n = sec(phrase.len);
    let mut out = Stereo {
        l: vec![0.0; n],
        r: vec![0.0; n],
    };
    let mut at = 0;
    let mut next = 0;
    while at < n {
        while next < ev.len() && ev[next].0 <= at {
            match ev[next].2 {
                E::Pedal(p) => c.set_pedal(p),
                E::Off(k, id) => c.note_off(k, id),
                E::On(k, v, id) => c.note_on(k, v, id),
            }
            next += 1;
        }
        let stop = ev.get(next).map_or(n, |e| e.0.min(n));
        let end = (at + 512).min(stop).max(at + 1).min(n);
        c.process(&mut out.l[at..end], &mut out.r[at..end]);
        at = end;
    }
    out
}
