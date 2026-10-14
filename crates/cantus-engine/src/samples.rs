//! Loading sample files for the kit's sample pads, and attaching them to snapshots.
//!
//! The audio thread cannot read a file, and `cantus-core` does no I/O, so this is the one
//! place a pad's path becomes audio. [`SampleCache::attach`] fills in
//! `TrackSnapshot::samples` — the same snapshot then serves real-time playback and offline
//! render, which is what keeps the two bit-identical without a second code path.
//!
//! **The cache never evicts.** Every `Arc<SampleData>` the audio thread holds has a sibling
//! here, so dropping one there is an atomic decrement and never a free (ADR 0005). A
//! session's worth of drum samples is megabytes; the rule is cheap and it is what makes
//! the audio thread's handling of `Arc`s safe by construction rather than by care.

use cantus_core::{SampleData, Snapshot};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "resample.rs"]
pub mod resample;

#[path = "samples_assets.rs"]
pub mod assets;

use assets::{AssetLoader, Want};

/// What became of a path the project points at.
#[derive(Clone, Debug, PartialEq)]
pub enum SampleStatus {
    Loaded {
        rate: u32,
        channels: u8,
        frames: usize,
    },
    /// The file could not be read; the pad is silent and the inspector says why.
    Failed(String),
    /// Never asked for — a path no snapshot has referenced yet.
    Unknown,
    /// Being decoded and converted on the loader thread; plays silence until it is in.
    Loading,
}

#[derive(Debug, Default)]
pub struct SampleCache {
    loaded: HashMap<PathBuf, Arc<SampleData>>,
    failed: HashMap<PathBuf, String>,
    /// Exemplar's octave mip-maps of loaded files (never evicted either).
    mips: crate::inst::sampler::MipCache,
    /// Audio clips' sources (ADR 0025 §9), decoded and converted to a rate, by the
    /// resolved path and that rate. Never evicted, like the rest.
    audio: HashMap<(PathBuf, u32), Arc<SampleData>>,
    /// The folder a source's relative path is relative to: the project file's.
    project_dir: Option<PathBuf>,
    /// The loader thread, when audio is loaded in the background ([`Self::set_background`]).
    loader: Option<Loader>,
    /// Exemplar's streamed files (ADR 0050): each a stream's head, one per file.
    streamed: HashMap<PathBuf, Arc<SampleData>>,
    /// Streams no snapshot asks for any more, released once nothing holds them (never on
    /// the audio thread: only when this is the last holder — `stream::release`).
    graveyard: Vec<(PathBuf, Arc<SampleData>)>,
    /// What loads had to say once (the cache could not be written…), for the status line.
    notes: Vec<crate::inst::stream::LoadNote>,
    /// Loads arrived through [`Self::wait_assets`]: the next [`Self::poll_loaded`] says so,
    /// so the live engine gets them too (review 4, 15, 26).
    arrived: bool,
    /// The background loader was busy at the last poll: when it goes quiet, the cache
    /// folder is tidied (its size limit, gone sources) beside the window.
    was_busy: bool,
    /// Whether Exemplar streams long samples ([`Self::set_streaming`]).
    streaming: bool,
    /// Instruments' files loaded in the background (with [`Self::set_background`]).
    assets: Option<AssetLoader>,
    /// The zones' starts past their heads in the last snapshot attached, the board that
    /// keeps them ready (second review #16), and until when a snapshot waits for them.
    starts: Starts,
}

use cantus_dsp::exemplar::stream::StartPin;

/// Zone starts kept ready ([`SampleCache::unready_tracks`]).
#[derive(Debug, Default)]
struct Starts {
    pins: Vec<StartPin>,
    board: Option<Arc<cantus_dsp::exemplar::stream::Board>>,
    by_track: Vec<(cantus_core::TrackId, Vec<StartPin>)>,
}

/// An audio source to load: its resolved path and the rate to convert it to.
type AudioKey = (PathBuf, u32);
type Loaded = (AudioKey, Result<SampleData, String>);

