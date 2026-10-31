//! **Convert to MIDI…** (round 34, ADR 0086): an audio clip into notes on a new
//! instrument track — melody, polyphony (Basic Pitch) or drums (Onsets and Frames Drums
//! and Cantus's own detector) — as **one undo step**, and the `audio_to_midi` agent tool's
//! shared path.
//!
//! The path, for the dialog and the agent alike:
//! Melody mode is Cantus's own pitch tracker (the pitch editor's, ADR 0054), which
//! measured better than Basic Pitch's notes on a single line (ADR 0086 §2): it runs in
//! process on a worker ([`melody`]) and needs no model.
//!
//! 1. [`source`] (interface thread): what the clip plays — its file, the frames of one
//!    period of its window, how source frames map to the timeline (tempo, warp, slip,
//!    reverse: the pitch editor's [`ClipTime`]).
//! 2. [`write_input`] (a worker): those frames mixed to mono and converted to the model's
//!    rate (22 050 Hz for Basic Pitch, 44 100 Hz for drums), written as a WAV in the
//!    cache.
//! 3. The helper's `transcribe` job ([`job_args`]), started through the one ML service.
//! 4. [`plan`] (interface thread): the notes placed on the timeline through the same
//!    mapping, repeated over a looped clip's periods, optionally quantised, into a new
//!    track under the source with a MIDI clip aligned to the audio clip — `InsertTrack` and
//!    `InsertClip` in one `Batch`.

#[path = "transcribe_read.rs"]
pub mod read;
#[path = "transcribe_run.rs"]
pub mod run;
#[path = "transcribe_view.rs"]
pub mod view;
pub use read::{read_mono_range, write_input};

use crate::app::CantusApp;
use crate::pitch::ClipTime;
use cantus_core::inst::{InstrumentKind, NativeSpec};
use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
use cantus_core::{ClipId, Command, CommandBus, InstrumentSpec, PulsusParams, Ticks, TrackId};
use cantus_ml::spec::{Device, RuntimeSpec};
use cantus_ml::transcribe::spec::{
    DrumClass, DrumSettings, Mode, NoteFamily, NoteModel, NoteSettings, TranscribeArgs,
    Transcription,
};
use std::path::{Path, PathBuf};

/// The note model (Basic Pitch) and the drum model (Onsets and Frames Drums) in the
/// content manager's list.
pub const NOTE_MODEL: &str = "basic-pitch";
pub const DRUM_MODEL: &str = "oaf-drums";
/// Stems' 4-stem separator, used to take the drums out of a mix first.
pub const STEMS_MODEL: &str = "htdemucs";

/// The instrument the notes go to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Instrument {
    /// Unda, the wavetable synth: a clear voice for any line.
    #[default]
    Unda,
    /// Exemplar, the sampler.
    Exemplar,
}

/// A grid to quantise the notes' starts to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grid {
    #[default]
    Off,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl Grid {
    pub const ALL: [Grid; 4] = [Grid::Off, Grid::Eighth, Grid::Sixteenth, Grid::ThirtySecond];

    pub fn ticks(self) -> Option<Ticks> {
        match self {
            Grid::Off => None,
            Grid::Eighth => Some(Ticks(Ticks::QUARTER.0 / 2)),
            Grid::Sixteenth => Some(Ticks(Ticks::QUARTER.0 / 4)),
            Grid::ThirtySecond => Some(Ticks(Ticks::QUARTER.0 / 8)),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Grid::Off => "off",
            Grid::Eighth => "1/8",
            Grid::Sixteenth => "1/16",
            Grid::ThirtySecond => "1/32",
        }
    }

    pub fn parse(s: &str) -> Option<Grid> {
        Grid::ALL.into_iter().find(|g| g.name() == s)
    }
}

/// What the user (or the agent) chose.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub mode: Mode,
    /// 0 (only the clearest notes) … 1 (the faintest); 0.5 is each model's own default.
    pub sensitivity: f32,
    /// The shortest note kept, milliseconds (Basic Pitch's 127.7 by default).
    pub shortest_ms: f32,
    /// Each note's pitch bends, as per-note bend curves (MPE-style, ADR 0080).
    pub bends: bool,
    /// Off by default: the notes stay where they were played.
    pub quantize: Grid,
    pub instrument: Instrument,
    /// The drum classes written (kick, snare, toms, hi-hat, cymbals).
    pub classes: [bool; 5],
    /// Drums: separate the drums from the mix first (HT-Demucs, when installed).
    pub separate: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::Polyphonic,
            sensitivity: 0.5,
            shortest_ms: 127.7,
            bends: false,
            quantize: Grid::Off,
            instrument: Instrument::Unda,
            classes: default_classes(false),
            separate: false,
        }
    }
}

