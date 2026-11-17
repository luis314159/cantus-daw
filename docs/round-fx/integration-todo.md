# The effects side round — the integrator's list

What the builders left for integration, and findings for elsewhere. Struck when done.

## Before the side round closes

- Run the whole suite on `fx/integration` after each merge (Cribrum merged at `122ada78`
  without its last re-run of `cantus-ai`, `cantus-project` and the rest of the UI suite).
- ~~Cribrum's site pictures: `tools/shots/reshoot.sh units`, then `check_units.py --fix`
  (`web/assets/units/cribrum-*.webp` are referenced by the card and missing).~~ — done at the close-out for all five (`shoot_skins` + `units_pictures.py`, `check_units.py --fix`).
- ~~The MCP effect-word sentence in `cantus-ai/src/tools_mixer.rs` still says the five units are
  "being built": rewrite once all five land, `CANTUS_TOOLS_DUMP=1` for the tools golden.~~ — done (`5aa385a7`).
- ~~Colourways (Cribrum's mechanism, ADR 0130 §Colourways, extends ADR 0074 §5): give Fragor
  its palettes B (cream and brass) and C (steel with one red) as colourways, and Lapsus one
  or two, in `kit/skin/side.rs`.~~ — Fragor's done (Cream, Steel; `069e28ea`); Lapsus's left to the owner (`integration.md`, *Open for the owner*).
- Fragor's display: the window's UI pass playing was 34.6 → 7.6 ms mean after its worker
  thread — compare with the frame budget (ADR 0110) on a quiet machine before closing.
- Re-measure the site's LCP on a quiet machine (`side/web` sits at 1.96–2.0 s against 2.0).
- ~~The trivial merge conflicts from rustfmt re-aligning `fx:contract` comments in
  `engine/tests/support/heavy.rs` and `ui/tests/preset_audit.rs`.~~ — resolved; the stale contract comments taken out (`5aa385a7`).
- Renumber at the merge after round 40: schema 62 → after r40's last, ADRs 0130–0133 (four
  ADRs: Vetus and Libra share 0133) — the steps in `integration.md`, *At the merge*.

## Findings for elsewhere (BACKLOG at the merge)

- **Cantus cannot read 32-bit FLAC** (`claxon` refuses it): the CC0 guitar's FLACs are 32-bit;
  Cribrum's builder converted them to WAV to render. A user dropping such a file in Exemplar
  or the browser gets the same error. Owner of the decoder: the sample loading path
  (Exemplar / `cantus-dsp` readers) — decide: another decoder for >24-bit, or a clear message.
- Cribrum's *Cry wah* (the owner's pick, the "deep" voicing) measured on the guitar: rests
  ≈ 530 Hz between strums, each strum lifts it ≈ 1 kHz (about one octave) — the DI sustains,
  so its level falls only ≈ 8 dB between strums. `env.range` (new, default 48 dB = no change)
  can close it further; the owner liked it as heard.

## After the Lapsus merge (dd7b9edf / a3061be4 — 5 833 tests pass, 0 fail, clippy clean)

- ~~**Fragor and Lapsus wear near-identical faces** (`Material::Glass`, amber accent ≈ 36°;
  `kit/skin/side.rs`): each builder thought glass near amber was free because neither saw the
  other. Lapsus's second accent (ice blue 190°) is Cribrum's main hue and its surface (192°)
  Cribrum's Petrol colourway. The owner approved both palettes as seen alone — decide with
  him; give `side.rs` the families' distinctness test (`no_unit_of_any_family_shares_hue_and_
  material_with_ours`) — asked of the vetus builder.~~ — Lapsus v3 (the owner: change its material, keep its colours); the tests in `side.rs`.
- ~~`cantus-bench/src/kernels.rs`: the heavy settings of AutoFilter, Pedal and TimeShaper are
  still the placeholder `("out", 6.0)` — use each unit's `heavy.rs` settings.~~ — done (`00b1076d`).
- ~~The README's unit grid names none of the side round's units.~~ — done (`a817d4bd`).
- ~~`web/src/units_data.rs`: `EFFECTS` is now `[Unit; 28]` (it was 25 while holding 27 cards —
  the web crate is outside the workspace, so nothing caught it): build the site at the end.~~ — `[Unit; 30]`, checked for wasm32 and ssr (`dbc697c6`).
- Lapsus: patterns count 4/4 bars from the song start (a 3/4 song's bars are not followed) —
  a later decision; frame budget not measured; its pre-split drum examples are stale.

## After Lapsus v3 (367b1dbe — matte teal-petrol, ice blue main accent, amber second voice)

- ~~`side.rs` now holds the round's distinctness tests (default faces; sheets' voices). Found
  but not failing (colourways are not yet held to the house rule): **Cribrum's Petrol**
  (lacquer, 24°) is 19° from **Chorda** (lacquer, 43°); **Fragor's B** (brushed, 44°) clashes
  with **Aula** (brushed, 46°) and **Analog** (brushed, 24°) — fix both before those
  colourways ship, then hold colourways to the house rule too.~~ — held and fixed (`069e28ea`; Vetus's Walnut too).
- Lapsus's knobs and selected keys turned ice blue (amber stays the time curve): the owner
  approves or not (v3 sheet with the three side by side in `compartir\lapsus\design\v3\`).

## The close-out (2026-11-14)

Everything above that is not struck is in `integration.md` — *Open for the owner*, *Verified
only in emulation* and *At the merge after round 40* — with the suite's totals.
