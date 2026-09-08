# ADR 0003 — GUI framework: egui

**Status:** Accepted (revisit if the Phase-0 stress view fails) · **Date:** 2026-09-07

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
stress mode (`--stress`) to measure this continuously.

## Consequences
- Every UI string goes through `i18n::t()` (ADR 0007).
- The UI never holds engine state; it holds the `Project` + receives `EngineEvent`s.
