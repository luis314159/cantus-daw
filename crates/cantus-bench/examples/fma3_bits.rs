//! Does the CRT's FMA3 switch change what the engine renders on this machine? (ADR 0031 R9.)
//!
//! Renders the bench's scaled session (every demo's instruments and effects, the mastering
//! chain) block by block with the universal CRT's FMA3 path on, then off, each in a fresh
//! processor, and compares the output bit for bit. Windows x64 only; elsewhere it says so.
//!
//! ```text
//! cargo run -p cantus-bench --example fma3_bits --release -- [--tracks 10] [--seconds 20]
//! ```

use cantus_bench::rig::{Rig, SAMPLE_RATE};
use cantus_bench::sessions;
use cantus_core::Ticks;
use cantus_rt::fpenv::{crt_fma3_enabled, disable_crt_fma3, enable_crt_fma3_for_tests};

fn render(tracks: usize, seconds: f64) -> (Vec<u32>, usize) {
    let p = sessions::scaled(tracks);
    let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
    let mut rig = Rig::new(&p, start, 128);
    let blocks = (seconds * SAMPLE_RATE / 128.0) as usize;
    let mut out = Vec::with_capacity(blocks * 256);
    for _ in 0..blocks {
        rig.block();
        out.extend(rig.output().iter().map(|x| x.to_bits()));
    }
    (out, blocks)
}

fn main() {
    let mut tracks = 10;
    let mut seconds = 20.0;
    let a: Vec<String> = std::env::args().skip(1).collect();
    for w in a.windows(2) {
        match w[0].as_str() {
            "--tracks" => tracks = w[1].parse().unwrap_or(tracks),
            "--seconds" => seconds = w[1].parse().unwrap_or(seconds),
            _ => {}
        }
    }
    let Some(initial) = crt_fma3_enabled() else {
        println!("no CRT FMA3 switch on this platform");
        return;
    };
    println!("CRT FMA3 path at start: {initial}");
    let on_ok = enable_crt_fma3_for_tests();
    let (on, blocks) = render(tracks, seconds);
    disable_crt_fma3();
    let (off, _) = render(tracks, seconds);
    let differ = on.iter().zip(&off).filter(|(a, b)| a != b).count();
    let first = on.iter().zip(&off).position(|(a, b)| a != b);
    let peak = on
        .iter()
        .zip(&off)
        .map(|(a, b)| (f32::from_bits(*a) - f32::from_bits(*b)).abs())
        .fold(0.0f32, f32::max);
    println!(
        "scaled({tracks}), {blocks} blocks of 128: FMA3 path on (enabled: {on_ok:?}) vs off: {differ} of {} samples differ{}, peak difference {peak:e}",
        on.len(),
        first.map_or(String::new(), |i| format!(" (first at sample {})", i / 2)),
    );
}
