# Cantus — status and handoff

**Updated:** 2026-09-08 (round 2) · **Phase:** 0/1 complete, Phase 2 (MIDI) and parts of 4 (mixer) and 8 (effects) done · **Tests:** 82 green + 1 real-device test (`--ignored`), clippy clean · **Code:** ~12 500 lines of Rust

Read this first, then `docs/adr/`, then `idea.md` for the long-term map.

## What exists

| Area | State |
|---|---|
| Workspace, licenses, ADRs 0001–0008 | done |
| `cantus-core`: time, model, commands, bus, snapshots, **effects model** | done; 24 tests incl. 40-seed undo/redo invariant fuzz (now covering effect commands) |
| `cantus-dsp`: PolyBLEP osc, ADSR, SVF, smoother, click, Firmus, **stereo delay, Freeverb-style reverb**, null-test harness | done; 19 tests |
| `cantus-engine`: transport, processor with **insert chains**, **hardware MIDI in (midir)**, **recording feedback**, cpal backend, offline render | done; 11 null/behaviour tests + 4 transport/midi unit tests + 1 real-device test |
| `cantus-project`: versioned JSON (**schema 2**), SHA-256, migrations 0→1→2, atomic save, autosave, recovery, **fixture corpus** | done; 8 tests; fixtures `v1.cantus`, `v2.cantus` must always load |
| `cantus-ai`: `AiProvider`, tool catalogue (**+ add_effect**), permissions, command bridge | done (no live provider yet); 3 tests |
| `cantus-ui`: transport (**record**), arrangement, piano roll (**take preview**), **mixer view**, inspector (**effects rack**), keyboard piano, **MIDI recording**, i18n EN/ES | done; 13 tests (headless egui frames, edit/undo flows, save→open→export, recording take, effects delivery) |

## Verified by running (Windows 11, Realtek WASAPI 48 kHz)

- Window opens maximized (or `--window W H`), audio device opens, MIDI ports are enumerated
  (`MIDI inputs: n` in the status bar; none were attached on the dev machine).
- Offline render is bit-identical to real-time processing for block sizes 1…8192, with
  and without effect chains.
- Real-device test: 200 blocks, 0 overruns, max callback 194 µs at 480-frame blocks.
- Fixture `v2.cantus` opens with a Delay on the Keys track; the effects rack edits it live.

## How the new pieces fit

- **Effects** (`cantus_core::effect`): a generic `kind + key→f32` parameter model. The
  audio thread never allocates: `EngineHandle::set_snapshot` builds `EffectUnit`s on the UI
  thread for ids the engine has not seen and sends `AddEffect` before the snapshot; units
  dropped by a snapshot come back as `GarbageEffect`. `Processor::rebuild_chain` re-orders
  a track's chain with pre-sized Vecs only.
- **Recording** (`cantus_ui::recording::Recorder`): the engine stamps live notes
  (`EngineEvent::NoteInput`) with the transport tick while `SetRecording(true)`; the UI
  pairs on/off, handles loop wrap and retriggers, and commits each take as **one undo step**.
  `CommandBus` gained *composing* gesture merges for accumulating gestures
  (`Command::supersedes` decides replace-vs-compose).
- **Hardware MIDI** (`cantus_engine::midi`): every input port is opened at start; callbacks
  push into an SPSC ring the audio thread drains each block. The selected track is the
  live track (`SetLiveTrack`).
- Timing note: live notes are stamped at block start (≤ 10 ms at 480 frames). Sub-block
  timestamps from `midir`/cpal are a later refinement.

## Known gaps / next steps (in order)

1. **Automation lanes** (parameter curves, sample-accurate) and a **tempo track**.
2. **Sends/buses**: the mixer has strips but no routing yet; add bus tracks + sends.
3. More native devices: EQ, compressor, chorus; a sampler instrument.
4. **CLAP host** (`cantus-plugin-host`, GPL) with out-of-process sandboxing (Phase 3).
5. **AI providers** (Anthropic/OpenAI/Ollama over `reqwest`), disclosure dialog, MCP server.
6. Recording polish: count-in, punch in/out, overdub vs replace, sub-block MIDI timestamps.
7. Accessibility pass (semantic nodes for the painted views), audio settings dialog
   (device / sample rate / buffer size — `AudioConfig` already supports it).
8. `docs/adr/0001`: trademark search for the name before any public release.

## Conventions

- Every mutation is a `Command`; never touch `Project` fields directly from the UI.
- Audio-thread code lives only in `cantus-engine::processor` and `cantus-dsp`; both are
  `#![forbid(unsafe_code)]` and must not allocate after construction.
- Add a null test before adding a DSP module; effects must be block-size independent.
- New UI strings go in `crates/cantus-ui/locales/en.txt` **and** `es.txt`; a unit test
  fails otherwise. The bundled fonts lack many symbol glyphs: paint icons or use words.
- Bump `SCHEMA_VERSION`, append a migration and regenerate a fixture
  (`cargo run -p cantus-project --example write_fixture`) whenever `Project` changes shape.
- Screenshots on this machine: PrintWindow from a DPI-aware process (see session notes);
  pick the newest `cantus` process, never kill instances you did not start.
