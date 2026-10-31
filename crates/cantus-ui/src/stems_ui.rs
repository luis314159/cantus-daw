//! *Separate stems…* (round 34, ADR 0085): an audio clip, the audio clips of a track or of
//! the selection, separated by a model into its parts — drums, bass, other, vocals (and
//! guitar and piano with the 6-stem model) — laid back as new tracks under a group bus,
//! each clip on the same spot as the clip it came from, sample for sample, the originals
//! muted, never deleted: **one undo step**. Each new file says which model made it, of
//! which version, under which licence, when, and from what (schema 48), and its clips wear
//! a small "AI" badge.
//!
//! The work: each source file the clips play is decoded and, if need be, converted to the
//! model's 44.1 kHz on a worker thread; the helper separates it (`MlService`); the stems
//! are converted back to the source's rate and length, so a stem clip is the original
//! clip with another file under it — its window, offset, fades, gain and warp unchanged —
//! and plays in step with it. Then the batch ([`plan`]) on the interface's thread.

#[path = "stems_ui_convert.rs"]
pub mod convert;
#[path = "stems_ui_models.rs"]
pub mod models;
#[path = "stems_ui_run.rs"]
pub mod run;
#[path = "stems_ui_view.rs"]
pub mod view;

use crate::clips::ClipRef;
use cantus_core::{
    AudioSource, AudioSourceId, Color, Command, CommandBus, GroupId, Provenance, StripGroup,
    TrackId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// What the user chose last (kept in the settings, `Settings::stems`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StemsSettings {
    /// The model: `htdemucs` (4 stems) or `htdemucs-6s`.
    pub model: String,
    /// Best: demucs's shift trick, four passes averaged (four times slower).
    pub best: bool,
    /// Try the graphics processor (DirectML, CoreML); the processor when it cannot.
    pub gpu: bool,
}

impl Default for StemsSettings {
    fn default() -> Self {
        StemsSettings {
            model: "htdemucs".into(),
            best: false,
            gpu: false,
        }
    }
}

/// The passes *Best* makes (demucs's `--shifts`).
pub const BEST_SHIFTS: u32 = 4;

/// What is being separated.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// These audio clips (the clip's menu, the selection).
    Clips(Vec<ClipRef>),
    /// Every audio clip of a track.
    Track(TrackId),
}

impl Target {
    /// The audio clips it takes, in the project as it is: the arrangement's only (a
    /// session clip is not laid out in time, review 2 #14).
    pub fn clips(&self, p: &cantus_core::Project) -> Vec<ClipRef> {
        let all: Vec<ClipRef> = match self {
            Target::Clips(c) => c.clone(),
            Target::Track(t) => p
                .track(*t)
                .map(|tr| tr.clips.iter().map(|c| (*t, c.id)).collect())
                .unwrap_or_default(),
        };
        all.into_iter()
            .filter(|r| arrangement_clip(p, *r).is_some_and(|c| c.audio.is_some()))
            .collect()
    }

    /// Whether its clips are on more than one track: refused, since each track's parts go
    /// under a bus where that track fed, at its level (review 2 #13).
    pub fn spans_tracks(&self, p: &cantus_core::Project) -> bool {
        let clips = self.clips(p);
        clips.iter().any(|c| c.0 != clips[0].0)
    }

    /// The words that name it: "“Song.wav” on “Mix”", "3 clips on “Mix”", "4 clips".
    pub fn words(&self, p: &cantus_core::Project) -> String {
        let clips = self.clips(p);
        let tracks: Vec<TrackId> = {
            let mut v: Vec<TrackId> = clips.iter().map(|c| c.0).collect();
            v.dedup();
            v
        };
        let track_name = |t: TrackId| p.track(t).map(|t| t.name.clone()).unwrap_or_default();
        match (clips.as_slice(), tracks.as_slice()) {
            ([one], [t]) => {
                let name = crate::audio_clips::region(p, *one)
                    .and_then(|(c, a)| p.audio_source(a.source).map(|s| (c, s)))
                    .map(|(_, s)| s.name.clone())
                    .unwrap_or_default();
                crate::i18n::tf(
                    "stems.target.one",
                    &[("source", &name), ("track", &track_name(*t))],
                )
            }
            (_, [t]) => crate::i18n::tf_count(
                "stems.target.clips_on",
                clips.len(),
                &[("track", &track_name(*t))],
            ),
            _ => crate::i18n::tf_count("stems.target.clips", clips.len(), &[]),
        }
    }

