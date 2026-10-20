//! MIDI learn and controller mapping in the interface (round 29, ADR 0062).
//!
//! What lives here: the controller tap's interface half and its state (the per-mapping
//! takeover, the gesture, the activity), the user's **global map** (`controllers.json`
//! beside `keymap.toml`, read in the boot's Files stage), the learn mode and learning, and
//! the verbs the rest of the interface calls — [`CantusApp::learn_target`],
//! [`CantusApp::forget_target`], [`CantusApp::toggle_learn_mode`],
//! [`CantusApp::link_last_tweaked`]. Each frame [`CantusApp::controller_input`] reads what the
//! tap heard (`controllers_input.rs`), learns (`controllers_learn.rs`) and sends feedback
//! (`controllers_feedback.rs`).
//!
//! **Where a learnt mapping goes**: a target that means the same in every project (the
//! transport's actions, the tempo, the master, *the selected track*, *the focused device*)
//! to the global map, so play/stop works in every song; anything else to the project (a
//! command, one undo step). A message a project mapping takes is not looked up in the global
//! map: **the project comes first**.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::controller::{
    Control, ControllerMap, DeskParam, Focus, MapMode, Mapping, MappingId, MappingTarget,
    MidiSource, Takeover,
};
use cantus_core::{AutoTarget, Command, InstrumentSpec};
use cantus_engine::midi::{Heard, MidiOutputs, MidiTap};
use cantus_engine::MidiEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Which map a mapping is in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    /// The user's, in every project.
    Global,
    /// This project's.
    Project,
}

/// A mapping's key in the interface's state.
pub type Key = (Scope, MappingId);

/// A 14-bit pair's state on one port, channel and number: its halves, and whether its fine
/// half has ever come (a controller in 7-bit mode never sends it).
///
/// A half whose partner may follow is held, never driven half-made (review 3, #5, #9),
/// for at most [`PAIR_GAP`]. Which half a controller sends first is told by time (review
/// 4, #4, #11, #37): a coarse half held after a fine half alone takes as its own the fine
/// half that is the closer to it — the one before it (a fine-first pair; the next fine
/// half is then the next pair's) or the one after it (a coarse-first pair after a fine-only
/// update) — so pairs a millisecond or two apart are read whole, and the last one too.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Wide {
    pub msb: u8,
    pub lsb: u8,
    pub lsb_seen: bool,
    /// A coarse half waiting for its fine half (when it came): read alone with the fine
    /// half it holds (0, as MIDI resets it, or the fine half alone just before it) after
    /// [`PAIR_GAP`].
    pub pending: Option<Instant>,
    /// When the fine half alone just before the held coarse half came: the pair a
    /// fine-first controller sends, unless a closer fine half follows.
    pub pending_after: Option<Instant>,
    /// A fine half came alone and was read with the coarse half as it stands (a fine-only
    /// update), when: a coarse half right after it is held with it.
    pub lsb_at: Option<Instant>,
    /// The controller sends its fine half first: known after [`FINE_FIRST`] pairs in a row
    /// told so by time, none the other way.
    pub lsb_first: bool,
    /// Pairs told fine half first since the last told coarse half first.
    pub fine_first_seen: u8,
    /// A fine half of a controller that looks fine-first, waiting for its coarse half
    /// (never driven with the old coarse half, review 3, #56).
    pub pending_lsb: Option<Instant>,
}

/// A half this close to its partner is the same pair; a half alone this long is read
/// alone (a coarse half with a fine 0, as MIDI says).
pub const PAIR_GAP: std::time::Duration = std::time::Duration::from_millis(5);

/// Pairs seen fine half first, in a row, before a controller is taken as sending its fine
/// half first.
pub const FINE_FIRST: u8 = 3;

/// Why a mapping does nothing now (the pane's flag; `None` from
/// `CantusApp::mapping_state` when it is live, review 2, #49).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Idle {
    /// Its target is gone (a deleted track or device, a parameter the plug-in lacks).
    Missing,
    /// Learnt on another plug-in than the one there now (kept for an undo of the swap).
    OtherPlugin,
    /// Its plug-in is in the project but not loaded (not found, crashed, stopped).
    NotLoaded,
    /// Its sound is frozen: moves are refused until it is unfrozen.
    Frozen,
    /// Its port is not open here (the controller unplugged, another computer's).
    NotConnected,
    /// A strip's record arm with no track to arm: none with an input feeds the strip
    /// (review 4, #6).
    NoInput,
}

/// The pairs whose half has waited [`PAIR_GAP`] by `now` without its partner: each read
/// now — a coarse half with a fine 0, a fine-first controller's fine half with the coarse
/// half as it stands — (port, channel, number, value, when). A younger one waits for the
/// next frame (the controllers' wakes ask for it): a pair split between two frames is
/// never driven half-made (review 3, #5, #9).
pub(crate) fn wide_alone(
    wide: &mut HashMap<(u8, u8, u8), Wide>,
    now: Instant,
) -> Vec<(u8, u8, u8, u16, Instant)> {
    let old = |t: &Instant| now.saturating_duration_since(*t) >= PAIR_GAP;
    let mut out = Vec::new();
    for (&(port, ch, n), e) in wide.iter_mut() {
        if let Some(at) = e.pending.filter(old) {
            e.pending = None;
            // Held after a fine half and nothing closer came: that was its pair.
            if e.pending_after.take().is_some() {
                told_fine_first(e);
            }
            out.push((port, ch, n, u16::from(e.msb) << 7 | u16::from(e.lsb), at));
        }
        if let Some(at) = e.pending_lsb.filter(old) {
            e.pending_lsb = None;
            out.push((port, ch, n, u16::from(e.msb) << 7 | u16::from(e.lsb), at));
        }
    }
    out
}

/// A pair told fine half first: counted, and the controller known as such at
/// [`FINE_FIRST`].
fn told_fine_first(e: &mut Wide) {
    e.fine_first_seen = e.fine_first_seen.saturating_add(1);
    if e.fine_first_seen >= FINE_FIRST {
        e.lsb_first = true;
    }
}

/// A target armed to learn the next hardware control moved.
#[derive(Clone, Debug, PartialEq)]
pub struct Learning {
    pub target: MappingTarget,
    pub scope: Scope,
    /// Learnt again: the mapping whose source is replaced.
    pub replace: Option<MappingId>,
    /// What the control sent so far (its port and channel are the first message's).
    pub heard: Vec<(Control, u16)>,
    pub source: Option<(u8, u8)>,
    /// When the first message came.
    pub first: Option<Instant>,
    /// When it was armed: an armed learn that hears nothing is dropped after
    /// `controllers_learn::LEARN_TIMEOUT`, so a forgotten learn does not eat the next note.
    pub armed_at: Instant,
    /// The two positions the control it was armed from stands for (*From*, *To*): a band's
    /// solo LED (off ↔ that band), a scale key (as the scale ↔ its click), a lamp lit at 0
    /// (review 4, #46, #53). The mapping is made over them, and read as two states.
    pub range: Option<(f32, f32)>,
}

