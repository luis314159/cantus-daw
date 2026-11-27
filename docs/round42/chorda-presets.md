# Round 42 — `chorda-presets`: the voicing by hand and the character presets

**Status: done, awaiting the owner's ear.** Branch `r42/chorda-c17` (the same worktree) from 7d85795ef. ADR 0098's
eighth amendment. Schema 88 (85–87 held, empty, for the integration's: 86 Tympanum's, 87
Arcus's). No `DSP_VERSION` change: every control at its default is the shipped voicing to the bit.

The owner (2026-11-26): character presets — *a very dark one, a very imposing one, and so on*,
each still a real instrument — and *"el usuario debe poder mover cada uno de los parámetros a
mano"*.

## 1. The voicing by hand (Part A)

`cantus_dsp::chorda::user::UserVoicing` over the shipped `Voicing`; parameters `voicing.*`
(`cantus_core::inst::chorda`), grouped by `VOICING_GROUPS`; the **advanced voicing**, the second
page of Chorda's Edit view (`View::Voicing`; the switch *Parts · Advanced voicing* at the top of
both; a fifth header segment cut the instrument's name in Spanish at the narrowest window). Six
cards, each with its picture from the voicing itself (the technician's number dashed, the one set
over it); a row whose cards' cells would shrink stacks them. Screenshots:
`F:\projects\cantus-work\listen\chorda-presets\capturas\` (Slate Lapis and its paper, En and Es).

| Group | New controls | Range (default) | Chorda's own, shown with them |
|---|---|---|---|
| Hammers | Bass hardness, Treble hardness (`felt_bass`, `felt_treble`) | −100 … 100 % (0) | Hardness |
| | Hammer mass (`mass`) | 50 … 200 % (100) | |
| Strings | Bass / Middle / Treble sustain (`decay_*`) | 25 … 400 % (100) | Decay, Unison |
| | Overtone sustain (`hf`), Overtone spread (`hf_spread`) | 25 … 400 %, 0 … 200 % (100) | |
| | Unison unevenness (`uneven`), Second polarisation (`polar`) | 0 … 300 %, 0 … 400 % (100) | |
| Soundboard and case | Bass formant (`formant_hz`), gain (`formant_db`), Q (`formant_q`) | 150 … 1 500 Hz (600), −12 … 24 dB (12), 0.5 … 8 (3) | Soundboard, Bridge coupling, Sympathy |
| Action | Key knock (`knock`), Knock tone (`knock_tone`) | 0 … 400 % (100), −2 … 2 oct (0) | Action noise |
| | Key return (`key_return`) | 0 … 300 % (100) | |
| | Damper strength (`damper`), Bass dampers (`damper_bass`) | 25 … 400 % (100) | |
| | Damped floor (`damped`) | −120 … −30 dB (−70) | |
| Microphones | Arrival spread (`itd`) | 0 … 300 % (100) | Mic position, Width, Lid, Room, Room size |
| Tone | Top roll-off (`top_hz`) | 2 … 20 kHz (11.06 kHz) | Tilt, Colour |
| | Voicing amount (`amount`: each key's partial table) | 0 … 200 % (100) | |

Not exposed: the keyboard's design (made off the audio thread; the tuning passes rest on it),
the body's responses (they allocate), the per-key tables (the base; *Voicing amount* scales the
partial table).

## 2. The character presets (Part B)

Eight factory presets (`chorda.preset.v_*`, tag *Voiced character*; `views/inst/chorda.rs`).
Each still a real instrument: nothing over the model, only its voicing and Chorda's own controls.

| Preset | Look | What moves | Fitted against |
|---|---|---|---|
| Dark Felt | upright, black | soft felt (−80 %, the treble −40 %), the top rolled off at 4.5 kHz, overtones and treble shorter, the knock lower and louder, the key's return, mics over the hammers, lid shut | no free felt piano exists (CC0/CC BY): the shipped Chorda |
| Imposing Concert | concert, black | harder hammers, more mass, longer bass, a stronger, lower formant (520 Hz +14 dB), a wider pair, uneven unisons, a large room | the Salamander (Yamaha C5) and the bitKlavier Stage Lid ORTF (Steinway D) |
| Bright Pop | grand, white | hard treble felt, overtones longer, the top open (16 kHz), the low formant (150 Hz), compressed dynamics, close mics | the Salamander (close pair) |
| Intimate Close | parlour, black | the action heard (knock, key return), a wide close pair, a soft top (5 kHz), a small room | the bitKlavier Piano Bar and the Upright Piano KW |
| Upright Parlour | upright, walnut | soft felt (−70 %, the treble −80 %), short stiff strings (stiffness 130 %), short middle and treble, open unisons (6 ct, uneven), no bass weight (the formant −12 dB at 150 Hz), the case over the top (2 kHz) | the Upright Piano KW (Kawai upright, at the pianist's head) |
| Vintage Honky-Tonk | upright, rosewood | the widest unisons (8 ct, uneven, a strong second polarisation), hard hammers, compressed dynamics, a short treble, a thin bass, a loud action, bright | the Old Piano FB (Francis Bacon player piano) |
| Jazz Club Grand | grand, walnut | a slightly softer treble, the knock and the noise present, the lid half open, close mics, a small room | the bitKlavier Piano Bar |
| Cinematic Felt | concert, black | soft felt, the top round (6 kHz), long treble, much sympathy, a large room far away | no free recording of its kind: the shipped Chorda |

**The references** (CC0 / CC BY only, `tools/sound-refs/catalog.toml`): the Upright Piano KW and
the Old Piano FB (FreePats, CC0; new set `piano-character`, 67 MB, installed through Cantus's
pack code into `$CANTUS_REFS/piano/content`), the bitKlavier Piano Bar and Stage Lid ORTF
(CC BY 4.0) and the Salamander (CC BY 3.0) already there. 67 MB downloaded this pass. No free
felt piano was found: the felt pianos offered free (Spitfire/Orchestral Tools' Spindle, the
Westwood, the Kawai Felt, Pianobook's) are Kontakt or SINE libraries under their own terms.

**The features** (`tools/chorda-voicing/probes/presets/features.py`, all through the engine,
`chorda_compare render`): the long-term spectrum's tilt (dB/oct, 100 Hz – 8 kHz, the five
phrases), the attack's brightness (the share over 2 kHz of a note's first 100 ms, C3 … C6 at
mf) and its climb from pp to ff, the 20-dB fall time by register, the unison's beating (the
fundamental's envelope around its line), the bass weight (30 – 250 Hz against 250 Hz – 4 kHz).
`fit.py` widens the references' span by each feature's tolerance (tilt 0.6 dB/oct, brightness
2.5 dB, its climb 3 dB, decay 25 %, beat 1 dB, bass 2 dB — under the two grands' own
differences). The Old Piano FB was sampled at one velocity: its climb (0 dB) is no measure.

**36 of 47 checks inside.** What stays out:
- **The unison's beat** (5 presets): Chorda's unisons were tuned beatless on purpose (the unison
  pass, `DSP_VERSION` 11, after the owner's ear); the measure reads 1.4 – 2.2 dB where the
  recordings read 2.7 – 5.1 even with the unevenness and the detune raised. A recording's noise
  floor under a fast decay reads as beating too (the KW).
- **The Upright Parlour's attack brightness**: the KW's mezzo-forte is its soft layer of two
  (−41.6 dB over 2 kHz); Chorda's top roll-off at its lowest (2 kHz, 12 dB/oct) reaches −27.
  Its tilt, decays and velocity climb are inside; its bass weight is 1.2 dB over the span: the
  closest that keeps it inside its level window (`preset_render`: at its loudest gain, 12 dB, a
  board quieter than 0.9 left it 10 LU under).
- **Bright Pop and Jazz Club's attack** by 0.1 – 1.7 dB, and their climb with velocity: Bright
  Pop's compressed dynamics (a pop piano's) narrow it.
- **The Honky-Tonk's treble decay**: 0.34 s against 0.24 (+ 0.06 tolerance); the treble's own
  decay is at its least, what remains is the room's.

<details><summary>Every table (fit.py on the final renders)</summary>


## v_imposing (v_imposing) against salamander, bk_stage
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -7.01 | -6.20 / -6.59 | -7.19 … -5.60 | in |
| bright | -26.67 | -22.64 / -24.88 | -27.38 … -20.14 | in |
| bright_vel | 23.08 | 23.59 / 21.78 | 18.78 … 26.59 | in |
| decay_bass | 3.50 | 3.50 / 2.92 | 2.04 … 4.38 | in |
| decay_mid | 2.09 | 1.15 / 1.76 | 0.71 … 2.20 | in |
| decay_treble | 0.26 | 0.48 / 0.30 | 0.18 … 0.60 | in |
| beat | 1.55 | 3.86 / 2.68 | 1.68 … 4.86 | OUT |
| bass | 5.08 | 5.60 / -1.71 | -3.71 … 7.60 | in |

## v_bright (v_bright) against salamander
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -5.95 | -6.20 | -6.80 … -5.60 | in |
| bright | -25.28 | -22.64 | -25.14 … -20.14 | OUT |
| bright_vel | 17.98 | 23.59 | 20.59 … 26.59 | OUT |
| decay_bass | 3.50 | 3.50 | 2.62 … 4.38 | in |
| decay_mid | 1.25 | 1.15 | 0.86 … 1.44 | in |
| decay_treble | 0.38 | 0.48 | 0.36 … 0.60 | in |
| beat | 2.21 | 3.86 | 2.86 … 4.86 | OUT |
| bass | 6.54 | 5.60 | 3.60 … 7.60 | in |

## v_intimate (v_intimate) against bk_bar, kw
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -6.29 | -6.78 / -7.75 | -8.35 … -6.18 | in |
| bright | -25.16 | -23.50 / -41.58 | -44.08 … -21.00 | in |
| bright_vel | 18.32 | 25.92 / 13.29 | 10.29 … 28.92 | in |
| decay_bass | 3.50 | 2.92 / 3.50 | 2.04 … 4.38 | in |
| decay_mid | 1.30 | 1.64 / 0.76 | 0.35 … 2.05 | in |
| decay_treble | 0.26 | 0.36 / 0.26 | 0.17 … 0.45 | in |
| beat | 1.53 | 2.20 / 5.14 | 1.20 … 6.14 | in |
| bass | 1.99 | 4.14 / -2.92 | -4.92 … 6.14 | in |

## v_upright (v_upright) against kw
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -8.17 | -7.75 | -8.35 … -7.15 | in |
| bright | -29.06 | -41.58 | -44.08 … -39.08 | OUT |
| bright_vel | 15.06 | 13.29 | 10.29 … 16.29 | in |
| decay_bass | 3.34 | 3.50 | 2.62 … 4.38 | in |
| decay_mid | 0.92 | 0.76 | 0.57 … 0.95 | in |
| decay_treble | 0.28 | 0.26 | 0.20 … 0.33 | in |
| beat | 1.30 | 5.14 | 4.14 … 6.14 | OUT |
| bass | 0.24 | -2.92 | -4.92 … -0.92 | OUT |

## v_honky (v_honky) against fb
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -4.63 | -4.20 | -4.80 … -3.60 | in |
| bright | -20.36 | -21.85 | -24.35 … -19.35 | in |
| bright_vel | 8.93 | — | — | |
| decay_bass | 3.50 | 3.50 | 2.62 … 4.38 | in |
| decay_mid | 1.07 | 1.15 | 0.86 … 1.44 | in |
| decay_treble | 0.34 | 0.24 | 0.18 … 0.30 | OUT |
| beat | 1.49 | 4.75 | 3.75 … 5.75 | OUT |
| bass | -1.51 | -1.19 | -3.19 … 0.81 | in |

## v_jazz (v_jazz) against bk_bar
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -6.88 | -6.78 | -7.38 … -6.18 | in |
| bright | -27.69 | -23.50 | -26.00 … -21.00 | OUT |
| bright_vel | 21.12 | 25.92 | 22.92 … 28.92 | OUT |
| decay_bass | 3.50 | 2.92 | 2.19 … 3.65 | in |
| decay_mid | 1.30 | 1.64 | 1.23 … 2.05 | in |
| decay_treble | 0.28 | 0.36 | 0.27 … 0.45 | in |
| beat | 1.38 | 2.20 | 1.20 … 3.20 | in |
| bass | 2.65 | 4.14 | 2.14 … 6.14 | in |

## v_dark (v_dark) against no recorded piano of its kind
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -7.38 | — | — | |
| bright | -27.79 | — | — | |
| bright_vel | 15.04 | — | — | |
| decay_bass | 3.50 | — | — | |
| decay_mid | 1.27 | — | — | |
| decay_treble | 0.24 | — | — | |
| beat | 1.09 | — | — | |
| bass | 2.33 | — | — | |

## v_cinematic (v_cinematic) against no recorded piano of its kind
| feature | preset | references | span ± tolerance | |
|---|---|---|---|---|
| tilt | -6.79 | — | — | |
| bright | -24.58 | — | — | |
| bright_vel | 16.88 | — | — | |
| decay_bass | 3.50 | — | — | |
| decay_mid | 1.62 | — | — | |
| decay_treble | 0.52 | — | — | |
| beat | 3.76 | — | — | |
| bass | 3.69 | — | — | |

36 of 47 inside

</details>

## 3. For the owner's ears

- **The named preview** (not blind): `F:\projects\cantus-work\listen\chorda-presets\escuchar.html`,
  in Spanish: each preset's five phrases (soft chords, middle C, bass, fast run, ballad) beside
  the recorded piano of its kind (or the shipped Chorda), 16-bit WAVs at −20 LUFS in `wav16\`
  (`niveles.csv`: each file's gain), the Voicing view's screenshots at the end.
- Built by `probes/presets/render.sh` → `features.py`/`fit.py` → `page.py`
  (`plan.json` in `F:\projects\cantus-work\chorda-presets\`).

## 4. Tests

- `cantus-dsp --lib chorda::tests_user` (6): defaults leave the voicing to the bit; a unit at the
  defaults plays the shipped bits (moved and back too); each control moves the sound (the damped
  floor checked at the strike: with candidate 18's knock the voice outlives the floor); all moved:
  every block size and thread count, idle as new, rest; the filters moved on a ringing unit.
- `cantus-dsp --lib chorda::` (151): every pin held (no sample of the shipped voicing moved).
- `cantus-engine --test inst_chorda` (15): every parameter at its default changes no sample (the
  null test, now over the 22 new keys); every voicing control at either end, and all at each end,
  plays finite and under full scale (it caught the unison's loss scatter ringing up past 1:
  `UNI_LOSS_MAX`).
- `cantus-engine` lib: `the_voicing_controls_defaults_are_the_shipped_voicing`.
- `cantus-core`: `every_voicing_control_is_grouped_once_and_automatable`; the undo fuzz
  `chordas_voicing_by_hand_undoes_and_redoes_among_the_others`.
- `cantus-project`: `schema_88_keeps_chordas_voicing_by_hand`, fixture `v88.cantus`.
- `cantus-ui`: `chorda_presets_e2e` (through `CantusApp::assemble`: the picker's Chorda, Dark Felt
  applied as the browser applies it, darker; the Voicing view's *Top roll-off* dragged by the
  pointer, brighter; one undo returns the preset's bits, another the shipped ones), `kit_lint`,
  i18n, `inst_chorda`, `chorda_c17_e2e`, `chorda_model2_e2e`.

## Open

- Nothing verified by ear yet: the named preview is for the owner.
- The unison's beat stays under the recordings' in five presets (Chorda's unisons beatless by design).
- The Upright Parlour's attack is brighter than the KW's soft mezzo-forte layer (Chorda's top roll-off is 12 dB/oct), and its bass weight is 1.2 dB over the span (its level window).
- No free (CC0 / CC BY) felt piano exists to fit Dark Felt and Cinematic Felt against.
- Not mine, failing on this branch before it: `instruments_scale_e2e`'s header test cuts Tympanum's kit name "Studio Maple" in Spanish; `instruments_narrow_e2e` cuts Arcus's "INTÉRPRETE" in Spanish.
- The integrator: schemas 85–87 are held by empty migrations here (`migrate_84_to_85` … `86_to_87`); put the integration's in their place, keep 88, and write `fixtures/v88.cantus` again after the merge.
- Not run: the whole workspace suite, the WSL/Linux check.
