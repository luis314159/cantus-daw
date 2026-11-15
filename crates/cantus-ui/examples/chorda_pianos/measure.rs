//! `--measure`: bitKlavier with 1, 2 and 4 images on at once (the coordinator asked):
//! the zones and the unit's zone table, the voices, the CPU a block takes against real
//! time, the sample memory the unit holds and the process's working set — the `ballad`
//! phrase (pedalled chords) through Exemplar's unit as the engine runs it.

use super::phrases::Phrase;
use cantus_core::inst::chorda_source::{self as cs, ChordaSource, RecordedPiano};
use cantus_core::inst_out::OutputId;
use cantus_core::{InstrumentKind, NativeSpec, Snapshot};
use cantus_engine::inst::{Controller, NoteOn};
use cantus_engine::SampleCache;
use std::fmt::Write as _;
use std::path::Path;

/// The images, in the order they are turned on: Stage Lid ORTF, Piano Bar, House ORTF,
/// Decca Tail.
const ORDER: [u16; 4] = [6, 5, 0, 2];
/// Their packs, and Accurate-Salamander's (its installed size is measured too).
pub const PACKS: [&str; 5] = [
    "bitklavier-grand-stage-lid-ortf",
    "bitklavier-grand-piano-bar",
    "bitklavier-grand-house-ortf",
    "bitklavier-grand-decca-tail",
    "accurate-salamander-grand-piano",
];

pub fn run(content: &Path, phrase: &Phrase) -> String {
    let mut out = String::from(
        "images,zones,unit_zone_table,voices_max,cpu_pct_of_realtime,block_us_mean,block_us_max,sample_mib,working_set_mib\n",
    );
    let m = cantus_content::Manager::new(content, cantus_content::Manifest::bundled());
    let set = RecordedPiano {
        pack: "bitklavier-grand".into(),
        instrument: "grand".into(),
    };
    let entry = cantus_ui::views::inst::chorda_source::read::entry(&m, &set)
        .expect("bitKlavier's images installed");
    // `CHORDA_MEASURE_N=2` measures that many images alone (a fresh process each: the first
    // load of an image decodes every file whole, ≈ 5 GB for one of bitKlavier's).
    let counts: Vec<usize> = match std::env::var("CHORDA_MEASURE_N") {
        Ok(v) => v.split(',').filter_map(|x| x.trim().parse().ok()).collect(),
        Err(_) => vec![1, 2, 4],
    };
    for n in counts {
        let on: Vec<u16> = ORDER[..n].to_vec();
        let (zones, left) = cantus_ui::views::inst::chorda_source::read::zones(
            &entry,
            Some(OutputId(ORDER[0])),
            &|o: OutputId| on.contains(&o.0),
        )
        .expect("zones");
        assert!(left.is_empty(), "{n} images fit");
        let spec = cs::with_source(
            &NativeSpec::new(InstrumentKind::Chorda),
            &ChordaSource::Recorded(set.clone()),
            Some(&zones),
        );
        let p = super::render::project(spec, phrase);
        let mut snap = Snapshot::compile(&p, 1);
        let ws0 = cantus_rt::usage::working_set_bytes().unwrap_or(0);
        // As the live engine loads (ADR 0050): long samples stream from the cache files.
        let mut cache = SampleCache::new();
        cache.set_streaming(true);
        let errors = cache.attach(&mut snap);
        assert!(errors.is_empty(), "{errors:?}");
        let mut units = cantus_engine::inst::units_for(&snap, 48_000.0, |_, _| false);
        let mut u = units.remove(0);
        let table = match u.kind {
            cantus_engine::inst::UnitKind::Exemplar { zones, .. } => zones as usize,
            _ => cantus_core::inst::sampler::BASE_ZONES,
        };
        let t = &snap.tracks[0];
        u.unit.configure(t.instrument.native().unwrap(), &t.assets);
        u.unit.set_offline(true);
        let tele = u.unit.telemetry();
        let sample_mib = tele[cantus_engine::inst::sampler::tel::MEMORY_MIB];
        // Twice: the first fills the stream cache; the second is measured.
        let mut stats = (0usize, 0f64, 0f64, 0usize);
        for pass in 0..2 {
            u.unit.reset();
            stats = play(u.unit.as_mut(), phrase);
            let _ = pass;
        }
        let ws1 = cantus_rt::usage::working_set_bytes().unwrap_or(0);
        let (voices, total_us, max_us, blocks) = stats;
        let seconds = phrase.len;
        let _ = writeln!(
            out,
            "{n},{},{table},{voices},{:.2},{:.1},{:.1},{:.1},{:.1}",
            zones_in(&zones),
            total_us / 1e6 / seconds * 100.0,
            total_us / blocks.max(1) as f64,
            max_us,
            sample_mib,
            (ws1 as f64 - ws0 as f64) / 1_048_576.0,
        );
        eprintln!("{}", out.lines().last().unwrap_or_default());
        eprintln!("{n} image(s) measured");
    }
    out
}

fn zones_in(spec: &NativeSpec) -> usize {
    spec.text.keys().filter(|k| k.ends_with(".sample")).count()
}

/// The phrase through the unit in 512-frame blocks: (voices at most, µs in all, µs at
/// most in one block, blocks).
fn play(u: &mut dyn cantus_engine::inst::InstrumentUnit, ph: &Phrase) -> (usize, f64, f64, usize) {
    const SR: f64 = 48_000.0;
    const BLOCK: usize = 512;
    let frames = (ph.len * SR) as usize;
    // (frame, key, velocity, on, the note's id)
    let mut ev: Vec<(usize, u8, u8, bool, u64)> = Vec::new();
    for (i, (start, len, key, vel)) in ph.notes.iter().enumerate() {
        ev.push(((start * SR) as usize, *key, *vel, true, i as u64 + 1));
        ev.push((((start + len) * SR) as usize, *key, 0, false, i as u64 + 1));
    }
    ev.sort_by_key(|e| (e.0, e.3));
    let pedal: Vec<(usize, f32)> = ph
        .pedal
        .iter()
        .map(|(t, v)| ((t * SR) as usize, *v))
        .collect();
    let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
    let (mut at, mut e, mut pd) = (0usize, 0usize, 0usize);
    let (mut voices, mut total, mut most, mut blocks) = (0usize, 0f64, 0f64, 0usize);
    while at < frames {
        while e < ev.len() && ev[e].0 < at + BLOCK {
            let (_, key, v, on, id) = ev[e];
            if on {
                u.note_on(NoteOn {
                    key,
                    velocity: f32::from(v) / 127.0,
                    id,
                    channel: 0,
                });
            } else {
                u.note_off(key, id, 0.5);
            }
            e += 1;
        }
        while pd < pedal.len() && pedal[pd].0 < at + BLOCK {
            u.controller(Controller::Sustain, pedal[pd].1);
            pd += 1;
        }
        l.fill(0.0);
        r.fill(0.0);
        let t = std::time::Instant::now();
        u.process(&mut l, &mut r);
        let us = t.elapsed().as_secs_f64() * 1e6;
        total += us;
        most = most.max(us);
        blocks += 1;
        voices = voices.max(u.active_voices());
        at += BLOCK;
    }
    (voices, total, most, blocks)
}
