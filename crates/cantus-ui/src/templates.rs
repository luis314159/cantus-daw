//! Project and track templates (round 31, ADR 0072).
//!
//! - **Project templates** are project files (`.cantus`) in the data folder's
//!   `templates/projects`, written by *Save as template* and read by *New from template* —
//!   through `cantus-project`, so a template from an older version is migrated as any
//!   project is. Four starters come with Cantus, built here from our own instruments and
//!   effects (never files in the repository): *Empty song*, *Beat*, *Band recording* and
//!   *Mastering*. A project made from a template is a new, untitled one: saving asks where.
//! - **Track templates** are project files too, in `templates/tracks`, each holding one
//!   track — its instrument, colour, icon, level and balance, input, modulators — with its
//!   mixer strip (inserts, EQ, fader, pan, width, sends and output), and, as named stand-ins,
//!   the strips its sends, output and side-chains reach. Its clips are not kept: a template
//!   is a sound and a channel, not a song. Inserted into a project it is one undo step: its
//!   strip takes the first free one's place (or the end), every id is new, and what it
//!   reached is found **by name** in that project — a send to *Reverb* finds the *Reverb*
//!   return there; what is not there is said and left out (an output to the master).

use crate::i18n::t;
use cantus_core::{
    AutoTarget, Command, EffectId, InstrumentKind, InstrumentSpec, MixerStrip, ModulatorId,
    Project, StripId, Ticks, Track, TrackId,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The folder of the user's project templates under a templates root.
pub const PROJECTS: &str = "projects";
/// The folder of the user's track templates under a templates root.
pub const TRACKS: &str = "tracks";

/// A starter that comes with Cantus.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Starter {
    Empty,
    Beat,
    Band,
    Mastering,
}

impl Starter {
    pub const ALL: [Starter; 4] = [
        Starter::Empty,
        Starter::Beat,
        Starter::Band,
        Starter::Mastering,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Starter::Empty => "empty",
            Starter::Beat => "beat",
            Starter::Band => "band",
            Starter::Mastering => "mastering",
        }
    }

    /// The i18n key of its name.
    pub fn name_key(self) -> &'static str {
        match self {
            Starter::Empty => "workflow.starter.empty",
            Starter::Beat => "workflow.starter.beat",
            Starter::Band => "workflow.starter.band",
            Starter::Mastering => "workflow.starter.mastering",
        }
    }

    /// The i18n key of the line saying what it holds.
    pub fn what_key(self) -> &'static str {
        match self {
            Starter::Empty => "workflow.starter.empty.what",
            Starter::Beat => "workflow.starter.beat.what",
            Starter::Band => "workflow.starter.band.what",
            Starter::Mastering => "workflow.starter.mastering.what",
        }
    }

    /// The project it starts, in the interface's language.
    pub fn build(self) -> Project {
        match self {
            Starter::Empty => empty_song(),
            Starter::Beat => beat(),
            Starter::Band => band(),
            Starter::Mastering => mastering(),
        }
    }
}

/// Where a project template comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TemplateRef {
    Starter(Starter),
    /// One of the user's, by its file.
    User(PathBuf),
}

impl TemplateRef {
    /// Stable across languages: what the palette counts its uses by.
    pub fn key(&self) -> String {
        match self {
            TemplateRef::Starter(s) => format!("template:starter:{}", s.id()),
            TemplateRef::User(p) => format!("template:user:{}", p.display()),
        }
    }
}

/// A project template as the start page and the palette list it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectTemplate {
    pub template: TemplateRef,
    pub name: String,
    /// What it holds, in a few words (a starter's line; the user's say *Your template*).
    pub what: String,
}

/// A track template as the picker and the palette list it.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackTemplate {
    pub path: PathBuf,
    pub name: String,
}

/// The user's templates in the data folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Store {
    /// `<data>/templates`; `None` before the start-up wiring (tests make their own).
    pub root: Option<PathBuf>,
}

