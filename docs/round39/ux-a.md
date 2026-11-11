# Round 39 — `ux-a`: a driven control says so, the help line, nothing truncated

Builder `ux-a`, branch `r39/ux-a`, base master `80d5f109` (beta.13). ADR 0109. Locale area
`areas/ux_signs.{en,es}.txt`. No schema change.

## What changed

### 1. A driven control says so (BACKLOG §6j, the owner's "broken" Chorda knobs of round 38)

- Every control bound to a parameter wears the signs of what drives it through one kit
  call, `kit::automate::signs`: the clip's dot (selection colour, upper right), the
  modulator's (upper left), the controller's (lower right) and a new VCA dot (lower left).
  The desk's pan, fader and width now wear the clip's sign too (they showed the clip's value
  with no mark).
- It already moved with its driven value (the knobs read the lane where the song is); the
  e2e test now holds that.
- Its hint says what drives it and how to take it back, in the menus' own words:
  *Automated by the clip ‘Hardness’ on Piano · right-click ▸ Edit automation, or delete the
  clip to set it by hand*; *Modulated by LFO 1 · Alt+drag sets the depth; right-click ▸
  Modulate… disconnects*; *Moved by the controller CC 74 · right-click ▸ Forget MIDI
  controller to let it go*; *Scaled by the VCA Drums · right-click the strip ▸ Follow VCA to
  release it*. The kit carries only a tag (`ModOffer::drive`); the application finds the
  target once and keeps the words (`src/drive.rs`) — nothing is formatted while the pointer
  rests. The top bar's hint panel shows them on two lines; a device window's footer adds
  them and drops whole phrases from its end where they do not fit.
- What a hand does on a driven knob is unchanged (ADR 0027 §6: the clip wins while the song
  plays); the hint now says why it springs back.

### 2. The help line is never empty

- A walk through `CantusApp::assemble` over every view found 210 of 1 268 controls whose
  hover wrote nothing. Now none, of more than 1 700 (the walk asserts it reaches over 500).
- The kit's parts speak by themselves where the caller gave no words (fields, tabs,
  splitters, scrollbars, menu rows with their keys, segmented choices, LED keys; a hint with
  no name is named by its value). After the frame, a resize or move cursor says *Drag to
  resize*/*Drag to move* (egui's window edges, the dock's separators) and a stock entry in a
  menu says its words.
- The views' gaps filled with a sentence each (both locales): the top menus, the top bar's
  ground, the inspector (title menu, band **+**, placeholders, the pad's *Editor*, the input
  row, the pinned strip's fader and keys), the desk (scrollbars, the cable band), the piano
  roll's ruler, the devices panel's rows, the session (heads, grid, scene column), the
  instrument window (band titles, keyboard), the sampler's key map, Settings' pages, the
  export dialog, the floating dancer's buttons.

### 3. Nothing truncated

- The kit counts every text `text::paint` elided or dropped (`kit::text::probe`). A sweep of
  every view at 100 % and 150 % of a 1920 × 1080 screen, night and day, English and Spanish
  (440 view-looks) found 97 distinct cut texts (3 486 cuts), 43 once each view is looked at
  on the default layout. Now none.
- Fixed by layout: kit menus as wide as their longest row; toolbar LED keys as wide as their
  word; the session's quantise field; the inspector's pad actions one under the other when
  narrow; its receives key cell as wide as its words; the maximizer's target rule on two
  lines; Chorda's inspector line broken before the stretch; the controllers pane's "no
  MIDI input" note to its first sentence (the whole on the help line); a compact strip's
  route chip without its arrow when the number would not fit.
- Fixed by wording: the devices panel's search placeholder, `kHz` with one decimal from
  10 kHz, the EQ's slope pill "24 dB", MPE chips by their gesture ("BEND"), and Spanish
  words and short forms (`.short`, the kit's designed abbreviations) for the stereo,
  maximizer, limiter and multiband labels, the inspector's rows, the session, the score, FM's
  graph hint.
- Not counted as cuts, by design: the project's own names (tracks, strips, clips) cut in
  their middle where a place is narrower than the name (ADR 0024 §3, FL's way) — e.g. the
  desk's rotated strip names at 150 % on a 1280 × 720-point window. The owner's eye decides
  whether that place should grow with the new design.

## How it is tested

- `kit::hint::tests::a_driven_hint_asks_for_its_words_once`.
- `tests/ux_signs_e2e.rs` (through `assemble`): Chorda's *Hardness* with a clip — the sign,
  the clip's value, the words in the hint and the top bar's panel; the clip deleted — no sign,
  no words, its own value.
- `tests/ux_help_line.rs` (through `assemble`): every view, every control that takes a click
  or a drag and that nothing covers, hovered — fails on any without a hint.
- `tests/ux_whole_words.rs` (through `assemble`): every view in eight looks with the kit's
  text probe — fails on any cut.
- `--ignored` shot tests photograph every view (`CANTUS_UX_SHOTS`, `CANTUS_UX_LOOK`).

## Pictures

`docs/round39/ux-a/`.

## Verified only in emulation

Everything here runs headless through the CPU rasteriser at a fixed 1920 × 1080 screen; the
words were read on those pictures, not on a monitor.

## What is left

- The signs are today's dots, drawn with today's tokens; the new look (ADR 0111, the kit
  builder) redraws them inside `automate::signs`.
- Touched outside the owned files (one-line hint or layout changes, said here for the
  merge): `app.rs` (two calls in `frame`), `kit/shell.rs` (the footer's help line),
  `kit/field.rs`, `kit/menu.rs`, `kit/widgets.rs`, `kit/text.rs`, many views (listed in the
  commits).

### The pictures

- `driven-knob-chorda-hardness.png` — the e2e test's moment: Chorda's *Hardness* driven by
  a clip, hovered: the hairline and dot on the knob, both lines of words in the top bar's
  hint panel, the footer's help line whole.
- `before-es-night-100-stereo-cut.png` — before: Latitudo in Spanish ("CRUCE…", "CAM…",
  "apag…", "RETAR…").
- `whole-words-es-day-150-*.png`, `whole-words-en-night-100-*.png` — after: the same
  editors and others in Spanish, day theme, 150 %, nothing cut.
