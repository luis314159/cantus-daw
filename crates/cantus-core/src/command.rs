//! The command layer (ADR 0004).
//!
//! A [`Command`] is the *only* way to mutate a [`Project`]. Applying one returns its
//! inverse, which is what makes undo/redo, auditing and AI reversibility fall out for free.

use crate::effect::EffectSlot;
use crate::instrument::{InstrumentParam, InstrumentSpec};
use crate::model::*;
use crate::pads::{DrumPad, PadId, PadLayer};
use crate::time::{clamp_bpm, Ticks, TimeSignature};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Who issued a command. Semantics are identical; this only feeds auditing/permissions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Origin {
    User,
    Script,
    Ai,
    Mcp,
    System,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CommandError {
    #[error("track {0} not found")]
    TrackNotFound(TrackId),
    #[error("clip {0} not found")]
    ClipNotFound(ClipId),
    #[error("note {0} not found")]
    NoteNotFound(NoteId),
    #[error("index {index} out of range (len {len})")]
    IndexOutOfRange { index: usize, len: usize },
    #[error("duplicate id {0}")]
    DuplicateId(u64),
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("instrument on track {0} does not have that parameter")]
    ParamNotApplicable(TrackId),
    #[error("effect {0} not found")]
    EffectNotFound(EffectId),
    #[error("mixer strip {0} not found")]
    StripNotFound(StripId),
    #[error("routing {0} there would feed it back into itself")]
    RoutingCycle(StripId),
    #[error("mixer strip {0} is still in use")]
    StripInUse(StripId),
    #[error("mixer strip {0} has no room for another insert")]
    TooManyInserts(StripId),
    #[error("mixer strip {0} has no room for another send")]
    TooManySends(StripId),
    #[error("mixer strip {0} has no send there")]
    SendNotFound(StripId),
    #[error("mixer group {0} not found")]
    GroupNotFound(GroupId),
    #[error("mixer group {0} still has strips in it")]
    GroupInUse(GroupId),
    #[error("audio source {0} not found")]
    AudioSourceNotFound(AudioSourceId),
    #[error("audio source {0} is still played by a clip")]
    AudioSourceInUse(AudioSourceId),
    /// The edit would change the sound of a frozen strip or track (ADR 0064): unfreeze it
    /// first. Names the frozen target.
    #[error("{0:?} is frozen: unfreeze it to change this")]
    Frozen(crate::freeze::FreezeTarget),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    // --- project-wide -----------------------------------------------------------------
    SetProjectName {
        name: String,
    },
    SetTempo {
        bpm: f64,
    },
    SetTimeSignature {
        time_signature: TimeSignature,
    },
    SetLoopRegion {
        region: Option<LoopRegion>,
    },
    SetLoopEnabled {
        enabled: bool,
    },
    SetMetronome {
        enabled: bool,
    },
    /// FL's PAT / SONG switch (ADR 0052): `on` plays the current pattern alone, looped.
    SetPatternMode {
        on: bool,
    },
    // --- the tempo track, metre changes and markers (round 29, ADR 0060) -------------
    /// The tempo at tick 0 and the whole tempo track at once: `points` empty removes the
    /// tempo track (the song plays at `bpm`); otherwise its first point is at tick 0, at
    /// `bpm`.
    SetTempoTrack {
        bpm: f64,
        points: Vec<crate::tempo::TempoPoint>,
    },
    /// A tempo point between two others (never before the first).
    InsertTempoPoint {
        index: usize,
        point: crate::tempo::TempoPoint,
    },
    RemoveTempoPoint {
        index: usize,
    },
    /// A point moved, its tempo or its ramp changed; it stays between its neighbours (the
    /// first stays at tick 0, and sets the song's tempo).
    SetTempoPoint {
        index: usize,
        point: crate::tempo::TempoPoint,
    },
    /// The metre at tick 0 and every metre change at once (empty: one metre).
    SetMetreTrack {
        signature: TimeSignature,
        metres: Vec<crate::tempo::MetreChange>,
    },
    /// A metre change at a bar line (added, or the one there changed); the changes after
    /// it keep their bars.
    SetMetreAt {
        tick: Ticks,
        signature: TimeSignature,
    },
    /// The metre change at `tick` removed; the changes after it keep their bars.
    RemoveMetreAt {
        tick: Ticks,
    },
    /// The metre change at `from` moved to the bar line `to` of the metre before it; the
    /// changes after it keep their bars.
    MoveMetre {
        from: Ticks,
        to: Ticks,
    },
    AddMarker {
        marker: crate::tempo::Marker,
    },
    RemoveMarker {
        id: crate::tempo::MarkerId,
    },
    /// A marker moved, renamed or recoloured.
    SetMarker {
        marker: crate::tempo::Marker,
    },
    // --- MIDI controller mappings (round 29, ADR 0062) ---------------------------------
    /// A controller mapping added to the project's map at `index` (clamped to its end).
    InsertMapping {
        index: usize,
        mapping: Box<crate::controller::Mapping>,
    },
    /// A controller mapping taken out of the project's map.
    RemoveMapping {
        id: crate::controller::MappingId,
    },
    /// A controller mapping, found by its id, replaced whole: learnt again, its target,
    /// mode, range, bend, takeover, feedback or sensitivity changed.
    SetMapping {
        mapping: Box<crate::controller::Mapping>,
    },
    /// A modulator's amount (0…1) alone: what a controller or a script sets on
    /// `AutoTarget::ModAmount`, read back by `AutoTarget::value_of_command` (round 29,
    /// review 1 of ADR 0062). Its inverse is the amount it had.
    SetModulatorAmount {
        id: crate::model::ModulatorId,
        amount: f32,
    },
    SetMasterGain {
        gain_db: f32,
    },
    SetMasterPan {
        pan: f32,
    },
    SetMasterWidth {
        width: f32,
    },
    SetMasterPhaseInvert {
        invert: bool,
    },
    /// The master's whole insert chain in or out (ADR 0026), FL's master FX lamp.
    SetMasterFxEnabled {
        enabled: bool,
    },
    /// Project-wide, because it changes what the project renders as (ADR 0012).
    SetPanLaw {
        law: PanLaw,
    },
    /// Replace the whole project (used by "open"). Inverse restores the previous one.
    ReplaceProject {
        project: Box<Project>,
    },

    // --- tracks -----------------------------------------------------------------------
    InsertTrack {
        index: usize,
        track: Box<Track>,
    },
    RemoveTrack {
        id: TrackId,
    },
    MoveTrack {
        id: TrackId,
        index: usize,
    },
    SetTrackName {
        id: TrackId,
        name: String,
    },
    SetTrackColor {
        id: TrackId,
        color: Color,
    },
    SetTrackGain {
        id: TrackId,
        gain_db: f32,
    },
    SetTrackPan {
        id: TrackId,
        pan: f32,
    },
    /// Which mixer strip the track feeds; `None` is the master.
    SetTrackStrip {
        id: TrackId,
        strip: Option<StripId>,
    },

    // --- mixer strips -----------------------------------------------------------------
    /// Add a strip at `index` in the mixer's order. The `+` at the end of the mixer, and
    /// the inverse of removing one.
    InsertStrip {
        index: usize,
        strip: Box<MixerStrip>,
    },
    /// Remove a strip nothing feeds and nothing is routed to. Refused otherwise: a strip
    /// that vanished under a track would silently reroute it to the master.
    RemoveStrip {
        id: StripId,
    },
    SetStripName {
        id: StripId,
        name: String,
    },
    SetStripGain {
        id: StripId,
        gain_db: f32,
    },
    SetStripPan {
        id: StripId,
        pan: f32,
    },
    /// 0 is mono, 1 as recorded, 2 twice as wide.
    SetStripWidth {
        id: StripId,
        width: f32,
    },
    SetStripPhaseInvert {
        id: StripId,
        invert: bool,
    },
    SetStripMute {
        id: StripId,
        muted: bool,
    },
    /// Where the strip's output goes. Refused if it would close a loop.
    SetStripOutput {
        id: StripId,
        output: Option<StripId>,
    },
    SetStripColor {
        id: StripId,
        color: Option<Color>,
    },
    /// Solo with connected routes (see [`MixerStrip::soloed`]).
    SetStripSolo {
        id: StripId,
        soloed: bool,
    },
    SetStripSwapChannels {
        id: StripId,
        swap: bool,
    },
    /// The whole insert chain in or out.
    SetStripFxEnabled {
        id: StripId,
        enabled: bool,
    },
    /// One number of one band of the strip's EQ.
    SetStripEq {
        id: StripId,
        band: u8,
        field: EqField,
        value: f32,
    },
    /// Add a send or change one: matched by destination. Refused if it would close a
    /// loop, if the strip is the destination, or if the strip has no room for another.
    SetStripSend {
        id: StripId,
        send: Send,
    },
    RemoveStripSend {
        id: StripId,
        to: StripId,
    },
    /// A new send at `index` in the strip's list (clamped to its end): the exact inverse
    /// of [`Command::RemoveStripSend`], which puts a removed send back where it was.
    InsertStripSend {
        id: StripId,
        index: usize,
        send: Send,
    },
    /// Move a strip to another place on the desk. Numbers follow the order; every route
    /// is by id, so nothing else changes.
    MoveStrip {
        id: StripId,
        index: usize,
    },
    /// Pin a strip left or right, or let it scroll with the middle of the desk.
    SetStripDock {
        id: StripId,
        dock: Dock,
    },
    /// Put a strip behind a group's separator, or take it out (`None`).
    SetStripGroup {
        id: StripId,
        group: Option<GroupId>,
    },
    InsertStripGroup {
        index: usize,
        group: Box<StripGroup>,
    },
    /// Remove a group nothing is in. Refused otherwise; the app takes the members out
    /// first, in the same batch.
    RemoveStripGroup {
        id: GroupId,
    },
    SetStripGroupName {
        id: GroupId,
        name: String,
    },
    SetStripGroupColor {
        id: GroupId,
        color: Color,
    },
    SetStripGroupCollapsed {
        id: GroupId,
        collapsed: bool,
    },
    /// How tall the track is drawn, as a multiple of the base row. A view thing, and a
    /// command all the same: it is stored in the project, so it is undoable like the rest.
    SetTrackHeight {
        id: TrackId,
        height: f32,
    },
    /// `None` puts the track back on the icon its instrument implies.
    SetTrackIcon {
        id: TrackId,
        icon: Option<TrackIcon>,
    },
    SetTrackMute {
        id: TrackId,
        muted: bool,
    },
    SetTrackSolo {
        id: TrackId,
        soloed: bool,
    },
    /// Replace the track's instrument wholesale — a synth becomes a kit, or the other way.
    /// The clips stay: a note is a pitch either way, and the editors' lanes follow the new
    /// instrument (`InstrumentSpec::lanes`).
    SetInstrument {
        track: TrackId,
        spec: Box<InstrumentSpec>,
    },
    /// Add a pad to a kit. Pads are kept sorted by pitch and a pitch can hold one pad, so
    /// this fails rather than shadowing an existing one; an id already in the kit fails
    /// too, and a pad with no id (0) is given the kit's next one.
    InsertPad {
        track: TrackId,
        pad: Box<DrumPad>,
    },
    /// Remove a pad, by identity.
    RemovePad {
        track: TrackId,
        pad: PadId,
    },
    /// Set one parameter of whatever instrument the track hosts.
    ///
    /// The address is an [`InstrumentParam`], not a `FirmusParam`: a second instrument
    /// must not need a second command, or every later one needs another.
    SetInstrumentParam {
        track: TrackId,
        param: InstrumentParam,
        value: f32,
    },
    /// Rename a pad of a kit. The name lives **in the kit**, so this renames the lane in
    /// every clip on the track at once — which is the point of a pad having a name at all.
    SetPadName {
        track: TrackId,
        pad: PadId,
        name: String,
    },
    /// Point a pad at a sample file (or at none). The pad's voice is set separately, so a
    /// pad can hold on to its file while it is switched back to a synthesized voice.
    SetPadSample {
        track: TrackId,
        pad: PadId,
        path: Option<String>,
    },
    /// Replace a pad's sample layers (Pulsus II): adding, removing and editing a layer are
    /// all this one command, whose inverse is the list as it was.
    SetPadLayers {
        track: TrackId,
        pad: PadId,
        layers: Vec<PadLayer>,
    },
    /// Set one number of a native instrument (ADR 0023). Setting a key that was never
    /// stored has "clear it" as its inverse, so undo returns the map to exactly what it was.
    SetInstrumentValue {
        track: TrackId,
        key: String,
        value: f32,
    },
    /// Take a native instrument's number back to "never set": it reads as its default and
    /// is not written to the file.
    ClearInstrumentValue {
        track: TrackId,
        key: String,
    },
    /// Set or remove one text value of a native instrument: a sample's path, a name.
    SetInstrumentText {
        track: TrackId,
        key: String,
        text: Option<String>,
    },

    // --- effects: inserts live on mixer strips ----------------------------------------
    InsertEffect {
        strip: StripId,
        index: usize,
        slot: Box<EffectSlot>,
    },
    RemoveEffect {
        strip: StripId,
        effect: EffectId,
    },
    MoveEffect {
        strip: StripId,
        effect: EffectId,
        index: usize,
    },
    /// Swap an insert for another **in its place** (round 20, ADR 0029): the slot `effect`
    /// leaves the chain and `slot` takes its index. One step of the history, whose inverse
    /// puts the old slot back. What the new slot carries over from the old one (mix,
    /// bypass, a key, a note source) is the caller's choice; a key or a note source the new
    /// kind cannot take is refused as `SetEffectSidechain` and `SetEffectMidiSource`
    /// refuse it, and so is an id already in the chain.
    ReplaceEffect {
        strip: StripId,
        effect: EffectId,
        slot: Box<EffectSlot>,
    },
    SetEffectBypass {
        strip: StripId,
        effect: EffectId,
        bypass: bool,
    },
    SetEffectParam {
        strip: StripId,
        effect: EffectId,
        key: String,
        value: f32,
    },
    /// Take a parameter back to "never set": it reads as its default and is not written
    /// to the file. The inverse of setting a parameter a sparse effect did not store, so
    /// undo leaves the project exactly as it was, map and all.
    ClearEffectParam {
        strip: StripId,
        effect: EffectId,
        key: String,
    },
    /// Key an effect's detector from another strip, or back to its own input (`None`).
    /// Refused for a kind that takes no key, for the effect's own strip, and for a key that
    /// would close a loop.
    SetEffectSidechain {
        strip: StripId,
        effect: EffectId,
        source: Option<StripId>,
    },
    /// The slot's own dry/wet ([`EffectSlot::mix`]).
    SetEffectMix {
        strip: StripId,
        effect: EffectId,
        mix: f32,
    },
    /// One parameter of a hosted plug-in, by the plug-in's id, in its own range; `None`
    /// takes the value out of the project (the plug-in's state decides it). Only for an
    /// [`crate::EffectKind::External`] slot.
    SetPluginParam {
        strip: StripId,
        effect: EffectId,
        id: u32,
        value: Option<f64>,
    },
    /// A hosted plug-in's whole state and parameter values at once: a preset loaded, the
    /// other side of A/B, or a state the plug-in changed by itself.
    SetPluginState {
        strip: StripId,
        effect: EffectId,
        state: crate::external::PluginState,
        params: std::collections::BTreeMap<u32, f64>,
    },
    /// One parameter of a track's hosted instrument (ADR 0028), by the plug-in's id, in its
    /// own range; `None` takes the value out of the project. The instrument twin of
    /// [`Command::SetPluginParam`]; only for an [`InstrumentSpec::External`] track.
    SetInstrumentPluginParam {
        track: TrackId,
        id: u32,
        value: Option<f64>,
    },
    /// A track's hosted instrument's whole state and values at once: a preset, the other
    /// side of A/B, or a state the plug-in changed by itself (its own preset browser).
    SetInstrumentPluginState {
        track: TrackId,
        state: crate::external::PluginState,
        params: std::collections::BTreeMap<u32, f64>,
    },
    /// Where a hosted insert runs (round 29, ADR 0063): `None` follows the machine's
    /// default. Refused for a slot that holds no hosted plug-in.
    SetPluginIsolation {
        strip: StripId,
        effect: EffectId,
        isolation: Option<crate::external::PluginIsolation>,
    },
    /// Where a track's hosted instrument runs (round 29, ADR 0063); the instrument twin of
    /// [`Command::SetPluginIsolation`].
    SetInstrumentIsolation {
        track: TrackId,
        isolation: Option<crate::external::PluginIsolation>,
    },
    /// Freeze a mixer strip (round 29, ADR 0064) — `Some` with its content key now, which
    /// is checked — or unfreeze it (`None`).
    SetStripFrozen {
        strip: StripId,
        frozen: Option<crate::freeze::Frozen>,
    },
    /// Freeze a track's own source (round 29, ADR 0064), or unfreeze it; the twin of
    /// [`Command::SetStripFrozen`].
    SetTrackFrozen {
        track: TrackId,
        frozen: Option<crate::freeze::Frozen>,
    },
    /// Set or clear the delay a track keeps before its level ([`Track::lateness`], a
    /// bounce's — round 29's third review): `None` plays its source on time. Refused for
    /// one that is not valid.
    SetTrackLateness {
        id: TrackId,
        lateness: Option<crate::model::Lateness>,
    },
    /// Send a track's notes into an effect ([`EffectSlot::midi_from`]), or none (`None`).
    /// Refused for a kind that takes no notes and for a track that does not exist.
    SetEffectMidiSource {
        strip: StripId,
        effect: EffectId,
        source: Option<TrackId>,
    },

    // --- clips ------------------------------------------------------------------------
    InsertClip {
        track: TrackId,
        clip: Box<Clip>,
    },
    RemoveClip {
        track: TrackId,
        clip: ClipId,
    },
    SetClipBounds {
        track: TrackId,
        clip: ClipId,
        start: Ticks,
        length: Ticks,
    },
    SetClipName {
        track: TrackId,
        clip: ClipId,
        name: String,
    },
    /// Slip: move the content under the window without moving the window.
    SetClipOffset {
        track: TrackId,
        clip: ClipId,
        offset: Ticks,
    },
    /// Re-anchor audio windows of one track at once (ADR 0060 §4, review 5): each its
    /// offset and fine offset (`slip_frames`) — what a tempo edit's window fix writes, in
    /// one pass over the track (a command a window looked each clip up again: quadratic in
    /// a sliced take). Refused for a clip that is missing or not audio, and for a fine
    /// offset changed on a linked clip (it is its group's). The inverse puts them back.
    SetClipAnchors {
        track: TrackId,
        anchors: Vec<(ClipId, Ticks, i32)>,
    },
    /// Turn the clip's repeat on (`Some(period)`) or off (`None`).
    SetClipLoop {
        track: TrackId,
        clip: ClipId,
        content_length: Option<Ticks>,
    },
    SetClipMute {
        track: TrackId,
        clip: ClipId,
        muted: bool,
    },
    /// `None` puts the clip back on the track's colour.
    SetClipColor {
        track: TrackId,
        clip: ClipId,
        color: Option<Color>,
    },
    /// Move a clip to another track, keeping its id — so a drag across tracks is one
    /// reversible step and the selection survives it.
    MoveClipToTrack {
        from: TrackId,
        to: TrackId,
        clip: ClipId,
    },

    // --- audio (ADR 0025) -------------------------------------------------------------
    /// Add a file to the pool. The id comes from `Project::alloc_id`, like a clip's.
    InsertAudioSource {
        source: crate::audio::AudioSource,
    },
    /// Take a file out of the pool; refused while a clip (on any lane) still plays it.
    RemoveAudioSource {
        id: AudioSourceId,
    },
    /// Point a source at another path (the file was moved into the project's folder, or
    /// found again).
    SetAudioSourcePath {
        id: AudioSourceId,
        path: String,
    },
    /// Set or clear what makes a clip an audio clip: clip gain, fades and their curves,
    /// reverse. One command for all of them, so a drag on a fade handle merges.
    SetClipAudio {
        track: TrackId,
        clip: ClipId,
        audio: Option<crate::audio::AudioRegion>,
    },
    /// Set or clear an audio clip's pitch edits (ADR 0054): the notes and every edit on
    /// them, whole — a drag merges into one step (when `op` merges) and the inverse is the
    /// edit as it was. `op` names the edit for the undo menu. Refused on a clip that is
    /// not audio, and for an edit that is not valid (`PitchEdit::is_valid`).
    SetClipPitch {
        track: TrackId,
        clip: ClipId,
        pitch: Option<Box<crate::pitch::PitchEdit>>,
        op: crate::pitch::PitchOp,
    },
    /// Set or clear an audio clip's warp (ADR 0061): its markers, mode, transposition and
    /// tempo, whole — a drag merges into one step (when `op` merges) and the inverse is the
    /// warp as it was. Refused on a clip that is not audio, and for a warp that is not
    /// valid (`Warp::is_valid`). Linked clips share it (it is content, ADR 0037).
    SetClipWarp {
        track: TrackId,
        clip: ClipId,
        warp: Option<Box<crate::warp::Warp>>,
        op: crate::warp::WarpOp,
    },
    /// Set or clear what makes a clip an automation clip (ADR 0027): its target and its
    /// curve. Every edit of a curve — a point added, moved, bent, an articulator — is this
    /// one command with the whole new curve, so a drag merges into one step and its
    /// inverse is simply the curve as it was.
    SetClipAutomation {
        track: TrackId,
        clip: ClipId,
        automation: Option<crate::automation::Automation>,
    },
    /// Show or hide the automation lanes under a track.
    SetTrackAutomationOpen {
        id: TrackId,
        open: bool,
    },
    /// A note clip's controller lanes, whole (ADR 0068): every edit of a lane — a pencil
    /// stroke, a line, a point moved, a lane added or removed — is this one command with
    /// every lane as it is now, so a drag merges into one step. Content a link group
    /// shares. Refused for an invalid lane, two lanes of one kind, or an audio or
    /// automation clip.
    SetClipLanes {
        track: TrackId,
        clip: ClipId,
        lanes: Vec<crate::clip_lanes::ClipLane>,
    },
    /// A track's automation mode (ADR 0068): Read, Write, Touch or Latch.
    SetTrackAutoMode {
        track: TrackId,
        mode: crate::automation::AutoMode,
    },
    /// The master's automation mode (ADR 0068).
    SetMasterAutoMode {
        mode: crate::automation::AutoMode,
    },
    /// Put a modulator on a track, a strip or the master, at `index` in its panel
    /// (ADR 0057). Refused for a modulator that is not valid, an id already used, a host
    /// that is gone or full.
    InsertModulator {
        host: crate::modulator::ModHost,
        index: usize,
        modulator: Box<crate::modulator::Modulator>,
    },
    /// Take a modulator away (its routes with it).
    RemoveModulator {
        id: crate::model::ModulatorId,
    },
    /// Replace a modulator, found by its id, whole: a source edited, a route added or
    /// removed, a depth dragged — one command, so a drag merges into one step and its
    /// inverse is simply the modulator as it was.
    SetModulator {
        modulator: Box<crate::modulator::Modulator>,
    },
    /// Move a comp's seam — where playing-lane window `left` ends and `right` begins — to
    /// `at`, both sides at once (ADR 0058). Refused for two windows that do not meet, for
    /// another lane, a reversed window, a seam that would leave its windows.
    SetCompSeam {
        track: TrackId,
        left: ClipId,
        right: ClipId,
        at: Ticks,
    },
    /// A track's alternate comps, whole, and which plays (ADR 0058).
    SetTrackComps {
        track: TrackId,
        comps: Vec<crate::comp::Comp>,
        active: u8,
    },
    /// Put a clip into a link group (`Some`) or take it out of one (`None`) (ADR 0037).
    /// Joining a group is refused unless the clip already has the group's content: the
    /// builders send a `SetClipContent` first ("Link to…"). Taking a clip out keeps its
    /// content ("Make unique").
    SetClipLink {
        track: TrackId,
        clip: ClipId,
        link: Option<LinkId>,
    },
    /// Replace everything a clip shares with its link group — name, colour, notes, audio
    /// settings, automation curve — at once, on the clip and on every sibling (ADR 0037).
    SetClipContent {
        track: TrackId,
        clip: ClipId,
        content: Box<crate::links::ClipContent>,
    },
    /// Move a clip to another lane: 0 plays, 1… are take lanes.
    SetClipLane {
        track: TrackId,
        clip: ClipId,
        lane: u8,
    },
    /// Make a clip a comp window or not (`Clip::comp`, ADR 0058): a glue that takes a
    /// window in, a flatten that ends the comp, a clip moved to another track.
    SetClipComp {
        track: TrackId,
        clip: ClipId,
        comp: bool,
    },
    SetTrackInput {
        id: TrackId,
        input: Option<crate::audio::TrackInput>,
    },
    SetTrackArmed {
        id: TrackId,
        armed: bool,
    },
    SetTrackTakesOpen {
        id: TrackId,
        open: bool,
    },
    /// Show or hide a track's pitch lane (ADR 0054).
    SetTrackPitchOpen {
        id: TrackId,
        open: bool,
    },
    SetRecordSettings {
        settings: crate::audio::RecordSettings,
    },

    // --- note effects, the key and grooves (round 31, ADR 0069) ------------------------
    /// A track's whole note-effect chain at once: an effect added, removed, moved, switched
    /// on or off, its kind changed. Refused for a chain that is not valid
    /// (`notefx::chain_is_valid`) or that reuses an id another track's chain holds.
    SetTrackNoteFx {
        track: TrackId,
        chain: Vec<crate::notefx::NoteEffect>,
    },
    /// One parameter of a note effect, in its own units (clamped, a stepped one rounded): a
    /// knob's drag merges into one step. Refused for an effect or a key that is not there.
    SetNoteFxParam {
        track: TrackId,
        effect: crate::notefx::NoteFxId,
        key: String,
        value: f32,
    },
    /// The song's key and scale, or none.
    SetProjectKey {
        key: Option<crate::scale::Key>,
    },
    /// A note clip's groove and its amounts, or none (content: a link group shares it).
    /// Refused on an audio or automation clip, for a source that is not there, and for
    /// amounts out of range.
    SetClipGroove {
        track: TrackId,
        clip: ClipId,
        groove: Option<crate::groove::ClipGroove>,
    },
    /// Whether a note clip plays past its track's note effects (a baked clip; content).
    SetClipSkipNoteFx {
        track: TrackId,
        clip: ClipId,
        skip: bool,
    },
    /// Put a groove in the project's pool at `index`. Refused for one that is not valid
    /// or whose id is taken.
    InsertGroove {
        index: usize,
        groove: Box<crate::groove::Groove>,
    },
    /// Take a groove out of the pool; refused while a clip plays with it.
    RemoveGroove {
        id: crate::groove::GrooveId,
    },
    /// Replace a pooled groove, found by its id (renamed, edited).
    SetGroove {
        groove: Box<crate::groove::Groove>,
    },

    // --- notes ------------------------------------------------------------------------
    InsertNotes {
        track: TrackId,
        clip: ClipId,
        notes: Vec<Note>,
    },
    RemoveNotes {
        track: TrackId,
        clip: ClipId,
        ids: Vec<NoteId>,
    },
    /// Replace notes wholesale, matched by id. Used for move/resize/velocity edits.
    SetNotes {
        track: TrackId,
        clip: ClipId,
        notes: Vec<Note>,
    },

    /// Several commands as one undo step.
    Batch {
        label_key: String,
        commands: Vec<Command>,
    },
}

