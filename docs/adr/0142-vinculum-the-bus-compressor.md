# ADR 0142 — Vinculum, the bus compressor

**Status:** accepted, phase B done, the opto matched (amended 2026-11-17) (fx side round wave B, provisional number — renumbered at merge) ·
**Date:** 2026-11-17 · **Schema:** 65 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0005 (the audio thread), ADR 0018 (null defaults), ADR 0022 (an effect's
sidechain), ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert)

## Context

Comprimo is a general compressor with styles; a mix bus wants the three characters
engineers reach for by name — the console's VCA bus compressor with its Auto release, the
photocell leveller, the variable-mu valve limiter — each with the controls its panel has.
The DSP was built first (`docs/round-fx/vinculum.md`): the VCA after Giannoulis, Massberg
and Reiss (JAES 2012) with a two-capacitor Auto release; the opto as a T4-style cell in
feedback; the vari-mu as a remote-cutoff valve whose loop is solved at zero delay through
its tabulated equilibrium, with an odd-harmonic colour at 2× on the residual only. Latency
0; CPU ≤ 0.37 % of a core.

## Decision

A new effect kind, `EffectKind::BusCompressor`, model `cantus_core::fx::vinculum`, engine
unit `cantus_engine::fx::vinculum`, DSP `cantus_dsp::vinculum`.

- **Parameters** (12 keys): `model`, `thr`, the VCA's panel as stepped positions (`ratio`
  2/4/10, `att` 0.1–30 ms, `rel` 0.1–1.2 s and Auto), the opto's and the vari-mu's
  Compress/Limit (`char`), the vari-mu's six time positions (`time`), the side chain's
  high-pass (`hpf`, 20 Hz is off), `makeup`, `colour`, `mix`, `out`. **Every default is a
  null**: the threshold at its top (+24 dBFS) over any signal, no make-up, no colour, `out`
  at 0 dB not applied — tested to the bit through the engine for each model.
- **The external key** is the slot's sidechain (another strip's post-fader output, the
  engine's ordinary key path); with none routed the input is its own key. In the feedback
  models the key takes the input's place in the loop. `Vinculum::process_keyed(…, None)` is
  `process` to the bit.
- **Starting preset** (ADR 0071): *Glue* — the VCA at 4 : 1, 30 ms, Auto, −18 dB, +2 dB.

## The opto matched (amended 2026-11-17, the owner's decision)

The open choice is closed. The owner's words: "the opto cell matched to published
LA-2A/T4 measurements: attack ~10 ms; release in two stages — ~60 ms for about the first
half, then 1–15 s for the rest — with memory (the longer and harder it has been compressed,
the slower the second stage; a cell that rested recovers fast)". Before, the Limit's
attack measured 0.9 ms to 63 % at 16 dB over the threshold: in the feedback loop the
eighth-power panel lit the cell far past where it settles, and a feedback loop speeds its
cell by its own gain.

**Sources** (each figure below is held by a test that names it,
`crates/cantus-dsp/src/vinculum_tests_opto.rs`):

- **UA-M** — Universal Audio, *Model LA-2A Leveling Amplifier* user's guide, rev. 1.3
  (2000), <https://www.adorama.com/col/productManuals/UALA2AD.pdf>: p. 1, "Release Time:
  approximately 0.06 seconds for 50% release, 0.5 to 5 seconds for complete release
  depending upon the amount of previous reduction"; p. 10, the cell "releases quickly
  (40-80 milliseconds) to approximately half … The remainder of its release can take place
  over as much as several seconds", and its "memory": "the release time is slower if the
  unit has either been in compression for a while, or the amount of compression is large".
- **UA-T** — Universal Audio, *Tips & Tricks — Teletronix LA-2A Classic Leveler Plug-In
  Collection*, <https://www.uaudio.com/blogs/ua/la-2a-collection-tips-tricks>: "the
  average Attack time is 10 milliseconds, and the Release time is about 60 milliseconds for
  50% of the release, and anywhere from 1 to 15 seconds for the rest".
- **PE** — PerkinElmer Optoelectronics, *Photoconductive Cells* (the CdS cell handbook),
  <https://wp.optics.arizona.edu/milster/wp-content/uploads/sites/48/2016/06/APP_PhotocellIntroduction.pdf>,
  pp. 6–7: resistance a power of illumination (a straight line in log–log, its γ); "All
  material types show faster speed at higher light levels"; the light history effect
  "depends upon the new light level, and upon the time spent at each of these light levels.
  this effect is reversible".
