//! Machine-learning models through the content manager (ADR 0085, after ADRs 0030 and
//! 0042): a manifest compiled into the binary (`manifest/models.json`) lists each model —
//! what it does, its licence, its provenance card, every file's size and SHA-256 — and the
//! inference runtime's library for each system. Nothing is in the repository or the
//! installer but this list and the models' graphs (no weights: `models/*.onnx.gz`).
//!
//! Installing a model, once its licence is accepted ([`Models::accept`]):
//!
//! 1. the runtime's library for this system, if it is not there yet, and the model's
//!    files, downloaded by the content manager into `<data dir>/Models/<id>/` — resumed
//!    after a cut, every byte's hash checked, a proxy's HTML page refused (ADR 0030);
//! 2. the model *prepared* on a worker thread: its graph written from the binary, its
//!    weights rebuilt from the maker's own file (`models_th`), both hashes checked, the
//!    maker's file removed;
//! 3. [`Models::ready`] says where everything is, for the job that runs it.
//!
//! Removing a model removes its folder, and the runtime's when no other model needs it.

use crate::fetch::{Net, Transport};
use crate::install::{Patience, Phase};
use crate::manager::{Manager, Status};
use crate::manifest::{self, Category, Credit, FileEntry, Manifest, Pack, Source, Text};
use crate::models_th::{self, WeightMap};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
// r34:transcribe: a maker's own ONNX graph, used as it is (Basic Pitch).
#[path = "models_onnx.rs"]
mod onnx;
pub use onnx::{apply as onnx_apply, OnnxPatch}; // r34:transcribe

/// The model manifest's format this build reads.
pub const FORMAT: u32 = 1;

/// The manifest bundled with this build.
pub const BUNDLED: &str = include_str!("../manifest/models.json");

/// The licences a model and its runtime may be under (ADR 0042, extended for models by
/// ADR 0085): permissive ones that allow redistribution and commercial use of what the
/// model makes. Never NC, ND or SA.
pub const LICENCES: [&str; 7] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "CC-BY-3.0",
    "CC-BY-4.0",
];

/// The systems a runtime is listed for (`platform()`).
pub const PLATFORMS: [&str; 4] = [
    "windows-x86_64",
    "linux-x86_64",
    "macos-aarch64",
    "macos-x86_64",
];

/// This system, as the manifest names it.
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// A model's graph, compiled in: `(name, gzip of the .onnx, its weight map)`.
pub const GRAPHS: [(&str, &[u8], &str); 2] = [
    (
        "htdemucs",
        include_bytes!("../models/htdemucs.onnx.gz"),
        include_str!("../models/htdemucs.map.json"),
    ),
    (
        "htdemucs_6s",
        include_bytes!("../models/htdemucs_6s.onnx.gz"),
        include_str!("../models/htdemucs_6s.map.json"),
    ),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    pub format: u32,
    pub updated: String,
    pub runtimes: Vec<Runtime>,
    pub models: Vec<Model>,
}

/// An inference runtime's library for one system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub id: String,
    /// `onnxruntime`.
    pub kind: String,
    pub version: String,
    /// One of [`PLATFORMS`].
    pub platform: String,
    /// The library, relative to the runtime's folder.
    pub library: String,
    /// The library's SHA-256 once unpacked: the job checks it before it loads it, so a
    /// file put in its place is never run (review 1 #5).
    pub library_sha256: String,
    pub licence: String,
    pub licence_url: String,
    pub licence_file: String,
    pub credits: Vec<Credit>,
    pub checked: String,
    pub installed_size: u64,
    pub source: Source,
    /// Where the file comes from and why that one, for the record.
    pub note: String,
    /// The graphics processor's provider this build carries (`coreml`), if any: none for
    /// a processor-only build, which is what Windows and Linux download (review 2 #11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    /// A song into its sources.
    Stems,
    /// Audio into notes.
    Transcription,
}

/// What a model's provenance card says (`research/ai/legal.md` §3–4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub authors: String,
    pub paper: String,
    pub code: String,
    /// Where the weights come from, as the maker publishes them.
    pub weights: Text,
    /// What it was trained on, as the maker says.
    pub training: Text,
    /// Its published quality.
    pub quality: Text,
    /// What Cantus does to the maker's files, and with what.
    pub conversion: Text,
}

