//! Listening, for the agent tools and the mastering assistant (round 34, ADR 0083): what a
//! render needs from the application, the checkpoints kept between listens, ranges in the
//! units an agent writes (bars:beats, beats, a marker's name), a listen run on a worker,
//! and what it heard turned into numbers.
//!
//! The application's side is [`ListenHost`]: the snapshot compiled for a render (samples
//! attached), the hosted plug-ins' render instances, and the [`Service`] that keeps the
//! checkpoints. `CantusApp` is one; the tests have their own.

use cantus_ai::{JobCx, ToolError, ToolHost};
use cantus_core::{Project, Snapshot, Ticks, TrackId};
use cantus_dsp::speculum::ear::{octave_hz, Ear, Reading, OCTAVES};
use cantus_engine::effects::EffectUnit;
use cantus_engine::inst::NativeUnit;
use cantus_engine::listen::{self, Checkpoints, Heard, ListenOptions, Resumed};
use cantus_engine::stems::StemSource;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// After this long with no listen, the checkpoints are let go (review 3: a session that
/// used the assistant once kept their memory for hours).
pub const QUIET: Duration = Duration::from_secs(10 * 60);

/// What a render of a project needs, taken on the UI thread.
pub struct RenderInputs {
    pub snapshot: Snapshot,
    pub units: Vec<EffectUnit>,
    pub instruments: Vec<NativeUnit>,
    pub sample_rate: u32,
}

/// The application's side of a listen.
pub trait ListenHost {
    /// The inputs of a render of `project` (not necessarily the open one: a solo, the
    /// master switched off). Refused, with what to do, while samples or a render's
    /// plug-ins are still loading.
    fn listen_inputs(&mut self, project: &Project) -> Result<RenderInputs, ToolError>;
    fn listen_service(&mut self) -> &mut Service;
    /// A number that changes whenever another project replaces the open one: what the
    /// checkpoints and the proposals belong to.
    fn listen_epoch(&self) -> u64;
}

/// The concrete host behind a tool call.
pub fn host_of(host: &mut dyn ToolHost) -> Result<&mut dyn ListenHost, ToolError> {
    let any = host.as_any_mut();
    if any.is::<crate::app::CantusApp>() {
        return Ok(any
            .downcast_mut::<crate::app::CantusApp>()
            .expect("checked") as &mut dyn ListenHost);
    }
    #[cfg(test)]
    if any.is::<tests::TestHost>() {
        return Ok(any.downcast_mut::<tests::TestHost>().expect("checked") as &mut dyn ListenHost);
    }
    Err(ToolError::Failed(
        "this host cannot render (no engine behind it)".into(),
    ))
}

/// What the application keeps for listening: the checkpoints of the open project, the
/// folder heard renders are written to, and a note that a render holding hosted plug-ins'
/// instances has ended (they are let go on the UI thread, [`Service::frame`]).
pub struct Service {
    pub checkpoints: Arc<Mutex<Checkpoints>>,
    pub wav_dir: PathBuf,
    /// Listens running with hosted plug-ins' render instances.
    pub hosted_running: Arc<AtomicUsize>,
    /// One such listen ended: the instances go at the next frame.
    pub hosted_done: Arc<AtomicBool>,
    /// The mastering assistant's proposals, for `apply_master_chain` and the panel.
    pub proposals: Arc<Mutex<super::run::Proposals>>,
    /// The mastering assistant's panel.
    pub panel: super::panel::Panel,
    /// The project epoch the checkpoints, the proposals and the panel's answer belong to.
    epoch: Option<u64>,
    /// Raised when the project is replaced: every listen prepared before it stops at its
    /// next block (review 2: agents' listens ran on, and a proposal leaked into the next
    /// project's store).
    pub stale: Arc<AtomicBool>,
    /// When a listen last began or ended ([`QUIET`]).
    heard: Arc<Mutex<Instant>>,
}

