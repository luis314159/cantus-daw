//! The dancers, live on the interface (round 35, ADR 0087): what the stage, the
//! companion window and the dancer's own window draw.
//!
//! **One source.** For every track hosting a dancer the engine runs a unit that publishes
//! a [`DanceFrame`] at the end of every segment (`cantus_engine::dance`); its feed was
//! taken when the unit was made and sent (`EngineHandle::dance_feed`). Once a screen frame
//! ([`frame`], from the app's frame) this reads each feed once, keeps a short history,
//! and poses the dancer at the position **heard** (ADR 0022) — the frame of the segment
//! that held that moment, moved to it — so what dances is what is heard, not what the
//! engine is rendering a buffer ahead. A dancer whose unit has not arrived (no audio
//! device, the first frame) dances from the transport and its own numbers alone.
//!
//! **Its character** comes from the `character` text key: `builtin:<id>` from the cast,
//! `file:<path>` from the project's folder (an import), each loaded once and cached by id;
//! one that cannot be found draws the default character and says so
//! ([`LiveDancer::missing`]), never an error.
//!
//! A project switch forgets every history; a track that no longer hosts a dancer drops
//! its feed. Then [`DanceLive::dancers`] is the API the stage draws from.

use crate::app::CantusApp;
use cantus_core::inst::dancer::{self, CHARACTER_KEY};
use cantus_core::{InstrumentKind, NativeSpec, Ticks, TrackId, PPQ};
use cantus_dance::character::Character;
use cantus_dance::pose::{pose_at, Pose};
use cantus_engine::dance::{DanceFeed, DanceFrame, VALUES};
use std::collections::{HashMap, VecDeque};
use std::path::Path;

// Imported characters, read by a worker (`dance_live_load.rs`).
#[path = "dance_live_load.rs"]
pub mod load_worker;
use std::sync::Arc;

/// Frames kept per dancer: at a frame a block, more than a second of history — further
/// back than any output latency the heard position lies behind.
pub const HISTORY: usize = 128;
/// How far the shown position runs on from the last heard one between the engine's
/// reports (seconds), so the motion is smooth at any screen rate.
pub const RUN_ON: f64 = 0.06;

/// One dancer as the screen draws it now.
#[derive(Clone)]
pub struct LiveDancer {
    pub track: TrackId,
    pub character: Arc<dyn Character>,
    pub pose: Pose,
    /// The frame it was posed from (moved to the heard position): the song's place, the
    /// numbers after automation and modulation.
    pub frame: DanceFrame,
    /// The `character` it asked for when that could not be found (the default draws), and
    /// why, in words.
    pub missing: Option<String>,
    pub why: Option<String>,
    /// An imported character still being read (the default dances meanwhile).
    pub loading: bool,
    /// Where it shows (`view.mode`: 0 floating, 1 docked, 2 hidden) and whether on top.
    pub view_mode: u8,
    pub on_top: bool,
    /// From the engine's feed (`false`: from the transport alone).
    pub live: bool,
}

impl std::fmt::Debug for LiveDancer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveDancer")
            .field("track", &self.track)
            .field("character", &self.character.info().id)
            .field("pose", &self.pose)
            .field("missing", &self.missing)
            .field("view_mode", &self.view_mode)
            .field("live", &self.live)
            .finish()
    }
}

/// One dancer's feed and the frames read from it.
#[derive(Debug)]
struct Track {
    track: TrackId,
    feed: Option<Arc<DanceFeed>>,
    seen: u64,
    history: VecDeque<DanceFrame>,
}

/// The shown song position: the heard one, run on smoothly between the engine's reports
/// and never stepping back a hair (a seek or a loop wrap still jumps).
#[derive(Debug, Default)]
struct Clock {
    raw: f64,
    raw_at: f64,
    shown: f64,
    playing: bool,
}

