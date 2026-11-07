//! Hearing a preset before choosing it (round 37, ADR 0101): **nothing in the project
//! changes until a preset is applied**. An audition is an offline render — of the preset on
//! its role's phrase (an instrument, a kit), or of the effect at the preset on the selected
//! clip (an effect; a fixed source when nothing is selected) — made on a worker thread from
//! a *copy* of the project, then played by the browser's audition voice, after the master
//! as the sample browser's (`cantus_engine::preview`). The engine's playing snapshot, the
//! undo history and the saved file never see it.
//!
//! The same phrases are what the render check (`tests/preset_render.rs`) measures every
//! factory preset on and what the website's excerpts are rendered from.

use crate::demos::song::{colors, Feel, Song};
use crate::demos::theory::Ev;
use crate::preset_catalog::{Entry, Payload, Unit};
use crate::preset_tags::Role;
use cantus_core::{
    ClipId, EffectId, EffectKind, EffectSpec, InstrumentKind, InstrumentSpec, NativeSpec, Project,
    PulsusParams, StripId, Ticks, TrackIcon, TrackId,
};
use std::collections::BTreeMap;

/// The phrases' tempo.
pub const TEMPO: f64 = 120.0;
/// Their length in bars of 4/4 (four seconds at [`TEMPO`]).
pub const BARS: f64 = 2.0;
/// What is rendered after them, so a release or a tail is heard.
pub const TAIL: f64 = 1.5;
/// The longest stretch of a clip an effect is auditioned on, in seconds.
pub const CLIP_SECONDS: f64 = 8.0;

/// The phrase a role is heard on, in beats at [`TEMPO`]: two bars in C minor, written so
/// that each kind of part does what it does in a song — a bass line in the bass's
/// register, a melody, held chords, comping, an arpeggio, held notes for an effect patch,
/// a groove for drums.
pub fn phrase(role: Role) -> Vec<Ev> {
    let notes = |v: &[(f64, f64, u8, u8)]| -> Vec<Ev> {
        v.iter()
            .map(|&(at, len, pitch, vel)| Ev::new(at, len, pitch, vel))
            .collect()
    };
    let chords = |v: &[(f64, f64, &[u8], u8)]| -> Vec<Ev> {
        v.iter()
            .flat_map(|&(at, len, ps, vel)| ps.iter().map(move |&p| Ev::new(at, len, p, vel)))
            .collect()
    };
    match role {
        Role::Bass => notes(&[
            (0.0, 0.9, 36, 104),
            (1.0, 0.4, 36, 92),
            (1.5, 0.4, 39, 96),
            (2.0, 0.9, 41, 100),
            (3.0, 0.4, 43, 96),
            (3.5, 0.4, 46, 92),
            (4.0, 1.4, 36, 104),
            (5.5, 0.4, 34, 94),
            (6.0, 0.9, 31, 100),
            (7.0, 0.9, 34, 96),
        ]),
        Role::Lead => notes(&[
            (0.0, 0.45, 67, 100),
            (0.5, 0.45, 70, 96),
            (1.0, 0.95, 72, 104),
            (2.0, 0.45, 70, 96),
            (2.5, 0.45, 67, 94),
            (3.0, 0.45, 65, 96),
            (3.5, 0.45, 63, 92),
            (4.0, 1.45, 65, 100),
            (5.5, 0.45, 67, 96),
            (6.0, 1.9, 60, 100),
        ]),
        Role::Pad => chords(&[
            (0.0, 3.9, &[48, 51, 55, 58, 62], 90),
            (4.0, 3.9, &[44, 48, 51, 55, 58], 90),
        ]),
        Role::Keys => chords(&[
            (0.0, 1.4, &[48, 55, 58, 63], 96),
            (1.5, 0.4, &[48, 55, 58, 63], 84),
            (2.5, 1.4, &[48, 55, 58, 63], 92),
            (4.0, 1.4, &[44, 51, 55, 60], 96),
            (5.5, 0.4, &[44, 51, 55, 60], 84),
            (6.5, 1.4, &[43, 50, 55, 59], 92),
        ]),
        Role::Pluck => {
            let up = [60u8, 63, 67, 70, 72, 70, 67, 63];
            let down = [56u8, 60, 63, 67, 68, 67, 63, 60];
            (0..32)
                .map(|i| {
                    let p = if i < 16 { up[i % 8] } else { down[i % 8] };
                    let vel = if i % 4 == 0 { 104 } else { 88 };
                    Ev::new(i as f64 * 0.25, 0.22, p, vel)
                })
                .collect()
        }
        Role::Fx => notes(&[(0.0, 3.5, 60, 100), (4.0, 3.5, 67, 100)]),
        // A groove on the General MIDI drum map (kick, snare, closed and open hat), which
        // a kit plays as drums and a drum-like synth patch as pitches.
        Role::Drums | Role::Vocal | Role::Bus | Role::Master | Role::Creative => {
            let mut evs = Vec::new();
            for bar in 0..2 {
                let b = f64::from(bar) * 4.0;
                for at in [0.0, 1.5, 2.0, 2.75] {
                    evs.push(Ev::new(b + at, 0.2, 36, 104));
                }
                evs.push(Ev::new(b + 1.0, 0.2, 38, 100));
                evs.push(Ev::new(b + 3.0, 0.2, 38, 100));
                for i in 0..8 {
                    if i != 7 {
                        let vel = if i % 2 == 0 { 92 } else { 76 };
                        evs.push(Ev::new(b + f64::from(i) * 0.5, 0.1, 42, vel));
                    }
                }
                evs.push(Ev::new(b + 3.5, 0.4, 46, 88));
            }
            evs
        }
    }
}

