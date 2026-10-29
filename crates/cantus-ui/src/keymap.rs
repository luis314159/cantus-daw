//! The keyboard map: every shortcut in one editable file.
//!
//! Musicians arrive from other DAWs with a decade of muscle memory, so Cantus keeps no
//! shortcut hard-coded in the views. An [`Action`] is a thing the application can do; a
//! [`Binding`] is a key plus modifiers; a [`Keymap`] joins them and lives in
//! `keymap.toml` next to the theme and the layouts.
//!
//! Three things are worth knowing before changing this module:
//!
//! - **Modifiers match exactly.** `Ctrl+Z` must not fire when the user presses
//!   `Ctrl+Shift+Z`, because that is Redo. Every comparison is on the whole modifier
//!   triple, never "contains".
//! - **Letter shortcuts yield to the typing piano.** When the computer keyboard is playing
//!   notes it owns the letter keys, so bindings without a modifier are suppressed
//!   ([`Keymap::pressed`] takes that as an argument). That is a mode, not a conflict, and
//!   it is why the tool actions also have number bindings.
//! - **The presets for other DAWs are approximations.** They follow each program's
//!   conventions for the actions Cantus actually has; they are a starting point to edit,
//!   not a claim of parity. Only [`Preset::Cantus`] is authoritative.

use egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

// ---- actions ------------------------------------------------------------------------

/// Everything the keyboard can ask the application to do.
///
/// The string form is the key in `keymap.toml` and must stay stable: renaming one silently
/// unbinds it in everybody's file. Adding one is free — it simply has no binding until a
/// preset or the user gives it one.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    // Transport
    PlayPause,
    Stop,
    Record,
    ToStart,
    ToggleLoop,
    ToggleMetronome,
    /// Tap tempo (round 28, ADR 0052).
    TapTempo,
    /// FL's PAT / SONG switch (round 28, ADR 0052).
    TogglePatternMode,
    Panic,
    /// A marker at the playhead (round 29, ADR 0060).
    AddMarker,
    /// The playhead to the marker before it / after it.
    PreviousMarker,
    NextMarker,
    /// The loop from the marker at or before the playhead to the next.
    LoopBetweenMarkers,
    /// The time selection from the marker at or before the playhead to the next.
    SelectBetweenMarkers,
    /// The arrangement's tempo track shown or hidden.
    ToggleTempoTrack,
    /// Freeze the selected track (its own strip, or its source), or unfreeze what holds it
    /// (round 29, ADR 0064).
    FreezeTrack,
    /// Bounce in place with the strip's inserts: the selected clips, the time selection or
    /// the whole selected track.
    BounceInPlace,
    /// Bounce in place, the source only (before the track's level).
    BounceSource,
    /// Low-latency monitoring on or off.
    ToggleLowLatency,
    // File
    NewProject,
    Open,
    Save,
    SaveAs,
    ExportWav,
    DemoSongs,
    /// The Settings window (round 30, ADR 0066): macOS's ⌘, — and Ctrl+, elsewhere.
    OpenSettings,
    /// The start page: a new project from a template, the recent projects (round 31, ADR
    /// 0072).
    NewFromTemplate,
    /// The project as it is, kept as a template of the user's.
    SaveAsTemplate,
    /// The selected track, its strip and what it reaches, kept as a track template.
    SaveTrackTemplate,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Duplicate,
    SelectAll,
    Delete,
    Quantize,
    ClearSelection,
    // Arrangement
    LoopToSelection,
    ZoomToSelection,
    SelectPrevTrack,
    SelectNextTrack,
    // Clips
    SplitClip,
    GlueClips,
    ToggleClipMute,
    ToggleClipLoop,
    ClipColorNext,
    RenameClip,
    /// Take the selected clips out of their link group (ADR 0037).
    MakeUnique,
    /// Link the selected clips to the clip being edited: they become the same clip.
    LinkClips,
    /// Add every linked copy of the selected clips to the selection.
    SelectLinked,
    /// Open the *Link to…* picker: link the selection to a group or clip chosen by name.
    LinkTo,
    // Notes
    NudgeLeft,
    NudgeRight,
    NudgeLeftBar,
    NudgeRightBar,
    TransposeUp,
    TransposeDown,
    TransposeOctaveUp,
    TransposeOctaveDown,
    // Tools
    ToolDraw,
    ToolPaint,
    ToolSelect,
    ToolDelete,
    ToolMute,
    ToolSlice,
    ToolPlayback,
    // View
    ToggleTypingPiano,
    ViewPianoRoll,
    ViewChannelRack,
    ViewMixer,
    /// Show or hide the arrangement (FL's F5).
    ViewArrangement,
    /// Show or hide the inspector (I).
    ToggleInspector,
    /// Show or hide the sample browser (FL's Alt+F8; ADR 0030).
    ToggleBrowser,
    /// The clip browser: link groups and clips as tiles to drag (round 26, ADR 0037).
    ToggleClips,
    /// Show or hide the Routing tab's matrix (Alt+F9, beside the mixer's F9; round 25).
    ViewRouting,
    /// Close every floating device window (F12).
    CloseFloating,
    /// The interface's zoom: Ctrl +, Ctrl −, Ctrl 0 (ADR 0024 §11).
    ZoomIn,
    ZoomOut,
    ZoomReset,
    /// "Pick an element": click anything to recolour what it is painted with.
    PickElement,
    /// Show or hide the keyboard card of the instrument whose editor is in front (K in the
    /// device shell; Alt+K from anywhere, so it never takes a note from the typing piano).
    ToggleDeviceKeyboard,
    // Mixer
    MoveStripLeft,
    MoveStripRight,
    ToggleStripSolo,
    ToggleStripMute,
    ToggleMeterWave,
    /// Open the selected strip's route menu where its ▼ chip is (Shift+F10, the
    /// platform's context-menu key; round 25). Acts while the mixer is being edited.
    RouteMenu,
    // Recording (ADR 0025)
    /// Arm the selected track for recording.
    ArmTrack,
    /// A new track with an input, armed and selected.
    AddAudioTrack,
    /// Count-in on or off.
    ToggleCountIn,
    /// Punch in/out on or off.
    TogglePunch,
    /// Show or hide the selected track's take lanes.
    ShowTakes,
    /// FL's *Tools ▸ Last tweaked ▸ Create automation clip* (ADR 0027).
    AutomateLastTweaked,
    /// The automation arm (round 31, ADR 0068): Write, Touch and Latch write while it is on.
    ToggleAutomationArm,
    /// The selected track's automation mode, the next of Read, Touch, Latch, Write.
    CycleTrackAutoMode,
    /// Open or close the selected track's instrument window, or bring it back from the
    /// device tray (F8; round 20).
    ToggleInstrumentWindow,
    // MIDI controllers (round 29, ADR 0062)
    /// The learn mode (Ableton's MIDI Map mode): every learnable control outlined.
    MidiLearnMode,
    /// FL's *Link to controller* for the last tweaked parameter.
    LinkLastTweaked,
    /// Show or hide the controller mappings.
    ViewControllers,
    // Workflow (round 31, ADR 0072)
    /// The command palette: every action, area, setting, device, recent project and
    /// template, found by typing (VS Code's Ctrl+Shift+P).
    CommandPalette,
    /// Show or hide the undo history.
    ViewHistory,
    // The score (round 32, ADR 0075)
    /// Show or hide the staff under the piano roll.
    ShowScore,
    /// Show or hide the Score pane (the selected track's part).
    ViewScore,
}

/// How the shortcuts dialog groups the list.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Transport,
    File,
    Edit,
    Arrangement,
    Clips,
    Notes,
    Tools,
    View,
    Mixer,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Transport,
        Category::File,
        Category::Edit,
        Category::Arrangement,
        Category::Clips,
        Category::Notes,
        Category::Tools,
        Category::View,
        Category::Mixer,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Category::Transport => "keymap.cat.transport",
            Category::File => "keymap.cat.file",
            Category::Edit => "keymap.cat.edit",
            Category::Arrangement => "keymap.cat.arrangement",
            Category::Clips => "keymap.cat.clips",
            Category::Notes => "keymap.cat.notes",
            Category::Tools => "keymap.cat.tools",
            Category::View => "keymap.cat.view",
            Category::Mixer => "keymap.cat.mixer",
        }
    }
}

impl Action {
    pub const ALL: [Action; 105] = [
        Action::PlayPause,
        Action::Stop,
        Action::Record,
        Action::ToStart,
        Action::ToggleLoop,
        Action::ToggleMetronome,
        Action::TapTempo,
        Action::TogglePatternMode,
        Action::AddMarker,
        Action::PreviousMarker,
        Action::NextMarker,
        Action::LoopBetweenMarkers,
        Action::SelectBetweenMarkers,
        Action::ToggleTempoTrack,
        Action::Panic,
        Action::NewProject,
        Action::Open,
        Action::Save,
        Action::SaveAs,
        Action::ExportWav,
        Action::DemoSongs,
        Action::Undo,
        Action::Redo,
        Action::Cut,
        Action::Copy,
        Action::Paste,
        Action::Duplicate,
        Action::SelectAll,
        Action::Delete,
        Action::Quantize,
        Action::ClearSelection,
        Action::LoopToSelection,
        Action::ZoomToSelection,
        Action::SelectPrevTrack,
        Action::SelectNextTrack,
        Action::SplitClip,
        Action::GlueClips,
        Action::ToggleClipMute,
        Action::ToggleClipLoop,
        Action::ClipColorNext,
        Action::RenameClip,
        Action::MakeUnique,
        Action::LinkClips,
        Action::SelectLinked,
        Action::LinkTo,
        Action::NudgeLeft,
        Action::NudgeRight,
        Action::NudgeLeftBar,
        Action::NudgeRightBar,
        Action::TransposeUp,
        Action::TransposeDown,
        Action::TransposeOctaveUp,
        Action::TransposeOctaveDown,
        Action::ToolDraw,
        Action::ToolPaint,
        Action::ToolSelect,
        Action::ToolDelete,
        Action::ToolMute,
        Action::ToolSlice,
        Action::ToolPlayback,
        Action::ToggleTypingPiano,
        Action::ViewPianoRoll,
        Action::ViewChannelRack,
        Action::ViewMixer,
        Action::ViewArrangement,
        Action::ToggleInspector,
        Action::ToggleBrowser,
        Action::ToggleClips,
        Action::ViewRouting,
        Action::CloseFloating,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
        Action::PickElement,
        Action::ToggleDeviceKeyboard,
        Action::MoveStripLeft,
        Action::MoveStripRight,
        Action::ToggleStripSolo,
        Action::ToggleStripMute,
        Action::ToggleMeterWave,
        Action::RouteMenu,
        Action::ArmTrack,
        Action::AddAudioTrack,
        Action::ToggleCountIn,
        Action::TogglePunch,
        Action::ShowTakes,
        Action::AutomateLastTweaked,
        Action::ToggleInstrumentWindow,
        // Round 29 (ADR 0064).
        Action::FreezeTrack,
        Action::BounceInPlace,
        Action::BounceSource,
        Action::ToggleLowLatency,
        Action::MidiLearnMode,
        Action::LinkLastTweaked,
        Action::ViewControllers,
        // Round 30 (ADR 0066).
        Action::OpenSettings,
        // Round 31 (ADR 0072).
        Action::CommandPalette,
        Action::ViewHistory,
        Action::NewFromTemplate,
        Action::SaveAsTemplate,
        Action::SaveTrackTemplate,
        // Round 31 (ADR 0068).
        Action::ToggleAutomationArm,
        Action::CycleTrackAutoMode,
        // Round 32 (ADR 0075).
        Action::ShowScore,
        Action::ViewScore,
    ];