/// The controllers' state in the interface.
#[derive(Default)]
pub struct ControllerUi {
    /// The tap the MIDI ports tee into (set on the boot path; kept across rescans).
    pub tap: MidiTap,
    /// The user's global map, and its file.
    pub global: ControllerMap,
    pub global_path: Option<PathBuf>,
    /// The global file this session cannot save over (it could neither be read whole nor
    /// kept beside): each global edit says so (review 4, #28).
    pub global_unsaved: Option<PathBuf>,
    /// Ableton's MIDI Map mode: every learnable control outlined, a click arms it.
    pub learn_mode: bool,
    pub learning: Option<Learning>,
    /// Per mapping: takeover and toggles (core's arithmetic state).
    pub(crate) states: HashMap<Key, cantus_core::controller::DriveState>,
    /// When each mapping last moved something: the mappings view's activity dot.
    pub activity: HashMap<Key, Instant>,
    /// The gesture going on: the targets it moved, its undo key, its last message.
    pub gesture: crate::controllers_input::Gesture,
    /// Gestures begun so far: each one's key is its own.
    pub(crate) gestures: u64,
    /// The project these mappings' session state is about (`CantusApp::epoch`): another
    /// project in its place starts it again (review 3, #25).
    pub(crate) epoch: u64,
    /// The instruments' choices as their editors drew them (the pane's words, review 3).
    pub inst_words: crate::controllers_words::InstWords,
    /// The playhead last frame, and when a loop's wrap was last seen.
    pub(crate) prev_pos: Option<i64>,
    pub(crate) last_wrap: Option<Instant>,
    /// Whether the last snapshot was built with a recording pass's latch on.
    pub(crate) latching: bool,
    /// The filter last published, and what it was made from: the project's revision, the
    /// global map, the ports.
    /// The project's revision, the loaded hosted parameters' signature, the global map, the
    /// ports.
    pub(crate) published: Option<(u64, u64, ControllerMap, Vec<String>)>,
    /// MIDI out for feedback, and the value last sent per mapping.
    pub outputs: MidiOutputs,
    pub sent: HashMap<Key, u16>,
    /// Feedback just sent, by port: what comes back within a moment is its echo.
    pub echoes: Vec<(u8, [u8; 3], Instant)>,
    /// Feedback written here instead of to MIDI out (the tests' sink).
    pub feedback_log: Option<Vec<(String, Vec<u8>)>>,
    /// The coarse and fine halves of 14-bit controllers, per port, channel and number.
    pub(crate) wide: HashMap<(u8, u8, u8), Wide>,
    /// A frame's messages (kept to reuse the allocation).
    pub(crate) buf: Vec<Heard>,
    /// The mappings view's state.
    pub view: crate::views::controllers::ViewState,
}

impl std::fmt::Debug for ControllerUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControllerUi")
            .field("global", &self.global.mappings.len())
            .field("learn_mode", &self.learn_mode)
            .finish()
    }
}

impl ControllerUi {
    /// The ports were opened again (a rescan, an audio restart): their indices may have
    /// moved, the outputs are closed, the filter must be published again.
    pub fn reopened(&mut self) {
        self.published = None;
        self.outputs.reset();
        self.sent.clear();
        self.echoes.clear();
        self.wide.clear();
        self.states.clear();
    }
}

/// A tap that wakes this window when a mapped message comes (the boot path's).
pub fn waking_tap(ctx: &egui::Context) -> MidiTap {
    let ctx = ctx.clone();
    MidiTap::new(Some(std::sync::Arc::new(move || ctx.request_repaint())))
}

// ---- the global map's file ----------------------------------------------------------------

/// The global map's file: `controllers.json` beside `keymap.toml`.
pub fn global_path(dir: &Path) -> PathBuf {
    dir.join("controllers.json")
}

/// The file's format version.
pub const GLOBAL_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct GlobalFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    mappings: Vec<serde_json::Value>,
}

/// Read the global map: absent or unreadable, an empty one; a mapping that does not parse,
/// is malformed or names a target that is not portable is left out (logged), so one bad
/// line never loses the rest.
pub fn load_global(path: &Path) -> ControllerMap {
    load_global_checked(path).0
}

/// What reading the global map found, beyond the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobalLoad {
    /// Read whole (or no file yet).
    Whole,
    /// Something was lost; the whole file is kept beside, at this path.
    Kept(PathBuf),
    /// Something was lost and the file could not be kept beside: nothing may be saved over
    /// it (review 3, #30).
    Unsafe,
}

/// [`load_global`], and whether anything in the file was lost: then the file is kept
/// beside, as `controllers.json.bad` (an earlier one kept too), before anything can write
/// over it (review 1, #31) — a file that is no JSON or no text, one that cannot be read,
/// one written by a newer Cantus, and one with a mapping this build cannot read (review 2,
/// #9, #29, #39).
pub fn load_global_checked(path: &Path) -> (ControllerMap, bool) {
    let (map, load) = load_global_report(path);
    (map, load != GlobalLoad::Whole)
}

/// [`load_global_checked`], with where the file was kept — once: a file kept already with
/// the same bytes is not kept again at every start (review 3, #30, #40).
pub fn load_global_report(path: &Path) -> (ControllerMap, GlobalLoad) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (ControllerMap::default(), GlobalLoad::Whole)
        }
        Err(e) => {
            log::warn!("controllers: cannot read {}: {e}", path.display());
            return (ControllerMap::default(), keep_bad(path, None, false));
        }
    };
    let file: GlobalFile = match serde_json::from_slice(&bytes) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("controllers: cannot read {}: {e}", path.display());
            return (ControllerMap::default(), keep_bad(path, Some(&bytes), true));
        }
    };
    let mut lost = file.version > GLOBAL_VERSION;
    let mut map = ControllerMap::default();
    for v in file.mappings {
        match serde_json::from_value::<Mapping>(v) {
            Ok(m) if m.is_valid() && m.target.is_portable() && map.get(m.id).is_none() => {
                map.mappings.push(m)
            }
            Ok(m) => {
                log::warn!("controllers: mapping {} left out", m.id.0);
                lost = true;
            }
            Err(e) => {
                log::warn!("controllers: a mapping left out: {e}");
                lost = true;
            }
        }
    }
    if !lost {
        return (map, GlobalLoad::Whole);
    }
    // What could be read is used; the whole file stays beside for what could not.
    let kept = keep_bad(path, Some(&bytes), false);
    (map, kept)
}

