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
| Colourways | — | `kit/skin.rs` (`Colourway`, `colourways`, `colourway_of`, `set_colourways`, `swatch`; `of` returns the chosen sheet; the digest keyed by the choices), `kit/menu.rs` + `menu_slate.rs` (`Check::Swatch`), `settings.rs` (`unit_colourways`), `app.rs` (`apply_unit_colourways` in `assemble`, `set_unit_colourway`) |
| Docs | `docs/adr/0130-…`, `docs/plugins/cribrum.md`, this file | manual EN/ES (a section and the list of effects), `web/src/units_data.rs` (the unit card) |

## The owner's notes (Phase B)

1. **Colour: the user's choice — colourways.** The owner, shown three variants, asked for all
   three, selectable. A reusable mechanism of the skin system (ADR 0130 §Colourways, extending
   ADR 0074 §5): a unit's sheet may declare colourways (`skin::Colourway`, `skin::colourways`),
   its default first; the choice is a user preference per unit kind in `settings.json`
   (`unit_colourways`), applied by `CantusApp::assemble` and whenever it changes, chosen in the
   window's ⋯ menu with a swatch per colourway (`menu::Check::Swatch`). Cribrum's: **Nocturne**
   (its face: violet carbon, ice-cyan phosphor, amber), **Brass** (gold anodised, teal VFD,
   brick), **Petrol** (teal-blue lacquer, hot-orange phosphor, lime) — every display dark in
   both themes. Pictures: `compartir\cribrum\design\colour\final\` (`shoot_cribrum --
   <dir> --colourways`). The exported plug-in's window opens in Nocturne and keeps a choice for
   its session (not saved in the plug-in's state).
2. Kept: the grains, the single Filter card, one movement source at a time.
3. The owner's standing note, fine strokes: the response, the rails, the rings, the dots and
   the lanes drawn thinner (1.0–1.4 pt traces, 0.8 pt rails and secondary lines).
4. Defects: (a) the reach is a rail through the chosen filter's own handle, ticked at its ends,
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

## The owner's listening (Phase B)

- The guitar set: everything good but the first auto-wah ("muy abierto, poco wah"). Three
  envelope wahs rendered (classic, deep, funk); he chose **deep**: it is now **Cry wah**
  (rest 250 Hz, Q about 6, attack 12 ms, release 450 ms); *Classic wah* and *Funk quack* are
  presets of their own. Measured on the guitar render (the follower at its default 48 dB
  range): Cry wah rests at about 530 Hz between strums and each strum throws it to about
  1 kHz; the DI guitar sustains, so its level falls only about 8 dB between strums. A new
  `env.range` (6 … 60 dB, default 48 — no change at the default) narrows the levels the
  follower spans, for a wah that closes further between notes (computed from the levels
  measured: at a 12 dB range and a 2.5-octave depth these strums would sweep about 1.8 octaves;
  not rendered); the presets keep the voicing the owner heard and chose.
- Carbonthief's FLACs are **32-bit**, which Cantus's FLAC decoder (`claxon`) refuses
  ("header without bits per sample info"); the renders read WAVs made from them with ffmpeg.
  A sampler user loading such a file would meet the same refusal — noted for the decoder's
  owner.

## Sound examples

`compartir\cribrum\sound\` (the audition sources: a sweep, drums on a synced LFO, a talking
vowel, an envelope wah, each with its dry twin, level-matched within 1 dB) and
`compartir\cribrum\sound\guitar\` (the owner's reference progression on Carbonthief's CC0
DI guitar: `00-guitar-di`, *Cry wah*, *Classic wah*, *Funk quack*, a synced sweep, a vowel, *Gated steps*; README credits
the guitar; the three wahs) — `render_cribrum` and `render_cribrum_guitar` (the samples' folder passed in, never
committed).

## Tests

- `cantus-dsp` `cribrum` (18), `cantus-engine` unit (3), `fx_cribrum` (6), `fx_contract`
  (split and rest for Cribrum), `rt_memory::cribrum_never_allocates`.
- `cantus-core`: the table (4), the starting preset.
- `cantus-ui`: `fx_cribrum` (5), `cribrum_e2e` (1, through `CantusApp::assemble`),
  `cribrum_colourways` (2: the choice through a restart, every colourway's contrast and paint), `demos`,
  `preset_audit`, `preset_render`, `fx_lint`, `skins`, the library (532).
- `cantus-plugins`: see the finishing run below.

## Verified only in emulation / needs the owner

- The sound by ear: the four examples in `compartir\cribrum\sound\` and the presets.
- The exported plug-in's window in a real host (the same editor; tested headless).

## Left

- The site's unit pictures (`web/assets/units/cribrum-*.webp`) come from the `units` reshoot
  (`tools/shots/reshoot.sh units`, which shoots every unit) at the integration; the card's
  sizes are then checked by `tools/shots/check_units.py --fix`.
- The MCP tools list's effect-word sentence still says the five side-round units are "being
  built" (`tools_mixer.rs`, shared by the five): the integrator rewrites it once they land
  (`CANTUS_TOOLS_DUMP=1`).