    /// The stable id used in `keymap.toml`.
    pub fn id(self) -> &'static str {
        match self {
            Action::PlayPause => "play-pause",
            Action::Stop => "stop",
            Action::Record => "record",
            Action::ToStart => "to-start",
            Action::ToggleLoop => "toggle-loop",
            Action::ToggleMetronome => "toggle-metronome",
            Action::TapTempo => "tap-tempo",
            Action::TogglePatternMode => "toggle-pattern-mode",
            Action::AddMarker => "add-marker",
            Action::PreviousMarker => "previous-marker",
            Action::NextMarker => "next-marker",
            Action::LoopBetweenMarkers => "loop-between-markers",
            Action::SelectBetweenMarkers => "select-between-markers",
            Action::ToggleTempoTrack => "toggle-tempo-track",
            Action::FreezeTrack => "freeze-track",
            Action::BounceInPlace => "bounce-in-place",
            Action::BounceSource => "bounce-source",
            Action::ToggleLowLatency => "toggle-low-latency",
            Action::Panic => "panic",
            Action::NewProject => "new-project",
            Action::DemoSongs => "demo-songs",
            Action::Open => "open",
            Action::Save => "save",
            Action::SaveAs => "save-as",
            Action::ExportWav => "export-wav",
            Action::Undo => "undo",
            Action::Redo => "redo",
            Action::Cut => "cut",
            Action::Copy => "copy",
            Action::Paste => "paste",
            Action::Duplicate => "duplicate",
            Action::SelectAll => "select-all",
            Action::Delete => "delete",
            Action::Quantize => "quantize",
            Action::ClearSelection => "clear-selection",
            Action::LoopToSelection => "loop-to-selection",
            Action::ZoomToSelection => "zoom-to-selection",
            Action::SelectPrevTrack => "select-prev-track",
            Action::SelectNextTrack => "select-next-track",
            Action::SplitClip => "split-clip",
            Action::GlueClips => "glue-clips",
            Action::ToggleClipMute => "toggle-clip-mute",
            Action::ToggleClipLoop => "toggle-clip-loop",
            Action::ClipColorNext => "clip-color-next",
            Action::RenameClip => "rename-clip",
            Action::MakeUnique => "make-unique",
            Action::LinkClips => "link-clips",
            Action::SelectLinked => "select-linked",
            Action::LinkTo => "link-to",
            Action::NudgeLeft => "nudge-left",
            Action::NudgeRight => "nudge-right",
            Action::NudgeLeftBar => "nudge-left-bar",
            Action::NudgeRightBar => "nudge-right-bar",
            Action::TransposeUp => "transpose-up",
            Action::TransposeDown => "transpose-down",
            Action::TransposeOctaveUp => "transpose-octave-up",
            Action::TransposeOctaveDown => "transpose-octave-down",
            Action::ToolDraw => "tool-draw",
            Action::ToolPaint => "tool-paint",
            Action::ToolSelect => "tool-select",
            Action::ToolDelete => "tool-delete",
            Action::ToolMute => "tool-mute",
            Action::ToolSlice => "tool-slice",
            Action::ToolPlayback => "tool-playback",
            Action::ToggleTypingPiano => "toggle-typing-piano",
            Action::ViewPianoRoll => "view-piano-roll",
            Action::ViewChannelRack => "view-channel-rack",
            Action::ViewMixer => "view-mixer",
            Action::ViewArrangement => "view-arrangement",
            Action::ToggleInspector => "toggle-inspector",
            Action::ToggleBrowser => "toggle-browser",
            Action::ToggleClips => "toggle-clips",
            Action::ViewRouting => "view-routing",
            Action::CloseFloating => "close-floating",
            Action::ZoomIn => "zoom-in",
            Action::ZoomOut => "zoom-out",
            Action::ZoomReset => "zoom-reset",
            Action::PickElement => "pick-element",
            Action::ToggleDeviceKeyboard => "toggle-device-keyboard",
            Action::MoveStripLeft => "move-strip-left",
            Action::MoveStripRight => "move-strip-right",
            Action::ToggleStripSolo => "toggle-strip-solo",
            Action::ToggleStripMute => "toggle-strip-mute",
            Action::ToggleMeterWave => "toggle-meter-wave",
            Action::RouteMenu => "route-menu",
            Action::ArmTrack => "arm-track",
            Action::AddAudioTrack => "add-audio-track",
            Action::ToggleCountIn => "toggle-count-in",
            Action::TogglePunch => "toggle-punch",
            Action::ShowTakes => "show-takes",
            Action::AutomateLastTweaked => "automate-last-tweaked",
            Action::ToggleAutomationArm => "toggle-automation-arm",
            Action::CycleTrackAutoMode => "cycle-track-auto-mode",
            Action::ToggleInstrumentWindow => "toggle-instrument-window",
            Action::MidiLearnMode => "midi-learn-mode",
            Action::LinkLastTweaked => "link-last-tweaked",
            Action::ViewControllers => "view-controllers",
            Action::OpenSettings => "open-settings",
            Action::CommandPalette => "command-palette",
            Action::ViewHistory => "view-history",
            Action::ShowScore => "show-score",
            Action::ViewScore => "view-score",
            Action::NewFromTemplate => "new-from-template",
            Action::SaveAsTemplate => "save-as-template",
            Action::SaveTrackTemplate => "save-track-template",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    /// i18n key for the name shown in the shortcuts dialog.
    pub fn label_key(self) -> String {
        format!("action.{}", self.id())
    }

    pub fn category(self) -> Category {
        use Action::*;
        match self {
            PlayPause | Stop | Record | ToStart | ToggleLoop | ToggleMetronome | TapTempo
            | TogglePatternMode | Panic | AddMarker | PreviousMarker | NextMarker
            | LoopBetweenMarkers | ToggleAutomationArm => Category::Transport,
            SelectBetweenMarkers => Category::Arrangement,
            ToggleTempoTrack => Category::View,
            // Freezing and bouncing are the track's; low-latency monitoring the recording's
            // (round 29, ADR 0064).
            FreezeTrack | BounceInPlace | BounceSource | ToggleLowLatency => Category::Arrangement,
            NewProject | DemoSongs | Open | Save | SaveAs | ExportWav | OpenSettings
            | NewFromTemplate | SaveAsTemplate | SaveTrackTemplate => Category::File,
            Undo | Redo | Cut | Copy | Paste | Duplicate | SelectAll | Delete | Quantize
            | ClearSelection => Category::Edit,
            // Recording sits with the arrangement, where its tracks and lanes are.
            LoopToSelection | ZoomToSelection | SelectPrevTrack | SelectNextTrack | ArmTrack
            | AddAudioTrack | ShowTakes | ToggleCountIn | TogglePunch | AutomateLastTweaked
            | CycleTrackAutoMode => Category::Arrangement,
            SplitClip | GlueClips | ToggleClipMute | ToggleClipLoop | ClipColorNext
            | RenameClip | MakeUnique | LinkClips | SelectLinked | LinkTo => Category::Clips,
            NudgeLeft | NudgeRight | NudgeLeftBar | NudgeRightBar | TransposeUp | TransposeDown
            | TransposeOctaveUp | TransposeOctaveDown => Category::Notes,
            ToolDraw | ToolPaint | ToolSelect | ToolDelete | ToolMute | ToolSlice
            | ToolPlayback => Category::Tools,
            ToggleTypingPiano
            | ViewPianoRoll
            | ViewChannelRack
            | ViewMixer
            | ViewArrangement
            | ToggleInspector
            | CloseFloating
            | ZoomIn
            | ZoomOut
            | ZoomReset
            | PickElement
            | ToggleDeviceKeyboard
            | ToggleBrowser
            | ToggleClips
            | ViewRouting
            | ToggleInstrumentWindow
            | ViewControllers
            | CommandPalette
            | ViewHistory
            | ShowScore
            | ViewScore => Category::View,
            MidiLearnMode => Category::Tools,
            LinkLastTweaked => Category::Arrangement,
            MoveStripLeft | MoveStripRight | ToggleStripSolo | ToggleStripMute
            | ToggleMeterWave | RouteMenu => Category::Mixer,
        }
    }
}

// ---- bindings -----------------------------------------------------------------------

/// A key plus the modifiers that must be held with it — exactly those, no others.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Binding {
    pub key: Key,
    /// Ctrl on Windows and Linux, Cmd on macOS (egui's `Modifiers::command`).
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Binding {
    pub fn key(key: Key) -> Binding {
        Binding {
            key,
            command: false,
            shift: false,
            alt: false,
        }
    }

    pub fn cmd(key: Key) -> Binding {
        Binding {
            command: true,
            ..Binding::key(key)
        }
    }

    pub fn cmd_shift(key: Key) -> Binding {
        Binding {
            command: true,
            shift: true,
            ..Binding::key(key)
        }
    }

    pub fn shift(key: Key) -> Binding {
        Binding {
            shift: true,
            ..Binding::key(key)
        }
    }

    pub fn alt(key: Key) -> Binding {
        Binding {
            alt: true,
            ..Binding::key(key)
        }
    }

    /// Does this binding need no modifier? Those are the ones the typing piano takes over.
    pub fn is_bare(&self) -> bool {
        !self.command && !self.shift && !self.alt
    }

    /// Is this an arrow key? Those are the ones a focused fader takes over, the same way
    /// the typing piano takes the letters — a mode, not a conflict (`crate::widgets`).
    pub fn is_arrow(&self) -> bool {
        matches!(
            self.key,
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
        )
    }

    fn matches(&self, m: Modifiers) -> bool {
        // Exact match: `Ctrl+Z` must not fire on `Ctrl+Shift+Z`.
        m.command == self.command && m.shift == self.shift && m.alt == self.alt
    }

    /// What to show the user. On macOS the command modifier is Cmd, not Ctrl.
    pub fn display(&self) -> String {
        let cmd = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        let mut s = String::new();
        if self.command {
            s.push_str(cmd);
            s.push('+');
        }
        if self.shift {
            s.push_str("Shift+");
        }
        if self.alt {
            // The key is labelled Option on a Mac keyboard.
            s.push_str(if cfg!(target_os = "macos") {
                "Option+"
            } else {
                "Alt+"
            });
        }
        s.push_str(self.key.name());
        s
    }

    pub fn parse(text: &str) -> Option<Binding> {
        let mut b = Binding {
            key: Key::Space,
            command: false,
            shift: false,
            alt: false,
        };
        let mut key = None;
        for part in text.split('+') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" | "super" => b.command = true,
                "shift" => b.shift = true,
                "alt" | "option" => b.alt = true,
                _ => key = Some(Key::from_name(part)?),
            }
        }
        b.key = key?;
        Some(b)
    }
}