/// How a model is made ready after its files arrive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Prepare {
    /// A demucs `.th`: the compiled-in graph `graph` written, its weights rebuilt from
    /// `th` (a file of the model's) by the graph's map.
    DemucsTh { th: String, graph: String },
    /// The maker's own ONNX graph, used as it is (Basic Pitch's `nmp.onnx`): `file` is one
    /// of the model's files, its hash checked when it arrived; preparing only checks it
    /// again and writes the marker. r34:transcribe
    Onnx {
        file: String,
        /// A same-length byte edit made to a copy of `file` (Onsets and Frames Drums: the
        /// output's gather made to take the hi-hat's trained pitch), its result's hash
        /// checked. r34:transcribe
        #[serde(default, skip_serializing_if = "Option::is_none")]
        patch: Option<OnnxPatch>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id: String,
    pub version: u32,
    pub name: Text,
    pub description: Text,
    pub task: Task,
    /// How the job drives it (`ht_demucs`).
    pub family: String,
    /// What it separates, in its output order.
    pub sources: Vec<String>,
    pub sample_rate: u32,
    pub segment: usize,
    /// The model offered first.
    #[serde(default)]
    pub recommended: bool,
    /// `onnxruntime`: the runtime it needs.
    pub runtime: String,
    pub licence: String,
    pub licence_url: String,
    pub licence_file: String,
    pub credits: Vec<Credit>,
    pub checked: String,
    pub provenance: Provenance,
    pub files: Vec<FileEntry>,
    pub prepare: Prepare,
    /// Bytes on disk once prepared.
    pub installed_size: u64,
}

