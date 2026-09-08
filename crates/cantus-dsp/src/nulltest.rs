//! Null-test harness (idea.md §6): compare two renders and report the first divergence.
//! Built before the second DSP module exists, as the spec demands.

#[derive(Clone, Debug, PartialEq)]
pub struct NullReport {
    pub samples: usize,
    pub max_abs_diff: f32,
    pub first_diff_index: Option<usize>,
    pub rms_a: f32,
    pub rms_b: f32,
    pub peak_a: f32,
    pub peak_b: f32,
}

impl NullReport {
    pub fn passes(&self, tolerance: f32) -> bool {
        self.max_abs_diff <= tolerance
    }
}

pub fn rms(buf: &[f32]) -> f32 {
    if buf.is_empty() {
        return 0.0;
    }
    (buf.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>() / buf.len() as f64).sqrt() as f32
}

pub fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0f32, |m, x| m.max(x.abs()))
}

/// Compare two buffers sample by sample. Length mismatch counts as a difference at the
/// shorter length.
pub fn compare(a: &[f32], b: &[f32]) -> NullReport {
    let n = a.len().min(b.len());
    let mut max = 0f32;
    let mut first = None;
    for i in 0..n {
        let d = (a[i] - b[i]).abs();
        if d > 0.0 && first.is_none() {
            first = Some(i);
        }
        if d > max {
            max = d;
        }
    }
    if a.len() != b.len() {
        first = first.or(Some(n));
        max = f32::INFINITY;
    }
    NullReport {
        samples: n,
        max_abs_diff: max,
        first_diff_index: first,
        rms_a: rms(a),
        rms_b: rms(b),
        peak_a: peak(a),
        peak_b: peak(b),
    }
}

/// Assert two renders are equal within `tolerance` (0.0 for bit-identical).
pub fn assert_null(a: &[f32], b: &[f32], tolerance: f32, what: &str) {
    let r = compare(a, b);
    assert!(
        r.passes(tolerance),
        "null test failed for {what}: max diff {} at sample {:?} (len {} vs {})",
        r.max_abs_diff,
        r.first_diff_index,
        a.len(),
        b.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_difference() {
        let a = vec![0.0, 0.5, 1.0];
        let mut b = a.clone();
        assert!(compare(&a, &b).passes(0.0));
        b[1] = 0.6;
        let r = compare(&a, &b);
        assert_eq!(r.first_diff_index, Some(1));
        assert!((r.max_abs_diff - 0.1).abs() < 1e-6);
        assert!(!compare(&a, &b[..2]).passes(1.0));
    }
}