impl fmt::Display for Binding {
    /// The file form, which always says `Ctrl` so a keymap is portable between machines.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.command {
            write!(f, "Ctrl+")?;
        }
        if self.shift {
            write!(f, "Shift+")?;
        }
        if self.alt {
            write!(f, "Alt+")?;
        }
        write!(f, "{}", self.key.name())
    }
}

// ---- presets ------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Preset {
    Cantus,
    FlStudio,
    Live,
    Bitwig,
    /// Cantus's own map with macOS's conventions (round 30, ADR 0066): the default there.
    Mac,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Cantus,
        Preset::FlStudio,
        Preset::Live,
        Preset::Bitwig,
        Preset::Mac,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Preset::Cantus => "cantus",
            Preset::FlStudio => "fl-studio",
            Preset::Live => "live",
            Preset::Bitwig => "bitwig",
            Preset::Mac => "macos",
        }
    }

    pub fn from_id(id: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.id() == id)
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Preset::Cantus => "keymap.preset.cantus",
            Preset::FlStudio => "keymap.preset.fl",
            Preset::Live => "keymap.preset.live",
            Preset::Bitwig => "keymap.preset.bitwig",
            Preset::Mac => "keymap.preset.macos",
        }
    }

    pub fn keymap(self) -> Keymap {
        match self {
            Preset::Cantus => cantus_keymap(),
            Preset::FlStudio => fl_keymap(),
            Preset::Live => live_keymap(),
            Preset::Bitwig => bitwig_keymap(),
            Preset::Mac => mac_keymap(),
        }
    }
}

const TOOL_LETTERS: [(Action, Key); 7] = [
    (Action::ToolDraw, Key::P),
    (Action::ToolPaint, Key::B),
    (Action::ToolSelect, Key::E),
    (Action::ToolDelete, Key::D),
    (Action::ToolMute, Key::T),
    (Action::ToolSlice, Key::C),
    (Action::ToolPlayback, Key::Y),
];

const TOOL_DIGITS: [(Action, Key); 7] = [
    (Action::ToolDraw, Key::Num1),
    (Action::ToolPaint, Key::Num2),
    (Action::ToolSelect, Key::Num3),
    (Action::ToolDelete, Key::Num4),
    (Action::ToolMute, Key::Num5),
    (Action::ToolSlice, Key::Num6),
    (Action::ToolPlayback, Key::Num7),
];

/// The shared spine every preset starts from: the shortcuts that are the same in every DAW
/// worth copying, because they are the same in every application.
fn common() -> Vec<(Action, Binding)> {
    use Action::*;
    vec![
        (PlayPause, Binding::key(Key::Space)),
        (Record, Binding::key(Key::R)),
        (ToStart, Binding::key(Key::Home)),
        (NewProject, Binding::cmd(Key::N)),
        (DemoSongs, Binding::cmd_shift(Key::N)),
        (OpenSettings, Binding::cmd(Key::Comma)),
        (Open, Binding::cmd(Key::O)),
        (Save, Binding::cmd(Key::S)),
        (SaveAs, Binding::cmd_shift(Key::S)),
        (ExportWav, Binding::cmd(Key::E)),
        (Undo, Binding::cmd(Key::Z)),
        (Redo, Binding::cmd(Key::Y)),
        (Redo, Binding::cmd_shift(Key::Z)),
        (Cut, Binding::cmd(Key::X)),
        (Copy, Binding::cmd(Key::C)),
        (Paste, Binding::cmd(Key::V)),
        (SelectAll, Binding::cmd(Key::A)),
        (Delete, Binding::key(Key::Delete)),
        (Delete, Binding::key(Key::Backspace)),
        (ClearSelection, Binding::key(Key::Escape)),
        (ToggleLoop, Binding::cmd(Key::L)),
        (ToggleMetronome, Binding::cmd(Key::M)),
        // The top bar (round 28, ADR 0052): FL's L for PAT / SONG (a bare letter: the typing
        // piano takes it while it plays — the bar's switch is always there); Shift+T taps
        // the tempo (FL has no key for it; Shift keeps it clear of the tools' T).
        (TogglePatternMode, Binding::key(Key::L)),
        (TapTempo, Binding::shift(Key::T)),
        // Markers (round 29, ADR 0060): Alt and a key, so the typing piano never takes
        // them — M for marker; Alt+, and Alt+. step to the one before and after (Logic's
        // keys, with Alt); Alt+Shift+O loops between the markers around the playhead — a
        // letter, the same key on every layout (review 3: Shift+, is ';' on a Spanish or
        // German keyboard, and Alt+Shift+L is Select linked's in every preset).
        (AddMarker, Binding::alt(Key::M)),
        (PreviousMarker, Binding::alt(Key::Comma)),
        (NextMarker, Binding::alt(Key::Period)),
        (
            LoopBetweenMarkers,
            Binding {
                shift: true,
                ..Binding::alt(Key::O)
            },
        ),
        // Alt+Shift+M selects between them; Alt+Shift+T shows the tempo track.
        (
            SelectBetweenMarkers,
            Binding {
                shift: true,
                ..Binding::alt(Key::M)
            },
        ),
        (
            ToggleTempoTrack,
            Binding {
                shift: true,
                ..Binding::alt(Key::T)
            },
        ),
        // Freeze (round 29, ADR 0064): Alt+Shift+F freezes the selected track or thaws it,
        // Alt+Shift+B bounces it in place; Alt+Shift+R switches low-latency monitoring (R
        // as recording: Alt+R arms).
        (
            FreezeTrack,
            Binding {
                shift: true,
                ..Binding::alt(Key::F)
            },
        ),
        (
            BounceInPlace,
            Binding {
                shift: true,
                ..Binding::alt(Key::B)
            },
        ),
        // Ctrl+Alt+B bounces the source only (the same gesture, one modifier more).
        (
            BounceSource,
            Binding {
                command: true,
                ..Binding::alt(Key::B)
            },
        ),
        (
            ToggleLowLatency,
            Binding {
                shift: true,
                ..Binding::alt(Key::R)
            },
        ),
        (ToggleTypingPiano, Binding::cmd(Key::T)),
        // FL's function keys for the three central views. They are the most widely known
        // — every preset gets them, because arriving from anywhere else they are simply
        // free keys, and a view you cannot reach from the keyboard is a view you forget
        // is there.
        // The time selection's two operations. Live puts loop-to-selection on `Ctrl+L`,
        // which is ours for the loop *switch* — and a switch has to stay reachable when
        // there is no selection, so the two cannot share a key. `Z` for zoom-to-selection
        // is Live's, and it is the one most people arrive with.
        (LoopToSelection, Binding::shift(Key::L)),
        (ZoomToSelection, Binding::key(Key::Z)),
        // `Ctrl` + the arrows, so they do not collide with nudging notes or clips: the
        // modifier match is exact, so `Ctrl+ArrowUp` never fires `TransposeUp`.
        (SelectPrevTrack, Binding::cmd(Key::ArrowUp)),
        (SelectNextTrack, Binding::cmd(Key::ArrowDown)),
        (ViewArrangement, Binding::key(Key::F5)),
        (ViewChannelRack, Binding::key(Key::F6)),
        (ViewPianoRoll, Binding::key(Key::F7)),
        (ViewMixer, Binding::key(Key::F9)),
        // The workspace (ADR 0024 §5, §10, §11): I shows and hides the inspector, F12
        // closes the floating device windows (FL), Ctrl +/−/0 zoom the whole interface,
        // Ctrl+Shift+I picks an element to recolour (the browser's inspector key).
        //
        // Bare letters yield to the typing piano while it plays (`Keymap::pressed`), so
        // every bare letter here also has a modifier form that always works: Alt+I for
        // the inspector, Alt+K for an instrument's keyboard card. The piano itself never
        // reads a key held with a modifier (`KeyboardPiano::poll`).
        (ToggleInspector, Binding::key(Key::I)),
        (ToggleInspector, Binding::alt(Key::I)),
        (ToggleBrowser, Binding::alt(Key::F8)),
        // The clip browser beside the arrangement: F5 is the arrangement (FL's playlist,
        // whose picker panel it is).
        (ToggleClips, Binding::alt(Key::F5)),
        // The Routing tab beside the mixer's F9 (the owner's answer 9, round 25).
        (ViewRouting, Binding::alt(Key::F9)),
        (ToggleDeviceKeyboard, Binding::alt(Key::K)),
        (CloseFloating, Binding::key(Key::F12)),
        // Round 20: the selected track's instrument window, open or closed, from anywhere.
        (ToggleInstrumentWindow, Binding::key(Key::F8)),
        (ZoomIn, Binding::cmd(Key::Plus)),
        (ZoomIn, Binding::cmd(Key::Equals)),
        (ZoomOut, Binding::cmd(Key::Minus)),
        (ZoomReset, Binding::cmd(Key::Num0)),
        (PickElement, Binding::cmd_shift(Key::I)),
        // M and S act on the selected strip (FL's mixer keys); they are bare letters, so
        // the typing piano takes them while it plays.
        (ToggleStripMute, Binding::key(Key::M)),
        (ToggleStripSolo, Binding::key(Key::S)),
        (NudgeLeft, Binding::key(Key::ArrowLeft)),
        (NudgeRight, Binding::key(Key::ArrowRight)),
        (NudgeLeftBar, Binding::shift(Key::ArrowLeft)),
        (NudgeRightBar, Binding::shift(Key::ArrowRight)),
        (TransposeUp, Binding::key(Key::ArrowUp)),
        (TransposeDown, Binding::key(Key::ArrowDown)),
        (TransposeOctaveUp, Binding::shift(Key::ArrowUp)),
        (TransposeOctaveDown, Binding::shift(Key::ArrowDown)),
        // FL's keys for moving a mixer track along the desk; `Alt` keeps them clear of
        // the nudges. Solo and mute on a strip are in its menu and stay unbound: the
        // letters belong to the tools and the typing piano.
        (MoveStripLeft, Binding::alt(Key::ArrowLeft)),
        (MoveStripRight, Binding::alt(Key::ArrowRight)),
        // FL's `Alt+W`.
        (ToggleMeterWave, Binding::alt(Key::W)),
        // The selected strip's route menu: the platform's context-menu key, free in every
        // preset and never a letter the typing piano wants (round 25).
        (RouteMenu, Binding::shift(Key::F10)),
        // Recording (ADR 0025): Alt and a letter, so the typing piano never takes them.
        // R records; Alt+R arms the selected track (Cubase's "R" on the track, moved off
        // the bare letter), Alt+C counts in, Alt+P punches, Alt+T shows the takes.
        (ArmTrack, Binding::alt(Key::R)),
        (ToggleCountIn, Binding::alt(Key::C)),
        (TogglePunch, Binding::alt(Key::P)),
        (ShowTakes, Binding::alt(Key::T)),
        // FL's Tools ▸ Last tweaked ▸ Create automation clip (ADR 0027): A for automate.
        (AutomateLastTweaked, Binding::alt(Key::A)),
        // Round 31 (ADR 0068): the selected track's automation mode, W for write.
        (
            CycleTrackAutoMode,
            Binding {
                shift: true,
                ..Binding::alt(Key::W)
            },
        ),
        // MIDI controllers (round 29, ADR 0062): Ctrl+Shift+M for the learn mode (Live's
        // Cmd+M, which is our metronome), Alt+Shift+A links the last tweaked parameter
        // beside Alt+A's automation clip, Ctrl+Shift+K shows the mappings (K for knobs).
        (MidiLearnMode, Binding::cmd_shift(Key::M)),
        (
            LinkLastTweaked,
            Binding {
                shift: true,
                ..Binding::alt(Key::A)
            },
        ),
        (ViewControllers, Binding::cmd_shift(Key::K)),
        (AddAudioTrack, Binding::cmd_shift(Key::A)),
        // The command palette (round 31, ADR 0072): VS Code's Ctrl+Shift+P — free in every
        // preset, on Windows, Linux and macOS (⌘⇧P), and never a letter the typing piano
        // wants. The undo history beside it: Alt+H (H for history; Alt keeps it from the
        // typing piano).
        (CommandPalette, Binding::cmd_shift(Key::P)),
        (ViewHistory, Binding::alt(Key::H)),
        // The score under the piano roll (round 32, ADR 0075): Alt+S, S for score — free in
        // every preset; Alt keeps it from the typing piano.
        (ShowScore, Binding::alt(Key::S)),
        // The Score pane beside it: Alt+Shift+S.
        (
            ViewScore,
            Binding {
                shift: true,
                ..Binding::alt(Key::S)
            },
        ),
    ]
}