impl Model {
    pub fn download_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// The disk an install needs at its peak: the maker's files beside what is rebuilt.
    pub fn space_needed(&self) -> u64 {
        self.download_size() + self.installed_size
    }
}

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn sha_ok(s: &str) -> bool {
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// The compiled-in graph called `name`: its gzip and its map.
pub fn graph(name: &str) -> Option<(&'static [u8], WeightMap)> {
    GRAPHS
        .iter()
        .find(|(n, _, _)| *n == name)
        .and_then(|(_, gz, map)| Some((*gz, serde_json::from_str(map).ok()?)))
}

impl ModelManifest {
    pub fn parse(text: &str) -> Result<ModelManifest, String> {
        let m: ModelManifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if m.format > FORMAT {
            return Err(format!(
                "model manifest format {} is newer than this Cantus reads",
                m.format
            ));
        }
        Ok(m)
    }

    pub fn bundled() -> ModelManifest {
        Self::parse(BUNDLED).expect("the bundled model manifest parses")
    }

    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    /// The runtime of `kind` for `platform`.
    pub fn runtime(&self, kind: &str, platform: &str) -> Option<&Runtime> {
        self.runtimes
            .iter()
            .find(|r| r.kind == kind && r.platform == platform)
    }

    /// Everything a model or runtime entry must hold. `https_only` is false for tests'
    /// manifests, which serve files from disk.
    pub fn validate(&self, https_only: bool) -> Result<(), String> {
        let url_ok =
            |u: &str| u.starts_with("https://") || (!https_only && u.starts_with("file://"));
        let mut ids = std::collections::HashSet::new();
        let licence = |id: &str, l: &str, url: &str| -> Result<(), String> {
            if !LICENCES.contains(&l) {
                return Err(format!("{id}: licence {l} is not one a model may be under"));
            }
            if !url.starts_with("https://") {
                return Err(format!("{id}: its licence's text must be linked"));
            }
            Ok(())
        };
        for r in &self.runtimes {
            if !id_ok(&r.id) || !ids.insert(r.id.clone()) {
                return Err(format!("runtime id {} is malformed or repeated", r.id));
            }
            if !PLATFORMS.contains(&r.platform.as_str()) {
                return Err(format!("{}: unknown system {}", r.id, r.platform));
            }
            licence(&r.id, &r.licence, &r.licence_url)?;
            manifest::check_path(&r.library).map_err(|e| format!("{}: {e}", r.id))?;
            if !sha_ok(&r.library_sha256) {
                return Err(format!("{}: its library's hash", r.id));
            }
            match &r.source {
                Source::Zip(z) | Source::TarGz(z) => {
                    if !url_ok(&z.url) || !sha_ok(&z.sha256) || z.size == 0 {
                        return Err(format!("{}: its archive's address, size or hash", r.id));
                    }
                    for want in [&r.library, &r.licence_file] {
                        if !crate::install::keeps(z, want) {
                            return Err(format!("{}: {want} is not kept from the archive", r.id));
                        }
                    }
                }
                Source::Files(_) => return Err(format!("{}: a runtime is one archive", r.id)),
            }
        }
        for m in &self.models {
            if !id_ok(&m.id) || !ids.insert(m.id.clone()) {
                return Err(format!("model id {} is malformed or repeated", m.id));
            }
            licence(&m.id, &m.licence, &m.licence_url)?;
            if m.sources.is_empty() || m.sample_rate == 0 || m.segment == 0 {
                return Err(format!("{}: its sources, rate or segment", m.id));
            }
            if m.files.is_empty() {
                return Err(format!("{}: no files", m.id));
            }
            for f in &m.files {
                manifest::check_path(&f.to).map_err(|e| format!("{}: {e}", m.id))?;
                if !url_ok(&f.url) || !sha_ok(&f.sha256) || f.size == 0 {
                    return Err(format!("{}: {}'s address, size or hash", m.id, f.to));
                }
            }
            if !m.files.iter().any(|f| f.to == m.licence_file) {
                return Err(format!("{}: its licence file is not among its files", m.id));
            }
            match &m.prepare {
                Prepare::DemucsTh { th, graph: g } => {
                    let file = m
                        .files
                        .iter()
                        .find(|f| &f.to == th)
                        .ok_or_else(|| format!("{}: {th} is not among its files", m.id))?;
                    let (_, map) = graph(g)
                        .ok_or_else(|| format!("{}: no graph called {g} in this build", m.id))?;
                    if map.source_sha256 != file.sha256 {
                        return Err(format!("{}: graph {g} was made from another file", m.id));
                    }
                }
                // r34:transcribe
                Prepare::Onnx { file, patch } => {
                    if !file.ends_with(".onnx") || !m.files.iter().any(|f| &f.to == file) {
                        return Err(format!("{}: {file} is not an .onnx among its files", m.id));
                    }
                    if let Some(p) = patch {
                        onnx::check_patch(p).map_err(|e| format!("{}: {e}", m.id))?;
                    }
                }
            }
            if PLATFORMS
                .iter()
                .any(|p| self.runtime(&m.runtime, p).is_none())
            {
                return Err(format!(
                    "{}: runtime {} is not listed for every system",
                    m.id, m.runtime
                ));
            }
        }
        Ok(())
    }

    /// The manifest the content manager installs from: one pack per runtime and model.
    /// (The category is the manager's, for sound packs; models never reach the browser.)
    fn packs(&self) -> Manifest {
        let mut packs = Vec::new();
        for r in &self.runtimes {
            packs.push(Pack {
                id: r.id.clone(),
                version: 1,
                name: Text {
                    en: format!("{} {}", r.kind, r.version),
                    es: String::new(),
                },
                description: Text {
                    en: r.note.clone(),
                    es: String::new(),
                },
                category: Category::Fx,
                recommended: false,
                licence: r.licence.clone(),
                licence_url: r.licence_url.clone(),
                licence_file: r.licence_file.clone(),
                credits: r.credits.clone(),
                checked: r.checked.clone(),
                installed_size: r.installed_size,
                source: r.source.clone(),
                attribution: None,
                instruments: Vec::new(),
                build: None,
            });
        }
        for m in &self.models {
            packs.push(Pack {
                id: m.id.clone(),
                version: m.version,
                name: m.name.clone(),
                description: m.description.clone(),
                category: Category::Fx,
                recommended: m.recommended,
                licence: m.licence.clone(),
                licence_url: m.licence_url.clone(),
                licence_file: m.licence_file.clone(),
                credits: m.credits.clone(),
                checked: m.checked.clone(),
                installed_size: m.space_needed(),
                source: Source::Files(m.files.clone()),
                attribution: None,
                instruments: Vec::new(),
                build: None,
            });
        }
        Manifest {
            format: manifest::FORMAT,
            updated: self.updated.clone(),
            packs,
        }
    }
}

/// Where a model stands.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelStatus {
    /// Not installed; whether its licence has been accepted.
    Absent {
        accepted: bool,
    },
    /// No runtime is listed for this system.
    Unavailable,
    /// Downloading (the runtime and the model together).
    Downloading {
        done: u64,
        total: u64,
    },
    /// Waiting to try again (a proxy holding the file, a dropped connection).
    Retrying {
        error: String,
    },
    /// Rebuilding the weights from the maker's file.
    Preparing,
    Ready,
    /// The last install failed (it can be tried again).
    Failed(String),
}