impl Default for Service {
    fn default() -> Self {
        Service::new(crate::app::data_dir().join("cache").join("listen"))
    }
}

impl Service {
    pub fn new(wav_dir: PathBuf) -> Service {
        Service {
            checkpoints: Arc::new(Mutex::new(Checkpoints::default())),
            wav_dir,
            hosted_running: Arc::new(AtomicUsize::new(0)),
            hosted_done: Arc::new(AtomicBool::new(false)),
            proposals: Arc::default(),
            panel: Default::default(),
            epoch: None,
            stale: Arc::new(AtomicBool::new(false)),
            heard: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// The checkpoints let go when no listen has begun or ended for [`QUIET`] — never
    /// waiting: a running listen holds their lock, and is not quiet.
    pub fn release_if_quiet(&mut self, now: Instant) {
        let last = self.heard.lock().map_or(now, |t| *t);
        if now.saturating_duration_since(last) < QUIET {
            return;
        }
        if let Ok(mut c) = self.checkpoints.try_lock() {
            if !c.is_empty() {
                c.clear();
            }
        }
    }

    /// How long until the checkpoints are quiet enough to let go, for the window to wake
    /// then (integration review: an idle window draws no frame, and a minimised one kept
    /// them until touched) — `None` when there are none (or a listen holds them: its end
    /// says it was heard, and the frames it brings ask again).
    pub fn quiet_wake(&self, now: Instant) -> Option<Duration> {
        let kept = self
            .checkpoints
            .try_lock()
            .ok()
            .is_some_and(|c| !c.is_empty());
        if !kept {
            return None;
        }
        let last = self.heard.lock().map_or(now, |t| *t);
        Some(QUIET.saturating_sub(now.saturating_duration_since(last)))
    }

    /// A listen began or ended now.
    fn note_heard(heard: &Mutex<Instant>) {
        if let Ok(mut t) = heard.lock() {
            *t = Instant::now();
        }
    }

    /// Every frame, on the UI thread: a finished render's plug-in instances let go once
    /// no other listen holds any.
    pub fn frame(&mut self, plugins: &mut cantus_host::PluginHost) {
        if self.hosted_running.load(Ordering::Acquire) == 0
            && self.hosted_done.swap(false, Ordering::AcqRel)
        {
            plugins.finish_render();
        }
    }

    /// Forget everything of the last project: its checkpoints (whole engines, its samples
    /// in them), its proposals (an id from it must not apply to another), the panel's
    /// answer. The panel stays open, its choices kept.
    ///
    /// Never waits (review 2: a worker holds the checkpoints' lock for its whole render, and
    /// locking them here froze the window for its length): the runs are told to stop first,
    /// then fresh stores take the old ones' place — what a stopping run still holds goes
    /// with it, and a proposal it still makes lands in the old store, which nothing reads.
    pub fn forget(&mut self) {
        self.stale.store(true, Ordering::Release);
        self.stale = Arc::new(AtomicBool::new(false));
        let panel = &mut self.panel;
        if let Some(r) = panel.running.take() {
            r.cancel.store(true, Ordering::Relaxed);
        }
        self.checkpoints = Arc::new(Mutex::new(Checkpoints::default()));
        self.proposals = Arc::default();
        panel.done = None;
        panel.error = None;
        panel.menu = None;
        // The A/B's preview is the engine's: the panel stops it at its next frame.
        if panel.ab.take().is_some() {
            panel.stop_preview = true;
        }
    }

    /// Note the open project's epoch; forget the last project's things when it changed
    /// (review 1: nothing forgot them on a project switch). Called by every listen, every
    /// apply and every frame.
    pub fn sync_epoch(&mut self, epoch: u64) {
        if self.epoch != Some(epoch) {
            if self.epoch.is_some() {
                self.forget();
            }
            self.epoch = Some(epoch);
        }
        self.release_if_quiet(Instant::now());
    }
}

/// Whether `project` hosts a plug-in anywhere (an insert or an instrument).
pub fn hosts_plugins(project: &Project) -> bool {
    project
        .all_effects()
        .any(|(_, e)| e.spec.kind == cantus_core::EffectKind::External)
        || project
            .tracks
            .iter()
            .any(|t| matches!(t.instrument, cantus_core::InstrumentSpec::External(_)))
}

// ---- ranges ------------------------------------------------------------------------------

/// A range of the song, in ticks; `end: None` is the song's end.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: Ticks,
    pub end: Option<Ticks>,
}

/// A position as an agent writes it: beats from the song's start (a number), or
/// `"bar:beat"` / `"bar"` (1-based, through the metre changes).
pub fn position(project: &Project, v: &Value, name: &str) -> Result<Ticks, ToolError> {
    let map = project.tempo_map();
    if let Some(b) = v.as_f64() {
        if !b.is_finite() || b < 0.0 {
            return Err(ToolError::bad(name, "beats from the start, 0 or more"));
        }
        return Ok(Ticks((b * cantus_core::PPQ as f64).round() as i64));
    }
    let s = v
        .as_str()
        .ok_or_else(|| ToolError::bad(name, "a number of beats or \"bar:beat\""))?;
    let mut it = s.trim().split(':');
    let bar: i64 = it
        .next()
        .and_then(|b| b.trim().parse().ok())
        .filter(|b| *b >= 1)
        .ok_or_else(|| ToolError::bad(name, "a bar from 1, as \"17\" or \"17:3\""))?;
    let beat: f64 = match it.next() {
        Some(b) => b
            .trim()
            .parse()
            .ok()
            .filter(|b: &f64| *b >= 1.0)
            .ok_or_else(|| ToolError::bad(name, "a beat from 1, as \"17:3\""))?,
        None => 1.0,
    };
    let start = map.bar_tick(bar);
    let beat_len = map.beat_len_at(start).0 as f64;
    Ok(start + Ticks(((beat - 1.0) * beat_len).round() as i64))
}

/// A tick as an agent reads it: bar:beat (1-based), beats and seconds from the start.
pub fn tick_json(project: &Project, tick: Ticks) -> Value {
    let map = project.tempo_map();
    let (bar, beat, rest) = map.bbt(tick);
    let bar_beat = if rest == 0 {
        format!("{bar}:{beat}")
    } else {
        let len = map.beat_len_at(tick).0.max(1) as f64;
        format!("{bar}:{:.3}", beat as f64 + rest as f64 / len)
    };
    json!({
        "bar_beat": bar_beat,
        "beats": round(tick.as_beats_f64(), 3),
        "seconds": round(map.seconds_at(tick.0 as f64), 3),
    })
}

/// The range the arguments name: `section` (a marker's name: from it to the next marker),
/// or `start` and `end`; the whole song by default.
pub fn range_of(project: &Project, args: &Value) -> Result<Range, ToolError> {
    if let Some(name) = args.get("section").and_then(Value::as_str) {
        let mut marks: Vec<&cantus_core::tempo::Marker> = project.markers.iter().collect();
        marks.sort_by_key(|m| m.tick);
        let i = marks
            .iter()
            .position(|m| m.name.eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| {
                let names: Vec<&str> = marks.iter().map(|m| m.name.as_str()).collect();
                ToolError::bad(
                    "section",
                    format!("no marker called {name:?} (markers: {names:?})"),
                )
            })?;
        return Ok(Range {
            start: marks[i].tick,
            end: marks.get(i + 1).map(|m| m.tick),
        });
    }
    let start = match args.get("start") {
        Some(v) if !v.is_null() => position(project, v, "start")?,
        _ => Ticks::ZERO,
    };
    let end = match args.get("end") {
        Some(v) if !v.is_null() => Some(position(project, v, "end")?),
        _ => None,
    };
    if end.is_some_and(|e| e <= start) {
        return Err(ToolError::bad("end", "after the start"));
    }
    if end.is_none() && start >= project.content_end() {
        return Err(ToolError::bad("start", "before the song's end"));
    }
    Ok(Range { start, end })
}

/// Where sections start: the markers, or every eight bars when there are none — where
/// checkpoints are taken and section loudness is read.
pub fn section_starts(project: &Project) -> Vec<Ticks> {
    let mut v: Vec<Ticks> = project.markers.iter().map(|m| m.tick).collect();
    if v.is_empty() {
        let map = project.tempo_map();
        let end = project.content_end();
        let mut bar = 9;
        loop {
            let t = map.bar_tick(bar);
            if t >= end {
                break;
            }
            v.push(t);
            bar += 8;
        }
    }
    v.retain(|t| *t > Ticks::ZERO);
    v.sort();
    v.dedup();
    v
}

/// `project` with `solo` soloed and `mute` muted (track ids), and the master chain on or
/// off: what a listen renders. Unknown ids are refused by name.
pub fn heard_project(
    project: &Project,
    solo: &[TrackId],
    mute: &[TrackId],
    master: bool,
) -> Result<Project, ToolError> {
    let mut p = project.clone();
    for (what, ids) in [("solo", solo), ("mute", mute)] {
        for id in ids {
            if p.track(*id).is_none() {
                return Err(ToolError::bad(what, format!("no track {}", id.0)));
            }
        }
    }
    if !solo.is_empty() {
        for t in &mut p.tracks {
            t.soloed = solo.contains(&t.id);
        }
    }
    for t in &mut p.tracks {
        if mute.contains(&t.id) {
            t.muted = true;
        }
    }
    p.master.fx_enabled = master && p.master.fx_enabled;
    Ok(p)
}

/// Track ids from an argument (an array of integers).
pub fn ids(args: &Value, name: &str) -> Result<Vec<TrackId>, ToolError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| {
                v.as_u64()
                    .map(TrackId)
                    .ok_or_else(|| ToolError::bad(name, "track ids (integers)"))
            })
            .collect(),
        Some(_) => Err(ToolError::bad(name, "an array of track ids")),
    }
}

