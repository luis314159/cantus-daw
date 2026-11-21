# Round 41 — `nitor`: Nitor, the one-knob enhancer (ADR 0151)

Branch `r41/nitor` from `r41/contract`. **Stopped for the owner twice over**: the face's design
sheet v1 (his OK before the full editor), and the presets' sound (his ears). Everything else
below is built and measured.

## What it is

FL Studio's Soundgoodizer, Low Lifter and Maximus/Emphasis exciters in one unit, level-matched:

- **One knob** (`amount`) over five curated **characters** — Clear, Punch, Warm, Air, Loud —
  each a recipe for Multiplex's dynamic-phase dynamics (four bands, downward *and* upward
  compression, a static tilt) plus what it adds to the exciter and the low end.
- **Level independence** (better than the reference: Soundgoodizer acts on how hot you drive
  it): a K-weighted loudness of the input brings any programme to −18 LUFS before the
  dynamics and back after. Measured: band peaks and gains of the test programme identical at
  0 dB and −20 dB of input (`band_levels_of_programme_at_the_reference`).
- **The low end**: a sub that follows the bass an octave down (Saltus's divider) or sits on the
  **song's tonic** (the project's key, ADR 0153), the bass's harmonics for small speakers,
  *tight* (each hit opens the low band 30 ms, then a 120 ms fall toward −12 dB), a *lift* on
  the tonic (+6 dB) and its octave (+3 dB).
- **The exciter by band** (body 400 Hz, presence 2.5 kHz, air over 7 kHz): only harmonics,
  even … odd by *colour*, 4× oversampled; the same share at any level (measured within 0.5 dB
  from −6 to −46 dBFS).
- **The level match** (on by default, every preset keeps it): what you hear is the colour, not
  the loudness.

Design in full: ADR 0151's *builder's decisions*; the page `docs/plugins/nitor.md`; the
manual's section (EN/ES) after Comparatio.

## Done

