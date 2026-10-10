# ADR 0039 — Instrument output calibration: a hidden trim, and the old level written down

**Status:** Accepted · **Date:** 2026-10-09 · **Round:** 24 (beta-rest) · **Extends:** ADR 0006 (project format and migrations), ADR 0023 (native instruments)

> Numbering: 0039 was reserved for this branch in the round-24 brief; the integrator
> renumbers it if another branch took it. Schema 21 likewise.

## Context

`docs/BETA.md` lists, as strongly recommended for the beta, *"instrument gain staging —
Lamina and Exemplar ~8 dB quiet, with a migration"*. Round 19 measured it
(`docs/round19/fixes-mix.md` §4) and left it, because changing an instrument's level changes
the sound of every project saved with it. Measured again on the round's base (a C-major
triad at velocity 100 for two seconds, at unity, nothing on the master —
`instrument_levels.rs::default_patches_staging_report`; every factory preset with
`factory_presets_staging_report`):

| Default patch | Integrated | Peak |
|---|---|---|
| Unda / Calor / Modulus / Gravis / Organum | −15.7 … −20.0 LUFS (median −18.7; −18.95 without Gravis's single bass note) | −7.8 … −11.9 dBFS |
| **Lamina** | **−27.9 LUFS** | **−14.1 dBFS** |
| **Exemplar** (nothing loaded: its placeholder sine at −18 dB) | **−27.2 LUFS** | **−21.6 dBFS** |

Every Lamina factory preset sits with its default (−25 … −29 LUFS; the reed ones −37). The
demos had compensated by hand: their Lamina keys carried +8, +8 and +9 dB of channel trim.
Exemplar's *loaded* instruments are another matter: a sampler plays its samples at their
own level, and its factory presets were balanced by their own `gain` in round 19 — only the
placeholder a fresh Exemplar plays is quiet.

Lamina is also the most dynamic instrument: at velocity 127 its *Ballad* preset peaks at
−5.6 dBFS on the headroom test's dense riff (no factory preset may pass 0 dBFS there), so
it cannot be lifted by the full 8.7 dB its integrated loudness asks for.

Three ways were weighed:

1. **Change the `gain` knob's default** (0 → +X dB) and have the migration store 0 dB where
   nothing was stored. Bit-exact for old projects and no engine change, but a new Lamina
   would show *Gain +5 dB* — a boost that is not one — and lose 5 dB of the knob's range.
2. **Lift the output inside the DSP and lower every stored `gain` by the same amount.** The
   knob reads 0 dB at the calibrated level, but `(g − 5) + 5` is not `g` in `f32` for most
   `g`, a gain near −48 dB clamps, and automation lanes on `gain` would need rewriting —
   saved projects would not render to the same bits.
3. **A hidden output trim whose default is the calibration** — chosen.

## Decision

- **`cantus_core::inst::TRIM_KEY` (`"trim"`, dB)**: a numeric key of a native instrument's
  `NativeSpec`, accepted only by kinds that were recalibrated
  (`InstrumentKind::calibration_db() ≠ 0`) and **not listed** in `params()` — no knob, no
  automation lane, no modulation target. Its default is the calibration: **Lamina +5 dB,
  Exemplar +8 dB**. A new instance stores nothing (sparse, as every key) and so reads the
  calibration.
- **What it lifts.** Lamina: added to the gain stage's dB, the last stage of the voice
  sum (`gain_db + trim`; a linear output gain). Exemplar: added to the placeholder zone's
  −18 dB only; an instrument with a sample, a built-in sound in a zone, an SFZ or a preset
  plays at exactly the level it did.
- **Schema 21**: the migration writes `"trim": 0.0` into every Lamina and Exemplar of a
  project saved before (a trim already there is kept). `gain_db + 0.0` and `−18 + 0.0` are
  the old numbers to the bit, so an old project renders to the same samples.
- **The amounts.** Lamina +5 dB, not +8.7: the most the headroom test allows (*Ballad* now
  −0.6 dBFS at velocity 127). Its default patch is at −22.9 LUFS and −9.1 dBFS peak —
  inside the other instruments' peak range and 4.2 LU under their median loudness (3.95
  under the four synths' −18.95), the rest being Lamina's real dynamics (an electric piano struck at 100 is not a held saw).
  The two reed presets (−37 LUFS) take +5 dB of their own factory `gain` (−27.7 / −26.6
  LUFS; −1.6 / −1.0 dBFS on the riff). Exemplar's placeholder +8 dB: −19.2 LUFS, within
  0.5 LU of the others' median (−18.7).
- **User presets** (bare `NativeSpec` JSON, no version): a preset of a recalibrated kind
  is written with its trim spelled out (`NativeSpec::spell_out_trim`, through
  `views::inst::preset_to_file`), and one read without a trim was written before the
  calibration and gets 0 dB (`NativeSpec::read_unversioned`, through `preset_from_file`;
  a trim equal to the calibration is cleared, so it reads as a new instance) — so a user
  preset keeps its level too.
- **DAWproject** (ADR 0021): a Cantus instrument's device state is the same bare JSON, so
  the export spells the trim out and the import reads the state with the same rule — a
  `.dawproject` exported before schema 21 comes back at its old level. Factory presets are code
  and take the calibration.
- **The demos** took the calibration out of their keys' channel trims (+8/+8/+9 →
  +3/+3/+4 dB), so they sound as they did.
