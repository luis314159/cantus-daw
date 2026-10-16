//! Rhythm (stub).
use super::Request;
pub(super) fn render(channels: &[&[f32]], _rate: f32, req: &Request, _ratio: f64) -> Vec<Vec<f32>> {
    channels.iter().map(|_| vec![0.0; req.len]).collect()
}
