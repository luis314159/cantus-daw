# ADR 0003 — GUI framework: egui

**Status:** Accepted · re-confirmed by [ADR 0011](0011-gui-toolkit-re-evaluated.md), which measured the guardrail below for the first time · **Date:** 2026-09-07

## Options considered
- **egui/eframe** — immediate mode, very mature, huge community, accesskit integration
  (screen readers) built in, custom painting via `Painter` is cheap, wgpu/glow backends.
- **vizia** — retained mode, designed for audio (Meadowlark), but smaller community,
  slower release cadence, thinner accessibility story.
- **iced** — retained, Elm-style; canvas-heavy DAW views fight the architecture.

## Decision
`egui` 0.36 via `eframe` (glow backend). All dense views (piano roll, arrangement, meters)
are drawn with `Painter` primitives on culled visible ranges — never one widget per note.

## Guardrail (from idea.md §12)
The piano roll must keep 60 fps with 20 000 visible notes. `cantus-ui` ships a
stress mode (`--stress`), and `stress_piano_roll_frame_cost` measures it. **Note:** until
ADR 0011 that test ran at a zoom where culling left only ~139 notes on screen, so it was
not checking this at all. Measured properly it now stands at 6.9 ms for 20 000 notes.

## Consequences
- Every UI string goes through `i18n::t()` (ADR 0007).
- The UI never holds engine state; it holds the `Project` + receives `EngineEvent`s.
