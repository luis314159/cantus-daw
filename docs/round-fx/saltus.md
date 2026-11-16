# Saltus, pitch — the builder's report (fx side round, wave 3)

Branch `fx/saltus` (from `fx/integration` at the wave-3 contract; `fx/integration` with
Tabula merged in at `04b7c389`). ADR 0135 (provisional), and ADR 0138 (provisional, the
user's own starting presets — the owner's request in his v2 verdict). Plug-in page
`docs/plugins/saltus.md`.

## Done

- **DSP** (`cantus-dsp/src/saltus*.rs`, Phase A): Shift, Harmony, Octave, Whammy; the mono
  engine (PSOLA, formants kept), the poly engine (phase vocoder, 4 096 points), the divider;
  the null to the bit, fixed latencies, block-size independence, exact rest. Phase B added
  the whammy played by MIDI notes (`wsrc`).
- **Model**: 29 keys, every default a null, `out` kept; `TAKES_NOTES`; the starting preset
  *Octave up, twelve-string* (poly).
- **Engine**: the unit (settings, notes, telemetry, latency per setting); heavy settings.
- **Window** (design v1 → v2 → v3 after the owner's verdicts): the leap (keyboard, the note
  heard, an arc to each voice, draggable landings), a grand staff as engraved (two staves
  with their gap, pen-drawn clefs) writing the last seconds and the notes now, each falling to
  its key; the whammy's pedal in depth; the lit mode legends; the engine switch with each
  engine's latency; the mode's cards; the face `SALTUS` (graphite anodised, chartreuse and
  lilac on a VFD); its pedal on Tabula (the leap motif, its knobs' words).
- **The user's starting preset for every unit** (ADR 0138): *Use as starting preset* /
  *Reset to factory starting preset* in every preset list, kept in `settings.json`, honoured
  by every insert path; the chosen start on a row of its own (nothing elided).
- **Presets**: eighteen, in both engines, styles and roles, levels fitted (`preset_tool
  fit`).
- **Export**: the CLAP/VST3 plug-in (its descriptor from the contract), its ids blessed, its
  DSP named in the tests' direct path, notes through both formats.
- **Docs**: ADR 0135, ADR 0138, `docs/plugins/saltus.md`, the manual (EN and ES: Saltus and
  choosing a starting preset), the AI's unit and effect words, the site's card.
- **Tabula** (shared, marked `fx3:saltus`): the Leap motif; a pedal's extra parameter with a
  long name takes a wider cell or waits for the full editor (never elided).

## Measured

| What | Measured |
|---|---|
| Shift accuracy | within 0.16 cents across four octaves on sines (Phase A) |
| Formants kept | within 1.1 dB, −12 to +12 st, on a sung vowel |
| Level shifted down | within a dB from −12 st |
| Poly | chord partials within 0.01 cent; a real strum's spectrum moved within 0.6 dB |
| Latency | 128 samples (mono), 4 095 (poly), 0 (analog octave) at 48 kHz; at its defaults the whole mix comes out exactly that late |
| Rest | 0.2 – 0.4 s after the signal ends (the family's twins) |
| Presets' levels | moved −1.8 … +3.9 dB by `preset_tool fit` |

## Tests and what each holds

See ADR 0135 *What the tests hold* and ADR 0138. Files:
`cantus-dsp/src/saltus_tests*.rs`; `cantus-core/src/fx/saltus.rs`,
`cantus-core/tests/saltus_commands.rs`, `cantus-core/tests/starting_user.rs`;
`cantus-engine/src/fx/saltus.rs`, `cantus-engine/tests/fx_saltus.rs`, `fx_contract`
(`split::saltus`, `at_rest::saltus`, `a_board_of_one_is_the_unit`), `rt_memory`
(`saltus_never_allocates`); `cantus-ui/tests/saltus_e2e.rs`, `starting_choice_e2e.rs`,
`start_menu_whole.rs`, `saltus_staff` unit tests, the skins' and presets' suites;
`cantus-plugins/tests/notes.rs` (Saltus's case), `ids.rs`, the CLAP/VST3 suites.

## Shared files touched (lines marked `fx3:saltus` or `fx3:start`)

- Core: `starting.rs` (the starting preset; the user's start first), `lib.rs`
  (`starting_user`); tests `undo_invariants.rs` untouched (the fuzz reaches every key).
- Engine: `tests/support/heavy.rs`, `tests/rt_memory.rs`; `cantus-bench/src/kernels.rs`.
- UI: `kit/skin/side.rs` (`SALTUS`, the distinctness lists), `views/fx/frame.rs` (designed
  size; the starting-preset rows, `Chrome::open_presets_at`), `views/fx/mod.rs`
  (`saltus_art`), `views/fx/pedal_art.rs`, `tabula_faces.rs`, `tabula_board.rs` (the Leap
  motif, the knobs' words), `views/fx/tabula.rs` (extras never elided), `settings.rs`,
  `app.rs`, `lib.rs`, `i18n.rs` (the starting choices), `agent_units.rs`,
  `tests/preset_audit.rs` (the floor line removed), `locales/areas/clipfx.*.txt` (the
  neutral-insert note).
- Plug-ins: `tests/common/mod.rs` (`Direct::Saltus`, settings line), `tests/notes.rs`,
  `tests/golden/ids.txt` (appended).
- AI: `tools_mixer.rs` (the effect words).
- Web: `web/src/units_data.rs` (the card; `EFFECTS` 32).

## Verified only in emulation

- The exported plug-in in our own CLAP/VST3 harness, not in a third-party DAW.
- No screen reader was run; the sounds were judged by measurement and rendered files
  (`compartir\saltus\sound\`), not on monitors by the owner yet.
- The starting-preset menu was exercised through its methods and pictured; its clicks were
  not driven by a pointer in a test.

## Left

- Owner-only: listening to the sound set and the presets.
- At the integration: the site's pictures (`web/assets/units/saltus-*.webp`, the units
  reshoot) and the preset excerpts (`render_unit_sounds`), as for the round's other units;
  renumbering ADRs 0135 and 0138.