impl Clock {
    fn heard(&mut self, raw: f64, playing: bool, tempo: f64, wall: f64) -> f64 {
        if !playing {
            *self = Clock {
                raw,
                raw_at: wall,
                shown: raw,
                playing,
            };
            return raw;
        }
        if raw != self.raw || !self.playing {
            self.raw = raw;
            self.raw_at = wall;
        }
        let run = (wall - self.raw_at).clamp(0.0, RUN_ON) * tempo / 60.0;
        let mut shown = raw + run;
        if self.playing && shown < self.shown && self.shown - shown < 0.25 {
            shown = self.shown;
        }
        self.shown = shown;
        self.playing = true;
        shown
    }
}

/// What the interface keeps of the dancers between frames.
#[derive(Default)]
pub struct DanceLive {
    tracks: Vec<Track>,
    /// The cast's characters by id, kept: the character, or why there is none.
    characters: HashMap<String, Result<Arc<dyn Character>, String>>,
    /// Imported ones, read off this thread and asked for again now and then.
    loads: load_worker::Loads,
    clock: Clock,
    epoch: Option<u64>,
    dancers: Vec<LiveDancer>,
    /// Frames this has read from the feeds (the tests and the stage's measure).
    reads: u64,
    /// The wall clock of the last update (what the picker's previews dance to).
    wall: f64,
    /// The breath's clock, which stops while nothing happens (round 38).
    pub doze: Doze,
    /// The last update read a note on a dancer's feed (a key held, a new move-map note):
    /// played live from a MIDI keyboard it is something happening, as the pointer is.
    stirred: bool,
    /// What a warm frame reuses (it allocates nothing): the dancers' specs and the tempo map
    /// as of the project's revision, the feeds' buffer, the default character.
    revision: Option<u64>,
    specs: Vec<(TrackId, NativeSpec)>,
    tempo_map: Option<cantus_core::TempoMap>,
    feeds: Vec<Option<Arc<DanceFeed>>>,
    default: Option<Arc<dyn Character>>,
}

impl std::fmt::Debug for DanceLive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DanceLive")
            .field("tracks", &self.tracks.len())
            .field("characters", &self.characters.len())
            .field("imports", &self.loads.len())
            .field("dancers", &self.dancers)
            .finish()
    }
}

/// The seconds since the process started: the idle breath's wall clock.
pub fn wall_seconds() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
}

/// How long a dancer goes on breathing, stopped, once nothing happens (round 38, the
/// scorecard): then it holds still and the window draws nothing for it until something
/// does — the pointer over the window, a key, Play, a drag. Each breath redrew the whole
/// window (16 points of a core at rest, measured); a dancer that holds costs nothing.
pub const DOZE_AFTER: f64 = 10.0;

/// The breath's clock: the wall clock with the time spent holding still taken out, so a
/// dancer that wakes goes on from the pose it held — never a jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Doze {
    /// How long it breathes once nothing happens ([`DOZE_AFTER`]; the tests shorten it).
    pub after: f64,
    /// The wall clock when something last happened.
    last_active: f64,
    /// Seconds spent holding still, in all.
    held: f64,
    /// Since when it holds still, if it does.
    since: Option<f64>,
}

impl Default for Doze {
    fn default() -> Self {
        Doze {
            after: DOZE_AFTER,
            last_active: 0.0,
            held: 0.0,
            since: None,
        }
    }
}

impl Doze {
    /// Something happened at `wall`: it breathes again (from the pose it held).
    pub fn wake(&mut self, wall: f64) {
        if let Some(s) = self.since.take() {
            self.held += (wall - s).max(0.0);
        }
        self.last_active = wall;
    }

    /// The breath's clock at `wall`: still while it holds, which it starts to
    /// [`Self::after`] seconds after the last thing that happened.
    pub fn clock(&mut self, wall: f64) -> f64 {
        if self.since.is_none() && wall - self.last_active >= self.after {
            self.since = Some(self.last_active + self.after);
        }
        match self.since {
            Some(s) => s - self.held,
            None => wall - self.held,
        }
    }

    /// It holds still: the window asks no frame for its breath.
    pub fn holding(&self) -> bool {
        self.since.is_some()
    }
}

