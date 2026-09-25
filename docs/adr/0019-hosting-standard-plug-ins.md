# ADR 0019 — Hosting plug-ins made to the market's standards

**Status:** Accepted · **Date:** 2026-09-24 · **Extends:** ADR 0005 (the audio thread), ADR 0018 (delay compensation and the plug-in contract)

## Context

The user's direction for round 17: keep Cantus's own plug-ins native, and let people use
the plug-ins they already own — *follow the standard*. A hosted plug-in has to feel as
native as Aequalis or Comprimo: delay compensation, a sidechain from any strip, undo,
saving with the project, a project that opens on a machine where the plug-in is missing.
CLAP comes first (MIT, a clean C ABI, a threading model written down); VST3 is built by
another agent in parallel against the same contract (`crates/cantus-host/src/vst3/`).

The contract was written before the round (`crates/cantus-host/src/backend.rs`): a
`Backend` per format (search paths, candidates, `scan`, `load`) and a plug-in split by
thread into `MainThread` and `AudioThread`, destroyed on the main thread once both halves
are dropped.

## Decisions

### Crates and licences

**`cantus-host` stays GPL-3.0-or-later and depends on the engine, never the other way.**
ADR 0001 made the engine MIT OR Apache-2.0 and said the dependency direction is the
licence direction. So the engine learns no format and no host type: it gained a handful of
generic hooks (below), and the adapter from a hosted plug-in to the engine's `FxUnit` lives
in `cantus-host`. The reason ADR 0001 gave for GPL here — Steinberg's GPLv3 grant for the
VST3 SDK — no longer holds: the VST3 SDK has been MIT since 3.8, and CLAP is MIT. The host
crate *could* be permissive (MIT OR Apache-2.0), which would let other hosts and headless
renderers embed it. That is **a decision for the user**, not taken here.

**CLAP bindings: `clap-sys` 0.5** (CLAP 1.2.2, MIT OR Apache-2.0), used by both the host
and the test plug-in. It is the ABI translated from the official headers with nothing on
top, used by nih-plug and clack, so it has been exercised against the plug-ins people use.
`clack` 0.2 (a safe host and plug-in layer on top of `clap-sys`) was the alternative: it
encodes the threading in types, but what matters here — which thread a call happens on,
what the audio thread may touch, how an instance is torn down — has to match ADR 0005 and
ADR 0018 exactly, and is written once in `clap/`. `libloading` (ISC) opens the library;
`windows-sys` (MIT OR Apache-2.0) makes the plug-in window.

### The CLAP backend (`cantus_host::clap`)

- **One `Library` per file**, shared by every instance from it and kept in a table of weak
  references, so `clap_entry.init` and `deinit` run once per load, as the entry header asks.
- **The host side** (`clap/host.rs`): `request_callback`, `request_restart` and
  `request_process`, and the host extensions `thread-check`, `params` (rescan, clear,
  request-flush), `state` (mark-dirty), `latency` (changed), `audio-ports`, `note-ports`,
  `gui` and `log`. Every callback only sets a flag or stores a number, so each is lock- and
  allocation-free wherever the plug-in calls it from; `MainThread::idle` reads them.
- **Thread checks** use a thread-local role, set by the thread that loads plug-ins (the
  main thread) and raised for the length of every audio-half call. A plug-in asking
  `is_audio_thread` during `process` gets yes even when an offline render runs on the thread
  that loaded it.
- **Lifecycle, exactly as `plugin.h` has it.** `create`, `init`, `activate` on the main
  thread in `Backend::load`; `start_processing` lazily, on the audio thread, in the first
  `process`; `stop_processing` in `AudioThread::stop`, which the engine calls on the audio
  thread before it hands a unit back (`Processor::dispose_effect`); `deactivate` and
  `destroy` when the second half is dropped, on the main thread — or, if that drop happens
  elsewhere, by the next main-thread call (`clap::collect_garbage`).
- **Who may call the processing side** is an atomic owner (`none`, `audio`, `main`). The
  audio half takes it at `start_processing`; the main half takes it for `params.flush`
  while nothing processes (no audio device, or before the first block), so a flush and a
  `process` can never overlap. If the main thread holds it at that instant, the block is a
  wire and the next one tries again.
- **Process.** The strip's pair is copied into the main input port and the key into the
  sidechain port (silence when no key is routed; a mono port gets the mid); the main output
  is copied back. Buffers for every port the plug-in declares are allocated when the audio
  half is made. Parameter changes and notes go in one fixed-capacity event list, merged by
  time; parameter gestures and values come out through the output list into a `ParamOut`.
  A transport event is built from the engine's song position. A block longer than the
  activation's `max_frames` goes in pieces; `CLAP_PROCESS_ERROR` leaves the block untouched.