/// The drums phrase on a kit's own pads: each General MIDI voice the groove plays (kick,
/// snare, closed and open hat) goes to the kit's pad of that note, else to the next pad of the
/// kit the groove does not use yet — so a kit laid out its own way (congas, bongos…) is heard
/// whole, not as silence.
pub fn kit_phrase(kit: &PulsusParams) -> Vec<Ev> {
    let have: Vec<u8> = kit.pads.iter().map(|p| p.pitch).collect();
    let mut used: Vec<u8> = Vec::new();
    let mut map = std::collections::HashMap::new();
    for (gm, wanted) in [
        (36u8, &[36u8, 35][..]),
        (38, &[38, 40, 39, 37]),
        (42, &[42, 44]),
        (46, &[46]),
    ] {
        let pad = wanted
            .iter()
            .copied()
            .find(|w| have.contains(w) && !used.contains(w))
            .or_else(|| have.iter().copied().find(|h| !used.contains(h)));
        if let Some(p) = pad {
            used.push(p);
            map.insert(gm, p);
        }
    }
    phrase(Role::Drums)
        .into_iter()
        .filter_map(|e| map.get(&e.pitch).map(|&p| Ev::new(e.at, e.len, p, e.vel)))
        .collect()
}

/// A one-part project: `spec` playing its role's phrase at [`TEMPO`], at unity, nothing on
/// the master — what an instrument's preset is auditioned and measured on.
pub fn phrase_project(spec: InstrumentSpec, role: Role) -> Project {
    phrase_project_at(spec, role, TEMPO)
}

/// [`phrase_project`] at `bpm` (the browser's *Sync BPM*: the project's tempo).
pub fn phrase_project_at(spec: InstrumentSpec, role: Role, bpm: f64) -> Project {
    let spec_kit = match &spec {
        InstrumentSpec::Pulsus(k) => Some(k.clone()),
        _ => None,
    };
    let mut s = Song::new("audition", bpm.clamp(40.0, 300.0), 1);
    let part = s.part("preset", spec, colors::KEYS, TrackIcon::Keys);
    s.feel(part, Feel::TIGHT);
    let evs = match (&spec_kit, role) {
        (Some(kit), Role::Drums) => kit_phrase(kit),
        _ => phrase(role),
    };
    s.clip(part, "phrase", 0.0, BARS, &evs);
    s.finish()
}

/// Each part of a fixed source sits this far under unity, dB: the band at about −23 LUFS
/// with its peaks near −12 dBFS, the headroom a mix bus leaves an insert (at unity the
/// band peaked at −3.5 dBFS and a phaser's or a transient shaper's preset could not be
/// told from the source's own clipping).
pub const SOURCE_TRIM: f32 = -8.0;