pub fn round(x: f64, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (x * k).round() / k
}

/// A level for JSON: rounded, and `null` for −∞ / not a number (silence).
pub fn level(x: f64) -> Value {
    if x.is_finite() {
        json!(round(x, 2))
    } else {
        Value::Null
    }
}

// ---- a listen on a worker -----------------------------------------------------------------

/// The longest range a listen hears, seconds (every tap's ear grows with it).
pub const MAX_LISTEN_S: f64 = 20.0 * 60.0;
/// The most frames a listen that keeps the master's samples holds (a WAV, the mastering
/// assistant's mix and its trials): twenty minutes at 48 kHz — a whole long song at the
/// usual rates, a shorter one at 192 kHz (review 2: a cap in seconds did not bound memory at
/// high rates, and 15 minutes refused whole songs). Measured at the cap (review 3: a
/// 19-minute song through the panel, `listen_measure::the_assistants_memory_at_the_cap`):
/// 1.1 GB of working set at its peak — the mix and the mastered audio 0.9 GB (a trial in
/// their place while the search runs), the 24 checkpoints 190 MB, 7.9 MB each
/// (`the_checkpoints_memory_on_a_long_song`; ADR 0083).
pub const MAX_AUDIO_FRAMES: f64 = 20.0 * 60.0 * 48_000.0;

