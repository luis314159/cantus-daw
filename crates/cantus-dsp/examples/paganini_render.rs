//! A score played on Arcus, offline (round 42, the Paganini comparison —
//! `tools/listening/paganini_*.py` write the score and read the result): every note with its
//! own articulation and bow, the controllers at their times, sample-accurate; the violin
//! cantabile preset as the engine reads it (`views::inst::arcus_upper_presets`, `bow.pos`
//! 1.09 → −0.249), the player on. And the room: one of Aula's factory responses, shaped
//! as the unit shapes it, written out to be convolved with the dry render.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example paganini_render -- <score.txt> <out.wav>
//! cargo run -p cantus-dsp --release --example paganini_render -- ir <index> <low Hz> <high Hz> <out.wav>
//! ```
//!
//! The score, one line each (`#` a comment; times in seconds):
//!
//! ```text
//! set <field> <value>                       # vib_depth, dynamic, player, bow_pos, …
//! cc <t> <which> <value>                    # Arcus::controller: 0 CC1, 1 CC11, 4 CC21, …
//! note <on> <off> <key> <vel 0..1> <art> <bow> [string]
//!                                           # art: - sus leg stac spic pizz trem; bow: - d u;
//!                                           # string: 0 the lowest (G) … 3 (E), or -
//! ```
//!
//! `set pair 1` turns on one bow on two strings (`bow.pair`, ADR 0164): a note on another
//! string leaves the one the bow left ringing (a crossing), a note held under another is
//! bowed with it — what the Fiddle preset uses, here on the classical violin.
//!
//! A note whose `on` comes before the sounding one's `off` slurs (legato mode); one that
//! starts at or after it is a new bow. At one instant controllers come first, then the
//! note-offs, then the note-ons. The output: 48 kHz, stereo, 32-bit float.

use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{Arcus, ArcusSettings, Art, BowDir, NoteHow};
use cantus_dsp::aula_gen;
use cantus_dsp::aula_shape::{self, Shape};
use std::path::Path;

const SR: f32 = 48_000.0;
const TAIL_S: f64 = 3.0;

/// The violin cantabile preset (`arcus.preset.violin_cantabile`) as the engine reads it.
fn cantabile() -> ArcusSettings {
    ArcusSettings {
        instrument: Instrument::Violin,
        vib_depth: 1.2,
        vib_delay_ms: 160.0,
        vib_attack_ms: 320.0,
        bow_pos: -0.249,
        bow_attack: 1.4,
        dynamic: 0.65,
        body_ring: 1.15,
        gain_db: 6.6,
        player: 1.0,
        // r42 arcus-classical: `ARCUS_SHAPE=name=value,…` tries a stroke shape (never stored).
        shape_try: std::env::var("ARCUS_SHAPE")
            .ok()
            .map(|t| cantus_dsp::arcus::shape::KNOBS.parse(&t)),
        // … and `ARCUS_EQ=g0,…,g11` the violin's match correction (dB per band).
        matched: std::env::var("ARCUS_EQ").ok().map(|t| {
            let mut m = Instrument::Violin.spec().matched;
            for (g, v) in m.eq_db.iter_mut().zip(t.split(',')) {
                if let Ok(v) = v.trim().parse() {
                    *g = v;
                }
            }
            m
        }),
        ..ArcusSettings::default()
    }
}

fn set_field(s: &mut ArcusSettings, name: &str, v: f32) {
    match name {
        "vib_depth" => s.vib_depth = v,
        "vib_rate" => s.vib_rate = v,
        "vib_delay_ms" => s.vib_delay_ms = v,
        "vib_attack_ms" => s.vib_attack_ms = v,
        "vib_var" => s.vib_var = v,
        "dynamic" => s.dynamic = v,
        "vel_dyn" => s.vel_dyn = v,
        "vel_attack" => s.vel_attack = v,
        "player" => s.player = v,
        "bow_pos" => s.bow_pos = v,
        "bow_force" => s.bow_force = v,
        "bow_speed" => s.bow_speed = v,
        "bow_attack" => s.bow_attack = v,
        "bow_release_ms" => s.bow_release_ms = v,
        "change_ms" => s.change_ms = v,
        "finger_ms" => s.finger_ms = v,
        "porta_ms" => s.porta_ms = v,
        "ring_s" => s.ring_s = v,
        "body_ring" => s.body_ring = v,
        "gain_db" => s.gain_db = v,
        "seed" => s.seed = v as u32,
        "fine_ct" => s.fine_ct = v,
        "pair" => s.pair = v,
        "tilt" => s.tilt = v,
        "just" => s.just = v,
        "sym" => s.sym = v,
        _ => panic!("no field {name}"),
    }
}

#[derive(Clone, Copy)]
enum Ev {
    Cc(u8, f32),
    Off(u8, u64),
    On(u8, f32, u64, NoteHow),
}

impl Ev {
    fn order(&self) -> u8 {
        match self {
            Ev::Cc(..) => 0,
            Ev::Off(..) => 1,
            Ev::On(..) => 2,
        }
    }
}

fn art_of(s: &str) -> Option<Art> {
    match s {
        "sus" => Some(Art::Sustain),
        "leg" => Some(Art::Legato),
        "stac" => Some(Art::Staccato),
        "spic" => Some(Art::Spiccato),
        "pizz" => Some(Art::Pizzicato),
        "trem" => Some(Art::Tremolo),
        _ => None,
    }
}