/// The clip shortcuts every preset gets.
///
/// `Ctrl+E` for split and `Ctrl+J` for glue are Live's and Logic's, and they are what most
/// people arrive with. Splitting a clip is something a musician does a hundred times an
/// hour and exporting a WAV a handful of times a day, so where the two collide it is the
/// export that moves — to `Ctrl+Shift+E`, which is where the Live preset already had it.
fn clip_bindings() -> Vec<(Action, Binding)> {
    use Action::*;
    vec![
        (SplitClip, Binding::cmd(Key::E)),
        (ExportWav, Binding::cmd_shift(Key::E)),
        (GlueClips, Binding::cmd(Key::J)),
        // Live mutes a clip with `0`; the bare digit is free because the tools took 1-7.
        (ToggleClipMute, Binding::key(Key::Num0)),
        // Repeat is the clip's own loop, so it sits beside the transport's `Ctrl+L`.
        (ToggleClipLoop, Binding::cmd_shift(Key::L)),
        (ClipColorNext, Binding::cmd_shift(Key::C)),
        // F2 renames the thing that is selected, everywhere, in everything.
        (RenameClip, Binding::key(Key::F2)),
        // Linked clips (ADR 0037): Alt and a letter, so the typing piano never takes them
        // — U for unique, L for link.
        (MakeUnique, Binding::alt(Key::U)),
        (LinkClips, Binding::alt(Key::L)),
        (
            SelectLinked,
            Binding {
                shift: true,
                ..Binding::alt(Key::L)
            },
        ),
        // Link to a group chosen by name (round 26): N for name.
        (LinkTo, Binding::alt(Key::N)),
    ]
}

/// Build a preset from the shared spine plus its own bindings.
///
/// A later binding takes the shortcut from whoever held it, so a preset can never conflict
/// with itself: a preset that wants `Ctrl+E` for its own action simply displaces the spine's
/// use of it, and is then expected to give that action another shortcut. The test
/// `no_shipped_preset_conflicts_with_itself` is what keeps that honest.
fn build(preset: Preset, extra: Vec<(Action, Binding)>) -> Keymap {
    let mut map = Keymap {
        preset: preset.id().to_string(),
        bindings: BTreeMap::new(),
    };
    for (action, binding) in common().into_iter().chain(extra) {
        for list in map.bindings.values_mut() {
            list.retain(|b| *b != binding);
        }
        map.bindings.entry(action).or_default().push(binding);
    }
    map.bindings.retain(|_, v| !v.is_empty());
    map
}

/// Cantus's own map: FL's tool letters, which are the most widely copied, plus number keys
/// so the tools stay reachable while the typing piano owns the letters.
pub fn cantus_keymap() -> Keymap {
    let mut extra: Vec<(Action, Binding)> = clip_bindings();
    extra.extend(vec![
        (Action::Duplicate, Binding::cmd(Key::B)),
        (Action::Duplicate, Binding::cmd(Key::D)),
        (Action::Quantize, Binding::key(Key::Q)),
    ]);
    extra.extend(TOOL_LETTERS.map(|(a, k)| (a, Binding::key(k))));
    extra.extend(TOOL_DIGITS.map(|(a, k)| (a, Binding::key(k))));
    build(Preset::Cantus, extra)
}

/// Modelled on FL Studio: the tool letters and `Ctrl+B` duplicate.
pub fn fl_keymap() -> Keymap {
    let mut extra: Vec<(Action, Binding)> = clip_bindings();
    extra.extend([
        (Action::Duplicate, Binding::cmd(Key::B)),
        (Action::Quantize, Binding::key(Key::Q)),
    ]);
    extra.extend(TOOL_LETTERS.map(|(a, k)| (a, Binding::key(k))));
    build(Preset::FlStudio, extra)
}

/// Modelled on Ableton Live: `Ctrl+D` duplicate, `Ctrl+U` quantize, `B` for the pencil,
/// `V` for the select tool. Split and glue come from [`clip_bindings`], which is Live's
/// layout to begin with.
pub fn live_keymap() -> Keymap {
    let mut extra: Vec<(Action, Binding)> = clip_bindings();
    extra.extend([
        (Action::Duplicate, Binding::cmd(Key::D)),
        (Action::Quantize, Binding::cmd(Key::U)),
        (Action::ToolDraw, Binding::key(Key::B)),
        (Action::ToolSelect, Binding::key(Key::V)),
    ]);
    build(Preset::Live, extra)
}

/// Modelled on Bitwig Studio: tools on the number row, `Ctrl+D` duplicate.
pub fn bitwig_keymap() -> Keymap {
    let mut extra: Vec<(Action, Binding)> = clip_bindings();
    extra.extend([
        (Action::Duplicate, Binding::cmd(Key::D)),
        (Action::Quantize, Binding::cmd(Key::Q)),
    ]);
    extra.extend(TOOL_DIGITS.map(|(a, k)| (a, Binding::key(k))));
    build(Preset::Bitwig, extra)
}