impl Command {
    /// i18n key describing the command for the undo menu.
    pub fn label_key(&self) -> &str {
        use Command::*;
        match self {
            SetProjectName { .. } => "cmd.set_project_name",
            SetTempo { .. } => "cmd.set_tempo",
            SetTimeSignature { .. } => "cmd.set_time_signature",
            SetTempoTrack { .. } => "cmd.tempo.track",
            InsertTempoPoint { .. } => "cmd.tempo.add_point",
            RemoveTempoPoint { .. } => "cmd.tempo.remove_point",
            SetTempoPoint { .. } => "cmd.tempo.set_point",
            SetMetreTrack { .. } => "cmd.tempo.metres",
            SetMetreAt { .. } => "cmd.tempo.set_metre",
            RemoveMetreAt { .. } => "cmd.tempo.remove_metre",
            MoveMetre { .. } => "cmd.tempo.move_metre",
            InsertMapping { .. } => "cmd.controllers.add",
            RemoveMapping { .. } => "cmd.controllers.remove",
            SetMapping { .. } => "cmd.controllers.set",
            SetModulatorAmount { .. } => "cmd.set_modulator",
            AddMarker { .. } => "cmd.tempo.add_marker",
            RemoveMarker { .. } => "cmd.tempo.remove_marker",
            SetMarker { .. } => "cmd.tempo.set_marker",
            SetLoopRegion { .. } => "cmd.set_loop_region",
            SetLoopEnabled { .. } => "cmd.set_loop_enabled",
            SetMetronome { .. } => "cmd.set_metronome",
            SetPatternMode { .. } => "cmd.set_pattern_mode",
            SetMasterGain { .. } => "cmd.set_master_gain",
            SetMasterPan { .. } => "cmd.set_master_pan",
            SetMasterWidth { .. } => "cmd.set_master_width",
            SetMasterPhaseInvert { .. } => "cmd.set_master_phase",
            SetMasterFxEnabled { .. } => "cmd.set_master_fx_enabled",
            SetPanLaw { .. } => "cmd.set_pan_law",
            ReplaceProject { .. } => "cmd.replace_project",
            InsertTrack { .. } => "cmd.insert_track",
            RemoveTrack { .. } => "cmd.remove_track",
            MoveTrack { .. } => "cmd.move_track",
            SetTrackName { .. } => "cmd.set_track_name",
            SetTrackColor { .. } => "cmd.set_track_color",
            SetTrackGain { .. } => "cmd.set_track_gain",
            SetTrackPan { .. } => "cmd.set_track_pan",
            SetTrackStrip { .. } => "cmd.set_track_strip",
            InsertStrip { .. } => "cmd.insert_strip",
            RemoveStrip { .. } => "cmd.remove_strip",
            SetStripName { .. } => "cmd.set_strip_name",
            SetStripGain { .. } => "cmd.set_strip_gain",
            SetStripPan { .. } => "cmd.set_strip_pan",
            SetStripWidth { .. } => "cmd.set_track_width",
            SetStripPhaseInvert { .. } => "cmd.set_track_phase",
            SetStripMute { .. } => "cmd.set_strip_mute",
            SetStripOutput { .. } => "cmd.set_strip_output",
            SetStripColor { .. } => "cmd.set_strip_color",
            SetStripSolo { .. } => "cmd.set_strip_solo",
            SetStripSwapChannels { .. } => "cmd.set_strip_swap",
            SetStripFxEnabled { .. } => "cmd.set_strip_fx_enabled",
            SetStripEq { .. } => "cmd.set_strip_eq",
            SetStripSend { .. } => "cmd.set_strip_send",
            RemoveStripSend { .. } => "cmd.remove_strip_send",
            InsertStripSend { .. } => "cmd.insert_strip_send",
            MoveStrip { .. } => "cmd.move_strip",
            SetStripDock { .. } => "cmd.set_strip_dock",
            SetStripGroup { .. } => "cmd.set_strip_group",
            InsertStripGroup { .. } => "cmd.insert_strip_group",
            RemoveStripGroup { .. } => "cmd.remove_strip_group",
            SetStripGroupName { .. } => "cmd.set_strip_group_name",
            SetStripGroupColor { .. } => "cmd.set_strip_group_color",
            SetStripGroupCollapsed { .. } => "cmd.set_strip_group_collapsed",
            SetEffectMix { .. } => "cmd.set_effect_mix",
            SetPluginParam { .. } => "cmd.set_plugin_param",
            SetPluginState { .. } => "cmd.set_plugin_state",
            SetInstrumentPluginParam { .. } => "cmd.set_plugin_param",
            SetInstrumentPluginState { .. } => "cmd.set_plugin_state",
            SetPluginIsolation { .. } | SetInstrumentIsolation { .. } => "cmd.set_plugin_isolation",
            SetStripFrozen {
                frozen: Some(_), ..
            }
            | SetTrackFrozen {
                frozen: Some(_), ..
            } => "cmd.freeze",
            SetStripFrozen { frozen: None, .. } | SetTrackFrozen { frozen: None, .. } => {
                "cmd.unfreeze"
            }
            SetTrackLateness { .. } => "cmd.set_track_lateness",
            ClearEffectParam { .. } => "cmd.set_effect_param",
            SetEffectSidechain { .. } => "cmd.set_effect_sidechain",
            SetEffectMidiSource { .. } => "cmd.set_effect_midi_source",
            SetTrackHeight { .. } => "cmd.set_track_height",
            SetTrackIcon { .. } => "cmd.set_track_icon",
            SetTrackMute { .. } => "cmd.set_track_mute",
            SetTrackSolo { .. } => "cmd.set_track_solo",
            SetInstrument { .. } => "cmd.set_instrument",
            InsertPad { .. } => "cmd.insert_pad",
            RemovePad { .. } => "cmd.remove_pad",
            SetInstrumentParam { .. } => "cmd.set_instrument_param",
            SetPadName { .. } => "cmd.set_pad_name",
            SetPadSample { .. } => "cmd.set_pad_sample",
            SetPadLayers { .. } => "cmd.set_pad_layers",
            SetInstrumentValue { .. } | ClearInstrumentValue { .. } => "cmd.set_instrument_value",
            SetInstrumentText { .. } => "cmd.set_instrument_text",
            InsertEffect { .. } => "cmd.insert_effect",
            RemoveEffect { .. } => "cmd.remove_effect",
            MoveEffect { .. } => "cmd.move_effect",
            ReplaceEffect { .. } => "cmd.replace_effect",
            SetEffectBypass { .. } => "cmd.set_effect_bypass",
            SetEffectParam { .. } => "cmd.set_effect_param",
            InsertClip { .. } => "cmd.insert_clip",
            RemoveClip { .. } => "cmd.remove_clip",
            SetClipBounds { .. } => "cmd.set_clip_bounds",
            SetClipName { .. } => "cmd.set_clip_name",
            SetClipOffset { .. } => "cmd.set_clip_offset",
            SetClipAnchors { .. } => "cmd.set_clip_anchors",
            SetClipLoop { .. } => "cmd.set_clip_loop",
            SetClipMute { .. } => "cmd.set_clip_mute",
            SetClipColor { .. } => "cmd.set_clip_color",
            MoveClipToTrack { .. } => "cmd.move_clip_to_track",
            InsertNotes { .. } => "cmd.insert_notes",
            RemoveNotes { .. } => "cmd.remove_notes",
            SetNotes { .. } => "cmd.set_notes",
            SetTrackNoteFx { .. } => "cmd.notefx.chain",
            SetNoteFxParam { .. } => "cmd.notefx.param",
            SetProjectKey { .. } => "cmd.notefx.key",
            SetClipGroove { .. } => "cmd.notefx.groove",
            SetClipSkipNoteFx { .. } => "cmd.notefx.skip",
            InsertGroove { .. } => "cmd.notefx.add_groove",
            RemoveGroove { .. } => "cmd.notefx.remove_groove",
            SetGroove { .. } => "cmd.notefx.set_groove",
            InsertAudioSource { .. } => "cmd.insert_audio_source",
            RemoveAudioSource { .. } => "cmd.remove_audio_source",
            SetAudioSourcePath { .. } => "cmd.set_audio_source_path",
            SetClipAudio { .. } => "cmd.set_clip_audio",
            SetClipLane { .. } => "cmd.set_clip_lane",
            SetClipComp { .. } => "cmd.set_clip_comp",
            SetClipLink { link: None, .. } => "cmd.make_unique",
            SetClipLink { .. } => "cmd.link_clip",
            SetClipContent { .. } => "cmd.set_clip_content",
            SetClipAutomation { .. } => "cmd.set_clip_automation",
            SetClipPitch { op, .. } => op.label_key(),
            SetClipWarp { op, .. } => op.label_key(),
            SetTrackAutomationOpen { .. } => "cmd.set_track_automation_open",
            SetClipLanes { .. } => "cmd.set_clip_lanes",
            SetTrackAutoMode { .. } => "cmd.set_track_auto_mode",
            SetMasterAutoMode { .. } => "cmd.set_master_auto_mode",
            InsertModulator { .. } => "cmd.insert_modulator",
            RemoveModulator { .. } => "cmd.remove_modulator",
            SetModulator { .. } => "cmd.set_modulator",
            SetCompSeam { .. } => "cmd.set_comp_seam",
            SetTrackComps { .. } => "cmd.set_track_comps",
            SetTrackInput { .. } => "cmd.set_track_input",
            SetTrackArmed { .. } => "cmd.set_track_armed",
            SetTrackTakesOpen { .. } => "cmd.set_track_takes_open",
            SetTrackPitchOpen { .. } => "cmd.pitch.lane",
            SetRecordSettings { .. } => "cmd.set_record_settings",
            Batch { label_key, .. } => label_key,
        }
    }

