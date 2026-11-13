# The effects side round — `cribrum`: Cribrum, the auto filter (ADR 0130, provisional)

Branch `fx/cribrum` from `fx/integration` fd29e27d. Phase A (DSP, engine, design sheet,
sounds) then, after the owner's verdict, Phase B (his notes, the full editor, presets,
end-to-end, export, docs).

## Done

| Layer | Files (owned) | Shared files (additive, marked `// fx:cribrum`) |
|---|---|---|
| Model | `cantus-core/src/fx/cribrum.rs` (the table: 46 globals and 16 curve points, sparse) | `starting.rs` (*Bar sweep*) |
| DSP | `cantus-dsp/src/cribrum.rs`, `cribrum_filter.rs`, `cribrum_mod.rs`, `cribrum_response.rs`, tests `cribrum_tests.rs` | `va_filter.rs` (three read-only accessors: `taps`, `quiet`, `at_rest`) |
| Engine | `cantus-engine/src/fx/cribrum.rs`, `tests/fx_cribrum.rs` | `tests/support/heavy.rs` (heavy settings), `tests/rt_memory.rs` (`cribrum_never_allocates`) |
| UI | `views/fx/cribrum.rs`, `cribrum_display.rs`, `cribrum_move.rs`, `cribrum_curve.rs`, locales `fx/cribrum.{en,es}.txt`, `presets/effects/autofilter.toml`, examples `shoot_cribrum.rs`, `render_cribrum.rs`, tests `fx_cribrum.rs`, `cribrum_e2e.rs` | `views/fx/frame.rs` (window size), `kit/skin/side.rs` (`CRIBRUM` sheet), `agent_units.rs` (the AI's description), `demos/heart/desk.rs` (the arp through *Bar sweep*), `tests/demos.rs` (range to 27), `tests/preset_audit.rs` (the contract's floor line removed) |
| Plug-ins | — | `src/params.rs` (the curve points' group), `tests/common/mod.rs` (Cribrum's `Direct` arm and settings) |
| Docs | `docs/adr/0130-…`, `docs/plugins/cribrum.md`, this file | manual EN/ES (a section and the list of effects), `web/src/units_data.rs` (the unit card) |

## The owner's notes (Phase B)

1. **Colour: three variants** rendered whole (playing, Slate EN and Paper ES at 100 %) in
   `C:\projects\daw-fx\compartir\cribrum\design\colour\` with `colour-variants.png`:
   A *Nocturne* (violet carbon, ice-cyan glass, amber), B *Brass instrument* (gold anodised,
   teal VFD, brick), C *Petrol lacquer* (teal-blue lacquer, hot-orange phosphor, lime). Each
   keeps the contrast promises in every preset (0 failures) and no other unit wears its hue
   and material. `shoot_cribrum -- <dir> --variants` renders them (`skin::set_override`).
   The colour lives only in `side.rs`'s `CRIBRUM` sheet: the pick is a data change there.
2. Kept: the grains, the single Filter card, one movement source at a time.
3. Defects: (a) the reach is a rail through the chosen filter's own handle, ticked at its ends,
   the stretch moved now drawn solid, and only for that filter; (b) the movement's controls
   and depths are two titled, recessed groups filling the row, the cutoff's reach in Hz at the
   depth group's right; (c) Spanish keeps the engineers' words: SVF, Ladder, OTA, Diode ladder,
   Notch, Drive; (d) the lower row lifted: the cards cast shadows, the Filter card (the one in
   hand) deeper with its accent rim, cutoff and mix a size up, the deck lit by the theme's
   glows.

## Measured

See ADR 0130 *What the tests hold*: self-oscillation within ±4.5 cents; drawn = heard within
0.096 dB; aliasing −16.7 / −23.2 / −34.2 dB at 1× / 2× / 4×; vowels within ±2 dB; at rest
after ~1.5 s at heavy settings; the sound examples level-matched within 1 dB of their dry
versions.

## Tests

- `cantus-dsp` `cribrum` (18), `cantus-engine` unit (3), `fx_cribrum` (6), `fx_contract`
  (split and rest for Cribrum), `rt_memory::cribrum_never_allocates`.
- `cantus-core`: the table (4), the starting preset.
- `cantus-ui`: `fx_cribrum` (5), `cribrum_e2e` (1, through `CantusApp::assemble`), `demos`,
  `preset_audit`, `preset_render`, `fx_lint`, `skins`, the library (532).
- `cantus-plugins`: see the finishing run below.

## Verified only in emulation / needs the owner

- The colour pick (three variants waiting).
- The sound by ear: the four examples in `compartir\cribrum\sound\` and the presets.
- The exported plug-in's window in a real host (the same editor; tested headless).

## Left

- The owner's colour pick (swap the `CRIBRUM` sheet's tint, accents, material and display).
- The site's unit pictures (`web/assets/units/cribrum-*.webp`) come from the `units` reshoot
  (`tools/shots/reshoot.sh units`, which shoots every unit) — best run once the colour is
  picked; the card's sizes are then checked by `tools/shots/check_units.py --fix`.
- The MCP tools list's effect-word sentence still says the five side-round units are "being
  built" (`tools_mixer.rs`, shared by the five): the integrator rewrites it once they land
  (`CANTUS_TOOLS_DUMP=1`).