/// The classes written by default: kick, snare and hi-hat always; toms and cymbals only
/// with the drum model (without it they are too often wrong: ADR 0086 §3).
pub fn default_classes(with_model: bool) -> [bool; 5] {
    [true, true, with_model, true, with_model]
}

/// The class's index in [`Settings::classes`].
pub fn class_index(c: DrumClass) -> usize {
    match c {
        DrumClass::Kick => 0,
        DrumClass::Snare => 1,
        DrumClass::Toms => 2,
        DrumClass::HiHat => 3,
        DrumClass::Cymbals => 4,
    }
}

impl Settings {
    /// Basic Pitch's thresholds for the sensitivity: its own 0.5 and 0.3 at 0.5.
    pub fn notes(&self) -> NoteSettings {
        let s = self.sensitivity.clamp(0.0, 1.0) as f64;
        NoteSettings {
            onset_threshold: 0.5 + (0.5 - s) * 0.6,
            frame_threshold: 0.3 + (0.5 - s) * 0.3,
            min_note_ms: self.shortest_ms.clamp(10.0, 2_000.0) as f64,
            bends: self.bends,
            ..NoteSettings::default()
        }
    }
}

/// What a conversion takes from the project, on the interface thread.
#[derive(Clone, Debug)]
pub struct Source {
    pub track: TrackId,
    pub clip: ClipId,
    pub clip_name: String,
    pub track_name: String,
    /// The audio file, resolved.
    pub path: PathBuf,
    /// Its rate.
    pub rate: u32,
    /// The source frames the clip plays, `a .. b` (every piece of a loop together).
    pub frames: (u64, u64),
    /// The pieces the engine plays: the whole clip, or one per period of a loop (the last
    /// perhaps shorter), each with its own mapping — a reversed piece is mirrored in its
    /// own window, as `Snapshot::compile` plays it.
    pub pieces: Vec<Piece>,
    /// The clip's start and end on the timeline.
    pub start: Ticks,
    pub end: Ticks,
}

/// One piece of a clip: where it lies on the timeline, how its ticks map to source frames,
/// and which frames it plays.
#[derive(Clone, Debug)]
pub struct Piece {
    pub start: Ticks,
    pub end: Ticks,
    pub time: ClipTime,
    pub frames: (f64, f64),
}

impl Source {
    pub fn seconds(&self) -> f64 {
        (self.frames.1 - self.frames.0) as f64 / self.rate.max(1) as f64
    }
}

/// The clip `clip` on `track`, if it is an arrangement audio clip whose file is there. A
/// session clip is refused: it has no place on the timeline for the notes to line up with.
pub fn source(app: &CantusApp, track: TrackId, clip: ClipId) -> Result<Source, String> {
    let t = app
        .project()
        .track(track)
        .ok_or_else(|| crate::i18n::t("transcribe.err.no_clip"))?;
    // `Track::clip` finds session clips too: only the arrangement's lane is looked in.
    let Some(c) = t.clips.iter().find(|c| c.id == clip) else {
        return Err(crate::i18n::t(if t.clip(clip).is_some() {
            "transcribe.err.session"
        } else {
            "transcribe.err.no_clip"
        }));
    };
    if c.audio.is_none() {
        return Err(crate::i18n::t("transcribe.err.not_audio"));
    }
    let (s, path) = app
        .pitch_source(track, clip)
        .ok_or_else(|| crate::i18n::t("transcribe.err.no_clip"))?;
    if !path.is_file() {
        return Err(crate::i18n::t("transcribe.err.missing"));
    }
    let base = ClipTime::of(app.project(), c, &s);
    let pieces: Vec<Piece> = clip_pieces(c.start, c.end(), c.period())
        .into_iter()
        .map(|(start, end)| {
            let time = ClipTime {
                start,
                end,
                ..base.clone()
            };
            let (fa, fb) = (time.frame(start), time.frame(end));
            Piece {
                start,
                end,
                time,
                frames: (fa.min(fb).max(0.0), fa.max(fb).max(0.0)),
            }
        })
        .collect();
    let a = pieces.iter().map(|p| p.frames.0).fold(f64::MAX, f64::min);
    let b = pieces.iter().map(|p| p.frames.1).fold(0.0, f64::max);
    Ok(Source {
        track,
        clip,
        clip_name: c.name.clone(),
        track_name: t.name.clone(),
        path,
        rate: s.sample_rate.max(1),
        frames: (a.max(0.0).floor() as u64, b.ceil() as u64),
        pieces,
        start: c.start,
        end: c.end(),
    })
}