    /// `true` when `self` sets the same parameter as `older`, so in a merged gesture the
    /// newer value simply replaces the older one (a fader drag). Anything else — inserts,
    /// removals, different targets — accumulates instead.
    pub fn supersedes(&self, older: &Command) -> bool {
        use Command::*;
        match (self, older) {
            (SetProjectName { .. }, SetProjectName { .. })
            | (SetTempo { .. }, SetTempo { .. })
            | (SetTimeSignature { .. }, SetTimeSignature { .. })
            | (SetLoopRegion { .. }, SetLoopRegion { .. })
            | (SetLoopEnabled { .. }, SetLoopEnabled { .. })
            | (SetMetronome { .. }, SetMetronome { .. })
            | (SetPatternMode { .. }, SetPatternMode { .. })
            | (SetMasterGain { .. }, SetMasterGain { .. })
            | (SetMasterPan { .. }, SetMasterPan { .. })
            | (SetMasterWidth { .. }, SetMasterWidth { .. })
            | (SetMasterPhaseInvert { .. }, SetMasterPhaseInvert { .. })
            | (SetMasterFxEnabled { .. }, SetMasterFxEnabled { .. })
            | (SetPanLaw { .. }, SetPanLaw { .. })
            | (SetRecordSettings { .. }, SetRecordSettings { .. })
            | (SetTempoTrack { .. }, SetTempoTrack { .. })
            | (SetMetreTrack { .. }, SetMetreTrack { .. }) => true,
            (SetTempoPoint { index: a, .. }, SetTempoPoint { index: b, .. }) => a == b,
            (SetMarker { marker: a }, SetMarker { marker: b }) => a.id == b.id,
            (SetMapping { mapping: a }, SetMapping { mapping: b }) => a.id == b.id,
            (SetModulatorAmount { id: a, .. }, SetModulatorAmount { id: b, .. }) => a == b,
            (SetTrackArmed { id: a, .. }, SetTrackArmed { id: b, .. }) => a == b,
            (SetTrackInput { id: a, .. }, SetTrackInput { id: b, .. }) => a == b,
            (
                SetClipAudio {
                    track: ta,
                    clip: ca,
                    ..
                },
                SetClipAudio {
                    track: tb,
                    clip: cb,
                    ..
                },
            ) => ta == tb && ca == cb,
            (
                SetClipAutomation {
                    track: ta,
                    clip: ca,
                    ..
                },
                SetClipAutomation {
                    track: tb,
                    clip: cb,
                    ..
                },
            ) => ta == tb && ca == cb,
            // A lane drawn (ADR 0068): the newest lanes.
            (
                SetClipLanes {
                    track: ta,
                    clip: ca,
                    ..
                },
                SetClipLanes {
                    track: tb,
                    clip: cb,
                    ..
                },
            ) => ta == tb && ca == cb,
            (SetTrackAutoMode { track: a, .. }, SetTrackAutoMode { track: b, .. }) => a == b,
            (SetMasterAutoMode { .. }, SetMasterAutoMode { .. }) => true,
            // A modulator edited whole (ADR 0057): a drag keeps the newest.
            (SetModulator { modulator: a }, SetModulator { modulator: b }) => a.id == b.id,
            // Round 31 (ADR 0069): a knob's drag, a chain's edit, a groove's amounts.
            (SetTrackNoteFx { track: a, .. }, SetTrackNoteFx { track: b, .. }) => a == b,
            (
                SetNoteFxParam {
                    effect: a, key: ka, ..
                },
                SetNoteFxParam {
                    effect: b, key: kb, ..
                },
            ) => a == b && ka == kb,
            (SetProjectKey { .. }, SetProjectKey { .. }) => true,
            (
                SetClipGroove {
                    track: ta,
                    clip: ca,
                    ..
                },
                SetClipGroove {
                    track: tb,
                    clip: cb,
                    ..
                },
            ) => ta == tb && ca == cb,
            (SetGroove { groove: a }, SetGroove { groove: b }) => a.id == b.id,
            // A seam dragged (ADR 0058): the newest place.
            (
                SetCompSeam {
                    track: ta,
                    left: la,
                    right: ra,
                    ..
                },
                SetCompSeam {
                    track: tb,
                    left: lb,
                    right: rb,
                    ..
                },
            ) => ta == tb && la == lb && ra == rb,
            (
                SetClipPitch {
                    track: ta,
                    clip: ca,
                    op: oa,
                    ..
                },
                SetClipPitch {
                    track: tb,
                    clip: cb,
                    op: ob,
                    ..
                },
            ) => ta == tb && ca == cb && oa == ob && oa.merges(),
            // A warp marker or the transposition dragged (ADR 0061): the newest.
            (
                SetClipWarp {
                    track: ta,
                    clip: ca,
                    op: oa,
                    ..
                },
                SetClipWarp {
                    track: tb,
                    clip: cb,
                    op: ob,
                    ..
                },
            ) => ta == tb && ca == cb && oa == ob && oa.merges(),
            (SetTrackName { id: a, .. }, SetTrackName { id: b, .. })
            | (SetTrackColor { id: a, .. }, SetTrackColor { id: b, .. })
            | (SetTrackGain { id: a, .. }, SetTrackGain { id: b, .. })
            | (SetTrackPan { id: a, .. }, SetTrackPan { id: b, .. })
            | (SetTrackStrip { id: a, .. }, SetTrackStrip { id: b, .. })
            | (SetTrackMute { id: a, .. }, SetTrackMute { id: b, .. })
            | (SetTrackSolo { id: a, .. }, SetTrackSolo { id: b, .. }) => a == b,
            (SetTrackHeight { id: a, .. }, SetTrackHeight { id: b, .. }) => a == b,
            (SetStripName { id: a, .. }, SetStripName { id: b, .. })
            | (SetStripGain { id: a, .. }, SetStripGain { id: b, .. })
            | (SetStripPan { id: a, .. }, SetStripPan { id: b, .. })
            | (SetStripWidth { id: a, .. }, SetStripWidth { id: b, .. })
            | (SetStripPhaseInvert { id: a, .. }, SetStripPhaseInvert { id: b, .. })
            | (SetStripMute { id: a, .. }, SetStripMute { id: b, .. })
            | (SetStripOutput { id: a, .. }, SetStripOutput { id: b, .. })
            | (SetStripColor { id: a, .. }, SetStripColor { id: b, .. })
            | (SetStripSolo { id: a, .. }, SetStripSolo { id: b, .. })
            | (SetStripSwapChannels { id: a, .. }, SetStripSwapChannels { id: b, .. })
            | (SetStripFxEnabled { id: a, .. }, SetStripFxEnabled { id: b, .. })
            | (SetStripDock { id: a, .. }, SetStripDock { id: b, .. })
            | (SetStripGroup { id: a, .. }, SetStripGroup { id: b, .. }) => a == b,
            (SetStripGroupName { id: a, .. }, SetStripGroupName { id: b, .. })
            | (SetStripGroupColor { id: a, .. }, SetStripGroupColor { id: b, .. })
            | (SetStripGroupCollapsed { id: a, .. }, SetStripGroupCollapsed { id: b, .. }) => {
                a == b
            }
            (
                SetStripEq {
                    id: a,
                    band: ba,
                    field: fa,
                    ..
                },
                SetStripEq {
                    id: b,
                    band: bb,
                    field: fb,
                    ..
                },
            ) => a == b && ba == bb && fa == fb,
            (SetStripSend { id: a, send: sa }, SetStripSend { id: b, send: sb }) => {
                a == b && sa.to == sb.to
            }
            (
                SetEffectMix {
                    strip: a,
                    effect: ea,
                    ..
                },
                SetEffectMix {
                    strip: b,
                    effect: eb,
                    ..
                },
            ) => a == b && ea == eb,
            (
                SetPluginParam {
                    strip: a,
                    effect: ea,
                    id: ia,
                    ..
                },
                SetPluginParam {
                    strip: b,
                    effect: eb,
                    id: ib,
                    ..
                },
            ) => a == b && ea == eb && ia == ib,
            (
                SetPluginState {
                    strip: a,
                    effect: ea,
                    ..
                },
                SetPluginState {
                    strip: b,
                    effect: eb,
                    ..
                },
            ) => a == b && ea == eb,
            (
                SetInstrumentPluginParam {
                    track: a, id: ia, ..
                },
                SetInstrumentPluginParam {
                    track: b, id: ib, ..
                },
            ) => a == b && ia == ib,
            (
                SetInstrumentPluginState { track: a, .. },
                SetInstrumentPluginState { track: b, .. },
            ) => a == b,
            // A batch of the same shape — the ganged faders of a multi-selection — is one
            // gesture too: every pair supersedes, so the batch does.
            (Batch { commands: a, .. }, Batch { commands: b, .. }) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.supersedes(y))
            }
            (
                SetInstrumentParam {
                    track: a,
                    param: pa,
                    ..
                },
                SetInstrumentParam {
                    track: b,
                    param: pb,
                    ..
                },
            ) => a == b && pa == pb,
            // A layer knob dragged: every frame replaces the same pad's list.
            (
                SetPadLayers {
                    track: a, pad: pa, ..
                },
                SetPadLayers {
                    track: b, pad: pb, ..
                },
            ) => a == b && pa == pb,
            (
                SetInstrumentValue {
                    track: a, key: ka, ..
                },
                SetInstrumentValue {
                    track: b, key: kb, ..
                },
            ) => a == b && ka == kb,
            (
                SetEffectBypass {
                    strip: a,
                    effect: ea,
                    ..
                },
                SetEffectBypass {
                    strip: b,
                    effect: eb,
                    ..
                },
            ) => a == b && ea == eb,
            (
                SetEffectParam {
                    strip: a,
                    effect: ea,
                    key: ka,
                    ..
                },
                SetEffectParam {
                    strip: b,
                    effect: eb,
                    key: kb,
                    ..
                },
            ) => a == b && ea == eb && ka == kb,
            (
                SetClipBounds {
                    track: a, clip: ca, ..
                },
                SetClipBounds {
                    track: b, clip: cb, ..
                },
            )
            | (
                SetClipName {
                    track: a, clip: ca, ..
                },
                SetClipName {
                    track: b, clip: cb, ..
                },
            )
            | (
                SetClipOffset {
                    track: a, clip: ca, ..
                },
                SetClipOffset {
                    track: b, clip: cb, ..
                },
            ) => a == b && ca == cb,
            // The same windows of one track re-anchored again.
            (
                SetClipAnchors {
                    track: a,
                    anchors: xa,
                },
                SetClipAnchors {
                    track: b,
                    anchors: xb,
                },
            ) => a == b && xa.len() == xb.len() && xa.iter().zip(xb).all(|(x, y)| x.0 == y.0),
            (
                SetNotes {
                    track: a,
                    clip: ca,
                    notes: na,
                },
                SetNotes {
                    track: b,
                    clip: cb,
                    notes: nb,
                },
            ) => {
                a == b
                    && ca == cb
                    && na.len() == nb.len()
                    && na.iter().all(|n| nb.iter().any(|m| m.id == n.id))
            }
            _ => false,
        }
    }

    /// Apply to `project`, returning the inverse command. On error the project is
    /// unchanged (batches roll back).
    pub fn apply(self, project: &mut Project) -> Result<Command, CommandError> {
        // Frozen sounds are locked (ADR 0064): an edit that would change one is refused.
        crate::freeze::guard(self, project, Command::apply_unguarded)
    }

    /// The command's body, without the frozen lock ([`Self::apply`] is what runs it).
    fn apply_unguarded(self, project: &mut Project) -> Result<Command, CommandError> {
        use Command::*;
        match self {
            SetProjectName { name } => {
                let old = std::mem::replace(&mut project.name, name);
                Ok(SetProjectName { name: old })
            }
            SetTempo { bpm } => {
                let old = project.tempo_bpm;
                project.tempo_bpm = clamp_bpm(bpm);
                // With a tempo track its first point is the song's tempo (ADR 0060).
                if let Some(p) = project.tempo_points.first_mut() {
                    p.bpm = project.tempo_bpm;
                }
                Ok(SetTempo { bpm: old })
            }
            SetTimeSignature { time_signature } => {
                if time_signature.numerator == 0
                    || !matches!(time_signature.denominator, 1 | 2 | 4 | 8 | 16 | 32)
                {
                    return Err(CommandError::InvalidValue(format!(
                        "time signature {time_signature}"
                    )));
                }
                // With metre changes the song's metre is the first, and the later changes
                // keep their bars, as `SetMetreAt` at tick 0 (ADR 0060, review 1).
                if !project.metre_changes.is_empty() {
                    return crate::tempo::edit::apply_set_metre_at(
                        project,
                        Ticks::ZERO,
                        time_signature,
                    );
                }
                let old = std::mem::replace(&mut project.time_signature, time_signature);
                Ok(SetTimeSignature {
                    time_signature: old,
                })
            }
            SetLoopRegion { region } => {
                if let Some(r) = &region {
                    if r.end <= r.start || r.start.is_negative() {
                        return Err(CommandError::InvalidValue("loop region".into()));
                    }
                }
                let old = std::mem::replace(&mut project.loop_region, region);
                Ok(SetLoopRegion { region: old })
            }
            SetLoopEnabled { enabled } => {
                let old = std::mem::replace(&mut project.loop_enabled, enabled);
                Ok(SetLoopEnabled { enabled: old })
            }
            SetMetronome { enabled } => {
                let old = std::mem::replace(&mut project.metronome, enabled);
                Ok(SetMetronome { enabled: old })
            }
            SetPatternMode { on } => {
                let old = std::mem::replace(&mut project.pattern_mode, on);
                Ok(SetPatternMode { on: old })
            }
            SetMasterGain { gain_db } => {
                let old = project.master.gain_db;
                project.master.gain_db = clamp_gain(gain_db)?;
                Ok(SetMasterGain { gain_db: old })
            }
            SetMasterPan { pan } => {
                let old = std::mem::replace(&mut project.master.pan, clamp_pan(pan)?);
                Ok(SetMasterPan { pan: old })
            }
            SetMasterWidth { width } => {
                let old = std::mem::replace(&mut project.master.width, clamp_width(width)?);
                Ok(SetMasterWidth { width: old })
            }
            SetMasterPhaseInvert { invert } => {
                let old = std::mem::replace(&mut project.master.phase_invert, invert);
                Ok(SetMasterPhaseInvert { invert: old })
            }
            SetMasterFxEnabled { enabled } => {
                let old = std::mem::replace(&mut project.master.fx_enabled, enabled);
                Ok(SetMasterFxEnabled { enabled: old })
            }
            SetPanLaw { law } => {
                let old = std::mem::replace(&mut project.pan_law, law);
                Ok(SetPanLaw { law: old })
            }
            ReplaceProject { project: new } => {
                let old = std::mem::replace(project, *new);
                Ok(ReplaceProject {
                    project: Box::new(old),
                })
            }

            InsertTrack { index, track } => {
                let len = project.tracks.len();
                if index > len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                if project.track(track.id).is_some() {
                    return Err(CommandError::DuplicateId(track.id.0));
                }
                // A delay a bounce could not have kept (review 3).
                if track.lateness.is_some_and(|l| !l.is_valid()) {
                    return Err(CommandError::InvalidValue("lateness".into()));
                }
                crate::modulator::check_new(project, &track.modulators)?;
                // Its note effects (ADR 0069): valid, their ids no other track's.
                crate::notefx_cmd::check_new(project, &track.note_fx)?;
                // Its alternate comps (ADR 0058): no kept window may share an id with
                // anything the project or the track itself holds (review 1).
                crate::comp::check(project, track.id, &track.comps, track.active_comp)?;
                if let Some(w) = track
                    .comps
                    .iter()
                    .flat_map(|c| c.windows.iter())
                    .find(|w| track.clips.iter().any(|c| c.id == w.id))
                {
                    return Err(CommandError::DuplicateId(w.id.0));
                }
                for (i, c) in track.clips.iter().enumerate() {
                    // Each clip as `InsertClip` takes it (round 29's second review: a warp
                    // `SetClipWarp` refuses came in with a track).
                    validate_clip(c)?;
                    check_joins(project, c)?;
                    crate::notefx_cmd::check_clip(project, c)?;
                    // The track's own clips must agree with each other too: the project
                    // does not hold them yet, so `check_joins` cannot see them.
                    if let Some(first) = track.clips[..i]
                        .iter()
                        .find(|f| f.link.is_some() && f.link == c.link)
                    {
                        if !crate::links::same_content(first, c) {
                            return Err(CommandError::InvalidValue(format!(
                                "clip {} does not match clip {} of its link group",
                                c.id, first.id
                            )));
                        }
                    }
                }
                let id = track.id;
                project.tracks.insert(index, *track);
                Ok(RemoveTrack { id })
            }
            RemoveTrack { id } => {
                let index = project
                    .track_index(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let track = project.tracks.remove(index);
                Ok(InsertTrack {
                    index,
                    track: Box::new(track),
                })
            }
            MoveTrack { id, index } => {
                let from = project
                    .track_index(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let len = project.tracks.len();
                if index >= len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                let t = project.tracks.remove(from);
                project.tracks.insert(index, t);
                Ok(MoveTrack { id, index: from })
            }
            SetTrackName { id, name } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.name, name);
                Ok(SetTrackName { id, name: old })
            }
            SetTrackColor { id, color } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.color, color);
                Ok(SetTrackColor { id, color: old })
            }
            SetTrackGain { id, gain_db } => {
                let gain_db = clamp_gain(gain_db)?;
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.gain_db, gain_db);
                Ok(SetTrackGain { id, gain_db: old })
            }
            SetTrackPan { id, pan } => {
                if !pan.is_finite() {
                    return Err(CommandError::InvalidValue("pan".into()));
                }
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.pan, pan.clamp(-1.0, 1.0));
                Ok(SetTrackPan { id, pan: old })
            }
            SetTrackStrip { id, strip } => {
                if let Some(sid) = strip {
                    if project.strip(sid).is_none() {
                        return Err(CommandError::StripNotFound(sid));
                    }
                }
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.strip, strip);
                Ok(SetTrackStrip { id, strip: old })
            }

            InsertStrip { index, strip } => {
                let len = project.strips.len();
                if index > len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                if project.strip(strip.id).is_some() {
                    return Err(CommandError::DuplicateId(strip.id.0));
                }
                crate::modulator::check_new(project, &strip.modulators)?;
                let id = strip.id;
                project.strips.insert(index, *strip);
                Ok(RemoveStrip { id })
            }
            RemoveStrip { id } => {
                let index = project
                    .strip_index(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let fed = project.tracks.iter().any(|t| t.strip == Some(id))
                    || project.strips.iter().any(|s| s.feeds().any(|to| to == id))
                    || project.strips.iter().any(|s| s.keys().any(|k| k == id))
                    || project
                        .master
                        .effects
                        .iter()
                        .any(|e| e.sidechain == Some(id));
                if fed {
                    return Err(CommandError::StripInUse(id));
                }
                let strip = project.strips.remove(index);
                Ok(InsertStrip {
                    index,
                    strip: Box::new(strip),
                })
            }
            SetStripName { id, name } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.name, name);
                Ok(SetStripName { id, name: old })
            }
            SetStripGain { id, gain_db } => {
                let gain_db = clamp_gain(gain_db)?;
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.gain_db, gain_db);
                Ok(SetStripGain { id, gain_db: old })
            }
            SetStripPan { id, pan } => {
                if !pan.is_finite() {
                    return Err(CommandError::InvalidValue("pan".into()));
                }
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.pan, pan.clamp(-1.0, 1.0));
                Ok(SetStripPan { id, pan: old })
            }
            SetStripWidth { id, width } => {
                let width = clamp_width(width)?;
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.width, width);
                Ok(SetStripWidth { id, width: old })
            }
            SetStripPhaseInvert { id, invert } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.phase_invert, invert);
                Ok(SetStripPhaseInvert { id, invert: old })
            }
            SetStripMute { id, muted } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.muted, muted);
                Ok(SetStripMute { id, muted: old })
            }
            SetStripOutput { id, output } => {
                if let Some(o) = output {
                    if project.strip(o).is_none() {
                        return Err(CommandError::StripNotFound(o));
                    }
                }
                if project.strip(id).is_none() {
                    return Err(CommandError::StripNotFound(id));
                }
                if project.routing_would_cycle(id, output) {
                    return Err(CommandError::RoutingCycle(id));
                }
                let s = project.strip_mut(id).expect("checked above");
                let old = std::mem::replace(&mut s.output, output);
                Ok(SetStripOutput { id, output: old })
            }
            SetStripColor { id, color } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.color, color);
                Ok(SetStripColor { id, color: old })
            }
            SetStripSolo { id, soloed } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.soloed, soloed);
                Ok(SetStripSolo { id, soloed: old })
            }
            SetStripSwapChannels { id, swap } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.swap_channels, swap);
                Ok(SetStripSwapChannels { id, swap: old })
            }
            SetStripFxEnabled { id, enabled } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.fx_enabled, enabled);
                Ok(SetStripFxEnabled { id, enabled: old })
            }
            SetStripEq {
                id,
                band,
                field,
                value,
            } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                if band as usize >= s.eq.bands.len() {
                    return Err(CommandError::IndexOutOfRange {
                        index: band as usize,
                        len: s.eq.bands.len(),
                    });
                }
                let old = s.eq.get(band as usize, field);
                s.eq.set(band as usize, field, value)
                    .ok_or_else(|| CommandError::InvalidValue("eq".into()))?;
                Ok(SetStripEq {
                    id,
                    band,
                    field,
                    value: old,
                })
            }
            SetStripSend { id, send } => {
                if !send.level.is_finite() {
                    return Err(CommandError::InvalidValue("send level".into()));
                }
                if project.strip(send.to).is_none() {
                    return Err(CommandError::StripNotFound(send.to));
                }
                if project.strip(id).is_none() {
                    return Err(CommandError::StripNotFound(id));
                }
                let existing = project.strip(id).and_then(|s| s.send_to(send.to)).copied();
                // A send that is already there is a route already; only a *new* one can
                // close a loop or run out of room.
                if existing.is_none() {
                    if project.routing_would_cycle(id, Some(send.to)) {
                        return Err(CommandError::RoutingCycle(id));
                    }
                    if project.strip(id).map(|s| s.sends.len()).unwrap_or(0) >= MAX_SENDS {
                        return Err(CommandError::TooManySends(id));
                    }
                }
                let send = Send {
                    level: send.level.clamp(0.0, 1.0),
                    ..send
                };
                let s = project.strip_mut(id).expect("checked above");
                match s.send_to_mut(send.to) {
                    Some(slot) => {
                        let old = std::mem::replace(slot, send);
                        Ok(SetStripSend { id, send: old })
                    }
                    None => {
                        s.sends.push(send);
                        Ok(RemoveStripSend { id, to: send.to })
                    }
                }
            }
            RemoveStripSend { id, to } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let index = s
                    .sends
                    .iter()
                    .position(|x| x.to == to)
                    .ok_or(CommandError::SendNotFound(id))?;
                let old = s.sends.remove(index);
                Ok(InsertStripSend {
                    id,
                    index,
                    send: old,
                })
            }
            InsertStripSend { id, index, send } => {
                if !send.level.is_finite() {
                    return Err(CommandError::InvalidValue("send level".into()));
                }
                if project.strip(send.to).is_none() {
                    return Err(CommandError::StripNotFound(send.to));
                }
                let Some(strip) = project.strip(id) else {
                    return Err(CommandError::StripNotFound(id));
                };
                if strip.send_to(send.to).is_some() {
                    return Err(CommandError::InvalidValue(
                        "a send to that strip exists".into(),
                    ));
                }
                if strip.sends.len() >= MAX_SENDS {
                    return Err(CommandError::TooManySends(id));
                }
                if project.routing_would_cycle(id, Some(send.to)) {
                    return Err(CommandError::RoutingCycle(id));
                }
                let send = Send {
                    level: send.level.clamp(0.0, 1.0),
                    ..send
                };
                let s = project.strip_mut(id).expect("checked above");
                let at = index.min(s.sends.len());
                s.sends.insert(at, send);
                Ok(RemoveStripSend { id, to: send.to })
            }
            MoveStrip { id, index } => {
                let from = project
                    .strip_index(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let len = project.strips.len();
                if index >= len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                let s = project.strips.remove(from);
                project.strips.insert(index, s);
                Ok(MoveStrip { id, index: from })
            }
            SetStripDock { id, dock } => {
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.dock, dock);
                Ok(SetStripDock { id, dock: old })
            }
            SetStripGroup { id, group } => {
                if let Some(g) = group {
                    if project.strip_group(g).is_none() {
                        return Err(CommandError::GroupNotFound(g));
                    }
                }
                let s = project
                    .strip_mut(id)
                    .ok_or(CommandError::StripNotFound(id))?;
                let old = std::mem::replace(&mut s.group, group);
                Ok(SetStripGroup { id, group: old })
            }
            InsertStripGroup { index, group } => {
                let len = project.strip_groups.len();
                if index > len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                if project.strip_group(group.id).is_some() {
                    return Err(CommandError::DuplicateId(group.id.0));
                }
                let id = group.id;
                project.strip_groups.insert(index, *group);
                Ok(RemoveStripGroup { id })
            }
            RemoveStripGroup { id } => {
                let index = project
                    .strip_groups
                    .iter()
                    .position(|g| g.id == id)
                    .ok_or(CommandError::GroupNotFound(id))?;
                if project.strips.iter().any(|s| s.group == Some(id)) {
                    return Err(CommandError::GroupInUse(id));
                }
                let group = project.strip_groups.remove(index);
                Ok(InsertStripGroup {
                    index,
                    group: Box::new(group),
                })
            }
            SetStripGroupName { id, name } => {
                let g = project
                    .strip_group_mut(id)
                    .ok_or(CommandError::GroupNotFound(id))?;
                let old = std::mem::replace(&mut g.name, name);
                Ok(SetStripGroupName { id, name: old })
            }
            SetStripGroupColor { id, color } => {
                let g = project
                    .strip_group_mut(id)
                    .ok_or(CommandError::GroupNotFound(id))?;
                let old = std::mem::replace(&mut g.color, color);
                Ok(SetStripGroupColor { id, color: old })
            }
            SetStripGroupCollapsed { id, collapsed } => {
                let g = project
                    .strip_group_mut(id)
                    .ok_or(CommandError::GroupNotFound(id))?;
                let old = std::mem::replace(&mut g.collapsed, collapsed);
                Ok(SetStripGroupCollapsed { id, collapsed: old })
            }
            SetTrackHeight { id, height } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = t.height;
                t.height = if height.is_finite() {
                    height.clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT)
                } else {
                    1.0
                };
                Ok(SetTrackHeight { id, height: old })
            }

            SetTrackIcon { id, icon } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = t.icon;
                t.icon = icon;
                Ok(SetTrackIcon { id, icon: old })
            }

            SetTrackMute { id, muted } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.muted, muted);
                Ok(SetTrackMute { id, muted: old })
            }
            SetTrackSolo { id, soloed } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.soloed, soloed);
                Ok(SetTrackSolo { id, soloed: old })
            }
            SetInstrumentParam {
                track,
                param,
                value,
            } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let old = t
                    .instrument
                    .param(param)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                t.instrument
                    .set_param(param, value)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                Ok(SetInstrumentParam {
                    track,
                    param,
                    value: old,
                })
            }

            SetInstrumentValue { track, key, value } => {
                let n = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?
                    .instrument
                    .native_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let stored = n.params.get(&key).copied();
                n.set(&key, value)
                    .ok_or_else(|| CommandError::InvalidValue(format!("instrument value {key}")))?;
                Ok(match stored {
                    Some(old) => SetInstrumentValue {
                        track,
                        key,
                        value: old,
                    },
                    None => ClearInstrumentValue { track, key },
                })
            }
            ClearInstrumentValue { track, key } => {
                let n = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?
                    .instrument
                    .native_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                if n.kind.param(&key).is_none() {
                    return Err(CommandError::InvalidValue(format!(
                        "instrument value {key}"
                    )));
                }
                Ok(match n.clear(&key) {
                    Some(old) => SetInstrumentValue {
                        track,
                        key,
                        value: old,
                    },
                    // Clearing what was never set is nothing, and its inverse is nothing.
                    None => ClearInstrumentValue { track, key },
                })
            }
            SetInstrumentText { track, key, text } => {
                let n = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?
                    .instrument
                    .native_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let old = n
                    .set_text(&key, text)
                    .map_err(|()| CommandError::InvalidValue(format!("instrument text {key}")))?;
                Ok(SetInstrumentText {
                    track,
                    key,
                    text: old,
                })
            }

            SetInstrument { track, spec } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let old = std::mem::replace(&mut t.instrument, *spec);
                Ok(SetInstrument {
                    track,
                    spec: Box::new(old),
                })
            }

            InsertPad { track, pad } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                if kit.index_of_pitch(pad.pitch).is_some() {
                    return Err(CommandError::DuplicateId(pad.pitch as u64));
                }
                let mut pad = *pad;
                if pad.id.0 == 0 {
                    pad.id = kit.next_id();
                } else if kit.index_of(pad.id).is_some() {
                    return Err(CommandError::DuplicateId(pad.id.0 as u64));
                }
                let id = pad.id;
                let at = kit.pads.partition_point(|p| p.pitch < pad.pitch);
                kit.pads.insert(at, pad);
                Ok(RemovePad { track, pad: id })
            }

            RemovePad { track, pad: id } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let at = kit
                    .index_of(id)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let pad = kit.pads.remove(at);
                Ok(InsertPad {
                    track,
                    pad: Box::new(pad),
                })
            }

            SetPadName { track, pad, name } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let slot = kit
                    .by_id_mut(pad)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let old = std::mem::replace(&mut slot.name, name);
                Ok(SetPadName {
                    track,
                    pad,
                    name: old,
                })
            }

            SetPadSample { track, pad, path } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let slot = kit
                    .by_id_mut(pad)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let old = std::mem::replace(&mut slot.sample, path);
                Ok(SetPadSample {
                    track,
                    pad,
                    path: old,
                })
            }

            SetPadLayers { track, pad, layers } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let slot = kit
                    .by_id_mut(pad)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let mut layers = layers;
                layers.truncate(crate::pads::MAX_LAYERS);
                let old = std::mem::replace(&mut slot.layers, layers);
                Ok(SetPadLayers {
                    track,
                    pad,
                    layers: old,
                })
            }

            InsertEffect { strip, index, slot } => {
                let t = chain_mut(project, strip)?;
                let len = t.len();
                if index > len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                if len >= MAX_INSERTS {
                    return Err(CommandError::TooManyInserts(strip));
                }
                if t.iter().any(|e| e.id == slot.id) {
                    return Err(CommandError::DuplicateId(slot.id.0));
                }
                let id = slot.id;
                t.insert(index, *slot);
                Ok(RemoveEffect { strip, effect: id })
            }
            RemoveEffect { strip, effect } => {
                let t = chain_mut(project, strip)?;
                let index = t
                    .iter()
                    .position(|e| e.id == effect)
                    .ok_or(CommandError::EffectNotFound(effect))?;
                let slot = t.remove(index);
                Ok(InsertEffect {
                    strip,
                    index,
                    slot: Box::new(slot),
                })
            }
            MoveEffect {
                strip,
                effect,
                index,
            } => {
                let t = chain_mut(project, strip)?;
                let from = t
                    .iter()
                    .position(|e| e.id == effect)
                    .ok_or(CommandError::EffectNotFound(effect))?;
                let len = t.len();
                if index >= len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                let e = t.remove(from);
                t.insert(index, e);
                Ok(MoveEffect {
                    strip,
                    effect,
                    index: from,
                })
            }
            ReplaceEffect {
                strip,
                effect,
                slot,
            } => {
                check_effect_routes(project, strip, &slot)?;
                let t = chain_mut(project, strip)?;
                let index = t
                    .iter()
                    .position(|e| e.id == effect)
                    .ok_or(CommandError::EffectNotFound(effect))?;
                if slot.id != effect && t.iter().any(|e| e.id == slot.id) {
                    return Err(CommandError::DuplicateId(slot.id.0));
                }
                let id = slot.id;
                let old = std::mem::replace(&mut t[index], *slot);
                Ok(ReplaceEffect {
                    strip,
                    effect: id,
                    slot: Box::new(old),
                })
            }
            SetEffectBypass {
                strip,
                effect,
                bypass,
            } => {
                let e = slot_mut(project, strip, effect)?;
                let old = std::mem::replace(&mut e.bypass, bypass);
                Ok(SetEffectBypass {
                    strip,
                    effect,
                    bypass: old,
                })
            }
            SetEffectParam {
                strip,
                effect,
                key,
                value,
            } => {
                let e = slot_mut(project, strip, effect)?;
                let stored = e.spec.params.get(&key).copied();
                e.spec
                    .set(&key, value)
                    .ok_or_else(|| CommandError::InvalidValue(format!("effect param {key}")))?;
                Ok(match stored {
                    Some(old) => SetEffectParam {
                        strip,
                        effect,
                        key,
                        value: old,
                    },
                    None => ClearEffectParam { strip, effect, key },
                })
            }
            SetEffectSidechain {
                strip,
                effect,
                source,
            } => {
                if let Some(src) = source {
                    // The master cannot be keyed from: every strip feeds it, so a key from
                    // it into anything is a loop (ADR 0026). A key *into* a master insert
                    // from any strip is fine — the master is always processed last.
                    if src.is_master() {
                        return Err(CommandError::RoutingCycle(strip));
                    }
                    if project.strip(src).is_none() {
                        return Err(CommandError::StripNotFound(src));
                    }
                    if src == strip || project.routing_would_cycle(src, Some(strip)) {
                        return Err(CommandError::RoutingCycle(strip));
                    }
                }
                let e = slot_mut(project, strip, effect)?;
                if source.is_some() && !e.spec.takes_sidechain() {
                    return Err(CommandError::InvalidValue("sidechain".into()));
                }
                let old = std::mem::replace(&mut e.sidechain, source);
                Ok(SetEffectSidechain {
                    strip,
                    effect,
                    source: old,
                })
            }
            SetEffectMidiSource {
                strip,
                effect,
                source,
            } => {
                if let Some(src) = source {
                    if project.track(src).is_none() {
                        return Err(CommandError::TrackNotFound(src));
                    }
                }
                let e = slot_mut(project, strip, effect)?;
                if source.is_some() && !e.spec.kind.takes_notes() {
                    return Err(CommandError::InvalidValue("midi source".into()));
                }
                let old = std::mem::replace(&mut e.midi_from, source);
                Ok(SetEffectMidiSource {
                    strip,
                    effect,
                    source: old,
                })
            }
            ClearEffectParam { strip, effect, key } => {
                let e = slot_mut(project, strip, effect)?;
                if e.spec.kind.param(&key).is_none() {
                    return Err(CommandError::InvalidValue(format!("effect param {key}")));
                }
                Ok(match e.spec.params.remove(&key) {
                    Some(old) => SetEffectParam {
                        strip,
                        effect,
                        key,
                        value: old,
                    },
                    // Clearing what was never set is nothing, and its inverse is nothing.
                    None => ClearEffectParam { strip, effect, key },
                })
            }
            SetEffectMix { strip, effect, mix } => {
                if !mix.is_finite() {
                    return Err(CommandError::InvalidValue("mix".into()));
                }
                let e = slot_mut(project, strip, effect)?;
                let old = std::mem::replace(&mut e.mix, mix.clamp(0.0, 1.0));
                Ok(SetEffectMix {
                    strip,
                    effect,
                    mix: old,
                })
            }
            SetPluginParam {
                strip,
                effect,
                id,
                value,
            } => {
                if value.is_some_and(|v| !v.is_finite()) {
                    return Err(CommandError::InvalidValue("plug-in parameter".into()));
                }
                let plugin = external_mut(project, strip, effect)?;
                let old = match value {
                    Some(v) => plugin.params.insert(id, v),
                    None => plugin.params.remove(&id),
                };
                Ok(SetPluginParam {
                    strip,
                    effect,
                    id,
                    value: old,
                })
            }
            SetPluginState {
                strip,
                effect,
                state,
                params,
            } => {
                if params.values().any(|v| !v.is_finite()) {
                    return Err(CommandError::InvalidValue("plug-in parameter".into()));
                }
                let plugin = external_mut(project, strip, effect)?;
                let old_state = std::mem::replace(&mut plugin.state, state);
                let old_params = std::mem::replace(&mut plugin.params, params);
                Ok(SetPluginState {
                    strip,
                    effect,
                    state: old_state,
                    params: old_params,
                })
            }
            SetInstrumentPluginParam { track, id, value } => {
                if value.is_some_and(|v| !v.is_finite()) {
                    return Err(CommandError::InvalidValue("plug-in parameter".into()));
                }
                let plugin = instrument_plugin_mut(project, track)?;
                let old = match value {
                    Some(v) => plugin.params.insert(id, v),
                    None => plugin.params.remove(&id),
                };
                Ok(SetInstrumentPluginParam {
                    track,
                    id,
                    value: old,
                })
            }
            SetInstrumentPluginState {
                track,
                state,
                params,
            } => {
                if params.values().any(|v| !v.is_finite()) {
                    return Err(CommandError::InvalidValue("plug-in parameter".into()));
                }
                let plugin = instrument_plugin_mut(project, track)?;
                let old_state = std::mem::replace(&mut plugin.state, state);
                let old_params = std::mem::replace(&mut plugin.params, params);
                Ok(SetInstrumentPluginState {
                    track,
                    state: old_state,
                    params: old_params,
                })
            }
            // Round 29, isolation (ADR 0063): where a hosted plug-in runs.
            SetPluginIsolation {
                strip,
                effect,
                isolation,
            } => {
                let plugin = external_mut(project, strip, effect)?;
                let old = std::mem::replace(&mut plugin.isolation, isolation);
                Ok(SetPluginIsolation {
                    strip,
                    effect,
                    isolation: old,
                })
            }
            SetInstrumentIsolation { track, isolation } => {
                let plugin = instrument_plugin_mut(project, track)?;
                let old = std::mem::replace(&mut plugin.isolation, isolation);
                Ok(SetInstrumentIsolation {
                    track,
                    isolation: old,
                })
            }
            SetStripFrozen { strip, frozen } => {
                let s = project
                    .strip_mut(strip)
                    .ok_or(CommandError::StripNotFound(strip))?;
                let old = std::mem::replace(&mut s.frozen, frozen);
                Ok(SetStripFrozen { strip, frozen: old })
            }
            SetTrackFrozen { track, frozen } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let old = std::mem::replace(&mut t.frozen, frozen);
                Ok(SetTrackFrozen { track, frozen: old })
            }
            SetTrackLateness { id, lateness } => {
                if lateness.is_some_and(|l| !l.is_valid()) {
                    return Err(CommandError::InvalidValue("lateness".into()));
                }
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.lateness, lateness);
                Ok(SetTrackLateness { id, lateness: old })
            }
            InsertClip { track, clip } => {
                validate_clip(&clip)?;
                check_joins(project, &clip)?;
                // Its groove (ADR 0069): as `SetClipGroove` would take it.
                crate::notefx_cmd::check_clip(project, &clip)?;
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                if t.clip(clip.id).is_some() {
                    return Err(CommandError::DuplicateId(clip.id.0));
                }
                let id = clip.id;
                t.clips.push(*clip);
                // By start, then by id: two clips at the same start must land in the same
                // order whatever the history, or undo cannot restore a project exactly.
                t.clips.sort_by_key(|c| (c.start, c.id));
                Ok(RemoveClip { track, clip: id })
            }
            RemoveClip { track, clip } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let idx = t
                    .clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(CommandError::ClipNotFound(clip))?;
                let removed = t.clips.remove(idx);
                Ok(InsertClip {
                    track,
                    clip: Box::new(removed),
                })
            }
            SetClipBounds {
                track,
                clip,
                start,
                length,
            } => {
                if start.is_negative() || length <= Ticks::ZERO {
                    return Err(CommandError::InvalidValue("clip bounds".into()));
                }
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let (old_start, old_len) = (c.start, c.length);
                c.start = start;
                c.length = length;
                t.clips.sort_by_key(|c| (c.start, c.id));
                Ok(SetClipBounds {
                    track,
                    clip,
                    start: old_start,
                    length: old_len,
                })
            }
            SetClipOffset {
                track,
                clip,
                offset,
            } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.offset, offset);
                Ok(SetClipOffset {
                    track,
                    clip,
                    offset: old,
                })
            }
            SetClipAnchors { track, anchors } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let at: std::collections::HashMap<ClipId, usize> =
                    t.clips.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
                // Every window checked before any is moved: all or nothing.
                let mut places = Vec::with_capacity(anchors.len());
                for &(clip, _, slip) in &anchors {
                    let i = *at.get(&clip).ok_or(CommandError::ClipNotFound(clip))?;
                    let c = &t.clips[i];
                    let a = c
                        .audio
                        .as_ref()
                        .ok_or_else(|| CommandError::InvalidValue("not an audio clip".into()))?;
                    if c.link.is_some() && a.slip_frames != slip {
                        return Err(CommandError::InvalidValue(
                            "a linked clip's fine offset is its group's".into(),
                        ));
                    }
                    places.push(i);
                }
                let mut old = Vec::with_capacity(anchors.len());
                for (&(clip, offset, slip), i) in anchors.iter().zip(places) {
                    let c = &mut t.clips[i];
                    let a = c.audio.as_mut().expect("checked above");
                    old.push((clip, c.offset, a.slip_frames));
                    c.offset = offset;
                    a.slip_frames = slip;
                }
                old.reverse();
                Ok(SetClipAnchors {
                    track,
                    anchors: old,
                })
            }
            SetClipLoop {
                track,
                clip,
                content_length,
            } => {
                if let Some(l) = content_length {
                    if l <= Ticks::ZERO {
                        return Err(CommandError::InvalidValue("clip loop length".into()));
                    }
                }
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.content_length, content_length);
                Ok(SetClipLoop {
                    track,
                    clip,
                    content_length: old,
                })
            }
            SetClipMute { track, clip, muted } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.muted, muted);
                Ok(SetClipMute {
                    track,
                    clip,
                    muted: old,
                })
            }
            InsertAudioSource { source } => {
                if project.audio_source(source.id).is_some() {
                    return Err(CommandError::DuplicateId(source.id.0));
                }
                let id = source.id;
                project.next_id = project.next_id.max(id.0.saturating_add(1));
                project.audio_pool.push(source);
                Ok(RemoveAudioSource { id })
            }
            RemoveAudioSource { id } => {
                let index = project
                    .audio_pool
                    .iter()
                    .position(|s| s.id == id)
                    .ok_or(CommandError::AudioSourceNotFound(id))?;
                // A window kept in an alternate comp plays it too, when that comp plays
                // (ADR 0058).
                let used = project.tracks.iter().any(|t| {
                    t.clips
                        .iter()
                        .chain(t.comps.iter().flat_map(|c| c.windows.iter()))
                        .any(|c| c.audio.as_ref().is_some_and(|a| a.source == id))
                });
                if used {
                    return Err(CommandError::AudioSourceInUse(id));
                }
                let source = project.audio_pool.remove(index);
                Ok(InsertAudioSource { source })
            }
            SetAudioSourcePath { id, path } => {
                let s = project
                    .audio_source_mut(id)
                    .ok_or(CommandError::AudioSourceNotFound(id))?;
                let old = std::mem::replace(&mut s.path, path);
                Ok(SetAudioSourcePath { id, path: old })
            }
            SetClipAudio { track, clip, audio } => {
                if let Some(a) = &audio {
                    if project.audio_source(a.source).is_none() {
                        return Err(CommandError::AudioSourceNotFound(a.source));
                    }
                    if !a.gain_db.is_finite()
                        || a.fade_in < Ticks::ZERO
                        || a.fade_out < Ticks::ZERO
                        || a.xfade < Ticks::ZERO
                        || a.xfade > crate::audio::MAX_XFADE
                        // A warp only as valid as `SetClipWarp` takes it (ADR 0061).
                        || a.warp.as_ref().is_some_and(|w| !w.is_valid())
                        // A gain envelope in order and in range (ADR 0070).
                        || a.gain_env.as_ref().is_some_and(|e| !e.is_valid())
                    {
                        return Err(CommandError::InvalidValue("clip audio".into()));
                    }
                }
                let old = edit_shared(project, track, clip, |c| {
                    Ok(std::mem::replace(&mut c.audio, audio.clone()))
                })?;
                Ok(SetClipAudio {
                    track,
                    clip,
                    audio: old,
                })
            }
            SetClipPitch {
                track,
                clip,
                pitch,
                op,
            } => {
                if pitch.as_ref().is_some_and(|p| !p.is_valid()) {
                    return Err(CommandError::InvalidValue("pitch edit".into()));
                }
                let old = edit_shared(project, track, clip, |c| {
                    let a = c
                        .audio
                        .as_mut()
                        .ok_or_else(|| CommandError::InvalidValue("not an audio clip".into()))?;
                    Ok(std::mem::replace(&mut a.pitch, pitch.clone()))
                })?;
                Ok(SetClipPitch {
                    track,
                    clip,
                    pitch: old,
                    op,
                })
            }
            SetClipWarp {
                track,
                clip,
                warp,
                op,
            } => {
                if warp.as_ref().is_some_and(|w| !w.is_valid()) {
                    return Err(CommandError::InvalidValue("warp".into()));
                }
                let old = edit_shared(project, track, clip, |c| {
                    let a = c
                        .audio
                        .as_mut()
                        .ok_or_else(|| CommandError::InvalidValue("not an audio clip".into()))?;
                    Ok(std::mem::replace(&mut a.warp, warp.clone()))
                })?;
                Ok(SetClipWarp {
                    track,
                    clip,
                    warp: old,
                    op,
                })
            }
            SetClipAutomation {
                track,
                clip,
                automation,
            } => {
                if automation.as_ref().is_some_and(|a| !a.is_valid()) {
                    return Err(CommandError::InvalidValue("automation curve".into()));
                }
                let old = edit_shared(project, track, clip, |c| {
                    Ok(std::mem::replace(&mut c.automation, automation.clone()))
                })?;
                Ok(SetClipAutomation {
                    track,
                    clip,
                    automation: old,
                })
            }
            SetTrackAutomationOpen { id, open } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.automation_open, open);
                Ok(SetTrackAutomationOpen { id, open: old })
            }
            SetClipLanes { track, clip, lanes } => {
                if !crate::clip_lanes::lanes_valid(&lanes) {
                    return Err(CommandError::InvalidValue("controller lanes".into()));
                }
                let mut lanes = lanes;
                crate::clip_lanes::sort_lanes(&mut lanes);
                let old = edit_shared(project, track, clip, |c| {
                    if c.audio.is_some() || c.automation.is_some() {
                        return Err(CommandError::InvalidValue("not a note clip".into()));
                    }
                    Ok(std::mem::replace(&mut c.lanes, lanes.clone()))
                })?;
                Ok(SetClipLanes {
                    track,
                    clip,
                    lanes: old,
                })
            }
            SetTrackAutoMode { track, mode } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let old = std::mem::replace(&mut t.auto_mode, mode);
                Ok(SetTrackAutoMode { track, mode: old })
            }
            SetMasterAutoMode { mode } => {
                let old = std::mem::replace(&mut project.master.auto_mode, mode);
                Ok(SetMasterAutoMode { mode: old })
            }
            InsertModulator {
                host,
                index,
                modulator,
            } => crate::modulator::apply_insert(project, host, index, *modulator),
            RemoveModulator { id } => crate::modulator::apply_remove(project, id),
            SetTempoTrack { bpm, points } => {
                crate::tempo::edit::apply_set_tempo_track(project, bpm, points)
            }
            InsertTempoPoint { index, point } => {
                crate::tempo::edit::apply_insert_tempo_point(project, index, point)
            }
            RemoveTempoPoint { index } => {
                crate::tempo::edit::apply_remove_tempo_point(project, index)
            }
            SetTempoPoint { index, point } => {
                crate::tempo::edit::apply_set_tempo_point(project, index, point)
            }
            SetMetreTrack { signature, metres } => {
                crate::tempo::edit::apply_set_metre_track(project, signature, metres)
            }
            SetMetreAt { tick, signature } => {
                crate::tempo::edit::apply_set_metre_at(project, tick, signature)
            }
            RemoveMetreAt { tick } => crate::tempo::edit::apply_remove_metre_at(project, tick),
            MoveMetre { from, to } => crate::tempo::edit::apply_move_metre(project, from, to),
            AddMarker { marker } => crate::tempo::edit::apply_add_marker(project, marker),
            RemoveMarker { id } => crate::tempo::edit::apply_remove_marker(project, id),
            SetMarker { marker } => crate::tempo::edit::apply_set_marker(project, marker),
            InsertMapping { index, mapping } => {
                crate::controller::apply_insert(project, index, *mapping)
            }
            RemoveMapping { id } => crate::controller::apply_remove(project, id),
            SetMapping { mapping } => crate::controller::apply_set(project, *mapping),
            SetModulatorAmount { id, amount } => {
                if !amount.is_finite() || !(0.0..=1.0).contains(&amount) {
                    return Err(CommandError::InvalidValue("modulator amount".into()));
                }
                let (host, index, _) = project.modulator(id).ok_or_else(|| {
                    CommandError::InvalidValue(format!("modulator {id} not found"))
                })?;
                let list = project
                    .modulators_mut(host)
                    .ok_or_else(|| CommandError::InvalidValue("modulator host".into()))?;
                let old = std::mem::replace(&mut list[index].amount, amount);
                Ok(SetModulatorAmount { id, amount: old })
            }
            SetModulator { modulator } => crate::modulator::apply_set(project, *modulator),
            // Round 31 (ADR 0069).
            SetTrackNoteFx { track, chain } => crate::notefx_cmd::set_chain(project, track, chain),
            SetNoteFxParam {
                track,
                effect,
                key,
                value,
            } => crate::notefx_cmd::set_param(project, track, effect, key, value),
            SetProjectKey { key } => crate::notefx_cmd::set_key(project, key),
            SetClipGroove {
                track,
                clip,
                groove,
            } => crate::notefx_cmd::set_clip_groove(project, track, clip, groove),
            SetClipSkipNoteFx { track, clip, skip } => {
                crate::notefx_cmd::set_clip_skip(project, track, clip, skip)
            }
            InsertGroove { index, groove } => {
                crate::notefx_cmd::insert_groove(project, index, *groove)
            }
            RemoveGroove { id } => crate::notefx_cmd::remove_groove(project, id),
            SetGroove { groove } => crate::notefx_cmd::set_groove(project, *groove),
            SetCompSeam {
                track,
                left,
                right,
                at,
            } => crate::comp::apply_seam(project, track, left, right, at),
            SetTrackComps {
                track,
                comps,
                active,
            } => crate::comp::apply_set_comps(project, track, comps, active),
            SetClipLane { track, clip, lane } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.lane, lane);
                Ok(SetClipLane {
                    track,
                    clip,
                    lane: old,
                })
            }
            SetClipComp { track, clip, comp } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.comp, comp);
                Ok(SetClipComp {
                    track,
                    clip,
                    comp: old,
                })
            }
            SetClipLink { track, clip, link } => {
                let c = project
                    .track(track)
                    .ok_or(CommandError::TrackNotFound(track))?
                    .clip(clip)
                    .ok_or(CommandError::ClipNotFound(clip))?;
                if c.link != link {
                    let joining = Clip { link, ..c.clone() };
                    check_joins(project, &joining)?;
                }
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.link, link);
                Ok(SetClipLink {
                    track,
                    clip,
                    link: old,
                })
            }
            SetClipContent {
                track,
                clip,
                content,
            } => {
                for n in &content.notes {
                    validate_note(n)?;
                }
                // No check that an audio source is in the pool: like `InsertClip`, this
                // can restore a clip whose file is missing (it plays silence), and an
                // inverse must always apply.
                if content.automation.as_ref().is_some_and(|a| !a.is_valid()) {
                    return Err(CommandError::InvalidValue("automation curve".into()));
                }
                let old = edit_shared(project, track, clip, |c| {
                    Ok(content.as_ref().clone().swap_into(c))
                })?;
                Ok(SetClipContent {
                    track,
                    clip,
                    content: Box::new(old),
                })
            }
            SetTrackInput { id, input } => {
                if let Some(i) = &input {
                    if !i.trim_db.is_finite() {
                        return Err(CommandError::InvalidValue("input trim".into()));
                    }
                }
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.input, input);
                Ok(SetTrackInput { id, input: old })
            }
            SetTrackArmed { id, armed } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.armed, armed);
                Ok(SetTrackArmed { id, armed: old })
            }
            SetTrackTakesOpen { id, open } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.takes_open, open);
                Ok(SetTrackTakesOpen { id, open: old })
            }
            SetTrackPitchOpen { id, open } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.pitch_open, open);
                Ok(SetTrackPitchOpen { id, open: old })
            }
            SetRecordSettings { settings } => {
                if !settings.click_db.is_finite() {
                    return Err(CommandError::InvalidValue("click level".into()));
                }
                let old = std::mem::replace(&mut project.recording, settings);
                Ok(SetRecordSettings { settings: old })
            }
            SetClipColor { track, clip, color } => {
                let old = edit_shared(project, track, clip, |c| {
                    Ok(std::mem::replace(&mut c.color, color))
                })?;
                Ok(SetClipColor {
                    track,
                    clip,
                    color: old,
                })
            }
            MoveClipToTrack { from, to, clip } => {
                if from == to {
                    // Not an error: a drag that ends on the track it started on is a
                    // no-op, and its inverse is itself. It still has to name a clip that
                    // exists, or a batch would half-apply and report success.
                    let t = project
                        .track(from)
                        .ok_or(CommandError::TrackNotFound(from))?;
                    t.clip(clip).ok_or(CommandError::ClipNotFound(clip))?;
                    return Ok(MoveClipToTrack { from, to, clip });
                }
                if project.track(to).is_none() {
                    return Err(CommandError::TrackNotFound(to));
                }
                let src = project
                    .track_mut(from)
                    .ok_or(CommandError::TrackNotFound(from))?;
                let idx = src
                    .clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(CommandError::ClipNotFound(clip))?;
                let moved = src.clips.remove(idx);
                let dst = project
                    .track_mut(to)
                    .ok_or(CommandError::TrackNotFound(to))?;
                if dst.clip(clip).is_some() {
                    // Cannot happen with ids from `Project::alloc_id`, but a project file
                    // is a file: put the clip back rather than lose it.
                    let src = project.track_mut(from).expect("checked above");
                    src.clips.insert(idx, moved);
                    return Err(CommandError::DuplicateId(clip.0));
                }
                dst.clips.push(moved);
                dst.clips.sort_by_key(|c| (c.start, c.id));
                Ok(MoveClipToTrack {
                    from: to,
                    to: from,
                    clip,
                })
            }
            SetClipName { track, clip, name } => {
                let old = edit_shared(project, track, clip, |c| {
                    Ok(std::mem::replace(&mut c.name, name.clone()))
                })?;
                Ok(SetClipName {
                    track,
                    clip,
                    name: old,
                })
            }

            InsertNotes { track, clip, notes } => {
                for n in &notes {
                    validate_note(n)?;
                }
                edit_shared(project, track, clip, |c| {
                    for n in &notes {
                        if c.note(n.id).is_some() {
                            return Err(CommandError::DuplicateId(n.id.0));
                        }
                    }
                    c.notes.extend(notes.iter().cloned());
                    c.notes.sort_by_key(|n| (n.start, n.pitch));
                    Ok(())
                })?;
                let ids = notes.iter().map(|n| n.id).collect();
                Ok(RemoveNotes { track, clip, ids })
            }
            RemoveNotes { track, clip, ids } => {
                let removed = edit_shared(project, track, clip, |c| {
                    for id in &ids {
                        if c.note(*id).is_none() {
                            return Err(CommandError::NoteNotFound(*id));
                        }
                    }
                    let mut removed = Vec::with_capacity(ids.len());
                    c.notes.retain(|n| {
                        if ids.contains(&n.id) {
                            removed.push(n.clone());
                            false
                        } else {
                            true
                        }
                    });
                    Ok(removed)
                })?;
                Ok(InsertNotes {
                    track,
                    clip,
                    notes: removed,
                })
            }
            SetNotes { track, clip, notes } => {
                for n in &notes {
                    validate_note(n)?;
                }
                let old = edit_shared(project, track, clip, |c| {
                    let mut old = Vec::with_capacity(notes.len());
                    for n in &notes {
                        old.push(
                            c.note(n.id)
                                .cloned()
                                .ok_or(CommandError::NoteNotFound(n.id))?,
                        );
                    }
                    for n in &notes {
                        if let Some(slot) = c.note_mut(n.id) {
                            *slot = n.clone();
                        }
                    }
                    c.notes.sort_by_key(|n| (n.start, n.pitch));
                    Ok(old)
                })?;
                Ok(SetNotes {
                    track,
                    clip,
                    notes: old,
                })
            }

            Batch {
                label_key,
                commands,
            } => {
                let mut inverses = Vec::with_capacity(commands.len());
                for cmd in commands {
                    match cmd.apply(project) {
                        Ok(inv) => inverses.push(inv),
                        Err(e) => {
                            // Roll back what was applied so far.
                            while let Some(inv) = inverses.pop() {
                                let _ = inv.apply(project);
                            }
                            return Err(e);
                        }
                    }
                }
                inverses.reverse();
                Ok(Batch {
                    label_key,
                    commands: inverses,
                })
            }
        }
    }
}