/// The fixed source an effect is heard on when no clip is chosen, for its role: a drum
/// groove (drums), a lead line standing in for a voice (vocal), or a small band — drums,
/// bass and keys (a bus, the master, a creative effect). Made by Cantus's own instruments.
pub fn source_project(role: Role) -> Project {
    source_project_at(role, TEMPO)
}

/// [`source_project`] at `bpm`.
pub fn source_project_at(role: Role, bpm: f64) -> Project {
    let mut s = Song::new("source", bpm.clamp(40.0, 300.0), 1);
    let drums = |s: &mut Song| {
        let p = s.part(
            "drums",
            InstrumentSpec::Pulsus(PulsusParams::default()),
            colors::KICK,
            TrackIcon::Drums,
        );
        s.feel(p, Feel::TIGHT);
        s.clip(p, "groove", 0.0, BARS, &phrase(Role::Drums));
        s.mix(p.strip, SOURCE_TRIM, 0.0);
    };
    let line =
        |s: &mut Song, kind: InstrumentKind, role: Role, name: &str, icon: TrackIcon, pan: f32| {
            let p = s.part(
                name,
                InstrumentSpec::Native(NativeSpec::new(kind)),
                colors::BASS,
                icon,
            );
            s.feel(p, Feel::TIGHT);
            s.clip(p, name, 0.0, BARS, &phrase(role));
            s.mix(p.strip, SOURCE_TRIM, pan);
        };
    match role {
        Role::Drums => drums(&mut s),
        Role::Vocal => line(
            &mut s,
            InstrumentKind::Analog,
            Role::Lead,
            "lead",
            TrackIcon::Keys,
            0.0,
        ),
        _ => {
            drums(&mut s);
            line(
                &mut s,
                InstrumentKind::Bass,
                Role::Bass,
                "bass",
                TrackIcon::Synth,
                0.0,
            );
            line(
                &mut s,
                InstrumentKind::ElectricPiano,
                Role::Keys,
                "keys",
                TrackIcon::Keys,
                0.35,
            );
        }
    }
    s.finish()
}

/// An effect's values after a preset is loaded into `spec`, as
/// `CantusApp::apply_fx_params` leaves them: the keys it names set, every other key at its
/// default (cleared, for a sparse kind).
pub fn effect_params_after(
    spec: &EffectSpec,
    preset: &BTreeMap<String, f32>,
) -> BTreeMap<String, f32> {
    let mut out = BTreeMap::new();
    for info in spec.kind.params() {
        match preset.get(info.key) {
            Some(v) => {
                out.insert(info.key.to_string(), *v);
            }
            None if !spec.kind.sparse() => {
                out.insert(info.key.to_string(), info.default);
            }
            None => {}
        }
    }
    out
}

/// Where a preset would be loaded: the instrument (or kit) on a track, or an insert. A
/// browser opened from nowhere in particular has no target and auditions on the phrases.
/// Where a preset would be loaded: the instrument (or kit) on a track, or an insert. A
/// browser opened from nowhere in particular has no target and auditions on the phrases.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Target {
    Track(TrackId),
    Effect {
        strip: StripId,
        effect: EffectId,
    },
    #[default]
    None,
}

/// What an audition is heard on.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum HeardOn {
    /// Its role's phrase at this tempo.
    Phrase(Role, f64),
    /// The clip selected in the user's project.
    SelectedClip,
    /// The first clip of a track this channel hears.
    FirstClip,
    /// Cantus's fixed source for this role.
    Source(Role),
}

/// What an audition renders: a project made for it and the stretch to render.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub project: Project,
    pub start: Ticks,
    pub end: Ticks,
    pub on: HeardOn,
    /// It plays files (audio clips, sampled zones): the engine compiles it, with what its
    /// sample cache holds, never waiting. Else a bare `Snapshot::compile` on the worker.
    pub needs_engine: bool,
    /// What is heard instead when this renders silent (a clip whose audio is not loaded).
    pub fallback: Option<Box<Prepared>>,
    /// A slicing style on the target's sliced loop (r37:piano, ADR 0099): the instrument as
    /// it is and the style — the worker re-cuts the user's own loop the style's way (it reads
    /// the file) and auditions that, what *Load* gives, never the style's built-in sounds.
    pub restyle: Option<(NativeSpec, NativeSpec, Role, f64, f64)>,
}

