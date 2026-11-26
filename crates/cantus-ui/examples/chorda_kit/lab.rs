//! The voicing lab: Chorda's DSP driven directly (no project, no engine) with a voicing
//! other than the shipped one, so a comparison can try a change without a rebuild. What it
//! renders is what the engine renders for the same notes, the same model and settings —
//! the engine's own path is `render.rs`'s, used for every number the report states.

use super::phrases::Phrase;
use super::render::{Stereo, SR};
use cantus_dsp::chorda::voicing::Voicing;
use cantus_dsp::chorda::{Chorda, ChordaSettings};

/// `key=v,key=v`: a voicing's keys, and the settings `room`, `noise`, `width`, `body`,
/// `hardness`, `tilt`, `decay`, `model`. Every spec is read over beta.14's voicing, as every
/// candidate was made (`DSP_VERSION` 7 shipped candidate 16: its own spec and tables give
/// the shipped voicing; an empty spec, beta.14's).
pub fn parse(spec: &str) -> (Voicing, ChordaSettings, f64) {
    // Round 42 (`chorda-c17`): a spec may start with `base=<voicing>` — `shipped` (what a new
    // Chorda plays), `unison` (`DSP_VERSION` 11–15's: candidate 16 with the technician's and the
    // unison passes), `c17` (round 42's candidate 17 over the unison pass, `chorda-c18`) or `c16`
    // (candidate 16 alone, what the owner's ABX kit 16 played) — and is
    // then read over that voicing, its own tables kept unless `CHORDA_BASE_TABLES` is set (then
    // the environment's tables replace them).
    let (mut v, spec, tables) = match spec.split_once(',').unwrap_or((spec, "")) {
        ("base=shipped", rest) => (Voicing::shipped(), rest, false),
        ("base=unison", rest) => (Voicing::unison(), rest, false),
        ("base=c17", rest) => (Voicing::c17(), rest, false),
        ("base=c16", rest) => (Voicing::candidate16(), rest, false),
        _ => (Voicing::beta14(), spec, true),
    };
    let tables = tables || std::env::var_os("CHORDA_BASE_TABLES").is_some();
    // A new Chorda's own parameters, as the engine reads them (round 40: the lab began from
    // the DSP's defaults, 3 dB louder than the engine's staged gain — its fortissimo chord
    // met the output ceiling the engine's does not).
    let mut ns = cantus_core::NativeSpec::new(cantus_core::InstrumentKind::Chorda);
    ns.set("model", 2.0);
    let (mut s, mut hardness): (ChordaSettings, f64) =
        cantus_engine::inst::chorda::settings_from(&cantus_engine::inst::chorda::values_of(&ns));
    // Round 40: each key's partial voicing from a file (`CHORDA_PARTIALS`: lines of
    // `key,g2,…,g7` in dB), for trying a fitted table without a rebuild.
    if let Some(path) = std::env::var("CHORDA_PARTIALS").ok().filter(|_| tables) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                let f: Vec<f32> = line
                    .split(',')
                    .filter_map(|x| x.trim().parse().ok())
                    .collect();
                if f.len() == 7 && (21.0..=108.0).contains(&f[0]) {
                    let i = f[0] as usize - 21;
                    for (j, g) in f[1..].iter().enumerate() {
                        v.partials[i][j] = *g;
                    }
                }
            }
        }
    }
    // Round 40: each key's own level from a file (`CHORDA_LEVELS`: lines of `key,dB`).
    if let Some(path) = std::env::var("CHORDA_LEVELS").ok().filter(|_| tables) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                let f: Vec<f32> = line
                    .split(',')
                    .filter_map(|x| x.trim().parse().ok())
                    .collect();
                if f.len() == 2 && (21.0..=108.0).contains(&f[0]) {
                    v.levels[f[0] as usize - 21] = f[1];
                }
            }
        }
    }
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
            "gain" => s.gain_db = x,
            "sym" => s.sympathetic = x,
            "detune" => s.voice.detune_ct = x,
            "colour" => s.colour = x,
            _ => assert!(v.set(k, x), "no voicing key {k}"),
        }
    }
    (v, s, hardness)
}

/// Render `phrase` through the DSP with `spec`.
pub fn render(spec: &str, phrase: &Phrase) -> Stereo {
    let (v, s, hardness) = parse(spec);
    // Model 2 plays its own keyboard (its voicing's asymptotes and treble octaves), as the
    // engine's `tuning_of` asks for it.
    let tuning = if s.model == 2 {
        v.tuning(Default::default())
    } else {
        Default::default()
    };
    let kb = cantus_engine::inst::chorda::ensure(tuning, SR as f32);
    let mut c = Chorda::new(SR as f32);
    c.set_keyboard(kb);
    c.set_voicing(&v);
    c.set(&s, hardness);
    if std::env::var_os("CHORDA_EVENING").is_some() {
        let g: Vec<String> = (21u8..=108)
            .step_by(3)
            .map(|k| format!("{:+.1}", 20.0 * cantus_dsp::math::log10(c.evening(k))))
            .collect();
        eprintln!("evening {}", g.join(" "));
    }
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
