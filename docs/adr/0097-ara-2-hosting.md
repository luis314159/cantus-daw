# ADR 0097 — ARA 2 hosting

**Status:** accepted; slice 1 built (round 36) · **Date:** 2026-11-02 · **Schema:** 53 ·
**Relates to:** ADR 0019 (hosting standard plug-ins), ADR 0054 (the pitch editor), ADR 0063
(plug-ins out of process), ADR 0047 (licences)

## Context

BACKLOG §3c left "ARA 2 hosting" open: a user who owns Melodyne, SpectraLayers, VocAlign or RX
should use it inside Cantus as in Studio One, Cubase, Logic, Reaper and Waveform. Round 36
asks to measure first and decide. Everything below was read on Celemony's own repositories and
pages on 2026-11-02; what could not be verified is marked **unverified**.

### Evidence

- **Licence.** `ARA_SDK`, `ARA_API`, `ARA_Library`, `ARA_Examples` are "published under the
  Apache License, Version 2.0", copyright 2012-2025 Celemony Software GmbH
  (https://github.com/Celemony/ARA_SDK, https://github.com/Celemony/ARA_API,
  https://github.com/Celemony/ARA_Library, https://github.com/Celemony/ARA_Examples). Open source
  since May 2021 (https://www.celemony.com/en/10-years-of-ara). The ARA_API README says
  implementing the API in a host or plug-in, like using an OS API, "is considered normal use and
  does not make that software a derived work".
- **Compatibility.** Apache-2.0 is one-way compatible with GPL-3.0 (not GPL-2.0), so the
  application (GPL-3.0-or-later) may include it. It could also be combined with MPL-2.0 code,
  but we keep it out of the MPL crates (see Decision). The SDK warns that *companion* APIs
  (VST3, AU, AAX, JUCE) keep their own terms; VST3 is already in use here (ADR 0019).
- **Cost and agreements.** No fee, registration, signing or developer agreement appears on any
  Celemony page or repository read (10-years page, the four READMEs, the change log). The only
  registration the SDK mentions is Avid's, for AAX, which we do not use. **Unverified:**
  whether Celemony asks for a mention of the "ARA" name or logo, or a compatibility notice; no
  page read says so. Ask Celemony in writing before the beta text names "ARA 2".
- **Versions.** 2.0 (2021-05-03), 2.1 (2022-01), 2.2 (2022-11, surround), 2.3.001 (2025-11-07:
  AAX and CLAP fully supported as companion APIs). Public releases must build on a tagged
  release, not HEAD (ARA_API README). Source:
  https://raw.githubusercontent.com/Celemony/ARA_API/main/ChangeLog.txt.
- **Surface.** `ARAInterface.h` is a C API (structs of function pointers). `ARAVST3.h`: the
  VST3 factory exposes a class of category "ARA Main Factory Class" (`IMainFactory::getFactory()`
  returns the `ARAFactory`); the `IAudioProcessor` component implements `IPlugInEntryPoint2`
  whose `bindToDocumentControllerWithRoles()` must be called before the first `setActive`,
  `setState`, `getProcessContextRequirements` or `IPlugView` creation
  (https://raw.githubusercontent.com/Celemony/ARA_API/main/ARAVST3.h). `ARA_Library` is C++
  help aimed at implementing plug-ins; a host can be written against the C headers
  (`ARAMiniHost` in `ARA_Examples` is "pure C").
- **Free test material.** `ARATestPlugIn` (VST3/AU/CLAP, Apache-2.0, built from source with
  CMake) fakes an analysis and passes audio through; `ARATestHost` is a reference host. That is
  enough to test every host path with no licence. Real behaviour needs a commercial plug-in:
  no free ARA 2 plug-in with a real editor was found (SpectraLayers Elements ships as a VST3
  ARA2 plug-in, price **unverified**; VocAlign has 14-day demos; a Melodyne trial is
  **unverified**).
- **What our host does today.** `cantus-host` loads VST3 (`vst3/module.rs`: `GetPluginFactory`;
  `instance.rs`: `IComponent`/`IEditController` casts) and CLAP, split into `MainThread` and
  `AudioThread` halves behind `Backend`; third-party plug-ins may run in `cantus-sandbox`
  (ADR 0063). The `vst3` crate (0.3) has no ARA bindings.

### What hosting requires (the gap)

1. Bindings of the C headers (ARAInterface, ARAVST3, ARACLAP) in a new crate (unsafe, so
   outside `cantus-engine`, which forbids it).
2. Host-side implementation of the document controller's host instance (model updates,
   archiving, content access, audio access, playback controller): a document, musical contexts
   (tempo/bar map, chord track from the project), audio sources (clips' assets, with an audio
   reader that serves samples off the audio thread), audio modifications and playback regions
   (clip start, length, offset, fades, warp).
3. Binding an instance with roles (playback renderer, edit renderer, editor view) and running
   it as an insert: the plug-in renders the regions itself, the clip's own audio is not played,
   with delay compensation for its latency.
4. Edit sync both ways: clip move/split/delete/undo become ARA model updates; the plug-in's
   edits become Commands with inverses (ADR 0004). Its state is saved in the project through
   ARA archiving as an opaque asset.
5. The plug-in's editor view, in the clip's place or its own window (native editor path: ADR 0073).
6. The hard one: ARA objects live in the plug-in's process, and our default for third-party
   plug-ins is out of process (ADR 0063). ARA's audio reader and model calls cannot cross it
   without ARA's IPC (`ARATestHost` can run a plug-in over IPC; vendors' support is
   **unverified**). Slice 1 therefore runs ARA plug-ins **in process only**, said on screen.

## Decision

**GO, in three slices; slice 1 may start in this round once the integrator says so.**

- **Licence placement:** a new crate `cantus-ara` under GPL-3.0-or-later (the application's
  licence). Apache-2.0 notice in THIRD-PARTY-NOTICES; an exact release tag, never HEAD. ARA
  types stay out of the MPL crates: `cantus-core` carries only plain data (which clip has an
  ARA modification, its opaque archive blob).
- **Slice 1:** bindings; a host document built from the project (musical context, sources,
  regions); bind `ARATestPlugIn` in process as an insert; playback renderer and preview; edits
  sync from Cantus to the plug-in; undo; state saved and reloaded; the scan marks ARA-capable
  plug-ins by the factory category.
- **Slice 2:** the plug-in's edits as Commands, editor view in the clip's place, delay
  compensation, freeze and bounce of an ARA clip (offline = real time), MCP tools.
- **Slice 3:** out of process through ARA IPC if vendors support it; a manual test with
  Melodyne or SpectraLayers.
- **Effort:** slice 1 about one builder-round, slices 2 and 3 about one each: 2-3 rounds.
- **Owner's, outside the code:** (a) write to Celemony to confirm no agreement or logo terms for
  naming "ARA 2 support"; (b) a Melodyne or SpectraLayers licence to judge it by ear. Without
  (b) we never claim "works with Melodyne"; we claim "ARA 2 host, tested with Celemony's
  reference plug-in".
- **What would turn this into no-go:** a Celemony requirement of an agreement incompatible with
  the GPL, or a measured in-process crash risk that cannot wait for slice 3.

## Consequences

- Few free DAWs host ARA: Melodyne and RX users keep their tools. The cost is more `unsafe`, a
  format extension in the host, and a second in-process path beside the sandbox, stated to
  the user per plug-in.
- Clip operations gain an ARA-sync hook; every new Command touching clips must be classified
  (the freeze guard's no-wildcard style).
- A project that uses ARA needs the plug-in at load; without it the clip plays its original
  audio and says so; the archive blob is kept, nothing is lost.

## What the tests would hold

Model sync: a clip move/trim/split/delete/undo produces the exact ARA update sequence (against
a recording fake plug-in); archive round trip (store, reload, identical blob); `ARATestPlugIn`
bound before `setActive`, roles as assigned; pass-through null (an ARA clip with the test
plug-in equals the clip bit for bit); block-size independence at tolerance 0; offline = real
time; the audio reader never called on the audio thread; `rt_memory` zero allocations; a
missing plug-in keeps the clip and names it; undo-fuzz arms for each new Command.


## Slice 1 as built (round 36, after the integrator's go)

### 1. Where the code and the unsafe live

- `crates/cantus-ara` (GPL-3.0-or-later): `sys.rs` → `sys_x86.rs` / `sys_arm.rs`, the
  bindings generated from `ARAInterface.h` of **ARA_API `releases/2.3.0`** (commit 65ec5c43;
  ARA_SDK 0005c532). ARA packs its structs at **1 byte on x86 and 8 on ARM**, so the bindings
  are generated per architecture and chosen by `cfg(target_arch)`: one build for every
  release target (Windows, Linux, both Macs). `vst3.rs`: `IMainFactory`,
  `IPlugInEntryPoint`, `IPlugInEntryPoint2` by hand (`ARAVST3.h` needs the VST3 SDK to
  parse). `callbacks.rs`: the five host controller interfaces. `host.rs`: one plug-in's
  document controller. `document.rs`: safe and pure.
- `cantus-host` (GPL): `vst3/ara.rs` (the main factory class; an instance bound with roles
  and given its region **inside a hook that runs right after `initialize`**, before the
  controller reads the component's state, `setupProcessing` or `setActive`:
  `Parts::create_bound`), and `ara.rs` (`AraSession`: documents, instances, renders).
- **The unsafe lives in `cantus-ara` and `cantus-host` only.** `cantus-engine` stays
  `forbid(unsafe_code)` and knows no ARA: it plays *renders made outside it*
  (`clip_renders.rs`); `cantus-core` holds plain data (`cantus_core::ara`). `cantus-rt` was
  not needed.
- The SDK is never committed. `THIRD-PARTY-NOTICES.md` carries the Apache-2.0 notice of the
  generated bindings (`xtask/notices/head.md`).

### 2. How a bound clip sounds: a render, not a live insert

The brief named "an in-process ARA-bound insert". What was built keeps the plug-in **off
the audio thread**: each bound clip has its own VST3 instance, bound to the plug-in's
document as the **playback renderer** of the clip's region (roles known: all three;
assigned: playback renderer). `AraSession` renders the region offline through that
instance (`render`, blocks of 1 024, the transport at the region's samples) into a copy of
the clip's source **at the source's rate**, and the engine plays it in place of the source,
converted by the very resampler that converts sources (`SampleCache::set_clip_renders`).
Why:

- **ADR 0005 holds without an exception.** The plug-in runs in Cantus's process (ARA's
  objects cannot cross to the sandbox, ADR 0063), so a stall in it cannot be fenced off by
  the bridge's deadline; off the audio thread a slow plug-in cannot drop out the device.
- **Everything after the source applies as to any clip**: clip gain and its envelope, fades,
  crossfades, the strip, the mix. A live insert would render the regions *after* the clip
  stage, losing fades and clip gain (ARA carries neither).
- **Offline equals real time and block size cannot matter, by construction**; the bounce,
  freeze and the stems hear exactly what plays.
- The cost: an edit is heard after its re-render (a clip of minutes with a heavy plug-in
  takes a moment, on the render thread: §9). The render is made again when the clip's
  region changes or the plug-in says its content changed (`notifyModelUpdates`, polled every
  frame while a clip is bound).

Regions sit on the source's sample grid (`RegionFrames`): start, length and source offset
are whole frames, so the plug-in's render and Cantus's placement agree to the sample. The
render covers the clip's window, the plug-in's head and tail and 0.1 s of source on each side
(§9).

### 3. The project and undo (schema 53)

- `AudioRegion::ara: Option<Box<AraClip>>`: the plug-in (VST3 class id, name, vendor, path,
  ARA factory id) and its **partial archive** of the clip's modification (`AraArchive`: the
  bytes, the document archive id, the persistent ids written under). Migration 52 → 53
  writes nothing; `fixtures/v53.cantus` binds one clip. Schemas 50–52 are placeholders in
  this branch (mix, external, editor): the integrator replaces them.
