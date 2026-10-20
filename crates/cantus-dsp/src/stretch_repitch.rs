//! Re-pitch: the source read at the map's speed, pitch and speed together (tape), through
//! Exemplar's windowed-sinc kernel on its octave mip levels — a level chosen so the local
//! speed read there is at most 1, so nothing above the output's band folds back.

use super::Map;
use crate::exemplar::resample::{mip_chain, Kernel, MAX_TAPS};
use crate::math;

const TAPS: usize = 32;
const BETA: f64 = 9.0;

pub(super) fn render(channels: &[&[f32]], map: &Map, len: usize) -> Vec<Vec<f32>> {
    let kernel = Kernel::new(TAPS, BETA);
    let mut coefs = [0.0f32; MAX_TAPS];
    let top = (0..len).fold(1.0f64, |m, t| m.max(map.slope(t as f64)));
    let levels = (math::log2(top).ceil().max(0.0) as usize).min(7);
    channels
        .iter()
        .map(|x| {
            let mips = if levels > 0 {
                mip_chain(x, 1, levels, false)
            } else {
                Vec::new()
            };
            (0..len)
                .map(|t| {
                    let s = map.source(t as f64);
                    let k = map.slope(t as f64);
                    let want = if k > 1.0 {
                        math::log2(k).ceil() as usize
                    } else {
                        0
                    };
                    let level = want.min(mips.len());
                    let data: &[f32] = if level == 0 { x } else { &mips[level - 1] };
                    let pos = s / (1u64 << level) as f64;
                    let i = pos.floor();
                    let frac = pos - i;
                    let i = i as i64;
                    let get = |j: i64| -> f32 {
                        if j >= 0 && (j as usize) < data.len() {
                            data[j as usize]
                        } else {
                            0.0
                        }
                    };
                    if frac == 0.0 {
                        return get(i);
                    }
                    kernel.coefs(frac, &mut coefs);
                    let first = i - (TAPS as i64 / 2 - 1);
                    coefs[..TAPS]
                        .iter()
                        .enumerate()
                        .map(|(j, c)| c * get(first + j as i64))
                        .sum()
                })
                .collect()
        })
        .collect()
}
