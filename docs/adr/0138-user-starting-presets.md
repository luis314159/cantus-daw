# ADR 0138 — The user chooses each unit's starting preset

**Status:** accepted by the owner (2026-11-15), provisional number (fx side round, wave 3 —
renumbered at the merge after round 40) · **Relates to:** ADR 0071 (effects that sound on
insert), ADR 0101 (presets), ADR 0134 (Tabula), ADR 0135 (Saltus)

## Context

ADR 0071 gave every unit a factory starting preset, so an insert is heard at once, and one
global switch (*Insert effects at their neutral settings*) to turn that off. Saltus has two
first-class engines (mono at 2.7 ms for playing live, poly at 85 ms for chords), and the
owner asked that its start be "configurable poly and mono" — and, seeing that Cantus had no
way to choose a unit's start, to build it for every unit (2026-11-15).

## Decision

- **Every unit's preset menu** (the device frame's, so every native effect and Tabula's
  board) offers *Use as starting preset*: the unit's settings as they are now become what
  its new inserts start on — named after the preset loaded when nothing has moved since,
  else "Your settings". When a choice exists the menu says which (*Inserts start on: …*)
  and offers *Reset to factory starting preset*. Only where the host is Cantus: a plug-in
  window inside another DAW, and a hosted plug-in, offer neither.
- **A user setting, not a project's**: `Settings::starting` (`settings.json`), a list of
  the chosen whole specs (values, and a board's pedals or a bank where the unit has them)
  with their names. No schema change: a song keeps the values it inserted, so it opens the
  same on any machine.
- **One place every insert asks**: `cantus_core::starting_user` holds the choices for the
  process (set from the settings at `assemble` and on every change); `starting::apply`,
  and so `starting_spec`, takes the person's choice before the factory's. That covers the
  picker, the browser, a drag, a swap, Tabula's *Add pedal*, the agents' `add_effect` and
  `swap_effect`, and a drum pad's chain. A program that never sets choices (the exported
  plug-ins, the engine's tests) sees the factory's.
- **The neutral switch keeps its rule**: a neutral insert takes no starting preset at all,
  the person's or the factory's (the paths that already honoured it still do; Tabula's *Add
  pedal* and a pad's chain always start a unit on its starting preset, as before).

## Consequences

- A preset changed or deleted later does not change a chosen start: the start keeps the
  values it was chosen with (it is a copy, not a reference).
- The choice is per unit kind, for the whole program; per-project starts were not asked for.
- The AI tools' unit list still describes the factory's starting preset.

## What the tests hold

- `cantus-core/tests/starting_user.rs`: a chosen start is `apply`'s and `starting_spec`'s,
  another unit keeps its factory start, a board can be given one, a hosted plug-in cannot,
  a reset brings back the factory's, the choices at start replace the old ones.
- `cantus-ui/tests/starting_choice_e2e.rs` (on the boot path): a `settings.json` naming
  Saltus's start as the mono *Fifth up, live* is read by `CantusApp::assemble`; Saltus
  inserted by the picker's path, by Tabula's *Add pedal* and by an agent's `add_effect`
  starts there and is heard; a board's start chosen from its settings is the next board's;
  the choice is in the settings file; *Reset* brings back the poly twelve-string; the
  neutral switch inserts at the defaults.
