//! VCA faders (round 36, ADR 0093).
//!
//! A VCA is a strip with **no audio**: its fader scales the gain stage of the strips that
//! follow it, in decibels, and its mute and solo are theirs too. Nothing a member's own
//! fader, its automation or its modulators say is rewritten — the VCA adds on top, so the
//! member's fader still shows where the member is. How it is decided, and tested:
//!
//! * **Who follows a VCA.** A strip's `vcas`, and every strip of a group the VCA is given
//!   to ([`MixBook::group_vcas`](crate::mix_snap::MixBook)). A VCA can follow another VCA
//!   (nesting), through the same two ways.
//! * **What a strip hears of VCAs** is the set of VCAs that reach it by any path, **each
//!   counted once**: a member of two VCAs gets both (their decibels add), and a member
//!   reached by one VCA twice (through a nested VCA and directly) gets it once. A nested
//!   VCA's fader scales its members too, being one of the VCAs that reach them.
//! * **Mute and solo follow.** A muted VCA mutes everyone it reaches; a soloed VCA solos
//!   everyone it reaches (FL's solo: with their routes).
//! * **At unity nothing is touched**: no VCA, or every one at 0 dB, leaves each sample
//!   as it was, to the bit (`Snapshot::compile` adds nothing for them).

use crate::automation::{AutoLane, AutoSeg, AutoTarget};
use crate::model::{MixerStrip, Project, StripId, MIN_GAIN_DB};
use serde::{Deserialize, Serialize};

/// What a strip is.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StripKind {
    /// A channel of the desk: audio in, inserts, a fader, an output.
    #[default]
    Audio,
    /// A VCA fader: no audio, no inserts, no routing; a fader, mute and solo that act on
    /// the strips that follow it.
    Vca,
}

impl StripKind {
    pub fn is_audio(&self) -> bool {
        matches!(self, StripKind::Audio)
    }
}

/// Two fader levels added in decibels, with the fader's floor kept: either at or below
/// [`MIN_GAIN_DB`] (the fader's bottom, silence) is silence whatever the other says — a
/// muted-down member under a raised VCA stays silent, and so does a member under a VCA
/// pulled to the bottom (review 1, item 3).
#[inline]
pub fn sum_db(a: f32, b: f32) -> f32 {
    if a <= MIN_GAIN_DB || b <= MIN_GAIN_DB {
        MIN_GAIN_DB
    } else {
        (a + b).max(MIN_GAIN_DB)
    }
}

/// A member's gain that the engine adds to its own lane's value (dB, summed with the
/// member's automated fader): `static_db` from the VCAs that are not automated plus the
/// value of each automated VCA's lane (`from`, indices into `Snapshot::automation`).
#[derive(Clone, Debug, PartialEq)]
pub struct VcaLink {
    /// The member's own `StripGain` lane.
    pub lane: usize,
    pub static_db: f32,
    pub from: Vec<usize>,
}

impl Project {
    /// Whether `id` is a VCA.
    pub fn is_vca(&self, id: StripId) -> bool {
        self.strip(id).is_some_and(|s| s.kind == StripKind::Vca)
    }

    /// Whether any strip is a VCA (the fast path of everything that follows).
    pub fn has_vca(&self) -> bool {
        self.strips.iter().any(|s| s.kind == StripKind::Vca)
    }

