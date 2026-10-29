# Round 32 — modulation: a face of its own for each modulation unit

Builder: `modulation` (branch `r32/modulation`). Units: Concentus, Flexus, Phasis, Tremor,
Vibrans, Anulus, Translatio, Fluctus. Sheets: `crates/cantus-ui/src/kit/skin/modulation.rs`.

Movement is these units' story, so each face is built around the display that shows how
the unit moves the sound, and the other channel or voice is drawn in the face's second
colour (before this round it was a faint grey line — and in Translatio, once skinned, the
right sideband would have taken the unit's own hue, the same as the left). The kit's
display looks (`canvas::Ink`) give each display its ground, colours and cover; the main
traces glow through `canvas::trace`.

## The design notes

Hues spread round the wheel (30°, 52°, 100°, 160°, 206°, 245°, 285°, 330°), eight
materials for eight units; none shares hue (within 20°) and material with a unit of the
dynamics or instruments families (their sheets on `r32/dynamics`, `r32/instruments`).

**Concentus** (chorus). *Sound:* two or four delayed copies swept slowly — thick, wide.
*Reference:* an 80s stereo ensemble rack (the Dimension-style four-button box) and a
polysynth's bucket-brigade chorus. *Colour/material:* brushed silver, accent ice blue
(206°), second voice amber (38°); rack ears and screws. *Display:* each voice's delay under
glass, drawn as a bucket brigade — beads along each line, the left voices blue, the right
amber. *Controls/lettering:* machined caps, lamp keys (the model buttons), a printed legend,
wide capitals.

**Flexus** (through-zero flanger). *Sound:* the jet — a comb swept through zero.
*Reference:* a 70s stomp-box flanger and tape flanging. *Colour/material:* violet lacquer
(285°), lime second voice (100°); screws. *Display:* the comb glows on a phosphor CRT with a
lime mark under every notch (the teeth); through zero, the wet's line flares lime where it
crosses the dry. *Controls/lettering:* skirted knobs, bat toggles, a badge, condensed type.

**Phasis** (phaser). *Sound:* a few notches sweeping. *Reference:* the orange 70s stomp
phaser. *Colour/material:* matte orange powder coat (30°), teal second voice (178°);
screws. *Display:* the response on a fluorescent display (the VFD glows in the teal),
every notch marked on the axis. *Controls/lettering:* soft rubber knobs, pills, a stripe
under the name, capitals, mono readouts.

**Tremor** (tremolo, auto-pan, harmonic). *Sound:* level or pan in time. *Reference:* a 60s
amplifier's tremolo channel in a coloured vinyl covering, and a chart recorder. *Colour/material:*
seafoam vinyl covering (`Material::Tolex`, 160°), amber second voice (40°, the pilot's jewel); vents. *Display:* the
shape on chart paper with the recorder's time ticks along the top and its pen at the
playhead, the other side in amber. *Controls/lettering:* numbered dials, rockers, an
engraved name.

**Vibrans** (vibrato). *Sound:* the pitch wavers. *Reference:* a photocell vibe pedal (its
lamp behind glass) and a tuner's display. *Colour/material:* smoked glass, periwinkle
(245°), rose second voice (340°). *Display:* the pitch in cents on a tuner's liquid-crystal
panel, with the tuner's scale down the side and a pointer at the pitch now.
*Controls/lettering:* value rings, pills, the name etched in the glass, light type, mono
readouts.

**Anulus** (ring modulator). *Sound:* sums and differences — bells, metal, robots.
*Reference:* a modular synthesiser's ring-modulator module in a wooden case.
*Colour/material:* walnut (30° tint), accent brass gold (52°), orchid second voice (300°); screws
and printed rules. *Display:* the partials on a lab oscilloscope, each sideband pair joined
by a ring round the input — *anulus* is Latin for ring. *Controls/lettering:* knobs that are
rings (the unit's own painter, `KnobShape::Custom`), rockers, a printed legend, condensed
capitals, mono readouts.

**Translatio** (frequency shifter). *Sound:* every partial moved by the same hertz — no
longer harmonic. *Reference:* a lab frequency shifter in a black rack, a spectrum
analyser's CRT. *Colour/material:* black anodised (95° tint), chartreuse (100°), coral
second voice (14°); rack ears, vents. *Display:* the spectrum on green phosphor with an
arrow carrying each harmonic to where it lands, the right sideband in coral.
*Controls/lettering:* pointer (chicken-head) knobs, lamp keys, a stripe, wide light
capitals, mono readouts.

**Fluctus** (volume shaper). *Sound:* the level drawn over a cycle — sidechain pumping
without a key. *Reference:* a modern volume shaper's dark editor (LFO Tool, Gross Beat).
*Colour/material:* carbon weave (325° tint), magenta (330°), cyan second voice (190°);
printed rules. *Display:* the shape under dark glass, the playhead in cyan with a ring on
the level it rides now. *Controls/lettering:* value rings, pills, a badge, condensed type.

## What was done

- **The eight sheets** (`kit/skin/modulation.rs`), each drawn by the kit's painters
  (materials, display looks, knob and switch shapes, plates, lettering, ornaments), and
  **Anulus's ring knob** — the family's one `KnobShape::Custom`: a torus with the face seen
  through its hole, the pointer a notch across the ring, the kit's track, value arc,
  default notch, live dot, modulation rings and focus ring round it.
- **The characteristic displays** (`views/fx/modfx_face.rs`, a module of `modfx.rs`): the
  shared movement display takes `Extras::marks` — `Buckets` (Concentus), `Crossings`
  (Flexus through zero), `Chart` (Tremor), `Tuner` (Vibrans) — and draws the right channel
  in the face's second voice; `notches` (Flexus's comb, Phasis's response), `ring`
  (Anulus's sideband pairs), `arrow` (Translatio's shift), the riding level (Fluctus). The
  main traces go through `canvas::trace`, so they glow as the display's look does (a CRT, a
  VFD, a scope). `second_voice` is the look's second trace (the kit keeps it 3:1 on the
  ground and apart from the main trace); the plain look keeps the faint line.
- **The unit's own colour after the skin fix round.** A skinned style keeps the theme's
  modulation hue for `Mod` (modulation cues, ADR 0074), and the device frame asks for
  `Mod` for every modulation unit — so the nameplate, its stripe, the lamps and the cards
  went the theme's violet on every unit of this family. `frame::face_function` (marked
  lines in `views/fx/frame.rs`) asks for the tone hue — where the tokens hold the accent —
  for a modulation unit in a face of its own; `modfx::body`/`modfx::own` do the same for
  the bodies and the cards that ask for `Mod`. The plain look is unchanged. Test:
  `every_face_s_header_and_body_paint_in_its_accent` (every unit, every preset).