/// What a job needs to run a ready model.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadyModel {
    pub id: String,
    pub family: String,
    pub graph: PathBuf,
    pub library: PathBuf,
    /// The library's SHA-256, for the job to check before it loads it.
    pub library_sha256: String,
    /// The model's files and their SHA-256 (the graph, its weights), for the job to check.
    pub files: Vec<(PathBuf, String)>,
    pub sources: Vec<String>,
    pub sample_rate: u32,
    pub segment: usize,
    /// The model's name, version and licence, for the provenance of what it makes.
    pub name: String,
    pub version: u32,
    pub licence: String,
}

/// A licence the user accepted, kept in `<root>/accepted.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Accepted {
    pub model: String,
    pub licence: String,
    pub runtime_licence: String,
    /// Seconds since the Unix epoch.
    pub at: u64,
}

/// What [`Models::poll`] reports.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelEvent {
    Ready(String),
    Failed(String, String),
}

/// Written beside a prepared model: what was made, so a later start knows it is whole.
pub const MARKER: &str = "cantus-model.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Marker {
    id: String,
    version: u32,
    graph: String,
    graph_sha256: String,
    weights_size: u64,
    weights_sha256: String,
}

pub struct Models {
    manifest: ModelManifest,
    manager: Manager,
    root: PathBuf,
    platform: String,
    accepted: Vec<Accepted>,
    /// Models whose install was asked for and is not finished (runtime, files, prepare).
    wanted: Vec<String>,
    preparing: HashMap<String, Receiver<Result<(), String>>>,
    errors: HashMap<String, String>,
    /// Graphs beside the compiled-in ones (tests' small models).
    extra_graphs: Vec<(String, Vec<u8>, WeightMap)>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Models {
    /// The models under `root` (`<data dir>/Models`), with the real network.
    pub fn new(root: impl Into<PathBuf>, manifest: ModelManifest) -> Models {
        Self::with_transport(root, manifest, Arc::new(Net::new()), Patience::default())
    }

    pub fn with_transport(
        root: impl Into<PathBuf>,
        manifest: ModelManifest,
        transport: Arc<dyn Transport>,
        patience: Patience,
    ) -> Models {
        Self::with_graphs(root, manifest, transport, patience, Vec::new())
    }

    /// As [`Models::with_transport`], with graphs beside the compiled-in ones (tests'
    /// small models), known from the start (a preparing cut short resumes with them).
    pub fn with_graphs(
        root: impl Into<PathBuf>,
        manifest: ModelManifest,
        transport: Arc<dyn Transport>,
        patience: Patience,
        extra_graphs: Vec<(String, Vec<u8>, WeightMap)>,
    ) -> Models {
        let root = root.into();
        let manager = Manager::with_transport(&root, manifest.packs(), transport, patience);
        let accepted = std::fs::read(root.join("accepted.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut m = Models {
            manifest,
            manager,
            root,
            platform: platform(),
            accepted,
            wanted: Vec::new(),
            preparing: HashMap::new(),
            errors: HashMap::new(),
            extra_graphs,
        };
        // A model whose files arrived but whose preparing was cut short (Cantus closed,
        // a crash) is prepared now.
        let ids: Vec<String> = m.manifest.models.iter().map(|x| x.id.clone()).collect();
        for id in ids {
            if m.manager.is_installed(&id) && !m.prepared(&id) {
                if m.can_prepare(&id) {
                    m.start_prepare(&id);
                } else {
                    // Its weights are gone and the maker's file with them: it is not
                    // installed any more; an install downloads it again (review 1 #8).
                    let _ = m.manager.remove(&id);
                }
            }
        }
        m
    }

    /// A graph beside the compiled-in ones, by name: tests' small models. Models already
    /// downloaded but not prepared are prepared on the next [`Models::install`].
    pub fn add_graph(&mut self, name: &str, gz: Vec<u8>, map: WeightMap) {
        self.extra_graphs.push((name.to_string(), gz, map));
    }

    fn graph_for(&self, name: &str) -> Option<(Vec<u8>, WeightMap)> {
        self.extra_graphs
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, g, m)| (g.clone(), m.clone()))
            .or_else(|| graph(name).map(|(g, m)| (g.to_vec(), m)))
    }

