//! *Split to strips* and *Merge back* in the application (Pulsus III, ADR 0100): the
//! actions the kit's ⋯ menu and the mixer's strip menu call, each one undo step.

use crate::app::CantusApp;
use crate::i18n::tf;
use cantus_core::padout::{self, Refusal};
use cantus_core::{EffectId, EffectKind, PadId, StripId, TrackId};

impl CantusApp {
    /// Give every pad of `track`'s kit that sounds a strip of its own, into the kit's strip
    /// (one undo step). `false`, with a notice, when there is nothing to split.
    pub fn split_kit(&mut self, track: TrackId) -> bool {
        match padout::split(&mut self.bus, track) {
            Ok(cmd) => self.exec(cmd),
            Err(r) => {
                self.refused(track, r, "padout.split_nothing");
                false
            }
        }
    }

    /// Every pad of `track`'s kit back on the kit's strip, and the strips the split made
    /// removed when nothing else uses them (one undo step).
    pub fn merge_kit(&mut self, track: TrackId) -> bool {
        match padout::merge(self.project(), track) {
            Ok(cmd) => self.exec(cmd),
            Err(r) => {
                self.refused(track, r, "padout.merge_nothing");
                false
            }
        }
    }

    /// The kit a mixer strip splits or merges: the track that feeds it, on its own strip
    /// or through one of its pads.
    pub fn kit_on_strip(&self, strip: StripId) -> Option<TrackId> {
        let p = self.project();
        p.tracks
            .iter()
            .find(|t| t.instrument.pulsus().is_some() && t.strip == Some(strip))
            .map(|t| t.id)
            .or_else(|| padout::kit_of_strip(p, strip))
    }

    /// Add an insert of `kind` (its starting preset) at the end of a pad's chain — the
    /// chain made with it when the pad has none (ADR 0100 §6). One undo step.
    pub fn add_to_pad_chain(&mut self, track: TrackId, pad: PadId, kind: EffectKind) -> bool {
        let slot = self.new_insert(kind);
        let project = self.project().clone();
        match padout::chain_add(&project, track, pad, slot, |n| {
            self.bus.new_strip(tf("padout.chain_name", &[("pad", &n)]))
        }) {
            Ok(cmd) => self.exec(cmd),
            Err(r) => {
                self.refused(track, r, "padout.chain_nothing");
                false
            }
        }
    }

    /// Take an insert out of a pad's chain (the last takes the chain with it). One undo
    /// step.
    pub fn remove_from_pad_chain(&mut self, track: TrackId, pad: PadId, effect: EffectId) -> bool {
        match padout::chain_remove(self.project(), track, pad, effect) {
            Ok(cmd) => self.exec(cmd),
            Err(r) => {
                self.refused(track, r, "padout.chain_nothing");
                false
            }
        }
    }

    fn refused(&mut self, track: TrackId, r: Refusal, nothing: &str) {
        let name = self
            .project()
            .track(track)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let key = match r {
            Refusal::NotAKit => "padout.not_a_kit",
            Refusal::NoPad => "padout.no_pad",
            Refusal::Nothing => nothing,
            Refusal::NoRoom => "padout.no_room", // r37:fixes
        };
        self.set_error(tf(key, &[("name", &name)]));
    }
}

// ---- Kits from outside (ADR 0100 §7) ---------------------------------------------------

/// The most audio files a folder dropped as a kit is read for, the most entries its walk
/// looks at, and the most bytes its files may hold: a kit, not a sample library.
pub const KIT_MAX_FILES: usize = 512;
pub const KIT_MAX_ENTRIES: usize = 8_192;
pub const KIT_MAX_BYTES: u64 = 4 << 30;

/// Why a folder was not read as a kit.
#[derive(Clone, Debug, PartialEq)]
pub enum Scan {
    Files(Vec<String>),
    /// More than [`KIT_MAX_FILES`] audio files, [`KIT_MAX_ENTRIES`] entries or
    /// [`KIT_MAX_BYTES`]: not a kit.
    TooBig,
}