    /// The new bus's name, short enough for a strip: "Song stems" after the track the
    /// clips are on, "Stems" when they are on several.
    pub fn bus_name(&self, p: &cantus_core::Project) -> String {
        let mut tracks: Vec<TrackId> = match self {
            Target::Clips(c) => c.iter().map(|c| c.0).collect(),
            Target::Track(t) => vec![*t],
        };
        tracks.dedup();
        match tracks.as_slice() {
            [t] => match p.track(*t) {
                Some(track) => crate::i18n::tf("stems.bus", &[("track", &track.name)]),
                None => crate::i18n::t("stems.bus_many"),
            },
            _ => crate::i18n::t("stems.bus_many"),
        }
    }
}

/// Clip `r` on its track's arrangement (never a session clip).
pub fn arrangement_clip(p: &cantus_core::Project, r: ClipRef) -> Option<&cantus_core::Clip> {
    p.track(r.0)?.clips.iter().find(|c| c.id == r.1)
}

/// One source file's stems, at its own rate and length.
#[derive(Clone, Debug, PartialEq)]
pub struct Stem {
    /// The model's name for the part (`vocals`).
    pub part: String,
    pub path: PathBuf,
    pub frames: u64,
    pub rate: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Separated {
    pub source: AudioSourceId,
    pub stems: Vec<Stem>,
}

/// The model, as the provenance names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub licence: String,
}

/// What [`plan`] made.
pub struct Planned {
    pub command: Command,
    /// The new tracks, one per part, in the model's order.
    pub tracks: Vec<TrackId>,
    pub bus: cantus_core::StripId,
}

/// The palette's entry for a part: the drums red, the bass blue, the vocals green.
fn color_of(part: &str) -> Color {
    let i = match part {
        "drums" => 4,
        "bass" => 6,
        "vocals" => 3,
        "guitar" => 0,
        "piano" => 7,
        _ => 2,
    };
    Color::PALETTE[i]
}

/// The graphics processor's provider the runtime this system downloads for `model` carries
/// (`coreml`), if any: the processor-only builds Windows and Linux download carry none
/// (review 2 #11).
pub fn gpu_provider(app: &crate::app::CantusApp, model: &str) -> Option<String> {
    let manifest = app.ml.models().manifest();
    let m = manifest.model(model)?;
    manifest
        .runtime(&m.runtime, &cantus_content::models::platform())?
        .gpu
        .clone()
}

/// Where `model` runs: the graphics processor when the user asked for it and the runtime
/// carries one, else the processor.
pub fn device(app: &crate::app::CantusApp, model: &str) -> cantus_ml::spec::Device {
    if app.settings.stems.gpu && gpu_provider(app, model).is_some() {
        cantus_ml::spec::Device::Auto
    } else {
        cantus_ml::spec::Device::Cpu
    }
}

/// The fader level that makes a centred strip unity under `law`: +3.01 dB under −3 dB, 0
/// under 0 dB.
pub fn unity_at_centre(law: cantus_core::PanLaw) -> f32 {
    let (g, _) = law.gains(0.0);
    if g > 0.0 && g < 1.0 {
        cantus_core::linear_to_db(1.0 / g)
    } else {
        0.0
    }
}

/// The strips a separation made: each part track's strip (a track whose every audio clip
/// plays a separated file) and the bus it feeds when that bus is in the strip's group.
pub fn stems_strips(p: &cantus_core::Project) -> Vec<cantus_core::StripId> {
    let separated = |c: &cantus_core::Clip| {
        c.audio.as_ref().is_some_and(|a| {
            p.provenance
                .get(&a.source)
                .is_some_and(|v| v.kind == "separated")
        })
    };
    let mut out = Vec::new();
    for t in &p.tracks {
        let audio: Vec<&cantus_core::Clip> = t.clips.iter().filter(|c| c.audio.is_some()).collect();
        if audio.is_empty() || !audio.iter().all(|c| separated(c)) {
            continue;
        }
        let Some(strip) = t.strip.and_then(|s| p.strip(s)) else {
            continue;
        };
        if strip.group.is_none() {
            continue;
        }
        if !out.contains(&strip.id) {
            out.push(strip.id);
        }
        if let Some(bus) = strip.output.and_then(|o| p.strip(o)) {
            if bus.group == strip.group && !out.contains(&bus.id) {
                out.push(bus.id);
            }
        }
    }
    out
}