/// Keep `path` beside as `controllers.json.bad` — renamed (`move_it`) or copied — unless a
/// kept file already holds the same `bytes`; when one with other bytes is there, with the
/// time in the name, so an earlier one is never lost.
fn keep_bad(path: &Path, bytes: Option<&[u8]>, move_it: bool) -> GlobalLoad {
    let same = |p: &Path| bytes.is_some_and(|b| std::fs::read(p).is_ok_and(|x| x == b));
    let dir = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(&stem) && name.ends_with(".bad") && same(&e.path()) {
                if move_it {
                    let _ = std::fs::remove_file(path);
                }
                return GlobalLoad::Kept(e.path());
            }
        }
    }
    let mut bad = path.with_extension("json.bad");
    if bad.exists() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        bad = path.with_extension(format!("json.{secs}.bad"));
    }
    let kept = if move_it {
        std::fs::rename(path, &bad)
    } else {
        std::fs::copy(path, &bad).map(|_| ())
    };
    match kept {
        Ok(()) => GlobalLoad::Kept(bad),
        Err(e) => {
            log::warn!("controllers: cannot keep {}: {e}", bad.display());
            GlobalLoad::Unsafe
        }
    }
}

/// Write the global map.
pub fn save_global(path: &Path, map: &ControllerMap) -> std::io::Result<()> {
    let file = GlobalFile {
        version: GLOBAL_VERSION,
        mappings: map
            .mappings
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Through a file beside, flushed, then renamed: a crash mid-write leaves the old map.
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write as _;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

// ---- words --------------------------------------------------------------------------------

/// A note's name, as the piano roll writes it (C4 is 60).
pub fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[usize::from(n % 12)], i32::from(n / 12) - 1)
}

/// The control in words: "CC 74", "14-bit CC 7", "Note C2", "Pitch bend"…
pub fn control_text(c: Control) -> String {
    match c {
        Control::Cc(n) => tf("ctl.src.cc", &[("n", &n)]),
        Control::Cc14(n) => tf("ctl.src.cc14", &[("n", &n)]),
        Control::Note(n) => tf("ctl.src.note", &[("n", &note_name(n))]),
        Control::PitchBend => t("ctl.src.bend"),
        Control::Program(n) => tf("ctl.src.program", &[("n", &(u32::from(n) + 1))]),
        Control::ChannelPressure => t("ctl.src.pressure"),
        Control::PolyPressure(n) => tf("ctl.src.key_pressure", &[("n", &note_name(n))]),
    }
}

/// The channel in words: "Ch 1" or "Any channel".
pub fn channel_text(c: Option<u8>) -> String {
    match c {
        Some(c) => tf("ctl.src.channel", &[("n", &(u32::from(c) + 1))]),
        None => t("ctl.src.any_channel"),
    }
}

/// The port in words: its name (with its ordinal when two share it), or "Any port".
pub fn port_text(p: &cantus_core::controller::PortRef) -> String {
    if p.is_any() {
        t("ctl.src.any_port")
    } else {
        // The device's name (a Linux port's client numbers left out, review 3, #45).
        let name = cantus_core::controller::clean_port_name(&p.name);
        if p.ordinal > 0 {
            format!("{name} ({})", u32::from(p.ordinal) + 1)
        } else {
            name.into_owned()
        }
    }
}

/// The mode in words.
pub fn mode_text(m: MapMode) -> String {
    use cantus_core::controller::Encoding as E;
    t(match m {
        MapMode::Absolute => "ctl.mode.absolute",
        MapMode::Relative(E::TwosComplement) => "ctl.mode.rel_twos",
        MapMode::Relative(E::BinaryOffset) => "ctl.mode.rel_offset",
        MapMode::Relative(E::SignBit) => "ctl.mode.rel_sign",
        MapMode::Toggle => "ctl.mode.toggle",
        MapMode::Momentary => "ctl.mode.momentary",
        MapMode::Trigger => "ctl.mode.trigger",
    })
}

/// The takeover in words.
pub fn takeover_text(k: Takeover) -> String {
    t(match k {
        Takeover::Jump => "ctl.takeover.jump",
        Takeover::Pickup => "ctl.takeover.pickup",
        Takeover::Scale => "ctl.takeover.scale",
    })
}

fn desk_text(p: DeskParam) -> String {
    t(match p {
        DeskParam::Volume => "ctl.desk.volume",
        DeskParam::Pan => "ctl.desk.pan",
        DeskParam::Mute => "ctl.desk.mute",
        DeskParam::Solo => "ctl.desk.solo",
        DeskParam::Arm => "ctl.desk.arm",
    })
}

/// The source in words, whole: "CC 74 · Ch 1 · Keystation 49".
pub fn source_text(s: &MidiSource) -> String {
    format!(
        "{} \u{b7} {} \u{b7} {}",
        control_text(s.control),
        channel_text(s.channel),
        port_text(&s.port)
    )
}

// ---- the verbs ----------------------------------------------------------------------------

impl CantusApp {
    /// A mapping's target in words: a hosted parameter learnt on another plug-in than the
    /// one there now is named after the plug-in it was learnt on — never the loaded one's
    /// parameter (review 3, #32, #44).
    pub fn mapping_name(&self, m: &Mapping) -> String {
        let p = self.project();
        if let (MappingTarget::Param(a), Some(learnt)) = (&m.target, &m.plugin) {
            if !m.fits(p) {
                // The names it was learnt with (review 4, #47); else the catalogue's name, a
                // lookup with no sorting (#62); else its id.
                let plugin = if learnt.name.is_empty() {
                    let format = cantus_host::from_project_format(learnt.format);
                    self.plugins
                        .db
                        .find(format, &learnt.id)
                        .map_or_else(|| learnt.id.clone(), |i| i.name.clone())
                } else {
                    learnt.name.clone()
                };
                let param = if learnt.param.is_empty() {
                    let id = match a {
                        AutoTarget::Plugin { id, .. } | AutoTarget::PluginInstrument { id, .. } => {
                            *id
                        }
                        _ => 0,
                    };
                    tf("ctl.target.plugin_param", &[("n", &id)])
                } else {
                    learnt.param.clone()
                };
                return format!("{plugin} \u{b7} {param}");
            }
        }
        self.mapping_target_name(&m.target)
    }