/// What a restyled audition plays: the preset through the editor's own path on the
/// target's instrument (on the worker: it reads the loop). r37:piano
pub fn restyled(prepared: &Prepared) -> Option<NativeSpec> {
    let (now, style, _, _, song) = prepared.restyle.as_ref()?;
    Some(crate::views::inst::preset_target(now, style, *song))
}

/// The project and stretch an audition of `entry` renders. `project` is the user's, read
/// only; `clip` the clip selected in it; `bpm` the phrases' tempo ([`TEMPO`], or the
/// project's with *Sync BPM*).
pub fn prepare(
    project: &Project,
    target: Target,
    entry: &Entry,
    clip: Option<ClipId>,
    bpm: f64,
) -> Prepared {
    let phrase_end = Ticks::from_beats_f64(BARS * 4.0);
    let whole = |p: Project, on: HeardOn, needs_engine: bool| Prepared {
        project: p,
        start: Ticks::ZERO,
        end: phrase_end,
        on,
        needs_engine,
        fallback: None,
        restyle: None,
    };
    let phrase = HeardOn::Phrase(entry.role(), bpm);
    match &entry.payload {
        Payload::Native(spec) => {
            let files = spec
                .text
                .iter()
                .any(|(k, v)| spec.kind.is_asset_key(k) && !v.is_empty());
            let p = phrase_project_at(InstrumentSpec::Native(spec.clone()), entry.role(), bpm);
            let mut w = whole(p, phrase, files);
            // r37:piano — a slicing style on a sliced loop auditions that loop, cut its way.
            if let Target::Track(t) = target {
                let now = project
                    .track(t)
                    .and_then(|x| x.instrument.native())
                    .cloned();
                if let Some(now) = now.filter(|n| cantus_core::sectio::Slicing::of(n).is_some()) {
                    if crate::sectio_ui::styles::style_of(spec).is_some() && spec.get("sl.n") < 1.0
                    {
                        w.restyle = Some((now, spec.clone(), entry.role(), bpm, project.tempo_bpm));
                        w.needs_engine = false;
                    }
                }
            }
            w
        }
        Payload::Kit(kit) => {
            let p = phrase_project_at(InstrumentSpec::Pulsus(kit.clone()), Role::Drums, bpm);
            whole(p, phrase, true)
        }
        Payload::Effect(values) => {
            let source = |kind: Option<EffectKind>| {
                let mut p = source_project_at(entry.role(), bpm);
                if let Some(kind) = kind {
                    let mut slot = p.make_effect(kind);
                    slot.spec.params = effect_params_after(&slot.spec, values);
                    p.master.effects.push(slot);
                }
                whole(p, HeardOn::Source(entry.role()), false)
            };
            let Unit::Effect(kind) = entry.unit else {
                return source(None);
            };
            if let Target::Effect { strip, effect } = target {
                if let Some(mut p) = on_the_users_clip(project, strip, effect, values, clip) {
                    p.fallback = Some(Box::new(source(Some(kind))));
                    return p;
                }
            }
            source(Some(kind))
        }
    }
}

/// Where a track's sound leaves it: its strip (the master when it has none) and, for a
/// split kit, each pad's own strip (ADR 0100).
fn sources_of(t: &cantus_core::Track) -> Vec<StripId> {
    let mut out = vec![t.strip.unwrap_or(StripId::MASTER)];
    if let Some(k) = t.instrument.pulsus() {
        out.extend(k.pads.iter().filter_map(|p| p.output));
    }
    out.dedup();
    out
}

/// Whether an insert on `strip` hears `t`: from its strip or a pad's, through buses and
/// sends (what `Project::tracks_on_strip` holds directly, followed downstream).
pub fn heard_through(p: &Project, t: &cantus_core::Track, strip: StripId) -> bool {
    p.tracks_on_strip(strip).any(|x| x.id == t.id)
        || sources_of(t)
            .into_iter()
            .any(|from| reaches(p, from, strip))
}

