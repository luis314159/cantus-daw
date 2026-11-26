//! Model 5's rules: silent without its state and at rest, the same bits however a block is
//! cut, one object per piece (a re-strike adds to the ring), a hat's closed stroke and its
//! pedal choke the open ring, a harder hit louder and brighter, every piece comes to rest.

use super::super::{DrumVoice, PadSettings, Pulsus, MEASURED_FOOT};

#[test]
fn the_tables_blob_reads_and_round_trips_to_the_byte() {
    let blob = include_bytes!("pulsus_measured.bin");
    let t = super::data::decode(blob).expect("this build reads its own blob");
    assert!(t.len() >= 8, "crash, hat, ride, kick, snare, three toms");
    assert!(
        super::data::encode(&t) == blob.as_slice(),
        "decode then encode: the same bytes"
    );
    // A blob of another version is refused, not misread.
    let mut other = blob.to_vec();
    other[4] = 9;
    assert!(super::data::decode(&other).is_none());
    assert!(super::data::decode(&blob[..blob.len() - 1]).is_none());
}

/// The owner's "iuuu" (drums-gen 2): no layer's band swells after its peak — 40 of 128
/// layers rose 11-22 dB between 2.2 and 3.2 s, once per hit, a whistle as the cymbal died.
#[test]
fn no_band_swells_after_its_peak() {
    let blob = include_bytes!("pulsus_measured.bin");
    let tables = super::data::decode(blob).expect("this build reads its own blob");
    let mut swells = Vec::new();
    for t in &tables {
        for (si, s) in t.states.iter().enumerate() {
            for l in &s.layers {
                for b in 0..super::NB {
                    let e = &l.env[b * super::NBP..(b + 1) * super::NBP];
                    let peak = (0..super::NBP)
                        .max_by(|&i, &j| e[i].total_cmp(&e[j]))
                        .unwrap_or(0);
                    for k in peak + 1..super::NBP {
                        if e[k] > e[k - 1] + 0.02 {
                            swells.push((t.id, si, l.vel, b, k, e[k] - e[k - 1]));
                        }
                    }
                }
            }
        }
    }
    assert!(
        swells.is_empty(),
        "bands rising after their peak: {swells:?}"
    );
}

const SR: f32 = 48_000.0;

fn pad(table: u8, id: u16, bus: u8) -> PadSettings {
    let mut s = PadSettings::of(DrumVoice::HiHat, 0);
    s.model = 5;
    s.measured = table;
    s.id = id;
    s.bus = bus;
    s.tune = 0.0;
    s.decay = 1.0;
    s.gain = 1.0;
    s.click = 1.0;
    s.pedal = table & !MEASURED_FOOT == 2;
    s
}

/// A kit of the crash (key 49), the closed and open hat (42, 46) and its foot (44).
fn kit(on: bool) -> Pulsus {
    let mut p = Pulsus::new(SR);
    if on {
        p.enable_measured();
    }
    p.set_kit(3, 7, 4, false);
    p.set_pad_count(4);
    p.set_pad(0, 49, &pad(1, 1, 0));
    let mut closed = pad(2, 2, 1);
    closed.openness = 0.0;
    p.set_pad(1, 42, &closed);
    let mut open = pad(2, 3, 1);
    open.openness = 0.95;
    p.set_pad(2, 46, &open);
    p.set_pad(3, 44, &pad(2 | MEASURED_FOOT, 4, 1));
    p
}