- A later recalibration of any kind uses the same key: raise the default, migrate the old
  value in. No `DSP_VERSION` bump: no unit renders a given spec differently; a spec that
  stores nothing now means something louder, which is what the schema bump says.

## Consequences

- A Lamina or Exemplar placeholder made today is as loud as the rest of the native
  instruments; a project, a user preset or a copied track from before keeps its sound.
- Two Laminas can differ in level for a reason the interface does not show (one migrated,
  one new). The trim is reachable through the command bus (`SetInstrumentParam` with the
  key), so an *upgrade level* action can be added if users ask; choosing any preset or
  resetting the instrument gives the calibrated level.
- An older build refuses a schema-21 project (a newer schema), the usual rule; it would
  otherwise play a new Lamina without its calibration.

## What the tests hold

- `crates/cantus-ui/tests/gain_staging.rs`: `tests/data/v20-quiet-instruments.cantus` was
  written by the base build (schema 20: Lamina at default, Lamina with its gain moved and
  a modulation route on it, Exemplar empty, Exemplar with a built-in zone) and its render
  hashed then; opened now it is migrated, every Lamina/Exemplar holds `trim = 0`, and it
  renders **to the same hash** — saved again at schema 21 and reopened, still the same.
  (With the migration switched off it fails.) A new instance is louder by exactly its
  calibration (peak within 0.05 dB, loudness within 0.1 LU); an Exemplar with something
  loaded is bit-identical with or without a trim; a user preset written before reads at
  0 dB and one written now names its trim; the three demos with Lamina, rebuilt the old
  way, differ by under −70 dB of their peak through their master chains (rounding).
- `crates/cantus-project/tests/dawproject.rs::a_dawproject_from_before_the_calibration_keeps_its_level`:
  a state written without a trim (what the build before wrote) imports at trim 0; one
  exported now comes back as the same spec, at the calibrated level.
- `crates/cantus-ui/tests/instrument_levels.rs`: the headroom test now also plays every
  kind's default patch; nothing passes 0 dBFS.
- `cantus-core` `inst::tests::the_trim_is_hidden_and_only_recalibrated_kinds_have_it`;
  `cantus-project` `format::tests::schema_21_writes_the_old_level_of_lamina_and_exemplar_down`,
  `fixtures/v21.cantus` (a Lamina made now, an Exemplar with its trim written down).