    /// Answer as if this were `platform` (tests).
    pub fn set_platform(&mut self, platform: &str) {
        self.platform = platform.to_string();
    }

    pub fn manifest(&self) -> &ModelManifest {
        &self.manifest
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn runtime_of(&self, m: &Model) -> Option<&Runtime> {
        self.manifest.runtime(&m.runtime, &self.platform)
    }

    /// Whether `id`'s licence (and its runtime's) has been accepted as they are now.
    pub fn accepted(&self, id: &str) -> bool {
        let Some(m) = self.manifest.model(id) else {
            return false;
        };
        let rl = self
            .runtime_of(m)
            .map(|r| r.licence.clone())
            .unwrap_or_default();
        self.accepted
            .iter()
            .any(|a| a.model == id && a.licence == m.licence && a.runtime_licence == rl)
    }

    /// The user accepted `id`'s licence and its runtime's, as shown: remembered.
    pub fn accept(&mut self, id: &str) -> std::io::Result<()> {
        let m = self
            .manifest
            .model(id)
            .ok_or_else(|| std::io::Error::other("no such model"))?;
        let rl = self
            .runtime_of(m)
            .map(|r| r.licence.clone())
            .unwrap_or_default();
        let entry = Accepted {
            model: id.to_string(),
            licence: m.licence.clone(),
            runtime_licence: rl,
            at: now(),
        };
        self.accepted.retain(|a| a.model != id);
        self.accepted.push(entry);
        std::fs::create_dir_all(&self.root)?;
        let json = serde_json::to_vec_pretty(&self.accepted).map_err(std::io::Error::other)?;
        let tmp = self.root.join("accepted.json.part");
        std::fs::write(&tmp, json)?;
        std::fs::rename(tmp, self.root.join("accepted.json"))
    }

    /// Install `id` (and its runtime, if need be). Refused until its licence is accepted.
    pub fn install(&mut self, id: &str) -> Result<(), String> {
        let m = self.manifest.model(id).ok_or("no such model")?.clone();
        if !self.accepted(id) {
            return Err("its licence has not been accepted".into());
        }
        let r = self
            .runtime_of(&m)
            .ok_or("no runtime for this system")?
            .id
            .clone();
        self.errors.remove(id);
        if !self.wanted.iter().any(|w| w == id) {
            self.wanted.push(id.to_string());
        }
        if !self.manager.is_installed(&r) {
            self.manager.install(&r);
        }
        if !self.manager.is_installed(id) || self.prepared_version(id) != Some(m.version) {
            if self.manager.is_installed(id) && !self.prepared(id) && self.can_prepare(id) {
                self.start_prepare(id);
            } else {
                // Not there, an older version, or its files gone with the maker's: from
                // the maker again (review 1 #8).
                self.manager.install(id);
            }
        }
        Ok(())
    }

    /// Stop an install; what was downloaded is kept for the next try.
    pub fn cancel(&mut self, id: &str) {
        self.manager.cancel(id);
        if let Some(m) = self.manifest.model(id) {
            if let Some(r) = self.runtime_of(m).map(|r| r.id.clone()) {
                if !self.wanted.iter().any(|w| {
                    w != id
                        && self
                            .manifest
                            .model(w)
                            .is_some_and(|o| o.runtime == m.runtime)
                }) {
                    self.manager.cancel(&r);
                }
            }
        }
        self.wanted.retain(|w| w != id);
    }

    /// Remove `id`, and the runtime when no other installed model needs it.
    pub fn remove(&mut self, id: &str) -> std::io::Result<()> {
        if self.preparing.contains_key(id) {
            return Err(std::io::Error::other("the model is being prepared"));
        }
        self.manager.remove(id)?;
        self.wanted.retain(|w| w != id);
        let Some(m) = self.manifest.model(id).cloned() else {
            return Ok(());
        };
        let others = self
            .manifest
            .models
            .iter()
            .any(|o| o.id != id && o.runtime == m.runtime && self.manager.is_installed(&o.id));
        if !others {
            if let Some(r) = self.runtime_of(&m).map(|r| r.id.clone()) {
                if self.manager.is_installed(&r) {
                    self.manager.remove(&r)?;
                }
            }
        }
        Ok(())
    }
}

impl Models {
    fn marker(&self, id: &str) -> Option<Marker> {
        let b = std::fs::read(self.manager.pack_dir(id).join(MARKER)).ok()?;
        serde_json::from_slice(&b).ok()
    }