/// Once a screen frame, from the app's frame (before the views draw): the dancers posed at
/// what is heard, on the breath's clock — which stops once nothing has happened for
/// [`DOZE_AFTER`] seconds ([`Doze`]). Not while the companion window is open: what it
/// streams keeps breathing.
pub fn frame_main(app: &mut CantusApp, ctx: &egui::Context) {
    let wall = wall_seconds();
    let now = ctx.input(|i| i.time);
    let happening = app.engine.is_playing()
        || app.settings.stage.companion_open
        || app.stage.drag.is_some()
        // A hover lingers a moment after the pointer left (the frame's fade), then is over.
        || app.stage.hovered(now).is_some()
        || ctx.input(|i| !i.events.is_empty() || i.pointer.is_moving() || i.pointer.any_down());
    if happening || app.dance_live.stirred {
        app.dance_live.doze.wake(wall);
    }
    let clock = app.dance_live.doze.clock(wall);
    DanceLive::update(app, clock);
    // A note read now wakes it at once (this frame was posed on the held clock; the next one
    // breathes, and the breath asks for it).
    if app.dance_live.stirred {
        app.dance_live.doze.wake(wall);
    }
}

/// Once a frame of the companion's own (the main window minimised): the breath's clock as
/// the main window left it.
pub fn frame(app: &mut CantusApp) {
    let wall = wall_seconds();
    app.dance_live.doze.wake(wall);
    let clock = app.dance_live.doze.clock(wall);
    DanceLive::update(app, clock);
}

impl DanceLive {
    /// Every dancer as the screen draws it now, in track order.
    pub fn dancers(app: &CantusApp) -> Vec<LiveDancer> {
        app.dance_live.dancers.clone()
    }

    /// Frames read from the feeds since it was made.
    pub fn reads(&self) -> u64 {
        self.reads
    }

    /// The wall clock (seconds) of the last update.
    pub fn wall(&self) -> f64 {
        self.wall
    }