/// A length in minutes as "20 min 18 s" ("20 min" when whole) — never rounded to look
/// within a limit it is over (review 3: "20 … 20 at most").
pub fn min_sec(minutes: f64) -> String {
    let total = (minutes * 60.0).round().max(0.0) as u64;
    match (total / 60, total % 60) {
        (m, 0) => format!("{m} min"),
        (0, s) => format!("{s} s"),
        (m, s) => format!("{m} min {s} s"),
    }
}

/// A range longer than a listen may hear.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TooLong {
    pub minutes: f64,
    pub most_minutes: f64,
}

/// Whether `range` of `project` may be listened to, keeping its audio or not, at
/// `sample_rate` (see [`MAX_LISTEN_S`], [`MAX_AUDIO_FRAMES`]).
pub fn length_check(
    project: &Project,
    range: Range,
    keep_audio: bool,
    sample_rate: u32,
) -> Result<(), TooLong> {
    let end = range.end.unwrap_or_else(|| project.content_end());
    let seconds = project
        .tempo_map()
        .seconds_between(range.start, end.max(range.start));
    let most = if keep_audio {
        (MAX_AUDIO_FRAMES / sample_rate.max(1) as f64).min(MAX_LISTEN_S)
    } else {
        MAX_LISTEN_S
    };
    if seconds > most {
        Err(TooLong {
            minutes: seconds / 60.0,
            most_minutes: most / 60.0,
        })
    } else {
        Ok(())
    }
}

