//! The switches of the mix cues (round 36, ADR 0093 §3): what automation cannot carry —
//! mute and solo (as the audibility they resolve to), polarity, swap, the inserts' switch,
//! each insert's bypass, the strip's own EQ, and the master's polarity, inserts' switch and
//! bypasses — compiled into a timeline of regions, one per cue, each holding the whole
//! desk's switches from its tick on.
//!
//! The engine reads it at its control points by **tick** (the region whose start is the last
//! at or before the song position), so a seek, a loop wrap, a block size or an offline render
//! changes nothing of what is heard; a change is faded out over the 16 samples before the
//! cue's grid point and in over the 16 after it, never cut. A project without cues — or with
//! cues that switch nothing — compiles to an empty timeline and the engine does nothing new.

use crate::mix_snap::{apply_state, MixCue};
use crate::model::{Project, StripEq};
use crate::time::Ticks;

/// One strip's switches in a region.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneStrip {
    /// Mute and solo resolved (`Project::strip_audibility`): silent in this region.
    pub silent: bool,
    pub phase_invert: bool,
    pub swap: bool,
    pub fx_enabled: bool,
    /// Per insert, in the strip's order.
    pub bypass: Vec<bool>,
    pub eq: StripEq,
}

/// The master's switches in a region.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneMaster {
    pub phase_invert: bool,
    pub fx_enabled: bool,
    pub bypass: Vec<bool>,
}

/// A region of the timeline: from `at` until the next region.
#[derive(Clone, Debug, PartialEq)]
pub struct MixScene {
    pub at: Ticks,
    /// Indexed like the snapshot's strips (the project's).
    pub strips: Vec<SceneStrip>,
    pub master: SceneMaster,
}

impl MixScene {
    fn same_switches(&self, o: &MixScene) -> bool {
        self.strips == o.strips && self.master == o.master
    }
}

/// The region playing at `tick`: the last one starting at or before it (0, the project's own
/// switches, before the first cue).
#[inline]
pub fn region_at(scenes: &[MixScene], tick: f64) -> usize {
    scenes
        .partition_point(|s| s.at.0 as f64 <= tick)
        .saturating_sub(1)
}

fn scene_of(p: &Project, at: Ticks) -> MixScene {
    let audible = p.strip_audibility();
    MixScene {
        at,
        strips: p
            .strips
            .iter()
            .zip(audible)
            .map(|(s, a)| SceneStrip {
                silent: !a,
                phase_invert: s.phase_invert,
                swap: s.swap_channels,
                fx_enabled: s.fx_enabled,
                bypass: s.effects.iter().map(|e| e.bypass).collect(),
                eq: s.eq,
            })
            .collect(),
        master: SceneMaster {
            phase_invert: p.master.phase_invert,
            fx_enabled: p.master.fx_enabled,
            bypass: p.master.effects.iter().map(|e| e.bypass).collect(),
        },
    }
}

/// The cues' switch timeline: region 0 is the project as it is, then one region per cue (in
/// time order, a tie to the later id), each the previous one with the cue's snapshot applied
/// as a recall would (its scope, safe strips sitting out a cue of everything). Empty when no
/// cue changes a switch.
pub fn timeline(project: &Project) -> Vec<MixScene> {
    if project.mix.cues.is_empty() {
        return Vec::new();
    }
    // Only what the switches and the audibility read: the strips, the groups' VCAs, the
    // master — never the tracks and their notes.
    let mut light = Project::empty("");
    light.strips = project.strips.clone();
    light.strip_groups = project.strip_groups.clone();
    light.mix.group_vcas = project.mix.group_vcas.clone();
    light.master = project.master.clone();
    let mut out = vec![scene_of(&light, Ticks(i64::MIN))];
    let mut cues: Vec<&MixCue> = project.mix.cues.iter().collect();
    cues.sort_by_key(|c| (c.at, c.id));
    for cue in cues {
        let Some(snap) = project.mix.snapshot(cue.snapshot) else {
            continue;
        };
        apply_state(
            &mut light,
            &snap.state,
            &cue.scope,
            cue.scope.is_empty(),
            false,
            false,
        );
        out.push(scene_of(&light, cue.at));
    }
    if out.iter().all(|s| s.same_switches(&out[0])) {
        return Vec::new();
    }
    out
}