fn clamp_gain(db: f32) -> Result<f32, CommandError> {
    if !db.is_finite() {
        return Err(CommandError::InvalidValue("gain".into()));
    }
    Ok(db.clamp(MIN_GAIN_DB, MAX_GAIN_DB))
}

fn clamp_pan(pan: f32) -> Result<f32, CommandError> {
    if !pan.is_finite() {
        return Err(CommandError::InvalidValue("pan".into()));
    }
    Ok(pan.clamp(-1.0, 1.0))
}

fn clamp_width(width: f32) -> Result<f32, CommandError> {
    if !width.is_finite() {
        return Err(CommandError::InvalidValue("width".into()));
    }
    Ok(width.clamp(0.0, MAX_WIDTH))
}

/// The insert chain a host id names — a strip's, or the master's (ADR 0026).
fn chain_mut(project: &mut Project, strip: StripId) -> Result<&mut Vec<EffectSlot>, CommandError> {
    project
        .effects_of_mut(strip)
        .ok_or(CommandError::StripNotFound(strip))
}

/// The routes a whole new slot brings (`ReplaceEffect`): its key and its note source,
/// checked as `SetEffectSidechain` and `SetEffectMidiSource` check them.
fn check_effect_routes(
    project: &Project,
    strip: StripId,
    slot: &EffectSlot,
) -> Result<(), CommandError> {
    if let Some(src) = slot.sidechain {
        if !slot.spec.takes_sidechain() {
            return Err(CommandError::InvalidValue("sidechain".into()));
        }
        if src.is_master() {
            return Err(CommandError::RoutingCycle(strip));
        }
        if project.strip(src).is_none() {
            return Err(CommandError::StripNotFound(src));
        }
        if src == strip || project.routing_would_cycle(src, Some(strip)) {
            return Err(CommandError::RoutingCycle(strip));
        }
    }
    if let Some(src) = slot.midi_from {
        if !slot.spec.kind.takes_notes() {
            return Err(CommandError::InvalidValue("midi source".into()));
        }
        if project.track(src).is_none() {
            return Err(CommandError::TrackNotFound(src));
        }
    }
    Ok(())
}