    fn prepared_version(&self, id: &str) -> Option<u32> {
        self.marker(id).map(|m| m.version)
    }

    /// The graph `m` runs and its weight map, as this build has them: what its files must
    /// hash to — never what a file on disk says (review 2 #9).
    fn map_of(&self, m: &Model) -> Option<(String, WeightMap)> {
        let g = match &m.prepare {
            Prepare::DemucsTh { graph: g, .. } => g,
            // r34:transcribe: the manifest this build carries holds the graph's hash.
            Prepare::Onnx { file, patch } => return onnx::map_of(m, file, patch.as_ref()),
        };
        let map = self
            .extra_graphs
            .iter()
            .find(|(n, _, _)| n == g)
            .map(|(_, _, map)| map.clone())
            .or_else(|| graph(g).map(|(_, map)| map))?;
        Some((g.clone(), map))
    }

    /// Whether `id` was prepared at its manifest's version: its marker, its graph and its
    /// weights at their size. Cheap — read at start and every frame; the hashes are the
    /// job's to check, before it loads anything (review 2 #15).
    fn prepared(&self, id: &str) -> bool {
        let (Some(m), Some(k)) = (self.manifest.model(id), self.marker(id)) else {
            return false;
        };
        let Some((g, map)) = self.map_of(m) else {
            return false;
        };
        let dir = self.manager.pack_dir(id);
        k.version == m.version
            && k.graph == g
            && dir.join(format!("{g}.onnx")).is_file()
            // r34:transcribe: a maker's ONNX graph holds its weights: its own size counts.
            && std::fs::metadata(dir.join(match m.prepare {
                Prepare::Onnx { .. } => format!("{g}.onnx"),
                _ => format!("{g}.weights"),
            }))
            .is_ok_and(|md| md.len() == map.weights_size)
    }

    /// Whether `id` can be prepared from what is on disk: the maker's file still there.
    fn can_prepare(&self, id: &str) -> bool {
        let Some(m) = self.manifest.model(id) else {
            return false;
        };
        let file = match &m.prepare {
            Prepare::DemucsTh { th, .. } => th,
            Prepare::Onnx { file, .. } => file, // r34:transcribe
        };
        self.manager.pack_dir(id).join(file).is_file()
    }