/// The audio files of a folder and its subfolders (three deep), in path order: what a
/// folder dropped as a kit holds — bounded (see [`Scan::TooBig`]); `allow` judges each
/// file (an agent's file roots), a folder link included.
pub fn kit_files_within(dir: &std::path::Path, allow: &dyn Fn(&std::path::Path) -> bool) -> Scan {
    fn walk(
        dir: &std::path::Path,
        depth: usize,
        out: &mut Vec<std::path::PathBuf>,
        seen: &mut usize,
        bytes: &mut u64,
        allow: &dyn Fn(&std::path::Path) -> bool,
    ) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return true;
        };
        let mut entries: Vec<std::path::PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            *seen += 1;
            if *seen > KIT_MAX_ENTRIES {
                return false;
            }
            if !allow(&p) {
                continue;
            }
            if p.is_dir() {
                if depth < 3 && !walk(&p, depth + 1, out, seen, bytes, allow) {
                    return false;
                }
            } else if crate::audio_files::is_audio_path(&p) {
                *bytes += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                out.push(p);
                if out.len() > KIT_MAX_FILES || *bytes > KIT_MAX_BYTES {
                    return false;
                }
            }
        }
        true
    }
    let (mut out, mut seen, mut bytes) = (Vec::new(), 0usize, 0u64);
    if !walk(dir, 0, &mut out, &mut seen, &mut bytes, allow) {
        return Scan::TooBig;
    }
    Scan::Files(
        out.into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
    )
}

/// [`kit_files_within`] with every file allowed (the user's own drop).
pub fn kit_files(dir: &std::path::Path) -> Vec<String> {
    match kit_files_within(dir, &|_| true) {
        Scan::Files(f) => f,
        Scan::TooBig => Vec::new(),
    }
}

/// Whether some file's name says a drum (EN or ES): a folder of drums, not any folder.
pub fn looks_like_drums(files: &[String]) -> bool {
    files
        .iter()
        .any(|f| cantus_core::pads::import::role_of(&file_label(std::path::Path::new(f))).is_some())
}

/// The regions of an SFZ file, read as Exemplar reads them (includes beside it), with
/// their sample paths made absolute.
pub fn sfz_zones(path: &std::path::Path) -> Result<Vec<cantus_core::inst::sampler::Zone>, String> {
    sfz_zones_within(path, &|_| true).map(|(z, _)| z)
}

/// [`sfz_zones`] where `allow` judges every file it reads or names: an `#include` outside is
/// not read, a region whose sample lies outside is left out — counted (an agent's file
/// roots).
pub fn sfz_zones_within(
    path: &std::path::Path,
    allow: &dyn Fn(&std::path::Path) -> bool,
) -> Result<(Vec<cantus_core::inst::sampler::Zone>, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let base = path
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_path_buf();
    let inc = base.clone();
    let import = crate::views::inst::sampler::sfz::parse(
        &text,
        &base,
        &|rel| {
            let p = inc.join(rel);
            if allow(&p) {
                std::fs::read_to_string(p).ok()
            } else {
                None
            }
        },
        &|_| None,
    );
    let mut outside = 0usize;
    let zones = import
        .zones
        .into_iter()
        .filter_map(|mut z| {
            if let Some(s) = z.sample.as_mut() {
                let p = std::path::Path::new(s.as_str());
                if p.is_relative() {
                    *s = base.join(p).to_string_lossy().into_owned();
                }
                if !allow(std::path::Path::new(s.as_str())) {
                    outside += 1;
                    return None;
                }
            }
            Some(z)
        })
        .collect();
    Ok((zones, outside))
}

impl CantusApp {
    /// A folder of one-shots as a mapped kit (names parsed, takes as round robins, dynamics
    /// as velocity layers): on `track`'s kit, or on a new kit track named after the folder.
    /// One undo step. The new or the kit's track.
    pub fn make_kit_from_folder(
        &mut self,
        track: Option<TrackId>,
        dir: &std::path::Path,
    ) -> Option<TrackId> {
        let files = match kit_files_within(dir, &|_| true) {
            Scan::Files(f) => f,
            Scan::TooBig => {
                self.set_error(tf("padout.kit_too_big", &[("name", &file_label(dir))]));
                return None;
            }
        };
        if files.is_empty() {
            self.set_error(tf("padout.kit_empty", &[("name", &file_label(dir))]));
            return None;
        }
        let (kit, log) = cantus_core::pads::import::kit_from_files(&files);
        self.place_kit(track, &file_label(dir), kit, log)
    }

    /// An SFZ drum kit as a mapped kit, likewise. One undo step.
    pub fn make_kit_from_sfz(
        &mut self,
        track: Option<TrackId>,
        path: &std::path::Path,
    ) -> Option<TrackId> {
        let zones = match sfz_zones(path) {
            Ok(z) => z,
            Err(e) => {
                self.set_error(tf(
                    "padout.kit_unreadable",
                    &[("name", &file_label(path)), ("error", &e)],
                ));
                return None;
            }
        };
        let (kit, log) = cantus_core::pads::import::kit_from_zones(&zones);
        if kit.pads.is_empty() {
            self.set_error(tf("padout.kit_empty", &[("name", &file_label(path))]));
            return None;
        }
        self.place_kit(track, &file_label(path), kit, log)
    }