/// One insert of a strip or of the master.
fn slot_mut(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
) -> Result<&mut EffectSlot, CommandError> {
    chain_mut(project, strip)?
        .iter_mut()
        .find(|e| e.id == effect)
        .ok_or(CommandError::EffectNotFound(effect))
}

/// The hosted plug-in in a slot, for the commands that only mean something there.
fn external_mut(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
) -> Result<&mut crate::external::ExternalPlugin, CommandError> {
    slot_mut(project, strip, effect)?
        .spec
        .external
        .as_deref_mut()
        .ok_or_else(|| CommandError::InvalidValue("not a hosted plug-in".into()))
}

/// A track's hosted instrument, for the commands that only mean something there.
fn instrument_plugin_mut(
    project: &mut Project,
    track: TrackId,
) -> Result<&mut crate::external::ExternalPlugin, CommandError> {
    project
        .track_mut(track)
        .ok_or(CommandError::TrackNotFound(track))?
        .instrument
        .external_mut()
        .ok_or_else(|| CommandError::InvalidValue("not a hosted instrument".into()))
}

/// Run `edit` on clip `clip` of `track`, then on every other member of its link group
/// (ADR 0037), and return what it returned on the named clip. The members share their
/// content, so the edit does to each exactly what it did to the first; a member it refuses
/// (only possible in a project that broke the invariant) takes the named clip's content
/// instead, so the group comes out equal either way. A clip in no group costs nothing more.
pub(crate) fn edit_shared<R>(
    project: &mut Project,
    track: TrackId,
    clip: ClipId,
    mut edit: impl FnMut(&mut Clip) -> Result<R, CommandError>,
) -> Result<R, CommandError> {
    let ti = project
        .track_index(track)
        .ok_or(CommandError::TrackNotFound(track))?;
    let c = project.tracks[ti]
        .clip_mut(clip)
        .ok_or(CommandError::ClipNotFound(clip))?;
    let out = edit(c)?;
    let Some(link) = c.link else {
        return Ok(out);
    };
    let others: Vec<(usize, usize)> = project
        .tracks
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            t.clips
                .iter()
                .enumerate()
                .filter(move |(_, c)| c.link == Some(link) && !(i == ti && c.id == clip))
                .map(move |(j, _)| (i, j))
        })
        .collect();
    let mut source: Option<Clip> = None;
    for (i, j) in others {
        if edit(&mut project.tracks[i].clips[j]).is_err() {
            let src = source.get_or_insert_with(|| {
                project.tracks[ti]
                    .clip(clip)
                    .cloned()
                    .expect("edited above")
            });
            crate::links::adopt(&mut project.tracks[i].clips[j], src);
        }
    }
    Ok(out)
}