/// A listen prepared on the UI thread, run on a worker.
pub struct Prepared {
    pub inputs: RenderInputs,
    pub opts: ListenOptions,
    pub checkpoints: Arc<Mutex<Checkpoints>>,
    _hosted: Option<HostedGuard>,
    /// Raised when the project this listen was prepared for is replaced.
    stale: Arc<AtomicBool>,
    /// The proposals store of the project it was prepared for — taken after the epoch was
    /// synced (review 3: one copied before `prepare` was the store a switch had just
    /// replaced, and the proposal was lost).
    proposals: Arc<Mutex<super::run::Proposals>>,
    /// When a listen last began or ended (the service's).
    heard: Arc<Mutex<Instant>>,
}

/// A listen holding hosted plug-ins' render instances: when it ends — run, failed or
/// dropped unrun — the UI thread is told to let them go ([`Service::frame`]).
struct HostedGuard {
    running: Arc<AtomicUsize>,
    done: Arc<AtomicBool>,
}

impl Drop for HostedGuard {
    fn drop(&mut self) {
        self.running.fetch_sub(1, Ordering::AcqRel);
        self.done.store(true, Ordering::Release);
    }
}

/// What a listen needs, taken now: `project` as it is to be heard, over `range`, hearing
/// `taps` besides the master; `keep_audio` keeps the master's samples (a WAV).
pub fn prepare(
    host: &mut dyn ListenHost,
    project: &Project,
    range: Range,
    taps: Vec<StemSource>,
    keep_audio: bool,
    tail_seconds: f64,
) -> Result<Prepared, ToolError> {
    let epoch = host.listen_epoch();
    host.listen_service().sync_epoch(epoch);
    let inputs = host.listen_inputs(project)?;
    length_check(project, range, keep_audio, inputs.sample_rate).map_err(|t| {
        ToolError::Refused(format!(
            "the range is {} long; choose a range of at most {}",
            min_sec(t.minutes),
            min_sec(t.most_minutes)
        ))
    })?;
    let hosted = !inputs.units.is_empty() || !inputs.instruments.is_empty();
    let service = host.listen_service();
    let hosted = hosted.then(|| {
        service.hosted_running.fetch_add(1, Ordering::AcqRel);
        HostedGuard {
            running: service.hosted_running.clone(),
            done: service.hosted_done.clone(),
        }
    });
    let mut sections = section_starts(project);
    sections.push(range.start);
    if let Some(e) = range.end {
        sections.push(e);
    }
    Ok(Prepared {
        opts: ListenOptions {
            sample_rate: inputs.sample_rate,
            start: range.start,
            end: range.end,
            tail_seconds,
            taps,
            keep_audio,
            sections,
            ..Default::default()
        },
        inputs,
        checkpoints: service.checkpoints.clone(),
        _hosted: hosted,
        stale: service.stale.clone(),
        proposals: service.proposals.clone(),
        heard: {
            Service::note_heard(&service.heard);
            service.heard.clone()
        },
    })
}