    /// A target in words, for the mappings view, the status line and the menus.
    pub fn mapping_target_name(&self, target: &MappingTarget) -> String {
        let p = self.project();
        let track = |id: cantus_core::TrackId| {
            p.track(id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| t("auto.target.missing"))
        };
        let strip = |id: cantus_core::StripId| {
            crate::automation::target_parts(p, &AutoTarget::StripGain(id)).0
        };
        let effect = |id: cantus_core::EffectId| {
            p.effect_host(id)
                .and_then(|h| p.effect(h, id))
                .map(|e| match e.spec.external.as_deref() {
                    Some(x) => x.name.clone(),
                    None => t(e.spec.kind.name_key()),
                })
                .unwrap_or_else(|| t("auto.target.missing"))
        };
        let two = |a: String, b: String| format!("{a} \u{b7} {b}");
        match target {
            MappingTarget::Param(a) => self.target_name(a),
            MappingTarget::TrackMute(id) => two(track(*id), t("ctl.desk.mute")),
            MappingTarget::TrackSolo(id) => two(track(*id), t("ctl.desk.solo")),
            MappingTarget::TrackArm(id) => two(track(*id), t("ctl.desk.arm")),
            MappingTarget::StripMute(id) => two(strip(*id), t("ctl.desk.mute")),
            MappingTarget::StripSolo(id) => two(strip(*id), t("ctl.desk.solo")),
            MappingTarget::EffectBypass(id) => two(effect(*id), t("ctl.target.bypass")),
            // The master as the pane names it everywhere (review 4, #51).
            MappingTarget::StripFx(id) if id.is_master() => {
                two(t("kit.mixer.master"), t("ctl.desk.fx"))
            }
            MappingTarget::StripFx(id) => two(strip(*id), t("ctl.desk.fx")),
            MappingTarget::StripArm(id) => two(strip(*id), t("ctl.desk.arm")),
            MappingTarget::EffectMix(id) => two(effect(*id), t("ctl.target.mix")),
            MappingTarget::Tempo => t("ctl.target.tempo"),
            // An action by its own group, as *Add a mapping* offers it (review 2, #43).
            MappingTarget::Action(id) => match crate::keymap::Action::from_id(id) {
                Some(a) => two(t(a.category().label_key()), t(&a.label_key())),
                // An action this build does not know: no group to name it by (review 3, #49).
                None => two(t("ctl.target.unknown_action"), id.clone()),
            },
            MappingTarget::SelectedTrack(d) => two(t("ctl.target.selected_track"), desk_text(*d)),
            MappingTarget::SelectedStrip(d) => two(t("ctl.target.selected_strip"), desk_text(*d)),
            MappingTarget::DeviceParam(n) => {
                tf("ctl.target.device_param", &[("n", &(u32::from(*n) + 1))])
            }
        }
    }

    /// The map of `scope`.
    pub fn controller_map(&self, scope: Scope) -> &ControllerMap {
        match scope {
            Scope::Global => &self.controllers.global,
            Scope::Project => &self.project().controllers,
        }
    }

    /// The mappings that drive `target`, in either map (the project's first).
    pub fn mappings_of(&self, target: &MappingTarget) -> Vec<(Scope, Mapping)> {
        let mut out: Vec<(Scope, Mapping)> = self
            .project()
            .controllers
            .of_target(target)
            .map(|m| (Scope::Project, m.clone()))
            .collect();
        out.extend(
            self.controllers
                .global
                .of_target(target)
                .map(|m| (Scope::Global, m.clone())),
        );
        out
    }

    /// Whether a controller drives a target `is` says yes to — asked without building the
    /// target (a control drawn every frame allocates nothing, review 3, #51).
    pub fn is_mapped_where(&self, is: impl Fn(&MappingTarget) -> bool) -> bool {
        let p = self.project();
        p.controllers
            .mappings
            .iter()
            .any(|m| is(&m.target) && m.fits(p))
            || self
                .controllers
                .global
                .mappings
                .iter()
                .any(|m| is(&m.target))
    }

    /// Whether a controller drives `target`.
    pub fn is_mapped(&self, target: &MappingTarget) -> bool {
        let p = self.project();
        p.controllers.of_target_in(target, p).next().is_some()
            || self.controllers.global.of_target(target).next().is_some()
    }

    /// What a control bound to `target` shows of MIDI learn (the kit's offer).
    pub fn learn_offer(&self, target: &MappingTarget) -> crate::kit::automate::LearnOffer {
        crate::kit::automate::LearnOffer {
            mapped: self.is_mapped(target),
            mode: self.controllers.learn_mode,
            armed: self
                .controllers
                .learning
                .as_ref()
                .is_some_and(|l| &l.target == target),
        }
    }

    /// The map a learnt `target` goes to (the module's rule).
    pub fn scope_for(target: &MappingTarget) -> Scope {
        if target.is_portable() {
            Scope::Global
        } else {
            Scope::Project
        }
    }

    /// Arm `target`: the next hardware control moved is mapped to it (learnt again when it
    /// is mapped already: its first mapping's source is replaced). Says so on the status
    /// line; Esc, or arming it again, lets go.
    pub fn learn_target(&mut self, target: MappingTarget) {
        if self
            .controllers
            .learning
            .as_ref()
            .is_some_and(|l| l.target == target)
        {
            self.cancel_learning();
            return;
        }
        // A key that is set, never automated (Chorda's tuning, ADR 0059): not learnt
        // either — a controller would record automation on it.
        if let MappingTarget::Param(AutoTarget::Instrument { track, key }) = &target {
            let kind = self
                .project()
                .track(*track)
                .and_then(|t| t.instrument.native())
                .map(|n| n.kind);
            if kind.is_some_and(|k| !k.automatable(key)) {
                self.set_error(t("chorda.not_automatable"));
                return;
            }
        }
        let scope = Self::scope_for(&target);
        // Learnt again: a mapping of the plug-in there now — one learnt on a plug-in swapped
        // out is left for an undo of the swap (review 2, #51).
        let p = self.project();
        let replace = self
            .controller_map(scope)
            .of_target_in(&target, p)
            .next()
            .map(|m| m.id);
        let name = self.mapping_target_name(&target);
        self.controllers.learning = Some(Learning {
            target,
            scope,
            replace,
            heard: Vec::new(),
            source: None,
            first: None,
            armed_at: Instant::now(),
            range: None,
        });
        self.controllers.tap.filter().set_learning(true);
        self.set_status(tf("ctl.status.armed", &[("name", &name)]));
    }

