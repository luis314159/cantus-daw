//! Harmonic/percussive split (stub).
use super::Map;
pub(super) fn split(channels: &[&[f32]], _rate: f32) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let z: Vec<Vec<f32>> = channels.iter().map(|c| vec![0.0; c.len()]).collect();
    (z.clone(), z)
}
pub(super) fn grains(channels: &[&[f32]], _rate: f32, _map: &Map, len: usize, _ratio: f64) -> Vec<Vec<f32>> {
    channels.iter().map(|_| vec![0.0; len]).collect()
}
