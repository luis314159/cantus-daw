//! Every modulator of a project, compiled for the engine (ADR 0057): what each one is, the
//! notes and the audio it hears, and the lanes it drives.
//!
//! **A modulated target is a lane.** The snapshot's automation already holds one
//! [`AutoLane`] per automated target; a modulated target that no clip automates gets a lane
//! with no pieces (its value is the project's, everywhere), so the engine finds every
//! destination — automated, modulated or both — in one list and adds the modulation to the
//! lane's position: `clamp(position + Σ depth × amount × output, 0, 1)`.
//!
//! **The null rule.** A muted modulator, a route of depth 0, a route whose target names
//! nothing and a modulator left with no route compile to nothing at all: no plan, no lane
//! — the engine then does exactly what it did without them.

use super::eval::Gates;
use super::{EnvTrigger, Listen, ModHost, ModSource};
use crate::automation::range::{base_value, range_of};
use crate::automation::{AutoLane, AutoTarget};
use crate::model::{ModulatorId, Project};
use crate::snapshot::TrackSnapshot;
use std::sync::Arc;

/// Whose audio a follower or an audio envelope hears, as the snapshot indexes it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ListenAt {
    /// A strip, by its index in the snapshot's strips.
    Strip(usize),
    Master,
    /// What it listened to is gone (a removed strip, a track with no strip): silence.
    Silent,
}

/// One modulator as the engine plays it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModPlan {
    pub id: ModulatorId,
    pub source: ModSource,
    /// Its amount where nothing automates or modulates it.
    pub amount: f32,
    /// The lane (index into `Snapshot::automation`) its amount is read from, when a clip
    /// automates it or another modulator drives it.
    pub amount_lane: Option<usize>,
    /// The track (snapshot index) whose notes it hears, and that track's note gates.
    pub notes: Option<usize>,
    pub gates: Option<Arc<Gates>>,
    /// Whose audio it hears (a follower, an audio-triggered envelope).
    pub listen: Option<ListenAt>,
    /// The lanes it drives (indices into `Snapshot::automation`), each with its depth.
    pub routes: Vec<(usize, f32)>,
}

/// The routes a modulator keeps: not muted, depth not zero, the target there.
fn live_routes<'a>(
    project: &'a Project,
    m: &'a super::Modulator,
) -> impl Iterator<Item = &'a super::ModRoute> + 'a {
    m.routes
        .iter()
        .filter(move |r| !m.muted && r.depth != 0.0 && r.depth.is_finite())
        .filter(move |r| range_of(project, &r.target).is_some())
        // One level of nesting: a route into another modulator's amount is kept only if
        // that modulator itself drives something.
        .filter(move |r| match &r.target {
            AutoTarget::ModAmount(other) => project
                .modulator(*other)
                .is_some_and(|(_, _, o)| o.id != m.id && !o.muted && drives_plain(project, o)),
            _ => true,
        })
}

/// Whether `m` drives a parameter that is not another modulator's amount.
fn drives_plain(project: &Project, m: &super::Modulator) -> bool {
    m.routes.iter().any(|r| {
        r.depth != 0.0
            && r.depth.is_finite()
            && !matches!(r.target, AutoTarget::ModAmount(_))
            && range_of(project, &r.target).is_some()
    })
}

/// Compile every modulator of `project` against the snapshot's `tracks`, adding a lane to
/// `lanes` for each modulated target that has none (and keeping `lanes` in target order).
pub fn compile(
    project: &Project,
    tracks: &[TrackSnapshot],
    lanes: &mut Vec<AutoLane>,
) -> Vec<ModPlan> {
    // Which modulators play, and the targets they need lanes for.
    let playing: Vec<(ModHost, &super::Modulator)> = project
        .all_modulators()
        .filter(|(_, m)| !m.muted && live_routes(project, m).next().is_some())
        .collect();
    if playing.is_empty() {
        return Vec::new();
    }
    let mut needed: Vec<AutoTarget> = playing
        .iter()
        .flat_map(|(_, m)| live_routes(project, m).map(|r| r.target.clone()))
        .collect();
    needed.sort();
    needed.dedup();
    for target in needed {
        if lanes.iter().any(|l| l.target == target) {
            continue;
        }
        let (Some(range), Some(base)) = (range_of(project, &target), base_value(project, &target))
        else {
            continue;
        };
        lanes.push(AutoLane {
            target,
            range,
            base,
            segs: Vec::new(),
        });
    }
    lanes.sort_by(|a, b| a.target.cmp(&b.target));
    let lane = |t: &AutoTarget| lanes.binary_search_by(|l| l.target.cmp(t)).ok();
    let track_index = |id| tracks.iter().position(|t| t.id == id);
    let mut gates_of: Vec<Option<Arc<Gates>>> = vec![None; tracks.len()];
    let mut out = Vec::with_capacity(playing.len());
    for (host, m) in playing {
        let notes_track = match &m.source {
            ModSource::Lfo(l) if l.retrigger == super::Retrigger::Note => {
                Some(l.notes.or_else(|| project.host_notes_track(host)))
            }
            ModSource::Envelope(e) => match e.trigger {
                EnvTrigger::Notes { track } => {
                    Some(track.or_else(|| project.host_notes_track(host)))
                }
                EnvTrigger::Audio { .. } => None,
            },
            _ => None,
        }
        .flatten()
        .and_then(track_index);
        let gates = notes_track.map(|ti| {
            let shared = gates_of[ti]
                .get_or_insert_with(|| Arc::new(Gates::of_events(&tracks[ti].events)))
                .clone();
            // An envelope reads the level each gate starts from, computed here, off the
            // audio thread, for its own settings at the project's tempo.
            match &m.source {
                ModSource::Envelope(e) => {
                    Arc::new(shared.with_levels(e, 60.0 / project.tempo_bpm.max(1e-9)))
                }
                _ => shared,
            }
        });
        let listen = match &m.source {
            ModSource::Follower(f) => Some(f.listen),
            ModSource::Envelope(e) => match e.trigger {
                EnvTrigger::Audio { listen, .. } => Some(listen),
                EnvTrigger::Notes { .. } => None,
            },
            _ => None,
        }
        .map(|l| listen_at(project, host, l));
        out.push(ModPlan {
            id: m.id,
            source: m.source.clone(),
            amount: m.amount.clamp(0.0, 1.0),
            amount_lane: lane(&AutoTarget::ModAmount(m.id)),
            notes: notes_track,
            gates,
            listen,
            routes: live_routes(project, m)
                .filter_map(|r| lane(&r.target).map(|i| (i, r.depth.clamp(-1.0, 1.0))))
                .collect(),
        });
    }
    out
}

/// Where `listen` points, for a modulator on `host`: a strip that is gone is silence,
/// never the whole mix.
fn listen_at(project: &Project, host: ModHost, listen: Listen) -> ListenAt {
    let strip = match listen {
        Listen::Master => return ListenAt::Master,
        Listen::Host if host == ModHost::Master => return ListenAt::Master,
        Listen::Host => project.host_listen_strip(host),
        Listen::Strip(s) => Some(s),
    };
    strip
        .and_then(|s| project.strip_index(s))
        .map_or(ListenAt::Silent, ListenAt::Strip)
}