- **NMHR** — J. Najnudel, R. Müller, T. Hélie, D. Roze, *Power-balanced dynamic modeling of
  vactrols: application to a VTL5C3/2*, DAFx-23,
  <https://www.dafx.de/paper-archive/2023/DAFx23_paper_50.pdf>: "the attack time decreases
  with the received light"; in their optical compressor "the higher the input voltage, the
  shorter the attack and the longer the release".

No measured step response of a T4 cell itself was found published (a capture, or a paper's
plot of one); the figures are the manufacturer's and the CdS cell's. "Release" is measured on
the reduction the needle shows, in dB (UA-M p. 3: the meter "reads the amount of compression
in dB"; UA-M p. 10 speaks of the cell's "off resistance", which we read as the same 50 %);
"complete" as 90 % (the reduction down to a tenth).

**The model.** The static law is unchanged (`OptoLaw`: `1/(1 + G)`, `G = c·(u − u₀)^p`, the
input `y·(1 + G(y))`). What changed is time:

1. **The loop solved at its equilibrium**, as the vari-mu's is: the panel's light is the
   key's mean square over 5 ms; the reduction whose output lights the cell to exactly that
   reduction is tabulated once (`OptoEquilibrium`, 513 points from the panel's turn-on to
   64 dB over the threshold, then its last slope; within 5·10⁻⁴ dB of the law's solve,
   exactly 0 under the turn-on) and the cell moves toward it. So the published times hold on
   the reduction at any depth and either character.
2. **The photocell** (`OptoCell`), three states in the reduction's dB (the CdS resistance is
   a power of its light, PE, so its decibels follow the light): a **fast** stage — the free
   carriers — rising at 12 ms dim to 6 ms fully lit (12 dB of reduction; PE, NMHR: the
   cell quickens with light) and falling at 65 ms; a **slow** stage — the trapped carriers —
   charging toward the fast one over 0.1 s while lit and letting go over 0.6 s fresh to 12 s
   with a full memory; the cell holds the larger of the fast stage and 42 % of the slow one
   (under a half, so the first half of every release is the fast stage's); and the
   **memory**, 0 … 1, filling over 4 s toward the slow stage's depth (how long, and how
   hard: full at 12 dB) and emptying over 3 s as the slow stage empties (a rested cell is a
   new one, PE "reversible").

**Measured against the sources** (48 kHz, steady levels, threshold −20 dBFS):

| Figure | Published | Before | After |
|---|---|---|---|
| Attack to 63 %, Compress, 6 / 16 / 26 dB over | ~10 ms average (UA-T); faster with more light (PE, NMHR) | 23.0 / 8.6 / 3.6 ms | 15.0 / 10.2 / 7.8 ms (mean 10.6) |
| Attack to 63 %, Limit, 6 / 16 / 26 dB over | the same | 6.1 / 0.9 / 0.2 ms | 14.8 / 9.0 / 7.6 ms (mean 10.0) |
| Half released, 3 – 22 dB, after 0.1, 1 and 10 s | 0.06 s (UA-M p. 1, UA-T); 40 – 80 ms (UA-M p. 10) | 48 – 71 ms; Limit at 22 dB 84 / 393 / 445 ms | 50 – 60 ms at every depth and time |
| Complete, Compress 8.6 dB, after 0.1 / 1 / 10 / 30 s | 0.5 – 5 s (UA-M); 1 – 15 s (UA-T); longer the longer (UA-M p. 10) | 0.96 / 4.13 / 4.24 / 4.24 s | 0.81 / 1.65 / 4.68 / 5.01 s |
| Complete, Limit 13.1 dB, after 0.1 / 1 / 10 / 30 s | the same | 2.46 / 5.01 / 5.10 / 5.10 s | 0.87 / 2.32 / 8.87 / 9.50 s |
| Complete after 10 s gentle / hard, Compress (4.0 / 15.0 dB) | longer the harder (UA-M pp. 1, 10) | 3.43 / 5.47 s | 2.05 / 9.93 s |
| the same, Limit (3.0 / 22.0 dB) | the same | 3.27 / 6.96 s | 1.70 / 12.97 s |
| A burst after 30 s of rest, against a new cell's | reversible (PE) | 0.96 / 0.96 s (C), 2.46 / 2.46 s (L) | 0.81 / 0.81 s, 0.87 / 0.87 s |
| The same burst 1 s after a 10 s passage | the memory (UA-M p. 10) | 3.57 s (C), 4.66 s (L) | 4.26 s (C), 8.56 s (L) |