/// Cantus's map with macOS's conventions (round 30, ADR 0066). The command modifier is
/// already ⌘ there (egui's `command`), so the map is Cantus's but for the chords macOS keeps
/// for itself: ⌘M minimises (the metronome takes ⌘K, and Logic's K while the typing piano
/// is off), and ⌘Y is not Redo on a Mac
/// (⌘⇧Z is, and stays). ⌘, opens the settings, as in every Mac application (all presets).
pub fn mac_keymap() -> Keymap {
    let mut map = cantus_keymap();
    map.preset = Preset::Mac.id().to_string();
    let swap = |map: &mut Keymap, action: Action, old: Binding, new: Option<Binding>| {
        if let Some(list) = map.bindings.get_mut(&action) {
            list.retain(|b| *b != old);
            if let Some(new) = new {
                if !list.contains(&new) {
                    list.push(new);
                }
            }
        }
        map.bindings.retain(|_, v| !v.is_empty());
    };
    swap(
        &mut map,
        Action::ToggleMetronome,
        Binding::cmd(Key::M),
        Some(Binding::cmd(Key::K)),
    );
    // Logic's bare K too, while the typing piano is off (it owns the letters when on).
    swap(
        &mut map,
        Action::ToggleMetronome,
        Binding::cmd(Key::K),
        Some(Binding::key(Key::K)),
    );
    if let Some(list) = map.bindings.get_mut(&Action::ToggleMetronome) {
        list.insert(0, Binding::cmd(Key::K));
    }
    swap(&mut map, Action::Redo, Binding::cmd(Key::Y), None);
    map
}

/// The chords macOS keeps for itself (its menus and the system's own shortcuts): a binding
/// on one of them never reaches Cantus on a Mac. ⌘, is the settings' own, so it is not here.
pub fn mac_reserved() -> [Binding; 12] {
    let cmd_alt = |key| Binding {
        alt: true,
        ..Binding::cmd(key)
    };
    [
        Binding::cmd(Key::Q),
        Binding::cmd(Key::H),
        Binding::cmd(Key::M),
        Binding::cmd(Key::W),
        Binding::cmd(Key::Tab),
        Binding::cmd(Key::Space),
        Binding::cmd(Key::Backtick),
        cmd_alt(Key::H),
        cmd_alt(Key::M),
        cmd_alt(Key::Escape),
        Binding::cmd_shift(Key::Num3),
        Binding::cmd_shift(Key::Num4),
    ]
}

// ---- the map ------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Keymap {
    /// Which preset this started from. Purely informational once the user edits it.
    pub preset: String,
    pub bindings: BTreeMap<Action, Vec<Binding>>,
}

impl Default for Keymap {
    fn default() -> Self {
        cantus_keymap()
    }
}

impl Keymap {
    /// The map a first start writes: the macOS preset on a Mac, Cantus's elsewhere.
    pub fn for_this_system() -> Keymap {
        if cfg!(target_os = "macos") {
            mac_keymap()
        } else {
            cantus_keymap()
        }
    }
}

/// One binding claimed by more than one action.
#[derive(Clone, Debug, PartialEq)]
pub struct Conflict {
    pub binding: Binding,
    pub actions: Vec<Action>,
}

impl Keymap {
    pub fn bindings_for(&self, action: Action) -> &[Binding] {
        self.bindings.get(&action).map_or(&[], |v| v.as_slice())
    }

    /// The bindings for `action`, joined for display: "Ctrl+B, Ctrl+D".
    pub fn display_for(&self, action: Action) -> String {
        self.bindings_for(action)
            .iter()
            .map(|b| b.display())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Give `action` this binding, taking it away from whoever else had it.
    ///
    /// Rebinding is how conflicts are avoided in the first place; [`Self::conflicts`]
    /// exists for maps that arrive already broken, from a hand-edited file.
    pub fn bind(&mut self, action: Action, binding: Binding) {
        for list in self.bindings.values_mut() {
            list.retain(|b| *b != binding);
        }
        self.bindings.entry(action).or_default().push(binding);
        self.bindings.retain(|_, v| !v.is_empty());
        self.preset = CUSTOM.to_string();
    }

    pub fn unbind(&mut self, action: Action, binding: Binding) {
        if let Some(list) = self.bindings.get_mut(&action) {
            list.retain(|b| *b != binding);
            if list.is_empty() {
                self.bindings.remove(&action);
            }
        }
        self.preset = CUSTOM.to_string();
    }

    /// Bindings claimed by more than one action, with the actions that claim them. Empty
    /// for every shipped preset — a test asserts that.
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut by_binding: BTreeMap<Binding, Vec<Action>> = BTreeMap::new();
        for (action, list) in &self.bindings {
            for b in list {
                by_binding.entry(*b).or_default().push(*action);
            }
        }
        by_binding
            .into_iter()
            .filter(|(_, actions)| actions.len() > 1)
            .map(|(binding, actions)| Conflict { binding, actions })
            .collect()
    }

    /// Which actions fired this frame.
    ///
    /// `letters_taken` is true while the typing piano owns the letter keys; bindings with
    /// no modifier are then ignored, which is why every tool also has a number binding.
    /// The actions whose shortcut was pressed this frame.
    ///
    /// `letters_taken` is the typing piano owning the bare letters; `arrows_taken` is a
    /// focused widget owning the arrow keys. Both are modes, not conflicts, and both are
    /// resolved here rather than at the call sites, so no key is ever suppressed by an
    /// action's *identity* — only by the key the user actually pressed. A user who binds
    /// Save to `ArrowUp` gets the same rule as the shipped nudge does.
    pub fn pressed(
        &self,
        ctx: &egui::Context,
        letters_taken: bool,
        arrows_taken: bool,
    ) -> Vec<Action> {
        ctx.input(|i| {
            let m = i.modifiers;
            // Ctrl+C, Ctrl+X and Ctrl+V never arrive as key presses (`clipboard_binding`).
            let chords: Vec<Binding> = i
                .events
                .iter()
                .filter_map(|e| clipboard_binding(e, m))
                .collect();
            let mut fired = Vec::new();
            for (action, list) in &self.bindings {
                for b in list {
                    if letters_taken && b.is_bare() && is_letter(b.key) {
                        continue;
                    }
                    if arrows_taken && b.is_arrow() {
                        continue;
                    }
                    if (b.matches(m) && i.key_pressed(b.key)) || chords.contains(b) {
                        fired.push(*action);
                        break;
                    }
                }
            }
            fired
        })
    }

    // ---- file ----

    pub fn load(path: &Path) -> Keymap {
        match std::fs::read_to_string(path) {
            Ok(text) => Keymap::from_toml(&text).unwrap_or_else(|e| {
                log::warn!("keymap.toml is not usable ({e}); falling back to the default");
                Keymap::for_this_system()
            }),
            Err(_) => Keymap::for_this_system(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, self.to_toml()).map_err(|e| e.to_string())
    }

    pub fn from_toml(text: &str) -> Result<Keymap, String> {
        let file: KeymapFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut bindings: BTreeMap<Action, Vec<Binding>> = BTreeMap::new();
        for (id, keys) in file.bindings {
            // An unknown id is a keymap from a newer version, or a typo. Neither is worth
            // refusing to start over; keep the rest and say so.
            let Some(action) = Action::from_id(&id) else {
                log::warn!("keymap.toml: unknown action {id:?}, ignored");
                continue;
            };
            for text in keys {
                match Binding::parse(&text) {
                    Some(b) => bindings.entry(action).or_default().push(b),
                    None => log::warn!("keymap.toml: cannot read the shortcut {text:?}, ignored"),
                }
            }
        }
        let mut map = Keymap {
            preset: file.preset.unwrap_or_else(|| CUSTOM.to_string()),
            bindings,
        };
        if file.version.unwrap_or(1) < 2 {
            map.fill_in(&NEW_IN_V2);
        }
        if file.version.unwrap_or(1) < 3 {
            map.fill_in(&NEW_IN_V3);
        }
        if file.version.unwrap_or(1) < 4 {
            map.fill_in(&NEW_IN_V4);
        }
        if file.version.unwrap_or(1) < 5 {
            map.fill_in(&NEW_IN_V5);
        }
        if file.version.unwrap_or(1) < 6 {
            map.fill_in(&NEW_IN_V6);
        }
        if file.version.unwrap_or(1) < 7 {
            map.fill_in(&NEW_IN_V7);
        }
        if file.version.unwrap_or(1) < 8 {
            map.fill_in(&NEW_IN_V8);
        }
        if file.version.unwrap_or(1) < 9 {
            map.fill_in(&NEW_IN_V9);
        }
        if file.version.unwrap_or(1) < 10 {
            map.fill_in(&NEW_IN_V10);
        }
        if file.version.unwrap_or(1) < 11 {
            map.fill_in(&NEW_IN_V11);
        }
        if file.version.unwrap_or(1) < 12 {
            map.fill_in(&NEW_IN_V12);
        }
        if file.version.unwrap_or(1) < 13 {
            map.fill_in(&NEW_IN_V13);
        }
        Ok(map)
    }

    /// Give `actions` the bindings their preset has for them (Cantus's for a custom map),
    /// wherever the key is not taken. Leaves the preset name alone.
    fn fill_in(&mut self, actions: &[Action]) {
        let source = Preset::from_id(&self.preset)
            .unwrap_or(Preset::Cantus)
            .keymap();
        for action in actions {
            if self.bindings.contains_key(action) {
                continue;
            }
            for b in source.bindings_for(*action) {
                let taken = self.bindings.values().any(|list| list.contains(b));
                if !taken {
                    self.bindings.entry(*action).or_default().push(*b);
                }
            }
        }
    }

    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("# Cantus keyboard map. Edit freely; the app reads it at startup.\n");
        out.push_str("# A shortcut is a key plus modifiers, e.g. \"Ctrl+Shift+S\" or \"Space\".\n");
        out.push_str("# Modifiers are Ctrl, Shift and Alt; on macOS Ctrl means Cmd.\n");
        out.push_str("# An action may have several shortcuts. Delete a line to unbind it.\n\n");
        out.push_str(&format!(
            "preset = {:?}\nversion = {FILE_VERSION}\n\n[bindings]\n",
            self.preset
        ));
        // Grouped by category, in the order the shortcuts dialog shows them, so a hand
        // edit lands near the shortcuts it belongs with.
        for category in Category::ALL {
            let mut wrote_header = false;
            for action in Action::ALL.into_iter().filter(|a| a.category() == category) {
                let list = self.bindings_for(action);
                if list.is_empty() {
                    continue;
                }
                if !wrote_header {
                    out.push_str(&format!("\n# {category:?}\n"));
                    wrote_header = true;
                }
                let keys: Vec<String> = list
                    .iter()
                    .map(|b| format!("{:?}", b.to_string()))
                    .collect();
                out.push_str(&format!("{} = [{}]\n", action.id(), keys.join(", ")));
            }
        }
        out
    }
}