/// `events` (frame, key, velocity; key 0 a pedal move to `velocity / 127`) rendered in
/// blocks of `block`: the summed lanes.
fn play(p: &mut Pulsus, events: &[(usize, u8, u8)], n: usize, block: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    let mut at = 0;
    let mut e = 0;
    while at < n {
        while e < events.len() && events[e].0 <= at {
            let (_, key, vel) = events[e];
            if key == 0 {
                p.controller(4, vel as f32 / 127.0);
            } else {
                p.note_on(key, vel, e as u64 + 1);
            }
            e += 1;
        }
        let next = events.get(e).map_or(n, |x| x.0.max(at + 1));
        let c = (n - at).min(block).min(next - at);
        p.process_buses(c, &mut |_, off, l, r| {
            for (i, (a, b)) in l.iter().zip(r).enumerate() {
                out[at + off + i] += 0.5 * (a + b);
            }
        });
        at += c;
    }
    out
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

/// Spectral centroid by zero crossings' proxy: the first difference's RMS over the RMS.
fn brightness(x: &[f32]) -> f32 {
    let d: Vec<f32> = x.windows(2).map(|w| w[1] - w[0]).collect();
    rms(&d) / rms(x).max(1e-20)
}

const GROOVE: [(usize, u8, u8); 9] = [
    (100, 49, 110),
    (2_000, 42, 80),
    (9_000, 46, 100),
    (15_000, 42, 70),
    (21_000, 49, 60),
    (21_000, 46, 127),
    (30_000, 0, 0),
    (31_000, 44, 90),
    (40_000, 42, 127),
];

#[test]
fn silent_without_its_state_and_at_rest() {
    let mut p = kit(false);
    let x = play(&mut p, &GROOVE, 50_000, 256);
    assert!(
        x.iter().all(|v| *v == 0.0),
        "a model 5 pad without its bank is silent"
    );
    let mut p = kit(true);
    let mut called = false;
    assert!(!p.process_buses(512, &mut |_, _, _, _| called = true));
    assert!(!called, "nothing struck, nothing rendered");
}

#[test]
fn the_same_bits_however_the_block_is_cut() {
    let base = play(&mut kit(true), &GROOVE, 60_000, 512);
    assert!(rms(&base) > 1e-4, "it sounds");
    for block in [1, 37, 64, 256, 333] {
        let x = play(&mut kit(true), &GROOVE, 60_000, block);
        assert!(x == base, "block {block} differs");
    }
}

#[test]
fn a_restrike_adds_to_the_ring_on_one_object() {
    let mut p = kit(true);
    let two = play(&mut p, &[(0, 49, 100), (24_000, 49, 100)], 48_000, 512);
    let m = p.measured.as_deref().unwrap();
    assert_eq!(
        m.objects.iter().filter(|o| o.active).count(),
        1,
        "one crash"
    );
    let one = play(&mut kit(true), &[(0, 49, 100)], 48_000, 512);
    // After the second hit, the crash rings louder than the first alone at that time.
    assert!(rms(&two[30_000..48_000]) > 1.3 * rms(&one[30_000..48_000]));
}

#[test]
fn a_crash_struck_while_it_rings_blooms_brighter() {
    // A 80 a second after a 127 adds more than a 80 on its own would (in power, and in its
    // highs): the ring's energy blooms the new hit.
    let first = play(&mut kit(true), &[(0, 49, 127)], 72_000, 512);
    let both = play(
        &mut kit(true),
        &[(0, 49, 127), (48_000, 49, 80)],
        72_000,
        512,
    );
    let alone = play(&mut kit(true), &[(48_000, 49, 80)], 72_000, 512);
    let w = 48_000..52_800;
    let p = |x: &[f32]| rms(x) * rms(x);
    let hi = |x: &[f32]| {
        let d: Vec<f32> = x.windows(2).map(|w| w[1] - w[0]).collect();
        p(&d)
    };
    assert!(p(&both[w.clone()]) > 1.1 * (p(&first[w.clone()]) + p(&alone[w.clone()])));
    assert!(hi(&both[w.clone()]) > 1.1 * (hi(&first[w.clone()]) + hi(&alone[w])));
}

#[test]
fn a_closed_stroke_and_the_pedal_choke_the_open_hat() {
    let open = play(&mut kit(true), &[(0, 46, 110)], 40_000, 256);
    let closed = play(
        &mut kit(true),
        &[(0, 46, 110), (9_600, 42, 40)],
        40_000,
        256,
    );
    let pedal = play(
        &mut kit(true),
        &[(0, 0, 127), (1, 46, 110), (9_600, 0, 0)],
        40_000,
        256,
    );
    let late = 9_600 + 9_600..40_000;
    assert!(rms(&open[late.clone()]) > 1e-4);
    assert!(
        rms(&closed[late.clone()]) < 0.05 * rms(&open[late.clone()]),
        "closed stroke chokes"
    );
    assert!(
        rms(&pedal[late.clone()]) < 0.05 * rms(&open[late]),
        "the pedal chokes"
    );
    // The foot claps as it closes on the ringing hat.
    assert!(rms(&pedal[9_600..10_600]) > rms(&pedal[late_frames()]));
}

fn late_frames() -> std::ops::Range<usize> {
    30_000..40_000
}

#[test]
fn harder_is_louder_and_brighter() {
    for key in [49u8, 42, 46] {
        let soft = play(&mut kit(true), &[(0, key, 40)], 24_000, 512);
        let hard = play(&mut kit(true), &[(0, key, 127)], 24_000, 512);
        let w = 0..4_800;
        assert!(
            rms(&hard[w.clone()]) > 3.0 * rms(&soft[w.clone()]),
            "key {key} louder"
        );
        // The crash's colour climbs with the force (DRSKit's 2.2 → 5 kHz); the recorded
        // hats' does not, so neither do the tables'.
        if key == 49 {
            assert!(
                brightness(&hard[w.clone()]) > brightness(&soft[w]),
                "brighter"
            );
        }
    }
}

#[test]
fn every_piece_comes_to_rest() {
    let mut p = kit(true);
    play(&mut p, &GROOVE, 60_000, 512);
    // The crash's longest mode rings some seconds: a minute is far past it.
    for _ in 0..(60.0 * SR) as usize / 4_096 {
        p.process_buses(4_096, &mut |_, _, _, _| {});
    }
    assert!(!p.measured.as_deref().unwrap().any(), "all at rest");
    assert!(!p.process_buses(512, &mut |_, _, _, _| {}));
}

/// The cost of a busy groove's cymbals (16th-note hats opening every bar, a crash every
/// bar, 30 s): run with `cargo test --release -p cantus-dsp --lib busy_groove -- --ignored
/// --nocapture`. Prints the share of one core.
#[test]
#[ignore]
fn busy_groove_cost() {
    let mut ev = Vec::new();
    let sixteenth = 6_000; // 120 bpm
    for step in 0..240usize {
        let at = step * sixteenth;
        let key = if step % 16 == 14 { 46 } else { 42 };
        ev.push((at, key, 70 + (step % 4) as u8 * 15));
        if step % 16 == 0 {
            ev.push((at, 49, 110));
        }
    }
    let n = 240 * sixteenth;
    let t = std::time::Instant::now();
    let x = play(&mut kit(true), &ev, n, 256);
    let s = t.elapsed().as_secs_f64();
    eprintln!(
        "busy groove: {:.2} s of sound in {:.3} s: {:.1} % of a core (rms {:.4})",
        n as f64 / SR as f64,
        s,
        100.0 * s / (n as f64 / SR as f64),
        rms(&x)
    );
}

/// The drums: the kick (36), the snare's head, rim and stick (38, 40, 37), a tom (45).
fn drums() -> Pulsus {
    let mut p = Pulsus::new(SR);
    p.enable_measured();
    p.set_kit(3, 9, 4, false);
    p.set_pad_count(5);
    p.set_pad(0, 36, &pad(4, 1, 0));
    for (i, (key, zone)) in [(38u8, 0.0f32), (40, 0.5), (37, 1.0)].iter().enumerate() {
        let mut s = pad(5, 2 + i as u16, 1);
        s.openness = *zone;
        p.set_pad(1 + i, *key, &s);
    }
    p.set_pad(4, 45, &pad(6, 5, 2));
    p
}

/// Zero crossings per second of `x` (its pitch, for a drum's low ring).
fn crossings(x: &[f32]) -> f32 {
    let n = x
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    n as f32 * SR / x.len() as f32 / 2.0
}

#[test]
fn the_drums_render_the_same_bits_however_cut_and_come_to_rest() {
    let ev = [
        (0, 36, 110),
        (6_000, 38, 100),
        (12_000, 40, 120),
        (18_000, 37, 90),
        (24_000, 45, 127),
        (30_000, 36, 60),
    ];
    let base = play(&mut drums(), &ev, 72_000, 512);
    assert!(rms(&base) > 1e-4);
    for block in [1, 37, 333] {
        assert!(
            play(&mut drums(), &ev, 72_000, block) == base,
            "block {block}"
        );
    }
    let mut p = drums();
    play(&mut p, &ev, 72_000, 512);
    for _ in 0..(40.0 * SR) as usize / 4_096 {
        p.process_buses(4_096, &mut |_, _, _, _| {});
    }
    assert!(!p.measured.as_deref().unwrap().any(), "at rest");
}

#[test]
fn a_hard_tom_starts_sharp_and_falls() {
    // The low ring, a one-pole low-pass at 250 Hz twice: its pitch early above its pitch late.
    let x = play(&mut drums(), &[(0, 45, 127)], 48_000, 512);
    let a = 1.0 - crate::math::exp(-std::f32::consts::TAU * 250.0 / SR);
    let (mut y1, mut y2) = (0.0f32, 0.0f32);
    let lo: Vec<f32> = x
        .iter()
        .map(|v| {
            y1 += a * (v - y1);
            y2 += a * (y1 - y2);
            y2
        })
        .collect();
    let early = crossings(&lo[480..4_800]);
    let late = crossings(&lo[14_400..28_800]);
    assert!(early > late * 1.02, "early {early} Hz, late {late} Hz");
}

/// A drum's onset is its head's push, the same hit after hit (round 40, drums-gen: each
/// mode's phase and early part measured, blob version 2): two hard kicks' first 6 ms below
/// ~300 Hz nearly the same shape, though every hit draws its own variation — where random
/// phases (version 1) made each a different smear.
#[test]
fn a_drum_starts_with_the_same_push_each_hit() {
    let k = super::table(4).expect("the kick");
    assert!(
        k.states[0]
            .layers
            .iter()
            .all(|l| l.ets > 0.0 && l.ph.len() == k.hz.len()),
        "the studio kick's layers carry their onset"
    );
    let low = |x: &[f32]| {
        let a = 1.0 - crate::math::exp(-std::f32::consts::TAU * 300.0 / SR);
        let (mut y1, mut y2) = (0.0f32, 0.0f32);
        x.iter()
            .map(|v| {
                y1 += a * (v - y1);
                y2 += a * (y1 - y2);
                y2
            })
            .collect::<Vec<f32>>()
    };
    // Two kicks a second apart, each a hit of its own (another note, another draw).
    let x = play(
        &mut drums(),
        &[(0, 36, 120), (48_000, 36, 120)],
        96_000,
        512,
    );
    let (a, b) = (low(&x[..288]), low(&x[48_000..48_288]));
    let dot: f32 = a.iter().zip(&b).map(|(p, q)| p * q).sum();
    let corr = dot / (rms(&a) * rms(&b) * a.len() as f32).max(1e-30);
    assert!(corr > 0.8, "the two pushes' correlation {corr}");
}

/// The cost of a busy full kit (kick, snare, toms, hats, crash, ride; 16ths, 30 s), and of
/// each piece alone: `cargo test --release -p cantus-dsp --lib full_kit_cost -- --ignored
/// --nocapture`. Prints the share of one core.
#[test]
#[ignore]
fn full_kit_cost() {
    let pieces: [(u8, u8, u8, f32); 9] = [
        (36, 4, 0, 0.0),
        (38, 5, 1, 0.0),
        (45, 6, 2, 0.0),
        (43, 7, 3, 0.0),
        (42, 2, 4, 0.0),
        (46, 2, 4, 0.95),
        (49, 1, 5, 0.0),
        (51, 3, 6, 0.0),
        (53, 3, 6, 1.0),
    ];
    let kit = || {
        let mut p = Pulsus::new(SR);
        p.enable_measured();
        p.set_kit(3, 5, 4, false);
        p.set_pad_count(pieces.len());
        for (i, (key, table, bus, open)) in pieces.iter().enumerate() {
            let mut s = pad(*table, i as u16 + 1, *bus);
            s.openness = *open;
            s.pedal = false;
            p.set_pad(i, *key, &s);
        }
        p
    };
    let sixteenth = 6_000;
    let mut ev = Vec::new();
    for step in 0..240usize {
        let at = step * sixteenth;
        let v = 70 + (step % 4) as u8 * 15;
        ev.push((at, if step % 16 == 14 { 46 } else { 42 }, v));
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
        if step % 32 == 30 {
            ev.push((at, 43, 110));
        }
        if step % 64 == 60 {
            ev.push((at, 53, 100));
        }
    }
    let n = 240 * sixteenth;
    let secs = n as f64 / SR as f64;
    // The best of five (a busy machine only ever adds).
    let best = (0..5)
        .map(|_| {
            let t = std::time::Instant::now();
            play(&mut kit(), &ev, n, 256);
            t.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min);
    eprintln!("full kit: {:.1} % of a core", 100.0 * best / secs);
    for key in [36u8, 38, 45, 42, 49, 51] {
        let only: Vec<_> = ev
            .iter()
            .copied()
            .filter(|e| e.1 == key || (key == 42 && e.1 == 46) || (key == 51 && e.1 == 53))
            .collect();
        let t = std::time::Instant::now();
        play(&mut kit(), &only, n, 256);
        eprintln!(
            "  key {key}: {:.2} %",
            100.0 * t.elapsed().as_secs_f64() / secs
        );
    }
}

/// drums-gen 3 — where a cymbal's cost goes: its modes, its bands' filters (main and side),
/// the whole render; ns per sample, one piece struck every half second.
#[test]
#[ignore]
fn cost_breakdown() {
    use super::{Bank, Strike};
    for (name, table) in [
        ("ride", 3u8),
        ("crash", 1),
        ("hat", 2),
        ("kick", 4),
        ("snare", 5),
    ] {
        let mut b = Bank::new(SR);
        let strike = |b: &mut Bank, seed: u64| {
            b.strike(&Strike {
                table,
                bus: 0,
                vel: 100.0,
                openness: 0.0,
                foot: false,
                seed,
                ratio: 1.0,
                decay: 1.0,
                gain: 1.0,
                click: 1.0,
                tilt: 0.0,
            })
        };
        let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        let n = 48_000 * 4;
        let mut whole = 0.0;
        let mut at = 0;
        let t = std::time::Instant::now();
        while at < n {
            if at % 24_064 == 0 {
                strike(&mut b, at as u64 + 1);
            }
            b.render(&mut l, &mut r, &mut |_, _, _| {}, 1.0);
            at += 256;
            whole += l[0] as f64;
        }
        let total = t.elapsed().as_secs_f64() * 1e9 / n as f64;
        let coefs = super::tick::BankCoefs {
            sr: b.sr,
            nbands: b.nbands,
            side_from: 20,
            low_to: 15,
            lp: &b.drum_lp,
            b0: &b.b0,
            a1: &b.a1,
            a2: &b.a2,
            norm: &b.norm,
        };
        let o = &mut b.objects[0];
        let live: usize = o
            .banks
            .iter()
            .map(|k| k.live.iter().filter(|x| **x).count())
            .sum();
        let flive: usize = o
            .banks
            .iter()
            .map(|k| k.flive.iter().filter(|x| **x).count())
            .sum();
        let m = 4_800;
        let mut s = 0.0;
        let (mut modes, mut bands) = (f64::MAX, f64::MAX);
        let (lo, hi) = (o.blo, o.bhi);
        for _ in 0..9 {
            let t = std::time::Instant::now();
            for _ in 0..m {
                s += o.modes().0;
            }
            modes = modes.min(t.elapsed().as_secs_f64() * 1e9 / m as f64);
            let t = std::time::Instant::now();
            for i in 0..m {
                super::tick::pair(&mut o.z, &mut o.vz, &coefs, i as f32 * 1e-6, lo, hi);
                super::tick::pair(&mut o.y, &mut o.vy, &coefs, i as f32 * 1e-6, lo, hi);
                s += o.vz[lo] + o.vy[lo];
            }
            bands = bands.min(t.elapsed().as_secs_f64() * 1e9 / m as f64);
        }
        eprintln!(
            "{name:6} whole {total:6.1} ns/sample · modes {modes:6.1} ({live} + {flive} blocks of {}) · bands {bands:6.1} ({lo}..{hi}) [{s:.1e} {whole:.1e}]",
            o.n
        );
    }
}

/// drums-gen 3 — one measured piece's dry source (main, side), for the measures:
/// `CANTUS_SOURCE_DUMP=<dir>`; each table 1–8 and 17–24 at velocities 40/80/110/127, one
/// second each, interleaved f32 main/side, `<dir>/t<id>.f32`.
#[test]
#[ignore]
fn dump_source() {
    let Some(dir) = std::env::var_os("CANTUS_SOURCE_DUMP") else {
        return;
    };
    use super::{Bank, Strike};
    for table in (1u8..=8).chain(17..=24) {
        let mut bytes = Vec::new();
        for vel in [40.0f32, 80.0, 110.0, 127.0] {
            let mut b = Bank::new(SR);
            b.strike(&Strike {
                table,
                bus: 0,
                vel,
                openness: 0.0,
                foot: false,
                seed: 7,
                ratio: 1.0,
                decay: 1.0,
                gain: 1.0,
                click: 1.0,
                tilt: 0.0,
            });
            let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
            for _ in 0..(48_000 / 256) {
                let mut lane = (vec![0.0f32; 256], vec![0.0f32; 256]);
                b.render(
                    &mut l,
                    &mut r,
                    &mut |_, a, c| {
                        lane.0.copy_from_slice(a);
                        lane.1.copy_from_slice(c);
                    },
                    1.0,
                );
                for (a, c) in lane.0.iter().zip(&lane.1) {
                    bytes.extend_from_slice(&(0.5 * (a + c)).to_le_bytes());
                    bytes.extend_from_slice(&(0.5 * (a - c)).to_le_bytes());
                }
            }
        }
        std::fs::write(
            std::path::Path::new(&dir).join(format!("t{table}.f32")),
            bytes,
        )
        .unwrap();
    }
}

/// A sine of `hz` through `f` (one sample at a time), its steady RMS over its input's, dB.
fn gain_db(hz: f32, mut f: impl FnMut(f32) -> f32) -> f32 {
    let (mut ei, mut eo) = (0.0f64, 0.0f64);
    for i in 0..96_000 {
        let x = crate::math::sin(std::f32::consts::TAU * hz * i as f32 / SR);
        let y = f(x);
        if i >= 48_000 {
            ei += f64::from(x * x);
            eo += f64::from(y * y);
        }
    }
    (10.0 * crate::math::log10(eo / ei)) as f32
}

/// drums-gen 3 (cues 4 and 5): a tom's noise bands are high-passed under its fundamental
/// (eighth order) and its low bands low-passed over a kilohertz; a cymbal's bands under
/// 90 Hz; a snare's, a kick's and the studio floor tom's left as they were.
#[test]
fn a_drums_noise_rests_under_its_fundamental_and_its_low_bands_under_its_highs() {
    use super::{Bank, Strike};
    let strike = |b: &mut Bank, table: u8| {
        b.strike(&Strike {
            table,
            bus: 0,
            vel: 100.0,
            openness: 0.0,
            foot: false,
            seed: 1,
            ratio: 1.0,
            decay: 1.0,
            gain: 1.0,
            click: 1.0,
            tilt: 0.0,
        })
    };
    let mut b = Bank::new(SR);
    strike(&mut b, 7);
    let o = b.objects[0].clone();
    let f0 = super::table(7).expect("a tom").low_hz;
    assert!(f0 > 50.0 && f0 < 200.0, "a tom's fundamental: {f0} Hz");
    assert!(o.split && (o.hp_hz - f0).abs() < 1e-3, "{} Hz", o.hp_hz);
    let hp = |hz: f32| {
        let mut z = [0.0f32; 8];
        gain_db(hz, |x| super::tick::high_pass(&o.hp, &mut z, x))
    };
    assert!(hp(0.4 * f0) < -60.0, "{} dB", hp(0.4 * f0));
    assert!(hp(2.0 * f0).abs() < 0.5, "{} dB", hp(2.0 * f0));
    let lp = |hz: f32| {
        let mut z = [0.0f32; 4];
        gain_db(hz, |x| super::tick::low_pass(&b.drum_lp, &mut z, x))
    };
    assert!(lp(4_000.0) < -40.0, "{} dB", lp(4_000.0));
    assert!(lp(400.0).abs() < 0.5, "{} dB", lp(400.0));
    for (table, split, hp) in [
        (1u8, false, true),
        (3, false, true),
        (4, false, false),
        (5, false, false),
        (8, false, false),
        (24, true, true),
    ] {
        let mut b = Bank::new(SR);
        strike(&mut b, table);
        let o = &b.objects[0];
        assert_eq!(o.split, split, "table {table}");
        assert_eq!(o.hp_hz > 0.0, hp, "table {table}: {} Hz", o.hp_hz);
    }
}

/// Every table's level against velocity, through Pulsus (round 42, tympanum-real): the first
/// 80 ms's RMS of three hits (three seeds) at each of the law's velocities — what
/// `tools/tympanum-real/law.py` reads (each table's law applied: run it with an empty `LAW` to
/// read the tables as they are). Run with `cargo test --release -p
/// cantus-dsp --lib level_law_dump -- --ignored --nocapture`.
#[test]
#[ignore]
fn level_law_dump() {
    for table in [1u8, 2, 3, 4, 5, 6, 7, 8, 17, 18, 19, 20, 21, 22, 23, 24] {
        if super::table(table).is_none() {
            continue;
        }
        let mut line = format!("table {table:2}:");
        for v in super::law::LAW_VELS {
            let vel = v as u8;
            let mut e = 0.0f32;
            for id in 0..3u64 {
                let mut p = Pulsus::new(SR);
                p.enable_measured();
                p.set_kit(3, 9, 4, false);
                p.set_pad_count(1);
                p.set_pad(0, 60, &pad(table, 1, 0));
                let mut out = vec![0.0f32; 3_840];
                let mut at = 0;
                p.note_on(60, vel, id + 1);
                while at < out.len() {
                    let c = (out.len() - at).min(512);
                    p.process_buses(c, &mut |_, off, l, r| {
                        for (i, (a, b)) in l.iter().zip(r).enumerate() {
                            out[at + off + i] += 0.5 * (a + b);
                        }
                    });
                    at += c;
                }
                e += rms(&out).powi(2);
            }
            line += &format!(" {vel}:{:.1}", 10.0 * (e / 3.0).max(1e-20).log10());
        }
        eprintln!("{line}");
    }
}

/// One hit of table `table` at `vel` (seed `id`): its first 80 ms's RMS, dB.
fn first_80ms_db(table: u8, vel: u8, id: u64) -> f32 {
    let mut p = Pulsus::new(SR);
    p.enable_measured();
    p.set_kit(3, 9, 4, false);
    p.set_pad_count(1);
    p.set_pad(0, 60, &pad(table, 1, 0));
    p.note_on(60, vel, id);
    let mut out = vec![0.0f32; 3_840];
    let mut at = 0;
    while at < out.len() {
        let c = (out.len() - at).min(512);
        p.process_buses(c, &mut |_, off, l, r| {
            for (i, (a, b)) in l.iter().zip(r).enumerate() {
                out[at + off + i] += 0.5 * (a + b);
            }
        });
        at += c;
    }
    20.0 * rms(&out).max(1e-10).log10()
}

/// The level law (r42:tympanum-real): every measured table grows with the velocity at every
/// step (none falls by more than a hit's own variation) — the studio kick at 48 was louder than at 64 and as loud as at 80 — and spans at least
/// 12 dB from 16 to 127.
#[test]
fn every_table_grows_with_velocity_at_every_step() {
    for (table, _) in super::law::LAW {
        let lv: Vec<f32> = [16u8, 32, 48, 64, 80, 96, 112, 127]
            .iter()
            .map(|&v| 0.5 * (first_80ms_db(*table, v, 1) + first_80ms_db(*table, v, 2)))
            .collect();
        for w in lv.windows(2) {
            // (a hit's own variation, ± ~1.5 dB a mode, leaves a flat step within 1 dB)
            assert!(w[1] > w[0] - 1.0, "table {table}: {lv:?}");
        }
        assert!(
            lv[7] - lv[0] >= 12.0,
            "table {table} spans {:.1} dB",
            lv[7] - lv[0]
        );
    }
}

/// The law (r42:tympanum-real) is linear between its velocities, exact at them, and a table
/// without one adds nothing — every such table's hit as before, to the bit.
#[test]
fn the_level_law_interpolates_and_a_table_without_one_adds_nothing() {
    use super::law::{law_db, LAW, LAW_VELS};
    assert_eq!(law_db(200, 64.0), 0.0);
    let (id, c) = LAW[0];
    for (i, v) in LAW_VELS.iter().enumerate() {
        assert!((law_db(id, *v) - 0.01 * c[i] as f32).abs() < 1e-5);
    }
    let mid = law_db(id, 0.5 * (LAW_VELS[3] + LAW_VELS[4]));
    assert!((mid - 0.005 * (c[3] + c[4]) as f32).abs() < 1e-4);
    // clamped outside the range
    assert_eq!(law_db(id, 0.0), law_db(id, 1.0));
    assert_eq!(law_db(id, 200.0), law_db(id, 127.0));
}

/// The jazz toms' burst (r42:tympanum-real): a table listed in `BURST` at 0 plays no burst,
/// every other keeps its own.
#[test]
fn a_table_without_its_burst_and_every_other_with_its_own() {
    use super::law::burst;
    assert_eq!(burst(22), 0.0);
    for id in [1u8, 4, 5, 6, 17, 20, 21, 23] {
        assert_eq!(burst(id), 1.0);
    }
    // and only the jazz hat's variation is scaled
    assert!(super::law::spread(18) < 1.0);
    for id in [1u8, 2, 4, 5, 17, 19, 22] {
        assert_eq!(super::law::spread(id), 1.0);
    }
}
