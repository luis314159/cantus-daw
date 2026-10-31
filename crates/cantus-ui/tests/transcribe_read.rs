//! A take read and converted a block at a time (round 34's review: long files) is the
//! whole file converted at once, to the bit — whatever the rates, the blocks' size and
//! the length.

use cantus_engine::samples::resample::resample;
use cantus_ui::transcribe_ui::read::Resampler;

fn signal(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.wrapping_mul(2_654_435_761).max(1);
    (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            0.3 * (s as f32 / u32::MAX as f32 - 0.5) + 0.2 * ((i as f32) * 0.013).sin()
        })
        .collect()
}

fn streamed(x: &[f32], from: u32, to: u32, block: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let mut r = Resampler::new(from, to, x.len() as u64);
    let mut put = |y: &[f32]| -> Result<(), String> {
        out.extend_from_slice(y);
        Ok(())
    };
    for b in x.chunks(block) {
        r.push(b, &mut put).unwrap();
    }
    let n = r.finish(&mut put).unwrap();
    assert_eq!(n as usize, out.len());
    out
}

#[test]
fn blocks_convert_to_the_bit_as_the_whole() {
    for (from, to) in [
        (48_000u32, 22_050u32),
        (44_100, 22_050),
        (48_000, 44_100),
        (96_000, 44_100),
        (22_050, 44_100),
        (32_000, 22_050),
        (44_100, 44_100),
    ] {
        for (len, block) in [
            (1_234_567usize, 65_536usize),
            (500_001, 480_000),
            (2_000, 300),
        ] {
            let x = signal(len, from ^ len as u32);
            let whole = resample(&x, 1, from, to);
            let parts = streamed(&x, from, to, block);
            assert_eq!(parts.len(), whole.len(), "{from} → {to}, {len} in {block}");
            let first = parts
                .iter()
                .zip(&whole)
                .position(|(a, b)| a.to_bits() != b.to_bits());
            assert_eq!(first, None, "{from} → {to}, {len} in blocks of {block}");
        }
    }
}
