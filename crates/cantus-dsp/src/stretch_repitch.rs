//! Repitch (stub).
use super::Map;
pub(super) fn render(channels: &[&[f32]], _map: &Map, len: usize) -> Vec<Vec<f32>> {
    channels.iter().map(|_| vec![0.0; len]).collect()
}
