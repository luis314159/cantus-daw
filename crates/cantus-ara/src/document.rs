//! The host document: what a project looks like to an ARA plug-in, and the updates between
//! two states of it.
//!
//! ARA's model is a document of **audio sources** (a recording), **audio modifications**
//! (a plug-in's edits of a source, one per source here) and **playback regions** (a part
//! of a modification placed on the timeline, in seconds). A Cantus audio clip on the playing
//! lane is a region; its window and the tempo map decide the seconds.
//!
//! The plug-in owns its model, so the host never hands it a new document; it sends the
//! difference. [`diff`] produces those updates in the order ARA needs: regions removed
//! before the source they use, sources added before the regions that use them.

use std::collections::BTreeMap;

use cantus_core::audio_time::source_seconds;
use cantus_core::model::{AudioSourceId, Clip, ClipId, Project, TrackId};
use cantus_core::tempo::TempoMap;

/// A recording the plug-in can read.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub id: AudioSourceId,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub frames: u64,
    /// SHA-256 of the file when known: the plug-in's cache key for an analysis.
    pub sha256: Option<String>,
}

/// A part of a source on the timeline, in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub clip: ClipId,
    pub track: TrackId,
    pub source: AudioSourceId,
    /// Where it starts on the timeline.
    pub start: f64,
    /// How long it plays on the timeline.
    pub duration: f64,
    /// The source's position, in seconds, under the region's start (negative when the
    /// clip begins before the recording does).
    pub source_start: f64,
    pub muted: bool,
    /// The binding number (`AraClip::binding`): another one is a fresh modification.
    pub binding: u64,
}

/// The project as a plug-in sees it. Two documents are equal when their sources and regions
/// are (`tracks` names the regions' tracks, carried for the plug-in's display).
#[derive(Clone, Debug, Default)]
pub struct Document {
    pub sources: BTreeMap<AudioSourceId, Source>,
    pub regions: BTreeMap<ClipId, Region>,
    /// The tracks that hold a region (ARA's region sequences): name and order.
    pub tracks: BTreeMap<TrackId, (String, i32)>,
}

/// One step that takes a plug-in's copy of the document toward the next state.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::enum_variant_names)] // ARA's own verbs: add/update/remove an object
pub enum Update {
    AddSource(Source),
    RemoveSource(AudioSourceId),
    AddRegion(Region),
    /// A region whose geometry or mute changed; same clip and source, new values.
    UpdateRegion(Region),
    RemoveRegion(ClipId),
}

impl Document {
    /// The document of a project: every audio clip on the **playing lane** (take lanes are
    /// never heard, ADR 0025) that points at a source in the pool, and the sources those
    /// clips use. A bound clip carries no warp (`SetClipAra` refuses one), so its window
    /// alone maps it; note and automation clips are not audio and do not appear.
    pub fn of_project(project: &Project) -> Self {
        Self::of_project_where(project, |_| true)
    }

    /// The document one ARA plug-in sees: the clips bound to it (by its VST3 class id).
    pub fn of_plugin(project: &Project, plugin_id: &str) -> Self {
        Self::of_project_where(project, |c| {
            c.audio
                .as_ref()
                .and_then(|a| a.ara.as_ref())
                .is_some_and(|a| a.plugin.id.eq_ignore_ascii_case(plugin_id))
        })
    }

    /// The document of the playing-lane audio clips `keep` accepts.
    pub fn of_project_where(project: &Project, keep: impl Fn(&Clip) -> bool) -> Self {
        let map = project.tempo_map();
        let mut doc = Document::default();
        for (order, track) in project.tracks.iter().enumerate() {
            for clip in &track.clips {
                let Some(audio) = &clip.audio else { continue };
                if clip.lane != 0 || !keep(clip) {
                    continue;
                }
                doc.tracks
                    .entry(track.id)
                    .or_insert_with(|| (track.name.clone(), order as i32));
                let Some(src) = project.audio_pool.iter().find(|s| s.id == audio.source) else {
                    continue;
                };
                doc.sources.entry(src.id).or_insert_with(|| Source {
                    id: src.id,
                    name: src.name.clone(),
                    sample_rate: src.sample_rate,
                    channels: src.channels,
                    frames: src.frames,
                    sha256: src.sha256.clone(),
                });
                let slip = audio.slip_frames as f64 / f64::from(src.sample_rate.max(1));
                doc.regions
                    .insert(clip.id, region_of(&map, track.id, clip, audio.source, slip));
            }
        }
        doc
    }
}

