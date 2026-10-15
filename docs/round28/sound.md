# Round 28 — sound: an acoustic drum kit to download, then Chorda

*Builder: sound · branch `r28/sound` (from `r28/integration` at `405bd9e`) · ADR 0042
(amended), ADR 0023 (amended), ADR 0059 · schema 28 on this branch (30 in the round's
table) · BACKLOG §5, §3e.*

**Both parts are done.** A: DRSKit 2.1 (DrumGizmo, CC BY 4.0) installs in one step from
Sound packs, mixed to stereo at install, a General MIDI kit in Exemplar with chokes,
velocity layers and round robins that never repeat a hit in a roll. B: Chorda, the
modelled grand piano, meets every measurable target of its *done when* and is in the picker
and the ballad demo. What stays is the owner's (the end of this file).

## A. The acoustic drum kit

**The kit, its licence read at the source** (ADR 0042's round-28 amendment has the whole
reading). **Naked Drums is refused**: the CC BY 4.0 licence on its SFZ port was put there
by the people who converted it; Wilkinson Audio, the maker, never stated a licence (it sold
the kit, then gave it away in 2016; its archived pages name none). **DRSKit 2.1 is
accepted**: DrumGizmo states CC BY 4.0 on the kit's page and in the `README.md` inside the
archive; their sentence about crediting it in compositions is quoted word for word on the
pack's page and, now, in About ▸ Credits. The archive's published MD5 matched the download;
the manifest pins its size (2 803 397 710 bytes) and our SHA-256.

**One step, verified, credited** (`cantus-content`): manifest format 3 gives a pack a
**build** step — `drumgizmo.rs` reads the kit (13 microphones, channels stored in another
order than the kit lists them), `kitbuild.rs` mixes each hit to 24-bit stereo (per
microphone gain, equal-power pan, polarity, bleed on the drums it does not face; per drum
balance; deterministic sums), measures every hit, writes the SFZ (velocity layers by level
on SFZ's curve, round robins of three per layer, General MIDI keys plus DrumGizmo's extras,
the hi-hat choke with a 60 ms release, the groups named by the manifest's labels, the
credit at the top) and prunes what it does not keep; the install's budget is the build's
unpacked size; a failed build installs nothing. Sound packs shows *Mixing the microphones:
n of m hits*. The pack is on the picker's **Drums** shelf, with 64 voices and Exemplar's
**drum rows** on (the piano roll and the channel rack show a row per drum by name).