/// The `preset` value written once the user has edited their map by hand or in the dialog.
pub const CUSTOM: &str = "custom";

#[derive(Serialize, Deserialize)]
struct KeymapFile {
    preset: Option<String>,
    /// Absent in files written before round 18.
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    bindings: BTreeMap<String, Vec<String>>,
}

/// The file format. Version 2 added the workspace's actions (round 18); version 3 the
/// recording, demo, sample-browser and automation actions (rounds 19–20); version 4 the
/// linked-clip actions (round 24, ADR 0037); version 5 the Routing tab (round 25);
/// version 6 the route menu's key (round 25's integration); version 7 *Link to…* (round 26);
/// version 8 tap tempo and PAT / SONG (round 28); version 9 the markers' and the tempo
/// track's actions (round 29); version 10 the settings' key (round 30, ADR 0066); version
/// 11 the command palette's and the undo history's (round 31, ADR 0072); version 12 the
/// automation mode's key (round 31, ADR 0068); version 13 the score's (round 32, ADR 0075).
pub const FILE_VERSION: u32 = 13;

/// Actions added in version 13 (round 32, ADR 0075): Alt+S shows the staff under the piano
/// roll, Alt+Shift+S the Score pane.
const NEW_IN_V13: [Action; 2] = [Action::ShowScore, Action::ViewScore];

/// Actions added in version 11 (round 31): Ctrl+Shift+P opens the command palette, Alt+H
/// shows the undo history (ADR 0072).
const NEW_IN_V11: [Action; 2] = [Action::CommandPalette, Action::ViewHistory];

/// Actions added in version 10 (round 30): ⌘, / Ctrl+, opens the settings (ADR 0066).
const NEW_IN_V10: [Action; 1] = [Action::OpenSettings];

/// Actions added in version 12 (round 31, ADR 0068): Alt+Shift+W cycles the selected track's
/// automation mode (W for write; Alt+W is the meters' wave). Version 11 is the command
/// palette's.
const NEW_IN_V12: [Action; 1] = [Action::CycleTrackAutoMode];

/// Actions added in version 9 (round 29): the tempo builder's markers and tempo track (ADR
/// 0060); the other round-29 builders append theirs here.
const NEW_IN_V9: [Action; 13] = [
    Action::AddMarker,
    Action::PreviousMarker,
    Action::NextMarker,
    Action::LoopBetweenMarkers,
    Action::SelectBetweenMarkers,
    Action::ToggleTempoTrack,
    // Freeze (round 29, ADR 0064).
    Action::FreezeTrack,
    Action::BounceInPlace,
    Action::BounceSource,
    Action::ToggleLowLatency,
    // The controllers builder's (ADR 0062).
    Action::MidiLearnMode,
    Action::LinkLastTweaked,
    Action::ViewControllers,
];

/// Actions added in version 8: the top bar's tap tempo (Shift+T) and PAT / SONG (L), round
/// 28, ADR 0052.
const NEW_IN_V8: [Action; 2] = [Action::TapTempo, Action::TogglePatternMode];

/// Actions added in version 7: the *Link to…* picker (Alt+N) and the clip browser (Alt+F5),
/// ADR 0037.
const NEW_IN_V7: [Action; 2] = [Action::LinkTo, Action::ToggleClips];

/// Actions added in version 6: the selected strip's route menu (Shift+F10).
const NEW_IN_V6: [Action; 1] = [Action::RouteMenu];

/// Actions added in version 5: the Routing tab's key.
const NEW_IN_V5: [Action; 1] = [Action::ViewRouting];

/// Actions added in version 4: the linked clips' Make unique, Link and Select linked.
const NEW_IN_V4: [Action; 3] = [Action::MakeUnique, Action::LinkClips, Action::SelectLinked];

/// Actions added in version 3. Round 19's keys shipped without a version bump, so a file
/// written by round 18 never got them; version 3 fills them in too.
const NEW_IN_V3: [Action; 9] = [
    Action::AddAudioTrack,
    Action::ArmTrack,
    Action::ShowTakes,
    Action::ToggleCountIn,
    Action::TogglePunch,
    Action::DemoSongs,
    Action::ToggleBrowser,
    Action::AutomateLastTweaked,
    Action::ToggleInstrumentWindow,
];

/// Actions added in version 2: a file written before them gets the preset's bindings for
/// them, where those keys are still free — an old file never mentions them, and "no line"
/// would otherwise mean "unbound" for good.
const NEW_IN_V2: [Action; 10] = [
    Action::ViewArrangement,
    Action::ToggleInspector,
    Action::CloseFloating,
    Action::ZoomIn,
    Action::ZoomOut,
    Action::ZoomReset,
    Action::PickElement,
    Action::ToggleDeviceKeyboard,
    Action::ToggleStripMute,
    Action::ToggleStripSolo,
];

/// The chord behind a clipboard event, given the modifiers held.
///
/// egui-winit turns every Ctrl+C, Ctrl+X and Ctrl+V — with Shift or Alt too — into
/// `Event::Copy`, `Event::Cut` and `Event::Paste(text)` and sends **no** key press for them
/// (and on Windows Ctrl+Ins, Shift+Del and Shift+Ins as well). A binding is matched against
/// the chord the user pressed: Ctrl+Shift+C stays Ctrl+Shift+C. The chords without Ctrl
/// (Shift+Del, Shift+Ins, a keyboard's own Copy key) are the platform's spellings of the
/// plain one. A `Paste` event that answers a menu's request is taken out of the input
/// before this runs (`crate::clipboard`).
pub fn clipboard_binding(event: &egui::Event, m: Modifiers) -> Option<Binding> {
    let key = match event {
        egui::Event::Copy => Key::C,
        egui::Event::Cut => Key::X,
        egui::Event::Paste(_) => Key::V,
        _ => return None,
    };
    Some(if m.command {
        Binding {
            key,
            command: true,
            shift: m.shift,
            alt: m.alt,
        }
    } else {
        Binding::cmd(key)
    })
}

/// The chord a clipboard event records in the shortcuts dialog: only one made with Ctrl.
///
/// Shift+Del, Shift+Ins and a keyboard's own Cut/Copy/Paste keys also arrive as these
/// events, with no key press, so recording them as Ctrl+X / Ctrl+V would bind a chord the
/// user never pressed: they are not recorded at all (they still work as Cut and Paste).
/// Ctrl+Ins cannot be told from Ctrl+C — both arrive as `Event::Copy` with Ctrl held — so
/// it records as Ctrl+C, and a binding of Ctrl+Ins never fires.
pub fn captured_clipboard_binding(event: &egui::Event, m: Modifiers) -> Option<Binding> {
    if !m.command {
        return None;
    }
    clipboard_binding(event, m)
}

fn is_letter(key: Key) -> bool {
    matches!(
        key,
        Key::A
            | Key::B
            | Key::C
            | Key::D
            | Key::E
            | Key::F
            | Key::G
            | Key::H
            | Key::I
            | Key::J
            | Key::K
            | Key::L
            | Key::M
            | Key::N
            | Key::O
            | Key::P
            | Key::Q
            | Key::R
            | Key::S
            | Key::T
            | Key::U
            | Key::V
            | Key::W
            | Key::X
            | Key::Y
            | Key::Z
    )
}