fn region_of(
    map: &TempoMap,
    track: TrackId,
    clip: &Clip,
    source: AudioSourceId,
    slip: f64,
) -> Region {
    let start = map.seconds_at(clip.start.0 as f64);
    let end = map.seconds_at((clip.start + clip.length).0 as f64);
    Region {
        clip: clip.id,
        track,
        source,
        start,
        duration: end - start,
        // The engine's rule (ADR 0025 §7): the source frame under content tick `t` is
        // `seconds(t) × rate + slip_frames`.
        source_start: source_seconds(map, clip.start, clip.offset, clip.start.0 as f64) + slip,
        muted: clip.muted,
        binding: clip
            .audio
            .as_ref()
            .and_then(|a| a.ara.as_ref())
            .map_or(0, |a| a.binding),
    }
}

/// The musical context a plug-in reads: tempo entries (seconds, quarters) and bar
/// signatures. One tempo: two entries. A tempo map that moves: an entry on every quarter
/// note up to `until` seconds (ARA interpolates linearly between entries, so a ramp is
/// followed to a quarter note).
pub fn musical_of(project: &Project, until: f64) -> crate::callbacks::Musical {
    let map = project.tempo_map();
    let q = |tick: f64| tick / cantus_core::time::PPQ as f64;
    let tempo = if map.is_constant() {
        let one = cantus_core::time::PPQ as f64;
        vec![(0.0, 0.0), (map.seconds_at(one), 1.0)]
    } else {
        let last = map.tick_at_seconds(until.max(1.0)) / cantus_core::time::PPQ as f64;
        (0..=(last.ceil() as i64 + 1))
            .map(|n| {
                let tick = (n * cantus_core::time::PPQ) as f64;
                (map.seconds_at(tick), q(tick))
            })
            .collect()
    };
    let mut bars = vec![(
        i32::from(project.time_signature.numerator),
        i32::from(project.time_signature.denominator),
        0.0,
    )];
    for m in &project.metre_changes {
        let entry = (
            i32::from(m.signature.numerator),
            i32::from(m.signature.denominator),
            q(m.tick.0 as f64),
        );
        if m.tick.0 <= 0 {
            bars[0] = entry;
        } else {
            bars.push(entry);
        }
    }
    crate::callbacks::Musical { tempo, bars }
}

/// The updates that take `old` to `new`. A source whose file changed is removed and added
/// again (the plug-in's analysis of the old recording is void), and the regions on it are
/// removed first and added again after. Identical documents give none; [`apply`] of the
/// result to `old` gives `new`.
pub fn diff(old: &Document, new: &Document) -> Vec<Update> {
    let replaced = |id: &AudioSourceId| matches!((old.sources.get(id), new.sources.get(id)), (Some(a), Some(b)) if a != b);
    let mut out = Vec::new();
    for (id, r) in &old.regions {
        if !new.regions.contains_key(id) || replaced(&r.source) {
            out.push(Update::RemoveRegion(*id));
        }
    }
    for id in old.sources.keys() {
        if !new.sources.contains_key(id) || replaced(id) {
            out.push(Update::RemoveSource(*id));
        }
    }
    for (id, s) in &new.sources {
        if old.sources.get(id) != Some(s) {
            out.push(Update::AddSource(s.clone()));
        }
    }
    for (id, r) in &new.regions {
        match old.regions.get(id) {
            Some(o) if !replaced(&r.source) && o == r => {}
            Some(o) if !replaced(&r.source) && o.source == r.source => {
                out.push(Update::UpdateRegion(r.clone()))
            }
            // New, moved to another source, or on a replaced source: added fresh (a region
            // that changed source was removed above only if its old source went, so remove
            // it here first).
            Some(_) if !replaced(&r.source) => {
                out.push(Update::RemoveRegion(*id));
                out.push(Update::AddRegion(r.clone()));
            }
            _ => out.push(Update::AddRegion(r.clone())),
        }
    }
    out
}

/// Applies updates to a document: what a plug-in's own copy does.
pub fn apply(doc: &mut Document, updates: &[Update]) {
    for u in updates {
        match u {
            Update::AddSource(s) => {
                doc.sources.insert(s.id, s.clone());
            }
            Update::RemoveSource(id) => {
                doc.sources.remove(id);
            }
            Update::AddRegion(r) | Update::UpdateRegion(r) => {
                doc.regions.insert(r.clip, r.clone());
            }
            Update::RemoveRegion(id) => {
                doc.regions.remove(id);
            }
        }
    }
}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.sources == other.sources && self.regions == other.regions
    }
}