impl Prepared {
    /// The flag raised when the project this listen was prepared for is replaced — for
    /// what the job does after hearing (the assistant's trials).
    pub fn stale_flag(&self) -> Arc<AtomicBool> {
        self.stale.clone()
    }

    /// The proposals store of the project this listen was prepared for.
    pub fn proposals(&self) -> Arc<Mutex<super::run::Proposals>> {
        self.proposals.clone()
    }

    /// Render and hear (on a worker): progress and cancel through the job's context.
    pub fn run(self, cx: &JobCx<'_>) -> Result<Heard, ToolError> {
        let Prepared {
            inputs,
            opts,
            checkpoints,
            _hosted,
            stale,
            proposals: _,
            heard,
        } = self;
        if stale.load(Ordering::Acquire) {
            return Err(ToolError::Cancelled);
        }
        let mut cps = checkpoints
            .lock()
            .map_err(|_| ToolError::Failed("the checkpoints were poisoned".into()))?;
        let r = listen::listen(
            &inputs.snapshot,
            &opts,
            inputs.units,
            inputs.instruments,
            &mut cps,
            &mut |f| {
                (cx.progress)(f);
                !(cx.cancelled)() && !stale.load(Ordering::Relaxed)
            },
        );
        // Heard before the lock goes (integration review): a frame between the two would find
        // a listen of over ten minutes quiet and let go what it just kept.
        Service::note_heard(&heard);
        drop(cps);
        drop(_hosted);
        r.map_err(|e| match e {
            cantus_engine::RenderError::Cancelled => ToolError::Cancelled,
            e => ToolError::Failed(e.to_string()),
        })
    }
}

/// How a listen began, for the agent.
pub fn resumed_json(project: &Project, r: &Resumed) -> Value {
    match r {
        Resumed::Start => json!({ "from": "start", "exact": true }),
        Resumed::Checkpoint(t) => json!({
            "from": "checkpoint", "exact": true, "checkpoint": tick_json(project, *t)
        }),
        Resumed::Preroll { from, seconds, why } => json!({
            "from": "preroll", "exact": false, "preroll_from": tick_json(project, *from),
            "preroll_seconds": round(*seconds, 2), "why": why
        }),
    }
}

// ---- what was heard -----------------------------------------------------------------------

/// A reading's loudness, peaks, stereo and dynamics.
pub fn reading_json(r: &Reading) -> Value {
    let opt = |x: Option<f64>| x.map_or(Value::Null, level);
    json!({
        "seconds": round(r.seconds, 2),
        "integrated_lufs": opt(r.integrated_lufs),
        "short_term_max_lufs": opt(r.short_term_max_lufs),
        "momentary_max_lufs": opt(r.momentary_max_lufs),
        "range_lu": opt(r.range_lu),
        "true_peak_dbtp": level(r.true_peak_dbtp as f64),
        "sample_peak_dbfs": level(r.sample_peak_dbfs as f64),
        "rms_dbfs": level(r.rms_dbfs as f64),
        "crest_db": level(r.crest_db as f64),
        "correlation": r.correlation.map_or(Value::Null, |c| json!(round(c as f64, 3))),
        "correlation_min": r.correlation_min.map_or(Value::Null, |c| json!(round(c as f64, 3))),
        "width_db": level(r.width_db as f64),
        "mono_loss_lu": opt(r.mono_loss_lu),
    })
}

/// The octave spectrum relative to the 1 kHz octave, beside a reference curve when given
/// (the same relative scale), with the difference.
pub fn spectrum_json(r: &Reading, reference: Option<&[f32; OCTAVES]>) -> Value {
    let k1 = r.octaves_db[5];
    let bands: Vec<Value> = (0..OCTAVES)
        .map(|o| {
            let rel = (r.octaves_db[o] - k1) as f64;
            let mut b = json!({ "hz": round(octave_hz(o) as f64, 1), "db": level(rel) });
            if let Some(c) = reference {
                b["reference_db"] = json!(c[o]);
                b["difference_db"] = level(rel - c[o] as f64);
            }
            b
        })
        .collect();
    json!(bands)
}