/// Whether sound leaving strip `from` reaches strip `to` (its output and sends, followed;
/// everything reaches the master).
pub fn reaches(p: &Project, from: StripId, to: StripId) -> bool {
    if to.is_master() || from == to {
        return true;
    }
    let mut seen = vec![from];
    let mut stack = vec![from];
    while let Some(s) = stack.pop() {
        for next in p
            .strip(s)
            .map(|x| x.feeds().collect::<Vec<_>>())
            .unwrap_or_default()
        {
            if next == to {
                return true;
            }
            if !seen.contains(&next) {
                seen.push(next);
                stack.push(next);
            }
        }
    }
    false
}

/// The user's clip an insert on `strip` is auditioned on: the selected clip when its track
/// is heard through that strip, else the first clip of a track that is. The copy keeps that
/// track and that clip alone, unmuted, with the insert at `values` (not bypassed) — **cut
/// down** so its compile reads one track's files. `None` — the fixed source then — when no
/// track is heard there, or the path holds what an offline render cannot play (a hosted
/// plug-in, an external unit, an ARA clip).
fn on_the_users_clip(
    project: &Project,
    strip: StripId,
    effect: EffectId,
    values: &BTreeMap<String, f32>,
    clip: Option<ClipId>,
) -> Option<Prepared> {
    project.effect(strip, effect)?;
    let heard = |t: &cantus_core::Track| heard_through(project, t, strip);
    let selected = clip.and_then(|id| {
        project
            .tracks
            .iter()
            .filter(|t| heard(t))
            .find_map(|t| t.clips.iter().find(|c| c.id == id).map(|c| (t.id, c.id)))
    });
    let (track, clip_id, on) = match selected {
        Some((t, c)) => (t, c, HeardOn::SelectedClip),
        None => {
            let (t, c) = project
                .tracks
                .iter()
                .filter(|t| heard(t))
                .find_map(|t| t.clips.first().map(|c| (t.id, c.id)))?;
            (t, c, HeardOn::FirstClip)
        }
    };
    let t = project.track(track)?;
    let c = t.clips.iter().find(|c| c.id == clip_id)?;
    // What an offline render cannot play: the fixed source instead.
    if matches!(t.instrument, InstrumentSpec::External(_))
        || c.audio.as_ref().is_some_and(|a| a.ara.is_some())
    {
        return None;
    }
    let sources = sources_of(t);
    let path: Vec<StripId> = sources
        .iter()
        .copied()
        .chain(project.strips.iter().map(|s| s.id))
        .filter(|s| sources.iter().any(|from| reaches(project, *from, *s)))
        .filter(|s| s.is_master() || reaches(project, *s, strip))
        .collect();
    let hosted = |effects: &[cantus_core::EffectSlot]| {
        effects.iter().any(|e| e.spec.kind == EffectKind::External)
    };
    if path.iter().any(|s| match project.strip(*s) {
        Some(x) => hosted(&x.effects),
        None => s.is_master() && hosted(&project.master.effects),
    }) {
        return None;
    }
    let mut p = project.clone();
    p.tracks.retain(|x| x.id == track);
    p.session = Default::default();
    for x in &mut p.tracks {
        x.muted = false;
        x.soloed = false;
        // A frozen track plays its file: the audition is of the preset, live.
        x.frozen = None;
        x.clips.retain(|k| k.id == clip_id);
        for k in &mut x.clips {
            k.muted = false;
        }
    }
    for s in &mut p.strips {
        s.soloed = false;
        s.frozen = None;
        if path.contains(&s.id) {
            s.muted = false;
            s.fx_enabled = true;
        }
    }
    p.master.fx_enabled = true;
    let slot = p.effect_mut(strip, effect)?;
    slot.spec.params = effect_params_after(&slot.spec, values);
    slot.bypass = false;
    let (start, length) = (c.start, c.length);
    let most = Ticks::from_seconds(CLIP_SECONDS, p.tempo_bpm);
    let len = if length > most { most } else { length };
    Some(Prepared {
        project: p,
        start,
        end: start + len,
        on,
        needs_engine: true,
        fallback: None,
        restyle: None,
    })
}

// ---- the audition in the application ------------------------------------------------------