/// The pieces a clip plays as (start, end), as `Snapshot::compile` cuts them: one per
/// period of a loop shorter than the clip (the last cut at the clip's end), else the whole.
pub fn clip_pieces(start: Ticks, end: Ticks, period: Option<Ticks>) -> Vec<(Ticks, Ticks)> {
    match period {
        Some(p) if p > Ticks::ZERO && p < end - start => {
            let mut out = Vec::new();
            let mut at = start;
            while at < end && out.len() < MAX_PIECES {
                out.push((at, (at + p).min(end)));
                at += p;
            }
            out
        }
        _ => vec![(start, end)],
    }
}

/// The most pieces a loop is played in (`Snapshot::compile`'s own limit).
const MAX_PIECES: usize = 4096;

/// The rate the job wants for `mode` (and whether the drum model runs).
pub fn input_rate(mode: Mode) -> u32 {
    match mode {
        Mode::Drums => 44_100,
        _ => cantus_ml::transcribe::notes::SAMPLE_RATE,
    }
}

/// The cache folder conversions work in.
pub fn cache_root() -> PathBuf {
    crate::app::data_dir().join("Cache").join("transcribe")
}

/// A conversion's folder in the cache, held by a lock file while the conversion lives and
/// removed with everything in it when the last holder lets go. A folder left by a Cantus
/// that ended mid-conversion is found by [`sweep`]: its lock is no longer held.
pub struct WorkDir {
    pub path: PathBuf,
    lock: Option<std::fs::File>,
}

const LOCK: &str = "held.lock";