| Layer | Files (owned) |
|---|---|
| DSP | `cantus-dsp/src/nitor.rs` (settings, characters, recipes, meter), `nitor_run.rs` (glides, `set`, `process`, rest, `snap`), `nitor_chunk.rs` (a grid cell's audio; the dynamics parked and woken), `nitor_level.rs` (pre-gain and level match), `nitor_excite.rs`, `nitor_low.rs`; tests `nitor_tests.rs`, `nitor_tests_measure.rs`, `nitor_testkit.rs`, `tests/nitor_alloc.rs` |
| Core | `cantus-core/src/fx/nitor.rs`: 16 parameters, `tonic` |
| Engine | `cantus-engine/src/fx/nitor.rs`: the unit, the song's key (re-reads only the tonic, no allocation), 21 telemetry slots |
| UI | `views/fx/nitor.rs` (the editor: corona + cards), `nitor_corona.rs` (the corona); `kit/skin/side_r41.rs` (the face); `presets/effects/enhancer.toml` (13); locales `fx/nitor.{en,es}.txt`; examples `shoot_nitor.rs`, `nitor_listen.rs`; `tests/nitor_e2e.rs` |
| Docs | `docs/plugins/nitor.md`, the manual EN/ES, ADR 0151, this file, `docs/beta-checklist.md` (Ronda 41 — Nitor), `docs/DEVELOPING.md` (the two tools) |

### Shared files touched (each line marked `r41:nitor`)

`cantus-dsp/src/lib.rs` (one line), `cantus-core/src/starting.rs` (the starting preset),
`cantus-engine/tests/support/heavy.rs` and `cantus-bench/src/kernels.rs` (heavy settings),
`cantus-plugins/tests/common/mod.rs` (test settings, the `Direct::Nitor` arm),
`cantus-plugins/tests/golden/ids.txt` (the new parameters, blessed),
`cantus-ui/src/views/picker_snap.rs` (`effect_name`), `cantus-ui/tests/preset_audit.rs` (the
floor-0 line deleted), `docs/manual/{en,es}/README.md`, `docs/beta-checklist.md`,
`docs/DEVELOPING.md`. Not touched: `web/src/units_data.rs` — **the integrator's card**.

## Measured

| What | Measured |
|---|---|
| Every default, and every character at amount 0 (match on/off, mix 0.5, tonic sub mode, odd crossover), blocks 1/37/64/512/mixed, −0.0 and denormals in the input | **the input to the bit**; at rest |
| Heavy settings (every part on, Loud at 100 %), changes at samples 40 000 / 70 001 / 90 000, blocks 1, 37, 64, 512, mixed | **identical to the bit** across block sizes |
| A render resumed from a fork mid-song | the whole render's second half to the bit |
| Heavy settings, 1.5 s of song then silence | at rest within 20 s (the match forgets at 8 s, the dynamics park at 0.5 s); skipping = processing silence = a new unit, to the bit |
| Turning everything back to 0 mid-song | the input to the bit from 0.2 s after (the glides and the match's way back) |
| Tonic sub, a 98 Hz tone, key G then D | the sub at 49.0 Hz, then 36.7 Hz; > 30 dB over the other note |
| Octave sub on 110 Hz | the sub at 55 Hz, > 20 dB over neighbours |
| Exciter, presence at 2 kHz | colour 0: the 2nd harmonic > 20 dB over the 3rd; colour 1: the reverse; the fundamental within 0.5 dB |
| Exciter's share at −6 and −46 dBFS | within 0.5 dB of each other |
| Lift at 1, tonic G | +6 dB at 49 Hz (±1), +3 dB at 98 Hz (±1, the fundamental's bell adds 0.8), 0 dB at 300 Hz |
| Tight at 1 on a 50 Hz note struck every 0.5 s | the hit within 1.5 dB, the tail 9.3 dB down |
| The level match, each character at 100 % on 8 s of programme | within 0.5 LU of the dry (the match: +0.2 … +0.6 dB — the characters' downward compression of the lows reads a little quieter K-weighted) |
| Level independence | the four bands' peaks and the character's gains identical at 0 and −20 dB of input |
| The e2e (`nitor_e2e`, through `CantusApp::assemble`) | inserted: *Gentle shine*, 383 986 of 768 000 samples moved, −14.09 → −14.09 LUFS; tonic sub in G: G1 −17.9 dB / D1 −75.1 dB; project in D: G1 −74.1 / D1 −17.9; live with the key changed mid-play: −17.8/−57.3 then −59.0/−17.9 dB; live in odd blocks = offline to the bit; amount 0 and every part off = the dry song to the bit |
| The 13 presets on their audition sources (`nitor_listen`) | loudness change −0.46 … +0.01 LU (the window is ±2) |
| CPU, each preset's whole render (the source's instruments included) | 0.040 … 0.094 of a core; the dry sources alone 0.012 … 0.031 — Nitor's share 0.03 … 0.065 (budget 0.10). **Measured on a machine out of memory with three other builds** (0.2 GB free): to be measured quiet by the integrator |
| Plug-in (CLAP and VST3) | bit-identical to the DSP and to the engine, at its starting preset and at test settings, automated at any block size; ids blessed (14 new parameters) |

## Tests

- DSP: `nitor_tests` 15 + `nitor_tests_measure` 2 + alloc 1 (`tests/nitor_alloc.rs`: every
  character and part, settings changed between blocks, parked and woken, `skip`, `snap`,
  `reset` — no allocation).
- Core: `fx::nitor` 2 (the defaults are nulls, every stepped key names its choices); 640 in the
  crate pass.
- Engine: `fx_contract` (the null, block sizes, rest twins at defaults and heavy, the board of
  one) — 102 pass; `rt_memory` 62; `song_key` 1.
- Plug-ins: 118 (clap 26, vst3 21 …), ids blessed.
- UI: `nitor_e2e` 1, `preset_audit` 2, `fx_lint` 1, `learn_everywhere` 11 of 12 (the failure
  is `FragorOverdrive: its editor drew no parameter` — the pedals' stub, not Nitor's;
  Nitor's `char` now offers its automation and learn on every facet), the skin tests 37.

## Verified only by measurement (no ear, no eye but mine yet)

- No one has **heard** Nitor: the characters' recipes were set from measured band peaks of a
  synthetic programme, not by ear. The owner's listening is the judgement
  (`F:\projects\daw-r41\nitor-listen\`).
- The face is a **design sheet v1** for the owner's OK (`docs/round41/nitor/`).
- The plug-in in our own CLAP/VST3 harness only, not in a third-party DAW.

## Left

- The owner: the sheet (approve or redirect), the presets' sound.
- After his OK: the face's finish (pointer tests for the facets and the sun knob, the 75–200 %
  probe at every state, colourways if he wants them), clippy on the UI crate and the whole
  suites of the touched crates, `cargo fmt`.
- The integrator: the site's card (`web/src/units_data.rs`), the quiet CPU measurement, the
  Linux check.

## Review fixes (integration)

Branch `r41/fix-nitor` from `r41/integration` 4f1b74b9e: round 41's integrated review, five
findings, each confirmed by a read and, where it is behaviour, by a test that failed first.

1. **A freeze ignored the song's key that Nitor follows.** `freeze::subject_with` wrote
   `project.key` only for note effects; a Nitor on `key` 0 (the default), alone or as a
   Tabula pedal, follows it since ADR 0153 — a key change reused a render tuned to the old
   key. Now **`EffectSpec::reads_song_key`** (in `effect.rs`, beside `takes_notes`; Nitor's
   own `fx::nitor::follows_song`, a board when a pedal does) is the one place that says it,
   and the subject holds the key when any effect of the region's strips (and of the master,
   when the whole mix is in it) reads it. The guard (`Touch::Global`) then refuses a key
   change while such a strip is frozen. Tests: core `freeze::an_effect_following_the_song_s_key_puts_the_key_in_the_freeze`
   (red first: the content key did not move) — Nitor, a board's Nitor, and Nitor on its own
   key (which leaves the key out); the app (`cantus-ui/tests/freeze.rs`
   `a_frozen_strip_with_nitor_following_the_song_refuses_a_key_change`): frozen in C, the key
   change refused and said, the mix the live one to the bit, thawed the new key heard; and the
   engine's guard `song_key::an_effect_whose_sound_moves_with_the_key_says_it_reads_it` —
   every kind at heavy settings rendered in C and in G: a sound that moves with the key must
   say `reads_song_key`, so the next key-following unit cannot forget it.
2. **Stems through the master and the mastering assistant's trials tuned Nitor to C.**
   `Through::of_chain` now takes the key and hands it to each unit after its spec, as the
   master does; `Through::new` passes the snapshot's, `listen::run_chain` takes it
   (`run_chain(chain, on, sr, key, audio)`). The assistant's own chain (EQ, compressor,
   stereo, limiter) follows no key, so `mastering::trial` hands none and says so (a
   `debug_assert`). Tests (red first, max difference 0.133 with the line taken out):
   `song_key::a_stem_through_the_master_hears_the_song_s_key` (the lead alone through Nitor
   following a song in G is the master's own output, within 1e-6; in C it differs) and
   `song_key::a_trial_of_a_master_chain_is_the_real_render_in_the_song_s_key`.
3. **A Nitor colouring nothing never rested after a Mix change.** `idle_chunk` moved only
   the output glide; `set` starts the mix glide whatever the state, so `rest_blocker` said
   "glides" for ever. The idle chunk now moves the mix sample by sample (exactly as a busy
   cell would; it shapes nothing there). Test: `nitor_tests::a_mix_change_with_nothing_to_colour_lands_within_the_glide`
   — at amount 0 a Mix change is still a wire to the bit and at rest within the 20 ms glide,
   at every block pattern; colouring later is to the bit a unit that had that Mix from the
   start. Every other Nitor test (18 with it) and `nitor_alloc` unchanged.
4. **Stale contract words.** The agent's description (`agent_units.rs`) and the MCP kind
   list (`tools_mixer.rs`) still said Nitor passes the sound through: both now describe it and
   every parameter key with its range; the golden `mcp-tools-list.json` re-blessed
   (`CANTUS_TOOLS_DUMP=1`, `ai_mcp_boot`), its diff Nitor's words only.
5. **Tabula's count.** r41/pedals added *Stacked drives*: 16 boards, four of them holding the
   amp (crunch amp rig, blues crunch, fuzz lead, stacked drives). README's Tabula lines and
   the site's card (count and feature line, both languages) changed, nothing else there;
   `stated_preset_counts` passes.

Also: `nitor_e2e.rs`'s unused `use std::sync::Arc` (a warning under CI's `-D warnings`) gone.

Run: core 933 pass (all of it, the undo fuzz included); DSP `nitor` 18 + `nitor_alloc` 1;
engine `song_key` 4, `master_chain` 16, `fx_contract` 102; UI `freeze` 94, `ai_mcp_boot` 1,
`stated_preset_counts` 1, `nitor_e2e` 1. Clippy `-D warnings` on cantus-core, -dsp, -engine,
-ai (all targets) and on cantus-ui's library and the touched test targets.
