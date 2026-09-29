# Moving an instrument onto the new look (ADR 0024, Phase B)

The brief every instrument builder of round 18 works from. The decisions are ADR 0024
(read §2 colour, §3 type, §4 the kit, §8 the device frame, §13 performance, §14 tests);
`docs/instruments/BUILDING.md` still holds for everything that is not about the look
(worktrees, commits, file ownership, the house rules of the DSP).

## What already exists — do not rebuild it

- **The shell** (`crate::kit::shell`, drawn by `views/inst/frame.rs::window`): the 28-pt header
  that is the title bar (preset well with ‹ ›, A|B, undo, Play · Edit · Matrix, ⋯, size), the
  24-pt footer (help line, the "Voices n / m" chip and yours, the output meter, the grip), the
  per-window scale, and the **bands**: the macro card and the modulation card sharing a row,
  and the keyboard card (Play view by default; ⋯ › Keyboard). You fill only the area above
  the bands.
- **Your two entry points** receive that area as `ui` and the shell's `Style`:
  `pub fn play(app, ui, track, spec, s: &Style)` and `pub fn body(app, ui, track, spec, s: &Style)`
  (Edit). Keep their signatures: the workspace builder hosts the window in a dock tab.
- **Controls bound to a parameter** (`views/inst/frame.rs`):
  - `cell(app, ui, track, spec, key, s, KnobSize::M)` — the kit **Cell** (value pill on top,
    knob, label below, one object): range, taper and unit from the parameter, the label
    abbreviated through `<label_key>.short`, one undo step per drag, a dropped modulation
    source connects it, what modulates it drawn as rings.
  - `knob(...)` — the same without pill and label (inside a display, a dense row).
  - `write_readout(v, unit, &mut String)` — the house readout, without allocating.
- **Kit widgets** (`crate::kit::widgets`): `segmented` (mode switches: 808 · Synth · String,
  filter types, slopes, quality), `led_button` (every on/off), `menu_field` (type pickers, with
  ‹ ›), `readout`, `tile`, `menu_list`, `meter`. **Painters** for your own displays go through
  `kit::with_brush(&painter, |b| …)` and `b.register(rect, "your.part", TOKENS, &[])`; colours
  are tokens only (`s.t.*`, `s.hue`, `s.t.hue(Function::…)`) — `tests/kit_lint.rs` bans literals.
- **Cards**: `crate::kit::band::card(b, rect, s, state, &CardLook { title: Src::Key(..), … })`
  returns the title bar and body rects; lay cards out with `kit::shell::Grid` (`full`, `row`
  with fractions, 4-pt gutters) inside your area.
- **Text**: `kit::text::paint(b, rect, &Text::key(key, Role::Label, s.t.label).caps(true)
  .policy(Policy::Abbreviate), s.tk())` — every word in a box, measured, elided or abbreviated.

## The layout every instrument follows (§8)

1. **The display first**, 40–55 % of the area's height, bleeding to the card edges: the thing
   you drag *is* the parameter (Unda's table, Calor's filter curve, Modulus's operator graph,
   Exemplar's map and waveform, Gravis's pitch curve or string, Organum's drawbars and
   cabinet, Lamina's pickup, Pulsus's pad grid).
2. **Then cards**, one per section of the signal flow (OSC 1, OSC 2, FILTER, AMP ENVELOPE,
   LFO 1, VOICE…), each with its **function hue** (`Function::Source` for oscillators and
   samples, `Tone` for filters and EQ, `Dynamics` for envelopes and drive, `Mod` for LFOs and
   modulation, `Space` for chorus, reverb and width, `Output` for voice, glide and level),
   cells inside, a small display in the card when it helps (an oscillator's cycle, an
   envelope's shape).
3. **Play** shows the display and the few parameters that matter live; **Edit** shows every
   section. Edit may be taller than the window: it scrolls there, never squeezes.
4. **One XL knob at most**, for the primary parameter; L for the key ones, M in cards, S in
   dense rows. **No loose words**: a word is a control's label in its cell, text inside a
   control, a card's title, or the left cell of a label | value row. No bare `ui.label` or
   `selectable_label` in a row of controls, no stock egui widget (`Button`, `ComboBox`,
   `Slider`, `DragValue`, `checkbox`) in your files.
5. **Abbreviations** for every label that might not fit go into your locale files as
   `<label_key>.short = DET` (EN and ES).

## Tests you add

- Your editor opens in the shell in Play, Edit and Matrix, Slate and Paper
  (`tests/inst_shell.rs` already walks every kind; extend it for yours if you need more).
- **No overlap**: rendered at 75 % and 200 % and in Spanish, every text fits its box (the kit's
  `text::paint` returns what it painted; assert nothing was clipped).
- A gesture on a cell is one undo step; a dropped source connects.
- Screenshots: `CANTUS_INST_SHOTS=<dir> cargo test -p cantus-ui --test inst_shell` writes a PNG
  per kind, view and theme. Look at them (Read), compare with
  `C:\projects\daw-r18\kit\target\proof\c-instrument-slate-1x.png` (Calor as the proof drew
  it) and iterate until yours is as good.

## Files you own

Your instruments' editor files (`views/inst/<kind>.rs`, `<kind>_*.rs`), their locale files and
their tests. **Not yours**: `views/inst/{mod.rs,frame.rs}` (the coordinator's), the kit
(`crate::kit`, the effects session's — a missing part or a bug there is a small additive fix in
its own commit starting with `SHARED:`, listed in your report), `app.rs` and every other view.
The default theme flips to Slate at the final merge, not in your branch.
