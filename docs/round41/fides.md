# Round 41 — `fides`: Fides, the resonator (ADR 0150)

*The `fides` builder's report. Branch `r41/fides` from `r41/contract`.*

## What was built

**Fides** (`InstrumentKind::Resonator`): an exciter into one or two resonators, coupled, through
a body — the class of Collision, Tension, Sculpture and Chromaphone. The design is ADR 0150;
the player's view is `docs/manual/en/fides.md` (and `es`).

- **Six exciters**: pluck, pick, bow (a friction curve: stick and slip), mallet (Lamina's
  power-law spring pulse), noise (continuous or grains, with its envelope; also a layer under
  any other), live input (the DSP and the unit's `process_input`; the processor's routing is
  the integrator's — see *Left*).
- **Twelve resonators**: a waveguide string, a closed tube and an open pipe (two loops meeting
  at the exciter, Gravis's public tuning maths); a bar, a marimba bar, a tine, a plate, a
  membrane, a drumhead, a bell, a glass, a steel-pan note (banks of up to 48 modes, Fides's own
  cited tables). Two of them, in parallel or A into B.
- **Seven bodies** (none, guitar, violin, cello, harp, box, shell): banks of measured low modes
  and a cloud above, two microphones, continuous in size and ring.
- **MPE**: pressure on the bow's pressure, slide on where it is played, tuning per note.
- **25 factory presets** (the floor 24): the eight named — marimba, vibraphone, kalimba, steel
  pan, harp, nylon guitar, bowed cello, struck glass — and glass harmonica, xylophone,
  glockenspiel, celesta, music box, bell, timpani, frame drum, hammered dulcimer, banjo,
  pizzicato, bowed strings pad, bowed vibraphone, pan flute, rain on glass, gong, crystal pluck.
- **The editor**: Play (the instrument drawn as a plan, the display the control — drag where
  it is played — the partials lit by what sounds; the exciter, resonator, body and output
  cards), Edit (every part as a card), Matrix (the frame's); its own face
  (`kit/skin/fides.rs`: rosewood, gold, a plan's paper).

## How it is tested

- **DSP** (`cantus-dsp/src/fides_tests.rs`, 7 + 2 ignored probes): a new instance silent and at
  rest; every exciter on every resonator (B on and off, serial and parallel, every body,
  vibrato, tremolo, cuts) finite and bounded and sounding; renders repeat to the bit and are the
  same at blocks of 1, 37, 64, 512 and ragged splits; the voices on a pool that runs its jobs
  backwards are the serial bits; the pitch measure finds a sine to 0.001 cent; **pitch within
  0.1 cent at every bend** (string, stiff string, tube, pipe, marimba, tine, pan; E1 to A6; bends
  of 0, ±2, +0.37 st and an MPE tuning of +1.61 st); a bowed string's pitch measured.
- **Engine** (`cantus-engine/tests/inst_fides.rs`, 7): six shapes (the default, a bowed string
  with vibrato, a mallet on a marimba bar into its tube, noise into a tube with cuts and
  tremolo, two voices stolen on a plate and a bell, a macro on the body's size and velocity on
  the decay) through the engine's render at blocks of 1, 37, 64, 512, 4096 and twice, to the
  bit; the extras at their defaults change no sample (B off whatever B says, the cuts at their
  ends, tremolo and vibrato at depth 0, a route at depth 0); MPE pressure presses the bow and
  slide moves it, and another note's expression touches nothing; every shape dies to exact
  silence, comes to rest, and is then as good as new (a fresh unit skipped as far renders the
  next note's bits); an eight-note chord at velocity 127 keeps its headroom (the default −1.3
  dBFS, bowed −5.2); pool = serial through the unit; 32 heavy voices well inside real time.
- **Model** (`cantus-core/src/inst/fides.rs`, 2): the index table follows the list, A's and
  B's keys match, the choices' counts; the extras default to neutral.
- **Undo** (`cantus-core/tests/undo_invariants.rs`, `fides_edits_undo_and_redo_among_the_others`):
  a Fides inserted, every key set and cleared, slot texts set, among the other commands; undo
  all and redo all restore the song.
- **End to end** (`cantus-ui/tests/fides_pick_e2e.rs`, written, runs once Fides is in
  `InstrumentKind::ALL`): listed in both languages on the Keys shelf; chosen through
  `CantusApp::assemble` and `apply_pick`; its model stored; a phrase plays through the real
  engine wiring and is heard; the marimba preset applied as the browser applies it plays.

## Measured

- **Pitch**: struck and plucked, worst 0.0014 cent over the set above (the measure's own floor
  at 41 Hz is 0.001). **Bowed**: the stick and slip move it a few cents from the loop, as a real
  bow does — at the default pressure +3.1 (C2), +2.5 (C3), +1.2 (C4), +0.2 (C5), −1.2 cents (C6);
  softer, C2 −9.0. A correcting "ear" (a period tracker moving the loop) was tried and removed:
  it held C4–C6 within 0.5–0.7 cent but put C2 13 cents out. Bowed tuning within 0.1 cent is
  open (*Left*).
- **CPU** (a debug-optimised build on a loaded machine, three other builds running — not a
  claim): 32 voices of a 48-partial bowed plate with a bell in parallel and the noise layer,
  21.7–37.1 % of real time on one thread (`thirty_two_heavy_voices…`). The bench's
  `inst.fides.v32` is measured when Fides is in `ALL` (the integrator, quiet).
- **Levels**: every exciter on every resonator at C4, velocity 100, brought to about −12 dBFS
  peak by per-kind trims (`examples/fides_sounds -- levels`); the presets at their roles'
  targets (`examples/fides_listen -- fit`): 21 of 25 within 0.2 LU; Timpani −4.3, Frame drum
  −2.7 and Glass harmonica −2.7 LU under, held there by their crest (the fit stops at a phrase
  peak of −1.2 dBFS).

## Shared files touched (each line marked `r41:fides`)

- `crates/cantus-dsp/src/lib.rs` — `pub mod fides;` (the contract declared none for the DSP).
- `crates/cantus-core/src/inst/mod.rs` — `automatable`'s arm for Resonator (`fides::SET_KEYS`:
  `model`, `poly`).
- `crates/cantus-ui/src/kit/skin/instruments.rs` — Fides's sheet (`#[path] mod fides`, `ALL`
  12 sheets, `of()` → `FIDES` instead of Gravis's).
- `crates/cantus-core/tests/undo_invariants.rs` — one test at the end
  (`fides_edits_undo_and_redo_among_the_others`).
- `docs/beta-checklist.md` (a *Ronda 41 — Fides* section), `docs/BACKLOG-excellence.md`
  (appended: what Fides's first round leaves).
- Nothing of Gravis's, Chorda's, Lamina's, Pulsus's or Tympanum's files changed: Fides calls
  `gravis::string`'s public tuning functions and `lamina::voice::HammerShape` as they are, so no
  golden hash of theirs can move; `DSP_VERSION` stays 10.

## Verified only in emulation

- **The sound**: every preset was judged by measurement only — levels (LUFS against the role's
  target), peaks, partial amplitudes, decays, pitch. Nobody has listened. The owner's ear is
  the judge (the files are in `F:\projects\daw-r41\fides-listen\`).
- **MPE**: pressure, slide and tuning tested with expressions written by the tests, never from
  a controller.
- **The look**: the Play view shot on the CPU renderer in Slate Lapis Night and Paper
  (`docs/round41/fides/play-*.png`); not seen on a real screen or at other scales.

## What the owner should listen to and look at

- `F:\projects\daw-r41\fides-listen\NN-<preset>.wav` — each preset on the phrase the preset
  browser auditions it on; `NN-<preset>-notes.wav` — C2, C3, C4, C5, C6 held 1.5 s each. The
  eight named first: **01 Marimba, 02 Vibraphone, 03 Kalimba, 04 Steel pan, 05 Harp, 06 Nylon
  guitar, 07 Bowed cello, 08 Struck glass**. For each: is it *convincing in solo*? What is
  wrong — the attack, the decay, the lows, the highs, the body?
- `docs/round41/fides/play-dark.png` and `play-light.png` — the Play view (the Marimba preset):
  is the picture of the instrument (the bar, its tube under it, the mallet, the partials) the
  face you want, or should a design sheet come first?

## Left

- **`Resonator` in `InstrumentKind::ALL` and the picker** — one commit, after the owner's ear
  on the eight presets (the brief's order): `READY`, `ALL`, the picker row (Keys + Other
  instruments), the agents' `INSTRUMENTS` row and the MCP golden, `inst_contract`'s
  `per_kind!` and count, the learn/offers tests' explanations if any, the null corpus's new
  item and the bench's `inst.fides.v32` measured, `fides_pick_e2e` run. It needs the UI suite,
  which this machine could not hold this session (memory).
- **The live input**: the DSP and `Unit::process_input` are done; the processor does not yet
  hand an instrument a track's audio (the integrator's `processor*.rs`). Until it does, the
  *Input* exciter is silent.
- **A bowed string's tuning** within 0.1 cent (today a few cents, as the physics gives;
  *Measured*).
- The web card (`web/src/units_data.rs`) is the integrator's.

## Review fixes (integration)

*Branch `r41/fix-fides` from `r41/integration` 4f1b74b9e: the integrated review's six findings
on Fides. Each test below was written first and failed on the bug.*

1. **A slow position never moved a modal object.** `Res::step` placed the strike and
   microphone weights again only when the position moved more than 0.002 *since the last
   step*, and the last step's position was stored every step — so an MPE slide (about
   0.0016 a step) or the Position macro (about 0.001 a CC step) never moved a bowed bar,
   plate, glass or pan. It now compares with where the weights were **last placed**
   (`Res::placed`). Test: `a_slow_position_moves_a_bowed_modal_object` (a 0.25 ramp over
   two seconds, 0.00008 a step, on a bowed marimba bar, plate and glass).
2. **The noise generator was frozen at the note's start.** Its band was set once and the
   layer's envelope started only when `noise.level > 0` at the start, so a route or a knob
   on `noise.color`/`noise.width` did nothing mid-note (the blown presets' Macro 1,
   *Colour*), and a route to `noise.level` on a patch whose level is 0 stayed silent.
   `Exciter::follow` now runs on every control step after the first: the band's
   coefficients follow the step (the state runs on — a direct-form filter retuned every 32
   samples, as the tone's low-pass is; no allocation). **The envelope decided:** a layer
   that was not playing comes in when its level rises above 0 while the key is held, on
   its own envelope from the attack (as a layer that starts with the note would); once it
   plays, its level only scales it; after the key is let go nothing starts. The first
   step's modulated level counts too (a velocity route that raises it from the start).
   Test: `the_noise_band_and_level_follow_the_note` (the band moved up an order of
   magnitude in brightness with no sample past twice the steady peak where it moved; a
   level raised mid-note heard from its attack; nothing after the key is up; through the
   instrument, Macro 1 on the colour and on the level of a level-0 patch both heard).
3. **A stage out of use froze its state.** `Body::process` returned before its stillness
   check with no body or a mix of 0, and `Output::process` with both cuts off, so `still`
   stayed false, `Fides::at_rest()` never came back, and the frozen state would ring when
   the stage returned. A body out of use now lets go of its state (and is still); a cut
   switched off clears its filters in `Output::set`, and with both off the stage is still.
   Test: `a_stage_taken_out_mid_ring_comes_to_rest` (body none, mix 0, both cuts off, the
   low cut alone off — each mid-ring: the instrument comes to rest, and the stage put back
   sounds nothing from nothing).
4. **B switched on mid-note was rendered without being started** (another note's kind and
   state, against its own comment). A voice now keeps `b_struck`: B sounds only when it was
   on at the note's start and has stayed on. Switched on mid-note it **waits for the next
   note** and A keeps its level meanwhile; switched off mid-note it is cleared, so it
   cannot come back frozen. Test: `resonator_b_switched_on_mid_note_waits_for_the_next_note`
   (until the next note, the render is the B-off render to the bit, both for on-mid-note
   and for on, off, on again; the next note has B).
5. **A checkpoint's image counted Fides at the contract's 2 KB.** A fork of the real unit,
   counted by the allocator (`cantus-engine/tests/fides_image.rs`): **1 347 KB at 48 kHz**,
   1 265 at 44.1, 2 360 at 96 — the 36 voices' waveguide lines (two a voice, sized for
   20 Hz) grow with the rate; `approx_bytes` scales every unit by the rate's ratio, which
   holds Fides from above. The other figures came from the working-set probe
   (`listen::tests::what_each_unit_adds_to_an_image`), which this loaded machine could not
   read (its deltas went negative: a freed image's pages reused); counted the same way
   beside their figures, a small unit's working set runs 300–700 KB over its own bytes and
   a large one's is level (Chorda 5 621 counted, 5 150 figure), so Fides takes **1 400 KB**
   and the test holds it within 10 %. The probe now measures kinds not yet in `ALL` too.
   *For the integrator:* Tympanum's fork counts **7 171 KB** against its stub's 2.
6. **`fides_pick_e2e`'s picker test needs Fides in `ALL`.** Marked
   `#[ignore = "runs once Fides is in ALL — the owner's listening verdict (docs/round41/fides.md)"]`
   (only `the_picker_lists_fides_in_both_languages_on_the_keys_shelf`);
   `choosing_fides_adds_a_track_that_plays_a_phrase_and_a_preset` passes without it, and
   `inst_faces` and `inst_shell` only name Fides in an exhaustive match (they iterate `ALL` or
   their own lists, neither with Fides). The probe below lives in the same file.

Also, for `clippy --all-targets -D warnings` (Rust 1.97): five lints in `fides_body.rs` and
`fides_modal.rs` (range loops to iterators, a collapsed `if` — the same arithmetic, the
same bits) and `inst_fides.rs`'s `f32::log10` → `cantus_dsp::math::log10`.

**What changed in sound.** The factory presets at their defaults, each on its audition
phrase and on its five notes through the engine
(`fides_pick_e2e::probe_the_factory_presets_bits`, 50 renders, bits before and after):
**all 50 the same bits** (the render after every fix, the lint refactors included). The fixes change only what was broken — a position moving slowly on a modal
object, a noise colour or width moved mid-note, a noise level raised from 0, a stage taken
out of use and brought back, B switched mid-note.

**Verified only in emulation:** an MPE controller's slide and a hardware knob on the
noise's colour — the tests write the expressions and the macros.

## Fides in ALL (integration)

*Branch `r41/fides-all` from `r41/integration` e354510b7, after the owner approved Fides by
ear (2026-11-21).*

**The ALL commit.** `READY` true; `InstrumentKind::ALL` eleven (Fides before the dancer);
the picker's row (`fides`, Keys and Other instruments — `the_picker_lists_fides_in_both_
languages_on_the_keys_shelf` runs, no longer ignored); the agents' `INSTRUMENTS` row
(`fides`, *Fides*) and the MCP golden written again (`CANTUS_TOOLS_DUMP=1`, `ai_mcp_boot`:
the diff is Fides's id in the two instrument enums and its line in their descriptions,
nothing else); `inst_contract`'s `per_kind!` (`fides`) and its count (11); `listen.rs`'s
probe of kinds waiting outside `ALL` left empty; `demos`' *not yet* list names Fides (no
demo plays it: a demo that does changes that demo's sound and hash, the integrator's
call). The null corpus's `inst.fides.v8` and the bench's `inst.fides.v1/v8/v32` come from
`ALL` as every instrument's do (`kernels::instruments`; `the_null_corpus_instruments_all_
sound` counts it); no stored baseline lists instruments (`perf/baselines` holds the
laptop's quick run only), so there is no number to enter: the quiet measurement is the
integrator's.

**What the kind-wide tests found** (each with a test written first that failed on it):

1. *The contract's split, voices and at-rest twins* (`inst_contract`, 3 of Fides's 4
   failed): a block that began while a waveguide's lengths glided (a release, a bend)
   scaled the pluck's last leak by another period — the strike's gain is now scaled by the
   period taken at the control step (`a_string_released_while_its_pluck_still_leaks_is_
   the_same_at_every_block_size`). And the output stage was told its input was zeros
   whenever no voice played at a chunk's start, while the body still rang, and skipped its
   gain and tremolo there (`the_body_s_tail_after_the_last_voice_is_the_same_at_every_
   block_size`).
2. *A waveguide's loop held a DC level for ever*: its loss filter is a low-pass whose DC
   gain is capped at 0.99999, and a pluck's leak (a released displacement) leaves DC in the
   loop; the DC settled on an `f32` fixed point — voices that never ended (their slots,
   their CPU), an offset on the output up to −46 dBFS in the probe and **−28 dBFS on the
   nylon guitar's phrase** (its old render never went silent). A DC blocker in the loop
   (`(1 + R)/2 · (1 − z⁻¹)/(1 − R z⁻¹)`, corner `f0 / 64`), its phase lead counted in the
   tuning and its loss at f0 taken back from the loss filter's (never past the cap, so the
   loop stays under 1 at every frequency); out of the loop while a bow is drawn (its lead
   carried the bowed tuning 6–7 cents flat at `f0 / 32`; a bowed note is the same bits as
   before until the bow lifts), in when it lifts. The stiffness is fitted to the loss
   filter alone, as before (counted in the fit, the lead tipped the nylon's slight
   Inharmonicity from no allpass to one: +1 to +3.5 dB). `a_released_string_ends_whatever_
   its_decay_and_material` (plucked and bowed, three decays, two releases, three
   materials, C2–C6: every voice ends within ten seconds of its release). The pitch tests
   hold (0.1 cent); the bowed tuning is unchanged to the bit.

   **What it changed in sound** (`probe_the_factory_presets_bits`, 50 renders against the
   integration's, `FIDES_BITS_BASE`): 18 the same bits (the modal presets without a body
   ring: kalimba, glass, glass harmonica, glockenspiel, bell, timpani, rain on glass, gong
   and a steel pan's and a music box's render), 6 within −148 dBFS (celesta, frame drum, the
   other two: the out-stage fix on a tail); the waveguide presets' RMS within 0.2 dB (marimba, vibraphone and xylophone
   −0.03 — their tube), but the nylon phrase −0.68 dB (the −28 dBFS DC it lost), the pan
   flute's notes +0.7 dB (its upper notes +1.4 to +1.8: a noise-driven pipe whose loop no
   longer integrates the noise's DC) and the dulcimer phrase −0.2. The partials above the
   fundamental sit up to 4.3 cents flat of where they were (the blocker's lead). **The
   owner should listen again to the pan flute, the nylon guitar, the harp, the banjo, the
   dulcimer, the pizzicato and the crystal pluck** in `F:\projects\daw-r41ides-listen-r41all\`
   (`examples/fides_listen`, the same phrase and notes as `fides-listen\`): every preset
   within 0.3 LU of its role's target but the three held by their crest — timpani −4.3,
   frame drum −2.7 (as before) and glass harmonica −1.9 (its gain +0.8 dB, its phrase peak
   −0.5 dBFS).

**The words** (cantus-6f's shot of the site card: Spanish labels cut in the window;
`r41_whole`'s Fides test, from `r41/no-elision`, listed them all — now un-ignored and
passing; `fides_words`, new: Play with each of the six exciters, Edit and Matrix, at 100
and 150 %, Night and Paper, both languages, in the sweep's plain kit). Two looks, two
fixes:

- *Fides's own face* (the window a user sees): the Spanish names reworded to fit a cell
  whole, never a smaller font — *Tono/vel.*, *Chasquido* (the strike's noise), *Inarmonía*,
  *Cuerpo* (the body's mix, in the body's card), *Caída/tecla*, *Corte bajo / Corte alto*,
  *Frec. vib.*, *Demora vib.*, *Frec. trém.*, *Rango bend*, *Seg. tecla*, *Vel. arco*,
  *Desliz. MPE*, *Marimba* (the resonator); the body's seven kinds in two rows (four and
  three) as the resonators' twelve are, so *Ninguno* and *Guitarra* are whole.
- *The plain kit's capitals* (`ux_whole_words`, which reaches Fides now that it is in
  `ALL`): a 58-pt cell holds about eight capitals, and English was cut there too
  (VELOCITY TONE, INHARMONICITY, TRANSPOSE …): a designed short form (`<key>.short`, the
  house's way, as Chorda's) for every label too long, in both languages (28).
- The factory presets' macro names are locale keys (`fides.macro.*`: *Dureza, Caída,
  Cuerpo, Posición*); the macro band shows a stored name that is a locale key in the
  window's language, any other as typed (`frame::macro_name`).
- The device line, cut at 180 pt in the Devices pane: *Plucked, bowed and struck* /
  *Pulsado, frotado y golpeado*.
- Shot in both languages (`shoot_skins fides --es`): `F:/projects/daw-r41/fides-all-shots/{en,es}/fides-slate-100.png`.

**The face's accent: gold → copper (8°).** The house rule (`skin::side` and
`skin::modulation`'s tests, which reach every unit in `ALL`): no two faces share a
material and an accent within 20°. On wood every gold is taken — Vetus's walnut at 31°,
the ring modulator at 52° — so Fides's rosewood now wears copper (a wound string's), its
dark lightness raised to 0.73 for the accent's 4.5 : 1 on the deck and the title; the teal
of what rings stays. The owner had not judged the face (fides.md asked whether a design
sheet should come first): his eye decides it.

**Tests run** (Windows, this branch): `cantus-dsp` fides 14 (+2 probes ignored);
`cantus-engine` inst_contract 51, inst_fides 7, fides_image 2, listen 14, render_from_bar 7,
rt_memory 63; `cantus-core` undo_invariants 16; `cantus-ai` 58; `cantus-bench` bench 9;
`cantus-ui` lib (all), r41_whole 4 (Fides's test un-ignored), fides_words 2,
fides_pick_e2e 2 (+ the bits probe), ux_whole_words 1, learn_everywhere 12, demos 13,
instrument_reference 1, preset_render 2, preset_audit 2, ai_mcp_boot 1, agent_tools 14,
controllers_review4 33, picker 14, frame_budget 19, gain_staging 5, id_clash 3,
inst_frame 4, inst_shell 3, inst_faces 2, inst_views_scroll 1, instruments_scale_e2e 3,
library_fixes 3, modulate_everywhere 5, dropdowns_audit 1, stated_preset_counts 1,
views_budget 1, preset_browser_e2e 3, preset_browser_review 8, room_card_layout 1,
ux_help_line 1, tympanum_words 2, tympanum_pick_e2e 2. Not run here: the Linux check
(WSL is the integrator's), the bench's timed run (quiet machine).

**Left for others:** the Inharmonicity knob does nothing below a threshold that grows
with the loss (at material 0.32, 0.12 fits no allpass: `dispersion_fit`'s `need ≤ 0`) —
measured here, not changed (it would move the nylon guitar).

**The live input is not built: ADR 0150 does not decide it.** It says only "*input* — a
track's audio"; which audio, from where, with what latency and what a render or a freeze
hears are open, and the brief said not to invent them. The three readings, with what
each costs (the DSP, `Fides::process_input` and `Unit::process_input`, take a mono block
the length of the call; nothing of them changes with the choice):

| | A. Its own track's device input | B. Its own track's audio clips | C. A sidechain from a strip |
|---|---|---|---|
| What excites it | the interface channel(s) `Track::input` names (ADR 0025), as monitoring hears them | the clips on Fides's own track, before they reach the strip | another strip's audio, chosen in the Input card as an effect's key is |
| Latency | the driver's round trip; the block is in hand when the track runs (`InputBlock`), no block added | none (same pass, same thread) | none: the graph orders the source strip first (`plan_compile`'s sidechain edge); a source with latency needs the key's compensation |
| Offline render / freeze | **differ from real time by nature** (no device offline): a render hears silence or must use the take recorded on the track | equal to real time; a freeze bakes it | equal to real time; a freeze must wait for its source (the source is not frozen with it) |
| Model and undo | none (the track's input exists) | none (but the clips no longer go to the strip while Fides takes them — a rule to state) | a field on the instrument (`sidechain: Option<StripId>`, schema bump, migration, fixture), a `Command` with its inverse, an undo-fuzz arm, cycle refusal as for effects |
| Real-time work | a copy of the track's channels into a preallocated mono block | the clips' render into a preallocated block before the unit | the source strip's output read from its cell (as an effect key reads it) |
| What it is for | a mic or a guitar played through the resonator live | a recorded loop or a take through it | any track drives it (a drum bus exciting a marimba, a voice a string) |

A would be the "live" in "live input" and breaks *offline = real time* unless its render
rule is decided; C is the house's own pattern (an effect's key: its strip runs first, `strip_order`) and
the most useful in a mix; B is the cheapest and the least expected. Until one is chosen
the *Input* exciter is offered in the editor and is silent (with its own level, attack,
lift and tone knobs): either the choice lands before beta.17, or the exciter's switch
hides *Input* until it does — the integrator's or the owner's call.