/// The samples of `ear` from `a` to `b` (song ticks), as a reading: the range's own start
/// is sample 0 of the ear.
pub fn reading_between(
    ear: &Ear,
    project: &Project,
    sample_rate: u32,
    range_start: Ticks,
    a: Ticks,
    b: Ticks,
) -> Reading {
    let map = project.tempo_map();
    let s = |t: Ticks| {
        let secs = map.seconds_between(range_start, t.max(range_start));
        (secs * sample_rate as f64).round().max(0.0) as u64
    };
    ear.reading(s(a), s(b))
}

/// The section boundaries inside `range` (with the range's own ends) for the per-section
/// readings.
pub fn boundaries(project: &Project, range: Range) -> Vec<Ticks> {
    let end = range.end.unwrap_or_else(|| project.content_end());
    let mut v = vec![range.start];
    v.extend(
        section_starts(project)
            .into_iter()
            .filter(|t| *t > range.start && *t < end),
    );
    v.push(end);
    v
}

/// Clicks at section boundaries: the sharpest corner within 30 ms of each boundary inside
/// the range, against the median of the same on the bar lines that are not boundaries — a
/// boundary much sharper than the music's own downbeats is a click (more than twice the
/// largest of those, and over 0.05).
pub fn clicks_json(ear: &Ear, project: &Project, sample_rate: u32, range: Range) -> Value {
    let map = project.tempo_map();
    let bounds = boundaries(project, range);
    let inner: Vec<Ticks> = bounds[1..bounds.len().saturating_sub(1)].to_vec();
    let at = |t: Ticks| (map.seconds_between(range.start, t) * sample_rate as f64).round() as u64;
    let radius = (0.03 * sample_rate as f64) as u64;
    let end = *bounds.last().unwrap_or(&range.start);
    let mut downbeats: Vec<f32> = map
        .bar_lines(range.start, end)
        .filter(|t| *t > range.start && !inner.contains(t))
        .map(|t| ear.sharpness_near(at(t), radius))
        .collect();
    downbeats.sort_by(f32::total_cmp);
    let median = downbeats.get(downbeats.len() / 2).copied().unwrap_or(0.0);
    let largest = downbeats.last().copied().unwrap_or(0.0);
    let found: Vec<Value> = inner
        .iter()
        .map(|t| {
            let s = ear.sharpness_near(at(*t), radius);
            json!({
                "at": tick_json(project, *t),
                "sharpness": round(s as f64, 3),
                "click": s > 0.05 && s > 2.0 * largest.max(median),
            })
        })
        .collect();
    json!({
        "downbeats_median": round(median as f64, 3),
        "downbeats_largest": round(largest as f64, 3),
        "boundaries": found,
    })
}

/// Write the master's samples heard to a 32-bit float WAV in `dir` (kept to the newest
/// few), returning its path.
pub fn write_wav(dir: &Path, name: &str, heard: &Heard) -> Result<PathBuf, ToolError> {
    std::fs::create_dir_all(dir).map_err(|e| ToolError::Failed(e.to_string()))?;
    prune(dir, 8);
    let path = dir.join(name);
    let r = cantus_engine::Rendered {
        sample_rate: heard.sample_rate,
        channels: 2,
        samples: heard.audio.clone(),
        peak: 0.0,
        cut_notes: 0,
    };
    cantus_engine::render::write_wav(&r, &path).map_err(|e| ToolError::Failed(e.to_string()))?;
    Ok(path)
}

