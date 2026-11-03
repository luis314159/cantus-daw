//! Renders made outside the engine that clips play in place of their sources (round 36,
//! ADR 0097): an ARA plug-in's render of a bound clip, made by `cantus-host` on its render
//! thread. A render covers only the clip's stretch of the source ([`ClipRender::start`] is
//! where in the source it begins) and arrives already at the engine's rate; the sample
//! cache attaches it to the snapshot after the pitch renders and before the warp ones,
//! moving the clip's fine offset by the render's start, so the clip reads it exactly where
//! it would read its source.
//!
//! A clip with no render here plays its source: never silence.

use cantus_core::{ClipId, SampleData, Snapshot};
use std::collections::HashMap;
use std::sync::Arc;

/// A render a clip plays in place of its source.
#[derive(Clone, Debug)]
pub struct ClipRender {
    /// The samples, at the engine's rate (converted on the render thread).
    pub data: Arc<SampleData>,
    /// Where in the source `data` begins, in seconds.
    pub start: f64,
    /// The source frames it covers, [first, end), at the source's rate.
    pub source_frames: (i64, i64),
    pub source_rate: u32,
    /// The same samples at the source's rate (what a part rendered again is laid over);
    /// the very samples of `data` when the rates agree.
    pub at_source_rate: Arc<[f32]>,
}

#[derive(Debug, Default)]
pub(crate) struct ClipRenders {
    renders: HashMap<ClipId, ClipRender>,
    /// A render the engine's rate changed under, converted here until the host renders it
    /// again: (the render's address, the rate) → the conversion.
    at_rate: HashMap<(usize, u32), Arc<SampleData>>,
}

impl ClipRenders {
    /// Replace the renders. Conversions of renders no longer held are let go.
    pub fn set(&mut self, renders: HashMap<ClipId, ClipRender>) {
        let keep: std::collections::HashSet<usize> = renders
            .values()
            .map(|r| Arc::as_ptr(&r.data) as usize)
            .collect();
        self.at_rate.retain(|(p, _), _| keep.contains(p));
        self.renders = renders;
    }

    fn convert(&mut self, r: &Arc<SampleData>, rate: u32) -> Arc<SampleData> {
        if r.rate == rate || rate == 0 {
            return r.clone();
        }
        self.at_rate
            .entry((Arc::as_ptr(r) as usize, rate))
            .or_insert_with(|| {
                let ch = r.channels.max(1) as usize;
                Arc::new(SampleData {
                    stream: None,
                    rate,
                    channels: r.channels,
                    frames: super::resample::resample(&r.frames, ch, r.rate, rate).into(),
                })
            })
            .clone()
    }

    /// Every clip with a render plays it (at `rate`) in place of its source.
    pub fn attach(&mut self, snapshot: &mut Snapshot, rate: u32) {
        if self.renders.is_empty() {
            return;
        }
        for track in &mut snapshot.tracks {
            for clip in &mut track.audio {
                if let Some(r) = self.renders.get(&clip.clip).cloned() {
                    clip.data = Some(self.convert(&r.data, rate));
                    // The render begins `start` seconds into the source: read it there.
                    clip.slip -= r.start;
                }
            }
        }
    }
}
