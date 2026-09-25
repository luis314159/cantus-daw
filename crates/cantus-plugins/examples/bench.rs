//! Microseconds per 512-sample stereo block, each plug-in three ways (ADR 0020):
//!
//! - **engine** — the engine's `EffectUnit::process_keyed`, as Cantus calls an insert;
//!   and **engine+FTZ**, the same with the plug-in's denormal mode, the baseline the
//!   wrapper's overhead is measured against;
//! - **clap** — the same plug-in through this binary's own `clap_entry`: the whole wrapper
//!   (events, the grid, the bypass line, FTZ, the host's buffers) around the same unit;
//! - **exported** — the bundled binary itself (`target/bundled/Cantus <Name>.clap`),
//!   loaded with `LoadLibrary` as a host loads it, when it has been built; and the
//!   x86-64-v3 flavour from `target/bundled-x86-64-v3/` when that exists.
//!
//! ```text
//! cargo run -p cantus-plugins --example bench --profile plugin --no-default-features --features all
//! ```
//!
//! The ways take turns block by block over 6000 blocks (64 s of audio each) and each
//! figure is the median, so a busy machine's slow stretches fall on all of them alike and
//! the blocks an interruption lands in do not count.

#[path = "../tests/common/mod.rs"]
mod common;

use cantus_core::EffectKind;
use clap_sys::entry::clap_plugin_entry;
use common::*;
use std::path::PathBuf;
use std::time::Instant;

const BLOCK: usize = 512;
const BLOCKS: usize = 6_000;

fn workspace_target() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// A built plug-in's `clap_entry`, loaded as a host would. The library is kept loaded.
fn load_entry(path: &std::path::Path) -> Option<&'static clap_plugin_entry> {
    if !path.exists() {
        return None;
    }
    unsafe {
        let lib = Box::leak(Box::new(libloading::Library::new(path).ok()?));
        let sym: libloading::Symbol<*const clap_plugin_entry> = lib.get(b"clap_entry\0").ok()?;
        Some(&**sym)
    }
}

/// One way of running a plug-in: something that processes a block in place.
type Way<'a> = Box<dyn FnMut(&mut [f32], &mut [f32], &[f32], &[f32]) + 'a>;

fn clap_way(entry: &'static clap_plugin_entry, kind: EffectKind) -> Way<'static> {
    let plug = Plug::new_from(entry, kind);
    let events: Vec<Ev> = settings(kind)
        .iter()
        .map(|(k, v)| {
            let (id, value) = clap_value(kind, k, *v);
            (0, id, value)
        })
        .collect();
    plug.flush(&events);
    plug.activate(BLOCK as u32);
    let none = EventList::new(&[]);
    Box::new(move |l, r, kl, kr| plug.process_list(l, r, Some((kl, kr)), &none))
}

/// The median time of each way, µs per block, and whether each way's output was the
/// first way's to the bit, every block. The ways take turns block by block (a different
/// one first each block), so a busy machine's slow stretches fall on all of them alike,
/// and the median ignores the blocks an interruption lands in.
fn race(ways: &mut [Way<'_>]) -> (Vec<f64>, Vec<bool>) {
    let (src_l, src_r) = signal(BLOCK * 64, 7);
    let (key_l, key_r) = signal(BLOCK, 9);
    let n = ways.len();
    let mut out = vec![(vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]); n];
    let mut times: Vec<Vec<f64>> = vec![Vec::with_capacity(BLOCKS); n];
    let mut same = vec![true; n];
    for b in 0..BLOCKS + 200 {
        let at = (b % 64) * BLOCK;
        for k in 0..n {
            let w = (b + k) % n;
            let (l, r) = &mut out[w];
            l.copy_from_slice(&src_l[at..at + BLOCK]);
            r.copy_from_slice(&src_r[at..at + BLOCK]);
            let t = Instant::now();
            (ways[w])(l, r, &key_l, &key_r);
            let dt = t.elapsed().as_secs_f64() * 1e6;
            // The first 200 blocks warm caches and let the DSP settle.
            if b >= 200 {
                times[w].push(dt);
            }
        }
        for w in 1..n {
            let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
            if bits(&out[w].0) != bits(&out[0].0) || bits(&out[w].1) != bits(&out[0].1) {
                same[w] = false;
            }
        }
    }
    let medians = times
        .into_iter()
        .map(|mut v| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        })
        .collect();
    (medians, same)
}

fn main() {
    let target = workspace_target();
    println!(
        "median µs per {BLOCK}-sample stereo block at 48 kHz over {BLOCKS} blocks, the ways \
         taking turns block by block; = the engine's output to the bit in every block"
    );
    println!(
        "{:<10} {:>9} {:>11} {:>9} {:>10} {:>9} {:>9}",
        "plug-in", "engine", "engine+FTZ", "clap", "overhead", "exported", "x86-64-v3"
    );
    for d in &cantus_plugins::desc::ALL {
        let kind = d.kind;
        let exported = load_entry(&target.join("bundled").join(format!("{}.clap", d.full_name)));
        let v3 = load_entry(
            &target
                .join("bundled-x86-64-v3")
                .join(format!("{}.clap", d.full_name)),
        );
        for entry in [exported, v3].into_iter().flatten() {
            let f = factory_of(entry);
            // SAFETY: a factory from a CLAP entry.
            let count = unsafe { f.get_plugin_count.unwrap()(f) };
            assert_eq!(
                count, 1,
                "{}: an exported binary carries one plug-in",
                d.full_name
            );
        }
        let spec = spec_with(kind, &settings(kind));
        let mut unit = engine_unit(&spec);
        let mut flushed = engine_unit(&spec);
        let mut ways: Vec<Way<'_>> = vec![
            Box::new(move |l: &mut [f32], r: &mut [f32], _: &[f32], _: &[f32]| {
                unit.process_keyed(l, r, None)
            }),
            // The engine's unit with the plug-in's denormal mode: the fair baseline for
            // the wrapper's own cost.
            Box::new(move |l: &mut [f32], r: &mut [f32], _: &[f32], _: &[f32]| {
                let _ftz = cantus_plugins::ftz::DenormalGuard::new();
                flushed.process_keyed(l, r, None)
            }),
            clap_way(entry(), kind),
        ];
        for e in [exported, v3].into_iter().flatten() {
            ways.push(clap_way(e, kind));
        }
        let (t, same) = race(&mut ways);
        // `=` marks a way whose output was the engine's to the bit in every block.
        let mark = |i: usize| if same[i] { "=" } else { "≠" };
        let show = |i: usize, present: bool| {
            if present {
                format!("{:.2}{}", t[i], mark(i))
            } else {
                "—".to_string()
            }
        };
        let x = 3;
        let y = 3 + exported.is_some() as usize;
        println!(
            "{:<10} {:>9.2} {:>11} {:>9} {:>+9.2}% {:>9} {:>9}",
            d.name,
            t[0],
            show(1, true),
            show(2, true),
            (t[2] - t[1]) / t[1] * 100.0,
            show(x, exported.is_some()),
            show(y, v3.is_some())
        );
    }
}