    /// Read every dancer's feed once and pose it at the heard position; `wall` is the wall
    /// clock in seconds (the idle breath while stopped, the run-on between reports).
    pub fn update(app: &mut CantusApp, wall: f64) {
        let epoch = app.epoch;
        let playing = app.engine.is_playing();
        let heard_ticks = app.engine.position();
        // The dancers' specs and the tempo map, read again only when the project changed.
        let revision = app.bus.revision();
        if app.dance_live.revision != Some(revision) || app.dance_live.epoch != Some(epoch) {
            let specs: Vec<(TrackId, NativeSpec)> = app
                .project()
                .tracks
                .iter()
                .filter_map(|t| {
                    let n = t.instrument.native()?;
                    (n.kind == InstrumentKind::Dancer).then(|| (t.id, n.clone()))
                })
                .collect();
            let map = (!specs.is_empty()).then(|| app.project().tempo_map());
            let live = &mut app.dance_live;
            live.specs = specs;
            live.tempo_map = map;
            live.revision = Some(revision);
        }
        app.dance_live.stirred = false;
        if app.dance_live.specs.is_empty() {
            app.dance_live.tracks.clear();
            app.dance_live.dancers.clear();
            app.dance_live.wall = wall;
            app.dance_live.epoch = Some(epoch);
            return;
        }
        // An import's folder (the saved project's, or the untitled folder before the first
        // save, ADR 0089) — asked only when a dancer shows an import.
        let imports = app.dance_live.specs.iter().any(|(_, s)| {
            s.text(CHARACTER_KEY)
                .is_some_and(|c| c.starts_with("file:"))
        });
        let project_dir = if imports {
            crate::dance_import::project_dir(app)
        } else {
            std::path::PathBuf::new()
        };
        let mut feeds = std::mem::take(&mut app.dance_live.feeds);
        feeds.clear();
        feeds.extend(
            app.dance_live
                .specs
                .iter()
                .map(|(t, _)| app.engine.dance_feed(*t)),
        );
        let live = &mut app.dance_live;
        let still = match live.tempo_map.as_ref() {
            Some(map) => transport_frame(map, heard_ticks, playing),
            None => DanceFrame::default(),
        };
        if live.epoch != Some(epoch) {
            // Another project: nothing of the last one's dancers carries over.
            live.tracks.clear();
            live.characters.clear();
            live.loads.clear();
            live.epoch = Some(epoch);
        }
        let specs = std::mem::take(&mut live.specs);
        live.tracks
            .retain(|t| specs.iter().any(|(id, _)| *id == t.track));
        let tempo = f64::from(still.tempo_bpm);
        let heard = live
            .clock
            .heard(heard_ticks.as_beats_f64(), playing, tempo, wall);
        let mut out = std::mem::take(&mut live.dancers);
        out.clear();
        for ((track, spec), feed) in specs.iter().zip(feeds.drain(..)) {
            let state = match live.tracks.iter().position(|t| t.track == *track) {
                Some(i) => &mut live.tracks[i],
                None => {
                    live.tracks.push(Track {
                        track: *track,
                        feed: None,
                        seen: 0,
                        history: VecDeque::with_capacity(HISTORY),
                    });
                    live.tracks.last_mut().expect("just pushed")
                }
            };
            if !same_feed(&state.feed, &feed) {
                state.feed = feed;
                state.seen = 0;
                state.history.clear();
            }
            if let Some(f) = state.feed.as_ref() {
                let n = f.published();
                if n != state.seen {
                    if let Some(frame) = f.latest() {
                        let note_was = state.history.back().and_then(|b| b.note);
                        if frame.held != [0; 2] || frame.note != note_was {
                            live.stirred = true;
                        }
                        if state.history.len() == HISTORY {
                            state.history.pop_front();
                        }
                        state.history.push_back(frame);
                        state.seen = n;
                        live.reads += 1;
                    }
                }
            }
            let (frame, is_live) = match heard_frame(&state.history, heard, playing) {
                Some(f) => (f, true),
                None => (with_values(still, spec, heard), false),
            };
            let id = spec
                .text(CHARACTER_KEY)
                .unwrap_or(dancer::DEFAULT_CHARACTER);
            // The import re-check runs on the real wall clock, as `character` asks it (the
            // breath's clock stands still while it holds: two clocks would re-ask every frame).
            let r = live.resolve(id, &project_dir, wall_seconds());
            let pose = pose_at(&frame, r.character.as_ref(), wall);
            out.push(LiveDancer {
                track: *track,
                character: r.character,
                pose,
                frame,
                missing: r.missing,
                why: r.why,
                loading: r.loading,
                view_mode: spec.get("view.mode").round().clamp(0.0, 2.0) as u8,
                on_top: spec.get("view.top") >= 0.5,
                live: is_live,
            });
        }
        live.specs = specs;
        live.feeds = feeds;
        live.dancers = out;
        live.wall = wall;
    }

    /// The character for an id now: the cast's, kept by id; an import, read by the worker
    /// (the default dances while it loads) and asked for again every
    /// [`load_worker::RECHECK`] seconds — never kept for good; one that cannot be found
    /// draws the default and says why.
    pub fn resolve(&mut self, id: &str, project_dir: &Path, wall: f64) -> Resolved {
        let default = self
            .default
            .get_or_insert_with(cantus_dance::cast::default_character)
            .clone();
        let fallback = |missing: Option<String>, why: Option<String>, loading| Resolved {
            character: default.clone(),
            missing,
            why,
            loading,
        };
        if id.starts_with("file:") {
            return match self.loads.get(id, project_dir, wall) {
                load_worker::Got::Ready(c) => Resolved {
                    character: c,
                    missing: None,
                    why: None,
                    loading: false,
                },
                load_worker::Got::Loading => fallback(None, None, true),
                load_worker::Got::Failed(e) => fallback(Some(id.to_string()), Some(e), false),
            };
        }
        if !self.characters.contains_key(id) {
            let got = load(id, project_dir);
            self.characters.insert(id.to_string(), got);
        }
        match self
            .characters
            .get(id)
            .cloned()
            .unwrap_or_else(|| Err(String::new()))
        {
            Ok(c) => Resolved {
                character: c,
                missing: None,
                why: None,
                loading: false,
            },
            Err(e) => fallback(Some(id.to_string()), Some(e), false),
        }
    }