    pub(crate) fn place_kit(
        &mut self,
        track: Option<TrackId>,
        name: &str,
        kit: cantus_core::PulsusParams,
        log: Vec<String>,
    ) -> Option<TrackId> {
        let pads = kit.pads.len();
        let id = match track.filter(|t| self.project().track(*t).is_some()) {
            Some(t) => {
                self.set_instrument(t, cantus_core::InstrumentSpec::Pulsus(kit));
                t
            }
            None => {
                let mut t = self.bus.new_track(name);
                t.instrument = cantus_core::InstrumentSpec::Pulsus(kit);
                let id = t.id;
                let index = self.project().tracks.len();
                if !self.exec(cantus_core::Command::InsertTrack {
                    index,
                    track: Box::new(t),
                }) {
                    return None;
                }
                id
            }
        };
        for line in &log {
            log::info!("kit {name}: {line}");
        }
        let key = if log.is_empty() {
            "padout.kit_made"
        } else {
            "padout.kit_made_short"
        };
        self.set_status(tf(
            key,
            &[
                ("name", &name.to_string()),
                ("n", &pads.to_string()),
                ("left", &log.len().to_string()),
            ],
        ));
        Some(id)
    }
}

fn file_label(p: &std::path::Path) -> String {
    p.file_stem()
        .or_else(|| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A kit being read on its own thread, for the track it lands on (`None`: a new one).
pub struct PendingKit {
    pub onto: Option<TrackId>,
    pub name: String,
    rx: std::sync::mpsc::Receiver<Result<(cantus_core::PulsusParams, Vec<String>), String>>,
}

impl std::fmt::Debug for PendingKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingKit")
            .field("name", &self.name)
            .finish()
    }
}

/// A folder or an SFZ file dropped from the system: a kit — on the kit whose pads it lands
/// on, else on a new kit track — read on a thread of its own (a folder of thousands of files
/// never stalls the window) and landed by [`poll_kit`]. A folder dropped elsewhere than on a
/// kit's pads must say drums in some file's name; one too big to be a kit is refused.
/// `false` when the drop is neither a folder nor an SFZ.
pub fn drop_kit(
    app: &mut CantusApp,
    ctx: &egui::Context,
    path: &std::path::Path,
    pos: Option<egui::Pos2>,
) -> bool {
    let sfz = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sfz"));
    if !sfz && !path.is_dir() {
        return false;
    }
    let onto = pos
        .and_then(|p| crate::samples::drop::target_at(ctx, p))
        .and_then(|(_, t)| match t {
            crate::samples::drop::Target::Pad { track, .. } => Some(track),
            _ => None,
        });
    start_kit(app, onto, path.to_path_buf(), sfz);
    true
}

/// Read a folder or an SFZ as a kit on a thread of its own (see [`drop_kit`]).
pub fn start_kit(app: &mut CantusApp, onto: Option<TrackId>, path: std::path::PathBuf, sfz: bool) {
    let (tx, rx) = std::sync::mpsc::channel();
    let name = file_label(&path);
    let label = name.clone();
    std::thread::Builder::new()
        .name("cantus-kit-scan".into())
        .spawn(move || {
            let r = if sfz {
                sfz_zones(&path).map(|z| cantus_core::pads::import::kit_from_zones(&z))
            } else {
                match kit_files_within(&path, &|_| true) {
                    Scan::TooBig => Err(tf("padout.kit_too_big", &[("name", &label)])),
                    Scan::Files(f) if onto.is_none() && !looks_like_drums(&f) => {
                        Err(tf("padout.kit_not_drums", &[("name", &label)]))
                    }
                    Scan::Files(f) => Ok(cantus_core::pads::import::kit_from_files(&f)),
                }
            };
            let _ = tx.send(r);
        })
        .ok();
    app.pulsus.pending_kit = Some(PendingKit { onto, name, rx });
}

/// Land a kit read on its thread, once it is there (each frame; at once in a test that
/// waits). Whether one landed.
pub fn poll_kit(app: &mut CantusApp, wait: bool) -> bool {
    let Some(p) = app.pulsus.pending_kit.take() else {
        return false;
    };
    let got = if wait {
        p.rx.recv().ok()
    } else {
        match p.rx.try_recv() {
            Ok(r) => Some(r),
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                app.pulsus.pending_kit = Some(p);
                return false;
            }
            Err(_) => None,
        }
    };
    match got {
        Some(Ok((kit, log))) if !kit.pads.is_empty() => {
            app.place_kit(p.onto, &p.name, kit, log);
            true
        }
        Some(Ok(_)) => {
            app.set_error(tf("padout.kit_empty", &[("name", &p.name)]));
            false
        }
        Some(Err(e)) => {
            app.set_error(e);
            false
        }
        None => false,
    }
}