impl WorkDir {
    pub fn new(id: u64) -> Result<WorkDir, String> {
        let path = cache_root().join(format!("{}-{id}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        let f = std::fs::File::create(path.join(LOCK)).map_err(|e| e.to_string())?;
        f.lock().map_err(|e| e.to_string())?;
        Ok(WorkDir {
            path,
            lock: Some(f),
        })
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        // The lock goes first: Windows does not delete an open file.
        drop(self.lock.take());
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Remove the folders of conversions no Cantus holds any more (one that ended while a
/// conversion ran). Returns how many went.
pub fn sweep() -> usize {
    let Ok(entries) = std::fs::read_dir(cache_root()) else {
        return 0;
    };
    let mut n = 0;
    for e in entries.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.join(LOCK))
            .is_ok_and(|f| match f.try_lock() {
                Ok(()) => {
                    let _ = f.unlock();
                    false
                }
                Err(_) => true,
            });
        if !held && std::fs::remove_dir_all(&dir).is_ok() {
            n += 1;
        }
    }
    n
}

/// The helper job's arguments. `models` says where the ready models are; the note modes
/// need Basic Pitch, drums take Onsets and Frames Drums when it is ready.
pub fn job_args(
    models: &cantus_content::models::Models,
    s: &Settings,
    input: PathBuf,
    out: PathBuf,
) -> Result<TranscribeArgs, String> {
    let ready = |id: &str| models.ready(id);
    let (runtime, model) = match s.mode {
        Mode::Drums => match ready(DRUM_MODEL) {
            Some(r) => (
                Some(runtime_of(&r.library, &r.library_sha256)),
                Some(NoteModel {
                    graph: r.graph,
                    family: NoteFamily::OafDrums,
                    check: checks(&r.files),
                }),
            ),
            None => (None, None),
        },
        // Melody is Cantus's own tracker, in process: no model.
        Mode::Melody => (None, None),
        Mode::Polyphonic => {
            let r = ready(NOTE_MODEL).ok_or_else(|| crate::i18n::t("transcribe.err.no_model"))?;
            (
                Some(runtime_of(&r.library, &r.library_sha256)),
                Some(NoteModel {
                    graph: r.graph,
                    family: NoteFamily::BasicPitch,
                    check: checks(&r.files),
                }),
            )
        }
    };
    Ok(TranscribeArgs {
        mode: s.mode,
        input,
        out,
        runtime,
        model,
        notes: s.notes(),
        drums: DrumSettings {
            sensitivity: s.sensitivity.clamp(0.0, 1.0),
        },
    })
}

/// The `separate` job's arguments: the clip's audio (written at 44.1 kHz) into stems'
/// sources under `out_dir`, when HT-Demucs is ready.
pub fn separate_args(
    models: &cantus_content::models::Models,
    input: PathBuf,
    out_dir: PathBuf,
) -> Option<cantus_ml::spec::SeparateArgs> {
    let r = models.ready(STEMS_MODEL)?;
    Some(cantus_ml::spec::SeparateArgs {
        runtime: runtime_of(&r.library, &r.library_sha256),
        model: cantus_ml::spec::ModelSpec {
            check: checks(&r.files),
            graph: r.graph,
            family: cantus_ml::spec::Family::HtDemucs,
            sources: r.sources,
            sample_rate: r.sample_rate,
            segment: r.segment,
        },
        input,
        out_dir,
        overlap: cantus_ml::spec::default_overlap(),
        // The drum stem is for listening, not for playing: one pass is enough.
        shifts: 0,
    })
}

fn runtime_of(library: &Path, sha256: &str) -> RuntimeSpec {
    RuntimeSpec {
        library: library.to_path_buf(),
        threads: 0,
        device: Device::Cpu,
        sha256: Some(sha256.to_string()),
    }
}

/// A ready model's files as the job checks them before it loads anything.
fn checks(files: &[(PathBuf, String)]) -> Vec<cantus_ml::spec::FileCheck> {
    files
        .iter()
        .map(|(path, sha256)| cantus_ml::spec::FileCheck {
            path: path.clone(),
            sha256: sha256.clone(),
        })
        .collect()
}

/// The track a conversion makes: its instrument.
pub fn instrument(s: &Settings) -> InstrumentSpec {
    match (s.mode, s.instrument) {
        // The pads named in the interface's language, as the kits the user loads are.
        (Mode::Drums, _) => {
            let mut kit = PulsusParams::studio();
            for pad in &mut kit.pads {
                pad.name = crate::app::localized_pad_name(pad);
            }
            InstrumentSpec::Pulsus(kit)
        }
        (_, Instrument::Unda) => InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
        // Exemplar with its first factory preset (plucked strings from the built-in
        // sounds): an empty sampler would be silent.
        (_, Instrument::Exemplar) => InstrumentSpec::Native(crate::views::inst::preset_spec(
            InstrumentKind::Sampler,
            &crate::views::inst::sampler::FACTORY_PRESETS[0],
        )),
    }
}

/// What [`plan`] made.
pub struct Planned {
    pub command: Command,
    pub track: TrackId,
    pub clip: ClipId,
    pub notes: usize,
}

/// A note on the timeline: start and end ticks, pitch, velocity, its bend as points
/// `(share of the note's length, semitones)`.
type Placed = (Ticks, Ticks, u8, u8, Vec<(f64, f64)>);

/// The same in seconds from the clip's first frame, before placing.
type Timed = (f64, f64, u8, u8, Vec<(f64, f64)>);

/// A note's bend points: Basic Pitch's thirds of a semitone evenly spaced over it, or the
/// tracker's curve.
fn bend_points(n: &cantus_ml::transcribe::spec::NoteEvent) -> Vec<(f64, f64)> {
    if !n.curve.is_empty() {
        return n
            .curve
            .iter()
            .map(|(a, b)| (*a as f64, *b as f64))
            .collect();
    }
    let k = n.bends.len();
    n.bends
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (
                if k > 1 {
                    i as f64 / (k - 1) as f64
                } else {
                    0.0
                },
                *b as f64 / 3.0,
            )
        })
        .collect()
}

/// The transcription's notes (or hits) on the timeline: each piece of the clip places what
/// its own frames hold through its own mapping (a loop's pieces after a tempo change, a
/// reversed piece mirrored in its own window, a short last piece). A note cut by a piece's
/// edge keeps the part of its bend over what is kept; a reversed note's bend runs the
/// other way. A drum hit lands where it is struck, a fixed short length after.
pub fn place(src: &Source, s: &Settings, t: &Transcription) -> Vec<Placed> {
    let frame = |sec: f64| src.frames.0 as f64 + sec * src.rate as f64;
    let drums = s.mode == Mode::Drums;
    let items: Vec<Timed> = if drums {
        t.hits
            .iter()
            .filter(|h| s.classes[class_index(h.class)])
            .map(|h| (h.time, h.time, h.key, h.velocity, Vec::new()))
            .collect()
    } else {
        t.notes
            .iter()
            .map(|n| {
                let b = if s.bends { bend_points(n) } else { Vec::new() };
                (n.start, n.end, n.pitch, n.velocity, b)
            })
            .collect()
    };
    let mut out = Vec::new();
    for p in &src.pieces {
        let (pa, pb) = p.frames;
        for (a, b, pitch, vel, bends) in &items {
            let (fa, fb) = (frame(*a), frame(*b));
            if drums {
                // An onset inside the piece's frames, at the tick it sounds.
                if fa < pa || fa >= pb {
                    continue;
                }
                let at = p.time.tick(fa).max(p.start);
                if at >= p.end {
                    continue;
                }
                let len = Ticks(Ticks::QUARTER.0 / 8).min(src.end - at);
                out.push((at, at + len.max(Ticks(1)), *pitch, *vel, Vec::new()));
                continue;
            }
            // The part of the note this piece plays, in frames.
            let (ka, kb) = (fa.max(pa), fb.min(pb));
            if kb <= ka {
                continue;
            }
            let (ta, tb) = (p.time.tick(ka), p.time.tick(kb));
            let reversed = tb < ta;
            let (x, y) = (ta.min(tb).max(p.start), ta.max(tb).min(p.end));
            if y <= x {
                continue;
            }
            let span = (fb - fa).max(1e-9);
            let kept = ((ka - fa) / span, (kb - fa) / span);
            out.push((x, y, *pitch, *vel, remap_bend(bends, kept, reversed)));
        }
    }
    if let Some(g) = s.quantize.ticks() {
        for n in &mut out {
            let len = n.1 - n.0;
            let q = Ticks(((n.0 .0 as f64 / g.0 as f64).round() as i64) * g.0).max(src.start);
            n.0 = q;
            n.1 = (q + len).max(q + Ticks(1));
        }
    }
    out.sort_by_key(|n| (n.0, n.2));
    out
}

/// A bend's points `(share of the note, semitones)` over the kept share `kept` of the note
/// only, rescaled to it (its edges' values interpolated), and run backwards when the note
/// plays reversed.
pub fn remap_bend(points: &[(f64, f64)], kept: (f64, f64), reversed: bool) -> Vec<(f64, f64)> {
    if points.is_empty() {
        return Vec::new();
    }
    let value = |at: f64| -> f64 {
        match points.iter().position(|p| p.0 >= at) {
            None => points[points.len() - 1].1,
            Some(0) => points[0].1,
            Some(i) => {
                let (a, b) = (points[i - 1], points[i]);
                let w = if b.0 > a.0 {
                    (at - a.0) / (b.0 - a.0)
                } else {
                    0.0
                };
                a.1 + w * (b.1 - a.1)
            }
        }
    };
    let (k0, k1) = kept;
    let len = (k1 - k0).max(1e-12);
    let mut out = vec![(0.0, value(k0))];
    out.extend(
        points
            .iter()
            .filter(|p| p.0 > k0 && p.0 < k1)
            .map(|p| ((p.0 - k0) / len, p.1)),
    );
    out.push((1.0, value(k1)));
    if reversed {
        out = out.into_iter().rev().map(|(a, v)| (1.0 - a, v)).collect();
    }
    out
}

/// A note's bend points as a per-note bend curve, at the note's ticks from its start: a
/// point where the value moves by 2 cents or more (and the last), so a held pitch is one
/// point and a step stays a step.
fn bend_curve(len: Ticks, points: &[(f64, f64)]) -> Option<Box<NoteExpr>> {
    if points.iter().all(|p| p.1.abs() < 0.02) {
        return None;
    }
    let mut e = NoteExpr::default();
    let curve = e.curve_mut(ExprKind::Bend);
    let mut last: Option<f64> = None;
    for (i, (at, v)) in points.iter().enumerate() {
        if last.is_some_and(|l| (l - v).abs() < 0.02) && i + 1 != points.len() {
            continue;
        }
        let at = (len.0 as f64 * at.clamp(0.0, 1.0)).round() as i64;
        curve.push(ExprPoint(at, ExprKind::Bend.store(*v)));
        last = Some(*v);
    }
    e.normalised().boxed()
}

/// Melody mode: Cantus's own pitch tracker on the clip's audio (a worker's part) — the
/// pitch editor's notes, each with its pitch curve as its bend, notes shorter than the
/// shortest kept dropped. (Sensitivity is the models': the tracker decides what is voiced
/// by itself, so melody mode does not show it.) An error once `cancel` is set.
pub fn melody(
    src: &Source,
    s: &Settings,
    cancel: &dyn Fn() -> bool,
) -> Result<Transcription, String> {
    let (mono, rate) = read_mono_range(&src.path, src.frames.0, src.frames.1, cancel)?;
    let rate = rate.max(1);
    let take = cantus_engine::pitch::analyse_take(&[&mono], rate);
    if cancel() {
        return Err(crate::i18n::t("transcribe.stopped"));
    }
    let hop = take.analysis.hop.max(1) as u64;
    let shortest = (s.shortest_ms.max(0.0) as f64 / 1000.0 * rate as f64) as u64;
    let notes = cantus_core::pitch::midi::notes(&take.edit())
        .into_iter()
        .filter(|n| n.end.saturating_sub(n.start) >= shortest)
        .map(|n| {
            let curve = if s.bends && n.end > n.start {
                let (f0, f1) = (n.start / hop, n.end.div_ceil(hop));
                (f0..f1)
                    .filter_map(|i| {
                        let m = *take.midi.get(i as usize)?;
                        let at =
                            ((i * hop).saturating_sub(n.start)) as f64 / (n.end - n.start) as f64;
                        (!m.is_nan()).then_some((at.min(1.0) as f32, m - n.pitch as f32))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            cantus_ml::transcribe::spec::NoteEvent {
                start: n.start as f64 / rate as f64,
                end: n.end as f64 / rate as f64,
                pitch: n.pitch,
                velocity: n.velocity,
                amplitude: n.velocity as f32 / 127.0,
                bends: Vec::new(),
                curve,
            }
        })
        .collect();
    Ok(Transcription {
        mode: Mode::Melody,
        duration: mono.len() as f64 / rate as f64,
        notes,
        hits: Vec::new(),
    })
}

/// The conversion as one command: a new track under the source with the instrument, and a
/// MIDI clip aligned to the audio clip holding the notes.
pub fn plan(
    bus: &mut CommandBus,
    src: &Source,
    s: &Settings,
    t: &Transcription,
) -> Result<Planned, String> {
    let placed = place(src, s, t);
    let project = bus.project();
    let index = project
        .track_index(src.track)
        .map_or(project.tracks.len(), |i| i + 1);
    let strip = crate::audio_files::free_strips(project).first().copied();
    let color = project.track(src.track).map(|t| t.color);
    let suffix = match s.mode {
        Mode::Drums => crate::i18n::t("transcribe.suffix.drums"),
        _ => "MIDI".to_string(),
    };
    let name = format!("{} · {suffix}", src.clip_name);
    let mut track = bus.new_track(name.clone());
    track.instrument = instrument(s);
    track.strip = strip;
    if let Some(c) = color {
        track.color = c;
    }
    let tid = track.id;
    let mut clip = bus.new_clip(name, src.start, src.end - src.start);
    for (a, b, pitch, vel, bends) in &placed {
        let start = *a - src.start;
        let len = (*b - *a).max(Ticks(1));
        let mut n = bus.new_note(start, len, *pitch, (*vel).clamp(1, 127));
        n.expr = bend_curve(len, bends);
        clip.notes.push(n);
    }
    clip.notes.sort_by_key(|n| (n.start, n.pitch));
    let cid = clip.id;
    let notes = clip.notes.len();
    Ok(Planned {
        command: Command::Batch {
            label_key: "cmd.transcribe".into(),
            commands: vec![
                Command::InsertTrack {
                    index,
                    track: Box::new(track),
                },
                Command::InsertClip {
                    track: tid,
                    clip: Box::new(clip),
                },
            ],
        },
        track: tid,
        clip: cid,
        notes,
    })
}

/// The mode's name for the history and the agent ("melody", "polyphonic", "drums").
pub fn mode_name(m: Mode) -> &'static str {
    match m {
        Mode::Melody => "melody",
        Mode::Polyphonic => "polyphonic",
        Mode::Drums => "drums",
    }
}

pub fn parse_mode(s: &str) -> Option<Mode> {
    [Mode::Melody, Mode::Polyphonic, Mode::Drums]
        .into_iter()
        .find(|m| mode_name(*m) == s)
}
