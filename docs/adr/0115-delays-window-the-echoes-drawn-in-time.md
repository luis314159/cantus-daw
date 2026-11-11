# ADR 0115 — Delay's window: the echoes drawn in time

**Status:** accepted (round 39) · **Date:** 2026-11-10 · **Extends:** ADR 0074 (a face of its
own for each unit), ADR 0017/0020 (the plug-in family's frame and host), ADR 0101 (presets),
ADR 0106 (at rest costs nothing)

## Context

Delay (`EffectKind::Delay`, `cantus_dsp::delay::StereoDelay`) was the only effect on the
website and in the README with no window of its own. It was edited only under its insert in
the inspector — a row per parameter (round 37) — and its card's pictures were a crop of that
(`shoot_delay`). The other 24 effects each have an editor window in a face of their own (ADR
0074 §6 listed "the legacy delay and reverb" as what keeps the plain look). The owner
approved giving Delay its window before beta.14, keeping it in the inspector as the other
windowed effects are kept: a click on the insert opens the window, the mini view under it
has the quick controls.

What had to hold: no DSP change (the sound bit-identical, the golden hashes still); every
control a `Command` with undo; the family's frame (presets, A/B, the device's undo, bypass,
size, the window's own buttons); the parameter menus (automate, modulate, learn) on every
control; both locales; and nothing repainting at rest.

## Decision

1. **Delay has an editor** (`EffectKind::has_editor` is false for the old Reverb only;
   `views::fx::delay`, `views::fx::Editor::Delay`). Everything that keys on an editor now
   includes it: the inspector's insert row opens the window (its tip says so) and its mini
   view shows the first three values (time, feedback, tone) and the dry/wet, as the other
   windowed effects' do; the picker's thumbnail is the window's; the menus' *Open the
   editor*; the device swap; every test that walks the editors (the frame, learn, modulate,
   the faces, the master chain).
2. **The display is the echoes in time, and it is the control** (`delay_view.rs`). The left
   side above a centre line, the right below. The sound at 0; each repeat a stem from the
   centre at its time, its level relative to the first, drawn as its square root so the
   quiet ones still show; on its side when ping-pong is on (the first on the left, then
   alternating — the unit writes the sides' sum into its left line and crosses the
   feedback), on both when it is off. Its colour runs from the face's accent (all its highs)
   to its second voice as the tone takes them: the one-pole low-pass's loss at 8 kHz, once
   per pass. Dashed: the decay through the heads, and the feedback's level in each lane from
   the first repeat. The model is the DSP's own, and a test runs the real unit through the
   engine's wiring and finds every repeat where the display draws it (time, side, level,
   8 kHz content).
   - **A drag** anywhere on it sets the time across (the first repeat follows the pointer)
     and the feedback up and down (in the lane it began in: up in the left, down in the
     right, so the level lines follow the pointer), as **one step of undo**
     (`cmd.delay_echoes`). Shift: a fifth of the rate. Double-click: the defaults. The span
     (a round time holding about six and a half repeats) is held still during a drag, so the
     axis does not move under the pointer.
3. **Tempo: milliseconds, and a note value set once.** The unit's time has no sync, and no
   DSP was added. *At the tempo* lists the note values (the modulation family's divisions)
   that fit 1 ms … 2 s at the song's tempo; a choice sets the time once, the field names the
   note when the time is one, and two lines under it say the tempo it was read at and that it
   does not follow a tempo change. A synced time would be a new parameter and a DSP change: a
   later decision (BACKLOG).
4. **What is heard**: the repeats to −60 dB, the tail they make, the tone's loss per repeat at
   8 kHz, the first repeat's side, and one sentence on how the sides repeat.
5. **The face** (`kit/skin/tone.rs`, `DELAY`, key `fx.delay`): a tape echo at night — an
   oxblood tint (350°) in vinyl amp covering, the echoes on a phosphor screen in a coral
   accent (354°) whose second voice is rust (16°, a darkened repeat), skirted knobs, a rocker
   for ping-pong, upper-case lettering tracked +0.12 with mono values, an engraved name,
   screws and printed rules. Siblings: not within 20° of Tonus (326°, glass) or any tone
   family unit with the same material or display; the family's sheet test holds it.
6. **At rest, no frame.** `frame::Out::at_rest` lets an editor say nothing in it moves; the
   window then asks the ticker for nothing (every other editor leaves it off and keeps the
   meters' rate). Delay's says it moves only in a frame where the analyser tap delivered and
   either the input sounded or a repeat glows: each repeat glows with what the input was its
   time ago, times its level (`Heard`, a peak per tap batch stamped with its sample count),
   so the echoes leave in turn while sound plays. Half a second with nothing from the tap and
   what was heard is forgotten.
7. **The pictures**: `shoot_skins` shoots Delay like every other unit (`delay-*`), so the
   site's card and the README's row are the window's; `shoot_delay` now shoots the
   inspector's view (`delay-inspector-*`) and the window over the application
   (`delay-app-*`) for the round's report.

## Consequences

- Every effect of the family but the old Reverb (kept only for the projects that use it,
  ADR 0101) has a window in a face of its own; ADR 0074 §6's "legacy delay" is gone.
- The inspector's mini view of Delay no longer lists every parameter and the ping-pong
  switch: those are the window's (as for every windowed effect); the three first values and
  the dry/wet stay.
- **The exported plug-ins:** Delay has never been one of the CLAP/VST3 plug-ins (ADR 0020's
  family is the round-17 units and their successors; `cantus_plugins::desc::ALL` has no
  Delay). Its editor runs on `FxHost`, so it is ready for one; exporting it is new ids that
  are forever and a product decision — left to the owner (BACKLOG).
- `frame::Out` has a field more; open windows of the other editors behave as before.
- `cantus_plugins::editor::has_editor` now asks for an exported kind as well as an editor on
  `FxHost` (Delay has the second, not the first).

## What the tests hold

- `views::fx::delay::view::tests`: the echoes drawn are the unit's own — the real
  `EffectUnit`'s impulse response, ping-pong off and on, every repeat at its time, on its
  side (nothing on the other), at its level (DC gain, to 1e-3) and with its 8 kHz content
  (to 2 %); the counts, the span, the note values; the drag's mapping (across, up in the left
  lane, down in the right, Shift, the limits); the heard history read back by age.
- `tests/delay_window_e2e.rs` (through `CantusApp::assemble`, driven by pointer events): a
  click on the insert in the inspector opens the window; a drag on the echoes moves the time
  and the feedback in one step that undo takes back; each knob, the ping-pong switch and the
  note value write their parameter and undo restores it; the window asks for no frame at rest,
  moves while the tap brings sound and is still again after; the window in Spanish; every
  English word with its Spanish twin.
- The family's walkers now include Delay: `fx_frame`, `learn_everywhere`,
  `modulate_everywhere`, `master_chain`, `skins`, `skins_body`, `skins_budget`'s sheets,
  `kit::skin::tone::tests` (siblings and the contrast floor in every preset).
- `cantus-plugins/tests/editor.rs`: every exported editor still opens and wears its face
  (Delay is not among them).
- No DSP file changed: `cantus-dsp` and `cantus-engine` untouched; the golden hashes stand.
