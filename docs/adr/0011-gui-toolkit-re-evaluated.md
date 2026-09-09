# ADR 0011 — GUI toolkit re-evaluated: egui stays, because the bottleneck was ours

**Status:** Accepted · **Date:** 2026-09-09 · **Extends:** ADR 0003

## Context

The question was asked directly: performance is the first priority and UI/UX the second, so
would Slint be a better foundation than egui — and is it worth rewriting `cantus-ui` now,
while it is still 9 000 lines, rather than later?

ADR 0003 chose egui and named the condition for revisiting: *"the piano roll must keep
60 fps with 20 000 visible notes"*. The honest answer was that **nobody knew whether we
met it**, because the test that claimed to check it did not:

```
stress: worst frame 11.50 ms, notes drawn 139
```

139 notes, not 20 000. The stress project has 20 000 notes per clip, but at the default
zoom culling means only a couple of hundred are ever on screen, and the ceiling (250 ms)
was loose enough to pass regardless. The criterion for changing GUI toolkits was not being
measured. So it was measured first.

## What the measurement showed

Zoomed out until all 20 000 notes were on screen, and counting tessellation (`run_ui` only
builds the shape list; turning shapes into triangles is where the CPU goes):

| | Frame | Triangles | Per note |
|---|---|---|---|
| Before | **74.7 ms** (13 fps) | 416 772 | 20.8 |
| After | **6.9 ms** (146 fps) | 96 772 | 4.8 |

Two causes, both ours:

1. **No level of detail.** Each note was drawn as a rounded rectangle plus a stroked
   outline, at every zoom. Zoomed out a note is a fraction of a pixel wide, so we were
   tessellating corners and a 1-px outline onto something invisible.
2. **Anti-aliasing on shapes that did not need it.** epaint feathers every filled shape
   into a ring — roughly twenty triangles for a rectangle. Fine for a button; ruinous
   twenty thousand times a frame.

The fix is about forty lines: `NoteLod` decides what a note is worth at its size on screen,
and everything below the thresholds goes into a single `egui::Mesh` — two triangles each,
one shape for the lot, and the tessellator has almost nothing left to do. The velocity lane
got the same treatment. Tessellation went from 60.4 ms to 2.9 ms.

## Decision

**Stay with egui.** The evidence says the toolkit was never the constraint.

- **A DAW's screen is canvases, not widgets.** Piano roll, arrangement, meters, and later
  automation, waveforms and spectra are all custom-painted. That work is the same in any
  toolkit, and it is where the frames go.
- **The escape hatch we needed is first-class in egui.** `Shape::Mesh` let us bypass the
  convenience API for the dense path without giving up the widgets, theming, focus rings,
  layouts, keymap and settings already built on it. In Slint the equivalent dense path
  would mean rendering to a texture or writing a native item — the same work, with less of
  the surrounding machinery already done.
- **The remaining headroom is large.** 6.9 ms for 20 000 notes leaves the rest of the frame
  for everything else, and the same batching applies to the arrangement and future
  automation lanes when they need it.

Rewriting `cantus-ui` would have cost roughly 4 500–5 000 lines rewritten and ~4 000
reworked, plus the 50 UI tests' harness — to fix a problem that turned out to be a missing
`if` in our own drawing code.

## Where Slint would still be the better answer

Recorded so this is a decision and not a preference:

- A dedicated designer iterating the UI without writing Rust. Slint's markup and live
  preview are genuinely better for that; we have no designer.
- Complex text input and IME, where immediate mode is weaker.
- If egui's accessibility (AccessKit) proves insufficient in the pass we still owe.

Licensing also favours egui: MIT/Apache against Slint's GPLv3 / royalty-free-with-conditions
/ paid. GPL would be compatible with `cantus-ui` today, but it adds commercial terms to the
product's UI that MIT/Apache does not.

## Consequences

- ADR 0003's guardrail is now genuinely measured, in
  `stress_piano_roll_frame_cost`, at both the widest zoom a user can reach and the 20 000
  note case. The ceiling is per build profile because debug is several times slower.
- **Dense views must not use the convenience painters per item.** Anything drawn once per
  note, per clip, per sample or per automation point goes into a `Mesh`, and gets a level
  of detail rule. This is now a convention, not an optimisation to remember.
- We revisit this ADR if the guardrail fails again for reasons that are *not* ours, or if
  any of the three cases above becomes real.