    fn start_prepare(&mut self, id: &str) {
        let Some(m) = self.manifest.model(id).cloned() else {
            return;
        };
        if self.preparing.contains_key(id) {
            return;
        }
        let dir = self.manager.pack_dir(id);
        let g = match &m.prepare {
            Prepare::DemucsTh { graph: g, .. } => g.clone(),
            Prepare::Onnx { .. } => String::new(), // r34:transcribe: no compiled-in graph
        };
        let graph = if g.is_empty() {
            Some((Vec::new(), onnx::no_map())) // r34:transcribe
        } else {
            self.graph_for(&g)
        };
        let Some(graph) = graph else {
            self.errors
                .insert(id.to_string(), format!("no graph called {g} in this build"));
            return;
        };
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name(format!("cantus-model-{id}"))
            .spawn(move || {
                let _ = tx.send(prepare(&m, &dir, graph));
            });
        if spawned.is_ok() {
            self.preparing.insert(id.to_string(), rx);
        } else {
            self.errors
                .insert(id.to_string(), "no thread to prepare it on".into());
        }
    }

    pub fn status(&self, id: &str) -> ModelStatus {
        let Some(m) = self.manifest.model(id) else {
            return ModelStatus::Unavailable;
        };
        let Some(r) = self.runtime_of(m) else {
            return ModelStatus::Unavailable;
        };
        if self.preparing.contains_key(id) {
            return ModelStatus::Preparing;
        }
        if let Some(e) = self.errors.get(id) {
            return ModelStatus::Failed(e.clone());
        }
        let busy: Vec<Option<Phase>> = [r.id.as_str(), id]
            .iter()
            .filter_map(|p| match self.manager.status(p) {
                Status::Busy(ph) => Some(ph),
                _ => None,
            })
            .collect();
        if !busy.is_empty() {
            for ph in busy.iter().flatten() {
                if let Phase::Retrying { error, .. } = ph {
                    return ModelStatus::Retrying {
                        error: error.clone(),
                    };
                }
            }
            let total = m.download_size()
                + if self.manager.is_installed(&r.id) {
                    0
                } else {
                    self.manager
                        .manifest()
                        .pack(&r.id)
                        .map_or(0, |p| p.download_size())
                };
            let done = [r.id.as_str(), id]
                .iter()
                .map(|p| match self.manager.status(p) {
                    Status::Busy(Some(Phase::Downloading { done, .. })) => done,
                    Status::Busy(Some(_)) | Status::Installed { .. } | Status::Update { .. } => {
                        self.manager
                            .manifest()
                            .pack(p)
                            .map_or(0, |x| x.download_size())
                    }
                    _ => 0,
                })
                .sum::<u64>()
                .min(total);
            return ModelStatus::Downloading { done, total };
        }
        if self.manager.is_installed(&r.id) && self.prepared(id) {
            return ModelStatus::Ready;
        }
        ModelStatus::Absent {
            accepted: self.accepted(id),
        }
    }

    /// Where a ready model's files are.
    pub fn ready(&self, id: &str) -> Option<ReadyModel> {
        if self.status(id) != ModelStatus::Ready {
            return None;
        }
        let m = self.manifest.model(id)?;
        let r = self.runtime_of(m)?;
        let (g, map) = self.map_of(m)?;
        let dir = self.manager.pack_dir(id);
        Some(ReadyModel {
            id: id.to_string(),
            family: m.family.clone(),
            graph: dir.join(format!("{g}.onnx")),
            library: self.manager.pack_dir(&r.id).join(&r.library),
            library_sha256: r.library_sha256.clone(),
            // The hashes this build was made with: the helper checks the files against
            // them before it loads anything.
            files: std::iter::once((dir.join(format!("{g}.onnx")), map.graph_sha256.clone()))
                // r34:transcribe: a maker's ONNX graph holds its weights; there is no file.
                .chain(
                    (!matches!(m.prepare, Prepare::Onnx { .. }))
                        .then(|| (dir.join(format!("{g}.weights")), map.weights_sha256.clone())),
                )
                .collect(),
            sources: m.sources.clone(),
            sample_rate: m.sample_rate,
            segment: m.segment,
            name: m.name.en.clone(),
            version: m.version,
            licence: m.licence.clone(),
        })
    }

