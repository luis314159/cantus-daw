# ADR 0142 — Vinculum, the bus compressor

**Status:** accepted, phase B done (fx side round wave B, provisional number — renumbered at merge) ·
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

## The owner's open choice

- **The opto Limit's attack** measures 0.9 ms to 63 % at 16 dB over the threshold against
  the published ~10 ms average (the panel's eighth power lights the cell far past where it
  settles). The DSP is kept as measured; closing it (a bounded conductance, or a
  drive-dependent rate) changes how the Limit sounds on hits, so it waits for the owner's
  ear (`compartir\vinculum\sound`).

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

`cantus-dsp`: the models against their laws (27 tests), `the_external_key_drives_the_side_chain`,
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