/// The project's pan law set to `law`, with every separation's part strips and bus trimmed
/// by the change in the centre's make-up — one undo step — so the stems keep summing to
/// the original (review 3 #3).
pub fn pan_law_command(p: &cantus_core::Project, law: cantus_core::PanLaw) -> Command {
    let delta = unity_at_centre(law) - unity_at_centre(p.pan_law);
    let mut commands = vec![Command::SetPanLaw { law }];
    if delta != 0.0 {
        for id in stems_strips(p) {
            if let Some(s) = p.strip(id) {
                commands.push(Command::SetStripGain {
                    id,
                    gain_db: s.gain_db + delta,
                });
            }
        }
    }
    if commands.len() == 1 {
        Command::SetPanLaw { law }
    } else {
        Command::Batch {
            label_key: "cmd.set_pan_law".into(),
            commands,
        }
    }
}

/// The batch: the stems' files with their provenance; a group and a bus (feeding where
/// the first clip's track fed); per part a strip on the bus and a track under the last
/// separated track, with the original clips' copies on it, each playing that part of
/// its clip's file; the originals muted. `part_name` gives a part's words, `bus_name`
/// the bus's. Ids from `bus`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    bus: &mut CommandBus,
    clips: &[ClipRef],
    results: &[Separated],
    card: &Card,
    base: Option<&Path>,
    date: &str,
    part_name: &dyn Fn(&str) -> String,
    bus_name: &str,
) -> Result<Planned, String> {
    let p = bus.project().clone();
    let parts: Vec<String> = results
        .first()
        .ok_or("nothing was separated")?
        .stems
        .iter()
        .map(|s| s.part.clone())
        .collect();
    let mut cmds = Vec::new();
    let mut files: HashMap<(AudioSourceId, &str), AudioSourceId> = HashMap::new();
    for r in results {
        let from = p
            .audio_source(r.source)
            .ok_or("a file the clips played is gone")?;
        for st in &r.stems {
            let id = AudioSourceId(bus.alloc_id());
            cmds.push(Command::InsertAudioSource {
                source: AudioSource {
                    id,
                    name: format!("{} — {}", from.name, part_name(&st.part)),
                    path: crate::audio_files::stored_path(base, &st.path),
                    sample_rate: st.rate,
                    channels: 2,
                    frames: st.frames,
                    sha256: None,
                    recorded: false,
                },
            });
            cmds.push(Command::SetAudioProvenance {
                id,
                provenance: Some(Box::new(Provenance {
                    kind: "separated".into(),
                    model: card.id.clone(),
                    model_name: card.name.clone(),
                    model_version: card.version,
                    licence: card.licence.clone(),
                    date: date.to_string(),
                    from: from.name.clone(),
                    part: st.part.clone(),
                })),
            });
            files.insert((r.source, st.part.as_str()), id);
        }
    }
    let first = clips.first().ok_or("no audio clip")?;
    if clips.iter().any(|c| c.0 != first.0) {
        return Err(crate::i18n::t("stems.status.one_track"));
    }
    let orig = p.track(first.0).ok_or("the clip's track is gone")?;
    // The group: how the desk shows them; the bus: where they sound, as the original did.
    let group = StripGroup {
        id: GroupId(bus.alloc_id()),
        name: bus_name.to_string(),
        color: Color::PALETTE[1],
        collapsed: false,
    };
    // Each stem passes two strips the original never did (its own and the bus), and each
    // centred strip takes the pan law's centre off (−3 dB by default): both start at the
    // level that gives that back, so the parts sum to the original at unity under every
    // law (review 2 #1).
    let stage_db = unity_at_centre(p.pan_law);
    let mut sum = bus.new_strip(bus_name);
    sum.gain_db = stage_db;
    sum.output = orig.strip;
    sum.group = Some(group.id);
    let sum_id = sum.id;
    let at = p.strips.len();
    cmds.push(Command::InsertStripGroup {
        index: p.strip_groups.len(),
        group: Box::new(group.clone()),
    });
    cmds.push(Command::InsertStrip {
        index: at,
        strip: Box::new(sum),
    });
    let below = clips
        .iter()
        .filter_map(|c| p.track_index(c.0))
        .max()
        .unwrap_or(p.tracks.len().saturating_sub(1));
    let mut tracks = Vec::new();
    for (k, part) in parts.iter().enumerate() {
        let mut strip = bus.new_strip(part_name(part));
        strip.gain_db = stage_db;
        strip.output = Some(sum_id);
        strip.group = Some(group.id);
        strip.color = Some(color_of(part));
        let strip_id = strip.id;
        cmds.push(Command::InsertStrip {
            index: at + 1 + k,
            strip: Box::new(strip),
        });
        let mut track = bus.new_track(part_name(part));
        track.color = color_of(part);
        track.strip = Some(strip_id);
        track.gain_db = orig.gain_db;
        track.pan = orig.pan;
        let tid = track.id;
        cmds.push(Command::InsertTrack {
            index: below + 1 + k,
            track: Box::new(track),
        });
        for c in clips {
            let clip = arrangement_clip(&p, *c).ok_or("a clip is gone")?;
            let a = clip.audio.as_ref().ok_or("not an audio clip")?;
            let file = *files
                .get(&(a.source, part.as_str()))
                .ok_or("a clip's file was not separated")?;
            // The clip whole — its crossfades, its comp window and seam, its lane, its mute
            // (a muted clip's parts stay silent, review 2 #2 and #3) — with an id of its own
            // and in no link group (its content is not the original's).
            let mut copy = clip.clone();
            copy.id = cantus_core::ClipId(bus.alloc_id());
            copy.link = None;
            copy.name = format!("{} — {}", clip.name, part_name(part));
            if let Some(ca) = copy.audio.as_mut() {
                ca.source = file;
                // The pitch editor's notes are the mix's, not a part's.
                ca.pitch = None;
            }
            cmds.push(Command::InsertClip {
                track: tid,
                clip: Box::new(copy),
            });
        }
        tracks.push(tid);
    }
    for c in clips {
        if p.track(c.0)
            .and_then(|t| t.clip(c.1))
            .is_some_and(|x| !x.muted)
        {
            cmds.push(Command::SetClipMute {
                track: c.0,
                clip: c.1,
                muted: true,
            });
        }
    }
    Ok(Planned {
        command: Command::Batch {
            label_key: "cmd.separate_stems".into(),
            commands: cmds,
        },
        tracks,
        bus: sum_id,
    })
}

