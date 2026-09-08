# The World's Best DAW — Master Specification & Roadmap
### A Rust Project — "Beating FL Studio by combining the best of every DAW"

> *"Whatever you do, work at it with all your heart, as working for the Lord, not for human masters."* — Colossians 3:23
>
> This document is built with the discipline required of any work meant to last: excellence in execution, the humility to study those who did it well before us, and the courage to attempt something great knowing it will demand years of real sacrifice. Viva Cristo Rey.

---

## 1. Vision & Guiding Principles

**Vision:** A free, open-source DAW written in Rust that combines FL Studio's workflow speed, Bitwig's modulation power, Pro Tools/Reaper's audio engine and editing precision, Ableton Live's live-performance fluidity, and Logic Pro's native instrument quality — with AI as a first-class citizen from day one.

**Design principles (non-negotiable):**

| Principle | What it means in practice |
|---|---|
| **Technical excellence** | Zero xruns, zero audio glitches, zero "almost works" |
| **Honest scope** | A small, finished MVP beats a giant roadmap with nothing shipped |
| **Humility** | Study existing projects in depth (Ardour, Meadowlark, Bitwig, Reaper) before reinventing — learn from their architectural decisions and their mistakes, without needing to contribute to them |
| **Perseverance** | Measured in years; each phase must close at quality before the next one opens |
| **User freedom** | Open-source, no vendor lock-in, open formats, no hidden telemetry |
| **AI-native from origin** | AI integration is not bolted on later as a third-party plugin; it is designed into the architecture from Phase 0, with total provider freedom (cloud or local) |
| **Data safety above all** | A user must never lose work. Not to a crash, not to a plugin, not to a bad file format migration |

---

## 2. Competitive Analysis — What We Take From Each DAW

| DAW | What we take from it |
|---|---|
| **FL Studio** | Powerful piano roll (chord stacking, arpeggios, humanize), pattern-based sequencing workflow, step sequencer, transparent plugin delay compensation, "free updates for life" as a philosophy |
| **Ableton Live** | Session View (clips + scenes for live improvisation), real-time audio warping, loop-based production workflow, Max for Live (visual scripting) |
| **Bitwig Studio** | Modulation system (The Grid, per-voice modulators, deep macros), per-process plugin sandboxing, true native multi-OS support |
| **Logic Pro** | Native instrument quality, Flex Time/Pitch, Smart Tempo, curated sample/loop library |
| **Cubase** | The most mature MIDI engine on the market (VST Expression, Logical Editor, Chord Track), integrated music notation |
| **Studio One** | Universal drag & drop, Chord Track + Chord Assist, contextual tutorials, macro-arrangement (Arranger Track) |
| **Reaper** | Extreme lightness, total customization (ReaScript/Lua), infinitely flexible audio routing, honest/affordable licensing |
| **Pro Tools** | Sample-accurate professional audio editing, take comping, industry-reference multitrack recording engine |
| **Renoise** | Tracker-style pattern editing as an alternative mode for electronic/chiptune producers |
| **Ardour** | Open-source governance lessons, Linux-first pro audio integration, session format transparency |

---

## 3. General Architecture

```
┌─────────────────────────────────────────────────────┐
│                  UI Layer (egui/vizia)               │
│  Playlist | Piano Roll | Mixer | Session View         │
│  Browser  | Modulation Grid | Notation | Video        │
└───────────────────┬───────────────────────────────────┘
                     │ lock-free channels (no locks on audio thread)
┌───────────────────┴───────────────────────────────────┐
│              Engine Core (separate crate)              │
│  Audio Graph | Transport/Clock | MIDI Router            │
│  Plugin Host (CLAP/VST3) | Native DSP | Modulation      │
└───────────────────┬───────────────────────────────────┘
                     │
┌───────────────────┴───────────────────────────────────┐
│         I/O Backend (cpal / jack / ASIO / CoreAudio)   │
└─────────────────────────────────────────────────────────┘

        ┌───────────────────────────────────────────┐
        │      AI Layer (separate crate, async)      │
        │  Provider Abstraction | MCP Client/Server   │
        │  Local Inference | Command Bridge (async)   │
        └──────────────────┬───────────────────────────┘
                            │ async commands (never touches the audio thread directly)
                            └── connects to UI Layer and Engine Core as just another client
```