/// What the browser is hearing.
#[derive(Default)]
pub struct Audition {
    /// The preset being rendered or heard (its entry's id).
    pub heard: Option<String>,
    /// Until when the audition voice plays it.
    pub until: Option<std::time::Instant>,
    /// Why the last audition could not be heard.
    pub failed: Option<String>,
    /// Auditions rendered since the start (for tests).
    pub renders: u32,
    /// The last render handed to the audition voice (for tests).
    pub last: Option<std::sync::Arc<cantus_core::SampleData>>,
    /// Each heard preset's outline, [`MINI`] columns (the rows' mini waveforms), by id.
    pub peaks: std::collections::HashMap<String, Vec<f32>>,
    /// The last one's outline, [`WIDE`] columns, for the player at the foot.
    pub wave: Vec<f32>,
    /// What the one heard was heard on.
    pub on: Option<HeardOn>,
    /// The preset `on` is about (the last one rendered).
    pub last_id: Option<String>,
    job: Option<Job>,
}

struct Job {
    id: String,
    rx: std::sync::mpsc::Receiver<Result<(cantus_core::SampleData, HeardOn), String>>,
    /// Shared with the worker: a stop or a new audition cancels the render at its next block.
    progress: std::sync::Arc<cantus_engine::RenderProgress>,
}

impl std::fmt::Debug for Audition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Audition")
            .field("heard", &self.heard)
            .field("rendering", &self.job.is_some())
            .finish()
    }
}

impl Audition {
    /// Whether a render is under way.
    pub fn rendering(&self) -> bool {
        self.job.is_some()
    }

    /// Whether `id` is being rendered or heard now.
    pub fn is_on(&self, id: &str) -> bool {
        self.heard.as_deref() == Some(id)
            && (self.job.is_some() || self.until.is_some_and(|u| std::time::Instant::now() < u))
    }

    /// The render under way, cancelled (it stops at its next block; its result is dropped).
    fn cancel(&mut self) {
        if let Some(job) = self.job.take() {
            job.progress.cancel();
        }
    }

    /// The progress of the render under way (tests watch it being cancelled).
    pub fn progress(&self) -> Option<std::sync::Arc<cantus_engine::RenderProgress>> {
        self.job.as_ref().map(|j| j.progress.clone())
    }

    /// r37:fixes — an audition rendered elsewhere (an agent's job, off the interface's
    /// thread): from now `id` is the one being rendered, as if the browser had asked; what
    /// is sent on the channel returned is played and kept as the browser's own (heard, until
    /// when, its outline), so Stop, Esc and the next audition stop it. `progress` is the
    /// render's: a stop cancels it.
    pub(crate) fn begin_elsewhere(
        &mut self,
        id: &str,
        progress: std::sync::Arc<cantus_engine::RenderProgress>,
    ) -> std::sync::mpsc::Sender<Result<(cantus_core::SampleData, HeardOn), String>> {
        self.cancel();
        let (tx, rx) = std::sync::mpsc::channel();
        self.failed = None;
        self.until = None;
        self.heard = Some(id.to_string());
        self.job = Some(Job {
            id: id.to_string(),
            rx,
            progress,
        });
        tx
    }
}

/// Render a prepared audition (what the worker does); `progress` cancels it.
pub fn render_prepared(
    snap: std::sync::Arc<cantus_core::Snapshot>,
    p: &Prepared,
    rate: u32,
    progress: &cantus_engine::RenderProgress,
) -> Result<cantus_core::SampleData, String> {
    let r = cantus_engine::render_with_progress(
        snap,
        &cantus_engine::RenderOptions {
            sample_rate: rate,
            channels: 2,
            start: p.start,
            end: Some(p.end),
            tail_seconds: TAIL,
            compensate_latency: true,
            ..Default::default()
        },
        Vec::new(),
        Vec::new(),
        progress,
    )
    .map_err(|e| e.to_string())?;
    Ok(cantus_core::SampleData {
        rate,
        channels: 2,
        frames: r.samples.into(),
        stream: None,
    })
}

/// Under this peak a render is taken as silence (a file not loaded yet): −60 dBFS.
const SILENT: f32 = 1e-3;