**What Exemplar gained** (ADR 0023's round-28 amendment): **round robin in order** — a clip's
note takes its turn by its ordinal (`NoteEvent::ordinal` through `note_on_seq`), so a roll
never plays the same hit twice and a render from any start cycles alike; live notes cycle
per key; `rr.order` (By note / In order) in the tools row, schema 28 writing *By note* into
every older Exemplar so old songs keep their sound (v28 fixture). **Drum rows** (`kit`):
`Lanes::Keys` named by the SFZ groups' `group_label` or General MIDI's names. SFZ
`group_label` names Exemplar's groups.

**The mixdown, measured** (ADR 0042's table): a stereo mix at install (490 MB before review 1, 471 MB after; 123 MiB of
heads, one voice a hit, 591 zones) against a stem per bus (1.47 GB, 1 773 zones) and every
microphone a zone (3.2 GB, 7 683 zones). Faders per microphone need an instrument with
several outputs (BACKLOG §3e item 11): left there.

**Measured on the real archive** (ignored tests `kit_build::the_real_drskit_installs_mixed_
from_its_archive`, `CANTUS_DRSKIT_ARCHIVE`; `pack_drums::the_real_drskit_plays_a_groove_at_
the_calibrated_level`, `CANTUS_DRSKIT`): installs in 104–128 s to 527 files, 489 577 116
bytes, peak −4.9 dBFS; imports in 0.09 s; a groove at −26.2 LUFS / −7.4 dBTP, everything at
velocity 127 at −0.9 dBTP (gain 0 dB by ADR 0039's rule); 186 velocity layers, each with at
least two hits in turn. Shots: `F:\projects\daw-r28\shots\sound\` (Sound packs before and
installed, the picker, About, Exemplar with the kit, the rows; 75/100/200 %, EN/ES).

**Tests:** `cantus-content/tests/kit_build.rs` (19), `cantus-dsp` `exemplar_tests4.rs` (5),
`cantus-core/tests/sampler_kit_lanes.rs` (3), `cantus-engine/tests/inst_sampler.rs` (2 new),
`cantus-project` schema 28, `cantus-ui/tests/pack_drums.rs` (6), `about.rs` (DRSKit's
credit and its authors' words).

## B. Chorda — the decision, before code (2026-10-15)

**Decision: Chorda can be finished this round to the measurable part of its *done when*,
and I build it now.** What follows is why, what "finished" means in numbers, how the
patents are avoided, and what stays the owner's.

### Why it can be finished

- **The engine's pieces exist.** Gravis's bidirectional waveguide string
  (`gravis_string.rs`: rails, phase-delay tuning to a fraction of a cent, a one-pole loss,
  first-order dispersion allpasses), Lamina's hammer shape and strike scaling
  (`lamina_voice.rs`), the voice allocator (`voices.rs`), voices split across the pool
  with a post-voice stage summed in voice order (ADR 0038: Unda, Calor and Exemplar already
  do it), the modulation matrix, the kit shell (Play / Edit / Matrix) and the instrument
  contract tests (`inst_contract.rs`: block size, determinism, headroom, silence). Chorda
  is new physics on a known chassis, not a new chassis.
- **The physics is published and closed-form.** Waveguide strings (Smith, *PASP*), a
  dispersion filter designed to the stiff-string law `f_n = n f₀ √(1 + B n²)` (Van Duyne &
  Smith 1994; Rauhala & Välimäki 2006; a least-squares phase fit), loss filters from two
  decay times (Välimäki et al. 1996), a hysteretic felt hammer (Stulov 1995) solved against
  the string's wave impedance with a fixed number of iterations (Chaigne & Askenfelt 1994;
  Bank 2000), unisons coupled at a bridge admittance (Weinreich 1977: beating and two-stage
  decay), phantom partials from tension modulation (Bank & Sujbert 2005), dampers and the
  pedal as felt losses, and a soundboard as a post-voice filter. The data the brief asks
  for is readable here: Balázs Bank's thesis (measured string parameters, decay times and
  the soundboard model; `home.mit.bme.hu/~bank/thesis`), Rigaud, David & Daudet's
  parametric models of inharmonicity and tuning across the keyboard (DAFx 2011, from
  measurements of several pianos), Chaigne & Askenfelt's strings, and the perceptual
  tolerance of decay (Tolonen & Järveläinen, −25 % / +40 %, via Bank et al. 2003).
- **The cost fits.** A voice is one to three strings of ~100 operations a sample (a delay
  line, a fractional delay, 4–16 dispersion sections, a loss filter, the bridge); 64
  pedal-down voices are ~20 k operations a sample, and the shared stages (the sympathetic
  strings at the keys' own pitches, the soundboard) a few thousand more — inside 25 % of one
  core at 256 frames if written for it (independent strings side by side, coefficients
  computed off the audio thread), and split across the pool besides.

### What "finished" means — the tests that will hold it

1. **Tuning:** on every one of the 88 keys, one string alone (unisons off), every partial
   below 5 kHz (at most 32) within **1 cent** of the target `n f₀ √(1 + B n²)`, with `f₀`
   from the stretched tuning and `B` from the inharmonicity model across the keyboard —
   measured on the rendered sound (FFT, interpolated peaks over seconds), not computed from
   the filter's formula.
2. **Decay:** each partial's T60 measured on the rendered sound (unisons off, pedal up, key
   held) within **−25 % / +40 %** of the published measurements at the notes they were
   published for, and of the model fitted to them in between.
3. **Half-pedal:** continuous — the decay of a held note changes monotonically and smoothly
   as the pedal moves from up to down (no step between two models, no click), and
   re-pedalling (up and down again within a note) leaves no discontinuity.
4. **Cost:** 64 voices with the pedal down inside **25 % of one core at 256 frames**
   (measured, best of several runs, on this machine; the number in ADR 0059).
5. **The family's rules (ADR 0023):** a control at its default changes no sample, the same
   bits at block sizes 1, 37, 64 and 512, offline equal to real time, parallel equal to
   serial, exact rest once the sound has died, headroom (no factory preset past 0 dBFS on
   the dense riff), deterministic randomness (hashes of the note).
6. **The product:** a `model_version` stored per instance, presets made with care, the kit
   shell with a picture of what plays (strings, hammer, pedal), `docs/instruments/chorda.md`,
   the manual in both languages, `READY = true`, a demo song.

### The patents (US; Google Patents' status, not a legal conclusion)

- **Modartt US 7,915,515** (to 2027-10-20): no presynthesis of per-partial frequencies and
  dampings from a soundboard parameter for a partial-based synthesizer. Chorda's strings are
  waveguides — partials are not synthesised one by one — and the dispersion and loss
  filters are designed from the string alone (its `f₀`, `B` and decay times); the
  soundboard is a separate filter after the strings.
- **Casio US 8,178,772**: no resonators tuned to delays that do not match the keys' pitches.
  The sympathetic strings are the keys' own strings, at their speaking pitches.
- **Yamaha US 10,818,278**: no second loop at a non-speaking pitch (no duplex scale model).
- **Yamaha US 8,729,376**: no delay loop excited by a band-split sampled waveform — the
  hammer is a physical model, no sample excites anything.
- Kawai US 9,330,648, Casio US 10,109,268 / 11,881,196, Yamaha US 12,039,960: their
  structures (stored resonance in a sample player, register IRs, excitation routed to
  low-string channels, a noise-derived damper envelope) are not used.
- The legal review against every claim stays the owner's, before release
  (`docs/beta-checklist.md`).

### What stays the owner's

The blind listening test (≥ 20 listeners against Pianoteq 9 and a sampled grand), the
legal review, and the presets' last word by ear.

### If a number cannot be met

If one of the four measurable targets proves out of reach with the published approach,
Chorda stays hidden (`READY = false`), this file says which and by how much, and the
integrator decides — nothing half-done reaches the picker.

## B. Chorda — what was built and measured

Built as decided above; ADR 0059 holds the decisions, `docs/instruments/chorda.md` the
instrument. In short: published tables per key (Rigaud et al.'s inharmonicity and aural
tuning, Bensa et al.'s strings, Chaigne & Askenfelt's hammers); waveguide strings with a
designed dispersion (Abel et al.'s second-order allpass sections refined by
Levenberg–Marquardt), a linear-phase loss filter and a resistive bridge (two-stage decay);
Stulov's hysteretic hammer; phantom partials; dampers with a continuous half-pedal law; the
soft pedal; the keys' own strings ringing in sympathy; a multirate soundboard and the
radiation's rise; 88 voices across the pool, summed in key order; designs in a lock-free
registry computed beside the window (at start-up, after a device change, when a project's
units are built), an offline render waiting for its own.

| Done when | Measured | Held by |
|---|---|---|
| Every partial within 1 cent of the stretched target, every key | designed: worst 0.47 ct (44.1 kHz), 0.49 ct (48 kHz); on the rendered sound: worst 0.40 ct | `chorda::tests_design`, `tests_string` |
| Per-partial T60 within −25 % / +40 % of published | 0.96–1.01 of Bensa et al. at C2, C4, C7 | `tests_string::a_string_decays_as_bensa_et_al_measured` |
| Half pedal continuous | 241 → 41 dB/s from up to down, monotonic, largest step 28 % of the range; re-pedalling without a jump | `tests_piano` |
| 64 pedal-down voices < 25 % of a core, 256 frames | 18.2–19.1 % (release, best of three, this PC) | `measure_sixty_four_pedal_down_voices` (ignored) |
| Null at defaults, block sizes, offline = real time, parallel = serial, exact rest | to the bit | `inst_chorda.rs` (engine), `inst_contract.rs`, `tests_engine` |
| The product | `model` stored; ten presets with four macros; Play / Edit / Matrix with the piano drawn from its numbers; manual EN/ES; `READY = true`; the ballad demo | `cantus-ui/tests/inst_chorda.rs`, `demos.rs`, `picker.rs` |

Also measured: prompt sound 24.6 dB/s against aftersound 8.7 dB/s at middle C; sympathy −17
dB with the pedal against −32 dB without; both hands fortissimo with the pedal −3.3 dBFS, a
mezzo-forte middle C −22.7 dBFS; every preset under −6 dBFS on the dense riff; a keyboard's
design about two seconds on eight threads.

**What the contract caught.** The first engine counted its 64-sample chunks from each
block: the sympathetic strings looked ahead in the chunk for drive and learnt which keys
sounded at chunk starts, so blocks of 1 against 512 differed (`split::chorda`,
`voices::chorda`, the skipped twin). The chunks now end on the stream's own grid, the
dampers learn on it, the strings wake on drive seen, and `tests_engine::notes_and_pedals_
inside_a_block…` (a chord at frame 0, staccato notes dying mid-block, pedal moves on odd
frames) fails on the old code and passes now.

**Schema.** None for Chorda: the kind and its keys are new (no old project holds one); the
shares are fractions shown in %, like every other instrument's. Schema 28 on this branch is
Exemplar's `rr.order` (A); the round's table numbers it 30 — the integrator chains it.

**Shared files touched (additive):** `views/inst/mod.rs` (`InstEditors::chorda`),
`views/inst/organ_parts.rs` (Chorda's placed cells), `app.rs` (warm the designs at
`assemble` and after a device change), `views/about.rs` (an author's note under its credit),
`demos/ballad.rs` (its piano is Chorda), tests `picker.rs` (the count follows the ready
kinds), `instrument_levels.rs` (Chorda's presets), `demos.rs` (a comment),
`about.rs` (DRSKit's credit), `engine/tests/rt_memory.rs` (Chorda's voices).

## Review 1 (2026-10-15)

`F:\projects\daw-r28\reviews\sound-1.json`: 35 confirmed findings, which come down to 26 once
duplicates across lenses are merged. Every one is fixed. Each has a test that I saw fail on
the code before the fix, except where noted below.

**Chorda**

| Finding | Fix | Test |
|---|---|---|
| HIGH: a sympathetic string had one inversion in its loop and rang at `(k−½)·f₀` | Two inversions (the nut's and the bridge's), as in a voice. The drive is high-passed at 20 Hz. The bridge's reflection loss keeps the loop under unity at DC. The loss filter is set from the loop's own trip time at each partial. | `a_sympathetic_string_rings_at_its_keys_own_pitch`: f₁ and f₂ sit 70–145 dB over f₀/2. Before the fix the string rang at f₀/2. |
| HIGH: automated A4/stretch/stiffness were never designed (an export waited 120 s per block) | `InstrumentKind::automatable`. The engine ignores a lane on these keys, the knob offers no clip, and the app refuses one with the reason. Offline, `ensure` designs on the spot, once. | engine `a_lane_on_the_tuning_is_inert_and_an_export_never_waits_for_it`; UI `the_tuning_is_set_never_automated`. On the old code the first of these would hang, so I did not run it there. |
| HIGH: a knob drag started a design per frame and filled the registry | One designer thread works one design at a time. A track's tuning is designed only once it has settled for 250 ms. Values are rounded so close tunings share a design. The registry holds 128 entries, and a tuning refused for lack of room is remembered and not tried again. | `a_dragged_tuning_designs_only_where_it_settles` (60 values asked, only the last designed) |
| HIGH: loading Chorda or changing the sample rate froze the interface thread | `Unit::new` never waits. Until the design arrives the piano is silent and the editor says *designing the strings: n %* (`progress`). An export waits for its own design. | `a_unit_never_waits_for_its_keyboard` (made in under 100 ms, silent, then plays); `while_the_strings_are_designed_the_editor_says_so` |
| A sympathetic string was frozen while its key's voice sounded and replayed as a ghost | The string is silenced when its voice starts and comes back silent, with its damper where the pedal leaves it. | `a_struck_string_leaves_no_ghost_in_its_sympathetic_string` |
| The sympathetic strings' dampers ignored the grip setting | `Sympathetic::set_grip`; the resting state follows it. | `without_dampers_the_strings_ring_in_sympathy` (−16 dB; −35 dB before) |
| A new keyboard landing mid-performance hard-reset strings | A ringing string, or a ringing voice, keeps its layout until it is silent. | `a_new_keyboard_never_cuts_the_strings_ringing` |
| A decay or coupling change did not reach a re-struck voice | Every strike recomputes the voice's loss; `set_decay` does the same for the sympathetic strings. | `a_new_decay_reaches_a_ringing_note_struck_again` |
| A new voice started with its damper down | A strike from rest lifts the damper. | `a_fresh_strike_finds_its_damper_already_lifted` |
| `model_version` was never stored | `NativeSpec::new` stores `model` for Chorda, and `read_unversioned` reads an absent key as 1. No schema bump is needed. | `a_chorda_stores_its_model_and_a_preset_file_keeps_versions` |
| "1 cent on every key" was measured on 8 keys | The test now renders all 88 keys at 44.1, 48 and 96 kHz, at A4 415 Hz with 70 % stretch, and at 150 % stiffness. | `every_key_rings_within_a_cent_at_every_rate_and_tuning`: worst 0.47–0.50 ct. It passed before the fix too: the claim was true, just under-tested. |
| The boot test passed without the boot path | It now has its own binary and checks that the keyboard is not warm before `assemble`. | `inst_chorda_boot` (seen failing with the warm-up commented out) |
| The inspector showed stretch as "1 %", and "A4" stayed English in Spanish | `preview_text` | `the_inspector_reads_the_tuning_as_the_knobs_do` |

**The drum kit and Exemplar**

| Finding | Fix | Test |
|---|---|---|
| HIGH: `kit_lanes` panicked on a zone with lo > hi | `hi = hi.max(lo)`, as the engine plays it | `a_zone_whose_high_key_is_below_its_low_one…` (panicked before) |
| Non-GM sounds sat on GM percussion keys | GM keys now hold the kit's own drums only. The other articulations move to 82–100, and GM keys the kit has no sound for stay silent. | `the_bundled_kit_plays_general_midis_keys_with_their_own_drums_only` |
| Round robin counted per pitch, so a tom fill or an accent layer repeated a sample | The snapshot counts turns per round-robin lane: the notes that reach the same samples (`RrLanes`). | core `turns_are_counted_by_drum_and_velocity_layer_not_by_key`; `pack_drums::a_drum_on_two_keys_and_an_accent_layer_take_their_own_turns` |
| The +24 dB gain cap conflicted with range_db 30 | The range is capped at 24 dB (`validate`), DRSKit uses 24, and `layers` drops any hit beyond. | `a_range_beyond_what_a_zone_can_make_up_is_refused`; `layers` test |
| A cancel, failure or crash during the mixdown threw away the download | The archive is removed only after the pack is in place. | `a_build_that_fails_keeps_the_download_and_a_good_install_lets_it_go` |
| Disk space needed during install was neither stated nor checked | `Pack::space_needed` is shown on the pack's page. The install is refused before any fetch when the disk lacks it (`NoSpace`, `cantus_rt::disk`), with the reason in both languages. | `an_install_without_room_is_refused_before_anything_is_fetched` |
| A pre-round-28 Exemplar user preset changed its round robin | `rr.order` is spelled out on save and read as 0 when absent. | in `a_chorda_stores_its_model_and_a_preset_file_keeps_versions` |
| The channel rack rebuilt the kit's rows every frame | `InstrumentSpec::is_chromatic`, plus no `format!` in `kit_lanes` | — (no new test; the rack and piano roll tests still pass) |
| Drum-row names were English-only | `views/drum_names.rs`: GM's 47 names and DRSKit's labels, in `drums.{en,es}` | `a_kits_rows_read_in_the_interfaces_language` |

The schema step is renumbered by the integrator (`migrate_29_to_30`, `v30.cantus` in
`r28/integration`); I left it as it is here.

**The real DRSKit after the fixes** (ignored tests, re-run on the archive)

- Installs in 113 s: 496 files, 470 879 054 bytes (the manifest pins this size).
- About 7.6 GB free is needed while it installs.
- Imports 548 zones and holds 117 MiB in memory.
- A groove plays at −26.0 LUFS / −8.2 dBTP; everything at velocity 127 reaches −0.8 dBTP.
- 170 velocity layers.

**Chorda's CPU after the fixes:** 64 pedal-down voices take 19.5 % of one core (release).

## Left

Owner-only (on `docs/beta-checklist.md`):

- Chorda's **blind listening test** (≥ 20 listeners against Pianoteq 9 and a sampled grand)
  and the presets' last word by ear.
- Chorda's **legal review** against the patents ADR 0059 names.
- Playing DRSKit and Chorda by hand (an 88-key keyboard with a continuous pedal).
- Asking Wilkinson Audio for Naked Drums' licence in writing, if the owner wants that kit
  too (ADR 0042).
- `F:\projects\daw-r28\downloads\` keeps the DRSKit archive (2.7 GB, for
  `CANTUS_DRSKIT_ARCHIVE`), the installed mix (`drsmix`, 469 MB, for `CANTUS_DRSKIT`) and the
  papers read (15 MB); `drsmix` was rebuilt after review 1. The unpacked kit (4.1 GB) is
  deleted. Keep or delete.

Nothing else is open.