    /// When the armed learn was armed (a control compares it before and after its offer:
    /// changed, the learn is its own).
    pub fn learn_armed_at(&self) -> Option<Instant> {
        self.controllers.learning.as_ref().map(|l| l.armed_at)
    }

    /// The armed learn of `target` is made over `from` … `to` (positions of its travel):
    /// what the control it was armed from stands for (review 4, #46, #53).
    pub fn set_learn_range(&mut self, target: &MappingTarget, from: f32, to: f32) {
        if let Some(l) = self.controllers.learning.as_mut() {
            if &l.target == target {
                l.range = Some((from.clamp(0.0, 1.0), to.clamp(0.0, 1.0)));
            }
        }
    }

    /// Whether a live mapping of `target` reaches `pos` at its *From* or its *To*: the mark
    /// of a control that stands for one value of a choice (a band's solo LED) is shown on
    /// the control whose value the mapping reaches only (review 4, #46).
    pub fn mapping_reaches(&self, target: &MappingTarget, pos: f32) -> bool {
        let near = |a: f32| (a - pos).abs() <= 1e-4;
        [Scope::Project, Scope::Global].into_iter().any(|scope| {
            self.controller_map(scope)
                .mappings
                .iter()
                .any(|m| &m.target == target && (near(m.min) || near(m.max)))
        })
    }

    /// Stop waiting for a control.
    pub fn cancel_learning(&mut self) {
        if self.controllers.learning.take().is_some() {
            self.set_status(t("ctl.status.cancelled"));
        }
        self.controllers.tap.filter().set_learning(false);
    }

    /// A device's name as the mixer shows it (a hosted plug-in's own).
    pub fn effect_display_name(&self, id: cantus_core::EffectId) -> String {
        let p = self.project();
        p.effect_host(id)
            .and_then(|h| p.effect(h, id))
            .map(|e| match e.spec.external.as_deref() {
                Some(x) => x.name.clone(),
                None => t(e.spec.kind.name_key()),
            })
            .unwrap_or_else(|| t("auto.target.missing"))
    }

    /// *Forget these controllers* on an insert: its bypass and mix, one step, the status
    /// naming the insert (review 2, #46).
    pub fn forget_slot(&mut self, id: cantus_core::EffectId) {
        let all: Vec<_> = (0..2).map(|w| slot_learn_target(id, w)).collect();
        let name = self.effect_display_name(id);
        self.forget_targets(&all, &name);
    }

    /// Take every mapping of `target` away (the project's: one undo step).
    pub fn forget_target(&mut self, target: &MappingTarget) {
        let name = self.mapping_target_name(target);
        self.forget_targets(std::slice::from_ref(target), &name);
    }

    /// Take every mapping of every target in `targets` away: the project's in one undo
    /// step, the global ones saved once, one status naming `what` (review 1, #51).
    pub fn forget_targets(&mut self, targets: &[MappingTarget], what: &str) {
        // The project's mappings of the plug-ins there now: one of a plug-in swapped out
        // stays for an undo of the swap (review 2, #51).
        let p = self.project();
        let project: Vec<MappingId> = p
            .controllers
            .mappings
            .iter()
            .filter(|m| targets.contains(&m.target) && m.fits(p))
            .map(|m| m.id)
            .collect();
        let global = self
            .controllers
            .global
            .mappings
            .iter()
            .filter(|m| targets.contains(&m.target))
            .count();
        if !project.is_empty() {
            let commands: Vec<Command> = project
                .into_iter()
                .map(|id| Command::RemoveMapping { id })
                .collect();
            let cmd = if commands.len() == 1 {
                commands.into_iter().next().expect("one")
            } else {
                Command::Batch {
                    label_key: "cmd.controllers.remove".into(),
                    commands,
                }
            };
            self.exec(cmd);
        }
        if global > 0 {
            self.controllers
                .global
                .mappings
                .retain(|m| !targets.contains(&m.target));
            self.store_global_map();
        }
        self.set_status(tf("ctl.status.forgot", &[("name", &what)]));
    }

    /// The learn mode on or off (Ableton's MIDI Map mode). Off lets go of a learn in
    /// progress.
    pub fn toggle_learn_mode(&mut self) {
        self.controllers.learn_mode = !self.controllers.learn_mode;
        if self.controllers.learn_mode {
            self.set_status(t("ctl.status.mode_on"));
        } else {
            self.cancel_learning();
            self.set_status(t("ctl.status.mode_off"));
        }
    }

    /// FL's *Link to controller* for the last tweaked parameter — the way into a hosted
    /// plug-in's own window: move a knob there, then this, then the hardware.
    pub fn link_last_tweaked(&mut self) -> bool {
        match self.automation.last_tweaked.clone() {
            Some(target) => {
                self.learn_target(MappingTarget::Param(target));
                true
            }
            None => {
                self.set_status(t("auto.status.nothing_tweaked"));
                false
            }
        }
    }

    /// A new mapping id, unique in both maps.
    pub(crate) fn new_mapping_id(&mut self) -> MappingId {
        loop {
            let id = MappingId(self.bus.alloc_id());
            if self.project().controllers.get(id).is_none()
                && self.controllers.global.get(id).is_none()
            {
                return id;
            }
        }
    }

    /// Add `m` to `scope`'s map (the project's: one undo step; the global one is saved).
    pub fn add_mapping(&mut self, scope: Scope, m: Mapping) -> bool {
        match scope {
            Scope::Project => {
                let index = self.project().controllers.mappings.len();
                self.exec(Command::InsertMapping {
                    index,
                    mapping: Box::new(m),
                })
            }
            Scope::Global => {
                if !m.is_valid() || !m.target.is_portable() {
                    return false;
                }
                self.controllers.global.mappings.push(m);
                self.store_global_map();
                true
            }
        }
    }

    /// Replace the mapping of `m`'s id in `scope` (a drag in the view merges under `key`).
    pub fn set_mapping(&mut self, scope: Scope, m: Mapping, key: Option<cantus_core::MergeKey>) {
        // Its mode or range may have changed: it takes over afresh.
        self.controllers.states.remove(&(scope, m.id));
        match scope {
            Scope::Project => {
                let cmd = Command::SetMapping {
                    mapping: Box::new(m),
                };
                match key {
                    Some(k) => self.exec_merging(cmd, k),
                    None => self.exec(cmd),
                };
            }
            Scope::Global => {
                if !m.is_valid() {
                    return;
                }
                if let Some(i) = self.controllers.global.position(m.id) {
                    self.controllers.global.mappings[i] = m;
                    if key.is_none() {
                        self.store_global_map();
                    }
                }
            }
        }
    }