/// What the worker does: the prepared render, else — silent — its fallback.
pub(crate) fn work(
    first: Option<std::sync::Arc<cantus_core::Snapshot>>,
    prepared: &Prepared,
    rate: u32,
    progress: &cantus_engine::RenderProgress,
) -> Result<(cantus_core::SampleData, HeardOn), String> {
    // r37:piano — a style re-cuts the user's loop here, on the worker, the files read here.
    let restyled_project = prepared.restyle.as_ref().and_then(|(_, _, role, bpm, _)| {
        let spec = restyled(prepared)?;
        Some(phrase_project_at(InstrumentSpec::Native(spec), *role, *bpm))
    });
    let snap = match &restyled_project {
        Some(p) => {
            let mut snap = cantus_core::Snapshot::compile(p, 0);
            let _ = cantus_engine::SampleCache::new().attach(&mut snap);
            std::sync::Arc::new(snap)
        }
        None => first.unwrap_or_else(|| {
            std::sync::Arc::new(cantus_core::Snapshot::compile(&prepared.project, 0))
        }),
    };
    let heard = render_prepared(snap, prepared, rate, progress)?;
    let loud = heard.frames.iter().any(|x| x.abs() > SILENT);
    match &prepared.fallback {
        Some(fb) if !loud => {
            let snap = std::sync::Arc::new(cantus_core::Snapshot::compile(&fb.project, 0));
            Ok((render_prepared(snap, fb, rate, progress)?, fb.on))
        }
        _ => Ok((heard, prepared.on)),
    }
}

