//! Re-pitch: the source read at the map's speed, pitch and speed together (tape). At speed
//! 1 or below through Exemplar's windowed-sinc kernel on the source itself; faster through
//! [`super::read::Fast`] — an octave mip level and a cut-off scaled by the local speed, so
//! the band follows `fs / 2k` continuously and nothing above it folds back (a speed just
//! over 1 keeps its top octave; a ramp crossing a power of two does not switch band).

use super::read::Fast;
use super::Map;
use crate::exemplar::resample::{Kernel, MAX_TAPS};

const TAPS: usize = 32;
const BETA: f64 = 9.0;

pub(super) fn render(channels: &[&[f32]], map: &Map, len: usize) -> Vec<Vec<f32>> {
    let kernel = Kernel::new(TAPS, BETA);
    let mut coefs = [0.0f32; MAX_TAPS];
    let top = (0..len).fold(1.0f64, |m, t| m.max(map.slope(t as f64)));
    let (a, b) = (map.source(0.0), map.source(len as f64));
    channels
        .iter()
        .map(|x| {
            let fast = (top > 1.0).then(|| Fast::new(x, a.min(b), a.max(b), top));
            (0..len)
                .map(|t| {
                    let s = map.source(t as f64);
                    let k = map.slope(t as f64);
                    if let Some(f) = fast.as_ref().filter(|_| k > 1.0) {
                        return f.read(s, k);
                    }
                    let i = s.floor();
                    let frac = s - i;
                    let i = i as i64;
                    let get = |j: i64| -> f32 {
                        if j >= 0 && (j as usize) < x.len() {
                            x[j as usize]
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