- `Command::SetClipAra` (`cmd.bind_ara` / `cmd.unbind_ara`), its inverse the binding as it
  was; refused on a clip that is not audio, or that has a warp or audible pitch edits (the
  plug-in does pitch and time); `SetClipWarp` and `SetClipPitch` refuse a bound clip in turn.
  The binding is each clip's own, linked or not (§12), with its own modification and
  archive. Freeze guard:
  touches the clip's track. Undo fuzz: its own stream (`arng`), bound, let go and refused
  each covered.
- One modification per bound clip (persistent id `cantus.clip.<id>`), one audio source per
  recording (`cantus.source.<id>`), a region sequence per track, one musical context with
  the tempo map (two entries at one tempo; an entry per quarter note when it moves) and the
  metre changes.
- **The plug-in's work survives undo.** A clip that leaves (deleted, let go) loses its
  region; its modification is *deactivated for undo* and reactivated when the clip comes
  back. When the last bound clip of a plug-in goes, its document closes and each
  modification's archive is kept by the session; undo restores from it. A clip arriving
  with an archive and no modification here (open, paste, import) is restored
  (`restoreObjectsFromArchive`, its ids mapped); a copy of a clip whose modification is
  alive is **cloned** (`cloneAudioModification`).
- **Saving** asks each live modification for its archive (`PluginHost::project_for_save`).

