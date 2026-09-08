# ADR 0002 — Workspace layout and dependency direction

**Status:** Accepted · **Date:** 2026-09-07

```
cantus (bin)  →  cantus-ui  →  cantus-engine  →  cantus-dsp
                     │              │
                     ├→ cantus-project ─┐
                     ├→ cantus-ai ──────┤
                     └──────────────────┴→ cantus-core
```

| Crate | Owns | May NOT depend on |
|---|---|---|
| `cantus-core` | musical time, ids, project model, `Command`, undo/redo, playback snapshot compilation | anything internal; no threads, no I/O |
| `cantus-dsp` | oscillators, envelopes, filters, instruments, null-test helpers | anything internal (pure math) |
| `cantus-engine` | audio thread, transport, sequencer, voice allocation, lock-free messaging, cpal backend, offline render | ui, ai, project |
| `cantus-project` | serialization, schema versioning, migrations, atomic save | engine, ui |
| `cantus-ai` | `AiProvider` trait, tool schema, command bridge | engine, ui |
| `cantus-ui` | egui views | — |

Rationale: build times (a UI change must not recompile DSP), license boundaries (ADR 0001),
and the golden rule that the engine is a library usable headless (tests, CLI render, servers).
