# Cantus — status and handoff

**Updated:** 2026-09-08 · **Phase:** 0/1 complete (MVP per ADR 0008) · **Tests:** 60 green + 1 real-device test (`--ignored`), clippy clean

Read this first, then `docs/adr/`, then `idea.md` for the long-term map.

## What exists

| Area | State |
|---|---|
| Workspace, licenses, ADRs 0001–0008 | done |
| `cantus-core`: time, model, commands, bus, snapshots | done; 21 tests incl. 40-seed undo/redo invariant fuzz |
| `cantus-dsp`: PolyBLEP osc, ADSR, SVF, smoother, click, Firmus, null-test harness | done; 13 tests incl. aliasing measurement |
| `cantus-engine`: transport, processor, cpal backend, offline render | done; 6 null tests + 3 transport tests + 1 real-device test (`--ignored`) |
| `cantus-project`: versioned JSON, SHA-256, migrations, atomic save, autosave, recovery | done; 7 tests |
| `cantus-ai`: `AiProvider`, tool catalogue, permissions, command bridge | done (no live provider yet); 3 tests |
| `cantus-ui`: transport, arrangement, piano roll, inspector, keyboard piano, i18n EN/ES | done; 7 tests (headless egui frames, edit/undo flows, save→open→export); verified on Windows 11 (WASAPI 48 kHz) |

## Verified by running

- Window opens maximized, audio device opens (`Speakers (Realtek(R) Audio) @ 48000 Hz`).
- Offline render is bit-identical to real-time processing for block sizes 1…8192.
- Loop wraps without drift; seek restarts deterministically; snapshot swaps mid-playback
  return the old snapshot to the UI thread (no free on the audio thread).

## Known gaps / next steps (in order)

1. **Recording**: MIDI input via `midir`, record into the selected clip (Phase 2).
2. **Automation lanes** and a tempo track (Phase 2/4).
3. **Sends/buses** and a proper mixer view; the mixer currently lives in the track headers.
4. **CLAP host** (`cantus-plugin-host`, GPL) with out-of-process sandboxing (Phase 3).
5. **AI providers**: Anthropic/OpenAI/Ollama implementations of `AiProvider` over
   `reqwest`, the disclosure dialog, and the DAW-as-MCP-server (Phase 13).
6. **Accessibility pass**: accesskit is compiled in; the custom-painted views need
   semantic nodes for screen readers (idea.md §5.19).
7. Stress view: `--stress` loads 8 tracks × 20 000 notes. Measure with *View → Engine
   statistics* (`ui … ms`, `notes drawn`) and keep the piano roll above 60 fps.
8. `docs/adr/0001`: trademark search for the name before any public release.

## Conventions

- Every mutation is a `Command`; never touch `Project` fields directly from the UI.
- Audio-thread code lives only in `cantus-engine::processor` and `cantus-dsp`; both are
  `#![forbid(unsafe_code)]` and must not allocate after construction.
- Add a null test before adding a DSP module.
- New UI strings go in `crates/cantus-ui/locales/en.txt` **and** `es.txt`; a unit test
  fails otherwise.
- Bump `SCHEMA_VERSION` and append a migration whenever `Project` changes shape.