**Architectural golden rules:**
- **Zero allocations in the audio callback.** Everything pre-allocated, reusable buffer pools.
- **Engine and UI in separate crates**, communicating over lock-free queues (`rtrb`, `ringbuf`).
- **Plugins in isolated processes** (Bitwig-style sandboxing) so an unstable plugin cannot take down the whole DAW.
- **Sample-accurate automation** throughout the pipeline, not just at buffer granularity.
- **AI never touches the real-time audio thread.** All inference (cloud or local) lives in its own async crate; it only pushes discrete commands (create track, move automation, apply preset) through the same command bus the UI uses — never directly into the audio callback.
- **One command bus to rule them all.** UI actions, scripting actions, MCP agent actions, and AI actions all flow through the same command layer. This means undo/redo, permissions, and auditing are implemented once and apply universally.

---

## 4. Native AI Integration — Designed In, Not Added On

This section deliberately sits *before* the feature list. If AI is designed as an afterthought, it will always feel like duct tape. Designed into the engine from the start, it becomes a first-class citizen — held to the same standard of technical excellence and honesty as everything else. No external provider may become a single point of failure or a mandatory dependency: the user decides which AI they work with, or whether they use one at all.

### 4.1 AI Layer Principles
- **Never blocks or touches the audio thread.** All inference is async, in its own crate, communicating over the same command bus the UI uses.
- **Provider-agnostic.** No DAW feature depends on a single API; everything goes through an abstraction layer.
- **Local-first for those who want it.** A user must be able to run the DAW with full AI capability without sending a single byte to the internet, if that is their choice — real freedom, not vendor lock-in dressed up as innovation.
- **Total transparency.** The user sees and controls exactly what data (audio, MIDI, project state) is sent to which provider, when, and why.
- **Reversible and auditable.** Every action AI performs on the project flows through the same undo/redo system as a manual user action.

### 4.2 Multi-Provider Abstraction (Cloud)
- [ ] Generic `AiProvider` trait with interchangeable implementations: Anthropic (Claude), OpenAI, Google (Gemini), plus open room for new providers
- [ ] Per-provider API key configuration, stored encrypted locally (never inside a shared project file)
- [ ] Per-task provider selection (e.g. "use Claude for arrangement, a local model for stem separation") — not everything has to come from one vendor
- [ ] Configurable automatic fallback when a provider fails or is unavailable
- [ ] Response streaming for conversational tasks (production chat, live suggestions)
- [ ] Per-provider cost tracking and configurable spend limits

### 4.3 Local Providers (Privacy, Latency, Offline)
- [ ] Local LLM support via `llama.cpp` bindings or Ollama's local API for reasoning/text tasks without cloud dependency
- [ ] Local audio models via ONNX Runtime or `tract` (pure-Rust) for non-LLM tasks: stem separation (Demucs), denoise, audio generation (MusicGen/Stable Audio where hardware allows)
- [ ] Hardware detection (GPU/NPU) with automatic selection of the best local model for the machine's capability
- [ ] Verifiable "100% offline" mode — the user can confirm no data left their machine

### 4.4 MCP (Model Context Protocol) — In Both Directions
- [ ] **DAW as MCP client:** the DAW connects to external MCP servers as tools — a music-theory server, a sample-library search server, a project-management server — extending capability without having to build everything internally
- [ ] **DAW as MCP server:** the DAW exposes its own actions (create track, apply plugin, move clip, render, read project state) as MCP tools, so Claude or other agents can drive the DAW programmatically and auditably
- [ ] Every MCP-exposed action flows through the same command bus and the same permission/undo system the UI uses — an agent gets no back door into the engine
- [ ] Granular permissions: the user decides which actions an agent may run without confirmation (e.g. "may suggest, may not export or overwrite") versus which require explicit approval

