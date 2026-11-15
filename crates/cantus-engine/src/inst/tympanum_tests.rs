//! The unit's own rules (round 40 phase 3, after checkpoint B): the modelled studio kit's
//! acoustic voices render the same bits however the block is cut; every new key at its
//! default touches no sample; the *Feel* card's timing lands a hit on its own frame
//! whatever the block; the foot's chick can be turned off; each mic's peak is in the
//! telemetry at `METER_TEL + n`.

use super::*;
use crate::inst::{InstrumentUnit, NoteOn};
use cantus_core::inst::tympanum::KIT_KEY;
use cantus_core::InstrumentKind;

fn spec(keys: &[(&str, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
    s.set_text(KIT_KEY, Some("model:studio".into())).unwrap();
    for (k, v) in keys {
        s.set(k, *v);
    }
    s
}

/// The groove's events: (frame, key, velocity 1…127) or a pedal move (key 0, CC4 value).
const EVENTS: [(usize, u8, u8); 10] = [
    (0, 36, 118),
    (0, 42, 90),
    (3_000, 38, 112),
    (6_000, 42, 60),
    (9_100, 45, 100),
    (12_000, 49, 120),
    (15_000, 0, 120),
    (15_050, 46, 100),
    (21_000, 0, 2),
    (24_000, 51, 96),
];

/// Render the groove through a fresh unit in blocks of `block`.
fn render(s: &NativeSpec, block: usize, frames: usize) -> (Vec<f32>, Vec<f32>, Unit) {
    // The app warms the modelled kits with each snapshot (`cache::prepare`).
    cache::warm_modelled("studio");
    let mut u = Unit::new(48_000.0);
    u.configure(s, &[]);
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    let mut at = 0;
    let mut e = 0;
    let mut ord = [0u32; 128];
    while at < frames {
        while e < EVENTS.len() && EVENTS[e].0 <= at {
            let (_, key, v) = EVENTS[e];
            if key == 0 {
                u.controller(Controller::Cc(4), v as f32 / 127.0);
            } else {
                let o = ord[key as usize];
                ord[key as usize] += 1;
                u.note_on_seq(
                    NoteOn {
                        key,
                        velocity: v as f32 / 127.0,
                        id: e as u64 + 1,
                        channel: 9,
                    },
                    o,
                );
            }
            e += 1;
        }
        let next = EVENTS.get(e).map_or(frames, |x| x.0.min(frames));
        let end = (at + block).min(next.max(at + 1)).min(frames);
        u.process(&mut l[at..end], &mut r[at..end]);
        at = end;
    }
    (l, r, u)
}

#[test]
fn the_acoustic_kit_renders_the_same_bits_however_the_block_is_cut() {
    let s = spec(&[]);
    let n = 30_000;
    let (l, r, _) = render(&s, 512, n);
    assert!(l.iter().any(|x| x.abs() > 1e-3), "the kit sounds");
    for block in [1, 37, 64, 333] {
        let (l2, r2, _) = render(&s, block, n);
        assert!(l == l2 && r == r2, "block {block}");
    }
}

#[test]
fn every_new_key_at_its_default_touches_no_sample() {
    let n = 30_000;
    let (l, r, _) = render(&spec(&[]), 256, n);
    let defaults = spec(&[
        ("timing", 0.0),
        ("vary", 1.0),
        ("hat.chick", 1.0),
        ("tune", 0.0),
        ("ring", 1.0),
        ("floor", 0.0),
        ("walls", 0.0),
        ("rug", 0.0),
        ("humanize", 0.0),
        ("beater", 0.0),
        ("p1.pos", 0.0),
        ("p1.spread", cantus_core::inst::tympanum::SPREAD),
    ]);
    let (l2, r2, _) = render(&defaults, 256, n);
    assert!(l == l2 && r == r2);
    // And each, moved, does change it.
    for (k, v) in [
        ("tune", 2.0),
        ("ring", 0.5),
        ("humanize", 0.5),
        ("p1.pos", 0.8),
        ("walls", 8.0),
        ("beater", 1.0),
    ] {
        let (l3, _, _) = render(&spec(&[(k, v)]), 256, n);
        assert!(l3 != l, "{k} moved changes the sound");
    }
}

#[test]
fn the_feel_cards_timing_lands_each_hit_on_its_own_frame_whatever_the_block() {
    let s = spec(&[("timing", 12.0), ("seed", 7.0)]);
    let n = 30_000;
    let (l, r, _) = render(&s, 512, n);
    for block in [1, 64, 300] {
        let (l2, r2, _) = render(&s, block, n);
        assert!(l == l2 && r == r2, "block {block}");
    }
    let (plain, _, _) = render(&spec(&[]), 512, n);
    // Nothing comes early: up to the first hit's moment both are silent, and it is late.
    let first = |x: &[f32]| x.iter().position(|v| v.abs() > 1e-4).unwrap();
    assert!(first(&l) >= first(&plain));
    assert!(l != plain);
}

#[test]
fn the_foot_chick_can_be_turned_off() {
    let n = 30_000;
    let (on, _, _) = render(&spec(&[]), 256, n);
    let (off, _, _) = render(&spec(&[("hat.chick", 0.0)]), 256, n);
    // Identical until the pedal closes the open hat (frame 21 000), different after.
    assert_eq!(on[..21_000], off[..21_000]);
    assert!(on[21_000..] != off[21_000..]);
}

#[test]
fn each_mics_peak_is_in_the_telemetry() {
    let (_, _, u) = render(&spec(&[]), 512, 400);
    let t = u.telemetry();
    let count = u.outs.count;
    assert!(count >= 10, "the studio kit's mics: {count}");
    assert!(
        t[METER_TEL..METER_TEL + count].iter().any(|p| *p > 0.0),
        "{t:?}"
    );
    assert!(t[..METER_TEL].iter().all(|p| *p == 0.0));
}

/// A fresh studio unit.
fn unit() -> Unit {
    cache::warm_modelled("studio");
    let mut u = Unit::new(48_000.0);
    u.configure(&spec(&[]), &[]);
    u
}

fn hit(u: &mut Unit, key: u8, id: u64) {
    let note = NoteOn {
        key,
        velocity: 100.0 / 127.0,
        id,
        channel: 9,
    };
    u.note_on_seq(note, 0);
}

#[test]
fn a_flam_is_two_strokes_and_a_held_roll_ends_with_its_note() {
    // A flam: the grace now, the stroke 18–28 ms after, louder.
    let mut u = unit();
    hit(&mut u, 91, 7);
    assert_eq!(u.n_pending, 1);
    let (left, key, vel) = (u.pending[0].left, u.pending[0].key, u.pending[0].vel);
    let ms = left as f32 / 48.0;
    assert!(
        (18.0..=28.0).contains(&ms) && key == 91 && vel == 100,
        "{ms} ms, {vel}"
    );
    // A drag: two strokes still to come; a buzz: seven bounces, each softer.
    let mut u = unit();
    hit(&mut u, 92, 8);
    assert_eq!(u.n_pending, 2);
    let mut u = unit();
    hit(&mut u, 93, 9);
    assert_eq!(u.n_pending, 7);
    let mut v: Vec<(u32, u8)> = u.pending[..7].iter().map(|p| (p.left, p.vel)).collect();
    v.sort();
    assert!(v.windows(2).all(|w| w[1].1 <= w[0].1), "{v:?}");
    // A held roll: one stroke waits at a time (fix-e), the next scheduled as each sounds;
    // the note-off lets the waiting one go, and no more come.
    let mut u = unit();
    hit(&mut u, 94, 10);
    assert_eq!(u.n_pending, 1);
    let (mut l, mut r) = (vec![0.0; 9_600], vec![0.0; 9_600]);
    u.process(&mut l, &mut r);
    assert_eq!(u.n_pending, 1, "200 ms in: the next stroke waits");
    u.note_off(94, 10, 0.0);
    assert_eq!(u.n_pending, 0);
    // The plain stroke on the snare's centre waits for nothing.
    let mut u = unit();
    hit(&mut u, 38, 11);
    assert_eq!(u.n_pending, 0);
}

#[test]
fn a_recorded_kit_in_one_of_cantuss_rooms_gets_a_unit_with_aulas_response() {
    use crate::inst::UnitKind;
    use cantus_core::InstrumentSpec;
    let kind = |kit: &str, room: f32| {
        let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
        s.set_text(KIT_KEY, Some(kit.into())).unwrap();
        s.set("room", room);
        UnitKind::of(&InstrumentSpec::Native(s))
    };
    assert_eq!(
        kind("pack:x/kit", 2.0),
        Some(UnitKind::Drums { room: 2, delays: 0 })
    );
    assert_eq!(
        kind("pack:x/kit", 3.0),
        Some(UnitKind::Drums { room: 3, delays: 0 })
    );
    // Its own room, or a modelled kit (whose rooms are its model): the plain unit.
    let plain = Some(UnitKind::Native(InstrumentKind::AcousticDrums));
    assert_eq!(kind("pack:x/kit", 0.0), plain);
    assert_eq!(kind("model:studio", 2.0), plain);
    let u = Unit::with_room(48_000.0, 3);
    assert!(u.aula.is_some() && u.aula_room == 3);
    assert!(
        u.aula.as_deref().unwrap().at_rest(),
        "it starts silent and at rest"
    );
}

#[test]
fn an_output_delay_gets_a_unit_with_its_line_and_no_other() {
    use crate::inst::UnitKind;
    use cantus_core::InstrumentSpec;
    let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
    s.set_text(KIT_KEY, Some("model:studio".into())).unwrap();
    assert_eq!(
        UnitKind::of(&InstrumentSpec::Native(s.clone())),
        Some(UnitKind::Native(InstrumentKind::AcousticDrums)),
        "no delay: the plain unit, no line"
    );
    s.set("o2.delay", 3.0);
    assert_eq!(
        UnitKind::of(&InstrumentSpec::Native(s)),
        Some(UnitKind::Drums {
            room: 0,
            delays: 1 << 2
        })
    );
    let u = super::Unit::with_delays(48_000.0, 1 << 2);
    assert_eq!(
        u.outs.delay_mask(),
        1 << 2,
        "a line for output 2 and no other"
    );
    // The plain unit owns no line (fix-e's Outputs: a mic without a line is undelayed).
    let mut plain = super::Unit::new(48_000.0);
    assert_eq!(plain.outs.delay_mask(), 0);
    assert!(plain.outs.line(2).is_none());
}

/// The modelled kit's whole cost — Pulsus's model 5 voices *and* the virtual microphones'
/// paths, every mic on — for a busy groove (30 s): `cargo test --release -p cantus-engine
/// --lib full_kit_with_mics_cost -- --ignored --nocapture`. Prints the share of one core.
#[test]
#[ignore]
fn full_kit_with_mics_cost() {
    cache::warm_modelled("studio");
    let sixteenth = 6_000usize;
    let mut ev: Vec<(usize, u8, u8)> = Vec::new();
    for step in 0..240usize {
        let at = step * sixteenth;
        ev.push((
            at,
            if step % 16 == 14 { 46 } else { 42 },
            70 + (step % 4) as u8 * 15,
        ));
        if step % 8 == 0 {
            ev.push((at, 36, 110));
        }
        if step % 8 == 4 {
            ev.push((at, 38, 115));
        }
        if step % 4 == 2 {
            ev.push((at, 51, 90));
        }
        if step % 32 == 0 {
            ev.push((at, 49, 120));
        }
        if step % 32 == 28 {
            ev.push((at, 45, 110));
        }
    }
    let n = 240 * sixteenth;
    // The best of five (a busy machine only ever adds).
    let mut best = f64::MAX;
    for _ in 0..5 {
        let mut u = Unit::new(48_000.0);
        u.configure(&spec(&[]), &[]);
        let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        let mut e = 0;
        let t = std::time::Instant::now();
        let mut at = 0;
        while at < n {
            while e < ev.len() && ev[e].0 <= at {
                let (_, key, v) = ev[e];
                u.note_on_seq(
                    NoteOn {
                        key,
                        velocity: v as f32 / 127.0,
                        id: e as u64 + 1,
                        channel: 9,
                    },
                    e as u32,
                );
                e += 1;
            }
            l.fill(0.0);
            r.fill(0.0);
            u.process(&mut l, &mut r);
            at += 256;
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    let secs = n as f64 / 48_000.0;
    eprintln!(
        "full modelled kit, every mic on: {:.1} % of a core",
        100.0 * best / secs
    );
}

/// drums-gen 3 — the cost test's groove (and the ride's bell, the floor tom) rendered through
/// each modelled kit, every mic on, written as interleaved f32 to `$CANTUS_TYMP_DUMP/<kit>.f32`:
/// a saving's before and after compared sample by sample (`cargo test --release -p
/// cantus-engine --lib full_kit_dump -- --ignored`).
#[test]
#[ignore]
fn full_kit_dump() {
    let Some(dir) = std::env::var_os("CANTUS_TYMP_DUMP") else {
        return;
    };
    let sixteenth = 6_000usize;
    let mut ev: Vec<(usize, u8, u8)> = Vec::new();
    for step in 0..96usize {
        let at = step * sixteenth;
        let hat = if step % 16 == 14 { 46 } else { 42 };
        ev.push((at, hat, 40 + (step % 4) as u8 * 29));
        if step % 8 == 0 {
            ev.push((at, 36, 60 + (step % 32) as u8 * 2));
        }
        if step % 8 == 4 {
            ev.push((at, 38, 50 + (step % 24) as u8 * 3));
        }
        if step % 4 == 2 {
            ev.push((at, 51, 90));
        }
        if step % 32 == 0 {
            ev.push((at, 49, 120));
        }
        if step % 32 == 28 {
            ev.push((at, 45, 110));
        }
        if step % 32 == 30 {
            ev.push((at, 43, 70));
        }
        if step % 32 == 20 {
            ev.push((at, 53, 100));
        }
    }
    let n = 96 * sixteenth + 3 * 48_000;
    for kit in ["studio", "jazz"] {
        cache::warm_modelled(kit);
        let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
        s.set_text(KIT_KEY, Some(format!("model:{kit}"))).unwrap();
        let mut u = Unit::new(48_000.0);
        u.configure(&s, &[]);
        let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        let mut bytes = Vec::with_capacity(n * 8);
        let (mut e, mut at) = (0, 0);
        let mut ord = [0u32; 128];
        while at < n {
            while e < ev.len() && ev[e].0 <= at {
                let (_, key, v) = ev[e];
                let o = ord[key as usize];
                ord[key as usize] += 1;
                u.note_on_seq(
                    NoteOn {
                        key,
                        velocity: v as f32 / 127.0,
                        id: e as u64 + 1,
                        channel: 9,
                    },
                    o,
                );
                e += 1;
            }
            l.fill(0.0);
            r.fill(0.0);
            u.process(&mut l, &mut r);
            for (a, b) in l.iter().zip(&r) {
                bytes.extend_from_slice(&a.to_le_bytes());
                bytes.extend_from_slice(&b.to_le_bytes());
            }
            at += 256;
        }
        std::fs::write(std::path::Path::new(&dir).join(format!("{kit}.f32")), bytes).unwrap();
    }
}

/// drums-gen 3 — the cost split: the unit whole against model 5's voices alone (the rest is
/// the paths, the room, the outputs), and the paths counted.
#[test]
#[ignore]
fn cost_split() {
    cache::warm_modelled("studio");
    let sixteenth = 6_000usize;
    let mut ev: Vec<(usize, u8, u8)> = Vec::new();
    for step in 0..240usize {
        let at = step * sixteenth;
        ev.push((
            at,
            if step % 16 == 14 { 46 } else { 42 },
            70 + (step % 4) as u8 * 15,
        ));
        if step % 8 == 0 {
            ev.push((at, 36, 110));
        }
        if step % 8 == 4 {
            ev.push((at, 38, 115));
        }
        if step % 4 == 2 {
            ev.push((at, 51, 90));
        }
        if step % 32 == 0 {
            ev.push((at, 49, 120));
        }
        if step % 32 == 28 {
            ev.push((at, 45, 110));
        }
    }
    let n = 240 * sixteenth;
    for (voices_only, ftz) in [(false, false), (false, true)] {
        let _guard = ftz.then(cantus_rt::DenormalGuard::new);
        let mut best = f64::MAX;
        for _ in 0..3 {
            let mut u = Unit::new(48_000.0);
            u.configure(&spec(&[]), &[]);
            if !voices_only {
                eprintln!("paths, live, groups: {:?}", u.model.path_stats());
            }
            let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
            let (mut e, mut at) = (0, 0);
            let t = std::time::Instant::now();
            while at < n {
                while e < ev.len() && ev[e].0 <= at {
                    let (_, key, v) = ev[e];
                    u.note_on_seq(
                        NoteOn {
                            key,
                            velocity: v as f32 / 127.0,
                            id: e as u64 + 1,
                            channel: 9,
                        },
                        e as u32,
                    );
                    e += 1;
                }
                if voices_only {
                    u.model.pulsus.process_buses(256, &mut |_, _, _, _| {});
                } else {
                    l.fill(0.0);
                    r.fill(0.0);
                    u.process(&mut l, &mut r);
                }
                at += 256;
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        eprintln!(
            "{}{}: {:.1} % of a core",
            if voices_only {
                "voices alone"
            } else {
                "the unit whole"
            },
            if ftz {
                ", denormals flushed (as in the app)"
            } else {
                ""
            },
            100.0 * best / (n as f64 / 48_000.0)
        );
    }
}

/// drums-gen 3 — the paths of each piece, printed (`--nocapture`).
#[test]
#[ignore]
fn print_paths() {
    cache::warm_modelled("studio");
    let mut u = Unit::new(48_000.0);
    u.configure(&spec(&[]), &[]);
    for p in 0..10u8 {
        for q in u.model.paths_of(p) {
            eprintln!(
                "piece {p} mic {} ch {} delay {:7.2} gain {:8.5} main {} side {}",
                q.0, q.1, q.2, q.3, q.4, q.5
            );
        }
    }
}

/// drums-gen 3 — one hit's dry source (model 5's lane for its piece) and the unit's output,
/// `CANTUS_TYMP_DUMP=<dir>`: `<dir>/hit<key>.f32`, interleaved src main, src side, out L, out R.
#[test]
#[ignore]
fn dump_hit_source_and_output() {
    let Some(dir) = std::env::var_os("CANTUS_TYMP_DUMP") else {
        return;
    };
    cache::warm_modelled("studio");
    for key in [36u8, 38, 45, 43, 42, 51] {
        let n = 48_000 / 256;
        let mut src = vec![[0.0f32; 2]; n * 256];
        let mut u = Unit::new(48_000.0);
        u.configure(&spec(&[]), &[]);
        let on = NoteOn {
            key,
            velocity: 110.0 / 127.0,
            id: 1,
            channel: 9,
        };
        u.note_on_seq(on, 0);
        for b in 0..n {
            u.model.pulsus.process_buses(256, &mut |_, at, l, r| {
                for (i, (a, c)) in l.iter().zip(r).enumerate() {
                    src[b * 256 + at + i][0] += 0.5 * (a + c);
                    src[b * 256 + at + i][1] += 0.5 * (a - c);
                }
            });
        }
        let mut u = Unit::new(48_000.0);
        u.configure(&spec(&[]), &[]);
        u.note_on_seq(on, 0);
        let mut bytes = Vec::new();
        let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        for b in 0..n {
            l.fill(0.0);
            r.fill(0.0);
            u.process(&mut l, &mut r);
            for i in 0..256 {
                for v in [src[b * 256 + i][0], src[b * 256 + i][1], l[i], r[i]] {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        std::fs::write(
            std::path::Path::new(&dir).join(format!("hit{key}.f32")),
            bytes,
        )
        .unwrap();
    }
}

/// drums-gen 3 — one hit through each mic alone (`o{m}.solo`), `<dir>/solo<key>_<m>.f32`
/// (L, R), and through the unit with `extra` keys (`CANTUS_TYMP_KEYS=k=v,…`) as `<dir>/keys<key>.f32`.
#[test]
#[ignore]
fn dump_hit_per_mic() {
    let Some(dir) = std::env::var_os("CANTUS_TYMP_DUMP") else {
        return;
    };
    cache::warm_modelled("studio");
    let extra: Vec<(String, f32)> = std::env::var("CANTUS_TYMP_KEYS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .filter_map(|(k, v)| Some((k.to_string(), v.parse().ok()?)))
        .collect();
    for key in [36u8, 45, 43] {
        for m in 0..13usize {
            let s = if m < 12 {
                let k = format!("o{m}.solo");
                spec(&[(k.as_str(), 1.0)])
            } else {
                let e: Vec<(&str, f32)> = extra.iter().map(|(k, v)| (k.as_str(), *v)).collect();
                spec(&e)
            };
            let mut u = Unit::new(48_000.0);
            u.configure(&s, &[]);
            u.note_on_seq(
                NoteOn {
                    key,
                    velocity: 110.0 / 127.0,
                    id: 1,
                    channel: 9,
                },
                0,
            );
            let mut bytes = Vec::new();
            let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
            for _ in 0..(48_000 / 256) {
                l.fill(0.0);
                r.fill(0.0);
                u.process(&mut l, &mut r);
                for i in 0..256 {
                    bytes.extend_from_slice(&l[i].to_le_bytes());
                    bytes.extend_from_slice(&r[i].to_le_bytes());
                }
            }
            let name = if m < 12 {
                format!("solo{key}_{m}.f32")
            } else {
                format!("keys{key}.f32")
            };
            std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
        }
    }
}