Before, the memory was nearly full after one second (4.1 s complete after 1 s and 4.2 after
30) and depth barely mattered; the Limit's attack was a tenth of the published one, and its
first half took 0.4 s at 22 dB.

**Kept.** The static curves (`the_opto_settles_on_its_cell_law`, the ratios 3 : 1 and about
10 : 1 unchanged), the controls and their meaning (threshold, Compress/Limit), no key
added or changed (no schema change), latency 0, the nulls (a threshold over the signal
touches no sample, mix 0 to the bit), block-size independence to the bit, exact rest, no
allocation (`vinculum_alloc`, `rt_memory`). The VCA's and the vari-mu's code paths are
untouched: their renders in `compartir\vinculum\sound` came out byte-identical. CPU: the opto 0.21 % of a core before, 0.27 – 0.28 % after (stereo, 48 kHz, release, best of three, `cpu_per_model`; the VCA measured 0.26 and 0.20 % in the same runs — the difference is the equilibrium's per-sample `log10`; the memory's coefficient comes from a 33-point table, as a per-sample exponential cost 0.1 % more).
No golden hash covers Vinculum (it is new on this branch and unreleased), none moved, so
`DSP_VERSION` stays.

**Presets** re-fitted: `preset_tool fit BusCompressor` (all fourteen) moved none: the three opto presets (Opto vocal bus, Smooth leveller, Opto vocal limit) still land in their level windows with the matched cell, and `preset_audit`, `vinculum_e2e` (each model compressing and limiting) and `vinculum_drag` are green.

**To hear**: `compartir\vinculum\sound` — the opto excerpts 04 – 06 and 24 – 26 beside
their `-before` twins, and the memory (30 dry, 31 through the opto: the same quiet phrase
after silence holds 3.0 → 2.7 dB, straight after twelve loud seconds 5.5 → 4.1 dB, level
matched on the first).

## Phase B (2026-11-17): the editor, presets, export (design sheet v1 approved)

- **The window as approved**, with the owner's note on v1: the rack rails' column of small
  square holes (and their cage nuts) read as the "vents" ornament he had removed from Lapsus
  and Vetus — the rails are plain brushed steel now, the unit's ears with their slots and
  screws saying it is racked (`vinculum_art::rack`). Shots:
  `compartir/vinculum/design/v2-step3/`.
- **The faceplate's switches** were real controls from the sheet (a click on an engraved
  position chooses it, a click on the knob steps it, a click on the bat throws it, each one
  step of undo; `stepped_offer` gives automation and MIDI learn); phase B holds them on real
  pointer events.