    /// The VCAs `strip` follows directly: its own list and its group's.
    fn vca_direct(&self, strip: &MixerStrip) -> impl Iterator<Item = StripId> + '_ {
        let group = strip
            .group
            .and_then(|g| self.mix.group_vca(g))
            .filter(|v| *v != strip.id);
        let own = strip.vcas.clone();
        own.into_iter().chain(group)
    }

    /// Every VCA that reaches `id`, each once, in the desk's order. Terminates on a cycle
    /// (which the commands refuse and a hand-edited file may hold).
    pub fn vca_governing(&self, id: StripId) -> Vec<StripId> {
        let mut seen: Vec<StripId> = Vec::new();
        let mut stack: Vec<StripId> = vec![id];
        while let Some(s) = stack.pop() {
            let Some(strip) = self.strip(s) else { continue };
            for v in self.vca_direct(strip) {
                if v != id && self.is_vca(v) && !seen.contains(&v) {
                    seen.push(v);
                    stack.push(v);
                }
            }
        }
        let mut out: Vec<StripId> = self
            .strips
            .iter()
            .map(|s| s.id)
            .filter(|s| seen.contains(s))
            .collect();
        out.dedup();
        out
    }

    /// Every strip `vca` reaches, each once, in the desk's order (VCAs included).
    pub fn vca_members(&self, vca: StripId) -> Vec<StripId> {
        if !self.is_vca(vca) {
            return Vec::new();
        }
        self.strips
            .iter()
            .filter(|s| s.id != vca && self.vca_governing(s.id).contains(&vca))
            .map(|s| s.id)
            .collect()
    }

    /// Whether the follow lists hold a cycle: a VCA that reaches itself.
    pub fn vca_cycle(&self) -> bool {
        self.strips.iter().any(|s| {
            s.kind == StripKind::Vca && {
                let mut seen: Vec<StripId> = Vec::new();
                let mut stack = vec![s.id];
                let mut hit = false;
                while let Some(x) = stack.pop() {
                    let Some(st) = self.strip(x) else { continue };
                    // The same follow edges as `vca_governing`: a VCA in its own group
                    // follows nothing through it (review 1, item 2).
                    for v in self.vca_direct(st) {
                        if v == s.id {
                            hit = true;
                        }
                        if self.is_vca(v) && !seen.contains(&v) {
                            seen.push(v);
                            stack.push(v);
                        }
                    }
                }
                hit
            }
        })
    }

    /// The VCAs' contribution to every strip's fader that does not ride a lane: the sum of
    /// the decibels of the VCAs that reach it and have no lane of their own, or `None`
    /// for a strip no VCA reaches (whose gain is then read exactly as before).
    pub fn vca_static_db(&self, id: StripId, laned: &dyn Fn(StripId) -> bool) -> Option<f32> {
        let g = self.vca_governing(id);
        if g.is_empty() {
            return None;
        }
        Some(
            g.iter()
                .filter(|v| !laned(**v))
                .filter_map(|v| self.strip(*v))
                .fold(0.0, |db, v| sum_db(db, v.gain_db)),
        )
    }
}

impl Project {
    /// [`Project::strip_audibility`] with VCAs: a strip is muted when it or a VCA that
    /// reaches it is; soloed when it or a VCA that reaches it is (and then its routes
    /// play, as FL's solo does).
    pub(crate) fn strip_audibility_vca(&self) -> Vec<bool> {
        let gov: Vec<Vec<StripId>> = self
            .strips
            .iter()
            .map(|s| self.vca_governing(s.id))
            .collect();
        let by = |v: &StripId, f: &dyn Fn(&MixerStrip) -> bool| self.strip(*v).is_some_and(f);
        let mut audible: Vec<bool> = self
            .strips
            .iter()
            .zip(&gov)
            .map(|(s, g)| !s.muted && !g.iter().any(|v| by(v, &|x| x.muted)))
            .collect();
        let soloed =
            |i: usize| self.strips[i].soloed || gov[i].iter().any(|v| by(v, &|x| x.soloed));
        if (0..self.strips.len()).any(soloed) {
            let keep = self.connected_strips(
                (0..self.strips.len())
                    .filter(|i| soloed(*i))
                    .map(|i| self.strips[i].id),
            );
            for (i, a) in audible.iter_mut().enumerate() {
                *a = *a && (keep.contains(&i) || self.strips[i].solo_safe);
            }
        }
        audible
    }
}

