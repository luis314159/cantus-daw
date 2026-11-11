# ADR 0109 — A driven control says so; the help line is never empty; nothing truncated

**Status:** accepted (round 39, builder `ux-a`) · **Date:** 2026-11-09 · **Schema:** none ·
**Extends:** ADR 0024 §9 (the hint), ADR 0027 (automation), ADR 0057 (modulation), ADR 0062
(MIDI learn), ADR 0093 (VCAs) · **Locale area:** `areas/ux_signs.{en,es}.txt`

## Context

Round 38's owner thought some of Chorda's knobs were broken: automation clips were setting
them, and nothing on the knob said so in a way he saw — a 2.6-pt dot at its corner, and a
hint bar that named the knob and its value only. BACKLOG §6j asks three things of the
whole interface in round 39, before the new look lands through the kit:

1. a control driven by something — an automation clip, a modulator, a hardware controller,
   a VCA — shows a sign, moves with the value it is driven to, and its hover hint says what
   drives it and how to take it back, in the app's real terms;
2. the help line (the top bar's hint panel, a device window's footer) is never empty over
   anything a pointer can act on;
3. no word of the interface is cut — not at 150 %, not in Spanish, not in the day theme.

What was there: the knobs of the kit (`kit::widgets::Param`), the effects' cells, the
inspector's rows and the desk already *showed* a clip's value (they read
`CantusApp::automated_value` where the song is) and drew the automated, modulated and
mapped dots — each site its own three calls, the desk none for a clip. The hint slot
(`kit::hint`) held a name, a value and four capability flags. A walk over the window
(below) found 210 of 1 268 controls whose hover wrote nothing.

## Decision

### 1. What drives a control: a tag in the hint, words from the application

- **The kit stays ignorant of projects.** `kit::automate::ModOffer` (which every control
  bound to a parameter already receives from `CantusApp::mod_offer`) gains `drive: u64` —
  a hash of the target (`drive::drive_tag`, never 0) when something drives it, else 0 —
  and `vca: bool`. `mod_offer` sets them: a lane (`is_automated`), an unmuted modulator
  route, a controller mapping (`learn_offer`), a VCA governing a strip's level.