    /// Take what the workers did. Call once a frame; never blocks.
    pub fn poll(&mut self) -> Vec<ModelEvent> {
        let mut out = Vec::new();
        for f in self.manager.poll() {
            if let Err(e) = &f.result {
                // A runtime's failure is every waiting model's.
                let ids: Vec<String> = self
                    .wanted
                    .iter()
                    .filter(|w| {
                        **w == f.id
                            || self
                                .manifest
                                .model(w)
                                .and_then(|m| self.runtime_of(m))
                                .is_some_and(|r| r.id == f.id)
                    })
                    .cloned()
                    .collect();
                for id in ids {
                    self.errors.insert(id.clone(), e.to_string());
                    self.wanted.retain(|w| *w != id);
                    out.push(ModelEvent::Failed(id, e.to_string()));
                }
            } else if self.manifest.model(&f.id).is_some() {
                self.start_prepare(&f.id);
            }
        }
        let done: Vec<(String, Result<(), String>)> = self
            .preparing
            .iter()
            .filter_map(|(id, rx)| rx.try_recv().ok().map(|r| (id.clone(), r)))
            .collect();
        for (id, r) in done {
            self.preparing.remove(&id);
            if let Err(e) = r {
                self.errors.insert(id.clone(), e.clone());
                self.wanted.retain(|w| *w != id);
                out.push(ModelEvent::Failed(id, e));
            }
        }
        // A wanted model is ready once its runtime and its own files are.
        let ready: Vec<String> = self
            .wanted
            .iter()
            .filter(|w| self.status(w) == ModelStatus::Ready)
            .cloned()
            .collect();
        for id in ready {
            self.wanted.retain(|w| *w != id);
            out.push(ModelEvent::Ready(id));
        }
        out
    }

    /// Block until nothing is downloading or preparing (tests, and the app on exit).
    pub fn wait_idle(&mut self, timeout: std::time::Duration) -> Vec<ModelEvent> {
        let t = std::time::Instant::now();
        let mut out = Vec::new();
        loop {
            out.extend(self.poll());
            if (!self.manager.busy() && self.preparing.is_empty()) || t.elapsed() > timeout {
                return out;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

/// Make a model whose files arrived ready to run: the graph written from the binary, the
/// weights rebuilt, the hashes checked, the maker's file removed, the marker written last.
/// `graph` is the gzip of the graph `m.prepare` names and its weight map.
pub fn prepare(m: &Model, dir: &Path, graph: (Vec<u8>, WeightMap)) -> Result<(), String> {
    let (gz, map) = graph;
    match &m.prepare {
        Prepare::DemucsTh { th, graph: g } => {
            let mut onnx = Vec::new();
            std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&gz[..]), &mut onnx)
                .map_err(|e| format!("the graph cannot be read: {e}"))?;
            let got = crate::fetch::hex(&<sha2::Sha256 as sha2::Digest>::digest(&onnx));
            if got != map.graph_sha256 {
                return Err(format!(
                    "graph {g} hashes to {got}, not {}",
                    map.graph_sha256
                ));
            }
            let marker = dir.join(MARKER);
            let _ = std::fs::remove_file(&marker);
            let graph_path = dir.join(format!("{g}.onnx"));
            let tmp = dir.join(format!("{g}.onnx.part"));
            std::fs::write(&tmp, &onnx).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &graph_path).map_err(|e| e.to_string())?;
            let weights = dir.join(format!("{g}.weights"));
            let source = dir.join(th);
            let have = std::fs::metadata(&weights).is_ok_and(|md| md.len() == map.weights_size)
                && crate::fetch::sha256_file(&weights).ok().as_deref()
                    == Some(map.weights_sha256.as_str());
            if !have {
                models_th::build_weights(&source, &map, &weights, &AtomicBool::new(false))?;
            }
            let k = Marker {
                id: m.id.clone(),
                version: m.version,
                graph: g.clone(),
                graph_sha256: map.graph_sha256.clone(),
                weights_size: map.weights_size,
                weights_sha256: map.weights_sha256.clone(),
            };
            let json = serde_json::to_vec_pretty(&k).map_err(|e| e.to_string())?;
            std::fs::write(&marker, json).map_err(|e| e.to_string())?;
            // The maker's file is not needed any more: its weights are in place.
            let _ = std::fs::remove_file(&source);
            Ok(())
        }
        Prepare::Onnx { file, patch } => onnx::prepare(m, dir, file, patch.as_ref()), // r34:transcribe
    }
}