/// Where the keymap lives, next to the theme and the layouts.
pub fn keymap_path(dir: &Path) -> PathBuf {
    dir.join("keymap.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review 2 (findings 5, 14, 31): every action new in version 9 has a key in every
    /// shipped preset — a later binding of the same chord (Select linked) once took Loop
    /// between markers' away in all four.
    /// Review 3 (findings 12, 23, 38): *Loop between markers* fires from the event a
    /// Spanish (or German, or French) keyboard sends — a letter is the same key there.
    #[test]
    fn loop_between_markers_fires_on_every_layout() {
        for preset in Preset::ALL {
            let map = preset.keymap();
            let ctx = egui::Context::default();
            let mods = egui::Modifiers {
                alt: true,
                shift: true,
                ..Default::default()
            };
            let input = egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(mods),
                    egui::Event::Key {
                        key: Key::O,
                        physical_key: Some(Key::O),
                        pressed: true,
                        repeat: false,
                        modifiers: mods,
                    },
                ],
                ..Default::default()
            };
            let mut fired = Vec::new();
            ctx.run_ui(input, |ui| fired = map.pressed(ui.ctx(), false, false))
                .textures_delta
                .clear();
            assert!(
                fired.contains(&Action::LoopBetweenMarkers),
                "{}: {fired:?}",
                preset.id()
            );
        }
    }

    #[test]
    fn every_new_action_has_a_key_in_every_preset() {
        for preset in Preset::ALL {
            let map = preset.keymap();
            for a in NEW_IN_V9 {
                assert!(
                    !map.bindings_for(a).is_empty(),
                    "{a:?} has no key in {}",
                    preset.id()
                );
            }
        }
    }

    #[test]
    fn every_action_has_a_stable_id_and_a_category() {
        let mut seen = std::collections::HashSet::new();
        for a in Action::ALL {
            assert!(seen.insert(a.id()), "duplicate action id {:?}", a.id());
            assert_eq!(Action::from_id(a.id()), Some(a), "id {:?}", a.id());
            // Every action must be reachable from its category, or the dialog hides it.
            assert!(Category::ALL.contains(&a.category()));
        }
        assert_eq!(seen.len(), Action::ALL.len());
        assert_eq!(Action::from_id("no-such-action"), None);
    }

    #[test]
    fn bindings_round_trip_through_text() {
        // The property that matters: whatever we write, we can read back — for every key
        // egui knows, not just the handful someone thought to list.
        for key in Key::ALL {
            for (command, shift, alt) in [
                (false, false, false),
                (true, false, false),
                (true, true, false),
                (false, false, true),
                (true, true, true),
            ] {
                let b = Binding {
                    key: *key,
                    command,
                    shift,
                    alt,
                };
                assert_eq!(Binding::parse(&b.to_string()), Some(b), "round trip of {b}");
            }
        }
        // And the spellings that end up in the file, so the format stays recognisable.
        assert_eq!(Binding::key(Key::Space).to_string(), "Space");
        assert_eq!(Binding::cmd(Key::Z).to_string(), "Ctrl+Z");
        assert_eq!(Binding::cmd_shift(Key::S).to_string(), "Ctrl+Shift+S");
        // Aliases people will actually type.
        assert_eq!(Binding::parse("cmd+z"), Binding::parse("Ctrl+Z"));
        assert_eq!(Binding::parse("Esc"), Binding::parse("Escape"));
        assert_eq!(Binding::parse("ArrowLeft"), Binding::parse("Left"));
        assert_eq!(Binding::parse("nonsense+Z"), None);
        assert_eq!(
            Binding::parse("Ctrl"),
            None,
            "a modifier alone is not a shortcut"
        );
    }

    /// The actions one frame with `events` fires (Ctrl held unless the events say so).
    fn fired_by(map: &Keymap, events: Vec<egui::Event>) -> Vec<Action> {
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| out = map.pressed(ui.ctx(), false, false))
            .textures_delta
            .clear();
        out
    }

    fn held(m: Modifiers) -> egui::Event {
        egui::Event::ModifiersChanged(m)
    }

    /// egui-winit sends `Event::Copy`, `Event::Cut` and `Event::Paste` *instead of* key
    /// presses for every Ctrl+C, Ctrl+X and Ctrl+V (Shift and Alt included) — so without
    /// reading them, those chords never reached any binding in the real window.
    #[test]
    fn clipboard_events_fire_cut_copy_paste() {
        let map = Keymap::default();
        let ctrl = Modifiers::COMMAND;
        assert!(fired_by(&map, vec![held(ctrl), egui::Event::Copy]).contains(&Action::Copy));
        assert!(fired_by(&map, vec![held(ctrl), egui::Event::Cut]).contains(&Action::Cut));
        assert!(
            fired_by(&map, vec![held(ctrl), egui::Event::Paste("x".into())])
                .contains(&Action::Paste)
        );
        // Windows' other chords (Shift+Del, Ctrl+Ins, Shift+Ins) mean the same.
        assert!(
            fired_by(&map, vec![held(Modifiers::SHIFT), egui::Event::Cut]).contains(&Action::Cut)
        );
        // A key press and its event in one frame (a test that sends both) fire once.
        let key = egui::Event::Key {
            key: Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: ctrl,
        };
        let both = fired_by(&map, vec![held(ctrl), key, egui::Event::Copy]);
        assert_eq!(both.iter().filter(|a| **a == Action::Copy).count(), 1);
        // Ctrl+Shift+C arrives as a copy event too, and is the clip colour, not Copy.
        let shifted = fired_by(&map, vec![held(ctrl | Modifiers::SHIFT), egui::Event::Copy]);
        assert!(shifted.contains(&Action::ClipColorNext), "{shifted:?}");
        assert!(!shifted.contains(&Action::Copy));
        // Copy rebound away from Ctrl+C: the event follows the binding.
        let mut moved = Keymap::default();
        moved.unbind(Action::Copy, Binding::cmd(Key::C));
        moved.bind(Action::Copy, Binding::cmd(Key::Insert));
        assert!(!fired_by(&moved, vec![held(ctrl), egui::Event::Copy]).contains(&Action::Copy));
        assert_eq!(
            clipboard_binding(&egui::Event::Copy, Modifiers::NONE),
            Some(Binding::cmd(Key::C))
        );
        assert_eq!(
            clipboard_binding(&egui::Event::Text("c".into()), ctrl),
            None
        );
    }

    /// The shortcuts dialog records only what was pressed: Shift+Del arrives as a cut
    /// event and must not be recorded as Ctrl+X, nor Shift+Ins as Ctrl+V.
    #[test]
    fn capture_records_clipboard_events_only_with_ctrl() {
        let ctrl = Modifiers::COMMAND;
        let paste = egui::Event::Paste("x".into());
        assert_eq!(
            captured_clipboard_binding(&egui::Event::Cut, Modifiers::SHIFT),
            None
        );
        assert_eq!(captured_clipboard_binding(&paste, Modifiers::SHIFT), None);
        assert_eq!(
            captured_clipboard_binding(&egui::Event::Copy, Modifiers::NONE),
            None
        );
        assert_eq!(
            captured_clipboard_binding(&egui::Event::Copy, ctrl),
            Some(Binding::cmd(Key::C))
        );
        assert_eq!(
            captured_clipboard_binding(&egui::Event::Copy, ctrl | Modifiers::SHIFT),
            Some(Binding::cmd_shift(Key::C))
        );
        assert_eq!(
            captured_clipboard_binding(&paste, ctrl),
            Some(Binding::cmd(Key::V))
        );
    }

    #[test]
    fn modifiers_must_match_exactly() {
        let undo = Binding::cmd(Key::Z);
        let redo = Binding::cmd_shift(Key::Z);
        let cmd = Modifiers {
            command: true,
            ..Default::default()
        };
        let cmd_shift = Modifiers {
            command: true,
            shift: true,
            ..Default::default()
        };
        assert!(undo.matches(cmd));
        assert!(
            !undo.matches(cmd_shift),
            "Ctrl+Z must not fire on Ctrl+Shift+Z"
        );
        assert!(redo.matches(cmd_shift));
        assert!(!redo.matches(cmd));
    }

    #[test]
    fn no_shipped_preset_conflicts_with_itself() {
        for preset in Preset::ALL {
            let map = preset.keymap();
            let conflicts = map.conflicts();
            assert!(
                conflicts.is_empty(),
                "preset {} has conflicting shortcuts: {:?}",
                preset.id(),
                conflicts
            );
        }
    }

    #[test]
    fn every_preset_binds_the_shortcuts_a_daw_cannot_be_used_without() {
        use Action::*;
        for preset in Preset::ALL {
            let map = preset.keymap();
            for action in [
                PlayPause,
                Record,
                Save,
                Undo,
                Redo,
                Copy,
                Paste,
                Delete,
                Duplicate,
                Quantize,
                ToolDraw,
                ToolSelect,
                ToggleMetronome,
            ] {
                assert!(
                    !map.bindings_for(action).is_empty(),
                    "preset {} leaves {:?} unbound",
                    preset.id(),
                    action
                );
            }
            // The metronome keeps a key while the typing piano owns the bare letters.
            assert!(
                map.bindings_for(ToggleMetronome)
                    .iter()
                    .any(|b| !b.is_bare()),
                "preset {}: the metronome has only bare keys",
                preset.id()
            );
        }
    }

    #[test]
    fn rebinding_takes_the_shortcut_away_from_its_previous_owner() {
        let mut map = cantus_keymap();
        let q = Binding::key(Key::Q);
        assert_eq!(map.bindings_for(Action::Quantize), [q]);

        map.bind(Action::Panic, q);
        assert_eq!(map.bindings_for(Action::Panic), [q]);
        assert!(
            map.bindings_for(Action::Quantize).is_empty(),
            "the old owner must lose it, or the map would conflict with itself"
        );
        assert!(map.conflicts().is_empty());
        assert_eq!(map.preset, CUSTOM);

        map.unbind(Action::Panic, q);
        assert!(map.bindings_for(Action::Panic).is_empty());
    }

    #[test]
    fn conflicts_are_reported_for_a_hand_broken_file() {
        let map = Keymap::from_toml(
            r#"
            preset = "custom"
            [bindings]
            quantize = ["Q"]
            panic = ["Q"]
            play-pause = ["Space"]
            "#,
        )
        .unwrap();
        let conflicts = map.conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].binding, Binding::key(Key::Q));
        assert_eq!(conflicts[0].actions, vec![Action::Panic, Action::Quantize]);
    }

    #[test]
    fn a_keymap_survives_a_round_trip_through_toml() {
        for preset in Preset::ALL {
            let map = preset.keymap();
            let back = Keymap::from_toml(&map.to_toml()).unwrap();
            assert_eq!(back, map, "preset {} did not survive TOML", preset.id());
        }
    }

    #[test]
    fn a_version_3_file_gets_the_link_keys() {
        // What every machine that started round 23 has on disk: the whole map at version
        // 3, and no line for the linked-clip actions (round 24).
        let linked = ["make-unique", "link-clips", "select-linked"];
        let text: String = Keymap::default()
            .to_toml()
            .lines()
            .filter(|l| !linked.iter().any(|id| l.trim_start().starts_with(id)))
            .map(|l| {
                if l.starts_with("version") {
                    "version = 3".to_string()
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        assert!(text.contains("version = 3") && !text.contains("make-unique"));
        let map = Keymap::from_toml(&text).unwrap();
        for action in [Action::MakeUnique, Action::LinkClips, Action::SelectLinked] {
            assert_eq!(
                map.bindings_for(action),
                Keymap::default().bindings_for(action),
                "{action:?}"
            );
        }
        assert!(map.conflicts().is_empty());
    }

    /// Round 28 (review 1): a map saved at version 7 has no line for tap tempo or
    /// PAT / SONG; it gets `Shift+T` and `L` where they are free, and a user's own `L`
    /// stays theirs.
    #[test]
    fn a_version_7_file_gets_tap_and_pattern_keys() {
        let v7 = |drop: &[&str], extra: &str| -> String {
            let mut lines: Vec<String> = Keymap::default()
                .to_toml()
                .lines()
                .filter(|l| !drop.iter().any(|d| l.trim_start().starts_with(d)))
                .map(|l| {
                    if l.starts_with("version") {
                        "version = 7".to_string()
                    } else {
                        l.to_string()
                    }
                })
                .collect();
            lines.push(extra.to_string());
            lines.join("\n")
        };
        let text = v7(&["tap-tempo", "toggle-pattern-mode"], "");
        assert!(!text.contains("tap-tempo") && !text.contains("toggle-pattern-mode"));
        let map = Keymap::from_toml(&text).unwrap();
        assert_eq!(
            map.bindings_for(Action::TapTempo),
            &[Binding::shift(Key::T)]
        );
        assert_eq!(
            map.bindings_for(Action::TogglePatternMode),
            &[Binding::key(Key::L)]
        );
        assert!(map.conflicts().is_empty());
        for preset in Preset::ALL {
            assert!(preset.keymap().conflicts().is_empty(), "{preset:?}");
        }
        // A user who gave bare L to something else keeps it; PAT / SONG stays unbound.
        let text = v7(
            &["tap-tempo", "toggle-pattern-mode", "panic"],
            "panic = [\"L\"]",
        );
        let map = Keymap::from_toml(&text).unwrap();
        assert_eq!(map.bindings_for(Action::Panic), &[Binding::key(Key::L)]);
        assert!(map.bindings_for(Action::TogglePatternMode).is_empty());
        assert!(map.conflicts().is_empty());
    }

    #[test]
    fn a_version_5_file_gets_the_route_menu_key() {
        // A map saved by round 25's builders: version 5, no line for the route menu.
        let text: String = Keymap::default()
            .to_toml()
            .lines()
            .filter(|l| !l.trim_start().starts_with("route-menu"))
            .map(|l| {
                if l.starts_with("version") {
                    "version = 5".to_string()
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        assert!(!text.contains("route-menu"));
        let map = Keymap::from_toml(&text).unwrap();
        assert_eq!(
            map.bindings_for(Action::RouteMenu),
            &[Binding::shift(Key::F10)]
        );
        assert!(map.conflicts().is_empty());
        for preset in Preset::ALL {
            let m = preset.keymap();
            assert_eq!(
                m.bindings_for(Action::RouteMenu),
                &[Binding::shift(Key::F10)]
            );
            assert!(m.conflicts().is_empty(), "{preset:?}");
        }
    }

    #[test]
    fn a_damaged_file_keeps_what_it_can_and_never_refuses_to_start() {
        // An unknown action and an unreadable shortcut are dropped; the rest survives.
        let map = Keymap::from_toml(
            r#"
            preset = "custom"
            [bindings]
            play-pause = ["Space"]
            fly-to-the-moon = ["Ctrl+M"]
            save = ["Ctrl+S", "not a key"]
            "#,
        )
        .unwrap();
        assert_eq!(
            map.bindings_for(Action::PlayPause),
            [Binding::key(Key::Space)]
        );
        assert_eq!(map.bindings_for(Action::Save), [Binding::cmd(Key::S)]);

        // Something that is not TOML at all is an error the caller falls back from.
        assert!(Keymap::from_toml("{{{ not toml").is_err());

        let dir = tempfile::tempdir().unwrap();
        let path = keymap_path(dir.path());
        assert_eq!(
            Keymap::load(&path),
            Keymap::for_this_system(),
            "a missing file is fine"
        );
        std::fs::write(&path, "{{{ not toml").unwrap();
        assert_eq!(
            Keymap::load(&path),
            Keymap::for_this_system(),
            "a broken file is fine"
        );
    }

    #[test]
    fn a_file_from_before_the_workspace_gets_its_keys_where_they_are_free() {
        // Round 17's file: no version, no F5, and M given to something else by the user.
        let map = Keymap::from_toml(
            r#"
            preset = "cantus"
            [bindings]
            play-pause = ["Space"]
            panic = ["M"]
            "#,
        )
        .unwrap();
        assert_eq!(
            map.bindings_for(Action::ViewArrangement),
            [Binding::key(Key::F5)]
        );
        assert_eq!(
            map.bindings_for(Action::ToggleInspector),
            [Binding::key(Key::I), Binding::alt(Key::I)]
        );
        assert!(
            map.bindings_for(Action::ToggleStripMute).is_empty(),
            "the user's M is theirs"
        );
        assert!(map.conflicts().is_empty());
        // A file of this version is taken as it is: a missing line means unbound.
        let now = Keymap::from_toml(&map.to_toml()).unwrap();
        assert_eq!(now, map);
        let mut edited = map.clone();
        edited.bindings.remove(&Action::ViewArrangement);
        let back = Keymap::from_toml(&edited.to_toml()).unwrap();
        assert!(back.bindings_for(Action::ViewArrangement).is_empty());
    }

    #[test]
    fn the_workspace_keys_never_take_a_note_from_the_typing_piano() {
        // The piano plays the bare letters (and ; and '): a workspace action on a bare
        // letter must be suppressed while it plays, and must also have a form with a
        // modifier, which the piano never reads.
        use Action::*;
        for preset in Preset::ALL {
            let map = preset.keymap();
            for action in [
                ViewArrangement,
                ViewChannelRack,
                ViewPianoRoll,
                ViewMixer,
                ToggleInspector,
                CloseFloating,
                ZoomIn,
                ZoomOut,
                ZoomReset,
                PickElement,
                ToggleDeviceKeyboard,
            ] {
                let list = map.bindings_for(action);
                assert!(
                    !list.is_empty(),
                    "{} leaves {action:?} unbound",
                    preset.id()
                );
                assert!(
                    list.iter().any(|b| !(b.is_bare() && is_letter(b.key))),
                    "{} gives {action:?} only bare letters, which the piano takes",
                    preset.id()
                );
                for b in list {
                    assert!(
                        !matches!(b.key, Key::Semicolon | Key::Quote) || !b.is_bare(),
                        "{action:?} on a piano key"
                    );
                }
            }
        }
    }

    #[test]
    fn the_written_file_says_what_it_is_and_groups_by_category() {
        let toml = cantus_keymap().to_toml();
        assert!(toml.starts_with("# Cantus keyboard map."));
        assert!(toml.contains("[bindings]"));
        assert!(toml.contains("# Transport"));
        assert!(toml.contains("play-pause = [\"Space\"]"));
        assert!(toml.contains("save-as = [\"Ctrl+Shift+S\"]"));
        // Multiple shortcuts on one action stay on one line.
        assert!(toml.contains("redo = [\"Ctrl+Y\", \"Ctrl+Shift+Z\"]"));
    }

    #[test]
    fn the_typing_piano_takes_the_bare_letters_but_not_the_shortcuts() {
        let map = cantus_keymap();
        // A tool bound to a bare letter is suppressed while the piano owns the letters,
        // which is why the same tool is also on a number.
        let draw = map.bindings_for(Action::ToolDraw);
        assert!(draw.contains(&Binding::key(Key::P)));
        assert!(draw.contains(&Binding::key(Key::Num1)));
        assert!(Binding::key(Key::P).is_bare() && is_letter(Key::P));
        assert!(!is_letter(Key::Num1), "numbers stay available");
        // Modified letters are never the piano's.
        assert!(!Binding::cmd(Key::S).is_bare());
    }

    /// The macOS preset (round 30, ADR 0066): Cantus's map, but nothing on a chord macOS
    /// keeps for itself, the metronome on Logic's K, Redo only on ⌘⇧Z, and ⌘, for the
    /// settings; it is what a first start writes on a Mac.
    #[test]
    fn the_mac_preset_leaves_the_system_its_chords() {
        let mac = Preset::Mac.keymap();
        assert_eq!(mac.preset, "macos");
        assert_eq!(Preset::from_id("macos"), Some(Preset::Mac));
        for reserved in mac_reserved() {
            let taken: Vec<Action> = Action::ALL
                .into_iter()
                .filter(|a| mac.bindings_for(*a).contains(&reserved))
                .collect();
            assert!(
                taken.is_empty(),
                "{reserved:?} is macOS's, yet bound to {taken:?}"
            );
        }
        assert_eq!(
            mac.bindings_for(Action::ToggleMetronome),
            [Binding::cmd(Key::K), Binding::key(Key::K)]
        );
        assert_eq!(mac.bindings_for(Action::Redo), [Binding::cmd_shift(Key::Z)]);
        assert_eq!(
            mac.bindings_for(Action::OpenSettings),
            [Binding::cmd(Key::Comma)]
        );
        assert!(mac.conflicts().is_empty(), "{:?}", mac.conflicts());
        // Everything else is Cantus's own.
        let cantus = Preset::Cantus.keymap();
        for a in Action::ALL {
            if !matches!(a, Action::ToggleMetronome | Action::Redo) {
                assert_eq!(mac.bindings_for(a), cantus.bindings_for(a), "{a:?}");
            }
        }
        let expected = if cfg!(target_os = "macos") {
            mac.clone()
        } else {
            cantus
        };
        assert_eq!(Keymap::for_this_system(), expected);
        // A round-trip through the file keeps it.
        assert_eq!(Keymap::from_toml(&mac.to_toml()).unwrap(), mac);
    }

    /// A version-9 file (round 29) gets the settings' key where it is free (version 10).
    #[test]
    fn a_version_9_file_gets_the_settings_key() {
        let old = "preset = \"cantus\"
version = 9

[bindings]
save = [\"Ctrl+S\"]
";
        let map = Keymap::from_toml(old).unwrap();
        assert_eq!(
            map.bindings_for(Action::OpenSettings),
            [Binding::cmd(Key::Comma)]
        );
        let taken = "preset = \"cantus\"
version = 9

[bindings]
save = [\"Ctrl+Comma\"]
";
        let map = Keymap::from_toml(taken).unwrap();
        assert!(
            map.bindings_for(Action::OpenSettings).is_empty(),
            "the key was taken"
        );
    }

    /// A version-10 file (round 30) gets the command palette's and the history's keys where
    /// they are free (version 11, round 31); the palette's chord is free in every preset.
    #[test]
    fn a_version_10_file_gets_the_palette_and_history_keys() {
        let old = "preset = \"fl-studio\"
version = 10

[bindings]
save = [\"Ctrl+S\"]
";
        let map = Keymap::from_toml(old).unwrap();
        assert_eq!(
            map.bindings_for(Action::CommandPalette),
            [Binding::cmd_shift(Key::P)]
        );
        assert_eq!(
            map.bindings_for(Action::ViewHistory),
            [Binding::alt(Key::H)]
        );
        let taken = "preset = \"cantus\"
version = 10

[bindings]
save = [\"Ctrl+Shift+P\"]
";
        let map = Keymap::from_toml(taken).unwrap();
        assert!(map.bindings_for(Action::CommandPalette).is_empty());
        for p in Preset::ALL {
            assert_eq!(
                p.keymap().bindings_for(Action::CommandPalette),
                [Binding::cmd_shift(Key::P)],
                "{p:?}"
            );
        }
        assert!(
            !mac_reserved().contains(&Binding::cmd_shift(Key::P)),
            "⌘⇧P reaches Cantus on a Mac"
        );
    }
}