/// Refuse a clip that names a link group whose members it does not match (ADR 0037).
fn check_joins(project: &Project, c: &Clip) -> Result<(), CommandError> {
    let Some(link) = c.link else {
        return Ok(());
    };
    match crate::links::first_other(project, link, c.id) {
        Some(other) if !crate::links::same_content(other, c) => Err(CommandError::InvalidValue(
            format!("clip {} does not match link group {link}", c.id),
        )),
        _ => Ok(()),
    }
}

fn validate_clip(c: &Clip) -> Result<(), CommandError> {
    if c.start.is_negative() || c.length <= Ticks::ZERO {
        return Err(CommandError::InvalidValue("clip bounds".into()));
    }
    for n in &c.notes {
        validate_note(n)?;
    }
    if c.automation.as_ref().is_some_and(|a| !a.is_valid()) {
        return Err(CommandError::InvalidValue("automation curve".into()));
    }
    // A comp seam's crossfade (ADR 0058): never negative, never longer than the most.
    if c.audio
        .as_ref()
        .is_some_and(|a| a.xfade < Ticks::ZERO || a.xfade > crate::audio::MAX_XFADE)
    {
        return Err(CommandError::InvalidValue("seam crossfade".into()));
    }
    // A warp only as valid as `SetClipWarp` takes it (ADR 0061).
    if c.audio
        .as_ref()
        .and_then(|a| a.warp.as_ref())
        .is_some_and(|w| !w.is_valid())
    {
        return Err(CommandError::InvalidValue("clip warp".into()));
    }
    // A gain envelope in order and in range (ADR 0070).
    if c.audio
        .as_ref()
        .and_then(|a| a.gain_env.as_ref())
        .is_some_and(|e| !e.is_valid())
    {
        return Err(CommandError::InvalidValue("clip gain envelope".into()));
    }
    Ok(())
}