- **The lamps are controls** (`vinculum_face::lamp_edit`): the **External key** lamp opens
  the key's list under it (its own input, or any strip of the desk — the same choice as the
  Side chain card's field), and in a plug-in window it throws the plug-in's sidechain switch;
  the **Auto release** lamp lights Auto and a second click gives back the release it had (the
  editor remembers it; with nothing remembered, 0.3 s, the parameter's default); the **Limit**
  lamp throws the character. The valve colour's lamp only shows (the colour is a knob). Each
  change one step of undo. The lamps are laid out once per frame before they are drawn, so
  the pointer and the picture agree (`vinculum_face::lamps`).
- **Presets** fourteen (`presets/effects/buscompressor.toml`): VCA — *Glue* (the starting
  preset), *Mix bus glue*, *Punchy drum bus*, *Drum smash*, *Parallel drums*, *Bus pump*,
  *Bus catch* (10 : 1, the fastest attack: the VCA's limit); Opto — *Opto vocal bus*, *Smooth
  leveller*, *Opto vocal limit*; Vari-mu — *Valve mastering glue*, *Valve warmth*, *Valve
  limiter*, *Lo-fi squash*. Every model compressing and limiting (a test holds it). Roles
  bus, master, drums, vocal, creative. Fitted by `preset_tool fit` with the level carried by
  the make-up first: the output −2 … +2.8 dB; **the starting preset moved −1.5 dB** (`out`, in
  `starting.rs` and the editor's demo).
- **Export** `Cantus Vinculum` (CLAP and VST3; `desc.rs`, the `vinculum` feature, xtask; the
  VST3 class id by its rule): the engine's unit through the generic processor, so its
  detector hears **the sidechain input** when the plug-in's *Sidechain* switch is on the
  external key (CLAP's second audio port, VST3's aux bus) — bit-identical to the DSP keyed by
  the same sound in both, and the unkeyed quiet input untouched to the bit. The plug-ins'
  Direct path names Vinculum's DSP with its output glide. No latency, no tail. Ids appended
  and blessed (16). The plug-ins' text now carries wave A's and wave B's locale files, so
  every kind's choices have their words (`pane_words`).
- **AI**: the effect words say what Vinculum is for (the three models, Compress or Limit, the
  sidechain key through a high-pass, parallel; glue, drum smash, parallel drums, vocal bus,
  mastering glue) and two more aliases (`vari_mu`, `opto`); the MCP tools golden regenerated
  on purpose.
- **A pedal on Tabula** (`pedal_allowed`): a compressor pedal is natural. Its face: model,
  threshold, make-up, mix (standard, the needle motif, "V"). On a board it compresses the
  board's own signal; a board takes a key when one of its pedals does, and passes it on, so
  a keyed board's key reaches Vinculum's detector exactly as an insert's. With no key on the
  board it hears its own input — never nothing.

## Consequences

- One more dynamics unit with a face of its own (a rack bus compressor, three faceplates).
- The side chain's high-pass now lets go of states under 1e-15 when fed exact zeros: on
  silence a low corner's states cycled around 1e-19 and the unit never rested (found by the
  engine's rest test); no output sample changes while there is signal.
- A unit's lamps can be controls: the key is chosen where it is shown, beside the field on
  the Side chain card (both the same command, one step of undo).
- Vinculum joins Tabula: `a_board_of_one_is_the_unit` covers it with every other pedal.
- The racks palette loses its cage nut (`racks::CAGE`): no rack draws square holes.

## What the tests hold

`cantus-dsp`: the models against their laws (27 tests; the opto's times against the published measurements in `vinculum_tests_opto.rs`, five, and its table in `the_opto_table_is_its_equilibrium`), `the_external_key_drives_the_side_chain`,
`the_side_chain_high_pass_comes_to_rest`; `cantus-engine/tests/fx_vinculum.rs` (the null for
each model, the starting preset at every block size and live, every model block-size
independent, the external key and its high-pass, forks), `fx_contract` (split and at rest),
`rt_memory::vinculum_never_allocates`.

Phase B: `cantus-ui/tests/vinculum_e2e.rs` (through `CantusApp::assemble`): a band in a bus
strip, a bass line to the master; Vinculum inserted as the picker does (Glue, latency 0);
**the reduction heard** — the threshold 20 dB under the band's peak at 10:1, the bus 7.7 dB
quieter; live in odd blocks equal to the render to the bit; **the external key** — over the
band, on its own key the band untouched to the bit, the bass strip as the slot's key presses
it 6.0 dB while the bass plays and 0.00 dB before; **the high-pass** at 500 Hz takes the
bass's lows from the key (4.7 dB); an edit heard and undone to the bit; every key at its
default the dry master to the bit (the key still routed); every preset names real keys in
both languages, each model compressing and limiting, the starting preset named Glue.
`vinculum_drag.rs`: the stepped switches, the Auto and limit lamps and the key's lamp with
its list on real pointer events, each change one undo step. `fx_vinculum::as_a_pedal_it_compresses_its_own_input_and_hears_the_board_s_key` and `fx_contract::a_board_of_one_is_the_unit`.
The plug-ins: `clap::vinculum_s_external_key_presses_the_input_only_when_asked`,
`clap::vinculum_s_models_match_the_dsp`, `vst3::vinculum_s_key_is_the_vst3_sidechain_bus`,
the family's bit-identity, block-size, automation, state and ids tests with Vinculum among
them.