fn bow_of(s: &str) -> Option<BowDir> {
    match s {
        "d" => Some(BowDir::Down),
        "u" => Some(BowDir::Up),
        _ => None,
    }
}

fn write_wav(path: &Path, channels: &[Vec<f32>]) {
    let spec = hound::WavSpec {
        channels: channels.len() as u16,
        sample_rate: SR as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("the output");
    let n = channels.iter().map(|c| c.len()).max().unwrap_or(0);
    for i in 0..n {
        for c in channels {
            w.write_sample(c.get(i).copied().unwrap_or(0.0))
                .expect("a sample");
        }
    }
    w.finalize().expect("the output written");
}

fn render(score: &Path, out: &Path) {
    let text = std::fs::read_to_string(score).expect("the score");
    let mut s = cantabile();
    let mut evs: Vec<(u64, Ev)> = Vec::new();
    let mut last = 0.0f64;
    let mut id = 0u64;
    let at = |t: f64| (t * SR as f64).round().max(0.0) as u64;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            [] => {}
            ["set", name, v] => set_field(&mut s, name, v.parse().expect("a value")),
            ["cc", t, which, v] => {
                let t: f64 = t.parse().expect("a time");
                evs.push((
                    at(t),
                    Ev::Cc(which.parse().expect("a cc"), v.parse().expect("v")),
                ));
            }
            ["note", on, off, key, vel, art, bow, rest @ ..] if rest.len() <= 1 => {
                let (on, off): (f64, f64) = (on.parse().expect("on"), off.parse().expect("off"));
                let key: u8 = key.parse().expect("a key");
                id += 1;
                let how = NoteHow {
                    art: art_of(art),
                    bow: bow_of(bow),
                    string: rest.first().and_then(|x| x.parse().ok()),
                    ..NoteHow::default()
                };
                evs.push((at(on), Ev::On(key, vel.parse().expect("vel"), id, how)));
                evs.push((at(off), Ev::Off(key, id)));
                last = last.max(off);
            }
            other => panic!("a line not understood: {other:?}"),
        }
    }
    evs.sort_by_key(|(t, e)| (*t, e.order()));
    // r42 arcus-classical: the playback's look-ahead, as the engine hands it (ADR 0166,
    // `processor::look_ahead`): each note's length and the next note's distance from its end
    // (`PAGANINI_LIVE=1`: none, as a live note).
    if std::env::var("PAGANINI_LIVE").ok().as_deref() != Some("1") {
        let offs: std::collections::HashMap<u64, u64> = evs
            .iter()
            .filter_map(|(t, e)| match e {
                Ev::Off(_, id) => Some((*id, *t)),
                _ => None,
            })
            .collect();
        let ons: Vec<u64> = evs
            .iter()
            .filter_map(|(t, e)| matches!(e, Ev::On(..)).then_some(*t))
            .collect();
        for (t, e) in evs.iter_mut() {
            if let Ev::On(_, _, id, how) = e {
                let off = offs[id];
                how.len = Some((off - *t) as u32);
                how.next = ons
                    .iter()
                    .find(|&&o| o > *t)
                    .map(|&o| (o as i64 - off as i64) as i32);
            }
        }
    }
    let mut a = Arcus::new(SR);
    a.set(&s);
    let frames = at(last + TAIL_S) as usize;
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    let mut pos = 0usize;
    let mut k = 0usize;
    while pos < frames {
        while k < evs.len() && evs[k].0 as usize <= pos {
            match evs[k].1 {
                Ev::Cc(which, v) => a.controller(which, v),
                Ev::Off(key, id) => a.note_off(key, id),
                Ev::On(key, vel, id, how) => {
                    a.note_how(id, how);
                    if let Some(len) = how.len {
                        a.note_len(id, len, how.next);
                    }
                    a.note_on(key, vel, id);
                }
            }
            k += 1;
        }
        let next = if k < evs.len() {
            evs[k].0 as usize
        } else {
            frames
        };
        let end = next
            .min(frames)
            .min(pos + cantus_dsp::arcus::CHUNK)
            .max(pos + 1);
        a.process(&mut l[pos..end], &mut r[pos..end]);
        pos = end;
    }
    let peak = l.iter().chain(&r).fold(0.0f32, |m, x| m.max(x.abs()));
    write_wav(out, &[l, r]);
    println!(
        "{}: {} notes, {:.1} s, peak {:.1} dBFS",
        out.display(),
        id,
        frames as f64 / SR as f64,
        20.0 * cantus_dsp::math::log10(peak.max(1e-9))
    );
}

fn room(index: usize, low: f32, high: f32, out: &Path) {
    let src = aula_gen::factory(index, SR);
    let shape = Shape {
        low_hz: low,
        high_hz: high,
        ..Shape::default()
    };
    let ir = aula_shape::shape(&src, SR, &shape, true);
    println!(
        "{}: {} ({} paths), {:.2} s",
        out.display(),
        aula_gen::FACTORY[index.min(aula_gen::FACTORY.len() - 1)].key,
        ir.channels.len(),
        ir.seconds()
    );
    write_wav(out, &ir.channels);
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.as_slice() {
        [ir, index, low, high, out] if ir == "ir" => room(
            index.parse().expect("an index"),
            low.parse().expect("Hz"),
            high.parse().expect("Hz"),
            Path::new(out),
        ),
        [score, out] => render(Path::new(score), Path::new(out)),
        _ => {
            eprintln!(
                "usage: paganini_render <score.txt> <out.wav> | ir <index> <low> <high> <out.wav>"
            );
            std::process::exit(2);
        }
    }
}
