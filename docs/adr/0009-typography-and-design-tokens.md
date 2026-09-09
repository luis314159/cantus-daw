# ADR 0009 — Bundled typography and the design system

**Status:** Accepted · **Date:** 2026-09-08

## Context

Until now the UI used whatever fonts `epaint` ships (Ubuntu-Light and Hack) and picked
sizes and paddings ad hoc at each call site — `.size(15.0)` here, `add_space(6.0)` there.
Two problems follow from that:

- **The look is not ours and not stable.** egui's defaults are a sensible starting point,
  not an identity, and they change between egui releases. A DAW is looked at for eight
  hours at a stretch; its type is not a detail.
- **Numbers jitter.** Bar/beat/tick counters, dB values and timers were drawn in a
  proportional face, so digits changed width as they counted and the readouts shifted
  horizontally while the transport ran.

We also had no way to show keyboard focus. egui's `Widgets::style` returns the *active*
visuals for a focused widget, so tabbing to a button makes it look pressed, and there is
no ring — keyboard navigation was effectively invisible (idea.md §12, accessibility).

## Decision

**1. Bundle IBM Plex Sans + IBM Plex Mono** (`crates/cantus-ui/assets/fonts/`, four faces:
Sans Regular, Sans SemiBold, Mono Regular, Mono Medium; ~750 KB total).

- A *designed superfamily*: the sans and the mono share metrics and voice, so a label and
  the number next to it sit together instead of colliding.
- Slightly humanist rather than neutral-grotesque, which suits "Ink & Gold" and does not
  make Cantus look like every other Inter-based application.
- Ships the full Latin/Greek/Cyrillic charset, so new locales (ADR 0007) need no new font.
- Bundling rather than using system fonts means the UI is pixel-identical on every
  platform, and screenshots and layout tests are reproducible.

**2. Every number in the UI is monospaced.** Positions, tempo, dB, hertz, milliseconds and
frame timings use `design::readout`, `readout_small`, `mono_small` or `design::paint::ruler`.

**3. Form lives in code, colour lives in the user's file.** `theme.rs` owns colour and is
serialized to `theme.json` for the user to edit. `design.rs` owns the type scale, the 4-px
spacing grid, elevation and focus, and is not user-editable. Layout that users *can*
change (panel sizes, named layouts) is a separate concern and does not belong in either.

**4. A global focus ring.** `design::paint_focus_ring` runs once at the end of each frame,
reads the focused widget's id from egui's memory, and strokes a ring on a foreground layer.
One implementation covers every widget in the application, including ones added later, with
no changes at any call site.

## Licensing

IBM Plex is **SIL Open Font License 1.1** (`assets/fonts/LICENSE-OFL.txt`). The OFL permits
bundling and redistribution inside both the permissive crates and the GPL application
(ADR 0001), and imposes two obligations we must keep:

- The licence file ships with the fonts and is not removed.
- "IBM Plex" is a **Reserved Font Name**: we ship the faces unmodified. If we ever subset or
  otherwise modify them, the result must be renamed.

This does not conflict with ADR 0001's "bundled samples and presets must be original or
CC0" — that rule is about *musical content*, where CC0 avoids attribution obligations
travelling with a user's exported track. A UI font is part of the application, not of the
user's output.

## Consequences

- New UI code must use `design::space` constants and the named text styles; the unit tests
  assert the spacing lands on the grid and that every named style is installed.
- egui **panics** on a text style it cannot resolve, so any `egui::Context` that renders a
  Cantus view must have had `design::install_style` called on it. The headless test harness
  does this exactly as `CantusApp::new` does.
- Adding a colour token means adding it to `Palette`, to `Theme` with `#[serde(default)]`,
  and to `Theme::fill_missing`, so that theme files written by older versions keep loading
  *and* get the new token written back for the user to edit.
- Binary size grows by ~750 KB. Accepted: it buys a stable identity and reproducible
  rendering. If it ever matters, subsetting is possible — under a different font name.