/// A name made safe as a file's: what a file system refuses becomes a dash, and an empty
/// name is *Template*.
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        "Template".into()
    } else {
        s.chars().take(80).collect()
    }
}

fn listed(dir: &Path) -> Vec<(String, PathBuf)> {
    let ext = cantus_project::FILE_EXTENSION;
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == ext))
                .filter_map(|p| {
                    let stem = p.file_stem()?.to_string_lossy().to_string();
                    Some((stem, p))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|(name, _)| crate::picker_catalog::fold(name));
    out
}

impl Store {
    pub fn at(root: PathBuf) -> Store {
        Store { root: Some(root) }
    }

    pub fn projects_dir(&self) -> Option<PathBuf> {
        self.root.as_ref().map(|r| r.join(PROJECTS))
    }

    pub fn tracks_dir(&self) -> Option<PathBuf> {
        self.root.as_ref().map(|r| r.join(TRACKS))
    }

    /// The starters, then the user's project templates by name.
    pub fn projects(&self) -> Vec<ProjectTemplate> {
        let mut out: Vec<ProjectTemplate> = Starter::ALL
            .iter()
            .map(|s| ProjectTemplate {
                template: TemplateRef::Starter(*s),
                name: t(s.name_key()),
                what: t(s.what_key()),
            })
            .collect();
        if let Some(dir) = self.projects_dir() {
            out.extend(listed(&dir).into_iter().map(|(name, p)| ProjectTemplate {
                template: TemplateRef::User(p),
                name,
                what: t("workflow.template.yours"),
            }));
        }
        out
    }

