//! The hybrid voice through Tympanum's unit (round 40, `drums-hyb`): with no residual the
//! unit renders the generated kit to the bit; with one, the residual sounds at the pieces'
//! sources and any chunking renders the same bits; its cost (ignored, release).

use super::*;
use crate::inst::tympanum::{cache, Unit};
use crate::inst::{InstrumentUnit, NoteOn};
use cantus_core::inst::tympanum::voice::HYBRID;
use cantus_core::inst::tympanum::{KIT_KEY, VOICE_KEY};
use cantus_core::InstrumentKind;
use cantus_dsp::tympanum_residual::{Art, Layer, Take};

/// A residual the size of a real pack (15 articulations, five layers, four takes, up to
/// 200 ms each): decaying seeded noise. The same every time, whichever test loads it.
fn synthetic() -> Residual {
    let keys: [(u8, u8); 15] = [
        (36, 255),
        (38, 255),
        (40, 255),
        (37, 255),
        (45, 255),
        (43, 255),
        (41, 255),
        (42, 0),
        (46, 10),
        (46, 50),
        (46, 95),
        (44, 0),
        (49, 255),
        (51, 255),
        (53, 255),
    ];
    let mut data = Vec::new();
    let mut x: u32 = 0x1234_5678;
    let mut arts = Vec::new();
    for (k, &(key, open)) in keys.iter().enumerate() {
        let len = 1_440 + 600 * k as u32; // 30 … 205 ms
        let mut layers = Vec::new();
        for (l, vel) in [30u8, 55, 80, 105, 127].iter().enumerate() {
            let mut takes = Vec::new();
            for _ in 0..4 {
                let start = data.len() as u32;
                for i in 0..len {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    let fade = 1.0 - i as f32 / len as f32;
                    data.push((((x >> 16) as i32 - 32_768) as f32 * fade * fade) as i16);
                }
                takes.push(Take {
                    start,
                    len,
                    scale: 0.02 * (l + 1) as f32 / 32_768.0,
                });
            }
            layers.push(Layer {
                vel: *vel,
                rho: 0.2,
                takes,
            });
        }
        arts.push(Art { key, open, layers });
    }
    let mut r = Residual {
        rate: 48_000,
        model: model_hash(),
        arts,
        data,
        soft: Vec::new(),
    };
    r.finish();
    r
}

/// The test source: the jazz kit's pack holds the synthetic residual; nothing else exists.
fn install() {
    set_source(Arc::new(|pack: &str, file: &str| {
        if pack == "tympanum-hybrid-jazz" && file == RESIDUAL_FILE {
            Ok(synthetic().encode())
        } else {
            Err("not installed".into())
        }
    }));
    warm("jazz");
    cache::warm_modelled("jazz");
    cache::warm_modelled("studio");
}

fn spec(kit: &str, hybrid: bool) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
    s.set_text(KIT_KEY, Some(format!("model:{kit}"))).unwrap();
    if hybrid {
        s.set_text(VOICE_KEY, Some(HYBRID.into())).unwrap();
    }
    s
}

/// A busy groove: (frame, key, velocity).
fn groove(steps: usize) -> Vec<(usize, u8, u8)> {
    let sixteenth = 6_000usize;
    let mut ev = Vec::new();
    for step in 0..steps {
        let at = step * sixteenth;
        ev.push((
            at,
            if step % 16 == 14 { 46 } else { 42 },
            70 + (step % 4) as u8 * 15,
        ));
        if step.is_multiple_of(8) {
            ev.push((at, 36, 110));
        }
        if step % 8 == 4 {
            ev.push((at, 38, 115));
        }
        if step % 4 == 2 {
            ev.push((at, 51, 90));
        }
        if step.is_multiple_of(32) {
            ev.push((at, 49, 120));
        }
        if step % 32 == 28 {
            ev.push((at, 45, 110));
        }
    }
    ev
}

/// The groove through a fresh unit in blocks of `block` (cut at every hit).
fn render(s: &NativeSpec, block: usize, frames: usize) -> Vec<f32> {
    let mut u = Unit::new(48_000.0);
    u.configure(s, &[]);
    let ev = groove(frames / 6_000 + 1);
    let mut out = Vec::with_capacity(frames * 2);
    let mut at = 0;
    let mut e = 0;
    while at < frames {
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
        let next = ev.get(e).map_or(frames, |x| x.0).min(frames);
        let n = block.min(next - at).max(1).min(frames - at);
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        u.process(&mut l, &mut r);
        for i in 0..n {
            out.push(l[i]);
            out.push(r[i]);
        }
        at += n;
    }
    out
}

fn same(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len());
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: differs at {i}: {} vs {}", a[i], b[i]);
    }
}

#[test]
fn the_hybrid_without_its_residual_is_the_generated_kit_to_the_bit_and_with_it_adds_the_attacks() {
    install();
    let frames = 48_000;
    // The studio kit has no residual here: hybrid plays generated, to the bit (never silence).
    let gen = render(&spec("studio", false), 256, frames);
    assert!(gen.iter().any(|x| *x != 0.0));
    same(
        &render(&spec("studio", true), 256, frames),
        &gen,
        "studio hybrid, no pack",
    );
    // The jazz kit: generated is the model alone; hybrid adds the residual.
    let gen = render(&spec("jazz", false), 256, frames);
    let hyb = render(&spec("jazz", true), 256, frames);
    let diff: f64 = gen
        .iter()
        .zip(&hyb)
        .map(|(a, b)| f64::from(a - b) * f64::from(a - b))
        .sum();
    assert!(diff > 1e-6, "the residual was not heard: {diff}");
    // Any chunking, the same bits.
    for block in [1usize, 64, 333, 512] {
        same(
            &render(&spec("jazz", true), block, frames),
            &hyb,
            &format!("block {block}"),
        );
    }
}

/// The full jazz kit's cost with every mic on, generated and hybrid, and the residual's
/// memory: `cargo test --release -p cantus-engine --lib hybrid_cost -- --ignored
/// --nocapture`.
#[test]
#[ignore]
fn hybrid_cost() {
    install();
    let frames = 240 * 6_000;
    for hybrid in [false, true] {
        let s = spec("jazz", hybrid);
        let mut best = f64::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            let _ = render(&s, 256, frames);
            best = best.min(t.elapsed().as_secs_f64());
        }
        eprintln!(
            "full jazz kit, every mic on, {}: {:.1} % of a core",
            if hybrid { "hybrid" } else { "generated" },
            100.0 * best / (frames as f64 / 48_000.0)
        );
    }
    let r = residual("jazz").unwrap();
    eprintln!(
        "a residual of {} articulations: {:.1} MB in memory",
        r.arts.len(),
        r.bytes() as f64 / 1e6
    );
}
