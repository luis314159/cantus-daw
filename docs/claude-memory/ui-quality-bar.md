---
name: ui-quality-bar
description: "User judged Cantus's UI ugly and plain on 2026-09-25 — the app, plug-ins and instruments must be redesigned to the level of the best DAWs, FL Studio as the aesthetic reference"
metadata:
  node_type: memory
  type: feedback
  originSessionId: ab6ce7df-a46e-4ebb-a229-b046ae37783d
  modified: 2026-09-25T22:29:18.412Z
---

On 2026-09-25, after trying the build, the user said the application, our effect plug-ins,
and (they suspected) the instruments look "super ugly, plain". Their specific complaints:

- The mixer's side panel (the inspector) looks very, very ugly.
- Letters sometimes run into each other.
- Labels read "like loose words thrown down".
- Interface elements should be resizable by dragging their edge, and movable within the
  interface.
- The mixer can't be scrolled down easily.
- Many small details make it hard to move around quickly.

They want the whole style rethought to the level of the best DAWs, researched with
parallel agents. "FL Studio is beautiful."

The same day they added: "the style I like most, by far, is FL Studio, but take even more
cool things from the others — let's get inspired and make something super pro". So FL
Studio is the **primary** visual reference. Ableton, Bitwig, Logic, Studio One and the
top plug-in/instrument UIs supply specific ideas on top of it (e.g. Bitwig's two-column
inspector, section toggles instead of vertical scroll, a hover status line).

They then shared research on how FL is built: Delphi, and its own vector renderer since
FL 12, so it is crisp at 4K and identical on every OS. They asked: "make it as excellent
as FL; it doesn't matter if we vectorize components by hand — it must look excellent and
have incredible performance". Hand-drawn vector components in egui/epaint are the path:
gradients, bevels, shadows, vector icons, a UI zoom, with a measured frame-time budget.

**Logos and splash (2026-09-25).** The user designed ten logo variants in
`docs/icons/cantus—Iconos.html`. It is a bundler export: each page's template holds one
SVG with `sc-camel-*` attribute names. They want these:
- drawn natively in vectors, as the app draws;
- **selectable** as the app's logo;
- **tinted by the configured interface colour**;
- **#2 "C de onda con playhead-cruz" kept as it is by default**;
- shown on a **splash while loading, like FL Studio**.

They also said: "let's make something as hardcore as FL Studio, almost from the machine's
bare metal, but in Rust". Performance and craft come from the ground up.

**Why:** the functional depth is there; the look and the feel of moving around decide
whether it feels like a first-rank product.

**How to apply:**
- Treat visual quality and navigation speed as first-class requirements, not polish.
- Judge any UI change by rendering it (`examples/shoot`) and comparing it with the
  references in `research/ui/` (FL Studio, Ableton, Bitwig, Logic/Studio One, pro
  plug-in and instrument UIs) and with `research/ui/cantus-audit.md`.
- Typography needs clear label/value units and hierarchy; no stray words.
- Panels need to be resizable and movable.
- Every scrollable area must scroll in both directions where content can overflow.

The redesign ADR is shared with the instruments session (`daw-b0`), which moves the
instrument frame onto it. See [[mixer-direction-fl-parity]] and [[cantus-workflow-habits]].