    /// [`Self::resolve`] as a pair: the character, and the id asked for when it is not
    /// the one drawing (missing, or still loading).
    pub fn character(
        &mut self,
        id: &str,
        project_dir: &Path,
    ) -> (Arc<dyn Character>, Option<String>) {
        let r = self.resolve(id, project_dir, wall_seconds());
        let not = (r.missing.is_some() || r.loading).then(|| id.to_string());
        (r.character, not)
    }

    /// Wait (at most `timeout`) for the imports being read: the tests'.
    pub fn wait_for_loads(&mut self, timeout: std::time::Duration) {
        self.loads.wait(timeout);
    }

    /// Imported characters read from their files so far (a re-check of unchanged files
    /// reads nothing).
    pub fn import_reads(&self) -> u64 {
        self.loads.reads()
    }
}

/// A character resolved for a frame.
pub struct Resolved {
    pub character: Arc<dyn Character>,
    pub missing: Option<String>,
    pub why: Option<String>,
    pub loading: bool,
}

/// A new instrument's spec as the picker makes it: a dancer names its character (the
/// cast's first), so the project says who dances whatever a later cast's default is.
pub fn new_spec(mut spec: NativeSpec) -> NativeSpec {
    if spec.kind == InstrumentKind::Dancer && spec.text(CHARACTER_KEY).is_none() {
        let _ = spec.set_text(CHARACTER_KEY, Some(dancer::DEFAULT_CHARACTER.to_string()));
    }
    spec
}

/// What a preset (or A/B) leaves of a dancer as it is: who dances and where it is shown —
/// its character, its place and size, whether it shows and where — so a preset changes
/// the dance, never the stage.
pub const KEPT_BY_PRESETS: &[&str] = &["x", "y", "size", "visible", "view.mode", "view.top"];

/// `target` with what a preset leaves of `now` (a dancer's) put back.
pub fn keep_through_preset(now: &NativeSpec, target: &NativeSpec) -> NativeSpec {
    let mut t = target.clone();
    if now.kind != InstrumentKind::Dancer {
        return t;
    }
    for k in KEPT_BY_PRESETS {
        match now.params.get(*k) {
            Some(v) => {
                t.params.insert((*k).to_string(), *v);
            }
            None => {
                t.params.remove(*k);
            }
        }
    }
    match now.text(CHARACTER_KEY) {
        Some(c) => {
            t.text.insert(CHARACTER_KEY.to_string(), c.to_string());
        }
        None => {
            t.text.remove(CHARACTER_KEY);
        }
    }
    t
}

/// What the unit catalogue adds for the dancer (`get_unit dancer`, ADR 0082): its move
/// map by key, its moves and moods by number and name, the characters included, and how
/// it reacts — so an agent can automate `move`, play notes and choose a character.
pub fn describe_for_agents(v: &mut serde_json::Value) {
    use cantus_core::inst::dancer::{Mood, Move, BASE_KEY};
    use serde_json::json;
    let en = |k: &str| crate::i18n::t_in(crate::i18n::Locale::En, k);
    v["note"] = json!("Makes no sound: a character that dances on screen. With move = 0 it dances its own routine to the beat (one move a bar); move = 1…16 plays Move number move − 1 below, from the bar line; a note on a key of the move map plays that move from the note's start (velocity: how big). react (0…1) hands that share of its energy to the level a follower modulator routes to `level` — the editor's React to makes one; character (text) is builtin:<id> or file:<path in the project>. view.mode and view.top are set, never automated.");
    v["move_map"] = Move::ALL
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let key = BASE_KEY + i as u8;
            json!({"key": key, "note": cantus_core::model::pitch_name(key), "move": m.name(), "name": en(m.label_key())})
        })
        .collect();
    v["moves"] = std::iter::once(json!({"move": 0, "name": "To the beat (its routine)"}))
        .chain(
            Move::ALL
                .iter()
                .enumerate()
                .map(|(i, m)| json!({"move": i + 1, "id": m.name(), "name": en(m.label_key())})),
        )
        .collect();
    v["moods"] = Mood::ALL
        .iter()
        .enumerate()
        .map(|(i, m)| json!({"mood": i, "id": m.name(), "name": en(m.label_key())}))
        .collect();
    v["characters"] = cantus_dance::cast::builtin()
        .iter()
        .map(|c| {
            let id = c.info().id.clone();
            let key = cantus_dance::cast::name_key(&id);
            let name = if crate::i18n::has(crate::i18n::Locale::En, &key) {
                en(&key)
            } else {
                c.info().name.clone()
            };
            json!({"character": id, "name": name})
        })
        .collect();
}