/// What compiling the VCAs into a snapshot gives: the links the engine adds to lanes, and
/// each strip's static dB offset (by strip index; `None`: untouched).
#[derive(Debug)]
pub struct Compiled {
    pub links: Vec<VcaLink>,
    pub fold_db: Vec<Option<f32>>,
    /// Per lane of the compiled automation: the index of its link in `links`, or
    /// [`NO_LINK`] — what the engine reads at a control point instead of searching.
    pub by_lane: Vec<u32>,
}

/// No link for this lane.
pub const NO_LINK: u32 = u32::MAX;

/// Compile the VCAs: lanes for members whose VCAs are automated, links to add, and the
/// static decibels folded into the fader of each member. Called after the automation and
/// modulator lanes are compiled (a modulated VCA fader is one of them).
pub fn compile(project: &Project, lanes: &mut Vec<AutoLane>) -> Compiled {
    let n = project.strips.len();
    let mut out = Compiled {
        links: Vec::new(),
        fold_db: vec![None; n],
        by_lane: Vec::new(),
    };
    if !project.has_vca() {
        return out;
    }
    let lane_of = |lanes: &[AutoLane], s: StripId| {
        lanes
            .iter()
            .position(|l| l.target == AutoTarget::StripGain(s))
    };
    for (i, s) in project.strips.iter().enumerate() {
        if s.kind != StripKind::Audio {
            continue;
        }
        let gov = project.vca_governing(s.id);
        if gov.is_empty() {
            continue;
        }
        let laned: Vec<(StripId, usize)> = gov
            .iter()
            .filter_map(|v| lane_of(lanes, *v).map(|l| (*v, l)))
            .collect();
        let is_laned = |v: StripId| laned.iter().any(|(x, _)| *x == v);
        let static_db = project.vca_static_db(s.id, &is_laned).unwrap_or(0.0);
        out.fold_db[i] = Some(static_db);
        // Nothing rides a lane: the static decibels are all, folded into the fader. A member
        // with a lane of its own (an automated or modulated fader) replaces the fader's
        // value at every control point, so the VCAs are added to the lane.
        if laned.is_empty() && lane_of(lanes, s.id).is_none() {
            continue;
        }
        // The member rides a lane of its own: made, holding its fader, when it has none.
        let target = AutoTarget::StripGain(s.id);
        let lane = match lane_of(lanes, s.id) {
            Some(l) => l,
            None => {
                let (Some(range), Some(base)) = (
                    crate::automation::range::range_of(project, &target),
                    crate::automation::range::base_value(project, &target),
                ) else {
                    continue;
                };
                lanes.push(AutoLane {
                    target,
                    range,
                    base,
                    segs: Vec::<AutoSeg>::new(),
                });
                lanes.len() - 1
            }
        };
        out.links.push(VcaLink {
            lane,
            static_db,
            from: laned.iter().map(|(_, l)| *l).collect(),
        });
    }
    if !out.links.is_empty() {
        out.by_lane = vec![NO_LINK; lanes.len()];
        for (k, l) in out.links.iter().enumerate() {
            out.by_lane[l.lane] = k as u32;
        }
    }
    out
}

/// A freeze region's strip part with its VCAs (ADR 0093, review 1 item 1): a strip summed
/// into a frozen bus is heard through its fader stage, so the VCAs that reach it — their
/// fader and mute — are part of what the render is made of. Nothing is added for a strip no
/// VCA reaches, so every key from before VCAs stays the key it was. (A VCA's automation and
/// modulators reach the region through `freeze::target_in_strip_region`.)
pub fn with_vcas(project: &Project, id: StripId, mut part: serde_json::Value) -> serde_json::Value {
    let gov = project.vca_governing(id);
    if gov.is_empty() {
        return part;
    }
    let vcas: Vec<serde_json::Value> = gov
        .iter()
        .filter_map(|v| project.strip(*v))
        .map(|v| serde_json::json!([v.id.0, v.gain_db, v.muted]))
        .collect();
    if let Some(o) = part.as_object_mut() {
        o.insert("vca".into(), serde_json::Value::Array(vcas));
    }
    part
}

#[cfg(test)]
#[path = "vca_tests.rs"]
mod tests;