- **The editor**: `win32` embedded as the child of a window Cantus makes; floating as the
  fallback, when a plug-in cannot be embedded.

### Contract additions (`backend.rs`; every one has a default, so a backend that does not
implement it still builds)

- `MainThread::restart()` — once the audio half is dropped, deactivate and activate the
  same instance and return a new audio half; `Ok(None)` while the old half is on its way
  back. The default is unsupported, and the host then restarts by loading a new instance and
  moving the state across.
- `MainThread::flush(params, out)` — deliver changes on the main thread while nothing
  processes; `false` while processing (they travel with the next block).
- `MainThread::{editor_is_floating, resize_editor, editor_can_resize}`.
- `TransportInfo` and `Audio::transport` (identical text in the VST3 branch).

### Scanning, in a separate process, with a cache

`cantus-scan <format> <file>` (a binary of `cantus-host`) runs one backend's `scan` on one
file and prints `CANTUS-SCAN-RESULT` and JSON as its last line; whatever the plug-in prints
before it is ignored, and Windows' error dialog is switched off in it (`SetErrorMode`). The
host runs it per file with a time limit (30 s; the tests use 15). **An exit that is not
clean, or no answer in time, blocks the file**: it is never loaded, and an ordinary scan
does not retry it, until the user asks (*Try again* in Settings, or *Rescan everything*).
A file that answers "not a plug-in" is *failed*, retried when it changes.

The database (`<data folder>/plugins.json`) is keyed by path, modification time and size;
an unchanged file is a cache read and starts no process. After a plug-in's first load,
what it says it is (`MainThread::info`) replaces what the scan found (`PluginDb::learn`):
a VST3 scan that read `moduleinfo.json` without loading the binary only guesses buses and
editor. Formats are registered in one place, `cantus_host::backend_for` / `backends()`;
VST3 is one line there. Folders: the CLAP standard ones —
`%COMMONPROGRAMFILES%\CLAP`, `%LOCALAPPDATA%\Programs\Common\CLAP`, then `CLAP_PATH` — plus
the user's. The scan runs on a background thread and its result lands in the next frame.

### The model and schema 14