/// Keep the newest `keep - 1` WAVs of `dir` (one more is about to be written).
fn prune(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "wav"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort();
    let extra = files.len().saturating_sub(keep.saturating_sub(1));
    for (_, p) in files.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
}

/// A file's `file://` link (RFC 8089): the path's separators as `/`, a drive letter kept,
/// and every byte but the unreserved ones percent-encoded as UTF-8 — a space or an accent
/// in a user's folder name makes a valid URI (review 1).
pub fn file_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for (i, b) in s.bytes().enumerate() {
        let keep = b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/')
            // `C:` — a drive's colon stays as it is.
            || (b == b':' && i == 1);
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ---- the application as a listen's host ---------------------------------------------------

impl ListenHost for crate::app::CantusApp {
    fn listen_inputs(&mut self, project: &Project) -> Result<RenderInputs, ToolError> {
        // As the export dialog does: never a compile that would wait on the UI thread.
        if self.engine.samples.loading() {
            return Err(ToolError::Failed(
                "Cantus is still loading this project's samples; ask again in a moment".into(),
            ));
        }
        let rate = self.engine.sample_rate.round() as u32;
        let (units, instruments) = if hosts_plugins(project) {
            if !self.plugins.render_ready(
                cantus_host::RenderFor::Listen,
                project,
                f64::from(rate),
                false,
            ) {
                return Err(ToolError::Failed(
                    "Cantus is loading the plug-ins' render instances; ask again in a few seconds"
                        .into(),
                ));
            }
            (
                self.plugins.render_units(project, f64::from(rate)),
                self.plugins.render_instruments(project, f64::from(rate)),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let snapshot = self.engine.compile_for_render(project, self.bus.revision());
        Ok(RenderInputs {
            snapshot: (*snapshot).clone(),
            units,
            instruments,
            sample_rate: rate,
        })
    }

    fn listen_service(&mut self) -> &mut Service {
        &mut self.listen
    }

    fn listen_epoch(&self) -> u64 {
        self.epoch
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A host with no window: the project's snapshot as `Snapshot::compile` makes it (no
    /// samples to attach), and a service writing into a folder of its own.
    pub struct TestHost {
        pub epoch: u64,
        pub bus: cantus_core::CommandBus,
        pub service: Service,
        pub dir: Option<PathBuf>,
    }

    impl TestHost {
        pub fn new(name: &str, project: Project) -> TestHost {
            let dir =
                std::env::temp_dir().join(format!("cantus-listen-{name}-{}", std::process::id()));
            TestHost {
                epoch: 0,
                bus: cantus_core::CommandBus::new(project),
                service: Service::new(dir.join("cache")),
                dir: Some(dir),
            }
        }
    }

    impl ToolHost for TestHost {
        fn bus(&self) -> &cantus_core::CommandBus {
            &self.bus
        }
        fn bus_mut(&mut self) -> &mut cantus_core::CommandBus {
            &mut self.bus
        }
        fn project_dir(&self) -> Option<PathBuf> {
            self.dir.clone()
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn a_files_link_is_a_valid_uri_whatever_the_folder_is_called() {
        assert_eq!(
            file_uri(Path::new(r"C:\Users\Luis Ángel\cache\a#1.wav")),
            "file:///C:/Users/Luis%20%C3%81ngel/cache/a%231.wav"
        );
        assert_eq!(
            file_uri(Path::new("/home/luis/my song/x.wav")),
            "file:///home/luis/my%20song/x.wav"
        );
    }

    impl ListenHost for TestHost {
        fn listen_inputs(&mut self, project: &Project) -> Result<RenderInputs, ToolError> {
            Ok(RenderInputs {
                snapshot: Snapshot::compile(project, 1),
                units: Vec::new(),
                instruments: Vec::new(),
                sample_rate: 48_000,
            })
        }
        fn listen_service(&mut self) -> &mut Service {
            &mut self.service
        }
        fn listen_epoch(&self) -> u64 {
            self.epoch
        }
    }
}