### 4.5 Concrete Features This Layer Enables
- [ ] Composition assistant: melody/chord/progression suggestions grounded in what already exists in the project
- [ ] Mixing assistant: analysis of the current mix with EQ/compression/balance suggestions (never applied without confirmation)
- [ ] Local stem separation (vocals, drums, bass, other) with no cloud dependency
- [ ] Text-to-sample/loop generation (local or cloud model, user's choice)
- [ ] Natural-language control of production tasks ("bring the drum bus up 2dB", "add a cathedral reverb to the vocal") translated into real engine commands, always reversible
- [ ] Auto-tagging and organization of samples/projects (tags, detected BPM, detected key)
- [ ] AI-assisted audio repair (declick, denoise) as an optional layer over the native editing tools

### 4.6 Risks & Safeguards for This Layer (Honesty First)
| Risk | Safeguard |
|---|---|
| Network latency breaks creative flow | Local-first as a genuine option, not a "degraded mode" |
| Dependency on a provider that changes pricing/API | Provider abstraction from day one; never couple a feature to a single vendor's SDK |
| An MCP agent makes unwanted changes | Everything goes through undo/redo + granular permissions; never direct audio-thread access |
| Cloud inference costs for the user | Per-provider cost transparency, configurable limits, prefer local where viable |
| User loses creative control by over-relying on AI | AI suggests, the human decides — no AI feature may be required to produce a complete piece of music in this DAW |

---

## 5. Complete Feature List

### 5.1 Audio Engine (Core)
- [ ] Allocation-free, lock-free audio callback
- [ ] Multi-backend support: ASIO (Windows), CoreAudio (macOS), JACK/ALSA/PipeWire (Linux), WASAPI
- [ ] Sample rates up to 384kHz, 32-bit float internal (64-bit float option for mastering bus)
- [ ] Latency reporting and automatic plugin delay compensation (PDC)
- [ ] Per-track/per-plugin configurable oversampling
- [ ] High-quality resampling engine (for warping, pitch, time-stretch)
- [ ] Track freeze/bounce to free CPU
- [ ] Multi-core audio graph rendering (parallelized across independent branches)
- [ ] Built-in per-track CPU/latency metering so users can diagnose performance themselves

### 5.2 Transport, Tempo & Sync
- [ ] Tempo track with BPM automation (not just fixed tempo)
- [ ] Time signature changes at any point
- [ ] Smart Tempo (detect tempo of imported audio, Logic-style)
- [ ] External sync: MIDI Clock, MTC, LTC/SMPTE timecode, Ableton Link
- [ ] Configurable metronome (subdivision, count-in, tap tempo)

### 5.3 MIDI Sequencing
- [ ] Piano roll with velocity, humanize, quantize (swing/groove templates), chord stacking, built-in arpeggiator
- [ ] Step sequencer (FL-style) as an alternative view to the piano roll
- [ ] Tracker mode (Renoise-style) for those who prefer it
- [ ] Global Chord Track with chord detection/suggestion (Cubase/Studio One style)
- [ ] MIDI Logical Editor (programmable MIDI transformations, Cubase-style)
- [ ] Expression maps / articulations for orchestral instruments
- [ ] Integrated music notation (score view) with MusicXML/PDF export
- [ ] Micro-timing and groove quantize with importable templates
- [ ] **MIDI 2.0 support** (higher resolution, bidirectional, property exchange) — a genuine forward-looking differentiator
- [ ] Full MPE (MIDI Polyphonic Expression) for expressive controllers

### 5.4 Arrangement Views
- [ ] **Playlist/Arrangement View** (FL/Cubase style): linear timeline with tracks, patterns, automation
- [ ] **Session View** (Ableton style): clips + scenes for improvisation and loop-based production, with launch quantization
- [ ] **Arranger Track** (Studio One style): macro sections (Intro, Verse, Chorus) that reorder all content at once
- [ ] Pattern-based workflow (FL-style pattern reuse) coexisting with linear arrangement
- [ ] Take comping (record multiple takes and comp the best, Pro Tools style)
- [ ] Folder/group tracks with nested organization
- [ ] Markers, regions, and arrangement-level navigation

### 5.5 Audio Editing
- [ ] Sample-accurate editing (cut, crossfade, fade in/out with curves)
- [ ] Real-time Flex Time / Warping (Logic/Ableton style): per-event time-stretch and pitch correction
- [ ] Elastic Audio with multiple algorithms (monophonic, polyphonic, percussive, rhythmic)
- [ ] Spectral editing (visual editing over a spectrogram, integrated iZotope-RX-style)
- [ ] Strip silence, automatic transient detection
- [ ] Visual comping of multiple takes across lanes
- [ ] Native pitch correction (Melodyne-class quality target)
- [ ] **ARA 2 support** (so Melodyne, VocAlign etc. integrate natively rather than as insert effects)

### 5.6 Mixer
- [ ] Unlimited tracks, buses, sends/returns
- [ ] Flexible routing matrix (sidechains, instrument multi-out, nested buses) — Reaper-level
- [ ] Professional metering (LUFS, RMS, true peak, phase correlation, real-time spectrum)
- [ ] Mix groups (VCA groups) and mix snapshots
- [ ] Automation of any parameter with curves (linear, exponential, S-curve)
- [ ] Hardware insert support with automatic latency compensation (ping measurement)
- [ ] External instrument routing (treat outboard synths as if they were plugins)

### 5.7 Modulation System (the differentiator vs FL Studio)
- [ ] **Grid-style** canvas where any parameter connects to any modulator
- [ ] Macros with custom mapping curves (Bitwig-style)
- [ ] LFOs, envelopes, step modulators, random/sample & hold as universal modulation sources
- [ ] Per-voice/polyphonic modulation (each note carries its own independent modulation)
- [ ] Audio-rate modulation where it makes musical sense

### 5.8 Plugin Architecture
- [ ] Native **CLAP** host (priority — open, modern, Rust-friendly)
- [ ] **VST3** support via bindings for existing-ecosystem compatibility
- [ ] **AU** support on macOS (required for that market)
- [ ] **LV2** support on Linux (required for that market)
- [ ] Per-process sandboxing (total isolation, Bitwig-style)
- [ ] Automatic, transparent plugin delay compensation
- [ ] 32-bit plugin bridging inside a 64-bit host (legacy compatibility)
- [ ] In-house SDK for building native Rust instruments/effects
- [ ] Plugin scanning that survives crashes (a bad plugin cannot brick startup)

### 5.9 Native Instruments
- [ ] Subtractive synth (oscillators, filters, classic envelopes)
- [ ] FM synth (DX7/FM8 class)
- [ ] Wavetable synth (Serum class)
- [ ] Multi-layer sampler with disk streaming (for large libraries)
- [ ] Drum machine with integrated step sequencer
- [ ] Granular synth
- [ ] Modal/physical modeling engine (for experimental instruments)
- [ ] Sample slicer/chopper (beat-slicing workflow)

### 5.10 Native Effects
- [ ] Parametric EQ with live spectral analysis (minimum 6 bands + high/low pass)
- [ ] Compressor (VCA, optical, FET, multiband)
- [ ] Algorithmic + convolution reverb (with impulse-response support)
- [ ] Delay (digital, modeled analog, ping-pong, tempo-synced)
- [ ] Saturation/distortion (tape, tube, waveshaper)
- [ ] Chorus/Flanger/Phaser
- [ ] Vocoder
- [ ] Mastering limiter/maximizer with true-peak metering
- [ ] Integrated mastering tools (loudness matching, dithering, streaming-platform targets)
- [ ] Gate/expander, de-esser, transient shaper
- [ ] Utility tools (gain, phase invert, mono/stereo, mid-side)

### 5.11 Recording
- [ ] Simultaneous multitrack recording (multichannel interfaces)
- [ ] Automatic and manual punch in/out
- [ ] Loop recording with automatic comping
- [ ] Low-latency monitoring (direct monitoring / ASIO passthrough)
- [ ] MIDI recording with configurable overdub/replace
- [ ] Pre-roll recording buffer (capture what happened *before* you hit record)

### 5.12 Spatial & Immersive Audio
- [ ] Multichannel support beyond stereo (5.1, 7.1, 7.1.4)
- [ ] Ambisonics support (for VR/360 content)
- [ ] Object-based mixing panner (Dolby Atmos-compatible workflow where licensing permits)
- [ ] Binaural monitoring of immersive mixes over headphones
> Note: full Dolby Atmos certification is a commercial licensing matter, not just a technical one. Design the architecture to be channel-agnostic so this remains possible without a rewrite.

### 5.13 Video & Scoring to Picture
- [ ] Video track with frame-accurate sync
- [ ] Video playback in a detachable window (multi-monitor friendly)
- [ ] Timecode display and SMPTE offset support
- [ ] Marker import from video editing tools (EDL/AAF)
- [ ] Basic video export with the finished mix muxed in

### 5.14 Interoperability & Migration (critical for adoption)
This is one of the most underrated features in the entire document. A DAW that cannot ingest a user's existing work will not be adopted, no matter how good it is. Lowering the switching cost is a feature.
- [ ] **Standard MIDI File** import/export (baseline, non-negotiable)
- [ ] **AAF / OMF** import/export (the professional post-production interchange standard)
- [ ] **Broadcast WAV** with embedded timestamps
- [ ] **Stem/multitrack import** with automatic track naming and alignment
- [ ] Read-only import of other DAWs' project formats where legally and technically feasible (documented formats first; reverse-engineered formats only where clearly lawful)
- [ ] **Preset import** for common formats where documented
- [ ] Export presets targeting other DAWs (so collaborators are not locked out)
- [ ] Track/clip drag & drop between the DAW and the OS file manager, both directions

### 5.15 Project, Data Safety & Collaboration
- [ ] Open, versioned project format (RON/JSON + assets), version-control friendly (git-friendly, human-diffable)
- [ ] **Schema versioning with forward/backward migration** — a project saved today must open in five years
- [ ] Autosave with version history and crash recovery
- [ ] Robust, unlimited undo/redo (including AI and scripted actions)
- [ ] Project consolidation (package every sample/plugin used, for sharing or archiving)
- [ ] **Corruption resistance:** atomic writes, checksums, never overwrite the last good save until the new one is verified
- [ ] Real-time collaboration (Soundtrap/BandLab style) — advanced phase, optional
- [ ] Project comments/notes and per-track annotations for team workflows

### 5.16 Extensibility & Scripting
- [ ] Scripting API (Lua or Rhai) for task automation, ReaScript-style
- [ ] Visual MIDI logic editor (Max for Live style) for building custom generators/processors
- [ ] Customizable theme/skin system
- [ ] Hardware controller integration (generic MIDI mapping + profiles for popular controllers)
- [ ] Stable, documented plugin/extension API so the community can build on it without forking

### 5.17 Live Performance
- [ ] Performance mode with clip/scene Session View
- [ ] Ableton Link for syncing with other devices/apps on the same network
- [ ] Hardware control (Push-like, Launchpad-like grids) with visual feedback
- [ ] Crash-resistant performance mode (if the UI dies, audio keeps playing)

### 5.18 Export & Rendering
- [ ] Bounce/export to WAV, FLAC, MP3, OGG, AAC, Opus
- [ ] Automatic stem export (per track, per bus, per group)
- [ ] Multi-core offline rendering for heavy projects
- [ ] Batch export with naming templates
- [ ] Export with loudness normalization targets per streaming platform (Spotify, Apple Music, YouTube)
- [ ] Video export with basic integrated audio visualizer

### 5.19 Accessibility (a real gap in almost every DAW)
Most DAWs are close to unusable for blind and low-vision producers. Getting this right is both the excellent thing to do and a genuine market differentiator — Reaper is the only DAW with a serious reputation here, and even that is largely community-built.
- [ ] Full screen-reader support (NVDA on Windows, VoiceOver on macOS, Orca on Linux)
- [ ] Complete keyboard-only navigation — every action reachable without a mouse
- [ ] Fully remappable keyboard shortcuts with importable presets from other DAWs
- [ ] Colorblind-safe palettes and configurable high-contrast themes
- [ ] Scalable UI (from small laptop screens to 4K/5K, per-monitor DPI awareness)
- [ ] Audio cues/sonification for non-visual feedback on parameter changes
- [ ] Reduced-motion mode for users sensitive to animation

### 5.20 Onboarding, Learning & Documentation
A powerful DAW that nobody can learn is a failed DAW. FL Studio wins partly on approachability.
- [ ] Contextual, in-app tutorials ("Show Me"-style)
- [ ] Curated project templates by genre and by task (podcast, band recording, EDM, film scoring)
- [ ] Searchable in-app manual with deep links from every UI panel
- [ ] Interactive tooltips explaining not just *what* a control does but *why* you'd use it
- [ ] Curated factory sample/loop/preset library shipped with the DAW
- [ ] Video documentation strategy from launch, not two years later

### 5.21 Internationalization
- [ ] Full i18n architecture from Phase 0 (no hardcoded UI strings, ever)
- [ ] RTL layout support (Arabic, Hebrew)
- [ ] CJK font and input-method handling
- [ ] Community translation pipeline with a defined workflow
- [ ] Locale-aware number/date formatting

---

## 6. Quality Assurance & Testing Strategy

Audio software fails in ways that ordinary software testing does not catch. A DAW that "compiles and runs" can still be unusable. This deserves its own discipline, not an afterthought.

| Test type | What it catches |
|---|---|
| **Null tests** | Render the same project twice, or through different code paths, and confirm bit-identical (or within-tolerance) output. Catches DSP regressions instantly. |
| **Xrun/latency stress tests** | Load the graph to 90% CPU and verify zero dropouts across all supported backends |
| **Long-run soak tests** | Run a session for 24–72 hours to catch memory leaks and drift |
| **Plugin fuzzing** | Feed malformed/hostile plugin behavior at the host boundary and verify the sandbox holds |
| **Project format round-trip tests** | Save, load, save again — verify identical state. Plus migration tests from every historical schema version. |
| **Cross-platform CI matrix** | Every commit built and tested on Windows/macOS/Linux, x86_64 and ARM64 |
| **Real hardware test farm** | Verified against real audio interfaces, not just virtual devices |
| **Automated UI regression** | Screenshot diffing on core views to catch visual breakage |
| **Undo/redo invariant tests** | For any command sequence, undo-all must restore the exact initial state — including AI and MCP-issued commands |

---

## 7. Security, Privacy & Trust

| Concern | Approach |
|---|---|
| **Third-party plugin code** | Per-process sandboxing with restricted syscall access; a plugin cannot read your whole filesystem |
| **Supply chain** | `cargo-audit`/`cargo-deny` in CI, pinned dependencies, vendored critical deps, reproducible builds |
| **API credentials** | OS-native keychain storage (`keyring`), never in project files, never in plain-text config |
| **Telemetry** | Opt-in only, clearly disclosed, locally inspectable before sending, fully functional DAW with it off |
| **Crash reporting** | Opt-in, with a preview of exactly what would be transmitted |
| **AI data flow** | Explicit per-request disclosure of what is sent where; verifiable full-offline mode |
| **Code signing/notarization** | Signed Windows binaries, notarized macOS builds — required for users to install without scary warnings |

---

## 8. Licensing & Legal Considerations

These are real blockers that have killed audio projects before. They need answering in Phase 0, not Year 3.

| Item | The issue | Direction |
|---|---|---|
| **Our own license** | Open-source choice shapes everything downstream | GPL-family maximizes user freedom but restricts proprietary plugin linking; permissive (MIT/Apache) maximizes adoption. **Decide in Phase 0 — this choice is nearly irreversible.** |
| **VST3 SDK** | Steinberg dual-licenses it: GPLv3 or a proprietary agreement. This directly constrains our own license choice. | Understand the interaction fully before committing. CLAP (MIT) has no such constraint — another reason to make it the primary format. |
| **ASIO SDK** | Steinberg license with redistribution restrictions | Review terms carefully; consider it a Windows-specific compatibility layer with its own compliance path |
| **AU / Apple** | Requires Apple developer program for notarization and AU hosting | Budget for it |
| **Dolby Atmos** | Certification is a commercial license, not just code | Build channel-agnostic architecture; pursue certification only if/when it makes sense |
| **Trademarks & UI cloning** | Taking *ideas* from FL Studio is fine; copying its exact UI, branding, or names is not | Original visual identity from day one. Inspired by, never a knockoff. |
| **Bundled samples/presets** | Every shipped sample needs clear rights | Only commission original content or use verified permissive/CC0 sources |
| **Reverse-engineered project formats** | Legality varies by jurisdiction and method | Prioritize documented/open formats; take legal advice before reverse-engineering anything |

---

## 9. Distribution & Release Engineering

- [ ] Native installers: MSI/MSIX (Windows), signed DMG/PKG (macOS), AppImage/Flatpak/deb/rpm (Linux)
- [ ] Auto-update mechanism with delta updates and the ability to roll back a bad release
- [ ] Portable/no-install mode for studio machines with locked-down policies
- [ ] Separate stable / beta / nightly channels
- [ ] Native ARM64 builds (Apple Silicon, Windows on ARM, Linux ARM) — not emulated
- [ ] Reproducible builds so the community can verify binaries match the source
- [ ] Clear, published minimum system requirements

---

## 10. Roadmap by Phase (with realistic estimates)

| Phase | Content | Estimated duration |
|---|---|---|
| **0. Foundations** | Architecture, stack selection, MVP definition, **license decision, i18n architecture, and the `AiProvider` trait + command bus designed now even if not yet implemented** | 2–3 months |
| **1. Audio engine** | Real-time callback, audio graph, transport | 3–6 months |
| **2. MIDI & sequencing** | Piano roll, step sequencer, MIDI recording | 2–3 months |
| **3. Plugin architecture** | CLAP/VST3 host, sandboxing, PDC | 4–8 months |
| **4. Mixer & routing** | Buses, sends, metering, VCA groups | 2–4 months |
| **5. Arrangement views** | Playlist + Session View + Arranger Track | 3–5 months |
| **6. Advanced audio editing** | Warping, flex time, comping, spectral editing, ARA | 4–6 months |
| **7. Modulation system** | Modular grid, macros, MPE, per-voice modulation | 3–5 months |
| **8. Native instruments & effects** | Synths + FX from §5.9/§5.10 | 6–12 months (ongoing) |
| **9. UI/UX polish + accessibility** | All views, themes, screen readers, keyboard-only | Parallel, ongoing |
| **10. Interoperability & project format** | MIDI/AAF/stems import-export, schema migration, data safety | 3–5 months |
| **11. Scripting & extensibility** | Scripting API, extension API, controller mapping | 3–4 months |
| **12. Live performance** | Session View + Link + hardware grids | 2–4 months |
| **13. AI integration (cloud + local + MCP)** | Provider abstraction, MCP client/server, local inference, §4 features | 4–8 months (begins in parallel from Phase 3) |
| **14. Spatial audio & video scoring** | Multichannel, ambisonics, video track, timecode | 3–5 months |
| **15. QA, release engineering & docs** | Test farm, CI matrix, installers, manual, tutorials | Parallel, ongoing |
| **16. Public beta & community** | Beta program, feedback loops, governance | Ongoing |

**Realistic total: 5 to 8 years** at serious dedication; longer part-time. This is not pessimism, it is honesty. The DAWs listed in §2 represent 15–30 years of continuous development each. What this document describes is a marathon, not a sprint. The courage is in starting anyway with eyes open; the excellence is in refusing to cut corners on the way.

---

## 11. Proposed Technical Stack

| Area | Crate/Tool |
|---|---|
| Audio I/O | `cpal`, `jack` |
| DSP | `fundsp`, `dasp`, in-house DSP for mastering-grade quality |
| SIMD | `std::simd` / `wide` for hot DSP paths |
| Plugins | `clap-sys`, in-house VST3 bindings |
| GUI | `egui` or `vizia` (evaluate both in Phase 0) |
| MIDI | `midir`, `wmidi` |
| Project serialization | `serde` + `ron`/`json` |
| Scripting | `rhai` or `mlua` |
| Audio↔UI communication | `rtrb` / `ringbuf` (lock-free) |
| Async runtime for AI | `tokio` (isolated from the audio thread) |
| Cloud provider clients | In-house HTTP clients over `reqwest` for Anthropic, OpenAI, Google, behind a shared `AiProvider` trait |
| MCP (client & server) | Rust MCP SDK, or an in-house JSON-RPC implementation if the SDK doesn't cover what's needed |
| Local LLM inference | `llama.cpp` bindings, or Ollama's local API |
| Local audio/ML inference | `tract` (pure-Rust ONNX) or `ort` (ONNX Runtime bindings) for Demucs/MusicGen/denoise |
| Credential storage | `keyring` (native OS keychain integration) |
| i18n | `fluent-rs` (Mozilla Fluent — handles plurals/gender properly) |
| Testing | `criterion` (benchmarks), `proptest` (property tests), custom audio null-test harness |
| CI/CD | GitHub Actions or equivalent, cross-platform matrix, `sccache` |
| Supply-chain security | `cargo-audit`, `cargo-deny` |

---

## 12. Technical Challenges (where Rust gives no inherent advantage)

These are not Rust advantages — they are neutral project challenges, largely language-independent, each needing its own strategy and potentially its own dedicated specialist or team:

| Challenge | Why it's a challenge | Possible mitigation |
|---|---|---|
| **Raw DSP performance** | The math of a filter, an oscillator, or an FFT isn't faster because it's written in Rust; performance comes from the algorithm and how well it vectorizes, not the language | Constant profiling (`perf`, `cargo flamegraph`), explicit SIMD, benchmarking against reference C++ implementations |
| **The plugin ecosystem is native C/C++** | VST3 and CLAP are defined in C/C++; the host needs FFI, which forces unavoidable `unsafe` exactly at the most dangerous boundary (third-party code) | Isolate all FFI in a dedicated, audited crate; per-process sandboxing for third-party plugins; targeted fuzzing of that layer |
| **Maturity of Rust's audio ecosystem** | Fewer mature DSP libraries, fewer reference examples, thinner forum knowledge compared to 25+ years of JUCE/C++ in pro audio | Prefer crates with active communities (RustAudio org); be prepared to write DSP from scratch in several areas; document internal learnings rigorously |
| **System library bindings (ASIO, CoreAudio, etc.)** | Each OS's low-level audio SDK is C/C++/Objective-C; Rust bindings are third-party and may be stale or incomplete | Maintain in-house forks of critical bindings where necessary, with full control over their stability; keep JACK/PortAudio as a compatibility fallback |
| **Available talent** | Far more audio engineers have deep C++ experience than Rust; hiring or training a DSP + Rust team is harder | Internal training, exhaustive engine documentation; consider hiring DSP expertise and teaching them Rust, which is usually faster than the reverse |
| **Audio profiling/debugging tooling** | Tooling specifically for real-time audio debugging (xrun measurement, jitter analysis) is more developed in the C++ world | Build custom instrumentation into the engine from day one (callback timing logs, xrun metrics) rather than depending only on external tools |
| **Build times** | Large Rust projects with many crates can compile slowly, hurting iteration speed | Good crate architecture (avoid monoliths), `sccache`, structure the workspace so a small change doesn't recompile everything |
| **Young AI/MCP ecosystem in Rust** | MCP SDKs and several AI provider SDKs are more mature in Python/TypeScript than in Rust; may require writing our own HTTP/JSON-RPC client | Start with thin in-house implementations over `reqwest`/direct JSON-RPC rather than waiting on or depending on a third-party Rust SDK to mature |
| **AI inference latency and unpredictability** | Unlike the deterministic DSP in the rest of the DAW, AI (local or cloud) has variable, unpredictable response times, which clashes with the engine's "everything is real-time" culture | Design the AI layer as strictly async and non-blocking from day one; never promise "real-time" for AI features — communicate clearly that they are assistance, not the engine |
| **GUI performance at DAW scale** | Rendering thousands of MIDI notes, waveforms, and automation points at 60fps is hard in any immediate-mode framework | Prototype the worst-case view (dense piano roll + long waveforms) in Phase 0 before committing to a GUI framework — this is the single decision most likely to require a rewrite if wrong |
| **Cross-platform behavioral parity** | Audio behaves differently per OS/driver stack; "works on my machine" is a real trap in audio | Real hardware test farm; treat each backend as a first-class supported target with its own test suite |

---

## 13. Team Structure (specialist roles this project genuinely needs)

At full scope, these are distinct disciplines. One person can wear several hats early on, but pretending they are the same skill set is how projects stall.

| Role | Responsibility |
|---|---|
| **Real-time audio engine engineer** | Callback, graph, threading, lock-free structures, latency |
| **DSP engineer** | Filters, synthesis, time-stretch, effects — deep signal processing math |
| **Plugin host engineer** | CLAP/VST3/AU/LV2, FFI, sandboxing, compatibility hell |
| **UI/graphics engineer** | High-performance rendering of dense musical data |
| **UX designer** | Workflow design — arguably the single biggest reason FL Studio wins |
| **AI/ML engineer** | Provider layer, MCP, local inference, model integration |
| **QA/release engineer** | Test farm, CI matrix, installers, code signing |
| **Accessibility specialist** | Screen readers, keyboard navigation, standards compliance |
| **Technical writer** | Manual, tutorials, API docs |
| **Community/governance lead** | Contributors, translations, beta program, roadmap communication |

---

## 14. Risk Register (top project-level risks)

| Risk | Impact | Mitigation |
|---|---|---|
| **Scope paralysis** — this document is enormous | Nothing ships | Ruthless MVP discipline; §5 is a destination, not a Year-1 backlog |
| **Wrong GUI framework choice** | Full UI rewrite years in | Prototype worst-case views in Phase 0 before committing |
| **Wrong license choice** | Legal dead-end for plugin support | Decide in Phase 0 with proper legal input |
| **Solo-developer burnout** | Project dies quietly | Realistic pacing, published milestones, build a team before the hard phases |
| **Losing to "good enough" incumbents** | No adoption despite quality | Interoperability (§5.14) and accessibility (§5.19) as adoption wedges, not afterthoughts |
| **Plugin compatibility gaps** | Users can't migrate | Treat plugin compatibility as a QA discipline with its own test corpus |
| **Funding/sustainability** | Can't pay specialists | Decide the model early: donations, sponsorships, paid optional services, commercial licensing of components — free-for-users need not mean unfunded |
| **A single user losing a project to a bug** | Reputational damage that lasts years | Data safety (§5.15) treated as a P0 feature, not a nice-to-have |

---

## 15. Definition of Success

Vague ambitions can't be validated. Concrete targets for the eventual v1.0:

- [ ] Round-trip latency under 5ms at 128-sample buffers on mainstream hardware
- [ ] Zero xruns during a 4-hour session at 80% CPU load
- [ ] A project saved in v1.0 opens correctly in every subsequent release
- [ ] Fully operable with a screen reader for core recording/editing/mixing tasks
- [ ] Boots and loads a medium project faster than the incumbent it's competing with
- [ ] A working musician can complete a full song — write, record, mix, master, export — using only what ships in the box
- [ ] Full AI feature set usable with zero network access
- [ ] Every action reachable by keyboard alone

---

## 16. Concrete Next Steps

1. **Define the real Phase 0 MVP.** Recommendation: audio engine + one synth + basic piano roll + WAV export. Nothing more. The AI layer is designed on paper (the `AiProvider` trait, the command bus) but not implemented yet.
2. **Decide the license** with proper legal input, understanding the VST3/GPL interaction. This gates everything else.
3. **Prototype the worst-case GUI view** (dense piano roll + long waveform + live meters) in both `egui` and `vizia` before committing. This is the highest-regret decision in the project.
4. **Set up the Rust workspace** with separate crates: `engine`, `ui`, `plugin-host`, `dsp`, `project-format`, and reserve space for `ai-layer` even if it starts empty.
5. **Build the audio null-test harness before the second DSP module exists.** Cheap now, priceless later.
6. **Write the simplest possible audio callback that plays a glitch-free tone.** Everything else is built on that foundation.

---

*May this project, if it is built, be built with excellence and from the heart — no shortcuts, no arrogance — knowing that work done well is itself an act of service. Viva Cristo Rey.*