- **`kit::automate::Drivers`** (clip, modulated, mapped, vca) from `Offer::drivers()`,
  and **`kit::automate::signs(b, rect, s, d)`** paints every sign in one call — the
  automated dot (selection colour, upper right), the modulated dot (upper left), the mapped
  dot (lower right), and a new VCA dot (the label's ink, lower left). Every parameter
  control calls it: `Param`, the effects' `Cell`, the inspector's pinned strip, its note
  effects and modulator rows, the desk's pan/fader/width (which now also wear the clip's
  sign — they showed the clip's value but no mark). A probe (`probe::start_signs`) notes
  each sign painted, for the tests.
- **The hint carries the tag.** `kit::hint::drive(ctx, tag)` (called by the control after
  its hint, through `automate::hint_drive(ctx, offer)`) marks the hint; a `set` in the same
  pass keeps the mark, so a control may say it before or after its hint. Once a frame,
  after the views drew, `drive::frame` reads `hint::wanted(ctx, stamp)`; when the words
  for that tag are not the ones for this stamp (the project's revision and the global
  controller map's size) it finds the target under the tag (`CantusApp::driven_target`:
  among the lanes, the routes, the mappings, the strips) and puts it in words
  (`CantusApp::drive_words`), kept in the slot (`hint::resolve`). A control hovered or
  dragged re-reads them: nothing is formatted while the pointer rests.
- **The words are the app's real behaviour, with the menus' own labels** (`{menu}` is the
  entry's own translation, so the hint and the menu never disagree). Two lines: what drives
  it, then how to take it back (several drivers: each line joined by " · "):
  - clip: *Automated by the clip ‘Chorda · Hardness’* / *Right-click → Edit automation, or
    delete the clip* — the clip under the playhead, else the first; its track is said
    (*… on Piano*) only when the clip's name does not hold it (a new clip is named after
    its target); several: *Automated by 3 clips, ‘…’ here*. ADR 0027 §6: while the song
    plays the clip wins; a hand changes the project's own value, which the next control
    point overrides. Writing over a clip is ADR 0068's Touch/Latch/Write with the arm —
    left out of a sentence that must fit the top bar.
  - modulator: *Modulated by LFO 1* / *Alt+drag sets the depth; right-click → Modulate…
    disconnects* (the popup's rows toggle a route).
  - controller: *Moved by the controller CC 74* / *Right-click → Forget MIDI controller to
    let it go*.
  - VCA: *Scaled by the VCA Drums* / *Right-click the strip → Follow VCA to release it*.
  - "→", not "▸": the interface's faces have no ▸ (it painted a box).
- **The sign reads.** A clip-driven control also wears a hairline round it in the sign's
  colour (round 38's owner missed the dot alone).
- **Where the words show.** The top bar's hint panel shows the two lines (the first after
  the control's name and value where they fit). A device window's footer adds them after
  *name · value* and, where they do not fit, drops whole phrases from the end — never
  inside a clip's name — and cuts with an ellipsis only when one phrase alone does not fit
  (the top bar says it all).
- Grabbing a driven knob does what it did (ADR 0027 §6); the value pill shows the value,
  the hint says why it springs back.

### 2. The help line is never empty

- **The kit's parts speak by themselves** where the caller gave no words: a field
  (`kit::field`) says its placeholder, else *Click to type · Enter keeps it, Esc puts it
  back*; a tab its label; a splitter and a scrollbar what a drag does; a menu row
  (`menu_list`) its words and its key (a band or a separator: the entry above); a
  segmented choice the choice under the pointer; an LED key its word. A hint written with
  no name is named by its value (`hint::set` swaps them) — a menu field says what it shows.
- **Fallbacks after the frame** (`views::hint::from_cursor`, the last thing `frame` does,
  only where no control wrote a hint this pass): a resize cursor says *Drag to resize*, a
  move or grab cursor *Drag to move* — egui's window edges, the dock's separators; a stock
  egui entry in a menu or popup says the words it paints (`menu_words`).
- **What the walk does not count as a control:** a window's own ground (egui moves or
  raises it there), a widget something covers, a canvas the size of a pane (it speaks for
  its tool), a sliver under 8 pt scrolled almost out of its view.
- **Helpers for the views:** `views::hint::said(resp, words)` and `said_with(resp, ||
  words)` (made only when hovered), and `views::hint::button(ui, words)` for the menus'
  stock buttons.
- **The views' gaps filled**, each with a sentence of what it does: the top menus' words
  (*The File menu*), the top bar's ground (*Right-click to choose what the top bar shows*),
  the inspector's title menu, its bands' **+**, its placeholders, the pad's *Editor*, the
  input row, the pinned strip's fader and keys, the desk's scrollbars and cable band, the
  piano roll's ruler, the devices panel (its rows say what each device is and how it is
  added), the session's heads, grid and scene column, the instrument window's band titles
  and keyboard, the sampler's key map, Settings' pages, the export dialog's presets and
  buttons.

### 3. Nothing truncated

- **The kit counts its cuts.** `kit::text::probe` notes every text `text::paint` elided
  (an ellipsis — an abbreviation that fits, `<key>.short`, is a designed form) or dropped
  (no room for an ellipsis, or a box too short for the line), with the whole text and its
  box. Off, it costs a flag read.
- **The rule for a fix: layout or wording, never smaller type.** A control as wide as its
  words (kit menus, toolbar LED keys, menu fields, cells), a second line where there is
  room (a rule, a preview line), whole phrases dropped where a line runs out (the footer,
  the controllers pane's note, its whole on the help line), a designed short form
  (`<key>.short`, the kit's abbreviation) where a knob's label is fixed by its cell.
- **What is not a cut:** the project's own names (tracks, strips, clips) cut in their middle
  where a place is narrower than the name — ADR 0024 §3's design, FL's way; the sweep counts
  them apart. Whether a place (the desk's rotated strip names at 150 % on a small window)
  should grow is the new design's question.
- The fixes, view by view, are in `docs/round39/ux-a.md`.

## Consequences

- Every parameter control shows what drives it the same way; a new control bound to a
  parameter gets the signs and the words by calling `automate::signs` and
  `automate::hint_drive` — the probe and the walk find one that does not.
- The help line's words cost nothing at rest: every fallback formats only on hover.
- The new look (ADR 0111, the kit builder) redraws the signs inside `automate::signs` and
  `vca_mark`; nothing else draws them.

## What the tests hold

- `kit::hint` — `a_driven_hint_asks_for_its_words_once`: a tag asks once, the words stay
  for the next hovers, a control nothing drives drops them.
- `tests/ux_signs_e2e.rs` — through `CantusApp::assemble` on the demo, a Chorda added the
  way the picker adds one, an automation clip on *Hardness* held at 90 %: exactly one
  control wears the clip's sign; hovered, its hint names it, shows the clip's value and
  says the clip's name, its track and *Edit automation*; the top bar's panel paints *
  Automated by the clip … Edit automation*; the clip deleted, no clip sign, no words, and
  the knob shows its own value.
- `tests/ux_help_line.rs` — through `assemble`, every view (the window, thirteen panes,
  nine instruments, twenty-three effects, Settings, the first-run setup, the export dialog,
  the six menus): the pointer on each widget that takes a click or a drag (not a window's
  own ground, not one covered) and fails on any whose hover writes no hint (over 1 700
  controls, every one with a sentence).
- `tests/ux_whole_words.rs` — the same views at 100 % and 150 % of a 1920 × 1080 screen,
  night and day, English and Spanish, with the kit's text probe on: fails on any cut.