fn validate_note(n: &Note) -> Result<(), CommandError> {
    if n.length <= Ticks::ZERO {
        return Err(CommandError::InvalidValue(format!("note {} length", n.id)));
    }
    if n.pitch > 127 {
        return Err(CommandError::InvalidValue(format!("note {} pitch", n.id)));
    }
    if n.velocity == 0 || n.velocity > 127 {
        return Err(CommandError::InvalidValue(format!(
            "note {} velocity",
            n.id
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with_track() -> (Project, TrackId, ClipId) {
        let mut p = Project::default();
        let track = p.make_track("Lead");
        let tid = track.id;
        Command::InsertTrack {
            index: 0,
            track: Box::new(track),
        }
        .apply(&mut p)
        .unwrap();
        let clip = p.make_clip("Clip", Ticks::ZERO, Ticks::beats(4));
        let cid = clip.id;
        Command::InsertClip {
            track: tid,
            clip: Box::new(clip),
        }
        .apply(&mut p)
        .unwrap();
        (p, tid, cid)
    }

    #[test]
    fn inverse_restores_state() {
        let (mut p, tid, cid) = project_with_track();
        let before = p.clone();
        let note = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
        let before_alloc = p.clone();
        let inv = Command::InsertNotes {
            track: tid,
            clip: cid,
            notes: vec![note],
        }
        .apply(&mut p)
        .unwrap();
        assert_ne!(p, before_alloc);
        inv.apply(&mut p).unwrap();
        assert_eq!(p, before_alloc);
        // Allocating an id is not a command and is not undone — by design.
        assert_ne!(p, before);
    }

    #[test]
    fn batch_rolls_back_on_error() {
        let (mut p, tid, cid) = project_with_track();
        let before = p.clone();
        let result = Command::Batch {
            label_key: "test".into(),
            commands: vec![
                Command::SetTempo { bpm: 90.0 },
                Command::RemoveNotes {
                    track: tid,
                    clip: cid,
                    ids: vec![NoteId(999)],
                },
            ],
        }
        .apply(&mut p);
        assert!(matches!(result, Err(CommandError::NoteNotFound(_))));
        assert_eq!(p, before);
    }

    #[test]
    fn remove_track_inverse_keeps_position() {
        let mut p = Project::default();
        for name in ["a", "b", "c"] {
            let t = p.make_track(name);
            let idx = p.tracks.len();
            Command::InsertTrack {
                index: idx,
                track: Box::new(t),
            }
            .apply(&mut p)
            .unwrap();
        }
        let before = p.clone();
        let id = p.tracks[1].id;
        let inv = Command::RemoveTrack { id }.apply(&mut p).unwrap();
        assert_eq!(p.tracks.len(), 2);
        inv.apply(&mut p).unwrap();
        assert_eq!(p, before);
    }

    #[test]
    fn rejects_bad_values() {
        let (mut p, tid, _) = project_with_track();
        assert!(Command::SetTrackGain {
            id: tid,
            gain_db: f32::NAN
        }
        .apply(&mut p)
        .is_err());
        assert!(Command::SetTrackGain {
            id: TrackId(4242),
            gain_db: 0.0
        }
        .apply(&mut p)
        .is_err());
        assert_eq!(
            Command::SetTempo { bpm: 5000.0 }
                .apply(&mut p)
                .ok()
                .map(|_| p.tempo_bpm),
            Some(999.0)
        );
    }

    /// Notes into an effect (ADR 0022): set and undone exactly; refused for a missing
    /// track, a missing effect, and a kind that takes no notes — and a refusal changes
    /// nothing.
    #[test]
    fn an_effect_hears_a_tracks_notes_and_undo_takes_it_back() {
        let mut p = Project::new("midi fx");
        let track = p.make_track("Drums");
        let tid = track.id;
        Command::InsertTrack {
            index: 0,
            track: Box::new(track),
        }
        .apply(&mut p)
        .unwrap();
        let strip = p.strips[0].id;
        let gate = p.make_effect(crate::effect::EffectKind::Gate);
        let eq = p.make_effect(crate::effect::EffectKind::Eq);
        let (gid, eid) = (gate.id, eq.id);
        p.strip_mut(strip).unwrap().effects.extend([gate, eq]);
        let before = p.clone();
        let inv = Command::SetEffectMidiSource {
            strip,
            effect: gid,
            source: Some(tid),
        }
        .apply(&mut p)
        .unwrap();
        assert_eq!(
            p.strip(strip).unwrap().effect(gid).unwrap().midi_from,
            Some(tid)
        );
        inv.apply(&mut p).unwrap();
        assert_eq!(p, before, "undo restores the project exactly");
        for (effect, source) in [
            (gid, Some(TrackId(4242))),
            (eid, Some(tid)),
            (EffectId(4242), None),
        ] {
            assert!(Command::SetEffectMidiSource {
                strip,
                effect,
                source
            }
            .apply(&mut p)
            .is_err());
            assert_eq!(p, before, "a refused command left something behind");
        }
    }
}