/// A thread that decodes and converts audio sources off the UI thread (a three-minute
/// file at another rate takes about a second): jobs in, results out, and what is on
/// its way so nothing is asked for twice.
#[derive(Debug)]
struct Loader {
    jobs: std::sync::mpsc::Sender<AudioKey>,
    done: std::sync::mpsc::Receiver<Loaded>,
    pending: std::collections::HashSet<AudioKey>,
    /// Sources asked for since the loader was last idle (the progress' total).
    asked: usize,
}

impl Loader {
    fn spawn() -> Option<Loader> {
        let (jobs, rx) = std::sync::mpsc::channel::<AudioKey>();
        let (tx, done) = std::sync::mpsc::channel::<Loaded>();
        std::thread::Builder::new()
            .name("cantus-audio-loader".into())
            .spawn(move || {
                while let Ok(key) = rx.recv() {
                    let r = decode_audio(&key.0, key.1);
                    if tx.send((key, r)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Loader {
            jobs,
            done,
            pending: Default::default(),
            asked: 0,
        })
    }
}

/// Decode an audio source (WAV, AIFF, FLAC, Ogg; at most two channels) and convert it
/// to `rate` (`0` or its own rate: untouched).
fn decode_audio(path: &Path, rate: u32) -> Result<SampleData, String> {
    crate::inst::sampler::decode::decode(path)
        .unwrap_or_else(|| decode_wav(path))
        .map(|d| {
            if d.rate == rate || rate == 0 {
                d
            } else {
                let ch = d.channels.max(1) as usize;
                SampleData {
                    stream: None,
                    rate,
                    channels: d.channels,
                    frames: resample::resample(&d.frames, ch, d.rate, rate).into(),
                }
            }
        })
}

impl SampleCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a WAV, or hand back the copy already loaded. Failures are remembered too, so
    /// a missing file is reported once rather than retried on every snapshot.
    pub fn load(&mut self, path: &Path) -> Result<Arc<SampleData>, String> {
        if let Some(s) = self.loaded.get(path) {
            return Ok(s.clone());
        }
        if let Some(e) = self.failed.get(path) {
            return Err(e.clone());
        }
        // AIFF, FLAC and Ogg by extension (Exemplar's decoders); everything else as WAV.
        let decoded =
            crate::inst::sampler::decode::decode(path).unwrap_or_else(|| decode_wav(path));
        match decoded {
            Ok(data) => {
                let data = Arc::new(data);
                self.loaded.insert(path.to_path_buf(), data.clone());
                Ok(data)
            }
            Err(e) => {
                self.failed.insert(path.to_path_buf(), e.clone());
                Err(e)
            }
        }
    }

    /// Forget a failure, so the next snapshot tries the file again — for when the user
    /// has just put it back.
    pub fn retry(&mut self, path: &Path) {
        self.failed.remove(path);
    }

    /// [`Self::retry`] for an audio source's stored path.
    pub fn retry_audio(&mut self, path: &str) {
        let p = self.resolve(path);
        self.failed.remove(&p);
    }

    pub fn status(&self, path: &Path) -> SampleStatus {
        let audio = || {
            self.audio
                .iter()
                .find(|((p, _), _)| p.as_path() == path)
                .map(|(_, s)| s)
        };
        if let Some(s) = self
            .loaded
            .get(path)
            .or_else(audio)
            .or_else(|| self.streamed.get(path))
        {
            // A stream's head stands for the whole file (ADR 0050).
            let frames = crate::inst::stream::lookup(s).map_or(s.len_frames(), |st| st.frames[0]);
            SampleStatus::Loaded {
                rate: s.rate,
                channels: s.channels,
                frames,
            }
        } else if self.assets.as_ref().is_some_and(|l| l.is_pending(path)) {
            SampleStatus::Loading
        } else if let Some(e) = self.failed.get(path) {
            SampleStatus::Failed(e.clone())
        } else {
            SampleStatus::Unknown
        }
    }

    /// Fill in every kit's `samples`, aligned with its pads. Returns the paths that could
    /// not be loaded, for the status line.
    pub fn attach(&mut self, snapshot: &mut Snapshot) -> Vec<(PathBuf, String)> {
        let mut errors = Vec::new();
        self.drop_failed();
        // Every native instrument's files: what is not here yet is loaded — in parallel,
        // here, or queued for the loader threads (a live engine; ADR 0050 §7), which a
        // later snapshot picks up. Meanwhile those zones are silent.
        let wanted: std::collections::HashSet<Want> = snapshot
            .tracks
            .iter()
            .filter_map(|t| t.instrument.native())
            .flat_map(|n| self.wants_of(n).into_iter().map(|(_, w)| w))
            .collect();
        self.bury(&wanted);
        let mut missing: Vec<Want> = wanted
            .iter()
            .filter(|w| !self.held(w) && !self.failed.contains_key(&w.path))
            .cloned()
            .collect();
        missing.sort_by(|a, b| a.path.cmp(&b.path));
        if let Some(l) = self.assets.as_mut() {
            l.want(&wanted, &missing);
        } else if !missing.is_empty() {
            let done = assets::load_all(&missing);
            self.keep(done);
        }
        let mut pins = Vec::new();
        for track in &mut snapshot.tracks {
            let id = track.id;
            // A native instrument's assets, by key (ADR 0023).
            track.assets = match track.instrument.native() {
                Some(n) => {
                    let mut assets: Vec<_> = self
                        .wants_of(n)
                        .into_iter()
                        .map(|(key, w)| {
                            let audio = match self.have(&w) {
                                Some(d) => Some(d.clone()),
                                None => {
                                    if let Some(e) = self.failed.get(&w.path) {
                                        errors.push((w.path.clone(), e.clone()));
                                    }
                                    None
                                }
                            };
                            (key, audio)
                        })
                        .collect();
                    if n.kind == cantus_core::InstrumentKind::Sampler {
                        crate::inst::sampler::attach_mips(n, &mut assets, &mut self.mips);
                        if self.streaming {
                            let p = crate::inst::sampler::start_pins(n, &assets);
                            if !p.is_empty() {
                                pins.push((id, p));
                            }
                        }
                    }
                    assets
                }
                None => Vec::new(),
            };
            let Some(kit) = track.instrument.pulsus() else {
                track.samples.clear();
                continue;
            };
            // One per pad, then every pad's layers in pad order (Pulsus II).
            track.samples = kit
                .pads
                .iter()
                .map(|pad| pad.sample.as_deref())
                .chain(kit.layer_paths().map(Some))
                .map(|path| {
                    let path = Path::new(path?);
                    match self.load(path) {
                        Ok(data) => Some(data),
                        Err(e) => {
                            errors.push((path.to_path_buf(), e));
                            None
                        }
                    }
                })
                .collect();
        }
        self.keep_starts_ready(pins);
        errors
    }

    /// The zones' starts past their heads, by track, handed to the reader at once (second
    /// review #16); nothing waits here (third review #6) — only the zone tables of the
    /// tracks whose starts are not read yet wait ([`Self::unready_tracks`]).
    fn keep_starts_ready(&mut self, by_track: Vec<(cantus_core::TrackId, Vec<StartPin>)>) {
        let mut pins: Vec<StartPin> = by_track.iter().flat_map(|(_, p)| p.clone()).collect();
        pins.sort_by_key(|p| (p.stream, p.block, p.count, p.levels));
        pins.dedup();
        if pins != self.starts.pins {
            self.starts.board =
                (!pins.is_empty()).then(|| crate::inst::stream::hub().keep_ready(&pins));
            self.starts.pins = pins;
        }
        self.starts.by_track = by_track;
    }

    /// The tracks of the last snapshot attached whose zones' starts are not all in the
    /// pool yet (fourth review #2: their zone tables wait, nothing else). Never waits.
    pub fn unready_tracks(&self) -> Vec<cantus_core::TrackId> {
        let hub = crate::inst::stream::hub();
        self.starts
            .by_track
            .iter()
            .filter(|(_, p)| !hub.pins_resident(p))
            .map(|(t, _)| *t)
            .collect()
    }

    /// Whether every zone start of the last snapshot attached is read.
    pub fn starts_ready(&mut self) -> bool {
        self.unready_tracks().is_empty()
    }

    /// Wait (at most `patience`) until [`Self::starts_ready`] — tests and tools that hold
    /// the snapshot themselves.
    pub fn wait_starts(&mut self, patience: std::time::Duration) {
        let t = std::time::Instant::now();
        while !self.starts_ready() && t.elapsed() < patience {
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    }

    /// The folder the project file is in, which a source's relative path is relative to
    /// (ADR 0025 §2); `None` for a project never saved. Sources loaded from where the old
    /// folder pointed stay cached; the next `attach_audio` resolves against the new one.
    pub fn set_project_dir(&mut self, dir: Option<PathBuf>) {
        self.project_dir = dir;
    }

    pub fn project_dir(&self) -> Option<&Path> {
        self.project_dir.as_deref()
    }

    /// Where a source's stored path points: itself when absolute, else under the
    /// project's folder (or the working directory, before the project has one).
    pub fn resolve(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        match &self.project_dir {
            Some(dir) if p.is_relative() => dir.join(p),
            _ => p.to_path_buf(),
        }
    }

    /// An audio clip's source at `rate`: decoded (WAV, AIFF, FLAC, Ogg), at most two
    /// channels, converted to `rate` with a windowed sinc (`resample`), and kept for the
    /// session. A file already at `rate` is its own samples, untouched. Failures are
    /// remembered like the kit's, until [`Self::retry`].
    pub fn load_audio(&mut self, path: &str, rate: u32) -> Result<Arc<SampleData>, String> {
        let resolved = self.resolve(path);
        let key = (resolved, rate);
        if let Some(s) = self.audio.get(&key) {
            return Ok(s.clone());
        }
        if let Some(e) = self.failed.get(&key.0) {
            return Err(e.clone());
        }
        if let Some(l) = self.loader.as_mut() {
            // In the background: silence until it arrives ([`Self::poll_loaded`]).
            if l.pending.insert(key.clone()) {
                if l.jobs.send(key.clone()).is_err() {
                    l.pending.remove(&key);
                } else {
                    l.asked += 1;
                    return Err(LOADING.into());
                }
            } else {
                return Err(LOADING.into());
            }
        }
        let decoded = decode_audio(&key.0, rate);
        match decoded {
            Ok(data) => {
                let data = Arc::new(data);
                self.audio.insert(key, data.clone());
                Ok(data)
            }
            Err(e) => {
                self.failed.insert(key.0, e.clone());
                Err(e)
            }
        }
    }

    /// What became of an audio source's stored path (resolved as [`Self::load_audio`]
    /// resolves it).
    pub fn audio_status(&self, path: &str) -> SampleStatus {
        let p = self.resolve(path);
        if self
            .loader
            .as_ref()
            .is_some_and(|l| l.pending.iter().any(|(q, _)| *q == p))
        {
            return SampleStatus::Loading;
        }
        self.status(&p)
    }

    /// Load audio sources on a thread of their own (a live engine: the UI thread must not
    /// stop for a long file), or here, at once (tests, offline renders). In the
    /// background a source not yet loaded plays silence; [`Self::poll_loaded`] says when
    /// one arrived, and the next snapshot has it.
    pub fn set_background(&mut self, on: bool) {
        match (on, self.loader.is_some()) {
            (true, false) => self.loader = Loader::spawn(),
            (false, true) => self.loader = None,
            _ => {}
        }
        match (on, self.assets.is_some()) {
            (true, false) => self.assets = AssetLoader::spawn(),
            (false, true) => {
                self.wait_assets();
                self.assets = None;
            }
            _ => {}
        }
    }

    /// Stream Exemplar's long samples from disk (ADR 0050): a live engine does; tests and
    /// tools load whole files unless they ask.
    pub fn set_streaming(&mut self, on: bool) {
        self.streaming = on;
    }

    pub fn streaming(&self) -> bool {
        self.streaming
    }

    /// Whether files load beside the caller ([`Self::set_background`]).
    pub fn background(&self) -> bool {
        self.assets.is_some()
    }

    /// Files loading in the background — instruments' and audio clips' sources (second
    /// review #13) — done of asked for (ADR 0050 §7); `None` when nothing loads.
    pub fn progress(&self) -> Option<(usize, usize)> {
        let assets = self.assets.as_ref().and_then(AssetLoader::progress);
        let audio = self
            .loader
            .as_ref()
            .filter(|l| !l.pending.is_empty())
            .map(|l| (l.asked.saturating_sub(l.pending.len()), l.asked));
        match (assets, audio) {
            (None, None) => None,
            (a, b) => {
                let (a, b) = (a.unwrap_or((0, 0)), b.unwrap_or((0, 0)));
                Some((a.0 + b.0, a.1 + b.1))
            }
        }
    }

    /// Wait for every instrument file on its way (an offline render waits for the loads,
    /// so it renders what playing will). Returns whether any arrived.
    pub fn wait_assets(&mut self) -> bool {
        let done = match self.assets.as_mut() {
            Some(l) => l.wait(),
            None => return false,
        };
        let any = !done.is_empty();
        self.keep(done);
        self.arrived |= any;
        any
    }

    /// A file held whole (never decoded here): the editor's waveform uses it when there.
    pub fn whole(&self, path: &Path) -> Option<Arc<SampleData>> {
        self.loaded.get(path).cloned()
    }

    /// What loads had to say since the last call (once each), for the status line.
    pub fn take_notes(&mut self) -> Vec<crate::inst::stream::LoadNote> {
        let mut n = std::mem::take(&mut self.notes);
        n.extend(crate::inst::stream::hub().take_notes());
        n
    }

    /// Streamed files whose streams are alive in this cache (tests, the memory readout).
    pub fn streamed_files(&self) -> usize {
        self.streamed.len()
    }

    /// Let go of streams no snapshot uses (`attach` buries them; they go when nothing —
    /// a snapshot the engine still plays, a voice — holds them any more).
    fn bury(&mut self, wanted: &std::collections::HashSet<Want>) {
        let gone: Vec<PathBuf> = self
            .streamed
            .keys()
            .filter(|p| !wanted.iter().any(|w| &w.path == *p))
            .cloned()
            .collect();
        for p in gone {
            if let Some(h) = self.streamed.remove(&p) {
                self.graveyard.push((p, h));
            }
        }
        // Asked for again: back from the graveyard, no load (never a failed stream).
        for w in wanted.iter().filter(|w| w.stream) {
            if !self.streamed.contains_key(&w.path) {
                if let Some(i) = self
                    .graveyard
                    .iter()
                    .position(|(p, h)| *p == w.path && !crate::inst::stream::failed(h))
                {
                    let (p, h) = self.graveyard.swap_remove(i);
                    self.streamed.insert(p, h);
                }
            }
        }
        self.sweep_graveyard();
    }

    /// Streams that failed (neither their cache file nor their source could be read — second
    /// review #3): let go, so the next attach loads the file again (and reports it missing
    /// when it still cannot be read).
    fn drop_failed(&mut self) {
        let failed: Vec<PathBuf> = self
            .streamed
            .iter()
            .filter(|(_, h)| crate::inst::stream::failed(h))
            .map(|(p, _)| p.clone())
            .collect();
        for p in failed {
            if let Some(h) = self.streamed.remove(&p) {
                self.graveyard.push((p, h));
            }
        }
    }

    fn sweep_graveyard(&mut self) {
        let mut keep = Vec::new();
        for (p, h) in self.graveyard.drain(..) {
            if let Err(h) = crate::inst::stream::release(h) {
                keep.push((p, h));
            }
        }
        self.graveyard = keep;
    }

    /// Put what loaded where `attach` finds it.
    fn keep(&mut self, done: Vec<assets::Done>) {
        for (w, r) in done {
            match r {
                Ok((d, note)) => {
                    if let Some(n) = note {
                        self.notes.push(n);
                    }
                    if d.stream.is_some() {
                        if let Some(old) = self.streamed.insert(w.path.clone(), d) {
                            // The file changed: its old stream goes when nothing holds it.
                            self.graveyard.push((w.path, old));
                        }
                    } else {
                        self.loaded.insert(w.path, d);
                    }
                }
                Err(e) => {
                    self.failed.insert(w.path, e);
                }
            }
        }
    }

    /// The audio to give a zone that wants `w`: its own kind when there, else the other kind
    /// already held for the file — an edit that changes how a file is wanted (a loop
    /// switched on) keeps it playing while the other kind loads (review 13).
    fn have(&self, w: &Want) -> Option<&Arc<SampleData>> {
        let (a, b) = if w.stream {
            (self.streamed.get(&w.path), self.loaded.get(&w.path))
        } else {
            (self.loaded.get(&w.path), self.streamed.get(&w.path))
        };
        a.or(b)
    }

    /// Whether `w` is held as it is wanted (a whole file also serves a streamable want).
    fn held(&self, w: &Want) -> bool {
        self.loaded.contains_key(&w.path) || (w.stream && self.streamed.contains_key(&w.path))
    }

    /// What a native instrument asks for: each asset key with its file, and whether it may
    /// stream.
    fn wants_of(&self, n: &cantus_core::NativeSpec) -> Vec<(String, Want)> {
        let files = if self.streaming && n.kind == cantus_core::InstrumentKind::Sampler {
            crate::inst::sampler::stream_files(n)
        } else {
            Vec::new()
        };
        n.assets()
            .map(|(key, path)| {
                let stream = files.iter().any(|(p, ok)| p == path && *ok);
                (
                    key.to_string(),
                    Want {
                        path: PathBuf::from(path),
                        stream,
                    },
                )
            })
            .collect()
    }

    /// Take in what the loader finished. Returns whether anything arrived (loaded or
    /// failed), so the caller recompiles the snapshot.
    pub fn poll_loaded(&mut self) -> bool {
        let done = self
            .assets
            .as_mut()
            .map(AssetLoader::take)
            .unwrap_or_default();
        let mut any = !done.is_empty() || std::mem::take(&mut self.arrived);
        // A stream that failed: the next snapshot loads its file again.
        any |= self
            .streamed
            .values()
            .any(|h| crate::inst::stream::failed(h));
        self.keep(done);
        self.sweep_graveyard();
        let busy = self.assets.as_ref().is_some_and(AssetLoader::busy);
        if self.was_busy && !busy && self.streaming {
            let _ = std::thread::Builder::new()
                .name("cantus-cache-sweep".into())
                .spawn(|| {
                    let _ = crate::inst::stream::hub().sweep();
                });
        }
        self.was_busy = busy;
        let Some(l) = self.loader.as_mut() else {
            return any;
        };
        while let Ok((key, r)) = l.done.try_recv() {
            l.pending.remove(&key);
            if l.pending.is_empty() {
                l.asked = 0;
            }
            any = true;
            match r {
                Ok(d) => {
                    self.audio.insert(key, Arc::new(d));
                }
                Err(e) => {
                    self.failed.insert(key.0, e);
                }
            }
        }
        any
    }

    /// Whether any source is still being loaded in the background.
    pub fn loading(&self) -> bool {
        self.loader.as_ref().is_some_and(|l| !l.pending.is_empty())
            || self.assets.as_ref().is_some_and(AssetLoader::busy)
    }

    /// Fill in every audio clip's `data` at the engine's `rate`. A missing or unreadable
    /// file leaves the clip silent and is returned, for the status line: a project must
    /// never fail to play because a file moved.
    pub fn attach_audio(&mut self, snapshot: &mut Snapshot, rate: u32) -> Vec<(PathBuf, String)> {
        let mut errors: Vec<(PathBuf, String)> = Vec::new();
        for track in &mut snapshot.tracks {
            for clip in &mut track.audio {
                clip.data = match self.load_audio(&clip.path, rate) {
                    Ok(d) => Some(d),
                    // Not an error: it is on its way.
                    Err(e) if e == LOADING => None,
                    Err(e) => {
                        let p = self.resolve(&clip.path);
                        if !errors.iter().any(|(q, _)| *q == p) {
                            errors.push((p, e));
                        }
                        None
                    }
                };
            }
        }
        errors
    }
}

/// What `load_audio` says while a source is on the loader thread.
const LOADING: &str = "loading";

/// Make every attached audio clip's data play at `rate`: a snapshot attached for the
/// device's rate, rendered offline at another, has its sources converted here (once per
/// source), off the audio thread. Clips already at `rate` are left as they are, so a
/// bounce at the device's rate plays exactly what was heard.
pub fn conform_audio(snapshot: &mut Snapshot, rate: u32) {
    let mut done: Vec<(*const SampleData, Arc<SampleData>)> = Vec::new();
    for track in &mut snapshot.tracks {
        for clip in &mut track.audio {
            let Some(d) = clip.data.as_ref().filter(|d| d.rate != rate && rate > 0) else {
                continue;
            };
            let key = Arc::as_ptr(d);
            let converted = match done.iter().find(|(k, _)| *k == key) {
                Some((_, c)) => c.clone(),
                None => {
                    let ch = d.channels.max(1) as usize;
                    let c = Arc::new(SampleData {
                        stream: None,
                        rate,
                        channels: d.channels,
                        frames: resample::resample(&d.frames, ch, d.rate, rate).into(),
                    });
                    done.push((key, c.clone()));
                    c
                }
            };
            clip.data = Some(converted);
        }
    }
}

/// Decode a WAV to interleaved `f32`, keeping at most two channels.
pub(crate) fn decode_wav(path: &Path) -> Result<SampleData, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let in_channels = spec.channels.max(1) as usize;
    let out_channels = in_channels.min(2);
    let mut frames: Vec<f32> = Vec::new();
    let mut push = |i: usize, v: f32| {
        if i % in_channels < out_channels {
            frames.push(v);
        }
    };
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for (i, s) in reader.samples::<f32>().enumerate() {
                push(i, s.map_err(|e| e.to_string())?);
            }
        }
        hound::SampleFormat::Int => {
            let scale = 1.0 / ((1i64 << (spec.bits_per_sample - 1)) as f32);
            for (i, s) in reader.samples::<i32>().enumerate() {
                push(i, s.map_err(|e| e.to_string())? as f32 * scale);
            }
        }
    }
    if frames.is_empty() {
        return Err("the file holds no audio".to_string());
    }
    Ok(SampleData {
        stream: None,
        rate: spec.sample_rate,
        channels: out_channels as u8,
        frames: frames.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &Path, channels: u16, rate: u32, frames: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..frames {
            for c in 0..channels {
                let v = (cantus_dsp::math::sin(i as f32 * 0.05 + c as f32) * 16_000.0) as i16;
                w.write_sample(v).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn loads_once_and_remembers_failures() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok.wav");
        write_wav(&ok, 2, 44_100, 1_000);
        let mut cache = SampleCache::new();
        let a = cache.load(&ok).unwrap();
        let b = cache.load(&ok).unwrap();
        assert!(
            Arc::ptr_eq(&a, &b),
            "the second load must be the same buffer"
        );
        assert_eq!(a.channels, 2);
        assert_eq!(a.rate, 44_100);
        assert_eq!(a.len_frames(), 1_000);
        assert!(matches!(
            cache.status(&ok),
            SampleStatus::Loaded { frames: 1_000, .. }
        ));

        let missing = dir.path().join("missing.wav");
        assert!(cache.load(&missing).is_err());
        assert!(matches!(cache.status(&missing), SampleStatus::Failed(_)));
        assert_eq!(cache.status(Path::new("never")), SampleStatus::Unknown);
    }

    #[test]
    fn a_five_channel_file_keeps_its_first_two() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("surround.wav");
        write_wav(&p, 5, 48_000, 200);
        let s = SampleCache::new().load(&p).unwrap();
        assert_eq!(s.channels, 2);
        assert_eq!(s.len_frames(), 200);
    }
}