`EffectKind::External`, with an `external: Option<Box<ExternalPlugin>>` on `EffectSpec`
(not written for native effects, so their JSON is unchanged): format, id, name and vendor
(what to show when it is missing), whether it has a sidechain input, the state (base64 of
an `Arc<Vec<u8>>`, so compiling a snapshot does not copy a sampler's megabytes) and the
**parameter values by the plug-in's ids, as `f64` in its own range**. Two commands:
`SetPluginParam { id, value: Option<f64> }` (`None` forgets a value, which is the exact
inverse of setting one the project did not hold) and `SetPluginState { state, params }`.
Both are in the undo fuzz. Storing values under string keys `p{id}` in the existing
`params` map was considered and rejected: that map is `f32`, clamped by a static parameter
table the kind does not have, and a value the plug-in reported would come back one
rounding step away — which the echo guard below would see as a change.
`serde_json` gains `float_roundtrip`, so an `f64` survives save → open to the bit.

`EffectSpec::takes_sidechain()` answers per plug-in (a second audio input), so
`SetEffectSidechain` accepts a key for a plug-in that has one.

The state in the project is **what the plug-in said when the project was last saved**
(`PluginHost::project_for_save` asks every loaded plug-in; autosave too). Parameter values
are kept live by commands. A state change that moved no value (a sample loaded inside the
plug-in) is therefore saved but not an undo step. A missing plug-in's record is written
back untouched.

### Engine glue (small, generic, and the only engine edits)

- `FxUnit` gained default methods `set_position`, `param_events`/`clear_param_events` and
  `stop`; `fx::Wire` passes audio through and is what an `External` slot gets when no host
  delivered a unit — so **a missing plug-in is a wire** in the engine, the offline render and
  every test, with nothing to configure.
- `EngineMessage::ReplaceEffect` swaps a unit in for the one with its id (keeping its place,
  bypass, mix and key), stops and returns the old one flagged `replaced` (so the handle keeps
  the id known), and **re-plans delay compensation**.
- `EngineEvent::FxParam { effect, event }`: what a unit's own editor did, pushed after each
  `process`; `EngineHandle::take_fx_params` hands them to the UI.
- `EngineHandle::{add_unit, replace_unit, knows_effect, manual}` (`manual` gives a handle and
  the processor behind it, for tests that drive the audio thread themselves);
  `render_with_units` for a bounce with hosted instances.

### The hosted unit (`cantus_host::unit::HostedUnit`)

An `FxUnit` around an `AudioThread`. **Latency** is the value read on the main thread when
the unit was made (CLAP only allows `latency.get` there); a change arrives as a restart, and
the new unit, through `ReplaceEffect`, carries the new value into compensation. **Values**:
each snapshot's are compared, on the audio thread and without allocating, with a fixed table
of what the plug-in is known to hold; a difference becomes a `ParamChange` at the next block.
**No echo**: a value the plug-in reports goes into the table, so a snapshot carrying it back
sends nothing; during the plug-in's own gesture on a parameter the (lagging) snapshots do not
touch it, and after the gesture they wait until one carries the plug-in's final value, or for
half a second. The host bumps a generation after loading a state, and the table forgets
everything it knew.

The processor calls inserts once per note-event segment, sometimes for one sample. That is
legal for both formats (any size up to `max_frames`) and the hosted unit is correct at any
size (the tests hold 1 … 4096), but it multiplies the per-call cost of third-party code. The
processor's loop is not restructured here (the instruments session is editing it); a
whole-block strip stage is planned after both merges.

### The session (`cantus_host::PluginHost`)

Held by the app. Before each snapshot, `sync` loads every hosted slot without an instance
(from the database by format and id), loads the project's state and flushes its values —
and if that made the plug-in ask for a restart (a state with another latency), restarts it
there and then, so its first block already runs at the right latency — delivers the audio
half (`add_unit`) — the engine then leaves that id alone — unloads what
left the project (keeping its state for an undo that brings the slot back), and loads a
state the project changed (undo, a preset, A/B). Each frame, `idle` serves the plug-ins'
requests (the frame loop is the main-thread pump; it repaints every 33 ms while plug-ins are
loaded), restarts the ones that asked (a wire stands in; `restart()` or a new instance; the
new unit replaces it), and records what their windows did: **a gesture's values merge into
one undo step** (`MergeKey` per effect and parameter, sealed at the gesture's end); a value
outside a gesture is a step of its own; a value equal to the project's is none. Values the
plug-in changed by itself (its own preset browser) become one `SetPluginState`.

**Offline render**: fresh instances loaded on the main thread with the live ones' state,
processed on the render thread, dropped on the main thread when the render reports back.
The CLAP `render` extension (offline mode) is not used yet.

### The interface

The insert menu lists the scanned effects by maker or by kind (the first CLAP feature that
is not plumbing) with a search field; the slot shows the plug-in's name, a format badge, and
*missing* in the danger colour with the reason. The generic editor sits in the shared FX
frame: the plug-in's parameters grouped by its module path, a house knob each with the
plug-in's own value text, a filter, the latency, the key choice when it has a sidechain
input, and **Open plug-in window**. Host A/B and presets work on whole states (`{state,
params}` as JSON per plug-in id under the data folder). Settings has the manager: standard
and user folders, scan and rescan, the blocked list with the reason and *Try again*.

**The plug-in window** (Windows): a top-level window owned by the main window, the editor
embedded as its child, sized to the editor, resized when the editor asks, and — when the
editor can resize — offering the user's drag to it. winit's message loop dispatches its
messages. macOS and Linux embedding is not built (`Unsupported`).

## VST3

The VST3 backend (`cantus_host::vst3`, behind the `vst3` cargo feature, on by default) was
built in parallel against the same contract and registered next to CLAP at merge
(`backend_for`). Its full design notes are `crates/cantus-host/src/vst3/README.md`; the
decisions:

- **Bindings: the `vst3` crate 0.3** (coupler-rs, MIT OR Apache-2.0), generated from the
  VST 3.8.0 headers, which Steinberg released under MIT in October 2025. Not `vst3-sys`
  (GPL-3, unmaintained since 2023), not hand-written vtables.
- **Discovery.** `.vst3` bundles (`Contents/x86_64-win/…`) and legacy single files, in the
  standard folders; `moduleinfo.json` is read when present so a scan need not load the
  binary (it then only *guesses* buses and editor, so the host writes `MainThread::info()`
  back to the database after the first load). One shared load per module, `InitDll` /
  `ExitDll` paired.
- **Instantiation in the SDK's own order**: single-component and separate-controller
  plug-ins, connected through `IConnectionPoint` with host-made `IMessage`s; stereo main in
  and out plus a stereo aux sidechain, the accepted arrangement read back; realtime,
  32-bit, the engine's largest block.
- **The audio thread allocates nothing**: parameter and event queues are built at load and
  refilled in place (a counting allocator proves it). VST3 speaks normalized values and
  only the controller converts, so the main thread builds a conversion map (linear, stepped
  or a 257-point table) the audio thread reads through a lock-free mailbox.
- **Gestures.** In VST3 the host carries an editor's change to the processor: the
  controller's `beginEdit`/`performEdit`/`endEdit` go through a lock-free ring to the audio
  thread, which applies them at the next block and reports them in `ParamOut` — the same
  channel CLAP uses, so the host core records both formats' gestures one way.
- **Restarts** (`restartComponent`) become `Notice`s; latency is cached in an atomic and
  never read from the plug-in on the audio thread.
- **State is a `.vstpreset` in memory** (`Comp` + `Cont` chunks), so the project keeps the
  bytes the plug-in wrote and the same blob opens as a preset in any VST3 host.
- **Tests** (`tests/vst3_backend.rs`, 16, plus 13 unit tests) against a VST3 plug-in built
  from source in this repository (`crates/cantus-test-plugins/vst3`): bit-exact at 0 dB,
  sample-accurate parameter changes, latency 256 reported and delivered, blocks 1…4096
  identical, zero host-side allocation in `process`, a controller edit as a gesture, state
  byte for byte, 100 loads with no live objects left.
- **Left:** `restart()` in the contract (a reload serves for now), tail / bypass parameter
  / programs / MIDI-CC mapping, HiDPI and host-initiated resize, macOS, multi-out buses.

### The round trip: our own plug-ins, hosted

`crates/cantus-plugins/tests/hosted_in_cantus.rs` builds our seven effects as a CLAP file
(ADR 0020), scans and loads it with this host exactly as a third party's plug-in, gives
each the state a project would, and compares it with the native insert the engine runs:
**all seven are bit-identical**, and report the native latency. The test found one real
defect on the way — a state loaded after activation (what this host, and many others, do)
made the plug-in glide from its defaults, a pre-delay sweep at the top of the song; a
state that arrives before the first sample is now where the effect starts.

## What the tests hold (`crates/cantus-host/tests/`, against `cantus-test-clap`)

The test plug-in (`crates/cantus-test-plugins/clap-gain`) is built from source by the tests
(`cargo build` into `target/test-plugins`): gain, a latency that takes a restart, a key
switch that adds the sidechain input, a hidden parameter that makes it perform a gesture on
Gain, a state with a tag that is not a parameter, and — by file name — a crash or a hang in
`clap_entry.init`.

- The scan finds it (name, vendor, 2 in / 2 out, sidechain, features); a second scan is a
  cache read; the database survives on disk; a touched file is read again.
- A crashing and a hanging copy are **blocked** (crashed / did not answer in 15 s), the
  good copy is still found, an ordinary rescan does not retry them, *Try again* reads one
  again, and this process survives.
- Through the whole engine, the plug-in at 0 dB is **bit-exact** against no plug-in.
- A project parameter change reaches it: the output is the input times the gain, to the bit;
  undo brings the plug-in back.
- With a latency of 100 samples (set as a parameter, so through a restart), an inverted twin
  on another strip **cancels to exact silence**; the engine reports 100 samples.
- A gesture from the plug-in is **one undo step**, nothing echoes over 40 more blocks, and
  undo takes the plug-in back too.
- State round-trips through save → open (the non-parameter tag included) and the values are
  restored.
- A missing plug-in is a wire, bit-exact, and its record survives save → open unchanged.
- Block sizes 1, 7, 64, 1000 and 4096 give identical output (gain −3 dB, latency 37).
- The sidechain reaches the second input: keyed from the riff's strip, the master carries
  the riff plus the key through the keyed strip's pan law, to the bit.
- An offline render with fresh instances (`render_units` + `render_with_units`, gain −6 dB,
  latency 64 set through the state) equals playing the project, to the bit.
- The plug-in window (Windows): the plug-in's view is a child of a top-level window whose
  client area is the editor's 320 × 200; a second open only brings it forward; closing it
  destroys the window and the plug-in carries on.
- In the app (`cantus-ui/tests/hosted_plugins.rs`): a missing plug-in shows its name, the
  CLAP badge and *missing* in the inspector, the editor says it is not on this computer, and
  saving writes its record back unchanged; the generic editor paints the test plug-in's
  parameters with its own value text ("-4.5 dB"), hides the hidden one, offers the window,
  and a knob gesture is one undo step.
- Schema 14: `fixtures/v14.cantus` holds a hosted plug-in installed nowhere; it opens, and
  its state and values (0.1 + 0.2 to the bit) are intact after a round trip.

## Consequences

- New: `cantus_host::{clap, scan, db, unit, session, window}`, the `cantus-scan` binary,
  `cantus-test-clap`, `cantus_core::external`, `EffectKind::External`,
  `Command::{SetPluginParam, SetPluginState}`, schema 14, the engine hooks above,
  `views::plugins`.
- Not built: crash isolation while *running* (a plug-in that crashes in `process` still
  takes Cantus down; the scanner protects only the scan), the plug-in window on macOS and
  Linux, per-plug-in DPI modes, CLAP's `render`, `tail`, `timer-support`, `posix-fd`,
  `remote-controls`, preset discovery, the draft `undo` extension, hosted instruments (the
  instruments session builds them on this crate), a bypass parameter driving the slot's
  bypass, whole-block processing for hosted units, and a clap-validator run (it did not fit
  the round's build budget).