    /// The user's track templates, by name.
    pub fn tracks(&self) -> Vec<TrackTemplate> {
        self.tracks_dir()
            .map(|dir| {
                listed(&dir)
                    .into_iter()
                    .map(|(name, path)| TrackTemplate { path, name })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The file a template of `name` is written to in `dir`.
    fn file_in(dir: &Path, name: &str) -> PathBuf {
        dir.join(format!(
            "{}.{}",
            file_stem(name),
            cantus_project::FILE_EXTENSION
        ))
    }

    /// Whether a project template of this name is there already (saving replaces it).
    pub fn project_exists(&self, name: &str) -> bool {
        self.projects_dir()
            .is_some_and(|d| Self::file_in(&d, name).exists())
    }

    /// Whether a track template of this name is there already (saving replaces it).
    pub fn track_exists(&self, name: &str) -> bool {
        self.tracks_dir()
            .is_some_and(|d| Self::file_in(&d, name).exists())
    }

    /// *Save as template*: the project as it is, under `name`; `base` is the project's own
    /// folder, which its relative audio paths are read against.
    pub fn save_project(
        &self,
        project: &Project,
        base: Option<&Path>,
        name: &str,
    ) -> Result<PathBuf, String> {
        let dir = self
            .projects_dir()
            .ok_or_else(|| t("workflow.error.no_folder"))?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = Self::file_in(&dir, name);
        let mut p = project.clone();
        p.name = name.trim().to_string();
        // A template writes no automation: every track and the master read their curves
        // (a track left in Write would overwrite on the first Play with the global arm on).
        read_only(&mut p);
        // Its audio comes along by where it is: a relative path would be read against the
        // templates' folder.
        absolute_audio(&mut p, base);
        cantus_project::save(&p, &path, crate::APP_VERSION).map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// *Save track as template*: `track` of `project`, its strip and what it reaches.
    pub fn save_track(
        &self,
        project: &Project,
        track: TrackId,
        name: &str,
    ) -> Result<PathBuf, String> {
        let dir = self
            .tracks_dir()
            .ok_or_else(|| t("workflow.error.no_folder"))?;
        let p = track_template(project, track, name).ok_or_else(|| t("workflow.error.no_track"))?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = Self::file_in(&dir, name);
        cantus_project::save(&p, &path, crate::APP_VERSION).map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// Delete one of the user's templates — only a file in this store's folders.
    pub fn delete(&self, path: &Path) -> Result<(), String> {
        let inside = [self.projects_dir(), self.tracks_dir()]
            .into_iter()
            .flatten()
            .any(|d| path.parent() == Some(d.as_path()));
        if !inside {
            return Err(t("workflow.error.not_a_template"));
        }
        std::fs::remove_file(path).map_err(|e| e.to_string())
    }
}

/// Audio paths made absolute against `base` (the project's folder), so a template's audio
/// is found wherever the template is opened.
fn absolute_audio(p: &mut Project, base: Option<&Path>) {
    for src in &mut p.audio_pool {
        let full = cantus_project::collect::resolve(base, &src.path);
        if full.is_absolute() {
            src.path = full.to_string_lossy().to_string();
        }
    }
}

/// The project a template starts: a starter built now, or the user's file read (migrated
/// when older) — named *Untitled*, unsaved.
pub fn open_template(template: &TemplateRef) -> Result<Project, String> {
    let mut p = match template {
        TemplateRef::Starter(s) => s.build(),
        TemplateRef::User(path) => {
            cantus_project::load(path, false)
                .map_err(|e| e.to_string())?
                .project
        }
    };
    p.name = t("app.untitled");
    Ok(p)
}

// ---- track templates --------------------------------------------------------------------

/// Whether a strip is blank: nothing on it, nothing in it, nothing routed to it — the
/// place a new track's strip can take without the desk growing.
fn blank(project: &Project, s: &MixerStrip) -> bool {
    let reached = project.strips.iter().any(|o| {
        o.output == Some(s.id)
            || o.sends.iter().any(|x| x.to == s.id)
            || o.effects.iter().any(|e| e.sidechain == Some(s.id))
    }) || project
        .master
        .effects
        .iter()
        .any(|e| e.sidechain == Some(s.id));
    project.tracks_on_strip(s.id).next().is_none()
        && !reached
        && s.name.is_empty()
        && s.effects.is_empty()
        && s.sends.is_empty()
        && s.output.is_none()
        && s.modulators.is_empty()
        && s.frozen.is_none()
        && s.group.is_none()
        && s.dock == cantus_core::Dock::Middle
        && s.gain_db == 0.0
        && s.pan == 0.0
        && s.width == 1.0
        && s.eq.is_flat()
}

/// The name a strip is found by from a template: its own, else its first track's — never
/// its number, which means another strip in another project (review 3).
fn routable_name(project: &Project, id: StripId) -> Option<String> {
    let s = project.strip(id)?;
    if !s.name.trim().is_empty() {
        return Some(s.name.clone());
    }
    project
        .tracks_on_strip(id)
        .next()
        .map(|t| t.name.clone())
        .filter(|n| !n.trim().is_empty())
}

/// Every track and the master back to reading their automation.
pub fn read_only(p: &mut Project) {
    for tr in &mut p.tracks {
        tr.auto_mode = cantus_core::AutoMode::Read;
    }
    p.master.auto_mode = cantus_core::AutoMode::Read;
}

/// A stand-in for a strip a template reaches, by its name; `None` for a strip with none
/// (the route is not kept).
fn stand_in(project: &Project, id: StripId) -> Option<MixerStrip> {
    let mut s = Project::default().make_strip(routable_name(project, id)?);
    s.id = id;
    Some(s)
}

/// The template of `track`: a project of its own holding the track (without its clips,
/// takes, comps, freeze or delay; not muted, soloed or armed), its strip as it is (not
/// frozen, in no group), and stand-ins for the strips its strip sends to, outputs to or
/// keys from. `None` when there is no such track.
pub fn track_template(project: &Project, track: TrackId, name: &str) -> Option<Project> {
    let mut t = project.track(track)?.clone();
    t.clips.clear();
    t.slots.clear(); // r33:session (its session clips are the song's, as its clips)
    t.comps.clear();
    t.active_comp = 0;
    t.frozen = None;
    t.lateness = None;
    t.muted = false;
    t.soloed = false;
    t.armed = false;
    t.takes_open = false;
    t.auto_mode = cantus_core::AutoMode::Read;
    let mut out = Project::new(name.trim());
    out.next_id = project.next_id;
    out.tracks.clear();
    out.strips.clear();
    out.strip_groups.clear();
    out.tempo_bpm = project.tempo_bpm;
    if let Some(s) = t.strip.and_then(|id| project.strip(id)) {
        let mut s = s.clone();
        s.frozen = None;
        s.group = None;
        s.muted = false;
        s.soloed = false;
        let mut reached: Vec<StripId> = s.sends.iter().map(|x| x.to).collect();
        reached.extend(s.output);
        reached.extend(s.effects.iter().filter_map(|e| e.sidechain));
        out.strips.push(s);
        for id in reached {
            if !id.is_master() && !out.strips.iter().any(|x| x.id == id) {
                out.strips.extend(stand_in(project, id));
            }
        }
    } else {
        t.strip = None;
    }
    out.tracks.push(t);
    Some(out)
}

/// A track template read from its file (migrated when older).
pub fn load_track_template(path: &Path) -> Result<Project, String> {
    let p = cantus_project::load(path, false)
        .map_err(|e| e.to_string())?
        .project;
    if p.tracks.len() != 1 {
        return Err(t("workflow.error.not_a_track_template"));
    }
    Ok(p)
}

/// What inserting a track template makes.
#[derive(Clone, Debug, PartialEq)]
pub struct Insert {
    /// One undo step (`cmd.insert_track_template`).
    pub command: Command,
    /// The new track.
    pub track: TrackId,
    /// What the template reached that this project has no strip of that name for: left
    /// out (a send dropped, an output to the master, a key taken away).
    pub missing: Vec<String>,
}

/// New ids for everything a template brings, and where its stand-ins land.
struct Ids {
    track: (TrackId, TrackId),
    strip: Option<(StripId, StripId)>,
    effects: HashMap<EffectId, EffectId>,
    mods: HashMap<ModulatorId, ModulatorId>,
    /// The track's note effects (ADR 0069): their new ids.
    notefx: HashMap<cantus_core::notefx::NoteFxId, cantus_core::notefx::NoteFxId>,
    /// A stand-in's id → the strip of that name in the project, if any.
    reached: HashMap<StripId, Option<StripId>>,
}

impl Ids {
    fn strip(&self, id: StripId) -> Option<StripId> {
        if id.is_master() {
            return Some(id);
        }
        match self.strip {
            Some((old, new)) if old == id => Some(new),
            _ => self.reached.get(&id).copied().flatten(),
        }
    }

    fn track(&self, id: TrackId) -> Option<TrackId> {
        (id == self.track.0).then_some(self.track.1)
    }

    fn effect(&self, id: EffectId) -> Option<EffectId> {
        self.effects.get(&id).copied()
    }

    /// A modulator's target in the new ids; `None` when it rode something the template does
    /// not bring (it is left out). Every target is named here, none by a wildcard: a new
    /// kind of target must say how a template carries it.
    fn target(&self, t: &AutoTarget) -> Option<AutoTarget> {
        use AutoTarget as A;
        Some(match t {
            A::TrackGain(tr) => A::TrackGain(self.track(*tr)?),
            A::TrackPan(tr) => A::TrackPan(self.track(*tr)?),
            A::StripGain(s) => A::StripGain(self.strip(*s)?),
            A::StripPan(s) => A::StripPan(self.strip(*s)?),
            A::StripWidth(s) => A::StripWidth(self.strip(*s)?),
            A::Send { strip, to } => A::Send {
                strip: self.strip(*strip)?,
                to: self.strip(*to)?,
            },
            A::MasterGain => A::MasterGain,
            A::MasterPan => A::MasterPan,
            A::MasterWidth => A::MasterWidth,
            A::Effect { effect, key } => A::Effect {
                effect: self.effect(*effect)?,
                key: key.clone(),
            },
            A::Instrument { track, key } => A::Instrument {
                track: self.track(*track)?,
                key: key.clone(),
            },
            A::Firmus { track, param } => A::Firmus {
                track: self.track(*track)?,
                param: *param,
            },
            A::Plugin { effect, id } => A::Plugin {
                effect: self.effect(*effect)?,
                id: *id,
            },
            A::PluginInstrument { track, id } => A::PluginInstrument {
                track: self.track(*track)?,
                id: *id,
            },
            A::ModAmount(m) => A::ModAmount(*self.mods.get(m)?),
            A::NoteFx { track, effect, key } => A::NoteFx {
                track: self.track(*track)?,
                effect: *self.notefx.get(effect)?,
                key: key.clone(),
            },
        })
    }

    fn modulators(&self, list: &mut [cantus_core::modulator::Modulator]) {
        for m in list {
            if let Some(id) = self.mods.get(&m.id) {
                m.id = *id;
            }
            m.routes = m
                .routes
                .iter()
                .filter_map(|r| {
                    let mut r = r.clone();
                    r.target = self.target(&r.target)?;
                    Some(r)
                })
                .collect();
        }
    }
}

/// The strip of `project` a stand-in named `name` lands on, by its name (its own or its
/// first track's; case and accents aside) — never `taken`, the blank strip the inserted
/// one replaces (review 3).
fn strip_named(project: &Project, name: &str, taken: Option<StripId>) -> Option<StripId> {
    let want = crate::picker_catalog::fold(name);
    if want.trim().is_empty() {
        return None;
    }
    project
        .strips
        .iter()
        .filter(|x| Some(x.id) != taken)
        .find(|x| {
            routable_name(project, x.id).is_some_and(|n| crate::picker_catalog::fold(&n) == want)
        })
        .map(|x| x.id)
}

/// Insert `template` (a track template's project) into `project` at track `index`: one
/// undo step, with ids from `next_id` up (the caller reserves them after). The template's
/// strip takes the first blank strip's place, or goes after the last strip of the desk's
/// middle; what it reached is found by name.
pub fn insert_track(
    project: &Project,
    template: &Project,
    index: usize,
    next_id: &mut u64,
) -> Result<Insert, String> {
    let mut fresh = || {
        let id = *next_id;
        *next_id += 1;
        id
    };
    let mut track = template
        .tracks
        .first()
        .cloned()
        .ok_or_else(|| t("workflow.error.not_a_track_template"))?;
    let own = track.strip.and_then(|id| template.strip(id)).cloned();
    let mut ids = Ids {
        track: (track.id, TrackId(fresh())),
        strip: own.as_ref().map(|s| (s.id, StripId(fresh()))),
        effects: HashMap::new(),
        mods: HashMap::new(),
        notefx: HashMap::new(),
        reached: HashMap::new(),
    };
    // The blank strip the new one takes the place of, chosen first: nothing is routed to it.
    let blank_at = own
        .as_ref()
        .and_then(|_| project.strips.iter().position(|x| blank(project, x)));
    let taken = blank_at.map(|i| project.strips[i].id);
    let mut missing = Vec::new();
    for s in &template.strips {
        if own.as_ref().is_some_and(|o| o.id == s.id) {
            continue;
        }
        let found = strip_named(project, &s.name, taken);
        if found.is_none() {
            missing.push(s.name.clone());
        }
        ids.reached.insert(s.id, found);
    }
    let mods: Vec<ModulatorId> = track
        .modulators
        .iter()
        .chain(own.iter().flat_map(|s| s.modulators.iter()))
        .map(|m| m.id)
        .collect();
    for m in mods {
        ids.mods.insert(m, ModulatorId(fresh()));
    }
    if let Some(s) = &own {
        for e in &s.effects {
            ids.effects.insert(e.id, EffectId(fresh()));
        }
    }
    // The note-effect chain (ADR 0069) with ids of its own: ids are the project's, one
    // owner each, so a template inserted twice gives two chains.
    let old: Vec<_> = track.note_fx.iter().map(|e| e.id).collect();
    cantus_core::notefx::renew_ids(&mut track.note_fx, &mut fresh);
    for (o, e) in old.into_iter().zip(&track.note_fx) {
        ids.notefx.insert(o, e.id);
    }
    track.id = ids.track.1;
    track.strip = ids.strip.map(|(_, new)| new);
    ids.modulators(&mut track.modulators);
    let mut commands = Vec::new();
    if let (Some(mut s), Some((_, new))) = (own, ids.strip) {
        s.id = new;
        for e in &mut s.effects {
            e.id = ids.effect(e.id).unwrap_or(e.id);
            e.sidechain = e.sidechain.and_then(|k| ids.strip(k));
            e.midi_from = e.midi_from.and_then(|k| ids.track(k));
        }
        s.sends.retain_mut(|x| match ids.strip(x.to) {
            Some(to) if !to.is_master() => {
                x.to = to;
                true
            }
            _ => false,
        });
        s.output = s
            .output
            .and_then(|o| ids.strip(o))
            .filter(|o| !o.is_master());
        ids.modulators(&mut s.modulators);
        match blank_at {
            Some(i) => {
                commands.push(Command::RemoveStrip {
                    id: project.strips[i].id,
                });
                commands.push(Command::InsertStrip {
                    index: i,
                    strip: Box::new(s),
                });
            }
            None => {
                let i = project
                    .strips
                    .iter()
                    .rposition(|x| x.dock == cantus_core::Dock::Middle)
                    .map_or(project.strips.len(), |i| i + 1);
                commands.push(Command::InsertStrip {
                    index: i,
                    strip: Box::new(s),
                });
            }
        }
    }
    let id = track.id;
    commands.push(Command::InsertTrack {
        index: index.min(project.tracks.len()),
        track: Box::new(track),
    });
    Ok(Insert {
        command: Command::Batch {
            label_key: "cmd.insert_track_template".into(),
            commands,
        },
        track: id,
        missing,
    })
}

/// The word a track template's instrument is named by in a list ("Gravis", "Pulsus II").
pub fn instrument_word(track: &Track) -> String {
    use crate::picker::PickChoice;
    let slug = match &track.instrument {
        InstrumentSpec::Firmus(_) => Some("firmus"),
        InstrumentSpec::Pulsus(_) => Some("pulsus"),
        InstrumentSpec::Native(n) => {
            crate::picker_catalog::slug_of(&PickChoice::Instrument(n.kind))
        }
        InstrumentSpec::External(e) => return e.name.clone(),
    };
    slug.map(|s| t(&format!("picker.dev.{s}.name")))
        .unwrap_or_default()
}

// ---- the starters -----------------------------------------------------------------------

/// A bar of 4/4, in ticks.
fn bars(n: i64) -> Ticks {
    Ticks::beats(4 * n)
}

/// A project to start from: named *Untitled*, `bpm`, looping its first `loop_bars`.
fn base(bpm: f64, loop_bars: i64) -> Project {
    let mut p = Project::new(t("app.untitled"));
    p.tempo_bpm = bpm;
    if loop_bars > 0 {
        p.loop_region = Some(cantus_core::LoopRegion {
            start: Ticks::ZERO,
            end: bars(loop_bars),
        });
        p.loop_enabled = true;
    }
    p
}

/// The *Beat* starter's reverb (the keys' chords): a dark tail.
pub const BEAT_REVERB: &str = "space.preset.dark_tail";
/// The *Band recording* starter's reverb (the vocal and the guitar): the vocal plate.
pub const BAND_REVERB: &str = "space.preset.vocal_plate";
/// The *Mastering* starter's master chain, in order, each with the factory preset chosen
/// for it (`None`: the effect as it comes — Speculum only measures).
pub const MASTERING_CHAIN: [(cantus_core::EffectKind, Option<&str>); 5] = [
    (cantus_core::EffectKind::Eq, Some("eq.preset.master")),
    (cantus_core::EffectKind::Multiband, Some("mb.preset.glue")),
    (
        cantus_core::EffectKind::Stereo,
        Some("preset.st.master_low"),
    ),
    (
        cantus_core::EffectKind::Maximizer,
        Some("preset.max.transparent"),
    ),
    (cantus_core::EffectKind::Analyzer, None),
];

/// `kind` with its factory preset `preset` (when named) set.
fn effect_with(
    p: &mut Project,
    kind: cantus_core::EffectKind,
    preset: Option<&str>,
) -> cantus_core::EffectSlot {
    let mut e = p.make_effect(kind);
    if let Some(name) = preset {
        let (_, values) = crate::views::fx::factory_presets(kind)
            .iter()
            .find(|(k, _)| *k == name)
            .unwrap_or_else(|| panic!("no factory preset {name} for {kind:?}"));
        for (k, v) in values.iter() {
            e.spec.set(k, *v);
        }
    }
    e
}

/// A return on the right of the desk: `name`, holding `kind` with `preset`, full wet as
/// every return Cantus makes (the sends set how much is heard).
fn add_return(p: &mut Project, name: &str, kind: cantus_core::EffectKind, preset: &str) -> StripId {
    let mut s = p.make_strip(name);
    s.dock = cantus_core::Dock::Right;
    let mut e = effect_with(p, kind, Some(preset));
    e.spec.set("mix", 1.0);
    s.effects.push(e);
    let id = s.id;
    p.strips.push(s);
    id
}

/// A send from `track`'s strip into `to`, at `level` (linear).
fn send(p: &mut Project, track: TrackId, to: StripId, level: f32) {
    let Some(strip) = p.track(track).and_then(|t| t.strip) else {
        return;
    };
    if let Some(s) = p.strip_mut(strip) {
        let mut x = cantus_core::Send::new(to);
        x.level = level;
        s.sends.push(x);
    }
}

/// An instrument track of `kind`, pushed.
fn instrument_track(p: &mut Project, name: &str, kind: InstrumentKind) -> TrackId {
    let mut tr = p.make_track(name);
    tr.instrument = InstrumentSpec::Native(cantus_core::inst::NativeSpec::new(kind));
    let id = tr.id;
    p.tracks.push(tr);
    id
}

/// An audio track recording mono input `channel` (0-based), pushed.
fn audio_track(p: &mut Project, name: &str, channel: u16, icon: cantus_core::TrackIcon) -> TrackId {
    let mut tr = p.make_track(name);
    tr.input = Some(cantus_core::TrackInput::mono(channel));
    tr.icon = Some(icon);
    let id = tr.id;
    p.tracks.push(tr);
    id
}

/// A clip of `bars` on `track` with `notes` (beat, length in beats, pitch, velocity).
fn clip(p: &mut Project, track: TrackId, name: &str, n_bars: i64, notes: &[(f64, f64, u8, u8)]) {
    let mut c = p.make_clip(name, Ticks::ZERO, bars(n_bars));
    for &(at, len, pitch, vel) in notes {
        let start = Ticks((at * Ticks::QUARTER.0 as f64).round() as i64);
        let length = Ticks((len * Ticks::QUARTER.0 as f64).round() as i64);
        let n = p.make_note(start, length, pitch, vel);
        c.notes.push(n);
    }
    if let Some(tr) = p.track_mut(track) {
        tr.clips.push(c);
    }
}

/// *Empty song*: one instrument track, nothing on it, 120 BPM.
fn empty_song() -> Project {
    let mut p = base(120.0, 0);
    let tr = p.make_track(t("workflow.starter.track"));
    p.tracks.push(tr);
    p
}

/// *Beat*: Pulsus II's studio kit playing two bars, Gravis on the root, an electric piano's
/// chords sent to a *Reverb* return (Space), 92 BPM, the four bars looping.
fn beat() -> Project {
    let mut p = base(92.0, 4);
    let mut drums = p.make_track(t("workflow.starter.drums"));
    drums.instrument = InstrumentSpec::Pulsus(cantus_core::PulsusParams::studio());
    drums.icon = Some(cantus_core::TrackIcon::Drums);
    let d = drums.id;
    p.tracks.push(drums);
    let mut hits: Vec<(f64, f64, u8, u8)> = Vec::new();
    for bar in 0..4 {
        let b = bar as f64 * 4.0;
        hits.extend([(b, 0.25, 36, 118), (b + 2.5, 0.25, 36, 100)]);
        hits.extend([(b + 1.0, 0.25, 38, 112), (b + 3.0, 0.25, 38, 114)]);
        for i in 0..8 {
            let vel = if i % 2 == 0 { 92 } else { 64 };
            hits.push((b + i as f64 * 0.5, 0.25, 42, vel));
        }
    }
    clip(&mut p, d, &t("workflow.starter.drums"), 4, &hits);
    let bass = instrument_track(&mut p, &t("workflow.starter.bass"), InstrumentKind::Bass);
    let roots = [36u8, 36, 41, 43];
    let line: Vec<(f64, f64, u8, u8)> = roots
        .iter()
        .enumerate()
        .flat_map(|(bar, &r)| {
            let b = bar as f64 * 4.0;
            [
                (b, 1.5, r, 104),
                (b + 2.5, 1.0, r, 92),
                (b + 3.5, 0.5, r + 12, 84),
            ]
        })
        .collect();
    clip(&mut p, bass, &t("workflow.starter.bass"), 4, &line);
    let keys = instrument_track(
        &mut p,
        &t("workflow.starter.keys"),
        InstrumentKind::ElectricPiano,
    );
    let chords: [[u8; 3]; 4] = [[60, 63, 67], [60, 63, 67], [65, 68, 72], [67, 70, 74]];
    let notes: Vec<(f64, f64, u8, u8)> = chords
        .iter()
        .enumerate()
        .flat_map(|(bar, c)| c.iter().map(move |&n| (bar as f64 * 4.0, 3.5, n, 80)))
        .collect();
    clip(&mut p, keys, &t("workflow.starter.keys"), 4, &notes);
    let reverb = add_return(
        &mut p,
        &t("workflow.starter.reverb"),
        cantus_core::EffectKind::Space,
        BEAT_REVERB,
    );
    send(&mut p, keys, reverb, 0.3);
    p
}

/// *Band recording*: a vocal, a guitar, a bass and a keyboard on audio tracks (inputs 1, 2,
/// 1 and 2), a *Reverb* return the vocal and the guitar are sent to, 120 BPM, metronome on.
fn band() -> Project {
    use cantus_core::TrackIcon;
    let mut p = base(120.0, 0);
    p.metronome = true;
    let vocal = audio_track(&mut p, &t("workflow.starter.vocal"), 0, TrackIcon::Vocal);
    let guitar = audio_track(&mut p, &t("workflow.starter.guitar"), 1, TrackIcon::Strings);
    audio_track(&mut p, &t("workflow.starter.bass"), 0, TrackIcon::Strings);
    audio_track(&mut p, &t("workflow.starter.keys"), 1, TrackIcon::Keys);
    let reverb = add_return(
        &mut p,
        &t("workflow.starter.reverb"),
        cantus_core::EffectKind::Space,
        BAND_REVERB,
    );
    send(&mut p, vocal, reverb, 0.25);
    send(&mut p, guitar, reverb, 0.2);
    p
}

/// *Mastering*: one track for the mix to be imported onto, and the master's chain — the
/// equaliser, the multiband compressor, the stereo tool, the maximiser and Speculum, each
/// with the preset [`MASTERING_CHAIN`] chose for it.
fn mastering() -> Project {
    let mut p = base(120.0, 0);
    let mut mix = p.make_track(t("workflow.starter.mix"));
    mix.icon = Some(cantus_core::TrackIcon::Fx);
    p.tracks.push(mix);
    for (kind, preset) in MASTERING_CHAIN {
        let e = effect_with(&mut p, kind, preset);
        p.master.effects.push(e);
    }
    p
}
