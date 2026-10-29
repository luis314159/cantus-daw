# Round 32 — dynamics: a face of its own for each dynamics unit

Builder: **dynamics** (`r32/dynamics`). Units: Comprimo, Terminus, Multiplex, Sibilus,
Porta, Culmen, Tonsor, Impetus, Discrimen. The sheets live in
`crates/cantus-ui/src/kit/skin/dynamics.rs`; ADR 0074 is the skin system.

Gain reduction is these units' story, so each face is built around the display that tells
it. Every sibling has a material of its own (nine units, nine materials) and an accent hue
at least 20° from its siblings'.

## Design notes

| Unit | Sound | Inspired by | Colour and material | Display | Controls | Lettering |
|---|---|---|---|---|---|---|
| **Comprimo** | the compressor | a 70s FET rack compressor (black face, backlit VU), Pro-C's level/GR history | black anodised (cool, near neutral), amber accent | cream chart paper for the history and transfer; a backlit VU needle reading the reduction | skirted knobs, bat toggles, rack ears and screws | a badge; capitals, tracked, semibold |
| **Terminus** | the limiter | a modern brickwall (Pro-L's dark scrolling waveform, FL's Fruity Limiter) | flat graphite with a red cast (matte), red accent, ice-blue second voice | the waveform and its reduction behind dark glass | value rings without bodies, pills | a red stripe under the name (the ceiling); as written, medium, mono readouts |
| **Multiplex** | multiband dynamics | Pro-MB's bands over the spectrum, FL's Maximus's three bands | smoked violet glass, violet accent, cyan second voice | phosphor: the bands glow in the face's violet, cyan and the hues between | value rings, pills, printed rules between the cards | engraved; condensed capitals |
| **Sibilus** | the de-esser | a broadcast de-esser rack, Pro-DS's detection display | brushed silver, ice-cyan accent | phosphor: the sibilance in a cyan glow | machined caps, rockers, screws and vents | a printed legend; condensed capitals, mono readouts |
| **Porta** | gate and expander ("the door") | a studio gate in a wooden case; the open/shut pilots | mahogany wood, green (open) and amber (release, shut) on the phase lane | a liquid-crystal panel | soft rubber knobs, bat toggles, screws | engraved; capitals |
| **Culmen** | the mastering maximizer ("the summit") | a mastering processor's fluorescent display, FL's Maximus | deep sapphire lacquer with rack ears, blue accent | a vacuum-fluorescent display: the bands and loudness | LED-ring encoders, lamp keys | a badge; wide, light capitals |
| **Tonsor** | the clipper ("the shearer") | a transfer curve measured on an X–Y scope; StandardCLIP, KClip | carbon weave, hot orange beam | an oscilloscope's graticule with the transfer curve | machined caps, bat toggles, vents | a printed legend; condensed capitals, mono readouts |
| **Impetus** | the transient processor ("the onset") | a drum amplifier: vinyl covering and pointer knobs | black vinyl amp covering (the kit's `Tolex` material), lime (attack) and violet (sustain) | glass over the face | chicken-head pointers, lamp keys, screws | a badge; condensed wide capitals |
| **Discrimen** | the transient splitter ("the dividing line") | a dual-beam oscilloscope (CH1, CH2), a lab instrument's printed card | printed card (paper), magenta (transient) and teal (body) | a scope: the two parts as two beams | numbered dials, pills, printed rules | a stripe; as written, mono readouts |

## What was done

- **The sheets** (`kit/skin/dynamics.rs`): the nine faces above, in data; siblings share no
  material and keep ≥ 20° of accent hue apart; each accent tuned per theme so it reads
  4.5:1 in all 12 presets (the dark-theme reds and magentas turn coral and pink at the
  lightness that needs — planned that way).
- **Culmen's own knob** (`led_ring_encoder`, a `KnobShape::Custom`): a dark machined cap
  with a lit index inside a ring of LEDs lit from the default to the value (one every
  4.5 pt of the sweep, an odd count), the default's LED a touch brighter, the live
  modulation dot on the ring; the kit keeps the modulation and focus rings.
- **Comprimo's needle** (`views/comp_editor/vu.rs`, the kit's `displays::needle_meter`): a
  *Reduction* card between the transfer and the meters — the needle rests at 0 VU and
  falls left by the decibels taken (a FET compressor's GR mode), the peak lamp lit when
  the output reaches full scale. Its numbers stay the meters card's: printed again under
  the needle they cost 46 allocations a frame (`frame_budget`'s Compressor test, 191 →
  238 against its budget of 200); the needle alone costs 1.
  Comprimo's designed width grows 1120 → 1260 in its face so the history keeps its room.
  The needle is part of the face: with *Units wear their own faces* off, Comprimo is the
  editor it was (1120 wide, three cards — `vu::faces_off_keep_comprimo_as_it_was`).
- **Second voices on the displays** (`dynfx::second_voice`): Terminus's output in ice blue
  under its red ceiling and reduction, Impetus's sustain cut in violet beside its lime
  attack, Discrimen's body as the scope's teal second beam beside the magenta transient.
  In the kit's plain look each keeps its old colour.
- **Words and plates in the right place** for a display that has a look of its own:
  `dynfx::on_display` (a handle, a lane, a label's backing inside a display take a step off
  the display's ground, away from its words, not the theme's deck — Multiplex's crossover
  labels were light on light in Paper; Porta's phase lane, Sibilus's and Terminus's labels,
  Culmen's lanes, Multiplex's empty-state card);
  `dynfx::deck_ink` (numbers set on a card's deck, not on a display — Tonsor's clip meter
  and Discrimen's *The parts now* read dim with the scope's colours).
- **Fixed on the way**: Terminus's *Delta* toggle cut to "DEL…" by the pill switch (its room
  now comes from the monitor's segments); Tonsor's *16×* key ran over its card's edge at
  the designed width (also in the plain look).
- **Shots**: every unit shot in Slate and Paper at 100 % and 200 % with `shoot_skins`
  after the kit's fix round; kept in `docs/round32/skins/dynamics/`: the nine at 100 % in
  both themes, and 200 % crops of Culmen's LED rings and Comprimo's needle (3.7 MB).
  `examples/shoot_dyn_faces.rs` stays as the family's quick shooter.
- **Manual** EN/ES: *Each dynamics unit's face* (section 10).

## The review's fix round (f4d0538c..2cc93f46)

- **Comprimo's needle has ballistics.** A one-pole toward the reduction with
  `a = 1 − e^(−dt/τ)`, `dt` the frame's `stable_dt`, **τ = 65 ms**: it reaches 99 % of a
  step in 300 ms (4.6 τ), a VU meter's rise time, so it reads as a needle and still follows
  a compressor's release, at any frame rate. The peak lamp is held by time: lit for 0.4 s
  after any frame whose output peak reaches full scale. Tests: a step to 10 dB read over
  300 ms at 60 and 144 frames a second lands within 0.1 dB at both, the first frame is not
  there yet; a one-frame over lights the lamp ≥ 0.3 s at both rates.
- **The device picker's thumbnails** capture on a canvas made from the largest designed
  editor (Comprimo's face is 1260 wide) plus the shadow's room, no longer a fixed
  1240 × 820 (`picker_snap::canvas`; test: every kind's editor fits it).
- **No colour the editor does not draw.** Porta's phases now wear its pilots (open in its
  green, release and shut in its amber — the display's second voice — each 3:1 on the
  LCD); Multiplex's bands take the face's colours (its violet, its cyan, the hues between
  and beyond; 3:1 on the phosphor's ground at the display's polarity, 4.5:1 on the deck
  where a band marks its card; the first four ≥ 40° apart). Comprimo, Sibilus, Culmen and
  Tonsor declared a second voice no code drew: dropped from their sheets, the design table
  and the sheet test (which now requires a second voice exactly where one is drawn). With
  faces off, Porta's phases and Multiplex's bands are as before (tests). The manual says
  "vinyl amp covering" for Impetus, not a brand's word.
- Checked after it, Windows `-D warnings`: `cargo clippy -p cantus-ui --all-targets` clean;
  `cargo test -p cantus-ui` — 174 binaries, 1 903 passed, 0 failed, 31 ignored.
- Waiting on the skin builder (finding 1, `canvas::Ink::of` flooring `danger` and `ok`
  against the look's ground): then `danger` 4.5:1 and `ok` 3:1 join the dynfx test's loop.

## How it was tested

After the kit's fix round (merged at f4d0538c), Windows, `RUSTFLAGS=-D warnings`: `cargo
clippy --workspace --all-targets` clean; `cargo test -p cantus-ui` — 174 binaries, 1 898 passed, 0 failed, 31 ignored.
Linux (WSL, own clone `~/ci-r32-dynamics`, its `target/` removed after): `check.sh r32/dynamics clippy` clean (workspace,
and the plug-ins without their editor); `cargo test -p cantus-ui` headless (`DISPLAY` and
`WAYLAND_DISPLAY` unset) at 73637034 — 174 binaries, 1 904 passed, 0 failed, 31 ignored;
and at 8d6c6c65 (after the faces-off and plates fixes): clippy clean again, the
cantus-ui library tests (303 passed) and `frame_budget`, `skins`, `skins_body` green.

New tests:

- `kit::skin::dynamics::tests` — every unit wears a face of its own (and `of` maps every
  kind of the family); siblings differ in material and by ≥ 20° of hue; no unit of any
  family shares hue (≤ 20°) and material with ours (against every sheet landed); every
  accent 4.5:1 on decks, card titles and displays in all 12 presets; every second voice
  3:1 on its deck and ink; **the kit's whole contrast floor, worn**, in all 12 presets
  (`short_pairings` + `skin::contrast_failures`, no longer ignored).
- `views::comp_editor::vu::tests` — the needle rests at 0 VU and falls left by the
  reduction, clamped at −20; faces off keep Comprimo as it was.
- `views::fx::dynfx::tests::words_inside_every_dynamics_display_read_on_what_they_are_drawn_on`
  — the nine faces × 12 presets: the ink's polarity is its display's (a CRT dark in Paper,
  an LCD light in Slate); text 4.5:1 and dim 3:1 on the display's ground and on every
  `on_display` plate; the unit's hue 4.5:1 on the **look's** ground (not only the theme's
  ink); `deck_ink`'s words 4.5:1 on the deck; with faces off the helpers change nothing.
- Kept green on purpose: `frame_budget::an_editors_idle_stepped_controls_cost_no_allocation`
  (Comprimo 192 allocations a frame against 200; the needle's numbers came out to keep it).

Looked at: all nine in Slate and Paper at 100 % and 200 % (`shoot_skins`), compared with the
kit's plain look (`--plain`) where something looked cut. Redrawn after looking: the LED ring's
unlit beads (too bright), Impetus's covering (green cast → black), Terminus's output (red on red
read as danger → ice blue), Multiplex's crossover labels and every in-display plate, the
numbers on decks, Comprimo's needle card (numbers out, the face centred).

## Shared-file lines touched

- `views/fx/frame.rs` (the skin builder's): one line, `designed_size(Compressor)` 1120 →
  1260 wide, with a comment naming why.
- `locales/areas/skin.{en,es}.txt`: a `dynamics` block (one key, the card's title).
- `docs/manual/{en,es}/README.md`: one new section before *When a plug-in crashes*.

## Needs the owner

Nothing.