- **Translatio's right sideband**: the face's second voice. It was the Dynamics hue, which
  a skin's accent replaces — left and right would have been one colour.
- `examples/shoot_mod_faces.rs`: the eight units in Slate and Paper at 100 % and 200 %
  (`--plain` for the switch off); `shoot_skins` covers them too.
- The manual (EN, ES): the family's faces, a paragraph and a table at the end of *The
  modulation effects* (after *In another DAW*).

## How it was tested

Windows, `RUSTFLAGS=-D warnings`, after merging the skin fix round (f4d0538c): `cargo
clippy --workspace --all-targets` clean; `cargo fmt -p cantus-ui -- --check` clean; `cargo
test -p cantus-ui` — 175 binaries, 1 897 passed; the 2 failures were `pack_sounds.rs`'s
500 ms timings (Exemplar's streamed waveform) while four builders compiled at once — that
binary passed three runs out of three alone; `cargo test -p cantus-plugins` — 76 passed.
Linux (WSL, `tools/wsl/check.sh r32/modulation clippy`, `~/ci-r32-modulation`, its
`target/` removed after): clippy clean, the plug-ins without the editor clean.

New tests:

- `kit/skin/modulation.rs`: every unit wears a face (accent, second voice, tint, material,
  display — none plain); siblings differ in material and hue (> 20°); no unit of any family
  that has landed shares hue (within 20°) and material with ours; every accent 4.5:1 on its
  decks, card titles and displays in the 12 presets; every second voice 3:1 on decks and
  ink; `every_face_keeps_the_kit_s_contrast_in_every_theme` (every pairing, plus
  `skin::contrast_failures`) — on.
- `views/fx/modfx_face.rs`, over the eight units × 12 presets: every unit has a second
  voice on its display, 3:1 on its ground; inside every display the words read on every
  fill they are drawn on (the look's own ground — `display_ink(s).ground` — and a label's
  plate: text 4.5:1, dim 3:1), the trace shows (3:1) and the ink's polarity is the
  display's, not the theme's (an LCD in Slate, a CRT or scope in Paper); the header and the
  body paint in the accent; with the switch off nothing of the faces reaches a picture (no
  marks, no second voice, the family's hue, the plain display ink). Every mark is drawn
  only in a unit's own face (`face::own_face`), so the plain look is the picture as before
  (checked by eye: Anulus with the switch off).
- Concentus's mono check: its words sit on the card, not on a display, so they take the
  card's text (they took the display's ink — the same in the plain look and under
  Concentus's glass, but of the other polarity on an LCD or a CRT).
- `tests/mod_faces_budget.rs` (timing, `--ignored`): each editor's frame (build +
  tessellate, 200 frames, this machine, the workspace's optimised dev profile, four
  builders compiling beside it), plain look against its face:

| Unit | 100 % plain / face p50 | 200 % plain / face p50 | worst face p95 |
|---|---|---|---|
| Concentus | 0.67 / 0.73 ms | 0.83 / 0.95 ms | 1.56 ms |
| Flexus | 1.26 / 1.40 ms | 1.35 / 1.63 ms | 1.91 ms |
| Phasis | 0.77 / 0.85 ms | 0.87 / 1.13 ms | 1.37 ms |
| Tremor | 0.37 / 0.47 ms | 0.48 / 0.94 ms | 1.44 ms |
| Vibrans | 0.53 / 0.39 ms | 0.53 / 0.44 ms | 0.84 ms |
| Anulus | 0.42 / 0.50 ms | 0.42 / 1.02 ms | 1.36 ms |
| Translatio | 0.33 / 0.42 ms | 0.45 / 0.53 ms | 0.89 ms |
| Fluctus | 0.26 / 0.28 ms | 0.34 / 0.61 ms | 0.78 ms |

  Asserted: a face's p50 within 1 ms of the plain look's and under 8 ms (twice the window
  budget). The dearest is Flexus (a 600-point comb drawn three times for the CRT's glow).

Shots: `docs/round32/skins/modulation/` (`shoot_skins`, every unit in Slate and Paper at
100 % and 200 %, 256-colour PNGs). Looked at: every unit in Slate and Paper at 100 % and
200 %; after the fix round, the header and cards had gone violet (above). Redrawn after looking: the
right voice was grey where it should have been the second voice (the colour test compared
luminance, so amber and blue of one lightness counted as one colour — now the kit's
voices); Flexus's zero-crossing flares were hidden under the dry line (larger, with a halo);
Anulus's gold was a lemon yellow (now brass, 52°).

## The review's fix round (f4d0538c..21a4ee0d)

- **Rising zero crossings** (Flexus through zero): a crossing that landed exactly on a
  sample was skipped when the line rose, so only falling crossings flared.
  `face::crossing_xs` counts every crossing once, rising or falling, a touch of the line
  once. Test `every_crossing_is_found_once_rising_or_falling` (fails before: `[3]` for
  `[1, 3, 5]`).
- **Nothing under the corner words**: Vibrans's tuner scale now starts under the rate
  readout's plate (its own scale between the readout and the plot's foot) and Tremor's
  chart ticks skip the corner words; the movement display paints its words first and
  hands their rects to the marks. Test `the_tuner_and_the_chart_keep_clear_of_the_corner_words`
  at 75, 100 and 200 % (fails before: the pointer at full depth inside the "5.50 Hz" plate).
- **The manual**: the value sits *above* each knob, its name under it (EN, ES); "Tolex"
  (a brand) is "a vinyl amp covering" / "vinilo de amplificador" in the faces' table.
- Shots of Flexus, Vibrans and Tremor taken again (`docs/round32/skins/modulation/`).
- Windows, `-D warnings`: `cargo test -p cantus-ui` — 175 binaries, 1 901 passed, 0
  failed; `cargo clippy --workspace --all-targets` and `cargo fmt -p cantus-ui -- --check`
  clean.

## Shared-file lines touched

`views/fx/frame.rs` (the skin builder's): `face_function` (a new function, 12 lines) and
two call sites (`Header::hue`, the header's style), each marked *Round 32, modulation
builder*. No `kit/*.rs`. `views/fx/mod.rs` untouched (the face module
is declared in `modfx.rs` with `#[path]`). Manual: one paragraph and table per language
after *In another DAW* in *The modulation effects*.

## Needs the owner

Nothing.