impl crate::app::CantusApp {
    /// Audition `entry` for `target`: render it on a worker and play it when it is ready
    /// (see [`Self::poll_preset_audition`]). A second call on the one playing stops it. The
    /// interface never waits: a project with files is compiled from what the sample cache
    /// holds (a file not loaded yet plays silent, and the fixed source is heard instead);
    /// every other is compiled on the worker.
    pub fn audition_preset(&mut self, target: Target, entry: &Entry) {
        if self.presets.audition.is_on(&entry.id) {
            self.stop_preset_audition();
            return;
        }
        self.stop_preset_audition();
        if !entry.unit.audible() {
            return;
        }
        let bpm = if self.presets.sync_bpm {
            self.project().tempo_bpm
        } else {
            TEMPO
        };
        let prepared = prepare(self.project(), target, entry, self.selected_clip, bpm);
        let first = prepared
            .needs_engine
            .then(|| self.engine.compile_for_audition(&prepared.project, 0));
        let rate = self.engine.sample_rate.round().max(1.0) as u32;
        let progress = std::sync::Arc::new(cantus_engine::RenderProgress::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let shared = progress.clone();
        let spawned = std::thread::Builder::new()
            .name("preset-audition".into())
            .spawn(move || {
                let _ = tx.send(work(first, &prepared, rate, &shared));
            });
        let a = &mut self.presets.audition;
        a.failed = None;
        match spawned {
            Ok(_) => {
                a.heard = Some(entry.id.clone());
                a.job = Some(Job {
                    id: entry.id.clone(),
                    rx,
                    progress,
                });
            }
            Err(e) => a.failed = Some(e.to_string()),
        }
    }

    /// Once a frame while the browser is open: hand a finished render to the audition
    /// voice, and forget one that has played out. Returns whether anything is under way
    /// (so the view asks for another frame).
    pub fn poll_preset_audition(&mut self) -> bool {
        let a = &mut self.presets.audition;
        if let Some(job) = &a.job {
            match job.rx.try_recv() {
                Ok(result) => {
                    let id = job.id.clone();
                    a.job = None;
                    match result {
                        Ok((sample, on)) => {
                            let secs = sample.len_frames() as f64 / f64::from(sample.rate.max(1));
                            let sample = std::sync::Arc::new(sample);
                            a.renders += 1;
                            a.peaks.insert(id.clone(), peaks_of(&sample, MINI));
                            a.wave = peaks_of(&sample, WIDE);
                            a.last = Some(sample.clone());
                            a.on = Some(on);
                            a.last_id = Some(id.clone());
                            a.heard = Some(id);
                            a.until = Some(
                                std::time::Instant::now()
                                    + std::time::Duration::from_secs_f64(secs),
                            );
                            self.engine.preview(Some((sample, 1.0)));
                        }
                        Err(e) => {
                            a.failed = Some(e);
                            a.heard = None;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    a.job = None;
                    a.heard = None;
                }
            }
        }
        match a.until {
            Some(u) if std::time::Instant::now() >= u => {
                a.until = None;
                a.heard = None;
                false
            }
            Some(_) => true,
            None => a.job.is_some(),
        }
    }

    /// Wait for the audition being rendered (tests and tools; the interface polls).
    pub fn finish_preset_audition(&mut self, patience: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        while self.presets.audition.rendering() && start.elapsed() < patience {
            self.poll_preset_audition();
            if self.presets.audition.rendering() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        !self.presets.audition.rendering()
    }

    /// Stop the audition: a fade in the engine, and a render under way cancelled.
    pub fn stop_preset_audition(&mut self) {
        let a = &mut self.presets.audition;
        let was = a.until.is_some();
        a.cancel();
        a.heard = None;
        a.until = None;
        if was {
            self.engine.preview(None);
        }
    }
}

/// Columns of a row's mini waveform.
pub const MINI: usize = 48;
/// Columns of the player's waveform.
pub const WIDE: usize = 400;

/// The outline of a render: the largest magnitude of either channel in each of `bins`
/// equal stretches, 0…1.
pub fn peaks_of(s: &cantus_core::SampleData, bins: usize) -> Vec<f32> {
    let ch = usize::from(s.channels.max(1));
    let frames = s.frames.len() / ch;
    if frames == 0 || bins == 0 {
        return vec![0.0; bins];
    }
    (0..bins)
        .map(|b| {
            let from = b * frames / bins;
            let to = ((b + 1) * frames / bins).max(from + 1).min(frames);
            s.frames[from * ch..to * ch]
                .iter()
                .fold(0f32, |m, x| m.max(x.abs()))
                .min(1.0)
        })
        .collect()
}

/// The headroom test's material (`tests/instrument_levels.rs`, round 19): two bars of
/// sixteenths over two octaves under four-note chords on every beat, at velocity 127 — the
/// most a part asks of an instrument at once. `low` is the line's lowest note (a bass plays
/// it from E1, everything else from C2). No factory preset may pass full scale on it.
pub fn dense_riff(low: u8) -> Vec<Ev> {
    let line = [0, 0, 12, 0, 7, 0, 10, 12, 0, 0, 12, 3, 5, 7, 10, 12];
    let mut evs = Vec::new();
    for bar in 0..2 {
        for (i, step) in line.iter().enumerate() {
            let at = f64::from(bar) * 4.0 + i as f64 * 0.25;
            evs.push(Ev::new(at, 0.2, low + step, 127));
        }
        for beat in 0..4 {
            let at = f64::from(bar) * 4.0 + f64::from(beat);
            for n in [24, 28, 31, 36] {
                evs.push(Ev::new(at, 0.9, low + n, 127));
            }
        }
    }
    evs
}

/// How long [`riff_project`] plays, bars.
pub const RIFF_BARS: f64 = 3.0;

/// The loud chord of Unda's headroom test (`tests/inst_wavetable.rs`): eight notes over four
/// octaves at velocity 127, held three beats — what a pad or a stack asks at its loudest.
pub const LOUD_CHORD: [u8; 8] = [36, 48, 55, 60, 64, 67, 72, 79];

/// An instrument preset on [`dense_riff`] at 120 BPM (a bass from E1), then — for Unda, whose
/// headroom test plays it — [`LOUD_CHORD`] in a third bar, so a level fitted under this
/// headroom passes both tests. (Not for every unit: a noise or an effect patch is never
/// played as an eight-note chord at full velocity, and the chord would fit it 9 dB down.)
pub fn riff_project(spec: &NativeSpec) -> Project {
    let low = if spec.kind == InstrumentKind::Bass {
        28
    } else {
        36
    };
    let mut s = Song::new("riff", TEMPO, 1);
    let part = s.part(
        "riff",
        InstrumentSpec::Native(spec.clone()),
        colors::KEYS,
        TrackIcon::Keys,
    );
    s.feel(part, Feel::TIGHT);
    let mut evs = dense_riff(low);
    if spec.kind == InstrumentKind::Wavetable {
        let at = BARS * 4.0;
        evs.extend(LOUD_CHORD.iter().map(|&n| Ev::new(at, 3.0, n, 127)));
    }
    s.clip(part, "riff", 0.0, RIFF_BARS, &evs);
    s.finish()
}