    /// Take the mapping `id` out of `scope`'s map.
    pub fn remove_mapping(&mut self, scope: Scope, id: MappingId) {
        match scope {
            Scope::Project => {
                self.exec(Command::RemoveMapping { id });
            }
            Scope::Global => {
                self.controllers.global.mappings.retain(|m| m.id != id);
                self.store_global_map();
            }
        }
        self.controllers.states.remove(&(scope, id));
    }

    /// Write the global map to its file (when it has one).
    pub fn store_global_map(&mut self) {
        if let Some(path) = &self.controllers.global_path {
            if let Err(e) = save_global(path, &self.controllers.global) {
                self.set_error(tf("ctl.status.save_failed", &[("error", &e.to_string())]));
            }
        } else if let Some(path) = &self.controllers.global_unsaved {
            // Not kept: said at every edit, never lost without a word (the start's notice
            // may have been covered by the boot's own).
            let file = path.display().to_string();
            self.set_error(tf("ctl.status.global_unsaved", &[("file", &file)]));
        }
    }

    /// The selected track and strip, and the focused device's first parameters: what the
    /// relative targets name now.
    pub fn controller_focus(
        &self,
    ) -> (
        Option<cantus_core::TrackId>,
        Option<cantus_core::StripId>,
        Vec<AutoTarget>,
    ) {
        (
            self.selected_track,
            self.selected_strip,
            self.focused_device_params(),
        )
    }

    /// The concrete target `m`'s names now.
    pub fn resolve_target(&self, target: &MappingTarget) -> Option<MappingTarget> {
        if !target.is_relative() {
            return Some(target.clone());
        }
        let (track, strip, device) = self.controller_focus();
        // The master chosen on the desk is the selected strip too (review 3, #7).
        let master = strip.is_none() && self.inspector.follows_master();
        target.resolve(&Focus {
            track,
            strip,
            master,
            device: &device,
        })
    }

    /// The focused device's first eight parameters: the device window clicked last (an
    /// effect's or an instrument's), else the open effect editor, else the selected
    /// track's instrument.
    pub fn focused_device_params(&self) -> Vec<AutoTarget> {
        let p = self.project();
        let key = self
            .theme_ctx
            .as_ref()
            .and_then(crate::views::workspace_device::focused);
        if let Some(key) = key {
            if let Some((_, e)) = p
                .all_effects()
                .find(|(_, e)| crate::views::fx::device_id(e.id).value() == key)
            {
                return self.effect_params(e.id);
            }
            if let Some(tr) = p
                .tracks
                .iter()
                .find(|tr| crate::views::inst::editor_id(tr.id).value() == key)
            {
                return self.instrument_params(tr.id);
            }
        }
        let open = self
            .fx
            .open
            .map(|(_, e, _)| e)
            .or(self.eq_editor.open.map(|(_, e)| e))
            .or(self.comp_editor.open.map(|(_, e)| e));
        if let Some(e) = open {
            return self.effect_params(e);
        }
        match self.selected_track {
            Some(tr) => self.instrument_params(tr),
            None => Vec::new(),
        }
    }

    /// An insert's first parameters: continuous ones first, its mix last.
    fn effect_params(&self, effect: cantus_core::EffectId) -> Vec<AutoTarget> {
        let p = self.project();
        let Some(slot) = p.effect_host(effect).and_then(|h| p.effect(h, effect)) else {
            return Vec::new();
        };
        if slot.spec.external.is_some() {
            let Some(h) = self.plugins.hosted(effect) else {
                return Vec::new();
            };
            return h
                .params
                .iter()
                .filter(|x| x.automatable && !x.hidden && !x.read_only)
                .take(usize::from(cantus_core::controller::DEVICE_PARAMS))
                .map(|x| AutoTarget::Plugin { effect, id: x.id })
                .collect();
        }
        let params = slot.spec.kind.params();
        let mut keys: Vec<&str> = params
            .iter()
            .filter(|i| !i.stepped && i.key != "mix")
            .map(|i| i.key)
            .collect();
        keys.extend(params.iter().filter(|i| i.stepped).map(|i| i.key));
        keys.into_iter()
            .take(usize::from(cantus_core::controller::DEVICE_PARAMS))
            .map(|k| AutoTarget::Effect {
                effect,
                key: k.to_string(),
            })
            .collect()
    }

