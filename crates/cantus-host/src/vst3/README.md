# The VST3 backend — design notes

For ADR 0019 (hosting plug-ins made to the market's standards). The code is in this
folder; the tests are `crates/cantus-host/tests/vst3_backend.rs` and run against
`crates/cantus-test-plugins/vst3`, a VST3 plug-in built from source in this repository.
That crate is a `cdylib` + `rlib` and a dev-dependency of `cantus-host`, so the cargo that
builds the tests builds the plug-in's DLL next to them (it is never linked); the tests copy
it into a `.vst3` bundle under `target/cantus-test-plugins/bundles`. A nested
`cargo build` was tried first and dropped: on a machine running several cargos it waited
for the global package-cache lock for up to an hour.

## 1. Bindings: the `vst3` crate

Three candidates were evaluated.

| | Licence | State | Verdict |
|---|---|---|---|
| **`vst3` 0.3.0** (coupler-rs, crates.io) | MIT OR Apache-2.0; its one dependency `com-scrape-types` 0.1.1 likewise | bindings pregenerated from `vst3_pluginterfaces` **3.8.0** (`SDKVersionString = "VST 3.8.0"` in the generated file), identical on Windows, macOS and Linux, no SDK needed to build | **chosen** |
| `vst3-sys` (RustAudio, git only) | GPL-3.0 ("a derivative work of the original SDK", from when the SDK's open licence was GPLv3) | not on crates.io, unmaintained since 2023; would need a pinned git `rev` | compatible with our GPL, but older headers, no maintenance, and no reason left to prefer it |
| Own bindings | ours | ~25 interfaces of hand-written vtables | error-prone (ABI, inheritance order, `extern "system"`), no gain |

**The SDK's licence.** Steinberg released VST 3.8.0 (October 2025) under the **MIT**
licence, and its README says "Licensing under GPLv3 and the Steinberg proprietary license
is no longer available". The headers `vst3` 0.3 is generated from are those 3.8.0
headers, so the bindings are MIT/Apache through and through. Both are compatible with
`cantus-host`'s GPL-3.0-or-later. Using the "VST" name and logo is optional under the MIT
SDK; the UI says "VST3" as a format name only.

What the crate gives beyond raw vtables: `ComPtr`/`ComRef` (reference counting done right
at the boundary) and `ComWrapper` + `Class`, which is how every host-side object here
(`IHostApplication`, `IComponentHandler`, `IBStream`, `IParameterChanges`, …) is written
in plain Rust.

New dependencies (all behind the `vst3` feature, on by default): `vst3` 0.3 (MIT OR
Apache-2.0), `libloading` 0.8 (ISC; already in the lock file), `rtrb` and `serde_json`
(workspace, MIT OR Apache-2.0).

## 2. Discovery, modules, scanning

- **Folders** (`search_paths`), per user first, as the SDK's "Plug-in Locations" orders
  them: `%LOCALAPPDATA%\Programs\Common\VST3`, then `%COMMONPROGRAMFILES%\VST3` (macOS and
  Linux folders listed too).
- **Candidates** (`is_candidate`): a `.vst3` folder (bundle) or a legacy single `.vst3`
  file — but not the binary *inside* a bundle (`X.vst3/Contents/x86_64-win/X.vst3`), so a
  walker that does not stop at bundles still finds each plug-in once.
- **Binary** (`module::binary_path`): `Contents/<arch>/<stem>.vst3` (`x86_64-win`;
  `arm64-win`/`arm64ec-win`/`arm64x-win`; `.so` in `x86_64-linux`), or the only binary in
  that folder.
- **Loading** (`module::open`): `LoadLibraryExW` with `LOAD_WITH_ALTERED_SEARCH_PATH` (a
  plug-in finds the DLLs it ships beside itself), `InitDll` when exported
  (`ModuleEntry` on Linux), `GetPluginFactory`, and `IPluginFactory3::setHostContext`.
  **One load per process**: a table of weak references by canonical path, so ten
  instances share one module; when the last instance or scan lets go, the factory is
  released, `ExitDll` is called, and the library is freed — in that order. macOS bundles
  (`CFBundle`) are not supported yet and say so (`HostError::Unsupported`).
- **`scan`** reads `Contents/Resources/moduleinfo.json` (or the pre-3.7.8
  `Contents/moduleinfo.json`) when the bundle has one, **without loading the binary**. The
  file is JSON5; comments and trailing commas are stripped (`moduleinfo::json5_to_json`)
  and `serde_json` reads the rest. It gives the classes, names, vendor, version and
  sub-categories; it does not give buses or the editor, so those are assumed (stereo,
  0 inputs for an instrument, an editor) and **made exact at load**:
  `MainThread::info()` returns the `PluginInfo` with the buses the plug-in actually
  configured. The host core should write that back into its database after a first load.
- Without the file, `scan` loads the module and enumerates the factory
  (`IPluginFactory3` Unicode infos → `IPluginFactory2` → `IPluginFactory`), keeps the
  "Audio Module Class" entries, and **instantiates each** (component + controller,
  initialized, connected, not activated) to read the buses — first `kMain` input and output
  → `main_inputs`/`main_outputs`, any `kAux` input → `sidechain` — and whether
  `createView("editor")` returns a view. `Vst3Backend::scan_loading` does this even when
  the file exists. The scan runs in the scanner process, so a plug-in that crashes there
  costs nothing.
- **Kind** from the sub-categories: `Instrument` → `Instrument`, `Fx` → `Effect`; with
  neither, an event input and no audio input → `Instrument`, an audio input → `Effect`.
  `NoteEffect` is never inferred: VST3 has no category for it.
- **Class ids** are printed as the SDK's `FUID::toString` does: 32 hex digits of the four
  32-bit words of `DECLARE_UID`. On Windows a `TUID`'s bytes are COM-ordered, elsewhere
  big-endian; the text is the same everywhere, so a project made on Windows finds the
  plug-in on Linux, and it matches `moduleinfo.json`'s `CID`.

## 3. Instantiation (`instance.rs`)

The SDK's own order (`plugprovider.cpp`, "Audio Processor Call Sequence"):

1. `createInstance(cid, IComponent)`, `initialize(host)`.
2. The controller: the component itself when it answers `IEditController` (a
   **single-component** plug-in; the test's Pulse is one), else `getControllerClassId` →
   `createInstance` → `initialize(host)`.
3. Separate objects only: each `IConnectionPoint::connect`ed to the other, directly (no
   proxy). Messages between them are `IMessage`/`IAttributeList` objects the host makes
   (`IHostApplication::createInstance`); the test's processor sends one on activation and
   the test checks it arrived.
4. `component.getState` → `controller.setComponentState`.
5. `setComponentHandler`.
6. `setBusArrangements`: stereo on the main input and output, stereo on the first aux
   input (the sidechain), the plug-in's own proposal on every other bus. What it accepted
   is **read back** (`getBusArrangement`), because a plug-in may answer with the nearest
   arrangement it supports. `activateBus` on main in, main out, the sidechain and the first
   event input. `canProcessSampleSize(kSample32)`, `setupProcessing` (realtime, 32-bit,
   the engine's largest block, the rate), `setActive(true)`, latency read and cached.

Teardown is the reverse, driven by `Drop`: the audio half calls `setProcessing(false)` when
it is dropped (if `stop()` has not already), then `setActive(false)`, the handler cleared,
disconnect, controller `terminate`, component `terminate`, every pointer released, and the
module last. Both halves hold an `Arc` of the instance; whichever is dropped last tears it
down, on the main thread as the contract says. The host context, handler and frame are our
own `ComWrapper`s, so a plug-in that keeps a reference too long keeps our object alive,
not a dangling pointer.

## 4. The audio thread (`audio_thread.rs`, `process_objects.rs`)

Everything is allocated at load: `IParameterChanges` in and out with 128 queues of 32
points each, `IEventList` in and out with 512 events, the `ProcessContext`, scratch
channels for every bus. On the audio thread they are cleared and refilled in place. A
counting global allocator in the tests proves `process`, `reset` and `stop` allocate and
free nothing on the host side.

Per block:

- a new parameter map is adopted if the main thread posted one (`params::Mailbox`: two
  atomic pointer slots; the audio thread swaps boxes, never allocates or frees);
- `setProcessing(true)` the first time (lazily, on the audio thread, as the SDK allows);
- the controller's edits waiting in the UI → audio ring go into the input queues at offset
  0, and their gestures into `ParamOut` (see §6);
- the host's `ParamChange`s, plain → normalized (§5), at their offsets;
- notes → `NoteOnEvent`/`NoteOffEvent`/`PolyPressureEvent`/`NoteExpressionValueEvent`
  (§7);
- `TransportInfo` → `ProcessContext`: tempo, musical position, bar start, time signature,
  cycle, playing/recording, each with its `state` bit; `kContTimeValid` always, with a
  running sample count;
- inputs: the main pair is **copied** to scratch, the outputs are written straight into
  the host's buffers (in place is legal in VST3 but not every plug-in survives it); the
  sidechain gets the key or silence (with its `silenceFlags` set); other buses silence. A
  mono main input gets the mid `(L+R)/2`; a mono main output is copied to both sides;
- `process`; then the processor's output parameter changes go to `ParamOut` as `Value`s
  (plain) and, like the host's own changes, to the controller through an audio → UI ring
  (drained by `idle`, which calls `setParamNormalized`): VST3 leaves it to the host to keep
  the controller in step with automation. The controller's own edits are not echoed back
  (that would fight a knob being dragged).

`reset()` is `setProcessing(false)`, and the next block turns it back on: VST3 has no reset
call, and a processing cycle is what hosts use to ask a plug-in to drop its tails (the
SDK allows `setProcessing(false)` then `(true)` at any time; whether a plug-in clears its
state then is up to it).
`stop()` is `setProcessing(false)`.

## 5. Plain ⇄ normalized without the controller (`params.rs`)

The contract carries plain values; VST3 carries normalized ones; only the controller
converts, and the controller is main-thread only. So the main thread asks it once per
parameter and builds a `ParamMap` the audio thread can use:

- **Linear** when 17 probes lie on the line `min + n·(max − min)` — exact both ways (0 dB
  on the test's ±48 dB gain is normalized 0.5 exactly, which is why 0 dB is bit-exact);
- **Steps** (≤ 4096 steps): the plug-in's own plain value at every step;
  normalized → plain is the SDK's `floor(n · (steps + 1))`, plain → normalized the nearest
  step;
- **Table** otherwise: 257 probes of a monotonic curve (log frequency, say), interpolated
  one way and bisected the other; a curve that is not monotonic falls back to linear.

`ParamInfo.min`/`max`/`default` are `normalizedParamToPlain(0 / 1 / default)`, `stepped`
is `stepCount > 0`, and the flags map `kCanAutomate`, `kIsHidden`, `kIsReadOnly`.
`group` is the `IUnitInfo` path ("Filter/Envelope"). `param_text` is
`getParamStringByValue` with the parameter's units appended ("-6.0 dB"). A new map is
built and posted when the plug-in restarts with `kParamTitlesChanged` or
`kParamIDMappingChanged`.

## 6. The controller's edits, restarts and the editor (`host_objects.rs`, `main_thread.rs`)

`IComponentHandler` receives `beginEdit`/`performEdit`/`endEdit` on the UI thread. In
VST3 it is **the host** that must pass an editor's change on to the processor, so each edit
goes into a lock-free ring to the audio thread, which applies the value at the start of its
next block and reports the gesture — `Begin`, `Value` (plain), `End`, in order — in that
block's `ParamOut`. That is the channel the contract already gives an editor's gestures
(`ParamEvent`, "what the plug-in's own editor did in a block"), and the one CLAP uses, so
**the host core records VST3 and CLAP gestures the same way and no contract change was
needed** for them. The cost: an edit made while the engine is not processing waits in the
ring (1024 edits) until the next block.

`restartComponent` flags and `IComponentHandler2::setDirty` become `Notice`s on the next
`idle()`:

| Flag | Notice |
|---|---|
| `kLatencyChanged` | `LatencyChanged(n)`, `n` read with `getLatencySamples` then cached |
| `kParamTitlesChanged`, `kParamIDMappingChanged` | `ParamsChanged` (and a new map for the audio thread) |
| `kParamValuesChanged`, `setDirty(true)` | `StateDirty` |
| `kIoChanged`, `kReloadComponent` | `RestartNeeded` |
| `IPlugFrame::resizeView` | `EditorResized(size)` |

A `load_state` that changes the latency also queues `LatencyChanged` (not every plug-in
says so itself).

**Latency** is cached in an atomic of the shared instance: read at activation, on
`kLatencyChanged` and after a state load. `MainThread::latency()` returns the cache and
never calls the plug-in, so it is cheap and the same number delay compensation will see.

**The editor**: `createView("editor")`, `isPlatformTypeSupported("HWND")`
(`"X11EmbedWindowID"` on Linux), `setFrame(our IPlugFrame)`, `attached(parent)`,
`getSize` → `EditorSize`. `resizeView` from the plug-in records the size for the notice
and calls the view's `onSize` at once, as the SDK's own host does; the host core resizes
its child window when it sees the notice. `close_editor` is `removed()` then
`setFrame(null)`.

## 7. Notes

`NoteKind::On`/`Off` → `NoteOnEvent`/`NoteOffEvent` with channel, key, velocity and
`noteId`. A note with `note_id = −1` gets `channel·128 + key`, so its expressions still find
it. `Choke` → a note-off with velocity 0 (VST3 has no choke). Expressions, normalized as
`ivstnoteexpression.h` defines them: tuning `0.5 + semitones/240`, volume `gain/4`
(0.25 = 0 dB), pan, brightness, vibrato and expression as given; pressure →
`PolyPressureEvent`. Events go only to a plug-in with an event input bus.

## 8. State: a `.vstpreset` in memory (`vstpreset.rs`)

The opaque blob is exactly the SDK's preset file (`vstpresetfile.h`): `'VST3'`, version 1,
the class id in 32 ASCII hex digits, the offset of the chunk list; then the chunks; then
`'List'`, a count, and per chunk its id, offset and size (all little-endian). Cantus writes
`Comp` (the component's `getState`) and `Cont` (the controller's), and reads any order,
ignoring chunks it does not know. So a project holds the bytes the plug-in wrote,
unchanged, and the same blob opens as a preset in any VST3 host (and is what a DAWproject
export would put in its zip).

Loading: the class id must match; `component.setState(Comp)`; for a separate controller
`setComponentState(Comp)`; `controller.setState(Cont)` when present.

## 9. What the tests hold (`tests/vst3_backend.rs`, 16 tests; 13 unit tests in the modules)

Against **Cantus Test Gain** (separate controller, ±48 dB gain applied sample-accurately,
0–4096 samples of latency, a sidechain bus, component and controller state, an editor) and
**Cantus Test Pulse** (single component, an instrument writing its events into the audio):

- the scan finds exactly the two audio classes with their complete `PluginInfo`; a bundle
  holding only `moduleinfo.json` is described without anything to load, and the same file
  beside a real binary gives a description that loading makes exact (the sidechain the file
  could not mention);
- the sidechain key reaches the aux bus, and a missing key arrives as silence with its
  silence flags set;
- 0 dB is **bit-exact** over 10 000 samples of noise;
- a gain change at offset 100 changes sample 100 and not 99;
- a latency set to 256 is reported (`LatencyChanged(256)` from the plug-in's own
  `restartComponent`, then `latency() == 256`) and the audio is exactly 256 samples late;
- output is **identical to the bit** for blocks of 1, 2, 3, 7, 64, 100, 1000, 4095 and 4096
  samples, with a latency and four gain changes at fixed song positions;
- the host side of `process` makes **0 allocations** (counting allocator);
- a `performEdit` in the plug-in's controller arrives as `Begin`, `Value(−24 dB)`, `End`
  and the processor applies it from the next block's first sample;
- the processor's `IMessage` reaches its controller;
- state round-trips **byte for byte**, including the controller-only part, and restores
  gain, latency and the controller's values; another class's blob is refused;
- 100 load/process/save/unload cycles leave **0 live plug-in objects** and every
  `InitDll` paired with `ExitDll`;
- notes, pan and pressure land on their exact samples, and the transport arrives with the
  right `ProcessContext` bits and values;
- the editor attaches to the given HWND, resizes through our frame (notice + `onSize`),
  and detaches.

## 10. What is left, and why

- **A deactivate/activate cycle.** `RestartNeeded` (for `kIoChanged`, `kReloadComponent`)
  has no contract method; the host core can reload (save state, drop, load, load state).
  A `restart()` on `MainThread`, with the audio half stopped and handed back first, would
  be cheaper — and is how an offline render should switch to `kOffline`.
- **Latency on the audio thread.** The cache is an atomic both halves share; if the host
  core wants to read it from the audio thread, `AudioThread::latency()` would be a
  one-line addition returning it. Not added: backend.rs had one sanctioned edit.
- **Tail, bypass, programs.** `getTailSamples` (a reverb's tail must keep its strip
  running), the `kIsBypass` parameter (soft bypass, ADR 0018), `IUnitInfo` program lists,
  `IMidiMapping` (MIDI CC → parameter) and `INoteExpressionController` are not surfaced:
  the contract has no place for them yet.
- **The editor's finer points**: `IPlugViewContentScaleSupport` (HiDPI scale), a host-side
  resize (no contract method; the host would call `checkSizeConstraint` then `onSize`),
  key and focus forwarding, and on Linux the `IRunLoop` a GUI needs from the host.
- **macOS** bundles (`CFBundle`, `bundleEntry`) are unsupported; Linux loads but its editor
  path is untested.
- **More buses**: multi-output instruments and extra aux inputs get scratch buffers and
  are not exposed; 64-bit processing is never asked for; note output events are ignored.
- **Limits** per block: 128 distinct parameters and 32 points per parameter (a full queue
  keeps the newest value), 512 events; beyond that, changes are dropped.
- **`moduleinfo.json`'s `Compatibility`** list (which old class ids a class replaces) is
  not read; a project that stored an old id will not find its replacement yet.
- **Threads**: a plug-in that calls the handler from a non-UI thread is served (the
  handler takes an uncontended lock) but not marshalled to the UI thread.
