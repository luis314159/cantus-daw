# Round 42 — the integrator's report (r42/integration)

Integrator: session cantus-57, 2026-11-22 to 2026-11-27, in `F:\projects\daw-r42\integration`.
The contract (`r42/contract` `3cd82f6f5`, with master merged at `c28fe171a`) declared the
project's tuning, MTS-ESP's seam, Arcus and the note's articulation (schemas 80 and 81, ADRs
0155–0158, `BUILDING.md`, `briefs.md`, `pitch-map.md`). The owner steered the round ask by ask
(each recorded in its builder's report), and on 2026-11-22 left every pending choice to the
integrator by the excellence bar; on 2026-11-26 he asked for everything to be published once the
round is finished (*"En cuanto termines publica todo"*). beta.17 (round 41) was published from
`rc/0.1.0-beta.17` on 2026-11-24 in the middle of the round; master then took `site/lcp`
(`290b82256`), and this branch took master's site work through `r42/site-plugins`.

## What is in, in merge order

**Wave 1** (2026-11-22 to 2026-11-26):

| Merge | Branch (tip) | What it brings |
|---|---|---|
| `4d181be86` | r42/tuning (`2b322dbf4`) | the project's tuning in every pitched unit (ADR 0155, schema 80) |
| `07f801824` | r42/tuning-face (`aa5aec1a4`) | *Options ▸ Tuning…*, Scala in and out, MTS-ESP client and master (ADR 0156); `433871a3e` put it on tuning's API |
| `57a424984`, `add392f22` | r42/fix-engine, r42/fix-ui | the first mid-round review's findings (tuning, engine, interface) |
| `cc7e34fc1` | r42/strings-rec (`871b089bc`) | recorded strings and the note's articulation (ADR 0158) |
| `4a123736f` | social/kit (`6d6e8d0e6`) | the social kit (`docs/social/`) |
| `42746b3dd`, `7135dc471`, `3bc7de6f9` | r42/arcus, r42/arcus-look, r42/arcus-upper | Arcus passes 1–4 (ADR 0157, schema 81), its picture (sheet v4), the violin and viola |
| `83f5b2cd7`, `53a46aa44` | r42/fix2-model, r42/fix2-arcus | the second mid-round review (three finders): fourteen model/file/UI/AI findings, nine on Arcus's and Exemplar's DSP |
| `a2fa33b4c`, `21e7bce78` | r42/fix3-suite | the full suite's five UI failures; Exemplar's Legato card |
| `aa14b2561` | r42/tymp-abx (`871a82b36`) | Tympanum's blind ABX kit and the shared `abx_kit` page |
| `6cb06f49f` | r42/fix4-narrow (`83325e8df`) | every instrument whole at every scale and the narrowest window |
| `11e6b8cad`, `eee97dd86` | r42/arcus-demo, r42/arcus-p5 | the *Andante cantabile* demo; pass 5 (`DSP_VERSION` 13) |
| `e3396db94`, `f42f88109` | r42/arcus-upper, r42/arcus-hair | the holders and the realistic figures (ADR 0159); the hair |
| `a223a36fc` | r42/soundlab (`902d68145`) | one sound lab for every instrument (ADR 0160) |
| `a61af28dc` | r42/arcus-hand (`94edbb75c`) | the left hand rebuilt from photographs, the eyes' setting, the clothes' first fixes |
| `538502b0c` | r42/editor-windows (`4b706a91f`) | editor windows that restore and pin (ADR 0161) |
| `94cab9580` | r42/arcus-p6 (`fbe9acba8`) | the cello's body to 10 kHz (`DSP_VERSION` 14) |
| `2ebb24c1c` | r42/sound-data (`60878c0d2`) | the sound references catalogued, `CANTUS_REFS` |
| `79f17cbbb` | r42/arcus-expr (`87d664753`) | Arcus played like a human (ADR 0162, `DSP_VERSION` 15, schema 83) |
| `9dd8fb0b3` | r42/chorda-c17 (`5754468d7`) | Chorda's candidate 17 (`DSP_VERSION` 16, schema 84) |
| `e63ceddd2` | r42/fiddle (`26210c532`) | the fiddle (ADR 0164, `DSP_VERSION` 17, schema 85) |

**The close** (2026-11-26 to 2026-11-27), the merges this report was asked to account for:

| Merge | Branch (tip) | What it brings |
|---|---|---|
| `5eb24cbc6` | r42/chorda-c17, candidate 18 (`7d85795ef`) | candidate 18 the default (`DSP_VERSION` 18, ADR 0098's seventh amendment); `Voicing::c17` pinned at its old hash; model 2's new pin `0xb19b_545c_67c3_0ee9`; `chorda_abx --catch` |
| `2918927af` | r42/fiddle, the scratch and Bite (`2410fa37b`) | the Fiddle character's `bow.noise` 0, `bow.grit` 0.75, `body.bridge` 1.3; `bow.bite` stored under schema 85 |
| `00bc02e72` | r42/fix-update-dim (`de4846b6f`) | the notice's veil, the blank publisher, the companion over the dialog; RELEASING L9 |
| `daa89365e` | r42/violet-paper (`0029e551d`) | Violet Paper (`PRESETS` 24 → 25, ADR 0111 §13) |
| `a80db8a16` | r42/arcus-classical (`57a5a5da2`) | the classical passes 1–2 (ADR 0166, `DSP_VERSION` 20), `player.fiddle` (schema 87), the playback's look-ahead |
| `698d2aa68` | r42/tympanum-real (`f999755b5`) | the level law and model 6 (`DSP_VERSION` 19, schema 86's real migration, ADR 0124's sixth and seventh amendments) |
| `6c4265168` | r42/chorda-c17, chorda-presets (`c6a40b8f5`) | the advanced voicing (22 `voicing.*` controls), the eight character presets, schema 88 (ADR 0098's eighth amendment) |
| `180024fd4` | r42/arcus-polish (`f0c65f12b`) | the musicians' polish passes 3–6 (ADR 0159) |
| `483830feb` | r42/arcus-hands (`49d69db42`) | the hands' passes 3–8 (ADR 0159) |
| `b8e9bc5a9` | r42/site-plugins (`be94edee1`) | the per-effect plug-in zips, the site's Plug-ins section (ADR 0167), RELEASING L10, with master's `site/lcp` |
| `9e15196d4` | r42/paganini (`72e7238bd`) | Caprice 24 against a real recording: the references' catalogue and `SOURCE.md` (the tools were already in, copied by arcus-classical in `c51f2ae65`; the merge kept those) |
| `7d3952e4a` | r42/drums-research (`499435d9c`) | the drums physics survey (`research/instruments-next/drums-physics-survey.md`) |

Beside the merges: `71d22f037` (`url` 2.5.9, RUSTSEC-2026-0592), `987d5dea2` (the
`drskit-tympanum` pack's unpacked size), `4721c1ec0` (CI runs when a sound-lab subject changes),
`a406f3a28` (`tools/wsl/guarded.sh` watches C: and F:), and the owner's BACKLOG entries and plan
(`docs/PLAN-after-round-42.md`).

## The schema and `DSP_VERSION` chains

Numbers were handed out by the coordinator ahead of the merges, so the branches landed out of
order; each step was confirmed at its merge.

| Schema | What | Branch | Note |
|---|---|---|---|
| 80 | the project's and the tracks' tuning | tuning | contract |
| 81 | Arcus and the note's articulation | arcus, strings-rec | contract |
| 82 | Exemplar's articulations, bows, crossfades, legato | strings-rec | |
| 83 | a bowed note's bow start, part and string | arcus-expr | |
| 84 | Chorda's `colour` | chorda-c17 | |
| 85 | the fiddle; `bow.bite` | fiddle | confirmed in `7281bf1d1` |
| 86 | Tympanum's model 2 (model 6's onset) | tympanum-real | arcus-classical, merged first, carried a placeholder step here; tympanum-real brought the real migration |
| 87 | `player.fiddle` | arcus-classical | a schema 85–86 Arcus with steel strings migrates to its steel |
| 88 | Chorda's voicing by hand | chorda-presets | 85–87 had been held empty on its branch |
| 89 | a new Tympanum plays model 1 again; model 2 by choice, marked experimental | the integration | a v86–v88 Tympanum that names no `model` played model 2, the default then: written in; `v89` holds the plain kit at 1 and the hybrid at 2 |

`804245a84` wrote the fixtures again with the merged model: `v88.cantus` holds Arcus's Bite and
`player.fiddle`, Tympanum's model 2 and Chorda's voicing; `v86` and `v87` (never shipped) were
written at their own schemas so each holds only what came before it (`v86` had no `bow.bite`,
`v87` no Tympanum model 2). Every older fixture loads (cantus-project 84 + 106 tests).

`DSP_VERSION` went 11 → 20: 12 Fides's bowed pitch, 13 and 14 Arcus passes 5 and 6, 15 Arcus's
player, 16 Chorda 17, 17 the fiddle, 18 Chorda 18, 19 Tympanum's level law and model 6, 20
Arcus's classical player. 19 merged after 20; its row in `cantus-dsp/src/lib.rs` carries the
integration's two Tympanum fixes (`e1ffca905`; unshipped, no golden hash moved — cantus-engine
1 522 and cantus-dsp lib 1 445 green). ADRs: 0155–0162, 0164, 0166, 0167; **0163 and 0165 are
unused** (ADR 0166's number confirmed in `804245a84`).

## The reds fixed on the merged tree

Each was found by the whole Windows suite on the merged tree, not by any branch alone.

- **Linux lints the branches never ran** (`8c45ce98c`, `b8934966b`): chorda-c17's knock test
  squared with `f64::powi` (clippy's `disallowed-methods`), and three `.clone()` calls on the
  `Copy` type `Voicing` (`clone_on_copy` under `-D warnings`). Same values.
- **CI paths** (`4721c1ec0`): `abx_kit/kit.rs` compiles in the sound lab's subject files, which
  `ci.yml`'s `!tools/**` ignored; xtask's `every_file_a_test_or_the_build_reads_runs_ci_when_it_changes`
  caught it.
- **The merged fixtures and words** (`804245a84`): `v86`–`v88` written again (above); Chorda's
  factory presets named their four macros in English in the Spanish window (now locale keys).
- **The known reds** (`1a4e2745b`):
  - `arcus_level_e2e` (a 3.12 LU spread): with `DSP_VERSION` 20's classical player the family
    plays at −20.7 … −21.5 LUFS while the Fiddle and the tremolos stayed at −18.3 … −19.9. The
    target is the cello cantabile, −21; four presets' gains came down (the Fiddle 5.4 → 3.0).
    Spread 0.80 LU. The gain is the output's trim, so the Fiddle's scratch and Bite measures
    are unchanged.
  - `arcus_fiddle_e2e`'s Bite: at bite 1/3 the noise between the harmonics rises about 0.4 dB
    on average, but one seed scatters ±0.3 dB about it. The step is now heard over four seeds and
    averaged; the rule (strictly rising) is the same: −27.9, −27.4, −26.7, −26.0 dB.
  - Two Spanish headers cut at the narrowest window: Tympanum's *Mezcla* (was *Mezclador*),
    Arcus's *Intérp.*, and the cross-tuning chips by their strings' letters (GDAE, AEAE…).
- **Tympanum's two reds, fixed in the model** (`373a83f7d`, then `e1ffca905`; `tympanum-real.md`
  §10): a model 6 kick or tom struck while it rang came out up to 2.56 dB under the same hit at
  rest — its few narrow low bands are the noise's draw as much as the envelope's, and a fresh
  hit always began on the draw `LAW6` was measured on. Every model 6 kick and tom stroke now
  starts its bands there (`FRESH_NOISE`): kick +0.15, snare +0.14, tom +0.41 dB. And the jazz
  ride at 127 fell 3.5 dB under its 112 on one seed: the table's stick burst is not monotonic
  across takes (0.50 at 110, 0.18 at 127); for a `model` 2 instance a layer's burst is now at
  least every softer layer's (+8.0, +3.3, +6.4 dB over three seeds). Model 5 is untouched.
- **The suite's other reds** (`04d4c96e6`): `fx::frame::window_ctl` had no pin (ADR 0161) and
  `fx_frame` clicked beside it; Chorda's stated preset count (33) in the README and the site; a
  test that set the process's language instead of its thread's; debug-build texture deltas
  `epaint` asserts on; two races in `tympanum_kit_change_e2e`; three clamps whose bounds could
  invert on a squeezed layout (the soak's panic, below); a cello's or a bass's neck hiding the
  arm by a box-wide ellipse 1.25 cm deeper than its drawn back (`hidden_by_neck` now uses
  `bowed_head::neck_profile`); rustfmt on the merges' leftovers (and `c442b3db8`).
- **The finger frame and its gates** (`9167ade90`, `fb887ec96`): below and in
  `docs/round42/arcus-upper-next.md`, *The r42 integration*.

## The vacuous-pass audit

The question asked of every gate the hands passed: does it look at what it names?

- **The High gates measured missing or white hands.** Two faults made `arcus_figure_specks` and
  `arcus_hand_form` read pictures without the hands they meant:
  - **The app** (`cb8802666`): the arm raster's passes (the left arm behind the neck, in front,
    the bow arm) fill one raster in turn, and each drew its kept texture in that raster's
    rectangle — the last-filled pass's. At rest nothing is filled again after the first frame,
    so the left hand's picture went into the bow arm's rectangle and **the left hand vanished**
    in the app itself (Photographic, High, any still hand). Each pass keeps its own rectangle
    now (`Scratch::ras_at`). `arcus_hold::a_still_hand_at_high_is_drawn_in_every_frame`
    (violin and cello, woman and man, vibrato 0, two shots on one context) fails without it:
    "Violin Woman arm 0, run 0: 1275 of 8533 px skin-coloured".
  - **The harness** (`1ad56e047`): egui sends a texture once and then only what changes; a
    `raster::shoot` run that began with no textures drew every one an earlier run on the same
    context had sent as white — the font atlas (text as white bars), a picture baked once, a
    still arm's High picture (a white block where the bow hand was). Each recent context's
    textures are kept across runs, as a screen's renderer keeps them.
  - So **the first ratchets were taken on missing hands**: the strays at 145/146 and the dark
    specks at none. Measured again on the hands as drawn (`fb887ec96`): the strays are 150 in
    each of four runs taking turns (ratchet 150), and one dark pixel at 200 % in his left hand
    whichever pass's B4 row (ratchet 1).
- **`arcus_hold`'s bake was in the wrong room.** A realistic figure under concert light stands
  on the Stage, but the test baked its reference on the Studio's backdrop, and its finger check
  had passed on that wrong backdrop alone. It bakes in the room the view draws now, the sounding
  string's clear width is its drawn swing and glow, and it compares with the bake's nearest pixel
  within one (two renderers a pixel apart on thin lines).
- **What was measured on the merged figure, not the branch's**: `arcus_hand_shape`'s lobe counted
  the merged cuff's 46 hand-bone skin vertices as hand (0.188 → 0.200); it is read on the hand's
  skin now, to `LOBE_REACH` 0.9 of the way to the wrist, the last tenth held by `BULGE` and the
  sleeve test. The sleeve test counts a pixel only with its four neighbours skin-coloured (its
  1 px and 3 px at 0.4 were the cloth's lit edge blended with the varnish).

## The finger frame (decided at the integration)

The owner's E5 on the A string in first position showed the free middle finger standing high in a
loop over the neck. `tools/arcus-figures/left_hand.py`'s `FRAME` (on by default;
`CANTUS_FINGER_FRAME=0` solves as pass 7): the fingers under the stopping one rest on their
string at their own notes (1 mm over it), those over it hover at most 0.5 cm over their string,
tip down. Every violin and viola row of both musicians was re-solved (`blend_fix.py --rules`,
five passes) and chosen row by row against `arcus_hand_table`: the frame's pose where the gate's
counts do not rise, else the pre-frame pose. **493 frame, 39 pre-frame, 28 the same**
(`left_hand_rows.json`). The owner's rows, her violin's **2-3-1 and 2-3-3**, are on the frame.
Two went back once the High shots showed the hands: her open A string (2-4-1; the left elbow 9.7
cm across, over `arcus_figure_shape`'s 9.4) and his B4 (2-1-1; one dark pixel at 200 %). The hand
gate went 36 rules / 27 cases → **31 / 20**, ratcheted there.

## Decided (the integrator, with the owner's mandate of 2026-11-22)

- The ratchets (table in STATUS) hold the musicians' gates where the round leaves them; each is a
  round-43 target, none is a pass.
- The Fiddle and the tremolo presets brought to the family's level (−21 LUFS) rather than the
  family raised: several presets' gains already sit at the +12 dB top.
- A new Tympanum keeps model 6 as its default (it measures nearer the recordings); old songs keep
  model 5. To be weighed again before beta.18's tag (STATUS, *The next session*).

## Verified only in emulation

Every picture and gesture (the CPU renderer); MTS-ESP against a real master; the plug-in zips
(never built from real plug-ins, `lipo` and `codesign` never run, no host tried); the macOS
preview, never opened on a real Mac; every build unsigned; the stage companion's z-order on a
real desktop. Every sound change is measured; the owner heard previews, not the merged build.
The owner's steps: `docs/beta-checklist.md`, *Ronda 42*.

## Open

- **The soak's clamp panic, never reproduced.** `soak_smoke` once panicked in an `f32::clamp`
  whose bounds had inverted; twelve loaded runs did not reproduce it. The three clamps whose
  bounds can invert on a squeezed layout are guarded (the meter's hold mark, a rounded stroke on
  an inverted rect, the limiter's GR label), but the one that fired is not proven to be among them.
- **`cargo fmt --all` fails on Windows** with os error 206 (it hands rustfmt every file of
  cantus-ui on one command line, past Windows's limit). This round ran rustfmt file by file on
  what it changed; `cargo fmt --all -- --check` is to run on Linux (WSL) before the push.
- **The WSL Linux check** on this tree is pending.
- **The 39 pre-frame rows**: round 43 starts from them (the knuckle rule high up and the stopping
  pads at 1–2.4 mm were most of it), and the realistic figures' table is to be solved under the
  Stage camera (6°) the view shows them in, not the Studio's (8°).
- The tests arcus-classical left red on its branch (`format::schema_84_keeps_chordas_colour`,
  `arcus_level_e2e`, the Bite step) are green on the merged tree (above).