/// The interface's state of the feature.
#[derive(Default)]
pub struct StemsUi {
    /// The dialog, while open, and what it would separate.
    pub dialog: Option<Target>,
    /// Separations under way (and the last ended, for its words).
    pub runs: Vec<run::Run>,
    /// The model the Models page shows, and a removal waiting for its confirmation.
    pub selected: Option<String>,
    pub confirm_remove: Option<String>,
    /// The project the dialog was opened on (`CantusApp::epoch`).
    pub dialog_epoch: u64,
    /// The window's context, for workers to wake it (set by [`frame`]).
    pub repaint: Option<egui::Context>,
    /// A model's removal asked for, and when: tried each frame until its files are free.
    pub removing: Option<(String, std::time::Instant)>,
    /// Agents' edits made of stems not yet known to be made: their folders, removed if the
    /// edit never is ([`run::settle_pending`]).
    pub pending: Vec<run::Pending>,
}

/// Today, as an ISO date.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", y + i64::from(m <= 2))
}

impl crate::app::CantusApp {
    /// Open *Separate stems…* on `target` (refused with a word when it holds no audio).
    pub fn open_separate_stems(&mut self, target: Target) {
        if !self.settings.ai.enabled {
            self.set_status(crate::i18n::t("stems.status.ai_off"));
            return;
        }
        if target.clips(self.project()).is_empty() {
            self.set_status(crate::i18n::t("stems.status.no_audio"));
            return;
        }
        if target.spans_tracks(self.project()) {
            self.set_status(crate::i18n::t("stems.status.one_track"));
            return;
        }
        self.stems.dialog = Some(target);
        self.stems.dialog_epoch = self.epoch;
    }
}

/// Every frame: the runs moved on, the dialog drawn.
pub fn frame(app: &mut crate::app::CantusApp, ctx: &egui::Context) {
    if app.stems.repaint.is_none() {
        app.stems.repaint = Some(ctx.clone());
    }
    run::step(app);
    // A run under way is looked at again soon even when nothing else asks for a frame
    // (its job's progress wakes the window too; this is the floor).
    if app.stems.runs.iter().any(|r| r.running()) {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    view::dialog(app, ctx);
}

/// The track menu's section: *Separate stems…* on a track with audio clips. True when it
/// was chosen (the menu closes).
pub fn track_menu(app: &mut crate::app::CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    // The AI area switched off hides it (Settings ▸ AI).
    if !app.settings.ai.enabled || Target::Track(id).clips(app.project()).is_empty() {
        return false;
    }
    let items = [
        Item::Band(Src::Key("stems.menu.band")),
        Item::Action {
            label: Src::Key("stems.menu.separate_track"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: true,
        },
    ];
    if crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked == Some(1) {
        app.open_separate_stems(Target::Track(id));
        return true;
    }
    false
}
