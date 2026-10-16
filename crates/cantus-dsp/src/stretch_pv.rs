//! Phase vocoder (stub).
use super::Map;
pub(crate) fn render(channels: &[&[f32]], _rate: f32, _map: &Map, len: usize, _ratio: f64, _formants: bool) -> Vec<Vec<f32>> {
    channels.iter().map(|_| vec![0.0; len]).collect()
}
