# The effects side round — the integrator's list

What the builders left for integration, and findings for elsewhere. Struck when done.

## Before the side round closes

- Run the whole suite on `fx/integration` after each merge (Cribrum merged at `122ada78`
  without its last re-run of `cantus-ai`, `cantus-project` and the rest of the UI suite).
- Cribrum's site pictures: `tools/shots/reshoot.sh units`, then `check_units.py --fix`
  (`web/assets/units/cribrum-*.webp` are referenced by the card and missing).
- The MCP effect-word sentence in `cantus-ai/src/tools_mixer.rs` still says the five units are
  "being built": rewrite once all five land, `CANTUS_TOOLS_DUMP=1` for the tools golden.
- Colourways (Cribrum's mechanism, ADR 0130 §Colourways, extends ADR 0074 §5): give Fragor
  its palettes B (cream and brass) and C (steel with one red) as colourways, and Lapsus one
  or two, in `kit/skin/side.rs`.
- Fragor's display: the window's UI pass playing was 34.6 → 7.6 ms mean after its worker
  thread — compare with the frame budget (ADR 0110) on a quiet machine before closing.
- Re-measure the site's LCP on a quiet machine (`side/web` sits at 1.96–2.0 s against 2.0).
- The trivial merge conflicts from rustfmt re-aligning `fx:contract` comments in
  `engine/tests/support/heavy.rs` and `ui/tests/preset_audit.rs`.
- Renumber at the merge after round 40: schema 62 → after r40's last, ADRs 0130–0134.

## Findings for elsewhere (BACKLOG at the merge)

- **Cantus cannot read 32-bit FLAC** (`claxon` refuses it): the CC0 guitar's FLACs are 32-bit;
  Cribrum's builder converted them to WAV to render. A user dropping such a file in Exemplar
  or the browser gets the same error. Owner of the decoder: the sample loading path
  (Exemplar / `cantus-dsp` readers) — decide: another decoder for >24-bit, or a clear message.
- Cribrum's *Cry wah* (the owner's pick, the "deep" voicing) measured on the guitar: rests
  ≈ 530 Hz between strums, each strum lifts it ≈ 1 kHz (about one octave) — the DI sustains,
  so its level falls only ≈ 8 dB between strums. `env.range` (new, default 48 dB = no change)
  can close it further; the owner liked it as heard.