    /// A track's instrument's first parameters.
    fn instrument_params(&self, track: cantus_core::TrackId) -> Vec<AutoTarget> {
        let n = usize::from(cantus_core::controller::DEVICE_PARAMS);
        let Some(tr) = self.project().track(track) else {
            return Vec::new();
        };
        match &tr.instrument {
            InstrumentSpec::Native(spec) => {
                let kind = spec.kind;
                let params = kind.params();
                let mut keys: Vec<&str> = params
                    .iter()
                    .filter(|i| kind.automatable(i.key) && !i.stepped)
                    .map(|i| i.key)
                    .collect();
                keys.extend(
                    params
                        .iter()
                        .filter(|i| kind.automatable(i.key) && i.stepped)
                        .map(|i| i.key),
                );
                keys.into_iter()
                    .take(n)
                    .map(|k| AutoTarget::Instrument {
                        track,
                        key: k.to_string(),
                    })
                    .collect()
            }
            InstrumentSpec::Firmus(_) => cantus_core::FirmusParam::ALL
                .iter()
                .take(n)
                .map(|p| AutoTarget::Firmus { track, param: *p })
                .collect(),
            InstrumentSpec::External(_) => self
                .plugins
                .hosted_instrument(track)
                .map(|h| {
                    h.params
                        .iter()
                        .filter(|x| x.automatable && !x.hidden && !x.read_only)
                        .take(n)
                        .map(|x| AutoTarget::PluginInstrument { track, id: x.id })
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The keymap's controller actions (round 29): the learn mode, *Link to controller*,
    /// the mappings pane. `true` when `action` was one.
    pub fn controllers_action(&mut self, action: crate::keymap::Action) -> bool {
        use crate::keymap::Action;
        match action {
            Action::MidiLearnMode => self.toggle_learn_mode(),
            Action::LinkLastTweaked => {
                self.link_last_tweaked();
            }
            Action::ViewControllers => crate::views::controllers::toggle_pane(self),
            _ => return false,
        }
        true
    }
}

/// A switch that is not a parameter (a mute, a solo, an arm, the transport's buttons) in
/// learn mode: outlined above whatever is drawn there, and a click on it arms it. `true`
/// when it was clicked (the caller then does nothing else with the click: the button under
/// is covered anyway). Outside learn mode it does nothing.
pub fn learn_spot(
    app: &mut CantusApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    target: MappingTarget,
) -> bool {
    learn_spot_in(app, ui, rect, ui.clip_rect(), target)
}

/// [`learn_spot`] inside `clip` (a view's own visible area: the mixer's strips scrolled
/// under a dock, an arrangement header under the ruler) — nothing outlined or covered
/// outside it (review 1, #46). Outside learn mode a mapped spot wears the mapped mark.
pub fn learn_spot_in(
    app: &mut CantusApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    clip: egui::Rect,
    target: MappingTarget,
) -> bool {
    let shown = rect.intersect(clip).intersect(ui.clip_rect());
    if shown.width() <= 0.0 || shown.height() <= 0.0 || !ui.is_rect_visible(shown) {
        return false;
    }
    // Noted where it is offered: shown, in the learn mode or wearing its mark (review 3,
    // #36 — never a flag that is true whatever the control does).
    if crate::kit::automate::probe::noting_learn() {
        crate::kit::automate::probe::note_learn(&format!("spot:{target:?}"), true);
    }
    let offer = app.learn_offer(&target);
    if !app.controllers.learn_mode {
        if offer.mapped {
            crate::kit::automate::learn_mark_over(ui, rect, shown);
        }
        return false;
    }
    let id = ui.id().with(("learn-spot", &target));
    let clicked = crate::kit::automate::learn_spot_clipped(ui, rect, shown, id, offer);
    if clicked {
        app.learn_target(target);
    }
    clicked
}

/// A stepped parameter drawn by a control that only sets its value — a segmented choice, an
/// LED, a menu field, a stepper (round 29, review 2 of ADR 0062, #44): its parameter's menu
/// (automate, modulate, *Learn MIDI controller*, reset to `default`), its mapped mark, and
/// the learn mode's outline and click. `true` while the learn mode has it: the caller then
/// takes no value from the control's own click.
pub fn stepped_param(
    app: &mut CantusApp,
    ui: &egui::Ui,
    target: AutoTarget,
    resp: &egui::Response,
    default: Option<f32>,
) -> bool {
    use crate::kit::automate as a;
    let s = crate::kit::Style::current();
    let pos = app
        .project_pos(&MappingTarget::Param(target.clone()))
        .unwrap_or(0.0);
    let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let (m, _) = app.mod_offer(&target, pos, &s, &mut rings);
    if a::probe::noting_learn() {
        a::probe::note_learn(&format!("stepped:{target:?}"), m.learn.is_some());
    }
    if let Some(l) = m.learn {
        if l.mode {
            if a::learn_spot(ui, resp.rect, resp.id, l) {
                app.learn_target(MappingTarget::Param(target));
            }
            return true;
        }
        if l.mapped {
            a::learn_mark_over(ui, resp.rect, ui.clip_rect());
        }
    }
    let offer = a::Offer {
        automated: app.is_automated(&target),
        typing: false,
        modulation: Some(m),
    };
    match a::menu(ui, resp, offer) {
        Some(a::Choice::Automate) => app.automate(target),
        Some(a::Choice::Reset) => {
            if let Some(cmd) = default.and_then(|d| target.command(app.project(), f64::from(d))) {
                app.exec(cmd);
            }
        }
        Some(c) => app.modulation_choice(&target, c, resp.rect),
        None => {}
    }
    false
}

/// [`stepped_param`] for an instrument's key.
pub fn stepped_inst(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: cantus_core::TrackId,
    spec: &cantus_core::NativeSpec,
    key: &str,
    resp: &egui::Response,
) -> bool {
    stepped_inst_of(app, ui, track, spec.kind, key, resp)
}

/// [`stepped_inst`] knowing only the instrument's kind.
pub fn stepped_inst_of(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: cantus_core::TrackId,
    kind: cantus_core::InstrumentKind,
    key: &str,
    resp: &egui::Response,
) -> bool {
    if !kind.automatable(key) {
        return false;
    }
    // Nothing of learn to show and no menu asked for: no target built (review 3, #51).
    let idle = !app.controllers.learn_mode
        && !resp.secondary_clicked()
        && !egui::Popup::is_any_open(ui.ctx())
        && !crate::kit::automate::probe::noting_learn()
        && !app.is_mapped_where(|t| {
            matches!(t, MappingTarget::Param(AutoTarget::Instrument { track: tr, key: k })
                if *tr == track && k == key)
        });
    if idle {
        return false;
    }
    let default = kind.param(key).map(|i| i.default);
    stepped_param(
        app,
        ui,
        AutoTarget::Instrument {
            track,
            key: key.to_string(),
        },
        resp,
        default,
    )
}

/// [`learn_spot`] for a key that runs a keymap action (the top bar's transport).
pub fn learn_action_spot(
    app: &mut CantusApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    action: crate::keymap::Action,
) -> bool {
    learn_spot(
        app,
        ui,
        rect,
        MappingTarget::Action(action.id().to_string()),
    )
}

/// The track header's menu rows for MIDI learn (round 29, ADR 0062): the track's volume,
/// pan, mute, solo and arm, each checked when mapped, and *Forget* when one is. `true` when
/// a row was chosen (the menu closes).
pub fn track_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: cantus_core::TrackId) -> bool {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    let targets = [
        (
            MappingTarget::Param(AutoTarget::TrackGain(id)),
            "ctl.menu.volume",
        ),
        (
            MappingTarget::Param(AutoTarget::TrackPan(id)),
            "ctl.menu.pan",
        ),
        (MappingTarget::TrackMute(id), "ctl.menu.mute"),
        (MappingTarget::TrackSolo(id), "ctl.menu.solo"),
        (MappingTarget::TrackArm(id), "ctl.menu.arm"),
    ];
    let mapped: Vec<bool> = targets.iter().map(|(t, _)| app.is_mapped(t)).collect();
    let any = mapped.iter().any(|m| *m);
    let mut items = vec![Item::Band(Src::Key("ctl.menu.band"))];
    for ((_, key), m) in targets.iter().zip(&mapped) {
        items.push(Item::Action {
            label: Src::Key(key),
            check: if *m { Check::On } else { Check::Off },
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    if any {
        items.push(Item::action(Src::Key("ctl.menu.forget")));
    }
    match crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
        Some(i) if (1..=targets.len()).contains(&i) => {
            app.learn_target(targets[i - 1].0.clone());
            true
        }
        Some(i) if any && i == targets.len() + 1 => {
            let all: Vec<MappingTarget> = targets.iter().map(|(t, _)| t.clone()).collect();
            let name = app
                .project()
                .track(id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            app.forget_targets(&all, &name);
            true
        }
        _ => false,
    }
}

/// What a strip's (or the master's) learn row `what` drives: volume (0), pan (1), width
/// (2), mute (3), solo (4); the master has no mute or solo.
pub fn strip_learn_target(id: cantus_core::StripId, what: u8) -> Option<MappingTarget> {
    use cantus_core::AutoTarget as A;
    let master = id.is_master();
    Some(match what {
        0 if master => MappingTarget::Param(A::MasterGain),
        1 if master => MappingTarget::Param(A::MasterPan),
        2 if master => MappingTarget::Param(A::MasterWidth),
        0 => MappingTarget::Param(A::StripGain(id)),
        1 => MappingTarget::Param(A::StripPan(id)),
        2 => MappingTarget::Param(A::StripWidth(id)),
        3 if !master => MappingTarget::StripMute(id),
        4 if !master => MappingTarget::StripSolo(id),
        // The effects switch and the record arm (review 3, #46).
        5 => MappingTarget::StripFx(id),
        6 if !master => MappingTarget::StripArm(id),
        _ => return None,
    })
}

/// What an insert's learn row `what` drives: its bypass (0) or its mix (1).
pub fn slot_learn_target(effect: cantus_core::EffectId, what: u8) -> MappingTarget {
    if what == 0 {
        MappingTarget::EffectBypass(effect)
    } else {
        MappingTarget::EffectMix(effect)
    }
}

/// A raw message's addresses and values as mappings read them: a 7-bit controller is also
/// its 14-bit pair's half; a note-off (or a note-on at 0) is a release.
pub(crate) fn decode(
    wide: &mut HashMap<(u8, u8, u8), Wide>,
    port: u8,
    ev: &MidiEvent,
    at: Instant,
) -> (u8, [(Control, u16, bool); 2], usize) {
    let ch = ev.status & 0x0F;
    let (d1, d2) = (ev.data1 & 0x7F, ev.data2 & 0x7F);
    let none = (Control::PitchBend, 0, false);
    match ev.status & 0xF0 {
        0xB0 => {
            let first = (Control::Cc(d1), u16::from(d2), true);
            let gap = |t: Option<Instant>| {
                t.is_some_and(|t| at.saturating_duration_since(t) <= PAIR_GAP)
            };
            let pair = |e: &Wide| u16::from(e.msb) << 7 | u16::from(e.lsb);
            if d1 < 32 {
                let e = wide.entry((port, ch, d1)).or_default();
                e.msb = d2;
                if gap(e.pending_lsb.take()) {
                    // Its fine half came first, from a controller that sends so.
                    e.pending = None;
                    e.pending_after = None;
                    e.lsb_at = None;
                    if !e.lsb_first {
                        told_fine_first(e);
                    }
                    (ch, [first, (Control::Cc14(d1), pair(e), true)], 2)
                } else if let Some(l_at) = e.lsb_at.take().filter(|t| gap(Some(*t))) {
                    // A fine half alone just before: held with it — its pair, unless a
                    // closer fine half follows (time tells, review 4, #4, #11).
                    e.pending = Some(at);
                    e.pending_after = Some(l_at);
                    (ch, [first, none], 1)
                } else if e.lsb_seen || e.lsb_first {
                    // Its fine half follows: the pair is read then, never half-made (review
                    // 1, #9) — or, if it does not come within PAIR_GAP, with a fine 0
                    // (MIDI's rule; `wide_alone`).
                    e.lsb = 0;
                    e.pending = Some(at);
                    e.pending_after = None;
                    (ch, [first, none], 1)
                } else {
                    // A pair that never sent its fine half (7-bit mode): the coarse value
                    // over the whole 14-bit travel, so its top is the top.
                    let v = (u32::from(d2) * 16_383 / 127) as u16;
                    (ch, [first, (Control::Cc14(d1), v, true)], 2)
                }
            } else if d1 < 64 {
                let e = wide.entry((port, ch, d1 - 32)).or_default();
                e.lsb_seen = true;
                if let Some(m_at) = e.pending.filter(|t| gap(Some(*t))) {
                    e.pending = None;
                    let after = e.pending_after.take();
                    let next_pairs = after.is_some_and(|l_at| {
                        at.saturating_duration_since(m_at) >= m_at.saturating_duration_since(l_at)
                    });
                    if next_pairs {
                        // The held coarse half's pair was the fine half before it (fine
                        // first); this one is the next pair's, and waits for its coarse half.
                        told_fine_first(e);
                        let v = pair(e);
                        e.lsb = d2;
                        e.pending_lsb = Some(at);
                        (ch, [first, (Control::Cc14(d1 - 32), v, true)], 2)
                    } else {
                        // Its own fine half, after it: the pair, coarse half first — this
                        // controller is not fine-first, whatever it looked like.
                        e.lsb_first = false;
                        e.fine_first_seen = 0;
                        e.lsb = d2;
                        (ch, [first, (Control::Cc14(d1 - 32), pair(e), true)], 2)
                    }
                } else if e.lsb_first || e.fine_first_seen > 0 {
                    // A fine-first controller's fine half: it waits for its coarse half.
                    e.lsb = d2;
                    e.pending_lsb = Some(at);
                    (ch, [first, none], 1)
                } else {
                    // A fine half alone: its coarse half the same (MIDI lets a sender leave
                    // it out), read now.
                    e.pending = None;
                    e.lsb = d2;
                    e.lsb_at = Some(at);
                    (ch, [first, (Control::Cc14(d1 - 32), pair(e), true)], 2)
                }
            } else {
                (ch, [first, none], 1)
            }
        }
        0x90 if d2 > 0 => (ch, [(Control::Note(d1), u16::from(d2), true), none], 1),
        0x80 | 0x90 => (ch, [(Control::Note(d1), 0, false), none], 1),
        0xA0 => (
            ch,
            [(Control::PolyPressure(d1), u16::from(d2), true), none],
            1,
        ),
        0xC0 => (ch, [(Control::Program(d1), 127, true), none], 1),
        0xD0 => (
            ch,
            [(Control::ChannelPressure, u16::from(d1), true), none],
            1,
        ),
        0xE0 => (
            ch,
            [
                (Control::PitchBend, u16::from(d2) << 7 | u16::from(d1), true),
                none,
            ],
            1,
        ),
        _ => (ch, [none, none], 0),
    }
}