### 4. Finding ARA plug-ins

A scan (in the scanner process, as always) marks a VST3 processor paired with an
"ARA Main Factory Class" (by name, or as the module's only processor) with the feature
`ARA`, from the factory and from `moduleinfo.json` alike. A plug-in database written
before is rescanned once for it (`PluginDb::ara_marked`).

### 5. The interface and agents

- The audio clip's menu has an **ARA plug-in** section: *Edit with* each ARA plug-in found;
  bound, what the plug-in does with the clip (or why it plays its source) and *Let go of*;
  and always **"ARA plug-ins run inside Cantus, not isolated"**, said again on the status
  line when a clip is bound. EN/ES (`locales/areas/ara.*.txt`), manual page
  `docs/manual/{en,es}/ara.md`.
- Agent tools (`cantus-ui/src/ai_tools_ara.rs`): `list_ara_plugins`, `bind_ara_plugin`,
  `unbind_ara_plugin`; `tools/list` golden regenerated.

### 6. Linux and macOS

Everything compiles on every system with no `cfg` gate beyond the bindings' architecture:
VST3 ARA plug-ins load on Linux and macOS the same way (the bundle's binary for the system),
and the test plug-in builds for each.

### 7. What the tests hold (built)

- `cantus-ara`: `tests/document.rs` (12: add, move, trim, slip, split, mute, delete, undo,
  replaced file, take lanes, tempo); `callbacks.rs` (a reader reads each channel and zeros
  outside the source; archives write, read back and refuse a read past their end; tempo and
  bars are read as published); the VST3 IIDs.
- `cantus-host/tests/ara.rs`, against **`cantus-test-ara`**, an ARA 2 plug-in built from
  source in this repository that renders its regions from the host's readers at a gain per
  modification, archives and restores it, and **counts every breach of ARA's call rules**
  (an edit outside `beginEditing`/`endEditing`, audio read without access, a region added
  while active, a bind after activation, an object destroyed while in use): a bound clip
  renders its source **to the bit**; an edit in the plug-in renders the window again; trim,
  delete and undo reach the plug-in and keep its edit; the archive is saved and restored in
  a new session; a copy is cloned with the original's edits; a missing plug-in leaves the
  source playing and says so; letting go of the last clip closes the document and undo binds
  it again with its edits; a moving tempo map is published; a scan marks the ARA plug-in and
  an old database is rescanned. Every test asserts **zero breaches**.
- Celemony's **ARATestPlugIn** (Apache-2.0): `tools/ara/build-test-plugin.sh` builds it
  reproducibly from the tagged SDK (with the VST3 SDK the ARA SDK pins), into
  `../downloads/ara/build`, never committed; `celemonys_reference_plugin_when_built` binds,
  renders, archives and restores it, and **skips saying why** when it is not built. In this
  round the automated session was not permitted to build fetched third-party code, so that
  test has **not yet run against ARATestPlugIn**: the owner (or CI) runs the script once.
- `cantus-engine/tests/clip_renders.rs`: a render equal to its source plays the source to
  the bit at 48 and 44.1 kHz and at blocks of 512, 333 and 64; a render that differs is what
  plays; no render, the source.
- `cantus-project/tests/schema53.rs`: the fixture's bound clip and archive, a save and load
  to the bit, every older fixture with no clip bound. Undo fuzz and freeze guard as above.
- `cantus-ui/tests/ara_ui.rs`: the agent tools list, bind (refused for an unknown plug-in)
  and let go; the window's engine sync renders the bound clip; saving takes the archive; undo
  binds again; the menu's words in both languages.


## Slice 2's essentials and the review's findings (round 36, same round)

The integrator brought the essentials of slice 2 into this round ("Melodyne inside Cantus as
in Studio One") and a review of the branch listed eleven findings. What was built:

### 9. Renders on a render thread (`cantus-host/src/ara_worker.rs`)

- One render thread per session; a job renders one clip's region through the clip's
  instance (its audio half behind a lock: one render at a time per instance, any thread, as
  ARA allows). Model edits stay on the document thread. A newer job cancels an older one
  between blocks (a generation per clip); **the last render plays until the new one is
  taken in**; the clip's body shows the progress (`views/ara_menu.rs::paint_clip`).
- A region that changes (a drag, a trim) renders once it has rested 150 ms (`DEBOUNCE`).
- **Range-limited re-renders**: the plug-in's `notifyAudioModificationContentChanged`
  range (modification time) becomes frames of the region; only that part is rendered again,
  over the clip's last render.
- **Only the clip's stretch of the source** is rendered — the region with the plug-in's
  head and tail (`getPlaybackRegionHeadAndTailTime`) and a 0.1 s margin — never a copy of
  the whole recording; it is converted to the engine's rate **on the render thread** and the
  engine places it by its start (`ClipRender::start`, read as a fine offset). The source the
  plug-in reads is the sample cache's own decode at the source's rate (on its loader thread;
  the very samples playback uses when the engine runs at that rate).
- **Timing**: the transport the plug-in sees follows the project's tempo map; a plug-in
  with latency is run that many samples longer and its first samples dropped (pre-roll), so
  its render lands in place. **An offline render needs no other delay compensation**, and
  the test proves it (`a_plugin_with_latency_renders_in_place`, 333 samples). Channels: a
  source is mono or stereo; both are rendered (the instance is stereo; a mono source takes
  its left).
- Measured (`memory_of_twenty_comp_clips_and_the_time_from_edit_to_heard`, Windows, debug
  test build, the test plug-in): 20 comp clips of 30 s over a 10-minute stereo take hold
  **231 MB of renders** (the take is 230 MB; a copy per clip would be 4.6 GB), all rendered
  in 0.42 s; **1 s edited inside a 3-minute clip is heard 31 ms later** (the render 27 ms).
  A real plug-in's own rendering time adds to it.

### 10. The plug-in's editor, and its edits in Cantus's undo

- Each instance is bound with all three roles; its editor view is told of the clip's region
  (`notifySelection`) and its own window opens from the clip's menu (*Open {name}*) through
  the window path the hosted VST3 editors use; a closed window is noticed each frame.
- **Each edit gesture in the plug-in is one undo step**: when the plug-in says a clip
  changed and its archive is not the one the project holds, the session hands the window a
  `Command::SetClipAraState` (the archive whole; never shared with linked clips) with a
  merge key per clip and gesture (edits within 700 ms merge). Undo puts the project's state
  back and the session restores it into the plug-in (`restoreObjectsFromArchive`); before a
  clip's first saved state, undo restores the state the plug-in had when the clip was bound.
  A plug-in that rewrites a state differently is not taken for an edit (what the project and
  the plug-in agree on is kept as a pair).
- The plug-in's transport wishes (`requestStartPlayback`, `…StopPlayback`,
  `…SetPlaybackPosition`) drive the transport. `notifyDocumentDataChanged` makes the session
  look at every clip's state, so any change becomes an undo step and the project is
  modified. (Cantus saves per-clip partial archives only: no document-level data.)

### 11. A plug-in that crashes or hangs

- **Marker**: before opening a plug-in, editing its document (bind, restore) and while a
  render of it runs, the session writes `ara-running.txt` beside the plug-in database naming
  it, and clears it after. A Cantus that starts with the marker there stops those plug-ins:
  their clips play their sources, the menu says why and offers **Retry**. Proven with a
  child process whose plug-in aborts in a render
  (`a_plugin_that_crashes_cantus_is_stopped_at_the_next_start_until_retry`).
- **Watchdog**: a render past the limit (30 s; tests 150 ms) stops the plug-in for the
  session; its render thread and instances are left to themselves (a hung plug-in cannot be
  stopped from outside, so they are never freed), a new render thread starts, and the window
  never waits (`a_plugin_that_hangs_is_stopped_and_the_window_never_waits`, child process).
- **Play sources only (ARA plug-ins off)** in the clip menu: no plug-in loads; every bound
  clip plays its source (`ara_plugins_off_play_every_source`).

### 12. The review's other findings

- **Fields checked** (finding 7): every document-controller field Cantus calls is checked
  once against the plug-in's `structSize` and for null when the controller is made
  (`host::missing_fields`, ARA_IMPLEMENTS_FIELD); a plug-in lacking one is refused there. The
  instance's playback renderer, editor renderer and editor view tables are checked at bind.
- **Layout proven** (finding 8): the bindings are generated with bindgen's layout tests for
  each architecture — every struct's size, alignment and field offsets as clang lays out the
  C header for x86 (packed 1) and ARM (packed 8), asserted at compile time; the ARM build
  (aarch64-apple-darwin) compiles.
- **Linked clips** (finding 5): the binding is each clip's own. `SetClipAra` edits one clip
  (its inverse exact); shared edits (`edit_shared`) and link content (`ClipContent`, `adopt`)
  never carry a binding; the undo fuzz binds linked clips with differing archives and sets
  plug-in states.
- **Fresh binds start clean** (finding 10): `AraClip::binding` numbers each binding; a fresh
  bind gets a new number and a fresh modification (no kept archive, no reactivated
  modification), undo brings the same number back with the edits. A clip let go keeps its
  binding and state for **Bind to {name} with its previous edits**.
- **Reverse** (finding 11): binding a reversed clip is refused, and so are reverse, warp and
  audible pitch edits on a bound clip or one linked to it. The menu says *Bind to {name}* and
  *Open {name}* — what each does.
- Schema 53 (finding 6): the integrator regenerates `v53.cantus` at integration
  (`docs/round36/ara.md`).

### 13. Still the owner's, and what is left

- Write to Celemony about naming "ARA 2"; run `tools/ara/build-test-plugin.sh` once; try a
  commercial ARA plug-in by ear. **No plug-in is named as compatible** until then.
- Left for a later round: out of process over ARA IPC (slice 3); a render of a plug-in's
  multichannel (surround) source (sources are mono or stereo today).

### 14. The fix round (after integration)

- **A part never leaves the rest unprocessed.** A part said changed is laid over the last
  render only when that render covers the same stretch of the source at the same rate;
  otherwise the whole region is rendered. A part said changed while a render is on its way
  covers what that render covered (a whole render in flight makes the new one whole too).
  Tests: `a_part_over_a_render_of_another_stretch_renders_the_whole_region`,
  `a_part_never_replaces_a_whole_render_in_flight` (both fail without the fix).
- **Offline renders wait for ARA renders.** `PluginHost::render_ready` — the one gate
  export, freeze (and so the recording mode's preparation and bounce's queue), video
  export, the mastering assistant's listens and modulator bakes already ask each frame — is
  false until `PluginHost::ara_settled`: no ARA render queued, running, waiting for a drag to
  rest, made and not taken in, or not handed to the engine, and no plug-in edit waiting to
  become a project step. A freeze's key is computed from the project's archive only once the
  render the engine plays is that archive's. The two renders made at once without the gate
  wait for it too: a bounce (`settle_ara`, refused with words after a minute) and a comp
  merge (waits as for a loading file). Stems separate a clip's file, not its processing:
  nothing to wait for. Tests: `the_session_is_settled_only_when_no_render_is_on_its_way`,
  and in `ara_ui` an export not ready while the clip's render is on its way, ready after.