/// A character by its stored id: the cast's, or an import in the project's folder
/// (`cantus_dance::import::load`: sandboxed, its mapping and licence from the package).
pub fn load(id: &str, project_dir: &Path) -> Result<Arc<dyn Character>, String> {
    if id.starts_with("builtin:") {
        return cantus_dance::cast::by_id(id).ok_or_else(|| format!("no character {id}"));
    }
    if let Some(rel) = id.strip_prefix("file:") {
        return cantus_dance::import::load(project_dir, rel).map_err(|e| e.to_string());
    }
    Err(format!("not a character: {id}"))
}

fn same_feed(a: &Option<Arc<DanceFeed>>, b: &Option<Arc<DanceFeed>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// The frame of the segment that held the heard moment, moved to it. The history is cut
/// into runs where the song jumped back (a loop wrap, a seek): the newest run that holds
/// the moment is searched — right after a wrap the heard position is still in the pass
/// before, which the engine has left — and in it the frame published first after the
/// moment (the closest above), else the newest at or before it. Stopped, the newest as it
/// is. `None` before anything was read.
fn heard_frame(history: &VecDeque<DanceFrame>, heard: f64, playing: bool) -> Option<DanceFrame> {
    let newest = history.back()?;
    if !playing || !newest.playing {
        return Some(*newest);
    }
    let eps = 1e-9;
    let back = |i: usize| history[i].beats < history[i - 1].beats - eps;
    // Runs, newest first: `start..end`, cut where the song went back.
    let run_start = |end: usize| {
        let mut i = end - 1;
        while i > 0 && !back(i) {
            i -= 1;
        }
        i
    };
    let holds = |start: usize, end: usize| {
        let r = || history.range(start..end).filter(|f| f.playing);
        let first = r().map(|f| f.beats).fold(f64::INFINITY, f64::min);
        first <= heard + eps && r().any(|f| f.beats > heard + eps)
    };
    let newest_run = (run_start(history.len()), history.len());
    let mut chosen = newest_run;
    let mut end = history.len();
    while end > 0 {
        let start = run_start(end);
        if holds(start, end) {
            chosen = (start, end);
            break;
        }
        end = start;
    }
    let run = || history.range(chosen.0..chosen.1).filter(|f| f.playing);
    let above = run()
        .filter(|f| f.beats > heard + eps)
        .min_by(|a, b| a.beats.total_cmp(&b.beats));
    let below = || {
        run()
            .filter(|f| f.beats <= heard + eps)
            .max_by(|a, b| a.beats.total_cmp(&b.beats))
    };
    above.or_else(below).map(|f| f.at_beats(heard))
}

/// What the transport alone says at the heard position: the song's place, the tempo and
/// the bar from the project's map, the numbers at their defaults.
fn transport_frame(map: &cantus_core::TempoMap, at: Ticks, playing: bool) -> DanceFrame {
    let tick = at.0 as f64;
    let start = map.bar_start(at);
    let len = map.bar_end(at).0 - start.0;
    let (bar, _, _) = map.bbt(at);
    DanceFrame {
        beats: at.as_beats_f64(),
        seconds: map.seconds_at(tick),
        tempo_bpm: map.bpm_at(tick) as f32,
        playing,
        bar: (bar - 1) as i32,
        bar_start_beats: start.as_beats_f64(),
        bar_beats: (len as f64 / PPQ as f64) as f32,
        ..DanceFrame::default()
    }
}

/// A transport frame with a dancer's own numbers (no automation reaches it there).
fn with_values(mut f: DanceFrame, spec: &NativeSpec, beats: f64) -> DanceFrame {
    let mut values = [0.0; VALUES];
    for (v, k) in values.iter_mut().zip(dancer::KEYS) {
        *v = spec.get(k);
    }
    f.values = values;
    if f.playing {
        f = f.at_beats(beats);
    }
    f
}

/// A track's picture for a dancer (`TrackIcon::Dance`): a figure mid-dance — a head, one
/// arm up, one out, a leg kicked — drawn at the track icons' weight from their half-height.
pub fn paint_icon(
    p: &egui::Painter,
    c: egui::Pos2,
    h: f32,
    thin: egui::Stroke,
    color: egui::Color32,
) {
    use egui::pos2;
    let head = pos2(c.x + h * 0.12, c.y - h * 0.72);
    p.circle_filled(head, h * 0.26, color);
    let neck = pos2(c.x + h * 0.06, c.y - h * 0.40);
    let hip = pos2(c.x - h * 0.06, c.y + h * 0.22);
    p.line_segment([neck, hip], thin);
    // Arms: one flung up, one out and down.
    p.line_segment([neck, pos2(c.x + h * 0.78, c.y - h * 1.02)], thin);
    p.line_segment([neck, pos2(c.x - h * 0.72, c.y - h * 0.18)], thin);
    // Legs: one planted, one kicked out.
    p.line_segment([hip, pos2(c.x - h * 0.40, c.y + h * 1.0)], thin);
    p.line_segment([hip, pos2(c.x + h * 0.72, c.y + h * 0.62)], thin);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(beats: f64, pass: f32) -> DanceFrame {
        let mut f = DanceFrame {
            beats,
            seconds: beats * 0.5,
            playing: true,
            ..DanceFrame::default()
        };
        f.values[0] = pass;
        f
    }

    #[test]
    fn the_heard_frame_comes_from_this_pass_of_a_loop() {
        // A loop of two beats: the first pass's frames up to 1.9, then the second pass
        // from 0.1 — the heard position (1.2, behind by the output's latency) is still in
        // the first pass... and once the second pass reaches it, only its frames count.
        let mut h: VecDeque<DanceFrame> =
            (1..=19).map(|i| frame(f64::from(i) * 0.1, 1.0)).collect();
        assert_eq!(heard_frame(&h, 1.2, true).unwrap().values[0], 1.0);
        for i in 1..=14 {
            h.push_back(frame(f64::from(i) * 0.1, 2.0));
        }
        let f = heard_frame(&h, 1.25, true).unwrap();
        assert_eq!(f.values[0], 2.0, "the newest pass, not the one before");
        assert!((f.beats - 1.25).abs() < 1e-9);
        // Just after the wrap the heard moment is still in the first pass (the engine is
        // in the second): the first pass's frame holds it.
        assert_eq!(heard_frame(&h, 1.85, true).unwrap().values[0], 1.0);
    }
}

#[cfg(test)]
mod doze_tests {
    use super::Doze;

    #[test]
    fn it_holds_still_after_a_while_and_wakes_where_it_held() {
        let mut d = Doze::default();
        d.wake(1.0);
        assert_eq!(d.clock(5.0), 5.0, "breathing: the wall clock");
        assert!(!d.holding());
        assert_eq!(
            d.clock(1.0 + d.after + 2.0),
            1.0 + d.after,
            "held from `after` on"
        );
        assert!(d.holding());
        assert_eq!(d.clock(100.0), 1.0 + d.after, "still held");
        d.wake(100.0);
        assert!(!d.holding());
        assert_eq!(
            d.clock(100.0),
            1.0 + d.after,
            "it goes on from the pose it held"
        );
        assert_eq!(d.clock(101.5), 1.0 + d.after + 1.5, "and breathes on");
        // A clock that never goes back, whatever wakes it.
        let mut last = d.clock(101.5);
        for w in [102.0, 150.0, 150.5, 300.0] {
            if w == 150.5 {
                d.wake(w);
            }
            let c = d.clock(w);
            assert!(c >= last);
            last = c;
        }
    }
}
